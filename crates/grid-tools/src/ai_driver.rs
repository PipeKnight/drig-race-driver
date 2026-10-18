//! Original GRID AI race driver, ported from the recovered pipeline in
//! docs/AI_DRIVER_RESEARCH.md: racing line from `ai_track.xml`, look-ahead
//! (0x779A20), brake-line corner speed (0x792C80), brake-line braking against the
//! car's speed->distance table (0x7886C0), throttle governor/limiters (0x783180,
//! 0x77A4C0, 0x77A880, 0x77A940), steering (0x77AC50), output slew (0x779F70)
//! and race-behaviour random offsets (0x78A4D0/0x78A610), with the shipped
//! config and the bilinear difficulty mappings.
//!
//! The lateral PID (0x7892E0) shifts the target point by PID(lateral error)
//! with the shipped gains and limits; its internal clamping is authored (the
//! PID vfunc is not decompiled) and its speed-dependent prediction and corner
//! factor terms are not yet recovered.
//!
//! Traffic (docs/AI_OVERTAKING_RESEARCH.md): the brake_for_vehicle,
//! attempt_overtake and alongside tests feed a per-tick request; behaviours
//! arbitrate by priority (alongside 8 > overtake 5 > race 1); the follow
//! throttle (0x77A630) and brake (0x77A1E0) consume the request's follow
//! fields. AUTHORED where not recovered: the path-obstacle scan is a straight
//! corridor along the own heading, the alongside yield and far-side squeeze
//! are omitted, cars on both sides hold the planned offset, and a stuck car
//! reverses briefly (the recovery behaviour is not recovered).
//!
//! The race manager (`crate::ai_pacing`) sets the corner-speed scale A and
//! floor B, the let-pass flag, failures and engine damage, and arms
//! choreographed mistakes; the mistake slot, lost_control, controlled_stop,
//! race_damaged and the retire failure branch are here
//! (docs/AI_MISTAKES_PACING_RESEARCH.md).
//!
//! Selected scope, not recovered or not yet ported: block/draft (data-gated
//! off), visibility_loss (no smoke value), traction control, AI ABS cap,
//! start-line reaction. The random offset generator is an authored LCG (the original RNG
//! stream is not identified). Characteristic values default to 1.0 (the live
//! player-slot set); the difficulty default is the live Normal value 0.606.
use crate::source_vehicle::Controls;
use grid_formats::{ai, binxml};
use std::{collections::BTreeMap, path::Path};

/// Live Normal difficulty observed in the original (all 12 driver objects).
pub const NORMAL_DIFFICULTY: f32 = 0.606;

/// Per-driver characteristic values (aggression, control, mistakes, reaction,
/// variance, speed_in, speed_out, virtual_performance) from a private TSV
/// (header row of names, then one row per driver: index then values), as read
/// from the original's driver objects. Row k drives car k.
pub fn read_characteristics(
    text: &str,
) -> Result<Vec<BTreeMap<String, f32>>, Box<dyn std::error::Error>> {
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let header: Vec<&str> = lines
        .next()
        .ok_or("empty characteristic table")?
        .split('\t')
        .collect();
    if header.first() != Some(&"driver") || header.len() < 2 {
        return Err("invalid characteristic table header".into());
    }
    let mut rows = Vec::new();
    for line in lines {
        let cells: Vec<&str> = line.split('\t').collect();
        if cells.len() != header.len() {
            return Err("characteristic row width differs from header".into());
        }
        let mut row = BTreeMap::new();
        for (name, value) in header[1..].iter().zip(&cells[1..]) {
            let v: f32 = value.parse()?;
            if !(0. ..=1.).contains(&v) {
                return Err("characteristic value outside 0..1".into());
            }
            row.insert(name.to_string(), v);
        }
        rows.push(row);
    }
    if rows.is_empty() {
        return Err("characteristic table has no drivers".into());
    }
    Ok(rows)
}

fn read_doc(path: &Path) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    Ok(std::fs::read(path)?)
}

/// Closed racing-line polyline in AI link order.
#[derive(Clone)]
pub struct Line {
    points: Vec<[f32; 3]>,
    lateral: Vec<[f32; 3]>,
    /// Racing-limit offsets relative to the racing line (left <= 0 <= right).
    limits: Vec<[f32; 2]>,
    /// Track-limit offsets (ai_track types 3/4) relative to the racing line.
    track_limits: Vec<[f32; 2]>,
    start: Vec<f32>,
    pub length: f32,
    /// Line index of each AI gate.
    gate_index: Vec<usize>,
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot2(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[2] * b[2]
}
fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
}
fn heading(d: [f32; 3]) -> f32 {
    d[0].atan2(d[2])
}
fn wrap(a: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    (a + std::f32::consts::PI).rem_euclid(tau) - std::f32::consts::PI
}

impl Line {
    fn new(track: &ai::Track) -> Result<Self, Box<dyn std::error::Error>> {
        let n = track.gates.len();
        if track.links.len() != n {
            return Err("AI links do not form one loop".into());
        }
        let mut order = Vec::with_capacity(n);
        let mut gate = track.links[0].1;
        for _ in 0..n {
            if order.contains(&gate) {
                return Err("AI links revisit a gate".into());
            }
            order.push(gate);
            gate = track
                .links
                .iter()
                .find(|l| l.0 == gate)
                .ok_or("AI link chain broken")?
                .1;
        }
        if gate != order[0] {
            return Err("AI links do not close".into());
        }
        let mut line = Line {
            points: Vec::with_capacity(n),
            lateral: Vec::with_capacity(n),
            limits: Vec::with_capacity(n),
            track_limits: Vec::with_capacity(n),
            start: Vec::with_capacity(n),
            length: 0.,
            gate_index: vec![0; n],
        };
        for (i, &g) in order.iter().enumerate() {
            let g_data = &track.gates[g];
            line.gate_index[g] = i;
            line.points.push(std::array::from_fn(|k| {
                g_data.position[k] + g_data.normal[k] * g_data.racing_line
            }));
            line.lateral.push(g_data.normal);
            line.limits.push([
                g_data.left_racing_limit - g_data.racing_line,
                g_data.right_racing_limit - g_data.racing_line,
            ]);
            line.track_limits.push([
                g_data.left_track_limit - g_data.racing_line,
                g_data.right_track_limit - g_data.racing_line,
            ]);
        }
        for i in 0..n {
            line.start.push(line.length);
            let d = sub(line.points[(i + 1) % n], line.points[i]);
            line.length += dot2(d, d).sqrt();
        }
        Ok(line)
    }

    fn segment(&self, i: usize) -> f32 {
        let next = if i + 1 < self.points.len() {
            self.start[i + 1]
        } else {
            self.length
        };
        next - self.start[i]
    }

    /// Point, unit direction, lateral direction and racing limits at arc length `s`.
    fn at(&self, s: f32) -> ([f32; 3], [f32; 3], [f32; 3], [f32; 2]) {
        let s = s.rem_euclid(self.length);
        let i = self.start.partition_point(|&x| x <= s).saturating_sub(1);
        let j = (i + 1) % self.points.len();
        let t = ((s - self.start[i]) / self.segment(i).max(1e-6)).clamp(0., 1.);
        let d = sub(self.points[j], self.points[i]);
        let len = dot2(d, d).sqrt().max(1e-6);
        let lateral = lerp3(self.lateral[i], self.lateral[j], t);
        let ll = dot2(lateral, lateral).sqrt().max(1e-6);
        (
            lerp3(self.points[i], self.points[j], t),
            [d[0] / len, 0., d[2] / len],
            [lateral[0] / ll, 0., lateral[2] / ll],
            [
                self.limits[i][0] + (self.limits[j][0] - self.limits[i][0]) * t,
                self.limits[i][1] + (self.limits[j][1] - self.limits[i][1]) * t,
            ],
        )
    }

    /// Track-limit offsets (left <= 0 <= right) at arc length `s`.
    fn track_limits_at(&self, s: f32) -> [f32; 2] {
        let s = s.rem_euclid(self.length);
        let i = self.start.partition_point(|&x| x <= s).saturating_sub(1);
        let j = (i + 1) % self.points.len();
        let t = ((s - self.start[i]) / self.segment(i).max(1e-6)).clamp(0., 1.);
        let [a, b] = [self.track_limits[i], self.track_limits[j]];
        [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
    }

    /// The racing-line points in link order (the HUD's minimap outline).
    pub fn points(&self) -> &[[f32; 3]] {
        &self.points
    }

    /// Arc length of the racing-line point nearest to `p` (global search).
    pub fn arc(&self, p: [f32; 3]) -> f32 {
        self.project(p, None).1
    }

    /// The racing line alone, from the route's `ai_track.xml`.
    pub fn load(root: &Path, route: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let bytes = read_doc(&root.join(route).join("ai_track.xml"))?;
        Self::new(&ai::track(&binxml::read(&bytes)?)?)
    }

    /// Player reset target for a car at `p`: the line point at the car's
    /// projection and the line's horizontal direction there.
    ///
    /// [I] The original takes the player's track-tracker point (0x5EC0E0:
    /// tracker +0x74/+0x78, possibly replaced by a saved point behind it or
    /// moved by 0x90C0F0) on the tracker's own line. Neither the saved point,
    /// the offset nor the tracker's line is ported; this uses the AI racing
    /// line, as the race progress does.
    pub fn reset_target(&self, p: [f32; 3]) -> ([f32; 3], [f32; 3]) {
        let (point, direction, _, _) = self.at(self.arc(p));
        (point, direction)
    }

    /// Projection of `p` near segment `hint` (or globally): segment, arc
    /// length and signed lateral distance (positive toward the right limit).
    fn project(&self, p: [f32; 3], hint: Option<usize>) -> (usize, f32, f32) {
        let n = self.points.len();
        let candidates: Vec<usize> = match hint {
            Some(h) => (0..8).map(|k| (h + n - 2 + k) % n).collect(),
            None => (0..n).collect(),
        };
        let mut best = (f32::INFINITY, 0, 0., 0.);
        for i in candidates {
            let a = self.points[i];
            let d = sub(self.points[(i + 1) % n], a);
            let square = dot2(d, d).max(1e-9);
            let t = (dot2(sub(p, a), d) / square).clamp(0., 1.);
            let q = lerp3(a, self.points[(i + 1) % n], t);
            let e = sub(p, q);
            let error = dot2(e, e);
            if error < best.0 {
                best = (error, i, self.start[i] + t * square.sqrt(), 0.);
            }
        }
        let (_, i, s, _) = best;
        let (point, _, lateral, _) = self.at(s);
        (i, s, dot2(sub(p, point), lateral))
    }
}

/// Speed->distance lookup over grip tables (0x7A64C0): linear in speed within
/// each table, then linear between the bracketing grips (grip clamped).
fn table_distance(table: &ai::GripTable, speed: f32) -> f32 {
    let p = &table.points;
    let speed = speed.max(0.);
    let mut previous = (0f32, 0f32);
    for &point in p {
        if speed <= point.0 {
            let t = (speed - previous.0) / (point.0 - previous.0).max(1e-6);
            return previous.1 + (point.1 - previous.1) * t;
        }
        previous = point;
    }
    // Above the table: continue the last segment.
    let n = p.len();
    if n < 2 {
        return p[n - 1].1;
    }
    let (a, b) = (p[n - 2], p[n - 1]);
    b.1 + (speed - b.0) * (b.1 - a.1) / (b.0 - a.0)
}
fn grip_distance(tables: &[ai::GripTable], grip: f32, speed: f32) -> f32 {
    let i = tables.partition_point(|t| t.grip < grip);
    if i == 0 {
        return table_distance(&tables[0], speed);
    }
    if i >= tables.len() {
        return table_distance(&tables[tables.len() - 1], speed);
    }
    let (a, b) = (&tables[i - 1], &tables[i]);
    let t = (grip - a.grip) / (b.grip - a.grip);
    let da = table_distance(a, speed);
    da + (table_distance(b, speed) - da) * t
}

/// Shipped AI data for one track, vehicle class and car.
#[derive(Clone)]
pub struct AiData {
    pub line: Line,
    /// `(arc length of the brake-line gate, class brake data)` per brake-line id.
    brake_lines: Vec<(f32, ai::BrakeData)>,
    brake_tables: Vec<ai::GripTable>,
    driver: BTreeMap<String, f32>,
    vehicle: BTreeMap<String, f32>,
    mappings: BTreeMap<String, ai::Mapping>,
    /// `behaviour_*` and `test_*` switches of the driver config (behaviour
    /// flags cfg+0xCB4+type, test flags cfg+0xCC4+type).
    switches: BTreeMap<String, f32>,
    /// The same data on the route's `drift` racing line, when the event's
    /// config enables the drift behaviour (line choice 0x794870 reads
    /// cfg+0xCC3 = behaviour_drift).
    drift: Option<Box<AiData>>,
}

/// `(arc length of the brake-line gate, class brake data)` per brake-line id.
type BrakeLines = Vec<(f32, ai::BrakeData)>;

/// Racing line and brake lines of `track` (brake-line arcs on that line).
fn track_lines(
    track: &ai::Track,
    class_lines: &Option<Vec<ai::BrakeData>>,
) -> Result<(Line, BrakeLines), Box<dyn std::error::Error>> {
    let line = Line::new(track)?;
    let mut brake_lines = Vec::new();
    for (id, b) in track.brake_lines.iter().enumerate() {
        let data = match class_lines {
            Some(lines) => *lines.get(id).ok_or("class brake line absent")?,
            None => b.data,
        };
        brake_lines.push((line.start[line.gate_index[b.gate]], data));
    }
    Ok((line, brake_lines))
}

impl AiData {
    /// `route` is the track route folder, e.g. `tracks/europe/donington/route_0`.
    pub fn load(
        root: &Path,
        route: &str,
        class: &str,
        car: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Self::load_for(root, route, class, car, None)
    }

    /// [`Self::load`] for an event of the competition format `format` (e.g.
    /// `drift_battle`): its `ai_driver_config.xml` section overrides the
    /// `default` values and switches, and its parameter-mapping section the
    /// `default` mappings [I: the section names are the format names; a
    /// single race has an empty `single_race` section and showed the
    /// `default` values live, F290]. With the drift behaviour switched on the
    /// data is also built on the route's `drift` racing line.
    pub fn load_for(
        root: &Path,
        route: &str,
        class: &str,
        car: &str,
        format: Option<&str>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Self::load_classes(root, route, (class, class), car, format)
    }

    /// [`Self::load_for`] with the car's own `database.bin` `vehicle` row:
    /// `ai_vehicle_type_config` selects the `ai_vehicle_config.xml` class,
    /// `ai_vehicle_type_track` the route's class brake lines and
    /// `ai_vehicle_type_statistics` the brake tables [D]. Lacetti and BMW are
    /// `touring`/`touring` with their own stems, as the fixed class before.
    pub fn load_car(
        root: &Path,
        route: &str,
        car: &crate::car::Car,
        format: Option<&str>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let v = car.vehicle(root)?;
        Self::load_classes(
            root,
            route,
            (&v.ai_config, &v.ai_track),
            &v.ai_statistics,
            format,
        )
    }

    /// `classes` = (`ai_vehicle_config.xml` class, `ai_vehicle_track.xml` class).
    pub fn load_classes(
        root: &Path,
        route: &str,
        classes: (&str, &str),
        car: &str,
        format: Option<&str>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let (class, track_class) = classes;
        let bytes = read_doc(&root.join(route).join("ai_track.xml"))?;
        let track_doc = binxml::read(&bytes)?;
        let track = ai::track(&track_doc)?;
        let bytes = read_doc(&root.join(route).join("ai_vehicle_track.xml"))?;
        let class_lines = ai::class_brake_lines(&binxml::read(&bytes)?, track_class)?;
        let (line, brake_lines) = track_lines(&track, &class_lines)?;
        let bytes = read_doc(&root.join("ai/ai_vehicle_statistics.xml"))?;
        let brake_tables = ai::statistics(&binxml::read(&bytes)?, car)?.brake;
        let bytes = read_doc(&root.join("ai/ai_driver_config.xml"))?;
        let config = binxml::read(&bytes)?;
        let mut driver = ai::config(&config, "ai_driver_config", &[("default", None)])?;
        let mut switches = BTreeMap::new();
        for section in std::iter::once("default").chain(format) {
            // A format without its own section keeps the defaults.
            if section != "default"
                && let Ok(values) = ai::config(&config, "ai_driver_config", &[(section, None)])
            {
                driver.extend(values);
            }
            for group in ["behaviour", "behaviourTest"] {
                if let Ok(values) =
                    ai::config(&config, "ai_driver_config", &[(section, None), (group, None)])
                {
                    switches.extend(values);
                }
            }
        }
        let bytes = read_doc(&root.join("ai/ai_vehicle_config.xml"))?;
        let doc = binxml::read(&bytes)?;
        let mut vehicle = ai::config(
            &doc,
            "ai_vehicle_config",
            &[("vehicle_types", None), ("vehicle_type", Some("default"))],
        )?;
        if let Ok(overrides) = ai::config(
            &doc,
            "ai_vehicle_config",
            &[("vehicle_types", None), ("vehicle_type", Some(class))],
        ) {
            vehicle.extend(overrides);
        }
        let bytes = read_doc(&root.join("ai/ai_driver_parameter_mappings.xml"))?;
        let mapping_doc = binxml::read(&bytes)?;
        let mut mappings = ai::mappings(&mapping_doc, "default")?;
        if let Some(format) = format
            && let Ok(overrides) = ai::mappings(&mapping_doc, format)
        {
            mappings.extend(overrides);
        }
        let mut data = Self {
            line,
            brake_lines,
            brake_tables,
            driver,
            vehicle,
            mappings,
            switches,
            drift: None,
        };
        if data.switch("behaviour_drift") {
            let (line, brake_lines) = track_lines(&ai::track_on(&track_doc, "drift")?, &class_lines)?;
            data.drift = Some(Box::new(Self {
                line,
                brake_lines,
                ..data.clone()
            }));
        }
        Ok(data)
    }

    /// A `behaviour_*` or `test_*` switch of the driver config.
    fn switch(&self, name: &str) -> bool {
        self.switches.get(name).is_some_and(|&v| v != 0.)
    }

