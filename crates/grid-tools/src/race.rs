//! Race standings and classification for a shared-world event, following the
//! original race manager where it is recovered (docs/RACE_RULES_RESEARCH.md):
//!
//! - a car finishes on the line crossing that completes the event's laps,
//!   lapped cars included; a single-player lap race is over once every car
//!   has finished [V 0x912170, 0x90e1b0 type 1];
//! - finished cars rank by finish time ahead of every unfinished car, and
//!   unfinished cars rank by race progress [V 0x908cf0];
//! - when the race is ended early ([`Standings::finalize`]), a car still
//!   racing above 5 mph gets its remaining distance estimated, a slower one
//!   and an unfinished human are retired [V 0x90fcd0, 0x911f30];
//! - the chequered flag is shown when the leader is on the last lap within
//!   250 m of the line [V 0x94aa70, rules.xml end_race].
//!
//! A time-limited race (the original's race type 3, Le Mans) is over when its
//! time limit expires: nobody finishes at the line, laps count up to the
//! event laps (unlimited for 0 or 52+), every car still racing at expiry is
//! classified, and classified cars rank by race progress [V 0x669700,
//! 0x912170, 0x90E1B0, 0x90F880, 0x908CF0; docs/LE_MANS.md §2].
//!
//! Race progress is the distance along the AI racing line from the
//! start-finish line, unwrapped tick by tick (the original uses its own lap
//! count plus lap fraction). The remaining-distance estimate is AUTHORED: the
//! original predicts each remaining lap from a per-vehicle reference table
//! with random variation (0x67FFB0), which is not ported.
use crate::ai_driver::Line;

/// Below this speed a car still racing at the end is retired instead of
/// estimated: 5 mph (0xCB093C converted by 0xA7DDB0, x 1.609344 / 3.6).
pub const FINALIZE_MINIMUM_SPEED: f64 = 5. * 1.609_344 / 3.6;
/// rules.xml `end_race distance_to_line` (single_race rule set).
pub const END_RACE_DISTANCE: f64 = 250.;
/// Event lap counts of 0 or at least 52 mean unlimited laps (counted up to
/// 50 internally) [V 0x90E020].
pub const UNLIMITED_LAPS: u32 = 52;

/// Why a car left the race.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Out {
    /// Below 5 mph when the race was ended [V].
    Stopped,
    /// Terminal damage (wheel, engine or gearbox failure) [V for the human:
    /// the wrecked screen retires; AI cars retired on it are AUTHORED].
    Wrecked,
    /// Retired by the player, or a human still racing when the race ended [V].
    Retired,
    /// Disqualified (Demolition Derby: 0x9E64E0(6, 0) for a flipped player
    /// or an AI car that stopped; docs/DEMOLITION_DERBY.md) [V].
    Disqualified,
}

/// Classification state of one car.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Racing,
    Finished,
    /// Finish time estimated when the race was ended (in a time-limited
    /// race: distance projected to the limit, entry +0xBF4).
    Estimated,
    Out(Out),
}

/// Flag shown for the race leader.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flag {
    Chequered,
}

#[derive(Clone, Debug)]
struct Car {
    distance: f64,
    last_arc: f64,
    finish: Option<f64>,
    status: Status,
    /// Race time at each completed lap (distance past k line lengths).
    laps: Vec<f64>,
}

#[derive(Clone, Debug)]
pub struct Standings {
    laps: u32,
    length: f64,
    start_arc: f64,
    cars: Vec<Car>,
    finalized: bool,
    /// Real time limit (s) of a time-limited race (race type 3).
    limit: Option<f64>,
}

/// `m:ss.mmm`.
pub fn format_time(seconds: f64) -> String {
    let milliseconds = (seconds.max(0.) * 1000.).round() as u64;
    format!(
        "{}:{:02}.{:03}",
        milliseconds / 60000,
        milliseconds / 1000 % 60,
        milliseconds % 1000
    )
}

/// Le Mans results time `MM:SS.cc` with total minutes (`%02u:%02u.%02u`,
/// 0x908860; 24 h prints as `1440:00.00`), `--:--:--` for zero [V 0x53FE10].
pub fn format_lemans_time(seconds: f64) -> String {
    let centiseconds = (seconds.max(0.) * 100.).round() as u64;
    if centiseconds == 0 {
        return "--:--:--".into();
    }
    format!(
        "{:02}:{:02}.{:02}",
        centiseconds / 6000,
        centiseconds / 100 % 60,
        centiseconds % 100
    )
}

