//! Player-car tyre/surface audio following docs/TYRE_AUDIO_RESEARCH.md: the
//! original 456640 chain (roll 455410, peel/slide 44F330, skid/scrub 44FE60)
//! driven per physics tick, with tables and looping PCM from the purchased
//! installation. The mixer, resampling and bus level are independent choices.
use crate::contact_surfaces::Surfaces;
use grid_formats::{audio_dictionary, binxml};
use std::{collections::HashMap, path::Path, sync::Arc};

pub const ROLL: usize = 0;
pub const PEEL: usize = 1;
pub const SLIDE: usize = 2;
pub const SKID: usize = 3;
pub const SCRUB: usize = 4;
pub const CATEGORIES: usize = 5;
const SILENT: f32 = -48.;

/// CollisionInfo curve (0x74-byte object, 45B700/459D30). Treble is unused by tyres.
#[derive(Clone, Debug, PartialEq)]
pub struct ColInfo {
    pub name: String,
    pub min_vel: f32,
    pub max_vel: f32,
    pub min_vol: f32,
    pub max_vol: f32,
    pub min_pitch: f32,
    pub max_pitch: f32,
    pub samples: Vec<usize>,
}
impl ColInfo {
    /// 44F160/44F010: clamped inverse lerp, dB-linear volume, linear pitch ratio.
    fn t(&self, x: f32) -> f32 {
        let span = self.max_vel - self.min_vel;
        if span == 0. {
            f32::from(u8::from(x >= self.max_vel))
        } else {
            ((x - self.min_vel) / span).clamp(0., 1.)
        }
    }
    pub fn lerp(&self, x: f32) -> (f32, f32) {
        let t = self.t(x);
        (
            self.min_vol + (self.max_vol - self.min_vol) * t,
            self.min_pitch + (self.max_pitch - self.min_pitch) * t,
        )
    }
}

/// 4138F0: linear amplitude to dB, floored at -48.
pub fn lin2db(x: f32) -> f32 {
    if x <= 0. {
        SILENT
    } else if x >= 1. {
        0.
    } else {
        (20. * x.log10()).max(SILENT)
    }
}

pub struct Bank {
    /// car_surfaces looped PCM16 normalized to [-1, 1), with descriptor rates.
    pub samples: Vec<Arc<[f32]>>,
    pub rates: Vec<u32>,
    pub infos: Vec<ColInfo>,
    /// AudioMaterial name -> preset categories (roll, peel, slide, skid, scrub).
    materials: HashMap<String, [Option<usize>; CATEGORIES]>,
}

fn attr<'a>(e: &binxml::Element<'a>, name: &str) -> Option<&'a str> {
    e.attributes
        .iter()
        .find(|(k, _)| *k == name)
        .map(|(_, v)| *v)
}
fn read(path: &Path, limit: usize) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let bytes = std::fs::read(path)?;
    if bytes.len() > limit {
        return Err("tyre audio source exceeds bound".into());
    }
    Ok(bytes)
}

