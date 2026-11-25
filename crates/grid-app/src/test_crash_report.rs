//! Names the thread and the code of an access violation in the grid-app test
//! process (Windows test builds only).
//!
//! A fault in a driver kills the whole libtest process with 0xc0000005, and
//! the Windows event log names only the module and offset: not the thread,
//! the test or the call path (F922: nvwgf2umx.dll+0x8040b2). This vectored
//! exception handler, installed before `main` like the compute pool
//! (`mobile::INIT_TEST_POOL`), writes one report per access violation to
//! stderr: the faulting module and offset, the access, the thread id and
//! name (libtest names each test's thread after the test) and the return
//! addresses of the faulting stack as module+offset. It then lets the
//! exception continue, so handled faults and the crash itself are unchanged.
//!
//! The handler does not allocate except through `GetThreadDescription`
//! (LocalAlloc), formats into a stack buffer and writes with `WriteFile`, so
//! it takes no Rust lock (stderr, the allocator) the faulting thread might
//! hold.

use core::ffi::c_void;
use core::fmt::Write as _;
use core::sync::atomic::{AtomicU32, Ordering};

#[repr(C)]
struct ExceptionRecord {
    code: u32,
    flags: u32,
    record: *mut ExceptionRecord,
    address: *mut c_void,
    parameters: u32,
    information: [usize; 15],
}

#[repr(C)]
struct ExceptionPointers {
    record: *mut ExceptionRecord,
    context: *mut c_void,
}

type Handler = unsafe extern "system" fn(*mut ExceptionPointers) -> i32;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn AddVectoredExceptionHandler(first: u32, handler: Handler) -> *mut c_void;
    fn GetCurrentThreadId() -> u32;
    fn GetCurrentThread() -> isize;
    fn GetThreadDescription(thread: isize, description: *mut *mut u16) -> i32;
    fn LocalFree(memory: *mut u16) -> *mut u16;
    fn GetModuleHandleExW(flags: u32, address: *const u16, module: *mut *mut c_void) -> i32;
    fn GetModuleFileNameW(module: *mut c_void, name: *mut u16, size: u32) -> u32;
    fn GetStdHandle(which: u32) -> *mut c_void;
    fn WriteFile(file: *mut c_void, data: *const u8, size: u32, written: *mut u32, overlapped: *mut c_void) -> i32;
    fn RtlCaptureStackBackTrace(skip: u32, count: u32, frames: *mut *mut c_void, hash: *mut u32) -> u16;
}

const ACCESS_VIOLATION: u32 = 0xc000_0005;
const CONTINUE_SEARCH: i32 = 0;
/// Reports per process: a crash loop must not flood the log.
const MAX_REPORTS: u32 = 8;
static REPORTS: AtomicU32 = AtomicU32::new(0);

/// A fixed buffer that drops what does not fit.
struct Buffer {
    bytes: [u8; 8192],
    len: usize,
}

impl core::fmt::Write for Buffer {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let n = s.len().min(self.bytes.len() - self.len);
        self.bytes[self.len..self.len + n].copy_from_slice(&s.as_bytes()[..n]);
        self.len += n;
        Ok(())
    }
}

/// Writes `address` as `module+0xoffset` (file name only), or the raw address.
fn write_location(out: &mut Buffer, address: usize) {
    const FROM_ADDRESS: u32 = 0x4;
    const UNCHANGED_REFCOUNT: u32 = 0x2;
    let mut module = core::ptr::null_mut();
    // SAFETY: FROM_ADDRESS reads `address` as an address, not a string.
    let found = unsafe { GetModuleHandleExW(FROM_ADDRESS | UNCHANGED_REFCOUNT, address as *const u16, &mut module) };
    if found == 0 {
        let _ = write!(out, "{address:#x}");
        return;
    }
    let mut name = [0u16; 260];
    // SAFETY: the buffer and its length match.
    let len = unsafe { GetModuleFileNameW(module, name.as_mut_ptr(), name.len() as u32) } as usize;
    let path = &name[..len.min(name.len())];
    let file = path.rsplit(|&c| c == u16::from(b'\\')).next().unwrap_or(path);
    for c in char::decode_utf16(file.iter().copied()) {
        let _ = out.write_char(c.unwrap_or('?'));
    }
    let _ = write!(out, "+{:#x}", address - module as usize);
}

unsafe extern "system" fn report(pointers: *mut ExceptionPointers) -> i32 {
    // SAFETY: the system passes valid exception pointers to the handler.
    let record = unsafe { &*(*pointers).record };
    if record.code != ACCESS_VIOLATION || REPORTS.fetch_add(1, Ordering::Relaxed) >= MAX_REPORTS {
        return CONTINUE_SEARCH;
    }
    let mut out = Buffer { bytes: [0; 8192], len: 0 };
    let _ = out.write_str("\ntest_crash_report: access violation at ");
    write_location(&mut out, record.address as usize);
    if record.parameters >= 2 {
        let access = match record.information[0] {
            0 => "read of",
            1 => "write to",
            8 => "execute at",
            _ => "access to",
        };
        let _ = write!(out, " ({access} {:#x})", record.information[1]);
    }
    // SAFETY: plain thread queries; the description is freed with LocalFree.
    let id = unsafe { GetCurrentThreadId() };
    let _ = write!(out, " on thread {id} \"");
    let mut description = core::ptr::null_mut();
    if unsafe { GetThreadDescription(GetCurrentThread(), &mut description) } >= 0 && !description.is_null() {
        let mut len = 0;
        // SAFETY: the description is a NUL-terminated wide string.
        while len < 512 && unsafe { *description.add(len) } != 0 {
            len += 1;
        }
        let name = unsafe { core::slice::from_raw_parts(description, len) };
        for c in char::decode_utf16(name.iter().copied()) {
            let _ = out.write_char(c.unwrap_or('?'));
        }
        unsafe { LocalFree(description) };
    }
    let _ = out.write_str("\"\ntest_crash_report: stack");
    let mut frames = [core::ptr::null_mut(); 48];
    // SAFETY: the array and its length match.
    let n = unsafe { RtlCaptureStackBackTrace(0, frames.len() as u32, frames.as_mut_ptr(), core::ptr::null_mut()) };
    for &frame in &frames[..usize::from(n)] {
        let _ = out.write_str("\n  ");
        write_location(&mut out, frame as usize);
    }
    let _ = out.write_str("\n");
    const STD_ERROR: u32 = -12i32 as u32;
    let mut written = 0;
    // SAFETY: raw handle write of the formatted bytes; no Rust lock taken.
    unsafe {
        WriteFile(GetStdHandle(STD_ERROR), out.bytes.as_ptr(), out.len as u32, &mut written, core::ptr::null_mut());
    }
    CONTINUE_SEARCH
}

extern "C" fn install() {
    // Last in the handler list: handlers that resolve their own faults run first.
    // SAFETY: registers a static function; the handle is never removed.
    unsafe { AddVectoredExceptionHandler(0, report) };
}

/// Installs [`report`] at process start, before libtest runs any test.
#[used]
#[unsafe(link_section = ".CRT$XCU")]
static INSTALL: extern "C" fn() = install;
