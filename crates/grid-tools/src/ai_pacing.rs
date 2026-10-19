//! Race manager pacing of the AI field, ported from the original
//! choreographer (`ai/ai_choreographer.xml`, game code 0x401000-0x40C000) and
//! catch-up (`ai/catchup.xml`, 0x90A000-0x917000): per AI car a corner-speed
//! scale A and floor B, an engine torque factor (engine +0xE8), a tyre grip
//! factor (wheel +0x80), the conveyor belt, race-start factors, the off-line
//! grip assist, lapping yield and scripted events (mistakes, tyre blow-outs,
//! engine failures) on cars just ahead of the human player.
//! docs/AI_MISTAKES_PACING_RESEARCH.md §2.6, §4 and
//! docs/AI_CHOREOGRAPHER_CATCHUP_RESEARCH.md (owner's research copy) §2-§3.
//!
//! The manager needs a reference car (the human player). Without one nothing
//! is computed and no vehicle field is written, so AI-only runs are unchanged.
//!
//! AUTHORED where not recovered: race distance is the distance along the AI
//! racing line (the original uses the progress track); the choreographer
//! RNG is a seeded MT19937 (the original uses the
//! game's global instance, whose stream depends on everything else that
//! draws from it); the conveyor runs in a single-class field (the live
//! Touring race showed one car at A = 1.1); places are 0-based.
use crate::ai_driver::{AiDriver, Line, NORMAL_DIFFICULTY};
use grid_formats::{
    ai::{self, EventKind},
    binxml,
};
use std::{fmt, path::Path};

/// Race settings that pace the AI field.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    /// AI difficulty 0..1 for the parameter mappings (live Normal 0.606).
    pub difficulty: f32,
    /// catchup.xml `user` config name, `None` = off.
    pub catch_up: Option<String>,
    /// Choreographer manager (conveyor, race-start factors, grip assist,
    /// yield, events).
    pub choreographer: bool,
    /// Choreographer events (mistakes, tyre blow-outs, engine failures).
    pub mistakes: bool,
}

/// The Race Day catch-up config of the default difficulty (Normal, star
/// level 1): [`race_day_catch_up`]`(NORMAL_DIFFICULTY, false)`.
pub const DEFAULT_CATCH_UP: &str = "rd_2_star";

/// The catch-up config a Race Day event selects (0x5D5530, game mode 5):
/// `rd_%d_star` with the event's star level + 1, and `_touge` appended for
/// the pro/midnight touge formats (enums 5/6). The star level is that of the
/// AI difficulty (`difficulty.ai_ratio`, [`crate::flashback::star_level`]);
/// Normal is level 1, so `rd_2_star`
/// (docs/AI_CHOREOGRAPHER_PORT.md §1).
pub fn race_day_catch_up(difficulty: f32, touge: bool) -> String {
    format!(
        "rd_{}_star{}",
        crate::flashback::star_level(difficulty) + 1,
        if touge { "_touge" } else { "" }
    )
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            difficulty: NORMAL_DIFFICULTY,
            catch_up: Some(race_day_catch_up(NORMAL_DIFFICULTY, false)),
            choreographer: true,
            mistakes: true,
        }
    }
}

impl Settings {
    pub const HELP: &str = "--ai SPEC (or an [ai] section in a settings file): AI race pacing, comma-separated, e.g. difficulty=0.606,catchup=on|touge|off|rd_2_star,choreographer=on|off,mistakes=on|off (default: difficulty 0.606, catch-up on = the Race Day config of the difficulty's star level, rd_2_star at Normal; touge = its _touge variant; choreographer and mistakes on).";

    /// `key=value` items separated by commas or new lines; `#` comments.
    pub fn parse(spec: &str) -> Result<Self, String> {
        let mut out = Self::default();
        // `on`/`touge` follow the final difficulty: Some(touge).
        let mut auto = Some(false);
        for item in spec
            .lines()
            .map(|l| {
                // `key = value` (settings file) as well as `key=value`.
                let l = l.split('#').next().unwrap_or("");
                l.split('=').map(str::trim).collect::<Vec<_>>().join("=")
            })
            .collect::<Vec<_>>()
            .iter()
            .flat_map(|l| l.split(|c: char| c == ',' || c.is_whitespace()))
            .filter(|i| !i.is_empty())
        {
            let (key, value) = item
                .split_once('=')
                .ok_or(format!("AI setting {item:?} needs key=value"))?;
            let (key, value) = (key.trim(), value.trim());
            let on = || match value {
                "on" | "true" | "1" => Ok(true),
                "off" | "false" | "0" => Ok(false),
                _ => Err(format!("AI setting {key} must be on or off")),
            };
            match key {
                "difficulty" => {
                    out.difficulty = match value {
                        "normal" => NORMAL_DIFFICULTY,
                        _ => value
                            .parse()
                            .ok()
                            .filter(|d: &f32| (0. ..=1.).contains(d))
                            .ok_or("AI difficulty must be 0..1 or normal")?,
                    }
                }
                "catchup" => {
                    auto = None;
                    out.catch_up = match value {
                        "off" | "none" | "false" | "0" => None,
                        "on" | "true" | "1" => {
                            auto = Some(false);
                            None
                        }
                        "touge" => {
                            auto = Some(true);
                            None
                        }
                        name if !name.is_empty()
                            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') =>
                        {
                            Some(name.into())
                        }
                        _ => return Err("invalid catch-up config name".into()),
                    }
                }
                "choreographer" => out.choreographer = on()?,
                "mistakes" => out.mistakes = on()?,
                _ => return Err(format!("unknown AI setting {key}")),
            }
        }
        if let Some(touge) = auto {
            out.catch_up = Some(race_day_catch_up(out.difficulty, touge));
        }
        Ok(out)
    }
}

