//! Explicitly requested GRID-only timed scan-code input experiment.
//! No focus changes, arbitrary applications, drivers or elevation requests.
use std::{ffi::c_void, mem::size_of, sync::atomic::{AtomicU32, Ordering},
          thread, time::{Duration, Instant}};

type Handle = *mut c_void;
#[repr(C)]
#[derive(Clone, Copy)]
struct KeyboardInput { vk: u16, scan: u16, flags: u32, time: u32, extra: usize }
#[repr(C)]
#[derive(Clone, Copy)]
struct MouseInput { x: i32, y: i32, data: u32, flags: u32, time: u32, extra: usize }
#[repr(C)]
union InputData { keyboard: KeyboardInput, mouse: MouseInput }
#[repr(C)]
struct Input { kind: u32, data: InputData }
#[repr(C)]
#[derive(Default)]
struct FileTime { low: u32, high: u32 }

#[link(name="user32")]
unsafe extern "system" {
    fn GetForegroundWindow() -> Handle;
    fn GetWindowThreadProcessId(window: Handle, pid: *mut u32) -> u32;
    fn GetAsyncKeyState(key: i32) -> i16;
    fn SendInput(count: u32, inputs: *const Input, bytes: i32) -> u32;
}
#[link(name="kernel32")]
unsafe extern "system" {
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
    fn CloseHandle(handle: Handle) -> i32;
    fn QueryFullProcessImageNameW(handle: Handle, flags: u32, name: *mut u16, len: *mut u32) -> i32;
    fn GetProcessTimes(handle: Handle, created: *mut FileTime, exited: *mut FileTime,
                       kernel: *mut FileTime, user: *mut FileTime) -> i32;
    fn SetConsoleCtrlHandler(handler: Option<unsafe extern "system" fn(u32) -> i32>, add: i32) -> i32;
}

const GRID_EXE: &str = r"C:\Users\debros\Tools\grid-research\grid-compat\grid.exe";
static HELD: AtomicU32 = AtomicU32::new(0);

#[derive(Clone, Copy)]
struct Key { scan: u16, extended: bool, vk: i32 }
impl Key {
    fn named(name: &str) -> Option<Self> {
        let (scan, extended, vk) = match name {
            "up" => (0x48, true, 0x26), "down" => (0x50, true, 0x28),
            "left" => (0x4b, true, 0x25), "right" => (0x4d, true, 0x27),
            "enter" => (0x1c, false, 0x0d), "escape" => (0x01, false, 0x1b),
            _ => return None,
        };
        Some(Self { scan, extended, vk })
    }
    fn encoded(self) -> u32 { self.scan as u32 | if self.extended { 0x100 } else { 0 } }
}

fn send(key: Key, up: bool) -> Result<(), String> {
    let mut input: Input = unsafe { std::mem::zeroed() };
    input.kind = 1;
    input.data.keyboard = KeyboardInput { vk: 0, scan: key.scan,
        flags: 8 | u32::from(key.extended) | if up { 2 } else { 0 }, time: 0,
        extra: 0x47524944 };
    if unsafe { SendInput(1, &input, size_of::<Input>() as i32) } != 1 {
        return Err(format!("SendInput {} failed: {}", if up { "release" } else { "press" },
                           std::io::Error::last_os_error()));
    }
    Ok(())
}

fn release_held() -> Result<(), String> {
    let code = HELD.swap(0, Ordering::SeqCst);
    if code == 0 { return Ok(()); }
    let key = Key { scan: (code & 0xff) as u16, extended: code & 0x100 != 0, vk: 0 };
    if let Err(error) = send(key, true) {
        // Leave state for the cleanup guard's retry, if a normal release failed.
        HELD.store(code, Ordering::SeqCst);
        return Err(error);
    }
    Ok(())
}
struct ReleaseGuard;
impl Drop for ReleaseGuard { fn drop(&mut self) { let _ = release_held(); } }
unsafe extern "system" fn console_stop(_: u32) -> i32 {
    let _ = release_held();
    0 // Preserve Windows' normal console termination handling.
}

fn foreground_pid() -> u32 {
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(GetForegroundWindow(), &mut pid); }
    pid
}
fn held(vk: i32) -> bool { unsafe { GetAsyncKeyState(vk) < 0 } }