fn wrap(delta: f64, length: f64) -> f64 {
    (delta + length / 2.).rem_euclid(length) - length / 2.
}

/// Zero-based grid slot from an event's one-based `base` and `range`
/// (event_track_pool player_grid_position_base/range, event_driver_grid_position
/// grid_position_base/range) in a field of `field` cars [V 0x5BE470, 0x5C2AC0]:
/// base is clamped to the field, range to the slots behind it, and the slot is
/// base - 1 plus `draw(range)`, the original's random integer below `range`
/// (0xA683E0; exclusive upper bound [I]).
pub fn grid_slot(base: u32, range: u32, field: usize, draw: impl FnOnce(u32) -> u32) -> usize {
    let first = (base.max(1) as usize).min(field.max(1)) - 1;
    let range = (range as usize).min(field.saturating_sub(first));
    if range > 0 {
        first + (draw(range as u32) as usize).min(range - 1)
    } else {
        first
    }
}

impl Standings {
    /// `start` is a point on the start-finish line; `positions` are the cars' grid positions.
    pub fn new(line: &Line, laps: u32, start: [f32; 3], positions: &[[f32; 3]]) -> Self {
        let length = f64::from(line.length);
        let start_arc = f64::from(line.arc(start));
        let mut standings = Self {
            laps,
            length,
            start_arc,
            cars: Vec::new(),
            finalized: false,
            limit: None,
        };
        standings.reset(line, positions);
        standings
    }

    /// Standings of a point-to-point run (one "lap" from `start` to
    /// `finish`, both points on the open line): the target is the line
    /// distance between them instead of the line length. [D] Touge events
    /// run one lap (`event_track_pool.num_laps` 1).
    pub fn new_run(line: &Line, start: [f32; 3], finish: [f32; 3], positions: &[[f32; 3]]) -> Self {
        let start_arc = f64::from(line.arc(start));
        let length = (f64::from(line.arc(finish)) - start_arc).max(1.);
        let mut standings = Self {
            laps: 1,
            length,
            start_arc,
            cars: Vec::new(),
            finalized: false,
            limit: None,
        };
        standings.reset(line, positions);
        standings
    }

    /// Restart from grid positions (cars behind the line start below zero).
    pub fn reset(&mut self, line: &Line, positions: &[[f32; 3]]) {
        self.finalized = false;
        self.cars = positions
            .iter()
            .map(|&p| {
                let arc = f64::from(line.arc(p));
                Car {
                    distance: wrap(arc - self.start_arc, self.length),
                    last_arc: arc,
                    finish: None,
                    status: Status::Racing,
                    laps: Vec::new(),
                }
            })
            .collect();
    }

    /// Standings on an abstract loop of `length` m with the cars at the given
    /// distances from the line (arc = distance mod length): tests and tools
    /// that drive [`Self::step_arcs`] directly.
    pub fn on_loop(laps: u32, length: f64, distances: &[f64]) -> Self {
        Self {
            laps,
            length,
            start_arc: 0.,
            cars: distances
                .iter()
                .map(|&d| Car {
                    distance: d,
                    last_arc: d.rem_euclid(length),
                    finish: None,
                    status: Status::Racing,
                    laps: Vec::new(),
                })
                .collect(),
            finalized: false,
            limit: None,
        }
    }

    /// A time-limited race (race type 3) of `seconds` real race time from GO:
    /// over when the race time reaches it, no finish at the line [V].
    pub fn with_time_limit(mut self, seconds: f64) -> Self {
        self.limit = (seconds.is_finite() && seconds > 0.).then_some(seconds);
        self
    }

    /// The real time limit (s) of a time-limited race.
    pub fn time_limit(&self) -> Option<f64> {
        self.limit
    }

    /// Laps a time-limited race counts: the event laps, unlimited for 0 or
    /// [`UNLIMITED_LAPS`]+ [V 0x90E020, 0x912170].
    fn lap_cap(&self) -> Option<usize> {
        (self.laps > 0 && self.laps < UNLIMITED_LAPS).then_some(self.laps as usize)
    }

