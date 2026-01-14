use super::uinput_ffi::*;
use super::{RumbleState, VirtualController, XboxControllerState};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

pub struct VirtualXboxController {
    uinput_file: Option<File>,
    connected: bool,
    rumble_state: Arc<Mutex<RumbleState>>,
    ff_effects: Arc<Mutex<HashMap<i16, FFRumbleEffect>>>,
    ff_thread_handle: Option<JoinHandle<()>>,
    ff_thread_running: Arc<std::sync::atomic::AtomicBool>,
}

impl VirtualXboxController {
    pub fn new() -> anyhow::Result<Self> {
        let mut controller = Self {
            uinput_file: None,
            connected: false,
            rumble_state: Arc::new(Mutex::new(RumbleState::default())),
            ff_effects: Arc::new(Mutex::new(HashMap::new())),
            ff_thread_handle: None,
            ff_thread_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        };

        controller.connect()?;
        Ok(controller)
    }

    fn connect(&mut self) -> anyhow::Result<()> {
        let uinput_file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(UINPUT_PATH)
            .map_err(|e| anyhow::anyhow!(
                "Failed to open {}: {}. Try: sudo chmod 666 /dev/uinput",
                UINPUT_PATH, e
            ))?;

        let uinput_fd = uinput_file.as_raw_fd();

        // Make sure uinput can't block when we try to poll things
        unsafe {
            let flags = libc::fcntl(uinput_fd, libc::F_GETFL);
            if flags < 0 {
                return Err(anyhow::anyhow!("Failed to get uinput file flags"));
            }
            if libc::fcntl(uinput_fd, libc::F_SETFL, flags | libc::O_NONBLOCK) < 0 {
                return Err(anyhow::anyhow!("Failed to set uinput non-blocking mode"));
            }
        }

        unsafe {
            // Enable event types
            if libc::ioctl(uinput_fd, UI_SET_EVBIT, EV_KEY as libc::c_int) < 0 {
                return Err(anyhow::anyhow!("Failed to set EV_KEY"));
            }
            if libc::ioctl(uinput_fd, UI_SET_EVBIT, EV_ABS as libc::c_int) < 0 {
                return Err(anyhow::anyhow!("Failed to set EV_ABS"));
            }
            if libc::ioctl(uinput_fd, UI_SET_EVBIT, EV_SYN as libc::c_int) < 0 {
                return Err(anyhow::anyhow!("Failed to set EV_SYN"));
            }
            // Enable force feedback support
            if libc::ioctl(uinput_fd, UI_SET_EVBIT, EV_FF as libc::c_int) < 0 {
                return Err(anyhow::anyhow!("Failed to set EV_FF"));
            }

            // Set buttons
            for btn in [BTN_A, BTN_B, BTN_X, BTN_Y, BTN_TL, BTN_TR,
                        BTN_SELECT, BTN_START, BTN_MODE, BTN_THUMBL, BTN_THUMBR] {
                if libc::ioctl(uinput_fd, UI_SET_KEYBIT, btn as libc::c_int) < 0 {
                    return Err(anyhow::anyhow!("Failed to set button {}", btn));
                }
            }

            // Set absolute axes
            for axis in [ABS_X, ABS_Y, ABS_RX, ABS_RY, ABS_Z, ABS_RZ, ABS_HAT0X, ABS_HAT0Y] {
                if libc::ioctl(uinput_fd, UI_SET_ABSBIT, axis as libc::c_int) < 0 {
                    return Err(anyhow::anyhow!("Failed to set axis {}", axis));
                }
            }

            // Set force feedback effect type
            if libc::ioctl(uinput_fd, UI_SET_FFBIT, FF_RUMBLE as libc::c_int) < 0 {
                return Err(anyhow::anyhow!("Failed to set FF_RUMBLE"));
            }

            // Create device struct
            let mut dev: UinputUserDev = std::mem::zeroed();
            let name = b"RoWheel Virtual Xbox Controller";
            dev.name[..name.len()].copy_from_slice(name);
            dev.id.bustype = 0x03; // BUS_USB
            dev.id.vendor = 0x045e; // Microsoft
            dev.id.product = 0x028e; // Xbox 360 Controller
            dev.id.version = 0x0110;
            dev.ff_effects_max = 16; // Support up to 16 force feedback effects

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

            if libc::write(uinput_fd, dev_bytes.as_ptr() as *const libc::c_void, dev_bytes.len()) < 0 {
                return Err(anyhow::anyhow!("Failed to write device struct"));
            }

            // Create device
            if libc::ioctl(uinput_fd, UI_DEV_CREATE) < 0 {
                return Err(anyhow::anyhow!("Failed to create device: {}", std::io::Error::last_os_error()));
            }
        }

        self.uinput_file = Some(uinput_file);
        self.connected = true;

        log::info!("Uinput gamepad created");

        self.start_ff_polling_thread()?;

        Ok(())
    }

