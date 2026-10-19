//! Car collision sounds (docs/AUDIO_RESEARCH.md §7.3 and §9): the original's
//! per-contact listener 0x4563F0 turns each solved car contact into an
//! intensity `x = 10·impact/mass` and 0x4559C0 picks a primary impact
//! (front/back, side, roof; nothing for the floor) whose ImpactVariety curve
//! (soft/med/hard CollisionInfos) gives one one-shot from `car_collisions`
//! (0x4535A0), plus the low/high sweeteners of hard hits. Presentation only.
use crate::hud_audio::{Sound, wip_sound};
use grid_formats::{audio_dictionary, binxml};
use std::{collections::HashMap, path::Path, sync::Arc};

/// Mix groups (41F570): the player's collisions on Panel Coll, AI cars on AI.
pub const PANEL_COLL: usize = 4;
pub const AI: usize = 17;
/// 435B20 skips a one-shot quieter than this.
pub const QUIETEST: f32 = -42.;

/// A CollisionInfo: its variety and the 435EA0 curve.
#[derive(Clone, Debug, PartialEq)]
pub struct Curve {
    pub variety: String,
    pub min_vel: f32,
    pub max_vel: f32,
    pub min_vol: f32,
    pub max_vol: f32,
    pub min_pitch: f32,
    pub max_pitch: f32,
}

impl Curve {
    /// 435EA0: t = clamp((x − minVel)/(maxVel − minVel)); volume and pitch
    /// lerped, then clamped by the voice setters (42C100, 42C150).
    pub fn at(&self, x: f32) -> (f32, f32) {
        let span = self.max_vel - self.min_vel;
        let t = if span != 0. { ((x - self.min_vel) / span).clamp(0., 1.) } else { 1. };
        let volume = self.min_vol + (self.max_vol - self.min_vol) * t;
        let pitch = self.min_pitch + (self.max_pitch - self.min_pitch) * t;
        (volume.clamp(-48., 0.), pitch.clamp(0.5, 2.))
    }
    fn contains(&self, x: f32) -> bool {
        self.min_vel <= x && x <= self.max_vel
    }
}

/// An AudioVariety of `car_collisions`: samples and preset distances.
#[derive(Clone, Debug, PartialEq)]
pub struct Variety {
    pub samples: Vec<usize>,
    pub min_distance: f32,
    pub max_distance: f32,
}

pub struct Bank {
    pub sounds: Vec<Sound>,
    pub varieties: HashMap<String, Variety>,
    pub infos: HashMap<String, Curve>,
    /// ImpactVariety name → soft, med, hard CollisionInfo (child order, 45B040).
    pub impacts: HashMap<String, [String; 3]>,
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
fn children<'d, 'a>(doc: &'d binxml::Document<'a>, i: usize) -> impl Iterator<Item = &'d binxml::Element<'a>> {
    doc.elements[doc.elements[i].children.clone()].iter()
}

impl Bank {
    pub fn load(root: &Path) -> Result<Arc<Self>, String> {
        let audio = root.join("audio");
        Ok(Arc::new(Self::parse(
            &read(&audio.join("CMSamples.dic"))?,
            &read(&audio.join("car_collisions.WIP"))?,
            &read(&audio.join("audioVarieties.bin"))?,
            &read(&audio.join("CollisionInfos.bin"))?,
            &read(&audio.join("ImpactVarieties.bin"))?,
        )?))
    }

