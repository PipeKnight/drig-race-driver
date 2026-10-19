//! Car collision sounds (docs/AUDIO_RESEARCH.md §7.3 and §9): the original's
//! per-contact listener 0x4563F0 turns each solved car contact into an
//! intensity `x = 10·impact/mass` and 0x4559C0 picks a primary impact
//! (front/back, side, roof; nothing for the floor) whose ImpactVariety curve
//! (soft/med/hard CollisionInfos) gives one one-shot from `car_collisions`
//! (0x4535A0), plus the low/high sweeteners of hard hits. Presentation only.
use crate::{
    body_impacts::Impact,
    hud_audio::{Sound, wip_sound},
    source_vehicle::multi::Contacted,
};
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
    /// AudioMaterial name → its preset's `carscrapeColInfo` (45BC00; the
    /// AudioMaterial loop reads only `preset`, TYRE_AUDIO_RESEARCH §1).
    pub car_scrape: HashMap<String, String>,
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
            &read(&audio.join("audioMaterials.bin"))?,
        )?))
    }

    pub fn parse(
        dictionary: &[u8],
        data: &[u8],
        varieties: &[u8],
        infos: &[u8],
        impacts: &[u8],
        materials: &[u8],
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
        // audioMaterials: presets name the carscrape info, materials a preset;
        // the first material of a name wins (4361C0).
        let doc = binxml::read(materials).map_err(|e| format!("audioMaterials: {e:?}"))?;
        let presets: HashMap<&str, &str> = doc
            .elements
            .iter()
            .filter(|e| e.name == "Preset")
            .filter_map(|e| Some((attr(e, "name")?, attr(e, "carscrapeColInfo")?)))
            .collect();
        let mut car_scrape = HashMap::new();
        for e in doc.elements.iter().filter(|e| e.name == "AudioMaterial") {
            if let (Some(name), Some(info)) = (attr(e, "name"), attr(e, "preset").and_then(|p| presets.get(p))) {
                car_scrape.entry(name.to_owned()).or_insert_with(|| (*info).to_owned());
            }
        }
        Ok(Self { sounds, varieties: out_varieties, infos: out_infos, impacts: out_impacts, car_scrape })
    }
}

/// One contact of a car this tick.
#[derive(Clone, Debug)]
pub struct Contact {
    pub point: [f32; 3],
    /// Solved contact +0x3C.
    pub impact: f32,
    /// AudioMaterial name of the other side (436170 on the contact's other
    /// material word); `None` when no AudioMaterial matches.
    pub surface: Option<String>,
}

/// What the listener needs of the car.
#[derive(Clone, Copy, Debug)]
pub struct Car {
    /// Physical pose, columns: x, y, z axes and position (`Runtime::pose`).
    pub pose: [f32; 16],
    pub mass: f32,
    /// Lowest and highest `collision_main` y (description +0x1B28/+0x1B54,
    /// body +0xED68/+0xED94).
    pub bottom: f32,
    pub roof: f32,
    pub player: bool,
    /// Linear velocity of the physical body (audio car +0xA480).
    pub velocity: [f32; 3],
    /// A69930: the body's impulse accumulator (+0x240) when its contact
    /// flag (+0x40C) is set this step, else `None`.
    pub impulse: Option<[f32; 3]>,
}

/// A car model's description heights: the lowest and highest
/// `collision_main` y (the values 0x9E2030 stores at description
/// +0x1B28/+0x1B54).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Heights {
    pub bottom: f32,
    pub roof: f32,
}

impl Heights {
    pub fn of(root: &Path, model: &crate::car::Car) -> Result<Self, Box<dyn std::error::Error>> {
        let bytes = std::fs::read(root.join(model.file(".ned")))?;
        let doc = binxml::read(&bytes)?;
        let mesh = grid_formats::car_collision::read(&doc)?;
        Ok(Self { bottom: mesh.bounds.min[1], roof: mesh.bounds.max[1] })
    }
}

