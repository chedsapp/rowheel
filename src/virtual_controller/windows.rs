use super::{RumbleState, VirtualController, XboxControllerState};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use vigem_client::{Client, XGamepad, XButtons, Xbox360Wired, TargetId};

pub struct VirtualXboxController {
    target: Xbox360Wired<Client>,
    connected: bool,
    rumble_state: Arc<Mutex<RumbleState>>,
    notification_thread: Option<JoinHandle<()>>,
}

impl VirtualXboxController {
    pub fn new() -> anyhow::Result<Self> {
        let client = Client::connect()
            .map_err(|e| anyhow::anyhow!(
                "Failed to connect to ViGEmBus: {:?}. Make sure ViGEmBus driver is installed from https://github.com/ViGEm/ViGEmBus/releases",
                e
            ))?;

        let id = TargetId::XBOX360_WIRED;
        let mut target = Xbox360Wired::new(client, id);

        target.plugin()
            .map_err(|e| anyhow::anyhow!("Failed to plug in virtual controller: {:?}", e))?;

        target.wait_ready()
            .map_err(|e| anyhow::anyhow!("Controller not ready: {:?}", e))?;

        log::info!("Virtual Xbox 360 controller created via ViGEmBus");

        let rumble_state = Arc::new(Mutex::new(RumbleState::default()));

        // Request notification for rumble/force feedback
        let notification_thread = match target.request_notification() {
            Ok(request_notification) => {
                let rumble_state_clone = Arc::clone(&rumble_state);

                let handle = request_notification.spawn_thread(move |_notif, data| {
                    // Update rumble state when we receive notifications from the game
                    if let Ok(mut state) = rumble_state_clone.lock() {
                        state.large_motor = data.large_motor as f32 / 255.0;
                        state.small_motor = data.small_motor as f32 / 255.0;
                        log::trace!("Rumble update: large={:.2}, small={:.2}",
                                   state.large_motor, state.small_motor);
                    }
                });

                log::info!("Force feedback notifications enabled");
                Some(handle)
            }
            Err(e) => {
                log::warn!("Failed to enable force feedback notifications: {:?}", e);
                None
            }
        };

        Ok(Self {
            target,
            connected: true,
            rumble_state,
            notification_thread,
        })
    }
}

impl VirtualController for VirtualXboxController {
    fn update(&mut self, state: &XboxControllerState) -> anyhow::Result<()> {
        if !self.connected {
            return Ok(());
        }

        let thumb_lx = (state.left_stick_x * 32767.0) as i16;
        let thumb_ly = (state.left_stick_y * 32767.0) as i16;
        let thumb_rx = (state.right_stick_x * 32767.0) as i16;
        let thumb_ry = (state.right_stick_y * 32767.0) as i16;

        let left_trigger = (state.left_trigger * 255.0) as u8;
        let right_trigger = (state.right_trigger * 255.0) as u8;

        // vigem-client wants us to or together all button flags
        let mut button_flags: u16 = 0;
        if state.buttons.a { button_flags |= XButtons::A; }
        if state.buttons.b { button_flags |= XButtons::B; }
        if state.buttons.x { button_flags |= XButtons::X; }
        if state.buttons.y { button_flags |= XButtons::Y; }
        if state.buttons.left_bumper { button_flags |= XButtons::LB; }
        if state.buttons.right_bumper { button_flags |= XButtons::RB; }
        if state.buttons.back { button_flags |= XButtons::BACK; }
        if state.buttons.start { button_flags |= XButtons::START; }
        if state.buttons.guide { button_flags |= XButtons::GUIDE; }
        if state.buttons.left_thumb { button_flags |= XButtons::LTHUMB; }
        if state.buttons.right_thumb { button_flags |= XButtons::RTHUMB; }
        if state.buttons.dpad_up { button_flags |= XButtons::UP; }
        if state.buttons.dpad_down { button_flags |= XButtons::DOWN; }
        if state.buttons.dpad_left { button_flags |= XButtons::LEFT; }
        if state.buttons.dpad_right { button_flags |= XButtons::RIGHT; }

        let buttons = XButtons { raw: button_flags };

        let gamepad = XGamepad {
            buttons,
            left_trigger,
            right_trigger,
            thumb_lx,
            thumb_ly,
            thumb_rx,
            thumb_ry,
        };

        self.target.update(&gamepad)
            .map_err(|e| anyhow::anyhow!("Failed to update controller: {:?}", e))?;

        Ok(())
    }

    fn get_rumble(&mut self) -> anyhow::Result<RumbleState> {
        let state = self.rumble_state.lock()
            .map_err(|_| anyhow::anyhow!("Failed to lock rumble state"))?;
        Ok(state.clone())
    }

    fn is_connected(&self) -> bool {
        self.connected
    }
}

impl Drop for VirtualXboxController {
    fn drop(&mut self) {
        // Exit notification thread
        let _ = self.target.unplug();
        
        if let Some(thread) = self.notification_thread.take() {
            let _ = thread.join();
        }
    }
}
