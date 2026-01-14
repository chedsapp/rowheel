use std::ffi::c_void;
use windows::core::{GUID, HRESULT, PCSTR};
use windows::Win32::Foundation::{HMODULE, HWND};

// This SHOULD match all definitions in dinput.h and dinputd.h

// GUIDs
pub const IID_IDIRECTINPUT8W: GUID = GUID::from_u128(0xbf798031_483a_4da2_aa99_5d64ed369700);
pub const GUID_CONSTANT_FORCE: GUID = GUID::from_u128(0x13541c20_8e33_11d0_9ad0_00a0c9a06e35);
pub const GUID_XAXIS: GUID = GUID::from_u128(0xa36d02e0_c9f3_11cf_bfc7_444553540000);
pub const GUID_YAXIS: GUID = GUID::from_u128(0xa36d02e1_c9f3_11cf_bfc7_444553540000);
pub const GUID_ZAXIS: GUID = GUID::from_u128(0xa36d02e2_c9f3_11cf_bfc7_444553540000);
pub const GUID_RXAXIS: GUID = GUID::from_u128(0xa36d02f4_c9f3_11cf_bfc7_444553540000);
pub const GUID_RYAXIS: GUID = GUID::from_u128(0xa36d02f5_c9f3_11cf_bfc7_444553540000);
pub const GUID_RZAXIS: GUID = GUID::from_u128(0xa36d02e3_c9f3_11cf_bfc7_444553540000);
pub const GUID_SLIDER: GUID = GUID::from_u128(0xa36d02e4_c9f3_11cf_bfc7_444553540000);
pub const GUID_POV: GUID = GUID::from_u128(0xa36d02f2_c9f3_11cf_bfc7_444553540000);
pub const GUID_BUTTON: GUID = GUID::from_u128(0xa36d02f0_c9f3_11cf_bfc7_444553540000);

// DirectInput constants
pub const DISCL_EXCLUSIVE: u32 = 0x00000001;
pub const DISCL_NONEXCLUSIVE: u32 = 0x00000002;
pub const DISCL_BACKGROUND: u32 = 0x00000008;
pub const DI8DEVCLASS_GAMECTRL: u32 = 4;
pub const DIEDFL_FORCEFEEDBACK: u32 = 0x00000100;
pub const DIEFF_CARTESIAN: u32 = 0x00000010;
pub const DIEFF_OBJECTOFFSETS: u32 = 0x00000002;
pub const DIEP_DIRECTION: u32 = 0x00000040;
pub const DIEP_TYPESPECIFICPARAMS: u32 = 0x00000100;
pub const DIEP_START: u32 = 0x20000000;
pub const DI_OK: i32 = 0;
pub const DIRECTINPUT_VERSION: u32 = 0x0800;
pub const DIPH_DEVICE: u32 = 0;
pub const DIPROPAUTOCENTER_OFF: u32 = 0;
pub const DIDF_ABSAXIS: u32 = 0x00000001;
pub const DIDFT_AXIS: u32 = 0x00000003;
pub const DIDFT_BUTTON: u32 = 0x0000000C;
pub const DIDFT_POV: u32 = 0x00000010;
pub const DIDFT_OPTIONAL: u32 = 0x80000000;
pub const DIDFT_ANYINSTANCE: u32 = 0x00FFFF00;

// Callback return type
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct DIBOOL(pub i32);
pub const DIENUM_CONTINUE: DIBOOL = DIBOOL(1);
pub const DIENUM_STOP: DIBOOL = DIBOOL(0);

#[repr(C)]
#[derive(Clone, Copy)]
pub struct DIDEVICEINSTANCEW {
    pub dw_size: u32,
    pub guid_instance: GUID,
    pub guid_product: GUID,
    pub dw_dev_type: u32,
    pub tsz_instance_name: [u16; 260],
    pub tsz_product_name: [u16; 260],
    pub guid_ff_driver: GUID,
    pub w_usage_page: u16,
    pub w_usage: u16,
}

#[repr(C)]
pub struct DIPROPHEADER {
    pub dw_size: u32,
    pub dw_header_size: u32,
    pub dw_obj: u32,
    pub dw_how: u32,
}

