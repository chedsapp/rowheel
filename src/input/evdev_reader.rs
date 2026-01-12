#![cfg(target_os = "linux")]
use crossbeam_channel::Receiver;
use evdev::{Device, EventType, AbsoluteAxisCode, AbsInfo};
use std::collections::HashMap;
use std::path::Path;
use std::thread;

pub enum EvdevEvent {
    AxisMoved { axis_code: u16, value: f32 },
    Disconnected,
}

pub struct EvdevReader {
    pub receiver: Receiver<EvdevEvent>,
}

impl EvdevReader {
    pub fn new(path: &Path) -> anyhow::Result<Self> {
        let (sender, receiver) = crossbeam_channel::unbounded();
        let path = path.to_path_buf();

        thread::spawn(move || {
            let mut device = match Device::open(&path) {
                Ok(device) => device,
                Err(e) => {
                    log::error!("Failed to open evdev device {}: {}", path.display(), e);
                    return;
                }
            };

            let abs_axes: HashMap<u16, AbsInfo> = if let Ok(info) = device.get_absinfo() {
                info.map(|(axis, info)| (axis.0, info)).collect()
            } else {
                HashMap::new()
            };

            loop {
                match device.fetch_events() {
                    Ok(events) => {
                        for event in events {
                            if event.event_type() == EventType::ABSOLUTE {
                                let axis = AbsoluteAxisCode(event.code());
                                let raw_value = event.value();

                                if let Some(info) = abs_axes.get(&axis.0) {
                                    let min = info.minimum();
                                    let max = info.maximum();

                                    if max != min {
                                        let value = (2.0 * (raw_value - min) as f32 / (max - min) as f32) - 1.0;
                                        if sender.send(EvdevEvent::AxisMoved { axis_code: axis.0, value }).is_err() {
                                            return;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(std::time::Duration::from_millis(10));
                        continue;
                    }
                    Err(e) => {
                        log::error!("Failed to fetch evdev events: {}", e);
                        let _ = sender.send(EvdevEvent::Disconnected);
                        return;
                    }
                }
            }
        });

        Ok(Self { receiver })
    }
}