impl fmt::Display for Settings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let on = |b: bool| if b { "on" } else { "off" };
        write!(
            f,
            "difficulty={},catchup={},choreographer={},mistakes={}",
            self.difficulty,
            self.catch_up.as_deref().unwrap_or("off"),
            on(self.choreographer),
            on(self.mistakes)
        )
    }
}

/// Shipped pacing data: the selected catch-up config and the `circuit`
/// choreographer set.
#[derive(Clone, Debug)]
pub struct Data {
    pub catch_up: Option<ai::CatchUp>,
    pub choreographer: ai::Choreographer,
}

impl Data {
    pub fn load(root: &Path, settings: &Settings) -> Result<Self, Box<dyn std::error::Error>> {
        Self::load_for(root, settings, "circuit")
    }

    /// [`Self::load`] with the choreographer set of `discipline` (e.g.
    /// `drift_battle`: manager on, no conveyor, events or race-start factors).
    pub fn load_for(
        root: &Path,
        settings: &Settings,
        discipline: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let catch_up = match &settings.catch_up {
            Some(user) => {
                let bytes = std::fs::read(root.join("ai/catchup.xml"))?;
                Some(
                    ai::catch_up(&binxml::read(&bytes)?, user)?
                        .ok_or(format!("catchup.xml has no config {user}"))?,
                )
            }
            None => None,
        };
        let bytes = std::fs::read(root.join("ai/ai_choreographer.xml"))?;
        let choreographer = ai::choreographer(&binxml::read(&bytes)?, discipline)?;
        Ok(Self {
            catch_up,
            choreographer,
        })
    }
}

/// MT19937 (the original's RNG algorithm, 0xA682A0) with the original's
/// float `U(a, b) = a + (b - a)·u32·2^-32` (0xA683B0) and inclusive integer
/// `lo + u32 mod (hi - lo + 1)` (0x778610).
#[derive(Clone)]
pub struct Mt {
    state: [u32; 624],
    index: usize,
}

impl Mt {
    pub fn new(seed: u32) -> Self {
        let mut state = [0u32; 624];
        state[0] = seed;
        for i in 1..624 {
            state[i] = 1_812_433_253u32
                .wrapping_mul(state[i - 1] ^ (state[i - 1] >> 30))
                .wrapping_add(i as u32);
        }
        Self { state, index: 624 }
    }
    pub fn next_u32(&mut self) -> u32 {
        if self.index >= 624 {
            for i in 0..624 {
                let y = (self.state[i] & 0x8000_0000) | (self.state[(i + 1) % 624] & 0x7fff_ffff);
                let mut v = self.state[(i + 397) % 624] ^ (y >> 1);
                if y & 1 != 0 {
                    v ^= 0x9908_b0df;
                }
                self.state[i] = v;
            }
            self.index = 0;
        }
        let mut y = self.state[self.index];
        self.index += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^ (y >> 18)
    }
    pub fn uniform(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * (self.next_u32() as f32 * (1. / 4_294_967_296.))
    }
    pub fn int(&mut self, lo: i64, hi: i64) -> i64 {
        lo + i64::from(self.next_u32()) % (hi - lo + 1).max(1)
    }
}

/// Catch-up power of one car (0x90CCE0): `gap` = car ahead of the reference
/// (positive), `distance` its race distance, `max_distance` the useLead-
/// extended maximum. `None` when inactive.
pub fn catch_up_power(
    c: &ai::CatchUp,
    speed: f32,
    distance: f32,
    reference: f32,
    max_distance: f32,
) -> Option<f32> {
    let v2 = speed * speed;
    let slow = v2 <= c.min_speed * c.min_speed || distance <= c.min_race_distance;
    let gap = if c.player_catchup {
        reference - distance
    } else {
        distance - reference
    };
    if c.enabled && !slow && gap > c.min_distance {
        let t = if max_distance == c.min_distance {
            0.
        } else {
            (gap - c.min_distance) / (max_distance - c.min_distance)
        };
        let (a, b) = (c.power_at_min_distance, c.power_at_max_distance);
        return Some((a + (b - a) * t).clamp(a.min(b), a.max(b)));
    }
    let behind = reference - distance;
    if c.reverse_enabled && !slow && behind > c.reverse_min_distance {
        let (x0, x1) = (c.reverse_min_distance, c.reverse_max_distance);
        let (y0, y1) = (
            c.reverse_power_at_min_distance,
            c.reverse_power_at_max_distance,
        );
        let t = if x1 == x0 {
            0.
        } else {
            (behind - x0) / (x1 - x0)
        };
        return Some((y0 + (y1 - y0) * t).clamp(y0.min(y1), y0.max(y1)));
    }
    None
}

