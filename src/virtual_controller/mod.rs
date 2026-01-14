#[cfg(target_os = "linux")]
mod uinput_ffi;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::VirtualXboxController;
#[cfg(windows)]
pub use windows::VirtualXboxController;

#[derive(Debug, Clone, Default)]
pub struct XboxControllerState {
    pub left_stick_x: f32,
    pub left_stick_y: f32,
    pub right_stick_x: f32,
    pub right_stick_y: f32,
    pub left_trigger: f32,
    pub right_trigger: f32,
    pub buttons: XboxButtons,
}

#[derive(Debug, Clone, Default)]
pub struct XboxButtons {
    pub a: bool,
    pub b: bool,
    pub x: bool,
    pub y: bool,
    pub left_bumper: bool,
    pub right_bumper: bool,
    pub back: bool,
    pub start: bool,
    pub guide: bool,
    pub left_thumb: bool,
    pub right_thumb: bool,
    pub dpad_up: bool,
    pub dpad_down: bool,
    pub dpad_left: bool,
    pub dpad_right: bool,
}

#[derive(Debug, Clone, Default)]
pub struct RumbleState {
    pub large_motor: f32,
    pub small_motor: f32,
}
pub trait VirtualController: Send {
    fn update(&mut self, state: &XboxControllerState) -> anyhow::Result<()>;
    fn get_rumble(&mut self) -> anyhow::Result<RumbleState>;
    fn is_connected(&self) -> bool;
}
