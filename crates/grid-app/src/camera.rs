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
//!   acceleration-driven eye shift (0x822510). Head (0x842DD0 → 0x820D10):
//!   seat position and tilt, the eye carried by a body spring and the view
//!   turned by a head spring, both driven by the seat's acceleration (0x83E6D0).
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
/// Impact knock (envelope 2 of 0x81EC90): a change of |v| above 50 m/s² per
/// second (0xE4DD78) starts a 1 s envelope (0xE4DD7C), held at 1 between 0.4
/// and 0.8 s left (0xE4DD84, 0xE4DD80), with √ rise and fall (0xE4DD88,
/// 0xE4DD8C); the reset 0x831D20 sets the spring's constants.
const KNOCK_ACCELERATION: f32 = 50.;
const KNOCK_TIME: f32 = 1.;
const KNOCK_HOLD_START: f32 = f32::from_bits(0x3ecc_cccd);
const KNOCK_HOLD_END: f32 = f32::from_bits(0x3f4c_cccd);
const KNOCK_POWER: f64 = 0.5;
const KNOCK_SPRING: Spring = Spring { scale: Vec2::ONE, damping: 45., inertia: 1.25, centering: 3., max_offset: 0.75 };
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
/// Head-cam springs (0x83E6D0): a step shorter than 0xCFDD14 only takes the
/// input velocity; the acceleration filter halves per 1/60 s (0xCBC4F8,
/// 0xCAFEC4); a speed below the root of 0xCFF550 stops.
const SPRING_MIN_STEP: f32 = 1e-4;
const SPRING_MIN_SQUARE: f32 = 1e-9;
/// At most this many camera steps per shot; a longer gap (a replay seek) drops
/// the rest [AUTHORED].
const MAX_STEPS: f32 = 30.;
/// Look input (0x8348F0 → slot 7 messages): the bumper/bonnet turn and the
/// head-cam's look follow the input by 0.2 per message (0xCB870C, 0xE4DD9C);
/// the bonnet eye rises and moves forward by 0.05 · |turn| (0xE4DDA0,
/// 0xE4DDA4); the head's look pitch is maxHeadLook · 0.00523599 (0xCFFDBC)
/// and its angles move at most 180°/s (object +0x294, constructor).
const LOOK_BLEND: f32 = f32::from_bits(0x3e4c_cccd);
const BONNET_LOOK_LIFT: f32 = f32::from_bits(0x3d4c_cccd);
const HEAD_LOOK_PITCH: f32 = f32::from_bits(0x3bab_92a7);
const HEAD_LOOK_RATE: f32 = 180.;
/// Chase look (0x7FE690): the look point's blend per message is 0.1 + 0.4 w
/// (0xCAFCD4, 0xCB22A4) and it snaps when the angle step reaches
/// 3.2 (1 − w) + w (0xCE6590), w = clamp(0.2 · speed) (0xCB870C); the point
/// sits 0.1 ahead (0xCAFCD4); angles wrap at ±3.141592 (0xCFF570) by
/// 6.2831855 (0xCB0DE0).
const CHASE_LOOK_BLEND: f32 = f32::from_bits(0x3dcc_cccd);
const CHASE_LOOK_SPEED_BLEND: f32 = f32::from_bits(0x3ecc_cccd);
const CHASE_LOOK_SNAP: f32 = f32::from_bits(0x404c_cccd);
const CHASE_LOOK_AHEAD: f32 = f32::from_bits(0x3dcc_cccd);
const CHASE_LOOK_PI: f32 = f32::from_bits(0x4049_0fd8);
const CHASE_LOOK_TAU: f32 = f32::from_bits(0x40c9_0fdb);
/// Camera shake noise (0x7FE510): a 6000-entry table (0xE7B230) read at 300
/// entries per second of shake time (0xCAFBE0), its vertical channel 0.1 s
/// ahead; amplitudes below 0xCFDD14 stop.
const SHAKE_TABLE: usize = 6000;
const SHAKE_RATE: f32 = 300.;
const SHAKE_LEAD: f32 = f32::from_bits(0x3dcc_cccd);
const SHAKE_MIN_AMPLITUDE: f32 = 1e-4;
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
    /// What the look input does (Mounted slot 7, 0x840A30, by ident).
    pub look: MountedLook,
    /// shake* parameters (binder 0x7F4BE0, object +0x134..+0x14C).
    pub shake: Shake,
    /// highResInterior (+0x190): the view is inside the car for Change View
    /// (0x7F9CA0).
    pub high_res_interior: bool,
}

/// A view's shake (0x815960): shakeAngle (degrees), shakeDamping (per
/// second), shakeMinSpeed/MaxSpeed (m/s), shakeScaleX/Y (m) and
/// shakeRotationSpeed (the angle channel's time scale).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Shake {
    pub angle: f32,
    pub damping: f32,
    pub min_speed: f32,
    pub max_speed: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub rotation_speed: f32,
}

/// The Mounted views that take the look input: only the idents `bumper`
/// (turn) and `bonnet` (turn and eye lift) do, not their reverses (0x840A30,
/// 0x822510).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MountedLook {
    #[default]
    Fixed,
    Turn,
    TurnAndLift,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Head {
    pub fov: f32,
    pub near: f32,
    pub position: Vec3,
    /// Degrees; negative looks down (0x801D10, emulated).
    pub tilt: f32,
    /// Eye lift above the seat position (bodyLength, object +0x218).
    pub body_length: f32,
    /// The eye's spring (object +0x110) and the view's (object +0x190).
    pub body: Spring,
    pub head: Spring,
    /// Largest look turn, degrees (maxHeadLook, object +0x28C).
    pub max_look: f32,
}

/// A head-cam spring's parameters as the reset 0x82C0E0 copies them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spring {
    /// Car-frame x and y scale of the offset limit (+0x60, +0x64); 0 keeps
    /// that component at zero.
    pub scale: Vec2,
    pub damping: f32,
    pub inertia: f32,
    pub centering: f32,
    pub max_offset: f32,
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
    /// DisableDriverHead (view +0xFD): the player's driver draws its headless
    /// `_x2` meshes (0x84E440, 0x85D310).
    pub disable_driver_head: bool,
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
                    // Absent shake parameters leave the shake off [AUTHORED].
                    shake: {
                        let o = |n: &str| v.scalar(n).unwrap_or(0.);
                        Shake {
                            angle: o("shakeAngle"),
                            damping: o("shakeDamping"),
                            min_speed: o("shakeMinSpeed"),
                            max_speed: o("shakeMaxSpeed"),
                            scale_x: o("shakeScaleX"),
                            scale_y: o("shakeScaleY"),
                            rotation_speed: o("shakeRotationSpeed"),
                        }
                    },
                    look: match v.ident.as_str() {
                        "bumper" => MountedLook::Turn,
                        "bonnet" => MountedLook::TurnAndLift,
                        _ => MountedLook::Fixed,
                    },
                    high_res_interior: v.flag("highResInterior").unwrap_or(false),
                }),
                Kind::Head => {
                    // Binder 0x7F3A70 over the constructor's defaults (0x820A40).
                    let o = |n: &str, d: f32| v.scalar(n).unwrap_or(d);
                    Rig::Head(Head {
                        fov: s("fov")?,
                        near: s("nearPlane")?,
                        position: vec("position")?,
                        tilt: s("tilt")?,
                        body_length: o("bodyLength", 0.01),
                        body: Spring {
                            scale: Vec2::new(o("maxBodyOffsetScaleSide", 1.), o("maxBodyOffsetScaleUp", 0.)),
                            damping: o("bodyDamping", 0.7),
                            inertia: o("bodyInertia", 0.1),
                            centering: o("bodyCenteringForce", 0.6),
                            max_offset: o("maxBodyOffset", 0.1),
                        },
                        // The head's scale is the constructor's (1, 0): its
                        // offset is a turn about the car's x and z.
                        head: Spring {
                            scale: Vec2::new(1., 0.),
                            damping: o("headDamping", 0.7),
                            inertia: o("headInertia", 0.2),
                            centering: o("headCenteringForce", 0.6),
                            max_offset: o("maxHeadOffset", 0.1),
                        },
                        max_look: o("maxHeadLook", 90.),
                    })
                }
            };
            views.push(View {
                ident: v.ident.clone(),
                hidden: v.hidden,
                reverse: None,
                rig,
                disable_driver_head: v.flag("DisableDriverHead").unwrap_or(false),
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
                disable_driver_head: false,
            },
            View {
                ident: format!("{DEFAULT_VIEW}_reverse"),
                hidden: true,
                reverse: None,
                rig: chase(Vec3::new(0., 3., 9.), Vec3::Y),
                disable_driver_head: false,
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
    /// The angular velocity is the car's own (false for replays, which know
    /// only poses: their chase lag follows the heading change instead).
    pub rate_known: bool,
    /// The car's speed word, car +0x1648 (+8).
    pub speed: f32,
    /// Per wheel: suspension displacement (wheel +0x40) and travel clip
    /// (wheel +0x4C) (+0x24, 0x7F7330).
    pub wheels: [(f32, f32); 4],
    /// Car +0xD234 is not 7, the controls are disabled (+0x28, 0x7F7380).
    pub controls_disabled: bool,
    /// The chase's drift blend weight: car +0xFE0, vehicle-properties record
    /// +0x168 (CSV column 330, empty in every shipped car, so 0).
    pub drift_weight: f32,
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
            rate_known: false,
            speed: velocity.length(),
            wheels: [(0., 1.); 4],
            controls_disabled: false,
            drift_weight: 0.,
            ticks,
            segment,
        }
    }

    /// The car state `t` of the way from `a` to `self` (Clock::Fps camera
    /// frames between two ticks): pose slerped, vectors and wheels linear.
    pub fn from(&self, a: &Sample, t: f32) -> Self {
        let (sa, ra, pa) = a.pose.to_scale_rotation_translation();
        let (sb, rb, pb) = self.pose.to_scale_rotation_translation();
        let lerp = |x: f32, y: f32| x + (y - x) * t;
        Self {
            pose: Mat4::from_scale_rotation_translation(sa.lerp(sb, t), ra.slerp(rb, t), pa.lerp(pb, t)),
            velocity: a.velocity.lerp(self.velocity, t),
            angular_velocity: a.angular_velocity.lerp(self.angular_velocity, t),
            speed: lerp(a.speed, self.speed),
            wheels: std::array::from_fn(|i| (lerp(a.wheels[i].0, self.wheels[i].0), lerp(a.wheels[i].1, self.wheels[i].1))),
            ..*self
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
                Vec3::new(f(0x40), f(0x44), f(0x48)),
                Vec3::new(f(0x1630), f(0x1634), f(0x1638)),
            ),
            velocity: Vec3::new(vx, vy, vz),
            angular_velocity: Vec3::new(wx, wy, wz),
            rate_known: true,
            speed: f(0x1648),
            wheels: std::array::from_fn(|i| {
                let w = 0x1990 + i * 0x150;
                (f(w + 0x40), f(w + 0x4c))
            }),
            controls_disabled: b[0xd234 / 4] != 7,
            drift_weight: f(0xfe0),
            ticks: runtime.ticks,
            segment,
        }
    }
}

/// The car's model frame, which the camera target answers (0x9AE3A0; target
/// +0x14/+0x18/+0x1C): the physics pose (car +0xDB0 → +8, body +0x1D0)
/// rotated by the chassis-motion matrix at car +0x10 (rows, 0x9BE1E0 via
/// 0x9BDF30, ported as grid_world::vehicle_chassis), its origin moved by the
/// matrix's translation row car +0x40 (unturned) and then by the rotation
/// times car +0x1630 (the Lacetti's (0, −0.39144, −0.14642): the model origin
/// lies below and behind the body origin). A rotation that is not one keeps
/// the pose's rotation.
pub fn model_frame(pose: Mat4, rows: [Vec3; 3], translation: Vec3, offset: Vec3) -> Mat4 {
    let local = Mat3::from_cols(rows[0], rows[1], rows[2]);
    let local = if local.is_finite() && (local.determinant() - 1.).abs() < 1e-3 { local } else { Mat3::IDENTITY };
    let translation = if translation.is_finite() { translation } else { Vec3::ZERO };
    // Row vectors: pose rows × the +0x10 rows; the translation is not turned.
    let rotation = local * Mat3::from_mat4(pose);
    let origin = pose.w_axis.truncate() + translation + rotation * offset;
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

/// Impact knock of a chase view (envelope 2 and its spring, 0x81EC90).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Knock {
    /// Envelope time left (record +0x140) and the last |v| (record +0x144).
    pub timer: f32,
    speed: f32,
    /// Weight of the offset (chase +0x240).
    pub blend: f32,
    /// The eye of the last update (spring +0x00, valid byte +0x50).
    eye: Option<Vec3>,
    /// The spring (chase +0x1A0..): offset +0x1D0 is the knock, in world axes.
    pub spring: SpringState,
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
    pub knock: Knock,
    /// Look point (record +0xA4, +0xA8).
    pub look: Vec2,
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
    /// Look turn, radians (record +0xB0).
    pub look: f32,
    /// Shake time and amplitude (record +0x90, +0x98).
    pub shake_time: f32,
    pub shake_amplitude: f32,
    velocity: Option<Vec3>,
}

