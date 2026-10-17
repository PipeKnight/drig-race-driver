//! The prototype's settings file (P5, docs/SETTINGS.md). One text file holds
//! what the launchers used to pass through `GRID_PLAY_*` variables and the
//! `GRID_*` diagnostic switches; command-line flags still override it.
//!
//! Order: built-in defaults, then `<data dir>/settings.ini` if present, then
//! each `--settings FILE` in order, then the other command-line flags.
//! Relative private-data paths resolve against the data dir.
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub const HELP: &str = "--settings FILE (repeatable): settings file, applied after DATA_DIR/settings.ini; command-line flags override it (docs/SETTINGS.md). --data-dir DIR: records, renders and the default settings file (default: local/ when the current folder has one, else the platform folder such as %APPDATA%\\GRID Reassembly). --game-root is optional: without it the Steam libraries are searched.";

/// Where a private parameter table comes from.
#[derive(Clone, Debug, PartialEq)]
pub enum Private {
    File(PathBuf),
    /// Read from the user's own `grid.exe` (grid_tools::private_parameters).
    Derive,
}

impl Private {
    pub fn from_cli(value: String) -> Self {
        if value.eq_ignore_ascii_case("derive") {
            Self::Derive
        } else {
            Self::File(value.into())
        }
    }

    /// The table bytes; `kind` is "BP01", "DF01" or "CD01".
    pub fn bytes(&self, root: &Path, kind: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        use grid_tools::private_parameters as p;
        let bytes = match self {
            Self::File(path) => std::fs::read(path)
                .map_err(|e| format!("private {kind} file {}: {e}", path.display()))?,
            Self::Derive => {
                let exe = std::fs::read(root.join("grid.exe")).or_else(|_| {
                    let index = grid_tools::install::FileIndex::build(root)?;
                    std::fs::read(index.resolve("grid.exe"))
                })?;
                match kind {
                    "BP01" => p::body_impact(&exe)?,
                    "CD01" => p::contact_directions(&exe)?,
                    _ => p::deformation(&exe)?,
                }
            }
        };
        println!(
            "drive_private_parameters kind={kind} source={} sha256={}",
            match self {
                Self::File(_) => "file",
                Self::Derive => "derived-from-grid.exe",
            },
            p::hex(&p::sha256(&bytes))
        );
        Ok(bytes)
    }
}

/// Validation and capture switches (formerly only `GRID_*` variables).
#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostics {
    /// `yaw,pitch` radians: fixed free-look view.
    pub free_look: Option<String>,
    /// `x,y,z,lx,ly,lz`: fixed driving camera.
    pub effects_camera: Option<String>,
    /// Skid marks, wheel debris and ground cover.
    pub ground_effects: bool,
    pub ground_trace: bool,
    /// The ported AI drives the player's car (scripted runs only).
    pub autopilot: bool,
    /// `TEMPLATE:INDEX`: scripted drive into a breakable prop.
    pub prop_script: Option<String>,
    pub prop_script_speed: Option<f64>,
    pub look_back: bool,
    pub damage_zones: bool,
    /// `car,x,y,z`: damage camera.
    pub damage_camera: Option<String>,
    /// Forced texture path for P8 checks: `rgba8` (CPU BC decode) or `etc2`
    /// (private ETC2 cache); `None` picks BC, ETC2, RGBA8 by GPU support.
    pub texture_fallback: Option<String>,
    /// Seconds: once a scripted drive ends, play an Instant Replay and delay
    /// the offscreen capture until it has run this long (docs/REPLAY.md).
    pub replay_capture: Option<f64>,
}

impl Default for Diagnostics {
    fn default() -> Self {
        Self {
            free_look: None,
            effects_camera: None,
            ground_effects: true,
            ground_trace: false,
            autopilot: false,
            prop_script: None,
            prop_script_speed: None,
            look_back: false,
            damage_zones: false,
            damage_camera: None,
            texture_fallback: None,
            replay_capture: None,
        }
    }
}

