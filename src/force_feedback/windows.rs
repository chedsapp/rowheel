use super::{ForceFeedback, RumbleState};
use std::ffi::c_void;
use std::ptr;
use windows::core::{GUID, HRESULT, PCSTR, PCWSTR};
use windows::Win32::Foundation::{HMODULE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassW, HWND_MESSAGE, WINDOW_EX_STYLE,
    WINDOW_STYLE, WNDCLASSW,
};

// GUIDs
const IID_IDIRECTINPUT8W: GUID = GUID::from_u128(0xbf798031_483a_4da2_aa99_5d64ed369700);
const GUID_CONSTANT_FORCE: GUID = GUID::from_u128(0x13541c20_8e33_11d0_9ad0_00a0c9a06e35);
const GUID_XAXIS: GUID = GUID::from_u128(0xa36d02e0_c9f3_11cf_bfc7_444553540000);
const GUID_YAXIS: GUID = GUID::from_u128(0xa36d02e1_c9f3_11cf_bfc7_444553540000);
const GUID_ZAXIS: GUID = GUID::from_u128(0xa36d02e2_c9f3_11cf_bfc7_444553540000);
const GUID_RXAXIS: GUID = GUID::from_u128(0xa36d02f4_c9f3_11cf_bfc7_444553540000);
const GUID_RYAXIS: GUID = GUID::from_u128(0xa36d02f5_c9f3_11cf_bfc7_444553540000);
const GUID_RZAXIS: GUID = GUID::from_u128(0xa36d02e3_c9f3_11cf_bfc7_444553540000);
const GUID_SLIDER: GUID = GUID::from_u128(0xa36d02e4_c9f3_11cf_bfc7_444553540000);
const GUID_POV: GUID = GUID::from_u128(0xa36d02f2_c9f3_11cf_bfc7_444553540000);
const GUID_BUTTON: GUID = GUID::from_u128(0xa36d02f0_c9f3_11cf_bfc7_444553540000);

// DirectInput constants
const DISCL_EXCLUSIVE: u32 = 0x00000001;
const DISCL_NONEXCLUSIVE: u32 = 0x00000002;
const DISCL_BACKGROUND: u32 = 0x00000008;
const DI8DEVCLASS_GAMECTRL: u32 = 4;
const DIEDFL_FORCEFEEDBACK: u32 = 0x00000100;
const DIEFF_CARTESIAN: u32 = 0x00000010;
const DIEFF_OBJECTOFFSETS: u32 = 0x00000002;
const DIEP_DIRECTION: u32 = 0x00000040;
const DIEP_TYPESPECIFICPARAMS: u32 = 0x00000100;
const DIEP_START: u32 = 0x20000000;
const DI_OK: i32 = 0;
const DIRECTINPUT_VERSION: u32 = 0x0800;
const DIPH_DEVICE: u32 = 0;
const DIPROPAUTOCENTER_OFF: u32 = 0;
const DIDF_ABSAXIS: u32 = 0x00000001;
const DIDFT_AXIS: u32 = 0x00000003;
const DIDFT_BUTTON: u32 = 0x0000000C;
const DIDFT_POV: u32 = 0x00000010;
const DIDFT_OPTIONAL: u32 = 0x80000000;
const DIDFT_ANYINSTANCE: u32 = 0x00FFFF00;

// Callback return type
#[repr(transparent)]
#[derive(Clone, Copy)]
struct DIBOOL(pub i32);
const DIENUM_CONTINUE: DIBOOL = DIBOOL(1);
const DIENUM_STOP: DIBOOL = DIBOOL(0);

#[repr(C)]
#[derive(Clone, Copy)]
struct DIDEVICEINSTANCEW {
    dw_size: u32,
    guid_instance: GUID,
    guid_product: GUID,
    dw_dev_type: u32,
    tsz_instance_name: [u16; 260],
    tsz_product_name: [u16; 260],
    guid_ff_driver: GUID,
    w_usage_page: u16,
    w_usage: u16,
}

#[repr(C)]
struct DIPROPHEADER {
    dw_size: u32,
    dw_header_size: u32,
    dw_obj: u32,
    dw_how: u32,
}

