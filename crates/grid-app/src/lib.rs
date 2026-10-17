//! The Android library of the prototype (P9, docs/ANDROID.md).
//!
//! GameActivity loads `libgrid_android.so` and calls the `android_main` that
//! `#[bevy_main]` generates on `main` in main.rs. The library is the same
//! source as the desktop binary; on every other target it is empty, so the
//! desktop build, its tests and its executable are unchanged.
#![cfg(target_os = "android")]

include!("main.rs");