impl Diagnostics {
    /// The legacy `GRID_*` variables, still honoured on desktop as the base
    /// that the settings file overrides.
    pub fn from_env() -> Self {
        let text = |name| std::env::var(name).ok();
        let set = |name| std::env::var_os(name).is_some();
        Self {
            free_look: text("GRID_FREE_LOOK"),
            effects_camera: text("GRID_EFFECTS_CAMERA"),
            ground_effects: text("GRID_GROUND_EFFECTS").is_none_or(|v| v != "0"),
            ground_trace: set("GRID_GROUND_TRACE"),
            autopilot: set("GRID_AUTOPILOT"),
            prop_script: text("GRID_PROP_SCRIPT"),
            prop_script_speed: text("GRID_PROP_SCRIPT_SPEED").and_then(|v| v.parse().ok()),
            look_back: set("GRID_LOOK_BACK"),
            damage_zones: set("GRID_DAMAGE_ZONES"),
            damage_camera: text("GRID_DAMAGE_CAMERA"),
            texture_fallback: text("GRID_TEXTURE_FALLBACK")
                .and_then(|v| texture_path("GRID_TEXTURE_FALLBACK", &v).ok().flatten()),
            replay_capture: text("GRID_REPLAY_CAPTURE").and_then(|v| v.parse().ok()),
        }
    }

    fn set(&mut self, key: &str, value: &str) -> Result<(), String> {
        let text = || (!value.is_empty() && value != "off").then(|| value.to_string());
        match key {
            "free_look" => self.free_look = text(),
            "effects_camera" => self.effects_camera = text(),
            "ground_effects" => self.ground_effects = on(key, value)?,
            "ground_trace" => self.ground_trace = on(key, value)?,
            "autopilot" => self.autopilot = on(key, value)?,
            "prop_script" => self.prop_script = text(),
            "prop_script_speed" => {
                self.prop_script_speed = Some(value.parse().map_err(|_| number(key))?)
            }
            "look_back" => self.look_back = on(key, value)?,
            "damage_zones" => self.damage_zones = on(key, value)?,
            "damage_camera" => self.damage_camera = text(),
            "texture_fallback" => self.texture_fallback = texture_path(key, value)?,
            "replay_capture" => {
                self.replay_capture = Some(value.parse().map_err(|_| number(key))?)
            }
            _ => return Err(format!("unknown diagnostics setting `{key}`")),
        }
        Ok(())
    }
}

static DIAGNOSTICS: OnceLock<Diagnostics> = OnceLock::new();
static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

/// The run's diagnostics (the legacy variables when no settings were loaded,
/// as in unit tests).
pub fn diagnostics() -> &'static Diagnostics {
    DIAGNOSTICS.get_or_init(Diagnostics::from_env)
}

/// The data dir for records, renders and the default settings file.
pub fn data_dir() -> &'static Path {
    DATA_DIR.get_or_init(default_data_dir)
}

/// The owner's checkout keeps its ignored `local/` (the launchers start in the
/// repository root); elsewhere the platform's per-user application folder.
pub fn default_data_dir() -> PathBuf {
    if Path::new("local").is_dir() {
        return "local".into();
    }
    platform_data_dir().unwrap_or_else(|| "local".into())
}

