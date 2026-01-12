use super::{AxisInfo, ButtonInfo, InputDevice, InputEvent, InputState};
use gilrs::{Axis, Button, EventType, Gilrs, GamepadId};
use std::collections::HashMap;

#[cfg(target_os = "linux")]
use super::evdev_reader::{EvdevReader, EvdevEvent};
#[cfg(target_os = "linux")]
use std::path::PathBuf;

pub struct InputReader {
    gilrs: Gilrs,
    state: InputState,
    devices: HashMap<String, InputDevice>,
    #[cfg(target_os = "linux")]
    udev: libudev::Context,
    #[cfg(target_os = "linux")]
    evdev_readers: HashMap<GamepadId, crossbeam_channel::Receiver<EvdevEvent>>,
}

impl InputReader {
    pub fn new() -> anyhow::Result<Self> {
        let gilrs = Gilrs::new().map_err(|e| anyhow::anyhow!("Failed to initialize gilrs: {}", e))?;

        let mut reader = Self {
            gilrs,
            state: InputState::default(),
            devices: HashMap::new(),
            #[cfg(target_os = "linux")]
            udev: libudev::Context::new()?,
            #[cfg(target_os = "linux")]
            evdev_readers: HashMap::new(),
        };

        reader.refresh_devices();

        Ok(reader)
    }

    fn refresh_devices(&mut self) {
        self.devices.clear();
        #[cfg(target_os = "linux")]
        self.evdev_readers.clear();

        for (id, gamepad) in self.gilrs.gamepads() {
            let device_id = format!("{:?}", id);
            let device = InputDevice {
                id: device_id.clone(),
                name: gamepad.name().to_string(),
                axes: self.get_axes_info(&gamepad),
                buttons: Self::get_buttons_info(&gamepad),
                has_force_feedback: gamepad.is_ff_supported(),
            };

            log::info!("Found device: {} ({}) - FF: {}",
                device.name, device_id, device.has_force_feedback);
            
            #[cfg(target_os = "linux")]
            if let Some(path) = self.find_device_path(&gamepad) {
                log::info!("Found evdev path for {}: {}", gamepad.name(), path.display());
                if let Ok(reader) = EvdevReader::new(&path) {
                    self.evdev_readers.insert(id, reader.receiver);
                } else {
                    log::error!("Failed to create EvdevReader for {}", gamepad.name());
                }
            } else {
                log::warn!("No evdev path found for {}", gamepad.name());
            }

            self.devices.insert(device_id, device);
        }
    }

    #[cfg(target_os = "linux")]
    fn find_device_path(&self, gamepad: &gilrs::Gamepad) -> Option<PathBuf> {
        log::info!("Searching for device path for: {} (Vendor: {:?}, Product: {:?})", gamepad.name(), gamepad.vendor_id(), gamepad.product_id());
        let mut enumerator = libudev::Enumerator::new(&self.udev).ok()?;
        enumerator.match_subsystem("input").ok()?;

        for device in enumerator.scan_devices().ok()? {
            log::debug!("Found udev device: {:?}", device.syspath());

            let vendor_id = device
                .property_value("ID_VENDOR_ID")
                .and_then(|s| s.to_str())
                .and_then(|s| u16::from_str_radix(s, 16).ok());

            let product_id = device
                .property_value("ID_PRODUCT_ID")
                .or(device.property_value("ID_MODEL_ID"))
                .and_then(|s| s.to_str())
                .and_then(|s| u16::from_str_radix(s, 16).ok());
            
            log::debug!("  udev vendor: {:?}, product: {:?}", vendor_id, product_id);
            log::debug!("  gilrs vendor: {:?}, product: {:?}", gamepad.vendor_id(), gamepad.product_id());

            if vendor_id == gamepad.vendor_id() && product_id == gamepad.product_id() {
                 if let Some(devnode) = device.devnode() {
                    log::info!("Found matching device with devnode: {}", devnode.to_string_lossy());
                    if devnode.to_string_lossy().contains("event") {
                        return Some(devnode.to_path_buf());
                    }
                }
            }
        }
        
        log::warn!("Device path not found for: {}", gamepad.name());
        None
    }

    fn get_axes_info(&self, gamepad: &gilrs::Gamepad) -> Vec<AxisInfo> {
        let mut axes = Vec::new();

        let axis_list = [
            (Axis::LeftStickX, "Left Stick X"),
            (Axis::LeftStickY, "Left Stick Y"),
            (Axis::RightStickX, "Right Stick X"),
            (Axis::RightStickY, "Right Stick Y"),
            (Axis::LeftZ, "Left Z"),
            (Axis::RightZ, "Right Z"),
            (Axis::DPadX, "DPad X"),
            (Axis::DPadY, "DPad Y"),
            (Axis::Unknown, "Unknown"),
        ];

        for (axis, name) in axis_list {
            if gamepad.axis_data(axis).is_some() {
                axes.push(AxisInfo {
                    code: axis as u32,
                    name: name.to_string(),
                });
            }
        }

        axes
    }

