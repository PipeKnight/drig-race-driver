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
    /// Seconds: in that Instant Replay, Jump In (Flashback) once it has run
    /// this long; the drive goes on from there (docs/FLASHBACK.md).
    pub flashback_capture: Option<f64>,
    /// Seconds: in a scripted run, the stats and results screens select
    /// Continue by themselves after this long (unattended career checks,
    /// docs/CAREER_SCREEN.md).
    pub results_continue: Option<f64>,
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
            flashback_capture: None,
            results_continue: None,
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
            flashback_capture: text("GRID_FLASHBACK_CAPTURE").and_then(|v| v.parse().ok()),
            results_continue: text("GRID_RESULTS_CONTINUE").and_then(|v| v.parse().ok()),
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
            "flashback_capture" => {
                self.flashback_capture = Some(value.parse().map_err(|_| number(key))?)
            }
            "results_continue" => {
                self.results_continue = Some(value.parse().map_err(|_| number(key))?)
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
    /// `[game] subroute`: a point-to-point route's direction, 0 (the
    /// `forwards` progress route) or 1 (`reverse`; docs/TOUGE.md).
    pub subroute: Option<u8>,
    /// `[game] car` / `opponent_cars` (docs/CAR_GENERALIZATION.md).
    pub car: Option<String>,
    pub opponent_cars: Option<String>,
    /// `[game] opponent_liveries`: each opponent's livery package index
    /// (`livery_NN`), in car order (the career field, docs/CAREER_SCREEN.md).
    pub opponent_liveries: Option<Vec<usize>>,
    /// `[game] opponent_names`: each opponent's name on the HUD and the
    /// results, comma separated, in car order (the career field).
    pub opponent_names: Option<Vec<String>>,
    /// `[game] points`: the event's points table by finishing position
    /// (career races), shown in the results' Points column.
    pub points: Option<Vec<i32>>,
    /// `[game] series`: a free-race circuit series (`pro_tuned`, `touring`,
    /// `gt1`, ...; docs/SERIES.md): its cars and grid fill.
    pub series: Option<String>,
    pub drive: bool,
    pub laps: Option<u32>,
    /// `[drive] time_limit`: seconds of a Freestyle drift event (90 in the
    /// career [D]) or of the Le Mans 24 Hours (`series = le_mans`, 720 in the
    /// career [D], docs/LE_MANS.md).
    pub time_limit: Option<f64>,
    pub opponents: Option<usize>,
    pub grid_position: Option<usize>,
    /// `grid_position = back`: the last slot, after the opponents.
    pub grid_back: bool,
    pub ai_characteristics: Option<PathBuf>,
    /// `[drive] discipline`: `race` (default), `drift_battle`, `drift_grand_prix`
    /// or `drift_freestyle`
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
    /// `[osd] fps`: the frame-rate counter (default on Android only).
    pub osd_fps: Option<bool>,
    /// `[video] render_height`: `native` or rows (render_scale.rs).
    pub render_height: Option<crate::render_scale::RenderHeight>,
    /// `[video] present_mode`, `refresh_hz` (`Some(None)` = system), `frame_latency`.
    pub present_mode: Option<bevy::window::PresentMode>,
    pub refresh_hz: Option<Option<f32>>,
    pub frame_latency: Option<std::num::NonZero<u32>>,
    /// `[front_end] show`: the Race Options front end before the race
    /// (docs/DRIVER_AIDS.md); default on.
    pub front_end: Option<bool>,
    /// `[front_end] event_menu`: Android starts in the event menu
    /// (event_menu.rs); default on. The desktop opens it with `--menu`.
    pub event_menu: Option<bool>,
    /// `[event]`: the event menu's last choice (`type`, `route`, `car`,
    /// `laps`, `opponents`, `time`, `direction`), read only by the menu.
    pub event: Vec<(String, String)>,
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

/// `touch = auto` -> `touch=auto`; lists such as `tcs=on, abs=off` stay.
fn joined_item(line: &str) -> String {
    line.split('=').map(str::trim).collect::<Vec<_>>().join("=")
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
                    "front_end",
                    "event",
                    "video",
                    "diagnostics",
                ]
                .contains(&section.as_str())
                {
                    return Err(at(format!("unknown settings section [{name}]")));
                }
                continue;
            }
            let list = match section.as_str() {
                "assists" => Some(&mut self.assists),
                "input" => Some(&mut self.input),
                "ai" => Some(&mut self.ai),
                _ => None,
            };
            if let Some(list) = list {
                // These sections take `key=value` items split at spaces, so
                // the file's `key = value` form is joined first.
                list.push_str(&joined_item(line));
                list.push('\n');
                continue;
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
                ("game", "subroute") => {
                    self.subroute = Some(
                        value
                            .parse()
                            .ok()
                            .filter(|s| *s <= 1)
                            .ok_or("[game] subroute must be 0 or 1")?,
                    )
                }
                ("game", "car") => self.car = Some(value.into()),
                ("game", "opponent_cars") => self.opponent_cars = Some(value.into()),
                ("game", "opponent_liveries") => {
                    self.opponent_liveries = Some(
                        value
                            .split(',')
                            .map(|v| v.trim().parse::<usize>())
                            .collect::<Result<_, _>>()
                            .map_err(|_| at("[game] opponent_liveries needs numbers".into()))?,
                    )
                }
                ("game", "points") => {
                    self.points = Some(
                        value
                            .split(',')
                            .map(|v| v.trim().parse::<i32>())
                            .collect::<Result<_, _>>()
                            .map_err(|_| at("[game] points needs numbers".into()))?,
                    )
                }
                ("game", "opponent_names") => {
                    self.opponent_names = Some(value.split(',').map(|v| v.trim().to_owned()).collect())
                }
                ("game", "series") => {
                    let name = value.to_ascii_lowercase();
                    if !grid_tools::series::CIRCUIT_SERIES.contains(&name.as_str()) {
                        return Err(at(format!(
                            "`series` needs one of {}",
                            grid_tools::series::CIRCUIT_SERIES.join(", ")
                        )));
                    }
                    self.series = Some(name);
                }
                ("drive", "enabled") => self.drive = on(&key, value).map_err(at)?,
                ("drive", "time_limit") => {
                    let seconds = value.parse::<f64>().map_err(|_| at(number(&key)))?;
                    self.time_limit = (seconds > 0.).then_some(seconds);
                }
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
                    d @ ("race" | "drift_battle" | "drift_grand_prix" | "drift_freestyle"
                    | "pro_touge" | "demolition_derby") => {
                        self.discipline = Some(d.into())
                    }
                    _ => {
                        return Err(at(
                            "`discipline` needs race, drift_battle, drift_grand_prix, drift_freestyle, pro_touge or demolition_derby"
                                .into(),
                        ));
                    }
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
                ("osd", "fps") => self.osd_fps = Some(on(&key, value).map_err(at)?),
                ("video", "present_mode") => {
                    self.present_mode = Some(crate::render_scale::present_mode(value).map_err(at)?)
                }
                ("video", "refresh_hz") => {
                    self.refresh_hz = Some(crate::render_scale::refresh_hz(value).map_err(at)?)
                }
                ("video", "frame_latency") => {
                    self.frame_latency = Some(crate::render_scale::frame_latency(value).map_err(at)?)
                }
                ("video", "render_height") => {
                    self.render_height = Some(crate::render_scale::RenderHeight::parse(value).map_err(at)?)
                }
                ("osd", "speed_units") => {
                    self.osd_mph = Some(match value.to_ascii_lowercase().as_str() {
                        "kph" | "km/h" => false,
                        "mph" => true,
                        _ => return Err(at("`speed_units` needs kph or mph".into())),
                    })
                }
                ("front_end", "show") => self.front_end = Some(on(&key, value).map_err(at)?),
                ("front_end", "event_menu") => {
                    self.event_menu = Some(on(&key, value).map_err(at)?)
                }
                (
                    "event",
                    "type" | "route" | "car" | "laps" | "opponents" | "time" | "direction",
                ) => {
                    self.event.retain(|(k, _)| *k != key);
                    self.event.push((key.to_string(), value.to_string()));
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

/// `[assists]` keys the Driver Aids screen owns.
const AID_KEYS: [&str; 14] = [
    "tcs",
    "traction",
    "traction_control",
    "stability",
    "stability_control",
    "scs",
    "esp",
    "braking",
    "braking_assist",
    "brake",
    "abs",
    "gearbox",
    "transmission",
    "gears",
];

/// `text` (a settings file) with the Driver Aids choice saved: earlier
/// traction, stability, braking and gearbox items are removed from the
/// `[assists]` lines and one line with the new values ends the section (later
/// keys win). Everything else, comments included, is kept.
pub fn with_driver_aids(text: &str, aids: &grid_tools::source_vehicle::assists::Assists) -> String {
    use grid_tools::source_vehicle::assists::Transmission;
    let line = format!(
        "tcs={} stability={} braking={} gearbox={}",
        aids.traction_control,
        if aids.stability_control { "on" } else { "off" },
        aids.braking_assist,
        match aids.transmission {
            Transmission::Automatic => "automatic",
            Transmission::Manual => "manual",
        }
    );
    with_items(text, "assists", &AID_KEYS, line)
}

/// `text` with the items of an item-grammar section (`[assists]`,
/// `[input]`: `key=value` items separated by spaces or commas) replaced:
/// every `owned` item is removed from that section's lines and `line` ends
/// the section (later keys win). Everything else, comments included, is
/// kept.
pub fn with_items(text: &str, section: &str, owned: &[&str], line: String) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut current = String::from("assists");
    // The new line goes after the section's last line.
    let mut end = None;
    for raw in text.lines() {
        let body = strip_comment(raw);
        let trimmed = body.trim();
        if let Some(name) = trimmed.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            current = name.trim().to_ascii_lowercase();
            out.push(raw.to_owned());
            if current == section {
                end = Some(out.len());
            }
            continue;
        }
        if current != section {
            out.push(raw.to_owned());
            continue;
        }
        // `key = value` counts as one item, as the reader joins it (F370).
        let joined = joined_item(trimmed);
        let items: Vec<&str> = joined
            .split([',', ' ', '\t', ';'])
            .filter(|i| !i.is_empty())
            .collect();
        let kept: Vec<&str> = items
            .iter()
            .copied()
            .filter(|i| {
                let key = i.split_once('=').map_or(*i, |(k, _)| k).to_ascii_lowercase();
                !owned.contains(&key.as_str())
            })
            .collect();
        if kept.len() == items.len() {
            out.push(raw.to_owned());
        } else if !kept.is_empty() {
            // A kept comment needs whitespace before its `#`.
            let comment = raw[body.len()..].trim_start();
            out.push(if comment.is_empty() {
                kept.join(" ")
            } else {
                format!("{} {comment}", kept.join(" "))
            });
        } else if body.len() < raw.len() {
            // Only its comment is left.
            out.push(raw[body.len()..].trim_start().to_owned());
        } else {
            continue;
        }
        if !raw.trim().is_empty() {
            end = Some(out.len());
        }
    }
    match end {
        Some(at) => out.insert(at, line),
        None => {
            out.push(format!("[{section}]"));
            out.push(line);
        }
    }
    let mut text = out.join("\n");
    text.push('\n');
    text
}