    pub fn parse(
        dictionary: &[u8],
        data: &[u8],
        varieties: &[u8],
        infos: &[u8],
        impacts: &[u8],
    ) -> Result<Self, String> {
        let dictionary = audio_dictionary::read(dictionary).map_err(|e| format!("{e:?}"))?;
        let bank = dictionary
            .banks
            .iter()
            .find(|b| b.name == "car_collisions")
            .ok_or("car_collisions bank absent")?;
        let mut sounds = Vec::new();
        let mut index = HashMap::new();
        for (i, s) in bank.samples.iter().enumerate() {
            index.insert(s.name, sounds.len());
            sounds.push(wip_sound(bank, i, data)?);
        }
        // audioVarieties: presets (min/max distance) and car_collisions varieties.
        let doc = binxml::read(varieties).map_err(|e| format!("audioVarieties: {e:?}"))?;
        let mut presets: HashMap<&str, (f32, f32)> = HashMap::new();
        for e in doc.elements.iter().filter(|e| e.name == "Preset") {
            if let (Some(n), Some(a), Some(b)) = (attr(e, "name"), number(e, "minDist"), number(e, "maxDist")) {
                presets.insert(n, (a, b));
            }
        }
        let mut out_varieties = HashMap::new();
        for (i, v) in doc.elements.iter().enumerate() {
            if v.name != "AudioVariety" || attr(v, "bank") != Some("car_collisions") {
                continue;
            }
            let samples: Vec<usize> = doc
                .elements
                .iter()
                .filter(|s| s.name == "Sample" && s.parent.is_some_and(|p| doc.elements[p].parent == Some(i)))
                .filter_map(|s| attr(s, "index").and_then(|n| index.get(n).copied()))
                .collect();
            let (min_distance, max_distance) = attr(v, "preset")
                .and_then(|p| presets.get(p).copied())
                .unwrap_or((1., 2.));
            if let Some(name) = attr(v, "name") {
                out_varieties.insert(name.to_owned(), Variety { samples, min_distance, max_distance });
            }
        }
        // CollisionInfos: presets give the curve, each CollisionInfo may override.
        let doc = binxml::read(infos).map_err(|e| format!("CollisionInfos: {e:?}"))?;
        const FIELDS: [&str; 6] = ["minVel", "maxVel", "minVol", "maxVol", "minPitch", "maxPitch"];
        let mut curves: HashMap<&str, [Option<f32>; 6]> = HashMap::new();
        for e in doc.elements.iter().filter(|e| e.name == "Preset") {
            if let Some(n) = attr(e, "name") {
                curves.insert(n, FIELDS.map(|f| number(e, f)));
            }
        }
        let mut out_infos = HashMap::new();
        for e in doc.elements.iter().filter(|e| e.name == "CollisionInfo") {
            let Some(name) = attr(e, "name") else { continue };
            let preset = attr(e, "preset").and_then(|p| curves.get(p)).copied().unwrap_or([None; 6]);
            let v: [f32; 6] = std::array::from_fn(|k| number(e, FIELDS[k]).or(preset[k]).unwrap_or(0.));
            out_infos.insert(
                name.to_owned(),
                Curve {
                    variety: attr(e, "variety").unwrap_or_default().to_owned(),
                    min_vel: v[0],
                    max_vel: v[1],
                    min_vol: v[2],
                    max_vol: v[3],
                    min_pitch: v[4],
                    max_pitch: v[5],
                },
            );
        }
        // ImpactVarieties: the first three children in file order (45B040).
        let doc = binxml::read(impacts).map_err(|e| format!("ImpactVarieties: {e:?}"))?;
        let mut out_impacts = HashMap::new();
        for (i, e) in doc.elements.iter().enumerate() {
            if e.name != "ImpactVariety" {
                continue;
            }
            let infos: Vec<String> = children(&doc, i)
                .filter_map(|c| attr(c, "collisionInfo").map(str::to_owned))
                .take(3)
                .collect();
            if let (Some(name), Ok(three)) = (attr(e, "name"), <[String; 3]>::try_from(infos)) {
                out_impacts.insert(name.to_owned(), three);
            }
        }
        Ok(Self { sounds, varieties: out_varieties, infos: out_infos, impacts: out_impacts })
    }
}

/// One contact of a car this tick.
#[derive(Clone, Copy, Debug)]
pub struct Contact {
    pub point: [f32; 3],
    /// Solved contact +0x3C.
    pub impact: f32,
}

/// What the listener needs of the car.
#[derive(Clone, Copy, Debug)]
pub struct Car {
    /// Physical pose, columns: x, y, z axes and position (`Runtime::pose`).
    pub pose: [f32; 16],
    /// Body offset (vehicle +0x1630): model coordinates = body − offset.
    pub offset: [f32; 3],
    pub mass: f32,
    /// Lowest and highest `collision_main` y (description +0x1B28/+0x1B54,
    /// body +0xED68/+0xED94).
    pub bottom: f32,
    pub roof: f32,
    pub player: bool,
}

/// A one-shot to start.
#[derive(Clone, Debug, PartialEq)]
pub struct Play {
    pub variety: String,
    pub volume_db: f32,
    pub pitch: f32,
    /// World position of the emitter.
    pub position: [f32; 3],
    pub group: usize,
}

/// The primary impact 4559C0 selects for a contact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    FrontBack,
    Sides,
    Roof,
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn normalize(a: [f32; 3]) -> [f32; 3] {
    let l = dot(a, a).sqrt();
    if l > 0. { a.map(|x| x / l) } else { a }
}

/// 4559C0: roof when the car-frame height is above roof − 0.7 (0xCB0834),
/// nothing for a contact below −bottom; otherwise, more than 0.5 m from the
/// car's origin, front/back when the direction is within 0.8 of the car's
/// longitudinal axis (0xCB0F24, 0xCB51B4), else the sides.
pub fn classify(car: &Car, point: [f32; 3]) -> Option<Side> {
    let p = car.pose;
    let axis = |k: usize| [p[4 * k], p[4 * k + 1], p[4 * k + 2]];
    let d = [point[0] - p[12], point[1] - p[13], point[2] - p[14]];
    // Car frame (422200), in the model coordinates of the description [I].
    let local_y = dot(d, axis(1)) - car.offset[1];
    if local_y > car.roof - 0.7 {
        return Some(Side::Roof);
    }
    if -car.bottom > local_y {
        return None;
    }
    if dot(d, d).sqrt() <= 0.5 {
        return None;
    }
    let along = dot(normalize(axis(2)), normalize(d));
    Some(if along > 0.8 || along < -0.8 { Side::FrontBack } else { Side::Sides })
}

/// The collision listener's state: the per-variety cooldowns of 435B20
/// (manager +0x50 + idx·0x28, shared by every car).
pub struct Listener {
    bank: Arc<Bank>,
    cooldown: HashMap<String, (f32, f32)>,
    random: u32,
}

impl Listener {
    pub fn new(bank: Arc<Bank>) -> Self {
        Self { bank, cooldown: HashMap::new(), random: 0x2545_f491 }
    }

