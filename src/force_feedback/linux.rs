use super::{ForceFeedback, RumbleState};
use evdev::{Device, FFEffect, FFEffectData, FFEffectKind, FFReplay, FFTrigger, FFEnvelope};
use std::path::Path;

pub struct ForceFeedbackDevice {
    device: Option<Device>,
    available: bool,
    constant_effect: Option<FFEffect>,
    effect_playing: bool,
}

impl ForceFeedbackDevice {
    pub fn new(device_path: Option<&str>) -> anyhow::Result<Self> {
        let mut ff_device = Self {
            device: None,
            available: false,
            constant_effect: None,
            effect_playing: false,
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

        if let Some(supported_ff) = device.supported_ff() {
            let ff_types: Vec<_> = supported_ff.iter().collect();
            log::info!("Device supports {} FF effect types: {:?}", ff_types.len(), ff_types);

            if ff_types.is_empty() {
                return Err(anyhow::anyhow!("Device {} does not support force feedback", path));
            }
        } else {
            return Err(anyhow::anyhow!("Device {} does not support force feedback", path));
        }

        log::info!("Connected to force feedback device: {} at {}", device.name().unwrap_or("Unknown"), path);
        self.device = Some(device);
        self.available = true;

        self.create_constant_effect()?;

        Ok(())
    }

    fn create_constant_effect(&mut self) -> anyhow::Result<()> {
        let device = self.device.as_mut()
            .ok_or_else(|| anyhow::anyhow!("No device available"))?;

        let effect_data = FFEffectData {
            direction: 16384, // East 0x4000
            trigger: FFTrigger::default(),
            replay: FFReplay {
                length: 0,
                delay: 0,
            },
            kind: FFEffectKind::Constant {
                level: 0,
                envelope: FFEnvelope {
                    attack_length: 0,
                    attack_level: 0,
                    fade_length: 0,
                    fade_level: 0,
                },
            },
        };

        match device.upload_ff_effect(effect_data) {
            Ok(effect) => {
                log::info!("Created constant force effect with ID: {}", effect.id());
                self.constant_effect = Some(effect);
                Ok(())
            }
            Err(e) => {
                log::warn!("Failed to create constant force effect: {}", e);
                Err(anyhow::anyhow!("Failed to create constant force effect: {}", e))
            }
        }
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

                        // Create the constant force effect
                        if let Err(e) = self.create_constant_effect() {
                            log::warn!("Failed to create constant force effect for auto-detected device: {}", e);
                            self.device = None;
                            self.available = false;
                            continue;
                        }

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

        // Ensure the constant effect is initialized
        if self.constant_effect.is_none() {
            log::debug!("Constant force effect not yet created, creating now.");
            self.create_constant_effect()?;
        }

        // Convert Xbox controller motor values to constant force
        // large_motor (0.0-1.0) → rightward force (positive)
        // small_motor (0.0-1.0) → leftward force (negative)
        // Combined: level = (large - small) * 32767
        //
        // Examples:
        //   large=1.0, small=0.0 → level=+32767 (full right)
        //   large=0.0, small=1.0 → level=-32767 (full left)
        //   large=0.5, small=0.0 → level=+16383 (half right)
        //   large=0.0, small=0.0 → level=0 (no force)
        let large_clamped = rumble.large_motor.clamp(0.0, 1.0);
        let small_clamped = rumble.small_motor.clamp(0.0, 1.0);
        let force_level = ((large_clamped - small_clamped) * 32767.0) as i16;

        log::debug!("Applying constant force to wheel: large={:.3}, small={:.3} → level={}",
                   rumble.large_motor, rumble.small_motor, force_level);

        if let Some(ref mut effect) = self.constant_effect {
            // Update the effect with the new force level
            let effect_data = FFEffectData {
                direction: 16384, // East (0x4000) - horizontal axis for steering wheel
                trigger: FFTrigger::default(),
                replay: FFReplay {
                    length: 0, // Infinite duration
                    delay: 0,
                },
                kind: FFEffectKind::Constant {
                    level: force_level,
                    envelope: FFEnvelope {
                        attack_length: 0,
                        attack_level: 0, // No envelope modulation
                        fade_length: 0,
                        fade_level: 0,
                    },
                },
            };

            // Update the effect with new parameters
            effect.update(effect_data)
                .map_err(|e| anyhow::anyhow!("Failed to update constant force effect: {}", e))?;

            // Start the effect only once (play(1) = play once, but with length=0 it runs indefinitely)
            if !self.effect_playing {
                log::info!("Starting constant force effect playback");
                effect.play(1)
                    .map_err(|e| anyhow::anyhow!("Failed to play constant force effect: {}", e))?;
                self.effect_playing = true;
            }
        }

        Ok(())
    }

    fn stop(&mut self) -> anyhow::Result<()> {
        if !self.available {
            return Ok(());
        }

        // Stop and drop the effect if it exists
        if let Some(mut effect) = self.constant_effect.take() {
            effect.stop()
                .map_err(|e| anyhow::anyhow!("Failed to stop effect: {}", e))?;

            // Effect is automatically erased when dropped
            log::debug!("Stopped constant force effect");
        }

        self.effect_playing = false;
        Ok(())
    }

    fn is_available(&self) -> bool {
        self.available
    }
}

impl Drop for ForceFeedbackDevice {
    fn drop(&mut self) {
        if let Err(e) = self.stop() {
            log::warn!("Failed to clean up force feedback on drop: {}", e);
        }
    }
}
