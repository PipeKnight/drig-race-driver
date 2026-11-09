//! Crowds (docs/AUDIO_RESEARCH.md §31): the original's crowd object (constructor 0x427730,
//! tick 0x44E220) places up to 35 voices on the visual crowd's grid cells near a point ahead of
//! the listened car. Each tick it may spawn (0x4499A0) NormalCrowd or GrandStandCrowd samples
//! (autoplaybanks, bank `env_crowd`) on the cells within 150 m, with the level and pitch set
//! once from the cell's head count, and it stops looping voices the car has left behind
//! (0x41F280). Race events raise the excitement (the speech data's ReactiveCrowd messages,
//! 0x41F2D0), which lets the excited samples play. A hard hit against a wall or the track
//! (the visual crowd's CrowdRecoil, 0x756C40 → 0x76A620) makes the nearest crowd cells gasp,
//! one cell per tick (0x41BCD0 → 0x449FA0). Group Crowd (11). Presentation only.
use crate::{
    ambience_audio::{AutoPlay, lerp, linstep},
    collision_audio::{Curve, Variety, read_infos, read_varieties},
    hud_audio::{Sound, wip_sound},
    route::Route,
};
use grid_formats::{audio_dictionary, binxml, crowd_placements};
use std::{collections::HashMap, path::Path, sync::Arc};

/// Mix group (41F570).
pub const CROWD: usize = 11;
/// Emitter slots (constructor 0x427789: 0x23).
pub const SLOTS: usize = 35;
/// Query radius (0xCB3EB4) and result capacity (0x44E543: 250).
const RADIUS: f32 = 150.;
const QUERY_MAX: usize = 250;
/// More people than this in the queried cells selects GrandStandCrowd (0x44E694).
const GRANDSTAND_PEOPLE: u32 = 2500;
/// The query point lies 70 m (0xCB5050) ahead at 120 mph (0xCB4694) and above.
const AHEAD: f32 = 70.;
/// A one-shot quieter than this is not started (0xCB4B00).
const QUIETEST: f32 = -32.;
/// A loop stops once the car is farther than this share of its MaxDistance (0xCAFE64).
const STOP_SHARE: f32 = 0.75;
/// A gasp quieter than this is not started (0xCB469C).
const GASP_QUIETEST: f32 = -36.;
/// A cell uses GrandStandCrowd's samples and the grand gasp above this many seated people
/// (0x449A93, 0xCAFBD8: people × seated share).
const SEATED: u32 = 20;
/// The mix modes 0x4499A0 and 0x44E220 test (`[[AM+0x28]+0xBF0]`, 0x411C10).
pub use crate::audio_mix::{END_OF_RACE, INTRO};

/// A visual crowd cell as the audio sees it (0x963D90, 0x97A630).
#[derive(Clone, Debug, PartialEq)]
pub struct Cell {
    pub min: [f32; 3],
    pub max: [f32; 3],
    /// +0x50: the box centre.
    pub centre: [f32; 3],
    /// +0x14: the instances placed in it.
    pub people: u32,
    /// Placed instances of the Grandstand type (+0x60 is their share) [I: type 7 =
    /// the `Grandstand` anim selector of the seated sets].
    pub seated: u32,
}

/// The route's crowd grid (crowd manager +0x24C..+0x268) from `crowd_standing.bin`.
pub struct Cells {
    origin: [f32; 2],
    size: [f32; 2],
    dims: [u32; 2],
    /// Grid slot → cell; only cells without moving instances are entered (0x769D50).
    map: Vec<Option<usize>>,
    pub cells: Vec<Cell>,
}

/// 0x771D30: the query's Shell sort, nearest first, with the gaps 2^k − 1 from 8191 down
/// (0xD088A0..0xD08870). Not stable: cells at the same distance keep the original's order.
fn shell_sort(v: &mut [(f32, usize)]) {
    for k in (0..13).rev() {
        let gap = (1usize << (k + 1)) - 1;
        for i in gap..v.len() {
            let key = v[i];
            if key.0 >= v[i - gap].0 {
                continue;
            }
            let mut j = i;
            while j >= gap && key.0 < v[j - gap].0 {
                v[j] = v[j - gap];
                j -= gap;
            }
            v[j] = key;
        }
    }
}

impl Cells {
    pub fn load(root: &Path, route: &Route) -> Result<Self, Box<dyn std::error::Error>> {
        let placements = std::fs::read(root.join(route.file("crowd_standing.bin")))?;
        let defs = std::fs::read(root.join("anims/crowdDefs.xml"))?;
        Self::parse(&placements, &defs)
    }

    pub fn parse(placements: &[u8], defs: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        let seated_sets = seated_sets(defs)?;
        let crowd = crowd_placements::read(placements).map_err(|e| format!("crowd_standing: {e:?}"))?;
        let mut cells: Vec<Cell> = crowd
            .cells
            .iter()
            .map(|c| {
                let (min, max) = ([c.bounds[0], c.bounds[1], c.bounds[2]], [c.bounds[3], c.bounds[4], c.bounds[5]]);
                Cell { min, max, centre: std::array::from_fn(|i| (min[i] + max[i]) * 0.5), people: 0, seated: 0 }
            })
            .collect();
        // [I] every standing instance is placed (0x97A630 places up to `instances`).
        for group in &crowd.groups {
            let crowd_placements::Placements::Standing(points) = &group.placements else { continue };
            let seated = seated_sets.iter().any(|s| s.as_bytes() == group.instance_set);
            for p in points {
                if let Some(c) = cells.get_mut(p.cell as usize) {
                    c.people += 1;
                    c.seated += u32::from(seated);
                }
            }
        }
        let mut map = vec![None; crowd.dimensions[0] as usize * crowd.dimensions[1] as usize];
        for (i, c) in crowd.cells.iter().enumerate() {
            if c.moving_instances == 0
                && let Some(slot) = map.get_mut(c.grid_index as usize)
            {
                *slot = Some(i);
            }
        }
        Ok(Self { origin: crowd.origin_xz, size: crowd.cell_size_xz, dims: crowd.dimensions, map, cells })
    }

