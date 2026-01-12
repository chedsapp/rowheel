use super::{RumbleState, VirtualController, XboxControllerState};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::io::AsRawFd;

// uinput constants
const UINPUT_PATH: &str = "/dev/uinput";

// Input event types
const EV_SYN: u16 = 0x00;
const EV_KEY: u16 = 0x01;
const EV_ABS: u16 = 0x03;

// Synchronization events
const SYN_REPORT: u16 = 0x00;

// Xbox controller buttons (Linux kernel codes)
const BTN_A: u16 = 0x130;
const BTN_B: u16 = 0x131;
const BTN_X: u16 = 0x133;
const BTN_Y: u16 = 0x134;
const BTN_TL: u16 = 0x136;
const BTN_TR: u16 = 0x137;
const BTN_SELECT: u16 = 0x13a;
const BTN_START: u16 = 0x13b;
const BTN_MODE: u16 = 0x13c;
const BTN_THUMBL: u16 = 0x13d;
const BTN_THUMBR: u16 = 0x13e;

// Absolute axes
const ABS_X: u16 = 0x00;
const ABS_Y: u16 = 0x01;
const ABS_Z: u16 = 0x02;
const ABS_RX: u16 = 0x03;
const ABS_RY: u16 = 0x04;
const ABS_RZ: u16 = 0x05;
const ABS_HAT0X: u16 = 0x10;
const ABS_HAT0Y: u16 = 0x11;

// uinput ioctl codes (for x86_64 Linux)
const UI_DEV_CREATE: libc::c_ulong = 0x5501;
const UI_DEV_DESTROY: libc::c_ulong = 0x5502;
const UI_SET_EVBIT: libc::c_ulong = 0x40045564;
const UI_SET_KEYBIT: libc::c_ulong = 0x40045565;
const UI_SET_ABSBIT: libc::c_ulong = 0x40045567;

// Axis ranges (Xbox 360 standard)
const AXIS_MIN: i32 = -32768;
const AXIS_MAX: i32 = 32767;
const TRIGGER_MIN: i32 = 0;
const TRIGGER_MAX: i32 = 255;

#[repr(C)]
struct InputEvent {
    tv_sec: libc::time_t,
    tv_usec: libc::suseconds_t,
    type_: u16,
    code: u16,
    value: i32,
}

impl InputEvent {
    fn new(type_: u16, code: u16, value: i32) -> Self {
        Self {
            tv_sec: 0,
            tv_usec: 0,
            type_,
            code,
            value,
        }
    }
}

#[repr(C)]
struct UinputUserDev {
    name: [u8; 80],
    id: InputId,
    ff_effects_max: u32,
    absmax: [i32; 64],
    absmin: [i32; 64],
    absfuzz: [i32; 64],
    absflat: [i32; 64],
}

#[repr(C)]
#[derive(Default)]
struct InputId {
    bustype: u16,
    vendor: u16,
    product: u16,
    version: u16,
}

pub struct VirtualXboxController {
    file: Option<File>,
    connected: bool,
}

impl VirtualXboxController {
    pub fn new() -> anyhow::Result<Self> {
        let mut controller = Self {
            file: None,
            connected: false,
        };

        controller.connect()?;
        Ok(controller)
    }

    fn connect(&mut self) -> anyhow::Result<()> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(UINPUT_PATH)
            .map_err(|e| anyhow::anyhow!(
                "Failed to open {}: {}. Try: sudo chmod 666 /dev/uinput",
                UINPUT_PATH, e
            ))?;

        let fd = file.as_raw_fd();

        unsafe {
            // Set event types
            if libc::ioctl(fd, UI_SET_EVBIT, EV_KEY as libc::c_int) < 0 {
                return Err(anyhow::anyhow!("Failed to set EV_KEY"));
            }
            if libc::ioctl(fd, UI_SET_EVBIT, EV_ABS as libc::c_int) < 0 {
                return Err(anyhow::anyhow!("Failed to set EV_ABS"));
            }
            if libc::ioctl(fd, UI_SET_EVBIT, EV_SYN as libc::c_int) < 0 {
                return Err(anyhow::anyhow!("Failed to set EV_SYN"));
            }

            // Set buttons
            for btn in [BTN_A, BTN_B, BTN_X, BTN_Y, BTN_TL, BTN_TR,
                        BTN_SELECT, BTN_START, BTN_MODE, BTN_THUMBL, BTN_THUMBR] {
                if libc::ioctl(fd, UI_SET_KEYBIT, btn as libc::c_int) < 0 {
                    return Err(anyhow::anyhow!("Failed to set button {}", btn));
                }
            }

            // Set absolute axes
            for axis in [ABS_X, ABS_Y, ABS_RX, ABS_RY, ABS_Z, ABS_RZ, ABS_HAT0X, ABS_HAT0Y] {
                if libc::ioctl(fd, UI_SET_ABSBIT, axis as libc::c_int) < 0 {
                    return Err(anyhow::anyhow!("Failed to set axis {}", axis));
                }
            }

            // Create device struct
            let mut dev: UinputUserDev = std::mem::zeroed();
            let name = b"RoWheel Virtual Xbox Controller";
            dev.name[..name.len()].copy_from_slice(name);
            dev.id.bustype = 0x03; // BUS_USB
            dev.id.vendor = 0x045e; // Microsoft
            dev.id.product = 0x028e; // Xbox 360 Controller
            dev.id.version = 0x0110;

            // Set axis ranges
            dev.absmin[ABS_X as usize] = AXIS_MIN;
            dev.absmax[ABS_X as usize] = AXIS_MAX;
            dev.absmin[ABS_Y as usize] = AXIS_MIN;
            dev.absmax[ABS_Y as usize] = AXIS_MAX;
            dev.absmin[ABS_RX as usize] = AXIS_MIN;
            dev.absmax[ABS_RX as usize] = AXIS_MAX;
            dev.absmin[ABS_RY as usize] = AXIS_MIN;
            dev.absmax[ABS_RY as usize] = AXIS_MAX;
            dev.absmin[ABS_Z as usize] = TRIGGER_MIN;
            dev.absmax[ABS_Z as usize] = TRIGGER_MAX;
            dev.absmin[ABS_RZ as usize] = TRIGGER_MIN;
            dev.absmax[ABS_RZ as usize] = TRIGGER_MAX;
            dev.absmin[ABS_HAT0X as usize] = -1;
            dev.absmax[ABS_HAT0X as usize] = 1;
            dev.absmin[ABS_HAT0Y as usize] = -1;
            dev.absmax[ABS_HAT0Y as usize] = 1;

            // Write device struct
            let dev_bytes = std::slice::from_raw_parts(
                &dev as *const _ as *const u8,
                std::mem::size_of::<UinputUserDev>()
            );

            if libc::write(fd, dev_bytes.as_ptr() as *const libc::c_void, dev_bytes.len()) < 0 {
                return Err(anyhow::anyhow!("Failed to write device struct"));
            }

            // Create device
            if libc::ioctl(fd, UI_DEV_CREATE) < 0 {
                return Err(anyhow::anyhow!("Failed to create device: {}", std::io::Error::last_os_error()));
            }
        }

