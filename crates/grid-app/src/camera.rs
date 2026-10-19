//! In-race camera views from the car's shipped `cameras.xml`
//! (docs/CAMERA_VIEWS.md). Presentation only: the views read the car pose and
//! velocity after the physics tick and never feed the simulation.
//!
//! - Change View cycles the visible views in file order (chase_close,
//!   chase_far, bumper, bonnet, head-cam); Look Back holds the view's
//!   `reverseCamera`.
//! - The chosen view persists like the original's profile key
//!   `current_camera` (default chase_close): restored at the start (a hidden
//!   view falls back to the first visible one, 0x582750) and saved at the end
//!   (a reverse view saves its owner, 0x584E70), here `<data dir>/camera.ini`.
//! - Chase (0x8435D0 → 0x81EC90 → 0x8114B0, 0x810E50): the eye is the
//!   file position, pushed back with speed (tiltFallback, gear drop-back),
//!   turned by the yaw lag about the car's Y and carried by the car frame,
//!   whose forward holds its pre-spin direction during a spin (impact
//!   envelope); its height drops with the car's pitch (kCameraCloseFollow*).
//!   It looks at the file target on the car, raised by the suspension heave,
//!   and rolls with the yaw rate. docs/CAMERA_VIEWS.md §2.
//!   Mounted: offset and yaw/pitch/roll on the body, with the
//!   acceleration-driven eye shift (0x822510). Head: seat position and tilt.
use bevy::prelude::*;
use grid_formats::vehicle_views::{self, Kind};
use std::path::{Path, PathBuf};

/// Speed at which the chase lag rate reaches lagMaximum (0xE4E0FC, 200 km/h).
const LAG_SPEED: f32 = f32::from_bits(0x425e_38ba);
/// Degrees to radians as the original multiplies (0xCB8200).
const DEGREES: f32 = f32::from_bits(0x3c8e_fa35);
/// Roll (0x800100): speed scale for minimum/maximumRollSpeed (0xE4DD94), the
/// yaw-rate gain 1/π (0xCE9608) · 1.4 (0xE4DD98) and the rate scale of
/// maxRollPerSecond (0xE4DD90).
const ROLL_SPEED_SCALE: f32 = 1.25;
const ROLL_PER_YAW_RATE: f32 = f32::from_bits(0x3ea2_f983);
const ROLL_GAIN: f32 = f32::from_bits(0x3fb3_3333);
const ROLL_RATE_SCALE: f32 = 0.5;
/// Suspension heave (0x81EC90): mix exponent per second (0xCB504C) and the
/// four-wheel average (0xCB01AC).
const SUSPENSION_MIX_RATE: f32 = 40.;
const SUSPENSION_AVERAGE: f32 = 0.25;
/// The eye height follows the car only from this speed on, once reached
/// (0x8002D0 with chase +0x170 = 0 and +0x174 = 5.0 from 0xCB093C, set by the
/// constructor 0x81E900; latched in record +0xE0).
const EYE_FOLLOW_SPEED: f32 = 5.;
/// Spin hold (impact envelope 1, 0x81EC90): angular acceleration threshold
/// (0xE4DD58), spin speed (0xE4DD5C), length (0xE4DD60), plateau bounds as
/// fractions of the length (0xE4DD68, 0xE4DD64), rise and fall exponents
/// (0xE4DD6C, 0xE4DD70) and frames of spinning that start it.
const SPIN_ACCELERATION: f32 = 18.;
const SPIN_SPEED: f32 = 2.5;
const SPIN_TIME: f32 = 2.;
const SPIN_HOLD_START: f32 = f32::from_bits(0x3f19_999a);
const SPIN_HOLD_END: f32 = f32::from_bits(0x3f66_6666);
const SPIN_RISE_POWER: f64 = 0.5;
const SPIN_FALL_POWER: f64 = 2.;
const SPIN_FRAMES: i32 = 3;
/// 0x7FF3E0: the held forward is turned in (cos below 0xCB0F30, angle scale
/// 0xE4E164) and a blended forward shorter than 0xCF1208 keeps the car frame.
const SPIN_TURN_COS: f32 = f32::from_bits(0x3f78_51ec);
const SPIN_MIN_FORWARD: f64 = 0.1;
/// Drift blend (0x8114B0): speed (0xCB093C) and rate (0xCAFCD4) of the slip
/// angle weight.
const DRIFT_SPEED: f32 = 5.;
const DRIFT_RATE: f32 = f32::from_bits(0x3dcc_cccd);
/// Mounted acceleration divisor (0xCAFDB8): Δv / (dt · 30).
const MOUNTED_ACCELERATION: f32 = 30.;
/// The original filters some states once per rendered frame; the port
/// applies those coefficients per 1/60 s so the result does not depend on the
/// frame rate (AUTHORED normalisation, docs/CAMERA_VIEWS.md).
const FRAME_RATE: f32 = 60.;
/// Default view of the original profile (0x5F1100: `current_camera`).
pub const DEFAULT_VIEW: &str = "chase_close";

