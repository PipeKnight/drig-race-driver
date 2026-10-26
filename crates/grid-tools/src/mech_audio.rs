//! Mechanical sounds of the car-audio object (docs/AUDIO_RESEARCH.md §21,
//! §22): the gear clunk (GearClucks), the backfire (Detonations) and the
//! turbo (TurboHiss, TurboSpindle, TurboDump, TurboPip). Each is a
//! CollisionInfo (CollisionInfos.bin, curve over x) whose AudioVariety
//! (audioVarieties.bin, template bank `mec_dbr`) names its samples and
//! distance preset; the samples come from a `mec_*` bank.
use crate::{
    collision_audio::{Curve, read_infos},
    hud_audio::{Sound, wip_sound},
};
use grid_formats::{audio_dictionary, binxml};
use std::{collections::HashMap, path::Path, sync::Arc};

/// The car-audio table entries (43D59D…43D6A3) with a CollisionInfo.
pub const INFOS: [&str; 6] = ["GearClucks", "Detonations", "TurboHiss", "TurboSpindle", "TurboDump", "TurboPip"];

/// One CollisionInfo with its variety's samples (indices into `Bank::sounds`)
/// and preset distances (A23B70 law).
#[derive(Clone, Debug, PartialEq)]
pub struct Info {
    pub curve: Curve,
    pub samples: Vec<usize>,
    pub min_distance: f32,
    pub max_distance: f32,
}

pub struct Bank {
    /// The `mec_*` bank the samples come from.
    pub name: String,
    pub sounds: Vec<Sound>,
    pub infos: HashMap<String, Info>,
}

fn attr<'a>(e: &binxml::Element<'a>, name: &str) -> Option<&'a str> {
    e.attributes.iter().find(|(k, _)| *k == name).map(|(_, v)| *v)
}
fn read(path: &Path) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))
}

impl Bank {
    /// The mechanical sounds from the `mec_*` bank `bank` (e.g. "mec_dbr").
    pub fn load(root: &Path, bank: &str) -> Result<Arc<Self>, String> {
        let audio = root.join("audio");
        Ok(Arc::new(Self::parse(
            bank,
            &read(&audio.join("CMSamples.dic"))?,
            &read(&audio.join(format!("{bank}.WIP")))?,
            &read(&audio.join("audioVarieties.bin"))?,
            &read(&audio.join("CollisionInfos.bin"))?,
        )?))
    }

    pub fn parse(name: &str, dictionary: &[u8], data: &[u8], varieties: &[u8], infos: &[u8]) -> Result<Self, String> {
        let dictionary = audio_dictionary::read(dictionary).map_err(|e| format!("{e:?}"))?;
        let bank = dictionary
            .banks
            .iter()
            .find(|b| b.name == name)
            .ok_or_else(|| format!("{name} bank absent"))?;
        let curves = read_infos(infos)?;
        let doc = binxml::read(varieties).map_err(|e| format!("audioVarieties: {e:?}"))?;
        let presets: HashMap<&str, (f32, f32)> = doc
            .elements
            .iter()
            .filter(|e| e.name == "Preset")
            .filter_map(|e| {
                let number = |k| attr(e, k).and_then(|v| v.trim().parse::<f32>().ok());
                Some((attr(e, "name")?, (number("minDist")?, number("maxDist")?)))
            })
            .collect();
        let mut sounds = Vec::new();
        let mut index: HashMap<&str, usize> = HashMap::new();
        let mut out = HashMap::new();
        for info in INFOS {
            let curve = curves.get(info).ok_or_else(|| format!("CollisionInfo {info} absent"))?.clone();
            let (v, variety) = doc
                .elements
                .iter()
                .enumerate()
                .find(|(_, e)| e.name == "AudioVariety" && attr(e, "name") == Some(curve.variety.as_str()))
                .ok_or_else(|| format!("AudioVariety {} absent", curve.variety))?;
            let mut samples = Vec::new();
            for s in doc.elements.iter().filter(|s| {
                s.name == "Sample" && s.parent.is_some_and(|p| doc.elements[p].parent == Some(v))
            }) {
                let Some(sample) = attr(s, "index") else { continue };
                let k = match index.get(sample) {
                    Some(&k) => k,
                    None => {
                        let i = bank
                            .samples
                            .iter()
                            .position(|b| b.name == sample)
                            .ok_or_else(|| format!("{name}: sample {sample} absent"))?;
                        sounds.push(wip_sound(bank, i, data)?);
                        index.insert(sample, sounds.len() - 1);
                        sounds.len() - 1
                    }
                };
                samples.push(k);
            }
            let (min_distance, max_distance) =
                attr(variety, "preset").and_then(|p| presets.get(p).copied()).unwrap_or((1., 2.));
            out.insert(info.to_owned(), Info { curve, samples, min_distance, max_distance });
        }
        Ok(Self { name: name.to_owned(), sounds, infos: out })
    }
}