fn platform_data_dir() -> Option<PathBuf> {
    let env = |name| {
        std::env::var_os(name)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    if cfg!(windows) {
        env("APPDATA").map(|p| p.join("GRID Reassembly"))
    } else if cfg!(target_os = "macos") {
        env("HOME").map(|h| h.join("Library/Application Support/GRID Reassembly"))
    } else if cfg!(target_os = "android") {
        // App-specific external storage from the activity (P9).
        android_data_dir()
    } else {
        env("XDG_DATA_HOME")
            .or_else(|| env("HOME").map(|h| h.join(".local/share")))
            .map(|p| p.join("grid-reassembly"))
    }
}

#[cfg(target_os = "android")]
fn android_data_dir() -> Option<PathBuf> {
    crate::android::data_dir()
}

#[cfg(not(target_os = "android"))]
fn android_data_dir() -> Option<PathBuf> {
    None
}

/// Values from the settings files. `None` keeps the built-in default.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Settings {
    pub root: Option<PathBuf>,
    pub route: Option<String>,
    /// `[game] car` / `opponent_cars` (docs/CAR_GENERALIZATION.md).
    pub car: Option<String>,
    pub opponent_cars: Option<String>,
    pub drive: bool,
    pub laps: Option<u32>,
    pub opponents: Option<usize>,
    pub grid_position: Option<usize>,
    /// `grid_position = back`: the last slot, after the opponents.
    pub grid_back: bool,
    pub ai_characteristics: Option<PathBuf>,
    /// `[drive] discipline`: `race` (default) or `drift_battle`
    /// (docs/DISCIPLINES.md).
    pub discipline: Option<String>,
    pub body_contacts: Option<Private>,
    pub body_events: Option<Private>,
    pub deformation: Option<Private>,
    /// `[assists]` and `[input]` text, in the assists-file grammar.
    pub assists: String,
    pub input: String,
    /// `[ai]` text, in the `--ai` grammar (grid_tools::ai_pacing::Settings).
    pub ai: String,
    /// `[sound] effects`, percent (the pause menu's Effects Volume).
    pub sound_effects: Option<u8>,
    /// `[osd]` Screen Setup of the pause menu: lap timer, race position,
    /// speed unit mph.
    pub osd_lap_timer: Option<bool>,
    pub osd_position: Option<bool>,
    pub osd_mph: Option<bool>,
    pub osd_minimap: Option<bool>,
    pub osd_dials: Option<bool>,
    pub diagnostics: Diagnostics,
}

/// `etc2`, `rgba8`, or on (= rgba8) / off (= automatic).
fn texture_path(key: &str, value: &str) -> Result<Option<String>, String> {
    match value.to_ascii_lowercase().as_str() {
        "etc2" => Ok(Some("etc2".into())),
        "rgba8" => Ok(Some("rgba8".into())),
        _ => on(key, value)
            .map(|on| on.then(|| "rgba8".into()))
            .map_err(|_| format!("setting `{key}` needs etc2, rgba8, on or off")),
    }
}

fn on(key: &str, value: &str) -> Result<bool, String> {
    match value.to_ascii_lowercase().as_str() {
        "on" | "true" | "1" | "yes" => Ok(true),
        "off" | "false" | "0" | "no" => Ok(false),
        _ => Err(format!("setting `{key}` needs on or off")),
    }
}

fn number(key: &str) -> String {
    format!("setting `{key}` needs a number")
}

/// `#` starts a comment at the start of a line or after whitespace, so paths
/// may contain `#`.
fn strip_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let cut = (0..bytes.len())
        .find(|&i| bytes[i] == b'#' && (i == 0 || bytes[i - 1].is_ascii_whitespace()));
    &line[..cut.unwrap_or(line.len())]
}

impl Settings {
    pub fn new() -> Self {
        Self {
            diagnostics: Diagnostics::from_env(),
            ..Self::default()
        }
    }