/// One head-cam spring's state (record +0x110.. body, +0x170.. head).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SpringState {
    /// Last input velocity, world.
    pub prev: Vec3,
    /// Filtered acceleration, world.
    pub acc: Vec3,
    /// Offset and its velocity, car frame.
    pub off: Vec3,
    pub vel: Vec3,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HeadState {
    pub body: SpringState,
    pub head: SpringState,
    /// Smoothed look input x, y (record +0xD8, +0xDC), the look yaw and
    /// pitch (+0xE4, +0xE8, radians) and the looking state (+0xF4 = 1).
    pub look: Vec2,
    pub look_angle: Vec2,
    looking: bool,
    /// Seat velocity of the last step; None until the reset.
    input: Option<Vec3>,
}

/// Filter histories, one per view, reset on a car reset or a view change.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct History {
    pub chase: ChaseState,
    pub mounted: MountedState,
    pub head: HeadState,
    /// Simulation time not yet stepped.
    pending: f32,
    /// The sample of the last camera frame (Clock::Fps interpolates from it).
    sample: Option<Sample>,
}

/// The look input of a frame: x = Look Left − Look Right, y = Look Up − Look
/// Down, the calibrated action values (0x8348F0, sent every frame).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Look {
    pub x: f32,
    pub y: f32,
}

/// How the views' per-frame state advances with simulation time.
///
/// The original updates every view once per rendered frame with the frame
/// time (0x8435D0, 0x822510, 0x820D10), and several rules have no dt: the
/// head-cam springs' centering, the look smoothing, the close-follow damp and
/// the spin hold's frame counts. So its camera behaves differently at
/// different frame rates (the head springs are about 1.6 times as stiff at
/// 165 fps as at 60 fps, docs/CAMERA_VIEWS.md §3).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Clock {
    /// One step per physics tick, as the original at 62.5 fps, whatever the
    /// display rate [AUTHORED: the owner's rule F320, no hardware-dependent
    /// behaviour; F286 fixed step].
    #[default]
    Tick,
    /// The original's per-frame rules at a fixed rate: simulation time is cut
    /// into camera frames of 1/fps, each fed the car state interpolated
    /// between the ticks (diagnostics `camera_fps`, for comparisons with the
    /// original at 60 or 165 fps).
    Fps(f32),
    /// Every shot is one rendered frame of its own dt (live-trace replays).
    #[cfg_attr(not(test), allow(dead_code))]
    Frame,
}

/// Whole physics ticks covered by `dt` plus what was left over [AUTHORED]:
/// the original updates its look and head-cam state once per rendered frame
/// without dt in places, the port once per tick (0.016 s), as the original at
/// 62.5 fps.
fn steps(pending: &mut f32, dt: f32) -> u32 {
    frames(pending, dt, grid_tools::source_vehicle::DELTA)
}

/// Whole frames of `step` seconds covered by `dt` plus what was left over;
/// a gap over MAX_STEPS frames (replay seek) drops the rest.
fn frames(pending: &mut f32, dt: f32, step: f32) -> u32 {
    *pending += dt.max(0.);
    let steps = (*pending / step + 1e-3).floor();
    *pending = if steps > MAX_STEPS { 0. } else { (*pending - steps * step).max(0.) };
    steps.min(MAX_STEPS) as u32
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
/// The weight is the car's own vehicle property car +0xFE0 (target +0xC is
/// the participant, its +0x24 the car; +0xE78 the properties copy, record
/// +0x168 from CSV column 330 by 0x9AEC60): 0 for every shipped car.
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

/// One update of the chase's impact knock (0x81F610..0x81F981; emulated,
/// local/cam/spin/knock.txt). The envelope reads |v| (target +0xC): a change
/// above 50 m/s² (not divided by a zero dt) starts it at 1 s when idle, or
/// restarts its plateau at 0.8 s; its weight rises as √ over 0.2 s, holds 1
/// from 0.8 to 0.4 s left and falls as √(t/0.4), and the time runs down by dt
/// without a hold. The spring of the head-cam (0x83E6D0) with damping 45,
/// inertia 1.25, centering 3 and at most 0.75 m takes the eye's velocity
/// (`eye`, before the knock, look turn and shake) in world axes; the offset,
/// weighted, moves the eye.
pub fn knock_step(k: &mut Knock, velocity: Vec3, eye: Vec3, dt: f32) {
    let speed = velocity.length();
    let mut change = (speed - k.speed).abs();
    if 0. < dt {
        change /= dt;
    }
    k.speed = speed;
    if KNOCK_ACCELERATION < change {
        if k.timer <= 0. {
            k.timer = KNOCK_TIME;
        } else if KNOCK_HOLD_START * KNOCK_TIME < k.timer && k.timer < KNOCK_HOLD_END * KNOCK_TIME {
            k.timer = KNOCK_HOLD_END * KNOCK_TIME;
        }
    }
    let t = k.timer;
    k.blend = if KNOCK_HOLD_END * KNOCK_TIME < t {
        pow(1. - (t / KNOCK_TIME - KNOCK_HOLD_END) / (1. - KNOCK_HOLD_END), KNOCK_POWER)
    } else if KNOCK_HOLD_START * KNOCK_TIME < t {
        1.
    } else if 0. < t {
        pow(t / KNOCK_TIME / KNOCK_HOLD_START, KNOCK_POWER)
    } else {
        0.
    };
    k.timer -= dt;
    // 0x840710: the eye's velocity, zero on the first update and for dt 0.
    let input = match k.eye {
        Some(last) if dt != 0. => (eye - last) * (1. / dt),
        _ => Vec3::ZERO,
    };
    k.eye = Some(eye);
    spring_step(&KNOCK_SPRING, &mut k.spring, dt, Mat3::IDENTITY, input);
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
#[allow(clippy::too_many_arguments)]
fn chase_shot(
    c: &Chase,
    h: &mut ChaseState,
    sample: &Sample,
    dt: f32,
    steps: u32,
    look: Look,
    orbit: impl Fn(Vec3) -> Vec3,
    per_rendered_frame: bool,
) -> Shot {
    // The frame's look messages (0x8348F0 → 0x7FE690), one per step.
    for _ in 0..steps {
        h.look = chase_look_message(h.look, look, sample.speed);
    }
    let pose = &sample.pose;
    let yaw = heading(pose);
    // The lag integrates dt · ω_y (target +0x10, lane 1); a replay without
    // the angular velocity takes the heading change [AUTHORED].
    let turn = if sample.rate_known {
        dt * sample.angular_velocity.y
    } else {
        h.heading.map_or(0., |prev| wrap(yaw - prev))
    };
    h.heading = Some(yaw);
    // Record reset (0x831D20): the spin speed starts at |ω|.
    h.spin.speed.get_or_insert(sample.angular_velocity.length());
    let advance = dt > 0.;
    if advance {
        h.suspension = suspension_heave(c, h.suspension, &sample.wheels, dt);
        h.lag = chase_lag(c, h.lag, turn, sample.speed, dt);
        (h.roll, h.roll_weight) = roll_step(c, h.roll, sample.angular_velocity.y, sample.speed, dt);
    }
    let lag = drift_lag(h.lag, sample.velocity, pose, sample.drift_weight);
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
            // Once per rendered frame without dt in the original; per 1/60 s
            // in Clock::Tick [AUTHORED].
            let damp = if per_rendered_frame { c.follow_damp } else { per_frame(c.follow_damp, dt) };
            h.follow += (slope - h.follow) * damp;
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
    // Both impact envelopes count every update, the reset's (dt 0) too.
    if advance || !h.started {
        spin_step(&mut h.spin, sample, frame.z_axis.truncate(), dt);
        knock_step(&mut h.knock, sample.velocity, eye, dt);
    }
    if advance {
        h.height_latched |= weight == 1.;
        h.started = true;
    }
    let target_local = c.target + Vec3::Y * (c.suspension_scale * h.suspension);
    let target = pose.transform_point3(target_local);
    // AUTHORED free look: orbit the eye about the target in the car frame.
    let offset = frame.inverse().transform_vector3(eye - target);
    eye += frame.transform_vector3(orbit(offset) - offset);
    let direction = (target - eye).normalize();
    // The knock moves the eye in world axes without turning the view.
    let eye = eye + h.knock.blend * h.knock.spring.off;
    let mut shot = Shot {
        eye,
        rotation: Transform::from_translation(eye)
            .looking_to(direction, rolled_up(direction, roll_angle(c, h)))
            .rotation,
        fov: c.fov.to_radians(),
        near: None,
    };
    chase_look_turn(&mut shot, h.look, pose.w_axis.truncate());
    shot
}

/// The original's double-precision CRT `atan2` (0xBF877D) on f32 operands.
fn atan2(y: f32, x: f32) -> f32 {
    f64::from(y).atan2(f64::from(x)) as f32
}

/// One chase look message (0x7FE690; emulated, 4000 cases): the look point
/// (A4, A8 + 0.1) follows the input point (x, y + 0.1) in polar form by the
/// blend 0.1 + 0.4 w per message, snapping to it when the angle step reaches
/// 3.2 (1 − w) + w, with w = clamp(0.2 · speed, 0, 1) of the signed car speed.
/// Above 5 m/s a full key snaps to 84.3°; at rest the turn eases in.
pub fn chase_look_message(point: Vec2, look: Look, speed: f32) -> Vec2 {
    let w = (speed * 0.2).clamp(0., 1.);
    let w = if w.is_nan() { 0. } else { w };
    let snap = (1. - w) * CHASE_LOOK_SNAP + w;
    let blend = w * CHASE_LOOK_SPEED_BLEND + CHASE_LOOK_BLEND;
    let q = point.y + CHASE_LOOK_AHEAD;
    let yy = look.y + CHASE_LOOK_AHEAD;
    let a1 = atan2(point.x, q);
    let a2 = atan2(look.x, yy);
    let mut d = a2 - a1;
    if -CHASE_LOOK_PI > d {
        d += CHASE_LOOK_TAU;
    }
    if d > CHASE_LOOK_PI {
        d -= CHASE_LOOK_TAU;
    }
    if snap > d.abs() {
        let angle = d * blend + a1;
        let r1 = (q * q + point.x * point.x).sqrt();
        let r2 = (yy * yy + look.x * look.x).sqrt();
        let r = (r2 - r1) * blend + r1;
        let (sin, cos) = (f64::from(angle).sin() as f32, f64::from(angle).cos() as f32);
        Vec2::new(sin * r, cos * r - CHASE_LOOK_AHEAD)
    } else {
        Vec2::new(look.x, yy - CHASE_LOOK_AHEAD)
    }
}

/// The chase look turn after the chase update (0x7FEC40; emulated and
/// live-checked): view and eye turn by atan2(A4, A8 + 0.1) about the world
/// vertical through the car's model origin (record +0x110); toward +X for a
/// positive angle. No input leaves the point at (0, 0) and the turn at 0.
fn chase_look_turn(shot: &mut Shot, point: Vec2, origin: Vec3) {
    let a = atan2(point.x, point.y + CHASE_LOOK_AHEAD);
    if a == 0. {
        return;
    }
    let turn = Quat::from_rotation_y(a);
    shot.rotation = turn * shot.rotation;
    shot.eye = origin + turn * (shot.eye - origin);
}

/// The shake noise table (0xE7B230), filled by the static initializer
/// 0x7F2B40: T[i] = 0.25 · Σ (0.5 · w + 0.5) over w = sin((x + 0.12) ·
/// 42.56), cos(7.81 x), sin(2.53 x), cos(x − 1.54), x = i · 0.0033333334,
/// in x87 precision with the f32 constants (0xCB22BC, 0xCFE6E8..0xCFE6F8,
/// 0xCB01AC, 0xCAF88C); f64 with the constants widened reproduces all 6000
/// entries of the emulated original.
fn shake_table() -> &'static [f32; SHAKE_TABLE] {
    static TABLE: std::sync::OnceLock<[f32; SHAKE_TABLE]> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        let c = |v: f32| f64::from(v);
        std::array::from_fn(|i| {
            let x = i as f64 * c(0.003_333_333_4);
            let wave = |w: f64| 0.5 * w + 0.5;
            (c(0.25)
                * (wave(((x + c(0.12)) * c(42.56)).sin())
                    + wave((x * c(7.81)).cos())
                    + wave((x * c(2.53)).sin())
                    + wave((x - c(1.54)).cos()))) as f32
        })
    })
}

/// Shake noise n(lead, scale) at shake time `time` (0x7FE510): the table read
/// at 300 · |(time + lead) · scale|, linearly interpolated and wrapping every
/// 20 s, as amplitude · (2 v − 1) limited to ±1.
pub fn shake_noise(time: f32, lead: f32, scale: f32, amplitude: f32) -> f32 {
    let table = shake_table();
    let u = ((time + lead) * scale).abs() * SHAKE_RATE;
    let i = u as usize;
    let (i0, i1) = (i % SHAKE_TABLE, (i % SHAKE_TABLE + 1) % SHAKE_TABLE);
    let f = u - i as f32;
    let v = ((table[i1] - table[i0]) * f + table[i0]) * 2. - 1.;
    (v * amplitude).clamp(-1., 1.)
}

