use super::{AxisInfo, ButtonInfo, InputDevice, InputEvent, InputState};
use gilrs::{Axis, Button, EventType, Gilrs, GilrsBuilder};
#[cfg(target_os = "linux")]
use gilrs::GamepadId;
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
        // We need to turn off the gilrs filters because it applies this incredibly dumbass 0.05 deadzone (thank u jasiah)
        let gilrs = GilrsBuilder::new()
            .with_default_filters(false)
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to initialize gilrs: {}", e))?;

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
            
            let has_ff = gamepad.is_ff_supported();

            let device = InputDevice {
                id: device_id.clone(),
                name: gamepad.name().to_string(),
                axes: self.get_axes_info(&gamepad),
                buttons: Self::get_buttons_info(&gamepad),
                has_force_feedback: has_ff,
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
        use std::ffi::CStr;
        use std::os::unix::io::AsRawFd;

        const EVIOCGNAME_LEN: usize = 256;

        log::info!("Searching for device path for: {} (Vendor: {:?}, Product: {:?})",
                   gamepad.name(), gamepad.vendor_id(), gamepad.product_id());

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

        log::debug!("Udev search failed, trying direct evdev scan for: {}", gamepad.name());

        let target_name = gamepad.name();
        for entry in std::fs::read_dir("/dev/input").ok()? {
            let entry = entry.ok()?;
            let path = entry.path();

            if !path.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("event"))
                .unwrap_or(false)
            {
                continue;
            }

            if let Ok(file) = std::fs::File::open(&path) {
                let fd = file.as_raw_fd();
                let mut name_buf = [0u8; EVIOCGNAME_LEN];

                let result = unsafe {
                    libc::ioctl(
                        fd,
                        (0x80004506 | ((EVIOCGNAME_LEN as u64) << 16)) as libc::c_ulong,
                        name_buf.as_mut_ptr()
                    )
                };

                if result >= 0 {
                    if let Ok(device_name) = CStr::from_bytes_until_nul(&name_buf) {
                        if let Ok(device_name_str) = device_name.to_str() {
                            log::debug!("Checking device {}: name=\"{}\"", path.display(), device_name_str);

                            if device_name_str == target_name {
                                log::info!("Found matching device via direct scan: {}", path.display());
                                return Some(path);
                            }
                        }
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
            (Axis::LeftZ, "Left Z / Brake"),
            (Axis::RightStickX, "Right Stick X"),
            (Axis::RightStickY, "Right Stick Y"),
            (Axis::RightZ, "Right Z / Gas"),
            (Axis::DPadX, "DPad X"),
            (Axis::DPadY, "DPad Y"),
            (Axis::Unknown, "Unknown"),
        ];

        for (axis, name) in axis_list {
            if let Some(data) = gamepad.axis_data(axis) {
                let code = gamepad.axis_code(axis)
                    .map(|c| c.into_u32())
                    .unwrap_or(axis as u32);
                log::info!("  Found axis: {} (code: {}, value: {})", name, code, data.value());

                axes.push(AxisInfo {
                    code,
                    name: name.to_string(),
                });
            }
        }

        // On Windows, also check for analog buttons that act as axes (Thank you hackerkm for noticing this btw...)
        // These appear to come through as ButtonChanged events with analog values
        #[cfg(not(target_os = "linux"))]
        {
            let analog_button_list = [
                (Button::LeftTrigger2, "Left Trigger / Slider"),
                (Button::RightTrigger2, "Right Trigger / Slider"),
                (Button::C, "C / Slider"),
                (Button::Z, "Z / Slider"),
                (Button::Unknown, "Unknown Slider"),
            ];

            for (button, name) in analog_button_list {
                if let Some(data) = gamepad.button_data(button) {
                    // Check if this button has analog values (not just 0/1)
                    let value = data.value();
                    if let Some(raw_code) = gamepad.button_code(button) {
                        // Use high bit to distinguish from regular axes
                        let code = raw_code.into_u32() | 0x80000000;
                        log::info!("  Found analog button/slider: {} (code: {:#x}, value: {})",
                                   name, code, value);

                        axes.push(AxisInfo {
                            code,
                            name: format!("{} (Slider)", name),
                        });
                    }
                }
            }
        }

        log::info!("Device {} has {} axes", gamepad.name(), axes.len());
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
            (Button::Unknown, "Unknown"),
            (Button::C, "C"),
            (Button::Z, "Z"),
        ];

        for (button, name) in button_list {
            if let Some(data) = gamepad.button_data(button) {
                let code = button as u32;
                log::info!("  Found button: {} (code: {}, pressed: {})", name, code, data.is_pressed());
                buttons.push(ButtonInfo {
                    code,
                    name: name.to_string(),
                });
            }
        }

        log::info!("Device {} has {} buttons", gamepad.name(), buttons.len());
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
            log::trace!("Gilrs event: {:?}", event.event);

            match event.event {
                #[cfg(not(target_os = "linux"))]
                EventType::AxisChanged(axis, value, code) => {
                    let device_id = format!("{:?}", event.id);
                    let gamepad = self.gilrs.gamepad(event.id);
                    let device_name = gamepad.name().to_string();
                    // Use raw code for better compatibility with sliders and non-standard axes
                    let axis_code = code.into_u32();

                    log::debug!("Axis changed: {:?} (raw code: {}) = {} on {}",
                               axis, axis_code, value, device_name);

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
                // Handle analog buttons (sliders, triggers) as axes on Windows
                // gilrs often maps sliders to ButtonChanged events with analog values
                #[cfg(not(target_os = "linux"))]
                EventType::ButtonChanged(button, value, code) => {
                    let device_id = format!("{:?}", event.id);
                    let gamepad = self.gilrs.gamepad(event.id);
                    let device_name = gamepad.name().to_string();
                    // Use raw code with high bit set to distinguish from regular axes
                    let axis_code = code.into_u32() | 0x80000000;

                    log::debug!("Analog button/slider: {:?} (raw code: {}, mapped: {:#x}) = {} on {}",
                               button, code.into_u32(), axis_code, value, device_name);

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
                EventType::ButtonPressed(button, code) => {
                    let device_id = format!("{:?}", event.id);
                    let gamepad = self.gilrs.gamepad(event.id);
                    let device_name = gamepad.name().to_string();
                    let button_code = code.into_u32();

                    log::debug!("Button pressed: {:?} (code: {}) on {}", button, button_code, device_name);

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
                EventType::ButtonReleased(button, code) => {
                    let device_id = format!("{:?}", event.id);
                    let gamepad = self.gilrs.gamepad(event.id);
                    let device_name = gamepad.name().to_string();
                    let button_code = code.into_u32();

                    log::debug!("Button released: {:?} (code: {}) on {}", button, button_code, device_name);

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