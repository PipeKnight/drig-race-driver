//! The original's drift scoring, shared by the three drift formats
//! (drift_battle, drift_grand_prix, drift_freestyle), and the Drift Battle
//! position multiplier. Research, addresses and labels: docs/DISCIPLINES.md,
//! section "Drift scoring".
//!
//! Original objects [V, hash-pinned analysis image]:
//! - drift manager / `DriftSettings` (constructor 0x6683D0, `drift.xml`
//!   parser 0x62FAE0, update of every entry 0x65F250);
//! - per car one entry (constructor 0x652E30): a drift detector (0x65DD50 and
//!   0x62FF80) followed at +0x60 by the scorer (constructor 0x6300B0, update
//!   0x643BC0, end of a drift 0x630560, combo bank 0x63C370, combo fail
//!   0x630490);
//! - race end: 0x64D940 banks every entry and scales the score of a car that
//!   is still racing.
//!
//! Field names follow the original's debug strings where it has them
//! (`totalDriftPoints`, `currentDriftType`, ...).

/// One `DriftType`: a drift that lasts longer than `time` seconds earns
/// `bonus` and the next rating (`osd_drift` ... `osd_drift_legend`).
#[derive(Clone, Debug, PartialEq)]
pub struct DriftType {
    pub name: String,
    pub time: f32,
    pub bonus: i32,
}

/// `DriftSettings` [V constructor 0x6683D0, parser 0x62FAE0]. The defaults are
/// the constructor's; `gameconfig/drift.xml` overrides most of them [D].
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    /// Degrees (+0x2C).
    pub threshold_slide_angle: f32,
    /// Seconds above the angle and speed before a drift counts (+0x30).
    pub threshold_slide_time: f32,
    /// Seconds a combo waits for the next drift (+0x34).
    pub combo_time: f32,
    /// mph needed to start a drift (+0x38).
    pub threshold_speed_enter: f32,
    /// mph needed to keep drifting (+0x3C).
    pub threshold_speed_exit: f32,
    /// +0x40: parsed but not read by the single-player scorer [V, see docs].
    pub collision_penalty: i32,
    /// Seconds between score steps (+0x44).
    pub score_update_interval: f32,
    /// +0x48.
    pub score_divisor: f32,
    /// A later drift must last this long (s) to raise the combo (+0x4C).
    pub minimum_combo_drift_length: f32,
    /// +0x50: seconds before the next collision can fail a combo again; no
    /// `drift.xml` attribute.
    pub combo_fail_cooldown: f32,
    /// +0x54/+0x58: smoke presentation; not read by the scorer.
    pub smoke_factor: f32,
    pub max_smoke_multiplier: f32,
    /// Close drift bonus at zero distance (+0x5C).
    pub max_close_drift_bonus: f32,
    /// Metres from a `close_drift_flag` (+0x60, +0x64).
    pub close_drift_radius: f32,
    pub close_drift_inner_radius: f32,
    /// +0x68: not read by the traced scorer.
    pub wrong_way_threshold: f32,
    /// Added to the main multiplier every score step of a drift (+0x6C).
    pub main_multiplier_increase: f32,
    /// Solved contact impact that fails a combo (+0x70).
    pub collision_threshold: f32,
    /// Drift Battle multipliers for the race leader and the last car
    /// (+0x74/+0x78; no `drift.xml` attribute, so 3 and 1).
    pub battle_first_multiplier: f32,
    pub battle_last_multiplier: f32,
    /// AI score factor per difficulty level 0..5 (+0x1B0).
    pub difficulty_bias: [f32; 6],
    /// Up to seven ratings (+0x7C, 0x2C bytes each).
    pub types: Vec<DriftType>,
    /// +0x1C8: drifts count without race progress (freestyle arenas) [I].
    pub ignore_progress: bool,
    /// +0x1C9: drifts count without the detector's +0x41 permission [I].
    pub ignore_permission: bool,
}

impl Default for Settings {
    fn default() -> Self {
        let types = [
            ("Drift", 0., 0),
            ("Great Drift", 2., 500),
            ("Fantastic Drift", 3., 1000),
            ("Incredible Drift", 4., 1500),
            ("Insane Drift", 5., 2000),
            ("Drift King ", 6., 5000),
            ("Drift Legend", 7., 10000),
        ];
        Self {
            threshold_slide_angle: 5.,
            threshold_slide_time: 0.2,
            combo_time: 2.,
            threshold_speed_enter: 40.,
            threshold_speed_exit: 20.,
            collision_penalty: 500,
            score_update_interval: 0.1,
            score_divisor: 100.,
            minimum_combo_drift_length: 0.5,
            combo_fail_cooldown: 0.5,
            smoke_factor: 0.1,
            max_smoke_multiplier: 3.,
            max_close_drift_bonus: 1000.,
            close_drift_radius: 10.,
            close_drift_inner_radius: 1.,
            wrong_way_threshold: 20.,
            main_multiplier_increase: 0.1,
            collision_threshold: 150.,
            battle_first_multiplier: 3.,
            battle_last_multiplier: 1.,
            difficulty_bias: [1.; 6],
            types: types
                .iter()
                .map(|&(name, time, bonus)| DriftType {
                    name: name.into(),
                    time,
                    bonus,
                })
                .collect(),
            ignore_progress: false,
            ignore_permission: false,
        }
    }
}

/// `drift.xml` is malformed or out of range.
#[derive(Debug, Clone, PartialEq)]
pub struct Error(pub String);
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Error {}

