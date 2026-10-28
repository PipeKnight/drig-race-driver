//! The device side of force feedback on Windows (docs/FORCE_FEEDBACK.md §5): a DirectInput 8 wheel
//! (input, one infinite constant-force effect with autocentre off, software damping) and XInput pad
//! motors, driven by the FFB task on its own thread every 10 ms like the original's `7ED380`.
//!
//! A thin hand-written binding: the COM interfaces are called through their vtables, the DLLs are
//! linked with `raw-dylib`, and the structures and constants follow Microsoft's `dinput.h` and
//! `xinput.h` (DirectInput 8, XInput 1.4). No crate is added. Which wheel is opened is [A]: the
//! first attached driving device, else the first force-feedback controller that is not a gamepad.

// FFI structures keep every field of the Windows layout, read or not.
#![allow(non_snake_case, clippy::upper_case_acronyms, dead_code)]

use super::{Damping, Ffb, Output, motors};
use std::ffi::c_void;
use std::sync::{Arc, Mutex, OnceLock, mpsc};
use std::time::{Duration, Instant};

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GUID(u32, u16, u16, [u8; 8]);

const fn axis_guid(last: u32) -> GUID {
    GUID(last, 0xC9F3, 0x11CF, [0xBF, 0xC7, 0x44, 0x45, 0x53, 0x54, 0x00, 0x00])
}
static IID_IDIRECTINPUT8W: GUID = GUID(0xBF79_8031, 0x483A, 0x4DA2, [0xAA, 0x99, 0x5D, 0x64, 0xED, 0x36, 0x97, 0x00]);
static GUID_CONSTANT_FORCE: GUID = GUID(0x1354_1C20, 0x8E33, 0x11D0, [0x9A, 0xD0, 0x00, 0xA0, 0xC9, 0xA0, 0x6E, 0x35]);
static GUID_X: GUID = axis_guid(0xA36D_02E0);
static GUID_Y: GUID = axis_guid(0xA36D_02E1);
static GUID_Z: GUID = axis_guid(0xA36D_02E2);
static GUID_RX: GUID = axis_guid(0xA36D_02F4);
static GUID_RY: GUID = axis_guid(0xA36D_02F5);
static GUID_RZ: GUID = axis_guid(0xA36D_02E3);
static GUID_SLIDER: GUID = axis_guid(0xA36D_02E4);
static GUID_POV: GUID = axis_guid(0xA36D_02F2);

const DIRECTINPUT_VERSION: u32 = 0x0800;
const DI8DEVCLASS_GAMECTRL: u32 = 4;
const DIEDFL_ATTACHEDONLY: u32 = 1;
const DI8DEVTYPE_GAMEPAD: u32 = 0x15;
const DI8DEVTYPE_DRIVING: u32 = 0x16;
const DIDC_FORCEFEEDBACK: u32 = 0x100;
const DISCL_EXCLUSIVE: u32 = 1;
const DISCL_NONEXCLUSIVE: u32 = 2;
const DISCL_BACKGROUND: u32 = 8;
const DIDF_ABSAXIS: u32 = 1;
const DIDFT_AXIS: u32 = 3;
const DIDFT_BUTTON: u32 = 0xC;
const DIDFT_POV: u32 = 0x10;
const DIDFT_ANYINSTANCE: u32 = 0x00FF_FF00;
const DIDFT_OPTIONAL: u32 = 0x8000_0000;
const DIPH_DEVICE: u32 = 0;
const DIPH_BYOFFSET: u32 = 1;
// MAKEDIPROP(n): the property "GUID" pointers are the small integers themselves.
const DIPROP_RANGE: usize = 4;
const DIPROP_DEADZONE: usize = 5;
const DIPROP_AUTOCENTER: usize = 9;
const DIEFF_OBJECTOFFSETS: u32 = 2;
const DIEFF_CARTESIAN: u32 = 0x10;
const DIEP_DIRECTION: u32 = 0x40;
const DIEP_TYPESPECIFICPARAMS: u32 = 0x100;
const DIEP_START: u32 = 0x2000_0000;
const DISFFC_STOPALL: u32 = 2;
const INFINITE: u32 = 0xFFFF_FFFF;
const DIEB_NOTRIGGER: u32 = 0xFFFF_FFFF;
const WS_POPUP: u32 = 0x8000_0000;
const PM_REMOVE: u32 = 1;