/// Group (`group_db`) indices of [`Levels`]: the player's groups of 4511E0
/// (+0x120 Gearbox), 451260 (+0x124 Exhaust), the turbo loops (car-audio +4,
/// Turbo) and its dump and pip (+0xA3C4, Engine).
pub const GEARBOX: usize = 0;
pub const EXHAUST: usize = 1;
pub const TURBO: usize = 2;
pub const ENGINE: usize = 3;
/// Emitter (`distance`) indices: GearBox, Exhaust, EngineBay.
pub const GEARBOX_EMITTER: usize = 0;
pub const EXHAUST_EMITTER: usize = 1;
pub const BAY_EMITTER: usize = 2;

/// What the presentation supplies each frame for one car's mechanical
/// sounds: its group levels in dB (group mix plus trim, 4223D0) and the
/// distances of its GearBox, Exhaust and EngineBay emitters from the
/// listener (A23B70 applies each variety's own preset to them).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Levels {
    pub group_db: [f32; 4],
    pub distance: [f32; 3],
}
impl Levels {
    /// Silent: every emitter out of range.
    pub const SILENT: Self = Self { group_db: [0.; 4], distance: [f32::INFINITY; 3] };
    pub fn to_bits(self) -> [u32; 7] {
        let [a, b, c, d] = self.group_db;
        let [e, f, g] = self.distance;
        [a, b, c, d, e, f, g].map(f32::to_bits)
    }
    pub fn from_bits(bits: [u32; 7]) -> Self {
        let v = bits.map(f32::from_bits);
        Self { group_db: [v[0], v[1], v[2], v[3]], distance: [v[4], v[5], v[6]] }
    }
}

/// The group and emitter of a CollisionInfo's voice.
fn route(info: &str) -> (usize, usize) {
    match info {
        "GearClucks" => (GEARBOX, GEARBOX_EMITTER),
        "Detonations" => (EXHAUST, EXHAUST_EMITTER),
        "TurboDump" | "TurboPip" => (ENGINE, BAY_EMITTER),
        _ => (TURBO, BAY_EMITTER),
    }
}

struct Voice {
    info: &'static str,
    sound: usize,
    position: f64,
    /// Rate ratio without the pitch.
    step: f64,
    /// Curve position of a 44F010 voice; its level and pitch come from the
    /// curve. `None`: a loop whose level and pitch the caller sets (435690).
    x: Option<f32>,
    /// The caller's level in dB and pitch.
    db: f32,
    pitch: f32,
    gain: f32,
}

/// One car's mechanical voices: 44F010 one-shots and loops (the sample's
/// loop flag), and 435690 loops with a level and pitch set every frame. Each
/// info has one handle (4511E0 keeps the clunk's at +0x228), so a restart
/// replaces the playing sample.
pub struct Voices {
    bank: Arc<Bank>,
    rate: u32,
    voices: Vec<Voice>,
    last: HashMap<&'static str, usize>,
    random: u32,
}
impl Voices {
    pub fn new(bank: Arc<Bank>, rate: u32) -> Self {
        Self { bank, rate, voices: Vec::new(), last: HashMap::new(), random: 0x2468_ace1 }
    }

    pub fn clear(&mut self) {
        self.voices.clear();
    }

    /// 413B80: a uniform pick that avoids repeating the last sample [I: RNG].
    fn pick(&mut self, info: &'static str) -> Option<usize> {
        let entry = self.bank.infos.get(info)?;
        if entry.samples.is_empty() {
            return None;
        }
        let previous = self.last.get(info).copied();
        let choices: Vec<usize> =
            entry.samples.iter().copied().filter(|s| Some(*s) != previous || entry.samples.len() == 1).collect();
        self.random = self.random.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let sound = choices[(self.random >> 8) as usize % choices.len()];
        self.last.insert(info, sound);
        Some(sound)
    }

