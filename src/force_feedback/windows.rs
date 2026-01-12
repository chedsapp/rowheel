use super::{ForceFeedback, RumbleState};

// Windows force feedback will use DirectInput
// For now, we'll use a simplified implementation
// Full implementation requires DirectInput FFB API

pub struct ForceFeedbackDevice {
    available: bool,
    // In a full implementation, this would hold:
    // - IDirectInputDevice8 for the force feedback device
    // - Effect handles for constant force effects
}

impl ForceFeedbackDevice {
    pub fn new(_device_id: Option<&str>) -> anyhow::Result<Self> {
        // TODO: Implement DirectInput force feedback initialization
        // This requires:
        // 1. Creating a DirectInput instance
        // 2. Enumerating devices
        // 3. Creating the device and setting cooperative level
        // 4. Acquiring the device
        // 5. Creating constant force effects

        log::warn!("Windows force feedback not yet implemented - requires DirectInput FFB");

        Ok(Self {
            available: false,
        })
    }
}

impl ForceFeedback for ForceFeedbackDevice {
    fn apply_rumble(&mut self, rumble: &RumbleState) -> anyhow::Result<()> {
        if !self.available {
            return Ok(());
        }

        // TODO: Update DirectInput constant force effect
        // Convert rumble.large_motor to left force
        // Convert rumble.small_motor to right force
        let _ = rumble;

        Ok(())
    }

    fn stop(&mut self) -> anyhow::Result<()> {
        if !self.available {
            return Ok(());
        }

        // TODO: Stop DirectInput force feedback effects

        Ok(())
    }

    fn is_available(&self) -> bool {
        self.available
    }
}