#[repr(C)]
#[derive(Clone, Copy)]
struct DIDEVICEINSTANCEW {
    dwSize: u32,
    guidInstance: GUID,
    guidProduct: GUID,
    dwDevType: u32,
    tszInstanceName: [u16; 260],
    tszProductName: [u16; 260],
    guidFFDriver: GUID,
    wUsagePage: u16,
    wUsage: u16,
}

#[repr(C)]
#[derive(Default)]
struct DIDEVCAPS {
    dwSize: u32,
    dwFlags: u32,
    dwDevType: u32,
    dwAxes: u32,
    dwButtons: u32,
    dwPOVs: u32,
    dwFFSamplePeriod: u32,
    dwFFMinTimeResolution: u32,
    dwFirmwareRevision: u32,
    dwHardwareRevision: u32,
    dwFFDriverVersion: u32,
}

#[repr(C)]
struct DIDEVICEOBJECTINSTANCEW {
    dwSize: u32,
    guidType: GUID,
    dwOfs: u32,
    dwType: u32,
    dwFlags: u32,
    tszName: [u16; 260],
    dwFFMaxForce: u32,
    dwFFForceResolution: u32,
    wCollectionNumber: u16,
    wDesignatorIndex: u16,
    wUsagePage: u16,
    wUsage: u16,
    dwDimension: u32,
    wExponent: u16,
    wReportId: u16,
}

#[repr(C)]
struct DIOBJECTDATAFORMAT {
    pguid: *const GUID,
    dwOfs: u32,
    dwType: u32,
    dwFlags: u32,
}

#[repr(C)]
struct DIDATAFORMAT {
    dwSize: u32,
    dwObjSize: u32,
    dwFlags: u32,
    dwDataSize: u32,
    dwNumObjs: u32,
    rgodf: *mut DIOBJECTDATAFORMAT,
}

#[repr(C)]
struct DIPROPHEADER {
    dwSize: u32,
    dwHeaderSize: u32,
    dwObj: u32,
    dwHow: u32,
}

#[repr(C)]
struct DIPROPDWORD {
    diph: DIPROPHEADER,
    dwData: u32,
}

#[repr(C)]
struct DIPROPRANGE {
    diph: DIPROPHEADER,
    lMin: i32,
    lMax: i32,
}

#[repr(C)]
struct DIEFFECT {
    dwSize: u32,
    dwFlags: u32,
    dwDuration: u32,
    dwSamplePeriod: u32,
    dwGain: u32,
    dwTriggerButton: u32,
    dwTriggerRepeatInterval: u32,
    cAxes: u32,
    rgdwAxes: *mut u32,
    rglDirection: *mut i32,
    lpEnvelope: *mut c_void,
    cbTypeSpecificParams: u32,
    lpvTypeSpecificParams: *mut c_void,
    dwStartDelay: u32,
}

#[repr(C)]
#[derive(Default)]
struct XINPUT_STATE {
    dwPacketNumber: u32,
    wButtons: u16,
    bLeftTrigger: u8,
    bRightTrigger: u8,
    sThumbLX: i16,
    sThumbLY: i16,
    sThumbRX: i16,
    sThumbRY: i16,
}

#[repr(C)]
struct XINPUT_VIBRATION {
    wLeftMotorSpeed: u16,
    wRightMotorSpeed: u16,
}

#[repr(C)]
struct MSG {
    hwnd: isize,
    message: u32,
    wParam: usize,
    lParam: isize,
    time: u32,
    pt: [i32; 2],
    lPrivate: u32,
}

