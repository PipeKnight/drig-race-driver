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
pub const TURBO: usize = 2;
pub const GEARBOX: usize = 3;
pub const WHEELS: usize = 6;
pub const SKIDS: usize = 7;
pub const OTHER_MECH: usize = 9;
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
pub const TALKING: usize = 1;
pub const CRASHED: usize = 2;
pub const PAUSED: usize = 3;
pub const START_LINE: usize = 4;
pub const INTRO: usize = 5;
pub const END_OF_RACE: usize = 6;
pub const PAUSED_REPLAY: usize = 7;
pub const TRANSITION: usize = 8;
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

/// The mix mode of 411C10 and its per-frame fade 427ED0
/// (docs/AUDIO_RESEARCH.md §13). The race flow (`system/flow.bin`
/// `StateAudioSetState`, 5791A0/579210) sets persistent modes: StartLine
/// before the pan to the car and through the start sequence, Normal with
/// `start_race` at the green light, StartLine again on a restart.
///
/// 427ED0, per frame and group: the target is
/// `mode + platform + camera + master + trim`. While the mode it left (+BF4)
/// differs from the current one (+BF0), a group not yet at its target is set
/// to `lerp(target of the old mode, target, e / (attack[new] + extra))`, e the
/// time since the change (+BF8 timer). The fade ends (+BF4 = +BF0, extra 0)
/// once at least 20 of the 21 groups equal their target. After that a group
/// still off target moves from its current level: `lerp(current, target,
/// e / 1.5)` each frame. StartLine differs from Normal only in AI (-3 dB),
/// so the fade ends on its first frame and AI follows that second law, both
/// into StartLine and back to Normal.
///
/// The state holds each group's mode term only (published level minus
/// platform, camera, master and trim), which evolves the same while those
/// stay constant [I]. The original steps once per game frame; here the step
/// is a fixed 60 Hz so the fade does not depend on the frame rate [I].
///
/// A mode set with a duration (Crashed, 0.2 s from 4535A0) is transient: the
/// mode it interrupts is remembered (+C24) and 427ED0 sets it again once the
/// timer passes the hold (+C18), the duration minus that mode's attack.
#[derive(Clone, Debug, PartialEq)]
pub struct ModeMix {
    mode: usize,
    from: usize,
    extra: f32,
    elapsed: f32,
    pending: f32,
    levels: Option<[f32; GROUPS]>,
    /// +C21: the current mode was set without a duration.
    persistent: bool,
    /// +C24: the mode a transient one returns to [I: Normal at start].
    back: usize,
    /// +C18: how long a transient mode holds.
    hold: f32,
    /// +C28: the mode is tied to a playing voice (0x411D40) instead of a duration.
    tied: bool,
}
impl Default for ModeMix {
    fn default() -> Self {
        Self {
            mode: NORMAL,
            from: NORMAL,
            extra: 0.,
            elapsed: 0.,
            pending: 0.,
            levels: None,
            persistent: true,
            back: NORMAL,
            hold: 0.,
            tied: false,
        }
    }
}
/// 4535A0's Crashed duration (CB0830).
pub const CRASH_SECONDS: f32 = 0.2;
/// 427ED0's step once the mode fade has ended (CB0BC4).
const SETTLE: f32 = 1.5;
/// Fixed step of the fade [I: the original runs it per frame].
pub const MODE_STEP: f32 = 1. / 60.;
impl ModeMix {
    pub fn mode(&self) -> usize {
        self.mode
    }
    /// 411C10 with a zero duration (a persistent mode).
    pub fn set(&mut self, mode: usize) {
        self.change(mode, 0., 0.);
    }
    /// 411C10 with `seconds` > 0 (a transient mode, Crashed from 4535A0).
    /// Refused while paused, in a pause replay or a transition, or when the
    /// mode is already current; returns whether it was set.
    pub fn set_for(&mut self, table: &Table, mode: usize, seconds: f32) -> bool {
        if matches!(self.mode, PAUSED | PAUSED_REPLAY | TRANSITION) {
            return false;
        }
        let attack = table.attack.get(self.back).copied().unwrap_or(0.);
        self.change(mode, seconds, attack)
    }
    /// 0x411D40: a transient `mode` that lasts while a voice plays (Talking for a speech line).
    /// Refused while paused, in a pause replay or a transition, or when the mode is already
    /// current. The mode it interrupts is remembered as for a timed one; [`ModeMix::release`]
    /// returns to it when the voice stops.
    pub fn set_tied(&mut self, mode: usize) -> bool {
        if matches!(self.mode, PAUSED | PAUSED_REPLAY | TRANSITION) || mode == self.mode || mode >= PLATFORM {
            return false;
        }
        // +C18 = 0 and +C21 = 0 (a voice pointer was given).
        self.change(mode, 0., 0.);
        self.persistent = false;
        self.tied = true;
        true
    }
    /// 427ED0's voice check: once the tied voice has stopped, the remembered mode is set
    /// again. Returns whether it was.
    pub fn release(&mut self) -> bool {
        if self.persistent || !self.tied {
            return false;
        }
        self.set(self.back);
        true
    }
    /// The common part of 411C10. The extra fade time of CB092C (2.5 s,
    /// Transition to Paused) and CB0BC4 (1.5 s, EndOfRace to StartLine)
    /// depends on the mode the previous fade left.
    fn change(&mut self, mode: usize, seconds: f32, attack: f32) -> bool {
        if mode == self.mode || mode >= PLATFORM {
            return false;
        }
        if self.from == TRANSITION && mode == PAUSED {
            self.extra = 2.5;
        } else if self.from == END_OF_RACE && mode == START_LINE {
            self.extra = SETTLE;
        }
        self.hold = if seconds > attack { seconds - attack } else { seconds };
        if self.persistent && !matches!(self.mode, PAUSED | END_OF_RACE | PAUSED_REPLAY | TRANSITION) {
            self.back = self.mode;
        }
        self.persistent = seconds == 0.;
        // A mode set with a duration or none drops the voice tie (+C28 = the new caller's
        // argument) [I: 411C10 writes +C28 = 0].
        self.tied = false;
        self.from = self.mode;
        self.mode = mode;
        self.elapsed = 0.;
        true
    }
    /// Advance the fade by `seconds`.
    pub fn advance(&mut self, table: &Table, seconds: f32) {
        if !seconds.is_finite() || seconds <= 0. {
            return;
        }
        self.pending = (self.pending + seconds).min(1.);
        while self.pending >= MODE_STEP {
            self.pending -= MODE_STEP;
            self.elapsed += MODE_STEP;
            self.frame(table);
        }
    }
    /// One 427ED0 frame on the mode terms.
    fn frame(&mut self, table: &Table) {
        // A transient mode past its hold sets the remembered mode again; the
        // groups keep their levels this frame.
        if !self.persistent && !self.tied && self.elapsed > self.hold {
            self.set(self.back);
            return;
        }
        let mut levels = self.levels.unwrap_or(table.modes[self.from]);
        let mut settled = 0;
        for (g, level) in levels.iter_mut().enumerate() {
            let target = table.modes[self.mode][g];
            if *level == target {
                settled += 1;
                continue;
            }
            let (from, time) = if self.from != self.mode {
                (table.modes[self.from][g], table.attack[self.mode] + self.extra)
            } else {
                (*level, SETTLE)
            };
            let t = self.elapsed / time;
            *level = if t >= 1. || t.is_nan() {
                target
            } else if t <= 0. {
                from
            } else {
                from + (target - from) * t
            };
        }
        if settled >= GROUPS - 1 {
            self.from = self.mode;
            self.extra = 0.;
        }
        self.levels = Some(levels);
    }
    /// The group level of 427ED0 with the current mode term.
    pub fn group_db(&self, table: &Table, camera: Camera, group: usize, trim: f32) -> f32 {
        let mode = self.levels.map_or(table.modes[self.from][group], |l| l[group]);
        table.group_db(NORMAL, camera, group, trim) - table.modes[NORMAL][group] + mode
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

/// Speed of sound of the Doppler term (stored by A22480 beside the floor and rolloff).
pub const SPEED_OF_SOUND: f32 = 343.;
/// The camera listener's velocity is its displacement over the frame, kept only below this
/// (0x43F570); a larger jump (a camera cut) counts as standing still [I: 0].
pub const LISTENER_JUMP: f32 = 8.;

/// The game's own Doppler factor (CMStream_Update a2cc1d–a2cd14; OpenAL's factor is 0):
/// `position` is the voice relative to its listener and `velocity` the voice's velocity minus
/// the listener's, times the voice's Doppler property (0xA233D0). v > 0 recedes:
/// v = position·velocity / (|position| + 0.01), f = c / (c + v), and 0 at or below −c.
pub fn doppler(position: [f32; 3], velocity: [f32; 3]) -> f32 {
    let distance = (position[0] * position[0] + position[1] * position[1] + position[2] * position[2]).sqrt();
    let v = (position[0] * velocity[0] + position[1] * velocity[1] + position[2] * velocity[2]) / (distance + 0.01);
    if v > -SPEED_OF_SOUND { SPEED_OF_SOUND / (SPEED_OF_SOUND + v) } else { 0. }
}
/// AL_PITCH of a 3D voice: its rate ratio times the Doppler factor, clamped to [0.5, 2].
pub fn doppler_pitch(pitch: f32, doppler: f32) -> f32 {
    (pitch * doppler).clamp(0.5, 2.)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doppler_follows_cmstream_update() {
        // At rest, and moving across the line of sight: no shift.
        assert_eq!(doppler([0., 0., 50.], [0.; 3]), 1.);
        assert_eq!(doppler([0., 0., 50.], [30., 0., 0.]), 1.);
        // Approaching at 34.3 m/s: v = −34.3·50/50.01, f ≈ 343/308.7.
        let f = doppler([0., 0., 50.], [0., 0., -34.3]);
        assert!((f - 343. / (343. - 34.3 * 50. / 50.01)).abs() < 1e-5, "{f}");
        // Receding lowers the pitch.
        assert!(doppler([0., 0., 50.], [0., 0., 34.3]) < 1.);
        // At or beyond the speed of sound towards the listener: 0, so the pitch clamps to 0.5.
        assert_eq!(doppler([0., 0., 50.], [0., 0., -400.]), 0.);
        assert_eq!(doppler_pitch(1., 0.), 0.5);
        assert_eq!(doppler_pitch(1.5, 1.5), 2.);
    }

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
        // StartLine differs from Normal in AI only; its attack is 1.5 s.
        let differ: Vec<usize> = (0..GROUPS).filter(|&g| t.modes[START_LINE][g] != t.modes[NORMAL][g]).collect();
        assert_eq!(differ, [AI]);
        assert_eq!(t.attack[START_LINE], 1.5);
    }

    #[test]
    fn start_line_lowers_ai_with_the_427ed0_fade() {
        let mut t = Table {
            master: -3.,
            modes: [[0.; GROUPS]; 10],
            attack: [0.15, 0., 0.2, 0., 1.5, 0., 0., 0., 0., 0.],
            cameras: [[0.; GROUPS]; 6],
        };
        t.modes[START_LINE][AI] = -3.;
        let mut mix = ModeMix::default();
        let ai = |m: &ModeMix| m.group_db(&t, Camera::Chase, AI, 0.);
        assert_eq!(ai(&mix), -3.);
        mix.set(START_LINE);
        // Before the first frame the old level holds.
        assert_eq!(ai(&mix), -3.);
        mix.advance(&t, MODE_STEP);
        // First frame: lerp from Normal over attack 1.5 s; 20 groups are
        // already at target, so the fade ends and the settle law takes over.
        let first = ai(&mix);
        assert!((first - (-3. - 3. * MODE_STEP / 1.5)).abs() < 1e-5, "{first}");
        assert_eq!(mix.from, START_LINE);
        // The settle law converges well before 1.5 s, and reaches it by then.
        mix.advance(&t, 0.5);
        let half = ai(&mix);
        assert!(half < -5.9 && half > -6., "{half}");
        mix.advance(&t, 1.);
        assert_eq!(ai(&mix), -6.);
        // Engine untouched.
        assert_eq!(mix.group_db(&t, Camera::Chase, ENGINE, 0.), -3.);
        // Back to Normal at the start: the same law from -6 to -3.
        mix.set(NORMAL);
        mix.advance(&t, 2.);
        assert_eq!(ai(&mix), -3.);
        // Setting the current mode again does not restart anything.
        let before = mix.clone();
        mix.set(NORMAL);
        assert_eq!(mix, before);
    }

    #[test]
    fn talking_lasts_while_the_voice_plays() {
        let t = Table {
            master: 0.,
            modes: [[0.; GROUPS]; 10],
            attack: [0.15, 0.2, 0.01, 0.5, 1.5, 0.5, 1., 1., 1., 0.],
            cameras: [[0.; GROUPS]; 6],
        };
        let mut mix = ModeMix::default();
        mix.set(START_LINE);
        assert!(mix.set_tied(TALKING));
        // No duration: it holds however long the line plays.
        mix.advance(&t, 10.);
        assert_eq!(mix.mode(), TALKING);
        assert!(!mix.set_tied(TALKING));
        assert!(mix.release());
        assert_eq!(mix.mode(), START_LINE);
        assert!(!mix.release());
        // Refused while paused.
        mix.set(PAUSED);
        assert!(!mix.set_tied(TALKING));
        // A timed mode replaces the tie.
        mix.set(NORMAL);
        assert!(mix.set_tied(TALKING));
        assert!(mix.set_for(&t, CRASHED, CRASH_SECONDS));
        assert!(!mix.release());
        mix.advance(&t, 1.);
        assert_eq!(mix.mode(), NORMAL);
    }

    #[test]
    fn crashed_ducks_the_engine_and_returns() {
        let mut t = Table {
            master: 0.,
            modes: [[0.; GROUPS]; 10],
            attack: [0.15, 0.2, 0.01, 0.5, 1.5, 0.5, 1., 1., 1., 0.],
            cameras: [[0.; GROUPS]; 6],
        };
        for g in [EXHAUST, ENGINE, 2, GEARBOX, WHEELS, 12, 20] {
            t.modes[CRASHED][g] = -24.;
        }
        t.modes[CRASHED][AI] = -6.;
        t.modes[START_LINE][AI] = -3.;
        t.modes[END_OF_RACE] = [-4.; GROUPS];
        let engine = |m: &ModeMix| m.group_db(&t, Camera::Chase, ENGINE, 0.);
        let mut mix = ModeMix::default();
        assert!(mix.set_for(&t, CRASHED, CRASH_SECONDS));
        // Already crashed: refused.
        assert!(!mix.set_for(&t, CRASHED, CRASH_SECONDS));
        // Attack 0.01 s: at -24 dB on the first frame.
        mix.advance(&t, MODE_STEP);
        assert_eq!(engine(&mix), -24.);
        // Hold 0.2 - attack(Normal) 0.15 = 0.05 s, then Normal again.
        mix.advance(&t, MODE_STEP);
        assert_eq!(mix.mode(), CRASHED);
        mix.advance(&t, 3. * MODE_STEP);
        assert_eq!(mix.mode(), NORMAL);
        // Back over Normal's 0.15 s attack, linearly from the Crashed level.
        let back = engine(&mix);
        assert!(back > -24. && back < 0., "{back}");
        mix.advance(&t, 0.2);
        assert_eq!(engine(&mix), 0.);
        // From StartLine: the hold uses the attack of the mode remembered
        // before (+C24 is read before it is updated: Normal, 0.05 s again);
        // the return takes StartLine's 1.5 s, linearly.
        mix.set(START_LINE);
        mix.advance(&t, 2.);
        assert!(mix.set_for(&t, CRASHED, CRASH_SECONDS));
        mix.advance(&t, 5. * MODE_STEP);
        assert_eq!(mix.mode(), START_LINE);
        mix.advance(&t, 0.75);
        let half = engine(&mix);
        assert!(half > -14. && half < -10., "{half}");
        mix.advance(&t, 1.);
        assert_eq!(engine(&mix), 0.);
        assert_eq!(mix.group_db(&t, Camera::Chase, AI, 0.), -3.);
        // Results (EndOfRace, attack 1) then a restart: StartLine fades over
        // its attack plus CB0BC4's extra 1.5 s, linearly, as many groups differ.
        mix.set(END_OF_RACE);
        mix.advance(&t, 0.75);
        mix.advance(&t, 0.75);
        assert_eq!(engine(&mix), -4.);
        mix.set(START_LINE);
        mix.advance(&t, 0.75);
        mix.advance(&t, 0.75);
        let midway = engine(&mix);
        assert!((midway + 2.).abs() < 0.1, "{midway}");
        mix.advance(&t, 0.8);
        mix.advance(&t, 0.8);
        assert_eq!(engine(&mix), 0.);
        // Paused refuses a crash.
        mix.set(PAUSED);
        assert!(!mix.set_for(&t, CRASHED, CRASH_SECONDS));
    }
}
