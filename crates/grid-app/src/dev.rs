//! Developer switches (backlog row 132, docs/SETTINGS.md "Player and
//! developer builds"). The `dev-tools` feature (on by default) keeps the
//! diagnostic tooling: the `GRID_*` environment switches, the
//! `[diagnostics]` settings section, the developer `--help` and the probe
//! entry points. The player build (`--no-default-features`) reads none of the
//! diagnostic variables, so it always runs the default path.

use std::ffi::OsString;

/// Whether this build carries the developer tooling.
pub const ENABLED: bool = cfg!(feature = "dev-tools");

/// Variables the player build reads too: hand-offs the program sets for
/// itself and the release smoke run (`GRID_MENU_AUTO=TICKS`: the menu starts
/// its saved event and the race ends after that many ticks;
/// docs/EVENT_MENU.md).
const PLAYER: &[&str] = &["GRID_SERIES_EXPERIMENTAL", "GRID_MENU_AUTO", "GRID_MENU_LOOPS"];

fn allowed(name: &str) -> bool {
    ENABLED || PLAYER.contains(&name)
}

/// `std::env::var` for a developer switch: never set in the player build.
pub fn var(name: &str) -> Result<String, std::env::VarError> {
    if allowed(name) {
        std::env::var(name)
    } else {
        Err(std::env::VarError::NotPresent)
    }
}

/// `std::env::var_os` for a developer switch: never set in the player build.
pub fn var_os(name: &str) -> Option<OsString> {
    if allowed(name) {
        std::env::var_os(name)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn player_variables_pass_in_both_builds() {
        assert!(super::allowed("GRID_MENU_AUTO"));
        assert_eq!(super::allowed("GRID_PERF_TIMING"), super::ENABLED);
    }
}