impl Settings {
    /// The constructor's values overridden by a decoded `drift.xml` (root
    /// `DriftSettings`). Like the parser, absent attributes keep their
    /// defaults; `DriftType` rows replace the table in file order (at most
    /// seven), `DifficultyBias` rows set their level.
    pub fn from_document(doc: &grid_formats::binxml::Document<'_>) -> Result<Self, Error> {
        let root = doc
            .elements
            .iter()
            .position(|e| e.name == "DriftSettings")
            .ok_or_else(|| Error("drift.xml has no DriftSettings".into()))?;
        let e = &doc.elements[root];
        let get = |name: &str| {
            e.attributes
                .iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| *v)
        };
        let float = |text: &str, name: &str| -> Result<f32, Error> {
            text.trim()
                .trim_end_matches('f')
                .parse::<f32>()
                .ok()
                .filter(|v| v.is_finite())
                .ok_or_else(|| Error(format!("drift.xml {name}={text:?} is not a number")))
        };
        let mut s = Self::default();
        let floats: [(&str, &mut f32); 18] = [
            ("thresholdSlideAngle", &mut s.threshold_slide_angle),
            ("thresholdSlideTime", &mut s.threshold_slide_time),
            ("comboTime", &mut s.combo_time),
            ("thresholdSpeedEnter", &mut s.threshold_speed_enter),
            ("thresholdSpeedExit", &mut s.threshold_speed_exit),
            ("scoreUpdateInterval", &mut s.score_update_interval),
            ("scoreDivisor", &mut s.score_divisor),
            ("minimumComboDriftLength", &mut s.minimum_combo_drift_length),
            ("smokeFactor", &mut s.smoke_factor),
            ("maxSmokeMultiplier", &mut s.max_smoke_multiplier),
            ("maxCloseDriftBonus", &mut s.max_close_drift_bonus),
            ("closeDriftRadius", &mut s.close_drift_radius),
            ("closeDriftInnerRadius", &mut s.close_drift_inner_radius),
            ("wrongWayThreshold", &mut s.wrong_way_threshold),
            (
                "driftScoreMainMultiplierIncrease",
                &mut s.main_multiplier_increase,
            ),
            ("collisionThreshold", &mut s.collision_threshold),
            ("driftBattleFirstMultiplier", &mut s.battle_first_multiplier),
            ("driftBattleLastMultiplier", &mut s.battle_last_multiplier),
        ];
        for (name, field) in floats {
            if let Some(text) = get(name) {
                *field = float(text, name)?;
            }
        }
        if let Some(text) = get("collisionPenalty") {
            s.collision_penalty = text
                .trim()
                .parse()
                .map_err(|_| Error(format!("drift.xml collisionPenalty={text:?}")))?;
        }
        let mut types = Vec::new();
        for c in e.children.clone() {
            let child = &doc.elements[c];
            let attr = |name: &str| {
                child
                    .attributes
                    .iter()
                    .find(|(k, _)| *k == name)
                    .map(|(_, v)| *v)
                    .ok_or_else(|| Error(format!("drift.xml {} lacks {name}", child.name)))
            };
            match child.name {
                "DifficultyBias" => {
                    let level: usize = attr("level")?
                        .trim()
                        .parse()
                        .map_err(|_| Error("drift.xml DifficultyBias level".into()))?;
                    let bias = float(attr("bias")?, "bias")?;
                    *s.difficulty_bias.get_mut(level).ok_or_else(|| {
                        Error(format!("drift.xml DifficultyBias level {level}"))
                    })? = bias;
                }
                "DriftType" => {
                    // The parser stops filling at seven and reports the rest.
                    if types.len() == 7 {
                        return Err(Error("more Drift Types than were expected".into()));
                    }
                    types.push(DriftType {
                        name: attr("name")?.into(),
                        time: float(attr("time")?, "time")?,
                        bonus: attr("bonus")?
                            .trim()
                            .parse()
                            .map_err(|_| Error("drift.xml DriftType bonus".into()))?,
                    });
                }
                _ => {}
            }
        }
        if !types.is_empty() {
            s.types = types;
        }
        if s.score_divisor == 0.
            || s.score_update_interval <= 0.
            || s.close_drift_radius <= s.close_drift_inner_radius
        {
            return Err(Error("drift.xml values out of range".into()));
        }
        Ok(s)
    }

    /// `gameconfig/drift.xml` of the installation.
    pub fn load(root: &std::path::Path) -> Result<Self, Box<dyn std::error::Error>> {
        let bytes = std::fs::read(root.join("gameconfig/drift.xml"))?;
        Ok(Self::from_document(&grid_formats::binxml::read(&bytes)?)?)
    }
}

/// `close_drift_flag` entity positions of a route (0x668670 collects every
/// entity whose name contains `close_drift_flag`).
pub fn close_drift_flags(
    root: &std::path::Path,
    route: &crate::route::Route,
) -> Result<Vec<[f32; 3]>, Box<dyn std::error::Error>> {
    use grid_formats::{entities, pssg};
    let read = |path: String| std::fs::read(root.join(path));
    let placements = read(route.file("objects.ens"))?;
    let types = read(route.track_file("objecttypes.pssg"))?;
    let placements = pssg::read_tree(&placements)?;
    let types = pssg::read_tree(&types)?;
    Ok(entities::read(&placements, &types)?
        .iter()
        .filter(|e| {
            let id = String::from_utf8_lossy(e.id);
            id.contains("close_drift_flag")
        })
        .map(|e| [e.transform[12], e.transform[13], e.transform[14]])
        .collect())
}

/// m/s to mph as 0xA7DE00 computes it: × 3600 × 0.001 × 0.62137121 (x87,
/// stored as a float).
pub fn mph(metres_per_second: f32) -> f32 {
    (f64::from(metres_per_second)
        * f64::from(3600f32)
        * f64::from(0.001f32)
        * f64::from(0.621_371_2_f32)) as f32
}

/// What the detector reads from a car each tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sample {
    pub velocity: [f32; 3],
    /// Car axes in the world: forward (pose +Z) and left (pose +X).
    pub forward: [f32; 3],
    pub left: [f32; 3],
    /// Wheel positions in the world.
    pub wheels: [[f32; 3]; 4],
    /// Race progress in laps (completed laps plus the lap fraction).
    pub progress: f32,
    /// Race position, 0 = leader, and the number of cars.
    pub position: usize,
    pub cars: usize,
    /// Largest solved contact impact of the car this tick (0 without contact).
    pub impact: f32,
}

impl Sample {
    /// Sample of a source car with its race progress (laps), 0-based race
    /// position, field size and largest solved contact impact this tick.
    pub fn of(
        car: &crate::source_vehicle::Runtime,
        progress: f32,
        position: usize,
        cars: usize,
        impact: f32,
    ) -> Self {
        let pose = car.pose();
        let v = car.velocity();
        let wheels = car.wheels().map(|w| {
            std::array::from_fn(|k| {
                pose[12 + k]
                    + w.anchor[0] * pose[k]
                    + w.anchor[1] * pose[4 + k]
                    + w.anchor[2] * pose[8 + k]
            })
        });
        Self {
            velocity: [v[0], v[1], v[2]],
            forward: [pose[8], pose[9], pose[10]],
            left: [pose[0], pose[1], pose[2]],
            wheels,
            progress,
            position,
            cars,
            impact,
        }
    }
}

