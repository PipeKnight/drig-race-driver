//! Location ambience (docs/AUDIO_RESEARCH.md §30): the original's in-race
//! ambience object (vtable 0xCB510C, update 0x453200) keeps a field of up to
//! 96 emitters around the listened car. Its AutoPlay is the one named by the
//! track model's `local_audio_ambience` (0x461430, chosen once at load); a
//! name without an AutoPlay (loc_europe, loc_japan, loc_lemans) is silent.
//! Each tick 0x4477F0 may spawn one emitter beside and ahead of the car, and
//! 0x447FC0 fades every emitter in, sets its level from the car's speed and
//! frees it beyond 117 m. The emitters play on the Ambience group (12) with
//! the A23B70 distance law. Presentation only.
use crate::{
    collision_audio::{Curve, read_infos},
    hud_audio::{Sound, wip_sound},
    route::Route,
};
use grid_formats::{audio_dictionary, binxml};
use std::{path::Path, sync::Arc};

/// Mix groups (41F570): the location ambience and the crowds.
pub const AMBIENCE: usize = 12;
pub const CROWD: usize = 11;
/// Emitter slots (0x44D420 clears 0x60 sample indices).
pub const SLOTS: usize = 96;
/// 0x447FC0 frees an emitter farther than this from the car (0xCB4A84).
pub const FREE_BEYOND: f32 = 117.;
/// The fade-in reaches 1 after this many ticks (0xCB4A80 = 1/30).
const FADE_TICKS: f32 = 30.;
/// A finished one-shot farther than this (0xCAFA58) may play again ...
const REPLAY_BEYOND: f32 = 10.;
/// ... with odds 1 in 600 per tick (0x4481C8).
const REPLAY_ODDS: u32 = 600;

/// An autoplaybanks Sample with its preset's values.
#[derive(Clone, Debug, PartialEq)]
pub struct Sample {
    pub name: String,
    /// The bank sound; `None` when the bank lacks it or the decoder refuses it.
    pub sound: Option<usize>,
    pub min_speed: f32,
    pub max_speed: f32,
    pub min_vol: f32,
    pub max_vol: f32,
    pub probability: f32,
    pub min_distance: f32,
    pub max_distance: f32,
    pub min_spawn: f32,
    pub max_spawn: f32,
    /// The bank sample's loop flag (0x461430 reads format bit 20).
    pub looped: bool,
}

/// An AutoPlay of autoplaybanks.bin with its samples resolved in its bank.
pub struct AutoPlay {
    pub name: String,
    pub bank: String,
    pub density: f32,
    pub max_loopers: u32,
    pub samples: Vec<Sample>,
    /// The `masterCollisionInfo` curve: an extra level from the speed.
    pub master: Option<Curve>,
    pub sounds: Vec<Sound>,
}

fn attr<'a>(e: &binxml::Element<'a>, name: &str) -> Option<&'a str> {
    e.attributes.iter().find(|(k, _)| *k == name).map(|(_, v)| *v)
}
fn number(e: &binxml::Element, name: &str) -> Option<f32> {
    attr(e, name).and_then(|v| v.trim().parse::<f32>().ok()).filter(|v| v.is_finite())
}
fn read(path: &Path) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// The route's `track_model.local_audio_ambience`.
pub fn location(root: &Path, route: &Route) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let db = crate::read_database(root)?;
    let parts: Vec<&str> = route.path().split('/').collect();
    let [_, folder, file, layout] = parts[..] else {
        return Err(format!("route {} is not tracks/<region>/<track>/<route>", route.path()).into());
    };
    let row = db
        .table("track_model")
        .ok_or("database track_model absent")?
        .rows()
        .find(|m| {
            let eq = |col: &str, v: &str| m.text(col).is_some_and(|t| t.eq_ignore_ascii_case(v));
            eq("folder_string", folder) && eq("file_string", file) && eq("route_string", layout)
        })
        .ok_or(format!("no track model for {}", route.path()))?;
    Ok(row.text("local_audio_ambience").map(str::to_owned))
}

