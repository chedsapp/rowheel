use super::{RumbleState, VirtualController, XboxControllerState};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::path::PathBuf;

// uinput constants
const UINPUT_PATH: &str = "/dev/uinput";

// Input event types
const EV_SYN: u16 = 0x00;
const EV_KEY: u16 = 0x01;
const EV_ABS: u16 = 0x03;
const EV_FF: u16 = 0x15;
const EV_UINPUT: u16 = 0x0101;

// Synchronization events
const SYN_REPORT: u16 = 0x00;

// Force feedback effect types
const FF_RUMBLE: u16 = 0x50;

// UI event codes
const UI_FF_UPLOAD: u16 = 1;
const UI_FF_ERASE: u16 = 2;

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
const UI_SET_FFBIT: libc::c_ulong = 0x4004556b;

// Force feedback ioctl codes
const UI_BEGIN_FF_UPLOAD: libc::c_ulong = 0xc06855c8;
const UI_END_FF_UPLOAD: libc::c_ulong = 0x406855c9;
const UI_BEGIN_FF_ERASE: libc::c_ulong = 0xc00455ca;
const UI_END_FF_ERASE: libc::c_ulong = 0x400455cb;

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

// Simplified FF effect struct (we only care about rumble)
// Must match Linux kernel struct ff_effect layout exactly
// On 64-bit systems, the union needs 8-byte alignment due to pointer members
#[repr(C)]
struct FFEffect {
    type_: u16,        // offset 0
    id: i16,           // offset 2
    direction: u16,    // offset 4
    trigger: [u8; 4],  // offset 6: ff_trigger (button u16 + interval u16)
    replay: [u8; 4],   // offset 10: ff_replay (length u16 + delay u16)
    _pad: [u8; 2],     // offset 14: padding for 8-byte alignment of union
    u: [u8; 48],       // offset 16: union (needs to be large enough for ff_periodic_effect)
}

// New struct for ff_rumble
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct FFRumbleEffect {
    strong_magnitude: u16,
    weak_magnitude: u16,
}

#[repr(C)]
struct UinputFFUpload {
    request_id: u32,
    retval: i32,
    effect: FFEffect,
    old: FFEffect,
}

#[repr(C)]
struct UinputFFErase {
    request_id: u32,
    retval: i32,
    effect_id: u32,
}

pub struct VirtualXboxController {
    uinput_file: Option<File>, // Renamed from 'file' for clarity
    virtual_device_file: Option<File>, // New field to hold the opened eventX file for reading FF events
    connected: bool,
    rumble_state: Arc<Mutex<RumbleState>>,
    ff_effects: Arc<Mutex<HashMap<i16, FFRumbleEffect>>>,
    // udev: libudev::Context, // Remove udev context
    virtual_device_path: Option<PathBuf>, // To store the determined eventX path
    ff_thread_handle: Option<JoinHandle<()>>,
    ff_thread_running: Arc<std::sync::atomic::AtomicBool>,
}

impl VirtualXboxController {
    pub fn new() -> anyhow::Result<Self> {
        // let udev_context = libudev::Context::new()?; // Initialize udev here

        let mut controller = Self {
            uinput_file: None,
            virtual_device_file: None,
            connected: false,
            rumble_state: Arc::new(Mutex::new(RumbleState::default())),
            ff_effects: Arc::new(Mutex::new(HashMap::new())),
            // udev: udev_context, // Assign udev context
            virtual_device_path: None,
            ff_thread_handle: None,
            ff_thread_running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        };

        controller.connect()?;
        Ok(controller)
    }

