//! Marshal flags of a race: the original's rules system (0x94A4A0) and flag
//! system (vtable 0xD10D6C) driven by `gameconfig/rules.xml`
//! (docs/RACE_RULES_RESEARCH.md section 3 and "Flag system").
//!
//! - The rules run every `rule_reaction_time` (0.3 s) with the time summed
//!   since their last run [V 0x94A4A0].
//! - `race_start` raises green_flag once, when the leader is within
//!   `distance_to_line` of the line after the start [V 0x94A1E0].
//! - `serious_incident` raises yellow_flag where a car has been sideways,
//!   backwards or nearly stopped for `minumum_time_to_register`, at most once
//!   per car every `minumum_time_between`, after `minumum_race_time`
//!   [V 0x94AC60, 0x94A6E0, 0x949B40, 0x94A7D0].
//! - `end_race` raises chequered_flag once with the leader on its final
//!   approach ([`crate::race::Standings::leader_flag`], 0x94AA70).
//! - A raised flag is global: it goes to the one registered vehicle
//!   (`max_vehicles` 1, the player [I]), lasts `duration` and covers the
//!   line from `track_distance_before` behind its point to
//!   `track_distance_after` past it [V 0x94B440, 0x94B2A0, 0x949960, 0x94BA10].
//! - What the player sees [V 0x6BA460]: the minimap flag plays "in" while
//!   the player is inside an active yellow flag's zone, "out" otherwise.
//!   Green and chequered have no HUD element.
//!
//! Positions are arc lengths along the AI racing line ([`Line::arc`]).
use crate::ai_driver::Line;
use std::path::Path;

pub const GREEN_FLAG: &str = "green_flag";
pub const YELLOW_FLAG: &str = "yellow_flag";
pub const CHEQUERED_FLAG: &str = "chequered_flag";
/// rules.xml competition formats.
pub const SINGLE_RACE: &str = "single_race";
pub const SINGLE_DEMOLITION_RACE: &str = "single_demolition_race";

/// One `<flag>` of a rule set (load 0x949F50).
#[derive(Clone, Debug, PartialEq)]
pub struct FlagType {
    pub name: String,
    pub priority: i32,
    /// Seconds a raised flag stays active.
    pub duration: f32,
    /// Zone (m along the line) behind and past the flag's point.
    pub before: f32,
    pub after: f32,
}

/// `serious_incident` thresholds (load 0x949A70; the original spells the
/// attributes `minumum_*`). Defaults of the constructor 0x94A120.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IncidentRule {
    /// Degrees between the car's heading and the line direction.
    pub minimum_angle: f32,
    /// m/s along the line [I: physics units].
    pub maximum_speed: f32,
    pub slow_speed: f32,
    /// Seconds between two yellow flags of one car.
    pub time_between: f32,
    /// Seconds an incident must last before it raises a flag.
    pub time_to_register: f32,
    /// Seconds of race before any incident counts.
    pub race_time: f32,
}

impl Default for IncidentRule {
    fn default() -> Self {
        Self {
            minimum_angle: 45.,
            maximum_speed: 10.,
            slow_speed: 1.,
            time_between: 30.,
            time_to_register: 1.,
            race_time: 5.,
        }
    }
}

/// A competition format of rules.xml.
#[derive(Clone, Debug, PartialEq)]
pub struct RuleSet {
    pub reaction_time: f32,
    pub max_flags_per_vehicle: usize,
    pub flags: Vec<FlagType>,
    /// `race_start distance_to_line`.
    pub race_start: Option<f32>,
    pub incident: Option<IncidentRule>,
    /// `end_race distance_to_line` (the chequered condition itself lives in
    /// [`crate::race::Standings::leader_flag`]).
    pub end_race: Option<f32>,
}

impl RuleSet {
    /// The shipped `single_race` set [D rules.xml].
    pub fn single_race() -> Self {
        let flag = |name: &str, priority, duration, before, after| FlagType {
            name: name.into(),
            priority,
            duration,
            before,
            after,
        };
        Self {
            reaction_time: 0.3,
            max_flags_per_vehicle: 12,
            flags: vec![
                flag(GREEN_FLAG, 2, 30., 400., 100.),
                flag(YELLOW_FLAG, 3, 10., 100., 20.),
                flag(CHEQUERED_FLAG, 4, 600., 400., 100.),
            ],
            race_start: Some(20.),
            incident: Some(IncidentRule {
                minimum_angle: 90.,
                maximum_speed: 40.,
                slow_speed: 3.,
                time_between: 8.,
                time_to_register: 0.5,
                race_time: 10.,
            }),
            end_race: Some(250.),
        }
    }

