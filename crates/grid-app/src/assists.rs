//! Driver-aid settings for the driving prototype and the manual gearbox keys.
//! The aids themselves are the original's ([research](../../../docs/DRIVING_ASSISTS_RESEARCH.md)).
use bevy::prelude::*;
use grid_tools::source_vehicle::{
    Runtime,
    assists::{Assists, Shift, Transmission},
};

pub const HELP: &str = "--assists SPEC | --assists-file PATH: original driver aids, comma-separated, e.g. tcs=off,stability=off,braking=on,gearbox=manual,input=keyboard|wheel (default: all on, automatic, keyboard). Manual gearbox: X gear up, Z gear down.";

/// Settings file: the same `key=value` items, one or more per line, `#` comments.
pub fn read_file(path: &str) -> Result<Assists, Box<dyn std::error::Error>> {
    if std::fs::metadata(path)?.len() > 64 * 1024 {
        return Err("assists settings file exceeds 64 KiB".into());
    }
    Ok(Assists::parse(&std::fs::read_to_string(path)?)?)
}

/// Original keyboard preset 0: Gear Up A, Gear Down Z, edge-triggered. A is
/// the prototype's steer-left alias, so Gear Up moves to X.
pub fn shift_keys(input: &ButtonInput<KeyCode>, runtime: &mut Runtime) {
    if runtime.assists.transmission != Transmission::Manual {
        return;
    }
    let shift = if input.just_pressed(KeyCode::KeyX) {
        Shift::Up
    } else if input.just_pressed(KeyCode::KeyZ) {
        Shift::Down
    } else {
        return;
    };
    // Only rejects invalid direct-gear values; Up/Down always succeed.
    let _ = runtime.request_shift(shift);
}
