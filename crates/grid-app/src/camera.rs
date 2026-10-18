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
//! - Chase: position/target in a heading frame that lags the car's yaw
//!   (0x8114B0) and drops with the car's pitch (kCameraCloseFollow*).
//!   Mounted: offset and yaw/pitch/roll on the body, with the
//!   acceleration-driven eye shift (0x822510). Head: seat position and tilt.
use bevy::prelude::*;
use grid_formats::vehicle_views::{self, Kind};
use std::path::{Path, PathBuf};

/// Speed at which the chase lag rate reaches lagMaximum (0xE4E0FC, 200 km/h).
const LAG_SPEED: f32 = 55.5554;
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

/// The car state a view reads, after the tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    /// Body to world.
    pub pose: Mat4,
    /// World velocity, m/s.
    pub velocity: Vec3,
    /// Committed physics ticks and the reset counter; camera time is the
    /// simulation time between samples, so pause and replays hold still.
    pub ticks: u64,
    pub segment: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ChaseState {
    /// Yaw lag angle (rad): the heading frame trails the car by this much.
    pub lag: f32,
    /// Filtered forward slope for the close-follow height.
    pub follow: f32,
    heading: Option<f32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MountedState {
    pub shift: Vec3,
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

/// Bevy cameras look along -Z; the car faces +Z.
fn facing_forward() -> Quat {
    Quat::from_rotation_y(std::f32::consts::PI)
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
        Rig::Chase(c) => {
            let h = &mut history.chase;
            let yaw = heading(&sample.pose);
            let turn = h.heading.map_or(0., |prev| wrap(yaw - prev));
            h.heading = Some(yaw);
            if dt > 0. {
                h.lag = chase_lag(c, h.lag, turn, sample.velocity.length(), dt);
                // The slope seen from the camera's side of the car.
                let side = if c.position.z <= c.target.z { 1. } else { -1. };
                let slope = (sample.pose.z_axis.y * side).clamp(-c.follow_y_clamp, c.follow_y_clamp);
                h.follow += (slope - h.follow) * per_frame(c.follow_damp, dt);
            }
            let frame = Quat::from_rotation_y(yaw - h.lag);
            let target = position + frame * c.target;
            let local_eye = c.position - Vec3::Y * (c.follow_y_scale * h.follow);
            let eye = target + frame * orbit(local_eye - c.target);
            Shot {
                eye,
                rotation: Transform::from_translation(eye)
                    .looking_at(target, Vec3::Y)
                    .rotation,
                fov: c.fov.to_radians(),
                near: None,
            }
        }
        Rig::Mounted(m) => {
            let h = &mut history.mounted;
            if m.movement_speed != 0. && dt > 0. {
                let change = h.velocity.map_or(Vec3::ZERO, |v| sample.velocity - v);
                let local = rotation.inverse() * change / (dt * MOUNTED_ACCELERATION);
                let goal = (-local * m.axis_scale).clamp(
                    Vec3::splat(-m.maximum_shift),
                    Vec3::splat(m.maximum_shift),
                );
                h.shift += m.movement_speed * (goal - h.shift) * dt;
            }
            h.velocity = Some(sample.velocity);
            let local = m.offset + h.shift * m.final_movement_scale;
            let eye = if m.offset_only {
                position + local
            } else {
                position + rotation * local
            };
            // Positive pitch looks down [I], yaw π looks back.
            let view = Quat::from_rotation_y(m.yaw)
                * facing_forward()
                * Quat::from_rotation_x(-m.pitch)
                * Quat::from_rotation_z(m.roll);
            Shot {
                eye,
                rotation: rotation * view,
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
        shot(&self.views.0[shown].rig, &mut self.history, sample, dt, orbit)
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
        Sample {
            pose,
            velocity,
            ticks,
            segment: 0,
        }
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
        let first = cameras.frame(&sample(uphill, Vec3::ZERO, 0), false, |v| v);
        let later = cameras.frame(&sample(uphill, Vec3::ZERO, 600), false, |v| v);
        let drop = first.eye.y - later.eye.y;
        // Settles at 5 · sin(0.1) below the level frame.
        assert!((drop - 5. * 0.1f32.sin()).abs() < 1e-3);
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