#[link(name = "dinput8", kind = "raw-dylib")]
unsafe extern "system" {
    fn DirectInput8Create(
        hinst: isize,
        version: u32,
        riid: *const GUID,
        out: *mut *mut c_void,
        outer: *mut c_void,
    ) -> i32;
}
#[link(name = "xinput1_4", kind = "raw-dylib")]
unsafe extern "system" {
    fn XInputGetState(user: u32, state: *mut XINPUT_STATE) -> u32;
    fn XInputSetState(user: u32, vibration: *const XINPUT_VIBRATION) -> u32;
}
#[link(name = "kernel32", kind = "raw-dylib")]
unsafe extern "system" {
    fn GetModuleHandleW(name: *const u16) -> isize;
}
#[link(name = "user32", kind = "raw-dylib")]
unsafe extern "system" {
    fn CreateWindowExW(
        ex: u32,
        class: *const u16,
        name: *const u16,
        style: u32,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        parent: isize,
        menu: isize,
        hinst: isize,
        param: *mut c_void,
    ) -> isize;
    fn PeekMessageW(msg: *mut MSG, hwnd: isize, min: u32, max: u32, remove: u32) -> i32;
    fn TranslateMessage(msg: *const MSG) -> i32;
    fn DispatchMessageW(msg: *const MSG) -> isize;
}

/// Calls vtable slot `$slot` of the COM object `$obj` with the signature `$ty`.
macro_rules! com {
    ($obj:expr, $slot:expr, $ty:ty $(, $arg:expr)*) => {{
        let object: *mut c_void = $obj;
        let table = unsafe { *(object as *const *const usize) };
        let method = unsafe { std::mem::transmute::<usize, $ty>(*table.add($slot)) };
        unsafe { method(object $(, $arg)*) }
    }};
}
type Release = unsafe extern "system" fn(*mut c_void) -> u32;
type Plain = unsafe extern "system" fn(*mut c_void) -> i32;

/// The joystick record of the data format below: 8 axes, 4 POVs, 64 buttons.
#[repr(C)]
#[derive(Clone, Copy)]
struct Record {
    axes: [i32; 8],
    pov: [u32; 4],
    buttons: [u8; 64],
}

/// Axes in record order with their `win_con_di_axis*` names.
pub const AXIS_NAMES: [&str; 8] = ["X", "Y", "Z", "Rx", "Ry", "Rz", "Slider0", "Slider1"];

/// What the input layer reads of the wheel: axes as raw/65536 (the original's 0..65535 range,
/// `None` when the device has no such axis), buttons as a bit mask, POVs in hundredths of degrees
/// (0xFFFF.. centred).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WheelState {
    pub axes: [Option<f32>; 8],
    pub buttons: u64,
    pub pov: [u32; 4],
}

impl Default for WheelState {
    fn default() -> Self {
        Self { axes: [None; 8], buttons: 0, pov: [u32::MAX; 4] }
    }
}

/// The opened wheel.
#[derive(Clone, Debug, PartialEq)]
pub struct Wheel {
    pub name: String,
    pub force_feedback: bool,
    pub axes: [bool; 8],
    pub buttons: u32,
}

/// State shared between the game and the FFB thread.
#[derive(Default)]
pub struct Shared {
    pub state: WheelState,
    /// The FFB task, set by the race (`crate::ffb`).
    pub ffb: Option<Ffb>,
    /// Send the effect channels to the first connected XInput pad.
    pub rumble: bool,
    /// When the race last queued a snapshot; the outputs go to zero 0.25 s after [A].
    pub fed: Option<Instant>,
    pub shutdown: bool,
}

pub struct Handle {
    pub wheel: Option<Wheel>,
    pub shared: Arc<Mutex<Shared>>,
}

static DEVICE: OnceLock<Handle> = OnceLock::new();

/// The process's device thread, started on first use; waits for the wheel enumeration.
pub fn device() -> &'static Handle {
    DEVICE.get_or_init(|| {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let (tx, rx) = mpsc::channel();
        let thread_shared = shared.clone();
        let started = std::thread::Builder::new()
            .name("FFB updateTask".into())
            .spawn(move || run(&tx, &thread_shared))
            .is_ok();
        let wheel = match started.then(|| rx.recv_timeout(Duration::from_secs(5))) {
            Some(Ok(wheel)) => wheel,
            Some(Err(_)) => {
                println!("ffb_device enumeration took over 5 s; no wheel");
                None
            }
            None => None,
        };
        Handle { wheel, shared }
    })
}

/// The device thread if something already started it.
pub fn started() -> Option<&'static Handle> {
    DEVICE.get()
}

