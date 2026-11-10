//! The front-end camera noise (docs/FRONT_END_FLOW.md §2, "Camera noise"):
//! a slow random orbit of a camera about its target, tuned by a
//! `<camera_tweaks><NAME><noise_tweaks>` block of
//! `frontend/persistentdatarender.xml` (`default`, `pause`, `loading`,
//! `in_game_cameras`).
//!
//! [D, static analysis of the unpacked exe]: the noise object (constructor
//! 0x487D00, update 0x47ECB0) holds two three-octave generators (0x476720).
//! Each frame it evaluates them at `rotation_rate × dt`, scales the first by
//! `noise_rotation_x` and the second by `noise_rotation_y`, and orbits the
//! camera's position about its target by those two angles in radians
//! (0x47C7F0): the first about the camera's side axis (pitch), the second
//! about its up axis (yaw). `noise_rotation_z` is read but the update does
//! not use it. A dampen share (0 by default) fades the angles by
//! `2f² − f³`, f = 1 − share; nothing in the loading path is known to set it,
//! so the port keeps the full angles [I].
use bevy::math::{Quat, Vec3};

/// The `noise_tweaks` of one `camera_tweaks` block.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tweaks {
    /// `noise_rotation_x`, `_y`, `_z` scales (radians at full noise).
    pub rotation: [f32; 3],
    /// `rotation_rate`: lattice steps per second of the noise.
    pub rate: f32,
}

/// The constructor's defaults (0x487D00) [D].
pub const DEFAULT: Tweaks = Tweaks { rotation: [1.2, 1.2, 1.01], rate: 1.066 };

impl Tweaks {
    /// The block `<NAME>` of `<camera_tweaks>`; a missing value keeps the
    /// constructor's default, a missing block gives `None`.
    pub fn read(xml: &str, name: &str) -> Option<Tweaks> {
        let tweaks = &xml[xml.find("<camera_tweaks>")?..];
        let tweaks = &tweaks[..tweaks.find("</camera_tweaks>")?];
        let block = &tweaks[tweaks.find(&format!("<{name}>"))?..];
        let block = &block[..block.find(&format!("</{name}>"))?];
        let noise = &block[block.find("<noise_tweaks>")?..];
        let noise = &noise[..noise.find("</noise_tweaks>")?];
        let value = |tag: &str, default: f32| -> f32 {
            let Some(at) = noise.find(&format!("<{tag}")) else { return default };
            let line = &noise[at..];
            let line = &line[..line.find("/>").unwrap_or(line.len())];
            line.find("scale=\"")
                .and_then(|i| {
                    let v = &line[i + 7..];
                    v[..v.find('"')?].trim().parse().ok()
                })
                .unwrap_or(default)
        };
        Some(Tweaks {
            rotation: [
                value("noise_rotation_x", DEFAULT.rotation[0]),
                value("noise_rotation_y", DEFAULT.rotation[1]),
                value("noise_rotation_z", DEFAULT.rotation[2]),
            ],
            rate: value("rotation_rate", DEFAULT.rate),
        })
    }
}

/// The integer lattice noise of an octave: 1 − h/2³⁰ with h the 31-bit hash
/// of index `n`, in (−1, 1] [D 0x484640].
fn lattice(n: u32) -> f32 {
    let n = (n << 13) ^ n;
    let h = n.wrapping_mul(n).wrapping_mul(15731).wrapping_add(789_221).wrapping_mul(n).wrapping_add(1_376_312_589) & 0x7fff_ffff;
    1. - h as f32 * 9.313_226e-10
}

/// One octave: four lattice values and a cubic through them [D 0x484640,
/// 0x484780].
#[derive(Clone, Debug)]
struct Octave {
    t: f32,
    v: [f32; 4],
    next: u32,
}

impl Octave {
    fn new(start: u32) -> Octave {
        Octave { t: 0., v: std::array::from_fn(|i| lattice(start + i as u32)), next: start + 4 }
    }

    /// Advances by `step` lattice units and returns the value there.
    fn eval(&mut self, step: f32) -> f32 {
        self.t += step;
        while self.t > 1. {
            self.v = [self.v[1], self.v[2], self.v[3], lattice(self.next)];
            self.next = self.next.wrapping_add(1);
            self.t -= 1.;
        }
        let [v0, v1, v2, v3] = self.v;
        let p = (v3 - v2) - (v0 - v1);
        let q = (v0 - v1) - p;
        let (r, s, t) = (v2 - v0, v1, self.t);
        ((p * t + q) * t + r) * t + s
    }
}

/// Three octaves at frequencies 1, 2, 4 and weights 1, ½, ¼, normalised by
/// their sum [D 0x476720].
#[derive(Clone, Debug)]
struct Generator([Octave; 3]);

impl Generator {
    fn eval(&mut self, step: f32) -> f32 {
        let mut sum = 0.;
        for (k, octave) in self.0.iter_mut().enumerate() {
            let f = (1 << k) as f32;
            sum += octave.eval(f * step) / f;
        }
        sum / 1.75
    }
}

/// The C runtime's `rand` (MSVC: state × 214013 + 2531011, bits 16..30),
/// which picks each octave's start index as `rand() % 100` [D 0x484640].
/// The seed is not known; the port starts at the runtime's default 1 [I].
struct Rand(u32);

impl Rand {
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(214_013).wrapping_add(2_531_011);
        (self.0 >> 16) & 0x7fff
    }
}