impl Bank {
    pub fn load(root: &Path) -> Result<Arc<Self>, Box<dyn std::error::Error>> {
        let audio = root.join("audio");
        let dictionary_bytes = read(&audio.join("CMSamples.dic"), 16 << 20)?;
        let dictionary = audio_dictionary::read(&dictionary_bytes)?;
        let bank = dictionary
            .banks
            .iter()
            .find(|b| b.name == "car_surfaces")
            .ok_or("car_surfaces bank absent")?;
        let data = read(&audio.join("car_surfaces.WIP"), 64 << 20)?;
        let mut samples = Vec::new();
        let mut rates = Vec::new();
        let mut index = HashMap::new();
        for (i, s) in bank.samples.iter().enumerate() {
            let (pcm, _looped) = bank.wip_pcm16(i, &data)?;
            let pcm: Arc<[f32]> = pcm
                .as_chunks::<2>()
                .0
                .iter()
                .map(|w| f32::from(i16::from_le_bytes(*w)) / 32768.)
                .collect();
            if pcm.is_empty() || s.rate == 0 {
                return Err("empty or unrated surface sample".into());
            }
            index.insert(s.name.to_owned(), samples.len());
            samples.push(pcm);
            rates.push(u32::from(s.rate));
        }

        let varieties_bytes = read(&audio.join("audioVarieties.bin"), 16 << 20)?;
        let varieties = binxml::read(&varieties_bytes)?;
        let mut variety_samples: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, e) in varieties.elements.iter().enumerate() {
            if e.name != "AudioVariety" || attr(e, "bank") != Some("car_surfaces") {
                continue;
            }
            let name = attr(e, "name").ok_or("unnamed variety")?;
            let list = varieties
                .elements
                .iter()
                .filter(|s| {
                    s.name == "Sample"
                        && s.parent
                            .is_some_and(|p| varieties.elements[p].parent == Some(i))
                })
                .filter_map(|s| attr(s, "index").and_then(|n| index.get(n).copied()))
                .collect();
            variety_samples.insert(name.to_owned(), list);
        }

        let infos_bytes = read(&audio.join("CollisionInfos.bin"), 16 << 20)?;
        let infos_doc = binxml::read(&infos_bytes)?;
        let fields = [
            "minVel", "maxVel", "minVol", "maxVol", "minPitch", "maxPitch",
        ];
        let mut presets: HashMap<&str, [Option<f32>; 6]> = HashMap::new();
        for e in &infos_doc.elements {
            if e.name == "Preset"
                && let Some(name) = attr(e, "name")
            {
                let mut values = [None; 6];
                for (slot, field) in values.iter_mut().zip(fields) {
                    *slot = attr(e, field).map(str::parse).transpose()?;
                }
                presets.insert(name, values);
            }
        }
        let mut infos = Vec::new();
        let mut info_index = HashMap::new();
        for e in &infos_doc.elements {
            if e.name != "CollisionInfo" {
                continue;
            }
            let name = attr(e, "name").ok_or("unnamed CollisionInfo")?;
            // Values are copied from the named preset, then overridden (459D30).
            let mut values = attr(e, "preset")
                .and_then(|p| presets.get(p).copied())
                .unwrap_or([None; 6]);
            for (slot, field) in values.iter_mut().zip(fields) {
                if let Some(v) = attr(e, field) {
                    *slot = Some(v.parse()?);
                }
            }
            let v = values.map(|v| v.unwrap_or(0.));
            if !v.iter().all(|x| x.is_finite()) {
                return Err("nonfinite CollisionInfo".into());
            }
            info_index.entry(name.to_owned()).or_insert(infos.len());
            infos.push(ColInfo {
                name: name.to_owned(),
                min_vel: v[0],
                max_vel: v[1],
                min_vol: v[2],
                max_vol: v[3],
                min_pitch: v[4],
                max_pitch: v[5],
                samples: attr(e, "variety")
                    .and_then(|n| variety_samples.get(n).cloned())
                    .unwrap_or_default(),
            });
        }

        let materials_bytes = read(&audio.join("audioMaterials.bin"), 16 << 20)?;
        let materials_doc = binxml::read(&materials_bytes)?;
        let keys = [
            "rollingColInfo",
            "peelingColInfo",
            "slidingColInfo",
            "skiddingColInfo",
            "scrubbingColInfo",
        ];
        let mut material_presets = HashMap::new();
        for e in &materials_doc.elements {
            if e.name == "Preset"
                && let Some(name) = attr(e, "name")
            {
                let set = keys.map(|k| attr(e, k).and_then(|n| info_index.get(n).copied()));
                material_presets.insert(name, set);
            }
        }
        // 45C2B8..45C4F3 reads only name/preset/kickupMultiplier/damping, so
        // per-material ColInfo attributes are ignored; first name wins (4361C0).
        let mut materials = HashMap::new();
        for e in &materials_doc.elements {
            if e.name == "AudioMaterial"
                && let (Some(name), Some(preset)) = (attr(e, "name"), attr(e, "preset"))
                && let Some(set) = material_presets.get(preset)
            {
                materials.entry(name.to_owned()).or_insert(*set);
            }
        }
        Ok(Arc::new(Self {
            samples,
            rates,
            infos,
            materials,
        }))
    }
    pub fn material(&self, name: &str) -> Option<[Option<usize>; CATEGORIES]> {
        self.materials.get(name).copied()
    }
}