    fn begin(&mut self, info: &'static str, x: Option<f32>) {
        let Some(sound) = self.pick(info) else { return };
        self.voices.retain(|v| v.info != info);
        let step = f64::from(self.bank.sounds[sound].rate) / f64::from(self.rate);
        self.voices.push(Voice { info, sound, position: 0., step, x, db: 0., pitch: 1., gain: 0. });
    }

    /// 44F010 at curve position `x`.
    pub fn start(&mut self, info: &'static str, x: f32) {
        self.begin(info, Some(x));
    }

    /// 435690 then 444280 each frame: a loop of `info` at `db` and `pitch`,
    /// started when none plays.
    pub fn hold(&mut self, info: &'static str, db: f32, pitch: f32) {
        if !self.playing(info) {
            self.begin(info, None);
        }
        if let Some(v) = self.voices.iter_mut().find(|v| v.info == info) {
            v.db = db;
            v.pitch = pitch;
        }
    }

    /// A23350: stops the voice of `info`; whether one played.
    pub fn stop(&mut self, info: &str) -> bool {
        let before = self.voices.len();
        self.voices.retain(|v| v.info != info);
        self.voices.len() != before
    }

    /// Whether a voice of `info` is playing.
    pub fn playing(&self, info: &str) -> bool {
        self.voices.iter().any(|v| v.info == info)
    }

    /// Per frame: each voice's level from its curve (44F010) or the caller,
    /// group and preset distance (A23B70); the setter's pitch clamp 0.5–2
    /// (42C150).
    pub fn frame(&mut self, levels: Levels) {
        for v in &mut self.voices {
            let entry = &self.bank.infos[v.info];
            let (db, pitch) = v.x.map_or((v.db, v.pitch), |x| entry.curve.at(x));
            v.pitch = pitch.clamp(0.5, 2.);
            let (group, emitter) = route(v.info);
            let distance = crate::audio_mix::distance_db(levels.distance[emitter], entry.min_distance, entry.max_distance);
            v.gain = crate::audio_mix::backend_gain(db, levels.group_db[group], distance);
        }
    }

    pub fn sample(&mut self) -> f32 {
        let mut value = 0.;
        let sounds = &self.bank.sounds;
        self.voices.retain_mut(|v| {
            let sound = &sounds[v.sound];
            let pcm = &sound.pcm;
            if v.position + 1. >= pcm.len() as f64 {
                if !sound.looped || pcm.len() < 2 {
                    return false;
                }
                v.position -= (pcm.len() - 1) as f64;
            }
            let index = v.position as usize;
            let fraction = (v.position - index as f64) as f32;
            value += (pcm[index] + (pcm[index + 1] - pcm[index]) * fraction) * v.gain;
            v.position += v.step * f64::from(v.pitch);
            true
        });
        value
    }
}

/// What 451260 reads from the engine controller each frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Exhaust {
    /// +0x304: the applied throttle's change since the last frame.
    pub change: f32,
    /// +0x20: the controller's RPM (+0x48 after 414C70).
    pub rpm: f32,
    /// +4 and +0xC: the last counted shift went up, seconds since it.
    pub up: bool,
    pub since_shift: f32,
    /// +0xD0: the rev limiter is active (414970).
    pub limiter: bool,
}

/// 451260, the backfire: Detonations (detloop1–3) at the Exhaust emitter.
/// A start needs the voice handle +0x21C free, and one of: a throttle drop
/// of more than 0.25 in a frame above 0.25 RPM, the first 0.2 s of an
/// upshift, or the limiter. While active (+0x318) it stops once 0.2 s past a
/// shift when the throttle rises by more than 0.25 in a frame, the RPM is at
/// or below 0.55, or the shift is 0.2–0.4 s old. Otherwise a timer (+0x31C,
/// (rand % 300)·0.01 + 0.1 s) runs down; when it expires the sample stops and
/// another starts (+0x320) once the handle is free [I: the frame after].
/// x = 1 (−11 dB, pitch 1.1) for the focused car; 411120 sets +0x334 for
/// the others, x = 0 (−30 dB).
#[derive(Clone, Debug, Default)]
pub struct Backfire {
    active: bool,
    timer: f32,
    restart: bool,
    /// The voice was stopped this frame and its handle is not yet free.
    stopping: bool,
    random: u32,
}
impl Backfire {
    pub fn new(seed: u32) -> Self {
        Self { random: seed | 1, ..Self::default() }
    }