/// A car's contacts of one tick split into the collision impact the drift
/// detector reads and whether a `close_drift_flag` box (`flags`) was touched.
/// - 0x62FFE0 leaves the combo alone when the other body's callback is the
///   car body-impact dispatcher 0xA01910 [V]: contact with another car is no
///   collision [I: 0xA01910 serves car bodies only].
/// - 0x64D840 skips a contact whose other object reports type 11 (0x958310)
///   [V], the wrapper class (vtable 0xD3A778) the body factory 0xA73FD0
///   builds for every packed `qt_*.vcqtc` collision chunk in the static
///   loader 0x5BB180 [V]: touching the road, kerbs or verges is no
///   collision (F387). The boundary sheet is shape kind 10 and still counts
///   [I: its wrapper type is not traced].
/// - The original's flags are jointed entities whose touch the listener
///   0x64D840 reports through 0x64D7E0; the port keeps them as static boxes
///   (no joint rows, F284), so their contact is a flag touch only [I].
pub fn split_contacts(
    contacts: &[(f32, crate::source_vehicle::multi::Contacted)],
    flags: &[usize],
) -> (f32, bool) {
    use crate::source_vehicle::multi::Contacted;
    let mut impact = 0f32;
    let mut touched = false;
    for &(value, other) in contacts {
        match other {
            Contacted::Prop(p) if flags.contains(&p) => touched = true,
            Contacted::Car(_) | Contacted::Track => {}
            _ => impact = impact.max(value),
        }
    }
    (impact, touched)
}

/// The drift `DifficultyBias` level of an AI difficulty: the nearest star
/// level (`crate::flashback::star_level`) [I: 0x40B700 returns the star level].
pub fn ai_level(difficulty: f32) -> usize {
    crate::flashback::star_level(difficulty)
}

/// The drift detector of one car [V 0x65DD50, 0x62FF80, 0x62FFE0].
#[derive(Clone, Debug, PartialEq)]
pub struct Detector {
    /// `isDrifting` (+0x08).
    pub drifting: bool,
    /// mph (+0x0C) and degrees (+0x10) of the current tick.
    pub speed_mph: f32,
    pub angle: f32,
    /// Signed slip angle in radians (+0x14); the sign's side is [I].
    pub signed_angle: f32,
    /// m/s (+0x18).
    pub speed: f32,
    /// A wheel is inside a close-drift radius (+0x1C) and the smallest
    /// normalised distance 0..1 (+0x20, `closeDriftDist`).
    pub close: bool,
    pub close_distance: f32,
    /// Seconds above the angle and speed thresholds (+0x24).
    pub slide_time: f32,
    /// Seconds since a collision above the threshold (+0x30) and since a
    /// `close_drift_flag` was touched (+0x38); both start at 9999.
    pub since_collision: f64,
    pub since_flag_touch: f64,
    /// Drift permission (+0x41) [I: always true here, see docs].
    pub permitted: bool,
    /// Drift Battle position multiplier (+0x44).
    pub position_multiplier: f32,
    /// Race progress in laps (+0x54).
    pub progress: f32,
}

impl Default for Detector {
    fn default() -> Self {
        Self {
            drifting: false,
            speed_mph: 0.,
            angle: 0.,
            signed_angle: 0.,
            speed: 0.,
            close: false,
            close_distance: 0.,
            slide_time: 0.,
            since_collision: 9999.,
            since_flag_touch: 9999.,
            permitted: false,
            position_multiplier: 1.,
            progress: 0.,
        }
    }
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
/// The X/Z lanes (Y is up), as the original's lane masks keep them.
fn horizontal(v: [f32; 3]) -> [f32; 3] {
    [v[0], 0., v[2]]
}
fn length(v: [f32; 3]) -> f32 {
    dot(v, v).sqrt()
}
fn normalised(v: [f32; 3]) -> [f32; 3] {
    let l = length(v);
    if l > 0. { v.map(|x| x / l) } else { v }
}

impl Detector {
    /// One tick of `dt` seconds.
    pub fn update(&mut self, s: &Settings, flags: &[[f32; 3]], sample: &Sample, dt: f32) {
        self.permitted = true;
        self.since_collision += f64::from(dt);
        self.since_flag_touch += f64::from(dt);
        // 0x62FFE0: a collision above the threshold restarts the clock.
        if sample.impact > s.collision_threshold {
            self.since_collision = 0.;
        }
        self.speed = length(sample.velocity);
        // Slip angle between the horizontal velocity and the car's nose;
        // faded below 1 m/s; negative towards the left axis [I side].
        let v = horizontal(sample.velocity);
        let speed = length(v);
        let mut angle = 0.;
        if speed > 0. {
            let direction = v.map(|x| x / speed);
            let cosine = dot(direction, normalised(horizontal(sample.forward))).clamp(-1., 1.);
            angle = cosine.acos();
            if speed < 1. {
                angle *= speed;
            }
            if dot(direction, normalised(horizontal(sample.left))) > 0. {
                angle = -angle;
            }
        }
        self.signed_angle = angle;
        // Close drift: wheels against every flag, in the X/Z plane.
        self.close = false;
        let mut nearest = 1f32;
        for wheel in &sample.wheels {
            for flag in flags {
                let (dx, dz) = (flag[0] - wheel[0], flag[2] - wheel[2]);
                let d = ((dz * dz + dx * dx).sqrt() - s.close_drift_inner_radius)
                    / (s.close_drift_radius - s.close_drift_inner_radius);
                let d = if d >= 0. { d.min(1.) } else { 0. };
                if d < nearest {
                    self.close = true;
                    nearest = d;
                }
            }
        }
        self.close_distance = nearest;
        self.progress = sample.progress;
        self.position_multiplier = if sample.cars > 1 {
            let first = s.battle_first_multiplier;
            first
                - (sample.position as f32 / (sample.cars - 1) as f32)
                    * (first - s.battle_last_multiplier)
        } else {
            1.
        };
        // 0x62FF80 with the angle in degrees and the speed in mph.
        self.angle = angle.abs() * 57.295_776;
        self.speed_mph = mph(self.speed);
        let needed = if self.drifting {
            s.threshold_speed_exit
        } else {
            s.threshold_speed_enter
        };
        self.slide_time = if self.angle < s.threshold_slide_angle || self.speed_mph < needed {
            0.
        } else {
            self.slide_time + dt
        };
        self.drifting = self.slide_time > s.threshold_slide_time;
    }

    /// The car touched a `close_drift_flag` (0x64D7E0).
    pub fn touch_flag(&mut self) {
        self.since_flag_touch = 0.;
    }
}

/// Banked count and points of one drift rating.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rating {
    pub points: f32,
    pub count: i32,
}