    fn rand(&mut self) -> u32 {
        // [I] The game's rand(); an LCG here.
        self.random = self.random.wrapping_mul(214_013).wrapping_add(2_531_011);
        (self.random >> 16) & 0x7fff
    }

    /// Advance the cooldown clocks by one tick.
    pub fn tick(&mut self, dt: f32) {
        for (elapsed, _) in self.cooldown.values_mut() {
            *elapsed += dt;
        }
    }

    fn one_shot(&mut self, info: &str, x: f32, position: [f32; 3], group: usize, cooldown: bool, out: &mut Vec<Play>) {
        let Some(curve) = self.bank.infos.get(info).cloned() else { return };
        let (volume_db, pitch) = curve.at(x);
        if volume_db < QUIETEST {
            return;
        }
        if cooldown {
            let elapsed = self.cooldown.get(&curve.variety).copied();
            if elapsed.is_some_and(|(e, limit)| e <= limit) {
                return;
            }
            let limit = (self.rand() % 3 + 1) as f32 * 0.1;
            self.cooldown.insert(curve.variety.clone(), (0., limit));
        }
        out.push(Play { variety: curve.variety, volume_db, pitch, position, group });
    }

    /// 4535A0: the ImpactVariety's soft, med or hard CollisionInfo by range;
    /// nothing below the soft range. Returns whether it played.
    fn impact(&mut self, variety: &str, x: f32, position: [f32; 3], group: usize, out: &mut Vec<Play>) {
        let Some(three) = self.bank.impacts.get(variety).cloned() else { return };
        let curves: Vec<Option<Curve>> = three.iter().map(|n| self.bank.infos.get(n).cloned()).collect();
        let Some(soft) = &curves[0] else { return };
        if x < soft.min_vel {
            return;
        }
        let pick = if soft.contains(x) {
            0
        } else if curves[1].as_ref().is_some_and(|c| c.contains(x)) {
            1
        } else {
            2
        };
        self.one_shot(&three[pick], x, position, group, true, out);
    }