    /// Braking distance from `from` down to `to` (0x7A6650).
    pub fn braking_distance(&self, grip: f32, from: f32, to: f32) -> f32 {
        grip_distance(&self.brake_tables, grip, from) - grip_distance(&self.brake_tables, grip, to)
    }
}

/// Evaluated parameters for one driver.
#[derive(Clone, Debug)]
pub struct Parameters {
    la: [f32; 10],
    th_target_small: f32,
    th_target_medium: f32,
    th_target_speed_small: f32,
    th_target_speed_big: f32,
    th_target_move_speed: f32,
    th_corner_lift_min: f32,
    th_brake_line_speed_drop: f32,
    tg_offset_speed_factor: f32,
    tg_offset_max_for_speed: f32,
    ra_generate: bool,
    ra_merge: f32,
    ra_merge_cornering: f32,
    ra_merge_alone: f32,
    ra_alone_on: f32,
    ra_alone_off: f32,
    ra_generate_time: f32,
    cornering_curvature: f32,
    st_ang_vel_factor: f32,
    st_angle_factor: f32,
    slide_lift: [f32; 3],
    brake_cruise_exponent: f32,
    /// Gains P/I/D and limits P/I/D/output (ai_vehicle_config pid*).
    pid: [f32; 7],
    pub steering_speed: f32,
    brake_on_speed: f32,
    brake_off_speed: f32,
    throttle_on_speed: f32,
    throttle_off_speed: f32,
    pub corner_speed_in_ratio: f32,
    pub corner_brake_distance_factor: f32,
    pub throttle_confidence: f32,
    corner_lift: [f32; 2],
    /// thFollow small/medium throttle, dist big/medium/small, speed small/medium.
    follow: [f32; 7],
    follow_brake: f32,
    traffic: Traffic,
    rescue: Rescue,
    deviation_small: f32,
    deviation_large: f32,
    faults: Faults,
    /// tgNonOptimalSpeedFactor: corner-speed factor on a line other than
    /// `optimal` (0x792C80).
    tg_non_optimal_speed_factor: f32,
    /// behaviour_race_damaged and behaviour_controlled_stop switches.
    race_damaged: bool,
    controlled_stop: bool,
    /// The drift behaviour and test, when the event's config switches both on.
    pub drift: Option<Drift>,
}

/// Drift behaviour parameters (docs/DISCIPLINES.md §4.1): ai_vehicle_config
/// `drift*` (vcfg+0x3B0..+0x4F0), thBrakeLineHoldTimeDrift (cfg+0x2E8) and the
/// mappings drift_skill (45) and drift_position (46). Built-in defaults where
/// the shipped XML is silent: driftInitiateGrip 1.2, driftRacingGrip 1.1,
/// driftBrakeLineAdvance 8 m.
#[derive(Clone, Debug)]
pub struct Drift {
    st_ang_vel_factor: f32,
    /// driftAngleRangeMin/Max and driftAngleMax (degrees).
    angle_range: [f32; 2],
    angle_max: f32,
    /// driftMinGrip / driftMaxGrip (rear wheels).
    min_grip: f32,
    max_grip: f32,
    /// driftStartAngle / driftEndAngle (degrees).
    start_angle: f32,
    end_angle: f32,
    merge: f32,
    speed_min: f32,
    grip_front: f32,
    initiate_grip: f32,
    racing_grip: f32,
    brake_line_advance: f32,
    hold_time: f32,
    pub skill: f32,
    pub position: f32,
}

/// Mistake, failure and let-pass parameters (docs/AI_MISTAKES_PACING_RESEARCH.md
/// §2, §4.2, §5; built-in defaults where the shipped XML is silent).
#[derive(Clone, Debug)]
pub struct Faults {
    /// mistake_probability (map 33) and the per-type factors, by mistake id:
    /// heavy, light, late, early, brake_spin, powerout_spin, brake_lock. The
    /// brake_lock slot reads holder 49, past the table: 0 live [V].
    probability: f32,
    factors: [f32; 7],
    /// generateMistakes (cfg+0xCAC, built-in false) and
    /// mkNumBrakeLinesBeforeMistakes (cfg+0xC84, 3).
    generate: bool,
    lines_before: u32,
    /// mistakeEntrySpinRearGrip, mistakePowerSpinRearGrip, mistakeBrakeLockGrip.
    spin_rear_grip: f32,
    power_rear_grip: f32,
    lock_grip: f32,
    racing_grip: f32,
    crash_front_grip: f32,
    crash_rear_grip: f32,
    crash_min_speed: f32,
    retirement_speed: f32,
    /// rtStoppedSpeed (cfg+0x4F0, built-in 0.5).
    stopped_speed: f32,
    damage_retirement_ratio: f32,
    /// coLetPassDistance (built-in 30) and coLetPassThrottleRatio.
    let_pass_distance: f32,
    let_pass_throttle: f32,
}

/// Avoid, recovery and retire parameters (docs/AI_AVOID_RECOVERY_RESEARCH.md
/// §2.4, §3, §4; built-in defaults where the shipped XML is silent).
#[derive(Clone, Debug)]
pub struct Rescue {
    av_eta: f32,
    av_decision: f32,
    av_buffer: f32,
    av_buffer_speed: f32,
    av_merge: f32,
    av_min_racing_speed: f32,
    av_max_racing_angle: f32,
    av_min_speed_normal: f32,
    av_min_speed_slow: f32,
    av_stopped_speed: f32,
    av_stopped_target: f32,
    av_normal_target: f32,
    av_wrong_way_target: f32,
    av_alongside: bool,
    av_figure_of_eight: bool,
    braking_buffer: f32,
    rc_merge: f32,
    rc_collision_timeout: f32,
    rt_stuck_time: f32,
    rt_stuck_distance_sq: f32,
    rt_player_distance_sq: f32,
    tg_merge: f32,
    target_speed_big: f32,
    contact_zone: f32,
}

/// Fire priorities (ai_behaviour_fire.xml, docs/AI_AVOID_RECOVERY_RESEARCH.md §4).
const AVOID_SCORE: u8 = 15;
const RECOVERY_SCORES: [u8; 4] = [11, 16, 17, 19];
const RECOVER_TBONE_SCORE: u8 = 18;
const RETIRE_STUCK_SCORE: u8 = 23;
/// Spun-test hysteresis (.data 0xE4D0F0 0.1·π, 0xCF6F08 π/3, 0xE4D0EC 3 m/s).
const SPUN_HOLD_SLIP: f32 = 0.1 * std::f32::consts::PI;
const SPUN_HOLD_HEADING: f32 = std::f32::consts::FRAC_PI_3;
const SPUN_HOLD_SPEED: f32 = 3.;
/// recover_tbone hold after the last qualifying contact (0x78ACC0).
const TBONE_TIME: f32 = 1.;
/// Recovery reverse speed and the stop speed before reversing (0x78EE70).
const RECOVERY_REVERSE_SPEED: f32 = 5.;
const RECOVERY_STOPPED_SPEED: f32 = 0.1;
/// avoid follow distance in side mode 0 (0x799DE0).
const AVOID_FOLLOW_DISTANCE: f32 = 3.;

/// Built-in defaults for values absent from the shipped XML
/// (docs/AI_DRIVER_RESEARCH.md §4).
const BRAKE_LINE_CANCEL_FACTOR: f32 = 0.8;
const SLOW_SPEED: f32 = 3.;
const MINIMUM_MOVING_SPEED: f32 = 1.;
const SLIDE_LIFT_MIN_SPEED: f32 = 5.;
/// Grip input for the speed->distance tables. The AI car-view grip (+0x144)
/// is not yet traced; the lowest shipped grip is used.
const TABLE_GRIP: f32 = 2.;

impl Parameters {
    pub fn new(
        data: &AiData,
        difficulty: f32,
        characteristic: impl Fn(&str) -> f32,
    ) -> Result<Self, String> {
        let d = |k: &str| {
            data.driver
                .get(k)
                .copied()
                .ok_or(format!("AI driver config {k} absent"))
        };
        let v = |k: &str| {
            data.vehicle
                .get(k)
                .copied()
                .ok_or(format!("AI vehicle config {k} absent"))
        };
        let d_or = |k: &str, default: f32| data.driver.get(k).copied().unwrap_or(default);
        let v_or = |k: &str, default: f32| data.vehicle.get(k).copied().unwrap_or(default);
        // A behaviour absent from the config keeps the port's default (on).
        let on = |k: &str| data.switches.get(k).is_none_or(|&s| s != 0.);
        let m = |k: &str| {
            data.mappings
                .get(k)
                .map(|m| m.evaluate(difficulty, characteristic(&m.characteristic)))
                .ok_or(format!("AI mapping {k} absent"))
        };
        Ok(Self {
            la: [
                d("laDistMin")?,
                d("laDistMax")?,
                d("laAngleMin")?,
                d("laAngleMax")?,
                d("laSpeedMin")?,
                d("laSpeedMax")?,
                d("laStepMax")?,
                d("laStepProportion")?,
                d("laDistExtraMin")?,
                d("laSpeedExtraMin")?,
            ],
            th_target_small: d("thTargetSmall")?,
            th_target_medium: d("thTargetMedium")?,
            th_target_speed_small: d("thTargetSpeedSmall")?,
            th_target_speed_big: d("thTargetSpeedBig")?,
            th_target_move_speed: d("thTargetMoveSpeed")?,
            th_corner_lift_min: d("thCornerLiftMin")?,
            th_brake_line_speed_drop: d("thBrakeLineSpeedDrop")?,
            tg_offset_speed_factor: d("tgOffsetSpeedFactor")?,
            tg_offset_max_for_speed: d("tgOffsetMaxForSpeed")?,
            ra_generate: d("raGenerateRandomOffsets")? != 0.,
            ra_merge: d("raOffsetMergeFactor")?,
            ra_merge_cornering: d("raOffsetMergeFactorCornering")?,
            ra_merge_alone: d("raOffsetMergeFactorAlone")?,
            ra_alone_on: d("raAloneDistanceOn")?,
            ra_alone_off: d("raAloneDistanceOff")?,
            ra_generate_time: d("raOffsetGenerateTime")?,
            cornering_curvature: d("crMaxTrackCurvature")?,
            st_ang_vel_factor: v("stAngVelFactor")?,
            st_angle_factor: v("stAngleFactorMin")?,
            slide_lift: [
                v("throttleSlideLiftStartAngle")?,
                v("throttleSlideLiftEndAngle")?,
                v("throttleSlideLiftMin")?,
            ],
            brake_cruise_exponent: v("brakeCruiseExponent")?,
            pid: [
                v("pidProportionalGain")?,
                v("pidIntegralGain")?,
                v("pidDerivativeGain")?,
                v("pidProportionalLimit")?,
                v("pidIntegralLimit")?,
                v("pidDerivativeLimit")?,
                v("pidOutputLimit")?,
            ],
            steering_speed: m("steering_speed")?,
            brake_on_speed: m("brake_on_speed")?,
            brake_off_speed: m("brake_off_speed")?,
            throttle_on_speed: m("throttle_on_speed")?,
            throttle_off_speed: m("throttle_off_speed")?,
            corner_speed_in_ratio: m("corner_speed_in_ratio")?,
            corner_brake_distance_factor: m("corner_brake_distance_factor")?,
            throttle_confidence: m("throttle_confidence")?,
            corner_lift: [
                m("throttle_corner_lift_start_angle")?,
                m("throttle_corner_lift_end_angle")?,
            ],
            follow: [
                d("thFollowSmall")?,
                d("thFollowMedium")?,
                d("thFollowDistBig")?,
                d("thFollowDistMedium")?,
                d("thFollowDistSmall")?,
                d("thFollowSpeedSmall")?,
                d("thFollowSpeedMedium")?,
            ],
            follow_brake: d("brFollowSpeedBrake")?,
            traffic: Traffic {
                ov_reject: d_or("ovRejectDistance", 60.),
                ov_side_reject: d_or("ovSideRejectDistance", 12.),
                ov_speed_difference: d_or("ovSpeedDifference", 0.1),
                ov_continue: d_or("ovContinueSpeedDifference", -2.),
                ov_time: d_or("ovTime", 3.),
                ov_auto_accept: d_or("ovAutoAcceptDistance", 3.),
                ov_slow_speed: d_or("ovSlowSpeed", 12.),
                ov_slow_reject: d_or("ovSlowRejectDistance", 12.),
                ov_follow: d_or("ovFollowDistance", 3.),
                ov_decision: d_or("ovDecisionTimeOut", 0.25),
                ov_safe: d_or("ovSafeSeparationDistance", 1.),
                ov_desired: d_or("ovDesiredSeparationDistance", 2.5),
                ov_min_fire: d_or("ovMinFireTime", 3.),
                ov_merge: d_or("ovOffsetMergeFactor", 4.),
                ov_merge_corner: d_or("ovOffsetMergeFactorCorner", 0.5),
                ov_corner: [
                    d_or("ovMinCornerCurvature", 0.001),
                    d_or("ovMaxCornerCurvature", 0.04),
                ],
                as_reject: d_or("asRejectDistance", 10.),
                as_merge: d_or("asOffsetMergeFactor", 4.),
                front_projection: [
                    m("alongside_front_projection_on_distance")?,
                    m("alongside_front_projection_off_distance")?,
                ],
                rear_projection: [
                    m("alongside_rear_projection_on_distance")?,
                    m("alongside_rear_projection_off_distance")?,
                ],
                min_space: m("alongside_min_space_to_leave_distance")?,
                max_space: m("alongside_max_space_to_leave_distance")?,
                yield_: [
                    m("alongside_yield_distance")?,
                    m("alongside_yield_max_brake")?,
                    m("alongside_yield_min_speed_diff")?,
                    m("alongside_yield_max_speed_diff")?,
                    m("alongside_yield_follow_distance")?,
                    m("alongside_yield_extra_respect_distance")?,
                    d_or("coYieldMinSpeed", 16.),
                    d_or("coYieldDistToLimit", 1.),
                ],
                pass_angle: d_or("avPassAngle", 0.15),
                similar_angle: d_or("avSimilarDirectionMaxAngle", 0.5),
                pass_slow_speed: d_or("avPassSlowSpeed", 15.),
                pass_spare_room: d_or("avPassMaxSpareRoom", 4.),
                pass_side_speed: [
                    d_or("avPassSideSpeedNear", 2.),
                    d_or("avPassSideSpeedFar", 10.),
                ],
                pass_relative_speed: d_or("avPassMaxRelativeSpeed", 20.),
                separation_speed: d_or("avSeparationSpeedDiff", 5.),
                slow_speed: d_or("avSlowSpeed", 40.),
                pedal_push: d_or("avPercentagePedalPushDistance", 1.2),
                buffer: v("brakeBufferZone")?,
                extra_prediction: v("brakeExtraPrediction")?,
                separation: [
                    m("safe_separation_distance_small")?,
                    m("safe_separation_distance_large")?,
                ],
                prediction_ratio: m("brake_for_vehicle_prediction_ratio")?,
                cruise_speed: m("brake_for_vehicle_cruise_speed")?,
                view_angle: m("driver_view_angle")?,
            },
            rescue: Rescue {
                av_eta: d_or("avETAReactionTime", 2.),
                av_decision: d_or("avDecisionTime", 0.25),
                av_buffer: d_or("avBufferZone", 2.),
                av_buffer_speed: d_or("avBufferZoneBaseSpeed", 10.),
                av_merge: d_or("avOffsetMergeFactor", 5.),
                av_min_racing_speed: d_or("avMinRacingSpeed", 4.),
                av_max_racing_angle: d_or("avMaxRacingAngleFromTrack", 0.785),
                av_min_speed_normal: d_or("avMinSpeedNormal", 0.),
                av_min_speed_slow: d_or("avMinSpeedSlow", 9.),
                av_stopped_speed: d_or("avStoppedSpeed", 1.),
                av_stopped_target: d_or("avStoppedTargetSpeed", 30.),
                av_normal_target: d_or("avNormalTargetSpeed", 200.),
                av_wrong_way_target: d_or("avWrongWayTargetSpeed", 5.),
                av_alongside: d_or("avAvoidAlongside", 1.) != 0.,
                av_figure_of_eight: d_or("avFigureOfEightTrack", 0.) != 0.,
                braking_buffer: m("braking_buffer_distance")?,
                rc_merge: d_or("rcMergeFactor", 1.),
                rc_collision_timeout: d_or("rcCollisionTimeOut", 2.),
                rt_stuck_time: d_or("rtStuckTime", 20.),
                rt_stuck_distance_sq: d_or("rtStuckDistanceSq", 25.),
                rt_player_distance_sq: d_or("rtPlayerDistanceSq", 10000.),
                tg_merge: d_or("tgOffsetMergeFactor", 2.5),
                target_speed_big: d_or("brTargetSpeedBig", 2.),
                contact_zone: d_or("contactZone", 0.1),
            },
            deviation_small: m("deviation_from_racing_line_small")?,
            deviation_large: m("deviation_from_racing_line_large")?,
            faults: Faults {
                probability: m("mistake_probability")?,
                factors: [
                    m("mistake_brake_heavy_factor")?,
                    m("mistake_brake_light_factor")?,
                    m("mistake_brake_late_factor")?,
                    m("mistake_brake_early_factor")?,
                    m("mistake_brake_spin_factor")?,
                    m("mistake_powerout_spin_factor")?,
                    0.,
                ],
                generate: d_or("generateMistakes", 0.) != 0.,
                lines_before: d_or("mkNumBrakeLinesBeforeMistakes", 3.) as u32,
                spin_rear_grip: v("mistakeEntrySpinRearGrip")?,
                power_rear_grip: v("mistakePowerSpinRearGrip")?,
                lock_grip: v("mistakeBrakeLockGrip")?,
                racing_grip: v("racingGripMultiplier")?,
                crash_front_grip: v("crashFrontGrip")?,
                crash_rear_grip: v("crashRearGrip")?,
                crash_min_speed: v("crashMinSpeed")?,
                retirement_speed: d("rtRetirementSpeed")?,
                stopped_speed: d_or("rtStoppedSpeed", 0.5),
                damage_retirement_ratio: m("damage_retirement_ratio")?,
                let_pass_distance: d_or("coLetPassDistance", 30.),
                let_pass_throttle: d_or("coLetPassThrottleRatio", 0.8),
            },
            tg_non_optimal_speed_factor: d_or("tgNonOptimalSpeedFactor", 0.8),
            race_damaged: on("behaviour_race_damaged"),
            controlled_stop: on("behaviour_controlled_stop"),
            drift: if data.switch("behaviour_drift") && data.switch("test_drift") {
                Some(Drift {
                    st_ang_vel_factor: v("driftStAngVelFactor")?,
                    angle_range: [v("driftAngleRangeMin")?, v("driftAngleRangeMax")?],
                    angle_max: v("driftAngleMax")?,
                    min_grip: v("driftMinGrip")?,
                    max_grip: v("driftMaxGrip")?,
                    start_angle: v("driftStartAngle")?,
                    end_angle: v("driftEndAngle")?,
                    merge: v("driftMergeFactor")?,
                    speed_min: v("driftSpeedMin")?,
                    grip_front: v("driftGripFront")?,
                    initiate_grip: v_or("driftInitiateGrip", 1.2),
                    racing_grip: v_or("driftRacingGrip", 1.1),
                    brake_line_advance: v_or("driftBrakeLineAdvance", 8.),
                    hold_time: d_or("thBrakeLineHoldTimeDrift", 0.2),
                    skill: m("drift_skill")?,
                    position: m("drift_position")?,
                })
            } else {
                None
            },
        })
    }
}

/// Traffic behaviour parameters (shipped config, built-in defaults where absent).
#[derive(Clone, Debug)]
pub struct Traffic {
    ov_reject: f32,
    ov_side_reject: f32,
    ov_speed_difference: f32,
    ov_continue: f32,
    ov_time: f32,
    ov_auto_accept: f32,
    ov_slow_speed: f32,
    ov_slow_reject: f32,
    ov_follow: f32,
    ov_decision: f32,
    ov_safe: f32,
    ov_desired: f32,
    ov_min_fire: f32,
    ov_merge: f32,
    ov_merge_corner: f32,
    ov_corner: [f32; 2],
    as_reject: f32,
    as_merge: f32,
    front_projection: [f32; 2],
    rear_projection: [f32; 2],
    min_space: f32,
    max_space: f32,
    /// Yield: distance, max brake, min/max speed difference, follow distance,
    /// extra respect, minimum own speed, room to the limit.
    yield_: [f32; 8],
    pass_angle: f32,
    similar_angle: f32,
    pass_slow_speed: f32,
    pass_spare_room: f32,
    pass_side_speed: [f32; 2],
    pass_relative_speed: f32,
    separation_speed: f32,
    slow_speed: f32,
    pedal_push: f32,
    buffer: f32,
    extra_prediction: f32,
    separation: [f32; 2],
    prediction_ratio: f32,
    cruise_speed: f32,
    view_angle: f32,
}

/// A car's AI traffic extents (m) along its own axes from the body origin.
///
/// The AI car-state update (0x402D20) takes front, rear and half width from
/// the body dimension getter 0x9B8750: the `.ned` `collision_main` maximum Z,
/// minimum Z and maximum X (body +0xED6C/+0xED88/+0xED70) plus the body
/// offset +0x1630 on Z. It then grows them to cover the tyres: front-left
/// wheel Z + radius, rear-right wheel Z - radius, front-left X + half tyre
/// width, |rear-right X - half tyre width|. The state keeps the half width
/// (+0x110), the length front - rear (+0x114) and the bumper points
/// centre + forward * front / rear (+0xF0/+0x100); observed records copy
/// them (0x777A80).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Extents {
    pub half_width: f32,
    /// Front bumper distance ahead of the body origin.
    pub front: f32,
    /// Rear bumper position along forward (negative: behind the origin).
    pub rear: f32,
}