/// One voice target published to the mixer for a wheel/category.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Voice {
    pub sample: Option<usize>,
    /// Changes whenever playback restarts from the sample start.
    pub generation: u32,
    pub volume_db: f32,
    pub pitch: f32,
}

/// Original rand()-style draws, reproducible per wheel.
#[derive(Clone, Debug)]
struct Random(u32);
impl Random {
    fn next(&mut self, n: u32) -> u32 {
        self.0 = self.0.wrapping_mul(214_013).wrapping_add(2_531_011);
        ((self.0 >> 16) & 0x7fff) % n.max(1)
    }
}

#[derive(Clone, Debug)]
struct Wheel {
    material: Option<[Option<usize>; CATEGORIES]>,
    voices: [Voice; CATEGORIES],
    last_sample: [Option<usize>; CATEGORIES],
    active: [bool; CATEGORIES],
    spin: f32,
    skid: f32,
    scrub: f32,
    roll_volume: f32,
    slip_angle: f32,
    previous_displacement: f32,
    slide_jitter: f32,
    skid_jitter: f32,
    skid_wobble: f32,
    wobble_timer: f32,
    random: Random,
}

pub struct Driver {
    bank: Arc<Bank>,
    wheels: [Wheel; 4],
    smoothed_height: Option<f32>,
    /// 457BE0: the player car runs all four wheel objects with roll and scrub;
    /// other cars run wheel object 0 only, and 455410/scrub/W+17C are player-only.
    player: bool,
}

/// Read-only per-tick inputs (vehicle and physical words of the player car).
pub struct Inputs<'a> {
    pub vehicle: &'a [u32],
    pub physical: &'a [u32],
    pub surfaces: &'a Surfaces,
    pub delta: f32,
}

fn f(b: &[u32], o: usize) -> f32 {
    f32::from_bits(b[o / 4])
}

impl Driver {
    pub fn new(bank: Arc<Bank>) -> Self {
        Self {
            bank,
            wheels: std::array::from_fn(|i| Wheel {
                material: None,
                voices: [Voice {
                    volume_db: SILENT,
                    pitch: 1.,
                    ..Default::default()
                }; CATEGORIES],
                last_sample: [None; CATEGORIES],
                active: [false; CATEGORIES],
                spin: 0.01,
                skid: 0.,
                scrub: 0.,
                roll_volume: SILENT,
                slip_angle: 0.,
                previous_displacement: 0.,
                slide_jitter: 0.,
                skid_jitter: 0.,
                skid_wobble: 0.,
                wobble_timer: 0.,
                random: Random(0x5eed_0000 + i as u32),
            }),
            smoothed_height: None,
            player: true,
        }
    }
    /// An AI car's tyre audio: wheel object 0, peel/slide/skid.
    pub fn opponent(bank: Arc<Bank>) -> Self {
        Self {
            player: false,
            ..Self::new(bank)
        }
    }
    pub fn voices(&self) -> [[Voice; CATEGORIES]; 4] {
        self.wheels.each_ref().map(|w| w.voices)
    }