    /// Applies one settings file's text over the current values.
    pub fn apply(&mut self, text: &str, data_dir: &Path) -> Result<(), String> {
        let private_path = |value: &str| {
            let path = PathBuf::from(value);
            if path.is_absolute() {
                path
            } else {
                data_dir.join(path)
            }
        };
        let mut section = String::from("assists");
        for (n, raw) in text.lines().enumerate() {
            let at = |e: String| format!("line {}: {e}", n + 1);
            let line = strip_comment(raw).trim();
            if let Some(name) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                section = name.trim().to_ascii_lowercase();
                if ![
                    "assists",
                    "input",
                    "ai",
                    "game",
                    "drive",
                    "private",
                    "sound",
                    "osd",
                    "diagnostics",
                ]
                .contains(&section.as_str())
                {
                    return Err(at(format!("unknown settings section [{name}]")));
                }
                continue;
            }
            match section.as_str() {
                "assists" => {
                    self.assists.push_str(raw);
                    self.assists.push('\n');
                    continue;
                }
                "input" => {
                    self.input.push_str(raw);
                    self.input.push('\n');
                    continue;
                }
                "ai" => {
                    self.ai.push_str(raw);
                    self.ai.push('\n');
                    continue;
                }
                _ => {}
            }
            if line.is_empty() {
                continue;
            }
            let (key, value) = line
                .split_once('=')
                .ok_or_else(|| at(format!("`{line}` needs key = value")))?;
            let key = key.trim().to_ascii_lowercase();
            let value = value.trim().trim_matches('"');
            let off = value.eq_ignore_ascii_case("off") || value.eq_ignore_ascii_case("none");
            let count = |v: &str| v.parse::<usize>().map_err(|_| number(&key));
            let private = |v: &str| {
                if off {
                    None
                } else if v.eq_ignore_ascii_case("derive") {
                    Some(Private::Derive)
                } else {
                    Some(Private::File(private_path(v)))
                }
            };
            match (section.as_str(), key.as_str()) {
                ("game", "root") => {
                    self.root = (!value.eq_ignore_ascii_case("auto")).then(|| value.into())
                }
                ("game", "route") => self.route = Some(value.into()),
                ("game", "car") => self.car = Some(value.into()),
                ("game", "opponent_cars") => self.opponent_cars = Some(value.into()),
                ("drive", "enabled") => self.drive = on(&key, value).map_err(at)?,
                ("drive", "laps") => {
                    let laps = value.parse::<u32>().map_err(|_| at(number(&key)))?;
                    self.laps = (laps > 0).then_some(laps);
                }
                ("drive", "opponents") => {
                    let n = count(value).map_err(at)?;
                    self.opponents = (n > 0).then_some(n);
                }
                ("drive", "grid_position") => {
                    self.grid_back = value.eq_ignore_ascii_case("back");
                    self.grid_position = if self.grid_back || value == "0" {
                        None
                    } else {
                        Some(count(value).map_err(at)?)
                    };
                }
                ("drive", "discipline") => match value.to_ascii_lowercase().as_str() {
                    d @ ("race" | "drift_battle") => self.discipline = Some(d.into()),
                    _ => return Err(at("`discipline` needs race or drift_battle".into())),
                },
                ("drive", "ai_characteristics") => {
                    self.ai_characteristics = (!off).then(|| private_path(value))
                }
                ("private", "body_contacts") => self.body_contacts = private(value),
                ("private", "body_events") => self.body_events = private(value),
                ("private", "deformation") => self.deformation = private(value),
                ("sound", "effects") => {
                    let v = count(value).map_err(at)?;
                    if v > 100 {
                        return Err(at("`effects` must be 0..100".into()));
                    }
                    self.sound_effects = Some(v as u8);
                }
                ("osd", "lap_timer") => self.osd_lap_timer = Some(on(&key, value).map_err(at)?),
                ("osd", "position") => self.osd_position = Some(on(&key, value).map_err(at)?),
                ("osd", "minimap") => self.osd_minimap = Some(on(&key, value).map_err(at)?),
                ("osd", "dials") => self.osd_dials = Some(on(&key, value).map_err(at)?),
                ("osd", "speed_units") => {
                    self.osd_mph = Some(match value.to_ascii_lowercase().as_str() {
                        "kph" | "km/h" => false,
                        "mph" => true,
                        _ => return Err(at("`speed_units` needs kph or mph".into())),
                    })
                }
                ("diagnostics", key) => self.diagnostics.set(key, value).map_err(at)?,
                (section, key) => {
                    return Err(at(format!("unknown setting `{key}` in [{section}]")));
                }
            }
        }
        Ok(())
    }

    pub fn assists(&self) -> Result<grid_tools::source_vehicle::assists::Assists, String> {
        grid_tools::source_vehicle::assists::Assists::parse(&self.assists)
    }

    pub fn ai(&self) -> Result<grid_tools::ai_pacing::Settings, String> {
        grid_tools::ai_pacing::Settings::parse(&self.ai)
    }

    pub fn input(&self) -> Result<crate::input::Settings, String> {
        crate::input::Settings::parse(&self.input)
    }
}

