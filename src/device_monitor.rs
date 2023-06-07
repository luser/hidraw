use anyhow::{anyhow, bail, Context as ErrorContext, Result};
use futures::Future;
use futures_util::StreamExt;
use log::{debug, info, warn};
use std::collections::HashSet;
use std::convert::TryInto;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc::Sender;
use tokio::task::LocalSet;
use tokio_udev::{AsyncMonitorSocket, Device, Enumerator, EventType, MonitorBuilder};

use crate::report::HidReportParser;

/// From Linux uapi/linux/input.h
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Bus {
    Usb = 0x03,
    Bluetooth = 0x05,
}

const EVENT_MINOR_BASE: usize = 64;

#[derive(Clone, Debug)]
pub struct DeviceInfo {
    pub sys_path: PathBuf,
    pub device_node: PathBuf,
    pub parser: Option<HidReportParser>,
    pub bus: Bus,
    pub name: String,
    pub version: u16,
    pub vendor_id: u16,
    pub product_id: u16,
}

#[derive(Debug)]
pub enum DeviceEvent {
    Added(DeviceInfo),
    Removed(PathBuf),
}

fn get_integer_prop(device: &Device, prop_name: &'static str) -> Result<u16> {
    Ok(u16::from_str_radix(get_prop(device, prop_name)?, 16)?)
}

fn get_prop<'dev>(device: &'dev Device, prop_name: &'static str) -> Result<&'dev str> {
    let raw_prop = device
        .property_value(prop_name)
        .with_context(|| anyhow!("Missing property: {prop_name}"))?;
    Ok(raw_prop
        .to_str()
        .with_context(|| anyhow!("Bad string value"))?)
}

async fn get_device_info(device: &Device) -> Result<DeviceInfo> {
    let sys_path = device.syspath().to_owned();
    debug!("get_device_info({sys_path:?})");
    let device_node = device.devnode().context("Missing device node")?.to_owned();
    if device.property_value("ID_INPUT_JOYSTICK").is_none() {
        bail!("Not a gamepad: {sys_path:?}");
    }
    // input/jsN have minors 0+, input/eventN have minors 64+
    if get_prop(device, "MINOR")?.parse::<usize>()? < EVENT_MINOR_BASE {
        bail!("Skipping old js device");
    }
    let vendor_id = get_integer_prop(device, "ID_VENDOR_ID")?;
    let product_id = get_integer_prop(device, "ID_MODEL_ID")?;
    let version = get_integer_prop(device, "ID_REVISION")?;
    let bus = get_prop(device, "ID_BUS")?;
    let bus = match bus {
        "usb" => Bus::Usb,
        "Bluetooth" => Bus::Bluetooth,
        b @ _ => bail!("Unknown bus: {b}"),
    };
    let name = get_prop(device, "ID_MODEL")?.to_owned();

    Ok(DeviceInfo {
        sys_path,
        device_node,
        parser: None,
        bus,
        name,
        version,
        vendor_id,
        product_id,
    })
}

async fn monitor_devices_internal(tx: Sender<DeviceEvent>) -> Result<()> {
    info!("Starting monitor_devices_internal");
    // We don't care about all devices, so keep track of the ones we do care about.
    let mut devices = HashSet::new();
    let mut enumerator = Enumerator::new()?;
    enumerator.match_subsystem("input")?;
    enumerator.match_is_initialized()?;
    enumerator.match_property("ID_INPUT_JOYSTICK", "1")?;
    for device in enumerator.scan_devices()? {
        match get_device_info(&device).await {
            Ok(info) => {
                devices.insert(info.sys_path.clone());
                tx.send(DeviceEvent::Added(info)).await?;
            }
            //TODO: better error handling
            Err(e) => {
                debug!("{e}");
            }
        }
    }

    let builder = MonitorBuilder::new()?;
    let mut monitor: AsyncMonitorSocket = builder.match_subsystem("input")?.listen()?.try_into()?;

    while let Some(event) = monitor.next().await {
        let event = event?;
        let syspath = event.syspath();
        match event.event_type() {
            EventType::Add => {
                // Check device type
                match get_device_info(&event).await {
                    Ok(info) => {
                        devices.insert(info.sys_path.clone());
                        tx.send(DeviceEvent::Added(info)).await?;
                    }
                    //TODO: better error handling
                    Err(e) => {
                        debug!("{e}");
                    }
                }
            }
            EventType::Remove => {
                if devices.remove(syspath) {
                    tx.send(DeviceEvent::Removed(syspath.to_owned())).await?;
                } else {
                    //TODO: better error handling
                    warn!("Remove event for unknown device: {:?}", syspath);
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// Monitor connected hidraw devices via udev.
///
/// Send a DeviceEvent::Added for each gamepad device that is added, and a matching
/// DeviceEvent::Removed for each gamepad device that was previously added but has now
/// been removed.
pub fn monitor_devices(tx: Sender<DeviceEvent>) -> impl Future<Output = ()> {
    info!("Starting monitor_devices");
    // The tokio-udev types are !Send, so we need to run them on a LocalSet.
    let local = LocalSet::new();
    local.spawn_local(monitor_devices_internal(tx));
    local
}
