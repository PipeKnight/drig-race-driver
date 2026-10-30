//! Crowds (docs/AUDIO_RESEARCH.md §31): the original's crowd object (constructor 0x427730,
//! tick 0x44E220) places up to 35 voices on the visual crowd's grid cells near a point ahead of
//! the listened car. Each tick it may spawn (0x4499A0) NormalCrowd or GrandStandCrowd samples
//! (autoplaybanks, bank `env_crowd`) on the cells within 150 m, with the level and pitch set
//! once from the cell's head count, and it stops looping voices the car has left behind
//! (0x41F280). Race events raise the excitement (the speech data's ReactiveCrowd messages,
//! 0x41F2D0), which lets the excited samples play. Group Crowd (11). Presentation only.
use crate::{
    ambience_audio::{AutoPlay, lerp, linstep},
    hud_audio::Sound,
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
/// The mix modes 0x4499A0 and 0x44E220 test (`[[AM+0x28]+0xBF0]`, 0x411C10).
pub const INTRO: usize = 5;
pub const END_OF_RACE: usize = 6;

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
    pub sounds: Arc<[Sound]>,
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
        Ok(Arc::new(Self { normal, grand, sounds: sounds.into() }))
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
    /// Seconds left of a one-shot; the slot frees when it ends (the voice handle E+0x14).
    left: f32,
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
            let play = if cell.seated > 20 { &bank.grand } else { &bank.normal };
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
                left: if smp.looped { f32::INFINITY } else { seconds / pitch },
            });
            out.push(Event::Start { slot: i });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excitement_holds_then_alternates() {
        let mut crowd = Crowd::new(
            Arc::new(Bank { normal: empty(), grand: empty(), sounds: Vec::new().into() }),
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
        crowd.since = 100.;
        crowd.excite(1.3, 5.);
        let mut excited = 0;
        for _ in 0..62 * 5 {
            out.clear();
            crowd.tick(&cells, &parked, 0, at, 0.016, &mut out);
            excited += out.iter().filter(|e| matches!(e, Event::Start { .. })).count();
        }
        println!("crowd version=1 window=excited_5s starts={excited} excitement={}", crowd.excitement());
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
