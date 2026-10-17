//! The original's replay director: trackside cameras switched by activation
//! zones that the replay's target car passes (`replay_camera_config.xml`,
//! grid_formats::replay_cameras; docs/REPLAY.md).
//!
//! [D] from the data: each zone's `entry` handler activates one ordinary
//! camera (and one `TERMINAL_` camera used by the terminal-damage replay);
//! `target.NAME` activates the target car's own `cameras.xml` view NAME;
//! `initialCamera` is used until the first zone. Camera parameters are read
//! from the file.
//!
//! [I]/AUTHORED camera behaviour (the original's camera classes were not
//! decompiled): a zoom camera stands at `Position` (or on its `sourcePath`
//! Bézier chain at the point nearest the car, `useTargetProjection`), looks
//! at the car plus `targetOffset` and `lookahead` x velocity, and zooms so
//! that a sphere of `radius` fills a third of the view: fov = 2 atan(3 x
//! radius / distance) (the factor 3 is AUTHORED: a sphere filling the whole
//! view put the car's bonnet across the screen), clamped to
//! `minimumFov`..`maximumFov` and rate limited by `zoomPerSecond` (degrees
//! per second); a static camera keeps `Fov` and
//! turns to the car when `turnEnabled`, else keeps its `orientation`. Zones
//! are oriented boxes with `size` as half extents [I: on the four circuits
//! the racing line passes 9-11 m below the zone centres, inside only with
//! half extents]; a zone fires when the
//! segment between two replay samples of the car crosses it. Shake is not
//! reproduced.
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

#[derive(Clone, Debug, PartialEq)]
pub struct TrackCamera {
    pub ident: String,
    pub kind: Kind,
    pub position: Vec3,
    pub orientation: Quat,
    pub default_fov: f32,
    pub minimum_fov: f32,
    pub maximum_fov: f32,
    pub radius: f32,
    pub target_offset: Vec3,
    pub zoom_per_second: f32,
    pub zoom: bool,
    pub turn: bool,
    pub lookahead: f32,
    pub roll: f32,
    /// Bézier chain points (4 per segment) of `sourcePath`.
    pub path: Vec<Vec3>,
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

/// AUTHORED: the `radius` sphere fills this fraction (1/3) of the view.
pub const FRAMING: f32 = 3.;

/// The current shot of a trackside camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shot {
    pub eye: Vec3,
    pub rotation: Quat,
    /// Vertical field of view, radians [I].
    pub fov: f32,
}

fn vec3(v: [f32; 3]) -> Vec3 {
    Vec3::from_array(v)
}

fn activation(config: &Config, ident: &str, index: &[String]) -> Option<Activation> {
    if let Some(view) = ident.strip_prefix("target.") {
        return Some(Activation::Car(view.to_owned()));
    }
    index.iter().position(|c| c == ident).map(Activation::Track).filter(|_| config.camera(ident).is_some())
}