impl Car {
    /// What the listener needs of `runtime`'s car this tick, with its solved
    /// contact records `impacts`.
    pub fn of(runtime: &crate::source_vehicle::Runtime, heights: Heights, player: bool, impacts: &[Impact]) -> Self {
        let velocity = runtime.velocity();
        // A69930: the body's impulse accumulator (+0x240, B0FC00 adds each
        // solved contact's +3C times its normal) when a contact flagged it
        // (+0x40C) [I: summed here from the solved records of the car].
        let impulse = (!impacts.is_empty())
            .then(|| impacts.iter().fold([0.; 3], |a, x| std::array::from_fn(|j| a[j] + x.impact * x.normal[j])));
        Self {
            pose: runtime.pose(),
            mass: f32::from_bits(runtime.state.car.owner.physical[0x3f0 / 4]),
            bottom: heights.bottom,
            roof: heights.roof,
            player,
            velocity: [velocity[0], velocity[1], velocity[2]],
            impulse,
        }
    }
}

/// The listener's contacts of one car: the AudioMaterial name of what each
/// touched is looked up only for contacts that can sound (not the floor):
/// the packed track triangle under it, CAR_ for another car; the boundary
/// sheet and route props are taken as WALL [I: their per-triangle/template
/// materials are not in the contact; every such AudioMaterial gives
/// CarScrapeSideRoad].
pub fn contacts_of(runtime: &crate::source_vehicle::Runtime, car: &Car, impacts: &[(Impact, Contacted)]) -> Vec<Contact> {
    impacts
        .iter()
        .map(|(x, contacted)| Contact {
            point: x.point,
            impact: x.impact,
            surface: classify(car, x.point).and_then(|_| match contacted {
                Contacted::Track => runtime.track_surface_at(x.point, x.normal).map(str::to_owned),
                Contacted::Car(_) => Some("CAR_".into()),
                Contacted::Prop(_) | Contacted::Static => Some("WALL".into()),
            }),
        })
        .collect()
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
    classify_mode(car, point).map(|(side, _)| side)
}

/// [`classify`] with the scrape mode 4559C0 passes to 453710: sides 0 when
/// the contact is in the front half (longitudinal cosine > 0), else 1; roof
/// 2; front 3; back 4.
pub fn classify_mode(car: &Car, point: [f32; 3]) -> Option<(Side, u8)> {
    let p = car.pose;
    let axis = |k: usize| [p[4 * k], p[4 * k + 1], p[4 * k + 2]];
    let d = [point[0] - p[12], point[1] - p[13], point[2] - p[14]];
    // Car frame (422200) of the audio pose, compared as it is with the
    // description heights: 4559C0 converts nothing (F490; F479 subtracted
    // the body offset, which put door-height car contacts on the roof).
    let local_y = dot(d, axis(1));
    if local_y > car.roof - 0.7 {
        return Some((Side::Roof, 2));
    }
    if -car.bottom > local_y {
        return None;
    }
    if dot(d, d).sqrt() <= 0.5 {
        return None;
    }
    let along = dot(normalize(axis(2)), normalize(d));
    Some(if along > 0.8 {
        (Side::FrontBack, 3)
    } else if along < -0.8 {
        (Side::FrontBack, 4)
    } else {
        (Side::Sides, if along > 0. { 0 } else { 1 })
    })
}

/// The collision listener's state: the per-variety cooldowns of 435B20
/// (manager +0x50 + idx·0x28, shared by every car).
pub struct Listener {
    bank: Arc<Bank>,
    cooldown: HashMap<String, (f32, f32)>,
    random: u32,
    /// The scrape loop of each car (audio car +0xA300..+0xA334).
    scrapes: Vec<Scrape>,
}

/// The roof scrape info (4559C0 passes the car's fixed roof entry instead of
/// the surface's; [I] taken by name, the only roof scrape CollisionInfo).
pub const ROOF_SCRAPE: &str = "CarScrapeRoof";