/// One shake update (0x815960; emulated, live-checked on the bonnet): the
/// amplitude jumps up to the speed ramp between shakeMinSpeed and
/// shakeMaxSpeed of the signed car speed (m/s), the shake time advances and
/// the amplitude decays at shakeDamping per second. All-zero shakes do
/// nothing.
pub fn shake_step(k: &Shake, h: &mut MountedState, speed: f32, dt: f32) {
    if k.angle == 0. && k.scale_x == 0. && k.scale_y == 0. {
        return;
    }
    let speed = speed.min(k.max_speed).max(k.min_speed);
    let ramp = (speed - k.min_speed) / (k.max_speed - k.min_speed);
    if ramp > h.shake_amplitude {
        h.shake_amplitude = ramp;
    }
    h.shake_time += dt;
    h.shake_amplitude -= k.damping * h.shake_amplitude * dt;
    if h.shake_amplitude.abs() < SHAKE_MIN_AMPLITUDE {
        h.shake_amplitude = 0.;
    }
}

/// The shake applied to a framed view (end of 0x815960): a pitch of
/// shakeAngle · n(0, rotationSpeed) degrees about the view's right axis
/// (positive looks down), and the eye moved along the view's right by
/// n(0, 1) · shakeScaleX and its up by n(0.1, 1) · shakeScaleY.
fn shake_shot(k: &Shake, h: &MountedState, shot: &mut Shot) {
    if k.angle == 0. && k.scale_x == 0. && k.scale_y == 0. {
        return;
    }
    // The original's rows: right is the view's +X of the game frame, the
    // camera's −X here (facing_forward).
    let right = shot.rotation * Vec3::NEG_X;
    let up = shot.rotation * Vec3::Y;
    if k.angle != 0. {
        let pitch = k.angle * DEGREES * shake_noise(h.shake_time, 0., k.rotation_speed, h.shake_amplitude);
        shot.rotation *= Quat::from_rotation_x(-pitch);
    }
    if k.scale_x != 0. || k.scale_y != 0. {
        let vertical = shake_noise(h.shake_time, SHAKE_LEAD, 1., h.shake_amplitude);
        let side = shake_noise(h.shake_time, 0., 1., h.shake_amplitude);
        shot.eye += right * (side * k.scale_x) + up * (vertical * k.scale_y);
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

/// One step of a head-cam spring (0x83E6D0; emulated and live-checked,
/// docs/CAMERA_VIEWS.md §2): the input velocity's change, filtered with a
/// half-life of 1/60 s, pushes the car-frame offset back by inertia; damping
/// takes a constant speed off per step (Coulomb), centering pulls the offset
/// back once per step (no dt: the original's spring is per rendered frame),
/// and the offset stays inside the ellipsoid max · (x scale, y scale, 1), where
/// an outward velocity is halved and loses its radial part. `rotation` turns
/// car to world.
pub fn spring_step(p: &Spring, s: &mut SpringState, dt: f32, rotation: Mat3, input: Vec3) {
    if dt.is_nan() || dt <= SPRING_MIN_STEP {
        s.prev = input;
        return;
    }
    let raw = (input - s.prev) / dt;
    s.prev = input;
    let q = pow(0.5, f64::from(FRAME_RATE * dt));
    s.acc = (1. - q) * raw + q * s.acc;
    s.vel -= rotation.transpose() * s.acc * p.inertia * dt;
    let stop = p.damping * dt;
    let speed2 = s.vel.length_squared();
    s.vel = if stop * stop < speed2 && speed2 >= SPRING_MIN_SQUARE {
        let speed = speed2.sqrt();
        (speed - stop) * (s.vel / speed)
    } else {
        Vec3::ZERO
    };
    s.vel -= p.centering * s.off;
    s.off += dt * s.vel;
    let scale = p.scale.extend(1.);
    let divide = Vec3::select(scale.cmpne(Vec3::ZERO), scale, Vec3::ONE);
    s.off /= divide;
    s.vel /= divide;
    let r2 = s.off.length_squared();
    if r2 > p.max_offset * p.max_offset {
        let n = s.off / r2.sqrt();
        let e = s.off - p.max_offset * n;
        s.off = p.max_offset * n;
        if 0. < s.vel.dot(e) {
            s.vel *= 0.5;
            let n = e.normalize();
            s.vel -= s.vel.dot(n) * n;
        }
    }
    s.off *= scale;
    s.vel *= scale;
}

/// The seat's world velocity, the head-cam's input (0x820D10 step 5):
/// v + ω × (position turned to world).
pub fn head_input(c: &Head, rotation: Mat3, velocity: Vec3, angular_velocity: Vec3) -> Vec3 {
    velocity + angular_velocity.cross(rotation * c.position)
}

/// One head-cam frame (0x820D10 steps 6–7): the body spring, then the head
/// spring driven by the input plus the body's new velocity in world.
pub fn head_step(c: &Head, h: &mut HeadState, dt: f32, rotation: Mat3, input: Vec3) {
    spring_step(&c.body, &mut h.body, dt, rotation, input);
    spring_step(&c.head, &mut h.head, dt, rotation, input + rotation * h.body.vel);
}

/// The head-cam's look messages of one frame (0x801230, emulated): the type 1
/// message moves the smoothed input toward (x, y) by 0.2 and marks looking
/// while the input is not zero; the type 2 message (look back, x = 0 while
/// the head-cam shows) then decays it by 0.8 unless looking.
pub fn head_look_message(h: &mut HeadState, look: Look) {
    let k = LOOK_BLEND;
    h.look.x = h.look.x * (1. - k) + look.x * k;
    h.look.y = (1. - k) * h.look.y + k * look.y;
    h.looking = look.x != 0. || look.y != 0.;
    if !h.looking {
        h.look *= 1. - k;
    }
}

/// The look curve of 0x814BC0: 0.5 (2|x|)² below |x| = 0.5, else
/// 1 − 0.5 (2 (1 − |x|))², with the sign of x.
fn look_curve(x: f32) -> f32 {
    let a = x.abs();
    let v = if a < 0.5 { 0.5 * pow(2. * a, 2.) } else { 1. - 0.5 * pow(2. * (1. - a), 2.) };
    if x < 0. { -v } else { v }
}

/// The head-cam's look angles of one frame (0x814BC0 → 0x801D10, emulated):
/// yaw toward maxHeadLook · curve(x) degrees and pitch toward maxHeadLook ·
/// curve(y) · 0.3, each at most 180°/s.
pub fn head_look_step(c: &Head, h: &mut HeadState, dt: f32) {
    let goal = Vec2::new(
        c.max_look * look_curve(h.look.x) * DEGREES,
        c.max_look * look_curve(h.look.y) * HEAD_LOOK_PITCH,
    );
    let rate = HEAD_LOOK_RATE * dt * DEGREES;
    h.look_angle += (goal - h.look_angle).clamp(Vec2::splat(-rate), Vec2::splat(rate));
}

/// The head-cam's view of its state (0x820D10 steps 8–9 and 14): the eye at
/// the seat raised by bodyLength and moved by the body offset; the view turned
/// by the head offset h about (h.z, 0, −h.x) by |h| radians (a forward offset
/// looks down, emulated), then by the look yaw (toward +X) and pitched by the
/// look pitch (up) and the tilt. Buffeting, TrackIR and transitions are
/// inactive in a race (docs/CAMERA_VIEWS.md).
pub fn head_view(c: &Head, h: &HeadState, pose: &Mat4) -> Shot {
    let (_, rotation, _) = pose.to_scale_rotation_translation();
    let eye = pose.transform_point3(c.position + Vec3::Y * c.body_length + h.body.off);
    let o = h.head.off;
    let axis = Vec3::new(o.z, 0., -o.x);
    let turn = if axis.length_squared() > SPRING_MIN_SQUARE {
        Quat::from_axis_angle(axis.normalize(), o.length())
    } else {
        Quat::IDENTITY
    };
    Shot {
        eye,
        rotation: rotation
            * turn
            * Quat::from_rotation_y(h.look_angle.x)
            * facing_forward()
            * Quat::from_rotation_x(c.tilt.to_radians() + h.look_angle.y),
        fov: c.fov.to_radians(),
        near: Some(c.near),
    }
}

/// The head-cam (0x842DD0 → 0x820D10). The reset (0x82C0E0) starts both
/// springs from the car's velocity at rest. The original steps once per
/// rendered frame, so its springs stiffen with the frame rate; the port steps
/// once per `step` of the camera clock (by default a physics tick, 0.016 s, as
/// the original at 62.5 fps) and spreads a shot's input change evenly over its
/// steps [AUTHORED]. Each step takes the frame's look messages first (the
/// controller runs before the views [I]).
fn head_shot(c: &Head, h: &mut HeadState, sample: &Sample, steps: u32, step: f32, look: Look) -> Shot {
    let rotation = Mat3::from_mat4(sample.pose);
    let input = head_input(c, rotation, sample.velocity, sample.angular_velocity);
    let from = *h.input.get_or_insert_with(|| {
        h.body.prev = sample.velocity;
        h.head.prev = sample.velocity;
        input
    });
    for k in 1..=steps {
        head_look_message(h, look);
        head_step(c, h, step, rotation, from.lerp(input, k as f32 / steps as f32));
        head_look_step(c, h, step);
    }
    if steps > 0 {
        h.input = Some(input);
    }
    head_view(c, h, &sample.pose)
}

/// Advances `history` by the simulation time `dt` with the look input `look`
/// and frames the view. `orbit` rotates a chase eye about its target (AUTHORED
/// free look).
pub fn shot(
    rig: &Rig,
    history: &mut History,
    sample: &Sample,
    dt: f32,
    look: Look,
    orbit: impl Fn(Vec3) -> Vec3,
) -> Shot {
    shot_clocked(Clock::Tick, rig, history, sample, dt, look, orbit)
}

/// [`shot`] on the given camera clock.
pub fn shot_clocked(
    clock: Clock,
    rig: &Rig,
    history: &mut History,
    sample: &Sample,
    dt: f32,
    look: Look,
    orbit: impl Fn(Vec3) -> Vec3,
) -> Shot {
    match clock {
        Clock::Tick => {
            let steps = steps(&mut history.pending, dt);
            advance(rig, history, sample, dt, steps, grid_tools::source_vehicle::DELTA, look, &orbit, false)
        }
        Clock::Frame => advance(rig, history, sample, dt, u32::from(dt > 0.), dt, look, &orbit, true),
        Clock::Fps(fps) => {
            let step = 1. / fps;
            let n = frames(&mut history.pending, dt, step);
            let from = history.sample.unwrap_or(*sample);
            if n == 0 {
                // No camera frame ends in this shot: frame the car where it
                // is without advancing the views.
                return advance(rig, &mut history.clone(), sample, 0., 0, step, look, &orbit, true);
            }
            let mut shot = None;
            for k in 1..=n {
                let s = sample.from(&from, k as f32 / n as f32);
                shot = Some(advance(rig, history, &s, step, 1, step, look, &orbit, true));
            }
            history.sample = Some(*sample);
            shot.expect("at least one frame")
        }
    }
}

/// One update of a view over `dt` with `steps` per-frame steps of `step`
/// seconds (look messages, head springs, shake); `per_rendered_frame` applies
/// the original's per-frame chase damp as is.
#[allow(clippy::too_many_arguments)]
fn advance(
    rig: &Rig,
    history: &mut History,
    sample: &Sample,
    dt: f32,
    steps: u32,
    step: f32,
    look: Look,
    orbit: &impl Fn(Vec3) -> Vec3,
    per_rendered_frame: bool,
) -> Shot {
    let (_, rotation, position) = sample.pose.to_scale_rotation_translation();
    match rig {
        Rig::Chase(c) => chase_shot(c, &mut history.chase, sample, dt, steps, look, orbit, per_rendered_frame),
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
            // Look turn: 0.2 of the way to the input per message (0x840A30).
            if m.look != MountedLook::Fixed {
                for _ in 0..steps {
                    h.look += (look.x - h.look) * LOOK_BLEND;
                }
            }
            // The bonnet's eye rises and moves forward with the turn (0x822510).
            let lift = if m.look == MountedLook::TurnAndLift {
                let lift = h.look.abs() * BONNET_LOOK_LIFT;
                Vec3::new(0., lift, lift)
            } else {
                Vec3::ZERO
            };
            // offsetOnly places the bare offset at the car's position.
            let eye = if m.offset_only {
                position + m.offset
            } else {
                position + rotation * (m.offset + h.shift * m.final_movement_scale + lift)
            };
            // Positive pitch looks down, positive yaw turns toward +X, yaw π
            // looks back (0x803270: R(roll) · P(pitch) · Y(yaw + look),
            // emulated).
            let view = Quat::from_rotation_y(m.yaw + h.look)
                * facing_forward()
                * Quat::from_rotation_x(-m.pitch)
                * Quat::from_rotation_z(m.roll);
            // The shake runs last, once per step (0x822EDF → 0x815960).
            for _ in 0..steps {
                shake_step(&m.shake, h, sample.speed, step);
            }
            let mut shot = Shot {
                eye,
                rotation: rotation * mounted_twist(m, h, sample) * view,
                fov: m.fov.to_radians(),
                near: Some(m.near),
            };
            shake_shot(&m.shake, h, &mut shot);
            shot
        }
        Rig::Head(c) => head_shot(c, &mut history.head, sample, steps, step, look),
    }
}

/// Length of a Change View blend between two views on the same side of the
/// glass (0xE4E160), and the samples of its lift (0xE4E15C).
const TRANSITION_TIME: f32 = 0.5;
const LIFT_SAMPLES: u32 = 32;
/// The chase views' near plane (record +0x34; the app's default near).
const CHASE_NEAR: f32 = 0.1;

/// A Change View blend (0x82A4E0, manager +0x4D0): from the shown view to the
/// next over `duration`, the eye on a cubic raised by `lift` over the car.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transition {
    pub from: usize,
    pub to: usize,
    pub t: f32,
    pub duration: f32,
    pub lift: f32,
    /// Simulation time not yet stepped (the views' clock).
    pending: f32,
}

