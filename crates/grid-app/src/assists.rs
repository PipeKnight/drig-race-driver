//! Driver-aid settings for the driving prototype and the manual gearbox requests.
//! The aids themselves are the original's ([research](../../../docs/DRIVING_ASSISTS_RESEARCH.md)).
use crate::input;
use grid_tools::source_vehicle::{
    Runtime,
    assists::{Assists, Shift, Transmission},
};

pub const HELP: &str = "--assists SPEC | --assists-file PATH: original driver aids, comma-separated, e.g. tcs=off,stability=off,braking=on,gearbox=manual,input=keyboard|wheel,damage=realistic|normal|off (default: all on, automatic, keyboard, realistic damage as in a fresh original profile). The file may add an [input] section (see --input). Manual gearbox: X gear up, Z gear down (pad: A up, X down).";

/// Settings file: the same `key=value` items, one or more per line, `#`
/// comments. Lines before any section header and in `[assists]` are driver
/// aids; `[input]` holds the controller settings.
pub fn read_file(path: &str) -> Result<(Assists, input::Settings), Box<dyn std::error::Error>> {
    if std::fs::metadata(path)?.len() > 64 * 1024 {
        return Err("assists settings file exceeds 64 KiB".into());
    }
    let (mut aids, mut controls) = (String::new(), String::new());
    let mut section = &mut aids;
    for line in std::fs::read_to_string(path)?.lines() {
        let item = line.split('#').next().unwrap_or("").trim();
        if let Some(name) = item.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            section = match name.trim().to_ascii_lowercase().as_str() {
                "assists" => &mut aids,
                "input" => &mut controls,
                _ => return Err(format!("unknown settings section [{name}]").into()),
            };
            continue;
        }
        section.push_str(line);
        section.push('\n');
    }
    Ok((Assists::parse(&aids)?, input::Settings::parse(&controls)?))
}

/// Gear Up/Down and direct gears from the input edge queue; manual only.
pub fn request_shift(shift: Option<Shift>, runtime: &mut Runtime) {
    if runtime.assists.transmission != Transmission::Manual {
        return;
    }
    if let Some(shift) = shift {
        // Only rejects invalid direct-gear values; Up/Down always succeed.
        let _ = runtime.request_shift(shift);
    }
}