impl Extents {
    /// From the car's collision hull bounds and its vehicle words (wheel
    /// positions +0x14E0.., radii +0x1520.., tyre widths +0x1540.., body
    /// offset +0x1630). The optional deformation-lattice transform of the
    /// two bumper points (0x9A39D0 when body +0xC7AD is set) is not ported:
    /// the extents stay those of the undamaged hull.
    pub fn from_vehicle(hull: grid_formats::car_collision::Bounds, vehicle: &[u32]) -> Self {
        let w = |o: usize| f32::from_bits(vehicle[o / 4]);
        let offset_z = w(0x1638);
        let mut front = hull.max[2] + offset_z;
        let mut rear = hull.min[2] + offset_z;
        // Wheels: +0x1500 front-left centre, +0x14F0 rear-right centre.
        let front_tyre = w(0x1508) + w(0x1528);
        let rear_tyre = w(0x14F8) - w(0x1524);
        // Only one of the two length tests runs (0x403409/0x403473).
        if front_tyre > front {
            front = front_tyre;
        } else if rear > rear_tyre {
            rear = rear_tyre;
        }
        let mut half_width = hull.max[0];
        let left_tyre = w(0x1500) + w(0x1548) * 0.5;
        let right_tyre = (w(0x14F0) - w(0x1544) * 0.5).abs();
        if left_tyre > half_width {
            half_width = left_tyre;
        } else if right_tyre > half_width {
            half_width = right_tyre;
        }
        Self {
            half_width,
            front,
            rear,
        }
    }
    /// Half the length (car state +0x114 / 2).
    pub fn half_length(&self) -> f32 {
        0.5 * (self.front - self.rear)
    }
}

/// Another car seen by the AI (observed record: id, centre, axes, velocity,
/// extents).
#[derive(Clone, Copy)]
pub struct Other {
    pub id: u32,
    pub position: [f32; 3],
    pub velocity: [f32; 4],
    /// Unit forward and car-left (+X) axes.
    pub forward: [f32; 3],
    pub left: [f32; 3],
    pub extents: Extents,
}

impl Other {
    /// From a 4x4 row-major pose (rows 0/2 = left/forward, 12..14 translation).
    pub fn from_pose(id: u32, pose: [f32; 16], velocity: [f32; 4], extents: Extents) -> Self {
        Self {
            id,
            position: [pose[12], pose[13], pose[14]],
            velocity,
            forward: [pose[8], pose[9], pose[10]],
            left: [pose[0], pose[1], pose[2]],
            extents,
        }
    }
    /// The observed record of a source car.
    pub fn of(id: u32, car: &crate::source_vehicle::Runtime) -> Self {
        Self::from_pose(id, car.pose(), car.velocity(), car.ai_extents())
    }
    fn half_width(&self) -> f32 {
        self.extents.half_width
    }
    fn half_length(&self) -> f32 {
        self.extents.half_length()
    }
    fn speed(&self) -> f32 {
        let v = self.velocity;
        (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
    }
    fn front(&self) -> [f32; 3] {
        std::array::from_fn(|k| self.position[k] + self.forward[k] * self.extents.front)
    }
    fn rear(&self) -> [f32; 3] {
        std::array::from_fn(|k| self.position[k] + self.forward[k] * self.extents.rear)
    }
    /// Record state 2 (not racing) when the car moves against its nose [I].
    fn backwards(&self) -> bool {
        let v = [self.velocity[0], self.velocity[1], self.velocity[2]];
        self.speed() > MINIMUM_MOVING_SPEED && dot3(v, self.forward) < 0.
    }
    /// Box corners inflated by `margin`, flattened to the car's height:
    /// front-right, front-left, rear-right, rear-left (0x77B400/0x77F9F0).
    fn corners(&self, margin: f32) -> [[f32; 3]; 4] {
        let f = horizontal(self.forward);
        let right = horizontal(self.left.map(|x| -x));
        let e = self.extents;
        let (front, rear, w) = (e.front + margin, e.rear - margin, e.half_width + margin);
        let at =
            |a: f32, b: f32| std::array::from_fn(|k| self.position[k] + f[k] * a + right[k] * b);
        [at(front, w), at(front, -w), at(rear, w), at(rear, -w)]
    }
    /// Box-local (forward, right) coordinates of `p`.
    fn local(&self, p: [f32; 3]) -> (f32, f32) {
        let d = sub(p, self.position);
        (
            dot2(d, horizontal(self.forward)),
            dot2(d, horizontal(self.left.map(|x| -x))),
        )
    }
    fn contains(&self, p: [f32; 3], margin: f32) -> bool {
        let (x, y) = self.local(p);
        let e = self.extents;
        e.rear - margin <= x && x <= e.front + margin && y.abs() <= e.half_width + margin
    }
}

/// contacted_vehicle (0x784650) contact type of `own` against `other`
/// (§3.5): 1-4 own corner inside the other box, 5-8 an other corner inside
/// our box by our nearest face (front, rear, right, left). The two-corner
/// length-ratio refinement is not ported.
fn contact_type(own: &Other, other: &Other, margin: f32) -> Option<u8> {
    if let Some(k) = own
        .corners(margin)
        .iter()
        .position(|&c| other.contains(c, margin))
    {
        return Some(k as u8 + 1);
    }
    let corner = other
        .corners(margin)
        .into_iter()
        .find(|&c| own.contains(c, margin))?;
    let (x, y) = own.local(corner);
    let e = own.extents;
    let w = e.half_width + margin;
    let faces = [e.front + margin - x, x - (e.rear - margin), w - y, w + y];
    let nearest = (0..4).min_by(|&a, &b| faces[a].total_cmp(&faces[b]))?;
    Some(5 + nearest as u8)
}

fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn horizontal(a: [f32; 3]) -> [f32; 3] {
    let l = (a[0] * a[0] + a[2] * a[2]).sqrt().max(1e-6);
    [a[0] / l, 0., a[2] / l]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Behaviour {
    #[default]
    Race,
    Overtake,
    Alongside,
    Avoid,
    Recovery,
    RecoverTbone,
    Retire,
    /// Choreographed failures and stops (docs/AI_MISTAKES_PACING_RESEARCH.md §5):
    /// lost_control (21), controlled_stop (20), race_damaged (9) and the
    /// retire behaviour's failure/damage branch (score 10).
    LostControl,
    ControlledStop,
    RaceDamaged,
    RetireFailure,
    /// Drift events (score 2/12/13, docs/DISCIPLINES.md §4.1).
    Drift,
}

/// The car's place in the race, read by the drift test (test context +0x14
/// count, +0x18: +8 place, +0x10 race progress).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RaceView {
    /// 0-based race position (0 = leader).
    pub place: u32,
    pub cars: u32,
    /// Race progress in laps (not positive before the start line).
    pub progress: f32,
}

impl Default for RaceView {
    fn default() -> Self {
        Self {
            place: 0,
            cars: 1,
            progress: 0.,
        }
    }
}

/// Drift test object (0x78FE90, created by 0x7A7950: +8 fired, +0x10 place
/// when it fired, +0x14 timer = -1, +0x18 before-start flag) and the drift
/// behaviour state (+0x30, 0 at construction 0x795AC0).
#[derive(Clone, Copy, Debug)]
struct DriftState {
    fired: bool,
    place: u32,
    timer: f32,
    before_start: bool,
    state: u8,
    /// The previous update's next brake line: (drift speed, distance), the
    /// request fields +0x1C/+0x28 the test reads (0x78CB30).
    line: Option<(f32, f32)>,
}

impl Default for DriftState {
    fn default() -> Self {
        Self {
            fired: false,
            place: 0,
            timer: -1.,
            before_start: false,
            state: 0,
            line: None,
        }
    }
}

impl DriftState {
    /// One update of the drift test 0x78FE90 with the race place `race` and
    /// `draw(a, b)` uniform between a and b. Returns true when the test is
    /// fired and the refusal 0x78CB30 must run.
    /// - Before the start (progress not positive) a first window is drawn.
    /// - Racing, the test fires when its timer runs out, or at once on an
    ///   even place after its first drift; a fired test lasts a random
    ///   window and stops when that ends unless the car gained places or
    ///   leads, then pauses for a random time.
    fn test(&mut self, race: RaceView, dt: f32, mut draw: impl FnMut(f32, f32) -> f32) -> bool {
        let RaceView {
            place,
            cars,
            progress,
        } = race;
        let f = if cars < 2 {
            0.
        } else {
            place as f32 / (cars - 1) as f32
        };
        let window = |ranges: [[f32; 2]; 2]| {
            (
                remap(f, 0., 1., ranges[0][0], ranges[0][1]),
                remap(f, 0., 1., ranges[1][0], ranges[1][1]),
            )
        };
        if progress <= 0. {
            self.fired = false;
            self.before_start = true;
            if self.timer < 0. {
                let a = (f * -8. + 10.).clamp(2., 10.);
                let b = (f * -12. + 20.).clamp(8., 20.);
                self.timer = draw(a, b);
            }
            return false;
        }
        self.timer -= dt;
        if self.fired {
            if self.timer <= 0. && self.place <= place && place != 0 {
                self.fired = false;
                let (a, b) = window(DRIFT_PAUSE);
                self.timer = draw(a, b);
                return false;
            }
        } else {
            if self.timer < 0. || (place % 2 == 0 && !self.before_start) {
                self.fired = true;
            }
            if !self.fired {
                return false;
            }
            let (a, b) = window(DRIFT_FIRE);
            self.timer = draw(a, b);
            self.place = place;
            self.before_start = false;
        }
        true
    }
}

impl Drift {
    /// Drift behaviour state update 0x78B3C0: 1 approach, 2 initiate (a
    /// cornering test below 50 m/s), 3 drifting (slip above driftStartAngle),
    /// 4 end (slip below driftEndAngle or the corner over), then 1 again;
    /// below driftSpeedMin it is 1.
    fn next_state(&self, state: u8, speed: f32, slip: f32, cornering: bool) -> u8 {
        let state = state.max(1);
        if speed < self.speed_min {
            return 1;
        }
        match state {
            1 if cornering && speed < DRIFT_INITIATE_SPEED => 2,
            2 if slip > self.start_angle.to_radians() => 3,
            3 if slip < self.end_angle.to_radians() || !cornering => 4,
            4 => 1,
            s => s,
        }
    }

    /// Request grips while drifting (0x783A10 state 3), rear L/R then front
    /// L/R: the front gets driftGripFront; the rear driftMinGrip below the
    /// skill-scaled angle range, 1 inside it and up to driftMaxGrip toward
    /// driftAngleMax above it. The range's top is lowered to the angle at
    /// which the car (half width, length) still fits the `outside` room to
    /// the track limit: asin((room - w) / (length + w)) (0xBF8C93 = asin).
    fn drifting_grip(&self, angle: f32, outside: f32, [half_width, length]: [f32; 2]) -> [f32; 4] {
        let (fit, reach) = (outside - half_width, length + half_width);
        let limit = if fit < reach {
            (fit / reach).asin().to_degrees()
        } else {
            90.
        };
        let mut high = self.angle_range[1];
        if limit < high {
            high = limit;
        }
        let low = self.skill * self.angle_range[0];
        let rear = if angle > high {
            remap(angle, high, self.angle_max, 1., self.max_grip)
        } else if low <= angle {
            1.
        } else {
            remap(angle, low, high, self.min_grip, 1.)
        };
        [rear, rear, self.grip_front, self.grip_front]
    }
}

/// Drift test ranges (0x78FE90, .rdata): first window before the start
/// clamp(10 - 8f, 2, 10)..clamp(20 - 12f, 8, 20) s, fired window 48..16 and
/// 24..8 s, pause 15..30 and 5..15 s over the place fraction f (0 leader,
/// 1 last), retry after a refused drift 3 s.
const DRIFT_FIRE: [[f32; 2]; 2] = [[48., 16.], [24., 8.]];
const DRIFT_PAUSE: [[f32; 2]; 2] = [[15., 30.], [5., 15.]];
const DRIFT_RETRY: f32 = 3.;
/// Drift behaviour: initiation needs speed below 50 m/s (0xCB1104); the wall
/// limit starts at 90 degrees (0xCB3D64); fire scores (ai_behaviour_fire.xml
/// drift entries 2, 12, 13).
const DRIFT_INITIATE_SPEED: f32 = 50.;
const DRIFT_SCORES: [u8; 3] = [2, 12, 13];
/// The next corner's direction is measured 20 m past the brake line (0x792580).
const NEXT_CORNER_DISTANCE: f32 = 20.;

/// Per-car inputs from the race manager (choreographer, ctx+0x38 =
/// drv+0x3150): corner-speed scale A (+0x3154) and floor B (+0x3158), the
/// backwards/yield flag (+0x316C), the engine-failure damage gate (+0x316D)
/// with the engine damage value (own+0x11C), the per-wheel failure flags
/// (car adapter +0x254) and a controlled-stop request.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pace {
    pub a: f32,
    pub b: f32,
    pub backwards: bool,
    pub damage: f32,
    pub damage_gate: bool,
    pub failures: [bool; 4],
    pub stop: bool,
}

impl Default for Pace {
    fn default() -> Self {
        Self {
            a: 1.,
            b: 0.,
            backwards: false,
            damage: 0.,
            damage_gate: false,
            failures: [false; 4],
            stop: false,
        }
    }
}

/// Mistake ids (enum 0xCF6F64).
pub const BRAKE_HEAVY: u8 = 0;
pub const BRAKE_LIGHT: u8 = 1;
pub const BRAKE_LATE: u8 = 2;
pub const BRAKE_EARLY: u8 = 3;
pub const BRAKE_SPIN: u8 = 4;
pub const POWEROUT_SPIN: u8 = 5;
pub const BRAKE_LOCK: u8 = 6;

/// Mistake slot drv+0x3100 (§2.1): armed (pending) type and timeout, active
/// type and timer, locked wheel, last brake-line id and the brake-line counter.
#[derive(Clone, Copy, Debug, Default)]
struct Mistake {
    pending: Option<(u8, f32)>,
    active: Option<(u8, f32)>,
    wheel: usize,
    line: Option<usize>,
    counter: u32,
}

/// Spin and other mistake durations (.rdata 0xCB1790 3 s, 0xCB093C 5 s), the
/// random arm timeout (0xCAFEC4 60 s), the cancel speed (0xCAFA58 10 m/s), the
/// brake_spin excess window (0xE4D0C0/0xE4D0BC) and the powerout throttle and
/// speed thresholds (0xE4D0C8/0xE4D0C4).
const MISTAKE_SPIN_TIME: f32 = 3.;
const MISTAKE_TIME: f32 = 5.;
const MISTAKE_ARM_TIMEOUT: f32 = 60.;
const MISTAKE_CANCEL_SPEED: f32 = 10.;
const BRAKE_SPIN_EXCESS: [f32; 2] = [8., 12.];
const POWEROUT_THROTTLE: f32 = 0.6;
const POWEROUT_SPEED: f32 = 10.;
/// lost_control: failed-wheel grip (0x776AB0), wobble rear grip, brake cap
/// and wobble period (0x77CF10).
const FAILED_WHEEL_GRIP: f32 = 0.1;
const WOBBLE_REAR_GRIP: f32 = 0.3;
const WOBBLE_BRAKE_CAP: f32 = 0.15;
const WOBBLE_PERIOD: f32 = 1.;
/// race_damaged: desired speed, throttle cap and side re-choice period (0x799FB0).
const DAMAGED_SPEED: f32 = 50.;
const DAMAGED_THROTTLE: f32 = 0.6;
const DAMAGED_PERIOD: f32 = 1.;

/// lost_control object (0x78B1F0): crash mode or the failed-wheel grip cut,
/// wobble timer and offset; the side/timer of race_damaged and retire.
#[derive(Clone, Copy, Default)]
struct Failure {
    crash: bool,
    wobble_timer: f32,
    wobble_offset: f32,
    side_timer: f32,
    side_offset: f32,
    removal: bool,
}