/// 0x7F9CA0's "inside": the head-cam, or a view with highResInterior.
fn inside(rig: &Rig) -> bool {
    match rig {
        Rig::Chase(_) => false,
        Rig::Mounted(m) => m.high_res_interior,
        Rig::Head(_) => true,
    }
}

fn near(rig: &Rig) -> f32 {
    match rig {
        Rig::Chase(_) => CHASE_NEAR,
        Rig::Mounted(m) => m.near,
        Rig::Head(h) => h.near,
    }
}

/// The car box segment test of 0x7F73B0 → 0xA6A530 for the segment `top`
/// → `low`: the fraction from `top` and the face normal where it enters; a
/// segment starting inside or missing the box does not hit [I: slab test in
/// the box frame, the setup harness's stand-in].
fn box_hit(b: &grid_world::phantom_post::Box, top: Vec3, low: Vec3) -> Option<(f32, Vec3)> {
    let centre = Vec3::new(b.centre[0], b.centre[1], b.centre[2]);
    let (mut near, mut far, mut normal) = (0f32, 1f32, None);
    for (k, axis) in b.axes.iter().enumerate() {
        let axis = Vec3::new(axis[0], axis[1], axis[2]);
        let (p, d, half) = ((top - centre).dot(axis), (low - top).dot(axis), b.half[k]);
        if d == 0. {
            if p.abs() > half {
                return None;
            }
            continue;
        }
        let (a, c) = ((-half - p) / d, (half - p) / d);
        let (a, c) = if a > c { (c, a) } else { (a, c) };
        if a > near {
            near = a;
            normal = Some(if d > 0. { -axis } else { axis });
        }
        far = far.min(c);
        if near > far {
            return None;
        }
    }
    normal.map(|n| (near, n))
}

/// 0x7F9CA0's lift: the eye path from `from` to `to` (world) sampled 32
/// times, each sample cast down onto the car box from half the eye distance
/// above; the highest clearance scaled to an ellipse peak.
pub fn transition_lift(from: Vec3, to: Vec3, margin: f32, car: &grid_world::phantom_post::Box) -> f32 {
    let d = to - from;
    let e = d.normalize_or_zero();
    let a = Vec3::new(e.z, 0., -e.x);
    let u = e.cross(a).normalize_or_zero();
    let length = e.dot(d);
    let half = length * 0.5;
    let mut lift = 0f32;
    for i in 1..=LIFT_SAMPLES {
        let s = i as f32 * length / (LIFT_SAMPLES + 1) as f32;
        let low = from + s * e - margin * u;
        let top = low + u * half;
        let Some((frac, normal)) = box_hit(car, top, low) else {
            continue;
        };
        if normal.dot(u) <= 0. {
            continue;
        }
        let x = (s - half) / length * 2.;
        let h = ((1. - frac) * half + margin) / (1. - x * x).sqrt();
        lift = lift.max(h);
    }
    lift
}

/// One evaluation of the blend (0x82A4E0) at `t` between the live shots.
pub fn transition_shot(tr: &Transition, from: &Shot, to: &Shot) -> Shot {
    let f = if tr.duration != 0. { tr.t / tr.duration } else { 1. };
    let (p0, p3) = (from.eye, to.eye);
    let d = (p3 - p0).normalize_or_zero();
    // '+y': the world up made perpendicular to the path.
    let k = Vec3::new(-d.x * d.y, d.x * d.x + d.z * d.z, -d.y * d.z).normalize_or_zero();
    let (p1, p2) = (p0 + tr.lift * k, p3 + tr.lift * k);
    let g = 1. - f;
    let eye = g * g * g * p0 + 3. * f * g * g * p1 + 3. * f * f * g * p2 + f * f * f * p3;
    let r = if f <= 0.5 {
        0.5 - 0.5 * pow(1. - 2. * f, 0.6)
    } else {
        0.5 + 0.5 * pow(2. * f - 1., 0.6)
    };
    let (mut q0, q1) = (from.rotation, to.rotation);
    let mut dot = q0.dot(q1);
    if dot < 0. {
        q0 = -q0;
        dot = -dot;
    }
    let (w0, w1) = if dot < 0.999 {
        let th = dot.acos();
        ((((1. - r) * th).sin()) / th.sin(), (r * th).sin() / th.sin())
    } else {
        (1. - r, r)
    };
    let (n0, n1) = (from.near.unwrap_or(CHASE_NEAR), to.near.unwrap_or(CHASE_NEAR));
    Shot {
        eye,
        // The original keeps the weighted sum; the port renders it normalised
        // [AUTHORED].
        rotation: Quat::from_vec4(w0 * Vec4::from(q0) + w1 * Vec4::from(q1)).normalize(),
        fov: (to.fov - from.fov) * f + from.fov,
        near: Some((n1 - n0) * f + n0),
    }
}

/// What a view draws of the player car (0x607BC0 per frame, 0x99C2F0,
/// 0x9FDEF0; docs/CAMERA_VIEWS.md "Own car per view").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnCar {
    /// Every exterior part, no cockpit.
    Whole,
    /// Nothing (the bumper views).
    Hidden,
    /// The cockpit and the exterior nodes `int_<stem>.xml` keeps
    /// (highResInterior: the head-cam).
    Interior,
}

impl OwnCar {
    /// Whether a player part is drawn: `interior` for a cockpit part, else
    /// `joint` is its node without the LOD prefix.
    pub fn shows(self, interior: bool, joint: &str, active: &[String]) -> bool {
        match self {
            OwnCar::Whole => !interior,
            OwnCar::Hidden => false,
            OwnCar::Interior => interior || active.iter().any(|a| a == joint),
        }
    }
}

/// The player car's rule this frame: the driving view's, the whole car
/// during a replay or without a cockpit [AUTHORED: replays pick their own
/// views].
pub fn own_car(cameras: Option<&Cameras>, replay: Option<&crate::replay::Replay>, cockpit: bool) -> OwnCar {
    if replay.is_some_and(|r| r.session.is_some()) {
        return OwnCar::Whole;
    }
    match cameras.map_or(OwnCar::Whole, Cameras::own_car) {
        OwnCar::Interior if !cockpit => OwnCar::Whole,
        rule => rule,
    }
}

/// The car's views, all updated every frame as the original's manager in a
/// race (+0x122 = 1), and the Change View blend between them.
#[derive(Resource, Clone, Debug)]
pub struct Cameras {
    pub views: Views,
    pub current: usize,
    /// The view on screen (the current view's reverse while looking back).
    shown: Option<usize>,
    /// Every view's state and its last shot (the original's records).
    histories: Vec<History>,
    shots: Vec<Option<Shot>>,
    /// Segment, ticks and pose of the last frame.
    last: Option<(u64, u64, Mat4)>,
    transition: Option<Transition>,
    /// Change View turned look back off until Look Back is released (+0x121).
    back_cleared: bool,
    /// Where the chosen view is saved at exit; None in tests.
    profile: Option<PathBuf>,
    /// Camera clock (diagnostics `camera_fps`; Clock::Tick by default).
    pub clock: Clock,
}

impl Cameras {
    pub fn new(views: Views, saved: &str, profile: Option<PathBuf>) -> Self {
        let current = views.restore(saved);
        let n = views.0.len();
        Self {
            views,
            current,
            shown: None,
            histories: vec![History::default(); n],
            shots: vec![None; n],
            last: None,
            transition: None,
            back_cleared: false,
            profile,
            clock: Clock::Tick,
        }
    }

    /// Change View (0x833DD0): starts the blend from the current view to the
    /// next visible one, placed with `sample`'s pose; `car` is the car's
    /// collision box for the lift. A press during a blend is dropped.
    pub fn change_view(&mut self, sample: &Sample, car: Option<&grid_world::phantom_post::Box>) -> Option<Transition> {
        if self.transition.is_some() {
            return None;
        }
        self.back_cleared = true;
        let (from, to) = (self.current, self.views.next(self.current));
        let (a, b) = (&self.views.0[from].rig, &self.views.0[to].rig);
        let (duration, lift) = if from == to || inside(a) != inside(b) {
            (0., 0.)
        } else if inside(a) || inside(b) {
            (TRANSITION_TIME, 0.)
        } else {
            // The records' eyes of the last frame, carried by the car to now.
            let carried = |i: usize| {
                let (_, _, pose) = self.last?;
                let eye = self.shots[i]?.eye;
                Some((sample.pose * pose.inverse()).transform_point3(eye))
            };
            let lift = match (carried(from), carried(to), car) {
                (Some(p0), Some(p3), Some(car)) => transition_lift(p0, p3, near(b), car),
                // No collision geometry: no lift [AUTHORED].
                _ => 0.,
            };
            (TRANSITION_TIME, lift)
        };
        let tr = Transition { from, to, t: 0., duration, lift, pending: self.histories[from].pending };
        self.transition = Some(tr);
        Some(tr)
    }

    pub fn ident(&self) -> &str {
        &self.views.0[self.current].ident
    }

    /// Stable original bumper policy: 607BC0 -> 99C2F0 -> 9CB0B0 selects
    /// one-past-last model LOD; 8F4F30 disables every real LOD node.
    /// The app adapts this only to the main-view mesh list, keeping the source shot.
    pub fn hides_player_model(&self) -> bool {
        matches!(
            self.views.0[self.shown.unwrap_or(self.current)].ident.as_str(),
            "bumper" | "bumper_reverse"
        )
    }