fn verify(pid: u32, expected_creation: u64) -> Result<(), String> {
    if foreground_pid() != pid { return Err("Foreground window is not the verified GRID process".into()); }
    let process = unsafe { OpenProcess(0x1000, 0, pid) };
    if process.is_null() { return Err(format!("OpenProcess: {}", std::io::Error::last_os_error())); }
    let mut name = vec![0u16; 32768];
    let mut len = name.len() as u32;
    let mut created = FileTime::default();
    let mut exited = FileTime::default();
    let mut kernel = FileTime::default();
    let mut user = FileTime::default();
    let name_ok = unsafe { QueryFullProcessImageNameW(process, 0, name.as_mut_ptr(), &mut len) } != 0;
    let time_ok = unsafe { GetProcessTimes(process, &mut created, &mut exited, &mut kernel, &mut user) } != 0;
    unsafe { CloseHandle(process); }
    if !name_ok || !time_ok { return Err("Could not verify executable path/process creation time".into()); }
    let path = String::from_utf16_lossy(&name[..len as usize]);
    let ticks = (created.high as u64) << 32 | created.low as u64;
    if !path.eq_ignore_ascii_case(GRID_EXE) || ticks != expected_creation {
        return Err("GRID executable or process creation time mismatch".into());
    }
    Ok(())
}

fn run() -> Result<(), String> {
    if size_of::<Input>() != 40 { return Err("This experiment requires the Windows x64 INPUT ABI".into()); }
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 4 && args.len() != 6 {
        return Err("Usage: grid-timed-input PID CREATION_FILETIME inspect | PID CREATION_FILETIME hold KEY MILLISECONDS".into());
    }
    let pid: u32 = args[1].parse().map_err(|_| "Invalid PID")?;
    let creation: u64 = args[2].parse().map_err(|_| "Invalid process creation FILETIME")?;
    verify(pid, creation)?;
    if args[3] == "inspect" && args.len() == 4 {
        println!("{{\"pid\":{pid},\"foreground_verified\":true,\"input_bytes\":{},\"injected\":false}}", size_of::<Input>());
        return Ok(());
    }
    if args.len() != 6 || args[3] != "hold" { return Err("Invalid action".into()); }
    let key = Key::named(&args[4]).ok_or("Allowed keys: up, down, left, right, enter, escape")?;
    let milliseconds: u64 = args[5].parse().map_err(|_| "Invalid duration")?;
    if !(30..=2000).contains(&milliseconds) { return Err("Hold must be 30..2000 ms".into()); }
    for vk in [0x10, 0x11, 0x12, 0x25, 0x26, 0x27, 0x28, 0x0d, 0x1b, 0x7b] {
        if held(vk) { return Err(format!("Physical/control key already held: VK {vk:x}")); }
    }
    if unsafe { SetConsoleCtrlHandler(Some(console_stop), 1) } == 0 {
        return Err("Could not install console key-release handler".into());
    }
    let _release = ReleaseGuard;
    verify(pid, creation)?;
    // Mark before sending so cleanup also covers an interruption near key-down.
    HELD.store(key.encoded(), Ordering::SeqCst);
    let started = Instant::now();
    send(key, false)?;
    let target = Duration::from_millis(milliseconds);
    let mut aborted = None;
    while started.elapsed() < target {
        if foreground_pid() != pid { aborted = Some("focus_lost"); break; }
        if held(0x7b) || (key.vk != 0x1b && held(0x1b)) { aborted = Some("physical_stop_key"); break; }
        thread::sleep(Duration::from_millis(2).min(target.saturating_sub(started.elapsed())));
    }
    release_held()?;
    println!("{{\"pid\":{pid},\"key\":\"{}\",\"scan\":{},\"extended\":{},\"requested_ms\":{milliseconds},\"observed_ms\":{:.3},\"released\":true,\"aborted\":{}}}",
             args[4], key.scan, key.extended, started.elapsed().as_secs_f64()*1000.0,
             aborted.map(|reason| format!("\"{reason}\"")).unwrap_or("null".into()));
    if aborted.is_some() { return Err("Input experiment stopped; key release sent".into()); }
    Ok(())
}
fn main() {
    if let Err(error) = run() { eprintln!("{error}"); std::process::exit(1); }
}
