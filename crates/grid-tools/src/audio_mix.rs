//! Original group mix and software distance law (docs/AUDIO_RESEARCH.md).
//!
//! `audio/GroupModeVolumes.bin` holds the mixer table read by 41FAE0: a PC
//! master, a platform modifier, ten modes and six audio cameras, each a dB
//! value for 21 groups. 427ED0 publishes, per frame and group,
//! `mode + platform + camera + master + trim`. The OpenAL backend runs with
//! `alDistanceModel(AL_NONE)` and Doppler factor 0 (A2DE87); A23B70 adds a
//! software distance term to `voice + group` and silences anything at or below
//! the noise floor.
use grid_formats::binxml;
use std::path::Path;

pub const GROUPS: usize = 21;
/// Group indices: name order of the 41F570 table (Wheels 6, Skids 7, AI 17
/// match the tyre and engine constructors 413E50/4223D0).
pub const EXHAUST: usize = 0;
pub const ENGINE: usize = 1;
pub const GEARBOX: usize = 3;
pub const WHEELS: usize = 6;
pub const SKIDS: usize = 7;
pub const AI: usize = 17;
pub const GROUP_NAMES: [&str; GROUPS] = [
    "Exhaust",
    "Engine",
    "Turbo",
    "Gearbox",
    "Panel Coll",
    "World Coll",
    "Wheels",
    "Skids",
    "Displacement",
    "Other Mech",
    "Race Music",
    "Crowd",
    "Ambience",
    "Global Ambience",
    "Codriver",
    "3DSpeech",
    "HUD",
    "AI",
    "Music",
    "Speech Level",
    "Sense Of Speed",
];
/// Mode indices of the 41F570 table; 9 is the platform modifier row.
pub const MODE_NAMES: [&str; 10] = [
    "Normal",
    "Talking",
    "Crashed",
    "Paused",
    "StartLine",
    "Intro",
    "EndOfRace",
    "PausedReplay",
    "Transition",
    "PlatformModifier",
];
pub const NORMAL: usize = 0;
const PLATFORM: usize = 9;

/// Audio cameras of the 41F570 table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Camera {
    Internal = 0,
    Bonnet = 1,
    Chase = 2,
    Detached = 3,
    DetachedMultiCar = 4,
    FrontEnd = 5,
}
pub const CAMERA_NAMES: [&str; 6] = [
    "Internal",
    "Bonnet",
    "Chase",
    "Detached",
    "DetachedMultiCar",
    "FrontEndCamera",
];
impl Camera {
    /// 4139F0: the view's class type (+6C, from the 817D60 base constructor:
    /// Chase 0, Mounted 1, trackside 2, Head 3) picks the audio camera.
    pub fn of_view(kind: grid_formats::vehicle_views::Kind) -> Self {
        use grid_formats::vehicle_views::Kind;
        match kind {
            Kind::Chase => Self::Chase,
            Kind::Mounted => Self::Bonnet,
            Kind::Head => Self::Internal,
        }
    }
}

/// Backend noise floor and roll-off: the 43EC80 defaults (-48 dB, 0.5) that
/// A22480 copies to E9C128 and to E9C12C = 1/roll-off.
pub const NOISE_FLOOR: f32 = -48.;
const EXPONENT: f32 = 1. / 0.5;
/// A23B70 replaces a silenced voice level with -96.3 dB.
pub const SILENT: f32 = -96.3;

#[derive(Clone, Debug, PartialEq)]
pub struct Table {
    pub master: f32,
    /// [mode][group]; rows absent from the file stay at the loader default -48.
    pub modes: [[f32; GROUPS]; 10],
    pub attack: [f32; 10],
    /// [camera][group]; absent entries stay 0.
    pub cameras: [[f32; GROUPS]; 6],
}

fn attr<'a>(e: &binxml::Element<'a>, name: &str) -> Option<&'a str> {
    e.attributes
        .iter()
        .find(|(k, _)| *k == name)
        .map(|(_, v)| *v)
}
fn number(e: &binxml::Element, name: &str) -> Result<f32, String> {
    attr(e, name)
        .and_then(|v| v.trim().parse::<f32>().ok())
        .filter(|v| v.is_finite())
        .ok_or_else(|| format!("GroupModeVolumes: bad `{name}` on {}", e.name))
}