    /// 413B80: uniform choice rejecting an immediate repeat; two samples cycle.
    fn start(bank: &Bank, w: &mut Wheel, category: usize, info: usize) {
        let list = &bank.infos[info].samples;
        let sample = match list.len() {
            0 => None,
            1 => Some(list[0]),
            2 => {
                let next = w.last_sample[category]
                    .and_then(|s| list.iter().position(|&x| x == s))
                    .map_or(0, |p| (p + 1) % 2);
                Some(list[next])
            }
            n => {
                let mut pick = list[w.random.next(n as u32) as usize];
                if Some(pick) == w.last_sample[category] {
                    pick = list[(list.iter().position(|&x| x == pick).unwrap() + 1) % n];
                }
                Some(pick)
            }
        };
        w.last_sample[category] = sample;
        let voice = &mut w.voices[category];
        voice.sample = sample;
        voice.generation = voice.generation.wrapping_add(1);
        w.active[category] = sample.is_some();
    }
    fn stop(w: &mut Wheel, category: usize) {
        w.active[category] = false;
        w.voices[category].sample = None;
        w.voices[category].volume_db = SILENT;
    }

    pub fn tick(&mut self, input: &Inputs) {
        let dt = input.delta;
        let b = input.vehicle;
        let p = input.physical;
        let velocity = [f(p, 0x220), f(p, 0x224), f(p, 0x228)];
        let speed = velocity.iter().map(|x| x * x).sum::<f32>().sqrt();
        let pose: [f32; 16] = std::array::from_fn(|i| f(p, 0x1d0 + i * 4));
        let height = pose[13];
        let smoothed = self.smoothed_height.get_or_insert(height);
        *smoothed += 0.2 * (height - *smoothed);
        let bump = 0.125 * (height - *smoothed);
        let axis = [pose[8], pose[9], pose[10]];
        let axis_length = axis.iter().map(|x| x * x).sum::<f32>().sqrt();
        let slip = if speed > 1e-4 && axis_length > 1e-6 {
            let dot =
                velocity.iter().zip(axis).map(|(v, a)| v * a).sum::<f32>() / (speed * axis_length);
            1. - dot.abs().min(1.)
        } else {
            0.
        };
        let mass = f(b, 0x15a0);
        let front_fraction = f(b, 0xe78 + 0x154);
        let bank = self.bank.clone();
        let player = self.player;
        for (i, w) in self.wheels.iter_mut().enumerate() {
            if !player && i > 0 {
                break;
            }
            let o = 0x1990 + i * 0x150;
            let omega = f(b, o + 0x104).abs();
            let radius = f(b, o + 0xf4);
            let displacement = f(b, o + 0x40);
            let d = if player {
                displacement - w.previous_displacement
            } else {
                0.
            };
            w.previous_displacement = displacement;
            let token = b[(o + 0xc0) / 4];
            let contact = token != 0;
            let anchor_x = f(b, o);
            let anchor_z = f(b, o + 8);
            let phase = f(b, o + 0x110);
            let load = f(b, o + 0x138);
            // W+164: rear wheels offset by 1/6 by side; fronts unchanged.
            let pitch_offset = if anchor_z < 0. {
                if anchor_x > 0. { 1. / 6. } else { -1. / 6. }
            } else {
                0.
            };

            // 456640: AudioMaterial from the wheel's material token; on change
            // stop current voices so the new preset starts on fresh samples.
            if contact {
                let material = input
                    .surfaces
                    .mechanics_name(token)
                    .and_then(|n| bank.material(n));
                if material.is_some() && material != w.material {
                    for c in 0..CATEGORIES {
                        Self::stop(w, c);
                    }
                    w.material = material;
                }
            }
            let material = w.material;
            let info = |c: usize| material.and_then(|m| m[c]);

            // Roll (455410).
            let x = omega + d;
            if player
                && contact
                && let Some(ci) = info(ROLL)
            {
                if !w.active[ROLL] {
                    Self::start(&bank, w, ROLL, ci);
                    w.roll_volume = -24.;
                }
                let c = &bank.infos[ci];
                let (volume, pitch) = c.lerp(x);
                let fade = if x < c.min_vel && c.min_vel > 0. {
                    lin2db(x / c.min_vel)
                } else {
                    0.
                };
                w.roll_volume += 0.2 * (volume - w.roll_volume);
                w.voices[ROLL].pitch = pitch - bump + pitch_offset;
                w.voices[ROLL].volume_db = w.roll_volume + fade;
            } else {
                w.roll_volume -= 32. * dt;
                w.voices[ROLL].volume_db = w.roll_volume;
            }

            // Peel and slide (44F330).
            let ground = if radius > 0. { speed / radius } else { 0. };
            w.spin += (omega - w.spin) / 3.;
            if w.spin < 0.01 {
                w.spin = 0.01;
            }
            let ratio = ground.max(0.) / w.spin;
            let mut peel = (1. - ratio).max(0.);
            let mut slide = (ratio - 1.).max(0.);
            if speed < 10. {
                peel *= 0.1 * speed;
                slide *= 0.1 * speed;
            }
            let fraction = if anchor_z > 0. {
                front_fraction
            } else {
                1. - front_fraction
            };
            let support = mass * fraction * 4.905;
            if support > 0. {
                peel *= load / support;
            }
            if let Some(ci) = info(SLIDE) {
                if contact {
                    if !w.active[SLIDE] {
                        Self::start(&bank, w, SLIDE, ci);
                    }
                    let (mut volume, mut pitch) = bank.infos[ci].lerp(slide - 25. * w.slip_angle);
                    let n = ((166. * w.slip_angle) as i32).max(1) as u32;
                    let target = (w.random.next(n) as f32 - (n / 2) as f32) / 30.;
                    w.slide_jitter += 0.1 * (target - w.slide_jitter);
                    pitch += 0.5 * w.slide_jitter;
                    volume -= w.slide_jitter;
                    w.voices[SLIDE].volume_db = volume;
                    w.voices[SLIDE].pitch = pitch_offset + pitch;
                } else {
                    w.voices[SLIDE].volume_db -= 1200. * dt;
                }
            }
            if let Some(ci) = info(PEEL) {
                let c = &bank.infos[ci];
                if !contact {
                    w.voices[PEEL].volume_db -= 1.;
                } else if peel > c.min_vel {
                    let peel = peel + (0.1 * phase).cos() * (phase.sin() + 1.) * 0.5 * 0.125;
                    if !w.active[PEEL] {
                        Self::start(&bank, w, PEEL, ci);
                        w.voices[PEEL].volume_db = c.lerp(peel).0;
                    }
                    let (volume, pitch) = c.lerp(peel);
                    let previous = w.voices[PEEL].volume_db;
                    w.voices[PEEL].volume_db = volume + 0.02 * (volume - previous);
                    w.voices[PEEL].pitch = pitch;
                } else if w.active[PEEL] {
                    w.voices[PEEL].volume_db -= 120. * dt;
                    if w.voices[PEEL].volume_db <= SILENT {
                        Self::stop(w, PEEL);
                    }
                }
            }

            // Skid and scrub (44FE60).
            w.slip_angle = slip;
            let mut a = slip * if speed < 5. { 0.2 * speed } else { 1. } + d;
            let mut s = slip * if speed < 30. { speed / 30. } else { 1. } + d;
            if omega < 10. {
                a *= 0.1 * omega;
                s *= 0.1 * omega;
            }
            w.skid += (a - w.skid) / 13.;
            let dropped = if w.random.next(100) < 7 { 0. } else { s };
            w.scrub += (dropped - w.scrub) / 3.;
            let mut skid = w.skid;
            if speed > 20. {
                skid = (skid * (1. + 0.05 * (speed - 20.)).min(1.8)).min(1.);
            }
            if player && let Some(ci) = info(SCRUB) {
                let c = &bank.infos[ci];
                let mut x = w.scrub;
                let excess = x - c.max_vel;
                if excess > 0. {
                    x -= 2. * excess;
                }
                let x = x.max(0.);
                if !w.active[SCRUB] {
                    Self::start(&bank, w, SCRUB, ci);
                }
                let knots = [0., c.min_vel, c.max_vel, c.min_vel + c.max_vel];
                let volumes = [SILENT, c.min_vol, c.max_vol, SILENT];
                let pitches = [
                    0.9 * c.min_pitch,
                    c.min_pitch,
                    c.max_pitch,
                    1.1 * c.max_pitch,
                ];
                let k = knots[1..].iter().position(|&k| x <= k).unwrap_or(2);
                let span = knots[k + 1] - knots[k];
                let t = if span > 0. {
                    ((x - knots[k]) / span).clamp(0., 1.)
                } else {
                    1.
                };
                let mut volume = volumes[k] + (volumes[k + 1] - volumes[k]) * t;
                let pitch = pitches[k] + (pitches[k + 1] - pitches[k]) * t;
                let mut m =
                    ((0.01 * phase).cos() + 1.) / 2. * (((0.1 * phase).sin() + 1.) / 2.) * 0.5
                        + 0.5;
                if speed < 8. {
                    m *= 0.125 * speed;
                }
                volume += lin2db(m);
                if x < c.min_vel {
                    volume = -47.;
                }
                w.voices[SCRUB].volume_db = volume;
                w.voices[SCRUB].pitch = pitch;
            } else {
                w.voices[SCRUB].volume_db -= 12. * dt;
            }
            if let Some(ci) = info(SKID) {
                let c = &bank.infos[ci];
                if !w.active[SKID] {
                    Self::start(&bank, w, SKID, ci);
                }
                if skid >= c.min_vel {
                    let t = c.t(skid - 5. * d);
                    let n = ((66. * skid) as i32).max(1) as u32;
                    let target = (w.random.next(n) as f32 - (n / 2) as f32) / 30.;
                    w.skid_jitter += 0.1 * (target - w.skid_jitter);
                    w.wobble_timer -= dt;
                    if w.wobble_timer <= 0. {
                        let v = speed.min(100.);
                        w.wobble_timer = (0.005 * w.random.next(100) as f32 - 0.04 * v).max(0.015);
                        w.skid_wobble = -0.001 * w.random.next(2200) as f32 * 0.4 * v;
                    }
                    let level = if speed < 8. { 0.125 * speed } else { 1. };
                    w.voices[SKID].pitch =
                        c.min_pitch + (c.max_pitch - c.min_pitch) * t + w.skid_jitter;
                    w.voices[SKID].volume_db =
                        c.min_vol + (c.max_vol - c.min_vol) * t + w.skid_wobble + lin2db(level);
                } else {
                    w.voices[SKID].volume_db = -47.;
                }
            }
            // 42C100/42C150 setter clamps.
            for v in &mut w.voices {
                v.volume_db = v.volume_db.clamp(SILENT, 0.);
                v.pitch = v.pitch.clamp(0.5, 2.);
            }
            if !w.voices[ROLL].volume_db.is_finite() {
                w.roll_volume = SILENT;
            }
        }
    }
}

