#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::ForceFeedbackDevice;
#[cfg(windows)]
pub use windows::ForceFeedbackDevice;

use super::virtual_controller::RumbleState;

/// Trait for force feedback output to physical devices
///
/// Data flow:
/// 1. Game sends rumble commands to virtual Xbox controller
/// 2. Virtual controller receives rumble notifications
/// 3. Rumble values are converted to steering wheel force feedback
///
/// Motor mapping for steering wheels:
/// - large_motor (Xbox rumble) → right side of steering wheel
/// - small_motor (Xbox rumble) → left side of steering wheel
pub trait ForceFeedback: Send {
    /// Apply force feedback based on rumble state from virtual controller
    ///
    /// For steering wheels:
    /// - rumble.large_motor → right side force
    /// - rumble.small_motor → left side force
    fn apply_rumble(&mut self, rumble: &RumbleState) -> anyhow::Result<()>;

    /// Stop all force feedback effects
    fn stop(&mut self) -> anyhow::Result<()>;

    /// Check if the device is available
    fn is_available(&self) -> bool;
}