    fn next_random(&mut self) -> u32 {
        // xorshift32 [I: A682A0's sequence is not reproduced].
        let mut x = self.random;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.random = x;
        x
    }

    pub fn frame(&mut self, e: Exhaust, dt: f32, x: f32, voices: &mut Voices) {
        const INFO: &str = "Detonations";
        // A voice stopped last frame has released its handle.
        self.stopping = false;
        let free = !voices.playing(INFO);
        let shifting = e.since_shift < 0.2 && e.up;
        if ((e.change < -0.25 && e.rpm > 0.25) || shifting || e.limiter) && free {
            voices.start(INFO, x);
            self.active = true;
            self.timer = (self.next_random() % 300) as f32 * 0.01 + 0.1;
        }
        if self.active {
            let window = 0.2 < e.since_shift && e.since_shift < 0.4;
            if (e.change > 0.25 || e.rpm <= 0.55 || window) && e.since_shift > 0.2 {
                self.active = false;
                if voices.stop(INFO) {
                    self.stopping = true;
                }
                return;
            }
            self.timer -= dt;
            if self.timer <= 0. {
                let chance = ((1. - self.timer * (1. / 3.)) * 100.).round() as u32;
                if self.next_random() % 100 < chance {
                    if voices.stop(INFO) {
                        self.stopping = true;
                    }
                    self.restart = true;
                }
            }
        }
        if self.restart && !self.stopping && !voices.playing(INFO) {
            voices.start(INFO, x);
            self.active = true;
            self.restart = false;
            self.timer = (self.next_random() % 300) as f32 * 0.01;
        }
    }
}

/// The turbo of 44D840 (player only, car-audio +0xA4D8): the model 416BC0 on
/// the object at car-audio +0x800, whose fields are named here by offset.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Turbo {
    /// +0: Spindle level (linear), follows the load at 1/s.
    pub spindle: f32,
    /// +4: Spindle pitch = gradient · +8 + base (carsetup Spidle 1.5, 0.75).
    pub spindle_pitch: f32,
    /// +8: slow boost, follows +0x30 at 0.5/s.
    slow: f32,
    /// +0xC (= +0x14): Hiss level (linear).
    pub hiss: f32,
    /// +0x10: Hiss pitch = gradient · rpm + base (carsetup Hiss 0, 1).
    pub hiss_pitch: f32,
    /// +0x14: boost level, rises at dt·d, falls at 2·dt·d.
    level: f32,
    /// +0x1C: published boost (dashboard), not used by the sound.
    pub boost: f32,
    /// +0x28: the last frame's load.
    load: f32,
    /// +0x2C: seconds above half load.
    above_half: f32,
    /// +0x30: boost demand, follows min(2·rpm, 1)·load at 16/s.
    demand: f32,
    /// +0x34: phase of the boost flutter.
    phase: f32,
    /// +0x38: kick after a fast throttle rise.
    kick: f32,
    /// +0x3C…+0x48: Spidle base, gradient; Hiss base, gradient (43B285 from
    /// the car record +0x70…+0x7C, carsetup `Turbo`).
    spidle: [f32; 2],
    hiss_curve: [f32; 2],
}

/// What one turbo frame plays besides the loops: +0x819 TurboDump, +0x818
/// TurboPip.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Blow {
    pub dump: bool,
    pub pip: bool,
}

impl Turbo {
    /// carsetup `Turbo` Spidle and Hiss (basePitch, gradient), the same in
    /// all 45 cars.
    pub const SPIDLE: [f32; 2] = [0.75, 1.5];
    pub const HISS: [f32; 2] = [1., 0.];

    /// The carsetup `Turbo` Spidle and Hiss (basePitch, gradient); 41E1A0's
    /// defaults are (1, 0.5) each.
    pub fn new(spidle: [f32; 2], hiss: [f32; 2]) -> Self {
        Self {
            spindle: 0.,
            spindle_pitch: 0.,
            slow: 0.,
            hiss: 0.,
            hiss_pitch: 0.,
            level: 0.,
            boost: 0.,
            load: 0.,
            above_half: 0.,
            demand: 0.,
            phase: 0.,
            kick: 0.,
            spidle,
            hiss_curve: hiss,
        }
    }