/// One car's scrape loop (453710, per frame 458780, levels 448810).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scrape {
    /// +0xA324: the CollisionInfo the voice started on; never changed later.
    pub info: Option<String>,
    /// +0xA328: envelope.
    pub envelope: f32,
    /// +0xA334: the last contact's mode (see [`classify_mode`]).
    pub mode: u8,
    /// +0xA31C: smoothed level (dB).
    pub level_db: f32,
    /// +0xA318: pitch before the setter's clamp.
    pub pitch: f32,
    /// +0xA300: emitter offset from the car (the contact for modes 3/4).
    pub offset: [f32; 3],
}

/// What the scrape voice of a car plays this tick.
#[derive(Clone, Debug, PartialEq)]
pub struct ScrapeVoice {
    pub variety: String,
    /// Clamped by the setters: [-48, 0] dB, [0.5, 2].
    pub volume_db: f32,
    pub pitch: f32,
    pub offset: [f32; 3],
    pub group: usize,
}

fn length(a: [f32; 3]) -> f32 {
    dot(a, a).sqrt()
}

impl Listener {
    pub fn new(bank: Arc<Bank>) -> Self {
        Self { bank, cooldown: HashMap::new(), random: 0x2545_f491, scrapes: Vec::new() }
    }

    pub fn scrape(&self, car: usize) -> Option<&Scrape> {
        self.scrapes.get(car)
    }

    fn scrape_mut(&mut self, car: usize) -> &mut Scrape {
        if self.scrapes.len() <= car {
            self.scrapes.resize(car + 1, Scrape::default());
        }
        &mut self.scrapes[car]
    }

    /// 453710 for one contact: the gates (a body contact this step, the
    /// horizontal speed of the mode, x > 1), then start the voice on `info`
    /// or refresh its envelope, position and levels.
    fn scrape_contact(&mut self, k: usize, car: &Car, info: &str, x: f32, offset: [f32; 3], mode: u8) {
        let threshold = match mode {
            0 | 1 => 0.3,
            2 => 1.,
            _ => 0.1,
        };
        let horizontal = car.velocity[0].hypot(car.velocity[2]);
        if car.impulse.is_none() || horizontal < threshold || x <= 1. || !self.bank.infos.contains_key(info) {
            return;
        }
        let scrape = self.scrape_mut(k);
        if scrape.info.is_none() {
            scrape.info = Some(info.to_owned());
            scrape.envelope = 0.;
            scrape.offset = offset;
            scrape.mode = mode;
            return;
        }
        if scrape.envelope < 1. {
            scrape.envelope = if horizontal > 15. { 1. } else { scrape.envelope + 0.1 };
        }
        scrape.offset = offset;
        scrape.mode = mode;
        Self::place(scrape, car);
        self.levels(k, car);
    }

    /// The mode's emitter: sides 1.5 m along the car's x axis (+ in the
    /// front half, - in the back half), the roof its y axis (unscaled),
    /// front/back the contact itself.
    fn place(scrape: &mut Scrape, car: &Car) {
        let p = car.pose;
        let axis = |c: usize, s: f32| [p[4 * c] * s, p[4 * c + 1] * s, p[4 * c + 2] * s];
        match scrape.mode {
            0 => scrape.offset = axis(0, 1.5),
            1 => scrape.offset = axis(0, -1.5),
            2 => scrape.offset = axis(1, 1.),
            _ => {}
        }
    }

    /// 448810 (the player's car [O]; AI cars here too [I: their level is
    /// not traced]): pitch from the speed, level lin2dB(|v|*0.04*t) with a
    /// 5 % -24 dB drop and -(rand % 9) dB jitter, smoothed by 1/3.
    fn levels(&mut self, k: usize, car: &Car) {
        let Some(info) = self.scrapes[k].info.as_ref().and_then(|n| self.bank.infos.get(n)) else { return };
        // t: the envelope's place between the info's minVol and maxVol (+5C,
        // +60), 1 above both; for the shipped `scraping` preset (-24..-18
        // dB) the envelope (0..1) always gives 1.
        let (lo, hi) = (info.min_vol, info.max_vol);
        let g = self.scrapes[k].envelope;
        let t = if hi <= lo {
            if hi < g {
                if g < lo { (g - hi) / (lo - hi) } else { 1. }
            } else {
                0.
            }
        } else if lo < g {
            if g < hi { (g - lo) / (hi - lo) } else { 1. }
        } else {
            0.
        };
        let speed = length(car.velocity);
        let s = speed / 45.;
        let pitch = if s > 0.8 {
            if s < 1.5 { (s - 0.8) / 0.7 } else { 1. }
        } else {
            0.
        };
        let mut target = crate::tyre_audio::lin2db(speed * 0.04 * t);
        if self.rand() % 100 < 5 {
            target -= 24.;
        }
        target -= (self.rand() % 9) as f32;
        let scrape = &mut self.scrapes[k];
        scrape.pitch = pitch;
        scrape.level_db += (target - scrape.level_db) / 3.;
    }

