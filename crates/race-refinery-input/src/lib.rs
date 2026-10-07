//! Background game-controller button bindings.
//!
//! Polls every attached DirectInput game controller (wheels, button boxes,
//! pedals, gamepads) in **background, non-exclusive** mode, so bound buttons
//! fire while iRacing has focus and iRacing still sees every press. Used for
//! the VR recenter binding; the watcher itself knows nothing about VR.

use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use race_refinery_settings::ControllerBinding;

type PressFn = Box<dyn Fn() + Send + Sync>;

struct Inner {
    binding: Mutex<Option<ControllerBinding>>,
    /// Set while the UI waits for "press a button to bind".
    capture: Mutex<Option<mpsc::Sender<ControllerBinding>>>,
    on_press: PressFn,
}

impl Inner {
    /// Route one rising edge: a pending capture wins over the bound action.
    fn handle_press(&self, pressed: ControllerBinding, guid_known: bool) {
        if let Some(tx) = self.capture.lock().take() {
            let _ = tx.send(pressed);
            return;
        }
        let matches = self
            .binding
            .lock()
            .as_ref()
            .is_some_and(|b| binding_matches(b, &pressed, guid_known));
        if matches {
            (self.on_press)();
        }
    }
}

/// A binding matches on instance GUID; when that device is not attached (GUIDs
/// can change after a driver reinstall), fall back to the product name.
fn binding_matches(
    bound: &ControllerBinding,
    pressed: &ControllerBinding,
    guid_known: bool,
) -> bool {
    if bound.button != pressed.button {
        return false;
    }
    if bound.device_guid == pressed.device_guid {
        return true;
    }
    !guid_known && bound.device_name == pressed.device_name
}

/// Owns the polling thread for the lifetime of the app.
#[derive(Clone)]
pub struct InputWatcher {
    inner: Arc<Inner>,
}

impl InputWatcher {
    /// Start polling. `hwnd` must be a top-level window owned by this process
    /// (DirectInput requires one even in background mode). `on_press` runs on
    /// the polling thread when the bound button goes down.
    pub fn start(hwnd: isize, on_press: impl Fn() + Send + Sync + 'static) -> Self {
        let inner = Arc::new(Inner {
            binding: Mutex::new(None),
            capture: Mutex::new(None),
            on_press: Box::new(on_press),
        });
        platform::spawn(Arc::clone(&inner), hwnd);
        Self { inner }
    }

    pub fn set_binding(&self, binding: Option<ControllerBinding>) {
        *self.inner.binding.lock() = binding;
    }

    /// Block until any controller button is pressed, or `timeout` elapses.
    /// The captured press does not trigger the bound action.
    pub fn capture_next(&self, timeout: Duration) -> Option<ControllerBinding> {
        let (tx, rx) = mpsc::channel();
        *self.inner.capture.lock() = Some(tx);
        let result = rx.recv_timeout(timeout).ok();
        self.inner.capture.lock().take();
        result
    }
}

#[cfg(windows)]
mod platform {
    use super::*;
    use std::collections::HashSet;
    use std::ffi::c_void;
    use std::time::Instant;

    use windows::core::{IUnknown, Interface, GUID};
    use windows::Win32::Devices::HumanInterfaceDevice::{
        DirectInput8Create, IDirectInput8W, IDirectInputDevice8W, DI8DEVCLASS_GAMECTRL,
        DIDATAFORMAT, DIDEVICEINSTANCEW, DIDFT_ANYINSTANCE, DIDFT_BUTTON, DIDF_ABSAXIS,
        DIEDFL_ATTACHEDONLY, DIENUM_CONTINUE, DIOBJECTDATAFORMAT, DIRECTINPUT_VERSION,
        DISCL_BACKGROUND, DISCL_NONEXCLUSIVE,
    };
    use windows::Win32::Foundation::{BOOL, HINSTANCE, HWND};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;

    const MAX_BUTTONS: usize = 128;
    /// Not exported by the `windows` crate.
    const DIDFT_OPTIONAL: u32 = 0x8000_0000;
    const POLL_INTERVAL: Duration = Duration::from_millis(16);
    const ENUM_INTERVAL: Duration = Duration::from_secs(3);

    /// Buttons-only equivalent of `c_dfDIJoystick2`: 128 optional buttons, one
    /// byte each, bit 7 set while pressed. Button `i` is the device's i-th button.
    struct ButtonFormat {
        _objs: Vec<DIOBJECTDATAFORMAT>,
        format: DIDATAFORMAT,
    }