    /// Whether the view on screen hides the driver's head (DisableDriverHead).
    pub fn driver_head_hidden(&self) -> bool {
        self.views.0[self.shown.unwrap_or(self.current)].disable_driver_head
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

    /// The view on screen: the current one, or its reverse while looking back.
    fn on_screen(&self, back: bool) -> usize {
        match back {
            true => self.views.0[self.current].reverse.unwrap_or(self.current),
            false => self.current,
        }
    }

    /// Advances every view by the ticks since the last frame (0x82C550: slot
    /// 11 for all, nothing reset on a view change) and frames the shown one,
    /// or the blend while Change View runs.
    pub fn frame(&mut self, sample: &Sample, back: bool, look: Look, orbit: impl Fn(Vec3) -> Vec3) -> Shot {
        // Change View turns look back off; it takes a new press (0x8348F0).
        self.back_cleared &= back;
        let back = back && !self.back_cleared;
        let reset = self.last.is_none_or(|(segment, ticks, _)| {
            segment != sample.segment || sample.ticks < ticks
        });
        if reset {
            self.histories.fill(History::default());
            self.shots.fill(None);
            self.transition = None;
        }
        let ticks = match self.last {
            Some((_, t, _)) if !reset => sample.ticks - t,
            _ => 0,
        };
        self.last = Some((sample.segment, sample.ticks, sample.pose));
        let dt = ticks as f32 * grid_tools::source_vehicle::DELTA;
        // The look messages go to every view (0x8348F0); the free look only
        // to the one on screen.
        let shown = self.on_screen(back);
        for (i, view) in self.views.0.iter().enumerate() {
            let history = &mut self.histories[i];
            self.shots[i] = Some(match i == shown {
                true => shot_clocked(self.clock, &view.rig, history, sample, dt, look, &orbit),
                false => shot_clocked(self.clock, &view.rig, history, sample, dt, look, |o| o),
            });
        }
        let mut blend = None;
        if let Some(tr) = &mut self.transition {
            let (frames, step) = match self.clock {
                Clock::Tick => (steps(&mut tr.pending, dt), grid_tools::source_vehicle::DELTA),
                Clock::Fps(fps) => (frames(&mut tr.pending, dt, 1. / fps), 1. / fps),
                Clock::Frame => (u32::from(dt > 0.), dt),
            };
            // 0x82A4E0 once per camera frame: past the duration it ends and
            // the manager commits the new view (+0x128 = +0x510).
            let mut ended = false;
            for _ in 0..frames {
                tr.t += step;
                ended |= tr.t > tr.duration;
            }
            if !ended {
                let shot = |i: usize| self.shots[i].expect("every view framed");
                blend = Some(transition_shot(tr, &shot(tr.from), &shot(tr.to)));
            }
        }
        if blend.is_none()
            && let Some(tr) = self.transition.take()
        {
            self.current = tr.to;
            if cfg!(not(test)) {
                println!("drive_camera event=commit view={} tick={}", self.views.0[tr.to].ident, sample.ticks);
            }
        }
        let shown = self.on_screen(back);
        self.shown = Some(shown);
        let shot = blend.unwrap_or_else(|| self.shots[shown].expect("every view framed"));
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

    /// What the view on screen draws of the player car (the current view
    /// until a blend commits): the bumper views none of it (0x607BC0 ident
    /// test), a highResInterior view its cockpit [I: the head-cam's own
    /// highResInterior flag is taken as "inside"].
    pub fn own_car(&self) -> OwnCar {
        let Some(view) = self.shown.map(|i| &self.views.0[i]) else {
            return OwnCar::Whole;
        };
        if matches!(view.ident.as_str(), "bumper" | "bumper_reverse") {
            OwnCar::Hidden
        } else if inside(&view.rig) {
            OwnCar::Interior
        } else {
            OwnCar::Whole
        }
    }

    /// The shown view's history.
    #[cfg(test)]
    fn history(&self) -> &History {
        &self.histories[self.shown.unwrap_or(self.current)]
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
    let mut cameras = Cameras::new(views, parse_profile(&text), Some(path));
    // Diagnostics camera_fps: the original's per-frame rules at that rate.
    if let Some(fps) = crate::settings::diagnostics().camera_fps.filter(|f| *f > 0.) {
        cameras.clock = Clock::Fps(fps);
        println!("drive_camera event=clock fps={fps}");
    }
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
            look: MountedLook::Fixed,
            shake: Shake::default(),
            high_res_interior: false,
        }
    }

    /// Diagnostic: the port's views driven by a decoded live trace of the
    /// original (local/cam/live/decode.py: t, step, speed, state, wheels x4,
    /// pose, angular, velocity, ...). Writes per row and view: eye, forward,
    /// up, then chase lag and spin blend, bonnet tilt and shift, head-cam body
    /// and head offsets, chase spin time and held forward. Also reads the
    /// port's own scenes (grid-probes `grid-camera-scene`).
    /// GRID_CAMERA_TRACE=IN, GRID_CAMERA_OUT=OUT, GRID_GAME_ROOT.
    #[test]
    #[ignore]
    fn replay_live_trace() {
        let (Ok(input), Ok(output)) = (std::env::var("GRID_CAMERA_TRACE"), std::env::var("GRID_CAMERA_OUT")) else {
            return;
        };
        let root = std::env::var("GRID_GAME_ROOT").expect("GRID_GAME_ROOT");
        let views = Views::load(Path::new(&root), "lac").unwrap();
        let idents = ["chase_close", "chase_far", "bumper", "bonnet", "head-cam"];
        // GRID_CAMERA_FPS=F replays on Clock::Fps(F), else one frame per row.
        let clock = std::env::var("GRID_CAMERA_FPS").ok().and_then(|v| v.parse().ok()).map_or(Clock::Frame, Clock::Fps);
        let mut histories = vec![History::default(); idents.len()];
        let mut out = String::new();
        let mut previous: Option<f64> = None;
        let mut previous_sample: Option<Sample> = None;
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
                    if v.len() >= 89 { Vec3::new(v[85], v[86], v[87]) } else { Vec3::ZERO },
                    Vec3::new(0., -0.39144, -0.14642),
                ),
                angular_velocity: Vec3::new(v[28], v[29], v[30]),
                rate_known: true,
                velocity: Vec3::new(v[31], v[32], v[33]),
                speed: v[2],
                wheels: std::array::from_fn(|i| (v[4 + 2 * i], v[5 + 2 * i])),
                controls_disabled: v[3] != 7.,
                drift_weight: 0.,
                ticks: 0,
                segment: 0,
            };
            // The camera runs on the frame time (the sampler's row gap), not on
            // the physics step +0x7C8.
            let dt = previous.map_or(0., |p| (t - p) as f32);
            previous = Some(t);
            // A row gap of n world steps (+0x7C8, column 1) holds n rendered
            // frames, n − 1 of them missed by the sampler: those are filled
            // with the car state interpolated from the previous row.
            let frames = match (clock, previous_sample) {
                (Clock::Frame, Some(_)) if v[1] > 0. => ((dt / v[1]).round() as u32).clamp(1, 8),
                _ => 1,
            };
            let from = previous_sample.unwrap_or(s);
            previous_sample = Some(s);
            let mut row = vec![v[0]];
            for (k, ident) in idents.iter().enumerate() {
                let i = views.0.iter().position(|x| &x.ident == ident).unwrap();
                // The original's clock: one update per rendered frame.
                let mut shot = None;
                for f in 1..=frames {
                    let s = s.from(&from, f as f32 / frames as f32);
                    shot = Some(shot_clocked(clock, &views.0[i].rig, &mut histories[k], &s, dt / frames as f32, Look::default(), |o| o));
                }
                let shot = shot.unwrap();
                row.extend(shot.eye.to_array());
                row.extend((shot.rotation * Vec3::NEG_Z).to_array());
                row.extend((shot.rotation * Vec3::Y).to_array());
            }
            // States: chase_close lag and spin blend, bonnet tilt and shift.
            row.extend([histories[0].chase.lag, histories[0].chase.spin.blend]);
            row.extend(histories[3].mounted.tilt.to_array());
            row.extend(histories[3].mounted.shift.to_array());
            // Head-cam body and head offsets.
            row.extend(histories[4].head.body.off.to_array());
            row.extend(histories[4].head.head.off.to_array());
            // chase_close spin hold: envelope time and held forward.
            row.push(histories[0].chase.spin.timer);
            row.extend(histories[0].chase.spin.held.to_array());
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
        let model = model_frame(pose, [Vec3::X, Vec3::Y, Vec3::Z], Vec3::ZERO, Vec3::new(0., -0.39144, -0.14642));
        assert!(model.w_axis.truncate().abs_diff_eq(Vec3::new(10. - 0.14642, -0.39144, 0.), 1e-6));
        assert!(Mat3::from_mat4(model).abs_diff_eq(Mat3::from_mat4(pose), 1e-7));
        // The chassis translation row (car +0x40) is added unturned.
        let lifted = model_frame(pose, [Vec3::X, Vec3::Y, Vec3::Z], Vec3::new(0., 0.02, 0.), Vec3::new(0., -0.39144, -0.14642));
        assert!(lifted.w_axis.truncate().abs_diff_eq(Vec3::new(10. - 0.14642, -0.37144, 0.), 1e-6));
        // A broken +0x10 block keeps the pose's rotation.
        let broken = model_frame(pose, [Vec3::ZERO; 3], Vec3::ZERO, Vec3::ZERO);
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
        shot(&rig, &mut history, &sample(Mat4::IDENTITY, Vec3::Z * 30., 0), dt, Look::default(), |v| v);
        shot(&rig, &mut history, &sample(Mat4::IDENTITY, Vec3::Z * (30. - 30. * dt), 1), dt, Look::default(), |v| v);
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