    fn get_buttons_info(gamepad: &gilrs::Gamepad) -> Vec<ButtonInfo> {
        let mut buttons = Vec::new();

        let button_list = [
            (Button::South, "South (A)"),
            (Button::East, "East (B)"),
            (Button::North, "North (Y)"),
            (Button::West, "West (X)"),
            (Button::LeftTrigger, "Left Bumper"),
            (Button::RightTrigger, "Right Bumper"),
            (Button::LeftTrigger2, "Left Trigger"),
            (Button::RightTrigger2, "Right Trigger"),
            (Button::Select, "Select"),
            (Button::Start, "Start"),
            (Button::LeftThumb, "Left Thumb"),
            (Button::RightThumb, "Right Thumb"),
            (Button::DPadUp, "DPad Up"),
            (Button::DPadDown, "DPad Down"),
            (Button::DPadLeft, "DPad Left"),
            (Button::DPadRight, "DPad Right"),
            (Button::Mode, "Mode"),
        ];

        for (button, name) in button_list {
            if gamepad.button_data(button).is_some() {
                buttons.push(ButtonInfo {
                    code: button as u32,
                    name: name.to_string(),
                });
            }
        }

        buttons
    }

    pub fn devices(&self) -> &HashMap<String, InputDevice> {
        &self.devices
    }

    pub fn state(&self) -> &InputState {
        &self.state
    }

    pub fn poll(&mut self) -> Vec<InputEvent> {
        let mut events = Vec::new();
        let mut refresh_needed = false;

        while let Some(event) = self.gilrs.next_event() {
            match event.event {
                #[cfg(not(target_os = "linux"))]
                EventType::AxisChanged(axis, value, _) => {
                    let device_id = format!("{:?}", event.id);
                    let gamepad = self.gilrs.gamepad(event.id);
                    let device_name = gamepad.name().to_string();
                    let axis_code = axis as u32;

                    self.state
                        .axes
                        .entry(device_id.clone())
                        .or_default()
                        .insert(axis_code, value);

                    events.push(InputEvent::AxisMoved {
                        device_id,
                        device_name,
                        axis_code,
                        value,
                    });
                }
                EventType::ButtonPressed(_button, code) => {
                    let device_id = format!("{:?}", event.id);
                    let gamepad = self.gilrs.gamepad(event.id);
                    let device_name = gamepad.name().to_string();
                    let button_code = code.into_u32();

                    self.state
                        .buttons
                        .entry(device_id.clone())
                        .or_default()
                        .insert(button_code, true);

                    events.push(InputEvent::ButtonPressed {
                        device_id,
                        device_name,
                        button_code,
                    });
                }
                EventType::ButtonReleased(_button, code) => {
                    let device_id = format!("{:?}", event.id);
                    let gamepad = self.gilrs.gamepad(event.id);
                    let device_name = gamepad.name().to_string();
                    let button_code = code.into_u32();

                    self.state
                        .buttons
                        .entry(device_id.clone())
                        .or_default()
                        .insert(button_code, false);

                    events.push(InputEvent::ButtonReleased {
                        device_id,
                        device_name,
                        button_code,
                    });
                }
                EventType::Connected => {
                    refresh_needed = true;
                    if let Some(device) = self.devices.get(&format!("{:?}", event.id)) {
                        events.push(InputEvent::DeviceConnected {
                            device: device.clone(),
                        });
                    }
                }
                EventType::Disconnected => {
                    events.push(InputEvent::DeviceDisconnected { device_id: format!("{:?}", event.id) });
                    refresh_needed = true;
                }
                _ => {}
            }
        }

        #[cfg(target_os = "linux")]
        {
            let mut disconnected_readers = Vec::new();
            for (id, rx) in &self.evdev_readers {
                while let Ok(ev) = rx.try_recv() {
                    let device_id = format!("{:?}", id);
                    let gamepad = self.gilrs.gamepad(*id);
                    let device_name = gamepad.name().to_string();

                    match ev {
                        EvdevEvent::AxisMoved { axis_code, value } => {
                            let axis_code = axis_code as u32;
                            self.state
                                .axes
                                .entry(device_id.clone())
                                .or_default()
                                .insert(axis_code, value);

                            events.push(InputEvent::AxisMoved {
                                device_id,
                                device_name,
                                axis_code,
                                value,
                            });
                        }
                        EvdevEvent::Disconnected => {
                            events.push(InputEvent::DeviceDisconnected { device_id: device_id.clone() });
                            disconnected_readers.push(*id);
                            refresh_needed = true;
                        }
                    }
                }
            }
            for id in disconnected_readers {
                self.evdev_readers.remove(&id);
            }
        }

        if refresh_needed {
            self.refresh_devices();
        }

        events
    }
}