    /// [`Self::update`] with the cars' arc positions along the line.
    pub fn step_arcs(&mut self, arcs: &[f64], time: f64, external: &[usize]) {
        self.step(arcs, time, external);
    }

    pub fn laps(&self) -> u32 {
        self.laps
    }
    pub fn len(&self) -> usize {
        self.cars.len()
    }
    pub fn is_empty(&self) -> bool {
        self.cars.is_empty()
    }
    fn target(&self) -> f64 {
        f64::from(self.laps) * self.length
    }

    /// Advance with the cars' current positions at race time `time` (s from GO).
    /// Cars listed in `external` finish only through [`Self::finish`] or
    /// [`Self::line_crossing`] (every car with lap timing).
    pub fn update(&mut self, line: &Line, positions: &[[f32; 3]], time: f64, external: &[usize]) {
        let arcs: Vec<f64> = positions.iter().map(|&p| f64::from(line.arc(p))).collect();
        self.step(&arcs, time, external);
    }

    /// [`Self::update`] with the cars' arc positions along the line.
    fn step(&mut self, arcs: &[f64], time: f64, external: &[usize]) {
        let timed = self.limit.is_some();
        if timed && self.finalized {
            return;
        }
        let target = self.target();
        let cap = if timed {
            self.lap_cap()
        } else {
            Some(self.laps as usize)
        };
        for (i, (car, &arc)) in self.cars.iter_mut().zip(arcs).enumerate() {
            if car.status != Status::Racing {
                continue;
            }
            car.distance += wrap(arc - car.last_arc, self.length);
            car.last_arc = arc;
            while cap.is_none_or(|c| car.laps.len() < c)
                && car.distance >= (car.laps.len() + 1) as f64 * self.length
            {
                car.laps.push(time);
            }
            // [V 0x912170] no finish at the line in a time-limited race.
            if !timed && car.distance >= target && !external.contains(&i) {
                car.finish = Some(time);
                car.status = Status::Finished;
            }
        }
        // [V 0x90E1B0 type 3, 0x90F880] at the limit the race ends at once and
        // every car still racing is classified with the limit as its time.
        if let Some(limit) = self.limit
            && time >= limit
        {
            for car in &mut self.cars {
                if car.status == Status::Racing {
                    car.finish = Some(limit);
                    car.status = Status::Finished;
                }
            }
            self.finalized = true;
        }
    }

    /// Race progress of `car` in laps: completed laps (capped at the counted
    /// laps in a time-limited race) plus the fraction of the current lap
    /// (the original's vf18, laps + lap fraction).
    pub fn progress(&self, car: usize) -> f64 {
        let laps = self.cars[car].distance / self.length;
        match (self.limit, self.lap_cap()) {
            (Some(_), Some(cap)) if laps >= cap as f64 => cap as f64 + laps.fract(),
            _ => laps,
        }
    }

    /// A car finishes (the player through its lap timing); ignored in a
    /// time-limited race, where nobody finishes at the line [V].
    pub fn finish(&mut self, car: usize, time: f64) {
        if self.limit.is_none()
            && let Some(c) = self.cars.get_mut(car)
            && c.status == Status::Racing
        {
            c.finish = Some(time);
            c.status = Status::Finished;
        }
    }

    /// A lap-timing line crossing that completes lap `number` of `car`: the car
    /// finishes when it completes the event laps [V 0x912170: every car's
    /// laps come from its line crossings]. Cars with lap timing are passed
    /// to [`Self::update`] as `external`, so the racing-line distance never
    /// finishes them: from a wide, slanted start gate (Okutama route 2) that
    /// distance runs out before the car reaches the gate.
    pub fn line_crossing(&mut self, car: usize, number: u32, time: f64) {
        if number >= self.laps {
            self.finish(car, time);
        }
    }

    /// Take a car still racing out of the race.
    pub fn retire(&mut self, car: usize, why: Out) {
        if let Some(c) = self.cars.get_mut(car)
            && c.status == Status::Racing
        {
            c.status = Status::Out(why);
        }
    }

