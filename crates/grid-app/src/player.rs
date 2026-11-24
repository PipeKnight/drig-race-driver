//! The player command line (backlog row 134, docs/SETTINGS.md "Player
//! flags"): the small, stable set `--help` documents. Every other flag is a
//! developer flag (`--help-dev` in a `dev-tools` build).
//!
//! With no flags, or only player flags, the program opens the front end (the
//! one-window menu host, front_end_host.rs). The window and quality flags are
//! settings overrides ([`overrides`]): `settings::load` applies them after the
//! settings files, so every screen and race of the run reads them.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

/// The release version (workspace `version`).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The player flags that take a value.
const WITH_VALUE: &[&str] = &[
    "--game-root",
    "--data-dir",
    "--resolution",
    "--render-height",
    "--quality",
    "--track",
];
/// The player flags without a value.
const SWITCHES: &[&str] = &[
    "--fullscreen",
    "--windowed",
    "--race",
    "--career",
    "--list-tracks",
    "--skip-intro",
    "--help",
    "-h",
    "--version",
];

pub fn help() -> String {
    format!(
        "DRIG {VERSION} - an independent open-source engine for Race Driver: GRID (2008).
It needs your own installed copy of GRID; no game data is included.

Usage: DRIG [FLAGS]
With no flags the menu opens.

  --game-root PATH      the GRID game folder (default: searched in the Steam libraries)
  --data-dir DIR        where settings, records, replays and the log go
                        (default: DRIG-data next to the program, or the per-user
                        folder when that is not writable)
  --fullscreen          borderless full screen on the current display ([video] window)
  --windowed            a window (the default)
  --resolution WxH      window size, e.g. 1920x1080 (default 1280x800; [video] resolution)
  --render-height H     render scale: auto, native or a height in rows, 240 to 4320
                        (default native; [video] render_height)
  --quality Q           quality preset: off, low, medium, high or ultra; sets the
                        shadow quality (default high; [video] shadows)
  --race                skip the menu: start the last event chosen in it (or the default one)
  --track ROUTE         start a race on this route, e.g. donington or
                        tracks/europe/donington/route_0 (implies --race)
  --list-tracks         print the routes the menu offers
  --career              open the career instead of the event menu
  --skip-intro          accepted; DRIG has no intro yet
  --version             print the version
  --help                print this text

Settings can also be written in DATA_DIR/settings.ini (docs/SETTINGS.md);
flags override them for this run. In-game updates are not available yet.
"
    )
}

pub fn version() -> String {
    format!("DRIG {VERSION} (revision {})", crate::BUILD_REVISION)
}

/// What the command line asks for.
#[derive(Debug, PartialEq)]
pub enum Start {
    Help,
    Version,
    /// The front end (menu or `--race`).
    Menu,
    Career,
    ListTracks,
    /// Developer flags: the long-standing dispatch in main.rs.
    Developer,
}

/// Whether every argument is a player flag (with its value).
pub fn only_player_flags(args: &[String]) -> bool {
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        if WITH_VALUE.contains(&a) {
            if i + 1 >= args.len() {
                return false;
            }
            i += 2;
        } else if SWITCHES.contains(&a) {
            i += 1;
        } else {
            return false;
        }
    }
    true
}

pub fn start(args: &[String]) -> Start {
    let has = |flag: &str| args.iter().any(|a| a == flag);
    if !only_player_flags(args) {
        return Start::Developer;
    }
    if has("--help") || has("-h") {
        Start::Help
    } else if has("--version") {
        Start::Version
    } else if has("--list-tracks") {
        Start::ListTracks
    } else if has("--career") {
        Start::Career
    } else {
        Start::Menu
    }
}

/// The last value of a flag.
fn value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.windows(2).rev().find(|w| w[0] == flag).map(|w| w[1].as_str())
}

/// The front end's quick start (`--race`, `--track`): taken by the first
/// menu screen of the run, which then continues at once.
#[derive(Debug, Default)]
pub struct Quick {
    pub track: Option<String>,
}

