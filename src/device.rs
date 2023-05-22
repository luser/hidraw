use anyhow::Result;
use log::info;
use tokio::fs::OpenOptions;
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc::Receiver;

use crate::device_monitor::DeviceInfo;

pub async fn watch_one_device(info: DeviceInfo, mut stop_rx: Receiver<()>) -> Result<()> {
    info!("Starting task for `{:?}`", &info.device_node);
    let mut hidraw_file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&info.device_node)
        .await?;
    let mut report = [0; 64];
    loop {
        tokio::select! {
            _ =  stop_rx.recv() => break,
            Ok(n) = hidraw_file.read(&mut report) => {
                info!("Read {n} byte report: {:x?}", &report[..n]);
            }
            else => break,
        };
    }
    info!("Stopping task for `{:?}`", &info.device_node);
    Ok(())
}