/// Catch-up maximum distance with useLeadForMaxDistance (0x911590).
pub fn catch_up_max_distance(c: &ai::CatchUp, leader: f32, reference: f32) -> f32 {
    if c.use_lead_for_max_distance && leader - reference >= c.max_distance {
        leader - reference
    } else {
        c.max_distance
    }
}

/// Engine torque factor clamp (0x98F2D0).
pub fn torque_factor(f: f32) -> f32 {
    f.clamp(0.1, 2.)
}

/// Off-line grip assist and race-start grip factor (0x404C90): `distance`
/// is the car's race distance, `lateral` its front's signed offset from the
/// racing line (positive right) and `curvature` the look-ahead curvature
/// (0x798E90, positive for a bend toward positive offsets). The original
/// takes max_assist_inside when `lateral · curvature >= 0`, the inside of
/// the bend.
pub fn grip_factor(
    m: &ai::Manager,
    speed: f32,
    distance: f32,
    lateral: f32,
    curvature: f32,
) -> f32 {
    if speed <= 10. {
        return 1.;
    }
    if distance < m.race_start_distance[1] {
        return m.race_start_factor[1];
    }
    if curvature.abs() <= m.min_curve_offline {
        return 1.;
    }
    let assist = if lateral * curvature >= 0. {
        m.max_assist_inside
    } else {
        m.max_assist_outside
    };
    let span = m.max_distance_offline - m.min_distance_offline;
    let t = if span == 0. {
        0.
    } else {
        (lateral.abs() - m.min_distance_offline) / span
    };
    1. + (t * assist).clamp(0., assist)
}

/// One car of the field this tick.
pub struct Car<'a> {
    pub position: [f32; 3],
    pub velocity: [f32; 4],
    pub racing: bool,
    /// The AI driver, `None` for the human player.
    pub driver: Option<&'a mut AiDriver>,
}

/// Vehicle inputs for one AI car: engine torque factor (clamped) and wheel
/// grip scales (rear L/R, front L/R).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Output {
    pub torque: f32,
    pub grip: [f32; 4],
}

#[derive(Clone, Default)]
struct Record {
    distance: f64,
    last_arc: f64,
    grid: usize,
    a: f32,
    b: f32,
    power: f32,
    grip: f32,
    backwards: bool,
    failures: [bool; 4],
    damage: f32,
    radiator: f32,
    gate: bool,
}

#[derive(Clone)]
struct Candidate {
    car: usize,
    selected: bool,
    max: f32,
    current: f32,
}

#[derive(Clone)]
struct TriggerState {
    timer: f32,
    last: f32,
    count: u32,
    successes: u32,
    /// Pending event (index into the choreographer's events).
    pending: Option<usize>,
    window: (f32, f32),
    allowed: Vec<bool>,
    candidates: Vec<Candidate>,
}

#[derive(Clone, Copy, Debug)]
struct Running {
    trigger: usize,
    event: usize,
    car: usize,
    state: u8,
    elapsed: f32,
}

/// Conveyor slot: 1/3 sleeping before forward/backward, 2 forward, 4 backward.
#[derive(Clone, Copy, Debug)]
struct Slot {
    car: usize,
    state: u8,
    threshold: f64,
    target: i64,
}

/// The race manager for one race.
#[derive(Clone)]
pub struct Field {
    data: Data,
    settings: Settings,
    length: f64,
    laps: u32,
    start_arc: f64,
    reference: usize,
    mistakes: Vec<f32>,
    seed: u32,
    rng: Mt,
    records: Vec<Record>,
    triggers: Vec<TriggerState>,
    running: Option<Running>,
    slots: Vec<Slot>,
    /// Diagnostics of the last tick (events, conveyor changes).
    pub log: Vec<String>,
}

