use crate::calibration::{CalibrationStep, CalibrationWizard};
use crate::config::WheelConfig;
use crate::force_feedback::{ForceFeedback, ForceFeedbackDevice};
use crate::input::InputReader;
use crate::virtual_controller::{VirtualController, VirtualXboxController, XboxControllerState};
use eframe::egui;

#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    /// Calibration wizard
    Calibrating,
    /// Normal operation - mapping inputs to virtual controller
    Running,
}

pub struct RoWheelApp {
    mode: AppMode,
    config: Option<WheelConfig>,
    input_reader: Option<InputReader>,
    virtual_controller: Option<Box<dyn VirtualController>>,
    force_feedback: Option<Box<dyn ForceFeedback>>,
    calibration: Option<CalibrationWizard>,

    // UI state
    detected_input_info: String,
    status_message: String,
    show_debug: bool,

    // Running state debug info
    current_state: XboxControllerState,
}

impl RoWheelApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Set up custom fonts if needed
        let fonts = egui::FontDefinitions::default();
        cc.egui_ctx.set_fonts(fonts);

        // Try to load existing config
        let config = WheelConfig::load();
        let has_config = config.as_ref().map(|c| c.is_complete()).unwrap_or(false);

        // Initialize input reader
        let input_reader = match InputReader::new() {
            Ok(reader) => Some(reader),
            Err(e) => {
                log::error!("Failed to initialize input reader: {}", e);
                None
            }
        };

        let mode = if has_config {
            AppMode::Running
        } else {
            AppMode::Calibrating
        };

        let calibration = if mode == AppMode::Calibrating {
            Some(CalibrationWizard::new(config.clone()))
        } else {
            None
        };

        // Initialize virtual controller (only when running)
        let virtual_controller: Option<Box<dyn VirtualController>> = if mode == AppMode::Running {
            match VirtualXboxController::new() {
                Ok(vc) => Some(Box::new(vc)),
                Err(e) => {
                    log::error!("Failed to create virtual controller: {}", e);
                    None
                }
            }
        } else {
            None
        };

        Self {
            mode,
            config,
            input_reader,
            virtual_controller,
            force_feedback: None,
            calibration,
            detected_input_info: String::new(),
            status_message: String::new(),
            show_debug: false,
            current_state: XboxControllerState::default(),
        }
    }

    fn start_calibration(&mut self) {
        self.mode = AppMode::Calibrating;
        self.calibration = Some(CalibrationWizard::new(self.config.clone()));
        // Disconnect virtual controller during calibration
        self.virtual_controller = None;
    }

    fn finish_calibration(&mut self) {
        if let Some(ref calibration) = self.calibration {
            self.config = Some(calibration.config.clone());

            // Initialize virtual controller
            match VirtualXboxController::new() {
                Ok(vc) => {
                    self.virtual_controller = Some(Box::new(vc));
                    self.status_message = "Gamepad connected".to_string();
                }
                Err(e) => {
                    self.status_message = format!("Failed to create gamepad: {}", e);
                    log::error!("{}", self.status_message);
                }
            }

            // Initialize force feedback
            match ForceFeedbackDevice::new(None) {
                Ok(ff) => {
                    if ff.is_available() {
                        self.force_feedback = Some(Box::new(ff));
                        log::info!("Force feedback initialized");
                    }
                }
                Err(e) => {
                    log::warn!("Force feedback not available: {}", e);
                }
            }
        }

        self.calibration = None;
        self.mode = AppMode::Running;
    }

    fn process_inputs(&mut self) {
        let Some(ref mut reader) = self.input_reader else {
            return;
        };

        let events = reader.poll();

        // During calibration, feed events to wizard
        if let Some(ref mut calibration) = self.calibration {
            for event in &events {
                calibration.process_event(event);
            }

            // Update detected input display
            if calibration.needs_axis_detection() {
                self.detected_input_info = calibration
                    .get_detected_axis_info()
                    .unwrap_or_else(|| "Move an input...".to_string());
            } else if calibration.needs_button_detection() {
                self.detected_input_info = calibration
                    .get_detected_button_info()
                    .unwrap_or_else(|| "Press a button...".to_string());
            }
        }

        // During running mode, map inputs to virtual controller
        if self.mode == AppMode::Running {
            if let Some(ref config) = self.config {
                let state = reader.state();

                let mut xbox_state = XboxControllerState::default();

                // Map steering to left stick X
                if let Some(ref steering) = config.steering {
                    if let Some(value) = state.get_axis(&steering.device_id, steering.axis_code) {
                        xbox_state.left_stick_x = steering.normalize(value);
                    }
                }

                // Map clutch to left stick Y
                if let Some(ref clutch) = config.clutch {
                    if let Some(value) = state.get_axis(&clutch.device_id, clutch.axis_code) {
                        xbox_state.left_stick_y = clutch.normalize(value);
                    }
                }

                // Map throttle to right trigger
                if let Some(ref throttle) = config.throttle {
                    if let Some(value) = state.get_axis(&throttle.device_id, throttle.axis_code) {
                        xbox_state.right_trigger = throttle.normalize_trigger(value);
                    }
                }

                // Map brake to left trigger
                if let Some(ref brake) = config.brake {
                    if let Some(value) = state.get_axis(&brake.device_id, brake.axis_code) {
                        xbox_state.left_trigger = brake.normalize_trigger(value);
                    }
                }

                // Map shift buttons
                if let Some(ref shift_up) = config.shift_up {
                    if let Some(pressed) = state.get_button(&shift_up.device_id, shift_up.button_code) {
                        xbox_state.buttons.y = pressed;
                    }
                }

                if let Some(ref shift_down) = config.shift_down {
                    if let Some(pressed) = state.get_button(&shift_down.device_id, shift_down.button_code) {
                        xbox_state.buttons.x = pressed;
                    }
                }

                self.current_state = xbox_state.clone();

                // Update virtual controller
                if let Some(ref mut vc) = self.virtual_controller {
                    if let Err(e) = vc.update(&xbox_state) {
                        log::error!("Failed to update virtual controller: {}", e);
                    }

                    // Get rumble and apply force feedback
                    if let Ok(rumble) = vc.get_rumble() {
                        // Only log when there's actual rumble to avoid spam
                        if rumble.large_motor > 0.01 || rumble.small_motor > 0.01 {
                            log::info!("Rumble from game: large={:.2}, small={:.2}",
                                       rumble.large_motor, rumble.small_motor);
                        }
                        if let Some(ref mut ff) = self.force_feedback {
                            if let Err(e) = ff.apply_rumble(&rumble) {
                                log::error!("Failed to apply force feedback: {}", e);
                            }
                        }
                    }
                }
            }
        }
    }

    fn render_calibration_ui(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                ui.heading("RoWheel Calibration");
                ui.add_space(30.0);

                if let Some(ref calibration) = self.calibration {
                    // This is probably the worst way to do the progress indicator lol
                    let progress = match calibration.step {
                        CalibrationStep::Welcome => 0.0,
                        CalibrationStep::SteeringLeft => 0.1,
                        CalibrationStep::SteeringRight => 0.2,
                        CalibrationStep::ThrottlePressed => 0.3,
                        CalibrationStep::ThrottleReleased => 0.4,
                        CalibrationStep::BrakePressed => 0.5,
                        CalibrationStep::BrakeReleased => 0.6,
                        CalibrationStep::ClutchPressed => 0.7,
                        CalibrationStep::ClutchReleased => 0.8,
                        CalibrationStep::ShiftUp => 0.85,
                        CalibrationStep::ShiftDown => 0.9,
                        CalibrationStep::Complete => 1.0,
                    };

                    ui.add(egui::ProgressBar::new(progress).show_percentage());
                    ui.add_space(20.0);

                    // Step name
                    let step_name = match calibration.step {
                        CalibrationStep::Welcome => "Welcome",
                        CalibrationStep::SteeringLeft | CalibrationStep::SteeringRight => "Steering",
                        CalibrationStep::ThrottlePressed | CalibrationStep::ThrottleReleased => "Throttle",
                        CalibrationStep::BrakePressed | CalibrationStep::BrakeReleased => "Brake",
                        CalibrationStep::ClutchPressed | CalibrationStep::ClutchReleased => "Clutch",
                        CalibrationStep::ShiftUp => "Shift Up",
                        CalibrationStep::ShiftDown => "Shift Down",
                        CalibrationStep::Complete => "Complete",
                    };
                    ui.label(egui::RichText::new(step_name).size(24.0).strong());
                    ui.add_space(15.0);

                    // Instructions
                    ui.label(egui::RichText::new(calibration.step.instructions()).size(16.0));
                    ui.add_space(20.0);

                    // Show detected input
                    if calibration.needs_axis_detection() || calibration.needs_button_detection() {
                        ui.group(|ui| {
                            ui.label("Detected:");
                            ui.label(egui::RichText::new(&self.detected_input_info).monospace());
                        });
                        ui.add_space(20.0);
                    }
                }

                // Get step info before button rendering
                let (is_complete, can_skip) = self.calibration
                    .as_ref()
                    .map(|c| (c.step == CalibrationStep::Complete, c.step.can_skip()))
                    .unwrap_or((false, false));

                // Buttons
                ui.horizontal(|ui| {
                    if is_complete {
                        if ui.button(egui::RichText::new("Start").size(18.0)).clicked() {
                            self.finish_calibration();
                        }
                    } else if self.calibration.is_some() {
                        if ui.button(egui::RichText::new("Next").size(18.0)).clicked() {
                            if let Some(ref mut cal) = self.calibration {
                                cal.advance();
                            }
                        }

                        if can_skip {
                            if ui.button(egui::RichText::new("Skip").size(18.0)).clicked() {
                                if let Some(ref mut cal) = self.calibration {
                                    cal.skip();
                                }
                            }
                        }
                    }
                });
            });
        });
    }

    fn render_running_ui(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("RoWheel");
                ui.separator();
                if ui.button("Recalibrate").clicked() {
                    self.start_calibration();
                }
                ui.separator();
                ui.checkbox(&mut self.show_debug, "Debug");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let status = if self.virtual_controller.as_ref().map(|vc| vc.is_connected()).unwrap_or(false) {
                        egui::RichText::new("Connected").color(egui::Color32::GREEN)
                    } else {
                        egui::RichText::new("Disconnected").color(egui::Color32::RED)
                    };
                    ui.label(status);
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if !self.status_message.is_empty() {
                ui.label(&self.status_message);
                ui.add_space(10.0);
            }

            // Virtual controller visualization
            ui.heading("Gamepad Output");
            ui.add_space(10.0);

            ui.columns(2, |columns| {
                // Left column - Sticks and triggers
                columns[0].group(|ui| {
                    ui.label("Left Stick");
                    ui.horizontal(|ui| {
                        ui.label(format!("X: {:.2}", self.current_state.left_stick_x));
                        ui.label(format!("Y: {:.2}", self.current_state.left_stick_y));
                    });
                    // Simple visual
                    let stick_x = (self.current_state.left_stick_x + 1.0) / 2.0;
                    ui.add(egui::ProgressBar::new(stick_x).text("Steering"));
                });

                columns[0].add_space(10.0);

                columns[0].group(|ui| {
                    ui.label("Triggers");
                    ui.add(egui::ProgressBar::new(self.current_state.left_trigger).text("Brake (LT)"));
                    ui.add(egui::ProgressBar::new(self.current_state.right_trigger).text("Throttle (RT)"));
                });

                // Right column - Buttons and clutch
                columns[1].group(|ui| {
                    ui.label("Buttons");
                    ui.horizontal(|ui| {
                        let y_color = if self.current_state.buttons.y {
                            egui::Color32::YELLOW
                        } else {
                            egui::Color32::DARK_GRAY
                        };
                        let x_color = if self.current_state.buttons.x {
                            egui::Color32::BLUE
                        } else {
                            egui::Color32::DARK_GRAY
                        };

                        ui.label(egui::RichText::new("Y (Shift Up)").color(y_color));
                        ui.label(egui::RichText::new("X (Shift Down)").color(x_color));
                    });
                });

                columns[1].add_space(10.0);

                columns[1].group(|ui| {
                    ui.label("Clutch (Left Stick Y)");
                    let clutch_display = (self.current_state.left_stick_y + 1.0) / 2.0;
                    ui.add(egui::ProgressBar::new(clutch_display).text("Clutch"));
                });
            });

            if self.show_debug {
                ui.add_space(20.0);
                ui.separator();
                ui.heading("Debug Info");

                if let Some(ref config) = self.config {
                    ui.collapsing("Configuration", |ui| {
                        if let Some(ref s) = config.steering {
                            let raw_value = self.input_reader.as_ref()
                                .and_then(|r| r.state().get_axis(&s.device_id, s.axis_code));
                            ui.label(format!("Steering: axis {} cal=[{:.6}, {:.6}]",
                                s.axis_code, s.min_value, s.max_value));
                            ui.label(format!("  raw={:.6} out={:.6}",
                                raw_value.unwrap_or(0.0), self.current_state.left_stick_x));
                        }
                        if let Some(ref t) = config.throttle {
                            ui.label(format!("Throttle: {} axis {} [{:.2} - {:.2}]",
                                t.device_name, t.axis_code, t.min_value, t.max_value));
                        }
                        if let Some(ref b) = config.brake {
                            ui.label(format!("Brake: {} axis {} [{:.2} - {:.2}]",
                                b.device_name, b.axis_code, b.min_value, b.max_value));
                        }
                        if let Some(ref c) = config.clutch {
                            ui.label(format!("Clutch: {} axis {} [{:.2} - {:.2}]",
                                c.device_name, c.axis_code, c.min_value, c.max_value));
                        }
                        if let Some(ref su) = config.shift_up {
                            ui.label(format!("Shift Up: {} button {} (Y={})",
                                su.device_name, su.button_code, self.current_state.buttons.y));
                        }
                        if let Some(ref sd) = config.shift_down {
                            ui.label(format!("Shift Down: {} button {} (X={})",
                                sd.device_name, sd.button_code, self.current_state.buttons.x));
                        }
                    });
                }

                if let Some(ref reader) = self.input_reader {
                    ui.collapsing("Connected Devices", |ui| {
                        for (id, device) in reader.devices() {
                            ui.label(format!("{}: {} (FF: {})",
                                id, device.name, device.has_force_feedback));
                        }
                    });

                    ui.collapsing("Raw Button States", |ui| {
                        let state = reader.state();
                        for (device_id, buttons) in &state.buttons {
                            for (code, pressed) in buttons {
                                if *pressed {
                                    ui.label(format!("Device {} Button {}: PRESSED", device_id, code));
                                }
                            }
                        }
                    });
                }
            }
        });
    }
}

impl eframe::App for RoWheelApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        
        self.process_inputs();
        ctx.request_repaint();

        match self.mode {
            AppMode::Calibrating => self.render_calibration_ui(ctx),
            AppMode::Running => self.render_running_ui(ctx),
        }
    }
}