    /// 416BC0 for `dt` seconds at controller RPM +0xC8 and load +0xB0;
    /// `shift` is 1 on the frame of a counted upshift, 2 of a downshift
    /// (controller +0xC = 0), else 0.
    pub fn frame(&mut self, dt: f32, rpm: f32, load: f32, shift: u32) -> Blow {
        let mut blow = Blow::default();
        self.above_half = if load > 0.5 { dt + self.above_half } else { 0. };
        let r2 = (rpm * 2.).min(1.);
        self.demand += (r2 * load - self.demand) * 16. * dt;
        let mut a = self.demand * 0.66;
        if a < 0.33 {
            a *= 5.;
            if a > 0.33 {
                a = 0.33;
            }
        }
        if rpm < 0.5 && a > 0.33 {
            a = 0.33;
        }
        if load < 0.5 && load - self.load > 0.5 {
            self.kick += 0.05;
            if self.kick > 0.75 {
                self.kick = 0.75;
            }
        } else {
            a += self.kick;
            self.kick *= 0.97;
        }
        let square = rpm * rpm;
        self.phase += square * rpm * rpm * rpm * load;
        let sine = f64::from(self.phase).sin() as f32;
        self.boost = (sine * (square * 0.04) + a) * 4.;
        if shift == 2 {
            if self.demand > 0.5 {
                self.level *= 0.75;
                self.spindle *= 0.25;
                blow.dump = true;
                self.boost *= 0.6;
                self.demand *= 0.05;
            }
        } else if shift == 1 {
            self.boost *= 0.8;
            self.level *= 0.75;
            self.spindle *= 0.75;
            blow.pip = true;
            self.demand *= 0.05;
        }
        if load - self.load < -0.25 && !blow.pip && self.demand > 0.5 {
            self.level *= 0.75;
            self.spindle *= 0.25;
            self.demand *= 0.05;
            self.boost *= 0.6;
            blow.dump = true;
        }
        let mut d = self.demand - self.level;
        self.load = load;
        if d <= 0. {
            d *= 2.;
        }
        self.level += dt * d;
        self.hiss = self.level;
        self.hiss_pitch = self.hiss_curve[1] * rpm + self.hiss_curve[0];
        self.slow += (self.demand - self.slow) * 0.5 * dt;
        self.spindle_pitch = self.spidle[1] * self.slow + self.spidle[0];
        self.spindle += (load - self.spindle) * dt;
        blow
    }