    /// 0x756E70: the cells with people whose box lies within `radius` of `q`, as (squared
    /// distance to the box, cell), grid x outer and z inner, at most `max`.
    pub fn query(&self, q: [f32; 3], radius: f32, max: usize) -> Vec<(f32, usize)> {
        let span = |c: f32, o: f32, size: f32, n: u32| {
            let last = n.max(1) as i64 - 1;
            let at = |v: f32| (((v - o) / size) as i64).clamp(0, last);
            (at(c - radius), at(c + radius))
        };
        let (x0, x1) = span(q[0], self.origin[0], self.size[0], self.dims[0]);
        let (z0, z1) = span(q[2], self.origin[1], self.size[1], self.dims[1]);
        let mut out = Vec::new();
        for x in x0..=x1 {
            for z in z0..=z1 {
                let Some(Some(i)) = self.map.get((self.dims[0] as i64 * z + x) as usize) else { continue };
                let c = &self.cells[*i];
                if c.people == 0 {
                    continue;
                }
                let d2: f32 = (0..3).map(|k| (q[k].clamp(c.min[k], c.max[k]) - q[k]).powi(2)).sum();
                if d2 < radius * radius {
                    out.push((d2, *i));
                    if out.len() >= max {
                        shell_sort(&mut out);
                        return out;
                    }
                }
            }
        }
        shell_sort(&mut out);
        out
    }
}

/// The instance sets whose maps use a `Grandstand` anim selector (crowdDefs.xml).
fn seated_sets(defs: &[u8]) -> Result<Vec<String>, String> {
    let doc = binxml::read(defs).map_err(|e| format!("crowdDefs: {e:?}"))?;
    let attr = |e: &binxml::Element, k: &str| e.attributes.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string());
    let grandstand: Vec<String> = doc
        .elements
        .iter()
        .filter(|e| e.name == "animSelector" && attr(e, "type").as_deref() == Some("Grandstand"))
        .filter_map(|e| attr(e, "name"))
        .collect();
    let seated_maps: Vec<String> = doc
        .elements
        .iter()
        .filter(|e| e.name == "instanceMap" && attr(e, "animSelector").is_some_and(|s| grandstand.contains(&s)))
        .filter_map(|e| attr(e, "name"))
        .collect();
    Ok(doc
        .elements
        .iter()
        .enumerate()
        .filter(|(_, e)| e.name == "instanceSet")
        .filter(|(i, _)| {
            doc.elements[doc.elements[*i].children.clone()]
                .iter()
                .any(|m| m.name == "map" && attr(m, "name").is_some_and(|n| seated_maps.contains(&n)))
        })
        .filter_map(|(_, e)| attr(e, "name"))
        .collect())
}

/// NormalCrowd and GrandStandCrowd over one decoded `env_crowd`.
pub struct Bank {
    pub normal: AutoPlay,
    pub grand: AutoPlay,
    /// The gasps' CollisionInfos and varieties; `None` when the data lacks one.
    pub gasps: Option<Gasps>,
    pub sounds: Arc<[Sound]>,
}

/// The CollisionInfos the constructor looks up for the gasps (0x42D3B0).
#[derive(Clone, Debug, PartialEq)]
pub struct Gasps {
    /// C+0x24 `group_gasp` and C+0x28 `grand_gasp`: level over the cell's head count, and the
    /// variety the sample comes from.
    pub group: (Curve, Variety),
    pub grand: (Curve, Variety),
    /// C+0x2C `gaspControl`: level over the recoil's intensity.
    pub control: Curve,
    /// C+0x30 `gaspDistControl`: level over the cell's distance from the recoil, which also
    /// sets the pitch.
    pub distance: Curve,
}

impl Gasps {
    /// The four CollisionInfos and the two varieties (bank `env_crowd`), decoding their
    /// samples into `sounds` (`decoded`: env_crowd sample index → sound, shared with the
    /// AutoPlay banks).
    pub fn parse(
        dictionary: &audio_dictionary::Dictionary,
        infos: &[u8],
        varieties: &[u8],
        data: &[u8],
        sounds: &mut Vec<Sound>,
        decoded: &mut HashMap<usize, usize>,
    ) -> Result<Option<Self>, String> {
        let mut infos = read_infos(infos)?;
        let Some(bank) = dictionary.banks.iter().find(|b| b.name.eq_ignore_ascii_case("env_crowd")) else {
            return Ok(None);
        };
        let mut sample = |_: &str, name: &str| -> Result<Option<usize>, String> {
            let Some(i) = bank.samples.iter().position(|s| s.name.eq_ignore_ascii_case(name)) else { return Ok(None) };
            if let Some(&k) = decoded.get(&i) {
                return Ok(Some(k));
            }
            let Ok(sound) = wip_sound(bank, i, data) else { return Ok(None) };
            sounds.push(sound);
            decoded.insert(i, sounds.len() - 1);
            Ok(Some(sounds.len() - 1))
        };
        let mut varieties = read_varieties(varieties, &["env_crowd"], &mut sample)?;
        let mut take = |name: &str| -> Option<(Curve, Variety)> {
            let curve = infos.remove(name)?;
            let variety = varieties.remove(&curve.variety)?;
            Some((curve, variety))
        };
        let (Some(group), Some(grand)) = (take("group_gasp"), take("grand_gasp")) else { return Ok(None) };
        let (Some(control), Some(distance)) = (infos.remove("gaspControl"), infos.remove("gaspDistControl")) else {
            return Ok(None);
        };
        Ok(Some(Self { group, grand, control, distance }))
    }
}