impl AutoPlay {
    /// The AutoPlay `name` (case-insensitive, as 0x433310) with its bank;
    /// `None` when autoplaybanks has no such AutoPlay.
    pub fn load(root: &Path, name: &str) -> Result<Option<Arc<Self>>, String> {
        let audio = root.join("audio");
        let banks = read(&audio.join("autoplaybanks.bin"))?;
        let Some(bank) = Self::bank_of(&banks, name)? else { return Ok(None) };
        let data = read(&audio.join(format!("{bank}.WIP")))?;
        Self::parse(
            &read(&audio.join("CMSamples.dic"))?,
            &banks,
            &read(&audio.join("CollisionInfos.bin"))?,
            name,
            &data,
        )
        .map(|a| a.map(Arc::new))
    }

    fn bank_of(banks: &[u8], name: &str) -> Result<Option<String>, String> {
        let doc = binxml::read(banks).map_err(|e| format!("autoplaybanks: {e:?}"))?;
        Ok(doc
            .elements
            .iter()
            .find(|e| e.name == "AutoPlay" && attr(e, "name").is_some_and(|n| n.eq_ignore_ascii_case(name)))
            .and_then(|e| attr(e, "bank"))
            .map(str::to_owned))
    }

    /// `data`: the AutoPlay's WIP bank.
    pub fn parse(dictionary: &[u8], banks: &[u8], infos: &[u8], name: &str, data: &[u8]) -> Result<Option<Self>, String> {
        let dictionary = audio_dictionary::read(dictionary).map_err(|e| format!("{e:?}"))?;
        let doc = binxml::read(banks).map_err(|e| format!("autoplaybanks: {e:?}"))?;
        const FIELDS: [&str; 9] = [
            "minSpeed",
            "maxSpeed",
            "minVol",
            "maxVol",
            "probability",
            "minDistance",
            "maxDistance",
            "minSpawiningDistance",
            "maxSpawiningDistance",
        ];
        let presets: std::collections::HashMap<&str, [f32; 9]> = doc
            .elements
            .iter()
            .filter(|e| e.name == "Preset")
            .filter_map(|e| Some((attr(e, "name")?, FIELDS.map(|f| number(e, f).unwrap_or(0.)))))
            .collect();
        let Some(at) = doc
            .elements
            .iter()
            .position(|e| e.name == "AutoPlay" && attr(e, "name").is_some_and(|n| n.eq_ignore_ascii_case(name)))
        else {
            return Ok(None);
        };
        let play = &doc.elements[at];
        let bank_name = attr(play, "bank").unwrap_or_default();
        let bank = dictionary
            .banks
            .iter()
            .find(|b| b.name.eq_ignore_ascii_case(bank_name))
            .ok_or(format!("{bank_name} bank absent"))?;
        let mut sounds = Vec::new();
        let mut samples = Vec::new();
        for e in doc.elements[play.children.clone()].iter().filter(|e| e.name == "Sample") {
            let Some(sample) = attr(e, "name") else { continue };
            let v = attr(e, "preset").and_then(|p| presets.get(p)).copied().unwrap_or([0.; 9]);
            let index = bank.samples.iter().position(|s| s.name.eq_ignore_ascii_case(sample));
            let (sound, looped) = match index.map(|i| (i, wip_sound(bank, i, data))) {
                Some((i, Ok(s))) => {
                    sounds.push(s);
                    (Some(sounds.len() - 1), bank.samples[i].flags & 0x10 != 0)
                }
                _ => (None, false),
            };
            samples.push(Sample {
                name: sample.to_owned(),
                sound,
                min_speed: v[0],
                max_speed: v[1],
                min_vol: v[2],
                max_vol: v[3],
                probability: v[4],
                min_distance: v[5],
                max_distance: v[6],
                min_spawn: v[7],
                max_spawn: v[8],
                looped,
            });
        }
        let master = attr(play, "masterCollisionInfo").and_then(|m| read_infos(infos).ok()?.remove(m));
        Ok(Some(Self {
            name: attr(play, "name").unwrap_or(name).to_owned(),
            bank: bank_name.to_owned(),
            density: number(play, "density").unwrap_or(0.),
            max_loopers: attr(play, "maxLoopers").and_then(|v| v.trim().parse().ok()).unwrap_or(0),
            samples,
            master,
            sounds,
        }))
    }
}