    /// One car's contacts of this tick (4563F0 per contact).
    pub fn contacts(&mut self, car: &Car, contacts: &[Contact], out: &mut Vec<Play>) {
        let group = if car.player { PANEL_COLL } else { AI };
        for c in contacts {
            if car.mass <= 0. {
                break;
            }
            let x = 10. * c.impact / car.mass;
            if x <= 0. || !x.is_finite() {
                continue;
            }
            let variety = match classify(car, c.point) {
                Some(Side::FrontBack) => "PrimaryFrontBack",
                Some(Side::Sides) => "PrimarySides",
                Some(Side::Roof) => "PrimaryRoof",
                None => continue,
            };
            self.impact(variety, x, c.point, group, out);
            // Sweeteners, fire-and-forget without cooldown (44F010).
            if x >= 40. {
                self.sweetener("HighSweetenerInfo", "HighSweeteners", x, c.point, group, out);
            }
            if x >= 15. {
                self.sweetener("LowSweetenerInfo", "LowSweeteners", x, c.point, group, out);
            }
        }
    }

    fn sweetener(&mut self, info: &str, variety: &str, x: f32, position: [f32; 3], group: usize, out: &mut Vec<Play>) {
        let Some(curve) = self.bank.infos.get(info) else { return };
        let (volume_db, pitch) = curve.at(x);
        if volume_db >= QUIETEST {
            out.push(Play { variety: variety.to_owned(), volume_db, pitch, position, group });
        }
    }
}

/// A playing one-shot.
#[derive(Clone, Copy)]
struct Voice {
    sound: usize,
    position: f64,
    step: f64,
    gain: f32,
}

/// Mixes collision one-shots at `rate`; gains include the voice level, its
/// group and its distance term, fixed when the voice starts.
pub struct Mixer {
    bank: Arc<Bank>,
    rate: u32,
    voices: Vec<Voice>,
    random: u32,
    last: HashMap<String, usize>,
}

/// One-shots playing at once at most [I: no cap found in 435B20; bounded here].
pub const VOICES: usize = 32;

impl Mixer {
    pub fn new(bank: Arc<Bank>, rate: u32) -> Self {
        Self { bank, rate, voices: Vec::with_capacity(VOICES), random: 0x1357_9bdf, last: HashMap::new() }
    }

    /// Start `play` at `gain` (linear, group and distance included).
    pub fn start(&mut self, variety: &str, pitch: f32, gain: f32) -> bool {
        let Some(v) = self.bank.varieties.get(variety) else { return false };
        if v.samples.is_empty() || gain <= 0. || self.voices.len() >= VOICES {
            return false;
        }
        // 413B80: a uniform pick that avoids repeating the last sample [I: RNG].
        let previous = self.last.get(variety).copied();
        let choices: Vec<usize> = v.samples.iter().copied().filter(|s| Some(*s) != previous || v.samples.len() == 1).collect();
        self.random = self.random.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let sound = choices[(self.random >> 8) as usize % choices.len()];
        self.last.insert(variety.to_owned(), sound);
        let s = &self.bank.sounds[sound];
        self.voices.push(Voice {
            sound,
            position: 0.,
            step: f64::from(pitch) * f64::from(s.rate) / f64::from(self.rate),
            gain,
        });
        true
    }

    pub fn active(&self) -> usize {
        self.voices.len()
    }