impl Bank {
    pub fn load(root: &Path) -> Result<Arc<Self>, String> {
        let audio = root.join("audio");
        let read = |name: &str| std::fs::read(audio.join(name)).map_err(|e| format!("{name}: {e}"));
        let dictionary = read("CMSamples.dic")?;
        let dictionary = audio_dictionary::read(&dictionary).map_err(|e| format!("{e:?}"))?;
        let (banks, infos, data) = (read("autoplaybanks.bin")?, read("CollisionInfos.bin")?, read("env_crowd.WIP")?);
        let (mut sounds, mut decoded) = (Vec::new(), HashMap::new());
        let mut play = |name: &str| {
            AutoPlay::parse_shared(&dictionary, &banks, &infos, name, &data, &mut sounds, &mut decoded)?
                .ok_or(format!("{name} absent"))
        };
        // 0x433738 / 0x433748.
        let normal = play("NormalCrowd")?;
        let grand = play("GrandStandCrowd")?;
        let gasps = Gasps::parse(&dictionary, &infos, &read("audioVarieties.bin")?, &data, &mut sounds, &mut decoded)?;
        Ok(Arc::new(Self { normal, grand, gasps, sounds: sounds.into() }))
    }
}

/// The listened car (+0x1570 position, +0x1580 forward) and its speed in m/s.
#[derive(Clone, Copy, Debug)]
pub struct Listener {
    pub position: [f32; 3],
    pub forward: [f32; 3],
    pub speed: f32,
}

/// A playing crowd voice (slot E = crowd + 0x40 + 0x60·i).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Emitter {
    pub sound: usize,
    pub looped: bool,
    /// The level (dB, clamped by 0x42C100) and pitch (0x42C150) set at the start.
    pub db: f32,
    pub pitch: f32,
    pub position: [f32; 3],
    pub min_distance: f32,
    pub max_distance: f32,
    /// The voice's Doppler property, set at the start (0x449F72): 0.5 for a loop once the
    /// race intro has handed over to the in-game cameras (C+0x15A0), else 0. Its velocity is
    /// zero (0x435747), so only the listener's motion shifts it.
    pub doppler: f32,
    /// Seconds left of a one-shot; the slot frees when it ends (the voice handle E+0x14).
    left: f32,
}

/// The visual crowd's recoil records (crowd manager +0x3B00 written, +0x3B04 read; 10 of
/// stride 0x30, count at +0x1E0) and the recoil cooldown (+0x3B08).
#[derive(Default)]
pub struct Recoils {
    write: Vec<Record>,
    read: Vec<Record>,
    cooldown: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Record {
    /// +0 the car.
    owner: usize,
    /// +0x10 the summed contact points, +4 their count.
    sum: [f32; 3],
    weight: f32,
    /// +0x20 the summed solved impacts.
    intensity: f32,
    /// +0x24: the other body is static geometry (shape +0x216) and the contact is not
    /// floor-like (|normal.y| < 0.2).
    flag: bool,
}

/// A `CrowdRecoil` message the crowd audio takes (0x764A40: +4 position, +0x10 intensity,
/// +0x15 the record's flag).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Recoil {
    pub position: [f32; 3],
    pub intensity: f32,
    pub flag: bool,
}

/// Records at most this many per frame (0x756C40).
const RECORDS: usize = 10;
/// A record broadcasts above this summed impact (0xCBD664) …
const RECOIL_IMPACT: f32 = 2000.;
/// … and then holds the next ones off for a second (0x76A620).
const RECOIL_COOLDOWN: f32 = 1.;

impl Recoils {
    /// 0x756C40, the contact listener 0x756E40 for each contact of a car: a contact within
    /// 1 m of the averaged point of the car's earlier records this frame adds to each of
    /// them, else it opens a record while fewer than ten are open. `other_static`: the other
    /// body's shape is static geometry (+0x216: the track, its boundary and the route
    /// props; not another car).
    pub fn add(&mut self, owner: usize, point: [f32; 3], normal: [f32; 3], impact: f32, other_static: bool) {
        let mut merged = false;
        for r in self.write.iter_mut().filter(|r| r.owner == owner) {
            let d2: f32 = (0..3).map(|k| (r.sum[k] / r.weight - point[k]).powi(2)).sum();
            if d2 < 1. {
                (0..3).for_each(|k| r.sum[k] += point[k]);
                r.intensity += impact;
                r.weight += 1.;
                merged = true;
            }
        }
        if !merged && self.write.len() < RECORDS {
            let flag = other_static && normal[1].abs() < 0.2;
            self.write.push(Record { owner, sum: point, weight: 1., intensity: impact, flag });
        }
    }

    /// One frame: 0x608940 makes the last frame's records the read list, 0x76A620 broadcasts
    /// each record above 2000 when the cooldown has run out, and 0x7648F0 runs the cooldown
    /// down by `dt`. The original broadcasts only outside replays (0x608940, mode +0xA30);
    /// the port has no replay playback.
    pub fn frame(&mut self, dt: f32, out: &mut Vec<Recoil>) {
        std::mem::swap(&mut self.read, &mut self.write);
        self.write.clear();
        if self.cooldown <= 0. {
            for r in self.read.iter().filter(|r| r.intensity > RECOIL_IMPACT) {
                self.cooldown = RECOIL_COOLDOWN;
                out.push(Recoil { position: r.sum.map(|v| v / r.weight), intensity: r.intensity, flag: r.flag });
            }
        }
        self.cooldown = (self.cooldown - dt.abs()).max(0.);
    }
}

/// The running gasp (C+0x15C8 set): the next query entry (+0x15C4), the intensity (+0x15CC)
/// and where the recoil happened (+0x15D0).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Gasp {
    next: usize,
    intensity: f32,
    position: [f32; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Start { slot: usize },
    Stop { slot: usize },
}

