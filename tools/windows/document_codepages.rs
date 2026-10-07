//! Read-only diagnostic: compare ANSI/Unicode Documents lookup under a manifest.
use std::ffi::c_void;

#[link(name = "shell32")]
unsafe extern "system" {
    fn SHGetFolderPathA(
        hwnd: *mut c_void,
        folder: i32,
        token: *mut c_void,
        flags: u32,
        path: *mut u8,
    ) -> i32;
    fn SHGetFolderPathW(
        hwnd: *mut c_void,
        folder: i32,
        token: *mut c_void,
        flags: u32,
        path: *mut u16,
    ) -> i32;
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetACP() -> u32;
    fn GetFileAttributesA(path: *const u8) -> u32;
    fn GetFileAttributesW(path: *const u16) -> u32;
    fn GetLastError() -> u32;
}

fn main() {
    let mut ansi = [0_u8; 260];
    let mut wide = [0_u16; 260];
    unsafe {
        let a_result = SHGetFolderPathA(
            std::ptr::null_mut(),
            5,
            std::ptr::null_mut(),
            0,
            ansi.as_mut_ptr(),
        );
        let w_result = SHGetFolderPathW(
            std::ptr::null_mut(),
            5,
            std::ptr::null_mut(),
            0,
            wide.as_mut_ptr(),
        );
        let a_attr = GetFileAttributesA(ansi.as_ptr());
        let a_error = if a_attr == u32::MAX {
            GetLastError()
        } else {
            0
        };
        let w_attr = GetFileAttributesW(wide.as_ptr());
        let w_error = if w_attr == u32::MAX {
            GetLastError()
        } else {
            0
        };
        let a_len = ansi.iter().position(|&b| b == 0).unwrap_or(260);
        let w_len = wide.iter().position(|&b| b == 0).unwrap_or(260);
        println!("ACP={}", GetACP());
        println!("DocumentsA_HRESULT=0x{:08x}", a_result as u32);
        println!("DocumentsA_Bytes={:02x?}", &ansi[..a_len]);
        println!("DocumentsA_Attributes=0x{a_attr:08x} Error={a_error}");
        println!("DocumentsW_HRESULT=0x{:08x}", w_result as u32);
        println!(
            "DocumentsW_Path={}",
            String::from_utf16_lossy(&wide[..w_len])
        );
        println!("DocumentsW_Attributes=0x{w_attr:08x} Error={w_error}");
    }
}