/// The DirectInput wheel.
struct DiWheel {
    di: *mut c_void,
    device: *mut c_void,
    effect: *mut c_void,
    info: Wheel,
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn utf16(text: &[u16]) -> String {
    String::from_utf16_lossy(&text[..text.iter().position(|&c| c == 0).unwrap_or(text.len())])
}

unsafe extern "system" fn collect(instance: *const DIDEVICEINSTANCEW, list: *mut c_void) -> i32 {
    let list = unsafe { &mut *(list as *mut Vec<DIDEVICEINSTANCEW>) };
    list.push(unsafe { *instance });
    1
}

impl DiWheel {
    /// Enumerate and open the wheel; `window` is a top-level window of this process.
    fn open(window: isize) -> Result<Option<Self>, String> {
        let hinst = unsafe { GetModuleHandleW(std::ptr::null()) };
        let mut di = std::ptr::null_mut();
        let hr = unsafe { DirectInput8Create(hinst, DIRECTINPUT_VERSION, &IID_IDIRECTINPUT8W, &mut di, std::ptr::null_mut()) };
        if hr < 0 || di.is_null() {
            return Err(format!("DirectInput8Create {hr:#x}"));
        }
        let mut list: Vec<DIDEVICEINSTANCEW> = Vec::new();
        type Enum = unsafe extern "system" fn(
            *mut c_void,
            u32,
            unsafe extern "system" fn(*const DIDEVICEINSTANCEW, *mut c_void) -> i32,
            *mut c_void,
            u32,
        ) -> i32;
        let list_ptr = (&mut list as *mut Vec<DIDEVICEINSTANCEW>).cast();
        let hr = com!(di, 4, Enum, DI8DEVCLASS_GAMECTRL, collect, list_ptr, DIEDFL_ATTACHEDONLY);
        if hr < 0 {
            com!(di, 2, Release);
            return Err(format!("EnumDevices {hr:#x}"));
        }
        let mut candidates: Vec<(u32, DIDEVICEINSTANCEW)> = Vec::new();
        for instance in &list {
            let kind = instance.dwDevType & 0xFF;
            println!(
                "ffb_device product={:?} type={:#x} subtype={:#x}",
                utf16(&instance.tszProductName),
                kind,
                (instance.dwDevType >> 8) & 0xFF
            );
            if kind == DI8DEVTYPE_GAMEPAD {
                continue;
            }
            candidates.push((u32::from(kind != DI8DEVTYPE_DRIVING), *instance));
        }
        candidates.sort_by_key(|(rank, _)| *rank);
        for (rank, instance) in candidates {
            match Self::create(di, &instance, window) {
                Ok(Some(wheel)) if rank == 0 || wheel.info.force_feedback => {
                    com!(di, 2, Release); // the wheel keeps its own reference
                    return Ok(Some(wheel));
                }
                Ok(Some(other)) => {
                    println!("ffb_device skipped={:?}: not a driving device and no force feedback", other.info.name);
                    drop(other)
                }
                Ok(None) => {}
                Err(error) => eprintln!("ffb: {} skipped: {error}", utf16(&instance.tszProductName)),
            }
        }
        com!(di, 2, Release);
        Ok(None)
    }

    fn create(di: *mut c_void, instance: &DIDEVICEINSTANCEW, window: isize) -> Result<Option<Self>, String> {
        type Create = unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void, *mut c_void) -> i32;
        let mut device = std::ptr::null_mut();
        let hr = com!(di, 3, Create, &instance.guidInstance, &mut device, std::ptr::null_mut());
        if hr < 0 || device.is_null() {
            return Err(format!("CreateDevice {hr:#x}"));
        }
        com!(di, 1, Release); // the wheel holds its own reference to DirectInput
        let mut wheel = Self {
            di,
            device,
            effect: std::ptr::null_mut(),
            info: Wheel { name: utf16(&instance.tszProductName), force_feedback: false, axes: [false; 8], buttons: 0 },
        };
        wheel.configure(window)?;
        Ok(Some(wheel))
    }

