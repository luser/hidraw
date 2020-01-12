use anyhow::{anyhow, bail, Context as ErrorContext, Result};
use futures_util::stream::StreamExt;
use nix::ioctl_read;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs::OpenOptions;
use tokio_udev::{Context, Device, Enumerator, EventType, MonitorBuilder};

mod report;

use report::HidReportParser;

// From Linux uapi/linux/input.h
const USB: u16 = 3;
const BLUETOOTH: u16 = 5;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Bus {
    Usb,
    Bluetooth,
}

// From uapi/linux/hid.h
const HID_MAX_DESCRIPTOR_SIZE: usize = 4096;

// From uapi/linux/hidraw.h
#[repr(C)]
pub struct hidraw_report_descriptor {
    size: u32,
    value: [u8; HID_MAX_DESCRIPTOR_SIZE],
}

ioctl_read!(hid_read_descriptor_size, b'H', 0x01, libc::c_int);
ioctl_read!(hid_read_descriptor, b'H', 0x02, hidraw_report_descriptor);

/// The start of a HID descriptor with usage page: generic desktop, usage: gamepad.
const GAMEPAD_USAGE: [u8; 4] = [0x05, 0x01, 0x09, 0x05];
/// The start of a HID descriptor with usage page: generic desktop, usage: joystick.
const JOYSTICK_USAGE: [u8; 4] = [0x05, 0x01, 0x09, 0x04];

#[derive(Clone, Debug)]
struct DeviceInfo {
    device_node: PathBuf,
    parser: Option<HidReportParser>,
    hid_descriptor: Vec<u8>,
    bus: Bus,
    name: String,
    serial: String,
    vendor_id: u16,
    product_id: u16,
}

fn spliteq(s: &str) -> Option<(&str, &str)> {
    let mut bits = s.splitn(2, '=');
    Some((bits.next()?, bits.next()?))
}

/// Parse uevent's `HID_ID` string into bus type, vendor id, product id.
///
/// Returns `None` for unsupported bus types.
fn parse_id(id: &str) -> Option<(Bus, u16, u16)> {
    let mut bits = id.splitn(3, ':');
    let bus = match u16::from_str_radix(bits.next()?, 16).ok()? {
        BLUETOOTH => Bus::Bluetooth,
        USB => Bus::Usb,
        _ => return None,
    };
    let vid = u16::from_str_radix(bits.next()?, 16).ok()?;
    let pid = u16::from_str_radix(bits.next()?, 16).ok()?;
    Some((bus, vid, pid))
}

async fn get_device_info(device: &Device) -> Result<DeviceInfo> {
    let device_node = device.devnode().context("Missing device node")?.to_owned();
    let hid_parent = device.parent_with_subsystem("hid")?
        .context("Couldn't find HID parent device")?;
    let uevent = hid_parent.attribute_value("uevent")
        .context("Couldn't read uevent attribute")?
    // I believe all strings exposed in the HID driver are supposed to be UTF-8
    // but it probably bears verification! In any event, the worst that's likely to happen
    // here is unicode replacement characters in the device name.
        .to_string_lossy();
    let mut id = None;
    let mut name = None;
    let mut serial = None;
    for line in uevent.lines() {
        if let Some((key, value)) = spliteq(line) {
            match key {
                "HID_ID" => id = parse_id(value),
                "HID_NAME" => name = Some(value.to_owned()),
                "HID_UNIQ" => serial = Some(value.to_owned()),
                _ => {},
            }
        }
    }
    if let (Some((bus, vendor_id, product_id)), Some(name), Some(serial)) = (id, name, serial) {
        let hid_descriptor = get_hid_descriptor(&device_node).await?;
        // Very rough test. Doesn't work for devices that report multiple HID types.
        // TODO: actually parse HID descriptors.
        if !hid_descriptor.starts_with(&GAMEPAD_USAGE) && !hid_descriptor.starts_with(&JOYSTICK_USAGE) {
            bail!("Not a gamepad");
        }
        let parser = report::find_report_parser_for_device(vendor_id, product_id);
        Ok(DeviceInfo {
            device_node,
            parser,
            hid_descriptor,
            bus,
            name,
            serial,
            vendor_id,
            product_id,
        })
    } else {
        Err(anyhow!("Couldn't find enough info"))
    }
}

async fn get_hid_descriptor(device_node: &Path) -> Result<Vec<u8>> {
    // Adapted from https://stackoverflow.com/a/51904291/69326
    use std::mem::MaybeUninit;
    use std::os::unix::io::AsRawFd;

    let mut options = OpenOptions::new();
    let file = options.read(true).write(true).open(device_node).await?;
    let fd = file.as_raw_fd();

    let desc_raw = unsafe {
        let mut size = 0;
        hid_read_descriptor_size(fd, &mut size)?;

        let mut desc_raw: MaybeUninit<hidraw_report_descriptor> = MaybeUninit::uninit();
        let mut desc_ptr = desc_raw.as_mut_ptr();
        (*desc_ptr).size = size as u32;
        hid_read_descriptor(file.as_raw_fd(), desc_ptr)?;
        desc_raw.assume_init()
    };
    Ok(desc_raw.value[..desc_raw.size as usize].to_owned())
}

fn log_info(info: &DeviceInfo) {
    println!("New device `{}` {:04x}:{:04x} on {:?} ({:?})", info.name,
             info.vendor_id, info.product_id, info.bus, info.device_node);
    println!("HID descriptor is {} bytes: have parser: {}", info.hid_descriptor.len(),
             info.parser.is_some());
}

#[derive(Debug, Clone)]
struct DeviceInput;

#[tokio::main]
async fn main() -> Result<()> {
    let mut devices = HashMap::new();

    let context = Context::new()?;
    let mut enumerator = Enumerator::new(&context)?;
    enumerator.match_subsystem("hidraw")?;
    for device in enumerator.scan_devices()? {
        match get_device_info(&device).await {
            Ok(info) => {
                log_info(&info);
                devices.insert(device.syspath().to_owned(), info);
            },
            Err(_) => {},
        }
    }

    let mut builder = MonitorBuilder::new(&context)?;
    //NOTE: hidg for gadget devices
    builder.match_subsystem("hidraw")?;
    let mut monitor = builder.listen()?;

    while let Some(event) = monitor.next().await {
        let syspath = event.syspath();
        match event.event_type() {
            EventType::Add => {
                // Check device type
                match get_device_info(&event).await {
                    Ok(info) => {
                        log_info(&info);
                        devices.insert(syspath.to_owned(), info);
                    },
                    Err(_) => {},
                }
            }
            EventType::Remove => {
                if let Some(info) = devices.remove(syspath) {
                    println!("Device `{}` removed", info.name);
                } else {
                    println!("Remove event for unknown device: {:?}", syspath);
                }
            }
            _ => {}
        }
    }
    Ok(())
}