    /// End the race at `time` (the original's finalisation when a race is
    /// skipped, retired from or left). `speeds` are the cars' speeds (m/s) and
    /// `humans` the human-driven cars.
    ///
    /// - A human still racing is retired [V].
    /// - If a human did not finish, every other car still racing is estimated;
    ///   otherwise only cars above [`FINALIZE_MINIMUM_SPEED`] are, and the rest
    ///   are retired as stopped [V].
    /// - Estimate (AUTHORED): remaining distance at the car's mean race speed
    ///   so far, or its current speed (floor 1 m/s) before it has covered any.
    pub fn finalize(&mut self, speeds: &[f64], time: f64, humans: &[usize]) {
        if let Some(limit) = self.limit {
            self.finalize_timed(speeds, time, limit, humans);
            return;
        }
        let target = self.target();
        let human_unfinished = humans.iter().any(|&h| {
            self.cars
                .get(h)
                .is_some_and(|c| !matches!(c.status, Status::Finished))
        });
        for (i, (car, &speed)) in self.cars.iter_mut().zip(speeds).enumerate() {
            if car.status != Status::Racing {
                continue;
            }
            if humans.contains(&i) {
                car.status = Status::Out(Out::Retired);
            } else if human_unfinished || speed > FINALIZE_MINIMUM_SPEED {
                let mean = if car.distance > 0. && time > 0. {
                    car.distance / time
                } else {
                    speed
                };
                car.finish = Some(time + (target - car.distance).max(0.) / mean.max(1.));
                car.status = Status::Estimated;
            } else {
                car.status = Status::Out(Out::Stopped);
            }
        }
        self.finalized = true;
    }

    /// Time-limited finalisation [V 0x911F30, 0x90F880]: at or past the limit
    /// every car still racing is classified with the limit as its time; a
    /// human ending the race early retires and every AI car still racing is
    /// projected to the limit and marked estimated. Projection (AUTHORED, as
    /// in [`Self::finalize`]): the car's mean race speed so far; the original
    /// adds AI-estimator laps (0x67FFB0, not ported).
    fn finalize_timed(&mut self, speeds: &[f64], time: f64, limit: f64, humans: &[usize]) {
        let short = time < limit;
        for (i, (car, &speed)) in self.cars.iter_mut().zip(speeds).enumerate() {
            if car.status != Status::Racing {
                continue;
            }
            if !short {
                car.finish = Some(limit);
                car.status = Status::Finished;
            } else if humans.contains(&i) {
                car.status = Status::Out(Out::Retired);
            } else {
                let mean = if car.distance > 0. && time > 0. {
                    car.distance / time
                } else {
                    speed
                };
                car.distance += (limit - time) * mean.max(0.);
                car.finish = Some(limit);
                car.status = Status::Estimated;
            }
        }
        self.finalized = true;
    }

    /// The race is over: ended with [`Self::finalize`], or no car is still
    /// racing (every car finished, or finished or out).
    pub fn over(&self) -> bool {
        self.finalized || self.cars.iter().all(|c| c.status != Status::Racing)
    }

    pub fn status(&self, car: usize) -> Status {
        self.cars[car].status
    }

    pub fn finished(&self, car: usize) -> Option<f64> {
        self.cars.get(car).and_then(|c| c.finish)
    }

    /// Completed laps of `car` and their times (s), measured on the race
    /// progress line from GO (AUTHORED: the original times each car's laps
    /// with its own track tracker). Estimated laps are not included.
    pub fn lap_times(&self, car: usize) -> Vec<f64> {
        let mut previous = 0.;
        self.cars[car]
            .laps
            .iter()
            .map(|&t| {
                let lap = t - previous;
                previous = t;
                lap
            })
            .collect()
    }