/// avoid test (0x79A960) memory and behaviour object (0x79C600): remembered
/// record, target id, side mode (0 follow, 1 negative, 2 positive), decision
/// timer, target speed and desired offset.
#[derive(Clone, Copy, Default)]
struct Avoid {
    remembered: Option<u32>,
    target: Option<u32>,
    side: u8,
    decision: f32,
    target_speed: f32,
    offset: f32,
}

/// recovery behaviour object (0x78EE70): mode, latched-after-stop, the
/// obstruction reverse toggle (kept across activations), elapsed time and
/// obstruction timer.
#[derive(Clone, Copy, Default)]
struct Recovery {
    mode: u8,
    latched: bool,
    toggle: bool,
    elapsed: f32,
    timer: f32,
}

/// Overtake behaviour object (+0x30 target, +0x34 state, +0x38 decision timer,
/// +0x3C minimum-fire timer).
#[derive(Clone, Copy, Default)]
struct Overtake {
    target: Option<u32>,
    state: u8,
    decision: f32,
    min_fire: f32,
}

/// Per-tick request fields edited by the behaviours.
struct Request {
    desired_offset: f32,
    merge: f32,
    desired_speed: f32,
    throttle_cap: f32,
    /// Followed car and follow distance (+0x70/+0x74); follow-brake cap +0x78.
    follow: Option<(Other, f32)>,
    follow_brake_cap: f32,
    /// Request flags +0xA4 brake to desired speed, +0xA5 force brake, +0xA6
    /// no steer, +0xB2 full brake, +0xAF reverse; limit types 3/4.
    brake_to_speed: bool,
    force_brake: bool,
    no_steer: bool,
    full_brake: bool,
    reverse: bool,
    track_limits: bool,
    /// Request grip multipliers +0xB8..+0xC4 in wheel order rear L/R, front
    /// L/R (0x776680), and the brake cap +0xA0.
    grip: [f32; 4],
    brake_cap: f32,
}

/// Car state seen by the AI.
pub struct View {
    /// 4x4 row-major pose: rows 0/1/2 = right-hand X / up / forward, 12..14 translation.
    pub pose: [f32; 16],
    pub velocity: [f32; 4],
    pub angular: [f32; 4],
    pub maximum_steering_angle: f32,
    /// World wheel positions (on_track test).
    pub wheels: [[f32; 3]; 4],
    /// Last tick's obstruction contact sum (collided_obstruction test);
    /// zero when no contact feed exists.
    pub obstruction: [f32; 3],
    /// The car's gearbox is in reverse (selects the pedal mapping of the
    /// AUTHORED reverse boundary).
    pub reverse_gear: bool,
    /// The car's AI traffic extents (own car state +0x110/+0x114).
    pub extents: Extents,
}

impl View {
    /// View of a source car with its obstruction contact sum.
    pub fn of(car: &crate::source_vehicle::Runtime, obstruction: [f32; 3]) -> Self {
        let pose = car.pose();
        let wheels = car.wheels().map(|w| {
            std::array::from_fn(|k| {
                pose[12 + k]
                    + w.anchor[0] * pose[k]
                    + w.anchor[1] * pose[4 + k]
                    + w.anchor[2] * pose[8 + k]
            })
        });
        Self {
            pose,
            velocity: car.velocity(),
            angular: car.angular_velocity(),
            maximum_steering_angle: car.maximum_steering_angle(),
            wheels,
            obstruction,
            reverse_gear: car.reverse_gear(),
            extents: car.ai_extents(),
        }
    }
}

/// Recovery octant 0x77C9B0 from the side-mirrored angles in degrees (§4.1).
fn octant(a: f32, b: f32) -> u8 {
    let within = |x: f32, lo: f32, hi: f32| lo < x && x < hi;
    const TABLE: [([f32; 2], [f32; 2]); 8] = [
        ([0., 90.], [0., 90.]),
        ([-90., 0.], [0., 90.]),
        ([-90., 0.], [-90., 0.]),
        ([-180., -90.], [-90., 0.]),
        ([-180., -90.], [-180., -90.]),
        ([90., 180.], [-180., -90.]),
        ([90., 180.], [90., 180.]),
        ([0., 90.], [90., 180.]),
    ];
    TABLE
        .iter()
        .position(|&([b0, b1], [a0, a1])| within(b, b0, b1) && within(a, a0, a1))
        .map_or(0, |i| i as u8 + 1)
}

#[derive(Clone, Copy)]
struct BrakeLineState {
    /// Brake line the car is currently braking for.
    active: Option<usize>,
    target: f32,
    modifier: f32,
    hold: f32,
    /// Last brake line passed while active: (id, target, hold, elapsed).
    passed: Option<(usize, f32, f32, f32)>,
    /// Request's brake line on the previous update.
    request: Option<usize>,
}

/// Diagnostics of the last update.
#[derive(Clone, Copy, Debug, Default)]
pub struct Telemetry {
    pub arc: f32,
    pub look_ahead: f32,
    pub curvature: f32,
    pub heading_error: f32,
    pub line_distance: f32,
    pub line_target: f32,
    pub governor_target: f32,
    pub offset: f32,
    pub behaviour: Behaviour,
    pub overtake_state: u8,
    pub following: bool,
    /// Signed distance of the car from the racing line (positive right).
    pub line_error: f32,
    pub slip: f32,
    /// Tests: spun, on_track, collided_obstruction (frontal), contacted
    /// vehicle type, avoid ETA (f32::MAX when not fired).
    pub spun: bool,
    pub on_track: bool,
    pub obstruction: bool,
    pub frontal: bool,
    pub contact: u8,
    pub avoid_eta: f32,
    /// avoid side mode, recovery mode and octant, reverse request.
    pub avoid_side: u8,
    pub recovery_mode: u8,
    pub octant: u8,
    pub reverse: bool,
    /// Signed lateral offset of our front from the racing line (0x40AE90).
    pub front_lateral: f32,
    /// Drift test fired, drift behaviour state, drift mode, drift line.
    pub drift_fired: bool,
    pub drift_state: u8,
    pub drift_mode: bool,
    pub drift_line: bool,
    /// Room to the left/right track limits at the look-ahead (drift drive).
    pub drift_room: [f32; 2],
}

#[derive(Clone)]
pub struct AiDriver {
    pub parameters: Parameters,
    seed: u32,
    random: u32,
    segment: Option<usize>,
    look_ahead: f32,
    curvature: f32,
    offset: f32,
    desired_offset: f32,
    offset_timer: f32,
    alone: bool,
    /// Lateral PID integral and previous error.
    pid_integral: f32,
    pid_error: Option<f32>,
    behaviour: Behaviour,
    overtake: Overtake,
    /// Grid revving (0x775BF0): (state, on phase, timer).
    rev: (u8, bool, f32),
    attempt_target: Option<u32>,
    alongside_fired: bool,
    avoid: Avoid,
    spun: bool,
    recovery: Recovery,
    /// recover_tbone timer (+0x34).
    tbone: f32,
    /// Retire stuck timer: reference position, time and latched flag (0x77E3B0).
    stuck: Option<([f32; 3], f32, bool)>,
    lines: BrakeLineState,
    /// Published (slewed) packet: throttle, brake, steering.
    throttle: f32,
    brake: f32,
    steering: f32,
    pub telemetry: Telemetry,
    /// Race-manager inputs, set before each update.
    pub pace: Pace,
    mistake: Mistake,
    failure: Failure,
    /// Request grip multipliers of the last update (read by the grip
    /// application 0x408710).
    grip: [f32; 4],
    /// Race place, set before each update (drift events).
    pub race: RaceView,
    drift: DriftState,
    /// The car follows the `drift` racing line.
    on_drift_line: bool,
}

fn clamp01(x: f32) -> f32 {
    x.clamp(0., 1.)
}
fn approach(current: f32, target: f32, up: f32, down: f32, dt: f32) -> f32 {
    if current <= target {
        (current + dt * up).min(target)
    } else {
        (current - dt * down).max(target)
    }
}
/// Linear remap of `x` over `[x0, x1]` onto `[y0, y1]`, clamped to the output range.
fn remap(x: f32, x0: f32, x1: f32, y0: f32, y1: f32) -> f32 {
    let t = if x1 == x0 { 0. } else { (x - x0) / (x1 - x0) };
    let y = y0 + (y1 - y0) * t;
    y.clamp(y0.min(y1), y0.max(y1))
}

