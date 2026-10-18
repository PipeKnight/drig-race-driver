//! Final check of the code-only APK's data import (P10, docs/ANDROID.md
//! "Importing the game data"): after `GridImporter.java` has copied the
//! user's files, `NativeCheck.deriveTables` derives CD01, BP01 and DF01 from
//! the imported `grid.exe` exactly as the game does at start (P5,
//! `settings::Private::Derive`), so a wrong executable is reported on the
//! import screen instead of when the race loads.

use std::path::Path;

use grid_tools::private_parameters as p;
use jni::JNIEnv;
use jni::objects::{JClass, JString};
use jni::sys::jstring;

/// One table from the executable (grid_tools::private_parameters).
type Derive = fn(&[u8]) -> Result<Vec<u8>, String>;

/// `None` when all three tables derive; otherwise the error text.
fn derive_tables(grid: &Path) -> Option<String> {
    let exe = match std::fs::read(grid.join("grid.exe")) {
        Ok(exe) => exe,
        Err(e) => return Some(format!("grid.exe: {e}")),
    };
    let tables: [(&str, Derive); 3] = [
        ("CD01", p::contact_directions),
        ("BP01", p::body_impact),
        ("DF01", p::deformation),
    ];
    for (kind, derive) in tables {
        match derive(&exe) {
            Ok(bytes) => crate::android::log(
                false,
                &format!(
                    "import_derive kind={kind} sha256={}",
                    p::hex(&p::sha256(&bytes))
                ),
            ),
            Err(e) => return Some(format!("{kind} from grid.exe: {e}")),
        }
    }
    None
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_reassembly_grid_NativeCheck_deriveTables(
    mut env: JNIEnv,
    _class: JClass,
    grid: JString,
) -> jstring {
    let error = match env.get_string(&grid) {
        Ok(dir) => derive_tables(Path::new(&String::from(dir))),
        Err(e) => Some(format!("folder name: {e}")),
    };
    match error {
        None => std::ptr::null_mut(),
        Some(text) => env
            .new_string(text)
            .map_or(std::ptr::null_mut(), JString::into_raw),
    }
}