/// Reads `--data-dir` and the settings files named by `--settings`, sets the
/// run's data dir and diagnostics, and returns the merged settings.
pub fn load(args: &[String]) -> Result<Settings, Box<dyn std::error::Error>> {
    let value = |flag: &str| -> Vec<String> {
        args.windows(2)
            .filter(|w| w[0] == flag)
            .map(|w| w[1].clone())
            .collect()
    };
    let dir = value("--data-dir")
        .pop()
        .map(PathBuf::from)
        .unwrap_or_else(default_data_dir);
    let mut files: Vec<PathBuf> = Vec::new();
    let user = dir.join("settings.ini");
    if user.is_file() {
        files.push(user);
    }
    files.extend(value("--settings").into_iter().map(PathBuf::from));
    if args
        .last()
        .is_some_and(|a| a == "--settings" || a == "--data-dir")
    {
        return Err("--settings and --data-dir need a path".into());
    }
    let mut settings = Settings::new();
    for file in &files {
        if std::fs::metadata(file)
            .map_err(|e| format!("settings file {}: {e}", file.display()))?
            .len()
            > 64 * 1024
        {
            return Err(format!("settings file {} exceeds 64 KiB", file.display()).into());
        }
        settings
            // A byte-order mark (Windows editors, PowerShell 5) is not a key.
            .apply(std::fs::read_to_string(file)?.trim_start_matches('\u{feff}'), &dir)
            .map_err(|e| format!("settings file {}: {e}", file.display()))?;
        println!("settings_file path={}", file.display());
    }
    println!("data_dir path={}", dir.display());
    let _ = DATA_DIR.set(dir);
    let _ = DIAGNOSTICS.set(settings.diagnostics.clone());
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RACE: &str = r#"
# A race profile
[game]
root = auto
route = tracks/europe/donington/route_0
car = bmw
opponent_cars = lac,bmw
[drive]
enabled = on
laps = 3
opponents = 11
grid_position = back
ai_characteristics = ai-driver-characteristics.tsv
discipline = Drift_Battle
[private]
body_contacts = body-contact-directions.bin
body_events = derive
deformation = C:\Data#1\df01.bin   # comment after a path with '#'
[assists]
tcs=off,gearbox=manual
[input]
device=pad steering_deadzone=0.1
[diagnostics]
ground_effects = off
autopilot = on
prop_script_speed = 12.5
"#;

    #[test]
    fn race_profile_parses_with_data_dir_paths() {
        let dir = Path::new("data");
        let mut s = Settings::default();
        s.apply(RACE, dir).unwrap();
        assert_eq!(s.root, None);
        assert_eq!(s.route.as_deref(), Some("tracks/europe/donington/route_0"));
        assert_eq!(s.car.as_deref(), Some("bmw"));
        assert_eq!(s.opponent_cars.as_deref(), Some("lac,bmw"));
        assert!(s.drive && s.grid_back);
        assert_eq!(s.discipline.as_deref(), Some("drift_battle"));
        assert_eq!(
            (s.laps, s.opponents, s.grid_position),
            (Some(3), Some(11), None)
        );
        assert_eq!(
            s.body_contacts,
            Some(Private::File(dir.join("body-contact-directions.bin")))
        );
        assert_eq!(s.body_events, Some(Private::Derive));
        let df01 = if cfg!(windows) {
            PathBuf::from(r"C:\Data#1\df01.bin")
        } else {
            dir.join(r"C:\Data#1\df01.bin")
        };
        assert_eq!(s.deformation, Some(Private::File(df01)));
        let aids = s.assists().unwrap();
        assert!(!aids.traction_control);
        assert_eq!(
            aids.transmission,
            grid_tools::source_vehicle::assists::Transmission::Manual
        );
        assert_eq!(s.input().unwrap().device, crate::input::Device::Pad);
        assert!(!s.diagnostics.ground_effects && s.diagnostics.autopilot);
        assert_eq!(s.diagnostics.prop_script_speed, Some(12.5));
    }

    #[test]
    fn later_files_override_and_off_clears() {
        let dir = Path::new("d");
        let mut s = Settings::default();
        s.apply(RACE, dir).unwrap();
        s.apply(
            "tcs=on\n[game]\nroot = D:\\Games\\Grid\n[drive]\nlaps = 0\nopponents = 0\ngrid_position = 2\n[private]\nbody_contacts = derive\nbody_events = off\ndeformation = derive\n",
            dir,
        )
        .unwrap();
        assert_eq!(s.root, Some(PathBuf::from(r"D:\Games\Grid")));
        assert_eq!(
            (s.laps, s.opponents, s.grid_position),
            (None, None, Some(2))
        );
        assert!(!s.grid_back);
        assert_eq!(s.body_contacts, Some(Private::Derive));
        assert_eq!(s.body_events, None);
        assert_eq!(s.deformation, Some(Private::Derive));
        // Lines before any header are driver aids, as in the assists file.
        assert!(s.assists().unwrap().traction_control);
    }

    #[test]
    fn pause_menu_sound_and_screen_options() {
        let dir = Path::new("d");
        let mut s = Settings::default();
        assert_eq!((s.sound_effects, s.osd_mph), (None, None));
        s.apply(
            "[sound]\neffects = 40\n[osd]\nlap_timer = off\nposition = on\nspeed_units = mph\n",
            dir,
        )
        .unwrap();
        assert_eq!(s.sound_effects, Some(40));
        assert_eq!(
            (s.osd_lap_timer, s.osd_position, s.osd_mph),
            (Some(false), Some(true), Some(true))
        );
        let values = crate::pause_menu::initial_options(&s, None);
        assert_eq!(values.effects, 40);
        assert!(!values.lap_timer && values.position && values.mph);
        for text in ["[sound]\neffects = 101\n", "[osd]\nspeed_units = knots\n"] {
            assert!(Settings::default().apply(text, dir).is_err(), "{text}");
        }
    }

    #[test]
    fn bad_settings_are_refused_with_the_line() {
        let dir = Path::new("d");
        for (text, error) in [
            ("[video]\n", "line 1: unknown settings section [video]"),
            (
                "[drive]\nspeed = 3\n",
                "line 2: unknown setting `speed` in [drive]",
            ),
            (
                "[drive]\nlaps = many\n",
                "line 2: setting `laps` needs a number",
            ),
            ("[game]\nroot\n", "line 2: `root` needs key = value"),
            (
                "[diagnostics]\nautopilot = maybe\n",
                "line 2: setting `autopilot` needs on or off",
            ),
        ] {
            assert_eq!(Settings::default().apply(text, dir).unwrap_err(), error);
        }
        let mut s = Settings::default();
        s.apply("bogus=1\n", dir).unwrap();
        assert!(s.assists().is_err());
    }

    #[test]
    fn shipped_profiles_parse() {
        let profiles = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../settings");
        let mut n = 0;
        for entry in std::fs::read_dir(profiles).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "ini") {
                let mut s = Settings::default();
                s.apply(&std::fs::read_to_string(&path).unwrap(), Path::new("local"))
                    .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
                s.assists().unwrap();
                s.input().unwrap();
                assert!(s.drive, "{}", path.display());
                n += 1;
            }
        }
        assert!(n >= 4);
    }

    #[test]
    fn comments_need_whitespace_before_hash() {
        assert_eq!(strip_comment("a = b#c # d"), "a = b#c ");
        assert_eq!(strip_comment("# all"), "");
    }
}