/// The crowd object's state.
pub struct Crowd {
    bank: Arc<Bank>,
    slots: Vec<Option<Emitter>>,
    /// +0xD64 the looping voices.
    loopers: u32,
    /// +0xD6C target and +0xD70 current excitement, +0xD74 hold, +0xD78 its timer.
    target: f32,
    current: f32,
    hold: f32,
    since: f32,
    /// +0x15A4 the slots kept for excited samples.
    reserved: usize,
    /// +0x1570/+0x1580: kept when a mode leaves them alone.
    position: [f32; 3],
    forward: [f32; 3],
    random: u32,
    /// +0x15A0: the crowd's loops get Doppler 0.5 from now on.
    doppler_on: bool,
    gasp: Option<Gasp>,
}

impl Crowd {
    pub fn new(bank: Arc<Bank>, seed: u32) -> Self {
        Self {
            bank,
            slots: vec![None; SLOTS],
            loopers: 0,
            target: 0.,
            current: 0.,
            hold: 0.,
            since: 0.,
            reserved: 10,
            position: [0.; 3],
            forward: [0., 0., 1.],
            random: seed | 1,
            doppler_on: false,
            gasp: None,
        }
    }

    pub fn emitters(&self) -> impl Iterator<Item = (usize, &Emitter)> {
        self.slots.iter().enumerate().filter_map(|(i, e)| e.as_ref().map(|e| (i, e)))
    }
    pub fn loopers(&self) -> u32 {
        self.loopers
    }
    pub fn excitement(&self) -> f32 {
        self.current
    }

    /// The game's random generator is not reproduced [I]: a 32-bit LCG.
    fn next(&mut self) -> u32 {
        self.random = self.random.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        self.random >> 8
    }
    fn uniform(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * (self.next() & 0xff_ffff) as f32 / 16_777_216.
    }

    /// 0x41F2D0: a race event's excitement `level` held for `hold` seconds. Only taken once
    /// the previous hold has run out; the reserved slots follow every call.
    pub fn excite(&mut self, level: f32, hold: f32) {
        self.reserved = if level > 1. { 25 } else { 10 };
        if self.since > self.hold {
            self.target = level;
            self.hold = hold;
            self.since = 0.;
        }
    }

    /// 0x41BCD0: a recoil with its flag set starts the gasps unless they are running.
    pub fn recoil(&mut self, r: &Recoil) {
        if r.flag && self.gasp.is_none() {
            self.gasp = Some(Gasp { next: 0, intensity: r.intensity, position: r.position });
        }
    }
    pub fn gasping(&self) -> bool {
        self.gasp.is_some()
    }

    /// 0x411600, each tick first. Once the hold has run out only the current value is
    /// cleared, so it alternates between the target and 0 each tick until the next event
    /// (as the original does, AUDIO_RESEARCH §31).
    fn settle(&mut self) {
        if self.current != self.target {
            self.current = self.target;
            return;
        }
        if self.hold != 0. && self.since > self.hold {
            self.reserved = 10;
            self.current = 0.;
        }
    }

    /// 0x44E220 for one tick of `dt` seconds. `mode`: the mix mode; `camera`: the camera
    /// position, the listener in Intro and EndOfRace.
    pub fn tick(&mut self, cells: &Cells, l: &Listener, mode: usize, camera: [f32; 3], dt: f32, out: &mut Vec<Event>) {
        self.since += dt;
        // C+0x15A0: set by 0x420280 when the race intro hands over to the in-game cameras
        // (0x58BAF0); the port has no intro, so the first tick outside Intro.
        self.doppler_on |= mode != INTRO;
        self.settle();
        // Finished one-shots free their slot.
        for slot in self.slots.iter_mut() {
            if let Some(e) = slot
                && !e.looped
            {
                e.left -= dt;
                if e.left <= 0. {
                    *slot = None;
                }
            }
        }
        let mph = crate::drift::mph(l.speed);
        let q = if mode == INTRO || mode == END_OF_RACE {
            self.position = camera;
            camera
        } else {
            self.position = l.position;
            self.forward = l.forward;
            let ahead = AHEAD * linstep(1., 120., mph);
            std::array::from_fn(|i| l.position[i] + l.forward[i] * ahead)
        };
        let entries = cells.query(q, RADIUS, QUERY_MAX);
        let people: u32 = entries.iter().map(|&(_, c)| cells.cells[c].people).sum();
        let count = entries.len().min(SLOTS);
        let grandstand = people > GRANDSTAND_PEOPLE;
        let density = if grandstand { &self.bank.grand } else { &self.bank.normal }.density;
        if count > 0 && density * mph.clamp(80., 100.) > self.uniform(0., 100.) {
            self.spawn(cells, &entries[..count], mph, mode, out);
        }
        // 0x41F280: a loop the car has left stops.
        for (slot, e) in self.slots.iter_mut().enumerate() {
            if let Some(v) = e
                && v.looped
            {
                let d = (0..3).map(|k| (v.position[k] - self.position[k]).powi(2)).sum::<f32>().sqrt();
                if d > v.max_distance * STOP_SHARE {
                    *e = None;
                    self.loopers -= 1;
                    out.push(Event::Stop { slot });
                }
            }
        }
        if self.gasp.is_some() {
            self.gasp_step(cells, &entries, mode, out);
        }
    }