/// Group levels per category for the player car (413E50: roll in Wheels,
/// the others in Skids) at Normal/Chase/PC (`audio_mix::Table::group_db`).
pub const CHASE_BUS_DB: [f32; CATEGORIES] = [-9., -8., -8., -8., -8.];
/// Min/max emitter distance per category: the variety presets (roll
/// `surfaceloops` 15/50, the others `skidloops` 10/20), with the peel and skid
/// overrides 10/50 of 44F330/44FE60 (ordinary race, W+4D/W+4E clear).
pub const DISTANCES: [(f32, f32); CATEGORIES] =
    [(15., 50.), (10., 50.), (10., 20.), (10., 50.), (10., 20.)];

/// Backend levels per wheel and category: the group mix (player 413E50: roll
/// Wheels, others Skids; other cars AI) plus each wheel's distance term.
pub fn bus_levels(
    table: Option<&crate::audio_mix::Table>,
    modes: &crate::audio_mix::ModeMix,
    camera: crate::audio_mix::Camera,
    player: bool,
    trim: f32,
    distances: [f32; 4],
) -> [[f32; CATEGORIES]; 4] {
    use crate::audio_mix::{AI, SKIDS, WHEELS, distance_db};
    distances.map(|d| {
        std::array::from_fn(|c| {
            let group = match (player, c) {
                (false, _) => AI,
                (true, ROLL) => WHEELS,
                _ => SKIDS,
            };
            let mix = table.map_or(if player { CHASE_BUS_DB[c] } else { -9. }, |t| {
                modes.group_db(t, camera, group, 0.)
            }) + trim;
            distance_db(d, DISTANCES[c].0, DISTANCES[c].1).map_or(f32::NEG_INFINITY, |x| mix + x)
        })
    })
}
/// World position of each wheel's anchor (vehicle +1990 + i*150, car frame).
pub fn wheel_positions(vehicle: &[u32], physical: &[u32]) -> [[f32; 3]; 4] {
    let m: [f32; 16] = std::array::from_fn(|i| f(physical, 0x1d0 + i * 4));
    std::array::from_fn(|i| {
        let o = 0x1990 + i * 0x150;
        let a = [f(vehicle, o), f(vehicle, o + 4), f(vehicle, o + 8)];
        std::array::from_fn(|r| m[r] * a[0] + m[4 + r] * a[1] + m[8 + r] * a[2] + m[12 + r])
    })
}