static QUICK: OnceLock<Quick> = OnceLock::new();
static QUICK_TAKEN: AtomicBool = AtomicBool::new(false);

/// Records `--race` / `--track` for the first menu screen.
pub fn set_quick_start(args: &[String]) {
    let track = value(args, "--track").map(str::to_owned);
    if track.is_some() || args.iter().any(|a| a == "--race") {
        let _ = QUICK.set(Quick { track });
    }
}

/// The quick start, once per run.
pub fn take_quick_start() -> Option<&'static Quick> {
    let quick = QUICK.get()?;
    (!QUICK_TAKEN.swap(true, Ordering::Relaxed)).then_some(quick)
}

/// The settings text of the player flags, applied by `settings::load` after
/// the settings files.
pub fn overrides(args: &[String]) -> String {
    let mut video = Vec::new();
    for (i, a) in args.iter().enumerate() {
        match a.as_str() {
            "--fullscreen" => video.push("window = fullscreen".to_owned()),
            "--windowed" => video.push("window = windowed".to_owned()),
            "--resolution" | "--render-height" | "--quality" => {
                if let Some(v) = args.get(i + 1) {
                    let key = match a.as_str() {
                        "--resolution" => "resolution",
                        "--render-height" => "render_height",
                        _ => "shadows",
                    };
                    video.push(format!("{key} = {v}"));
                    if key == "render_height" {
                        video.push(format!(
                            "render_height_version = {}",
                            crate::settings::RENDER_HEIGHT_VERSION
                        ));
                    }
                }
            }
            _ => {}
        }
    }
    if video.is_empty() {
        String::new()
    } else {
        format!("[video]\n{}\n", video.join("\n"))
    }
}

/// Resolves `--track` against the menu's routes: an exact route, else the
/// first route with a path part or venue equal to it (case-insensitive).
pub fn resolve_track<'a>(
    track: &str,
    routes: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Option<String> {
    let want = track.trim().to_ascii_lowercase();
    let mut first = None;
    for (route, venue) in routes {
        let lower = route.to_ascii_lowercase();
        if lower == want {
            return Some(route.to_owned());
        }
        if first.is_none()
            && (lower.split('/').any(|part| part == want) || venue.to_ascii_lowercase() == want)
        {
            first = Some(route.to_owned());
        }
    }
    first
}

/// The run's output (row 130, docs/SETTINGS.md "Run logs"): from a terminal
/// the text goes there; started without one (Explorer, a desktop file,
/// Finder) stdout and stderr go to a new file
/// `DATA_DIR/playtest/drig-<UTC yyyymmddTHHMMSSZ>-<revision>.log`, and only
/// the newest [`RUN_LOGS_KEPT`] such files stay. A redirect to a file or pipe
/// is kept, so the developer launchers' logs and a menu's child processes
/// (which inherit the parent's log) are unchanged. One path for Windows,
/// Linux and macOS, in both builds; Android writes its own files
/// (CrashLog.java, same naming and count).
pub fn setup_output(args: &[String], start: &Start) {
    let console_text = matches!(start, Start::Help | Start::Version | Start::ListTracks);
    if !os::detached() {
        return;
    }
    if console_text && os::attach_parent_console() {
        return;
    }
    let dir = value(args, "--data-dir")
        .map(PathBuf::from)
        .unwrap_or_else(crate::settings::default_data_dir)
        .join("playtest");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    if let Ok((path, file)) = run_log::create(&dir, &run_log::utc_stamp(now), crate::BUILD_REVISION) {
        os::redirect(file);
        println!("run_log path={}", path.display());
        for old in run_log::prune(&dir, RUN_LOGS_KEPT) {
            println!("run_log removed={}", old.display());
        }
    }
}

/// Run logs kept in `DATA_DIR/playtest/`, the new one included (as Android's
/// `CrashLog.KEEP`).
pub const RUN_LOGS_KEPT: usize = 10;

/// The per-run log files (`setup_output`): naming, the same-second clash
/// suffix and retention. Only names of exactly this form are counted or
/// deleted; `drig.log`, `drig.previous.log`, the time-trial records and the
/// developer launchers' logs in the same folder are never touched.
mod run_log {
    use std::fs::File;
    use std::path::{Path, PathBuf};

