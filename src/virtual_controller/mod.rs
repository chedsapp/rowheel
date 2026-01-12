#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::VirtualXboxController;
#[cfg(windows)]
pub use windows::VirtualXboxController;

/// Xbox controller state to be emitted
#[derive(Debug, Clone, Default)]
pub struct XboxControllerState {
    /// Left stick X axis (-1.0 to 1.0)
    pub left_stick_x: f32,
    /// Left stick Y axis (-1.0 to 1.0)
    pub left_stick_y: f32,
    /// Right stick X axis (-1.0 to 1.0)
    pub right_stick_x: f32,
    /// Right stick Y axis (-1.0 to 1.0)
    pub right_stick_y: f32,
    /// Left trigger (0.0 to 1.0)
    pub left_trigger: f32,
    /// Right trigger (0.0 to 1.0)
    pub right_trigger: f32,
    /// Button states
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

/// Rumble feedback received from the virtual controller
#[derive(Debug, Clone, Default)]
pub struct RumbleState {
    /// Large motor strength (0.0 to 1.0)
    pub large_motor: f32,
    /// Small motor strength (0.0 to 1.0)
    pub small_motor: f32,
}

/// Trait for virtual Xbox controller implementations
pub trait VirtualController: Send {
    fn update(&mut self, state: &XboxControllerState) -> anyhow::Result<()>;
    fn get_rumble(&mut self) -> anyhow::Result<RumbleState>;
    fn is_connected(&self) -> bool;
}