#[repr(C)]
struct DIPROPDWORD {
    diph: DIPROPHEADER,
    dw_data: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct DIOBJECTDATAFORMAT {
    pguid: *const GUID,
    dw_ofs: u32,
    dw_type: u32,
    dw_flags: u32,
}

#[repr(C)]
struct DIDATAFORMAT {
    dw_size: u32,
    dw_obj_size: u32,
    dw_flags: u32,
    dw_data_size: u32,
    dw_num_objs: u32,
    rgodf: *const DIOBJECTDATAFORMAT,
}

#[repr(C)]
struct DIEFFECT {
    dw_size: u32,
    dw_flags: u32,
    dw_duration: u32,
    dw_sample_period: u32,
    dw_gain: u32,
    dw_trigger_button: u32,
    dw_trigger_repeat_interval: u32,
    c_axes: u32,
    rgdw_axes: *mut u32,
    rgl_direction: *mut i32,
    lp_envelope: *mut c_void,
    cb_type_specific_params: u32,
    lp_type_specific_params: *mut c_void,
    dw_start_delay: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct DICONSTANTFORCE {
    l_magnitude: i32,
}

// DIJOYSTATE2 structure - matches the standard DirectInput joystick state
#[repr(C)]
struct DIJOYSTATE2 {
    lx: i32,
    ly: i32,
    lz: i32,
    lrx: i32,
    lry: i32,
    lrz: i32,
    rgl_slider: [i32; 2],
    rgdw_pov: [u32; 4],
    rgb_buttons: [u8; 128],
    lv_x: i32,
    lv_y: i32,
    lv_z: i32,
    lv_rx: i32,
    lv_ry: i32,
    lv_rz: i32,
    rgl_v_slider: [i32; 2],
    la_x: i32,
    la_y: i32,
    la_z: i32,
    la_rx: i32,
    la_ry: i32,
    la_rz: i32,
    rgl_a_slider: [i32; 2],
    lf_x: i32,
    lf_y: i32,
    lf_z: i32,
    lf_rx: i32,
    lf_ry: i32,
    lf_rz: i32,
    rgl_f_slider: [i32; 2],
}

// COM interface definitions
#[repr(C)]
struct IDirectInput8WVtbl {
    query_interface: *const c_void,
    add_ref: *const c_void,
    release: unsafe extern "system" fn(*mut IDirectInput8W) -> u32,
    create_device: unsafe extern "system" fn(
        *mut IDirectInput8W,
        *const GUID,
        *mut *mut IDirectInputDevice8W,
        *mut c_void,
    ) -> HRESULT,
    enum_devices: unsafe extern "system" fn(
        *mut IDirectInput8W,
        u32,
        unsafe extern "system" fn(*const DIDEVICEINSTANCEW, *mut c_void) -> DIBOOL,
        *mut c_void,
        u32,
    ) -> HRESULT,
}

#[repr(C)]
struct IDirectInput8W {
    lpvtbl: *const IDirectInput8WVtbl,
}

#[repr(C)]
struct IDirectInputDevice8WVtbl {
    query_interface: *const c_void,
    add_ref: *const c_void,
    release: unsafe extern "system" fn(*mut IDirectInputDevice8W) -> u32,
    get_capabilities: *const c_void,
    enum_objects: *const c_void,
    get_property: *const c_void,
    set_property: unsafe extern "system" fn(
        *mut IDirectInputDevice8W,
        *const GUID,
        *const DIPROPHEADER,
    ) -> HRESULT,
    acquire: unsafe extern "system" fn(*mut IDirectInputDevice8W) -> HRESULT,
    unacquire: unsafe extern "system" fn(*mut IDirectInputDevice8W) -> HRESULT,
    get_device_state: *const c_void,
    get_device_data: *const c_void,
    set_data_format: unsafe extern "system" fn(*mut IDirectInputDevice8W, *const DIDATAFORMAT) -> HRESULT,
    set_event_notification: *const c_void,
    set_cooperative_level: unsafe extern "system" fn(*mut IDirectInputDevice8W, HWND, u32) -> HRESULT,
    get_object_info: *const c_void,
    get_device_info: *const c_void,
    run_control_panel: *const c_void,
    initialize: *const c_void,
    create_effect: unsafe extern "system" fn(
        *mut IDirectInputDevice8W,
        *const GUID,
        *const DIEFFECT,
        *mut *mut IDirectInputEffect,
        *mut c_void,
    ) -> HRESULT,
    enum_effects: *const c_void,
    get_effect_info: *const c_void,
    get_force_feedback_state: *const c_void,
    send_force_feedback_command: *const c_void,
    enum_created_effect_objects: *const c_void,
    escape: *const c_void,
    poll: unsafe extern "system" fn(*mut IDirectInputDevice8W) -> HRESULT,
}

#[repr(C)]
struct IDirectInputDevice8W {
    lpvtbl: *const IDirectInputDevice8WVtbl,
}

#[repr(C)]
struct IDirectInputEffectVtbl {
    query_interface: *const c_void,
    add_ref: *const c_void,
    release: unsafe extern "system" fn(*mut IDirectInputEffect) -> u32,
    initialize: *const c_void,
    get_effect_guid: *const c_void,
    get_parameters: *const c_void,
    set_parameters: unsafe extern "system" fn(*mut IDirectInputEffect, *const DIEFFECT, u32) -> HRESULT,
    start: unsafe extern "system" fn(*mut IDirectInputEffect, u32, u32) -> HRESULT,
    stop: unsafe extern "system" fn(*mut IDirectInputEffect) -> HRESULT,
    get_effect_status: *const c_void,
    download: *const c_void,
    unload: unsafe extern "system" fn(*mut IDirectInputEffect) -> HRESULT,
}

#[repr(C)]
struct IDirectInputEffect {
    lpvtbl: *const IDirectInputEffectVtbl,
}

type DirectInput8CreateFn = unsafe extern "system" fn(
    HMODULE,
    u32,
    *const GUID,
    *mut *mut IDirectInput8W,
    *mut c_void,
) -> HRESULT;

// Static data format definition (equivalent to c_dfDIJoystick2)
// We store this in a struct to keep the data alive
struct JoystickDataFormat {
    objects: [DIOBJECTDATAFORMAT; 164],
    format: DIDATAFORMAT,
}

impl JoystickDataFormat {
    fn new() -> Self {
        let mut objects: [DIOBJECTDATAFORMAT; 164] = [DIOBJECTDATAFORMAT {
            pguid: ptr::null(),
            dw_ofs: 0,
            dw_type: 0,
            dw_flags: 0,
        }; 164];

        let mut idx = 0;

        // 8 axes (X, Y, Z, Rx, Ry, Rz, Slider0, Slider1)
        let axis_guids: [*const GUID; 8] = [
            &GUID_XAXIS, &GUID_YAXIS, &GUID_ZAXIS,
            &GUID_RXAXIS, &GUID_RYAXIS, &GUID_RZAXIS,
            &GUID_SLIDER, &GUID_SLIDER,
        ];
        let axis_offsets: [u32; 8] = [0, 4, 8, 12, 16, 20, 24, 28];
        for i in 0..8 {
            objects[idx] = DIOBJECTDATAFORMAT {
                pguid: axis_guids[i],
                dw_ofs: axis_offsets[i],
                dw_type: DIDFT_OPTIONAL | DIDFT_AXIS | DIDFT_ANYINSTANCE,
                dw_flags: 0,
            };
            idx += 1;
        }

        // 4 POV hats (offsets 32, 36, 40, 44)
        for i in 0..4 {
            objects[idx] = DIOBJECTDATAFORMAT {
                pguid: &GUID_POV,
                dw_ofs: 32 + (i * 4) as u32,
                dw_type: DIDFT_OPTIONAL | DIDFT_POV | DIDFT_ANYINSTANCE,
                dw_flags: 0,
            };
            idx += 1;
        }

        // 128 buttons (offsets 48-175)
        for i in 0..128 {
            objects[idx] = DIOBJECTDATAFORMAT {
                pguid: &GUID_BUTTON,
                dw_ofs: 48 + i as u32,
                dw_type: DIDFT_OPTIONAL | DIDFT_BUTTON | DIDFT_ANYINSTANCE,
                dw_flags: 0,
            };
            idx += 1;
        }

        // Velocity axes (8 more)
        for i in 0..8 {
            objects[idx] = DIOBJECTDATAFORMAT {
                pguid: axis_guids[i],
                dw_ofs: 176 + (i * 4) as u32,
                dw_type: DIDFT_OPTIONAL | DIDFT_AXIS | DIDFT_ANYINSTANCE,
                dw_flags: 0,
            };
            idx += 1;
        }

        // Acceleration axes (8 more)
        for i in 0..8 {
            objects[idx] = DIOBJECTDATAFORMAT {
                pguid: axis_guids[i],
                dw_ofs: 208 + (i * 4) as u32,
                dw_type: DIDFT_OPTIONAL | DIDFT_AXIS | DIDFT_ANYINSTANCE,
                dw_flags: 0,
            };
            idx += 1;
        }

        // Force axes (8 more)
        for i in 0..8 {
            objects[idx] = DIOBJECTDATAFORMAT {
                pguid: axis_guids[i],
                dw_ofs: 240 + (i * 4) as u32,
                dw_type: DIDFT_OPTIONAL | DIDFT_AXIS | DIDFT_ANYINSTANCE,
                dw_flags: 0,
            };
            idx += 1;
        }

        let mut df = Self {
            objects,
            format: DIDATAFORMAT {
                dw_size: std::mem::size_of::<DIDATAFORMAT>() as u32,
                dw_obj_size: std::mem::size_of::<DIOBJECTDATAFORMAT>() as u32,
                dw_flags: DIDF_ABSAXIS,
                dw_data_size: std::mem::size_of::<DIJOYSTATE2>() as u32,
                dw_num_objs: 164,
                rgodf: ptr::null(),
            },
        };
        df.format.rgodf = df.objects.as_ptr();
        df
    }

