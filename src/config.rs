use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const CONFIG_FILENAME: &str = "rowheel_config.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AxisBinding {
    /// UUID for windows and device path for linux
    pub device_id: String,
    pub device_name: String,
    pub axis_code: u32,
    pub min_value: f32,
    pub max_value: f32,
    pub inverted: bool,
}

impl AxisBinding {
    /// Normalize to -1.0..1.0 range based on calibration
    pub fn normalize(&self, raw_value: f32) -> f32 {
        let range = (self.max_value - self.min_value) as f64;
        if range.abs() < 0.001 {
            return 0.0;
        }
        let normalized = ((raw_value - self.min_value) as f64) / range * 2.0 - 1.0;
        let normalized = normalized.clamp(-1.0, 1.0);
        let result = if self.inverted {
            -normalized
        } else {
            normalized
        };
        result as f32
    }

    /// Normalize between 0 and 1 just in case scaling is weird
    pub fn normalize_trigger(&self, raw_value: f32) -> f32 {
        let range = (self.max_value - self.min_value) as f64;
        if range.abs() < 0.001 {
            return 0.0;
        }
        let normalized = ((raw_value - self.min_value) as f64) / range;
        let normalized = normalized.clamp(0.0, 1.0);
        let result = if self.inverted {
            1.0 - normalized
        } else {
            normalized
        };
        result as f32
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ButtonBinding {
    /// UUID for windows and device path for linux
    pub device_id: String,
    pub device_name: String,
    pub button_code: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WheelConfig {
    pub steering: Option<AxisBinding>,
    pub throttle: Option<AxisBinding>,
    pub brake: Option<AxisBinding>,
    pub clutch: Option<AxisBinding>,
    pub shift_up: Option<ButtonBinding>,
    pub shift_down: Option<ButtonBinding>,
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

}
