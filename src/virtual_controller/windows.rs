use super::{RumbleState, VirtualController, XboxControllerState};
use std::sync::{Arc, Mutex};
use vigem_client::{Client, XGamepad, XButtons, Xbox360Wired, TargetId};

pub struct VirtualXboxController {
    _client: Client,
    target: Xbox360Wired,
    connected: bool,
    rumble_state: Arc<Mutex<RumbleState>>,
}

impl VirtualXboxController {
    pub fn new() -> anyhow::Result<Self> {
        let client = Client::connect()
            .map_err(|e| anyhow::anyhow!(
                "Failed to connect to ViGEmBus: {:?}. Make sure ViGEmBus driver is installed from https://github.com/ViGEm/ViGEmBus/releases",
                e
            ))?;

        let id = TargetId::XBOX360_WIRED;
        let mut target = Xbox360Wired::new(&client, id);

        target.plugin()
            .map_err(|e| anyhow::anyhow!("Failed to plug in virtual controller: {:?}", e))?;

        target.wait_ready()
            .map_err(|e| anyhow::anyhow!("Controller not ready: {:?}", e))?;

        log::info!("Virtual Xbox 360 controller created via ViGEmBus");

        let rumble_state = Arc::new(Mutex::new(RumbleState::default()));

        Ok(Self {
            _client: client,
            target,
            connected: true,
            rumble_state,
        })
    }
}

impl VirtualController for VirtualXboxController {
    fn update(&mut self, state: &XboxControllerState) -> anyhow::Result<()> {
        if !self.connected {
            return Ok(());
        }

        // Convert axes to i16 range (-32768 to 32767)
        let thumb_lx = (state.left_stick_x * 32767.0) as i16;
        let thumb_ly = (state.left_stick_y * 32767.0) as i16;
        let thumb_rx = (state.right_stick_x * 32767.0) as i16;
        let thumb_ry = (state.right_stick_y * 32767.0) as i16;

        // Convert triggers to u8 range (0 to 255)
        let left_trigger = (state.left_trigger * 255.0) as u8;
        let right_trigger = (state.right_trigger * 255.0) as u8;

        // Build button mask
        let mut buttons = XButtons::empty();
        if state.buttons.a { buttons |= XButtons::A; }
        if state.buttons.b { buttons |= XButtons::B; }
        if state.buttons.x { buttons |= XButtons::X; }
        if state.buttons.y { buttons |= XButtons::Y; }
        if state.buttons.left_bumper { buttons |= XButtons::LB; }
        if state.buttons.right_bumper { buttons |= XButtons::RB; }
        if state.buttons.back { buttons |= XButtons::BACK; }
        if state.buttons.start { buttons |= XButtons::START; }
        if state.buttons.guide { buttons |= XButtons::GUIDE; }
        if state.buttons.left_thumb { buttons |= XButtons::LTHUMB; }
        if state.buttons.right_thumb { buttons |= XButtons::RTHUMB; }
        if state.buttons.dpad_up { buttons |= XButtons::UP; }
        if state.buttons.dpad_down { buttons |= XButtons::DOWN; }
        if state.buttons.dpad_left { buttons |= XButtons::LEFT; }
        if state.buttons.dpad_right { buttons |= XButtons::RIGHT; }

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
        // Note: vigem-client 0.1.x doesn't support rumble notifications easily
        // For full rumble support, we'd need to use a callback-based approach
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
        let _ = self.target.unplug();
    }
}