#[derive(Clone, Debug, PartialEq)]
pub struct Chase {
    pub fov: f32,
    pub lag_minimum: f32,
    pub lag_maximum: f32,
    pub angular_limit: f32,
    pub angular_limit_damping: f32,
    pub position: Vec3,
    pub target: Vec3,
    pub follow_damp: f32,
    pub follow_y_scale: f32,
    pub follow_y_clamp: f32,
    /// Roll with the yaw rate (0x800100, applied by 0x810E50).
    pub roll_angle: f32,
    pub minimum_roll_speed: f32,
    pub maximum_roll_speed: f32,
    pub max_roll_per_second: f32,
    pub invert_roll: bool,
    /// Speed fall-back of the eye (0x800320): tiltFallback metres over
    /// minimum..maximumTiltSpeed.
    pub tilt_fallback: f32,
    pub minimum_tilt_speed: f32,
    pub maximum_tilt_speed: f32,
    /// Gear drop-back (0x800320): gearUpDropBack, its overall and initial time.
    pub gear_drop_back: f32,
    pub gear_drop_back_time: f32,
    pub gear_drop_back_initial: f32,
    /// Suspension heave of the target (0x81EC90, 0x8005D0).
    pub suspension_scale: f32,
    pub suspension_mix: f32,
    pub suspension_dead_zone: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Mounted {
    pub fov: f32,
    pub near: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
    pub offset: Vec3,
    pub axis_scale: Vec3,
    pub maximum_shift: f32,
    pub movement_speed: f32,
    pub final_movement_scale: f32,
    pub offset_only: bool,
    /// Tilt state gains (0x822510) and the twist of the view's up axis
    /// (0x803270): axisScaleForTilt, twistScale, tiltDistance.
    pub axis_scale_for_tilt: Vec3,
    pub twist_scale: Vec3,
    pub tilt_distance: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Head {
    pub fov: f32,
    pub near: f32,
    pub position: Vec3,
    /// Degrees; negative looks down [I].
    pub tilt: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Rig {
    Chase(Chase),
    Mounted(Mounted),
    Head(Head),
}

#[derive(Clone, Debug, PartialEq)]
pub struct View {
    pub ident: String,
    pub hidden: bool,
    pub reverse: Option<usize>,
    pub rig: Rig,
}

/// One car's views.
#[derive(Clone, Debug, PartialEq)]
pub struct Views(pub Vec<View>);

fn missing(view: &str, name: &str) -> String {
    format!("camera view {view} lacks parameter {name}")
}

impl Views {
    pub fn from_definition(def: &vehicle_views::Views) -> Result<Self, String> {
        let mut views = Vec::new();
        for v in &def.views {
            let s = |n: &str| v.scalar(n).ok_or_else(|| missing(&v.ident, n));
            let vec = |n: &str| {
                v.vector(n)
                    .map(Vec3::from_array)
                    .ok_or_else(|| missing(&v.ident, n))
            };
            let rig = match v.kind {
                Kind::Chase => Rig::Chase(Chase {
                    fov: s("fov")?,
                    lag_minimum: s("lagMinimum")?,
                    lag_maximum: s("lagMaximum")?,
                    angular_limit: s("angularLimit")?,
                    angular_limit_damping: s("angularLimitDamping")?,
                    position: vec("position")?,
                    target: vec("target")?,
                    follow_damp: s("kCameraCloseFollowForwardsRollDamp")?,
                    follow_y_scale: s("kCameraCloseFollowYScale")?,
                    follow_y_clamp: s("kCameraCloseFollowYClamp")?,
                    roll_angle: s("rollAngle")?,
                    minimum_roll_speed: s("minimumRollSpeed")?,
                    maximum_roll_speed: s("maximumRollSpeed")?,
                    max_roll_per_second: s("maxRollPerSecond")?,
                    invert_roll: v.flag("invertRoll").ok_or_else(|| missing(&v.ident, "invertRoll"))?,
                    tilt_fallback: s("tiltFallback")?,
                    minimum_tilt_speed: s("minimumTiltSpeed")?,
                    maximum_tilt_speed: s("maximumTiltSpeed")?,
                    gear_drop_back: s("gearUpDropBack")?,
                    gear_drop_back_time: s("gearUpDropBackOverallTime")?,
                    gear_drop_back_initial: s("gearUpDropBackInitialTime")?,
                    suspension_scale: s("suspScale")?,
                    suspension_mix: s("suspMixScale")?,
                    suspension_dead_zone: s("suspDeadZone")?,
                }),
                Kind::Mounted => Rig::Mounted(Mounted {
                    fov: s("fov")?,
                    near: s("nearPlane")?,
                    yaw: s("yaw")?,
                    pitch: s("pitch")?,
                    roll: s("roll")?,
                    offset: vec("offset")?,
                    axis_scale: vec("axisScale")?,
                    maximum_shift: s("maximumShift")?,
                    movement_speed: s("movementSpeed")?,
                    final_movement_scale: s("finalMovementScale")?,
                    offset_only: v.flag("offsetOnly").unwrap_or(false),
                    axis_scale_for_tilt: vec("axisScaleForTilt")?,
                    twist_scale: vec("twistScale")?,
                    tilt_distance: s("tiltDistance")?,
                }),
                Kind::Head => Rig::Head(Head {
                    fov: s("fov")?,
                    near: s("nearPlane")?,
                    position: vec("position")?,
                    tilt: s("tilt")?,
                }),
            };
            views.push(View {
                ident: v.ident.clone(),
                hidden: v.hidden,
                reverse: None,
                rig,
            });
        }
        for (i, v) in def.views.iter().enumerate() {
            if let Some(name) = &v.reverse {
                views[i].reverse = Some(
                    def.views
                        .iter()
                        .position(|r| &r.ident == name)
                        .ok_or_else(|| format!("reverse camera {name} is not defined"))?,
                );
            }
        }
        if !views.iter().any(|v| !v.hidden) {
            return Err("camera definition has no visible view".into());
        }
        Ok(Self(views))
    }

    pub fn load(root: &Path, stem: &str) -> Result<Self, String> {
        let path = root.join(format!("cars/{stem}/cameras.xml"));
        let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let def = vehicle_views::read(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::from_definition(&def)
    }

    /// The prototype's earlier authored chase, used only when the car's
    /// camera file cannot be read.
    pub fn fallback() -> Self {
        let chase = |position: Vec3, target: Vec3| {
            Rig::Chase(Chase {
                fov: 45.,
                lag_minimum: 0.,
                lag_maximum: 0.,
                angular_limit: 10.,
                angular_limit_damping: 1.,
                position,
                target,
                follow_damp: 0.,
                follow_y_scale: 0.,
                follow_y_clamp: 0.,
                roll_angle: 0.,
                minimum_roll_speed: 0.,
                maximum_roll_speed: 0.,
                max_roll_per_second: 0.,
                invert_roll: false,
                tilt_fallback: 0.,
                minimum_tilt_speed: 0.,
                maximum_tilt_speed: 0.,
                gear_drop_back: 0.,
                gear_drop_back_time: 0.,
                gear_drop_back_initial: 0.,
                suspension_scale: 0.,
                suspension_mix: 0.,
                suspension_dead_zone: 0.,
            })
        };
        Self(vec![
            View {
                ident: DEFAULT_VIEW.into(),
                hidden: false,
                reverse: Some(1),
                rig: chase(Vec3::new(0., 3., -9.), Vec3::Y),
            },
            View {
                ident: format!("{DEFAULT_VIEW}_reverse"),
                hidden: true,
                reverse: None,
                rig: chase(Vec3::new(0., 3., 9.), Vec3::Y),
            },
        ])
    }

    fn first_visible(&self) -> usize {
        self.0.iter().position(|v| !v.hidden).unwrap_or(0)
    }

    /// 0x582750: the saved view by name; hidden or unknown falls back to the
    /// first visible view.
    pub fn restore(&self, ident: &str) -> usize {
        match self.0.iter().position(|v| v.ident == ident) {
            Some(i) if !self.0[i].hidden => i,
            _ => self.first_visible(),
        }
    }

    /// Change View: the next visible view in file order, wrapping [I order].
    pub fn next(&self, current: usize) -> usize {
        let n = self.0.len();
        (1..=n)
            .map(|k| (current + k) % n)
            .find(|&i| !self.0[i].hidden)
            .unwrap_or(current)
    }

    /// 0x584E70: the name saved for the current view; a hidden view saves
    /// the visible view whose reverse camera it is.
    pub fn saved(&self, current: usize) -> &str {
        if !self.0[current].hidden {
            return &self.0[current].ident;
        }
        self.0
            .iter()
            .find(|v| !v.hidden && v.reverse == Some(current))
            .map_or(&self.0[current].ident, |v| &v.ident)
    }
}

/// The car state a view reads, after the tick: what the original's vehicle
/// camera target (vtable 0xCFF640) answers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    /// Body to world (+0x18).
    pub pose: Mat4,
    /// World velocity, m/s (+0xC).
    pub velocity: Vec3,
    /// World angular velocity, rad/s (+0x10).
    pub angular_velocity: Vec3,
    /// The car's speed word, car +0x1648 (+8).
    pub speed: f32,
    /// Per wheel: suspension displacement (wheel +0x40) and travel clip
    /// (wheel +0x4C) (+0x24, 0x7F7330).
    pub wheels: [(f32, f32); 4],
    /// Car +0xD234 is not 7, the controls are disabled (+0x28, 0x7F7380).
    pub controls_disabled: bool,
    /// Committed physics ticks and the reset counter; camera time is the
    /// simulation time between samples, so pause and replays hold still.
    pub ticks: u64,
    pub segment: u64,
}

impl Sample {
    /// A car known only by pose and velocity (replays): speed |v|, no
    /// rotation, suspension at rest, controls enabled.
    pub fn moving(pose: Mat4, velocity: Vec3, ticks: u64, segment: u64) -> Self {
        Self {
            pose,
            velocity,
            angular_velocity: Vec3::ZERO,
            speed: velocity.length(),
            wheels: [(0., 1.); 4],
            controls_disabled: false,
            ticks,
            segment,
        }
    }

    /// The player's car after the tick, posed in its model frame.
    pub fn of(runtime: &grid_tools::source_vehicle::Runtime, segment: u64) -> Self {
        let b = &runtime.state.car.owner.vehicle;
        let f = |o: usize| f32::from_bits(b[o / 4]);
        let [vx, vy, vz, _] = runtime.velocity();
        let [wx, wy, wz, _] = runtime.angular_velocity();
        let rows = std::array::from_fn(|r| Vec3::new(f(0x10 + 16 * r), f(0x14 + 16 * r), f(0x18 + 16 * r)));
        Self {
            pose: model_frame(
                Mat4::from_cols_array(&runtime.pose()),
                rows,
                Vec3::new(f(0x1630), f(0x1634), f(0x1638)),
            ),
            velocity: Vec3::new(vx, vy, vz),
            angular_velocity: Vec3::new(wx, wy, wz),
            speed: f(0x1648),
            wheels: std::array::from_fn(|i| {
                let w = 0x1990 + i * 0x150;
                (f(w + 0x40), f(w + 0x4c))
            }),
            controls_disabled: b[0xd234 / 4] != 7,
            ticks: runtime.ticks,
            segment,
        }
    }
}

/// The car's model frame, which the camera target answers (0x9AE3A0; target
/// +0x14/+0x18/+0x1C): the physics pose (car +0xDB0 → +8, body +0x1D0)
/// rotated by the car's 3×3 at +0x10 (rows, nearly the identity) with its
/// origin moved by that rotation times car +0x1630 (the Lacetti's
/// (0, −0.39144, −0.14642): the model origin lies below and behind the body
/// origin). A rotation that is not one keeps the pose's rotation.
pub fn model_frame(pose: Mat4, rows: [Vec3; 3], offset: Vec3) -> Mat4 {
    let local = Mat3::from_cols(rows[0], rows[1], rows[2]);
    let local = if local.is_finite() && (local.determinant() - 1.).abs() < 1e-3 { local } else { Mat3::IDENTITY };
    // Row vectors: pose rows × the +0x10 rows; the translation is not turned.
    let rotation = local * Mat3::from_mat4(pose);
    let origin = pose.w_axis.truncate() + rotation * offset;
    Mat4::from_cols(
        rotation.x_axis.extend(0.),
        rotation.y_axis.extend(0.),
        rotation.z_axis.extend(0.),
        origin.extend(1.),
    )
}

/// Spin hold (impact envelope 1, 0x81EC90 with 0x7FF3E0).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Spin {
    /// Envelope time left (record +0x120), seconds.
    pub timer: f32,
    /// Weight of the held forward (chase +0x220).
    pub blend: f32,
    /// Forward held from the start of the spin (chase +0x230).
    pub held: Vec3,
    /// Last spin speed (record +0x124).
    speed: Option<f32>,
    /// Frames left to confirm a jump (record +0x148).
    armed: i32,
    /// Consecutive frames above the spin speed (record +0x14C).
    spinning: i32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ChaseState {
    /// Yaw lag angle (rad, record +0x94): the eye trails the car by this much.
    pub lag: f32,
    /// Filtered forward slope for the close-follow height (record +0x98).
    pub follow: f32,
    /// Rate-limited roll (record +0x9C) and its speed weight (+0xA0).
    pub roll: f32,
    pub roll_weight: f32,
    /// Filtered suspension heave (chase +0x190, record +0xC0).
    pub suspension: f32,
    /// Gear drop-back envelope (record +0x90).
    pub drop_back: f32,
    pub spin: Spin,
    /// Eye height of the last update (record +0xB4) and the latch of its
    /// speed blend (record +0xE0).
    eye_height: Option<f32>,
    height_latched: bool,
    /// The first update after a reset sets the follow state directly
    /// (record +0xAC).
    started: bool,
    heading: Option<f32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MountedState {
    /// Eye shift (record +0x80) and tilt state (record +0x70).
    pub shift: Vec3,
    pub tilt: Vec3,
    velocity: Option<Vec3>,
}

/// Filter histories, one per view, reset on a car reset or a view change.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct History {
    pub chase: ChaseState,
    pub mounted: MountedState,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shot {
    pub eye: Vec3,
    pub rotation: Quat,
    /// Vertical field of view, radians [I convention].
    pub fov: f32,
    pub near: Option<f32>,
}

fn wrap(a: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    if a < -PI {
        a + TAU
    } else if PI < a {
        a - TAU
    } else {
        a
    }
}

/// Car heading (rad) about world +Y; 0 when the car faces +Z.
fn heading(pose: &Mat4) -> f32 {
    let f = pose.z_axis;
    f.x.atan2(f.z)
}

/// One step of the chase yaw lag (0x8114B0): integrate the yaw change, limit
/// the angle with angularLimit/Damping, wrap to ±π, then return toward zero
/// at |lag| · (lagMinimum + (lagMaximum − lagMinimum) · speed / 55.5554); a
/// return that overtakes the yaw rate and crosses zero stops at zero.
pub fn chase_lag(c: &Chase, lag: f32, turn: f32, speed: f32, dt: f32) -> f32 {
    let mut lag = lag + turn;
    let damping = c.angular_limit_damping;
    if lag < -c.angular_limit {
        lag = -c.angular_limit * damping + lag * (1. - damping);
    }
    if c.angular_limit < lag {
        lag = c.angular_limit * damping + lag * (1. - damping);
    }
    lag = wrap(lag);
    let rate = lag.abs() * ((c.lag_maximum - c.lag_minimum) * (speed / LAG_SPEED) + c.lag_minimum);
    let yaw_rate = if dt > 0. { (turn / dt).abs() } else { 0. };
    if lag > 0. {
        lag -= dt * rate;
        if yaw_rate < rate && lag < 0. {
            lag = 0.;
        }
    } else if lag < 0. {
        lag += dt * rate;
        if yaw_rate < rate && lag > 0. {
            lag = 0.;
        }
    }
    lag
}

/// Per-frame coefficient `k` applied over `dt` seconds at 60 Hz.
fn per_frame(k: f32, dt: f32) -> f32 {
    1. - (1. - k.clamp(0., 1.)).powf(dt * FRAME_RATE)
}

/// The original's double-precision `pow` (0xBF53E5) on f32 operands.
fn pow(x: f32, y: f64) -> f32 {
    f64::from(x).powf(y) as f32
}

/// `(x − low) / (high − low)` limited to 0..1 as 0x800320/0x8002D0 do (NaN
/// gives 0).
fn ramp(x: f32, low: f32, high: f32) -> f32 {
    let t = (x - low) / (high - low);
    if 0. <= t { t.min(1.) } else { 0. }
}

/// Suspension heave (0x81EC90): each wheel's displacement beyond
/// suspDeadZone · travel, shrunk toward zero by that margin over
/// (1 − suspDeadZone), averaged and mixed with the previous value by
/// m = suspMixScale^(dt · 40). The target rises by suspScale times it
/// (0x8005D0).
pub fn suspension_heave(c: &Chase, previous: f32, wheels: &[(f32, f32); 4], dt: f32) -> f32 {
    let mut sum = 0.;
    for &(displacement, travel) in wheels {
        let margin = c.suspension_dead_zone * travel;
        if margin < displacement.abs() {
            let margin = margin / (1. - c.suspension_dead_zone);
            sum = if displacement <= 0. {
                margin + displacement + sum
            } else {
                (displacement - margin) + sum
            };
        }
    }
    let m = pow(c.suspension_mix, f64::from(dt * SUSPENSION_MIX_RATE));
    sum * SUSPENSION_AVERAGE * m + (1. - m) * previous
}

/// Gear-ratio weight of the drop-back by gear index (0x800320; 9 is reverse).
fn gear_weight(gear: u32) -> f32 {
    match gear {
        1 | 9 => 1.,
        2 => f32::from_bits(0x3f93_3333),
        3 => f32::from_bits(0x3fb9_db24),
        4 => f32::from_bits(0x3fe9_78d5),
        5 => f32::from_bits(0x4006_6667),
        _ => f32::from_bits(0x401a_5e36),
    }
}

/// How far the eye falls back along the car's −Z (0x800320): tiltFallback
/// times the speed ramp minimumTiltSpeed..maximumTiltSpeed, less the gear
/// drop-back after an upshift. `shift` is the gear and the time since the
/// shift (car +0x2E4); without it the car counts as never shifting. Updates
/// the drop-back envelope `drop` (record +0x90).
pub fn fall_back(c: &Chase, speed: f32, shift: Option<(u32, f32)>, drop: &mut f32) -> f32 {
    use std::f64::consts::PI;
    let full = c.tilt_fallback * ramp(speed, c.minimum_tilt_speed, c.maximum_tilt_speed);
    let (time, weight) = shift.map_or((f32::MAX, 1.), |(gear, time)| (time, gear_weight(gear)));
    let mut envelope = 0.;
    if c.gear_drop_back_initial <= time {
        if time < c.gear_drop_back_time {
            let x = (time - c.gear_drop_back_initial) / (c.gear_drop_back_time - c.gear_drop_back_initial);
            envelope = ((f64::from(x * PI as f32)).cos() as f32 + 1.) * 0.5;
        }
    } else {
        let rise = 0.5 - (f64::from(time / c.gear_drop_back_initial * PI as f32)).cos() as f32 * 0.5;
        envelope = if *drop <= rise { rise } else { *drop };
    }
    *drop = envelope;
    let amount = (c.gear_drop_back * c.tilt_fallback * weight).min(full);
    if c.tilt_fallback < 0. {
        envelope = 0.;
    }
    full - amount * envelope
}

/// One step of the roll (0x800100): the target is the yaw rate · (1/π) ·
/// rollAngle · 1.4, approached at maxRollPerSecond · 0.5 per second; the
/// weight ramps speed · 1.25 over minimum..maximumRollSpeed (1 when the range
/// is empty). Returns the new roll and weight.
pub fn roll_step(c: &Chase, roll: f32, yaw_rate: f32, speed: f32, dt: f32) -> (f32, f32) {
    let x = speed * ROLL_SPEED_SCALE;
    let mut clamped = c.minimum_roll_speed;
    if c.minimum_roll_speed <= x {
        clamped = x.min(c.maximum_roll_speed);
    }
    let weight = if c.minimum_roll_speed < c.maximum_roll_speed {
        (clamped - c.minimum_roll_speed) / (c.maximum_roll_speed - c.minimum_roll_speed)
    } else {
        1.
    };
    let goal = yaw_rate * ROLL_PER_YAW_RATE * c.roll_angle * ROLL_GAIN;
    let step = c.max_roll_per_second * ROLL_RATE_SCALE * dt;
    let difference = goal - roll;
    let next = if step < difference.abs() {
        let sign = if difference < 0. { -1. } else if 0. < difference { 1. } else { 0. };
        sign * step + roll
    } else {
        goal
    };
    (next, weight)
}

/// The view's roll angle about its direction (0x810E50): roll · weight in
/// degrees, positive with invertRoll set. Unlike the unused roll in 0x8114B0
/// it does not depend on cameraRollEnabled.
fn roll_angle(c: &Chase, h: &ChaseState) -> f32 {
    h.roll * if c.invert_roll { DEGREES } else { -DEGREES } * h.roll_weight
}

/// The up vector for a view along `direction` rolled by `angle` (0x7FE8B0):
/// cos · normalize(d × h) + sin · h with h = normalize((0, 1, 0) × d).
fn rolled_up(direction: Vec3, angle: f32) -> Vec3 {
    let h = Vec3::Y.cross(direction);
    let h = if h.length() > 0. { h / h.length() } else { Vec3::X };
    direction.cross(h).normalize() * (f64::from(angle).cos() as f32) + h * (f64::from(angle).sin() as f32)
}

/// Drift blend (0x8114B0): with a positive weight the lag angle moves toward
/// minus the slip angle atan2(v·X, v·Z), weighted by (|v| − 5) · 0.1 in 0..1.
/// The weight is read from an object behind the target (+0xC → +0x24,
/// +0xE78 + 0x168) whose source is not identified [I]; races pass 0.
pub fn drift_lag(lag: f32, velocity: Vec3, pose: &Mat4, weight: f32) -> f32 {
    if weight.is_nan() || weight <= 0. {
        return lag;
    }
    let t = (velocity.length() - DRIFT_SPEED) * DRIFT_RATE;
    let t = if t < 0. { 0. } else { t.min(1.) };
    let x = velocity.dot(pose.x_axis.truncate());
    let z = velocity.dot(pose.z_axis.truncate());
    let slip = f64::from(x).atan2(f64::from(z)) as f32;
    ((-0. - slip) - lag) * t * weight + lag
}

/// The chase's car frame (0x7FF3E0): the car pose with its forward turned
/// toward the held forward by `blend` (a straight blend when they are within
/// acos 0.97), rebuilt about the car's up. A degenerate forward keeps the
/// car pose.
pub fn spin_frame(pose: &Mat4, blend: f32, held: Vec3) -> Mat4 {
    let f = pose.z_axis.truncate();
    let up = pose.y_axis.truncate();
    let mut forward = held * blend + f * (1. - blend);
    let dot = f.dot(held);
    if dot < SPIN_TURN_COS {
        let angle = f64::from(dot.clamp(-1., 1.)).acos() as f32;
        // [I] The turn is toward the held forward (it reaches it at blend 1).
        forward = Quat::from_axis_angle(f.cross(held).normalize(), angle * blend) * f;
    }
    let length = f64::from(forward.length());
    if length.is_nan() || length <= SPIN_MIN_FORWARD {
        return *pose;
    }
    let z = forward.normalize();
    let x = up.cross(z).normalize();
    let y = z.cross(x).normalize();
    Mat4::from_cols(x.extend(0.), y.extend(0.), z.extend(0.), pose.w_axis)
}

/// One frame of the spin hold (0x81EC90). The spin speed is |ω| with half of
/// its components about the car's X and Z removed. A jump above 18 rad/s²
/// arms three frames; three frames above 2.5 rad/s within them start a 2 s
/// envelope holding the frame's current forward `forward`: rise
/// √(1 − (t/2 − 0.9)/0.1) over 0.2 s, a plateau to t = 1.2 s that restarts
/// while the spin goes on (and holds while the controls are disabled), then a
/// quadratic ease to 0 over 1.2 s. Frames are camera updates [AUTHORED: only
/// those that advance the simulation].
pub fn spin_step(s: &mut Spin, sample: &Sample, forward: Vec3, dt: f32) {
    let x = sample.pose.x_axis.truncate();
    let z = sample.pose.z_axis.truncate();
    let mut w = sample.angular_velocity;
    w -= x * (w.dot(x) * 0.5);
    w -= z * (w.dot(z) * 0.5);
    let speed = w.length();
    let mut acceleration = (speed - s.speed.unwrap_or(speed)).abs();
    if 0. < dt {
        acceleration /= dt;
    }
    s.speed = Some(speed);
    if SPIN_ACCELERATION < acceleration {
        if s.armed == 0 {
            s.spinning = 0;
        }
        s.armed = SPIN_FRAMES;
    }
    if speed <= SPIN_SPEED {
        s.spinning = 0;
    } else {
        s.spinning += 1;
    }
    let (start, end) = (SPIN_HOLD_START * SPIN_TIME, SPIN_HOLD_END * SPIN_TIME);
    if s.armed < 1 {
        if start < s.timer && s.timer < end {
            if SPIN_FRAMES <= s.spinning {
                s.timer = end;
            } else if !sample.controls_disabled {
                s.timer = start;
            }
        }
    } else if s.spinning < SPIN_FRAMES {
        s.armed -= 1;
    } else {
        s.spinning = 0;
        s.armed = 0;
        if 0. < s.timer {
            if start < s.timer && s.timer < end {
                s.timer = end;
            }
        } else {
            s.timer = SPIN_TIME;
            s.held = forward;
        }
    }
    let t = s.timer;
    s.blend = if end < t {
        pow(1. - (t / SPIN_TIME - SPIN_HOLD_END) / (1. - SPIN_HOLD_END), SPIN_RISE_POWER)
    } else if start < t {
        if sample.controls_disabled {
            s.timer += dt;
        }
        1.
    } else if 0. < t {
        let f = (t / SPIN_TIME) / SPIN_HOLD_START;
        if 0.5 <= f {
            1. - pow((1. - f) * 2., SPIN_FALL_POWER) * 0.5
        } else {
            pow(f * 2., SPIN_FALL_POWER) * 0.5
        }
    } else {
        0.
    };
    s.timer -= dt;
}

/// Bevy cameras look along -Z; the car faces +Z.
fn facing_forward() -> Quat {
    Quat::from_rotation_y(std::f32::consts::PI)
}

/// The chase view (0x81EC90 → 0x8114B0, 0x810E50). Per update, in the
/// original's order: suspension heave; yaw lag (and drift blend); the car frame
/// with the held forward of the spin envelope (from the previous update); the
/// eye at the file position less the fall-back, turned by the lag about the
/// car's Y and carried by that frame, its height car + position.y − yScale ·
/// follow (blended with the previous height until 5 m/s is first reached);
/// then the spin envelope; the view looks at the file target (raised by the
/// heave) on the car and rolls with the yaw rate.
fn chase_shot(c: &Chase, h: &mut ChaseState, sample: &Sample, dt: f32, orbit: impl Fn(Vec3) -> Vec3) -> Shot {
    let pose = &sample.pose;
    let yaw = heading(pose);
    let turn = h.heading.map_or(0., |prev| wrap(yaw - prev));
    h.heading = Some(yaw);
    // Record reset (0x831D20): the spin speed starts at |ω|.
    h.spin.speed.get_or_insert(sample.angular_velocity.length());
    let advance = dt > 0.;
    if advance {
        h.suspension = suspension_heave(c, h.suspension, &sample.wheels, dt);
        h.lag = chase_lag(c, h.lag, turn, sample.speed, dt);
        (h.roll, h.roll_weight) = roll_step(c, h.roll, sample.angular_velocity.y, sample.speed, dt);
    }
    let lag = drift_lag(h.lag, sample.velocity, pose, 0.);
    let frame = spin_frame(pose, h.spin.blend, h.spin.held);
    let back = if advance {
        fall_back(c, sample.speed, None, &mut h.drop_back)
    } else {
        let mut drop = h.drop_back;
        fall_back(c, sample.speed, None, &mut drop)
    };
    let local = Quat::from_rotation_y(-lag) * (c.position - Vec3::Z * back);
    // The slope seen from the camera's side of the car.
    let slope = frame.z_axis.y * if 0. < local.z { -1. } else { 1. };
    let slope = slope.clamp(-c.follow_y_clamp, c.follow_y_clamp);
    if advance {
        if h.started {
            h.follow += (slope - h.follow) * per_frame(c.follow_damp, dt);
        } else {
            h.follow = slope;
        }
    }
    let raw = frame.w_axis.y + local.y - c.follow_y_scale * h.follow;
    let weight = if h.height_latched { 1. } else { ramp(sample.speed, 0., EYE_FOLLOW_SPEED) };
    let height = raw * weight + (1. - weight) * h.eye_height.unwrap_or(raw);
    let mut eye = frame.transform_point3(local);
    eye.y = height;
    // The first frame keeps its height as the previous one (record +0xB4 is
    // the eye before the reset in the original).
    if advance || h.eye_height.is_none() {
        h.eye_height = Some(height);
    }
    if advance {
        h.height_latched |= weight == 1.;
        spin_step(&mut h.spin, sample, frame.z_axis.truncate(), dt);
        h.started = true;
    }
    let target_local = c.target + Vec3::Y * (c.suspension_scale * h.suspension);
    let target = pose.transform_point3(target_local);
    // AUTHORED free look: orbit the eye about the target in the car frame.
    let offset = frame.inverse().transform_vector3(eye - target);
    eye += frame.transform_vector3(orbit(offset) - offset);
    let direction = (target - eye).normalize();
    Shot {
        eye,
        rotation: Transform::from_translation(eye)
            .looking_to(direction, rolled_up(direction, roll_angle(c, h)))
            .rotation,
        fov: c.fov.to_radians(),
        near: None,
    }
}

/// The Mounted view's twist frame (0x803270, any twistScale component non-zero;
/// emulated): the up axis leans to L = (T.x · tw.x, T.y · tw.y + tiltDistance,
/// T.z · tw.z) of the tilt state T, whose x is replaced by
/// clamp(−ω_y · 0.2 · 0.5, ±1) (0xCB870C, 0xCAF88C) by the weight
/// 1 − clamp(0.05 · |v|, 0, 1) (0xCAFE60). The frame is
/// (U × Z, U, (U × Z) × U); the original leaves its rows unnormalised, the port
/// normalises them [AUTHORED, second order in the lean].
fn mounted_twist(m: &Mounted, h: &MountedState, sample: &Sample) -> Quat {
    if m.twist_scale == Vec3::ZERO {
        return Quat::IDENTITY;
    }
    let s = (sample.velocity.length() * 0.05).clamp(0., 1.);
    let c = (-sample.angular_velocity.y * 0.2 * 0.5).clamp(-1., 1.);
    let tx = c * (1. - s) + h.tilt.x * s;
    let tw = m.twist_scale;
    let lean = Vec3::new(tx * tw.x, h.tilt.y * tw.y + m.tilt_distance, h.tilt.z * tw.z);
    let up = if lean.length_squared() > 0. { lean.normalize() } else { Vec3::Y };
    let right = up.cross(Vec3::Z);
    let forward = right.cross(up);
    Quat::from_mat3(&Mat3::from_cols(right.normalize(), up, forward.normalize()))
}

/// Advances `history` by the simulation time `dt` and frames the view.
/// `orbit` rotates a chase eye about its target (AUTHORED free look).
pub fn shot(
    rig: &Rig,
    history: &mut History,
    sample: &Sample,
    dt: f32,
    orbit: impl Fn(Vec3) -> Vec3,
) -> Shot {
    let (_, rotation, position) = sample.pose.to_scale_rotation_translation();
    match rig {
        Rig::Chase(c) => chase_shot(c, &mut history.chase, sample, dt, orbit),
        Rig::Mounted(m) => {
            let h = &mut history.mounted;
            if m.movement_speed != 0. && dt > 0. {
                let change = h.velocity.map_or(Vec3::ZERO, |v| sample.velocity - v);
                let local = rotation.inverse() * change / (dt * MOUNTED_ACCELERATION);
                let limit = |v: Vec3| v.clamp(Vec3::splat(-m.maximum_shift), Vec3::splat(m.maximum_shift));
                let goal = limit(-local * m.axis_scale);
                let tilt = limit(-local * m.axis_scale_for_tilt);
                h.shift += m.movement_speed * (goal - h.shift) * dt;
                h.tilt += (tilt - h.tilt) * m.movement_speed * 2. * dt;
            }
            h.velocity = Some(sample.velocity);
            // offsetOnly places the bare offset at the car's position.
            let eye = if m.offset_only {
                position + m.offset
            } else {
                position + rotation * (m.offset + h.shift * m.final_movement_scale)
            };
            // Positive pitch looks down, positive yaw turns toward +X, yaw π
            // looks back (0x803270: R(roll) · P(pitch) · Y(yaw), emulated).
            let view = Quat::from_rotation_y(m.yaw)
                * facing_forward()
                * Quat::from_rotation_x(-m.pitch)
                * Quat::from_rotation_z(m.roll);
            Shot {
                eye,
                rotation: rotation * mounted_twist(m, h, sample) * view,
                fov: m.fov.to_radians(),
                near: Some(m.near),
            }
        }
        Rig::Head(c) => Shot {
            eye: position + rotation * c.position,
            rotation: rotation * facing_forward() * Quat::from_rotation_x(c.tilt.to_radians()),
            fov: c.fov.to_radians(),
            near: Some(c.near),
        },
    }
}

/// The current view and its filter history.
#[derive(Resource, Clone, Debug)]
pub struct Cameras {
    pub views: Views,
    pub current: usize,
    /// Which view the history belongs to (the reverse view while looking back).
    shown: Option<usize>,
    history: History,
    last: Option<(u64, u64)>,
    /// Where the chosen view is saved at exit; None in tests.
    profile: Option<PathBuf>,
}

impl Cameras {
    pub fn new(views: Views, saved: &str, profile: Option<PathBuf>) -> Self {
        let current = views.restore(saved);
        Self {
            views,
            current,
            shown: None,
            history: History::default(),
            last: None,
            profile,
        }
    }

    pub fn change_view(&mut self) {
        self.current = self.views.next(self.current);
    }

    pub fn ident(&self) -> &str {
        &self.views.0[self.current].ident
    }

    /// Audio camera of the view on screen (4139F0, docs/AUDIO_RESEARCH.md).
    pub fn audio_camera(&self) -> grid_tools::audio_mix::Camera {
        let rig = &self.views.0[self.shown.unwrap_or(self.current)].rig;
        grid_tools::audio_mix::Camera::of_view(match rig {
            Rig::Chase(_) => Kind::Chase,
            Rig::Mounted(_) => Kind::Mounted,
            Rig::Head(_) => Kind::Head,
        })
    }

    /// Frames the current (or, looking back, its reverse) view for `sample`.
    pub fn frame(&mut self, sample: &Sample, back: bool, orbit: impl Fn(Vec3) -> Vec3) -> Shot {
        let view = &self.views.0[self.current];
        let shown = if back {
            view.reverse.unwrap_or(self.current)
        } else {
            self.current
        };
        let reset = self.shown != Some(shown)
            || self.last.is_none_or(|(segment, ticks)| {
                segment != sample.segment || sample.ticks < ticks
            });
        if reset {
            self.history = History::default();
            self.shown = Some(shown);
            self.last = Some((sample.segment, sample.ticks));
        }
        let ticks = self.last.map_or(0, |(_, t)| sample.ticks - t);
        self.last = Some((sample.segment, sample.ticks));
        let dt = ticks as f32 * grid_tools::source_vehicle::DELTA;
        let shot = shot(&self.views.0[shown].rig, &mut self.history, sample, dt, orbit);
        if reset && cfg!(not(test)) {
            println!(
                "drive_camera event=start view={} tick={} eye={:?} model_origin={:?}",
                self.views.0[shown].ident,
                sample.ticks,
                shot.eye.to_array(),
                sample.pose.w_axis.truncate().to_array()
            );
        }
        shot
    }

    /// Writes the original's saved name (0x584E70) to the profile file.
    pub fn save(&self) -> std::io::Result<()> {
        let Some(path) = &self.profile else {
            return Ok(());
        };
        std::fs::write(
            path,
            format!(
                "# Last in-race camera view (the original's profile key current_camera).\ncurrent_camera = {}\n",
                self.views.saved(self.current)
            ),
        )
    }
}

/// `current_camera = IDENT` from the profile text; the default otherwise.
pub fn parse_profile(text: &str) -> &str {
    text.lines()
        .filter_map(|l| l.split_once('='))
        .find(|(k, _)| k.trim() == "current_camera")
        .map_or(DEFAULT_VIEW, |(_, v)| v.trim())
}

pub fn profile_path() -> PathBuf {
    crate::settings::data_dir().join("camera.ini")
}

/// Loads the car's views and the saved choice; the car file failing keeps the
/// earlier authored chase with a log line.
pub fn install(app: &mut App, root: &Path, stem: &str) {
    let path = profile_path();
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let views = Views::load(root, stem).unwrap_or_else(|error| {
        println!("drive_camera event=fallback error={error:?}");
        Views::fallback()
    });
    let cameras = Cameras::new(views, parse_profile(&text), Some(path));
    println!(
        "drive_camera event=restore view={} views={}",
        cameras.ident(),
        cameras
            .views
            .0
            .iter()
            .filter(|v| !v.hidden)
            .map(|v| v.ident.as_str())
            .collect::<Vec<_>>()
            .join(",")
    );
    app.insert_resource(cameras).add_systems(Last, save_on_exit);
}

fn save_on_exit(cameras: Res<Cameras>, mut exits: MessageReader<AppExit>) {
    if exits.read().next().is_some() {
        match cameras.save() {
            Ok(()) => println!("drive_camera event=save view={}", cameras.views.saved(cameras.current)),
            Err(error) => println!("drive_camera event=save-failed error={error}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chase(position: Vec3, target: Vec3) -> Chase {
        Chase {
            fov: 52.,
            lag_minimum: 2.7507,
            lag_maximum: 4.5133,
            angular_limit: 10.,
            angular_limit_damping: 1.,
            position,
            target,
            follow_damp: 0.08,
            follow_y_scale: 5.,
            follow_y_clamp: 0.58,
            roll_angle: 5.,
            minimum_roll_speed: 30.,
            maximum_roll_speed: 90.,
            max_roll_per_second: 10.,
            invert_roll: true,
            tilt_fallback: 1.,
            minimum_tilt_speed: 0.,
            maximum_tilt_speed: 60.,
            gear_drop_back: 0.,
            gear_drop_back_time: 1.4,
            gear_drop_back_initial: 0.4,
            suspension_scale: 3.,
            suspension_mix: 0.375,
            suspension_dead_zone: 0.08,
        }
    }

    fn mounted(offset: Vec3, yaw: f32, movement: f32) -> Mounted {
        Mounted {
            fov: 60.,
            near: 0.1,
            yaw,
            pitch: 0.0734,
            roll: 0.,
            offset,
            axis_scale: Vec3::new(0., 0.08, 0.08),
            maximum_shift: 0.0999,
            movement_speed: movement,
            final_movement_scale: 1.,
            offset_only: false,
            axis_scale_for_tilt: Vec3::splat(0.2),
            twist_scale: Vec3::ZERO,
            tilt_distance: 1.,
        }
    }

    /// Diagnostic: the port's views driven by a decoded live trace of the
    /// original (local/cam/live/decode.py: t, step, speed, state, wheels x4,
    /// pose, angular, velocity, ...). Writes per row and view: eye, forward,
    /// up. GRID_CAMERA_TRACE=IN, GRID_CAMERA_OUT=OUT, GRID_GAME_ROOT.
    #[test]
    #[ignore]
    fn replay_live_trace() {
        let (Ok(input), Ok(output)) = (std::env::var("GRID_CAMERA_TRACE"), std::env::var("GRID_CAMERA_OUT")) else {
            return;
        };
        let root = std::env::var("GRID_GAME_ROOT").expect("GRID_GAME_ROOT");
        let views = Views::load(Path::new(&root), "lac").unwrap();
        let idents = ["chase_close", "chase_far", "bumper", "bonnet", "head-cam"];
        let mut histories = vec![History::default(); idents.len()];
        let mut out = String::new();
        let mut previous: Option<f64> = None;
        for line in std::fs::read_to_string(input).unwrap().lines() {
            let t: f64 = line.split('\t').next().unwrap().parse().unwrap();
            let v: Vec<f32> = line.split('\t').map(|x| x.parse().unwrap()).collect();
            // Car +0x10 rows from CM02 captures (columns 73..), else the identity;
            // the Lacetti's +0x1630 from the live grid dump (docs/SERIES.md, F447).
            let rows = if v.len() >= 89 {
                std::array::from_fn(|r| Vec3::new(v[73 + 4 * r], v[74 + 4 * r], v[75 + 4 * r]))
            } else {
                [Vec3::X, Vec3::Y, Vec3::Z]
            };
            let s = Sample {
                pose: model_frame(
                    Mat4::from_cols_array(&std::array::from_fn(|i| v[12 + i])),
                    rows,
                    Vec3::new(0., -0.39144, -0.14642),
                ),
                angular_velocity: Vec3::new(v[28], v[29], v[30]),
                velocity: Vec3::new(v[31], v[32], v[33]),
                speed: v[2],
                wheels: std::array::from_fn(|i| (v[4 + 2 * i], v[5 + 2 * i])),
                controls_disabled: v[3] != 7.,
                ticks: 0,
                segment: 0,
            };
            // The camera runs on the frame time (the sampler's row gap), not on
            // the physics step +0x7C8.
            let dt = previous.map_or(0., |p| (t - p) as f32);
            previous = Some(t);
            let mut row = vec![v[0]];
            for (k, ident) in idents.iter().enumerate() {
                let i = views.0.iter().position(|x| &x.ident == ident).unwrap();
                let shot = shot(&views.0[i].rig, &mut histories[k], &s, dt, |o| o);
                row.extend(shot.eye.to_array());
                row.extend((shot.rotation * Vec3::NEG_Z).to_array());
                row.extend((shot.rotation * Vec3::Y).to_array());
            }
            // States: chase_close lag and spin blend, bonnet tilt and shift.
            row.extend([histories[0].chase.lag, histories[0].chase.spin.blend]);
            row.extend(histories[3].mounted.tilt.to_array());
            row.extend(histories[3].mounted.shift.to_array());
            out += &row.iter().map(|x| x.to_string()).collect::<Vec<_>>().join("\t");
            out.push('\n');
        }
        std::fs::write(output, out).unwrap();
    }

    #[test]
    fn cameras_mount_on_the_model_frame() {
        // 0x9AE3A0: body origin (10, 0, 0) turned π/2 about Y; the model
        // origin is R · (0, −0.39144, −0.14642) = (−0.14642, −0.39144, 0) away.
        let pose = Mat4::from_rotation_translation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2), Vec3::new(10., 0., 0.));
        let model = model_frame(pose, [Vec3::X, Vec3::Y, Vec3::Z], Vec3::new(0., -0.39144, -0.14642));
        assert!(model.w_axis.truncate().abs_diff_eq(Vec3::new(10. - 0.14642, -0.39144, 0.), 1e-6));
        assert!(Mat3::from_mat4(model).abs_diff_eq(Mat3::from_mat4(pose), 1e-7));
        // A broken +0x10 block keeps the pose's rotation.
        let broken = model_frame(pose, [Vec3::ZERO; 3], Vec3::ZERO);
        assert_eq!(broken, pose);
    }

    #[test]
    fn bonnet_twist_leans_the_up_axis_with_the_tilt_state() {
        let mut m = mounted(Vec3::new(0., 1.1055, 0.8556), 0., 1.7264);
        m.twist_scale = Vec3::new(-0.1423, 0., 0.4107);
        // Tilt state: braking at 30 m/s² (a.z = −1) drives the goal to
        // clamp(0.2, ±0.0999); one step moves 0.0999 · 1.7264 · 2 · 0.016.
        let rig = Rig::Mounted(m.clone());
        let mut history = History::default();
        let dt = 0.016;
        shot(&rig, &mut history, &sample(Mat4::IDENTITY, Vec3::Z * 30., 0), dt, |v| v);
        shot(&rig, &mut history, &sample(Mat4::IDENTITY, Vec3::Z * (30. - 30. * dt), 1), dt, |v| v);
        assert!((history.mounted.tilt.z - 0.0999 * 1.7264 * 2. * dt).abs() < 1e-7, "{}", history.mounted.tilt);
        // At speed (weight 1) L = (0.05 · −0.1423, 1, 0.1 · 0.4107).
        let h = MountedState { tilt: Vec3::new(0.05, 0., 0.1), ..MountedState::default() };
        let fast = sample(Mat4::IDENTITY, Vec3::Z * 30., 0);
        let q = mounted_twist(&m, &h, &fast);
        let up = Vec3::new(-0.007115, 1., 0.04107).normalize();
        assert!((q * Vec3::Y).abs_diff_eq(up, 1e-6));
        let forward = Vec3::new(-up.x * up.z, -up.y * up.z, up.x * up.x + up.y * up.y).normalize();
        assert!((q * Vec3::Z).abs_diff_eq(forward, 1e-6));
        // Standing still the lean's x comes from the yaw rate: −2 · 0.1 = −0.2.
        let mut still = sample(Mat4::IDENTITY, Vec3::ZERO, 0);
        still.angular_velocity = Vec3::Y * 2.;
        let up = Vec3::new(-0.2 * -0.1423, 1., 0.04107).normalize();
        assert!((mounted_twist(&m, &h, &still) * Vec3::Y).abs_diff_eq(up, 1e-6));
        // No twistScale (bumper, reverse views): no twist.
        m.twist_scale = Vec3::ZERO;
        assert_eq!(mounted_twist(&m, &h, &still), Quat::IDENTITY);
    }

    /// The Lacetti's five visible views and their reverse twins, in file order.
    fn lacetti() -> Views {
        let c = |p, t| Rig::Chase(chase(p, t));
        let m = |o, yaw| Rig::Mounted(mounted(o, yaw, 0.));
        let views = [
            ("chase_close", false, Some(1), c(Vec3::new(0., 1.7227, -4.7393), Vec3::new(0., 1.2552, 1.9141))),
            ("chase_close_reverse", true, None, c(Vec3::new(0., 1.7227, 5.116), Vec3::new(0., 1.2552, -1.9141))),
            ("chase_far", false, Some(3), c(Vec3::new(0., 1.8637, -5.1838), Vec3::new(0., 1.5535, 2.5941))),
            ("chase_far_reverse", true, None, c(Vec3::new(0., 1.8637, 5.523), Vec3::new(0., 1.5535, -2.5941))),
            ("bumper", false, Some(5), m(Vec3::new(0., 0.7275, 1.7706), 0.)),
            ("bumper_reverse", true, None, m(Vec3::new(0., 0.7275, -1.77), std::f32::consts::PI)),
            ("bonnet", false, Some(7), m(Vec3::new(0., 1.1055, 0.8556), 0.)),
            ("bonnet_reverse", true, None, m(Vec3::new(0., 1.24, -1.437), std::f32::consts::PI)),
            ("head-cam", false, Some(9), Rig::Head(Head { fov: 48., near: 0.075, position: Vec3::new(0.3604, 1.1047, -0.1923), tilt: -7.3736 })),
            ("head-cam_reverse", true, None, m(Vec3::new(0., 1.24, -1.437), std::f32::consts::PI)),
            ("replay_wing1", true, None, m(Vec3::ZERO, 0.)),
        ];
        Views(
            views
                .into_iter()
                .map(|(ident, hidden, reverse, rig)| View {
                    ident: ident.into(),
                    hidden,
                    reverse,
                    rig,
                })
                .collect(),
        )
    }

    fn sample(pose: Mat4, velocity: Vec3, ticks: u64) -> Sample {
        Sample::moving(pose, velocity, ticks, 0)
    }

    fn forward(shot: &Shot) -> Vec3 {
        shot.rotation * Vec3::NEG_Z
    }

    #[test]
    fn change_view_cycles_visible_views_and_wraps() {
        let views = lacetti();
        let mut i = views.restore(DEFAULT_VIEW);
        let mut order = vec![views.0[i].ident.clone()];
        for _ in 0..5 {
            i = views.next(i);
            order.push(views.0[i].ident.clone());
        }
        assert_eq!(
            order,
            ["chase_close", "chase_far", "bumper", "bonnet", "head-cam", "chase_close"]
        );
    }

    #[test]
    fn profile_restore_and_save_follow_the_original() {
        let views = lacetti();
        assert_eq!(views.restore("bonnet"), 6);
        // Hidden or unknown names fall back to the first visible view.
        assert_eq!(views.restore("bonnet_reverse"), 0);
        assert_eq!(views.restore("no_such_view"), 0);
        // A reverse view saves the view it belongs to.
        assert_eq!(views.saved(7), "bonnet");
        assert_eq!(views.saved(2), "chase_far");
        assert_eq!(parse_profile("# c\ncurrent_camera = bumper\n"), "bumper");
        assert_eq!(parse_profile(""), DEFAULT_VIEW);
    }

    #[test]
    fn chase_at_rest_matches_the_shipped_position_and_target() {
        let pose = Mat4::from_rotation_translation(Quat::from_rotation_y(0.7), Vec3::new(10., 2., -5.));
        let mut cameras = Cameras::new(lacetti(), DEFAULT_VIEW, None);
        let shot = cameras.frame(&sample(pose, Vec3::ZERO, 5), false, |v| v);
        let eye = pose.transform_point3(Vec3::new(0., 1.7227, -4.7393));
        let target = pose.transform_point3(Vec3::new(0., 1.2552, 1.9141));
        assert!(shot.eye.distance(eye) < 1e-4);
        assert!(forward(&shot).dot((target - eye).normalize()) > 0.99999);
        assert_eq!(shot.fov, 52f32.to_radians());
        // Look Back: the reverse chase sits ahead of the car looking back.
        let back = cameras.frame(&sample(pose, Vec3::ZERO, 5), true, |v| v);
        let ahead = pose.transform_vector3(Vec3::Z);
        assert!((back.eye - pose.w_axis.truncate()).dot(ahead) > 5.);
        assert!(forward(&back).dot(ahead) < -0.9);
    }

    #[test]
    fn chase_yaw_lag_trails_a_turn_and_returns_faster_at_speed() {
        let c = chase(Vec3::new(0., 1.7227, -4.7393), Vec3::new(0., 1.2552, 1.9141));
        let dt = grid_tools::source_vehicle::DELTA;
        // A 0.1 rad turn in one step lags by the turn less one step of return
        // at lagMinimum (standing still).
        let lag = chase_lag(&c, 0., 0.1, 0., dt);
        assert!((lag - 0.1 * (1. - dt * 2.7507)).abs() < 1e-7);
        // With the car steady the lag decays; faster at 200 km/h.
        let settle = |speed: f32| {
            let mut lag = 0.1;
            for _ in 0..100 {
                lag = chase_lag(&c, lag, 0., speed, dt);
            }
            lag
        };
        let (slow, fast) = (settle(0.), settle(LAG_SPEED));
        assert!(fast < slow && slow < 0.1 && fast > 0.);
        // At 200 km/h the rate is lagMaximum: explicit Euler, (1 − dt·4.5133)^n.
        let mut lag = 0.1;
        for _ in 0..62 {
            lag = chase_lag(&c, lag, 0., LAG_SPEED, dt);
        }
        assert!((lag / 0.1 - (1. - dt * 4.5133f32).powi(62)).abs() < 1e-4);
        // The angular limit (10 rad) never binds after the ±π wrap.
        assert!((chase_lag(&c, 3.1, 0.1, 0., 0.) - (3.2 - std::f32::consts::TAU)).abs() < 1e-6);
    }

    #[test]
    fn chase_frame_turns_with_the_lag() {
        let mut cameras = Cameras::new(lacetti(), DEFAULT_VIEW, None);
        let straight = Mat4::IDENTITY;
        let turned = Mat4::from_rotation_y(0.2);
        cameras.frame(&sample(straight, Vec3::Z * 30., 0), false, |v| v);
        let shot = cameras.frame(&sample(turned, Vec3::Z * 30., 1), false, |v| v);
        // The eye has not yet swung behind the turned car: still near -Z.
        let behind = turned.transform_point3(Vec3::new(0., 1.7227, -4.7393));
        assert!(shot.eye.x.abs() < behind.x.abs());
    }

    #[test]
    fn close_follow_lowers_the_chase_eye_uphill() {
        let mut cameras = Cameras::new(lacetti(), DEFAULT_VIEW, None);
        let uphill = Mat4::from_rotation_x(-0.1);
        assert!(uphill.z_axis.y > 0.);
        // Above 5 m/s the eye height follows the car; the first update sets
        // the follow state directly (record +0xAC).
        let first = cameras.frame(&sample(uphill, Vec3::Z * 30., 0), false, |v| v);
        let later = cameras.frame(&sample(uphill, Vec3::Z * 30., 1), false, |v| v);
        let drop = first.eye.y - later.eye.y;
        assert!((drop - 5. * 0.1f32.sin()).abs() < 1e-5, "{drop}");
    }

    #[test]
    fn eye_height_holds_until_five_metres_per_second() {
        // 0x8002D0: the height blends (speed − 0)/(5 − 0) with the previous
        // one and follows fully once 5 m/s was reached.
        let mut cameras = Cameras::new(lacetti(), DEFAULT_VIEW, None);
        let at = |y: f32| Mat4::from_translation(Vec3::new(0., y, 0.));
        let start = cameras_frame_y(&mut cameras, at(0.), Vec3::ZERO, 0);
        assert!((cameras_frame_y(&mut cameras, at(1.), Vec3::ZERO, 1) - start).abs() < 1e-6);
        // At 2 m/s: 0.4 of the new height (1 m up) and 0.6 of the held one.
        let y = cameras_frame_y(&mut cameras, at(1.), Vec3::Z * 2., 2);
        assert!((y - (start + 0.4)).abs() < 1e-5, "{y}");
        // At 5 m/s it latches; standing still afterwards it keeps following.
        cameras_frame_y(&mut cameras, at(1.), Vec3::Z * 5., 3);
        let y = cameras_frame_y(&mut cameras, at(2.), Vec3::ZERO, 4);
        assert!((y - (start + 2.)).abs() < 1e-5, "{y}");
    }

    fn cameras_frame_y(cameras: &mut Cameras, pose: Mat4, velocity: Vec3, ticks: u64) -> f32 {
        cameras.frame(&sample(pose, velocity, ticks), false, |v| v).eye.y
    }

    #[test]
    fn suspension_heave_beyond_the_dead_zone() {
        let c = chase(Vec3::new(0., 1.7227, -4.7393), Vec3::new(0., 1.2552, 1.9141));
        // 0.05 m on every wheel, travel 0.3: margin 0.08 · 0.3 = 0.024 < 0.05,
        // shrunk by 0.024 / 0.92 = 0.0260870; mix 0.375^(0.016 · 40) = 0.533801.
        let wheels = [(0.05, 0.3); 4];
        let heave = suspension_heave(&c, 0., &wheels, 0.016);
        let expected = 4. * (0.05 - 0.024 / 0.92) * 0.25 * 0.375f64.powf(0.64) as f32;
        assert!((heave - 0.0127649).abs() < 1e-6 && (heave - expected).abs() < 1e-7, "{heave}");
        // Inside the dead zone a wheel adds nothing; the old value decays by 1 − m.
        let quiet = suspension_heave(&c, heave, &[(0.02, 0.3); 4], 0.016);
        assert!((quiet - heave * (1. - 0.533801)).abs() < 1e-6, "{quiet}");
        // The target point rises by suspScale times the heave.
        let mut cameras = Cameras::new(lacetti(), DEFAULT_VIEW, None);
        let mut s = sample(Mat4::IDENTITY, Vec3::ZERO, 0);
        s.wheels = wheels;
        cameras.frame(&s, false, |v| v);
        s.ticks = 1;
        let shot = cameras.frame(&s, false, |v| v);
        let target = Vec3::new(0., 1.2552 + 3. * expected, 1.9141);
        assert!(forward(&shot).dot((target - shot.eye).normalize()) > 0.999_999);
    }

    #[test]
    fn eye_falls_back_with_speed_and_gear_drop_back() {
        let mut c = chase(Vec3::new(0., 1.7227, -4.7393), Vec3::new(0., 1.2552, 1.9141));
        let mut drop = 0.;
        // tiltFallback 1 m over 0..60: half at 30 m/s, all from 60.
        assert_eq!(fall_back(&c, 30., None, &mut drop), 0.5);
        assert_eq!(fall_back(&c, 90., None, &mut drop), 1.);
        assert_eq!(fall_back(&c, -3., None, &mut drop), 0.);
        // gearUpDropBack 0 (all in-race views): no gear effect.
        assert_eq!(fall_back(&c, 90., Some((2, 0.2)), &mut drop), 1.);
        // With 0.5: min(0.5 · 1 · 1.15, 1) = 0.575 times the envelope.
        c.gear_drop_back = 0.5;
        drop = 0.;
        // Rise 0.5 − cos(0.2/0.4 · π)/2 = 0.5 → 1 − 0.2875.
        assert!((fall_back(&c, 90., Some((2, 0.2)), &mut drop) - 0.7125).abs() < 1e-6);
        assert!((drop - 0.5).abs() < 1e-6);
        // The rise keeps the larger envelope.
        drop = 0.9;
        fall_back(&c, 90., Some((2, 0.2)), &mut drop);
        assert_eq!(drop, 0.9);
        // Fall (cos((0.9 − 0.4)/1.0 · π) + 1)/2 = 0.5; gone after 1.4 s.
        fall_back(&c, 90., Some((5, 0.9)), &mut drop);
        assert!((drop - 0.5).abs() < 1e-6);
        assert_eq!(fall_back(&c, 90., Some((2, 1.5)), &mut drop), 1.);
        // In the view: 30 m/s pushes the eye 0.5 m back.
        let mut cameras = Cameras::new(lacetti(), DEFAULT_VIEW, None);
        let shot = cameras.frame(&sample(Mat4::IDENTITY, Vec3::Z * 30., 0), false, |v| v);
        assert!(shot.eye.distance(Vec3::new(0., 1.7227, -5.2393)) < 1e-5, "{}", shot.eye);
    }

    #[test]
    fn roll_follows_the_yaw_rate_at_a_limited_rate() {
        let c = chase(Vec3::new(0., 1.7227, -4.7393), Vec3::new(0., 1.2552, 1.9141));
        // Goal 0.5 · (1/π) · 5 · 1.4 = 1.114085; step 10 · 0.5 · 0.016 = 0.08;
        // weight (60 · 1.25 − 30)/(90 − 30) = 0.75.
        let (roll, weight) = roll_step(&c, 0., 0.5, 60., 0.016);
        assert!((roll - 0.08).abs() < 1e-7 && weight == 0.75, "{roll} {weight}");
        let mut roll = roll;
        for _ in 0..20 {
            roll = roll_step(&c, roll, 0.5, 60., 0.016).0;
        }
        assert!((roll - 1.114_085).abs() < 1e-5, "{roll}");
        assert_eq!(roll_step(&c, 0., 0.5, 10., 0.016).1, 0.);
        // In the view: a steady yaw rate tilts the up vector by
        // 1.114085 · 0.75 degrees toward up × forward (invertRoll).
        let mut cameras = Cameras::new(lacetti(), DEFAULT_VIEW, None);
        let mut s = sample(Mat4::IDENTITY, Vec3::Z * 48., 0);
        s.speed = 60.;
        s.angular_velocity = Vec3::Y * 0.5;
        let mut shot = cameras.frame(&s, false, |v| v);
        for t in 1..30 {
            s.ticks = t;
            shot = cameras.frame(&s, false, |v| v);
        }
        let up = shot.rotation * Vec3::Y;
        let side = Vec3::Y.cross(forward(&shot)).normalize();
        let angle = up.dot(side).asin();
        assert!((angle - 1.114_085 * 0.75 * DEGREES).abs() < 1e-5, "{angle}");
    }

    #[test]
    fn spin_hold_envelope() {
        let mut s = Spin::default();
        let mut sample = sample(Mat4::IDENTITY, Vec3::ZERO, 0);
        let dt = 0.016;
        let held = Vec3::Z;
        spin_step(&mut s, &sample, held, dt);
        // A yaw-rate jump arms three frames; three frames spinning above
        // 2.5 rad/s start the 2 s envelope.
        sample.angular_velocity = Vec3::Y * 5.;
        spin_step(&mut s, &sample, held, dt);
        spin_step(&mut s, &sample, held, dt);
        assert!(s.timer <= 0.);
        spin_step(&mut s, &sample, held, dt);
        assert_eq!((s.timer, s.blend, s.held), (2. - dt, 0., held));
        // Rise √(1 − (1.984/2 − 0.9)/0.1) = √0.08.
        spin_step(&mut s, &sample, held, dt);
        assert!((s.blend - 0.08f32.sqrt()).abs() < 1e-5, "{}", s.blend);
        // Pitch and roll rates count half: |(4, 0, 4)| / 2 = 2.83 > 2.5.
        let mut t = Spin::default();
        let mut tilted = sample;
        tilted.angular_velocity = Vec3::ZERO;
        spin_step(&mut t, &tilted, held, dt);
        tilted.angular_velocity = Vec3::new(4., 0., 4.);
        for _ in 0..3 {
            spin_step(&mut t, &tilted, held, dt);
        }
        assert!(t.timer > 0.);
        // While spinning the plateau restarts at 1.8 s.
        for _ in 0..30 {
            spin_step(&mut s, &sample, held, dt);
        }
        assert_eq!(s.blend, 1.);
        assert!((s.timer - (1.8 - dt)).abs() < 1e-6);
        // The stop is itself a jump: three armed frames pass, then the
        // envelope goes to 1.2 s and eases out quadratically.
        sample.angular_velocity = Vec3::ZERO;
        for _ in 0..4 {
            spin_step(&mut s, &sample, held, dt);
        }
        assert!((s.timer - (1.2 - dt)).abs() < 1e-6 && s.blend == 1., "{} {}", s.timer, s.blend);
        // At 0.3 s left: f = 0.15/0.6 = 0.25 → (0.5)² / 2 = 0.125.
        s.timer = 0.3;
        spin_step(&mut s, &sample, held, dt);
        assert!((s.blend - 0.125).abs() < 1e-6, "{}", s.blend);
        // Disabled controls hold the plateau.
        let mut p = Spin { timer: 1.5, ..Spin::default() };
        let mut off = sample;
        off.controls_disabled = true;
        spin_step(&mut p, &off, held, dt);
        assert!((p.timer - 1.5).abs() < 1e-6 && p.blend == 1.);
    }

    #[test]
    fn spin_frame_holds_the_old_forward() {
        let turned = Mat4::from_rotation_translation(Quat::from_rotation_y(1.2), Vec3::new(3., 0., 1.));
        // No envelope: the car frame.
        assert!(spin_frame(&turned, 0., Vec3::Z).abs_diff_eq(turned, 1e-6));
        // Unset held forward (zero): the car frame (0x7FF3E0 degenerate path).
        assert!(spin_frame(&turned, 0.7, Vec3::ZERO).abs_diff_eq(turned, 1e-6));
        // Full blend: forward is the held one; half: halfway turned.
        let held = spin_frame(&turned, 1., Vec3::Z);
        assert!(held.z_axis.truncate().abs_diff_eq(Vec3::Z, 1e-6));
        assert_eq!(held.w_axis, turned.w_axis);
        let half = spin_frame(&turned, 0.5, Vec3::Z);
        assert!((heading(&half) - 0.6).abs() < 1e-5);
        // Within acos 0.97 the forward is a straight blend.
        let near = Mat4::from_rotation_y(0.2);
        let f = spin_frame(&near, 0.5, Vec3::Z).z_axis.truncate();
        let lerp = (near.z_axis.truncate() * 0.5 + Vec3::Z * 0.5).normalize();
        assert!(f.abs_diff_eq(lerp, 1e-6));
    }

    #[test]
    fn drift_blend_turns_the_lag_toward_the_slip() {
        let v = Vec3::new(10., 0., 10.);
        // Weight 0 (races): unchanged.
        assert_eq!(drift_lag(0.1, v, &Mat4::IDENTITY, 0.), 0.1);
        // Slip 45°, speed weight (14.1421 − 5) · 0.1 = 0.914214.
        let lag = drift_lag(0., v, &Mat4::IDENTITY, 1.);
        assert!((lag + std::f32::consts::FRAC_PI_4 * 0.914_214).abs() < 1e-5, "{lag}");
    }

    #[test]
    fn bumper_is_rigid_and_pitched_down() {
        let pose = Mat4::from_rotation_translation(Quat::from_rotation_y(-1.2), Vec3::new(-3., 1., 4.));
        let mut cameras = Cameras::new(lacetti(), "bumper", None);
        let v = pose.transform_vector3(Vec3::Z) * 40.;
        cameras.frame(&sample(pose, Vec3::ZERO, 0), false, |v| v);
        let shot = cameras.frame(&sample(pose, v, 1), false, |v| v);
        assert!(shot.eye.distance(pose.transform_point3(Vec3::new(0., 0.7275, 1.7706))) < 1e-4);
        let f = forward(&shot);
        assert!(f.dot(pose.transform_vector3(Vec3::Z)) > 0.99);
        assert!(f.y < 0.);
        assert_eq!(shot.near, Some(0.1));
        // Its reverse looks backwards.
        let back = cameras.frame(&sample(pose, v, 2), true, |v| v);
        assert!(forward(&back).dot(pose.transform_vector3(Vec3::Z)) < -0.9);
    }

    #[test]
    fn bonnet_eye_shifts_forward_under_braking_within_the_limit() {
        let m = mounted(Vec3::new(0., 1.1055, 0.8556), 0., 1.7264);
        let rig = Rig::Mounted(m);
        let mut history = History::default();
        let dt = grid_tools::source_vehicle::DELTA;
        let mut speed = 50.;
        let mut ticks = 0;
        let mut last = Vec3::ZERO;
        // 3 s at 30 m/s² deceleration (≈3 g, the full scale of 0x822510).
        while speed > 0. {
            let s = sample(Mat4::IDENTITY, Vec3::Z * speed, ticks);
            last = shot(&rig, &mut history, &s, dt, |v| v).eye;
            speed -= 30. * dt;
            ticks += 1;
        }
        let shift = last.z - 0.8556;
        assert!(shift > 0.05 && shift <= 0.08 + 1e-4, "{shift}");
    }

    #[test]
    fn head_cam_sits_in_the_seat() {
        let pose = Mat4::from_translation(Vec3::new(1., 2., 3.));
        let mut cameras = Cameras::new(lacetti(), "head-cam", None);
        let shot = cameras.frame(&sample(pose, Vec3::ZERO, 0), false, |v| v);
        assert!(shot.eye.distance(Vec3::new(1.3604, 3.1047, 2.8077)) < 1e-4);
        assert!(forward(&shot).z > 0.99 && forward(&shot).y < 0.);
        assert_eq!(shot.fov, 48f32.to_radians());
    }

    #[test]
    fn same_inputs_give_bit_identical_shots() {
        let run = || {
            let mut cameras = Cameras::new(lacetti(), DEFAULT_VIEW, None);
            let mut out = Vec::new();
            for t in 0..200u64 {
                let a = t as f32 * 0.01;
                let pose = Mat4::from_rotation_translation(
                    Quat::from_rotation_y(a.sin()),
                    Vec3::new(a, 0., a * 2.),
                );
                if t == 120 {
                    cameras.change_view();
                }
                let s = cameras.frame(&sample(pose, Vec3::new(a, 0., 30.), t), t % 50 > 40, |v| v);
                out.push((s.eye.to_array(), s.rotation.to_array()));
            }
            out
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn reset_clears_the_history() {
        let mut cameras = Cameras::new(lacetti(), DEFAULT_VIEW, None);
        cameras.frame(&sample(Mat4::IDENTITY, Vec3::ZERO, 0), false, |v| v);
        cameras.frame(&sample(Mat4::from_rotation_y(0.3), Vec3::ZERO, 1), false, |v| v);
        assert!(cameras.history.chase.lag > 0.);
        let mut s = sample(Mat4::IDENTITY, Vec3::ZERO, 0);
        s.segment = 1;
        cameras.frame(&s, false, |v| v);
        assert_eq!(cameras.history.chase.lag, 0.);
    }
}