impl Table {
    pub fn load(root: &Path) -> Result<Self, String> {
        let path = root.join("audio/GroupModeVolumes.bin");
        let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::parse(&bytes)
    }
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let doc = binxml::read(bytes).map_err(|e| format!("GroupModeVolumes: {e:?}"))?;
        let e = &doc.elements;
        let children = |i: usize| {
            e[e[i].children.clone()].iter().enumerate().map({
                let start = e[i].children.start;
                move |(k, c)| (start + k, c)
            })
        };
        let named = |i: usize, name: &str| children(i).find(|(_, c)| c.name == name);
        let groups = |i: usize, row: &mut [f32; GROUPS]| -> Result<(), String> {
            for (_, g) in children(i).filter(|(_, c)| c.name == "Group") {
                let name = attr(g, "name").unwrap_or_default();
                // 41FAE0 resolves group names exactly; unknown names are ignored.
                if let Some(k) = GROUP_NAMES.iter().position(|n| *n == name) {
                    row[k] = number(g, "value")?;
                }
            }
            Ok(())
        };
        let root = e
            .iter()
            .position(|x| x.parent.is_none() && x.name == "GroupModeVolumes")
            .ok_or("GroupModeVolumes: root absent")?;
        let mut table = Table {
            master: 0.,
            modes: [[NOISE_FLOOR; GROUPS]; 10],
            attack: [0.; 10],
            cameras: [[0.; GROUPS]; 6],
        };
        let (platforms, _) = named(root, "Platforms").ok_or("Platforms absent")?;
        let (pc, _) = named(platforms, "PC").ok_or("PC platform absent")?;
        table.master = number(named(pc, "Master").ok_or("Master absent")?.1, "volume")?;
        let (modifier, _) = named(pc, "Mode").ok_or("platform Mode absent")?;
        groups(modifier, &mut table.modes[PLATFORM])?;
        let (modes, _) = named(root, "Modes").ok_or("Modes absent")?;
        for (i, m) in children(modes).filter(|(_, c)| c.name == "Mode") {
            let name = attr(m, "name").unwrap_or_default();
            let Some(k) = MODE_NAMES[..PLATFORM].iter().position(|n| *n == name) else {
                continue;
            };
            table.attack[k] = number(m, "attackTime")?;
            groups(i, &mut table.modes[k])?;
        }
        let (cameras, _) = named(root, "Cameras").ok_or("Cameras absent")?;
        for (i, c) in children(cameras).filter(|(_, c)| c.name == "Camera") {
            let name = attr(c, "name").unwrap_or_default();
            if let Some(k) = CAMERA_NAMES.iter().position(|n| *n == name) {
                groups(i, &mut table.cameras[k])?;
            }
        }
        Ok(table)
    }
    /// 427ED0 steady state: mode + platform modifier + camera + master + trim.
    pub fn group_db(&self, mode: usize, camera: Camera, group: usize, trim: f32) -> f32 {
        self.modes[mode][group]
            + self.modes[PLATFORM][group]
            + self.cameras[camera as usize][group]
            + self.master
            + trim
    }
}

/// 4124A0: a volume slider in [0, 1] as a group trim in dB.
pub fn slider_db(v: f32) -> f32 {
    if v <= 0. {
        NOISE_FLOOR
    } else if v >= 1. {
        0.
    } else if v < 0.1 {
        NOISE_FLOOR
    } else {
        -((1. - v) * 24.)
    }
}

/// Which slider (411DE0 kind) trims a group: 0 Effects, 2 Speech, 3 Music.
pub fn slider_of(group: usize) -> u32 {
    match group {
        14 | 15 | 19 => 2,
        10 | 18 => 3,
        _ => 0,
    }
}

/// A23B70 distance term in dB for an emitter with the variety preset's
/// min/max distance; `None` beyond the maximum (silenced).
pub fn distance_db(distance: f32, min: f32, max: f32) -> Option<f32> {
    if distance >= max {
        return None;
    }
    if distance <= min {
        return Some(0.);
    }
    Some(NOISE_FLOOR / (max - min).powf(EXPONENT) * (distance - min).powf(EXPONENT))
}