/// The scorer of one car [V constructor 0x6300B0, update 0x643BC0].
#[derive(Clone, Debug, PartialEq)]
pub struct Scorer {
    /// Score step timer (+0x04).
    pub step_timer: f32,
    /// Seconds spent drifting (+0x08).
    pub drift_time: f32,
    /// `totalDriftPoints`: banked score (+0x0C).
    pub total: f32,
    /// Points of the current drift (+0x10).
    pub current: f32,
    /// Points of finished drifts in the open combo (+0x14).
    pub combo_points: f32,
    /// Combo multiplier, 0..99 (+0x18).
    pub combo: i32,
    /// Duration of the current drift (+0x1C).
    pub drift_duration: f32,
    /// `currentDriftType`: ratings reached by the current drift (+0x20).
    pub rating: usize,
    /// Combo timer (+0x24).
    pub combo_timer: f32,
    /// Last tick's drift counted (+0x28) and the current drift raised the
    /// combo (+0x29).
    pub was_drifting: bool,
    counted: bool,
    /// Detector permission, this and last tick (+0x2A/+0x2B).
    permitted: bool,
    /// Combo-fail cooldown (+0x2C).
    fail_cooldown: f32,
    /// Race progress and its maximum (+0x30/+0x34).
    pub progress: f32,
    max_progress: f32,
    /// Distance of the current drift (+0x38).
    pub drift_distance: f32,
    /// Close-drift flags of this/last tick (+0x3C/+0x3D), smallest distance
    /// (+0x40), armed (+0x44), awarded this drift (+0x45).
    close: bool,
    was_close: bool,
    close_minimum: f32,
    close_armed: bool,
    pub close_awarded: bool,
    /// Close drifts (+0x48).
    pub close_drifts: i32,
    /// The combo just failed; cleared by the next score step (+0x4C).
    pub combo_failed: bool,
    /// Wrong-way measure (+0x50) and the travel direction it is taken
    /// against (+0x120); kept for the HUD only.
    pub wrong_way: f32,
    direction: i32,
    /// Score-step multiplier (+0x54) and 1 = Drift Battle (+0x58).
    pub step_multiplier: f32,
    pub battle: bool,
    /// Last close drift: distance and bonus (+0x60/+0x64).
    pub last_close_distance: f32,
    pub last_close_bonus: f32,
    /// Main multiplier, growing every score step of a drift (+0x68).
    pub main_multiplier: f32,
    /// Plays the player's drift sounds (+0x78); not ported.
    pub player: bool,
    /// Pending ratings of the open combo (+0x7C) and banked ones (+0xB4).
    pending: [Rating; 7],
    pub ratings: [Rating; 7],
    /// Best banked combo (+0xEC), fastest drift speed in mph (+0xF0),
    /// longest drift distance (+0xF4) with its points (+0xF8), largest angle
    /// (+0xFC), longest drift (+0x100).
    pub best_combo: f32,
    pub fastest_drift: f32,
    pub longest_distance: f32,
    pub longest_distance_points: i32,
    pub largest_angle: f32,
    pub longest_drift: f32,
    /// `numSingleDrifts` (+0x104) and running averages of distance, angle and
    /// speed (+0x108/+0x10C/+0x114).
    pub drifts: i32,
    pub average_distance: f32,
    pub average_angle: f32,
    pub average_speed: f32,
    /// Largest angle and speed of the current drift (+0x110/+0x118).
    drift_angle: f32,
    drift_speed: f32,
    /// Best single drift and its rating (+0x128/+0x12C).
    pub best_drift: i32,
    pub best_drift_rating: i32,
    /// Displayed score (+0x130).
    pub display: f32,
    /// Banked by the race end (+0x134).
    pub banked_at_end: bool,
    /// AI difficulty level for the bias, `None` for the player (+0x124).
    pub difficulty: Option<usize>,
}

impl Scorer {
    pub fn new(battle: bool, difficulty: Option<usize>, player: bool) -> Self {
        Self {
            step_timer: 0.,
            drift_time: 0.,
            total: 0.,
            current: 0.,
            combo_points: 0.,
            combo: 0,
            drift_duration: 0.,
            rating: 0,
            combo_timer: 0.,
            was_drifting: false,
            counted: false,
            permitted: true,
            fail_cooldown: 0.,
            progress: 0.,
            max_progress: 0.,
            drift_distance: 0.,
            close: false,
            was_close: false,
            close_minimum: 0.,
            close_armed: false,
            close_awarded: false,
            close_drifts: 0,
            combo_failed: false,
            wrong_way: 0.,
            direction: 0,
            step_multiplier: 0.,
            battle,
            last_close_distance: 0.,
            last_close_bonus: 0.,
            main_multiplier: 1.,
            player,
            pending: [Rating::default(); 7],
            ratings: [Rating::default(); 7],
            best_combo: 0.,
            fastest_drift: 0.,
            longest_distance: 0.,
            longest_distance_points: 0,
            largest_angle: 0.,
            longest_drift: 0.,
            drifts: 0,
            average_distance: 0.,
            average_angle: 0.,
            average_speed: 0.,
            drift_angle: 0.,
            drift_speed: 0.,
            best_drift: 0,
            best_drift_rating: -1,
            display: 0.,
            banked_at_end: false,
            difficulty,
        }
    }

    /// One tick (0x643BC0) after the detector's update.
    pub fn update(&mut self, s: &Settings, d: &Detector, dt: f32) {
        self.step_multiplier = if self.battle {
            d.position_multiplier
        } else {
            1.
        };
        let bias = self
            .difficulty
            .and_then(|level| s.difficulty_bias.get(level).copied())
            .unwrap_or(1.);
        self.step_timer -= dt;
        self.fail_cooldown -= dt;
        self.permitted = d.permitted;
        // A jump of more than half a lap (the line or a reset) restarts the
        // furthest progress.
        let progress = d.progress;
        if (progress - self.progress).abs() > 0.5 {
            self.max_progress = progress;
        }
        self.progress = progress;
        if progress > self.max_progress {
            self.max_progress = progress;
        }
        if self.direction == 0 {
            if d.signed_angle > 0. {
                self.direction = 1;
            } else if d.signed_angle < 0. {
                self.direction = -1;
            }
        }
        self.wrong_way = if d.speed_mph > 4. {
            (d.signed_angle * std::f32::consts::FRAC_1_PI).abs() * self.direction as f32
        } else {
            0.
        };
        // A drift counts while racing forwards past the start.
        let mut scoring = d.drifting
            && (self.permitted || s.ignore_permission)
            && progress >= self.max_progress
            && progress > 0.;
        if s.ignore_progress {
            scoring = d.drifting;
            self.permitted = true;
        } else if s.ignore_permission {
            self.permitted = true;
        }
        if self.wrong_way.abs() < 0.5 {
            self.direction = 0;
        }
        if d.since_collision <= f64::from(dt) {
            self.fail_combo(s);
        }
        if d.since_flag_touch <= f64::from(dt) {
            self.close_armed = false;
        }
        if self.step_timer < 0. {
            loop {
                if scoring {
                    self.combo_failed = false;
                    let step = (((d.angle * d.speed_mph) / s.score_divisor)
                        * self.step_multiplier
                        * bias
                        * self.main_multiplier) as i32 as f32;
                    self.current = (step + self.current).max(1.);
                    self.drift_angle = self.drift_angle.max(d.angle);
                    self.drift_speed = self.drift_speed.max(d.speed_mph);
                    self.drift_time += dt;
                    self.drift_distance += d.speed * dt;
                    self.fastest_drift = self.fastest_drift.max(d.speed_mph);
                    self.main_multiplier += s.main_multiplier_increase;
                }
                self.step_timer += s.score_update_interval;
                if self.step_timer >= 0. {
                    break;
                }
            }
        }
        self.was_close = self.close;
        self.close = d.close;
        let distance = d.close_distance;
        if !scoring {
            if self.was_drifting {
                self.end_drift(s);
            }
            self.combo_timer -= dt;
            if self.combo_timer < 0. && self.combo != 0 {
                self.bank(s);
            }
        } else {
            self.drift_duration += dt;
            self.combo_timer = s.combo_time;
            if !self.counted
                && (self.drift_duration > s.minimum_combo_drift_length || self.combo == 0)
            {
                if self.combo < 99 {
                    self.combo += 1;
                }
                self.counted = true;
            }
            if self.close && !self.was_close {
                self.close_armed = true;
            }
            if let Some(t) = s.types.get(self.rating).filter(|_| self.rating < 7)
                && self.drift_duration > t.time
            {
                let bonus = (t.bonus as f32 * self.step_multiplier * bias) as i32 as f32;
                self.current = (bonus + self.current).max(1.);
                self.rating += 1;
            }
            if !self.was_close && self.close {
                self.close_minimum = distance;
            }
            let leaving = if self.close {
                if distance < self.close_minimum {
                    self.close_minimum = distance;
                }
                false
            } else {
                self.was_close && self.close_armed
            };
            if leaving {
                let closeness = ((1. - self.close_minimum) * 100.) as i32 as f32 * 0.01;
                self.close_drifts += 1;
                let bonus = self.step_multiplier * s.max_close_drift_bonus * closeness * bias;
                self.current += bonus as i32 as f32;
                self.close_awarded = true;
                self.last_close_distance = self.close_minimum;
                self.last_close_bonus = bonus;
            }
        }
        self.display = (self.combo_points + self.current) * self.combo as f32 + self.total;
        self.was_drifting = scoring;
    }