    /// Fastest completed lap of `car`: (seconds, one-based lap number).
    pub fn best_lap(&self, car: usize) -> Option<(f64, usize)> {
        self.lap_times(car)
            .into_iter()
            .enumerate()
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, t)| (t, i + 1))
    }

    /// Completed race distance of `car` in laps (may be negative before the line).
    pub fn laps_covered(&self, car: usize) -> f64 {
        self.cars[car].distance / self.length
    }

    /// Car indices from first to last: finishers by time, then estimated
    /// finishers by estimated time, then every other car by race progress
    /// (cars still racing and cars out alike, as in 0x908CF0).
    pub fn order(&self) -> Vec<usize> {
        if self.limit.is_some() {
            // [V 0x908CF0 type 3] classified cars first, then the rest, each
            // by progress.
            let classified =
                |c: usize| matches!(self.cars[c].status, Status::Finished | Status::Estimated);
            let mut order: Vec<usize> = (0..self.cars.len()).collect();
            order.sort_by(|&a, &b| {
                classified(b)
                    .cmp(&classified(a))
                    .then_with(|| self.progress(b).total_cmp(&self.progress(a)))
            });
            return order;
        }
        let rank = |c: &Car| match c.status {
            Status::Finished => 0,
            Status::Estimated => 1,
            Status::Racing | Status::Out(_) => 2,
        };
        let mut order: Vec<usize> = (0..self.cars.len()).collect();
        order.sort_by(|&a, &b| {
            let (ca, cb) = (&self.cars[a], &self.cars[b]);
            rank(ca)
                .cmp(&rank(cb))
                .then_with(|| match (ca.finish, cb.finish) {
                    (Some(x), Some(y)) if rank(ca) < 2 => x.total_cmp(&y),
                    _ => cb.distance.total_cmp(&ca.distance),
                })
        });
        order
    }

    /// Le Mans results order [V 0x4EC820]: every car by progress, times its
    /// `vehicle_class.lemans_progress_adjustment` (`adjustment[car]`) when its
    /// distance was estimated, descending, with no priority for classified
    /// cars; ties keep the running order.
    pub fn lemans_results_order(&self, adjustment: &[f64]) -> Vec<usize> {
        let value = |c: usize| {
            let scale = if self.cars[c].status == Status::Estimated {
                adjustment.get(c).copied().unwrap_or(1.)
            } else {
                1.
            };
            self.progress(c) * scale
        };
        let mut order = self.order();
        order.sort_by(|&a, &b| value(b).total_cmp(&value(a)));
        order
    }

    /// Zero-based position of each car within its class in the running
    /// order [V 0x90B5B0]: the class is `vehicle_class.lemans_sort_order`
    /// (LMP1 1, LMP2 2, GT1 3, GT2 4, other cars 0) clamped to 0..=5.
    pub fn class_positions(&self, classes: &[i32]) -> Vec<usize> {
        let mut counters = [0usize; 6];
        let mut positions = vec![0; self.cars.len()];
        for c in self.order() {
            let class = classes.get(c).copied().unwrap_or(0).clamp(0, 5) as usize;
            positions[c] = counters[class];
            counters[class] += 1;
        }
        positions
    }

    /// Flag for the race leader: chequered once the leader is on the last
    /// lap within [`END_RACE_DISTANCE`] of the line, or has finished.
    pub fn leader_flag(&self) -> Option<Flag> {
        if self.limit.is_some() {
            return None;
        }
        let leader = &self.cars[*self.order().first()?];
        let to_go = self.target() - leader.distance;
        (leader.finish.is_some() || (to_go <= END_RACE_DISTANCE && to_go < self.length))
            .then_some(Flag::Chequered)
    }

    /// Race distance (m) to the car directly ahead and directly behind `car`
    /// in the running order; `None` at the ends or once either is no longer racing.
    pub fn neighbour_gaps(&self, car: usize) -> (Option<f64>, Option<f64>) {
        let order = self.order();
        let Some(at) = order.iter().position(|&c| c == car) else {
            return (None, None);
        };
        let gap = |other: usize| {
            let (a, b) = (&self.cars[car], &self.cars[other]);
            (a.status == Status::Racing && b.status == Status::Racing)
                .then(|| (b.distance - a.distance).abs())
        };
        (
            at.checked_sub(1).and_then(|i| gap(order[i])),
            order.get(at + 1).and_then(|&c| gap(c)),
        )
    }

    /// One-based position of `car`.
    pub fn position(&self, car: usize) -> usize {
        self.order()
            .iter()
            .position(|&c| c == car)
            .map_or(0, |p| p + 1)
    }

    /// Classification lines: the winner's time, gaps to the winner, the
    /// laps completed by cars out of the race and the distance still to
    /// cover for cars racing on.
    pub fn classification(&self, names: &[String]) -> Vec<String> {
        let order = self.order();
        let leader = order.first().and_then(|&c| self.cars[c].finish);
        let target = self.target();
        order
            .iter()
            .enumerate()
            .map(|(rank, &c)| {
                let car = &self.cars[c];
                let laps = (car.distance / self.length).floor().max(0.) as u32;
                let lap_word = if laps == 1 { "lap" } else { "laps" };
                let result = match (car.status, car.finish, leader) {
                    (Status::Out(why), _, _) => format!(
                        "{}, {laps} {lap_word}",
                        match why {
                            Out::Stopped => "DNF",
                            Out::Wrecked => "Wrecked",
                            Out::Retired => "Retired",
                            Out::Disqualified => "Disqualified",
                        }
                    ),
                    (Status::Finished | Status::Estimated, _, _) if self.limit.is_some() => {
                        let mark = if car.status == Status::Estimated {
                            " (est.)"
                        } else {
                            ""
                        };
                        format!("{:.3} laps{mark}", self.progress(c))
                    }
                    (Status::Racing, _, _) => {
                        format!("running, {:.0} m to go", (target - car.distance).max(0.))
                    }
                    (status, Some(t), Some(l)) => {
                        let mark = if status == Status::Estimated {
                            " (est.)"
                        } else {
                            ""
                        };
                        if rank > 0 {
                            format!("+{:.3}{mark}", t - l)
                        } else {
                            format!("{}{mark}", format_time(t))
                        }
                    }
                    _ => String::new(),
                };
                format!(
                    "{}. {}  {}",
                    rank + 1,
                    names.get(c).map_or("?", String::as_str),
                    result
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Standings on a 1000 m loop with cars at the given distances from the line.
    fn standings(laps: u32, distances: &[f64]) -> Standings {
        Standings {
            laps,
            length: 1000.,
            start_arc: 0.,
            cars: distances
                .iter()
                .map(|&d| Car {
                    distance: d,
                    last_arc: d.rem_euclid(1000.),
                    finish: None,
                    status: Status::Racing,
                    laps: Vec::new(),
                })
                .collect(),
            finalized: false,
            limit: None,
        }
    }

    #[test]
    fn lap_times_and_best_lap_follow_the_line_crossings() {
        let mut s = standings(3, &[-5., -10.]);
        s.cars[0].last_arc = 995.;
        s.cars[1].last_arc = 990.;
        // Car 0: laps at 60, 115 (55 s) and 175 (60 s); car 1 crosses once, at 70.
        for (t, a0, a1) in [
            (10., 245., 230.),
            (20., 495., 470.),
            (30., 745., 710.),
            (40., 995., 950.),
            (60., 5., 980.),
            (70., 255., 5.),
            (80., 505., 255.),
            (90., 755., 505.),
            (115., 2., 755.),
            (125., 252., 800.),
            (135., 502., 850.),
            (150., 752., 900.),
            (175., 3., 950.),
        ] {
            s.step(&[a0, a1], t, &[]);
        }
        assert_eq!(s.lap_times(0), vec![60., 55., 60.]);
        assert_eq!(s.best_lap(0), Some((55., 2)));
        assert_eq!(s.status(0), Status::Finished);
        assert_eq!(s.finished(0), Some(175.));
        assert_eq!(s.lap_times(1), vec![70.]);
        assert_eq!(s.best_lap(1), Some((70., 1)));
        assert_eq!(standings(2, &[0.]).best_lap(0), None);
    }
    fn names(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("car{i}")).collect()
    }

    #[test]
    fn wrap_keeps_small_deltas_across_the_seam() {
        assert_eq!(wrap(3940. - 10., 3944.), 3930. - 3944.);
        assert_eq!(wrap(-3930., 3944.), 14.);
        assert_eq!(wrap(5., 3944.), 5.);
    }

    #[test]
    fn five_mph_threshold() {
        assert!((FINALIZE_MINIMUM_SPEED - 2.2352).abs() < 1e-9);
    }

    #[test]
    fn finishers_rank_by_time_ahead_of_everyone_else() {
        let mut s = standings(2, &[1500., 1990., 1200.]);
        s.finish(0, 120.);
        // Car 1 is further along but has not finished.
        assert_eq!(s.order(), vec![0, 1, 2]);
        s.finish(2, 119.);
        assert_eq!(s.order(), vec![2, 0, 1]);
        assert!(!s.over());
        s.finish(1, 121.);
        assert!(s.over());
    }

    #[test]
    fn lapped_cars_must_complete_the_full_distance() {
        let mut s = standings(2, &[1999., 900.]);
        s.cars[0].last_arc = 999.;
        s.cars[1].last_arc = 900.;
        // The leader crosses the line for the last time; the lapped car does not finish.
        s.step(&[2., 905.], 100., &[]);
        assert_eq!(s.status(0), Status::Finished);
        assert_eq!(s.status(1), Status::Racing);
        assert!(!s.over());
    }

    #[test]
    fn finalize_estimates_moving_cars_and_retires_stopped_ones() {
        let mut s = standings(2, &[2000., 1800., 1700., 1500.]);
        s.finish(0, 100.);
        s.finalize(&[0., 30., 2.0, 3.0], 100., &[0]);
        assert_eq!(s.status(1), Status::Estimated);
        assert_eq!(s.status(2), Status::Out(Out::Stopped));
        assert_eq!(s.status(3), Status::Estimated);
        // Mean race speed: 1800 m in 100 s -> 200 m at 18 m/s.
        assert!((s.finished(1).unwrap() - (100. + 200. / 18.)).abs() < 1e-9);
        assert_eq!(s.order(), vec![0, 1, 3, 2]);
        assert!(s.over());
        let lines = s.classification(&names(4));
        assert_eq!(lines[0], "1. car0  1:40.000");
        assert_eq!(lines[1], "2. car1  +11.111 (est.)");
        assert_eq!(lines[3], "4. car2  DNF, 1 lap");
    }

    #[test]
    fn unfinished_human_is_retired_and_everyone_else_estimated() {
        let mut s = standings(3, &[1200., 1500., 900.]);
        s.retire(0, Out::Wrecked);
        s.finalize(&[0., 0.5, 0.], 60., &[0]);
        assert_eq!(s.status(0), Status::Out(Out::Wrecked));
        assert_eq!(s.status(1), Status::Estimated);
        assert_eq!(s.status(2), Status::Estimated);
        assert_eq!(s.order(), vec![1, 2, 0]);
        assert_eq!(s.classification(&names(3))[2], "3. car0  Wrecked, 1 lap");
        let mut s = standings(3, &[1200., 1500.]);
        s.finalize(&[20., 0.], 60., &[0]);
        assert_eq!(s.status(0), Status::Out(Out::Retired));
        assert_eq!(s.status(1), Status::Estimated);
    }

    #[test]
    fn cars_out_rank_by_distance_with_unfinished_cars() {
        let mut s = standings(3, &[1200., 1500., 900.]);
        s.retire(1, Out::Wrecked);
        assert_eq!(s.order(), vec![1, 0, 2]);
        assert_eq!(s.neighbour_gaps(0), (None, Some(300.)));
        assert!(!s.over());
        s.retire(0, Out::Retired);
        s.retire(2, Out::Retired);
        assert!(s.over());
    }

    #[test]
    fn chequered_flag_for_the_leader_on_the_final_approach() {
        let s = standings(3, &[2700., 1500.]);
        assert_eq!(s.leader_flag(), None);
        let s = standings(3, &[2760., 1500.]);
        assert_eq!(s.leader_flag(), Some(Flag::Chequered));
        // 250 m from the line on the second-to-last lap: no flag.
        let s = standings(3, &[1760., 1500.]);
        assert_eq!(s.leader_flag(), None);
    }

    #[test]
    fn a_time_limited_race_ends_at_the_limit_and_ranks_by_progress() {
        let mut s = standings(8, &[-5., -10., -20.]).with_time_limit(100.);
        s.cars[0].last_arc = 995.;
        s.cars[1].last_arc = 990.;
        s.cars[2].last_arc = 980.;
        // The line is no finish: car 0 keeps racing after its laps.
        for (t, a) in [(30., [400., 300., 200.]), (60., [800., 600., 400.])] {
            s.step(&a, t, &[]);
            s.finish(0, t);
        }
        assert_eq!(s.status(0), Status::Racing);
        assert!(!s.over());
        assert_eq!(s.leader_flag(), None);
        s.step(&[100., 700., 850.], 90., &[]);
        assert_eq!(s.lap_times(0), vec![90.]);
        assert!(!s.over());
        s.step(&[500., 750., 950.], 100., &[]);
        assert!(s.over());
        for c in 0..3 {
            assert_eq!(s.status(c), Status::Finished);
            assert_eq!(s.finished(c), Some(100.));
        }
        assert_eq!(s.order(), vec![0, 2, 1]);
        assert!((s.progress(0) - 1.5).abs() < 1e-9);
        // Nothing moves after the end.
        s.step(&[900., 760., 960.], 110., &[]);
        assert!((s.progress(0) - 1.5).abs() < 1e-9);
        assert_eq!(s.classification(&names(3))[0], "1. car0  1.500 laps");
    }

    #[test]
    fn laps_count_up_to_the_event_laps_unless_unlimited() {
        let mut s = standings(2, &[2950.]).with_time_limit(1000.);
        s.cars[0].last_arc = 950.;
        s.step(&[50.], 10., &[]);
        assert_eq!(s.lap_times(0).len(), 2);
        assert!((s.progress(0) - 2.05).abs() < 1e-9);
        let mut s = standings(0, &[2950.]).with_time_limit(1000.);
        s.cars[0].last_arc = 950.;
        s.step(&[50.], 10., &[]);
        assert_eq!(s.lap_times(0).len(), 3);
        assert!((s.progress(0) - 3.05).abs() < 1e-9);
    }

    #[test]
    fn an_early_human_end_projects_the_ai_to_the_limit() {
        let mut s = standings(8, &[1000., 1500., 600.]).with_time_limit(200.);
        s.retire(2, Out::Wrecked);
        s.finalize(&[30., 30., 0.], 100., &[0]);
        assert_eq!(s.status(0), Status::Out(Out::Retired));
        assert_eq!(s.status(1), Status::Estimated);
        assert_eq!(s.status(2), Status::Out(Out::Wrecked));
        // 1500 m in 100 s, 100 s more at 15 m/s.
        assert!((s.progress(1) - 3.).abs() < 1e-9);
        assert_eq!(s.order(), vec![1, 0, 2]);
        // Results: the estimated car's progress times its class adjustment;
        // an unclassified car with more progress may rank above it.
        let mut s = standings(8, &[1000., 1500., 3100.]).with_time_limit(200.);
        s.retire(2, Out::Wrecked);
        s.finalize(&[30., 30., 0.], 100., &[0]);
        assert_eq!(s.lemans_results_order(&[1., 1., 1.]), vec![2, 1, 0]);
        assert_eq!(s.lemans_results_order(&[1., 1.08, 1.]), vec![1, 2, 0]);
        // At the limit nobody is estimated.
        let mut s = standings(8, &[1000., 1500.]).with_time_limit(200.);
        s.finalize(&[30., 30.], 200., &[0]);
        assert_eq!(s.status(0), Status::Finished);
        assert_eq!(s.status(1), Status::Finished);
    }

    #[test]
    fn class_positions_count_within_each_class() {
        let s = standings(2, &[500., 900., 700., 100.]);
        // Order 1, 2, 0, 3; classes LMP1, LMP1, GT1, GT1.
        assert_eq!(s.class_positions(&[1, 1, 3, 3]), vec![1, 0, 0, 1]);
        assert_eq!(s.class_positions(&[9, -1, 0, 5]), vec![0, 0, 1, 1]);
    }

    #[test]
    fn le_mans_times_print_total_minutes() {
        assert_eq!(format_lemans_time(86_400.), "1440:00.00");
        assert_eq!(format_lemans_time(754.236), "12:34.24");
        assert_eq!(format_lemans_time(0.), "--:--:--");
    }

    #[test]
    fn grid_slot_clamps_base_and_range_to_the_field() {
        // Most common shipped placement: base 8, range 19 in a 12-car field -> slots 7..=11.
        assert_eq!(grid_slot(8, 19, 12, |r| r - 1), 11);
        assert_eq!(
            grid_slot(8, 19, 12, |r| {
                assert_eq!(r, 5);
                0
            }),
            7
        );
        // Base beyond the field clamps to the last slot.
        assert_eq!(grid_slot(15, 1, 12, |_| 0), 11);
        // Range 0 is a fixed slot.
        assert_eq!(grid_slot(2, 0, 12, |_| unreachable!()), 1);
        assert_eq!(grid_slot(1, 12, 12, |r| r - 1), 11);
    }
}
