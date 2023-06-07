use anyhow::Result;
use env_logger::Builder;
use log::{info, LevelFilter};
use std::collections::HashMap;
use tokio::sync::mpsc;

use hidraw::device;
use hidraw::device_monitor::{self, DeviceEvent, DeviceInfo};

fn log_info(info: &DeviceInfo) {
    info!(
        "New device `{}` {:04x}:{:04x} on {:?} ({:?})",
        info.name, info.vendor_id, info.product_id, info.bus, info.device_node
    );
}

#[tokio::main]
async fn main() -> Result<()> {
    Builder::new()
        .filter_level(LevelFilter::Debug)
        .format_module_path(false)
        .format_target(false)
        .parse_default_env()
        .init();
    info!("Starting");
    let mut devices = HashMap::new();
    // Spawn a task to monitor devices via udev.
    let (tx, mut rx) = mpsc::channel(4);
    let mut local_set = device_monitor::monitor_devices(tx);
    loop {
        tokio::select! {
            Some(event) =  rx.recv() => {
                match event {
                    DeviceEvent::Added(info) => {
                        log_info(&info);
                        let (tx, rx) = mpsc::channel(4);
                        devices.insert(info.sys_path.clone(), tx);
                        tokio::task::spawn(device::watch_one_device(info, rx));
                    }
                    DeviceEvent::Removed(sys_path) => {
                        if let Some(tx) = devices.remove(&sys_path) {
                            tx.send(()).await?;
                        }
                    }
                }
            }
            _ = &mut local_set => {}
            else => break,
        };
    }
    info!("Shutting down");

    Ok(())
}