        self.file = Some(file);
        self.connected = true;

        log::info!("Virtual Xbox controller created via uinput");
        Ok(())
    }

    fn write_event(&mut self, type_: u16, code: u16, value: i32) -> anyhow::Result<()> {
        if let Some(ref mut file) = self.file {
            let event = InputEvent::new(type_, code, value);
            let bytes = unsafe {
                std::slice::from_raw_parts(
                    &event as *const _ as *const u8,
                    std::mem::size_of::<InputEvent>()
                )
            };
            file.write_all(bytes)?;
        }
        Ok(())
    }

    fn sync(&mut self) -> anyhow::Result<()> {
        self.write_event(EV_SYN, SYN_REPORT, 0)?;
        if let Some(ref mut file) = self.file {
            file.flush()?;
        }
        Ok(())
    }
}

impl Drop for VirtualXboxController {
    fn drop(&mut self) {
        if let Some(ref file) = self.file {
            unsafe {
                let _ = libc::ioctl(file.as_raw_fd(), UI_DEV_DESTROY);
            }
        }
    }
}

impl VirtualController for VirtualXboxController {
    fn update(&mut self, state: &XboxControllerState) -> anyhow::Result<()> {
        if !self.connected {
            return Ok(());
        }

        let lx = (state.left_stick_x * AXIS_MAX as f32) as i32;
        let ly = (state.left_stick_y * AXIS_MAX as f32) as i32;
        let rx = (state.right_stick_x * AXIS_MAX as f32) as i32;
        let ry = (state.right_stick_y * AXIS_MAX as f32) as i32;
        let lt = (state.left_trigger * TRIGGER_MAX as f32) as i32;
        let rt = (state.right_trigger * TRIGGER_MAX as f32) as i32;

        self.write_event(EV_ABS, ABS_X, lx)?;
        self.write_event(EV_ABS, ABS_Y, ly)?;
        self.write_event(EV_ABS, ABS_RX, rx)?;
        self.write_event(EV_ABS, ABS_RY, ry)?;
        self.write_event(EV_ABS, ABS_Z, lt)?;
        self.write_event(EV_ABS, ABS_RZ, rt)?;

        self.write_event(EV_KEY, BTN_A, state.buttons.a as i32)?;
        self.write_event(EV_KEY, BTN_B, state.buttons.b as i32)?;
        self.write_event(EV_KEY, BTN_X, state.buttons.x as i32)?;
        self.write_event(EV_KEY, BTN_Y, state.buttons.y as i32)?;
        self.write_event(EV_KEY, BTN_TL, state.buttons.left_bumper as i32)?;
        self.write_event(EV_KEY, BTN_TR, state.buttons.right_bumper as i32)?;
        self.write_event(EV_KEY, BTN_SELECT, state.buttons.back as i32)?;
        self.write_event(EV_KEY, BTN_START, state.buttons.start as i32)?;
        self.write_event(EV_KEY, BTN_MODE, state.buttons.guide as i32)?;
        self.write_event(EV_KEY, BTN_THUMBL, state.buttons.left_thumb as i32)?;
        self.write_event(EV_KEY, BTN_THUMBR, state.buttons.right_thumb as i32)?;

        let hat_x = match (state.buttons.dpad_left, state.buttons.dpad_right) {
            (true, false) => -1,
            (false, true) => 1,
            _ => 0,
        };
        let hat_y = match (state.buttons.dpad_up, state.buttons.dpad_down) {
            (true, false) => -1,
            (false, true) => 1,
            _ => 0,
        };
        self.write_event(EV_ABS, ABS_HAT0X, hat_x)?;
        self.write_event(EV_ABS, ABS_HAT0Y, hat_y)?;

        self.sync()?;
        Ok(())
    }

    fn get_rumble(&mut self) -> anyhow::Result<RumbleState> {
        // Rumble feedback from games would require reading force feedback events
        // For now, return default state
        Ok(RumbleState::default())
    }

    fn is_connected(&self) -> bool {
        self.connected
    }
}