    impl ButtonFormat {
        fn new() -> Box<Self> {
            let mut objs: Vec<DIOBJECTDATAFORMAT> = (0..MAX_BUTTONS as u32)
                .map(|i| DIOBJECTDATAFORMAT {
                    pguid: std::ptr::null(),
                    dwOfs: i,
                    dwType: DIDFT_BUTTON | DIDFT_ANYINSTANCE | DIDFT_OPTIONAL,
                    dwFlags: 0,
                })
                .collect();
            let format = DIDATAFORMAT {
                dwSize: std::mem::size_of::<DIDATAFORMAT>() as u32,
                dwObjSize: std::mem::size_of::<DIOBJECTDATAFORMAT>() as u32,
                dwFlags: DIDF_ABSAXIS,
                dwDataSize: MAX_BUTTONS as u32,
                dwNumObjs: MAX_BUTTONS as u32,
                rgodf: objs.as_mut_ptr(),
            };
            Box::new(Self {
                _objs: objs,
                format,
            })
        }
    }

    struct Device {
        guid: String,
        name: String,
        dev: IDirectInputDevice8W,
        prev: [u8; MAX_BUTTONS],
    }

    pub(super) fn spawn(inner: Arc<Inner>, hwnd: isize) {
        let spawned = std::thread::Builder::new()
            .name("race-refinery-input".into())
            .spawn(move || {
                if let Err(e) = run(&inner, hwnd) {
                    tracing::warn!("Controller input unavailable: {e:#}");
                }
            });
        if let Err(e) = spawned {
            tracing::warn!("Could not start controller input thread: {e}");
        }
    }

    fn guid_string(g: &GUID) -> String {
        format!("{g:?}")
    }

    fn wide_to_string(w: &[u16]) -> String {
        let len = w.iter().position(|&c| c == 0).unwrap_or(w.len());
        String::from_utf16_lossy(&w[..len])
    }

    unsafe extern "system" fn collect_device(
        inst: *mut DIDEVICEINSTANCEW,
        ctx: *mut c_void,
    ) -> BOOL {
        let list = &mut *(ctx as *mut Vec<DIDEVICEINSTANCEW>);
        list.push(*inst);
        BOOL(DIENUM_CONTINUE as i32)
    }

    fn run(inner: &Inner, hwnd: isize) -> windows::core::Result<()> {
        let di: IDirectInput8W = unsafe {
            let module = GetModuleHandleW(None)?;
            let mut raw: *mut c_void = std::ptr::null_mut();
            DirectInput8Create(
                HINSTANCE(module.0),
                DIRECTINPUT_VERSION,
                &IDirectInput8W::IID,
                &mut raw,
                None::<&IUnknown>,
            )?;
            IDirectInput8W::from_raw(raw)
        };
        let mut format = ButtonFormat::new();
        let hwnd = HWND(hwnd as *mut c_void);

        let mut devices: Vec<Device> = Vec::new();
        // Devices that refused setup; retried only after they disappear and return.
        let mut rejected: HashSet<String> = HashSet::new();
        let mut last_enum: Option<Instant> = None;

        loop {
            if last_enum.is_none_or(|t| t.elapsed() >= ENUM_INTERVAL) {
                last_enum = Some(Instant::now());
                refresh_devices(&di, hwnd, &mut format, &mut devices, &mut rejected);
            }

            let bound_guid = inner.binding.lock().as_ref().map(|b| b.device_guid.clone());
            let guid_known = bound_guid
                .as_deref()
                .is_some_and(|g| devices.iter().any(|d| d.guid == g));

            devices.retain_mut(|d| {
                let Some(state) = read_buttons(&d.dev) else {
                    tracing::info!("Controller disconnected: {}", d.name);
                    return false;
                };
                for (i, (&now, &before)) in state.iter().zip(d.prev.iter()).enumerate() {
                    if now & 0x80 != 0 && before & 0x80 == 0 {
                        inner.handle_press(
                            ControllerBinding {
                                device_guid: d.guid.clone(),
                                device_name: d.name.clone(),
                                button: i as u32,
                            },
                            guid_known,
                        );
                    }
                }
                d.prev = state;
                true
            });

            std::thread::sleep(POLL_INTERVAL);
        }
    }

