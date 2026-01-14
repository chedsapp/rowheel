mod reader;
#[cfg(target_os = "linux")]
pub mod evdev_reader;

pub use reader::*;

use std::collections::HashMap;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct InputDevice {
    pub id: String,
    pub name: String,
    pub axes: Vec<AxisInfo>,
    pub buttons: Vec<ButtonInfo>,
    pub has_force_feedback: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct AxisInfo {
    pub code: u32,
    pub name: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ButtonInfo {
    pub code: u32,
    pub name: String,
}

#[derive(Debug, Clone, Default)]
pub struct InputState {
    /// device_id -> axis_code -> current value
    pub axes: HashMap<String, HashMap<u32, f32>>,
    /// Same thing but with buttons
    pub buttons: HashMap<String, HashMap<u32, bool>>,
}

impl InputState {
    pub fn get_axis(&self, device_id: &str, axis_code: u32) -> Option<f32> {
        self.axes.get(device_id)?.get(&axis_code).copied()
    }

    pub fn get_button(&self, device_id: &str, button_code: u32) -> Option<bool> {
        self.buttons.get(device_id)?.get(&button_code).copied()
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum InputEvent {
    AxisMoved {
        device_id: String,
        device_name: String,
        axis_code: u32,
        value: f32,
    },
    ButtonPressed {
        device_id: String,
        device_name: String,
        button_code: u32,
    },
    ButtonReleased {
        device_id: String,
        device_name: String,
        button_code: u32,
    },
    DeviceConnected {
        device: InputDevice,
    },
    DeviceDisconnected {
        device_id: String,
    },
}
