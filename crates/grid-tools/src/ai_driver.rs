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
//! Following uses the recovered follow throttle (0x77A630) and follow brake
//! (0x77A1E0) with the shipped thFollow* values; the choice of the followed car
//! and the follow-brake amount are authored (the setting behaviour is not
//! recovered). A stuck car reverses briefly: AUTHORED stand-in for the
//! unrecovered recovery behaviour.
//!
//! Selected scope, not recovered or not yet ported: corner-speed scale A/B
//! (A=1, B=0), alongside/overtake/avoidance behaviours,
//! traction control, AI ABS cap, start-line reaction, choreographed mistakes,
//! catch-up. The random offset generator is an authored LCG (the original RNG
//! stream is not identified). Characteristic values default to 1.0 (the live
//! player-slot set); the difficulty default is the live Normal value 0.606.
use crate::source_vehicle::Controls;
use grid_formats::{ai, binxml};
use std::{collections::BTreeMap, path::Path};

/// Live Normal difficulty observed in the original (all 12 driver objects).
pub const NORMAL_DIFFICULTY: f32 = 0.606;

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

    /// Arc length of the racing-line point nearest to `p` (global search).
    pub fn arc(&self, p: [f32; 3]) -> f32 {
        self.project(p, None).1
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
}

impl AiData {
    /// `route` is the track route folder, e.g. `tracks/europe/donington/route_0`.
    pub fn load(
        root: &Path,
        route: &str,
        class: &str,
        car: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let bytes = read_doc(&root.join(route).join("ai_track.xml"))?;
        let track = ai::track(&binxml::read(&bytes)?)?;
        let bytes = read_doc(&root.join(route).join("ai_vehicle_track.xml"))?;
        let class_lines = ai::class_brake_lines(&binxml::read(&bytes)?, class)?;
        let line = Line::new(&track)?;
        let mut brake_lines = Vec::new();
        for (id, b) in track.brake_lines.iter().enumerate() {
            let data = match &class_lines {
                Some(lines) => *lines.get(id).ok_or("class brake line absent")?,
                None => b.data,
            };
            brake_lines.push((line.start[line.gate_index[b.gate]], data));
        }
        let bytes = read_doc(&root.join("ai/ai_vehicle_statistics.xml"))?;
        let brake_tables = ai::statistics(&binxml::read(&bytes)?, car)?.brake;
        let bytes = read_doc(&root.join("ai/ai_driver_config.xml"))?;
        let driver = ai::config(
            &binxml::read(&bytes)?,
            "ai_driver_config",
            &[("default", None)],
        )?;
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
        let mappings = ai::mappings(&binxml::read(&bytes)?, "default")?;
        Ok(Self {
            line,
            brake_lines,
            brake_tables,
            driver,
            vehicle,
            mappings,
        })
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
    follow_distance: f32,
    follow_brake: f32,
    deviation_small: f32,
    deviation_large: f32,
}

/// Built-in defaults for values absent from the shipped XML
/// (docs/AI_DRIVER_RESEARCH.md §4).
const BRAKE_LINE_CANCEL_FACTOR: f32 = 0.8;
const SLOW_SPEED: f32 = 3.;
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
            follow_distance: m("follow_distance")?,
            follow_brake: d("brFollowSpeedBrake")?,
            deviation_small: m("deviation_from_racing_line_small")?,
            deviation_large: m("deviation_from_racing_line_large")?,
        })
    }
}

/// Another car seen by the AI.
#[derive(Clone, Copy)]
pub struct Other {
    pub position: [f32; 3],
    pub velocity: [f32; 4],
}

/// Car state seen by the AI.
pub struct View {
    /// 4x4 row-major pose: rows 0/1/2 = right-hand X / up / forward, 12..14 translation.
    pub pose: [f32; 16],
    pub velocity: [f32; 4],
    pub angular: [f32; 4],
    pub maximum_steering_angle: f32,
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
    /// Signed distance of the car from the racing line (positive right).
    pub line_error: f32,
    pub slip: f32,
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
    /// Authored stuck recovery: (time slow, reverse time left).
    stuck: f32,
    reversing: f32,
    lines: BrakeLineState,
    /// Published (slewed) packet: throttle, brake, steering.
    throttle: f32,
    brake: f32,
    steering: f32,
    pub telemetry: Telemetry,
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
            stuck: 0.,
            reversing: 0.,
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

        // Followed car (AUTHORED choice): nearest car ahead within a car width
        // of our path and the follow range.
        let followed = others
            .iter()
            .filter_map(|o| {
                let d = sub(o.position, position);
                let ahead = d[0] * forward[0] + d[1] * forward[1] + d[2] * forward[2];
                let side = d[0] * right[0] + d[1] * right[1] + d[2] * right[2];
                (ahead > 0. && side.abs() < 2.2 && ahead < p.follow_distance + p.follow[2] + 10.)
                    .then_some((ahead, o))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, o)| {
                let d = sub(o.position, position);
                let w = o.velocity;
                (
                    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() - p.follow_distance,
                    d[0] * forward[0] + d[1] * forward[1] + d[2] * forward[2] - p.follow_distance,
                    speed - (w[0] * w[0] + w[1] * w[1] + w[2] * w[2]).sqrt(),
                )
            });

        // Track view: progress along the racing line (0x79C360).
        let (segment, arc, line_distance) = data.line.project(position, self.segment);
        self.segment = Some(segment);
        let (_, here, _, _) = data.line.at(arc);