/// Independent looping mixer at the output rate: linear interpolation over the
/// 40 kHz descriptor rate times pitch, and a per-sample gain ramp so 62.5 Hz
/// targets do not click.
pub struct Mixer {
    bank: Arc<Bank>,
    rate: u32,
    slots: [[Slot; CATEGORIES]; 4],
    /// Group level per wheel and category in dB (mix plus distance), added
    /// at the backend (A23B70); `-inf` silences.
    pub bus_db: [[f32; CATEGORIES]; 4],
}
#[derive(Clone, Copy, Default)]
struct Slot {
    sample: Option<usize>,
    generation: u32,
    position: f64,
    gain: f32,
}
impl Mixer {
    pub fn new(bank: Arc<Bank>, rate: u32) -> Self {
        Self {
            bank,
            rate,
            slots: [[Slot::default(); CATEGORIES]; 4],
            bus_db: [CHASE_BUS_DB; 4],
        }
    }
    /// One output sample. `audible=false` ramps every voice to silence.
    pub fn sample(&mut self, voices: &[[Voice; CATEGORIES]; 4], audible: bool) -> f32 {
        let mut out = 0.;
        for ((wheel, targets), buses) in self.slots.iter_mut().zip(voices).zip(self.bus_db) {
            for ((slot, voice), bus) in wheel.iter_mut().zip(targets).zip(buses) {
                if slot.sample != voice.sample || slot.generation != voice.generation {
                    if voice.sample.is_some() && slot.gain > 1e-4 && slot.sample.is_some() {
                        // Let the old voice fade before switching sample.
                        slot.gain *= 0.995;
                    } else {
                        slot.sample = voice.sample;
                        slot.generation = voice.generation;
                        slot.position = 0.;
                    }
                }
                let Some(index) = slot.sample else { continue };
                let target = if audible && slot.sample == voice.sample {
                    crate::audio_mix::backend_gain(voice.volume_db, bus, Some(0.))
                } else {
                    0.
                };
                slot.gain += (target - slot.gain) * 0.002;
                let pcm = &self.bank.samples[index];
                let length = pcm.len() as f64;
                let step = f64::from(voice.pitch.clamp(0.5, 2.))
                    * f64::from(self.bank.rates[index])
                    / f64::from(self.rate);
                // A silent voice keeps its cursor moving but costs no mixing
                // (most opponent voices most of the time; docs/AUDIO_RESEARCH.md 8).
                if target == 0. && slot.gain < 1e-6 {
                    slot.gain = 0.;
                    slot.position = (slot.position + step) % length;
                    continue;
                }
                let i = slot.position as usize % pcm.len();
                let frac = (slot.position - slot.position.floor()) as f32;
                let a = pcm[i];
                let b = pcm[(i + 1) % pcm.len()];
                out += (a + (b - a) * frac) * slot.gain;
                slot.position = (slot.position + step) % length;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_vehicle::{Controls, DELTA, Runtime};

    #[test]
    fn decibel_floor_and_dual_curve() {
        assert_eq!(lin2db(0.), -48.);
        assert_eq!(lin2db(2.), 0.);
        assert!((lin2db(0.5) + 6.0206).abs() < 1e-3);
        assert_eq!(lin2db(1e-6), -48.);
        let c = ColInfo {
            name: "TarmacRoll".into(),
            min_vel: 4.,
            max_vel: 155.,
            min_vol: -40.,
            max_vol: -12.,
            min_pitch: 0.75,
            max_pitch: 1.2,
            samples: vec![],
        };
        assert_eq!(c.lerp(0.), (-40., 0.75));
        assert_eq!(c.lerp(1000.), (-12., 1.2));
        let (v, p) = c.lerp(79.5);
        assert!((v + 26.).abs() < 1e-4 && (p - 0.975).abs() < 1e-6);
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn source_launch_peels_then_rolls_reproducibly() {
        std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024)
            .spawn(|| {
                let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
                let root = Path::new(&root);
                let bank = Bank::load(root).unwrap();
                assert_eq!(bank.samples.len(), 185);
                let tarmac = bank.material("TARMAC").unwrap();
                assert!(tarmac.iter().all(Option::is_some));
                let run = || {
                    let mut runtime = Runtime::lacetti(root, "slot_00", 200679).unwrap();
                    let mut driver = Driver::new(bank.clone());
                    let mut trace = Vec::new();
                    for tick in 0..400 {
                        runtime
                            .tick(Controls {
                                throttle: 1.,
                                handbrake: tick < 100,
                                ..Default::default()
                            })
                            .unwrap();
                        driver.tick(&Inputs {
                            vehicle: &runtime.state.car.owner.vehicle,
                            physical: &runtime.state.car.owner.physical,
                            surfaces: runtime.surfaces(),
                            delta: DELTA,
                        });
                        trace.push(driver.voices());
                    }
                    trace
                };
                let trace = run();
                assert_eq!(trace, run());
                assert!(
                    trace
                        .iter()
                        .flatten()
                        .flatten()
                        .all(|v| v.volume_db.is_finite()
                            && (-48. ..=0.).contains(&v.volume_db)
                            && (0.5..=2.).contains(&v.pitch))
                );
                // Clutch dump after the handbrake hold: front-wheel peel is loud.
                let peel = (100..160)
                    .flat_map(|t| [trace[t][2][PEEL].volume_db, trace[t][3][PEEL].volume_db])
                    .fold(-48f32, f32::max);
                assert!(peel > -10., "front peel {peel}");
                // At speed the rolling loop is clearly audible and peel has gone.
                assert!(trace[399][2][ROLL].volume_db > -20.);
                assert!(trace[399][2][PEEL].sample.is_none());
                let mut mixer = Mixer::new(bank.clone(), 48_000);
                let peak = (0..48_000)
                    .map(|i| mixer.sample(&trace[100 + i / 800], true).abs())
                    .fold(0f32, f32::max);
                assert!(peak.is_finite() && peak > 0.001 && peak < 1.);
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