    fn refresh_devices(
        di: &IDirectInput8W,
        hwnd: HWND,
        format: &mut ButtonFormat,
        devices: &mut Vec<Device>,
        rejected: &mut HashSet<String>,
    ) {
        let mut found: Vec<DIDEVICEINSTANCEW> = Vec::new();
        let enumerated = unsafe {
            di.EnumDevices(
                DI8DEVCLASS_GAMECTRL,
                Some(collect_device),
                &mut found as *mut _ as *mut c_void,
                DIEDFL_ATTACHEDONLY,
            )
        };
        if let Err(e) = enumerated {
            tracing::debug!("DirectInput EnumDevices failed: {e}");
            return;
        }

        let attached: HashSet<String> =
            found.iter().map(|i| guid_string(&i.guidInstance)).collect();
        rejected.retain(|g| attached.contains(g));

        for inst in &found {
            let guid = guid_string(&inst.guidInstance);
            if devices.iter().any(|d| d.guid == guid) || rejected.contains(&guid) {
                continue;
            }
            let name = wide_to_string(&inst.tszProductName);
            match open_device(di, hwnd, format, &inst.guidInstance) {
                Ok(dev) => {
                    tracing::info!("Controller attached: {name}");
                    devices.push(Device {
                        guid,
                        name,
                        dev,
                        prev: [0; MAX_BUTTONS],
                    });
                }
                Err(e) => {
                    tracing::debug!("Skipping controller {name}: {e}");
                    rejected.insert(guid);
                }
            }
        }
    }

    fn open_device(
        di: &IDirectInput8W,
        hwnd: HWND,
        format: &mut ButtonFormat,
        guid: &GUID,
    ) -> windows::core::Result<IDirectInputDevice8W> {
        unsafe {
            let mut dev: Option<IDirectInputDevice8W> = None;
            di.CreateDevice(guid, &mut dev, None::<&IUnknown>)?;
            let dev = dev.ok_or_else(windows::core::Error::empty)?;
            dev.SetDataFormat(&mut format.format)?;
            dev.SetCooperativeLevel(hwnd, DISCL_BACKGROUND | DISCL_NONEXCLUSIVE)?;
            let _ = dev.Acquire();
            Ok(dev)
        }
    }

    /// Current button bytes, re-acquiring once if the device was lost.
    /// `None` means the device is gone.
    fn read_buttons(dev: &IDirectInputDevice8W) -> Option<[u8; MAX_BUTTONS]> {
        let mut state = [0u8; MAX_BUTTONS];
        for _ in 0..2 {
            unsafe {
                let _ = dev.Poll();
                if dev
                    .GetDeviceState(MAX_BUTTONS as u32, state.as_mut_ptr() as *mut c_void)
                    .is_ok()
                {
                    return Some(state);
                }
                if dev.Acquire().is_err() {
                    return None;
                }
            }
        }
        None
    }
}

#[cfg(not(windows))]
mod platform {
    use super::*;

    /// No DirectInput off Windows; bindings never fire.
    pub(super) fn spawn(_inner: Arc<Inner>, _hwnd: isize) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding(guid: &str, name: &str, button: u32) -> ControllerBinding {
        ControllerBinding {
            device_guid: guid.into(),
            device_name: name.into(),
            button,
        }
    }

    #[test]
    fn matches_on_guid_and_button() {
        let bound = binding("A", "Wheel", 3);
        assert!(binding_matches(&bound, &binding("A", "Wheel", 3), true));
        assert!(!binding_matches(&bound, &binding("A", "Wheel", 4), true));
        assert!(!binding_matches(&bound, &binding("B", "Wheel", 3), true));
    }

    #[test]
    fn falls_back_to_name_when_guid_missing() {
        let bound = binding("A", "Wheel", 3);
        assert!(binding_matches(&bound, &binding("B", "Wheel", 3), false));
        assert!(!binding_matches(&bound, &binding("B", "Pedals", 3), false));
    }

    #[test]
    fn capture_takes_priority_over_binding() {
        let fired = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = Arc::clone(&fired);
        let inner = Inner {
            binding: Mutex::new(Some(binding("A", "Wheel", 3))),
            capture: Mutex::new(None),
            on_press: Box::new(move || flag.store(true, std::sync::atomic::Ordering::SeqCst)),
        };
        let (tx, rx) = mpsc::channel();
        *inner.capture.lock() = Some(tx);
        inner.handle_press(binding("A", "Wheel", 3), true);
        assert_eq!(rx.try_recv().unwrap().button, 3);
        assert!(!fired.load(std::sync::atomic::Ordering::SeqCst));

        inner.handle_press(binding("A", "Wheel", 3), true);
        assert!(fired.load(std::sync::atomic::Ordering::SeqCst));
    }
}