/// A23B70 final level: voice + group + distance, silenced at or below the
/// floor. Returns the linear gain OpenAL receives (10^(dB/20), A2C990).
pub fn backend_gain(voice_db: f32, group_db: f32, distance_db: Option<f32>) -> f32 {
    let Some(distance) = distance_db else {
        return 0.;
    };
    let db = voice_db + group_db + distance;
    if db <= NOISE_FLOOR || !db.is_finite() {
        0.
    } else {
        10f32.powf(db / 20.)
    }
}

/// Stereo gains of a 3D voice from its position in listener space (x right,
/// y up, z forward): the game hands OpenAL listener-space positions (A23DC0)
/// and leaves the panning to the device. "Generic Software"'s pan law is
/// implementation-defined, so this is an equal-power law on the horizontal
/// azimuth, p = sin(azimuth), front and back alike [I]. Scaled by √2 so a
/// centred voice keeps unit gain in both channels, as the mono sources had.
pub fn pan_gains(listener: [f32; 3]) -> [f32; 2] {
    let horizontal = listener[0].hypot(listener[2]);
    let p = if horizontal > 1e-3 && horizontal.is_finite() {
        (listener[0] / horizontal).clamp(-1., 1.)
    } else {
        0.
    };
    [(1. - p).sqrt(), (1. + p).sqrt()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pan_is_equal_power_on_the_azimuth() {
        assert_eq!(pan_gains([0., 0., 5.]), [1., 1.]);
        assert_eq!(pan_gains([0., 0., -5.]), [1., 1.]);
        assert_eq!(pan_gains([0., 3., 0.]), [1., 1.]);
        let right = pan_gains([4., 0., 0.]);
        assert_eq!(right, [0., 2f32.sqrt()]);
        assert_eq!(pan_gains([-4., 1., 0.]), [2f32.sqrt(), 0.]);
        let half = pan_gains([1., 0., 3f32.sqrt()]);
        // 30 degrees right: p = 0.5; the power sums to 2 like the centre.
        assert!((half[0] * half[0] + half[1] * half[1] - 2.).abs() < 1e-6);
        assert!((half[1] * half[1] - 1.5).abs() < 1e-6);
    }

    #[test]
    fn slider_distance_and_floor() {
        assert_eq!(slider_db(1.), 0.);
        assert_eq!(slider_db(0.5), -12.);
        assert_eq!(slider_db(0.05), NOISE_FLOOR);
        assert_eq!(slider_db(0.), NOISE_FLOOR);
        assert_eq!(distance_db(5., 10., 175.), Some(0.));
        assert_eq!(distance_db(175., 10., 175.), None);
        let half = distance_db(92.5, 10., 175.).unwrap();
        assert!((half + 12.).abs() < 1e-4, "{half}");
        assert_eq!(backend_gain(-40., -9., Some(0.)), 0.);
        assert!((backend_gain(-6., 0., Some(0.)) - 0.501_187).abs() < 1e-5);
        assert_eq!(backend_gain(0., 0., None), 0.);
        assert_eq!((slider_of(18), slider_of(19), slider_of(6)), (3, 2, 0));
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn purchased_table_reproduces_the_chase_engine_offsets() {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let t = Table::load(Path::new(&root)).unwrap();
        assert_eq!(t.master, -3.);
        assert_eq!(t.attack[NORMAL], 0.15);
        // F275 observed Chase bus offsets -14 engine / -7 exhaust.
        assert_eq!(t.group_db(NORMAL, Camera::Chase, ENGINE, 0.), -14.);
        assert_eq!(t.group_db(NORMAL, Camera::Chase, EXHAUST, 0.), -7.);
        assert_eq!(t.group_db(NORMAL, Camera::Chase, WHEELS, 0.), -9.);
        assert_eq!(t.group_db(NORMAL, Camera::Chase, SKIDS, 0.), -8.);
        assert_eq!(t.group_db(NORMAL, Camera::Chase, AI, 0.), -9.);
        assert_eq!(t.group_db(NORMAL, Camera::Bonnet, ENGINE, 0.), -8.);
        assert_eq!(t.group_db(NORMAL, Camera::Internal, EXHAUST, 0.), -10.);
        assert_eq!(t.modes[3][ENGINE], -48.);
        assert_eq!(t.modes[4][AI], -3.);
    }
}