    fn as_ptr(&self) -> *const DIDATAFORMAT {
        &self.format
    }
}

pub struct ForceFeedbackDevice {
    dinput: *mut IDirectInput8W,
    device: *mut IDirectInputDevice8W,
    effect: *mut IDirectInputEffect,
    available: bool,
    effect_started: bool,
    com_initialized: bool,
    // Keep these alive for the effect
    axes: [u32; 1],
    directions: [i32; 1],
    constant_force: DICONSTANTFORCE,
    // Keep data format alive
    _data_format: Option<Box<JoystickDataFormat>>,
    // Message-only window for DirectInput cooperative level
    message_window: HWND,
}

/// Creates a message-only window for DirectInput cooperative level.
/// Message-only windows are invisible and belong to the calling process,
/// which is required for exclusive device access.
fn create_message_window() -> anyhow::Result<HWND> {
    unsafe {
        let class_name: Vec<u16> = "RoWheelDIWindow\0".encode_utf16().collect();
        let hinstance = GetModuleHandleW(None)?;

        // Window procedure - just pass everything to default handler
        unsafe extern "system" fn wnd_proc(
            hwnd: HWND,
            msg: u32,
            wparam: WPARAM,
            lparam: LPARAM,
        ) -> LRESULT {
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }

        let wc = WNDCLASSW {
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinstance.into(),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };

        // RegisterClass may fail if already registered, that's ok
        let _ = RegisterClassW(&wc);

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            PCWSTR(class_name.as_ptr()),
            PCWSTR::null(),
            WINDOW_STYLE::default(),
            0, 0, 0, 0,
            Some(HWND_MESSAGE), // Parent = HWND_MESSAGE makes it a message-only window
            None,
            Some(hinstance.into()),
            None,
        )?;

        if hwnd.0.is_null() {
            return Err(anyhow::anyhow!("Failed to create message-only window"));
        }

        log::info!("Created message-only window for DirectInput");
        Ok(hwnd)
    }
}

unsafe impl Send for ForceFeedbackDevice {}

impl ForceFeedbackDevice {
    pub fn new(_device_path: Option<&str>) -> anyhow::Result<Self> {
        let mut ff = Self {
            dinput: ptr::null_mut(),
            device: ptr::null_mut(),
            effect: ptr::null_mut(),
            available: false,
            effect_started: false,
            com_initialized: false,
            axes: [0],
            directions: [0],
            constant_force: DICONSTANTFORCE { l_magnitude: 0 },
            _data_format: None,
            message_window: HWND::default(),
        };

        unsafe {
            let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
            if hr.is_ok() || hr.0 == 1 {
                ff.com_initialized = true;
            }
        }

        // Create message-only window for DirectInput cooperative level
        match create_message_window() {
            Ok(hwnd) => ff.message_window = hwnd,
            Err(e) => log::warn!("Failed to create message window: {}", e),
        }

        if let Err(e) = ff.initialize() {
            log::warn!("Failed to initialize DirectInput force feedback: {}", e);
        }

        Ok(ff)
    }