    /// 458780, once per vehicle tick after the contacts: at an envelope of
    /// 0.1 or less the voice is silent (-48 dB); otherwise the envelope
    /// halves while the car is slower than 0.5 m/s or its impulse
    /// accumulator is under 5, and the position and levels are refreshed.
    pub fn scrape_frame(&mut self, k: usize, car: &Car) -> Option<ScrapeVoice> {
        let info = self.scrapes.get(k)?.info.clone()?;
        let variety = self.bank.infos.get(&info)?.variety.clone();
        let group = if car.player { PANEL_COLL } else { AI };
        let scrape = &mut self.scrapes[k];
        if scrape.envelope <= 0.1 {
            return Some(ScrapeVoice {
                variety,
                volume_db: -48.,
                pitch: scrape.pitch.clamp(0.5, 2.),
                offset: scrape.offset,
                group,
            });
        }
        let impulse = car.impulse.map_or(0., length);
        if length(car.velocity) < 0.5 || impulse < 5. {
            scrape.envelope *= 0.5;
        }
        Self::place(scrape, car);
        self.levels(k, car);
        let scrape = &self.scrapes[k];
        Some(ScrapeVoice {
            variety,
            volume_db: scrape.level_db.clamp(-48., 0.),
            pitch: scrape.pitch.clamp(0.5, 2.),
            offset: scrape.offset,
            group,
        })
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
        if cooldown && self.cooling(&curve.variety) {
            return;
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

    /// Car `k`'s contacts of this tick (4563F0 per contact).
    pub fn contacts(&mut self, k: usize, car: &Car, contacts: &[Contact], out: &mut Vec<Play>) {
        let group = if car.player { PANEL_COLL } else { AI };
        for c in contacts {
            if car.mass <= 0. {
                break;
            }
            let x = 10. * c.impact / car.mass;
            if x <= 0. || !x.is_finite() {
                continue;
            }
            let Some((side, mode)) = classify_mode(car, c.point) else { continue };
            let variety = match side {
                Side::FrontBack => "PrimaryFrontBack",
                Side::Sides => "PrimarySides",
                Side::Roof => "PrimaryRoof",
            };
            // 453710 before the impact, only when the other side has an
            // AudioMaterial: its preset's carscrape info, the roof's own.
            if let Some(material) = &c.surface
                && let Some(info) = if side == Side::Roof {
                    Some(ROOF_SCRAPE.to_owned())
                } else {
                    self.bank.car_scrape.get(material).cloned()
                }
            {
                let p = car.pose;
                let offset = [c.point[0] - p[12], c.point[1] - p[13], c.point[2] - p[14]];
                self.scrape_contact(k, car, &info, x, offset, mode);
            }
            self.impact(variety, x, c.point, group, out);
            // Sweeteners from the info's minVel (x >= 40 high, >= 15 low),
            // played through 44F010 -> 435940 with its cooldown flag set
            // (4535A0 passes 1): the same per-variety cooldown as impacts,
            // no -42 dB gate.
            if x >= 40. {
                self.sweetener("HighSweetenerInfo", x, c.point, group, out);
            }
            if x >= 15. {
                self.sweetener("LowSweetenerInfo", x, c.point, group, out);
            }
        }
    }

    fn sweetener(&mut self, info: &str, x: f32, position: [f32; 3], group: usize, out: &mut Vec<Play>) {
        let Some(curve) = self.bank.infos.get(info).cloned() else { return };
        if self.cooling(&curve.variety) {
            return;
        }
        let (volume_db, pitch) = curve.at(x);
        out.push(Play { variety: curve.variety, volume_db, pitch, position, group });
    }

    /// 435940/435B20: a variety still inside its cooldown is skipped;
    /// otherwise its timer restarts with T = (rand % 3 + 1)·0.1 s.
    fn cooling(&mut self, variety: &str) -> bool {
        if self.cooldown.get(variety).is_some_and(|&(e, limit)| e <= limit) {
            return true;
        }
        let limit = (self.rand() % 3 + 1) as f32 * 0.1;
        self.cooldown.insert(variety.to_owned(), (0., limit));
        false
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
    /// One looping scrape voice per car; its sample is picked when it starts.
    loops: Vec<Option<Loop>>,
}

/// A looping scrape voice: gain and pitch follow the per-tick targets.
#[derive(Clone, Copy)]
struct Loop {
    sound: usize,
    position: f64,
    step: f64,
    target_step: f64,
    gain: f32,
    target: f32,
}

/// Per-sample approach of a loop's gain and pitch to the tick's targets,
/// about 10 ms at 48 kHz [I: smoothing between ticks only].
const LOOP_GLIDE: f32 = 0.002;

/// One-shots playing at once at most [I: no cap found in 435B20; bounded here].
pub const VOICES: usize = 32;

impl Mixer {
    pub fn new(bank: Arc<Bank>, rate: u32) -> Self {
        Self {
            bank,
            rate,
            voices: Vec::with_capacity(VOICES),
            random: 0x1357_9bdf,
            last: HashMap::new(),
            loops: Vec::new(),
        }
    }

    fn pick(&mut self, variety: &str) -> Option<usize> {
        let v = self.bank.varieties.get(variety)?;
        if v.samples.is_empty() {
            return None;
        }
        // 413B80: a uniform pick that avoids repeating the last sample [I: RNG].
        let previous = self.last.get(variety).copied();
        let choices: Vec<usize> = v.samples.iter().copied().filter(|s| Some(*s) != previous || v.samples.len() == 1).collect();
        self.random = self.random.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let sound = choices[(self.random >> 8) as usize % choices.len()];
        self.last.insert(variety.to_owned(), sound);
        Some(sound)
    }

    /// Car `car`'s scrape loop this tick: started on its first call (one
    /// sample of `variety`, looped), then gain and pitch follow.
    pub fn scrape(&mut self, car: usize, variety: &str, pitch: f32, gain: f32) {
        if self.loops.len() <= car {
            self.loops.resize(car + 1, None);
        }
        if self.loops[car].is_none() {
            let Some(sound) = self.pick(variety) else { return };
            let step = f64::from(pitch) * f64::from(self.bank.sounds[sound].rate) / f64::from(self.rate);
            self.loops[car] = Some(Loop { sound, position: 0., step, target_step: step, gain: 0., target: 0. });
        }
        if let Some(l) = &mut self.loops[car] {
            l.target = gain.max(0.);
            l.target_step = f64::from(pitch) * f64::from(self.bank.sounds[l.sound].rate) / f64::from(self.rate);
        }
    }

    pub fn loops(&self) -> usize {
        self.loops.iter().flatten().count()
    }

    /// Start `play` at `gain` (linear, group and distance included).
    pub fn start(&mut self, variety: &str, pitch: f32, gain: f32) -> bool {
        if gain <= 0. || self.voices.len() >= VOICES {
            return false;
        }
        let Some(sound) = self.pick(variety) else { return false };
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
        for l in self.loops.iter_mut().flatten() {
            let s = &bank.sounds[l.sound];
            let frames = s.pcm.len() / s.channels;
            if frames < 2 {
                continue;
            }
            l.gain += (l.target - l.gain) * LOOP_GLIDE;
            l.step += (l.target_step - l.step) * f64::from(LOOP_GLIDE);
            let i = l.position as usize % frames;
            let frac = (l.position - l.position.floor()) as f32;
            let a = s.pcm[i * s.channels];
            let b = s.pcm[(i + 1) % frames * s.channels];
            out += (a + (b - a) * frac) * l.gain;
            l.position = (l.position + l.step) % frames as f64;
        }
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
        Car {
            pose,
            mass: 1200.,
            bottom: 0.1,
            roof: 1.4,
            player: true,
            velocity: [0., 0., 10.],
            impulse: Some([0., 0., 900.]),
        }
    }

    fn contact(point: [f32; 3], impact: f32, surface: Option<&str>) -> Contact {
        Contact { point, impact, surface: surface.map(str::to_owned) }
    }

    fn scrape_bank() -> Arc<Bank> {
        let curve = |variety: &str, lo: f32, hi: f32| Curve {
            variety: variety.into(),
            min_vel: 0.,
            max_vel: 1.,
            min_vol: lo,
            max_vol: hi,
            min_pitch: 0.85,
            max_pitch: 1.2,
        };
        Arc::new(Bank {
            sounds: Vec::new(),
            varieties: HashMap::new(),
            infos: HashMap::from([
                ("CarScrapeSideRoad".into(), curve("ScrapeSideRoad", -24., -18.)),
                ("CarScrapeRoof".into(), curve("ScrapeRoofRoad", -24., -18.)),
            ]),
            impacts: HashMap::new(),
            car_scrape: HashMap::from([("TARMAC".into(), "CarScrapeSideRoad".into())]),
        })
    }

    #[test]
    fn scrapes_follow_453710_and_458780() {
        let bank = scrape_bank();
        let mut listener = Listener::new(bank);
        let mut c = car();
        let mut out = Vec::new();
        // Side contact in the front half: mode 0. No AudioMaterial: no scrape.
        let side = [0.9, 0.4, 0.3];
        listener.contacts(0, &c, &[contact(side, 1200., None)], &mut out);
        assert!(listener.scrape(0).is_none_or(|s| s.info.is_none()));
        // x = 10 > 1 on TARMAC: the voice starts with a zero envelope.
        listener.contacts(0, &c, &[contact(side, 1200., Some("TARMAC"))], &mut out);
        let s = listener.scrape(0).unwrap();
        assert_eq!((s.info.as_deref(), s.envelope, s.mode), (Some("CarScrapeSideRoad"), 0., 0));
        let v = listener.scrape_frame(0, &c).unwrap();
        assert_eq!((v.variety.as_str(), v.volume_db, v.group), ("ScrapeSideRoad", -48., PANEL_COLL));
        // Under 15 m/s each contact adds 0.1; above it the envelope is 1.
        listener.contacts(0, &c, &[contact(side, 1200., Some("TARMAC"))], &mut out);
        assert!((listener.scrape(0).unwrap().envelope - 0.1).abs() < 1e-6);
        c.velocity = [0., 0., 30.];
        listener.contacts(0, &c, &[contact(side, 1200., Some("TARMAC"))], &mut out);
        assert_eq!(listener.scrape(0).unwrap().envelope, 1.);
        // Mode 0: the emitter sits 1.5 m along the car's x axis.
        assert_eq!(listener.scrape(0).unwrap().offset, [1.5, 0., 0.]);
        for _ in 0..40 {
            listener.scrape_frame(0, &c);
        }
        let v = listener.scrape_frame(0, &c).unwrap();
        // 30 m/s: target lin2dB(1.2) = 0 dB minus up to 8 dB of jitter
        // (and the rare -24 drop), pitch (30/45 - 0.8)/0.7 < 0 -> 0 -> 0.5.
        assert!(v.volume_db <= 0. && v.volume_db > -24., "{v:?}");
        assert_eq!(v.pitch, 0.5);
        // Back half: mode 1, -1.5 along x. The info never changes.
        listener.contacts(0, &c, &[contact([0.9, 0.4, -0.3], 1200., Some("TARMAC"))], &mut out);
        assert_eq!(listener.scrape(0).unwrap().offset, [-1.5, 0., 0.]);
        // Slow or without an impulse the envelope halves each tick.
        c.impulse = None;
        let before = listener.scrape(0).unwrap().envelope;
        listener.scrape_frame(0, &c);
        assert_eq!(listener.scrape(0).unwrap().envelope, before * 0.5);
        // Without a body contact flag 453710 does nothing.
        listener.contacts(0, &c, &[contact(side, 1200., Some("TARMAC"))], &mut out);
        assert_eq!(listener.scrape(0).unwrap().envelope, before * 0.5);
        // Too slow for the side gate (0.3 m/s horizontal): nothing.
        c.impulse = Some([0., 0., 900.]);
        c.velocity = [0.2, 3., 0.];
        listener.contacts(0, &c, &[contact(side, 1200., Some("TARMAC"))], &mut out);
        assert_eq!(listener.scrape(0).unwrap().envelope, before * 0.5);
        // An AI car has its own voice on the AI group.
        c.player = false;
        c.velocity = [0., 0., 20.];
        listener.contacts(3, &c, &[contact([0., 0.8, 0.], 1200., Some("TARMAC"))], &mut out);
        let s = listener.scrape(3).unwrap();
        assert_eq!((s.info.as_deref(), s.mode), (Some(ROOF_SCRAPE), 2));
        assert_eq!(listener.scrape_frame(3, &c).unwrap().group, AI);
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
        // x = 10·impact/mass = 45: hard front hit and the sweeteners. Both
        // sweetener infos name the variety "dummy" (case-insensitively the
        // sample-less env_swooshzing Dummy), so they share one cooldown and
        // play nothing: car_collisions has no such variety (F490).
        listener.contacts(0, &c, &[contact([0., 0.4, 2.], 5400., None)], &mut out);
        let names: Vec<&str> = out.iter().map(|p| p.variety.as_str()).collect();
        assert_eq!(names, ["carFBHard", "dummy"]);
        assert_eq!(bank.infos["LowSweetenerInfo"].variety, "dummy");
        assert!(!bank.varieties.contains_key("dummy"));
        assert!(out.iter().all(|p| p.group == PANEL_COLL));
        // The same variety again within the cooldown is skipped.
        out.clear();
        listener.contacts(0, &c, &[contact([0., 0.4, 2.], 5400., None)], &mut out);
        assert_eq!(out.iter().filter(|p| p.variety == "carFBHard").count(), 0);
        for _ in 0..25 {
            listener.tick(0.016);
        }
        out.clear();
        listener.contacts(0, &c, &[contact([0., 0.4, 2.], 5400., None)], &mut out);
        assert_eq!(out[0].variety, "carFBHard");
        // Too soft: nothing.
        out.clear();
        listener.contacts(0, &c, &[contact([0.9, 0.4, 0.3], 100., None)], &mut out);
        assert!(out.is_empty());
        // Scrapes: every AudioMaterial preset names a carscrape info; only
        // the `grass` preset gives SideOther (per-material attributes, such
        // as WALL's, are not read).
        assert_eq!(bank.car_scrape["TARMAC"], "CarScrapeSideRoad");
        assert_eq!(bank.car_scrape["WALL"], "CarScrapeSideRoad");
        assert_eq!(bank.car_scrape["GRASS"], "CarScrapeSideOther");
        for info in ["CarScrapeSideRoad", "CarScrapeSideOther", ROOF_SCRAPE] {
            let curve = &bank.infos[info];
            assert_eq!((curve.min_vol, curve.max_vol), (-24., -18.), "{info}");
            assert!(!bank.varieties[&curve.variety].samples.is_empty(), "{info}");
        }
        let mut mixer = Mixer::new(bank, 48_000);
        assert!(mixer.start("carFBHard", 1., 0.5));
        assert!(!mixer.start("dummy", 1., 1.));
        let peak = (0..48_000).map(|_| mixer.sample().abs()).fold(0f32, f32::max);
        assert!(peak > 0.01 && peak < 1.);
        mixer.scrape(0, "ScrapeSideRoad", 0.5, 0.5);
        assert_eq!(mixer.loops(), 1);
        // The loop keeps playing past its sample's end.
        let late = (0..96_000).map(|_| mixer.sample().abs()).skip(72_000).fold(0f32, f32::max);
        assert!(late > 0.001 && late < 1., "{late}");
    }
}