    /// One mono sample.
    pub fn sample(&mut self) -> f32 {
        let mut out = 0.;
        let bank = &self.bank;
        self.voices.retain_mut(|v| {
            let s = &bank.sounds[v.sound];
            let frames = s.pcm.len() / s.channels;
            let i = v.position as usize;
            if i + 1 >= frames {
                return false;
            }
            let frac = (v.position - v.position.floor()) as f32;
            let a = s.pcm[i * s.channels];
            let b = s.pcm[(i + 1) * s.channels];
            out += (a + (b - a) * frac) * v.gain;
            v.position += v.step;
            true
        });
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn car() -> Car {
        let mut pose = [0.; 16];
        pose[0] = 1.;
        pose[5] = 1.;
        pose[10] = 1.;
        pose[15] = 1.;
        Car { pose, offset: [0.; 3], mass: 1200., bottom: 0.1, roof: 1.4, player: true }
    }

    #[test]
    fn contacts_are_classified_like_4559c0() {
        let c = car();
        assert_eq!(classify(&c, [0., 0.4, 2.]), Some(Side::FrontBack));
        assert_eq!(classify(&c, [0., 0.4, -2.]), Some(Side::FrontBack));
        assert_eq!(classify(&c, [0.9, 0.4, 0.3]), Some(Side::Sides));
        assert_eq!(classify(&c, [0., 0.8, 0.]), Some(Side::Roof));
        // Below −bottom: the floor makes no sound.
        assert_eq!(classify(&c, [0.5, -0.2, 1.]), None);
        // Within 0.5 m of the origin and not roof/floor: none.
        assert_eq!(classify(&c, [0.2, 0.3, 0.2]), None);
    }

    #[test]
    fn curves_interpolate_and_clamp() {
        let c = Curve {
            variety: "v".into(),
            min_vel: 3.,
            max_vel: 10.,
            min_vol: -3.,
            max_vol: 0.,
            min_pitch: 0.85,
            max_pitch: 1.1,
        };
        assert_eq!(c.at(3.), (-3., 0.85));
        assert_eq!(c.at(100.), (0., 1.1));
        let (v, p) = c.at(6.5);
        assert!((v + 1.5).abs() < 1e-6 && (p - 0.975).abs() < 1e-6);
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn shipped_collision_data_and_listener() {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let bank = Bank::load(Path::new(&root)).unwrap();
        assert_eq!(bank.sounds.len(), 307);
        assert_eq!(
            bank.impacts["PrimaryFrontBack"],
            ["carFBSoft".to_string(), "carFBMed".into(), "carFBHard".into()]
        );
        let soft = &bank.infos["carFBSoft"];
        assert_eq!((soft.min_vel, soft.max_vel, soft.min_vol, soft.max_vol), (3., 10., -3., 0.));
        assert_eq!(bank.infos["carBotSoft"].variety, "carLandSoft");
        let fb = &bank.varieties["carFBHard"];
        assert_eq!((fb.min_distance, fb.max_distance, fb.samples.len()), (4., 40., 9));
        let mut listener = Listener::new(bank.clone());
        let c = car();
        let mut out = Vec::new();
        // x = 10·impact/mass = 45: hard front hit, both sweeteners.
        listener.contacts(&c, &[Contact { point: [0., 0.4, 2.], impact: 5400. }], &mut out);
        let names: Vec<&str> = out.iter().map(|p| p.variety.as_str()).collect();
        assert_eq!(names, ["carFBHard", "HighSweeteners", "LowSweeteners"]);
        assert!(out.iter().all(|p| p.group == PANEL_COLL));
        // The same variety again within the cooldown is skipped.
        out.clear();
        listener.contacts(&c, &[Contact { point: [0., 0.4, 2.], impact: 5400. }], &mut out);
        assert_eq!(out.iter().filter(|p| p.variety == "carFBHard").count(), 0);
        for _ in 0..25 {
            listener.tick(0.016);
        }
        out.clear();
        listener.contacts(&c, &[Contact { point: [0., 0.4, 2.], impact: 5400. }], &mut out);
        assert_eq!(out[0].variety, "carFBHard");
        // Too soft: nothing.
        out.clear();
        listener.contacts(&c, &[Contact { point: [0.9, 0.4, 0.3], impact: 100. }], &mut out);
        assert!(out.is_empty());
        let mut mixer = Mixer::new(bank, 48_000);
        assert!(mixer.start("carFBHard", 1., 0.5));
        let peak = (0..48_000).map(|_| mixer.sample().abs()).fold(0f32, f32::max);
        assert!(peak > 0.01 && peak < 1.);
    }
}