    fn configure(&mut self, window: isize) -> Result<(), String> {
        let d = self.device;
        // Data format: the record above.
        let axis_guids: [&'static GUID; 8] = [&GUID_X, &GUID_Y, &GUID_Z, &GUID_RX, &GUID_RY, &GUID_RZ, &GUID_SLIDER, &GUID_SLIDER];
        let mut objects: Vec<DIOBJECTDATAFORMAT> = Vec::new();
        for (i, g) in axis_guids.iter().enumerate() {
            objects.push(DIOBJECTDATAFORMAT {
                pguid: *g,
                dwOfs: 4 * i as u32,
                dwType: DIDFT_AXIS | DIDFT_ANYINSTANCE | DIDFT_OPTIONAL,
                dwFlags: 0,
            });
        }
        for i in 0..4 {
            objects.push(DIOBJECTDATAFORMAT {
                pguid: &GUID_POV,
                dwOfs: 32 + 4 * i,
                dwType: DIDFT_POV | DIDFT_ANYINSTANCE | DIDFT_OPTIONAL,
                dwFlags: 0,
            });
        }
        for i in 0..64 {
            objects.push(DIOBJECTDATAFORMAT {
                pguid: std::ptr::null(),
                dwOfs: 48 + i,
                dwType: DIDFT_BUTTON | DIDFT_ANYINSTANCE | DIDFT_OPTIONAL,
                dwFlags: 0,
            });
        }
        let format = DIDATAFORMAT {
            dwSize: size_of::<DIDATAFORMAT>() as u32,
            dwObjSize: size_of::<DIOBJECTDATAFORMAT>() as u32,
            dwFlags: DIDF_ABSAXIS,
            dwDataSize: size_of::<Record>() as u32,
            dwNumObjs: objects.len() as u32,
            rgodf: objects.as_mut_ptr(),
        };
        let hr = com!(d, 11, unsafe extern "system" fn(*mut c_void, *const DIDATAFORMAT) -> i32, &format);
        if hr < 0 {
            return Err(format!("SetDataFormat {hr:#x}"));
        }
        let mut caps = DIDEVCAPS { dwSize: size_of::<DIDEVCAPS>() as u32, ..DIDEVCAPS::default() };
        let hr = com!(d, 3, unsafe extern "system" fn(*mut c_void, *mut DIDEVCAPS) -> i32, &mut caps);
        if hr < 0 {
            return Err(format!("GetCapabilities {hr:#x}"));
        }
        self.info.buttons = caps.dwButtons;
        type Level = unsafe extern "system" fn(*mut c_void, isize, u32) -> i32;
        let mut ff = caps.dwFlags & DIDC_FORCEFEEDBACK != 0;
        if ff && com!(d, 13, Level, window, DISCL_EXCLUSIVE | DISCL_BACKGROUND) < 0 {
            ff = false;
        }
        if !ff {
            let hr = com!(d, 13, Level, window, DISCL_NONEXCLUSIVE | DISCL_BACKGROUND);
            if hr < 0 {
                return Err(format!("SetCooperativeLevel {hr:#x}"));
            }
        }
        // The original's axis range 0..65535 without a driver dead zone.
        type Set = unsafe extern "system" fn(*mut c_void, usize, *const DIPROPHEADER) -> i32;
        let header = |size: usize| DIPROPHEADER {
            dwSize: size as u32,
            dwHeaderSize: size_of::<DIPROPHEADER>() as u32,
            dwObj: 0,
            dwHow: DIPH_DEVICE,
        };
        let range = DIPROPRANGE { diph: header(size_of::<DIPROPRANGE>()), lMin: 0, lMax: 65535 };
        com!(d, 6, Set, DIPROP_RANGE, &range.diph);
        let dead = DIPROPDWORD { diph: header(size_of::<DIPROPDWORD>()), dwData: 0 };
        com!(d, 6, Set, DIPROP_DEADZONE, &dead.diph);
        // Which axes exist (GetObjectInfo by offset).
        for i in 0..8 {
            // SAFETY: a plain-data structure; zero is a valid value.
            let mut object: DIDEVICEOBJECTINSTANCEW = unsafe { std::mem::zeroed() };
            object.dwSize = size_of::<DIDEVICEOBJECTINSTANCEW>() as u32;
            type Info = unsafe extern "system" fn(*mut c_void, *mut DIDEVICEOBJECTINSTANCEW, u32, u32) -> i32;
            self.info.axes[i] = com!(d, 14, Info, &mut object, 4 * i as u32, DIPH_BYOFFSET) >= 0;
        }
        if ff {
            // B8CB10: autocentre off, one infinite constant force on X, direction (1, 0).
            let off = DIPROPDWORD { diph: header(size_of::<DIPROPDWORD>()), dwData: 0 };
            com!(d, 6, Set, DIPROP_AUTOCENTER, &off.diph);
        }
        com!(d, 7, Plain);
        if ff {
            let mut axes = [0u32; 2];
            let mut direction = [1i32, 0];
            let mut constant = 0i32;
            let effect = DIEFFECT {
                dwSize: size_of::<DIEFFECT>() as u32,
                dwFlags: DIEFF_CARTESIAN | DIEFF_OBJECTOFFSETS,
                dwDuration: INFINITE,
                dwSamplePeriod: 0,
                dwGain: 10_000,
                dwTriggerButton: DIEB_NOTRIGGER,
                dwTriggerRepeatInterval: 0,
                cAxes: 1,
                rgdwAxes: axes.as_mut_ptr(),
                rglDirection: direction.as_mut_ptr(),
                lpEnvelope: std::ptr::null_mut(),
                cbTypeSpecificParams: 4,
                lpvTypeSpecificParams: (&mut constant as *mut i32).cast(),
                dwStartDelay: 0,
            };
            type CreateEffect =
                unsafe extern "system" fn(*mut c_void, *const GUID, *const DIEFFECT, *mut *mut c_void, *mut c_void) -> i32;
            let mut out = std::ptr::null_mut();
            let hr = com!(d, 18, CreateEffect, &GUID_CONSTANT_FORCE, &effect, &mut out, std::ptr::null_mut());
            if hr >= 0 && !out.is_null() {
                self.effect = out;
                self.info.force_feedback = true;
            }
        }
        Ok(())
    }

    fn poll(&mut self) -> Option<WheelState> {
        let d = self.device;
        com!(d, 25, Plain);
        let mut record = Record { axes: [0; 8], pov: [u32::MAX; 4], buttons: [0; 64] };
        type State = unsafe extern "system" fn(*mut c_void, u32, *mut c_void) -> i32;
        let size = size_of::<Record>() as u32;
        let ptr = (&mut record as *mut Record).cast();
        if com!(d, 9, State, size, ptr) < 0 {
            // Lost or not acquired: acquire again and read next time.
            com!(d, 7, Plain);
            return None;
        }
        let mut state = WheelState { pov: record.pov, ..WheelState::default() };
        for i in 0..8 {
            if self.info.axes[i] {
                state.axes[i] = Some(record.axes[i] as f32 / 65536.);
            }
        }
        for (i, b) in record.buttons.iter().enumerate() {
            if b & 0x80 != 0 {
                state.buttons |= 1 << i;
            }
        }
        Some(state)
    }

    /// `B8CD80`'s SetParameters: the constant force's magnitude.
    fn set_force(&mut self, magnitude: i32) {
        if self.effect.is_null() {
            return;
        }
        let mut axes = [0u32; 2];
        let mut direction = [1i32, 0];
        let mut constant = magnitude;
        let effect = DIEFFECT {
            dwSize: size_of::<DIEFFECT>() as u32,
            dwFlags: DIEFF_CARTESIAN | DIEFF_OBJECTOFFSETS,
            dwDuration: INFINITE,
            dwSamplePeriod: 0,
            dwGain: 10_000,
            dwTriggerButton: DIEB_NOTRIGGER,
            dwTriggerRepeatInterval: 0,
            cAxes: 1,
            rgdwAxes: axes.as_mut_ptr(),
            rglDirection: direction.as_mut_ptr(),
            lpEnvelope: std::ptr::null_mut(),
            cbTypeSpecificParams: 4,
            lpvTypeSpecificParams: (&mut constant as *mut i32).cast(),
            dwStartDelay: 0,
        };
        type SetParameters = unsafe extern "system" fn(*mut c_void, *const DIEFFECT, u32) -> i32;
        let flags = DIEP_START | DIEP_TYPESPECIFICPARAMS | DIEP_DIRECTION;
        if com!(self.effect, 6, SetParameters, &effect, flags) < 0 {
            com!(self.device, 7, Plain);
        }
    }
}

impl Drop for DiWheel {
    fn drop(&mut self) {
        if !self.effect.is_null() {
            self.set_force(0);
            com!(self.effect, 8, Plain);
            com!(self.effect, 2, Release);
        }
        com!(self.device, 22, unsafe extern "system" fn(*mut c_void, u32) -> i32, DISFFC_STOPALL);
        com!(self.device, 8, Plain);
        com!(self.device, 2, Release);
        com!(self.di, 2, Release);
    }
}

/// The first connected XInput pad, checked at most once a second.
struct Pad {
    user: Option<u32>,
    checked: Option<Instant>,
    last: (u16, u16),
}

impl Pad {
    fn set(&mut self, now: Instant, motors: (u16, u16)) {
        if self.checked.is_none_or(|t| now - t > Duration::from_secs(1)) {
            self.checked = Some(now);
            self.user = (0..4).find(|&u| {
                let mut state = XINPUT_STATE::default();
                unsafe { XInputGetState(u, &mut state) == 0 }
            });
        }
        if let Some(user) = self.user
            && motors != self.last
        {
            let v = XINPUT_VIBRATION { wLeftMotorSpeed: motors.0, wRightMotorSpeed: motors.1 };
            if unsafe { XInputSetState(user, &v) } != 0 {
                self.checked = None;
            }
            self.last = motors;
        }
    }
}

fn pump() {
    // SAFETY: MSG is plain data; the hidden window belongs to this thread.
    let mut msg: MSG = unsafe { std::mem::zeroed() };
    while unsafe { PeekMessageW(&mut msg, 0, 0, 0, PM_REMOVE) } != 0 {
        unsafe {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

/// `7ED380`: Sleep(10), tick the device with the current outputs, then process the snapshots.
fn run(tx: &mpsc::Sender<Option<Wheel>>, shared: &Arc<Mutex<Shared>>) {
    // A hidden top-level window for the exclusive cooperative level FFB needs.
    let class = wide("STATIC");
    let title = wide("DRIG force feedback");
    let window = unsafe {
        CreateWindowExW(0, class.as_ptr(), title.as_ptr(), WS_POPUP, 0, 0, 0, 0, 0, 0, 0, std::ptr::null_mut())
    };
    let mut wheel = match DiWheel::open(window) {
        Ok(w) => w,
        Err(error) => {
            eprintln!("ffb: DirectInput unavailable: {error}");
            None
        }
    };
    let _ = tx.send(wheel.as_ref().map(|w| w.info.clone()));
    let mut damping = Damping::default();
    let mut pad = Pad { user: None, checked: None, last: (0, 0) };
    let mut last = Instant::now();
    loop {
        std::thread::sleep(Duration::from_millis(10));
        let now = Instant::now();
        let dt = (now - last).as_secs_f64();
        last = now;
        pump();
        let state = wheel.as_mut().and_then(DiWheel::poll);
        let Ok(mut s) = shared.lock() else { return };
        if s.shutdown {
            drop(s);
            if let Some(w) = wheel.as_mut() {
                w.set_force(0);
            }
            pad.set(now, (0, 0));
            return;
        }
        if let Some(state) = state {
            s.state = state;
        }
        let fed = s.fed.is_some_and(|t| now - t < Duration::from_millis(250));
        let out = match &s.ffb {
            Some(ffb) if fed => ffb.output(),
            _ => Output::default(),
        };
        if let Some(w) = wheel.as_mut().filter(|w| w.info.force_feedback) {
            let axis = s.state.axes[0].unwrap_or(0.5);
            let magnitude = if s.ffb.as_ref().is_some_and(|f| f.wheel()) { damping.magnitude(&out, axis, dt) } else { 0 };
            w.set_force(magnitude);
        }
        if s.rumble {
            pad.set(now, motors(&out));
        }
        if let Some(ffb) = s.ffb.as_mut() {
            ffb.process(dt);
        }
    }
}