    /// The rest of 44D840: TurboDump and TurboPip one-shots at x = 1 (Engine
    /// group), then the Spindle (44D560) and Hiss (44D6D0) loops at
    /// lin2db(level) + the CollisionInfo's maximum volume (+0x60) and pitch
    /// +4 and +0x10 (Turbo group). Engine damage misfires (+0x830) are not
    /// fed [I: no engine damage in the port's audio].
    pub fn play(&self, blow: Blow, voices: &mut Voices) {
        if blow.dump {
            voices.start("TurboDump", 1.);
        }
        if blow.pip {
            voices.start("TurboPip", 1.);
        }
        for (info, level, pitch) in [
            ("TurboSpindle", self.spindle, self.spindle_pitch),
            ("TurboHiss", self.hiss, self.hiss_pitch),
        ] {
            let Some(maximum) = voices.bank.infos.get(info).map(|i| i.curve.max_vol) else { continue };
            voices.hold(info, crate::tyre_audio::lin2db(level) + maximum, pitch);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_turbo_spools_with_load_pips_on_an_upshift_and_dumps_on_a_lift() {
        let mut t = Turbo::new(Turbo::SPIDLE, Turbo::HISS);
        let dt = 0.016;
        for _ in 0..500 {
            assert_eq!(t.frame(dt, 0.8, 1., 0), Blow::default());
        }
        // demand → min(1.6, 1)·1 = 1; Spindle level → load, pitch → 1.5·slow + 0.75.
        assert!(t.demand > 0.99 && t.spindle > 0.99 && t.hiss > 0.99, "{t:?}");
        assert!(t.spindle_pitch > 1.5 && t.spindle_pitch <= 2.25, "{t:?}");
        assert_eq!(t.hiss_pitch, 1.);
        let pip = t.frame(dt, 0.6, 1., 1);
        assert_eq!(pip, Blow { dump: false, pip: true });
        assert!(t.demand < 0.1);
        for _ in 0..200 {
            t.frame(dt, 0.8, 1., 0);
        }
        let lift = t.frame(dt, 0.8, 0., 0);
        assert_eq!(lift, Blow { dump: true, pip: false });
        // A downshift dumps only with boost demand above 0.5.
        assert_eq!(t.frame(dt, 0.8, 0., 2), Blow::default());
    }

    /// 451260 on the Lacetti's bank: a lift above 0.25 RPM starts the
    /// backfire, which plays while the RPM stays above 0.55 and stops below.
    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn a_lift_backfires_until_the_rpm_falls() {
        let root = std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").unwrap());
        let mut voices = Voices::new(Bank::load(&root, "mec_lac").unwrap(), 48000);
        let mut b = Backfire::new(3);
        let held = Exhaust { change: 0., rpm: 0.8, up: true, since_shift: 5., limiter: false };
        b.frame(held, 0.016, 1., &mut voices);
        assert!(!voices.playing("Detonations"));
        b.frame(Exhaust { change: -1., ..held }, 0.016, 1., &mut voices);
        assert!(voices.playing("Detonations"));
        for _ in 0..30 {
            b.frame(held, 0.016, 1., &mut voices);
        }
        assert!(voices.playing("Detonations") || b.restart);
        b.frame(Exhaust { rpm: 0.5, ..held }, 0.016, 1., &mut voices);
        assert!(!voices.playing("Detonations"));
        // The first 0.2 s of an upshift.
        b.frame(Exhaust { since_shift: 0.05, rpm: 0.5, ..held }, 0.016, 1., &mut voices);
        assert!(voices.playing("Detonations"));
        b.frame(Exhaust { since_shift: 0.25, rpm: 0.5, ..held }, 0.016, 1., &mut voices);
        assert!(!voices.playing("Detonations"));
        let det = &voices.bank.infos["Detonations"];
        assert_eq!((det.min_distance, det.max_distance), (10., 40.));
        // Only the turbo loops carry the WIP loop flag: detloop1–3 are
        // one-shots, which the timer restarts.
        for info in INFOS {
            let looped = voices.bank.infos[info].samples.iter().all(|&k| voices.bank.sounds[k].looped);
            assert_eq!(looped, info == "TurboHiss" || info == "TurboSpindle", "{info}");
        }
    }

    /// Every shipped `mec_*` bank holds the mechanical samples; reports how
    /// the banks differ (sample length and a checksum per sample).
    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn every_mec_bank_holds_the_mechanical_samples() {
        let root = std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").unwrap());
        let dictionary = std::fs::read(root.join("audio/CMSamples.dic")).unwrap();
        let names: Vec<String> = audio_dictionary::read(&dictionary)
            .unwrap()
            .banks
            .iter()
            .filter(|b| b.name.starts_with("mec_"))
            .map(|b| b.name.to_owned())
            .collect();
        assert!(names.len() >= 40, "{names:?}");
        for name in &names {
            let bank = Bank::load(&root, name).unwrap_or_else(|e| panic!("{name}: {e}"));
            let sums: Vec<String> = INFOS
                .iter()
                .flat_map(|i| bank.infos[*i].samples.iter())
                .map(|&k| {
                    let s = &bank.sounds[k];
                    let sum = s.pcm.iter().fold(0u64, |a, v| a.wrapping_mul(31).wrapping_add(v.to_bits() as u64));
                    format!("{}:{}:{:x}", s.rate, s.pcm.len(), sum & 0xffff)
                })
                .collect();
            println!("mec_bank {name} {}", sums.join(" "));
        }
        let dbr = Bank::load(&root, "mec_dbr").unwrap();
        assert!(dbr.sounds.iter().all(|s| s.channels == 1), "mono samples");
        let clunk = &dbr.infos["GearClucks"];
        assert_eq!((clunk.samples.len(), clunk.min_distance, clunk.max_distance), (3, 3.5, 4.));
        assert_eq!(clunk.curve.at(1.), (0., 1.));
        assert_eq!(dbr.infos["Detonations"].curve.at(1.), (-11., 1.1));
    }
}
