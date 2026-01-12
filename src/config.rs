use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const CONFIG_FILENAME: &str = "rowheel_config.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AxisBinding {
    /// The device UUID or identifier
    pub device_id: String,
    /// Human-readable device name
    pub device_name: String,
    /// The axis code/index on the device
    pub axis_code: u32,
    /// Minimum value observed during calibration
    pub min_value: f32,
    /// Maximum value observed during calibration
    pub max_value: f32,
    /// Whether to invert the axis
    pub inverted: bool,
}

impl AxisBinding {
    /// Normalize a raw value to -1.0..1.0 range based on calibration
    pub fn normalize(&self, raw_value: f32) -> f32 {
        let range = self.max_value - self.min_value;
        if range.abs() < 0.001 {
            return 0.0;
        }
        let normalized = (raw_value - self.min_value) / range * 2.0 - 1.0;
        let normalized = normalized.clamp(-1.0, 1.0);
        if self.inverted {
            -normalized
        } else {
            normalized
        }
    }

    /// Normalize a raw value to 0.0..1.0 range (for triggers)
    pub fn normalize_trigger(&self, raw_value: f32) -> f32 {
        let range = self.max_value - self.min_value;
        if range.abs() < 0.001 {
            return 0.0;
        }
        let normalized = (raw_value - self.min_value) / range;
        let normalized = normalized.clamp(0.0, 1.0);
        if self.inverted {
            1.0 - normalized
        } else {
            normalized
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ButtonBinding {
    /// The device UUID or identifier
    pub device_id: String,
    /// Human-readable device name
    pub device_name: String,
    /// The button code on the device
    pub button_code: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WheelConfig {
    /// Steering wheel axis (maps to Left Stick X)
    pub steering: Option<AxisBinding>,
    /// Throttle pedal (maps to Right Trigger)
    pub throttle: Option<AxisBinding>,
    /// Brake pedal (maps to Left Trigger)
    pub brake: Option<AxisBinding>,
    /// Clutch pedal (maps to Left Stick Y)
    pub clutch: Option<AxisBinding>,
    /// Shift up button (maps to Y button)
    pub shift_up: Option<ButtonBinding>,
    /// Shift down button (maps to X button)
    pub shift_down: Option<ButtonBinding>,
    /// Device ID to use for force feedback output
    pub force_feedback_device: Option<String>,
}

impl WheelConfig {
    pub fn load() -> Option<Self> {
        let path = Self::config_path();
        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(contents) => match serde_json::from_str(&contents) {
                    Ok(config) => {
                        log::info!("Loaded config from {:?}", path);
                        return Some(config);
                    }
                    Err(e) => {
                        log::error!("Failed to parse config: {}", e);
                    }
                },
                Err(e) => {
                    log::error!("Failed to read config file: {}", e);
                }
            }
        }
        None
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::config_path();
        let contents = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, contents)?;
        log::info!("Saved config to {:?}", path);
        Ok(())
    }

    pub fn config_path() -> PathBuf {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."))
            .join(CONFIG_FILENAME)
    }

    pub fn is_complete(&self) -> bool {
        self.steering.is_some()
            && self.throttle.is_some()
            && self.brake.is_some()
            && self.shift_up.is_some()
            && self.shift_down.is_some()
    }

    pub fn is_partially_configured(&self) -> bool {
        self.steering.is_some()
            || self.throttle.is_some()
            || self.brake.is_some()
            || self.clutch.is_some()
            || self.shift_up.is_some()
            || self.shift_down.is_some()
    }
}