impl Director {
    pub fn from_config(config: &Config) -> Self {
        let cameras: Vec<TrackCamera> = config
            .cameras
            .iter()
            .map(|c| {
                let s = |n: &str, d: f32| c.scalar(n).unwrap_or(d);
                let fov = c.scalar("Fov");
                TrackCamera {
                    ident: c.ident.clone(),
                    kind: c.kind,
                    position: vec3(c.vector("Position").unwrap_or_default()),
                    orientation: c
                        .quaternion("orientation")
                        .map_or(Quat::IDENTITY, |q| Quat::from_array(q).normalize()),
                    default_fov: fov.unwrap_or(s("defaultFov", 45.)),
                    minimum_fov: fov.unwrap_or(s("minimumFov", 5.)),
                    maximum_fov: fov.unwrap_or(s("maximumFov", 60.)),
                    radius: s("radius", 2.),
                    target_offset: vec3(c.vector("targetOffset").unwrap_or_default()),
                    zoom_per_second: s("zoomPerSecond", 0.),
                    zoom: c.kind == Kind::Zoom && c.flag("zoomEnabled").unwrap_or(false),
                    turn: c.kind == Kind::Zoom || c.flag("turnEnabled").unwrap_or(false),
                    lookahead: s("lookahead", 0.),
                    roll: s("roll", 0.),
                    path: c
                        .path
                        .as_deref()
                        .and_then(|p| config.path(p))
                        .map(|p| p.points.iter().copied().map(vec3).collect())
                        .unwrap_or_default(),
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

/// Bézier chain point nearest to `target` (dense sampling).
pub fn project_on_path(path: &[Vec3], target: Vec3) -> Option<Vec3> {
    let mut best: Option<(f32, Vec3)> = None;
    for seg in path.as_chunks::<4>().0 {
        for k in 0..=32 {
            let t = k as f32 / 32.;
            let u = 1. - t;
            let p = seg[0] * (u * u * u)
                + seg[1] * (3. * u * u * t)
                + seg[2] * (3. * u * t * t)
                + seg[3] * (t * t * t);
            let d = p.distance_squared(target);
            if best.is_none_or(|(b, _)| d < b) {
                best = Some((d, p));
            }
        }
    }
    best.map(|(_, p)| p)
}

/// Bevy cameras look along -Z.
fn look(eye: Vec3, target: Vec3, roll: f32) -> Quat {
    let forward = (target - eye).normalize_or(Vec3::NEG_Z);
    let base = bevy::prelude::Transform::from_translation(eye)
        .looking_to(forward, Vec3::Y)
        .rotation;
    base * Quat::from_rotation_z(roll)
}

impl TrackCamera {
    /// Frames the car; `fov` carries the zoom state (degrees) between frames.
    pub fn shot(&self, car: &Pose, velocity: Vec3, fov: &mut Option<f32>, dt: f32) -> Shot {
        let target = car.translation() + self.target_offset + velocity * self.lookahead;
        let eye = project_on_path(&self.path, car.translation()).unwrap_or(self.position);
        let rotation = if self.turn {
            look(eye, target, self.roll)
        } else {
            self.orientation
        };
        let goal = self.zoom.then(|| {
            let distance = eye.distance(target).max(0.1);
            // Shipped data is not trusted to be ordered or positive (one Long
            // Beach camera zooms at -6 per second): magnitudes, no panics.
            let (lo, hi) = (
                self.minimum_fov.min(self.maximum_fov),
                self.minimum_fov.max(self.maximum_fov),
            );
            (2. * (FRAMING * self.radius.abs() / distance).atan())
                .to_degrees()
                .max(lo)
                .min(hi)
        });
        // A cut starts framed (AUTHORED); the zoom rate limits what follows.
        let current = fov.get_or_insert(goal.filter(|g| g.is_finite()).unwrap_or(self.default_fov));
        if let Some(goal) = goal {
            let step = (self.zoom_per_second.abs() * dt.max(0.)).min(180.);
            let change = (goal - *current).max(-step).min(step);
            if change.is_finite() {
                *current += change;
            }
        }
        Shot {
            eye,
            rotation,
            fov: current.to_radians(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use grid_formats::replay_cameras::{Camera, Value, Zone as RawZone};

    fn config() -> Config {
        let camera = |ident: &str, kind, extra: Vec<(String, Value)>| Camera {
            kind,
            ident: ident.into(),
            path: None,
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
        ];
        Config {
            route: "0".into(),
            initial: "first".into(),
            cameras: vec![
                camera("first", Kind::Zoom, zoom.clone()),
                camera("TERMINAL_second", Kind::Zoom, zoom.clone()),
                camera("second", Kind::Static, vec![("Fov".into(), Value::Scalar(55.))]),
            ],
            zones: vec![RawZone {
                ident: "gate".into(),
                size: [1., 5., 10.],
                position: [50., 0., 0.],
                orientation: [0., 0., 0., 1.],
                entry: vec!["TERMINAL_second".into(), "second".into()],
            }],
            paths: Vec::new(),
        }
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
    fn zoom_frames_the_car_within_its_limits() {
        let d = Director::from_config(&config());
        let camera = &d.cameras[0];
        let car = Pose::from_mat4(bevy::math::Mat4::from_translation(Vec3::new(0., 5., -40.)));
        let mut fov = None;
        let shot = camera.shot(&car, Vec3::ZERO, &mut fov, 0.);
        // A cut starts at the framing goal, 2 atan(3 x 2/40) = 17.06 deg.
        let goal = (2. * (6f32 / 40.).atan()).to_degrees();
        assert_eq!(fov, Some(goal));
        fov = Some(10.);
        // Looks straight at the car (Bevy cameras look along -Z).
        assert!((shot.rotation * Vec3::NEG_Z).distance(Vec3::NEG_Z) < 1e-5);
        // From 10 deg it zooms out at 10 deg/s.
        camera.shot(&car, Vec3::ZERO, &mut fov, 0.1);
        assert_eq!(fov, Some(11.));
        for _ in 0..100 {
            camera.shot(&car, Vec3::ZERO, &mut fov, 0.1);
        }
        assert!((fov.unwrap() - goal).abs() < 1e-4, "{fov:?}");
        // Unordered limits and a negative zoom rate (shipped on Long Beach).
        let mut odd = camera.clone();
        odd.zoom_per_second = -6.;
        (odd.minimum_fov, odd.maximum_fov) = (30., 5.);
        let mut fov = Some(20.);
        odd.shot(&car, Vec3::ZERO, &mut fov, 1.);
        assert!((fov.unwrap() - goal).abs() < 1e-4, "{fov:?}");
        let static_shot = d.cameras[2].shot(&car, Vec3::ZERO, &mut None, 1.);
        assert_eq!(static_shot.fov, 55f32.to_radians());
    }

    #[test]
    fn path_cameras_slide_to_the_nearest_point() {
        let path = [
            Vec3::ZERO,
            Vec3::new(10., 0., 0.),
            Vec3::new(20., 0., 0.),
            Vec3::new(30., 0., 0.),
        ];
        let p = project_on_path(&path, Vec3::new(15., 10., 0.)).unwrap();
        assert!((p - Vec3::new(15., 0., 0.)).length() < 0.6, "{p}");
        assert_eq!(project_on_path(&[], Vec3::ZERO), None);
    }
}
