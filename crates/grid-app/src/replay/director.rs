//! The original's replay director: trackside cameras switched by activation
//! zones that the replay's target car passes (`replay_camera_config.xml`,
//! grid_formats::replay_cameras; docs/REPLAY.md, docs/REPLAY_DIRECTOR_RESEARCH.md).
//!
//! [D] from the data: each zone's `entry` handler activates one ordinary
//! camera (and one `TERMINAL_` camera used by the terminal-damage replay);
//! `target.NAME` activates the target car's own `cameras.xml` view NAME;
//! `initialCamera` is used until the first zone. Zones are oriented boxes with
//! `size` as half extents [I: on the four circuits the racing line passes
//! 9-11 m below the zone centres, inside only with half extents]; a zone fires
//! when the segment between two replay samples of the car crosses it.
//!
//! Camera behaviour [D unless marked], from the zoom camera class (vtable
//! 0xD001A8: parameter binder 0x7F7590, activation 0x843700, update 0x824160
//! with eye 0x819D20, target 0x8199B0, zoom 0x819820/0x808C30, turn 0x808E20)
//! and the static camera class (vtable 0xD000D8, binder 0x7F57C0, update
//! 0x8231D0):
//!
//! - target: the car's pose applied to `targetOffset` (a car-local offset),
//!   or a point on the camera's `targetPath`;
//! - eye: `Position`, or a point on `sourcePath`. A `useTargetProjection` path
//!   projects the target (shifted so that it projects onto the path's start
//!   when the camera is activated) onto the path; the path parameter never
//!   goes back and follows the projection with the weight
//!   clamp(|speed in mph| × 0.01, 0.1, 0.9) per update. Any other path is
//!   driven by the time since activation over its `duration`, through its
//!   `percentageCurve` (speed-relative paths: [I] the same, see the doc);
//! - field of view: fixed `defaultFov` when zooming is off, the limits are
//!   equal or `zoomPerSecond` is 0; otherwise the goal atan(radius / distance)
//!   (clamped to the limits), approached at `zoomPerSecond` with an easing
//!   near the limits. A shot starts at `defaultFov`;
//! - orientation: the look at the target with `roll`; with `turnEnabled` the
//!   camera turns towards it with an angular velocity driven by
//!   `turnAcceleration`, damped by `turnDamping` and capped by `maxTurnSpeed`;
//! - static cameras keep `Position`, `orientation` and `fov` (the class binds
//!   no turn parameters).
//!
//! The original records the camera state in the replay (30 Hz) and
//! interpolates it; the port steps it at every replay sample from the shot's
//! activation (see replay.rs), so seeking shows the same shots.
//!
//! Not reproduced: shake, the lookahead smoothing (the port keeps its
//! AUTHORED `lookahead` × velocity), the near-plane adjustment and the
//! `local` path flag (the shipped path points are world positions).
use super::frame::Pose;
use bevy::math::{Quat, Vec3};
use grid_formats::replay_cameras::{self, Config, Kind};

#[derive(Clone, Debug, PartialEq)]
pub enum Activation {
    /// A trackside camera (index into [`Director::cameras`]).
    Track(usize),
    /// A view of the target car's `cameras.xml`.
    Car(String),
}

/// How a camera path is driven.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathMode {
    /// `useTargetProjection`: the target's projection onto the path.
    Projection,
    /// `targetSpeedRelative` [I: driven like `Time`; the speed scaling was not
    /// found].
    TargetSpeed,
    /// The time since activation over `duration`.
    Time,
}

/// A `percentageCurve` (`linearPercentage` or `bezierPercentage`).
#[derive(Clone, Debug, PartialEq)]
pub struct Curve {
    pub bezier: bool,
    /// `Value time value` points.
    pub values: Vec<[f32; 2]>,
}

