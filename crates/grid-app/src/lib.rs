//! Android GameActivity library; desktop CPU-only shader composition test harness.
//!
//! Ordinary Android builds include the same main.rs and retain its bevy_main entry.
//! Ordinary desktop builds stay empty; desktop library TESTS compile only the
//! standalone CPU shader composition module, avoiding the binary's full test crate.
#![cfg(any(target_os = "android", test))]

#[cfg(target_os = "android")]
include!("main.rs");

#[cfg(all(test, not(target_os = "android")))]
mod saved_tread_basis_composition;