/// `text` with `key = value` lines of one line-grammar section (`[osd]`,
/// `[video]`) set: an existing key line is rewritten in place (its comment
/// kept; later duplicates dropped), a `None` value removes the key, and
/// missing keys go after the section's last line, or into a new section at
/// the end. Everything else, comments included, is kept.
pub fn with_keys(text: &str, section: &str, keys: &[(&str, Option<String>)]) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut current = String::from("assists");
    let mut end = None;
    let mut written = vec![false; keys.len()];
    for raw in text.lines() {
        let body = strip_comment(raw);
        let trimmed = body.trim();
        if let Some(name) = trimmed.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            current = name.trim().to_ascii_lowercase();
            out.push(raw.to_owned());
            if current == section {
                end = Some(out.len());
            }
            continue;
        }
        if current == section {
            let key = trimmed
                .split_once('=')
                .map(|(k, _)| k.trim().to_ascii_lowercase());
            if let Some(i) = key.and_then(|k| keys.iter().position(|(name, _)| *name == k)) {
                if let (Some(value), false) = (&keys[i].1, written[i]) {
                    out.push(format!("{} = {value}{}", keys[i].0, &raw[body.trim_end().len()..]));
                    written[i] = true;
                    end = Some(out.len());
                }
                continue;
            }
            out.push(raw.to_owned());
            if !raw.trim().is_empty() {
                end = Some(out.len());
            }
            continue;
        }
        out.push(raw.to_owned());
    }
    let missing: Vec<String> = keys
        .iter()
        .zip(&written)
        .filter_map(|((name, value), done)| match (value, done) {
            (Some(v), false) => Some(format!("{name} = {v}")),
            _ => None,
        })
        .collect();
    if !missing.is_empty() {
        match end {
            Some(at) => {
                for (i, line) in missing.into_iter().enumerate() {
                    out.insert(at + i, line);
                }
            }
            None => {
                out.push(format!("[{section}]"));
                out.extend(missing);
            }
        }
    }
    let mut text = out.join("\n");
    text.push('\n');
    text
}