/// A camera's noise state.
#[derive(Clone, Debug)]
pub struct CameraNoise {
    tweaks: Tweaks,
    pitch: Generator,
    yaw: Generator,
}

impl CameraNoise {
    pub fn new(tweaks: Tweaks) -> CameraNoise {
        let mut rand = Rand(1);
        let mut generator = || Generator(std::array::from_fn(|_| Octave::new(rand.next() % 100)));
        let pitch = generator();
        let yaw = generator();
        CameraNoise { tweaks, pitch, yaw }
    }

    /// The (pitch, yaw) angles in radians after `dt` more seconds.
    pub fn angles(&mut self, dt: f32) -> (f32, f32) {
        let step = self.tweaks.rate * dt;
        (self.tweaks.rotation[0] * self.pitch.eval(step), self.tweaks.rotation[1] * self.yaw.eval(step))
    }
}

/// `eye` orbited about `target` by `pitch` about the camera's side axis and
/// `yaw` about its up axis, with `up` the world up [D 0x47C7F0; the order of
/// the two turns and their signs are not resolved, which a symmetric random
/// angle does not show, I].
pub fn orbit(eye: Vec3, target: Vec3, up: Vec3, pitch: f32, yaw: f32) -> Vec3 {
    let offset = eye - target;
    let dir = offset.normalize_or_zero();
    let side = up.cross(dir).normalize_or_zero();
    if side == Vec3::ZERO {
        return eye;
    }
    let cam_up = dir.cross(side);
    let turned = Quat::from_axis_angle(side, pitch) * (Quat::from_axis_angle(cam_up, yaw) * offset);
    target + turned
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_loading_block_reads_its_own_values() {
        let xml = r#"<camera_tweaks><default><noise_tweaks>
            <noise_rotation_x scale="0.5" /></noise_tweaks></default>
            <loading><spring_tweaks><centering_tension scale="1.0" /></spring_tweaks>
            <noise_tweaks>
            <noise_rotation_x scale="0.1" /> <!-- x -->
            <noise_rotation_y scale="0.1" />
            <noise_rotation_z scale="0.005" />
            <rotation_rate    scale="0.066" />
            <dampen_to_zero   duration="0.2" />
            </noise_tweaks></loading></camera_tweaks>"#;
        let t = Tweaks::read(xml, "loading").unwrap();
        assert_eq!(t, Tweaks { rotation: [0.1, 0.1, 0.005], rate: 0.066 });
        let d = Tweaks::read(xml, "default").unwrap();
        assert_eq!(d.rotation, [0.5, 1.2, 1.01]);
        assert!(Tweaks::read(xml, "pause").is_none());
    }

    #[test]
    fn the_lattice_is_the_integer_noise() {
        // n = 0 hashes to 1376312589 & 0x7fffffff.
        assert!((lattice(0) - (1. - 1_376_312_589. / 1_073_741_824.)).abs() < 1e-6);
        assert!((0..1000).map(lattice).all(|v| (-1.0..=1.0).contains(&v)));
    }

    #[test]
    fn an_octave_passes_through_its_lattice_values() {
        let mut o = Octave::new(7);
        let v = o.v;
        // At t = 0 the cubic is v1; one full step later it is v2.
        assert!((o.eval(0.) - v[1]).abs() < 1e-6);
        assert!((o.eval(1.) - v[2]).abs() < 1e-6, "t = 1 stays in the window");
        assert!((o.eval(1e-6) - v[2]).abs() < 1e-3, "then it shifts");
        assert_eq!(o.v[2], v[3]);
    }

    #[test]
    fn the_loading_noise_stays_small_and_slow() {
        let mut n = CameraNoise::new(Tweaks { rotation: [0.1, 0.1, 0.005], rate: 0.066 });
        let mut last = n.angles(0.);
        let mut peak = 0f32;
        for _ in 0..60 * 60 {
            let a = n.angles(1. / 60.);
            peak = peak.max(a.0.abs()).max(a.1.abs());
            // At 60 Hz the angles move by far less than a milliradian a frame.
            assert!((a.0 - last.0).abs() < 1e-3 && (a.1 - last.1).abs() < 1e-3);
            last = a;
        }
        assert!(peak > 0.005 && peak < 0.13, "{peak}");
    }

    #[test]
    fn the_orbit_keeps_the_distance_and_turns_by_the_angles() {
        let (eye, target) = (Vec3::new(0., 2., 10.), Vec3::ZERO);
        let d = eye.distance(target);
        let yawed = orbit(eye, target, Vec3::Y, 0., 0.1);
        assert!((yawed.distance(target) - d).abs() < 1e-4);
        assert!(((eye - target).angle_between(yawed - target) - 0.1).abs() < 1e-4);
        // Yaw turns about the camera's own up axis: for a level camera that
        // is the world up, so the height stays.
        let level = Vec3::new(0., 0., 10.);
        let yawed = orbit(level, target, Vec3::Y, 0., 0.1);
        assert!(yawed.y.abs() < 1e-5, "a level camera's yaw keeps the height");
        assert!((level.angle_between(yawed) - 0.1).abs() < 1e-4);
        let pitched = orbit(eye, target, Vec3::Y, 0.1, 0.);
        assert!(((eye - target).angle_between(pitched - target) - 0.1).abs() < 1e-4);
        assert!(pitched.x.abs() < 1e-5, "pitch stays in the vertical plane");
        assert!(orbit(eye, target, Vec3::Y, 0., 0.).distance(eye) < 1e-5);
    }
}
