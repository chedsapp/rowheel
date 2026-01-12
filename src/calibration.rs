use crate::config::{AxisBinding, ButtonBinding, WheelConfig};
use crate::input::InputEvent;
use std::collections::HashMap;

/// The current step in the calibration wizard
#[derive(Debug, Clone, PartialEq)]
pub enum CalibrationStep {
    /// Welcome screen
    Welcome,
    /// Steering wheel calibration - turn left
    SteeringLeft,
    /// Steering wheel calibration - turn right
    SteeringRight,
    /// Throttle calibration - press fully
    ThrottlePressed,
    /// Throttle calibration - release
    ThrottleReleased,
    /// Brake calibration - press fully
    BrakePressed,
    /// Brake calibration - release
    BrakeReleased,
    /// Clutch calibration - press fully (optional)
    ClutchPressed,
    /// Clutch calibration - release (optional)
    ClutchReleased,
    /// Shift up button
    ShiftUp,
    /// Shift down button
    ShiftDown,
    /// Calibration complete
    Complete,
}

impl CalibrationStep {
    pub fn instructions(&self) -> &'static str {
        match self {
            Self::Welcome => "Welcome to RoWheel calibration! Click NEXT to begin.",
            Self::SteeringLeft => "Turn the steering wheel all the way to the LEFT, then click NEXT.",
            Self::SteeringRight => "Turn the steering wheel all the way to the RIGHT, then click NEXT.",
            Self::ThrottlePressed => "Press the THROTTLE pedal all the way down, then click NEXT.",
            Self::ThrottleReleased => "Release the THROTTLE pedal completely, then click NEXT.",
            Self::BrakePressed => "Press the BRAKE pedal all the way down, then click NEXT.",
            Self::BrakeReleased => "Release the BRAKE pedal completely, then click NEXT.",
            Self::ClutchPressed => "Press the CLUTCH pedal all the way down, then click NEXT.\n(Or click SKIP if you don't have a clutch)",
            Self::ClutchReleased => "Release the CLUTCH pedal completely, then click NEXT.",
            Self::ShiftUp => "Press the SHIFT UP button/paddle, then click NEXT.",
            Self::ShiftDown => "Press the SHIFT DOWN button/paddle, then click NEXT.",
            Self::Complete => "Calibration complete! Your configuration has been saved.",
        }
    }

    pub fn can_skip(&self) -> bool {
        matches!(self, Self::ClutchPressed | Self::ClutchReleased)
    }

    pub fn next(&self) -> Self {
        match self {
            Self::Welcome => Self::SteeringLeft,
            Self::SteeringLeft => Self::SteeringRight,
            Self::SteeringRight => Self::ThrottlePressed,
            Self::ThrottlePressed => Self::ThrottleReleased,
            Self::ThrottleReleased => Self::BrakePressed,
            Self::BrakePressed => Self::BrakeReleased,
            Self::BrakeReleased => Self::ClutchPressed,
            Self::ClutchPressed => Self::ClutchReleased,
            Self::ClutchReleased => Self::ShiftUp,
            Self::ShiftUp => Self::ShiftDown,
            Self::ShiftDown => Self::Complete,
            Self::Complete => Self::Complete,
        }
    }

    pub fn skip_clutch(&self) -> Self {
        if *self == Self::ClutchPressed || *self == Self::ClutchReleased {
            Self::ShiftUp
        } else {
            self.next()
        }
    }
}

/// Tracks axis movement during calibration to find the most moved axis
#[derive(Debug, Clone)]
struct AxisTracker {
    device_id: String,
    device_name: String,
    axis_code: u32,
    initial_value: f32,
    current_value: f32,
}

impl AxisTracker {
    fn movement(&self) -> f32 {
        (self.current_value - self.initial_value).abs()
    }
}

/// Calibration state machine
pub struct CalibrationWizard {
    pub step: CalibrationStep,
    pub config: WheelConfig,