/// 0x413990: where `x` lies between `a` and `b` (either order), in [0, 1].
fn linstep(a: f32, b: f32, x: f32) -> f32 {
    let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
    if x <= lo {
        0.
    } else if x >= hi {
        1.
    } else {
        (x - lo) / (hi - lo)
    }
}
/// 0x413130: `a` to `b` by `t`, clamped.
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0., 1.)
}
/// 0x4138F0: amplitude to dB, floored at −48.
fn lin2db(v: f32) -> f32 {
    if v <= 0. {
        -48.
    } else if v >= 1. {
        0.
    } else {
        (20. * v.log10()).max(-48.)
    }
}

/// The listened car (0x453200: the first car whose +0x6C is 0): its pose
/// rows (right +0x3A30, forward +0x3A50, position +0x3A60, copied by
/// 0xA69630) and its speed |v| (+0x3A70).
#[derive(Clone, Copy, Debug)]
pub struct Listener {
    pub right: [f32; 3],
    pub forward: [f32; 3],
    pub position: [f32; 3],
    pub speed: f32,
}

impl Listener {
    /// From the physical body's matrix (+0x1D0, rows) and velocity (+0x220).
    pub fn of(pose: &[f32; 16], velocity: [f32; 4]) -> Self {
        Self {
            right: [pose[0], pose[1], pose[2]],
            forward: [pose[8], pose[9], pose[10]],
            position: [pose[12], pose[13], pose[14]],
            speed: velocity.iter().map(|v| v * v).sum::<f32>().sqrt(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Emitter {
    sample: usize,
    position: [f32; 3],
    ticks: u32,
    /// Seconds left of a one-shot's playback; 0 once it ended (its voice
    /// handle released).
    left: f32,
}

/// What the mixer does for a slot this tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    /// Start `sound` in `slot` (from its beginning, looped or once).
    Start { slot: usize, sound: usize, looped: bool },
    /// The slot's voice level (dB, before group and distance) and place.
    Level { slot: usize, db: f32, position: [f32; 3], min_distance: f32, max_distance: f32 },
    /// End the slot's voice.
    Stop { slot: usize },
}

/// The emitter field of one AutoPlay (the ambience object's +0x18E4 slots).
pub struct Field {
    play: Arc<AutoPlay>,
    slots: Vec<Option<Emitter>>,
    loopers: u32,
    random: u32,
}

impl Field {
    pub fn new(play: Arc<AutoPlay>, seed: u32) -> Self {
        Self { play, slots: vec![None; SLOTS], loopers: 0, random: seed | 1 }
    }

    pub fn play(&self) -> &AutoPlay {
        &self.play
    }

    pub fn active(&self) -> usize {
        self.slots.iter().flatten().count()
    }

    pub fn loopers(&self) -> u32 {
        self.loopers
    }

    /// The game's random generator is not reproduced [I]: a 32-bit LCG.
    fn next(&mut self) -> u32 {
        self.random = self.random.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        self.random >> 8
    }
    fn uniform(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * (self.next() & 0xff_ffff) as f32 / 16_777_216.
    }

    /// 0x44D420 for one tick of `dt` seconds: a spawn (0x4477F0), then every
    /// emitter's update (0x447FC0).
    pub fn tick(&mut self, listener: &Listener, dt: f32, out: &mut Vec<Event>) {
        self.spawn(listener, out);
        self.update(listener, dt, out);
    }

    /// The `masterCollisionInfo` level at `speed` (above its minVel only).
    fn master_db(&self, speed: f32) -> f32 {
        self.play.master.as_ref().filter(|c| speed > c.min_vel).map_or(0., |c| {
            lerp(c.min_vol, c.max_vol, linstep(c.min_vel, c.max_vel, speed))
        })
    }

    fn spawn(&mut self, l: &Listener, out: &mut Vec<Event>) {
        let k = (l.speed * 10.).clamp(10., 100.);
        if self.uniform(0., 50.) > self.play.density * k || self.play.samples.is_empty() {
            return;
        }
        let index = self.next() as usize % self.play.samples.len();
        let sample = self.play.samples[index].clone();
        // condition/timeOfDay: no shipped Sample sets them, so all match.
        if self.uniform(0., 1.) > sample.probability {
            return;
        }
        // 0x40FD60: a free slot [I: the first].
        let Some(slot) = self.slots.iter().position(Option::is_none) else { return };
        if sample.looped {
            if self.play.max_loopers < self.loopers {
                return;
            }
            self.loopers += 1;
        }
        let Some(sound) = sample.sound else {
            // The original starts a voice that cannot play; the slot stays
            // taken until the car is 117 m away.
            self.slots[slot] = Some(Emitter { sample: index, position: l.position, ticks: 0, left: 0. });
            return;
        };
        // Beside the car on a random side, up to four car lengths of the
        // forward axis ahead, times a spawning distance: each component
        // uniform between ±right and ±right + 4·forward [I: the per-component
        // ordering of 0x4682A0].
        let r = self.uniform(sample.min_spawn, sample.max_spawn);
        let side = if self.next() % 4 < 2 { -1. } else { 1. };
        let mut position = l.position;
        for (i, p) in position.iter_mut().enumerate() {
            let a = side * l.right[i];
            let b = a + 4. * l.forward[i];
            *p += r * self.uniform(a.min(b), a.max(b));
        }
        let s = &self.play.sounds[sound];
        let seconds = (s.pcm.len() / s.channels.max(1)) as f32 / s.rate.max(1) as f32;
        self.slots[slot] = Some(Emitter { sample: index, position, ticks: 0, left: if sample.looped { f32::INFINITY } else { seconds } });
        out.push(Event::Start { slot, sound, looped: sample.looped });
    }

    fn update(&mut self, l: &Listener, dt: f32, out: &mut Vec<Event>) {
        let master = self.master_db(l.speed);
        for slot in 0..SLOTS {
            let Some(mut e) = self.slots[slot] else { continue };
            e.ticks += 1;
            let distance = e.position.iter().zip(l.position).map(|(a, b)| (a - b) * (a - b)).sum::<f32>().sqrt();
            let sample = &self.play.samples[e.sample];
            if distance > FREE_BEYOND {
                if sample.sound.is_some() {
                    out.push(Event::Stop { slot });
                }
                if sample.looped {
                    self.loopers -= 1;
                }
                self.slots[slot] = None;
                continue;
            }
            let Some(sound) = sample.sound else {
                self.slots[slot] = Some(e);
                continue;
            };
            let (looped, min_speed, max_speed, min_vol, max_vol, min_distance, max_distance) = (
                sample.looped,
                sample.min_speed,
                sample.max_speed,
                sample.min_vol,
                sample.max_vol,
                sample.min_distance,
                sample.max_distance,
            );
            if !looped {
                e.left = (e.left - dt).max(0.);
            }
            if !looped && e.left <= 0. && distance > REPLAY_BEYOND {
                // 0x4481C1: one in 600 plays it again (0x43E4B0); otherwise
                // the level is left alone this tick.
                if !self.next().is_multiple_of(REPLAY_ODDS) {
                    self.slots[slot] = Some(e);
                    continue;
                }
                let s = &self.play.sounds[sound];
                e.left = (s.pcm.len() / s.channels.max(1)) as f32 / s.rate.max(1) as f32;
                out.push(Event::Start { slot, sound, looped });
            }
            let band = if min_speed < max_speed { linstep(min_speed, max_speed, l.speed) } else { 1. };
            let fade = (e.ticks as f32 / FADE_TICKS).min(1.);
            let t = (fade * band).clamp(0., 1.);
            let db = lerp(min_vol, max_vol, t) + master + lin2db(fade);
            out.push(Event::Level { slot, db: db.clamp(-48., 0.), position: e.position, min_distance, max_distance });
            self.slots[slot] = Some(e);
        }
    }
}

/// A playing ambience voice.
#[derive(Clone, Copy)]
struct Voice {
    sound: usize,
    position: f64,
    step: f64,
    looped: bool,
    gain: [f32; 2],
    target: [f32; 2],
}

/// Gain glide per sample toward the tick's target, as the collision loops
/// [I: smoothing between ticks only].
const GLIDE: f32 = 0.002;

/// Mixes the field's voices at `rate`, one per slot.
pub struct Mixer {
    play: Arc<AutoPlay>,
    rate: u32,
    voices: Vec<Option<Voice>>,
}

impl Mixer {
    pub fn new(play: Arc<AutoPlay>, rate: u32) -> Self {
        Self { play, rate, voices: vec![None; SLOTS] }
    }

    pub fn start(&mut self, slot: usize, sound: usize, looped: bool) {
        let Some(s) = self.play.sounds.get(sound) else { return };
        let step = f64::from(s.rate) / f64::from(self.rate);
        let gain = self.voices.get(slot).copied().flatten().map_or([0.; 2], |v| v.gain);
        if let Some(v) = self.voices.get_mut(slot) {
            *v = Some(Voice { sound, position: 0., step, looped, gain, target: gain });
        }
    }

    /// The slot's linear gain (group and distance included) and pan.
    pub fn level(&mut self, slot: usize, gain: f32, pan: [f32; 2]) {
        if let Some(Some(v)) = self.voices.get_mut(slot) {
            v.target = pan.map(|p| gain.max(0.) * p);
        }
    }

    pub fn stop(&mut self, slot: usize) {
        if let Some(v) = self.voices.get_mut(slot) {
            *v = None;
        }
    }

    /// Every voice glides to silence until its next level.
    pub fn hush(&mut self) {
        for v in self.voices.iter_mut().flatten() {
            v.target = [0.; 2];
        }
    }

    pub fn active(&self) -> usize {
        self.voices.iter().flatten().count()
    }

    /// One stereo frame (left, right).
    pub fn sample(&mut self) -> [f32; 2] {
        let mut out = [0.; 2];
        let play = &self.play;
        for slot in self.voices.iter_mut() {
            let Some(v) = slot else { continue };
            let s = &play.sounds[v.sound];
            let frames = s.pcm.len() / s.channels.max(1);
            if frames < 2 || (!v.looped && v.position as usize + 1 >= frames) {
                *slot = None;
                continue;
            }
            for (g, t) in v.gain.iter_mut().zip(v.target) {
                *g += (t - *g) * GLIDE;
            }
            let i = v.position as usize % frames;
            let frac = (v.position - v.position.floor()) as f32;
            let a = s.pcm[i * s.channels];
            let b = s.pcm[(i + 1) % frames * s.channels];
            let value = a + (b - a) * frac;
            out[0] += value * v.gain[0];
            out[1] += value * v.gain[1];
            v.position += v.step;
            if v.looped {
                v.position %= frames as f64;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers_follow_the_original() {
        assert_eq!(linstep(0., 50., 25.), 0.5);
        assert_eq!(linstep(50., 0., 25.), 0.5);
        assert_eq!(linstep(1.1, 1.1, 2.), 1.);
        assert_eq!(lerp(-1., -48., 1.), -48.);
        assert_eq!(lin2db(0.), -48.);
        assert!((lin2db(0.5) + 6.0206).abs() < 1e-3);
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn shipped_ambience_per_track() {
        let root = std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required"));
        let db = crate::read_database(&root).unwrap();
        let mut seen = std::collections::BTreeMap::<String, Vec<String>>::new();
        for m in db.table("track_model").unwrap().rows() {
            let name = m.text("local_audio_ambience").unwrap_or("-").to_owned();
            let track = format!("{}/{}", m.text("folder_string").unwrap_or("?"), m.text("file_string").unwrap_or("?"));
            let tracks = seen.entry(name).or_default();
            if !tracks.contains(&track) {
                tracks.push(track);
            }
        }
        for (name, tracks) in &seen {
            let playing = AutoPlay::load(&root, name).ok().flatten().is_some();
            println!("ambience version=1 location={name} autoplay={playing} tracks={tracks:?}");
        }
        // (route, location, has an AutoPlay).
        for (path, location, playing) in [
            ("tracks/europe/donington/route_0", "loc_europe", false),
            ("tracks/europe/le_mans/route_0", "loc_lemans", false),
            ("tracks/japan/shibuya/route_0", "loc_japan", false),
            ("tracks/japan/yokohama_docks/route_0", "loc_yoko", true),
            ("tracks/usa/washington/route_0", "loc_usa", true),
            ("tracks/europe/milan/route_0", "loc_europe", false),
        ] {
            let route = Route::new(path).unwrap();
            let name = location_of(&root, &route);
            println!("ambience version=1 route={path} location={name:?}");
            assert_eq!(name.as_deref().map(str::to_ascii_lowercase).as_deref(), Some(location), "{path}");
            let play = AutoPlay::load(&root, location).unwrap();
            assert_eq!(play.is_some(), playing, "{path}");
            if let Some(play) = play {
                let missing: Vec<_> = play.samples.iter().filter(|s| s.sound.is_none()).map(|s| s.name.clone()).collect();
                let loops = play.samples.iter().filter(|s| s.looped).count();
                println!(
                    "ambience version=1 autoplay={} bank={} density={} max_loopers={} samples={} loops={loops} missing={missing:?} master={:?}",
                    play.name,
                    play.bank,
                    play.density,
                    play.max_loopers,
                    play.samples.len(),
                    play.master
                );
                assert!(missing.is_empty(), "{path}: {missing:?}");
                assert!(play.master.is_some(), "{path}");
            }
        }
    }

    fn location_of(root: &Path, route: &Route) -> Option<String> {
        location(root, route).unwrap()
    }

    /// A parked and a moving car at Washington: the field fills, keeps its
    /// loopers bounded, fades with speed (lerp to maxVol −48 at 50 m/s) and
    /// empties when the car moves on.
    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn shipped_field_spawns_fades_and_frees() {
        let root = std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required"));
        let play = AutoPlay::load(&root, "loc_usa").unwrap().unwrap();
        let mut field = Field::new(play.clone(), 7);
        let mut pose = [0f32; 16];
        (pose[0], pose[5], pose[10], pose[15]) = (1., 1., 1., 1.);
        let parked = Listener::of(&pose, [0.; 4]);
        let mut out = Vec::new();
        let (mut starts, mut levels) = (0, Vec::new());
        for _ in 0..62 * 60 {
            out.clear();
            field.tick(&parked, 0.016, &mut out);
            for e in &out {
                match e {
                    Event::Start { .. } => starts += 1,
                    Event::Level { db, .. } => levels.push(*db),
                    Event::Stop { .. } => {}
                }
            }
            assert!(field.loopers() <= play.max_loopers + 1);
        }
        let loud = levels.iter().copied().fold(-48f32, f32::max);
        println!("ambience version=1 window=parked_60s starts={starts} active={} loopers={} loudest={loud:.1}", field.active(), field.loopers());
        assert!(starts > 0 && field.active() > 0);
        // Parked, the loops reach their minVol (−1 dB) once faded in.
        assert!(loud > -3., "{loud}");
        // At 50 m/s every loop sits at maxVol −48 plus the master curve.
        let moving = Listener { speed: 50., ..parked };
        let mut quiet = f32::MIN;
        for _ in 0..62 {
            out.clear();
            field.tick(&moving, 0.016, &mut out);
            for e in &out {
                if let Event::Level { db, slot, .. } = e
                    && play.samples[field.slots[*slot].unwrap().sample].looped
                {
                    quiet = quiet.max(*db);
                }
            }
        }
        println!("ambience version=1 window=moving_50 loudest_loop={quiet:.1}");
        assert!(quiet <= -47.9, "{quiet}");
        // 500 m behind (far spawns reach about 4.1·40 m ahead), every
        // emitter is freed.
        let away = Listener { position: [0., 0., -500.], ..parked };
        out.clear();
        field.update(&away, 0.016, &mut out);
        assert_eq!(field.active(), 0);
        assert_eq!(field.loopers(), 0);
    }
}
