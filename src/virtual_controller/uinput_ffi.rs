pub const UINPUT_PATH: &str = "/dev/uinput";

// Event stuff
pub const EV_SYN: u16 = 0x00;
pub const EV_KEY: u16 = 0x01;
pub const EV_ABS: u16 = 0x03;
pub const EV_FF: u16 = 0x15;
pub const EV_UINPUT: u16 = 0x0101;

pub const SYN_REPORT: u16 = 0x00;

// Force feedback effect types
pub const FF_RUMBLE: u16 = 0x50;

// UI event codes
pub const UI_FF_UPLOAD: u16 = 1;
pub const UI_FF_ERASE: u16 = 2;

// Xbox controller buttons as linux event codes
pub const BTN_A: u16 = 0x130;
pub const BTN_B: u16 = 0x131;
pub const BTN_X: u16 = 0x133;
pub const BTN_Y: u16 = 0x134;
pub const BTN_TL: u16 = 0x136;
pub const BTN_TR: u16 = 0x137;
pub const BTN_SELECT: u16 = 0x13a;
pub const BTN_START: u16 = 0x13b;
pub const BTN_MODE: u16 = 0x13c;
pub const BTN_THUMBL: u16 = 0x13d;
pub const BTN_THUMBR: u16 = 0x13e;

// Absolute axes
pub const ABS_X: u16 = 0x00;
pub const ABS_Y: u16 = 0x01;
pub const ABS_Z: u16 = 0x02;
pub const ABS_RX: u16 = 0x03;
pub const ABS_RY: u16 = 0x04;
pub const ABS_RZ: u16 = 0x05;
pub const ABS_HAT0X: u16 = 0x10;
pub const ABS_HAT0Y: u16 = 0x11;

// uinput ioctl codes (for x86_64 Linux)
pub const UI_DEV_CREATE: libc::c_ulong = 0x5501;
pub const UI_DEV_DESTROY: libc::c_ulong = 0x5502;
pub const UI_SET_EVBIT: libc::c_ulong = 0x40045564;
pub const UI_SET_KEYBIT: libc::c_ulong = 0x40045565;
pub const UI_SET_ABSBIT: libc::c_ulong = 0x40045567;
pub const UI_SET_FFBIT: libc::c_ulong = 0x4004556b;

// Force feedback ioctl codes
pub const UI_BEGIN_FF_UPLOAD: libc::c_ulong = 0xc06855c8;
pub const UI_END_FF_UPLOAD: libc::c_ulong = 0x406855c9;
pub const UI_BEGIN_FF_ERASE: libc::c_ulong = 0xc00455ca;
pub const UI_END_FF_ERASE: libc::c_ulong = 0x400455cb;

// Axis ranges (Xbox 360 standard)
pub const AXIS_MIN: i32 = -32768;
pub const AXIS_MAX: i32 = 32767;
pub const TRIGGER_MIN: i32 = 0;
pub const TRIGGER_MAX: i32 = 255;

#[repr(C)]
pub struct InputEvent {
    pub tv_sec: libc::time_t,
    pub tv_usec: libc::suseconds_t,
    pub type_: u16,
    pub code: u16,
    pub value: i32,
}

impl InputEvent {
    pub fn new(type_: u16, code: u16, value: i32) -> Self {
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
pub struct UinputUserDev {
    pub name: [u8; 80],
    pub id: InputId,
    pub ff_effects_max: u32,
    pub absmax: [i32; 64],
    pub absmin: [i32; 64],
    pub absfuzz: [i32; 64],
    pub absflat: [i32; 64],
}

#[repr(C)]
#[derive(Default)]
pub struct InputId {
    pub bustype: u16,
    pub vendor: u16,
    pub product: u16,
    pub version: u16,
}

// We only really care about the rumble effect since this is what roblox sends
#[repr(C)]
pub struct FFEffect {
    pub type_: u16,
    pub id: i16,
    pub direction: u16,
    pub trigger: [u8; 4],  // ff_trigger (button u16 + interval u16)
    pub replay: [u8; 4],   // ff_replay (length u16 + delay u16)
    pub _pad: [u8; 2],     // 8byte to align union
    pub u: [u8; 48],       // union (needs to be big enough for ff_periodic_effect)
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct FFRumbleEffect {
    pub strong_magnitude: u16,
    pub weak_magnitude: u16,
}

#[repr(C)]
pub struct UinputFFUpload {
    pub request_id: u32,
    pub retval: i32,
    pub effect: FFEffect,
    pub old: FFEffect,
}

#[repr(C)]
pub struct UinputFFErase {
    pub request_id: u32,
    pub retval: i32,
    pub effect_id: u32,
}