    fn initialize(&mut self) -> anyhow::Result<()> {
        unsafe {
            let dinput_name: Vec<u16> = "dinput8.dll\0".encode_utf16().collect();
            let hinst = LoadLibraryW(PCWSTR(dinput_name.as_ptr()))?;

            let proc_name = b"DirectInput8Create\0";
            let create_fn: DirectInput8CreateFn = match GetProcAddress(hinst, PCSTR(proc_name.as_ptr())) {
                Some(p) => std::mem::transmute(p),
                None => return Err(anyhow::anyhow!("Failed to get DirectInput8Create")),
            };

            let hmodule = GetModuleHandleW(None)?;
            let hr = create_fn(
                hmodule,
                DIRECTINPUT_VERSION,
                &IID_IDIRECTINPUT8W,
                &mut self.dinput,
                ptr::null_mut(),
            );
            if hr.is_err() {
                return Err(anyhow::anyhow!("DirectInput8Create failed: {:?}", hr));
            }

            log::info!("DirectInput8 initialized");

            self.find_ff_device()?;

            let dev_vtbl = &*(*self.device).lpvtbl;

            // Set data format BEFORE cooperative level and acquire
            let data_format = Box::new(JoystickDataFormat::new());
            let hr = (dev_vtbl.set_data_format)(self.device, data_format.as_ptr());
            if hr.is_err() {
                return Err(anyhow::anyhow!("SetDataFormat failed: {:?}", hr));
            }
            self._data_format = Some(data_format);
            log::info!("Data format set");

            // Set cooperative level - EXCLUSIVE required for FF
            // Use our message-only window which belongs to this process
            if self.message_window.0.is_null() {
                return Err(anyhow::anyhow!("No valid window handle for DirectInput"));
            }

            let hr = (dev_vtbl.set_cooperative_level)(self.device, self.message_window, DISCL_EXCLUSIVE | DISCL_BACKGROUND);
            if hr.is_err() {
                // Try non-exclusive as fallback (FF might still work on some drivers)
                log::warn!("Exclusive mode failed: {:?}, trying non-exclusive", hr);
                let hr2 = (dev_vtbl.set_cooperative_level)(self.device, self.message_window, DISCL_NONEXCLUSIVE | DISCL_BACKGROUND);
                if hr2.is_err() {
                    return Err(anyhow::anyhow!("SetCooperativeLevel failed: {:?}", hr2));
                }
                log::info!("Cooperative level set (NonExclusive|Background) - FF may be limited");
            } else {
                log::info!("Cooperative level set (Exclusive|Background)");
            }

            // Disable auto-center BEFORE acquiring - some drivers require this
            self.disable_auto_center();

            // Acquire the device
            let hr = (dev_vtbl.acquire)(self.device);
            if hr.is_err() {
                return Err(anyhow::anyhow!("Acquire failed: {:?}", hr));
            }
            log::info!("Device acquired");

            self.create_effect()?;

            self.available = true;
            log::info!("DirectInput force feedback ready");
            Ok(())
        }
    }