    /// The Lacetti's head-cam (shipped cameras.xml, docs/CAMERA_VIEWS.md).
    fn head() -> Head {
        Head {
            fov: 48.,
            near: 0.075,
            position: Vec3::new(0.3604, 1.1047, -0.1923),
            tilt: -7.3736,
            body_length: 0.01,
            body: Spring { scale: Vec2::new(0.9, 1.5), damping: 1.2, inertia: 0.1, centering: 2., max_offset: 0.07 },
            head: Spring { scale: Vec2::new(1., 0.), damping: 5.4045, inertia: 0.3399, centering: 3.8697, max_offset: 0.179 },
            max_look: 40.,
        }
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
            ("head-cam", false, Some(9), Rig::Head(head())),
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
                    disable_driver_head: false,
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

    /// setup_emu.py's car: at (10, 0, 20), unturned, a stand-in box.
    fn stand_in_car() -> grid_world::phantom_post::Box {
        grid_world::phantom_post::Box {
            axes: [[1., 0., 0., 0.], [0., 1., 0., 0.], [0., 0., 1., 0.]],
            centre: [10., 0.725, 20., 1.],
            half: [0.9, 0.725, 2.3, 0.],
        }
    }

    #[test]
    fn change_view_lift_matches_the_emulated_set_up() {
        let at = Vec3::new(10., 0., 20.);
        let lift = |a: Vec3, b: Vec3, near: f32| transition_lift(at + a, at + b, near, &stand_in_car());
        let (close, far) = (Vec3::new(0., 1.7227, -4.7393), Vec3::new(0., 1.8637, -5.1838));
        let (bumper, bonnet) = (Vec3::new(0., 0.7275, 1.7706), Vec3::new(0., 1.1055, 0.8556));
        // No sample hits the car between the chase views; 19 of 32 do on the
        // way to the bumper (whose eye is inside the box), only the last on
        // the way to the bonnet.
        assert_eq!(lift(close, far, 0.1), 0.);
        assert!((lift(far, bumper, 0.2) - 3.200_307).abs() < 1e-4);
        assert!((lift(bumper, bonnet, 0.1) - 1.706_726).abs() < 1e-4);
    }

    // The emulated original's values, printed in full (60° is the bumper's fov).
    #[allow(clippy::excessive_precision, clippy::approx_constant)]
    #[test]
    fn change_view_blend_matches_the_emulated_tracker() {
        let shot = |q: [f32; 4], eye: [f32; 3], fov: f32, near: f32| Shot {
            eye: Vec3::from(eye),
            rotation: Quat::from_array(q),
            fov,
            near: Some(near),
        };
        type Case<'a> = &'a [(f32, Option<Shot>)];
        let run = |from: &Shot, to: &Shot, duration: f32, lift: f32, case: Case| {
            let mut tr = Transition { from: 0, to: 1, t: 0., duration, lift, pending: 0. };
            for (k, (dt, expected)) in case.iter().enumerate() {
                tr.t += dt;
                let Some(e) = expected else {
                    assert!(tr.t > tr.duration, "case row {k}: ended");
                    continue;
                };
                let s = transition_shot(&tr, from, to);
                assert!(s.eye.abs_diff_eq(e.eye, 2e-6), "row {k}: {} {}", s.eye, e.eye);
                assert!(s.rotation.abs_diff_eq(e.rotation.normalize(), 2e-6), "row {k}: {} {}", s.rotation, e.rotation);
                assert!((s.fov - e.fov).abs() < 1e-6, "row {k}: {} {}", s.fov, e.fov);
                assert!((s.near.unwrap() - e.near.unwrap()).abs() < 1e-6, "row {k}");
            }
        };
        let close = shot([0.0436193874, 0., 0., 0.999048222], [0., 1.7227, -4.7393], 0.907571211, 0.1);
        let far = shot([0.0348994967, 0., 0., 0.999390827], [0., 1.8637, -5.1838], 0.959931089, 0.1);
        let bumper = shot([0.0497794182, 0., 0., 0.998760236], [0., 0.7275, 1.7706], 1.04719755, 0.2);
        let turned = shot([0., 0.996194698, 0., 0.0871557427], [0.3, 1.2, 2.], 0.837758041, 0.075);
        let a = 1. / 60.;
        run(&close, &far, 0.5, 0., &[
            (0., Some(shot([0.0436193869, 0., 0., 0.999048233], [0., 1.7227, -4.73929977], 0.907571197, 0.1))),
            (a, Some(shot([0.0434425883, 0., 0., 0.999055147], [0., 1.72315955, -4.74074841], 0.90931654, 0.1))),
            (a, Some(shot([0.04326066, 0., 0., 0.999062359], [0., 1.72449648, -4.74496317], 0.911061883, 0.1))),
            (0.1, Some(shot([0.042019289, 0., 0., 0.999111116], [0., 1.74743247, -4.81726837], 0.921533823, 0.1))),
        ]);
        // Over the car: the eye rises on the lifted cubic.
        run(&far, &bumper, 0.5, 0.8, &[
            (0.1, Some(shot([0.0368634835, 0., 0., 0.999307632], [0., 2.12451053, -4.39862633], 0.977384388, 0.120000005))),
            (0.1, Some(shot([0.039506834, 0., 0., 0.999195576], [0., 2.0322206, -2.64297676], 0.994837701, 0.140000001))),
            (0.1, Some(shot([0.0451720804, 0., 0., 0.998955488], [0., 1.69590545, -0.584474027], 1.01229095, 0.159999996))),
            (0.15, Some(shot([0.0488471165, 0., 0., 0.998799741], [0., 0.972487211, 1.6107049], 1.03847098, 0.189999998))),
            (0.1, None),
        ]);
        // A slerp over a large turn, and the same with the target's sign
        // flipped (the from quaternion is negated: every output negates).
        let turning = [
            (0.05, Some(shot([0.043077182, 0.0928215608, 0., 0.9947505], [0.00867827237, 1.78882229, -4.54434824], 0.900589883, 0.0975000039))),
            (0.2, Some(shot([0.0295825228, 0.675615966, 0., 0.736659944], [0.150772989, 1.68567753, -1.35228574], 0.87266463, 0.087500006))),
            (0.2, Some(shot([0.00406428706, 0.983811677, 0., 0.17915985], [0.291878283, 1.29539359, 1.81755054], 0.844739377, 0.0775000006))),
        ];
        run(&close, &turned, 0.5, 0.3, &turning);
        let negated = turning.map(|(dt, s)| (dt, s.map(|s| Shot { rotation: -s.rotation, ..s })));
        run(&close, &Shot { rotation: -turned.rotation, ..turned }, 0.5, 0.3, &negated);
        // Inside <-> outside: duration 0 draws the new view at once.
        run(&bumper, &turned, 0., 0., &[(0., Some(turned)), (a, None)]);
    }

    #[test]
    fn change_view_blends_half_a_second_and_keeps_every_view_running() {
        let pose = |t: u64| {
            Mat4::from_rotation_translation(Quat::from_rotation_y(t as f32 * 0.01), Vec3::new(0., 0., t as f32 * 0.5))
        };
        let at = |t: u64| sample(pose(t), Vec3::new(0., 0., 31.25), t);
        let mut cameras = Cameras::new(lacetti(), DEFAULT_VIEW, None);
        // chase_far shown all along: the views run hidden, nothing resets.
        let mut far = Cameras::new(lacetti(), "chase_far", None);
        for t in 0..10 {
            cameras.frame(&at(t), false, Look::default(), |o| o);
            far.frame(&at(t), false, Look::default(), |o| o);
        }
        let tr = cameras.change_view(&at(10), None).expect("press");
        assert_eq!((tr.from, tr.to, tr.duration, tr.lift), (0, 2, 0.5, 0.));
        assert!(cameras.change_view(&at(10), None).is_none(), "a press during a blend is dropped");
        // 31 blended frames (t to 0.496 s); the 32nd tick passes 0.5 s and
        // commits.
        for t in 10..=41 {
            let shot = cameras.frame(&at(t), false, Look::default(), |o| o);
            let reference = far.frame(&at(t), false, Look::default(), |o| o);
            if t < 41 {
                assert_eq!(cameras.ident(), "chase_close", "tick {t}");
                assert_ne!(shot, reference);
            } else {
                assert_eq!(cameras.ident(), "chase_far");
                assert_eq!(shot, reference);
            }
        }
        // Outside views blend over the car's box; bonnet -> head-cam cuts.
        let mut t = 42;
        for (ident, lifted) in [("bumper", true), ("bonnet", true)] {
            let car = grid_world::phantom_post::Box {
                axes: [[1., 0., 0., 0.], [0., 1., 0., 0.], [0., 0., 1., 0.]].map(|r| {
                    let v = pose(t).transform_vector3(Vec3::from_slice(&r));
                    [v.x, v.y, v.z, 0.]
                }),
                centre: {
                    let c = pose(t).transform_point3(Vec3::Y * 0.725);
                    [c.x, c.y, c.z, 1.]
                },
                half: [0.9, 0.725, 2.3, 0.],
            };
            let tr = cameras.change_view(&at(t), Some(&car)).expect("press");
            assert_eq!(tr.duration, 0.5);
            assert_eq!(tr.lift > 0.5, lifted, "{ident} {}", tr.lift);
            for _ in 0..32 {
                cameras.frame(&at(t), false, Look::default(), |o| o);
                t += 1;
            }
            assert_eq!(cameras.ident(), ident);
        }
        // A frame without a tick draws the head-cam through the blend at
        // f = 1; the next tick commits it.
        let tr = cameras.change_view(&at(t - 1), None).expect("press");
        assert_eq!((tr.duration, tr.lift), (0., 0.));
        let shot = cameras.frame(&at(t - 1), false, Look::default(), |o| o);
        let head = cameras.shots[8].unwrap();
        assert!(shot.eye.abs_diff_eq(head.eye, 1e-6) && shot.rotation.abs_diff_eq(head.rotation, 1e-6));
        assert_eq!(cameras.ident(), "bonnet");
        assert_eq!(cameras.frame(&at(t), false, Look::default(), |o| o), cameras.shots[8].unwrap());
        assert_eq!(cameras.ident(), "head-cam");
    }

    #[test]
    fn views_draw_the_own_car_as_the_original() {
        let at = |t: u64| sample(Mat4::IDENTITY, Vec3::ZERO, t);
        let active = ["bonnet".to_string(), "door_fl".to_string()];
        let mut seen = Vec::new();
        for ident in ["chase_close", "chase_far", "bumper", "bonnet", "head-cam"] {
            let mut cameras = Cameras::new(lacetti(), ident, None);
            assert_eq!(cameras.own_car(), OwnCar::Whole, "nothing shown yet");
            cameras.frame(&at(0), false, Look::default(), |o| o);
            seen.push(cameras.own_car());
            // Looking back from the bumper shows bumper_reverse: still hidden.
            cameras.frame(&at(1), true, Look::default(), |o| o);
            assert_eq!(cameras.own_car() == OwnCar::Hidden, ident == "bumper", "{ident} back");
        }
        assert_eq!(
            seen,
            [OwnCar::Whole, OwnCar::Whole, OwnCar::Hidden, OwnCar::Whole, OwnCar::Interior]
        );
        // The head-cam keeps the file's exterior nodes and the cockpit.
        let rule = OwnCar::Interior;
        assert!(rule.shows(true, "int_dash", &active) && rule.shows(false, "door_fl", &active));
        assert!(!rule.shows(false, "body", &active) && !rule.shows(false, "wheel_fl", &active));
        assert!(OwnCar::Whole.shows(false, "body", &active) && !OwnCar::Whole.shows(true, "x", &active));
        assert!(!OwnCar::Hidden.shows(false, "bonnet", &active));
        // Without a loaded cockpit the head-cam keeps the whole car.
        let mut cameras = Cameras::new(lacetti(), "head-cam", None);
        cameras.frame(&at(0), false, Look::default(), |o| o);
        assert_eq!(own_car(Some(&cameras), None, false), OwnCar::Whole);
        assert_eq!(own_car(Some(&cameras), None, true), OwnCar::Interior);
    }
    #[test]
    fn change_view_ends_look_back_until_it_is_pressed_again() {
        let at = |t: u64| sample(Mat4::IDENTITY, Vec3::ZERO, t);
        let mut cameras = Cameras::new(lacetti(), DEFAULT_VIEW, None);
        cameras.frame(&at(0), true, Look::default(), |o| o);
        assert_eq!(cameras.shown, Some(1));
        cameras.change_view(&at(1), None).expect("press");
        for t in 1..40 {
            cameras.frame(&at(t), true, Look::default(), |o| o);
        }
        // Still held after the commit: the new view, not its reverse.
        assert_eq!((cameras.current, cameras.shown), (2, Some(2)));
        cameras.frame(&at(40), false, Look::default(), |o| o);
        cameras.frame(&at(41), true, Look::default(), |o| o);
        assert_eq!(cameras.shown, Some(3));
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
        let shot = cameras.frame(&sample(pose, Vec3::ZERO, 5), false, Look::default(), |v| v);
        let eye = pose.transform_point3(Vec3::new(0., 1.7227, -4.7393));
        let target = pose.transform_point3(Vec3::new(0., 1.2552, 1.9141));
        assert!(shot.eye.distance(eye) < 1e-4);
        assert!(forward(&shot).dot((target - eye).normalize()) > 0.99999);
        assert_eq!(shot.fov, 52f32.to_radians());
        // Look Back: the reverse chase sits ahead of the car looking back.
        let back = cameras.frame(&sample(pose, Vec3::ZERO, 5), true, Look::default(), |v| v);
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
        cameras.frame(&sample(straight, Vec3::Z * 30., 0), false, Look::default(), |v| v);
        let shot = cameras.frame(&sample(turned, Vec3::Z * 30., 1), false, Look::default(), |v| v);
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
        let first = cameras.frame(&sample(uphill, Vec3::Z * 30., 0), false, Look::default(), |v| v);
        let later = cameras.frame(&sample(uphill, Vec3::Z * 30., 1), false, Look::default(), |v| v);
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
        // These speed steps (over 50 m/s² within a tick) also knock the eye
        // (envelope 2), which is not the height rule under test.
        let knock = cameras.history().chase.knock;
        let y = y - knock.blend * knock.spring.off.y;
        assert!(knock.blend > 0. && (y - (start + 2.)).abs() < 1e-5, "{y}");
    }

    fn cameras_frame_y(cameras: &mut Cameras, pose: Mat4, velocity: Vec3, ticks: u64) -> f32 {
        cameras.frame(&sample(pose, velocity, ticks), false, Look::default(), |v| v).eye.y
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
        cameras.frame(&s, false, Look::default(), |v| v);
        s.ticks = 1;
        let shot = cameras.frame(&s, false, Look::default(), |v| v);
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
        let shot = cameras.frame(&sample(Mat4::IDENTITY, Vec3::Z * 30., 0), false, Look::default(), |v| v);
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
        let mut shot = cameras.frame(&s, false, Look::default(), |v| v);
        for t in 1..30 {
            s.ticks = t;
            shot = cameras.frame(&s, false, Look::default(), |v| v);
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
    fn chase_takes_the_drift_weight_from_the_car() {
        // A car whose properties set column 330 (car +0xFE0) turns the chase
        // toward its slip; the shipped 0 leaves it alone.
        let run = |weight: f32| {
            let mut cameras = Cameras::new(lacetti(), DEFAULT_VIEW, None);
            let mut s = sample(Mat4::IDENTITY, Vec3::new(10., 0., 10.), 0);
            s.drift_weight = weight;
            forward(&cameras.frame(&s, false, Look::default(), |v| v))
        };
        assert!(run(0.).x.abs() < 1e-6);
        assert!(run(1.).x > 0.1, "{}", run(1.));
    }

    #[test]
    fn bumper_is_rigid_and_pitched_down() {
        let pose = Mat4::from_rotation_translation(Quat::from_rotation_y(-1.2), Vec3::new(-3., 1., 4.));
        let mut cameras = Cameras::new(lacetti(), "bumper", None);
        let v = pose.transform_vector3(Vec3::Z) * 40.;
        cameras.frame(&sample(pose, Vec3::ZERO, 0), false, Look::default(), |v| v);
        let shot = cameras.frame(&sample(pose, v, 1), false, Look::default(), |v| v);
        assert!(shot.eye.distance(pose.transform_point3(Vec3::new(0., 0.7275, 1.7706))) < 1e-4);
        let f = forward(&shot);
        assert!(f.dot(pose.transform_vector3(Vec3::Z)) > 0.99);
        assert!(f.y < 0.);
        assert_eq!(shot.near, Some(0.1));
        // Its reverse looks backwards.
        let back = cameras.frame(&sample(pose, v, 2), true, Look::default(), |v| v);
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
            last = shot(&rig, &mut history, &s, dt, Look::default(), |v| v).eye;
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
        let shot = cameras.frame(&sample(pose, Vec3::Z * 30., 0), false, Look::default(), |v| v);
        // The seat raised by bodyLength 0.01; tilt −7.3736° looks down.
        assert!(shot.eye.distance(Vec3::new(1.3604, 3.1147, 2.8077)) < 1e-4);
        let down = (-7.3736f32).to_radians();
        assert!(forward(&shot).abs_diff_eq(Vec3::new(0., down.sin(), down.cos()), 1e-6));
        assert_eq!(shot.fov, 48f32.to_radians());
        // Driving straight at a steady speed leaves both springs at rest.
        for t in 1..100 {
            let pose = Mat4::from_translation(Vec3::new(1., 2., 3. + 0.48 * t as f32));
            cameras.frame(&sample(pose, Vec3::Z * 30., t), false, Look::default(), |v| v);
        }
        assert_eq!(cameras.history().head.body.off, Vec3::ZERO);
        assert_eq!(cameras.history().head.head.off, Vec3::ZERO);
    }

    #[test]
    fn head_spring_step_under_braking() {
        // Body spring, car at rest frame, input velocity drops 0.5 m/s in one
        // 0.016 s step: a = −31.25 m/s² (z); q = 0.5^0.96 = 0.514057;
        // acc = 0.485943 · a = −15.18572; vel = 15.18572 · 0.1 · 0.016 =
        // 0.0242972, less damping 1.2 · 0.016 = 0.0050972; off = 0.016 · vel.
        let c = head();
        let mut s = SpringState { prev: Vec3::Z * 10., ..SpringState::default() };
        spring_step(&c.body, &mut s, 0.016, Mat3::IDENTITY, Vec3::Z * 9.5);
        assert!((s.acc.z + 15.18572).abs() < 1e-4, "{}", s.acc);
        assert!((s.vel.z - 0.0050972).abs() < 1e-6, "{}", s.vel);
        assert!((s.off.z - 8.15545e-5).abs() < 1e-9, "{}", s.off);
        assert_eq!((s.off.x, s.off.y, s.prev), (0., 0., Vec3::Z * 9.5));
        // Below damping/inertia (12 m/s² here) the offset does not move.
        let mut s = SpringState { prev: Vec3::Z * 10., ..SpringState::default() };
        spring_step(&c.body, &mut s, 0.016, Mat3::IDENTITY, Vec3::Z * 9.9);
        assert_eq!(s.off, Vec3::ZERO);
        // A car turned π/2 about Y feels the world −x braking as its own z.
        let turned = Mat3::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let mut s = SpringState { prev: Vec3::X * 10., ..SpringState::default() };
        spring_step(&c.body, &mut s, 0.016, turned, Vec3::X * 9.5);
        assert!((s.off.z - 8.15545e-5).abs() < 1e-9 && s.off.x.abs() < 1e-9, "{}", s.off);
        // Too short a step only takes the input.
        let mut s = SpringState { prev: Vec3::Z, ..SpringState::default() };
        spring_step(&c.body, &mut s, 1e-5, Mat3::IDENTITY, Vec3::ZERO);
        assert_eq!(s, SpringState::default());
    }

    #[test]
    fn head_spring_limit_is_an_ellipsoid() {
        // Body offset 0.06 sideways moving out at 1 m/s, no acceleration:
        // vel 1 − 0.0192 − 2 · 0.06 = 0.8608, off 0.06 + 0.016 · 0.8608 =
        // 0.0737728; scaled by 1/0.9 that is 0.0819698 > 0.07, so off = 0.07 ·
        // 0.9 = 0.063 and the outward velocity is halved, then its radial part
        // (all of it) removed.
        let c = head();
        let mut s = SpringState { off: Vec3::X * 0.06, vel: Vec3::X, ..SpringState::default() };
        spring_step(&c.body, &mut s, 0.016, Mat3::IDENTITY, Vec3::ZERO);
        assert!((s.off.x - 0.063).abs() < 1e-7, "{}", s.off);
        assert_eq!(s.vel, Vec3::ZERO);
        // Upward the limit is 0.07 · 1.5; the head's y scale 0 keeps y at zero.
        let mut s = SpringState { off: Vec3::Y * 0.2, ..SpringState::default() };
        spring_step(&c.body, &mut s, 0.016, Mat3::IDENTITY, Vec3::ZERO);
        assert!((s.off.y - 0.105).abs() < 1e-7, "{}", s.off);
        let mut s = SpringState { off: Vec3::Y * 0.05, vel: Vec3::Y, ..SpringState::default() };
        spring_step(&c.head, &mut s, 0.016, Mat3::IDENTITY, Vec3::ZERO);
        assert_eq!((s.off.y, s.vel.y), (0., 0.));
    }

    #[test]
    fn head_offset_turns_the_view() {
        let mut c = head();
        c.tilt = 0.;
        // Forward 0.1 rad pitches the view down by 0.1 rad.
        let mut h = HeadState::default();
        h.head.off = Vec3::Z * 0.1;
        let shot = head_view(&c, &h, &Mat4::IDENTITY);
        assert!(forward(&shot).abs_diff_eq(Vec3::new(0., -0.0998334, 0.9950042), 1e-6), "{}", forward(&shot));
        // Right 0.1 rad rolls the right axis down: (0, 0, 0.1) → axis (0, 0, −1).
        h.head.off = Vec3::X * 0.1;
        let shot = head_view(&c, &h, &Mat4::IDENTITY);
        assert!((shot.rotation * Vec3::NEG_X).abs_diff_eq(Vec3::new(0.9950042, -0.0998334, 0.), 1e-6));
        // The body offset moves the eye in the car frame.
        h.body.off = Vec3::new(0.01, 0.02, 0.03);
        let pose = Mat4::from_rotation_translation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2), Vec3::new(5., 0., 0.));
        let eye = head_view(&c, &h, &pose).eye;
        let seat = c.position + Vec3::new(0.01, 0.03, 0.03);
        assert!(eye.abs_diff_eq(Vec3::new(5. + seat.z, seat.y, -seat.x), 1e-6), "{eye}");
    }

    #[test]
    fn head_cam_leans_into_braking_and_settles() {
        let mut cameras = Cameras::new(lacetti(), "head-cam", None);
        let mut speed = 30.;
        let mut z = 0.;
        for t in 0..40u64 {
            cameras.frame(&sample(Mat4::from_translation(Vec3::Z * z), Vec3::Z * speed, t), false, Look::default(), |v| v);
            speed -= 20. * 0.016;
            z += speed * 0.016;
        }
        // 20 m/s² is above the body's 12 and the head's 15.9 m/s².
        let h = cameras.history().head;
        assert!(h.body.off.z > 0.005 && h.body.off.z <= 0.07 + 1e-6, "{}", h.body.off);
        // The head's equilibrium: (20 · 0.3399 − 5.4045) · 0.016 / 3.8697 ≈ 0.0058.
        assert!(h.head.off.z > 0.004 && h.head.off.z < 0.007, "{}", h.head.off);
        for t in 40..200u64 {
            cameras.frame(&sample(Mat4::from_translation(Vec3::Z * z), Vec3::Z * speed, t), false, Look::default(), |v| v);
        }
        let h = cameras.history().head;
        assert!(h.body.off.length() < 1e-4 && h.head.off.length() < 1e-4, "{h:?}");
    }

    #[test]
    fn chase_look_point_follows_the_input() {
        // Emulated 0x7FE690 (local/cam/look/results.txt): Look Left held at
        // 0 m/s gives (A4, A8) (0.0279, 0.0884), (0.075, 0.1614), (0.134,
        // 0.2182); frame 10 (0.6104, 0.2838); released after 25 frames
        // (0.8065, 0.1869), (0.6976, 0.2498).
        let left = Look { x: 1., y: 0. };
        let mut point = Vec2::ZERO;
        let mut seen = Vec::new();
        for frame in 0..27 {
            point = chase_look_message(point, if frame < 25 { left } else { Look::default() }, 0.);
            seen.push(point);
        }
        for (frame, a4, a8) in [(0, 0.0279, 0.0884), (1, 0.075, 0.1614), (2, 0.134, 0.2182), (10, 0.6104, 0.2838), (24, 0.9203, 0.0916), (25, 0.8065, 0.1869), (26, 0.6976, 0.2498)] {
            assert!(seen[frame].abs_diff_eq(Vec2::new(a4, a8), 1e-4), "{frame} {}", seen[frame]);
        }
        // Above 5 m/s (w = 1: blend 0.5, snap at 1 rad) the 84.29° step snaps
        // both ways; no input keeps the point at rest.
        let held = chase_look_message(Vec2::ZERO, left, 20.);
        assert_eq!(held, Vec2::new(1., 0.));
        assert_eq!(chase_look_message(held, Look::default(), 20.), Vec2::ZERO);
        assert_eq!(chase_look_message(Vec2::ZERO, Look::default(), 0.), Vec2::ZERO);
    }

    #[test]
    fn chase_look_turns_the_view_about_the_car() {
        let run = |look: Look| {
            let mut cameras = Cameras::new(lacetti(), DEFAULT_VIEW, None);
            cameras.frame(&sample(Mat4::IDENTITY, Vec3::Z * 20., 0), false, look, |v| v);
            cameras.frame(&sample(Mat4::IDENTITY, Vec3::Z * 20., 1), false, look, |v| v)
        };
        let still = run(Look::default());
        let left = run(Look { x: 1., y: 0. });
        // atan2(1, 0.1) = 84.29° toward +X about the vertical through the car.
        let turn = Quat::from_rotation_y(1f32.atan2(0.1));
        assert!(left.eye.abs_diff_eq(turn * still.eye, 1e-5), "{} {}", left.eye, still.eye);
        assert!(forward(&left).abs_diff_eq(turn * forward(&still), 1e-5));
        assert!(left.eye.x < -4. && forward(&left).x > 0.9);
    }

    #[test]
    fn bonnet_turns_and_lifts_with_the_look_input() {
        let mut bonnet = mounted(Vec3::new(0., 1.1055, 0.8556), 0., 0.);
        bonnet.look = MountedLook::TurnAndLift;
        let rig = Rig::Mounted(bonnet.clone());
        let mut history = History::default();
        let left = Look { x: 1., y: 0. };
        // Two ticks: B0 = 0.2, then 0.36 (0x840A30); the eye rises and moves
        // forward by 0.05 · 0.36 = 0.018 (0x822510).
        let shot = shot(&rig, &mut history, &sample(Mat4::IDENTITY, Vec3::ZERO, 0), 0.032, left, |v| v);
        assert!((history.mounted.look - 0.36).abs() < 1e-6, "{}", history.mounted.look);
        assert!(shot.eye.abs_diff_eq(Vec3::new(0., 1.1055 + 0.018, 0.8556 + 0.018), 1e-6), "{}", shot.eye);
        let yaw = forward(&shot);
        assert!((yaw.x.atan2(yaw.z) - 0.36).abs() < 1e-5, "{yaw}");
        // The bumper turns without the lift; other views ignore the input.
        bonnet.look = MountedLook::Turn;
        let mut history = History::default();
        let eye = shot_eye(&Rig::Mounted(bonnet.clone()), &mut history, left);
        assert!((history.mounted.look - 0.36).abs() < 1e-6 && eye.abs_diff_eq(Vec3::new(0., 1.1055, 0.8556), 1e-6));
        bonnet.look = MountedLook::Fixed;
        let mut history = History::default();
        shot_eye(&Rig::Mounted(bonnet), &mut history, left);
        assert_eq!(history.mounted.look, 0.);
    }

    fn shot_eye(rig: &Rig, history: &mut History, look: Look) -> Vec3 {
        shot(rig, history, &sample(Mat4::IDENTITY, Vec3::ZERO, 0), 0.032, look, |v| v).eye
    }

    #[test]
    fn head_look_smooths_the_input_and_turns_at_most_180_degrees_a_second() {
        // Emulated 0x801230: held 0.2, 0.36 … 0.9885 after 20 frames; release
        // 0.6326, 0.4049 (× 0.64 per frame).
        let mut h = HeadState::default();
        let mut seen = Vec::new();
        for frame in 0..22 {
            head_look_message(&mut h, if frame < 20 { Look { x: 1., y: 0. } } else { Look::default() });
            seen.push(h.look.x);
        }
        for (frame, d8) in [(0, 0.2), (1, 0.36), (19, 0.9885), (20, 0.6326), (21, 0.4049)] {
            assert!((seen[frame] - d8).abs() < 1e-4, "{frame} {}", seen[frame]);
        }
        // Curve: 0.5 (2 · 0.25)² = 0.125; 1 − 0.5 (2 · 0.25)² = 0.875.
        assert!((look_curve(0.25) - 0.125).abs() < 1e-7 && (look_curve(-0.75) + 0.875).abs() < 1e-7);
        // Full left and up: yaw → 40°, pitch → 12°, by at most 2.88° a tick.
        let c = head();
        let mut h = HeadState { look: Vec2::ONE, ..HeadState::default() };
        head_look_step(&c, &mut h, 0.016);
        assert!((h.look_angle.x - 2.88f32.to_radians()).abs() < 1e-6, "{}", h.look_angle);
        for _ in 0..20 {
            head_look_step(&c, &mut h, 0.016);
        }
        assert!(h.look_angle.abs_diff_eq(Vec2::new(40f32.to_radians(), 12f32.to_radians()), 1e-5), "{}", h.look_angle);
        // The view turns toward +X and up from the tilted seat view.
        let mut flat = c.clone();
        flat.tilt = 0.;
        let f = forward(&head_view(&flat, &h, &Mat4::IDENTITY));
        let up = 12f32.to_radians();
        let side = 40f32.to_radians();
        assert!(f.abs_diff_eq(Vec3::new(side.sin() * up.cos(), up.sin(), side.cos() * up.cos()), 1e-5), "{f}");
    }

    /// The shipped bonnet's shake (cameras.xml stores 4-decimal text).
    fn bonnet_shake() -> Shake {
        Shake {
            angle: 0.,
            damping: 0.1798,
            min_speed: 20.8641,
            max_speed: 90.,
            scale_x: 0.0001,
            scale_y: 0.0024,
            rotation_speed: 0.0022,
        }
    }

    #[test]
    fn shake_table_matches_the_original_initializer() {
        // Entries of the emulated 0x7F2B40 table (local/cam/shake).
        let t = shake_table();
        assert_eq!((t[0], t[34], t[35], t[5999]), (0.5134653, 0.5718816, 0.55590683, 0.8572497));
    }

    /// Diagnostic: the whole table against the emulated original's
    /// (GRID_SHAKE_TABLE = local/cam/shake/noise_table_emulated.npy, f32).
    #[test]
    #[ignore]
    fn shake_table_equals_the_emulated_table() {
        let Ok(path) = std::env::var("GRID_SHAKE_TABLE") else {
            return;
        };
        let bytes = std::fs::read(path).unwrap();
        let header = 10 + u16::from_le_bytes([bytes[8], bytes[9]]) as usize;
        let emulated: Vec<f32> = bytes[header..].as_chunks::<4>().0.iter().map(|b| f32::from_le_bytes(*b)).collect();
        assert_eq!(emulated.len(), SHAKE_TABLE);
        let mismatches = shake_table().iter().zip(&emulated).filter(|(a, b)| a.to_bits() != b.to_bits()).count();
        assert_eq!(mismatches, 0);
    }

    #[test]
    fn bonnet_shake_amplitude_follows_the_speed() {
        let k = bonnet_shake();
        // 55.46 m/s: ramp (55.46 − 20.8641) / 69.1359 = 0.5004043, decayed
        // by 0.1798 · 0.016 in the same step: 0.49896473.
        let mut h = MountedState::default();
        shake_step(&k, &mut h, 55.46, 0.016);
        assert!((h.shake_amplitude - 0.49896473).abs() < 1e-7 && h.shake_time == 0.016, "{h:?}");
        // Noise 0.116 s into the table (u 34.8): 0.05897939 (up), 0.075486794 (side).
        assert!((shake_noise(h.shake_time, SHAKE_LEAD, 1., h.shake_amplitude) - 0.05897939).abs() < 1e-6);
        assert!((shake_noise(h.shake_time, 0., 1., h.shake_amplitude) - 0.075486794).abs() < 1e-6);
        // Below shakeMinSpeed the amplitude only decays, at 0.1798 per second,
        // and stops below 1e-4.
        let before = h.shake_amplitude;
        shake_step(&k, &mut h, 10., 0.016);
        assert!((h.shake_amplitude - before * (1. - 0.1798 * 0.016)).abs() < 1e-7);
        for _ in 0..4000 {
            shake_step(&k, &mut h, 10., 0.016);
        }
        assert_eq!(h.shake_amplitude, 0.);
        // An all-zero shake (bumper) keeps no state.
        let mut h = MountedState::default();
        shake_step(&Shake::default(), &mut h, 55.46, 0.016);
        assert_eq!((h.shake_time, h.shake_amplitude), (0., 0.));
    }

    #[test]
    fn bonnet_shake_moves_the_eye_along_the_view() {
        let mut m = mounted(Vec3::new(0., 1.1055, 0.8556), 0., 0.);
        let calm = shot(&Rig::Mounted(m.clone()), &mut History::default(), &sample(Mat4::IDENTITY, Vec3::Z * 55.46, 0), 0.016, Look::default(), |v| v);
        m.shake = bonnet_shake();
        let mut history = History::default();
        let shaken = shot(&Rig::Mounted(m.clone()), &mut history, &sample(Mat4::IDENTITY, Vec3::Z * 55.46, 0), 0.016, Look::default(), |v| v);
        // Up 0.05897939 · 0.0024 m, right (the view's +X, the car's left)
        // 0.075486794 · 0.0001 m; the view itself does not turn (angle 0).
        let up = calm.rotation * Vec3::Y;
        let right = calm.rotation * Vec3::NEG_X;
        let expected = calm.eye + up * (0.05897939 * 0.0024) + right * (0.075486794 * 0.0001);
        assert!(shaken.eye.abs_diff_eq(expected, 1e-7), "{} {}", shaken.eye, expected);
        assert!(right.abs_diff_eq(Vec3::X, 1e-6));
        assert_eq!(shaken.rotation, calm.rotation);
        // A shake angle pitches the view down for a positive noise:
        // 10° · 0.075486794 (time scale 1).
        m.shake = Shake { angle: 10., rotation_speed: 1., ..bonnet_shake() };
        let pitched = shot(&Rig::Mounted(m), &mut History::default(), &sample(Mat4::IDENTITY, Vec3::Z * 55.46, 0), 0.016, Look::default(), |v| v);
        let drop = forward(&calm).y.asin() - forward(&pitched).y.asin();
        assert!((drop - (10. * 0.075486794f32).to_radians()).abs() < 1e-5, "{drop}");
    }

    #[test]
    fn head_cam_steps_per_tick_whatever_the_frame_rate() {
        // One shot per tick and one per two ticks give the same springs when
        // the input changes evenly.
        let run = |stride: u64| {
            let mut cameras = Cameras::new(lacetti(), "head-cam", None);
            let mut t = 0;
            while t <= 60 {
                let v = 30. - 20. * 0.016 * t as f32;
                cameras.frame(&sample(Mat4::IDENTITY, Vec3::Z * v, t), false, Look::default(), |v| v);
                t += stride;
            }
            cameras.history().head
        };
        let (a, b) = (run(1), run(2));
        assert!(a.body.off.abs_diff_eq(b.body.off, 1e-6) && a.head.off.abs_diff_eq(b.head.off, 1e-6), "{a:?} {b:?}");
        assert!(a.head.off.z > 0.004);
    }

    #[test]
    // The emulated original's values, printed in full.
    #[allow(clippy::excessive_precision)]
    fn spin_hold_matches_the_emulated_original_on_the_spin_scene() {
        // |ω'| of rows 420..523 of the deterministic spin scene (grid-probes
        // grid-camera-scene, a handbrake spin into a Donington barrier at
        // 62.5 fps); the envelope time and blend of the emulated original's
        // chase update (local/cam/spin/chase_emu.py, 0x8435D0) at those rows.
        const S: [f32; 104] = [
            2.4500, 2.5025, 2.5147, 2.5520, 2.6070, 2.5801, 2.5891, 2.5914, 2.5685, 3.0017, 2.9778, 2.9508, 2.9103, 2.8843,
            2.8764, 2.8258, 2.7793, 2.7397, 2.7028, 2.6592, 2.6041, 2.5562, 3.9997, 3.9027, 3.7851, 3.6831, 3.5735, 3.4009,
            3.2811, 3.1356, 3.0104, 2.8881, 2.7084, 2.5803, 2.3979, 2.2603, 2.1175, 1.9729, 1.8280, 1.6837, 1.5401, 1.3976,
            1.2559, 1.0749, 0.9688, 0.8740, 0.7490, 0.6136, 0.4653, 0.3047, 0.1987, 0.1347, 0.0998, 0.0885, 0.0939, 0.1050,
            0.1148, 0.1197, 0.1190, 0.1072, 0.0980, 0.0792, 0.0595, 0.0418, 0.0264, 0.0132, 0.0091, 0.0046, 0.0105, 0.0154,
            0.0192, 0.1171, 0.2245, 0.3110, 0.3764, 0.4205, 0.4442, 0.4477, 0.4306, 0.3977, 0.3556, 0.3077, 0.2605, 0.2144,
            0.1681, 0.1214, 0.0898, 0.0597, 0.0449, 0.0470, 0.0570, 0.0675, 0.0755, 0.0802, 0.0816, 0.0795, 0.0746, 0.0671,
            0.0577, 0.0466, 0.0362, 0.0274, 0.0203, 0.0151,
        ];
        let expected = [
            // Start at 431 (armed at 429), rise, re-armed at 442 and restarted
            // at 444, cut to 1.2 on the next frame although still spinning,
            // then the ease to 0.
            (431, 1.984, 0.),
            (432, 1.968, 0.28284299),
            (433, 1.952, 0.40000033),
            (443, 1.7919996, 0.97979677),
            (444, 1.7839999, 1.),
            (445, 1.184, 1.),
            (446, 1.168, 0.99964446),
            (470, 0.784, 0.77777773),
            (500, 0.30400047, 0.14222261),
            (519, 4.2e-7, 0.00035557424),
            (520, -0.016, 0.),
        ];
        let mut spin = Spin::default();
        let mut seen = Vec::new();
        for (row, s) in (420..).zip(S) {
            let mut sample = sample(Mat4::IDENTITY, Vec3::Z * 20., 0);
            sample.angular_velocity = Vec3::Y * s;
            spin_step(&mut spin, &sample, Vec3::Z, 0.016);
            seen.push((row, spin.timer, spin.blend));
        }
        for (row, timer, blend) in expected {
            let (_, t, b) = seen[row - 420];
            assert!((t - timer).abs() < 2e-5 && (b - blend).abs() < 1e-5, "row {row}: {t} {b}");
        }
    }

    #[test]
    fn knock_matches_the_emulated_dead_stop() {
        // local/cam/spin/knock_vectors.txt vector 1: the reset (dt 0) at 40
        // m/s with the chase eye at z −5.552386, the eye 0.64 m further each
        // 16 ms frame, a dead stop at frame 31 (the eye jumps to 14.31428 as
        // the fall-back drops out), then rest. Per frame: envelope time,
        // weight, offset z, its velocity and the eye change.
        let mut k = Knock::default();
        let eye = |z: f32| Vec3::new(0., 1.33126, z);
        knock_step(&mut k, Vec3::Z * 40., eye(-5.552386), 0.);
        let mut seen = Vec::new();
        for frame in 1..=40 {
            let (v, z) = if frame <= 30 { (40., -5.552386 + 0.64 * frame as f32) } else { (0., 14.31428) };
            knock_step(&mut k, Vec3::Z * v, eye(z), 0.016);
            seen.push((k.timer, k.blend, k.spring.off.z, k.spring.vel.z, (k.blend * k.spring.off).z));
        }
        // The reset's kick saturates the hidden offset at 0.75 m (frames 2–4).
        assert!((seen[1].2 + 0.75).abs() < 1e-5, "{:?}", seen[1]);
        for (frame, expected) in [
            (31, (0.984, 0., 0.1130663, -1.095677, 0.)),
            (32, (0.9680001, 0.2828425, 0.4752141, 22.63423, 0.1344109)),
            (33, (0.9520001, 0.3999996, 0.75, 0., 0.3000002)),
            (36, (0.9040002, 0.632455, 0.7301766, -1.238966, 0.4618034)),
            (40, (0.8400003, 0.8485274, 0.5005937, -4.979929, 0.4247675)),
        ] {
            let got = seen[frame - 1];
            let close = |a: f32, b: f32| (a - b).abs() < 2e-3 * (1. + b.abs());
            assert!(
                close(got.0, expected.0) && close(got.1, expected.1) && close(got.2, expected.2) && close(got.3, expected.3) && close(got.4, expected.4),
                "frame {frame}: {got:?} against {expected:?}"
            );
        }
    }

    #[test]
    fn camera_fps_takes_the_originals_frames() {
        let head_cam = |clock, ticks: u64, look| {
            let mut cameras = Cameras::new(lacetti(), "head-cam", None);
            cameras.clock = clock;
            let mut seen = Vec::new();
            for t in 0..=ticks {
                cameras.frame(&sample(Mat4::IDENTITY, Vec3::Z * 30., t), false, look, |v| v);
                seen.push(cameras.history().head.look.x);
            }
            seen
        };
        let full = Look { x: 1., y: 0. };
        // 62.5 fps is the tick: the same messages as Clock::Tick.
        assert_eq!(head_cam(Clock::Fps(62.5), 10, full), head_cam(Clock::Tick, 10, full));
        // 165 fps: 26 frames (look messages) in 10 ticks (0.16 s · 165 = 26.4).
        let fast = head_cam(Clock::Fps(165.), 10, full);
        assert!((fast[10] - (1. - 0.8f32.powi(26))).abs() < 1e-6, "{}", fast[10]);
        // 30 fps: a tick that ends no frame leaves the views as they were.
        let slow = head_cam(Clock::Fps(30.), 3, full);
        assert_eq!(slow, [0., 0., 0., 0.2]);
    }

    #[test]
    fn head_springs_stiffen_with_the_frame_rate() {
        // The centering has no dt (0x83E6D0): released from an offset, the
        // body spring swings back √(165/60) ≈ 1.66 times as fast at 165 fps
        // as at 60 fps (frequency ≈ √(centering / dt)).
        let swing = |fps: f32| {
            let p = Spring { damping: 0., ..head().body };
            let mut s = SpringState { off: Vec3::new(0., 0., 0.05), ..SpringState::default() };
            let dt = 1. / fps;
            let mut t = 0.;
            loop {
                let before = s.off.z;
                spring_step(&p, &mut s, dt, Mat3::IDENTITY, Vec3::ZERO);
                if s.off.z <= 0. {
                    // The crossing, interpolated within the frame.
                    return t + dt * before / (before - s.off.z);
                }
                t += dt;
            }
        };
        let ratio = swing(60.) / swing(165.);
        assert!((ratio - (165f32 / 60.).sqrt()).abs() < 0.15, "{ratio}");
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
                let s = sample(pose, Vec3::new(a, 0., 30.), t);
                if t == 120 {
                    cameras.change_view(&s, None);
                }
                let s = cameras.frame(&s, t % 50 > 40, Look::default(), |v| v);
                out.push((s.eye.to_array(), s.rotation.to_array()));
            }
            out
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn reset_clears_the_history() {
        let mut cameras = Cameras::new(lacetti(), DEFAULT_VIEW, None);
        cameras.frame(&sample(Mat4::IDENTITY, Vec3::ZERO, 0), false, Look::default(), |v| v);
        cameras.frame(&sample(Mat4::from_rotation_y(0.3), Vec3::ZERO, 1), false, Look::default(), |v| v);
        assert!(cameras.history().chase.lag > 0.);
        let mut s = sample(Mat4::IDENTITY, Vec3::ZERO, 0);
        s.segment = 1;
        cameras.frame(&s, false, Look::default(), |v| v);
        assert_eq!(cameras.history().chase.lag, 0.);
    }
}