    /// The current drift ends (0x630560): statistics, and its points join
    /// the open combo.
    fn end_drift(&mut self, s: &Settings) {
        if self.drift_distance > self.longest_distance {
            self.longest_distance = self.drift_distance;
            self.longest_distance_points = self.current as i32;
        }
        self.longest_drift = self.longest_drift.max(self.drift_duration);
        self.largest_angle = self.largest_angle.max(self.drift_angle);
        let rating = self.rating as i32 - 1;
        self.combo_timer = s.combo_time;
        if rating >= 0 {
            let r = &mut self.pending[rating as usize];
            r.count += 1;
            r.points += self.current;
        }
        let previous = self.drifts as f32;
        self.drifts += 1;
        if (self.best_drift as f32) < self.current {
            self.best_drift = self.current as i32;
            self.best_drift_rating = rating;
        }
        let share = 1. / self.drifts as f32;
        self.average_speed = share * (self.average_speed * previous + self.drift_speed);
        self.average_distance = share * (self.average_distance * previous + self.drift_distance);
        self.average_angle = share * (previous * self.average_angle + self.drift_angle);
        self.drift_distance = 0.;
        self.counted = false;
        self.rating = 0;
        self.combo_points += self.current;
        self.current = 0.;
        self.drift_duration = 0.;
        self.drift_angle = 0.;
        self.drift_speed = 0.;
        self.close_armed = false;
        self.close_minimum = 1.;
        self.was_drifting = false;
        self.main_multiplier = 1.;
    }

    /// The combo timer ran out (0x63C370): the combo is banked at its
    /// multiplier. Like the original it closes a drift first, which also
    /// counts towards `numSingleDrifts`.
    fn bank(&mut self, s: &Settings) {
        self.end_drift(s);
        let combo = self.combo as f32 * self.combo_points;
        for (banked, pending) in self.ratings.iter_mut().zip(&mut self.pending) {
            banked.count += pending.count;
            banked.points += self.combo as f32 * pending.points;
            *pending = Rating::default();
        }
        self.combo_points = 0.;
        self.combo_timer = 0.;
        self.total += combo;
        self.combo = 0;
        self.main_multiplier = 1.;
        if self.best_combo < combo {
            self.best_combo = combo;
        }
    }

    /// A collision during a combo (0x630490): the drift and the combo are
    /// lost.
    fn fail_combo(&mut self, s: &Settings) {
        if self.combo == 0 || self.fail_cooldown >= 0. {
            return;
        }
        self.combo_points = 0.;
        self.current = 0.;
        self.combo_timer = 0.;
        self.combo = 0;
        self.close_armed = false;
        self.fail_cooldown = s.combo_fail_cooldown;
        self.combo_failed = true;
        self.pending = [Rating::default(); 7];
        self.main_multiplier = 1.;
    }

    /// Race end for this car (0x64D940). The open combo is banked. A car
    /// still racing that moves faster than 5 mph has its score scaled up to
    /// the event distance. Returns true when the shown score is still zero,
    /// so the virtual race's estimate replaces it (`Entry::finish_race`).
    pub fn finish_race(&mut self, s: &Settings, finished: bool, speed: f32, laps: u32) -> bool {
        self.banked_at_end = true;
        self.bank(s);
        if finished {
            return false;
        }
        // 0xA7DDB0(5.0): 5 mph in m/s.
        let five_mph = (5f64
            * f64::from(1.609_344_f32)
            * f64::from(999.999_94_f32)
            * f64::from(0.000_277_777_8_f32)) as f32;
        let laps = laps as f32;
        if five_mph < speed && laps > self.progress && self.progress > 0. {
            self.total *= laps / self.progress;
        }
        self.display == 0.
    }
}

/// One car of a drift event.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub detector: Detector,
    pub scorer: Scorer,
    /// Banked at the race end with a zero score, which the virtual race's
    /// estimate replaced (`crate::virtual_race`).
    pub estimated: bool,
    pub done: bool,
}

impl Entry {
    pub fn new(battle: bool, difficulty: Option<usize>, player: bool) -> Self {
        Self {
            detector: Detector::default(),
            scorer: Scorer::new(battle, difficulty, player),
            estimated: false,
            done: false,
        }
    }
    /// One tick: the detector, then the scorer. Inactive after the race end.
    pub fn update(&mut self, s: &Settings, flags: &[[f32; 3]], sample: &Sample, dt: f32) {
        if self.done {
            return;
        }
        self.detector.update(s, flags, sample, dt);
        self.scorer.update(s, &self.detector, dt);
    }
    /// The score shown and classified.
    pub fn score(&self) -> i64 {
        if self.done {
            self.scorer.total as i64
        } else {
            self.scorer.display as i64
        }
    }
    /// Race end (0x64D940); `performance` is the car's driver skill for the
    /// virtual race (0x62B170: `Virtual_Performance`, 1.0 without a driver).
    pub fn finish_race(
        &mut self,
        s: &Settings,
        (finished, speed, laps): (bool, f32, u32),
        race: &mut crate::virtual_race::VirtualRace,
        performance: f32,
    ) {
        if self.done {
            return;
        }
        if self.scorer.finish_race(s, finished, speed, laps) {
            // The estimate times the laps replaces the banked total (+0xC)
            // and the shown score (+0x130).
            let score = race.drift_score(performance, laps);
            self.scorer.total = score;
            self.scorer.display = score;
            self.estimated = race.loaded;
        }
        self.done = true;
    }
}