    /// The rule set of competition format `format` in a decoded rules.xml.
    pub fn parse(
        doc: &grid_formats::binxml::Document<'_>,
        format: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let e = &doc.elements;
        let attr = |i: usize, key: &str| {
            e[i].attributes
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| *v)
        };
        let number = |i: usize, key: &str| -> Option<f32> { attr(i, key)?.trim().parse().ok() };
        // Every element below `i` (the format's settings, flag_system and
        // rules_system may sit beside or under competition_format).
        fn below(e: &[grid_formats::binxml::Element<'_>], i: usize, out: &mut Vec<usize>) {
            for c in e[i].children.clone() {
                out.push(c);
                below(e, c, out);
            }
        }
        let set = (0..e.len())
            .filter(|&i| e[i].name == "rule_set")
            .find(|&i| {
                let mut all = Vec::new();
                below(e, i, &mut all);
                all.iter()
                    .any(|&c| e[c].name == "competition_format" && attr(c, "name") == Some(format))
            })
            .ok_or_else(|| format!("rules.xml has no competition format {format}"))?;
        let mut all = Vec::new();
        below(e, set, &mut all);
        let mut rules = Self {
            reaction_time: 0.,
            max_flags_per_vehicle: 0,
            flags: Vec::new(),
            race_start: None,
            incident: None,
            end_race: None,
        };
        for c in all {
            match e[c].name {
                "settings" => {
                    rules.reaction_time = number(c, "rule_reaction_time").unwrap_or(0.);
                    rules.max_flags_per_vehicle =
                        number(c, "max_flags_per_vehicle").unwrap_or(0.) as usize;
                }
                // Defaults of 0x949EC0.
                "flag" => rules.flags.push(FlagType {
                    name: attr(c, "name").unwrap_or("").into(),
                    priority: number(c, "priority").unwrap_or(1.) as i32,
                    duration: number(c, "duration").unwrap_or(10.),
                    before: number(c, "track_distance_before").unwrap_or(10.),
                    after: number(c, "track_distance_after").unwrap_or(10.),
                }),
                "rule" => match attr(c, "name") {
                    Some("race_start") => {
                        rules.race_start = Some(number(c, "distance_to_line").unwrap_or(20.));
                    }
                    Some("end_race") => {
                        rules.end_race = Some(number(c, "distance_to_line").unwrap_or(250.));
                    }
                    Some("serious_incident") => {
                        let d = IncidentRule::default();
                        let n = |key, default| number(c, key).unwrap_or(default);
                        rules.incident = Some(IncidentRule {
                            minimum_angle: n("minumum_angle", d.minimum_angle),
                            maximum_speed: n("maximum_speed", d.maximum_speed),
                            slow_speed: n("slow_speed", d.slow_speed),
                            time_between: n("minumum_time_between", d.time_between),
                            time_to_register: n("minumum_time_to_register", d.time_to_register),
                            race_time: n("minumum_race_time", d.race_time),
                        });
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        Ok(rules)
    }

    /// `gameconfig/rules.xml` of the installation at `root`.
    pub fn load(root: &Path, format: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let bytes = std::fs::read(root.join("gameconfig/rules.xml"))?;
        Self::parse(&grid_formats::binxml::read(&bytes)?, format)
    }
}

/// A car as the rules see it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CarSample {
    /// Arc length along the line.
    pub position: f64,
    /// Radians between the car's flattened heading and the line direction
    /// (0x909F70), 0..π.
    pub angle: f32,
    /// Velocity along the line direction, negative going backwards (0x90A250).
    pub speed: f32,
    /// Still racing (the original skips cars failing the entry's vf+0x50
    /// check [I: finished or out]).
    pub racing: bool,
}

impl CarSample {
    /// Sample a car at `position` with unit `forward` axis and `velocity`.
    pub fn of(line: &Line, position: [f32; 3], forward: [f32; 3], velocity: [f32; 3], racing: bool) -> Self {
        let arc = line.arc(position);
        let direction = line.direction(arc);
        let flat = (forward[0] * forward[0] + forward[2] * forward[2]).sqrt();
        let cos = if flat > 0. {
            (forward[0] * direction[0] + forward[2] * direction[2]) / flat
        } else {
            1.
        };
        Self {
            position: f64::from(arc),
            angle: cos.clamp(-1., 1.).acos(),
            speed: velocity[0] * direction[0] + velocity[2] * direction[2],
            racing,
        }
    }
}

impl CarSample {
    /// Sample a car from its pose (row 2 forward, row 3 position) and velocity.
    pub fn of_pose(line: &Line, pose: &[f32; 16], velocity: &[f32; 4], racing: bool) -> Self {
        Self::of(
            line,
            [pose[12], pose[13], pose[14]],
            [pose[8], pose[9], pose[10]],
            [velocity[0], velocity[1], velocity[2]],
            racing,
        )
    }
}

/// What the race tells the rules at one simulation step.
#[derive(Clone, Copy, Debug)]
pub struct RaceState<'a> {
    /// The race has started (GO; race state +0x10).
    pub started: bool,
    /// The race is over (race state +0x11 [I]).
    pub ended: bool,
    pub cars: &'a [CarSample],
    /// Distance from the leader forward to the line, if there is a leader.
    pub leader_to_line: Option<f64>,
    /// Arc of the start-finish line.
    pub line: f64,
    /// [`crate::race::Standings::leader_flag`] is chequered.
    pub chequered: bool,
}

/// A flag raised by a rule.
#[derive(Clone, Debug, PartialEq)]
pub struct Raised {
    pub flag: String,
    /// The car whose incident raised it (yellow only).
    pub car: Option<usize>,
    pub position: f64,
    /// Flag-system time (s) when it was raised.
    pub time: f64,
}

#[derive(Clone, Debug)]
struct Active {
    flag: usize,
    remaining: f32,
    position: f64,
}

#[derive(Clone, Copy, Debug, Default)]
struct Entry {
    register: f32,
    last: Option<f64>,
    position: f64,
}

/// The rules and flag system of one race.
#[derive(Clone, Debug)]
pub struct RaceFlags {
    rules: RuleSet,
    length: f64,
    open: bool,
    /// Time since the rules last ran (RulesSystem +0x20).
    pending: f32,
    /// Time since construction: the per-car "time between" clock
    /// (AUTHORED: the original reads the system clock 0x4B1E20).
    clock: f64,
    /// serious_incident race-time accumulator (+0x24).
    race_time: f32,
    green_done: bool,
    chequered_done: bool,
    entries: Vec<Entry>,
    /// The registered vehicle's active flags.
    active: Vec<Active>,
}

impl RaceFlags {
    pub fn new(rules: RuleSet, line: &Line, cars: usize) -> Self {
        Self::on_line(rules, f64::from(line.length), line.is_open(), cars)
    }

    /// On an abstract line of `length` m (tests and tools).
    pub fn on_line(rules: RuleSet, length: f64, open: bool, cars: usize) -> Self {
        Self {
            rules,
            length,
            open,
            pending: 0.,
            clock: 0.,
            race_time: 0.,
            green_done: false,
            chequered_done: false,
            // The tracked list holds up to 20 vehicles [V 0x94A120].
            entries: vec![Entry::default(); cars.min(20)],
            active: Vec::new(),
        }
    }

    pub fn rules(&self) -> &RuleSet {
        &self.rules
    }

    /// The rule set of a race: `single_demolition_race` for a derby,
    /// `single_race` otherwise [I: the format selection was not traced].
    pub fn load(root: &Path, demolition: bool, line: &Line, cars: usize) -> Result<Self, Box<dyn std::error::Error>> {
        let format = if demolition { SINGLE_DEMOLITION_RACE } else { SINGLE_RACE };
        Ok(Self::new(RuleSet::load(root, format)?, line, cars))
    }

    /// One simulation step of a race kept by `standings`; `cars` are every
    /// car's pose and velocity in standings order.
    pub fn step(
        &mut self,
        dt: f32,
        started: bool,
        line: &Line,
        standings: &crate::race::Standings,
        cars: &[([f32; 16], [f32; 4])],
    ) -> Vec<Raised> {
        let samples: Vec<CarSample> = cars
            .iter()
            .enumerate()
            .map(|(i, (pose, velocity))| {
                let racing = standings.status(i) == crate::race::Status::Racing;
                CarSample::of_pose(line, pose, velocity, racing)
            })
            .collect();
        let race = RaceState {
            started,
            ended: standings.over(),
            cars: &samples,
            leader_to_line: standings.order().first().map(|&c| standings.to_line(c)),
            line: standings.line_arc(),
            chequered: standings.leader_flag().is_some(),
        };
        self.update(dt, &race)
    }

    /// Advance by `dt` seconds of simulation; returns the flags raised.
    pub fn update(&mut self, dt: f32, race: &RaceState) -> Vec<Raised> {
        self.clock += f64::from(dt);
        self.pending += dt;
        if self.pending < self.rules.reaction_time {
            return Vec::new();
        }
        let step = std::mem::take(&mut self.pending);
        // AUTHORED order: active flags count down before the rules raise new
        // ones, so a new flag keeps its whole duration.
        for flag in &mut self.active {
            flag.remaining -= step;
        }
        self.active.retain(|f| f.remaining >= 0.);
        let mut raised = Vec::new();
        // race_start [V 0x94A1E0].
        if let Some(distance) = self.rules.race_start
            && !self.green_done
            && race.started
            && race.leader_to_line.is_some_and(|d| d <= f64::from(distance))
        {
            self.green_done = true;
            self.raise(GREEN_FLAG, None, race.line, &mut raised);
        }
        // serious_incident [V 0x94AC60].
        if let Some(rule) = self.rules.incident
            && race.started
            && !race.ended
        {
            self.race_time += step;
            if self.race_time > rule.race_time {
                self.incidents(rule, step, race.cars, &mut raised);
            }
        }
        // end_race [V 0x94AA70].
        if self.rules.end_race.is_some() && !self.chequered_done && race.chequered {
            self.chequered_done = true;
            self.raise(CHEQUERED_FLAG, None, race.line, &mut raised);
        }
        raised
    }

    fn incidents(&mut self, rule: IncidentRule, step: f32, cars: &[CarSample], raised: &mut Vec<Raised>) {
        let angle = rule.minimum_angle.to_radians();
        let mut any = false;
        for (entry, car) in self.entries.iter_mut().zip(cars) {
            if !car.racing {
                continue;
            }
            // 0x949B40.
            if (car.angle > angle && car.speed < rule.maximum_speed) || car.speed < rule.slow_speed {
                entry.register += step;
                entry.position = car.position;
                any = true;
            } else {
                entry.register = 0.;
            }
        }
        if !any {
            return;
        }
        // 0x94A7D0.
        for i in 0..self.entries.len() {
            let entry = self.entries[i];
            if cars.get(i).is_some_and(|c| c.racing)
                && entry.register >= rule.time_to_register
                && entry.last.is_none_or(|l| self.clock - l >= f64::from(rule.time_between))
            {
                self.entries[i].last = Some(self.clock);
                self.raise(YELLOW_FLAG, Some(i), entry.position, raised);
            }
        }
    }

    fn raise(&mut self, name: &str, car: Option<usize>, position: f64, raised: &mut Vec<Raised>) {
        let Some(flag) = self.rules.flags.iter().position(|f| f.name == name) else {
            return;
        };
        raised.push(Raised {
            flag: name.into(),
            car,
            position,
            time: self.clock,
        });
        // 0x94B130: a full vehicle list rejects the flag.
        if self.active.len() < self.rules.max_flags_per_vehicle {
            self.active.push(Active {
                flag,
                remaining: self.rules.flags[flag].duration,
                position,
            });
        }
    }

    /// Zone test 0x94BA10: the zone's length exceeds the distance from the
    /// car forward to the zone's end. A zone longer than the line covers all
    /// of it (0x90CB10). On an open line the zone is a plain interval [I].
    fn in_zone(&self, flag: &Active, position: f64) -> bool {
        let kind = &self.rules.flags[flag.flag];
        let span = f64::from(kind.before) + f64::from(kind.after);
        if span > self.length {
            return true;
        }
        let start = flag.position - f64::from(kind.before);
        let end = flag.position + f64::from(kind.after);
        if self.open {
            return start < position && position <= end;
        }
        span > (end - position).rem_euclid(self.length)
    }

    /// An active flag named `name` covers line position `position`.
    pub fn showing(&self, name: &str, position: f64) -> bool {
        self.active
            .iter()
            .any(|f| self.rules.flags[f.flag].name == name && self.in_zone(f, position))
    }

    /// The HUD's minimap flag [V 0x6BA460]: a yellow flag covers `position`.
    /// Whether race_start has raised its green flag and end_race its
    /// chequered flag (each fires once per race).
    pub fn raised(&self) -> (bool, bool) {
        (self.green_done, self.chequered_done)
    }

    pub fn yellow(&self, position: f64) -> bool {
        self.showing(YELLOW_FLAG, position)
    }

    /// The highest-priority active flag covering `position` (0x94AD60;
    /// the original's debug view).
    pub fn top(&self, position: f64) -> Option<&str> {
        self.active
            .iter()
            .filter(|f| self.in_zone(f, position))
            .max_by_key(|f| self.rules.flags[f.flag].priority)
            .map(|f| self.rules.flags[f.flag].name.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn car(position: f64, angle_degrees: f32, speed: f32) -> CarSample {
        CarSample {
            position,
            angle: angle_degrees.to_radians(),
            speed,
            racing: true,
        }
    }

    fn state<'a>(cars: &'a [CarSample], leader_to_line: f64) -> RaceState<'a> {
        RaceState {
            started: true,
            ended: false,
            cars,
            leader_to_line: Some(leader_to_line),
            line: 0.,
            chequered: false,
        }
    }

    /// Run `seconds` in 1/60 s steps; collect what was raised.
    fn run(flags: &mut RaceFlags, seconds: f32, race: &RaceState) -> Vec<Raised> {
        let mut raised = Vec::new();
        for _ in 0..(seconds * 60.).round() as usize {
            raised.extend(flags.update(1. / 60., race));
        }
        raised
    }

    #[test]
    fn green_once_when_the_leader_nears_the_line_after_the_start() {
        let mut flags = RaceFlags::on_line(RuleSet::single_race(), 3000., false, 2);
        let cars = [car(2950., 0., 0.), car(2940., 0., 0.)];
        let mut before = state(&cars, 50.);
        before.started = false;
        assert!(run(&mut flags, 1., &before).is_empty());
        assert!(run(&mut flags, 1., &state(&cars, 50.)).is_empty());
        let raised = run(&mut flags, 1., &state(&cars, 15.));
        assert_eq!(raised.len(), 1);
        assert_eq!((raised[0].flag.as_str(), raised[0].position), (GREEN_FLAG, 0.));
        assert!(run(&mut flags, 1., &state(&cars, 15.)).is_empty());
        // Zone 400 m behind the line to 100 m past it; no yellow shown.
        assert_eq!(flags.top(2700.), Some(GREEN_FLAG));
        assert_eq!(flags.top(99.), Some(GREEN_FLAG));
        assert_eq!(flags.top(101.), None);
        assert!(!flags.yellow(0.));
        // 30 s later it is gone.
        run(&mut flags, 30., &state(&cars, 15.));
        assert_eq!(flags.top(0.), None);
    }

    #[test]
    fn a_spun_car_raises_a_yellow_after_the_race_time_and_register_time() {
        let mut flags = RaceFlags::on_line(RuleSet::single_race(), 3000., false, 2);
        let moving = car(500., 0., 30.);
        let spun = car(1000., 120., 5.);
        // Spun from the start: nothing before 10 s of race.
        let early = run(&mut flags, 9.9, &state(&[moving, spun], 500.));
        assert!(early.iter().all(|r| r.flag != YELLOW_FLAG), "{early:?}");
        let raised = run(&mut flags, 1.2, &state(&[moving, spun], 500.));
        let yellow: Vec<_> = raised.iter().filter(|r| r.flag == YELLOW_FLAG).collect();
        assert_eq!(yellow.len(), 1);
        assert_eq!((yellow[0].car, yellow[0].position), (Some(1), 1000.));
        // Zone 100 m behind to 20 m past the incident.
        assert!(flags.yellow(901.) && flags.yellow(1019.));
        assert!(!flags.yellow(899.) && !flags.yellow(1021.));
        // Still spun: the next yellow of this car waits 8 s.
        let again = run(&mut flags, 7., &state(&[moving, spun], 500.));
        assert!(again.iter().all(|r| r.flag != YELLOW_FLAG));
        let later = run(&mut flags, 1.5, &state(&[moving, spun], 500.));
        assert_eq!(later.iter().filter(|r| r.flag == YELLOW_FLAG).count(), 1);
        // 10 s after the last raise the flag is down.
        run(&mut flags, 10.5, &state(&[moving, moving], 500.));
        assert!(!flags.yellow(1000.));
    }

    #[test]
    fn the_incident_test_is_angle_and_speed_or_slow() {
        let rule = RuleSet::single_race();
        let check = |c: CarSample| {
            let mut flags = RaceFlags::on_line(rule.clone(), 3000., false, 1);
            let raised = run(&mut flags, 12., &state(&[c], 500.));
            raised.iter().any(|r| r.flag == YELLOW_FLAG)
        };
        assert!(!check(car(10., 0., 20.)), "driving");
        assert!(!check(car(10., 89., 20.)), "sideways under 90 degrees");
        assert!(check(car(10., 91., 20.)), "past 90 degrees, slow");
        assert!(!check(car(10., 91., 41.)), "past 90 degrees, fast");
        assert!(check(car(10., 0., 2.9)), "nearly stopped");
        assert!(check(car(10., 0., -5.)), "reversing");
        let mut out = car(10., 0., 0.);
        out.racing = false;
        assert!(!check(out), "not racing");
    }

    #[test]
    fn a_short_moment_does_not_register_and_zones_wrap() {
        let mut flags = RaceFlags::on_line(RuleSet::single_race(), 3000., false, 1);
        run(&mut flags, 11., &state(&[car(0., 0., 30.)], 500.));
        // 0.3 s slow (one rule tick) then moving: the accumulator resets.
        let a = run(&mut flags, 0.3, &state(&[car(10., 0., 1.)], 500.));
        let b = run(&mut flags, 1., &state(&[car(10., 0., 30.)], 500.));
        assert!(a.iter().chain(&b).all(|r| r.flag != YELLOW_FLAG));
        // An incident just past the line: the zone reaches back across it.
        let c = run(&mut flags, 1., &state(&[car(30., 0., 0.)], 500.));
        assert_eq!(c.iter().filter(|r| r.flag == YELLOW_FLAG).count(), 1);
        assert!(flags.yellow(2950.) && flags.yellow(45.) && !flags.yellow(60.));
    }

    #[test]
    fn the_demolition_set_has_no_yellow() {
        let mut rules = RuleSet::single_race();
        rules.flags.retain(|f| f.name != YELLOW_FLAG);
        rules.incident = None;
        let mut flags = RaceFlags::on_line(rules, 3000., false, 1);
        let raised = run(&mut flags, 15., &state(&[car(10., 180., 0.)], 500.));
        assert!(raised.iter().all(|r| r.flag != YELLOW_FLAG));
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT"]
    fn the_shipped_rules_match_the_decoded_values() {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let race = RuleSet::load(Path::new(&root), SINGLE_RACE).unwrap();
        assert_eq!(race, RuleSet::single_race());
        let derby = RuleSet::load(Path::new(&root), SINGLE_DEMOLITION_RACE).unwrap();
        assert!(derby.incident.is_none());
        let names: Vec<_> = derby.flags.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, [GREEN_FLAG, CHEQUERED_FLAG]);
    }

    #[test]
    fn chequered_once_from_the_end_race_condition() {
        let mut flags = RaceFlags::on_line(RuleSet::single_race(), 3000., false, 1);
        let cars = [car(2800., 0., 40.)];
        let mut race = state(&cars, 200.);
        race.chequered = true;
        let raised = run(&mut flags, 2., &race);
        let chequered = raised.iter().filter(|r| r.flag == CHEQUERED_FLAG).count();
        assert_eq!(chequered, 1);
        assert_eq!(flags.top(2800.), Some(CHEQUERED_FLAG));
    }
}