    /// `yyyymmddTHHMMSSZ` for seconds since the Unix epoch (UTC).
    pub fn utc_stamp(unix_secs: u64) -> String {
        let days = (unix_secs / 86_400) as i64;
        let rest = unix_secs % 86_400;
        // Civil date from days since 1970-01-01 (H. Hinnant's algorithm).
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = doy - (153 * mp + 2) / 5 + 1;
        let month = if mp < 10 { mp + 3 } else { mp - 9 };
        let year = yoe + era * 400 + i64::from(month <= 2);
        format!(
            "{year:04}{month:02}{day:02}T{:02}{:02}{:02}Z",
            rest / 3600,
            rest / 60 % 60,
            rest % 60
        )
    }

    /// The revision part of the name: letters, digits and `-` only (a
    /// `-dirty` mark stays); a longer hex hash is cut to 12 digits, as the
    /// packagers set it.
    pub fn revision_tag(revision: &str) -> String {
        let tag: String = revision
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
            .collect();
        if tag.len() > 12 && tag.chars().all(|c| c.is_ascii_hexdigit()) {
            tag[..12].to_owned()
        } else if tag.is_empty() {
            "unversioned".to_owned()
        } else {
            tag
        }
    }

    /// `drig-STAMP-REV.log`; on a clash (two starts in one second)
    /// `drig-STAMP-REV.N.log` with N from 2.
    pub fn name(stamp: &str, revision: &str, clash: u32) -> String {
        let revision = revision_tag(revision);
        if clash < 2 {
            format!("drig-{stamp}-{revision}.log")
        } else {
            format!("drig-{stamp}-{revision}.{clash}.log")
        }
    }

    /// The order of a run log among the others (stamp, then clash number,
    /// then name); `None` for every other file name.
    pub fn key(name: &str) -> Option<(&str, u32, &str)> {
        let body = name.strip_prefix("drig-")?.strip_suffix(".log")?;
        let stamp = body.get(..16)?;
        let b = stamp.as_bytes();
        let stamp_ok = b[..8].iter().chain(&b[9..15]).all(u8::is_ascii_digit)
            && b[8] == b'T'
            && b[15] == b'Z';
        let rest = body[16..].strip_prefix('-')?;
        let (revision, clash) = match rest.rsplit_once('.') {
            Some((revision, n)) => (revision, n.parse::<u32>().ok().filter(|n| *n >= 2)?),
            None => (rest, 1),
        };
        let revision_ok =
            !revision.is_empty() && revision.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
        (stamp_ok && revision_ok).then_some((stamp, clash, name))
    }

    /// Creates the run's file; a name another start already took gets the
    /// next clash number (`create_new`, so two processes never share one).
    pub fn create(dir: &Path, stamp: &str, revision: &str) -> std::io::Result<(PathBuf, File)> {
        let mut last = None;
        for clash in 1..=999 {
            let path = dir.join(name(stamp, revision, clash));
            match File::options().write(true).create_new(true).open(&path) {
                Ok(file) => return Ok((path, file)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => last = Some(error),
                Err(error) => return Err(error),
            }
        }
        Err(last.unwrap_or_else(|| std::io::Error::other("no free run log name")))
    }

    /// Deletes all but the newest `keep` run logs in `dir`; returns the
    /// deleted paths. A file that cannot be deleted (still open on Windows)
    /// stays until a later start.
    pub fn prune(dir: &Path, keep: usize) -> Vec<PathBuf> {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return Vec::new();
        };
        let mut names: Vec<String> = entries
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().is_ok_and(|t| t.is_file()))
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|name| key(name).is_some())
            .collect();
        names.sort_by(|a, b| key(b).cmp(&key(a)));
        names
            .into_iter()
            .skip(keep)
            .map(|name| dir.join(name))
            .filter(|path| std::fs::remove_file(path).is_ok())
            .collect()
    }
}

