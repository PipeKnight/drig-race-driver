//! The desktop program's crate root. The player build (without `dev-tools`)
//! is a Windows GUI program: no console window opens when it starts
//! (docs/SETTINGS.md section 6). main.rs is shared with the Android library
//! (lib.rs), so the crate attribute lives here.
#![cfg_attr(all(windows, not(feature = "dev-tools"), not(test)), windows_subsystem = "windows")]

include!("main.rs");