    // Tracking for axis calibration
    axis_trackers: HashMap<(String, u32), AxisTracker>,
    captured_axis: Option<(String, String, u32, f32)>, // device_id, device_name, axis_code, value

    // Tracking for button calibration
    captured_button: Option<(String, String, u32)>, // device_id, device_name, button_code
}

impl CalibrationWizard {
    pub fn new(existing_config: Option<WheelConfig>) -> Self {
        Self {
            step: CalibrationStep::Welcome,
            config: existing_config.unwrap_or_default(),
            axis_trackers: HashMap::new(),
            captured_axis: None,
            captured_button: None,
        }
    }

    /// Process an input event during calibration
    pub fn process_event(&mut self, event: &InputEvent) {
        match event {
            InputEvent::AxisMoved { device_id, device_name, axis_code, value } => {
                let key = (device_id.clone(), *axis_code);

                if let Some(tracker) = self.axis_trackers.get_mut(&key) {
                    tracker.current_value = *value;
                } else {
                    self.axis_trackers.insert(key, AxisTracker {
                        device_id: device_id.clone(),
                        device_name: device_name.clone(),
                        axis_code: *axis_code,
                        initial_value: *value,
                        current_value: *value,
                    });
                }
            }
            InputEvent::ButtonPressed { device_id, device_name, button_code } => {
                self.captured_button = Some((
                    device_id.clone(),
                    device_name.clone(),
                    *button_code,
                ));
            }
            _ => {}
        }
    }

    /// Get the most moved axis since tracking started
    fn get_most_moved_axis(&self) -> Option<&AxisTracker> {
        self.axis_trackers.values()
            .max_by(|a, b| a.movement().partial_cmp(&b.movement()).unwrap())
            .filter(|a| a.movement() > 0.1) // Minimum movement threshold
    }

    /// Capture the current axis position for the current step
    pub fn capture_axis_position(&mut self) {
        if let Some(tracker) = self.get_most_moved_axis() {
            self.captured_axis = Some((
                tracker.device_id.clone(),
                tracker.device_name.clone(),
                tracker.axis_code,
                tracker.current_value,
            ));
        }
    }