    /// 0x449FA0, last in the tick while a gasp runs: walks the query list from the next
    /// entry and starts at most one gasp per tick, on the cell of entry j in slot j. It stops
    /// for this tick at a looping slot, the list's end, a cell beyond the variety's
    /// MaxDistance or a gasp quieter than −36 dB, and ends once all 35 entries are passed.
    /// EndOfRace holds it.
    fn gasp_step(&mut self, cells: &Cells, entries: &[(f32, usize)], mode: usize, out: &mut Vec<Event>) {
        let Some(mut gasp) = self.gasp else { return };
        if mode == END_OF_RACE {
            return;
        }
        let bank = self.bank.clone();
        let Some(g) = bank.gasps.as_ref() else {
            self.gasp = None;
            return;
        };
        while gasp.next < SLOTS {
            let j = gasp.next;
            gasp.next += 1;
            self.gasp = Some(gasp);
            if self.slots[j].is_some_and(|e| e.looped) {
                return;
            }
            let Some(&(d2, c)) = entries.get(j) else { return };
            let cell = &cells.cells[c];
            if cell.people <= 5 {
                continue;
            }
            let (curve, variety) = if cell.seated > SEATED { &g.grand } else { &g.group };
            if d2 > variety.max_distance * variety.max_distance {
                return;
            }
            let (db, t) = gasp_level(g, curve, gasp.intensity, cell.people, gasp.position, cell.centre);
            if db < GASP_QUIETEST || variety.samples.is_empty() {
                return;
            }
            let sound = variety.samples[self.next() as usize % variety.samples.len()];
            let data = &bank.sounds[sound];
            let seconds = (data.pcm.len() / data.channels.max(1)) as f32 / data.rate.max(1) as f32;
            let pitch = lerp(0.95, 1.05, t).clamp(0.5, 2.);
            // A one-shot already in the slot is replaced.
            self.slots[j] = Some(Emitter {
                sound,
                looped: false,
                db: db.clamp(-48., 0.),
                pitch,
                position: cell.centre,
                min_distance: variety.min_distance,
                max_distance: variety.max_distance,
                doppler: 0.,
                left: seconds / pitch,
            });
            out.push(Event::Start { slot: j });
            return;
        }
        self.gasp = None;
    }

    /// 0x4499A0 over the query's first `entries`.
    fn spawn(&mut self, cells: &Cells, entries: &[(f32, usize)], mph: f32, mode: usize, out: &mut Vec<Event>) {
        let mut s = if mph <= 0. { 0. } else if mph >= 100. { 1. } else { mph * 0.01 };
        if entries.len() < self.reserved {
            self.reserved = entries.len() >> 1;
        }
        // The network receiver's "finish" branch ([C+0x0C]+0x24 == 1) is not taken offline.
        if mode == INTRO || mode == END_OF_RACE {
            self.target = 1.3;
            s = 0.4;
            self.reserved = 30;
        }
        let bank = self.bank.clone();
        for i in 0..SLOTS {
            if self.slots[i].is_some() {
                continue;
            }
            let (d2, c) = entries[i % entries.len()];
            let cell = &cells.cells[c];
            if cell.people <= 5 {
                continue;
            }
            let play = if cell.seated > SEATED { &bank.grand } else { &bank.normal };
            // Behind the car (fast, not EndOfRace): skipped.
            if s > 0.5 && mode != END_OF_RACE {
                let along: f32 = (0..3).map(|k| (cell.centre[k] - self.position[k]) * self.forward[k]).sum();
                if along < 0. {
                    continue;
                }
            }
            if play.samples.is_empty() {
                continue;
            }
            let smp = &play.samples[self.next() as usize % play.samples.len()];
            if d2 > smp.max_distance * smp.max_distance {
                // 0x449CA2 leaves the whole spawn, not only this slot.
                return;
            }
            let x = self.current;
            let excited = if (x > 1. || x < 0.) && i < self.reserved {
                if smp.min_speed > x || x > smp.max_speed {
                    continue;
                }
                true
            } else {
                if i < self.reserved || smp.min_speed > s || s > smp.max_speed {
                    continue;
                }
                false
            };
            let pf = cell.people as f32;
            if smp.min_spawn > pf || pf > smp.max_spawn {
                continue;
            }
            if smp.probability <= self.uniform(0., 1.) {
                continue;
            }
            let Some(sound) = smp.sound else { continue };
            if smp.looped {
                if self.loopers >= play.max_loopers {
                    continue;
                }
                self.loopers += 1;
            }
            let t = linstep(smp.min_spawn, smp.max_spawn, pf);
            let mut level = lerp(smp.min_vol, smp.max_vol, t);
            if !excited
                && let Some(m) = play.master.as_ref().filter(|m| mph > m.min_vel)
            {
                level += lerp(m.min_vol, m.max_vol, linstep(m.min_vel, m.max_vel, mph));
            }
            if level < QUIETEST && !smp.looped {
                continue;
            }
            let sound_data = &bank.sounds[sound];
            let seconds = (sound_data.pcm.len() / sound_data.channels.max(1)) as f32 / sound_data.rate.max(1) as f32;
            let pitch = lerp(0.95, 1.05, t).clamp(0.5, 2.);
            self.slots[i] = Some(Emitter {
                sound,
                looped: smp.looped,
                db: level.clamp(-48., 0.),
                pitch,
                position: cell.centre,
                min_distance: smp.min_distance,
                max_distance: smp.max_distance,
                doppler: if smp.looped && self.doppler_on { 0.5 } else { 0. },
                left: if smp.looped { f32::INFINITY } else { seconds / pitch },
            });
            out.push(Event::Start { slot: i });
        }
    }
}