    unsafe fn find_ff_device(&mut self) -> anyhow::Result<()> {
        struct EnumContext {
            dinput: *mut IDirectInput8W,
            device: *mut IDirectInputDevice8W,
            found: bool,
        }

        unsafe extern "system" fn enum_callback(
            device_instance: *const DIDEVICEINSTANCEW,
            context: *mut c_void,
        ) -> DIBOOL {
            let ctx = &mut *(context as *mut EnumContext);
            let inst = &*device_instance;

            let name_len = inst.tsz_product_name.iter().position(|&c| c == 0).unwrap_or(260);
            let name = String::from_utf16_lossy(&inst.tsz_product_name[..name_len]);
            log::info!("Found FF device: {}", name);

            let vtbl = &*(*ctx.dinput).lpvtbl;
            let hr = (vtbl.create_device)(ctx.dinput, &inst.guid_instance, &mut ctx.device, ptr::null_mut());

            if hr.is_ok() && !ctx.device.is_null() {
                log::info!("Successfully created device: {}", name);
                ctx.found = true;
                DIENUM_STOP
            } else {
                DIENUM_CONTINUE
            }
        }

        let mut ctx = EnumContext {
            dinput: self.dinput,
            device: ptr::null_mut(),
            found: false,
        };

        let vtbl = &*(*self.dinput).lpvtbl;
        let hr = (vtbl.enum_devices)(
            self.dinput,
            DI8DEVCLASS_GAMECTRL,
            enum_callback,
            &mut ctx as *mut _ as *mut c_void,
            DIEDFL_FORCEFEEDBACK,
        );

        if hr.is_err() || !ctx.found {
            return Err(anyhow::anyhow!("No force feedback device found"));
        }

        self.device = ctx.device;
        Ok(())
    }