impl AiDriver {
    pub fn new(parameters: Parameters, seed: u32) -> Self {
        let grip = [parameters.faults.racing_grip; 4];
        Self {
            look_ahead: parameters.la[0],
            parameters,
            seed,
            random: seed,
            segment: None,
            curvature: 0.,
            offset: 0.,
            desired_offset: 0.,
            offset_timer: 0.,
            alone: false,
            pid_integral: 0.,
            pid_error: None,
            behaviour: Behaviour::Race,
            overtake: Overtake::default(),
            rev: (0, false, 0.),
            attempt_target: None,
            alongside_fired: false,
            avoid: Avoid::default(),
            spun: false,
            recovery: Recovery::default(),
            tbone: -1.,
            stuck: None,
            lines: BrakeLineState {
                active: None,
                target: 0.,
                modifier: 1.,
                hold: -1.,
                passed: None,
                request: None,
            },
            throttle: 0.,
            brake: 0.,
            steering: 0.,
            telemetry: Telemetry::default(),
            pace: Pace::default(),
            mistake: Mistake::default(),
            failure: Failure::default(),
            grip,
            race: RaceView::default(),
            drift: DriftState::default(),
            on_drift_line: false,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new(self.parameters.clone(), self.seed);
    }

    /// Authored uniform random in [-1, 1] (original RNG stream not identified).
    fn uniform(&mut self) -> f32 {
        self.random = self
            .random
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        (self.random >> 8) as f32 / (1u32 << 24) as f32 * 2. - 1.
    }

    /// Throttle while held on the grid (0x775BF0): random on/off pulses.
    /// State 3: on 0.20..0.48 s, off 0.01..0.70 s; state 2: on 0.08..0.18 s,
    /// off 0.60..1.15 s (.rdata constants); other states give 0. Which start
    /// state applies when is not recovered; the random stream is authored.
    pub fn grid_rev(&mut self, state: u8, dt: f32) -> f32 {
        let ranges = match state {
            3 => [[0.2, 0.48], [0.01, 0.7]],
            2 => [[0.08, 0.18], [0.6, 1.15]],
            _ => {
                self.rev = (state, false, 0.);
                return 0.;
            }
        };
        let between =
            |driver: &mut Self, [a, b]: [f32; 2]| a + (b - a) * (driver.uniform() * 0.5 + 0.5);
        if self.rev.0 != state {
            let timer = between(self, ranges[0]);
            self.rev = (state, true, timer);
        }
        self.rev.2 -= dt;
        if self.rev.2 > 0. {
            return if self.rev.1 { 1. } else { 0. };
        }
        let (next_on, range) = if self.rev.1 {
            (false, ranges[1])
        } else {
            (true, ranges[0])
        };
        let timer = between(self, range);
        self.rev = (state, next_on, timer);
        0.
    }

    /// Look-ahead distance update (0x779A20) from speed and the previous curvature.
    fn update_look_ahead(&mut self, speed: f32) {
        let [
            dist_min,
            dist_max,
            angle_min,
            angle_max,
            speed_min,
            speed_max,
            step_max,
            proportion,
            extra_dist,
            extra_speed,
        ] = self.parameters.la;
        let target = if speed_min <= speed {
            let a = clamp01(if angle_max == angle_min {
                0.
            } else {
                (self.curvature.abs() - angle_max) / (angle_min - angle_max)
            });
            let s = clamp01(if speed_max == speed_min {
                0.
            } else {
                (speed - speed_min) / (speed_max - speed_min)
            });
            (dist_min + s * a * a * (dist_max - dist_min))
                .clamp(dist_min.min(dist_max), dist_min.max(dist_max))
        } else {
            let t = if extra_speed == speed_min {
                0.
            } else {
                (speed - extra_speed) / (speed_min - extra_speed)
            };
            (extra_dist + (dist_min - extra_dist) * t)
                .clamp(dist_min.min(extra_dist), dist_min.max(extra_dist))
        };
        let step = ((target - self.look_ahead).abs() * proportion).clamp(0., step_max);
        self.look_ahead = if target > self.look_ahead {
            (self.look_ahead + step).min(target)
        } else {
            (self.look_ahead - step).max(target)
        };
        self.look_ahead = self.look_ahead.clamp(dist_min.min(extra_dist), dist_max);
    }

    /// One AI update; `others` are the positions of the other cars.
    pub fn controls(&mut self, data: &AiData, view: &View, others: &[Other], dt: f32) -> Controls {
        let p = self.parameters.clone();
        let position = [view.pose[12], view.pose[13], view.pose[14]];
        let right = [view.pose[0], view.pose[1], view.pose[2]];
        let up = [view.pose[4], view.pose[5], view.pose[6]];
        let forward = [view.pose[8], view.pose[9], view.pose[10]];
        let v = view.velocity;
        let speed = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        let slip = if speed > 0.5 {
            wrap(heading([v[0], 0., v[2]]) - heading(forward)).abs()
        } else {
            0.
        };

        // Line choice (0x794870): with the drift behaviour switched on the
        // car follows the `drift` line once past the start line (before it
        // the `start` kind, absent on our routes, falls back to `optimal`).
        let drift_line = match (&p.drift, &data.drift) {
            (Some(_), Some(d)) if self.race.progress >= 0. => Some(d.as_ref()),
            _ => None,
        };
        if drift_line.is_some() != self.on_drift_line {
            // Keep the planned path in place across the change [I]: shift
            // the offsets by the two lines' lateral distance at the car.
            let new = drift_line.unwrap_or(data);
            let old = if self.on_drift_line {
                data.drift.as_deref().unwrap_or(data)
            } else {
                data
            };
            let (_, _, from) = old.line.project(position, self.segment);
            let (_, _, to) = new.line.project(position, self.segment);
            self.offset += to - from;
            self.desired_offset += to - from;
            self.on_drift_line = drift_line.is_some();
        }
        let data = drift_line.unwrap_or(data);

        // Track view: progress along the racing line (0x79C360).
        let (segment, arc, line_distance) = data.line.project(position, self.segment);
        self.segment = Some(segment);
        let (_, here, _, limits_here) = data.line.at(arc);
        let tr = p.traffic.clone();
        let own = Other::from_pose(0, view.pose, view.velocity, view.extents);
        let f = horizontal(forward);
        // Car-right in the original's sense is our -X (car left is +X).
        let right_vec = [-right[0], -right[1], -right[2]];
        let (own_front, own_rear) = (own.front(), own.rear());

        // brake_for_vehicle (0x78F6F0) on the nearest car in our path. AUTHORED
        // path scan: a straight corridor along our heading inside the view cone.
        let mut pass_vehicle = None;
        let mut brake_follow: Option<(Other, f32)> = None;
        let cone = (tr.view_angle.to_radians())
            .clamp(0., std::f32::consts::PI)
            .cos();
        if let Some(r) = others
            .iter()
            .filter(|r| {
                let d = sub(r.position, position);
                let along = dot3(d, f);
                let lat = dot3(d, right_vec);
                let to_rear = sub(r.rear(), own_rear);
                let visible = dot3(horizontal(to_rear), f) >= cone;
                along > 0.
                    && along < 200.
                    && lat.abs() < own.half_width() + r.half_width() + tr.buffer + 0.5
                    && visible
            })
            .min_by(|a, b| {
                dot3(sub(a.position, position), f).total_cmp(&dot3(sub(b.position, position), f))
            })
        {
            let w = own.half_width() + r.half_width();
            let lat = dot3(sub(r.position, position), right_vec);
            let (gap_r, gap_l) = (lat - w, -lat - w);
            let same = dot3(f, horizontal(r.forward));
            let closing = speed - r.speed();
            if same > tr.pass_angle.cos().abs() && (gap_r > tr.buffer || gap_l > tr.buffer) {
                let allowed = remap(
                    gap_r.max(gap_l) - tr.buffer,
                    0.,
                    tr.pass_spare_room,
                    tr.pass_side_speed[0],
                    tr.pass_side_speed[1],
                );
                let side_speed = dot3(
                    std::array::from_fn(|k| r.velocity[k] - view.velocity[k]),
                    right_vec,
                )
                .abs();
                if r.speed() < tr.pass_slow_speed
                    || side_speed < remap(closing, 0., tr.pass_relative_speed, allowed, 0.)
                {
                    pass_vehicle = Some(*r);
                }
            }
            if pass_vehicle.is_none() {
                let similar = same > tr.similar_angle.cos().abs();
                let dist = if similar {
                    dot3(sub(r.rear(), own_front), f)
                } else {
                    (dot3(sub(r.position, position), sub(r.position, position))).sqrt()
                        - own.half_length()
                        - r.half_length()
                };
                let sep = remap(
                    closing,
                    0.,
                    tr.separation_speed,
                    tr.separation[0],
                    tr.separation[1],
                );
                let sep2 = remap(speed, 0., tr.slow_speed, 0., sep);
                let available = (dist.abs() - sep2).max(0.);
                let multiplier = remap(
                    speed,
                    0.,
                    tr.slow_speed,
                    1.,
                    tr.extra_prediction + tr.prediction_ratio,
                );
                let safe = closing <= 0.
                    || available / closing * speed
                        > (speed / p.brake_on_speed.max(1e-3) * tr.pedal_push
                            + data.braking_distance(
                                TABLE_GRIP,
                                speed,
                                (speed - closing).clamp(0., speed),
                            ))
                            * multiplier;
                if !safe {
                    let distance = if similar && closing < tr.cruise_speed {
                        sep2
                    } else {
                        100.
                    };
                    brake_follow = Some((*r, distance));
                }
            }
        }

        // attempt_overtake (0x77DB90): nearest qualifying car ahead.
        let mut best = tr.ov_reject;
        let mut attempt = None;
        for r in others {
            let d = sub(r.rear(), own_front);
            let along = dot3(d, f);
            let lat = dot3(d, horizontal(r.left)).abs();
            let closing = speed - r.speed();
            if !(0. < along && along < best && lat < tr.ov_side_reject) {
                continue;
            }
            let ok = if Some(r.id) == self.attempt_target {
                closing > tr.ov_continue
            } else {
                along < tr.ov_auto_accept
                    || (closing > tr.ov_speed_difference && along / closing < tr.ov_time)
                    || (closing <= tr.ov_speed_difference
                        && along < tr.ov_slow_reject
                        && r.speed() < tr.ov_slow_speed)
            };
            if ok {
                best = along;
                attempt = Some(*r);
            }
        }
        self.attempt_target = attempt.map(|r| r.id);

        // alongside (0x795C00): overlapping cars within asRejectDistance.
        let margin = usize::from(self.alongside_fired);
        let (front_m, rear_m) = (tr.front_projection[margin], tr.rear_projection[margin]);
        let mut slots: [Option<(f32, bool, Other)>; 2] = [None, None];
        let mut fired = false;
        for r in others {
            if dot3(sub(r.position, position), sub(r.position, position)).sqrt() > tr.as_reject {
                continue;
            }
            let front_point: [f32; 3] = std::array::from_fn(|k| own_front[k] + f[k] * front_m);
            let rear_point: [f32; 3] = std::array::from_fn(|k| own_rear[k] - f[k] * rear_m);
            let beside = dot3(sub(r.rear(), front_point), f) < 0.
                && dot3(sub(r.front(), rear_point), f) > 0.;
            if !beside {
                continue;
            }
            fired = true;
            let strict =
                dot3(sub(r.rear(), own_front), f) < 0. && dot3(sub(r.front(), own_rear), f) > 0.;
            let lat = dot3(sub(r.rear(), own_front), horizontal(right_vec));
            let gap = (lat.abs() - own.half_width() - r.half_width()).max(0.);
            let slot = usize::from(lat > 0.);
            let replace = match slots[slot] {
                None => true,
                Some((g, s, _)) => (strict || !s) && gap < g,
            };
            if replace {
                slots[slot] = Some((gap, strict, *r));
            }
        }
        self.alongside_fired = fired;

        // Track view at the start of the tick (0x798E90, previous planned
        // offset and look-ahead): planned look-ahead point (+0xD0), racing-line
        // point and lateral at the car (+0x170/+0x180) and the lateral position
        // of our front (0x40AE90).
        let rs = p.rescue.clone();
        let (here_point, _, here_lateral, _) = data.line.at(arc);
        let (la_point, _, la_lateral, _) = data.line.at(arc + self.look_ahead);
        let planned: [f32; 3] = std::array::from_fn(|k| la_point[k] + la_lateral[k] * self.offset);
        // Angles from our forward, positive clockwise seen from above (toward
        // the car's right). The original's sign convention is not confirmed;
        // this one makes the recovery octants reverse away from the near edge [I].
        let clockwise = |d: [f32; 3]| wrap(heading(forward) - heading(d));
        let view_heading = clockwise(sub(planned, own_front));
        let own_lateral = dot2(sub(own_front, here_point), here_lateral);

        // spun (0x77E5B0) with its hysteresis.
        let moving_back = dot3([v[0], v[1], v[2]], forward) < 0.;
        let held_slip = if moving_back {
            std::f32::consts::PI - slip
        } else {
            slip
        };
        self.spun = view_heading.abs() > std::f32::consts::FRAC_PI_2
            || (self.spun
                && (held_slip.abs() > SPUN_HOLD_SLIP
                    || view_heading.abs() > SPUN_HOLD_HEADING
                    || speed < SPUN_HOLD_SPEED));
        // on_track (0x79B250): more than half the wheels inside the track limits.
        let inside = view
            .wheels
            .iter()
            .filter(|&&w| {
                let (_, s, lat) = data.line.project(w, Some(segment));
                let [left_limit, right_limit] = data.line.track_limits_at(s);
                left_limit <= lat && lat <= right_limit
            })
            .count();
        let on_track = 2 * inside > view.wheels.len();
        // collided_obstruction (0x776FE0): frontal when the summed normal is
        // more than 120 degrees from our nose.
        let o = view.obstruction;
        let o_length = dot3(o, o).sqrt();
        let obstruction = o_length > 0.;
        let o_direction = if o_length < 1e-5 {
            o
        } else {
            o.map(|x| x / o_length)
        };
        let frontal = obstruction && dot3(forward, o_direction) < -0.5;
        // contacted_vehicle (0x784650): first car whose box touches ours.
        let contact = others
            .iter()
            .find_map(|r| contact_type(&own, r, rs.contact_zone).map(|t| (*r, t)));
        // recover_tbone score (0x78ACC0): front contact at a T-bone, head-on or
        // slow nose-to-tail angle holds the score for 1 s.
        if let Some((r, kind)) = contact
            && matches!(kind, 1 | 2 | 5)
        {
            let theta = dot3(f, horizontal(r.forward))
                .clamp(-1., 1.)
                .acos()
                .to_degrees();
            let relative: [f32; 3] = std::array::from_fn(|k| v[k] - r.velocity[k]);
            let relative = dot3(relative, relative).sqrt();
            if (70. < theta && theta < 110. && relative < 5.)
                || (theta < 30. && relative < 5. && r.speed() < 5.)
                || theta > 160.
            {
                self.tbone = TBONE_TIME;
            }
        }
        // recovery score (0x78AB20).
        let n = usize::from(self.spun)
            + usize::from(!on_track)
            + usize::from(obstruction && !on_track)
            + usize::from(frontal);
        let recovery_score = if n == 0 { 0 } else { RECOVERY_SCORES[n - 1] };
        // retire stuck timer (0x77E3B0): 20 s within 5 m and no racing car
        // within 100 m.
        let moved = self.stuck.is_none_or(|(reference, _, _)| {
            let d = sub(position, reference);
            dot3(d, d) >= rs.rt_stuck_distance_sq
        });
        if moved {
            self.stuck = Some((position, 0., false));
        } else if let Some((_, time, latched)) = &mut self.stuck {
            *time += dt;
            *latched = *time > rs.rt_stuck_time
                && !others.iter().any(|o| {
                    let d = sub(o.position, position);
                    !o.backwards() && dot3(d, d) < rs.rt_player_distance_sq
                });
        }
        let stuck = self.stuck.is_some_and(|s| s.2);
        let faults = self.fault_scores(speed);

        // Path obstacle (0x78B560; AUTHORED corridor as brake_for_vehicle):
        // the car ahead we close on with the smallest time to reach it, our
        // box extended by the speed-scaled buffer zone.
        let buffer = clamp01(speed / rs.av_buffer_speed) * rs.av_buffer;
        let mut obstacle: Option<(Other, f32, f32)> = None;
        let mut best_time = f32::MAX;
        for r in others {
            let d = sub(r.position, position);
            let along = dot3(d, f);
            let lat = dot3(d, right_vec);
            let relative = [v[0] - r.velocity[0], 0., v[2] - r.velocity[2]];
            if along <= 0.
                || lat.abs() >= own.half_width() + r.half_width() + tr.buffer + 0.5
                || dot3(relative, d) <= 0.
            {
                continue;
            }
            let extent = dot2(horizontal(r.forward), f).abs() * r.half_length()
                + dot2(horizontal(r.left), f).abs() * r.half_width();
            let distance = (along - own.extents.front - extent - buffer).max(0.);
            let closing = dot3(relative, relative).sqrt();
            let time = distance / closing.max(1e-6);
            if time < best_time {
                best_time = time;
                obstacle = Some((*r, distance, closing));
            }
        }
        // avoid test (0x79A960): (target, ETA, closing speed).
        let mut avoid_fire: Option<(Other, f32, f32)> = None;
        if let Some((r, distance, closing)) = obstacle {
            // Racing-normally exclusion 0x79A530; the original's angle test
            // compares with the reversed track direction (§2.2).
            let (_, r_arc, _) = data.line.project(r.position, None);
            let (_, r_direction, _, _) = data.line.at(r_arc);
            let wrong_way = r_direction.map(|x| -x);
            let racing = r.speed() >= rs.av_min_racing_speed
                && !r.backwards()
                && dot2(f, horizontal(r.forward)) >= 0.
                && dot2(horizontal(r.forward), wrong_way).clamp(-1., 1.).acos()
                    > rs.av_max_racing_angle;
            let inert = closing <= rs.av_min_speed_normal
                && (rs.braking_buffer <= distance || closing >= rs.av_min_speed_slow);
            if !racing && !inert {
                if distance / closing < rs.av_eta {
                    avoid_fire = Some((r, distance / closing, closing));
                    self.avoid.remembered = Some(r.id);
                } else {
                    self.avoid.remembered = None;
                }
            }
        } else if let Some(r) = self
            .avoid
            .remembered
            .and_then(|id| others.iter().find(|o| o.id == id))
        {
            let d = [r.position[0] - position[0], 0., r.position[2] - position[2]];
            let along = dot3(d, f);
            let relative = [v[0] - r.velocity[0], 0., v[2] - r.velocity[2]];
            let closing = dot3(relative, relative).sqrt();
            if along > own.half_length() + r.half_length()
                && cone < dot3(f, horizontal(d))
                && along < rs.av_eta * closing
            {
                avoid_fire = Some((*r, along.abs() / closing, closing));
            } else {
                self.avoid.remembered = None;
            }
        } else {
            self.avoid.remembered = None;
        }

        // cornering (0x77ED00) and drift (0x78FE90) tests; drift score 0x78B360.
        let cornering = self.curvature.abs() > p.cornering_curvature;
        let drift_score = match &p.drift {
            Some(_) => {
                self.drift_test(data, speed, dt);
                let mut score = 0;
                if self.drift.fired {
                    score = DRIFT_SCORES[if cornering { 1 } else { 0 }];
                }
                if self.drift.state != 1 {
                    score = DRIFT_SCORES[2];
                }
                score
            }
            None => 0,
        };

        // Arbitration (0x79A400): the first strictly greatest score wins.
        let overtake_target = pass_vehicle.or(attempt);
        let scores = [
            (
                Behaviour::Retire,
                if stuck { RETIRE_STUCK_SCORE } else { 0 },
            ),
            (
                Behaviour::RecoverTbone,
                if self.tbone > 0. {
                    RECOVER_TBONE_SCORE
                } else {
                    0
                },
            ),
            (Behaviour::LostControl, faults[0]),
            (
                Behaviour::ControlledStop,
                if p.controlled_stop { faults[1] } else { 0 },
            ),
            (Behaviour::Recovery, recovery_score),
            (
                Behaviour::Avoid,
                if avoid_fire.is_some() { AVOID_SCORE } else { 0 },
            ),
            (Behaviour::Drift, drift_score),
            (Behaviour::RetireFailure, faults[2]),
            (
                Behaviour::RaceDamaged,
                if p.race_damaged { faults[3] } else { 0 },
            ),
            (Behaviour::Alongside, if fired { 8 } else { 0 }),
            (
                Behaviour::Overtake,
                if overtake_target.is_some() || self.overtake.min_fire > 0. {
                    5
                } else {
                    0
                },
            ),
            (Behaviour::Race, 1),
        ];
        let next = scores
            .iter()
            .fold(
                (Behaviour::Race, 0),
                |best, &s| if s.1 > best.1 { s } else { best },
            )
            .0;
        if next != self.behaviour {
            match self.behaviour {
                Behaviour::Overtake => self.overtake.min_fire = 0.,
                Behaviour::Recovery => self.recovery.elapsed = 0.,
                _ => {}
            }
            match next {
                Behaviour::Overtake => {
                    self.overtake = Overtake {
                        min_fire: tr.ov_min_fire,
                        ..Overtake::default()
                    }
                }
                // Race reactivation: a fresh random offset at the next update [I].
                Behaviour::Race => self.offset_timer = p.ra_generate_time,
                Behaviour::Avoid => {
                    self.avoid = Avoid {
                        remembered: self.avoid.remembered,
                        ..Avoid::default()
                    }
                }
                Behaviour::Recovery => {
                    self.recovery = Recovery {
                        toggle: self.recovery.toggle,
                        timer: -1.,
                        ..Recovery::default()
                    }
                }
                // Activation runs after scoring: the first tick does nothing.
                Behaviour::RecoverTbone => self.tbone = -1.,
                Behaviour::LostControl | Behaviour::RaceDamaged | Behaviour::RetireFailure => {
                    self.activate_failure(next, speed)
                }
                _ => {}
            }
            self.behaviour = next;
        }
        let mut octant_index = 0;

        // Common setup (0x789700) and the race behaviour's offsets.
        let nearest = others
            .iter()
            .map(|o| dot3(sub(o.position, position), sub(o.position, position)).sqrt())
            .fold(f32::INFINITY, f32::min);
        if self.alone && nearest < p.ra_alone_off {
            self.alone = false;
        } else if !self.alone && nearest > p.ra_alone_on {
            self.alone = true;
        }
        // Common setup 0x7978D0: request reset, merge tgOffsetMergeFactor.
        let mut request = Request {
            desired_offset: self.offset,
            merge: rs.tg_merge,
            desired_speed: f32::MAX,
            throttle_cap: 1.,
            follow: brake_follow,
            follow_brake_cap: 1.,
            brake_to_speed: false,
            force_brake: false,
            no_steer: false,
            full_brake: false,
            reverse: false,
            track_limits: false,
            grip: [p.faults.racing_grip; 4],
            brake_cap: 1.,
        };
        let corner =
            clamp01((self.curvature.abs() - tr.ov_corner[0]) / (tr.ov_corner[1] - tr.ov_corner[0]));
        // Request +0xC8: drift mode (set by the drift drive).
        let mut drift_mode = false;
        let mut drift_room = [0f32; 2];
        match self.behaviour {
            Behaviour::Drift => {
                let ([left, right], length) = (
                    data.line.track_limits_at(arc + self.look_ahead),
                    view.extents.front - view.extents.rear,
                );
                drift_room = [self.offset - left, right - self.offset];
                drift_mode = self.drive_drift(
                    data,
                    &mut request,
                    arc,
                    speed,
                    slip,
                    cornering,
                    // Room from the planned path at the look-ahead sample to
                    // the left/right track limits (view +0x218/+0x21C).
                    [self.offset - left, right - self.offset],
                    [view.extents.half_width, length],
                );
            }
            Behaviour::Race => {
                self.offset_timer += dt;
                if p.ra_generate && self.offset_timer >= p.ra_generate_time {
                    self.offset_timer = 0.;
                    let deviation = if self.alone {
                        p.deviation_small
                    } else {
                        p.deviation_large
                    };
                    self.desired_offset = self.uniform() * deviation;
                }
                request.desired_offset = self.desired_offset;
                request.merge = if self.alone {
                    p.ra_merge_alone
                } else if self.curvature.abs() > p.cornering_curvature {
                    p.ra_merge_cornering
                } else {
                    p.ra_merge
                };
                request.throttle_cap = p.throttle_confidence;
                if self.let_pass(others, own_rear, f) {
                    request.throttle_cap = p.throttle_confidence * p.faults.let_pass_throttle;
                }
            }
            Behaviour::Overtake => {
                // Drive 0x78A330: decide a side every ovDecisionTimeOut or on a new target.
                match overtake_target {
                    None => self.overtake.state = 5,
                    Some(target) => {
                        self.overtake.decision += dt;
                        if self.overtake.decision > tr.ov_decision
                            || self.overtake.target != Some(target.id)
                        {
                            self.overtake.target = Some(target.id);
                            self.overtake.decision = 0.;
                            // Side choice 0x77BD20 relative to our planned line.
                            let (_, target_arc, target_lat) =
                                data.line.project(target.position, None);
                            let _ = target_arc;
                            let s = target_lat - self.offset;
                            let dist = s.abs();
                            let w = own.half_width() + target.half_width();
                            let room_l = self.offset - limits_here[0] + s;
                            let room_r = limits_here[1] - self.offset - s;
                            let need = w + tr.ov_safe;
                            self.overtake.state = if dist > w + tr.ov_desired {
                                3
                            } else if dist > w + tr.ov_safe {
                                if s > 0. { 1 } else { 2 }
                            } else if self.overtake.state == 1 && room_l > need {
                                1
                            } else if self.overtake.state == 2 && room_r > need {
                                2
                            } else {
                                match (room_l > need, room_r > need) {
                                    (true, true) if s > own.half_width() => 1,
                                    (true, true) if s < -own.half_width() => 2,
                                    (true, true) => {
                                        if room_l > room_r {
                                            1
                                        } else {
                                            2
                                        }
                                    }
                                    (true, false) => 1,
                                    (false, true) => 2,
                                    (false, false) => 4,
                                }
                            };
                        }
                        // Execute 0x77C180.
                        match self.overtake.state {
                            1 | 2 => {
                                let toward_negative = self.overtake.state == 1;
                                request.desired_offset = self.offset
                                    + if toward_negative { -dt } else { dt } * tr.ov_merge;
                                // Positive curvature is a left-hand bend here [I]:
                                // moving left widens a right-hand bend.
                                let outside = if toward_negative {
                                    self.curvature < -tr.ov_corner[0]
                                } else {
                                    self.curvature > tr.ov_corner[0]
                                };
                                let c = if speed > tr.ov_slow_speed && outside {
                                    corner
                                } else {
                                    0.
                                };
                                request.merge = (tr.ov_merge
                                    + (tr.ov_merge_corner - tr.ov_merge) * c)
                                    .clamp(tr.ov_merge_corner, tr.ov_merge);
                            }
                            4 => {
                                request.desired_speed = target.speed();
                                if request.follow.is_none() {
                                    request.follow = Some((target, tr.ov_follow));
                                }
                            }
                            _ => {}
                        }
                    }
                }
                self.overtake.min_fire -= dt;
            }
            Behaviour::Alongside => {
                // Drive 0x799630: keep min space from a car on one side.
                request.merge = tr.as_merge;
                match slots {
                    [Some((g, _, _)), None] | [None, Some((g, _, _))] => {
                        // Slot 0: car on our left (negative offset side).
                        let away = if slots[0].is_some() { 1. } else { -1. };
                        let mut desired = self.offset;
                        if g < tr.min_space {
                            desired += away * (tr.min_space - g);
                        }
                        if desired * -away > 0. && g > tr.max_space {
                            desired = 0.;
                        }
                        request.desired_offset = desired;
                    }
                    _ => request.desired_offset = self.offset,
                }
                // Yield 0x789780 / 0x77B7D0: follow a car beside us that is mostly
                // ahead when we cannot move away from it.
                let [
                    distance,
                    max_brake,
                    min_diff,
                    max_diff,
                    follow,
                    respect,
                    min_speed,
                    to_limit,
                ] = tr.yield_;
                if distance > 0. && speed > min_speed {
                    let yields = |slot: &Option<(f32, bool, Other)>| {
                        slot.filter(|&(g, _, r)| {
                            let diff = r.speed() - speed;
                            let respect_point: [f32; 3] = std::array::from_fn(|k| {
                                r.rear()[k] + (r.front()[k] - r.rear()[k]) * respect
                            });
                            g < distance
                                && min_diff < diff
                                && diff < max_diff
                                && dot3(sub(respect_point, own_front), f) > 0.
                        })
                    };
                    let gap = |slot: &Option<(f32, bool, Other)>| slot.map_or(f32::MAX, |s| s.0);
                    let (left, right) = (yields(&slots[0]), yields(&slots[1]));
                    let room_l = self.offset - limits_here[0] - own.half_width();
                    let room_r = limits_here[1] - self.offset - own.half_width();
                    let chosen = match (left, right) {
                        (Some(a), Some(b)) => Some(if a.0 <= b.0 { a } else { b }),
                        (Some(a), None) if distance > gap(&slots[1]) || room_r < to_limit => {
                            Some(a)
                        }
                        (None, Some(b)) if distance > gap(&slots[0]) || room_l < to_limit => {
                            Some(b)
                        }
                        _ => None,
                    };
                    if let Some((_, _, r)) = chosen {
                        request.follow = Some((r, follow));
                        request.follow_brake_cap = max_brake;
                    }
                }
            }
            Behaviour::Avoid => {
                // Drive 0x79C600: decide every avDecisionTime or on a new target.
                self.avoid.decision -= dt;
                if let Some((r, eta, closing)) = avoid_fire
                    && (self.avoid.decision < 0. || self.avoid.target != Some(r.id))
                {
                    self.avoid.target = Some(r.id);
                    // Side choice 0x799B30 and offset 0x799DE0 around the
                    // target's predicted position.
                    let clearance =
                        r.half_length() + rs.av_buffer + own.half_width() + r.half_width();
                    let predicted: [f32; 3] =
                        std::array::from_fn(|k| r.position[k] + eta * r.velocity[k]);
                    let (_, p_arc, lat) = data.line.project(predicted, None);
                    let x = own_lateral;
                    self.avoid.side = if closing > speed {
                        1
                    } else {
                        let mut side = if (lat + clearance - x).abs() <= (lat - clearance - x).abs()
                        {
                            2
                        } else {
                            1
                        };
                        // Room check 0x797720 against the chosen track limit.
                        let w = 2. * own.half_width() + r.half_width() + 0.5;
                        let [left_limit, right_limit] = data.line.track_limits_at(p_arc);
                        let limit = if side == 1 { left_limit } else { right_limit };
                        if (lat - limit).abs() < w {
                            if rs.av_alongside && !fired {
                                side = 3 - side;
                            } else {
                                let gap = |slot: usize| slots[slot].map_or(f32::MAX, |s| s.0);
                                side = match side {
                                    1 if w < gap(1) => 2,
                                    2 if w < gap(0) => 1,
                                    _ => 0,
                                };
                            }
                        }
                        side
                    };
                    match self.avoid.side {
                        0 => request.follow = Some((r, AVOID_FOLLOW_DISTANCE)),
                        // Never move toward the target from where we are.
                        1 => self.avoid.offset = (lat - clearance).min(x),
                        _ => self.avoid.offset = (lat + clearance).max(x),
                    }
                    // Target speed 0x77B5A0, only on decision ticks.
                    self.avoid.target_speed = if r.speed() < rs.av_stopped_speed {
                        rs.av_stopped_target
                    } else if closing <= speed {
                        rs.av_normal_target
                    } else if !rs.av_figure_of_eight {
                        rs.av_wrong_way_target
                    } else {
                        0.
                    };
                    if self.avoid.target_speed > 0. {
                        request.desired_speed = self.avoid.target_speed;
                        request.brake_to_speed = true;
                    }
                    self.avoid.decision = rs.av_decision;
                }
                request.track_limits = true;
                request.desired_offset = self.avoid.offset;
                request.merge = rs.av_merge;
            }
            Behaviour::Recovery => {
                // Drive 0x78EE70. Offset 0x78ABB0: off track the planned line
                // drifts toward our side at rcMergeFactor; on track it moves to us.
                if on_track {
                    request.desired_offset = own_lateral;
                } else {
                    let step = if own_lateral <= 0. { -dt } else { dt };
                    request.desired_offset = self.offset + step * rs.rc_merge;
                    request.merge = rs.av_merge;
                }
                // Octant 0x77C9B0, mirrored when we are left of the line.
                let mirror = if own_lateral <= 0. { -1. } else { 1. };
                let b = (view_heading * mirror).to_degrees();
                let a = (clockwise(sub(planned, here_point)) * mirror).to_degrees();
                octant_index = octant(a, b);
                let rc = &mut self.recovery;
                if !rc.latched {
                    rc.mode = if octant_index <= 3 { 2 } else { 1 };
                }
                if rc.mode == 1 {
                    request.full_brake = true;
                    if speed < RECOVERY_STOPPED_SPEED {
                        rc.latched = true;
                        rc.mode = match octant_index {
                            0..=2 => 2,
                            3..=5 => 3,
                            _ => 5,
                        };
                    }
                }
                match rc.mode {
                    3 | 5 => {
                        let done = if rc.mode == 3 {
                            a.abs() < 5. || b.abs() < 5.
                        } else {
                            (80. < a.abs() && a.abs() < 90.) || b.abs() < 5.
                        };
                        request.throttle_cap = 1.;
                        if done {
                            request.desired_speed = if rc.mode == 3 { f32::MAX } else { 0. };
                            rc.mode = 2;
                        } else {
                            request.reverse = true;
                            request.desired_speed = RECOVERY_REVERSE_SPEED;
                        }
                    }
                    2 => request.reverse = false,
                    _ => {}
                }
                // Obstruction contacts toggle reverse/forward for rcCollisionTimeOut.
                if obstruction && rc.timer < 0. {
                    rc.timer = rs.rc_collision_timeout;
                    rc.toggle = !rc.toggle;
                    request.reverse = rc.toggle;
                } else if rc.timer > 0. {
                    request.reverse = rc.toggle;
                    rc.timer -= dt;
                }
                // Reset after rcRecoveryTimeOut is disabled by default (rcResetAllowed).
                rc.elapsed += dt;
            }
            Behaviour::RecoverTbone => {
                // Drive 0x776A00: back straight out.
                if self.tbone > 0. {
                    request.reverse = true;
                    request.no_steer = true;
                    self.tbone -= dt;
                }
            }
            Behaviour::Retire => {
                // Drive 0x79C6E0 for a stuck car: stop and hold.
                request.force_brake = true;
                request.no_steer = true;
            }
            Behaviour::LostControl
            | Behaviour::ControlledStop
            | Behaviour::RaceDamaged
            | Behaviour::RetireFailure => {
                self.drive_failure(data, &mut request, arc, speed, own.half_width(), dt);
            }
        }
        // Planned offset moves toward the request at its merge rate.
        if speed < SLOW_SPEED {
            self.offset = line_distance;
        } else {
            self.offset = approach(
                self.offset,
                request.desired_offset,
                request.merge,
                request.merge,
                dt,
            );
        }
        let followed = request.follow.map(|(r, distance)| {
            let gap = dot3(sub(r.rear(), own_front), sub(r.rear(), own_front)).sqrt() - distance;
            let along = dot3(sub(r.rear(), own_front), f) - distance;
            (gap, along, speed - r.speed())
        });

        // Look-ahead, target point and curvature (0x779A20, 0x798E90).
        self.update_look_ahead(speed);
        let (point, ahead, lateral, limits) = data.line.at(arc + self.look_ahead);
        // Limit types 1/2 (racing limits), or 3/4 (track limits) for avoid.
        let limits = if request.track_limits {
            data.line.track_limits_at(arc + self.look_ahead)
        } else {
            limits
        };
        self.offset = self.offset.clamp(limits[0], limits[1]);
        self.curvature = wrap(heading(ahead) - heading(here)) / self.look_ahead.max(1e-3);
        // Lateral PID (0x7892E0): error of the car from its desired line offset.
        let error = line_distance - self.offset;
        let [kp, ki, kd, lp, li, ld, lo] = p.pid;
        let proportional = (kp * error).clamp(-lp, lp);
        if ki != 0. {
            self.pid_integral =
                (self.pid_integral + error * dt).clamp(-li / ki.abs(), li / ki.abs());
        }
        let integral = (ki * self.pid_integral).clamp(-li, li);
        let derivative = self
            .pid_error
            .map_or(0., |e| (kd * (error - e) / dt.max(1e-6)).clamp(-ld, ld));
        self.pid_error = Some(error);
        let shift = (proportional + integral + derivative).clamp(-lo, lo);
        let target: [f32; 3] =
            std::array::from_fn(|k| point[k] + lateral[k] * (self.offset + shift));
        let to_target = sub(target, position);
        let heading_error = dot2(to_target, right).atan2(dot2(to_target, forward));

        // Next brake line ahead and its corner speed (0x792C80) with the
        // race manager's scale A and floor B.
        let (id, distance) = next_brake_line(data, arc);
        let line = data.brake_lines[id].1;
        // Off the optimal line the factor is tgNonOptimalSpeedFactor.
        let factor = if self.on_drift_line {
            p.tg_non_optimal_speed_factor
        } else {
            remap(
                self.offset.abs().max(line_distance.abs()),
                0.,
                p.tg_offset_max_for_speed,
                1.,
                p.tg_offset_speed_factor,
            )
        };
        let s = self.corner_speed_ratio(p.corner_speed_in_ratio);
        let line_target = (line.min_speed + (line.max_speed - line.min_speed) * factor * s).clamp(
            line.min_speed.min(line.max_speed),
            line.min_speed.max(line.max_speed),
        );
        // Drift mode: the line counts driftBrakeLineAdvance earlier and its
        // target is the drift speed (the maximum when that is 0); within the
        // advance of the line the request has no brake line.
        let (distance, line_target, line_valid) = match (&p.drift, drift_mode) {
            (Some(d), true) => (
                distance - d.brake_line_advance,
                if line.drift_speed != 0. {
                    line.drift_speed
                } else {
                    line.max_speed
                },
                distance - d.brake_line_advance > 0.,
            ),
            _ => (distance, line_target, true),
        };
        // Request +0x1C/+0x28 as the next drift test reads them; an early
        // return leaves the reset values 0 [I: provider kept valid].
        self.drift.line = Some(if line_valid {
            (line.drift_speed, distance)
        } else {
            (0., 0.)
        });

        // Mistake slot update (0x787620), after the behaviour drive.
        self.mistake_update(id, dt, &mut request.grip);

        // Brake-line state (0x7929D0 timer, 0x7886C0).
        if let Some(passed) = &mut self.lines.passed {
            passed.3 += dt;
        }
        let request_line = line_valid.then_some(id);
        if self.lines.request != request_line {
            if let Some(active) = self.lines.active {
                self.lines.passed = Some((active, self.lines.target, self.lines.hold, 0.));
            }
            self.lines.active = None;
        }
        self.lines.request = request_line;
        let mut line_brake = 0.;
        let previous_brake = self.brake;
        if line_valid {
            self.lines.target = line_target;
            self.lines.modifier = line.distance_modifier;
            self.lines.hold = line.hold_time;
        }
        if line_valid && speed <= line_target {
            if speed < line_target * BRAKE_LINE_CANCEL_FACTOR {
                self.lines.active = None;
            }
        } else if line_valid {
            let b = data.braking_distance(TABLE_GRIP, speed, line_target);
            let pedal = if p.brake_on_speed > 0. {
                speed / p.brake_on_speed
            } else {
                0.
            };
            let reaction = pedal * (1. - previous_brake);
            let mut d = if b <= reaction { b * 2. } else { reaction + b };
            d *= line.distance_modifier;
            // An armed late/early mistake scales the trigger distance and
            // activates when it is reached.
            let trigger = p.corner_brake_distance_factor * d;
            let trigger = match self.mistake.pending {
                Some((k @ (BRAKE_LATE | BRAKE_EARLY), _)) => trigger * p.faults.factors[k as usize],
                _ => trigger,
            };
            if distance <= trigger {
                if self.lines.active != Some(id) {
                    self.activate_mistake(&[BRAKE_LATE, BRAKE_EARLY]);
                }
                self.lines.active = Some(id);
            }
            if self.lines.active == Some(id) {
                let ratio = if b == d {
                    0.
                } else {
                    clamp01((distance - b) / (d - b))
                };
                line_brake = ratio.powf(p.brake_cruise_exponent);
                if line_brake > 0. {
                    self.activate_mistake(&[BRAKE_HEAVY, BRAKE_LIGHT, BRAKE_LOCK]);
                }
            }
        }

        // Brake (0x78E8A0): line brake, spin; cap 1. An active heavy/light
        // mistake is a floor/cap on the brake.
        let mut brake = match self.mistake.active {
            Some((BRAKE_HEAVY, _)) => line_brake.max(p.faults.factors[0]),
            Some((BRAKE_LIGHT, _)) => line_brake.min(p.faults.factors[1]),
            _ => line_brake,
        };
        // brake_spin activates while braking for a line with 8..12 m/s excess (0x77A110).
        if self.lines.active == Some(id) {
            let excess = speed - line_target;
            if BRAKE_SPIN_EXCESS[0] < excess && excess < BRAKE_SPIN_EXCESS[1] {
                self.activate_mistake(&[BRAKE_SPIN]);
            }
        }
        // Follow brake (0x77A1E0): inside the follow distance along our heading.
        if let Some((_, along, _)) = followed
            && along < 0.
            && speed > MINIMUM_MOVING_SPEED
        {
            brake = brake.max(request.follow_brake_cap.min(p.follow_brake));
        }
        if slip > std::f32::consts::FRAC_PI_2 && !request.reverse {
            brake = 1.;
        }
        if request.full_brake
            || request.force_brake
            || (request.brake_to_speed && speed - request.desired_speed > rs.target_speed_big)
        {
            brake = 1.;
        }
        brake = brake.min(1.).min(request.brake_cap);

        // Throttle (0x783180): governor and limiters; any previous brake -> 0.
        let mut governor_target = f32::MAX;
        let throttle = if previous_brake > 0. {
            0.
        } else {
            // Drift mode holds a passed line for thBrakeLineHoldTimeDrift.
            let hold_time = |hold: f32| match &p.drift {
                Some(d) if drift_mode => d.hold_time,
                _ => hold,
            };
            let within_hold = self
                .lines
                .passed
                .is_some_and(|(_, _, hold, t)| t < hold_time(hold));
            governor_target = governor_target.min(request.desired_speed);
            if within_hold {
                governor_target = governor_target.min(self.lines.passed.unwrap().1);
            } else if self.lines.active.is_some() {
                governor_target =
                    governor_target.min(self.lines.target - p.th_brake_line_speed_drop);
            }
            let e = speed - governor_target;
            let current = self.throttle;
            let governor = if e < -p.th_target_speed_big {
                1.
            } else {
                let goal = if e < -p.th_target_speed_small {
                    p.th_target_medium
                } else if e <= p.th_target_speed_small {
                    current
                } else if e <= p.th_target_speed_big {
                    p.th_target_small
                } else {
                    0.
                };
                let gap = (goal - current).abs();
                current + (dt * p.th_target_move_speed * (goal - current)).clamp(-gap, gap)
            };
            let slide = if speed > SLIDE_LIFT_MIN_SPEED {
                remap(slip, p.slide_lift[0], p.slide_lift[1], 1., p.slide_lift[2])
            } else {
                1.
            };
            let corner = remap(
                self.curvature.abs(),
                p.corner_lift[0],
                p.corner_lift[1],
                1.,
                p.th_corner_lift_min,
            );
            // Follow throttle (0x77A630).
            let [
                small,
                medium,
                dist_big,
                dist_medium,
                dist_small,
                speed_small,
                speed_medium,
            ] = p.follow;
            let follow = match followed {
                Some((gap, _, closing))
                    if gap <= dist_big
                        && closing >= -speed_medium
                        && speed >= MINIMUM_MOVING_SPEED =>
                {
                    if closing <= speed_small && gap > dist_medium {
                        medium
                    } else if closing <= speed_small && gap >= -dist_small {
                        if gap < 0. { small } else { current }
                    } else {
                        0.
                    }
                }
                _ => 1.,
            };
            let limited = governor
                .min(follow)
                .min(slide)
                .min(corner)
                .min(request.throttle_cap);
            // powerout_spin (0x77A450): armed, past the brake line, previous
            // throttle above 0.6 and moving; while active the throttle is full.
            if within_hold && self.throttle > POWEROUT_THROTTLE && speed > POWEROUT_SPEED {
                self.activate_mistake(&[POWEROUT_SPIN]);
            }
            match self.mistake.active {
                Some((POWEROUT_SPIN, _)) => limited.max(1.),
                _ => limited,
            }
        };

        // Steering (0x77AC50): heading gain plus yaw-rate damping, normalised by
        // the car's maximum steering angle. Written in the +E30 convention
        // (positive = left): the original negates both the heading error and
        // ω·up, and its publisher (0x996240) negates the command again, so
        // +ω·up here is the original's sign (docs/AI_DRIVER_PORT.md, F391).
        // Zero with no-steer; negated while reversing.
        let yaw = view.angular[0] * up[0] + view.angular[1] * up[1] + view.angular[2] * up[2];
        let steer = if request.no_steer {
            0.
        } else {
            // Drift mode damps the yaw rate with driftStAngVelFactor.
            let ang_vel_factor = match &p.drift {
                Some(d) if drift_mode => d.st_ang_vel_factor,
                _ => p.st_ang_vel_factor,
            };
            ((p.st_angle_factor * heading_error + ang_vel_factor * yaw)
                / view.maximum_steering_angle.max(1e-3))
            .clamp(-1., 1.)
        };
        let steer = if request.reverse { -steer } else { steer };
        // Reverse negates the throttle (0x783180); 0x7929D0 publishes
        // |throttle| with the reverse byte +0x1E.
        let throttle = if request.reverse { -throttle } else { throttle };
        let reverse = throttle < 0.;
        let throttle = throttle.abs();

        // Output slew (0x779F70).
        self.steering = approach(self.steering, steer, p.steering_speed, p.steering_speed, dt);
        self.throttle = approach(
            self.throttle,
            throttle,
            p.throttle_on_speed,
            p.throttle_off_speed,
            dt,
        );
        self.brake = approach(self.brake, brake, p.brake_on_speed, p.brake_off_speed, dt);
        // 0x794310 ends any active mistake at low speed.
        if speed <= MISTAKE_CANCEL_SPEED {
            self.mistake.active = None;
        }
        self.grip = request.grip;
        self.telemetry = Telemetry {
            arc,
            look_ahead: self.look_ahead,
            curvature: self.curvature,
            heading_error,
            line_distance: distance,
            line_target,
            governor_target,
            offset: self.offset,
            behaviour: self.behaviour,
            overtake_state: self.overtake.state,
            following: followed.is_some(),
            line_error: line_distance,
            slip,
            spun: self.spun,
            on_track,
            obstruction,
            frontal,
            contact: contact.map_or(0, |c| c.1),
            avoid_eta: avoid_fire.map_or(f32::MAX, |a| a.1),
            avoid_side: self.avoid.side,
            recovery_mode: self.recovery.mode,
            octant: octant_index,
            reverse,
            front_lateral: own_lateral,
            drift_fired: self.drift.fired,
            drift_state: self.drift.state,
            drift_mode,
            drift_line: self.on_drift_line,
            drift_room,
        };
        // AUTHORED reverse boundary: the car-side consumer of the reverse
        // byte is not traced (§5, §7.3). The assisted automatic gearbox
        // selects reverse when the brake pedal is held near rest and then
        // swaps the pedal roles, so the request is mapped onto the pedal that
        // drives the wanted direction in the current gear.
        let (throttle, brake) = match (view.reverse_gear, reverse) {
            (false, false) => (self.throttle, self.brake),
            (false, true) => (0., self.throttle.max(self.brake)),
            (true, true) => (self.brake, self.throttle),
            // In reverse gear the throttle pedal brakes a backward roll; at rest
            // a pure brake request must not select forward drive.
            (true, false) if moving_back => (self.throttle.max(self.brake), 0.),
            (true, false) => (self.throttle, 0.),
        };
        Controls {
            throttle,
            brake,
            steering: self.steering,
            handbrake: false,
        }
    }
}

/// The next brake line ahead of arc `arc`: (id, distance along the line).
fn next_brake_line(data: &AiData, arc: f32) -> (usize, f32) {
    data.brake_lines
        .iter()
        .enumerate()
        .map(|(i, &(s, _))| (i, (s - arc).rem_euclid(data.line.length)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .expect("brake lines")
}

/// brake_lock slot order: drawn index 0..3 -> request +0xC0, +0xC4, +0xB8,
/// +0xBC, i.e. wheels FL, FR, RL, RR in the rear-first wheel order (§2.4).
const LOCK_WHEEL: [usize; 4] = [2, 3, 0, 1];

/// Random mistake type (0x787620): the type whose factor is the smallest one
/// strictly greater than `r`; ties keep the lower id.
fn pick_mistake(factors: &[f32; 7], r: f32) -> Option<u8> {
    let mut pick: Option<(usize, f32)> = None;
    for (k, &w) in factors.iter().enumerate() {
        if w > r && pick.is_none_or(|(_, best)| w < best) {
            pick = Some((k, w));
        }
    }
    pick.map(|(k, _)| k as u8)
}

/// Mistakes, choreographed failures and race-manager inputs
/// (docs/AI_MISTAKES_PACING_RESEARCH.md §2, §4.3, §5).
impl AiDriver {
    /// Arm a mistake (0x779990 SetPending), as the choreographer's mistake
    /// event does; it activates at its own driving condition.
    pub fn arm_mistake(&mut self, kind: u8, timeout: f32) {
        self.mistake.pending = Some((kind, timeout));
    }
    /// Disarm (mistake event abort 0x4018D0).
    pub fn disarm_mistake(&mut self) {
        self.mistake.pending = None;
    }
    /// Armed and active mistake ids.
    pub fn mistake(&self) -> (Option<u8>, Option<u8>) {
        (
            self.mistake.pending.map(|m| m.0),
            self.mistake.active.map(|m| m.0),
        )
    }
    /// Request grip multipliers of the last update, rear L/R, front L/R.
    pub fn request_grip(&self) -> [f32; 4] {
        self.grip
    }
    /// The failure/damage retire drive has stopped the car (retire drive
    /// 0x79C6E0 below rtStoppedSpeed: removal request req+0x38 [I]).
    pub fn removal_requested(&self) -> bool {
        self.failure.removal
    }

    /// Uniform in [0, 1) from the authored driver stream.
    fn unit(&mut self) -> f32 {
        self.uniform() * 0.5 + 0.5
    }

    /// `S = max(A · corner_speed_in_ratio, B)` (0x792C80).
    fn corner_speed_ratio(&self, ratio: f32) -> f32 {
        let scaled = self.pace.a * ratio;
        if scaled > self.pace.b {
            scaled
        } else {
            self.pace.b
        }
    }

    /// Scores of lost_control (21), controlled_stop (20), the retire
    /// failure/damage branch (10) and race_damaged (9). The damage tests run
    /// only with the engine-failure gate drv+0x316D (§2.8). The spun-and-fast
    /// lost_control branch is not ported (coSpinSpeed's default is not recovered).
    fn fault_scores(&self, speed: f32) -> [u8; 4] {
        let f = &self.parameters.faults;
        let pace = &self.pace;
        let failed = pace.failures.iter().any(|&x| x);
        let lost = failed && speed > f.retirement_speed + 5.;
        let retire = failed || (pace.damage_gate && pace.damage > f.damage_retirement_ratio);
        let damaged = pace.damage_gate && pace.damage >= 0.5;
        [
            if lost { 21 } else { 0 },
            if pace.stop { 20 } else { 0 },
            if retire { 10 } else { 0 },
            if damaged { 9 } else { 0 },
        ]
    }

    /// Drift test 0x78FE90 ([`DriftState::test`]) and the refusal 0x78CB30
    /// on the previous update's request: no drift the car could not brake
    /// from to the next line's drift speed in time.
    /// AUTHORED: the draws use the driver's random stream (the original's
    /// RNG instance at test context +0x38 is not identified).
    fn drift_test(&mut self, data: &AiData, speed: f32, dt: f32) {
        let mut state = self.drift;
        let check = state.test(self.race, dt, |a, b| a + (b - a) * self.unit());
        self.drift = state;
        if check
            && let Some((drift_speed, distance)) = self.drift.line
            && drift_speed < speed
            && data.braking_distance(TABLE_GRIP, speed, drift_speed) > distance
        {
            self.drift.fired = false;
            self.drift.timer = DRIFT_RETRY;
        }
    }

    /// Drift behaviour: state update 0x78B3C0 and drive 0x783A10 (request
    /// edits; the shared race drive follows). Returns the drift-mode flag
    /// (+0xC8). `room` is the free room to the left/right track limits at
    /// the look-ahead sample, `size` the car's half width and length.
    #[allow(clippy::too_many_arguments)]
    fn drive_drift(
        &mut self,
        data: &AiData,
        request: &mut Request,
        arc: f32,
        speed: f32,
        slip: f32,
        cornering: bool,
        room: [f32; 2],
        size: [f32; 2],
    ) -> bool {
        let Some(d) = self.parameters.drift.clone() else {
            return false;
        };
        self.drift.state = d.next_state(self.drift.state, speed, slip, cornering);
        request.merge = 0.;
        match self.drift.state {
            1 => {
                // Toward the side of the next corner (view +0x84, 0x792580).
                let (id, _) = next_brake_line(data, arc);
                let at = data.brake_lines[id].0;
                let (_, here, _, _) = data.line.at(at);
                let (_, ahead, _, _) = data.line.at(at + NEXT_CORNER_DISTANCE);
                let next_corner = wrap(heading(ahead) - heading(here)) / NEXT_CORNER_DISTANCE;
                request.desired_offset = if next_corner < 0. {
                    -d.position
                } else {
                    d.position
                };
                request.merge = d.merge;
                request.grip = [d.racing_grip; 4];
                true
            }
            2 => {
                // Kick only once off the brakes [I: context +0x3C +4 = the
                // published brake].
                if self.brake <= 0. {
                    request.grip = [d.min_grip, d.min_grip, d.initiate_grip, d.initiate_grip];
                }
                true
            }
            3 => {
                // The outside of the bend: left room for a right-hand bend.
                let outside = if self.curvature >= 0. {
                    room[0]
                } else {
                    room[1]
                };
                request.grip = d.drifting_grip(slip.to_degrees(), outside, size);
                true
            }
            _ => false,
        }
    }

    /// Activation of lost_control (0x78B1F0: crash mode above crashMinSpeed,
    /// else the failed-wheel grip cut) and of the side choice of
    /// race_damaged/retire.
    fn activate_failure(&mut self, behaviour: Behaviour, speed: f32) {
        match behaviour {
            Behaviour::LostControl => {
                self.failure.crash = speed > self.parameters.faults.crash_min_speed;
                self.failure.wobble_timer = 0.;
            }
            _ => self.failure.side_timer = 0.,
        }
    }

    /// The nearer track limit, re-chosen every second (0x797F00 [I]): the
    /// planned offset keeps the car's half width plus 0.5 m inside it.
    fn limit_side(&mut self, data: &AiData, arc: f32, half_width: f32, dt: f32) -> f32 {
        self.failure.side_timer -= dt;
        if self.failure.side_timer <= 0. {
            self.failure.side_timer = DAMAGED_PERIOD;
            let [left, right] = data.line.track_limits_at(arc);
            let margin = half_width + 0.5;
            self.failure.side_offset = if (self.offset - left).abs() <= (right - self.offset).abs()
            {
                left + margin
            } else {
                right - margin
            };
        }
        self.failure.side_offset
    }

    /// Drives of lost_control (0x776B80), controlled_stop (0x78AA90),
    /// race_damaged (0x799FB0) and the retire failure branch (0x79A090).
    /// AUTHORED where not recovered: the crash PID, spin/flip angles and
    /// inertia factor are not ported (normal steering, no flip); the wobble
    /// amplitude is deviation_from_racing_line_large; the retire point is the
    /// nearer track limit, where the car stops.
    fn drive_failure(
        &mut self,
        data: &AiData,
        request: &mut Request,
        arc: f32,
        speed: f32,
        half_width: f32,
        dt: f32,
    ) {
        let f = self.parameters.faults.clone();
        match self.behaviour {
            Behaviour::LostControl if self.failure.crash => {
                request.desired_speed = 0.;
                request.full_brake = true;
                request.grip = [
                    f.crash_rear_grip,
                    f.crash_rear_grip,
                    f.crash_front_grip,
                    f.crash_front_grip,
                ];
            }
            Behaviour::LostControl => {
                request.desired_speed = f.retirement_speed;
                request.brake_to_speed = true;
                request.brake_cap = WOBBLE_BRAKE_CAP;
                self.failure.wobble_timer -= dt;
                if self.failure.wobble_timer <= 0. {
                    self.failure.wobble_timer = WOBBLE_PERIOD;
                    self.failure.wobble_offset = self.uniform() * self.parameters.deviation_large;
                }
                request.desired_offset = self.failure.wobble_offset;
                request.grip[0] = WOBBLE_REAR_GRIP;
                request.grip[1] = WOBBLE_REAR_GRIP;
            }
            Behaviour::ControlledStop => {
                request.force_brake = true;
                request.desired_offset = self.offset;
                request.no_steer = speed < MINIMUM_MOVING_SPEED;
            }
            Behaviour::RaceDamaged => {
                request.desired_speed = DAMAGED_SPEED;
                request.throttle_cap = DAMAGED_THROTTLE;
                request.merge = self.parameters.ra_merge;
                request.desired_offset = self.limit_side(data, arc, half_width, dt);
                request.track_limits = true;
            }
            Behaviour::RetireFailure => {
                let side = self.limit_side(data, arc, half_width, dt);
                request.desired_speed = f.retirement_speed;
                request.brake_to_speed = true;
                request.merge = self.parameters.ra_merge;
                request.desired_offset = side;
                request.track_limits = true;
                if (self.offset - side).abs() < 0.5 {
                    request.force_brake = true;
                    request.no_steer = speed < MINIMUM_MOVING_SPEED;
                    if speed < f.stopped_speed {
                        self.failure.removal = true;
                    }
                }
            }
            _ => {}
        }
    }

    /// Let-pass (race drive 0x77C2C0): while the car is dropping back or being
    /// lapped, a car whose front is behind our rear within coLetPassDistance.
    fn let_pass(&self, others: &[Other], own_rear: [f32; 3], forward: [f32; 3]) -> bool {
        let distance = self.parameters.faults.let_pass_distance;
        self.pace.backwards
            && others.iter().any(|r| {
                let d = sub(r.front(), own_rear);
                dot3(d, forward) < 0. && dot3(d, d) < distance * distance
            })
    }

    /// Activation 0x7799B0 of an armed mistake of one of `kinds`.
    fn activate_mistake(&mut self, kinds: &[u8]) {
        if self.mistake.active.is_some() {
            return;
        }
        let Some((kind, _)) = self.mistake.pending else {
            return;
        };
        if !kinds.contains(&kind) {
            return;
        }
        self.mistake.pending = None;
        let time = if kind == BRAKE_SPIN || kind == POWEROUT_SPIN {
            MISTAKE_SPIN_TIME
        } else {
            MISTAKE_TIME
        };
        self.mistake.active = Some((kind, time));
        if kind == BRAKE_LOCK {
            self.mistake.wheel = LOCK_WHEEL[((self.unit() * 4.) as usize).min(3)];
        }
    }

    /// Mistake slot update 0x787620: a new brake line clears active types
    /// 0-3/6 and may draw a random mistake (only with generateMistakes, false
    /// in shipped data); timers; then the grip overrides of the active type.
    /// AUTHORED: a failed wheel (blow_tyre) keeps grip 0.1, standing in for
    /// the vehicle's failed-wheel parameter, which the port does not model.
    fn mistake_update(&mut self, line: usize, dt: f32, grip: &mut [f32; 4]) {
        let f = self.parameters.faults.clone();
        if self.mistake.line != Some(line) {
            self.mistake.line = Some(line);
            if matches!(self.mistake.active, Some((k, _)) if k != BRAKE_SPIN && k != POWEROUT_SPIN)
            {
                self.mistake.active = None;
            }
            if f.generate && self.mistake.pending.is_none() {
                self.mistake.counter += 1;
                if self.mistake.counter > f.lines_before && self.unit() <= f.probability {
                    let r = self.unit();
                    if let Some(k) = pick_mistake(&f.factors, r) {
                        self.mistake.pending = Some((k, MISTAKE_ARM_TIMEOUT));
                    }
                }
            }
        }
        if let Some((_, t)) = &mut self.mistake.pending {
            *t -= dt;
            if *t <= 0. {
                self.mistake.pending = None;
            }
        }
        if let Some((_, t)) = &mut self.mistake.active {
            *t -= dt;
            if *t <= 0. {
                self.mistake.active = None;
            }
        }
        match self.mistake.active {
            Some((BRAKE_SPIN, _)) => *grip = [f.spin_rear_grip, f.spin_rear_grip, 1., 1.],
            Some((POWEROUT_SPIN, _)) => *grip = [f.power_rear_grip, f.power_rear_grip, 1., 1.],
            Some((BRAKE_LOCK, _)) => {
                *grip = [1.; 4];
                grip[self.mistake.wheel] = f.lock_grip;
            }
            _ => {}
        }
        for (g, &failed) in grip.iter_mut().zip(&self.pace.failures) {
            if failed {
                *g = g.min(FAILED_WHEEL_GRIP);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_rev_pulses_stay_within_shipped_ranges() {
        let Some(root) = std::env::var_os("GRID_GAME_ROOT") else {
            return;
        };
        let data = AiData::load(
            Path::new(&root),
            "tracks/europe/donington/route_0",
            "touring",
            "lac",
        )
        .unwrap();
        let mut driver = AiDriver::new(
            Parameters::new(&data, NORMAL_DIFFICULTY, |_| 1.).unwrap(),
            7,
        );
        let dt = 0.016;
        for (state, on, off) in [
            (3, [0.2, 0.48], [0.01, 0.7]),
            (2, [0.08, 0.18], [0.6, 1.15]),
        ] {
            let mut runs: Vec<(bool, f32)> = Vec::new();
            for _ in 0..4000 {
                let level = driver.grid_rev(state, dt) > 0.;
                match runs.last_mut() {
                    Some((l, n)) if *l == level => *n += dt,
                    _ => runs.push((level, dt)),
                }
            }
            // Skip the first and last (partial) runs.
            for &(level, length) in &runs[1..runs.len() - 1] {
                let [a, b] = if level { on } else { off };
                assert!(
                    length >= a - 2. * dt && length <= b + 2. * dt,
                    "{state} {level} {length}"
                );
            }
        }
        assert_eq!(driver.grid_rev(1, dt), 0.);
    }

    #[test]
    fn random_mistake_pick_follows_the_live_weights() {
        // Live Normal driver 0 (§2.2): heavy, light, late, early, spins, lock.
        let w = [0.600, 0.956, 0.441, 0.120, 1.0, 1.0, 0.0];
        let pick = |r| pick_mistake(&w, r);
        assert_eq!(pick(0.05), Some(BRAKE_EARLY));
        assert_eq!(pick(0.3), Some(BRAKE_LATE));
        assert_eq!(pick(0.5), Some(BRAKE_HEAVY));
        assert_eq!(pick(0.9), Some(BRAKE_LIGHT));
        // Ties keep the lower id: brake_spin before powerout_spin.
        assert_eq!(pick(0.97), Some(BRAKE_SPIN));
        assert_eq!(pick(1.0), None);
    }

    #[test]
    fn mistake_slot_windows_and_effects() {
        let Some(root) = std::env::var_os("GRID_GAME_ROOT") else {
            return;
        };
        let data = AiData::load(
            Path::new(&root),
            "tracks/europe/donington/route_0",
            "touring",
            "lac",
        )
        .unwrap();
        let mut d = AiDriver::new(
            Parameters::new(&data, NORMAL_DIFFICULTY, |_| 1.).unwrap(),
            7,
        );
        let f = d.parameters.faults.clone();
        assert!(!f.generate);
        assert_eq!(
            (f.spin_rear_grip, f.power_rear_grip, f.lock_grip),
            (0.2, 0.4, 0.2)
        );
        let mut grip = [1.; 4];
        d.mistake_update(0, 0.016, &mut grip);
        // Only the armed type activates, and only at its own condition.
        d.arm_mistake(BRAKE_SPIN, 60.);
        d.activate_mistake(&[BRAKE_LOCK]);
        assert_eq!(d.mistake(), (Some(BRAKE_SPIN), None));
        d.activate_mistake(&[BRAKE_SPIN]);
        assert_eq!(d.mistake(), (None, Some(BRAKE_SPIN)));
        // Spins: 3 s, survive a new brake line, rear grip 0.2.
        let mut grip = [1.; 4];
        d.mistake_update(1, 2.9, &mut grip);
        assert_eq!(grip, [0.2, 0.2, 1., 1.]);
        assert_eq!(d.mistake().1, Some(BRAKE_SPIN));
        d.mistake_update(1, 0.2, &mut grip);
        assert_eq!(d.mistake().1, None);
        // powerout_spin: rear grip 0.4.
        d.arm_mistake(POWEROUT_SPIN, 60.);
        d.activate_mistake(&[POWEROUT_SPIN]);
        let mut grip = [1.; 4];
        d.mistake_update(1, 0.016, &mut grip);
        assert_eq!(grip, [0.4, 0.4, 1., 1.]);
        d.mistake.active = None;
        // brake_lock: one wheel at 0.2 for 5 s, cleared by the next brake line.
        let mut line = 1;
        let mut wheels = [0; 4];
        for _ in 0..64 {
            d.arm_mistake(BRAKE_LOCK, 60.);
            d.activate_mistake(&[BRAKE_HEAVY, BRAKE_LIGHT, BRAKE_LOCK]);
            let mut grip = [1.; 4];
            d.mistake_update(line, 0.016, &mut grip);
            assert_eq!(grip.iter().filter(|&&g| g == 0.2).count(), 1);
            assert_eq!(grip[d.mistake.wheel], 0.2);
            wheels[d.mistake.wheel] += 1;
            line += 1;
            d.mistake_update(line, 0.016, &mut grip);
            assert_eq!(d.mistake().1, None);
        }
        assert!(wheels.iter().all(|&n| n > 0), "{wheels:?}");
        d.arm_mistake(BRAKE_LOCK, 60.);
        d.activate_mistake(&[BRAKE_LOCK]);
        d.mistake_update(line, 4.9, &mut [1.; 4]);
        assert_eq!(d.mistake().1, Some(BRAKE_LOCK));
        d.mistake_update(line, 0.2, &mut [1.; 4]);
        assert_eq!(d.mistake().1, None);
        // The arm timeout (60 s) disarms.
        d.arm_mistake(BRAKE_SPIN, 60.);
        d.mistake_update(line, 59.9, &mut [1.; 4]);
        assert_eq!(d.mistake().0, Some(BRAKE_SPIN));
        d.mistake_update(line, 0.2, &mut [1.; 4]);
        assert_eq!(d.mistake().0, None);
        // A failed wheel keeps grip 0.1; corner speed scale and floor.
        d.pace.failures[3] = true;
        let mut grip = [1.; 4];
        d.mistake_update(line, 0.016, &mut grip);
        assert_eq!(grip, [1., 1., 1., 0.1]);
        assert_eq!(d.corner_speed_ratio(0.6454), 0.6454);
        d.pace.a = 1.1;
        assert_eq!(d.corner_speed_ratio(0.5), 1.1 * 0.5);
        d.pace.b = 1.;
        assert_eq!(d.corner_speed_ratio(0.5), 1.);
        // Failure scores: lost_control above rtRetirementSpeed + 5, retire;
        // damage only behind the engine-failure gate.
        assert_eq!(d.fault_scores(25.), [21, 0, 10, 0]);
        assert_eq!(d.fault_scores(15.), [0, 0, 10, 0]);
        d.pace.failures = [false; 4];
        d.pace.damage = 1.;
        assert_eq!(d.fault_scores(25.), [0, 0, 0, 0]);
        d.pace.damage_gate = true;
        assert_eq!(d.fault_scores(25.), [0, 0, 10, 9]);
        d.pace.damage = 0.5;
        assert_eq!(d.fault_scores(25.), [0, 0, 0, 9]);
    }

    #[test]
    fn characteristic_table_rows_and_bounds() {
        let rows =
            read_characteristics("driver\taggression\tspeed_in\n00\t1\t1\n01\t0.7\t0.9\n").unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1]["aggression"], 0.7);
        assert_eq!(rows[1]["speed_in"], 0.9);
        assert!(read_characteristics("driver\taggression\n00\t1.5\n").is_err());
        assert!(read_characteristics("name\taggression\n00\t1\n").is_err());
        assert!(read_characteristics("driver\taggression\n00\t1\t2\n").is_err());
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT"]
    fn shipped_ai_data_reproduces_live_normal_values() {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let data = AiData::load(
            Path::new(&root),
            "tracks/europe/donington/route_0",
            "touring",
            "lac",
        )
        .unwrap();
        let p = Parameters::new(&data, NORMAL_DIFFICULTY, |_| 1.).unwrap();
        // Live Normal values of driver 0 (docs/AI_DRIVER_RESEARCH.md §7).
        assert!((p.throttle_confidence - 0.9606).abs() < 1e-4);
        assert!(
            (p.corner_speed_in_ratio - 0.6454).abs() < 1e-3,
            "{}",
            p.corner_speed_in_ratio
        );
        assert!(
            (p.corner_brake_distance_factor - 0.1182 - 1.).abs() < 1e-3,
            "{}",
            p.corner_brake_distance_factor
        );
        assert_eq!(p.steering_speed, 15.);
        assert_eq!(p.st_angle_factor, 2.7);
        assert!(
            (data.line.length - 3944.).abs() < 1.,
            "{}",
            data.line.length
        );
        assert_eq!(data.brake_lines.len(), 11);
        // Touring Redgate line: 28.8/46.0 m/s.
        assert_eq!(
            (
                data.brake_lines[0].1.min_speed,
                data.brake_lines[0].1.max_speed
            ),
            (28.8, 46.)
        );
        // Lacetti table: 70.28 m/s -> 132.48 m to stop at grip 2.0.
        assert!((data.braking_distance(2., 70.27765, 0.) - 132.48337).abs() < 1e-3);
        // Positive lateral offsets point to the driver's right (heading
        // decreases clockwise) at every gate; the octant mirror relies on it.
        // Gate normals are not exactly square to the line (up to ~0.25 rad).
        for i in 0..data.line.points.len() {
            let s = data.line.start[i] + 0.5 * data.line.segment(i);
            let (_, direction, lateral, _) = data.line.at(s);
            let turn = wrap(heading(lateral) - heading(direction));
            assert!(
                (turn + std::f32::consts::FRAC_PI_2).abs() < 0.5,
                "segment {i}: {turn}"
            );
            let [left, right] = data.line.track_limits_at(s);
            assert!(left < 0. && right > 0., "segment {i}: {left} {right}");
        }
    }

    #[test]
    fn recovery_octants_follow_the_table() {
        assert_eq!(octant(45., 45.), 1);
        assert_eq!(octant(45., -45.), 2);
        assert_eq!(octant(-45., -45.), 3);
        assert_eq!(octant(-45., -135.), 4);
        assert_eq!(octant(-135., -135.), 5);
        assert_eq!(octant(-135., 135.), 6);
        assert_eq!(octant(135., 135.), 7);
        assert_eq!(octant(135., 45.), 8);
        // Boundaries and the unlisted combinations give 0.
        assert_eq!(octant(90., 45.), 0);
        assert_eq!(octant(-45., 45.), 0);
    }

    #[test]
    fn extents_take_the_hull_offset_and_cover_the_tyres() {
        use grid_formats::car_collision::Bounds;
        let hull = Bounds {
            min: [-0.9, 0.1, -2.2],
            max: [0.92, 1.4, 2.25],
        };
        let mut vehicle = vec![0u32; 0x1640 / 4];
        let set = |vehicle: &mut [u32], o: usize, v: f32| vehicle[o / 4] = v.to_bits();
        set(&mut vehicle, 0x1638, -0.15);
        // Rear-right and front-left wheel centres, radii and tyre widths.
        for (o, v) in [(0x14F0, -0.76), (0x14F8, -1.2), (0x1500, 0.74), (0x1508, 1.35)] {
            set(&mut vehicle, o, v);
        }
        for o in [0x1524, 0x1528] {
            set(&mut vehicle, o, 0.3);
        }
        for o in [0x1544, 0x1548] {
            set(&mut vehicle, o, 0.27);
        }
        let e = Extents::from_vehicle(hull, &vehicle);
        assert_eq!(e.half_width, 0.92);
        assert_eq!(e.front, 2.25 + -0.15);
        assert_eq!(e.rear, -2.2 + -0.15);
        assert_eq!(e.half_length(), 0.5 * (e.front - e.rear));
        // A front tyre beyond the nose moves the front only: the rear test is
        // the else branch of the front test (0x403469).
        set(&mut vehicle, 0x1508, 2.0);
        set(&mut vehicle, 0x14F8, -2.3);
        let e = Extents::from_vehicle(hull, &vehicle);
        assert_eq!((e.front, e.rear), (2.0 + 0.3, -2.2 + -0.15));
        set(&mut vehicle, 0x1508, 1.35);
        let e = Extents::from_vehicle(hull, &vehicle);
        assert_eq!(e.rear, -2.3 - 0.3);
        // Tyre widths: the left (front-left) test first, then the right one.
        set(&mut vehicle, 0x14F0, -0.9);
        let e = Extents::from_vehicle(hull, &vehicle);
        assert_eq!(e.half_width, (-0.9f32 - 0.27 * 0.5).abs());
        set(&mut vehicle, 0x1500, 0.85);
        let e = Extents::from_vehicle(hull, &vehicle);
        assert_eq!(e.half_width, 0.85 + 0.27 * 0.5);
    }

    #[test]
    fn boxes_use_each_cars_own_asymmetric_extents() {
        let long = Extents {
            half_width: 0.95,
            front: 2.0,
            rear: -2.5,
        };
        let car = |position: [f32; 3], extents| Other {
            id: 1,
            position,
            velocity: [0.; 4],
            forward: [0., 0., 1.],
            left: [1., 0., 0.],
            extents,
        };
        let own = car([0.; 3], long);
        assert_eq!(own.front(), [0., 0., 2.]);
        assert_eq!(own.rear(), [0., 0., -2.5]);
        assert!(own.contains([0., 0., -2.45], 0.));
        assert!(!own.contains([0., 0., 2.05], 0.));
        assert!(own.contains([0.94, 0., 0.], 0.));
        assert!(!own.contains([0.96, 0., 0.], 0.));
        // A narrow car whose nose reaches 0.05 m into our rear face (type 6).
        let narrow = Extents {
            half_width: 0.5,
            front: 1.0,
            rear: -1.0,
        };
        assert_eq!(
            contact_type(&own, &car([0., 0., -3.45], narrow), 0.),
            Some(6)
        );
        assert_eq!(contact_type(&own, &car([0., 0., -3.55], narrow), 0.), None);
    }

    #[test]
    fn contact_types_classify_corners_and_faces() {
        let car = |position: [f32; 3], forward: [f32; 3]| Other {
            id: 1,
            position,
            velocity: [0.; 4],
            forward,
            // Car-left (+X for a car facing +Z): forward rotated by +90 degrees.
            left: [forward[2], 0., -forward[0]],
            extents: Extents {
                half_width: 0.9,
                front: 2.24,
                rear: -2.24,
            },
        };
        let own = car([0.; 3], [0., 0., 1.]);
        // Other car's rear overlapping our nose: our front corners are inside.
        assert!(matches!(
            contact_type(&own, &car([0., 0., 4.5], [0., 0., 1.]), 0.1),
            Some(1 | 2)
        ));
        // Nose of a car from our right (-X) into our side: right face.
        assert_eq!(
            contact_type(&own, &car([-2.5, 0., 0.], [1., 0., 0.]), 0.1),
            Some(7)
        );
        // Same from our left: left face.
        assert_eq!(
            contact_type(&own, &car([2.5, 0., 0.], [-1., 0., 0.]), 0.1),
            Some(8)
        );
        assert_eq!(
            contact_type(&own, &car([10., 0., 0.], [0., 0., 1.]), 0.1),
            None
        );
    }

    /// ai_vehicle_config `default` drift values (the touring class has no
    /// overrides) and built-in defaults; drift_skill 1, drift_position 0.5.
    fn default_drift() -> Drift {
        Drift {
            st_ang_vel_factor: -0.3,
            angle_range: [15., 30.],
            angle_max: 60.,
            min_grip: 0.5,
            max_grip: 3.,
            start_angle: 15.,
            end_angle: 10.,
            merge: 1.,
            speed_min: 15.,
            grip_front: 10.,
            initiate_grip: 1.2,
            racing_grip: 1.1,
            brake_line_advance: 8.,
            hold_time: 0.2,
            skill: 1.,
            position: 0.5,
        }
    }

    fn race(place: u32, progress: f32) -> RaceView {
        RaceView {
            place,
            cars: 8,
            progress,
        }
    }

    #[test]
    fn drift_test_windows_follow_the_race_place() {
        // Draws are recorded and return the window's first bound.
        let draws = std::cell::RefCell::new(Vec::new());
        let draw = |a: f32, b: f32| {
            draws.borrow_mut().push((a, b));
            a
        };
        // Before the start: one first window, leader 10..20 s, last 2..8 s.
        let mut leader = DriftState::default();
        assert!(!leader.test(race(0, 0.), 0.1, &draw));
        assert!(!leader.test(race(0, -0.01), 0.1, &draw));
        let mut last = DriftState::default();
        assert!(!last.test(race(7, 0.), 0.1, &draw));
        assert_eq!(*draws.borrow(), [(10., 20.), (2., 8.)]);
        assert!(leader.before_start && !leader.fired);
        // Racing: the test fires once the first window runs out.
        draws.borrow_mut().clear();
        let mut t = 0.;
        while !leader.test(race(0, 0.5), 0.5, &draw) {
            t += 0.5;
        }
        assert_eq!(t, 10.);
        // Fired window for the leader 48..24 s; the leader never stops.
        assert_eq!(*draws.borrow(), [(48., 24.)]);
        for _ in 0..200 {
            assert!(leader.test(race(0, 0.6), 0.5, &draw));
        }
        assert!(leader.fired);
        // The last car: 16..8 s, then a 30..15 s pause when it gained nothing.
        draws.borrow_mut().clear();
        while !last.test(race(7, 0.5), 0.5, &draw) {}
        assert_eq!(*draws.borrow(), [(16., 8.)]);
        let mut fired = 0;
        while last.test(race(7, 0.5), 0.5, &draw) {
            fired += 1;
        }
        assert_eq!(fired, 31);
        assert_eq!(*draws.borrow(), [(16., 8.), (30., 15.)]);
        // An odd place waits for the pause; an even place fires again at once
        // after its first drift.
        assert!(!last.test(race(7, 0.5), 0.5, &draw));
        assert!(last.test(race(6, 0.5), 0.5, &draw));
        // A car that gained places keeps the test fired past its window.
        let mut gaining = DriftState {
            fired: true,
            place: 5,
            timer: 0.1,
            ..DriftState::default()
        };
        assert!(gaining.test(race(3, 1.), 0.5, &draw));
        assert!(!gaining.test(race(5, 1.), 0.5, &draw));
    }

    #[test]
    fn drift_states_follow_angle_speed_and_corner() {
        let d = default_drift();
        let deg = |a: f32| a.to_radians();
        // State 0 (construction) acts as 1; below driftSpeedMin it is 1.
        assert_eq!(d.next_state(0, 20., 0., false), 1);
        assert_eq!(d.next_state(3, 14.9, deg(40.), true), 1);
        // Initiation needs a corner below 50 m/s.
        assert_eq!(d.next_state(1, 30., 0., true), 2);
        assert_eq!(d.next_state(1, 50., 0., true), 1);
        assert_eq!(d.next_state(1, 30., 0., false), 1);
        // Drifting past driftStartAngle (15 degrees), ending below
        // driftEndAngle (10 degrees) or out of the corner.
        assert_eq!(d.next_state(2, 30., deg(14.), true), 2);
        assert_eq!(d.next_state(2, 30., deg(16.), true), 3);
        assert_eq!(d.next_state(3, 30., deg(12.), true), 3);
        assert_eq!(d.next_state(3, 30., deg(9.), true), 4);
        assert_eq!(d.next_state(3, 30., deg(30.), false), 4);
        assert_eq!(d.next_state(4, 30., deg(30.), true), 1);
    }

    #[test]
    fn drifting_grip_holds_the_angle_range_and_the_wall() {
        let d = default_drift();
        let car = [0.9, 4.5];
        let open = 20.;
        // Rear L/R then front L/R; the front gets driftGripFront 10.
        assert_eq!(d.drifting_grip(10., open, car), [0.5, 0.5, 10., 10.]);
        assert_eq!(d.drifting_grip(20., open, car), [1., 1., 10., 10.]);
        assert_eq!(d.drifting_grip(30., open, car)[0], 1.);
        // Above the range toward driftAngleMax: 1 -> driftMaxGrip 3.
        assert!((d.drifting_grip(45., open, car)[0] - 2.).abs() < 1e-5);
        assert_eq!(d.drifting_grip(70., open, car)[0], 3.);
        // A wall 2.4 m away caps the range at asin(1.5 / 5.4) = 16.1 degrees.
        let near = d.drifting_grip(20., 2.4, car)[0];
        let expected = 1. + 2. * (20. - 16.128) / (60. - 16.128);
        assert!((near - expected).abs() < 1e-3, "{near} {expected}");
        // Drift skill scales the bottom of the range.
        let unskilled = Drift {
            skill: 0.5,
            ..default_drift()
        };
        assert_eq!(unskilled.drifting_grip(10., open, car)[0], 1.);
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT"]
    fn drift_battle_config_switches_on_the_drift_behaviour() {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let root = Path::new(&root);
        for route in [
            "tracks/usa/long_beach/route_1",
            "tracks/europe/nurburgring/route_1",
            "tracks/europe/istanbul_park/route_1",
            "tracks/europe/jarama/route_0",
            "tracks/europe/jarama/route_1",
        ] {
            let race = AiData::load(root, route, "touring", "bmw").unwrap();
            let drift = AiData::load_for(root, route, "touring", "bmw", Some("drift_battle")).unwrap();
            let p = Parameters::new(&race, NORMAL_DIFFICULTY, |_| 1.).unwrap();
            let q = Parameters::new(&drift, NORMAL_DIFFICULTY, |_| 1.).unwrap();
            // A race keeps the defaults: no drift behaviour or line.
            assert!(race.drift.is_none() && p.drift.is_none());
            assert!(p.race_damaged && p.controlled_stop);
            assert_eq!(p.la[0], 10.);
            // drift_battle: drift behaviour and test on, race_damaged and
            // controlled_stop off, laDistMin 15, the drift line [D].
            let d = q.drift.clone().unwrap();
            assert!(!q.race_damaged && !q.controlled_stop);
            assert_eq!(q.la[0], 15.);
            assert_eq!(d.hold_time, 0.2);
            assert_eq!(
                (d.angle_range, d.grip_front, d.start_angle, d.end_angle),
                ([15., 30.], 10., 15., 10.)
            );
            assert_eq!((d.initiate_grip, d.racing_grip, d.brake_line_advance), (1.2, 1.1, 8.));
            assert_eq!((d.skill, d.position), (1., 0.5));
            assert_eq!(q.tg_non_optimal_speed_factor, 0.8);
            let line = &drift.drift.as_ref().unwrap().line;
            assert_eq!(line.points.len(), drift.line.points.len());
            assert_ne!(line.points, drift.line.points, "{route}");
            assert!((line.length - drift.line.length).abs() < 0.05 * drift.line.length);
            // Mapping overrides of the section (deviation large at
            // difficulty 0.606 and characteristic 1: 3 + (1 - 3) * 0.606).
            assert!((q.deviation_large - (3. - 2. * NORMAL_DIFFICULTY)).abs() < 1e-5);
        }
    }
}