        // Race behaviour: alone test and random lateral offsets.
        let nearest = others
            .iter()
            .map(|o| {
                let d = sub(o.position, position);
                (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
            })
            .fold(f32::INFINITY, f32::min);
        if self.alone && nearest < p.ra_alone_off {
            self.alone = false;
        } else if !self.alone && nearest > p.ra_alone_on {
            self.alone = true;
        }
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
        if speed < SLOW_SPEED {
            self.offset = line_distance;
        } else {
            let rate = if self.alone {
                p.ra_merge_alone
            } else if self.curvature.abs() > p.cornering_curvature {
                p.ra_merge_cornering
            } else {
                p.ra_merge
            };
            self.offset = approach(self.offset, self.desired_offset, rate, rate, dt);
        }

        // Look-ahead, target point and curvature (0x779A20, 0x798E90).
        self.update_look_ahead(speed);
        let (point, ahead, lateral, limits) = data.line.at(arc + self.look_ahead);
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

        // Next brake line ahead and its corner speed (0x792C80, A=1 B=0).
        let (id, distance) = data
            .brake_lines
            .iter()
            .enumerate()
            .map(|(i, &(s, _))| (i, (s - arc).rem_euclid(data.line.length)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .expect("brake lines");
        let line = data.brake_lines[id].1;
        let factor = remap(
            self.offset.abs().max(line_distance.abs()),
            0.,
            p.tg_offset_max_for_speed,
            1.,
            p.tg_offset_speed_factor,
        );
        let s = p.corner_speed_in_ratio;
        let line_target = (line.min_speed + (line.max_speed - line.min_speed) * factor * s).clamp(
            line.min_speed.min(line.max_speed),
            line.min_speed.max(line.max_speed),
        );

        // Brake-line state (0x7929D0 timer, 0x7886C0).
        if let Some(passed) = &mut self.lines.passed {
            passed.3 += dt;
        }
        if self.lines.request != Some(id) {
            if let Some(active) = self.lines.active {
                self.lines.passed = Some((active, self.lines.target, self.lines.hold, 0.));
            }
            self.lines.active = None;
        }
        self.lines.request = Some(id);
        let mut line_brake = 0.;
        let previous_brake = self.brake;
        self.lines.target = line_target;
        self.lines.modifier = line.distance_modifier;
        self.lines.hold = line.hold_time;
        if speed <= line_target {
            if speed < line_target * BRAKE_LINE_CANCEL_FACTOR {
                self.lines.active = None;
            }
        } else {
            let b = data.braking_distance(TABLE_GRIP, speed, line_target);
            let pedal = if p.brake_on_speed > 0. {
                speed / p.brake_on_speed
            } else {
                0.
            };
            let reaction = pedal * (1. - previous_brake);
            let mut d = if b <= reaction { b * 2. } else { reaction + b };
            d *= line.distance_modifier;
            if distance <= p.corner_brake_distance_factor * d {
                self.lines.active = Some(id);
            }
            if self.lines.active == Some(id) {
                let ratio = if b == d {
                    0.
                } else {
                    clamp01((distance - b) / (d - b))
                };
                line_brake = ratio.powf(p.brake_cruise_exponent);
            }
        }

        // Brake (0x78E8A0): line brake, spin; cap 1.
        let mut brake = line_brake;
        // Follow brake (0x77A1E0): inside the follow distance along our heading.
        if let Some((_, along, closing)) = followed
            && along < 0.
            && speed > SLOW_SPEED
        {
            brake = brake.max((closing / p.follow[6]).clamp(0., p.follow_brake));
        }
        if slip > std::f32::consts::FRAC_PI_2 {
            brake = 1.;
        }
        brake = brake.min(1.);

        // Throttle (0x783180): governor and limiters; any previous brake -> 0.
        let mut governor_target = f32::MAX;
        let throttle = if previous_brake > 0. {
            0.
        } else {
            let within_hold = self.lines.passed.is_some_and(|(_, _, hold, t)| t < hold);
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
                    if gap <= dist_big && closing >= -speed_medium && speed >= SLOW_SPEED =>
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
            governor
                .min(follow)
                .min(slide)
                .min(corner)
                .min(p.throttle_confidence)
        };

        // Steering (0x77AC50): heading gain plus yaw-rate damping, normalised by
        // the car's maximum steering angle. The yaw term's sign is the damping
        // direction (original sign convention not confirmed).
        let yaw = view.angular[0] * up[0] + view.angular[1] * up[1] + view.angular[2] * up[2];
        let steer = ((p.st_angle_factor * heading_error + p.st_ang_vel_factor * yaw)
            / view.maximum_steering_angle.max(1e-3))
        .clamp(-1., 1.);

        // AUTHORED stuck recovery: slow for 2 s with throttle -> brake (the
        // automatic gearbox selects reverse) with opposite lock for 1.5 s.
        let (mut throttle, mut brake, mut steer) = (throttle, brake, steer);
        if self.reversing > 0. {
            self.reversing -= dt;
            throttle = 0.;
            brake = 1.;
            steer = -heading_error.signum();
        } else if speed < 1. && throttle > 0.5 {
            self.stuck += dt;
            if self.stuck > 2. {
                self.stuck = 0.;
                self.reversing = 1.5;
            }
        } else {
            self.stuck = 0.;
        }

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
        self.telemetry = Telemetry {
            arc,
            look_ahead: self.look_ahead,
            curvature: self.curvature,
            heading_error,
            line_distance: distance,
            line_target,
            governor_target,
            offset: self.offset,
            line_error: line_distance,
            slip,
        };
        Controls {
            throttle: self.throttle,
            brake: self.brake,
            steering: self.steering,
            handbrake: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    }
}
