use super::{ForceFeedback, RumbleState};
use evdev::Device;
use std::path::Path;

pub struct ForceFeedbackDevice {
    device: Option<Device>,
    available: bool,
}

impl ForceFeedbackDevice {
    pub fn new(device_path: Option<&str>) -> anyhow::Result<Self> {
        let mut ff_device = Self {
            device: None,
            available: false,
        };

        if let Some(path) = device_path {
            ff_device.connect(path)?;
        } else {
            ff_device.auto_detect()?;
        }

        Ok(ff_device)
    }

    fn connect(&mut self, path: &str) -> anyhow::Result<()> {
        let device = Device::open(path)
            .map_err(|e| anyhow::anyhow!("Failed to open device {}: {}", path, e))?;

        if !device.supported_ff().map(|ff| ff.iter().count() > 0).unwrap_or(false) {
            return Err(anyhow::anyhow!("Device {} does not support force feedback", path));
        }

        log::info!("Connected to force feedback device: {} at {}", device.name().unwrap_or("Unknown"), path);
        self.device = Some(device);
        self.available = true;
        Ok(())
    }

    fn auto_detect(&mut self) -> anyhow::Result<()> {
        for i in 0..32 {
            let path = format!("/dev/input/event{}", i);
            if Path::new(&path).exists() {
                if let Ok(device) = Device::open(&path) {
                    if device.supported_ff().map(|ff| ff.iter().count() > 0).unwrap_or(false) {
                        log::info!("Auto-detected FF device: {} at {}",
                            device.name().unwrap_or("Unknown"), path);
                        self.device = Some(device);
                        self.available = true;
                        return Ok(());
                    }
                }
            }
        }

        log::warn!("No force feedback device found");
        Ok(())
    }
}

impl ForceFeedback for ForceFeedbackDevice {
    fn apply_rumble(&mut self, rumble: &RumbleState) -> anyhow::Result<()> {
        if !self.available {
            return Ok(());
        }

        // TODO: Implement proper force feedback using evdev FFEffectData
        // For now, just log significant rumble values
        if rumble.large_motor > 0.1 || rumble.small_motor > 0.1 {
            log::trace!("FF: large={:.2}, small={:.2}", rumble.large_motor, rumble.small_motor);
        }

        Ok(())
    }

    fn stop(&mut self) -> anyhow::Result<()> {
        // TODO: Stop any running effects
        Ok(())
    }

    fn is_available(&self) -> bool {
        self.available
    }
}

impl Drop for ForceFeedbackDevice {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