    unsafe fn disable_auto_center(&self) {
        if self.device.is_null() {
            return;
        }

        // DIPROP_AUTOCENTER = MAKEDIPROP(9) - cast integer 9 to pseudo-GUID pointer
        let diprop_autocenter: *const GUID = 9 as *const GUID;

        let prop = DIPROPDWORD {
            diph: DIPROPHEADER {
                dw_size: std::mem::size_of::<DIPROPDWORD>() as u32,
                dw_header_size: std::mem::size_of::<DIPROPHEADER>() as u32,
                dw_obj: 0,
                dw_how: DIPH_DEVICE,
            },
            dw_data: DIPROPAUTOCENTER_OFF,
        };

        let dev_vtbl = &*(*self.device).lpvtbl;
        let hr = (dev_vtbl.set_property)(self.device, diprop_autocenter, &prop.diph);
        if hr.is_ok() {
            log::info!("Auto-center disabled");
        } else {
            log::warn!("Failed to disable auto-center: {:?}", hr);
        }
    }

    fn create_effect(&mut self) -> anyhow::Result<()> {
        unsafe {
            // DIJOFS_X = 0 (offset of X axis in DIJOYSTATE2)
            self.axes[0] = 0;
            self.directions[0] = 0;
            self.constant_force.l_magnitude = 0;

            let effect = DIEFFECT {
                dw_size: std::mem::size_of::<DIEFFECT>() as u32,
                dw_flags: DIEFF_CARTESIAN | DIEFF_OBJECTOFFSETS,
                dw_duration: u32::MAX,
                dw_sample_period: 0,
                dw_gain: 10000,
                dw_trigger_button: u32::MAX,
                dw_trigger_repeat_interval: 0,
                c_axes: 1,
                rgdw_axes: self.axes.as_mut_ptr(),
                rgl_direction: self.directions.as_mut_ptr(),
                lp_envelope: ptr::null_mut(),
                cb_type_specific_params: std::mem::size_of::<DICONSTANTFORCE>() as u32,
                lp_type_specific_params: &mut self.constant_force as *mut _ as *mut c_void,
                dw_start_delay: 0,
            };

            let dev_vtbl = &*(*self.device).lpvtbl;
            let hr = (dev_vtbl.create_effect)(
                self.device,
                &GUID_CONSTANT_FORCE,
                &effect,
                &mut self.effect,
                ptr::null_mut(),
            );

            if hr.is_err() || self.effect.is_null() {
                return Err(anyhow::anyhow!("CreateEffect failed: {:?}", hr));
            }

            log::info!("Constant force effect created");

            let eff_vtbl = &*(*self.effect).lpvtbl;
            let hr = (eff_vtbl.start)(self.effect, 1, 0);
            if hr.is_ok() || hr.0 == DI_OK {
                self.effect_started = true;
                log::info!("Effect started");
            } else {
                log::warn!("Effect start returned: {:?}", hr);
            }

            Ok(())
        }
    }