/// A game runs at full speed even while its window is behind another one
/// (Windows power throttling; no-op elsewhere). Both builds, one path.
pub fn keep_full_speed() {
    if os::keep_full_speed() {
        println!("power_throttling off");
    }
}

#[cfg(windows)]
mod os {
    use std::os::windows::io::IntoRawHandle;

    type Handle = *mut core::ffi::c_void;
    const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
    const STD_ERROR_HANDLE: u32 = -12i32 as u32;
    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
    const INVALID_HANDLE_VALUE: Handle = -1isize as Handle;
    /// `GetCurrentProcess()` pseudo-handle; not redeclared here because
    /// fixed_ticks.rs and front_end_host.rs declare it with another return type.
    const CURRENT_PROCESS: Handle = -1isize as Handle;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetStdHandle(which: u32) -> Handle;
        fn SetStdHandle(which: u32, handle: Handle) -> i32;
        fn AttachConsole(process: u32) -> i32;
        fn SetProcessInformation(process: Handle, class: i32, info: *const core::ffi::c_void, size: u32) -> i32;
    }

    /// PROCESS_POWER_THROTTLING_STATE.
    #[repr(C)]
    struct Throttling {
        version: u32,
        control_mask: u32,
        state_mask: u32,
    }

    /// Opts the process out of Windows power throttling (EcoQoS): execution
    /// speed and timer resolution. Windows throttles a GUI-subsystem program
    /// whose window is not in front; the player build ran 19.4 ms mean, p95
    /// 29 ms, instead of 17.0/17.6 ms in the benchmark race (F906 row 132).
    pub fn keep_full_speed() -> bool {
        const PROCESS_POWER_THROTTLING: i32 = 4; // ProcessPowerThrottling
        const EXECUTION_SPEED: u32 = 0x1;
        const IGNORE_TIMER_RESOLUTION: u32 = 0x4;
        let state = Throttling {
            version: 1,
            control_mask: EXECUTION_SPEED | IGNORE_TIMER_RESOLUTION,
            state_mask: 0,
        };
        // SAFETY: this process's pseudo handle and a pointer to a local of
        // the size the call is given.
        unsafe {
            SetProcessInformation(
                CURRENT_PROCESS,
                PROCESS_POWER_THROTTLING,
                (&state as *const Throttling).cast(),
                size_of::<Throttling>() as u32,
            ) != 0
        }
    }

    /// No usable stdout: a GUI-subsystem start without a redirect.
    pub fn detached() -> bool {
        // SAFETY: plain query of the process's standard handle.
        let handle = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };
        handle.is_null() || handle == INVALID_HANDLE_VALUE
    }

    /// Writes to the console of the shell that started the program.
    pub fn attach_parent_console() -> bool {
        // SAFETY: attaching fails harmlessly without a parent console.
        if unsafe { AttachConsole(ATTACH_PARENT_PROCESS) } == 0 {
            return false;
        }
        let open = || {
            std::fs::OpenOptions::new().write(true).open("CONOUT$").ok()
        };
        match (open(), open()) {
            (Some(out), Some(err)) => {
                // SAFETY: the handles are owned by the process from here on
                // (never closed); std reads the standard handle on each write.
                unsafe {
                    SetStdHandle(STD_OUTPUT_HANDLE, out.into_raw_handle());
                    SetStdHandle(STD_ERROR_HANDLE, err.into_raw_handle());
                }
                // The shell's prompt was already printed.
                println!();
                true
            }
            _ => false,
        }
    }

    pub fn redirect(file: std::fs::File) {
        let Ok(copy) = file.try_clone() else { return };
        // SAFETY: as in `attach_parent_console`.
        unsafe {
            SetStdHandle(STD_OUTPUT_HANDLE, file.into_raw_handle());
            SetStdHandle(STD_ERROR_HANDLE, copy.into_raw_handle());
        }
    }
}

#[cfg(unix)]
mod os {
    use std::os::fd::AsRawFd;