    /// Advance to the next step, saving calibration data
    pub fn advance(&mut self) {
        self.capture_axis_position();

        match self.step {
            CalibrationStep::SteeringLeft => {
                if let Some((device_id, device_name, axis_code, value)) = self.captured_axis.take() {
                    self.config.steering = Some(AxisBinding {
                        device_id,
                        device_name,
                        axis_code,
                        min_value: value,
                        max_value: value, // Will be updated in SteeringRight
                        inverted: false,
                    });
                }
            }
            CalibrationStep::SteeringRight => {
                if let (Some(ref mut steering), Some((_, _, _, value))) =
                    (&mut self.config.steering, self.captured_axis.take())
                {
                    steering.max_value = value;
                    // Check if inverted (left should be less than right for -1 to 1 mapping)
                    if steering.min_value > steering.max_value {
                        std::mem::swap(&mut steering.min_value, &mut steering.max_value);
                        steering.inverted = true;
                    }
                }
            }
            CalibrationStep::ThrottlePressed => {
                if let Some((device_id, device_name, axis_code, value)) = self.captured_axis.take() {
                    self.config.throttle = Some(AxisBinding {
                        device_id,
                        device_name,
                        axis_code,
                        min_value: value, // Will be swapped if needed
                        max_value: value,
                        inverted: false,
                    });
                }
            }
            CalibrationStep::ThrottleReleased => {
                if let (Some(ref mut throttle), Some((_, _, _, value))) =
                    (&mut self.config.throttle, self.captured_axis.take())
                {
                    // Released position is the "min" (0%), pressed is "max" (100%)
                    let pressed_value = throttle.max_value;
                    throttle.min_value = value;
                    throttle.max_value = pressed_value;

                    // Ensure proper direction
                    if throttle.min_value > throttle.max_value {
                        std::mem::swap(&mut throttle.min_value, &mut throttle.max_value);
                        throttle.inverted = true;
                    }
                }
            }
            CalibrationStep::BrakePressed => {
                if let Some((device_id, device_name, axis_code, value)) = self.captured_axis.take() {
                    self.config.brake = Some(AxisBinding {
                        device_id,
                        device_name,
                        axis_code,
                        min_value: value,
                        max_value: value,
                        inverted: false,
                    });
                }
            }
            CalibrationStep::BrakeReleased => {
                if let (Some(ref mut brake), Some((_, _, _, value))) =
                    (&mut self.config.brake, self.captured_axis.take())
                {
                    let pressed_value = brake.max_value;
                    brake.min_value = value;
                    brake.max_value = pressed_value;

                    if brake.min_value > brake.max_value {
                        std::mem::swap(&mut brake.min_value, &mut brake.max_value);
                        brake.inverted = true;
                    }
                }
            }
            CalibrationStep::ClutchPressed => {
                if let Some((device_id, device_name, axis_code, value)) = self.captured_axis.take() {
                    self.config.clutch = Some(AxisBinding {
                        device_id,
                        device_name,
                        axis_code,
                        min_value: value,
                        max_value: value,
                        inverted: false,
                    });
                }
            }
            CalibrationStep::ClutchReleased => {
                if let (Some(ref mut clutch), Some((_, _, _, value))) =
                    (&mut self.config.clutch, self.captured_axis.take())
                {
                    let pressed_value = clutch.max_value;
                    clutch.min_value = value;
                    clutch.max_value = pressed_value;

                    if clutch.min_value > clutch.max_value {
                        std::mem::swap(&mut clutch.min_value, &mut clutch.max_value);
                        clutch.inverted = true;
                    }
                }
            }
            CalibrationStep::ShiftUp => {
                if let Some((device_id, device_name, button_code)) = self.captured_button.take() {
                    self.config.shift_up = Some(ButtonBinding {
                        device_id,
                        device_name,
                        button_code,
                    });
                }
            }
            CalibrationStep::ShiftDown => {
                if let Some((device_id, device_name, button_code)) = self.captured_button.take() {
                    self.config.shift_down = Some(ButtonBinding {
                        device_id,
                        device_name,
                        button_code,
                    });
                }
            }
            CalibrationStep::Complete => {
                // Save config
                if let Err(e) = self.config.save() {
                    log::error!("Failed to save config: {}", e);
                }
            }
            _ => {}
        }

        // Reset trackers for next step
        self.axis_trackers.clear();
        self.captured_axis = None;
        self.captured_button = None;

        self.step = self.step.next();
    }

    /// Skip the current step (for optional steps)
    pub fn skip(&mut self) {
        self.axis_trackers.clear();
        self.captured_axis = None;
        self.captured_button = None;

        self.step = self.step.skip_clutch();
    }

    /// Get info about the detected axis for display
    pub fn get_detected_axis_info(&self) -> Option<String> {
        self.get_most_moved_axis().map(|tracker| {
            format!(
                "{} - Axis {} (movement: {:.2})",
                tracker.device_name,
                tracker.axis_code,
                tracker.movement()
            )
        })
    }

    /// Get info about the detected button for display
    pub fn get_detected_button_info(&self) -> Option<String> {
        self.captured_button.as_ref().map(|(_, name, code)| {
            format!("{} - Button {}", name, code)
        })
    }

    /// Check if we're in a step that needs axis detection
    pub fn needs_axis_detection(&self) -> bool {
        matches!(
            self.step,
            CalibrationStep::SteeringLeft
                | CalibrationStep::SteeringRight
                | CalibrationStep::ThrottlePressed
                | CalibrationStep::ThrottleReleased
                | CalibrationStep::BrakePressed
                | CalibrationStep::BrakeReleased
                | CalibrationStep::ClutchPressed
                | CalibrationStep::ClutchReleased
        )
    }

    /// Check if we're in a step that needs button detection
    pub fn needs_button_detection(&self) -> bool {
        matches!(
            self.step,
            CalibrationStep::ShiftUp | CalibrationStep::ShiftDown
        )
    }
}