/// The Video screen's choice (front_end.rs) as settings keys: `[osd] fps`
/// and `[video] render_height / present_mode / refresh_hz / frame_latency`.
pub fn save_video(path: &Path, video: &crate::front_end::Video) -> Result<(), String> {
    let old = match std::fs::read_to_string(path) {
        Ok(text) => text.trim_start_matches('\u{feff}').to_owned(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let new = with_keys(&old, "osd", &[("fps", Some(if video.fps { "on" } else { "off" }.into()))]);
    let new = with_keys(&new, "video", &video.keys());
    let mut check = Settings::new();
    check.apply(&new, path.parent().unwrap_or(Path::new(".")))?;
    if crate::front_end::Video::from_settings(&check) != *video {
        return Err("saved video settings do not read back".into());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, new).map_err(|e| format!("{}: {e}", path.display()))
}

/// Saves the Driver Aids choice in the settings file `path`
/// (`<data dir>/settings.ini`).
pub fn save_driver_aids(
    path: &Path,
    aids: &grid_tools::source_vehicle::assists::Assists,
) -> Result<(), String> {
    let old = match std::fs::read_to_string(path) {
        Ok(text) => text.trim_start_matches('\u{feff}').to_owned(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let new = with_driver_aids(&old, aids);
    // The result must still be a valid settings file with these aids.
    let mut check = Settings::new();
    check.apply(&new, path.parent().unwrap_or(Path::new(".")))?;
    let back = check.assists()?;
    let owned = |a: &grid_tools::source_vehicle::assists::Assists| {
        (a.traction_control, a.stability_control, a.braking_assist, a.transmission)
    };
    if owned(&back) != owned(aids) {
        return Err("saved driver aids do not read back".into());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, new).map_err(|e| format!("{}: {e}", path.display()))
}

/// `[input]` keys the pause menu's Advanced screen owns.
const TUNE_KEYS: [&str; 6] = [
    "steering_deadzone",
    "steering_saturation",
    "accelerate_deadzone",
    "accelerate_saturation",
    "brake_deadzone",
    "brake_saturation",
];

/// Saves the pause menu's options in the settings file `path`
/// (`<data dir>/settings.ini`), as the original autosaves its profile when
/// a settings screen is left (`*_st_*_autosave`, docs/PAUSE_MENU.md):
/// `[sound] effects`, `[osd] lap_timer / position / minimap / dials /
/// speed_units` and, with `tunes`, the `[input]` dead zones and saturations.
pub fn save_options(path: &Path, o: &crate::pause_menu::Options, tunes: bool) -> Result<(), String> {
    let old = match std::fs::read_to_string(path) {
        Ok(text) => text.trim_start_matches('\u{feff}').to_owned(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let on = |b: bool| Some(if b { "on" } else { "off" }.to_owned());
    let mut new = with_keys(&old, "sound", &[("effects", Some(o.effects.to_string()))]);
    new = with_keys(
        &new,
        "osd",
        &[
            ("lap_timer", on(o.lap_timer)),
            ("position", on(o.position)),
            ("minimap", on(o.minimap)),
            ("dials", on(o.dials)),
            ("speed_units", Some(if o.mph { "mph" } else { "kph" }.into())),
        ],
    );
    if tunes {
        let items: Vec<String> = TUNE_KEYS
            .iter()
            .enumerate()
            .map(|(i, k)| format!("{k}={}", f32::from(o.tunes[i / 2][i % 2]) / 100.))
            .collect();
        new = with_items(&new, "input", &TUNE_KEYS, items.join(" "));
    }
    // The result must still be a valid settings file with these options.
    let mut check = Settings::new();
    check.apply(&new, path.parent().unwrap_or(Path::new(".")))?;
    let input = check.input()?;
    let mut back = crate::pause_menu::initial_options(&check, None);
    if tunes {
        let percent = |x: Option<f32>| x.map_or(255, |x| (x * 100.).round() as u8);
        back.tunes = [
            [percent(input.steering.dead_zone), percent(input.steering.saturation)],
            [percent(input.accelerate.dead_zone), percent(input.accelerate.saturation)],
            [percent(input.brake.dead_zone), percent(input.brake.saturation)],
        ];
    } else {
        back.tunes = o.tunes;
    }
    if back != *o {
        return Err("saved options do not read back".into());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, new).map_err(|e| format!("{}: {e}", path.display()))
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
    // The bundled APK's event profile (track, discipline, field; replaced on
    // every install, docs/ANDROID.md), read first so the owner's settings.ini
    // (kept across installs, written by the in-app screens) overrides it.
    let event = dir.join("event.ini");
    if event.is_file() {
        files.push(event);
    }
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
        assert!(!aids.traction_control.is_on());
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
        assert!(s.assists().unwrap().traction_control.is_on());
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
            ("[audio]\n", "line 1: unknown settings section [audio]"),
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
    fn phone_profile_written_by_the_android_scripts_parses() {
        // The settings.ini that push_android_data.ps1 writes, the bundled APK
        // ships and the code-only APK's import writes (one template, P10)
        // uses `key = value` in [input] too.
        let text = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../android/app/src/main/assets/grid-import/settings.ini"),
        )
        .unwrap();
        let text = text.as_str();
        let mut s = Settings::default();
        s.apply(text, Path::new("files")).unwrap();
        let input = s.input().unwrap();
        assert_eq!(input.touch, None);
        assert!(input.touch_active(true));
        assert_eq!(input.tilt_settings().unwrap().max_angle, 30.);
        s.assists().unwrap();
        s.apply(
            "[assists]\ntcs = off, gearbox = manual\n[ai]\nmistakes = off\n",
            Path::new("d"),
        )
        .unwrap();
        assert!(!s.assists().unwrap().traction_control.is_on());
        s.ai().unwrap();
    }

    #[test]
    fn video_keys_are_set_in_place_and_read_back() {
        use crate::render_scale::RenderHeight;
        use bevy::window::PresentMode;
        let old = "# phone\n[osd]\nminimap = off # mine\nfps = on # counter\n[video]\nrender_height = 900 # cap\nframe_latency = 1\n[assists]\ntcs=on\n";
        let new = with_keys(old, "osd", &[("fps", Some("off".into()))]);
        let video = crate::front_end::Video {
            fps: false,
            render_height: RenderHeight::Native,
            present_mode: PresentMode::AutoVsync,
            refresh_hz: Some(120.),
            frame_latency: None,
        };
        let new = with_keys(&new, "video", &video.keys());
        assert_eq!(
            new,
            "# phone\n[osd]\nminimap = off # mine\nfps = off # counter\n[video]\nrender_height = native # cap\npresent_mode = auto_vsync\nrefresh_hz = 120\n[assists]\ntcs=on\n"
        );
        let mut s = Settings::new();
        s.apply(&new, Path::new(".")).unwrap();
        assert_eq!(crate::front_end::Video::from_settings(&s), video);
        // A file without the sections gets them at the end.
        let new = with_keys("[drive]\nenabled = on\n", "video", &[("refresh_hz", Some("system".into()))]);
        assert_eq!(new, "[drive]\nenabled = on\n[video]\nrefresh_hz = system\n");
        // save_video writes and reads back.
        let dir = std::env::temp_dir().join(format!("grid-video-save-{}", std::process::id()));
        let file = dir.join("settings.ini");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&file, old).unwrap();
        save_video(&file, &video).unwrap();
        let mut s = Settings::new();
        s.apply(&std::fs::read_to_string(&file).unwrap(), &dir).unwrap();
        assert_eq!(crate::front_end::Video::from_settings(&s), video);
        assert_eq!(s.osd_minimap, Some(false));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pause_options_are_saved_and_read_back() {
        let mut o = crate::pause_menu::Options {
            effects: 40,
            lap_timer: false,
            mph: true,
            ..Default::default()
        };
        o.tunes = [[10, 95], [5, 100], [0, 85]];
        let dir = std::env::temp_dir().join(format!("grid-options-save-{}", std::process::id()));
        let file = dir.join("settings.ini");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let old = "# mine\n[input]\ndevice=wheel steering_deadzone=0.3 # wheel\n[osd]\nfps = on\n";
        std::fs::write(&file, old).unwrap();
        // Sound and Screen Setup: the input tunes stay as they were.
        save_options(&file, &o, false).unwrap();
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.contains("steering_deadzone=0.3"), "{text}");
        let read = |text: &str| {
            let mut s = Settings::new();
            s.apply(text, &dir).unwrap();
            s
        };
        let s = read(&text);
        assert_eq!(crate::pause_menu::initial_options(&s, None).effects, 40);
        assert_eq!((s.osd_lap_timer, s.osd_mph, s.osd_fps), (Some(false), Some(true), Some(true)));
        // Advanced: the tunes replace the old items, the device stays.
        save_options(&file, &o, true).unwrap();
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.starts_with("# mine\n[input]\ndevice=wheel # wheel\nsteering_deadzone=0.1 "), "{text}");
        let input = read(&text).input().unwrap();
        assert_eq!(input.device, crate::input::Device::Wheel);
        assert_eq!((input.steering.dead_zone, input.brake.saturation), (Some(0.1), Some(0.85)));
        // Saving again changes nothing.
        save_options(&file, &o, true).unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), text);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn driver_aids_are_saved_in_the_assists_section() {
        use grid_tools::source_vehicle::assists::{Assists, Level, Transmission};
        let aids = Assists {
            traction_control: Level::new(25).unwrap(),
            stability_control: false,
            braking_assist: Level::ON,
            transmission: Transmission::Manual,
            ..Assists::default()
        };
        let read = |text: &str| {
            let mut s = Settings::new();
            s.apply(text, Path::new("local")).unwrap();
            s
        };
        // No file yet: a new section.
        let text = with_driver_aids("", &aids);
        assert_eq!(text, "[assists]\ntcs=25% stability=off braking=on gearbox=manual\n");
        assert_eq!(read(&text).assists().unwrap(), aids);
        // The aid keys go, other keys, comments and sections stay; the
        // driver aids end the [assists] section.
        let old = "# mine\ntcs=off input=wheel\n[game]\nroot = auto\n[assists]\nabs=off # old\ndamage=normal\n[osd]\nminimap = off\n";
        let text = with_driver_aids(old, &aids);
        assert_eq!(
            text,
            "# mine\ninput=wheel\n[game]\nroot = auto\n[assists]\n# old\ndamage=normal\ntcs=25% stability=off braking=on gearbox=manual\n[osd]\nminimap = off\n"
        );
        let s = read(&text);
        let back = s.assists().unwrap();
        assert_eq!(back.traction_control, aids.traction_control);
        assert!(!back.stability_control);
        assert_eq!(back.transmission, Transmission::Manual);
        assert_eq!(back.input, grid_tools::source_vehicle::assists::Input::Wheel);
        assert_eq!(s.osd_minimap, Some(false));
        // The `key = value` form is understood too.
        assert_eq!(
            with_driver_aids("[assists]\ntcs = off, damage = normal\n", &aids),
            "[assists]\ndamage=normal\ntcs=25% stability=off braking=on gearbox=manual\n"
        );
        // Saving again changes nothing.
        assert_eq!(with_driver_aids(&text, &aids), text);
        assert_eq!(read("[front_end]\nshow = off\n").front_end, Some(false));
    }

    #[test]
    fn comments_need_whitespace_before_hash() {
        assert_eq!(strip_comment("a = b#c # d"), "a = b#c ");
        assert_eq!(strip_comment("# all"), "");
    }
}