    /// Stdout is not a terminal, file or pipe: a desktop-file or Finder start
    /// (`/dev/null` or the session journal).
    pub fn detached() -> bool {
        // SAFETY: `isatty`/`fstat` only read the descriptor's state.
        unsafe {
            if libc::isatty(1) == 1 {
                return false;
            }
            let mut stat: libc::stat = std::mem::zeroed();
            if libc::fstat(1, &mut stat) != 0 {
                return true;
            }
            let kind = stat.st_mode & libc::S_IFMT;
            kind != libc::S_IFREG && kind != libc::S_IFIFO
        }
    }

    pub fn attach_parent_console() -> bool {
        false
    }

    pub fn keep_full_speed() -> bool {
        false
    }

    pub fn redirect(file: std::fs::File) {
        // SAFETY: duplicates an open descriptor onto stdout and stderr; the
        // file stays open through the duplicates.
        unsafe {
            libc::dup2(file.as_raw_fd(), 1);
            libc::dup2(file.as_raw_fd(), 2);
        }
    }
}

#[cfg(not(any(windows, unix)))]
mod os {
    pub fn detached() -> bool {
        false
    }
    pub fn attach_parent_console() -> bool {
        false
    }
    pub fn keep_full_speed() -> bool {
        false
    }
    pub fn redirect(_file: std::fs::File) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(text: &str) -> Vec<String> {
        text.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn player_flags_open_the_front_end() {
        assert_eq!(start(&[]), Start::Menu);
        assert_eq!(start(&args("--fullscreen --quality low --game-root D:/Grid")), Start::Menu);
        assert_eq!(start(&args("--track donington")), Start::Menu);
        assert_eq!(start(&args("--career --windowed")), Start::Career);
        assert_eq!(start(&args("--help")), Start::Help);
        assert_eq!(start(&args("--version")), Start::Version);
        assert_eq!(start(&args("--list-tracks --game-root X")), Start::ListTracks);
        // Developer flags keep the old dispatch.
        assert_eq!(start(&args("--menu")), Start::Developer);
        assert_eq!(start(&args("--drive --route tracks/x")), Start::Developer);
        // A missing value is not a player command line.
        assert_eq!(start(&args("--game-root")), Start::Developer);
    }

    #[test]
    fn overrides_become_video_settings() {
        let text = overrides(&args(
            "--fullscreen --resolution 1920x1080 --render-height 720 --quality low",
        ));
        let mut s = crate::settings::Settings::new();
        s.apply(&text, std::path::Path::new(".")).unwrap();
        assert_eq!(s.window, Some(crate::settings::WindowMode::Fullscreen));
        assert_eq!(s.window_resolution, Some((1920, 1080)));
        assert_eq!(s.shadows, Some(crate::shadow_layout::Quality::parse("low").unwrap()));
        assert_eq!(
            s.video_render_height(),
            crate::render_scale::RenderHeight::parse("720").unwrap()
        );
        assert!(overrides(&args("--race --game-root X")).is_empty());
    }

    #[test]
    fn bad_override_values_are_refused() {
        let mut s = crate::settings::Settings::new();
        assert!(s.apply(&overrides(&args("--quality extreme")), std::path::Path::new(".")).is_err());
        assert!(s.apply(&overrides(&args("--resolution big")), std::path::Path::new(".")).is_err());
    }

    #[test]
    fn tracks_resolve_by_route_part_or_venue() {
        let routes = [
            ("tracks/europe/donington/route_0", "Donington Park"),
            ("tracks/europe/donington/route_1", "Donington Park"),
            ("tracks/usa/washington/route_0", "Washington DC"),
        ];
        let find = |t: &str| resolve_track(t, routes.iter().copied());
        assert_eq!(find("donington").as_deref(), Some("tracks/europe/donington/route_0"));
        assert_eq!(find("tracks/europe/donington/route_1").as_deref(), Some("tracks/europe/donington/route_1"));
        assert_eq!(find("Washington DC").as_deref(), Some("tracks/usa/washington/route_0"));
        assert_eq!(find("nowhere"), None);
    }

    #[test]
    fn quick_start_is_taken_once() {
        set_quick_start(&args("--track donington"));
        assert_eq!(take_quick_start().and_then(|q| q.track.as_deref()), Some("donington"));
        assert!(take_quick_start().is_none());
    }

    #[test]
    fn run_log_names_carry_utc_time_and_revision() {
        assert_eq!(run_log::utc_stamp(0), "19700101T000000Z");
        // 2026-11-25 10:15:07 UTC; 2024-02-29 23:59:59 UTC (leap day).
        assert_eq!(run_log::utc_stamp(1_795_601_707), "20261125T101507Z");
        assert_eq!(run_log::utc_stamp(1_709_251_199), "20240229T235959Z");
        let stamp = "20261125T101507Z";
        assert_eq!(run_log::name(stamp, "1ed1b05002e4", 1), "drig-20261125T101507Z-1ed1b05002e4.log");
        assert_eq!(run_log::name(stamp, "1ed1b05002e4", 2), "drig-20261125T101507Z-1ed1b05002e4.2.log");
        // A full hash is cut to 12 digits; a dirty mark and odd characters.
        assert_eq!(
            run_log::name(stamp, "1ed1b05002e40000000000000000000000000000", 1),
            "drig-20261125T101507Z-1ed1b05002e4.log"
        );
        assert_eq!(run_log::revision_tag("1ed1b05002e4-dirty"), "1ed1b05002e4-dirty");
        assert_eq!(run_log::revision_tag("a/b.c"), "abc");
        assert_eq!(run_log::revision_tag(""), "unversioned");
        // Only this exact form counts as a run log.
        for own in [run_log::name(stamp, "unversioned", 1), run_log::name(stamp, "123456789012", 3)] {
            assert!(run_log::key(&own).is_some(), "{own}");
        }
        for other in [
            "drig.log",
            "drig.previous.log",
            "time-trial-records.tsv",
            "manual-6944-21324.log",
            "drig-20261125T101507Z-1ed1b05002e4.Ab12Cd",
            "drig-20261125T101507Z-1ed1b05002e4.1.log",
            "drig-2026112XT101507Z-1ed1b05002e4.log",
            "drig-20261125T101507Z-.log",
            "drig-notes.log",
        ] {
            assert!(run_log::key(other).is_none(), "{other}");
        }
    }

    #[test]
    fn run_logs_get_a_clash_suffix_and_only_the_newest_stay() {
        let dir = std::env::temp_dir().join(format!("drig-run-log-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let others = ["drig.log", "drig.previous.log", "time-trial-records.tsv", "manual-1-2.log", "drig-notes.log"];
        for other in others {
            std::fs::write(dir.join(other), other).unwrap();
        }
        // Three starts in one second: distinct files, in order.
        let stamp = "20261125T101507Z";
        let made: Vec<_> = (0..3).map(|_| run_log::create(&dir, stamp, "1ed1b05002e4").unwrap().0).collect();
        let names: Vec<_> = made.iter().map(|p| p.file_name().unwrap().to_str().unwrap().to_owned()).collect();
        assert_eq!(
            names,
            [
                "drig-20261125T101507Z-1ed1b05002e4.log",
                "drig-20261125T101507Z-1ed1b05002e4.2.log",
                "drig-20261125T101507Z-1ed1b05002e4.3.log"
            ]
        );
        // Ten older starts, one per second before.
        for second in 0..10 {
            run_log::create(&dir, &format!("20261125T1014{second:02}Z"), "0123456789ab").unwrap();
        }
        let removed = run_log::prune(&dir, RUN_LOGS_KEPT);
        assert_eq!(removed.len(), 3);
        let mut left: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .filter(|name| run_log::key(name).is_some())
            .collect();
        left.sort();
        assert_eq!(left.len(), RUN_LOGS_KEPT);
        // The oldest three went; the same-second clash files all stayed.
        assert_eq!(left[0], "drig-20261125T101403Z-0123456789ab.log");
        assert!(names.iter().all(|n| left.contains(n)));
        for other in others {
            assert_eq!(std::fs::read_to_string(dir.join(other)).unwrap(), other);
        }
        assert!(run_log::prune(&dir, RUN_LOGS_KEPT).is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