/// Drift results order [I]: higher score first; equal scores keep the race
/// order (`race_order` lists car indices, leader first).
pub fn classification(entries: &[Entry], race_order: &[usize]) -> Vec<usize> {
    let mut order: Vec<usize> = race_order.to_vec();
    order.sort_by_key(|&car| std::cmp::Reverse(entries[car].score()));
    order
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1. / 60.;

    #[test]
    fn flags_are_touched_and_cars_are_no_collision() {
        use crate::source_vehicle::multi::Contacted::{Car, Prop, Static, Track};
        let flags = [7, 9];
        // A flag box is a touch, never a collision; another car is neither.
        assert_eq!(
            split_contacts(&[(900., Prop(7)), (700., Car(2))], &flags),
            (0., true)
        );
        // The boundary sheet and other props are collisions; the largest
        // counts. The packed road chunks are not (0x958310 type 11).
        assert_eq!(
            split_contacts(&[(450., Static), (300., Prop(3)), (90., Prop(9))], &flags),
            (450., true)
        );
        assert_eq!(
            split_contacts(&[(455., Track), (300., Prop(3))], &flags),
            (300., false)
        );
        assert_eq!(split_contacts(&[], &flags), (0., false));
        // A touch disarms the close-drift bonus like the original's clock.
        let mut d = Detector::default();
        d.touch_flag();
        assert_eq!(d.since_flag_touch, 0.);
    }

    fn shipped() -> Settings {
        // gameconfig/drift.xml values [D] (local/race-rules/data).
        let mut s = Settings {
            threshold_slide_angle: 15.,
            threshold_slide_time: 0.5,
            combo_time: 3.5,
            threshold_speed_enter: 50.,
            threshold_speed_exit: 30.,
            collision_penalty: 500,
            score_update_interval: 0.1,
            score_divisor: 100.,
            minimum_combo_drift_length: 1.,
            max_close_drift_bonus: 20000.,
            close_drift_radius: 10.,
            close_drift_inner_radius: 2.,
            smoke_factor: 0.1,
            max_smoke_multiplier: 3.,
            wrong_way_threshold: 350.,
            main_multiplier_increase: 0.1,
            collision_threshold: 400.,
            difficulty_bias: [0.1, 0.2, 0.5, 1., 1.8, 4.],
            ..Settings::default()
        };
        let types = [
            ("osd_drift", 0., 0),
            ("osd_great_drift", 1., 500),
            ("osd_fantastic_drift", 2., 1000),
            ("osd_incredible_drift", 3., 1500),
            ("osd_insane_drift", 4., 2000),
            ("osd_drift_king", 5., 5000),
            ("osd_drift_legend", 6., 10000),
        ];
        s.types = types
            .iter()
            .map(|&(n, t, b)| DriftType {
                name: n.into(),
                time: t,
                bonus: b,
            })
            .collect();
        s
    }

    /// A car moving at `speed` m/s with its nose `angle` degrees off the
    /// velocity, `progress` laps into the race.
    fn sample(speed: f32, angle: f32, progress: f32) -> Sample {
        let a = angle.to_radians();
        Sample {
            velocity: [0., 0., speed],
            forward: [a.sin(), 0., a.cos()],
            left: [a.cos(), 0., -a.sin()],
            progress,
            cars: 1,
            ..Default::default()
        }
    }

    #[test]
    fn mph_conversion_matches_the_original_factors() {
        assert!((mph(10.) - 22.369_362).abs() < 1e-4);
        assert!((mph(22.352) - 50.).abs() < 1e-3);
    }

    #[test]
    fn detector_needs_angle_speed_and_half_a_second() {
        let s = shipped();
        let mut d = Detector::default();
        // 30 m/s = 67.1 mph, 30 degrees: drifting only after 0.5 s.
        for tick in 1..=40 {
            d.update(&s, &[], &sample(30., 30., 0.5), DT);
            if tick <= 29 || tick >= 32 {
                assert_eq!(d.drifting, tick >= 32, "tick {tick}");
            }
        }
        assert!((d.angle - 30.).abs() < 1e-3);
        // Below the exit speed (30 mph) the drift ends; 35 mph keeps it.
        d.update(&s, &[], &sample(15.7, 30., 0.5), DT);
        assert!(d.drifting, "35 mph is above the exit speed");
        d.update(&s, &[], &sample(13., 30., 0.5), DT);
        assert!(!d.drifting && d.slide_time == 0.);
        // 14 degrees never drifts; the enter speed is 50 mph.
        let mut d = Detector::default();
        for _ in 0..60 {
            d.update(&s, &[], &sample(30., 14., 0.5), DT);
            assert!(!d.drifting);
        }
        let mut d = Detector::default();
        for _ in 0..60 {
            d.update(&s, &[], &sample(21., 40., 0.5), DT);
            assert!(!d.drifting, "47 mph is below the enter speed");
        }
    }

    #[test]
    fn close_distance_is_normalised_between_the_radii() {
        let s = shipped();
        let mut d = Detector::default();
        let mut x = sample(30., 30., 0.5);
        x.wheels = [[6., 0., 0.], [20., 0., 0.], [30., 0., 0.], [40., 0., 0.]];
        d.update(&s, &[[0., 5., 0.]], &x, DT);
        assert!(d.close);
        assert!((d.close_distance - 0.5).abs() < 1e-6);
        x.wheels = [[1., 0., 0.]; 4];
        d.update(&s, &[[0., 0., 0.]], &x, DT);
        assert_eq!(d.close_distance, 0.);
        x.wheels = [[11., 0., 0.]; 4];
        d.update(&s, &[[0., 0., 0.]], &x, DT);
        assert!(!d.close && d.close_distance == 1.);
    }

    /// Drives one entry through `ticks` of `sample`.
    fn run(e: &mut Entry, s: &Settings, x: &Sample, ticks: usize) {
        for _ in 0..ticks {
            e.update(s, &[], x, DT);
        }
    }

    /// Freestyle drift (the arenas have no race progress): without the flag a
    /// drift at progress 0 never scores; with it ([V] set for the
    /// drift_freestyle format) it does.
    #[test]
    fn a_drift_without_race_progress_scores_only_when_progress_is_ignored() {
        let mut s = shipped();
        let drift = sample(30., 30., 0.);
        let mut e = Entry::new(false, None, true);
        run(&mut e, &s, &drift, 120);
        assert_eq!((e.scorer.display, e.scorer.total), (0., 0.));
        s.ignore_progress = true;
        let mut e = Entry::new(false, None, true);
        run(&mut e, &s, &drift, 120);
        assert!(e.scorer.display > 0. || e.scorer.total > 0.);
    }

    #[test]
    fn a_single_drift_scores_steps_ratings_and_banks_after_the_combo_time() {
        let s = shipped();
        let mut e = Entry::new(false, None, true);
        let drift = sample(30., 30., 0.5);
        // 0.5 s to start, then 1.2 s of drifting.
        run(&mut e, &s, &drift, 29);
        assert!(!e.scorer.was_drifting);
        run(&mut e, &s, &drift, 1);
        assert!(e.scorer.display <= 1., "at most the first rating clamp");
        run(&mut e, &s, &drift, 72);
        let sc = &e.scorer;
        assert!(sc.was_drifting && sc.combo == 1);
        // Ratings: osd_drift at once, osd_great_drift after 1 s (+500).
        assert_eq!(sc.rating, 2);
        let expected_step = |m: f32| ((30. * mph(30.) / 100.) * m) as i32 as f32;
        // 13 score steps in 1.2 s of 1/60 s ticks (steps on the 0.1 s grid):
        // the main multiplier grows by 0.1 per step.
        // The first rating (osd_drift, bonus 0) clamps the drift to 1 point.
        let mut points = 1f32;
        let mut m = 1f32;
        let steps = (sc.main_multiplier - 1.) / 0.1;
        let steps = steps.round() as usize;
        for _ in 0..steps {
            points = (expected_step(m) + points).max(1.);
            m += 0.1;
        }
        assert_eq!(
            sc.current,
            points + 500.,
            "{} vs {}",
            sc.current,
            points + 500.
        );
        assert_eq!(sc.display, sc.current);
        // Straightening up ends the drift; the combo is banked 3.5 s later.
        let straight = sample(30., 0., 0.6);
        run(&mut e, &s, &straight, 1);
        let drift_points = e.scorer.combo_points;
        assert!(drift_points > 500. && e.scorer.current == 0.);
        assert_eq!(e.scorer.total, 0.);
        run(&mut e, &s, &straight, 200);
        assert_eq!(e.scorer.total, 0., "still within the combo time");
        run(&mut e, &s, &straight, 20);
        assert_eq!(e.scorer.total, drift_points);
        assert_eq!(e.scorer.combo, 0);
        assert_eq!(e.scorer.best_combo, drift_points);
        // numSingleDrifts counts the drift and, as in the original, the bank.
        assert_eq!(e.scorer.drifts, 2);
        assert_eq!(e.scorer.ratings[1].count, 1);
        assert_eq!(e.scorer.ratings[1].points, drift_points);
    }

    #[test]
    fn combos_multiply_and_short_drifts_do_not_raise_them() {
        let s = shipped();
        let mut e = Entry::new(false, None, true);
        let drift = sample(30., 30., 0.5);
        let straight = sample(30., 0., 0.5);
        // First drift: 0.5 s detection + 1.5 s scoring → combo 1.
        run(&mut e, &s, &drift, 120);
        run(&mut e, &s, &straight, 30);
        let first = e.scorer.combo_points;
        // A second drift of 0.8 s (< minimumComboDriftLength) keeps x1.
        run(&mut e, &s, &drift, 30 + 48);
        assert_eq!(e.scorer.combo, 1);
        run(&mut e, &s, &straight, 30);
        // A third of 1.5 s raises the combo to x2.
        run(&mut e, &s, &drift, 30 + 90);
        assert_eq!(e.scorer.combo, 2);
        let pending = e.scorer.combo_points + e.scorer.current;
        assert_eq!(e.scorer.display, pending * 2.);
        run(&mut e, &s, &straight, 240);
        assert_eq!(e.scorer.total, pending * 2.);
        assert!(first < pending);
    }

    #[test]
    fn a_collision_loses_the_combo_once_per_cooldown() {
        // A short detection time lets a new combo start within the cooldown.
        let s = Settings {
            threshold_slide_time: 0.05,
            ..shipped()
        };
        let mut e = Entry::new(false, None, true);
        let drift = sample(30., 30., 0.5);
        let straight = sample(30., 0., 0.5);
        run(&mut e, &s, &drift, 90);
        assert!(e.scorer.display > 0.);
        // 399 is below the threshold; 401 fails the combo.
        let mut hit = drift;
        hit.impact = 399.;
        run(&mut e, &s, &hit, 1);
        assert!(e.scorer.combo == 1);
        hit.impact = 401.;
        run(&mut e, &s, &hit, 1);
        assert_eq!(
            (e.scorer.combo, e.scorer.display, e.scorer.total),
            (0, 0., 0.)
        );
        // The failed drift goes on but cannot raise the combo again (+0x29
        // stays set), so it shows nothing; its later points join the next
        // combo when it ends, as in the original.
        run(&mut e, &s, &drift, 10);
        assert_eq!((e.scorer.combo, e.scorer.display), (0, 0.));
        run(&mut e, &s, &straight, 1);
        assert!(e.scorer.combo_points > 0. && e.scorer.combo == 0);
        // A new drift opens a combo at once; a hit within the 0.5 s cooldown
        // after the failure does nothing.
        run(&mut e, &s, &drift, 6);
        assert_eq!(e.scorer.combo, 1);
        run(&mut e, &s, &hit, 1);
        assert_eq!(e.scorer.combo, 1);
        run(&mut e, &s, &drift, 30);
        run(&mut e, &s, &hit, 1);
        assert_eq!(e.scorer.combo, 0);
        assert_eq!(e.scorer.total, 0.);
    }

    #[test]
    fn going_backwards_or_before_the_line_never_scores() {
        let s = shipped();
        let mut e = Entry::new(false, None, true);
        run(&mut e, &s, &sample(30., 30., 0.), 120);
        assert_eq!(e.scorer.display, 0., "progress 0: before the line");
        let mut e = Entry::new(false, None, true);
        run(&mut e, &s, &sample(30., 30., 0.5), 1);
        run(&mut e, &s, &sample(30., 30., 0.4), 120);
        assert_eq!(e.scorer.display, 0., "behind the furthest progress");
        // A jump of more than half a lap restarts the furthest progress.
        run(&mut e, &s, &sample(30., 30., 1.2), 120);
        assert!(e.scorer.display > 0.);
    }

    #[test]
    fn close_drift_bonus_on_leaving_the_flag_while_drifting() {
        let s = shipped();
        let mut e = Entry::new(false, None, true);
        let flags = [[0., 0., 0.]];
        let mut x = sample(30., 30., 0.5);
        x.wheels = [[50., 0., 0.]; 4];
        for _ in 0..40 {
            e.update(&s, &flags, &x, DT);
        }
        assert!(e.scorer.was_drifting);
        // In: 6 m (0.5), then 4 m (0.25); out again.
        for d in [6., 4., 5.] {
            x.wheels = [[d, 0., 0.]; 4];
            e.update(&s, &flags, &x, DT);
        }
        let before = e.scorer.current;
        x.wheels = [[50., 0., 0.]; 4];
        e.update(&s, &flags, &x, DT);
        assert_eq!(e.scorer.close_drifts, 1);
        assert!((e.scorer.last_close_distance - 0.25).abs() < 1e-6);
        // int((1 - 0.25) * 100) * 0.01 * 20000 = 15000, plus any score step.
        assert!(e.scorer.current - before >= 15000.);
        assert!(e.scorer.current - before < 15100.);
        // Touching a flag disarms the bonus.
        for d in [6., 4.] {
            x.wheels = [[d, 0., 0.]; 4];
            e.update(&s, &flags, &x, DT);
        }
        e.detector.touch_flag();
        e.scorer.update(&s, &e.detector.clone(), DT);
        x.wheels = [[50., 0., 0.]; 4];
        e.update(&s, &flags, &x, DT);
        assert_eq!(e.scorer.close_drifts, 1);
    }

    #[test]
    fn battle_multiplier_follows_race_position_and_ai_bias() {
        let s = shipped();
        let mut leader = Entry::new(true, None, false);
        let mut last = Entry::new(true, None, false);
        let mut hard_ai = Entry::new(true, Some(5), false);
        let mut x = sample(30., 30., 0.5);
        x.cars = 8;
        for _ in 0..120 {
            x.position = 0;
            leader.update(&s, &[], &x, DT);
            hard_ai.update(&s, &[], &x, DT);
            x.position = 7;
            last.update(&s, &[], &x, DT);
        }
        assert_eq!(leader.detector.position_multiplier, 3.);
        assert_eq!(last.detector.position_multiplier, 1.);
        assert!(leader.scorer.display > 2.9 * last.scorer.display);
        // Difficulty 5 bias 4.0 on top of the leader's multiplier.
        assert!(hard_ai.scorer.display > 3.9 * leader.scorer.display);
        // Not a battle: no position multiplier.
        let mut gp = Entry::new(false, None, true);
        x.position = 0;
        for _ in 0..120 {
            gp.update(&s, &[], &x, DT);
        }
        assert_eq!(gp.scorer.display, last.scorer.display);
    }

    #[test]
    fn race_end_banks_scales_and_estimates_zero_scores() {
        use crate::virtual_race::{Format, VirtualRace, difficulty, drift_pool};
        let s = shipped();
        let pool = drift_pool(Format::DriftBattle, "tracks/usa/long_beach/route_1");
        let mut race = VirtualRace::new(pool, difficulty(2, 1), 3);
        let mut e = Entry::new(false, None, true);
        run(&mut e, &s, &sample(30., 30., 0.5), 120);
        let pending = e.scorer.display;
        // Half of a 2-lap race covered, still moving: score × 4.
        e.finish_race(&s, (false, 30., 2), &mut race, 1.);
        assert!(e.done && !e.estimated);
        assert_eq!(e.score(), (pending * 4.) as i64);
        // Finished cars are only banked; idle unfinished cars get no scaling
        // and, without points, the virtual race's estimate × laps.
        let mut f = Entry::new(false, None, true);
        run(&mut f, &s, &sample(30., 30., 0.5), 120);
        let p = f.scorer.display;
        f.finish_race(&s, (true, 30., 2), &mut race, 1.);
        assert_eq!(f.score(), p as i64);
        let mut idle = Entry::new(false, Some(2), false);
        run(&mut idle, &s, &sample(1., 0., 0.5), 10);
        let expected = race.clone().drift_score(0.5, 2);
        idle.finish_race(&s, (false, 1., 2), &mut race, 0.5);
        assert!(idle.estimated);
        assert_eq!(idle.score(), expected as i64);
        assert!(idle.scorer.display == expected && idle.scorer.total == expected);
        // 2 laps of 5,300,000 / 43 .. / 14 (Normal).
        assert!(
            (246_000..757_200).contains(&idle.score()),
            "{}",
            idle.score()
        );
        // Without base results the estimate is zero.
        let mut none = VirtualRace::new(None, difficulty(2, 1), 3);
        let mut zero = Entry::new(false, Some(2), false);
        zero.finish_race(&s, (false, 0., 2), &mut none, 1.);
        assert!(!zero.estimated && zero.score() == 0);
        // A second race end changes nothing; updates stop.
        e.finish_race(&s, (true, 0., 2), &mut race, 1.);
        run(&mut e, &s, &sample(30., 30., 0.9), 120);
        assert_eq!(e.score(), (pending * 4.) as i64);
        assert_eq!(classification(&[zero, e, f], &[0, 2, 1]), [1, 2, 0]);
    }

    #[test]
    fn settings_defaults_are_the_constructor_values() {
        let s = Settings::default();
        assert_eq!(
            (
                s.threshold_slide_angle,
                s.threshold_slide_time,
                s.combo_time
            ),
            (5., 0.2, 2.)
        );
        assert_eq!(
            (
                s.threshold_speed_enter,
                s.threshold_speed_exit,
                s.collision_threshold
            ),
            (40., 20., 150.)
        );
        assert_eq!(
            (s.battle_first_multiplier, s.battle_last_multiplier),
            (3., 1.)
        );
        assert_eq!(s.types.len(), 7);
        assert_eq!(s.types[6].bonus, 10000);
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT purchased source"]
    fn shipped_drift_xml_matches_the_decoded_values() {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let s = Settings::load(std::path::Path::new(&root)).unwrap();
        assert_eq!(s, shipped());
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT purchased source"]
    fn drift_routes_have_close_drift_flags() {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let root = std::path::Path::new(&root);
        for (route, at_least) in [
            ("tracks/usa/long_beach/route_1", 3),
            ("tracks/europe/nurburgring/route_1", 3),
            ("tracks/europe/istanbul_park/route_1", 3),
        ] {
            let flags = close_drift_flags(root, &crate::route::Route::new(route).unwrap()).unwrap();
            assert!(flags.len() >= at_least, "{route}: {}", flags.len());
            assert!(flags.iter().flatten().all(|v| v.is_finite()));
        }
        let gp = close_drift_flags(root, &crate::route::Route::default()).unwrap();
        assert!(gp.is_empty(), "Donington GP has no drift flags");
    }
}
