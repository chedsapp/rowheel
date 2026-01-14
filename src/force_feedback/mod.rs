#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod directinput_ffi;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::ForceFeedbackDevice;
#[cfg(windows)]
pub use windows::ForceFeedbackDevice;

use super::virtual_controller::RumbleState;

pub trait ForceFeedback: Send {

    fn apply_rumble(&mut self, rumble: &RumbleState) -> anyhow::Result<()>;
    fn stop(&mut self) -> anyhow::Result<()>;
    fn is_available(&self) -> bool;
    
}