    // Helper to find the evdev path of the newly created virtual device
    fn find_virtual_device_path() -> anyhow::Result<PathBuf> {
        use std::ffi::CStr;

        const DEVICE_NAME: &str = "RoWheel Virtual Xbox Controller";
        const EVIOCGNAME_LEN: usize = 256;

        // Retry loop for device detection
        let start_time = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(5);
        let retry_delay = std::time::Duration::from_millis(100);

        while start_time.elapsed() < timeout {
            // Scan /dev/input/event* devices directly
            for entry in std::fs::read_dir("/dev/input")? {
                let entry = entry?;
                let path = entry.path();

                // Only check event devices
                if !path.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with("event"))
                    .unwrap_or(false)
                {
                    continue;
                }

                // Try to open the device
                if let Ok(file) = std::fs::File::open(&path) {
                    let fd = file.as_raw_fd();

                    // Get device name using EVIOCGNAME ioctl
                    let mut name_buf = [0u8; EVIOCGNAME_LEN];
                    let result = unsafe {
                        libc::ioctl(
                            fd,
                            // EVIOCGNAME(len) = _IOC(_IOC_READ, 'E', 0x06, len)
                            (0x80004506 | ((EVIOCGNAME_LEN as u64) << 16)) as libc::c_ulong,
                            name_buf.as_mut_ptr()
                        )
                    };

                    if result >= 0 {
                        // Convert to string and check if it matches
                        if let Ok(device_name) = CStr::from_bytes_until_nul(&name_buf) {
                            if let Ok(device_name_str) = device_name.to_str() {
                                log::debug!("Checking device {}: name=\"{}\"", path.display(), device_name_str);

                                if device_name_str == DEVICE_NAME {
                                    log::info!("Found virtual device at: {}", path.display());
                                    return Ok(path);
                                }
                            }
                        }
                    }
                }
            }

            log::debug!("Virtual device not found yet, retrying...");
            std::thread::sleep(retry_delay);
        }

        Err(anyhow::anyhow!("Virtual device evdev path not found within timeout"))
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

        // Make uinput file descriptor non-blocking for FF event polling
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
            // Set event types
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

            log::info!("Force feedback capabilities set: EV_FF and FF_RUMBLE");

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

        // Find the newly created virtual device's evdev path
        let virtual_device_path = VirtualXboxController::find_virtual_device_path()?;
        self.virtual_device_path = Some(virtual_device_path.clone());

        // Note: We don't need to open the virtual device's event file anymore
        // since FF events come through the uinput fd, not the event device fd

        self.uinput_file = Some(uinput_file);
        self.virtual_device_file = None; // Not needed
        self.connected = true;

        log::info!("Virtual Xbox controller created via uinput with force feedback support");

        // Start the FF polling thread
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

        // Mark thread as running
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
                    libc::poll(&mut pollfd as *mut libc::pollfd, 1, 100) // 100ms timeout
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
                    // Timeout, continue loop to check if we should exit
                    continue;
                }

                // Read all available events
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

                                // End upload
                                if unsafe { libc::ioctl(thread_fd, UI_END_FF_UPLOAD, &upload as *const _) } < 0 {
                                    log::error!("UI_END_FF_UPLOAD failed: {}", std::io::Error::last_os_error());
                                } else {
                                    log::info!("FF effect {} uploaded successfully", upload.effect.id);
                                }
                            }
                            UI_FF_ERASE => {
                                log::debug!("FF erase request received (request_id={})", event.value);
                                let mut erase: UinputFFErase = unsafe { std::mem::zeroed() };
                                erase.request_id = event.value as u32;
                                // effect_id is filled in by UI_BEGIN_FF_ERASE

                                // Begin erase - kernel fills in effect_id
                                if unsafe { libc::ioctl(thread_fd, UI_BEGIN_FF_ERASE, &mut erase as *mut _) } < 0 {
                                    log::error!("UI_BEGIN_FF_ERASE failed: {}", std::io::Error::last_os_error());
                                    continue;
                                }

                                log::debug!("Erasing FF effect ID {}", erase.effect_id);

                                // Remove from stored effects
                                if let Ok(mut effects) = ff_effects.lock() {
                                    if effects.remove(&(erase.effect_id as i16)).is_some() {
                                        log::debug!("Removed FF effect ID {} from cache", erase.effect_id);
                                    }
                                }

                                // Accept the erase
                                erase.retval = 0;

                                // End erase
                                if unsafe { libc::ioctl(thread_fd, UI_END_FF_ERASE, &erase as *const _) } < 0 {
                                    log::error!("UI_END_FF_ERASE failed: {}", std::io::Error::last_os_error());
                                } else {
                                    log::debug!("FF effect {} erased successfully", erase.effect_id);
                                }
                            }
                            _ => {}
                        }
                    }
                    // Handle FF play events
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

            // Close the duplicated file descriptor
            unsafe {
                libc::close(thread_fd);
            }

            log::info!("FF polling thread stopped");
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
        // Signal the FF polling thread to stop
        self.ff_thread_running.store(false, std::sync::atomic::Ordering::SeqCst);

        // Wait for the thread to finish
        if let Some(handle) = self.ff_thread_handle.take() {
            if let Err(e) = handle.join() {
                log::error!("FF polling thread panicked: {:?}", e);
            }
        }

        // Destroy the uinput device
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

        // FF events are now polled in a dedicated background thread

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
