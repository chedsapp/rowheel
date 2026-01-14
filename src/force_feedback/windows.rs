use super::directinput_ffi::*;
use super::{ForceFeedback, RumbleState};
use std::ffi::c_void;
use std::ptr;
use windows::core::{GUID, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, LoadLibraryW};

use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassW, HWND_MESSAGE, WINDOW_EX_STYLE,
    WINDOW_STYLE, WNDCLASSW,
};

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
    axes: [u32; 1],
    directions: [i32; 1],
    constant_force: DICONSTANTFORCE,
    _data_format: Option<Box<JoystickDataFormat>>,
    message_window: HWND,
}

// For some absolutely disgusting reason we need a message-only window for DirectInput coop level and hide it
fn create_message_window() -> anyhow::Result<HWND> {
    unsafe {
        let class_name: Vec<u16> = "RoWheelDIWindow\0".encode_utf16().collect();
        let hinstance = GetModuleHandleW(None)?;

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
            let create_fn = get_directinput8_create(hinst)?;

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

            let data_format = Box::new(JoystickDataFormat::new());
            let hr = (dev_vtbl.set_data_format)(self.device, data_format.as_ptr());
            if hr.is_err() {
                return Err(anyhow::anyhow!("SetDataFormat failed: {:?}", hr));
            }
            self._data_format = Some(data_format);
            log::info!("Data format set");

            // Bro we HAVE to set this to exclusive background for FF to work properly
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

            // Disable auto-center BEFORE acquiring - some drivers require this (Thank you hackerkm I love you very much)
            self.disable_auto_center();

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

    fn build_effect(&mut self) -> DIEFFECT {
        DIEFFECT {
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
        }
    }

    fn create_effect(&mut self) -> anyhow::Result<()> {
        unsafe {
            // DIJOFS_X = 0 (offset of X axis in DIJOYSTATE2)
            self.axes[0] = 0;
            self.directions[0] = 0;
            self.constant_force.l_magnitude = 0;

            let effect = self.build_effect();

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
            // Sometimes we gotta poll the device first to keep it happy
            let dev_vtbl = &*(*self.device).lpvtbl;
            let _ = (dev_vtbl.poll)(self.device);

            self.constant_force.l_magnitude = magnitude;
            self.directions[0] = 0;

            let effect = self.build_effect();

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

            // Kill the god awful window we created for DirectInput
            if !self.message_window.0.is_null() {
                let _ = DestroyWindow(self.message_window);
            }

            if self.com_initialized {
                CoUninitialize();
            }
        }
    }
}