impl Curve {
    /// The path fraction at the time fraction `t`.
    pub fn at(&self, t: f32) -> f32 {
        let v = &self.values;
        if v.is_empty() {
            return t;
        }
        if self.bezier && v.len() >= 4 {
            // [I] cubic Bézier chains of the values (control times 0, 1/3, 2/3, 1).
            let n = v.len() / 4;
            let x = t.clamp(0., 1.) * n as f32;
            let s = (x.floor() as usize).min(n - 1);
            let u = x - s as f32;
            let k = 1. - u;
            let c = &v[s * 4..s * 4 + 4];
            return c[0][1] * k * k * k + c[1][1] * 3. * k * k * u + c[2][1] * 3. * k * u * u + c[3][1] * u * u * u;
        }
        // Piecewise linear in time.
        if t <= v[0][0] {
            return v[0][1];
        }
        for w in v.windows(2) {
            if t <= w[1][0] {
                let span = w[1][0] - w[0][0];
                let f = if span > 0. { (t - w[0][0]) / span } else { 1. };
                return w[0][1] + (w[1][1] - w[0][1]) * f;
            }
        }
        v[v.len() - 1][1]
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CameraPath {
    /// Bézier chain points, 4 per segment.
    pub points: Vec<Vec3>,
    pub mode: PathMode,
    pub duration: f32,
    pub looped: bool,
    pub curve: Option<Curve>,
}

impl CameraPath {
    fn segments(&self) -> &[[Vec3; 4]] {
        self.points.as_chunks::<4>().0
    }

    /// The point at path fraction `u` (0..1) [I: uniform per segment].
    pub fn at(&self, u: f32) -> Vec3 {
        let segments = self.segments();
        if segments.is_empty() {
            return self.points.first().copied().unwrap_or_default();
        }
        let n = segments.len();
        let x = if u.is_finite() { u.clamp(0., 1.) } else { 0. } * n as f32;
        let s = (x.floor() as usize).min(n - 1);
        bezier(&segments[s], x - s as f32)
    }

    /// The fraction of the point nearest to `target` (dense sampling).
    pub fn nearest(&self, target: Vec3) -> f32 {
        let segments = self.segments();
        let n = segments.len();
        let mut best = (f32::INFINITY, 0.);
        for (s, seg) in segments.iter().enumerate() {
            for k in 0..=32 {
                let t = k as f32 / 32.;
                let d = bezier(seg, t).distance_squared(target);
                if d < best.0 {
                    best = (d, (s as f32 + t) / n as f32);
                }
            }
        }
        best.1
    }

    /// The point `seconds` after activation (0x7F2880): looped paths wrap,
    /// others stop at the end; the curve maps time to path fraction.
    pub fn at_time(&self, seconds: f32) -> Vec3 {
        let duration = if self.duration > 0. { self.duration } else { 1. };
        let mut t = seconds.max(0.);
        if self.looped {
            t %= duration;
        }
        let t = t.min(duration) / duration;
        self.at(self.curve.as_ref().map_or(t, |c| c.at(t)))
    }
}

fn bezier(seg: &[Vec3; 4], t: f32) -> Vec3 {
    let u = 1. - t;
    seg[0] * (u * u * u) + seg[1] * (3. * u * u * t) + seg[2] * (3. * u * t * t) + seg[3] * (t * t * t)
}

#[derive(Clone, Debug, PartialEq)]
pub struct TrackCamera {
    pub ident: String,
    pub kind: Kind,
    pub position: Vec3,
    pub orientation: Quat,
    /// Degrees.
    pub default_fov: f32,
    pub minimum_fov: f32,
    pub maximum_fov: f32,
    pub radius: f32,
    /// Car-local offset of the target point.
    pub target_offset: Vec3,
    pub zoom_per_second: f32,
    pub zoom: bool,
    /// Zoom cameras: turn with angular dynamics instead of snapping.
    pub turn: bool,
    pub turn_acceleration: f32,
    pub turn_damping: f32,
    pub max_turn_speed: f32,
    pub lookahead: f32,
    pub roll: f32,
    /// `sourcePath`.
    pub path: Option<CameraPath>,
    /// `targetPath`.
    pub target_path: Option<CameraPath>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Zone {
    pub centre: Vec3,
    pub rotation: Quat,
    pub half: Vec3,
    pub activation: Option<Activation>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Director {
    pub cameras: Vec<TrackCamera>,
    pub zones: Vec<Zone>,
    pub initial: Option<Activation>,
}

/// The current shot of a trackside camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shot {
    pub eye: Vec3,
    pub rotation: Quat,
    /// Vertical field of view, radians [I].
    pub fov: f32,
}

impl Shot {
    /// The original interpolates the recorded camera state (0x81A110).
    pub fn lerp(self, other: Self, t: f32) -> Self {
        Self {
            eye: self.eye.lerp(other.eye, t),
            rotation: self.rotation.slerp(other.rotation, t),
            fov: self.fov + (other.fov - self.fov) * t,
        }
    }
}

/// A zoom camera's state between updates (the original's camera record).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct State {
    /// Seconds since activation.
    pub clock: f32,
    /// Field of view, radians.
    pub fov: f32,
    source: f32,
    target: f32,
    /// Activation shifts of the projected target (record +0xD0 / +0xE0).
    source_shift: Vec3,
    target_shift: Vec3,
    rotation: Quat,
    angular: Vec3,
}

/// Weight of a projection path's new parameter per update [D 0x819D20].
pub fn projection_weight(speed: f32) -> f32 {
    // m/s to mph: 3600 x 0.001 x 0.6213712 (0xA7DE00).
    let mph = speed.abs() * 3600. * 0.001 * 0.621_371_2;
    (mph * 0.01).clamp(0.1, 0.9)
}

const DEG: f32 = std::f32::consts::PI / 180.;

fn vec3(v: [f32; 3]) -> Vec3 {
    Vec3::from_array(v)
}

fn activation(config: &Config, ident: &str, index: &[String]) -> Option<Activation> {
    if let Some(view) = ident.strip_prefix("target.") {
        return Some(Activation::Car(view.to_owned()));
    }
    index.iter().position(|c| c == ident).map(Activation::Track).filter(|_| config.camera(ident).is_some())
}

fn camera_path(config: &Config, ident: Option<&str>) -> Option<CameraPath> {
    let p = config.path(ident?)?;
    let mode = if p.flag("useTargetProjection") {
        PathMode::Projection
    } else if p.flag("targetSpeedRelative") {
        PathMode::TargetSpeed
    } else {
        PathMode::Time
    };
    let curve = p.attribute("percentageCurve").and_then(|c| config.path(c)).map(|c| Curve {
        bezier: c.kind == "bezierPercentage",
        values: c.values.clone(),
    });
    Some(CameraPath {
        points: p.points.iter().copied().map(vec3).collect(),
        mode,
        duration: p.duration.unwrap_or(1.),
        looped: p.looped,
        curve,
    })
}

impl Director {
    pub fn from_config(config: &Config) -> Self {
        let cameras: Vec<TrackCamera> = config
            .cameras
            .iter()
            .map(|c| {
                let s = |n: &str, d: f32| c.scalar(n).unwrap_or(d);
                let fov = c.scalar("Fov");
                // Unset values: the zoom constructor's defaults (0x823FD0).
                TrackCamera {
                    ident: c.ident.clone(),
                    kind: c.kind,
                    position: vec3(c.vector("Position").unwrap_or_default()),
                    orientation: c
                        .quaternion("orientation")
                        .map_or(Quat::IDENTITY, |q| Quat::from_array(q).normalize()),
                    default_fov: fov.unwrap_or(s("defaultFov", 60.)),
                    minimum_fov: fov.unwrap_or(s("minimumFov", 5.)),
                    maximum_fov: fov.unwrap_or(s("maximumFov", 175.)),
                    radius: s("radius", 2.5),
                    target_offset: vec3(c.vector("targetOffset").unwrap_or_default()),
                    zoom_per_second: s("zoomPerSecond", 1.),
                    zoom: c.kind == Kind::Zoom && c.flag("zoomEnabled").unwrap_or(true),
                    turn: c.kind == Kind::Zoom && c.flag("turnEnabled").unwrap_or(true),
                    turn_acceleration: s("turnAcceleration", 0.5),
                    turn_damping: s("turnDamping", 3.),
                    max_turn_speed: s("maxTurnSpeed", 6.),
                    lookahead: s("lookahead", 0.),
                    roll: s("roll", 0.),
                    path: camera_path(config, c.path.as_deref()),
                    target_path: camera_path(config, c.target_path.as_deref()),
                }
            })
            .collect();
        let index: Vec<String> = cameras.iter().map(|c| c.ident.clone()).collect();
        let zones = config
            .zones
            .iter()
            .map(|z| Zone {
                centre: vec3(z.position),
                rotation: Quat::from_array(z.orientation).normalize(),
                half: vec3(z.size),
                // The ordinary replay's camera: the first non-terminal entry.
                activation: z
                    .entry
                    .iter()
                    .find(|i| !i.starts_with("TERMINAL_"))
                    .and_then(|i| activation(config, i, &index)),
            })
            .collect();
        Self {
            initial: activation(config, &config.initial, &index),
            cameras,
            zones,
        }
    }

    pub fn load(root: &std::path::Path, route: &grid_tools::route::Route) -> Result<Self, String> {
        let path = root.join(route.file("replay_camera_config.xml"));
        let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let config = replay_cameras::read(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(Self::from_config(&config))
    }

    /// Zones the segment `a`..`b` passes through, in path order.
    pub fn crossed(&self, a: Vec3, b: Vec3) -> Vec<usize> {
        let steps = ((a.distance(b) / 0.25).ceil() as usize).clamp(1, 400);
        let mut out: Vec<(usize, usize)> = Vec::new();
        for (z, zone) in self.zones.iter().enumerate() {
            let inverse = zone.rotation.inverse();
            let inside = |p: Vec3| {
                let local = inverse * (p - zone.centre);
                local.abs().cmple(zone.half).all()
            };
            if inside(a) {
                continue;
            }
            if let Some(k) = (1..=steps).find(|&k| inside(a.lerp(b, k as f32 / steps as f32))) {
                out.push((k, z));
            }
        }
        out.sort();
        out.into_iter().map(|(_, z)| z).collect()
    }
}

/// Bevy cameras look along -Z.
fn look(eye: Vec3, target: Vec3, roll: f32) -> Quat {
    look_to((target - eye).normalize_or(Vec3::NEG_Z), roll)
}

fn look_to(forward: Vec3, roll: f32) -> Quat {
    let up = if forward.cross(Vec3::Y).length_squared() < 1e-8 { Vec3::Z } else { Vec3::Y };
    bevy::prelude::Transform::IDENTITY.looking_to(forward, up).rotation * Quat::from_rotation_z(roll)
}

impl TrackCamera {
    /// The car-local target point.
    fn car_target(&self, car: &Pose) -> Vec3 {
        car.to_mat4().transform_point3(self.target_offset)
    }

    /// One path point: projection paths advance `param` (never back).
    fn path_point(path: &CameraPath, param: &mut f32, projected: Vec3, speed: f32, clock: f32) -> Vec3 {
        match path.mode {
            PathMode::Projection => {
                let u = path.nearest(projected);
                if u >= *param {
                    let w = projection_weight(speed);
                    *param = (1. - w) * *param + w * u;
                }
                path.at(*param)
            }
            PathMode::Time | PathMode::TargetSpeed => path.at_time(clock),
        }
    }

    fn eye_and_target(&self, state: &mut State, car: &Pose, velocity: Vec3) -> (Vec3, Vec3) {
        let base = self.car_target(car);
        let speed = velocity.length();
        let eye = match &self.path {
            Some(p) => Self::path_point(p, &mut state.source, base + state.source_shift, speed, state.clock),
            None => self.position,
        };
        let target = match &self.target_path {
            Some(p) => Self::path_point(p, &mut state.target, base + state.target_shift, speed, state.clock),
            // AUTHORED lookahead (the original's smoothing was not decoded).
            None => base + velocity * self.lookahead,
        };
        // Eye on the target: lifted 1 m (0x824160).
        if eye.distance_squared(target) < 0.001 {
            return (Vec3::new(eye.x, target.y + 1., eye.z), target);
        }
        (eye, target)
    }

    fn fixed_fov(&self) -> bool {
        !self.zoom || self.minimum_fov == self.maximum_fov || self.zoom_per_second == 0.
    }

    /// The framing goal (0x808C30), radians.
    fn goal(&self, eye: Vec3, target: Vec3) -> f32 {
        let (lo, hi) = (self.minimum_fov.min(self.maximum_fov), self.minimum_fov.max(self.maximum_fov));
        let distance = eye.distance(target);
        let ratio = if distance > 0. { self.radius.abs() / distance } else { 0. };
        ratio.atan().clamp(lo * DEG, hi * DEG)
    }

    /// The zoom step (0x819820): towards the goal at `zoomPerSecond`, slowing
    /// within `zoomPerSecond` x 0.5 s of a limit when the limits are at
    /// least 1 s apart. The rate's sign is ignored (one Long Beach camera
    /// ships -6; the original would zoom away from its goal).
    fn zoom_step(&self, current: f32, goal: f32, dt: f32) -> f32 {
        let rate = self.zoom_per_second.abs();
        let (lo, hi) = (self.minimum_fov.min(self.maximum_fov), self.minimum_fov.max(self.maximum_fov));
        let ease = if (hi - lo) / rate >= 0.5 * 2. { rate * 0.5 * DEG } else { 0.001 };
        let delta = goal - current;
        let room = if delta >= 0. { hi * DEG - current } else { current - lo * DEG };
        let step = rate * (room / ease).clamp(0., 1.) * dt.max(0.) * DEG;
        if step < delta.abs() { current + step.copysign(delta) } else { goal }
    }

    /// The state on activation (0x843700) and its shot.
    pub fn start(&self, car: &Pose, velocity: Vec3) -> (State, Shot) {
        let base = self.car_target(car);
        let shift = |p: &Option<CameraPath>| p.as_ref().map_or(Vec3::ZERO, |p| p.at_time(0.) - base);
        let mut state = State {
            clock: 0.,
            fov: self.default_fov * DEG,
            source: 0.,
            target: 0.,
            source_shift: shift(&self.path),
            target_shift: shift(&self.target_path),
            rotation: self.orientation,
            angular: Vec3::ZERO,
        };
        if self.kind == Kind::Static {
            return (state, self.static_shot());
        }
        let (eye, target) = self.eye_and_target(&mut state, car, velocity);
        if self.default_fov == 0. {
            state.fov = self.goal(eye, target);
        }
        state.rotation = look(eye, target, self.roll);
        (
            state,
            Shot {
                eye,
                rotation: state.rotation,
                fov: state.fov,
            },
        )
    }

    fn static_shot(&self) -> Shot {
        Shot {
            eye: self.position,
            rotation: self.orientation,
            fov: self.default_fov * DEG,
        }
    }

    /// One update `dt` seconds after the previous one (0x824160).
    pub fn step(&self, state: &mut State, car: &Pose, velocity: Vec3, dt: f32) -> Shot {
        if self.kind == Kind::Static {
            return self.static_shot();
        }
        state.clock += dt.max(0.);
        let (eye, target) = self.eye_and_target(state, car, velocity);
        state.fov = if self.fixed_fov() {
            self.default_fov * DEG
        } else {
            self.zoom_step(state.fov, self.goal(eye, target), dt)
        };
        let goal = look(eye, target, self.roll);
        state.rotation = if self.turn { self.turn_step(state, goal, dt) } else { goal };
        Shot {
            eye,
            rotation: state.rotation,
            fov: state.fov,
        }
    }

    /// Angular dynamics towards the look direction (0x808E20).
    fn turn_step(&self, state: &mut State, goal: Quat, dt: f32) -> Quat {
        let from = state.rotation * Vec3::NEG_Z;
        let to = goal * Vec3::NEG_Z;
        let angle = from.dot(to).clamp(-1., 1.).acos();
        let wanted = if angle < 1e-4 || dt <= 0. {
            Vec3::ZERO
        } else {
            from.cross(to).normalize_or_zero() * (angle / dt)
        };
        let dt = dt.max(0.);
        let mut w = state.angular;
        w += self.turn_acceleration * (wanted - w) * dt;
        w -= self.turn_damping * w * dt;
        if w.length() > self.max_turn_speed {
            w = w.normalize_or_zero() * self.max_turn_speed;
        }
        state.angular = w;
        let turn = w.length() * dt;
        let forward = if turn >= 1e-4 {
            Quat::from_axis_angle(w.normalize(), turn) * from
        } else {
            from
        };
        look_to(forward.normalize_or(to), self.roll)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use grid_formats::replay_cameras::{Camera, Path, Value, Zone as RawZone};

    fn config() -> Config {
        let camera = |ident: &str, kind, extra: Vec<(String, Value)>| Camera {
            kind,
            ident: ident.into(),
            path: None,
            target_path: None,
            parameters: [
                ("Position".to_owned(), Value::Vector([0., 5., 0.])),
                ("orientation".to_owned(), Value::Quaternion([0., 0., 0., 1.])),
            ]
            .into_iter()
            .chain(extra)
            .collect(),
        };
        let zoom = vec![
            ("defaultFov".to_owned(), Value::Scalar(10.)),
            ("minimumFov".to_owned(), Value::Scalar(5.)),
            ("maximumFov".to_owned(), Value::Scalar(30.)),
            ("radius".to_owned(), Value::Scalar(2.)),
            ("zoomPerSecond".to_owned(), Value::Scalar(10.)),
            ("zoomEnabled".to_owned(), Value::Bool(true)),
            ("turnEnabled".to_owned(), Value::Bool(false)),
        ];
        let mut pathed = camera("pathed", Kind::Zoom, zoom.clone());
        pathed.path = Some("line".into());
        Config {
            route: "0".into(),
            initial: "first".into(),
            cameras: vec![
                camera("first", Kind::Zoom, zoom.clone()),
                camera("TERMINAL_second", Kind::Zoom, zoom.clone()),
                camera("second", Kind::Static, vec![("Fov".into(), Value::Scalar(55.))]),
                pathed,
            ],
            zones: vec![RawZone {
                ident: "gate".into(),
                size: [1., 5., 10.],
                position: [50., 0., 0.],
                orientation: [0., 0., 0., 1.],
                entry: vec!["TERMINAL_second".into(), "second".into()],
            }],
            paths: vec![Path {
                ident: "line".into(),
                kind: "spline".into(),
                duration: Some(10.),
                looped: false,
                attributes: vec![("useTargetProjection".into(), "true".into())],
                points: vec![[0., 5., 0.], [10., 5., 0.], [20., 5., 0.], [30., 5., 0.]],
                values: Vec::new(),
            }],
        }
    }

    fn at(x: f32, z: f32) -> Pose {
        Pose::from_mat4(bevy::math::Mat4::from_translation(Vec3::new(x, 5., z)))
    }

    #[test]
    fn zones_fire_on_crossing_and_choose_the_ordinary_camera() {
        let d = Director::from_config(&config());
        assert_eq!(d.initial, Some(Activation::Track(0)));
        assert_eq!(d.zones[0].activation, Some(Activation::Track(2)));
        // A 3 m step across a 2 m thick gate is still seen.
        assert_eq!(d.crossed(Vec3::new(48.5, 0., 0.), Vec3::new(51.5, 0., 0.)), [0]);
        assert!(d.crossed(Vec3::new(40., 0., 0.), Vec3::new(45., 0., 0.)).is_empty());
        // Starting inside is not an entry.
        assert!(d.crossed(Vec3::new(50., 0., 0.), Vec3::new(52., 0., 0.)).is_empty());
        // Outside the gate's width.
        assert!(d.crossed(Vec3::new(48., 0., 30.), Vec3::new(52., 0., 30.)).is_empty());
    }

    #[test]
    fn a_shot_starts_at_the_default_fov_and_zooms_to_the_framing_goal() {
        let d = Director::from_config(&config());
        let camera = &d.cameras[0];
        let car = at(0., -40.);
        let (mut state, shot) = camera.start(&car, Vec3::ZERO);
        assert_eq!(shot.fov, 10. * DEG);
        // Looks straight at the car (Bevy cameras look along -Z).
        assert!((shot.rotation * Vec3::NEG_Z).distance(Vec3::NEG_Z) < 1e-5);
        // Goal atan(2 / 40) = 2.86 deg, clamped to the 5 deg minimum.
        let s = camera.step(&mut state, &car, Vec3::ZERO, 0.1);
        assert!((s.fov - 9. * DEG).abs() < 1e-6, "{}", s.fov / DEG);
        for _ in 0..100 {
            camera.step(&mut state, &car, Vec3::ZERO, 0.1);
        }
        assert!((state.fov - 5. * DEG).abs() < 1e-6);
        // Equal limits: the default FOV, whatever the distance.
        let mut fixed = camera.clone();
        fixed.maximum_fov = fixed.minimum_fov;
        let (mut state, _) = fixed.start(&car, Vec3::ZERO);
        assert_eq!(fixed.step(&mut state, &car, Vec3::ZERO, 1.).fov, 10. * DEG);
        // Negative rate (Long Beach): still towards the goal.
        let mut odd = camera.clone();
        odd.zoom_per_second = -6.;
        let (mut state, _) = odd.start(&car, Vec3::ZERO);
        assert!(odd.step(&mut state, &car, Vec3::ZERO, 0.1).fov < 10. * DEG);
        // Static cameras keep their FOV and orientation.
        let (mut state, s) = d.cameras[2].start(&car, Vec3::ZERO);
        assert_eq!(s.fov, 55. * DEG);
        assert_eq!(d.cameras[2].step(&mut state, &car, Vec3::ZERO, 1.).rotation, Quat::IDENTITY);
    }

    #[test]
    fn a_projection_path_follows_the_car_forwards_only() {
        let d = Director::from_config(&config());
        let camera = &d.cameras[3];
        assert_eq!(camera.path.as_ref().unwrap().mode, PathMode::Projection);
        // Activated with the car 20 m off the path start: the path start.
        let (mut state, shot) = camera.start(&at(0., 20.), Vec3::ZERO);
        assert!(shot.eye.distance(Vec3::new(0., 5., 0.)) < 1e-4);
        // The car moves 15 m along: the eye eases after it (weight 0.1 at rest).
        let s = camera.step(&mut state, &at(15., 20.), Vec3::ZERO, 1. / 30.);
        assert!((s.eye.x - 1.5).abs() < 0.6, "{}", s.eye);
        let fast = Vec3::new(80., 0., 0.); // 179 mph: weight 0.9
        let s = camera.step(&mut state, &at(15., 20.), fast, 1. / 30.);
        assert!((s.eye.x - 13.6).abs() < 0.8, "{}", s.eye);
        // Backwards: the eye stays.
        let s2 = camera.step(&mut state, &at(2., 20.), fast, 1. / 30.);
        assert_eq!(s2.eye, s.eye);
        assert_eq!(projection_weight(0.), 0.1);
        assert_eq!(projection_weight(1000.), 0.9);
    }

    #[test]
    fn time_paths_follow_their_curve() {
        let path = CameraPath {
            points: vec![Vec3::ZERO, Vec3::X * 10., Vec3::X * 20., Vec3::X * 30.],
            mode: PathMode::Time,
            duration: 2.,
            looped: false,
            curve: Some(Curve {
                bezier: false,
                values: vec![[0., 0.], [0.5, 0.], [1., 1.]],
            }),
        };
        assert_eq!(path.at_time(0.5), Vec3::ZERO);
        assert!(path.at_time(2.).distance(Vec3::X * 30.) < 1e-4);
        assert!(path.at_time(9.).distance(Vec3::X * 30.) < 1e-4);
        let bezier = Curve {
            bezier: true,
            values: vec![[0., 0.], [0.33, 0.13], [0.67, 1.], [1., 1.]],
        };
        assert_eq!(bezier.at(0.), 0.);
        assert!((bezier.at(1.) - 1.).abs() < 1e-6);
    }

    #[test]
    fn turning_cameras_lag_and_settle() {
        let d = Director::from_config(&config());
        let mut camera = d.cameras[0].clone();
        camera.turn = true;
        let (mut state, _) = camera.start(&at(0., -40.), Vec3::ZERO);
        // The car jumps sideways: the camera does not snap.
        let car = at(40., -40.);
        let s = camera.step(&mut state, &car, Vec3::ZERO, 1. / 30.);
        let goal = (Vec3::new(40., 5., -40.) - Vec3::new(0., 5., 0.)).normalize();
        let first = (s.rotation * Vec3::NEG_Z).angle_between(goal);
        assert!(first > 0.5, "{first}");
        for _ in 0..600 {
            camera.step(&mut state, &car, Vec3::ZERO, 1. / 30.);
        }
        let last = (state.rotation * Vec3::NEG_Z).angle_between(goal);
        assert!(last < 0.01, "{last}");
    }
}
