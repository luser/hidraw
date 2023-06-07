use anyhow::Result;
use libc::input_event;
use log::info;
use tokio::fs::OpenOptions;
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc::Receiver;

use crate::device_monitor::DeviceInfo;

pub async fn watch_one_device(info: DeviceInfo, mut stop_rx: Receiver<()>) -> Result<()> {
    info!("Starting task for `{:?}`", &info.device_node);
    let mut evdev_file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&info.device_node)
        .await?;

    let mut event_buf = [0; std::mem::size_of::<input_event>()];
    loop {
        tokio::select! {
            _ =  stop_rx.recv() => break,
            Ok(_) = evdev_file.read_exact(&mut event_buf) => {
                let event: input_event = unsafe { std::mem::transmute(event_buf) };
                info!("Read event: {:x?}", event);
            }
            else => break,
        };
    }
    info!("Stopping task for `{:?}`", &info.device_node);
    Ok(())
}