    fn update_force(&mut self, magnitude: i32) -> anyhow::Result<()> {
        if self.effect.is_null() || !self.available {
            return Ok(());
        }

        unsafe {
            // Poll the device first - required by some hardware for FF to work
            let dev_vtbl = &*(*self.device).lpvtbl;
            let _ = (dev_vtbl.poll)(self.device);

            self.constant_force.l_magnitude = magnitude;
            self.directions[0] = 0;

            let effect = DIEFFECT {
                dw_size: std::mem::size_of::<DIEFFECT>() as u32,
                dw_flags: DIEFF_CARTESIAN | DIEFF_OBJECTOFFSETS,
                dw_duration: u32::MAX,
                dw_sample_period: 0,
                dw_gain: 10000,
                dw_trigger_button: u32::MAX,
                dw_trigger_repeat_interval: 0,
                c_axes: 1,
                rgdw_axes: self.axes.as_mut_ptr(),
                rgl_direction: self.directions.as_mut_ptr(),
                lp_envelope: ptr::null_mut(),
                cb_type_specific_params: std::mem::size_of::<DICONSTANTFORCE>() as u32,
                lp_type_specific_params: &mut self.constant_force as *mut _ as *mut c_void,
                dw_start_delay: 0,
            };

            let eff_vtbl = &*(*self.effect).lpvtbl;
            let flags = DIEP_DIRECTION | DIEP_TYPESPECIFICPARAMS | DIEP_START;
            let hr = (eff_vtbl.set_parameters)(self.effect, &effect, flags);

            if hr.is_err() && hr.0 != DI_OK {
                // Re-acquire the device and poll before retrying
                let _ = (dev_vtbl.acquire)(self.device);
                let _ = (dev_vtbl.poll)(self.device);
                let hr2 = (eff_vtbl.set_parameters)(self.effect, &effect, flags);
                if hr2.is_err() && hr2.0 != DI_OK {
                    log::warn!("SetParameters failed: {:?}", hr2);
                }
            }
        }

        Ok(())
    }
}

impl ForceFeedback for ForceFeedbackDevice {
    fn apply_rumble(&mut self, rumble: &RumbleState) -> anyhow::Result<()> {
        if !self.available {
            return Ok(());
        }

        let large = rumble.large_motor.clamp(0.0, 1.0);
        let small = rumble.small_motor.clamp(0.0, 1.0);
        let magnitude = ((large - small) * 10000.0) as i32;

        log::info!("Force feedback: large={:.3}, small={:.3} → magnitude={}", large, small, magnitude);

        self.update_force(magnitude)
    }

    fn stop(&mut self) -> anyhow::Result<()> {
        if !self.available || self.effect.is_null() {
            return Ok(());
        }

        unsafe {
            let eff_vtbl = &*(*self.effect).lpvtbl;
            let _ = (eff_vtbl.stop)(self.effect);
            self.effect_started = false;
            log::debug!("Force effect stopped");
        }

        Ok(())
    }

    fn is_available(&self) -> bool {
        self.available
    }
}

impl Drop for ForceFeedbackDevice {
    fn drop(&mut self) {
        let _ = self.stop();

        unsafe {
            if !self.effect.is_null() {
                let eff_vtbl = &*(*self.effect).lpvtbl;
                let _ = (eff_vtbl.unload)(self.effect);
                (eff_vtbl.release)(self.effect);
            }

            if !self.device.is_null() {
                let dev_vtbl = &*(*self.device).lpvtbl;
                let _ = (dev_vtbl.unacquire)(self.device);
                (dev_vtbl.release)(self.device);
            }

            if !self.dinput.is_null() {
                let vtbl = &*(*self.dinput).lpvtbl;
                (vtbl.release)(self.dinput);
            }

            // Destroy the message-only window
            if !self.message_window.0.is_null() {
                let _ = DestroyWindow(self.message_window);
            }

            if self.com_initialized {
                CoUninitialize();
            }
        }
    }
}