fn wrap(delta: f64, length: f64) -> f64 {
    (delta + length / 2.).rem_euclid(length) - length / 2.
}
fn speed(v: [f32; 4]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

impl Field {
    /// `start` is a point on the start-finish line, `positions` the grid,
    /// `reference` the human car and `mistakes` each car's `mistakes`
    /// characteristic.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        data: Data,
        settings: Settings,
        line: &Line,
        laps: u32,
        start: [f32; 3],
        positions: &[[f32; 3]],
        reference: usize,
        mistakes: Vec<f32>,
        seed: u32,
    ) -> Self {
        let mut field = Self {
            data,
            settings,
            length: f64::from(line.length),
            laps,
            start_arc: f64::from(line.arc(start)),
            reference,
            mistakes,
            seed,
            rng: Mt::new(seed),
            records: Vec::new(),
            triggers: Vec::new(),
            running: None,
            slots: Vec::new(),
            log: Vec::new(),
        };
        field.reset(line, positions);
        field
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Validation: fail one wheel of an AI car now, as a blow_tyre event does.
    pub fn blow_tyre(&mut self, car: usize, wheel: usize) {
        if car != self.reference && wheel < 4 {
            self.records[car].failures[wheel] = true;
        }
    }

    /// Race start (0x407EB0): records, triggers and the conveyor.
    pub fn reset(&mut self, line: &Line, positions: &[[f32; 3]]) {
        self.rng = Mt::new(self.seed);
        self.running = None;
        self.log.clear();
        self.records = positions
            .iter()
            .map(|&p| {
                let arc = f64::from(line.arc(p));
                Record {
                    distance: wrap(arc - self.start_arc, self.length),
                    last_arc: arc,
                    a: 1.,
                    power: 1.,
                    grip: 1.,
                    ..Record::default()
                }
            })
            .collect();
        let n = self.records.len();
        let mut grid: Vec<usize> = (0..n).collect();
        grid.sort_by(|&a, &b| {
            self.records[b]
                .distance
                .total_cmp(&self.records[a].distance)
        });
        for (g, &car) in grid.iter().enumerate() {
            self.records[car].grid = g;
        }
        let ai: Vec<usize> = (0..n).filter(|&i| i != self.reference).collect();
        self.triggers = self
            .data
            .choreographer
            .triggers
            .iter()
            .map(|t| TriggerState {
                timer: t.min_time,
                last: -f32::MAX,
                count: 0,
                successes: 0,
                pending: None,
                window: (0., 0.),
                allowed: vec![true; t.positions.len()],
                candidates: ai
                    .iter()
                    .map(|&car| {
                        let max =
                            (1. - self.mistakes.get(car).copied().unwrap_or(1.)).clamp(0., 1.);
                        Candidate {
                            car,
                            selected: false,
                            max,
                            current: 0.5 * max,
                        }
                    })
                    .collect(),
            })
            .collect();
        self.slots.clear();
        // Conveyor start (0x407940): one front-third car will drop back, one
        // back-third car will push forward.
        if let Some(c) = self
            .manager_on()
            .then(|| self.data.choreographer.conveyor.clone())
            .flatten()
            && n >= c.min_grid_size as usize
        {
            let n = n as i64;
            let back = self.rng.int(0, n / 3);
            let forward = self.rng.int(2 * n / 3, n - 1);
            for (state, g) in [(3, back), (1, forward)] {
                if let Some(car) = ai
                    .iter()
                    .copied()
                    .find(|&i| self.records[i].grid as i64 == g)
                {
                    let threshold = self.records[car].distance
                        + f64::from(self.rng.uniform(0., c.max_sleep_distance));
                    self.slots.push(Slot {
                        car,
                        state,
                        threshold,
                        target: 0,
                    });
                }
            }
        }
    }

    fn manager_on(&self) -> bool {
        self.settings.choreographer && self.data.choreographer.manager.enable
    }

    /// One manager tick, before the AI drivers update. Writes each driver's
    /// [`crate::ai_driver::Pace`] and returns the vehicle inputs of the AI
    /// cars (`None` for the human).
    pub fn tick(&mut self, line: &Line, cars: &mut [Car<'_>], dt: f32) -> Vec<Option<Output>> {
        self.log.clear();
        for (r, car) in self.records.iter_mut().zip(cars.iter()) {
            if car.racing {
                let arc = f64::from(line.arc(car.position));
                r.distance += wrap(arc - r.last_arc, self.length);
                r.last_arc = arc;
            }
        }
        let n = cars.len();
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by(|&a, &b| {
            self.records[b]
                .distance
                .total_cmp(&self.records[a].distance)
        });
        let mut place = vec![0usize; n];
        for (p, &car) in order.iter().enumerate() {
            place[car] = p;
        }
        let reference = self.records[self.reference].distance as f32;
        if self.manager_on() && cars[self.reference].racing {
            // 0x404B50 reset; A persists.
            for (i, r) in self.records.iter_mut().enumerate() {
                if i != self.reference {
                    r.power = 1.;
                    r.grip = 1.;
                    r.b = 0.;
                    r.backwards = false;
                }
            }
            self.conveyor(&place, n);
            if self.settings.mistakes {
                self.events(cars, &place, reference, dt);
            }
            self.lapping_yield();
            self.race_start(cars, reference);
        }
        // Catch-up (0x911590) and application (0x5C5B50, 0x408710).
        let leader = self
            .records
            .iter()
            .map(|r| r.distance as f32)
            .fold(f32::MIN, f32::max);
        let catch_up = self
            .data
            .catch_up
            .clone()
            .filter(|c| c.enabled || c.reverse_enabled);
        let max_distance = catch_up
            .as_ref()
            .map(|c| catch_up_max_distance(c, leader, reference));
        let mut out = Vec::with_capacity(n);
        for (i, car) in cars.iter_mut().enumerate() {
            let r = &self.records[i];
            let Some(driver) = car.driver.as_deref_mut().filter(|_| i != self.reference) else {
                out.push(None);
                continue;
            };
            driver.pace.a = r.a;
            driver.pace.b = r.b;
            driver.pace.backwards = r.backwards;
            driver.pace.failures = r.failures;
            driver.pace.damage = r.damage;
            driver.pace.damage_gate = r.gate;
            let power = match (&catch_up, max_distance) {
                (Some(c), Some(m)) => {
                    catch_up_power(c, speed(car.velocity), r.distance as f32, reference, m)
                        .unwrap_or(1.)
                }
                _ => 1.,
            };
            let grip = driver.request_grip().map(|g| g * r.grip);
            out.push(Some(Output {
                torque: torque_factor(power * r.power),
                grip,
            }));
        }
        out
    }

    /// [`Self::tick`] for a shared world whose car 0 is the human player held
    /// outside `multi` (grid-app and grid-soak; opponent k-1 drives car k),
    /// writing the AI cars' vehicle inputs.
    pub fn tick_with_player(
        &mut self,
        line: &Line,
        player: &crate::source_vehicle::Runtime,
        multi: &mut crate::source_vehicle::multi::MultiRuntime,
        drivers: &mut [AiDriver],
        racing: &[bool],
        dt: f32,
    ) {
        let at = |c: &crate::source_vehicle::Runtime| {
            let p = c.pose();
            [p[12], p[13], p[14]]
        };
        let mut cars = vec![Car {
            position: at(player),
            velocity: player.velocity(),
            racing: racing[0],
            driver: None,
        }];
        for (k, driver) in drivers.iter_mut().enumerate() {
            let c = &multi.cars[k + 1];
            cars.push(Car {
                position: at(c),
                velocity: c.velocity(),
                racing: racing[k + 1],
                driver: Some(driver),
            });
        }
        let outputs = self.tick(line, &mut cars, dt);
        drop(cars);
        for (car, output) in multi.cars.iter_mut().zip(outputs) {
            if let Some(o) = output {
                car.set_ai_pacing(o.torque, o.grip);
            }
        }
    }

    /// Conveyor belt slots (0x403E20).
    fn conveyor(&mut self, place: &[usize], n: usize) {
        let Some(c) = self.data.choreographer.conveyor.clone() else {
            return;
        };
        for k in 0..self.slots.len() {
            let mut s = self.slots[k];
            let car = s.car;
            let mut a = 1.;
            let p = place[car] as i64;
            match s.state {
                1 | 3 if self.records[car].distance > s.threshold => {
                    if s.state == 1 {
                        s.state = 2;
                        s.target = self
                            .rng
                            .int(c.forwards_place[0].into(), c.forwards_place[1].into());
                    } else {
                        s.state = 4;
                        s.target = n as i64
                            - self
                                .rng
                                .int(c.backwards_place[0].into(), c.backwards_place[1].into())
                            - 1;
                    }
                    self.log.push(format!(
                        "conveyor car={car} state={} target_place={} place={p}",
                        s.state, s.target
                    ));
                }
                2 if p > s.target => {
                    a = c.forwards_multiple;
                    self.records[car].power = c.forwards_multiple;
                }
                4 if p < s.target => {
                    a = c.backwards_multiple;
                    self.records[car].power = c.backwards_multiple;
                    self.records[car].backwards = true;
                }
                2 | 4 => {
                    s.state = if s.state == 2 { 3 } else { 1 };
                    s.threshold = self.records[car].distance
                        + f64::from(self.rng.uniform(0., c.max_sleep_distance));
                    self.log.push(format!(
                        "conveyor car={car} state={} place={p} threshold={:.0}",
                        s.state, s.threshold
                    ));
                }
                _ => {}
            }
            self.records[car].a = a;
            self.slots[k] = s;
        }
    }

    /// Lapping yield (0x404860): a car about to be lapped by any other car
    /// within lapping_yield_distance sets the backwards flag. Team-mate yield
    /// is not ported (no teams).
    fn lapping_yield(&mut self) {
        let m = &self.data.choreographer.manager;
        let length = self.length;
        let distances: Vec<f64> = self.records.iter().map(|r| r.distance).collect();
        for (i, r) in self.records.iter_mut().enumerate() {
            if i == self.reference {
                continue;
            }
            let (lo, hi) = (
                r.distance + length - f64::from(m.lapping_yield_distance),
                r.distance + length,
            );
            if distances
                .iter()
                .enumerate()
                .any(|(j, &d)| j != i && lo < d && d < hi)
            {
                r.backwards = true;
            }
        }
    }

    /// Race-start factors and the off-line grip assist (0x404C90).
    fn race_start(&mut self, cars: &[Car<'_>], reference: f32) {
        let m = self.data.choreographer.manager.clone();
        for (i, car) in cars.iter().enumerate() {
            let Some(driver) = car.driver.as_deref().filter(|_| i != self.reference) else {
                continue;
            };
            let r = &mut self.records[i];
            let d = r.distance as f32;
            if reference < d && d < m.race_start_distance[0] {
                r.power = m.race_start_factor[0];
                r.b = 1.;
            }
            r.grip = grip_factor(
                &m,
                speed(car.velocity),
                d,
                driver.telemetry.front_lateral,
                driver.telemetry.curvature,
            );
        }
    }

    /// Triggers (0x4059F0 roll, 0x406CF0 selection) and the single event
    /// runner (0x408E50).
    fn events(&mut self, cars: &mut [Car<'_>], place: &[usize], reference: f32, dt: f32) {
        let chor = self.data.choreographer.clone();
        let race = self.laps as f32 * self.length as f32;
        let length = self.length as f32;
        let running_trigger = self.running.map(|r| r.trigger);
        // Roll.
        for (t, (trigger, ts)) in chor.triggers.iter().zip(&mut self.triggers).enumerate() {
            if trigger.use_time_limit
                || ts.pending.is_some()
                || running_trigger == Some(t)
                || trigger.events.is_empty()
                || (trigger.max_events != 0 && ts.count >= trigger.max_events)
            {
                continue;
            }
            ts.timer -= dt;
            let gap = ts.last + trigger.min_event_separation - reference;
            if !(ts.timer < 0. && gap < 0.) {
                continue;
            }
            for c in &mut ts.candidates {
                c.selected = self.rng.uniform(0., 1.) < c.current * trigger.chance;
                c.current = (c.current + 0.1 * c.max).clamp(0., c.max);
            }
            for (allowed, &(_, chance)) in ts.allowed.iter_mut().zip(&trigger.positions) {
                *allowed = self.rng.uniform(0., 1.) < chance;
            }
            let end = if trigger.max_distance > 0. {
                trigger.max_distance.min(race)
            } else {
                race
            };
            let start = (trigger.min_race_distance * race).max(trigger.min_distance);
            if end > start && trigger.max_events > 0 {
                let step =
                    ((end - start) / trigger.max_events as f32).max(trigger.min_event_separation);
                let lo = start + ts.count as f32 * step;
                ts.window = (lo.clamp(start, end), (lo + step).min(end));
            }
            ts.count += 1;
            let k = self.rng.int(0, trigger.events.len() as i64 - 1) as usize;
            ts.pending = Some(trigger.events[k]);
        }
        // Selection.
        let mut selection = None;
        for (t, (trigger, ts)) in chor.triggers.iter().zip(&mut self.triggers).enumerate() {
            let Some(event) = ts.pending else {
                continue;
            };
            if reference > ts.window.1 {
                ts.pending = None;
                continue;
            }
            if reference <= ts.window.0 {
                continue;
            }
            let mut target: Option<(usize, f32)> = None;
            for c in ts
                .candidates
                .iter()
                .filter(|c| c.selected && cars[c.car].racing)
            {
                let disallowed = trigger
                    .positions
                    .iter()
                    .zip(&ts.allowed)
                    .any(|(&(index, _), &allowed)| index as i64 == place[c.car] as i64 && !allowed);
                if disallowed {
                    continue;
                }
                let ahead = (self.records[c.car].distance as f32 - reference).rem_euclid(length);
                if !(trigger.min_distance_ahead < ahead && ahead < trigger.max_distance_ahead) {
                    continue;
                }
                if target.is_none()
                    || (trigger.select_closest && target.is_some_and(|(_, a)| ahead < a))
                {
                    target = Some((c.car, ahead));
                }
                if !trigger.select_closest {
                    break;
                }
            }
            if let Some((car, _)) = target
                && speed(cars[car].velocity) >= chor.events[event].min_speed
            {
                selection = Some((t, event, car));
                break;
            }
        }
        // Runner.
        match (self.running, selection) {
            (None, Some(s)) => self.start(cars, s),
            (Some(r), Some(s)) if r.state == 1 && (r.trigger, r.event, r.car) != s => {
                if let EventKind::Mistake { .. } = chor.events[r.event].kind
                    && let Some(d) = cars[r.car].driver.as_deref_mut()
                {
                    d.disarm_mistake();
                }
                self.log.push(format!(
                    "event={} car={} aborted",
                    chor.events[r.event].name, r.car
                ));
                self.start(cars, s);
            }
            _ => {}
        }
        let Some(mut r) = self.running else {
            return;
        };
        let event = &chor.events[r.event];
        let previous = r.state;
        match event.kind {
            EventKind::Mistake { id, .. } => {
                let (armed, active) = cars[r.car]
                    .driver
                    .as_deref()
                    .map_or((None, None), AiDriver::mistake);
                if r.state == 1 && active == Some(id) {
                    r.state = 3;
                    r.elapsed = 0.;
                } else if r.state == 1 && armed != Some(id) {
                    r.state = 2;
                } else if r.state == 3 && active != Some(id) {
                    r.state = 4;
                }
            }
            // Done after 2 s; blow_engine then sets the damage gate.
            EventKind::BlowTyre | EventKind::BlowEngine { .. }
                if r.state == 3 && r.elapsed >= 2. =>
            {
                if let EventKind::BlowEngine { .. } = event.kind {
                    self.records[r.car].gate = true;
                }
                r.state = 4;
            }
            _ => {}
        }
        if r.state == 3 {
            r.elapsed += dt;
            if r.elapsed > event.max_active_time || !cars[r.car].racing {
                r.state = 2;
            }
        }
        if r.state != previous {
            self.log.push(format!(
                "event={} car={} trigger={} state={}",
                event.name, r.car, chor.triggers[r.trigger].name, r.state
            ));
        }
        match r.state {
            4 => {
                let ts = &mut self.triggers[r.trigger];
                ts.successes += 1;
                if let Some(c) = ts.candidates.iter_mut().find(|c| c.car == r.car) {
                    c.current *= 0.5;
                }
                ts.timer = chor.triggers[r.trigger].min_time;
                ts.last = reference;
                ts.pending = None;
                self.running = None;
            }
            2 => {
                self.triggers[r.trigger].pending = None;
                self.running = None;
            }
            _ => self.running = Some(r),
        }
    }

    /// Event start (0x4018A0 mistake, 0x404200 blow_tyre, 0x401730 blow_engine).
    fn start(&mut self, cars: &mut [Car<'_>], (trigger, event, car): (usize, usize, usize)) {
        let e = self.data.choreographer.events[event].clone();
        let mut state = 1;
        let mut detail = String::new();
        match e.kind {
            EventKind::Mistake { id, timeout } => {
                if let Some(d) = cars[car].driver.as_deref_mut() {
                    d.arm_mistake(id, timeout);
                }
            }
            EventKind::BlowTyre => {
                let w = self.rng.int(0, 3) as usize;
                self.records[car].failures[w] = true;
                state = 3;
                detail = format!(" wheel={w}");
            }
            EventKind::BlowEngine { radiator, engine } => {
                let r = &mut self.records[car];
                r.damage = r.damage.max(engine);
                r.radiator = r.radiator.max(radiator);
                state = 3;
                detail = format!(" engine={} radiator={}", r.damage, r.radiator);
            }
        }
        self.log.push(format!(
            "event={} car={car} trigger={} start ahead={:.1}{detail}",
            e.name,
            self.data.choreographer.triggers[trigger].name,
            (self.records[car].distance - self.records[self.reference].distance)
                .rem_euclid(self.length)
        ));
        self.running = Some(Running {
            trigger,
            event,
            car,
            state,
            elapsed: 0.,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rd(min: f32, max: f32, p_max: f32) -> ai::CatchUp {
        ai::CatchUp {
            enabled: true,
            min_distance: min,
            max_distance: max,
            power_at_min_distance: 1.,
            power_at_max_distance: p_max,
            min_speed: 12.,
            min_race_distance: 500.,
            use_lead_for_max_distance: true,
            reverse_enabled: true,
            reverse_min_distance: 50.,
            reverse_max_distance: 400.,
            reverse_power_at_min_distance: 1.,
            reverse_power_at_max_distance: 1.2,
            ..ai::CatchUp::default()
        }
    }

    #[test]
    fn mt19937_matches_the_reference_stream() {
        let mut mt = Mt::new(5489);
        assert_eq!(mt.next_u32(), 3_499_211_612);
        assert_eq!(mt.next_u32(), 581_869_302);
        for _ in 2..9999 {
            mt.next_u32();
        }
        assert_eq!(mt.next_u32(), 4_123_659_995);
        let mut mt = Mt::new(1);
        for _ in 0..1000 {
            let x = mt.int(0, 3);
            assert!((0..=3).contains(&x));
            let u = mt.uniform(0., 1.);
            assert!((0. ..=1.).contains(&u));
        }
    }

    #[test]
    fn catch_up_tables() {
        let c = rd(50., 200., 0.5);
        let p = |gap: f32| catch_up_power(&c, 30., 2000. + gap, 2000., 200.);
        // rd_3_star ahead: 50 m -> inactive (gap must exceed minDistance).
        assert_eq!(p(50.), None);
        assert_eq!(p(125.), Some(0.75));
        assert_eq!(p(200.), Some(0.5));
        assert_eq!(p(300.), Some(0.5));
        // Behind: 50 m inactive, 225 m 1.1, 400 m+ 1.2.
        assert_eq!(p(-50.), None);
        assert!((p(-225.).unwrap() - 1.1).abs() < 1e-6);
        assert_eq!(p(-400.), Some(1.2));
        assert_eq!(p(-600.), Some(1.2));
        // Slow cars and the first 500 m are never paced.
        assert_eq!(catch_up_power(&c, 12., 1125., 1000., 200.), None);
        assert_eq!(catch_up_power(&c, 30., 400., 275., 200.), None);
        // useLeadForMaxDistance stretches the ramp to the leader's gap.
        assert_eq!(catch_up_max_distance(&c, 1100., 1000.), 200.);
        assert_eq!(catch_up_max_distance(&c, 1400., 1000.), 400.);
        assert_eq!(catch_up_power(&c, 30., 1225., 1000., 400.), Some(0.75));
        // 5-6 stars: forward off, the reverse boost stays.
        let off = ai::CatchUp {
            enabled: false,
            ..c.clone()
        };
        assert_eq!(catch_up_power(&off, 30., 1200., 1000., 200.), None);
        assert_eq!(catch_up_power(&off, 30., 600., 1000., 200.), Some(1.2));
        // The engine factor clamp.
        assert_eq!(torque_factor(0.05), 0.1);
        assert_eq!(torque_factor(2.5), 2.);
        assert_eq!(torque_factor(1.05 * 1.2), 1.05 * 1.2);
    }

    #[test]
    fn race_start_and_off_line_grip() {
        let m = ai::Manager {
            enable: true,
            race_start_distance: [100., 750.],
            race_start_factor: [1.05, 1.25],
            min_distance_offline: 0.,
            max_distance_offline: 4.,
            max_assist_inside: 0.25,
            max_assist_outside: 0.5,
            min_curve_offline: 0.001,
            ..ai::Manager::default()
        };
        assert_eq!(grip_factor(&m, 30., 500., 2., 0.01), 1.25);
        assert_eq!(grip_factor(&m, 9., 500., 2., 0.01), 1.);
        // Right-hand bend (curvature > 0): right of the line (positive) is inside.
        assert_eq!(grip_factor(&m, 30., 1000., 2., 0.01), 1.125);
        assert_eq!(grip_factor(&m, 30., 1000., -2., 0.01), 1.25);
        assert_eq!(grip_factor(&m, 30., 1000., -6., 0.01), 1.5);
        assert_eq!(grip_factor(&m, 30., 1000., 2., 0.0005), 1.);
    }

    #[test]
    fn settings_parse_and_print() {
        let s = Settings::parse("difficulty = 0.8, catchup=off\nmistakes = off # x").unwrap();
        assert_eq!(s.difficulty, 0.8);
        assert_eq!(s.catch_up, None);
        assert!(s.choreographer && !s.mistakes);
        assert_eq!(Settings::parse(&s.to_string()).unwrap(), s);
        assert_eq!(
            Settings::parse("catchup=on").unwrap().catch_up.as_deref(),
            Some(DEFAULT_CATCH_UP)
        );
        // 0x5D5530: star level + 1, whatever the order of the items.
        let name = |s: &str| Settings::parse(s).unwrap().catch_up.unwrap();
        assert_eq!(name("catchup=on, difficulty=0.506"), "rd_1_star");
        assert_eq!(name("difficulty=0.706, catchup=on"), "rd_3_star");
        assert_eq!(name("difficulty=1"), "rd_6_star");
        assert_eq!(name("catchup=touge"), "rd_2_star_touge");
        assert_eq!(name("catchup=rd_4_star_h2h"), "rd_4_star_h2h");
        assert_eq!(Settings::parse("").unwrap(), Settings::default());
        assert!(Settings::parse("difficulty=2").is_err());
        assert!(Settings::parse("speed=1").is_err());
        assert!(Settings::parse("mistakes=maybe").is_err());
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT"]
    fn shipped_pacing_data() {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let data = Data::load(Path::new(&root), &Settings::default()).unwrap();
        let c = data.catch_up.unwrap();
        assert_eq!((c.min_distance, c.max_distance), (50., 200.));
        // rd_2_star, the Normal Race Day config (F439).
        assert_eq!(c.power_at_max_distance, 0.45);
        assert!(c.reverse_enabled && c.use_lead_for_max_distance && !c.player_catchup);
        let chor = &data.choreographer;
        assert!(chor.manager.enable);
        assert_eq!(chor.manager.race_start_factor, [1.05, 1.25]);
        assert_eq!(chor.rev_length, 10.);
        let belt = chor.conveyor.as_ref().unwrap();
        assert_eq!((belt.min_grid_size, belt.forwards_multiple), (6, 1.1));
        assert_eq!(belt.backwards_place, [0, 1]);
        let names: Vec<&str> = chor.triggers.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(
            names,
            ["mechanical_failures", "major_errors", "minor_errors"]
        );
        assert_eq!(chor.triggers[1].positions, [(0, 0.33), (1, 0.66)]);
        assert_eq!(chor.triggers[2].max_events, 100);
        let lock = &chor.events[chor.triggers[2].events[0]];
        assert_eq!(
            lock.kind,
            EventKind::Mistake {
                id: crate::ai_driver::BRAKE_LOCK,
                timeout: 60.
            }
        );
        assert_eq!(lock.min_speed, 30.);
        let s5 = ai::catch_up(
            &binxml::read(&std::fs::read(Path::new(&root).join("ai/catchup.xml")).unwrap())
                .unwrap(),
            "rd_5_star",
        )
        .unwrap()
        .unwrap();
        assert!(!s5.enabled && s5.reverse_enabled);
    }
}