/// 0x449FA0's level: gaspControl over the intensity, the cell's gasp over its head count and
/// gaspDistControl over the distance from the recoil to the cell centre; and that distance's
/// position in gaspDistControl's range, which sets the pitch.
fn gasp_level(g: &Gasps, curve: &Curve, intensity: f32, people: u32, at: [f32; 3], centre: [f32; 3]) -> (f32, f32) {
    let level = |c: &Curve, x: f32| lerp(c.min_vol, c.max_vol, linstep(c.min_vel, c.max_vel, x));
    let away = (0..3).map(|k| (at[k] - centre[k]).powi(2)).sum::<f32>().sqrt();
    let t = linstep(g.distance.min_vel, g.distance.max_vel, away);
    (level(&g.control, intensity) + level(curve, people as f32) + lerp(g.distance.min_vol, g.distance.max_vol, t), t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excitement_holds_then_alternates() {
        let mut crowd = Crowd::new(
            Arc::new(Bank { normal: empty(), grand: empty(), gasps: None, sounds: Vec::new().into() }),
            1,
        );
        crowd.since = 1.;
        crowd.excite(1.3, 2.);
        assert_eq!(crowd.reserved, 25);
        crowd.settle();
        assert_eq!(crowd.current, 1.3);
        // A second event inside the hold only changes the reserved slots.
        crowd.excite(0.5, 1.);
        assert_eq!((crowd.target, crowd.reserved), (1.3, 10));
        crowd.since = 2.5;
        crowd.settle();
        assert_eq!(crowd.current, 0.);
        crowd.settle();
        assert_eq!(crowd.current, 1.3);
        crowd.settle();
        assert_eq!(crowd.current, 0.);
    }

    fn empty() -> AutoPlay {
        AutoPlay {
            name: String::new(),
            bank: String::new(),
            density: 0.,
            max_loopers: 0,
            samples: Vec::new(),
            master: None,
            sounds: Vec::new().into(),
        }
    }

    /// 0x756C40 merges a car's contacts within 1 m and 0x76A620 broadcasts records above 2000
    /// one frame later, then waits a second.
    #[test]
    fn recoil_records_merge_threshold_and_cooldown() {
        let mut r = Recoils::default();
        let mut out = Vec::new();
        let side = [1., 0., 0.];
        r.add(0, [0., 0., 0.], side, 1200., true);
        r.add(0, [0.5, 0., 0.], side, 900., true);
        // Another car at the same point opens its own record.
        r.add(1, [0., 0., 0.], side, 2500., false);
        // Floor-like contact: no flag.
        r.add(2, [9., 0., 0.], [0., 1., 0.], 3000., true);
        assert_eq!(r.write.len(), 3);
        assert_eq!((r.write[0].weight, r.write[0].intensity), (2., 2100.));
        r.frame(0.016, &mut out);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0], Recoil { position: [0.25, 0., 0.], intensity: 2100., flag: true });
        assert!(!out[1].flag && !out[2].flag);
        // Within the second: nothing; the records are this frame's only.
        out.clear();
        r.add(0, [0., 0., 0.], side, 5000., true);
        r.frame(0.5, &mut out);
        r.frame(0.5, &mut out);
        assert!(out.is_empty());
        r.add(0, [0., 0., 0.], side, 5000., true);
        r.frame(0.016, &mut out);
        r.frame(0.016, &mut out);
        assert_eq!(out.len(), 1);
        // Ten records at most; 2000 itself does not broadcast.
        let mut r = Recoils::default();
        for i in 0..12 {
            r.add(0, [i as f32 * 2., 0., 0.], side, 2000., true);
        }
        assert_eq!(r.write.len(), RECORDS);
        out.clear();
        r.frame(0.016, &mut out);
        assert!(out.is_empty());
    }

    fn curve(variety: &str, vel: (f32, f32), vol: (f32, f32)) -> Curve {
        Curve {
            variety: variety.into(),
            min_vel: vel.0,
            max_vel: vel.1,
            min_vol: vol.0,
            max_vol: vol.1,
            min_pitch: 1.,
            max_pitch: 1.,
        }
    }

    /// 0x449FA0: one gasp per tick on entry j's cell in slot j, small cells skipped within the
    /// tick, a loop or a far cell ending the tick, and the level and pitch from the three curves.
    #[test]
    fn gasps_walk_the_query_one_cell_per_tick() {
        let one = || Sound { pcm: vec![0.; 22050].into(), channels: 1, rate: 22050, looped: false };
        let variety = |s| Variety { samples: vec![s], min_distance: 5., max_distance: 100. };
        let gasps = Gasps {
            group: (curve("normal_gasp", (0., 100.), (-20., 0.)), variety(0)),
            grand: (curve("grand_gasp", (0., 100.), (-10., 0.)), variety(1)),
            control: curve("", (5000., 30000.), (-12., 0.)),
            distance: curve("", (0., 50.), (0., -24.)),
        };
        let bank = Arc::new(Bank { normal: empty(), grand: empty(), gasps: Some(gasps), sounds: vec![one(), one()].into() });
        let cell = |x: f32, people, seated| Cell { min: [x; 3], max: [x; 3], centre: [x, 0., 0.], people, seated };
        let cells = Cells {
            origin: [0.; 2],
            size: [1.; 2],
            dims: [1; 2],
            map: Vec::new(),
            cells: vec![cell(0., 50, 0), cell(10., 3, 0), cell(20., 200, 100), cell(150., 200, 0)],
        };
        let entries = [(0., 0), (100., 1), (400., 2), (22500., 3)];
        let mut crowd = Crowd::new(bank, 3);
        let mut out = Vec::new();
        // Unflagged recoils are ignored.
        crowd.recoil(&Recoil { position: [0.; 3], intensity: 30000., flag: false });
        assert!(!crowd.gasping());
        crowd.recoil(&Recoil { position: [0.; 3], intensity: 30000., flag: true });
        // EndOfRace holds it.
        crowd.gasp_step(&cells, &entries, END_OF_RACE, &mut out);
        assert!(out.is_empty() && crowd.gasping());
        crowd.gasp_step(&cells, &entries, 0, &mut out);
        assert_eq!(out, [Event::Start { slot: 0 }]);
        let e = crowd.slots[0].unwrap();
        // 0 (intensity at the top) + lerp(−20, 0, 0.5) + 0 (at the recoil) = −10.
        assert_eq!((e.db, e.pitch, e.sound, e.looped), (-10., 0.95, 0, false));
        // Entry 1 has 3 people: skipped within the tick; entry 2 is a grandstand cell 20 m off.
        out.clear();
        crowd.gasp_step(&cells, &entries, 0, &mut out);
        assert_eq!(out, [Event::Start { slot: 2 }]);
        let e = crowd.slots[2].unwrap();
        assert_eq!(e.sound, 1);
        assert!((e.db - (0. + 0. - 24. * 0.4)).abs() < 1e-4, "{}", e.db);
        assert!((e.pitch - 0.99).abs() < 1e-6);
        // Entry 3 lies beyond MaxDistance (150 m > 100 m): this tick ends without a start …
        out.clear();
        crowd.gasp_step(&cells, &entries, 0, &mut out);
        assert!(out.is_empty() && crowd.gasping());
        // … and the list's end stops each later tick until the 35 entries are passed. The tick
        // that passes the last entry still returns there; 0x449FA0 clears +0x15C8 on the next.
        for _ in 4..SLOTS {
            crowd.gasp_step(&cells, &entries, 0, &mut out);
        }
        assert!(out.is_empty() && crowd.gasping());
        crowd.gasp_step(&cells, &entries, 0, &mut out);
        assert!(out.is_empty() && !crowd.gasping());
        // A loop in the slot stops the tick; a quiet gasp (low intensity) starts nothing.
        crowd.slots[0] = Some(Emitter { looped: true, ..e });
        crowd.recoil(&Recoil { position: [0.; 3], intensity: 30000., flag: true });
        crowd.gasp_step(&cells, &entries, 0, &mut out);
        assert!(out.is_empty() && crowd.gasping());
        crowd.slots[0] = None;
        let mut quiet = Crowd::new(crowd.bank.clone(), 3);
        // −12 (intensity at the bottom) − 10 (50 people) − 24 (50 m from the recoil) = −46.
        quiet.recoil(&Recoil { position: [50., 0., 0.], intensity: 2001., flag: true });
        quiet.gasp_step(&cells, &entries, 0, &mut out);
        assert!(out.is_empty() && quiet.slots[0].is_none() && quiet.gasping());
        // −36 itself still starts (0x449FA0 tests below −36): 200 people (0 dB) 150 m away.
        let mut edge = Crowd::new(crowd.bank.clone(), 3);
        edge.recoil(&Recoil { position: [0.; 3], intensity: 2001., flag: true });
        edge.gasp_step(&cells, &[(0., 3)], 0, &mut out);
        assert_eq!(out, [Event::Start { slot: 0 }]);
        assert_eq!(edge.slots[0].unwrap().db, -36.);
    }

    /// 0x771D30 orders the query nearest first.
    #[test]
    fn query_list_is_sorted_nearest_first() {
        let mut v: Vec<(f32, usize)> = (0..300).map(|i| (((i * 7919) % 301) as f32, i)).collect();
        shell_sort(&mut v);
        assert!(v.windows(2).all(|w| w[0].0 <= w[1].0));
        let mut ties = vec![(0., 0), (0., 1), (1., 2), (0., 3)];
        shell_sort(&mut ties);
        assert_eq!(ties.iter().map(|t| t.1).collect::<Vec<_>>(), [0, 1, 3, 2]);
    }

    /// The shipped crowd: seated sets, the excitement messages, the cells of three routes, and
    /// spawning at Washington's busiest cell and on its grid, calm and then excited.
    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn shipped_crowd_cells_and_spawns() {
        let root = std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required"));
        let defs = std::fs::read(root.join("anims/crowdDefs.xml")).unwrap();
        let seated = seated_sets(&defs).unwrap();
        println!("crowd version=1 seated_sets={seated:?}");
        assert!(seated.iter().any(|s| s == "m_gr") && seated.iter().any(|s| s == "f_gr"), "{seated:?}");
        // The excitement events: the speech data's isEventTrigger (ReactiveCrowd) messages.
        let rules = crate::speech::Rules::load(&root).unwrap();
        for (k, event) in rules.events.iter().enumerate().filter(|(_, e)| e.trigger) {
            for m in rules.messages.iter().filter(|m| m.event == k) {
                println!(
                    "crowd version=1 trigger_event={} on={} message={} level={} hold={} priority={}",
                    event.kind,
                    event.on,
                    m.name,
                    m.user_data,
                    m.queue_value,
                    m.priority
                );
            }
        }
        assert!(rules.events.iter().any(|e| e.trigger));
        let bank = Bank::load(&root).unwrap();
        println!(
            "crowd version=1 normal_samples={} grand_samples={} sounds={} normal_density={} grand_density={}",
            bank.normal.samples.len(),
            bank.grand.samples.len(),
            bank.sounds.len(),
            bank.normal.density,
            bank.grand.density
        );
        assert!(bank.normal.samples.iter().chain(&bank.grand.samples).all(|s| s.sound.is_some()));
        for path in ["tracks/usa/washington/route_0", "tracks/europe/donington/route_0", "tracks/europe/le_mans/route_0"] {
            let route = Route::new(path).unwrap();
            let cells = Cells::load(&root, &route).unwrap();
            let people: u32 = cells.cells.iter().map(|c| c.people).sum();
            let grand = cells.cells.iter().filter(|c| c.seated > 20).count();
            println!("crowd version=1 route={path} cells={} people={people} grandstand_cells={grand}", cells.cells.len());
            assert!(people > 0, "{path}");
        }
        // Spawning at the busiest cell of Washington.
        let route = Route::new("tracks/usa/washington/route_0").unwrap();
        let cells = Cells::load(&root, &route).unwrap();
        let busiest = cells.cells.iter().max_by_key(|c| c.people).unwrap();
        let at = busiest.centre;
        let near = cells.query(at, RADIUS, QUERY_MAX);
        println!("crowd version=1 query_at_busiest={} people_there={}", near.len(), busiest.people);
        assert!(!near.is_empty());
        let mut crowd = Crowd::new(bank, 9);
        let parked = Listener { position: at, forward: [0., 0., 1.], speed: 0. };
        let (mut out, mut starts, mut loud) = (Vec::new(), 0, -48f32);
        for _ in 0..62 * 30 {
            out.clear();
            crowd.tick(&cells, &parked, 0, at, 0.016, &mut out);
            starts += out.iter().filter(|e| matches!(e, Event::Start { .. })).count();
            for (_, e) in crowd.emitters() {
                loud = loud.max(e.db);
            }
        }
        println!("crowd version=1 window=parked_30s starts={starts} active={} loopers={} loudest={loud:.1}", crowd.emitters().count(), crowd.loopers());
        assert!(starts > 0);
        // Outside Intro the loops carry Doppler 0.5, the one-shots 0 (0x449F72).
        assert!(crowd.emitters().all(|(_, e)| e.doppler == if e.looped { 0.5 } else { 0. }));
        crowd.since = 100.;
        crowd.excite(1.3, 5.);
        let mut excited = 0;
        for _ in 0..62 * 5 {
            out.clear();
            crowd.tick(&cells, &parked, 0, at, 0.016, &mut out);
            excited += out.iter().filter(|e| matches!(e, Event::Start { .. })).count();
        }
        println!("crowd version=1 window=excited_5s starts={excited} excitement={}", crowd.excitement());
        // A hard hit on a wall beside the busiest cell (CrowdRecoil): the gasp walks the
        // query one cell per tick and starts gasp samples from both varieties' banks.
        let bank = crowd.bank.clone();
        let gasps = bank.gasps.as_ref().expect("CollisionInfos gasp curves and audioVarieties gasp varieties");
        for (name, (curve, variety)) in [("group_gasp", &gasps.group), ("grand_gasp", &gasps.grand)] {
            println!(
                "crowd version=1 gasp curve={name} variety={} samples={} people={}..{} vol={}..{} distance={}..{}",
                curve.variety, variety.samples.len(), curve.min_vel, curve.max_vel, curve.min_vol, curve.max_vol, variety.min_distance, variety.max_distance
            );
            assert!(!variety.samples.is_empty(), "{name}");
        }
        let c = &gasps.control;
        let d = &gasps.distance;
        println!(
            "crowd version=1 gasp control_vel={}..{} control_vol={}..{} distance_vel={}..{} distance_vol={}..{}",
            c.min_vel, c.max_vel, c.min_vol, c.max_vol, d.min_vel, d.max_vel, d.min_vol, d.max_vol
        );
        let is_gasp = |sound: usize| gasps.group.1.samples.contains(&sound) || gasps.grand.1.samples.contains(&sound);
        crowd.recoil(&Recoil { position: at, intensity: 20_000., flag: true });
        assert!(crowd.gasping());
        let (mut gasp_starts, mut gasp_db, mut ticks) = (0, Vec::new(), 0);
        while crowd.gasping() && ticks < 100 {
            out.clear();
            crowd.tick(&cells, &parked, 0, at, 0.016, &mut out);
            ticks += 1;
            for e in &out {
                if let Event::Start { slot } = e
                    && let Some(em) = crowd.slots[*slot].as_ref()
                    && is_gasp(em.sound)
                {
                    gasp_starts += 1;
                    gasp_db.push(em.db);
                }
            }
        }
        let loudest_gasp = gasp_db.iter().copied().fold(-48f32, f32::max);
        println!("crowd version=1 gasp intensity=20000 ticks={ticks} starts={gasp_starts} loudest_db={loudest_gasp:.1}");
        assert!(!crowd.gasping() && ticks <= 36, "the walk ends within 35 query entries");
        assert!(gasp_starts > 0 && gasp_db.iter().all(|&db| (GASP_QUIETEST..=0.).contains(&db)));
        // The sample windows (speed band in mph/100 or excitement, head count).
        for (name, play) in [("normal", &crowd.bank.normal), ("grand", &crowd.bank.grand)] {
            for smp in &play.samples {
                println!(
                    "crowd version=1 sample bank={name} looped={} speed={}..{} people={}..{} probability={} vol={}..{} distance={}..{}",
                    smp.looped, smp.min_speed, smp.max_speed, smp.min_spawn, smp.max_spawn, smp.probability, smp.min_vol, smp.max_vol, smp.min_distance, smp.max_distance
                );
            }
        }
        // Parked on Washington's grid (the app harness's start pose): calm, then the
        // `startingGrid` message (1.3, hold 0).
        let grid = [-9.78, 25.14, -496.76];
        let near = cells.query(grid, RADIUS, QUERY_MAX);
        let heads: Vec<u32> = near.iter().take(SLOTS).map(|&(_, c)| cells.cells[c].people).collect();
        println!("crowd version=1 grid_query={} people={heads:?}", near.len());
        let mut crowd = Crowd::new(crowd.bank.clone(), 9);
        let parked = Listener { position: grid, forward: [0., 0., 1.], speed: 0. };
        let mut run = |crowd: &mut Crowd, seconds: usize| {
            let mut starts = 0;
            for _ in 0..62 * seconds {
                out.clear();
                crowd.tick(&cells, &parked, 0, grid, 0.016, &mut out);
                starts += out.iter().filter(|e| matches!(e, Event::Start { .. })).count();
            }
            starts
        };
        let calm = run(&mut crowd, 10);
        crowd.excite(1.3, 0.);
        let excited = run(&mut crowd, 10);
        println!("crowd version=1 window=grid calm_starts={calm} excited_starts={excited} active={}", crowd.emitters().count());
        assert!(calm > 0 && excited > 0, "the grid's crowd stays silent");
    }
}