    fn start_ff_polling_thread(&mut self) -> anyhow::Result<()> {
        let fd = match self.uinput_file.as_ref() {
            Some(f) => f.as_raw_fd(),
            None => return Err(anyhow::anyhow!("No uinput file available")),
        };

        // Duplicate the file descriptor so the thread has its own reference
        let thread_fd = unsafe { libc::dup(fd) };
        if thread_fd < 0 {
            return Err(anyhow::anyhow!("Failed to duplicate uinput fd: {}", std::io::Error::last_os_error()));
        }

        let rumble_state = Arc::clone(&self.rumble_state);
        let ff_effects = Arc::clone(&self.ff_effects);
        let running = Arc::clone(&self.ff_thread_running);

        running.store(true, std::sync::atomic::Ordering::SeqCst);

        let handle = std::thread::spawn(move || {
            log::info!("FF polling thread started with fd={}", thread_fd);
            let mut buffer = [0u8; std::mem::size_of::<InputEvent>()];

            while running.load(std::sync::atomic::Ordering::SeqCst) {
                // Use poll to wait for events with timeout
                let mut pollfd = libc::pollfd {
                    fd: thread_fd,
                    events: libc::POLLIN,
                    revents: 0,
                };

                let poll_result = unsafe {
                    libc::poll(&mut pollfd as *mut libc::pollfd, 1, 100) // 100ms
                };

                if poll_result < 0 {
                    let err = std::io::Error::last_os_error();
                    if err.kind() != std::io::ErrorKind::Interrupted {
                        log::error!("Poll error in FF thread: {}", err);
                        break;
                    }
                    continue;
                }

                if poll_result == 0 {
                    continue;
                }

                // Read all events
                loop {
                    let result = unsafe {
                        libc::read(
                            thread_fd,
                            buffer.as_mut_ptr() as *mut libc::c_void,
                            buffer.len()
                        )
                    };

                    if result < 0 {
                        let err = std::io::Error::last_os_error();
                        if err.kind() == std::io::ErrorKind::WouldBlock {
                            break; // No more events
                        }
                        log::warn!("Error reading FF events: {}", err);
                        break;
                    }

                    if result != buffer.len() as isize {
                        break; // Incomplete read
                    }

                    let event = unsafe {
                        std::ptr::read(buffer.as_ptr() as *const InputEvent)
                    };

                    // Handle UI events (FF upload/erase)
                    if event.type_ == EV_UINPUT {
                        match event.code {
                            UI_FF_UPLOAD => {
                                log::info!("FF upload request received (request_id={})", event.value);
                                let mut upload: UinputFFUpload = unsafe { std::mem::zeroed() };
                                upload.request_id = event.value as u32;

                                // Begin upload
                                if unsafe { libc::ioctl(thread_fd, UI_BEGIN_FF_UPLOAD, &mut upload as *mut _) } < 0 {
                                    log::error!("UI_BEGIN_FF_UPLOAD failed: {}", std::io::Error::last_os_error());
                                    continue;
                                }

                                // Accept the effect
                                upload.retval = 0;

                                // Extract rumble magnitudes and store
                                if upload.effect.type_ == FF_RUMBLE {
                                    // Debug: dump first 8 bytes of union to verify alignment
                                    log::debug!("FF effect union raw bytes: {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x}",
                                               upload.effect.u[0], upload.effect.u[1],
                                               upload.effect.u[2], upload.effect.u[3],
                                               upload.effect.u[4], upload.effect.u[5],
                                               upload.effect.u[6], upload.effect.u[7]);

                                    let rumble_effect: FFRumbleEffect = unsafe {
                                        std::ptr::read_unaligned(upload.effect.u.as_ptr() as *const FFRumbleEffect)
                                    };
                                    if let Ok(mut effects) = ff_effects.lock() {
                                        effects.insert(upload.effect.id, rumble_effect);
                                        log::info!("Stored FF effect ID {} with strong_magnitude={}, weak_magnitude={}",
                                                   upload.effect.id, rumble_effect.strong_magnitude, rumble_effect.weak_magnitude);
                                    }
                                } else {
                                    log::warn!("Uploaded effect is not FF_RUMBLE type: {}", upload.effect.type_);
                                }

                                if unsafe { libc::ioctl(thread_fd, UI_END_FF_UPLOAD, &upload as *const _) } < 0 {
                                    log::error!("UI_END_FF_UPLOAD failed: {}", std::io::Error::last_os_error());
                                } else {
                                    log::info!("FF effect {} uploaded", upload.effect.id);
                                }
                            }
                            UI_FF_ERASE => {
                                let mut erase: UinputFFErase = unsafe { std::mem::zeroed() };
                                erase.request_id = event.value as u32;

                                // kernel fills in effect_id
                                if unsafe { libc::ioctl(thread_fd, UI_BEGIN_FF_ERASE, &mut erase as *mut _) } < 0 {
                                    log::error!("UI_BEGIN_FF_ERASE failed: {}", std::io::Error::last_os_error());
                                    continue;
                                }

                                if let Ok(mut effects) = ff_effects.lock() {
                                    if effects.remove(&(erase.effect_id as i16)).is_some() {
                                        log::debug!("Removed FF effect ID {} from cache", erase.effect_id);
                                    }
                                }

                                erase.retval = 0;

                                if unsafe { libc::ioctl(thread_fd, UI_END_FF_ERASE, &erase as *const _) } < 0 {
                                    log::error!("UI_END_FF_ERASE failed: {}", std::io::Error::last_os_error());
                                }
                            }
                            _ => {}
                        }
                    }
                    else if event.type_ == EV_FF {
                        log::debug!("FF play event: effect_id={}, count={}", event.code, event.value);

                        if event.value > 0 {
                            // Play effect (count > 0)
                            if let Ok(effects) = ff_effects.lock() {
                                if let Some(rumble_effect) = effects.get(&(event.code as i16)) {
                                    if let Ok(mut state) = rumble_state.lock() {
                                        state.large_motor = rumble_effect.strong_magnitude as f32 / u16::MAX as f32;
                                        state.small_motor = rumble_effect.weak_magnitude as f32 / u16::MAX as f32;
                                        log::info!("FF effect {} playing - setting rumble: large={:.2}, small={:.2}",
                                                   event.code, state.large_motor, state.small_motor);
                                    }
                                } else {
                                    log::warn!("EV_FF play event for unknown effect ID {}", event.code);
                                }
                            }
                        } else {
                            // Stop effect (count == 0)
                            if let Ok(mut state) = rumble_state.lock() {
                                state.large_motor = 0.0;
                                state.small_motor = 0.0;
                                log::info!("FF effect {} stopped", event.code);
                            }
                        }
                    }
                }
            }

            unsafe {
                libc::close(thread_fd);
            }

            log::info!("FF polling stopped");
        });

        self.ff_thread_handle = Some(handle);
        Ok(())
    }

    fn write_event(&mut self, type_: u16, code: u16, value: i32) -> anyhow::Result<()> {
        if let Some(ref mut file) = self.uinput_file { // Use uinput_file
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
        if let Some(ref mut file) = self.uinput_file { // Use uinput_file
            file.flush()?;
        }
        Ok(())
    }
}

impl Drop for VirtualXboxController {
    fn drop(&mut self) {
        self.ff_thread_running.store(false, std::sync::atomic::Ordering::SeqCst);

        // Wait for the thread to finish
        if let Some(handle) = self.ff_thread_handle.take() {
            if let Err(e) = handle.join() {
                log::error!("FF polling thread panicked: {:?}", e);
            }
        }

        if let Some(ref file) = self.uinput_file {
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

        // Let's just handle all inputs even if rowheel isnt using them cuz why not

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
        let state = self.rumble_state.lock()
            .map_err(|_| anyhow::anyhow!("Failed to lock rumble state"))?;
        Ok(state.clone())
    }

    fn is_connected(&self) -> bool {
        self.connected
    }
}