#[repr(C)]
pub struct DIPROPDWORD {
    pub diph: DIPROPHEADER,
    pub dw_data: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct DIOBJECTDATAFORMAT {
    pub pguid: *const GUID,
    pub dw_ofs: u32,
    pub dw_type: u32,
    pub dw_flags: u32,
}

#[repr(C)]
pub struct DIDATAFORMAT {
    pub dw_size: u32,
    pub dw_obj_size: u32,
    pub dw_flags: u32,
    pub dw_data_size: u32,
    pub dw_num_objs: u32,
    pub rgodf: *const DIOBJECTDATAFORMAT,
}

#[repr(C)]
pub struct DIEFFECT {
    pub dw_size: u32,
    pub dw_flags: u32,
    pub dw_duration: u32,
    pub dw_sample_period: u32,
    pub dw_gain: u32,
    pub dw_trigger_button: u32,
    pub dw_trigger_repeat_interval: u32,
    pub c_axes: u32,
    pub rgdw_axes: *mut u32,
    pub rgl_direction: *mut i32,
    pub lp_envelope: *mut c_void,
    pub cb_type_specific_params: u32,
    pub lp_type_specific_params: *mut c_void,
    pub dw_start_delay: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct DICONSTANTFORCE {
    pub l_magnitude: i32,
}

#[repr(C)]
pub struct DIJOYSTATE2 {
    pub lx: i32,
    pub ly: i32,
    pub lz: i32,
    pub lrx: i32,
    pub lry: i32,
    pub lrz: i32,
    pub rgl_slider: [i32; 2],
    pub rgdw_pov: [u32; 4],
    pub rgb_buttons: [u8; 128],
    pub lv_x: i32,
    pub lv_y: i32,
    pub lv_z: i32,
    pub lv_rx: i32,
    pub lv_ry: i32,
    pub lv_rz: i32,
    pub rgl_v_slider: [i32; 2],
    pub la_x: i32,
    pub la_y: i32,
    pub la_z: i32,
    pub la_rx: i32,
    pub la_ry: i32,
    pub la_rz: i32,
    pub rgl_a_slider: [i32; 2],
    pub lf_x: i32,
    pub lf_y: i32,
    pub lf_z: i32,
    pub lf_rx: i32,
    pub lf_ry: i32,
    pub lf_rz: i32,
    pub rgl_f_slider: [i32; 2],
}

// COM interface definitions
#[repr(C)]
pub struct IDirectInput8WVtbl {
    pub query_interface: *const c_void,
    pub add_ref: *const c_void,
    pub release: unsafe extern "system" fn(*mut IDirectInput8W) -> u32,
    pub create_device: unsafe extern "system" fn(
        *mut IDirectInput8W,
        *const GUID,
        *mut *mut IDirectInputDevice8W,
        *mut c_void,
    ) -> HRESULT,
    pub enum_devices: unsafe extern "system" fn(
        *mut IDirectInput8W,
        u32,
        unsafe extern "system" fn(*const DIDEVICEINSTANCEW, *mut c_void) -> DIBOOL,
        *mut c_void,
        u32,
    ) -> HRESULT,
}

#[repr(C)]
pub struct IDirectInput8W {
    pub lpvtbl: *const IDirectInput8WVtbl,
}

#[repr(C)]
pub struct IDirectInputDevice8WVtbl {
    pub query_interface: *const c_void,
    pub add_ref: *const c_void,
    pub release: unsafe extern "system" fn(*mut IDirectInputDevice8W) -> u32,
    pub get_capabilities: *const c_void,
    pub enum_objects: *const c_void,
    pub get_property: *const c_void,
    pub set_property: unsafe extern "system" fn(
        *mut IDirectInputDevice8W,
        *const GUID,
        *const DIPROPHEADER,
    ) -> HRESULT,
    pub acquire: unsafe extern "system" fn(*mut IDirectInputDevice8W) -> HRESULT,
    pub unacquire: unsafe extern "system" fn(*mut IDirectInputDevice8W) -> HRESULT,
    pub get_device_state: *const c_void,
    pub get_device_data: *const c_void,
    pub set_data_format: unsafe extern "system" fn(*mut IDirectInputDevice8W, *const DIDATAFORMAT) -> HRESULT,
    pub set_event_notification: *const c_void,
    pub set_cooperative_level: unsafe extern "system" fn(*mut IDirectInputDevice8W, HWND, u32) -> HRESULT,
    pub get_object_info: *const c_void,
    pub get_device_info: *const c_void,
    pub run_control_panel: *const c_void,
    pub initialize: *const c_void,
    pub create_effect: unsafe extern "system" fn(
        *mut IDirectInputDevice8W,
        *const GUID,
        *const DIEFFECT,
        *mut *mut IDirectInputEffect,
        *mut c_void,
    ) -> HRESULT,
    pub enum_effects: *const c_void,
    pub get_effect_info: *const c_void,
    pub get_force_feedback_state: *const c_void,
    pub send_force_feedback_command: *const c_void,
    pub enum_created_effect_objects: *const c_void,
    pub escape: *const c_void,
    pub poll: unsafe extern "system" fn(*mut IDirectInputDevice8W) -> HRESULT,
}

#[repr(C)]
pub struct IDirectInputDevice8W {
    pub lpvtbl: *const IDirectInputDevice8WVtbl,
}

#[repr(C)]
pub struct IDirectInputEffectVtbl {
    pub query_interface: *const c_void,
    pub add_ref: *const c_void,
    pub release: unsafe extern "system" fn(*mut IDirectInputEffect) -> u32,
    pub initialize: *const c_void,
    pub get_effect_guid: *const c_void,
    pub get_parameters: *const c_void,
    pub set_parameters: unsafe extern "system" fn(*mut IDirectInputEffect, *const DIEFFECT, u32) -> HRESULT,
    pub start: unsafe extern "system" fn(*mut IDirectInputEffect, u32, u32) -> HRESULT,
    pub stop: unsafe extern "system" fn(*mut IDirectInputEffect) -> HRESULT,
    pub get_effect_status: *const c_void,
    pub download: *const c_void,
    pub unload: unsafe extern "system" fn(*mut IDirectInputEffect) -> HRESULT,
}

#[repr(C)]
pub struct IDirectInputEffect {
    pub lpvtbl: *const IDirectInputEffectVtbl,
}

pub type DirectInput8CreateFn = unsafe extern "system" fn(
    HMODULE,
    u32,
    *const GUID,
    *mut *mut IDirectInput8W,
    *mut c_void,
) -> HRESULT;

pub fn get_directinput8_create(hinst: HMODULE) -> anyhow::Result<DirectInput8CreateFn> {
    use windows::Win32::System::LibraryLoader::GetProcAddress;

    let proc_name = b"DirectInput8Create\0";
    unsafe {
        match GetProcAddress(hinst, PCSTR(proc_name.as_ptr())) {
            Some(p) => Ok(std::mem::transmute(p)),
            None => Err(anyhow::anyhow!("Failed to get DirectInput8Create")),
        }
    }
}
