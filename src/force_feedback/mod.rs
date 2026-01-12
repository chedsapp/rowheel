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
pub trait ForceFeedback: Send {
    /// Apply force feedback based on rumble state from virtual controller
    /// large_motor -> left constant force
    /// small_motor -> right constant force
    fn apply_rumble(&mut self, rumble: &RumbleState) -> anyhow::Result<()>;

    /// Stop all force feedback effects
    fn stop(&mut self) -> anyhow::Result<()>;

    /// Check if the device is available
    fn is_available(&self) -> bool;
}
