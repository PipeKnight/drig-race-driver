//! Phone tilt to steering (P11, docs/PLATFORMS_AND_INPUT_RESEARCH.md 6.2).
//!
//! Tilt is not an original GRID input, so everything here is AUTHORED. The
//! sensor gives the world "up" direction in device coordinates (game rotation
//! vector, or the accelerometer as a fallback). The steering angle is the
//! rotation of the phone about its screen normal in landscape, like turning a
//! steering wheel, relative to a calibrated neutral. It leaves the action
//! layer as a bi-axis raw value 0.5 + 0.5 tilt, so the original biLower /
//! biUpper calibration (dead zone, saturation, Advanced Controls) applies to
//! it as to a pad stick. Platform independent; the sensor itself is in
//! `android_sensor.rs`.

/// Settings of the `[input]` section (`tilt_*` keys).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// Phone rotation (degrees) that gives full lock. AUTHORED default 30 (the plan's).
    pub max_angle: f32,
    /// Fixed neutral angle in degrees; `None` captures it from the first
    /// reading and from the touch HUD's re-centre button.
    pub neutral: Option<f32>,
    /// Time constant (seconds) of the one-pole low-pass on the steering
    /// value; 0 turns it off. AUTHORED default 0.05 s.
    pub smoothing: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            max_angle: 30.,
            neutral: None,
            smoothing: 0.05,
        }
    }
}

/// Below this length of the up vector's projection on the screen plane (the
/// phone is within about 14 degrees of lying flat) the angle is unreliable,
/// so the last steering value is kept. AUTHORED.
const FLAT: f32 = 0.25;

/// Landscape orientations are inferred from gravity: the display uses
/// `sensorLandscape`, and the NDK has no display-rotation API (JNI
/// `Display.getRotation` is the alternative). A flip needs the phone more
/// than this far (degrees) from the current landscape, so steering never
/// reaches it (full lock is `max_angle` <= 90).
const FLIP: f32 = 135.;

/// World up in device coordinates from a rotation-vector quaternion
/// (x, y, z, w): the third row of SensorManager.getRotationMatrixFromVector.
// Used by the Android sensor thread and the tests.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn up_from_rotation_vector(x: f32, y: f32, z: f32, w: f32) -> [f32; 3] {
    [
        2. * (x * z - y * w),
        2. * (y * z + x * w),
        1. - 2. * (x * x + y * y),
    ]
}

/// The accelerometer at rest measures the reaction to gravity, which points up.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn up_from_acceleration(a: [f32; 3]) -> Option<[f32; 3]> {
    let n = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    (n.is_finite() && n > 1e-3).then(|| a.map(|v| v / n))
}

/// Wraps degrees into (-180, 180].
fn wrap(deg: f32) -> f32 {
    let w = deg.rem_euclid(360.);
    if w > 180. { w - 360. } else { w }
}

/// Angle of up in the screen plane, measured from the device's +y (top)
/// toward +x (right): 0 upright portrait, +90 landscape with the top edge
/// to the left (display rotation 90), -90 with the top to the right.
fn plane_angle(up: [f32; 3]) -> Option<f32> {
    let n = (up[0] * up[0] + up[1] * up[1]).sqrt();
    (n.is_finite() && n >= FLAT).then(|| up[0].atan2(up[1]).to_degrees())
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tilt {
    pub settings: Settings,
    /// Base plane angle of the current landscape (+90 or -90).
    base: Option<f32>,
    neutral: Option<f32>,
    /// Smoothed steering value, positive right.
    value: f32,
    /// Latest raw steering angle (degrees, positive clockwise = right).
    angle: Option<f32>,
}

impl Tilt {
    pub fn new(settings: Settings) -> Self {
        Self {
            settings,
            base: None,
            neutral: settings.neutral,
            value: 0.,
            angle: None,
        }
    }

    /// Steering angle of the phone in degrees, positive when it is turned
    /// clockwise (to the right) as seen from the front; updates the inferred
    /// landscape orientation.
    pub fn angle(&mut self, up: [f32; 3]) -> Option<f32> {
        let phi = plane_angle(up)?;
        let base = match self.base {
            Some(b) if wrap(phi - b).abs() <= FLIP => b,
            _ => {
                if phi >= 0. {
                    90.
                } else {
                    -90.
                }
            }
        };
        self.base = Some(base);
        // Turning the phone clockwise by a turns up the other way relative
        // to the device: phi = base - a.
        Some(wrap(base - phi))
    }

    /// One reading `dt` seconds after the previous: returns the smoothed
    /// steering value in [-1, 1] (positive right). A missing or flat
    /// reading keeps the last value.
    pub fn update(&mut self, up: Option<[f32; 3]>, dt: f32) -> f32 {
        let Some(angle) = up.and_then(|u| self.angle(u)) else {
            return self.value;
        };
        self.angle = Some(angle);
        let neutral = *self.neutral.get_or_insert(angle);
        let max = self.settings.max_angle;
        let target = (wrap(angle - neutral) / max).clamp(-1., 1.);
        let tau = self.settings.smoothing;
        self.value = if tau > 0. && dt.is_finite() && dt >= 0. {
            self.value + (target - self.value) * (1. - (-dt / tau).exp())
        } else {
            target
        };
        self.value
    }

    /// Re-centre: the latest angle becomes the neutral.
    pub fn recentre(&mut self) {
        if let Some(angle) = self.angle {
            self.neutral = Some(angle);
            self.value = 0.;
        }
    }

    pub fn neutral(&self) -> Option<f32> {
        self.neutral
    }

    pub fn value(&self) -> f32 {
        self.value
    }

    /// Bi-axis raw value for the action layer, centred at 0.5.
    pub fn raw(&self) -> f32 {
        0.5 + 0.5 * self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Up vector seen by a phone in landscape with display rotation 90 (top
    /// edge left), turned clockwise by `steer` degrees and leaned back from
    /// upright by `lean` degrees.
    fn landscape_up(steer: f32, lean: f32) -> [f32; 3] {
        // Upright: up = +x (plane angle 90). Turning clockwise gives plane
        // angle 90 - steer; leaning back tips up into +z.
        let (phi, l) = ((90. - steer).to_radians(), lean.to_radians());
        [phi.sin() * l.cos(), phi.cos() * l.cos(), l.sin()]
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn rotation_vector_up_matches_android_rotation_matrix() {
        // Identity: phone flat, screen up.
        assert_eq!(up_from_rotation_vector(0., 0., 0., 1.), [0., 0., 1.]);
        // 90 degrees about world X: upright portrait, up = device +y.
        let h = std::f32::consts::FRAC_1_SQRT_2;
        let up = up_from_rotation_vector(h, 0., 0., h);
        assert!(close(up[0], 0.) && close(up[1], 1.) && close(up[2], 0.));
        // Then +90 degrees about the device's own z (counter-clockwise seen
        // from the front, top edge to the left): qx(90) * qz(90) =
        // (0.5, -0.5, 0.5, 0.5), landscape rotation 90 with up = device +x.
        let up = up_from_rotation_vector(0.5, -0.5, 0.5, 0.5);
        assert!(
            close(up[0], 1.) && close(up[1], 0.) && close(up[2], 0.),
            "{up:?}"
        );
    }

    #[test]
    fn accelerometer_up_is_normalised() {
        let up = up_from_acceleration([9.81, 0., 0.]).unwrap();
        assert_eq!(up, [1., 0., 0.]);
        assert_eq!(up_from_acceleration([0., 0., 0.]), None);
        assert_eq!(up_from_acceleration([f32::NAN, 0., 1.]), None);
    }

    #[test]
    fn angle_follows_steering_in_both_landscapes_and_ignores_lean() {
        let mut t = Tilt::new(Settings::default());
        for lean in [0., 30., 60.] {
            for steer in [-40., -10., 0., 15., 45.] {
                let a = t.angle(landscape_up(steer, lean)).unwrap();
                assert!(close(a, steer), "lean {lean} steer {steer}: {a}");
            }
        }
        // Rotation 270 (top edge right): the same motion seen mirrored.
        let mut t = Tilt::new(Settings::default());
        for steer in [-20f32, 0., 20.] {
            let [x, y, z] = landscape_up(steer, 20.);
            let a = t.angle([-x, -y, z]).unwrap();
            assert!(close(a, steer), "rot270 steer {steer}: {a}");
        }
    }

    #[test]
    fn flat_phone_keeps_the_last_value() {
        let mut t = Tilt::new(Settings {
            smoothing: 0.,
            ..Settings::default()
        });
        assert_eq!(t.update(Some(landscape_up(0., 0.)), 0.016), 0.);
        assert!(close(t.update(Some(landscape_up(15., 0.)), 0.016), 0.5));
        assert!(close(t.update(Some(landscape_up(-80., 85.)), 0.016), 0.5));
        assert!(close(t.update(None, 0.016), 0.5));
    }

    #[test]
    fn orientation_flips_only_past_135_degrees() {
        let mut t = Tilt::new(Settings::default());
        t.angle(landscape_up(0., 0.));
        // 120 degrees clockwise stays in the same landscape.
        assert!(close(t.angle(landscape_up(120., 0.)).unwrap(), 120.));
        // 170 degrees: the phone is now in the other landscape.
        let a = t.angle(landscape_up(170., 0.)).unwrap();
        assert!(close(a, -10.), "{a}");
    }

    #[test]
    fn neutral_max_angle_clamp_and_recentre() {
        let settings = Settings {
            max_angle: 30.,
            neutral: None,
            smoothing: 0.,
        };
        let mut t = Tilt::new(settings);
        // First reading becomes the neutral.
        assert_eq!(t.update(Some(landscape_up(10., 20.)), 0.016), 0.);
        assert_eq!(t.neutral().map(|n| n.round()), Some(10.));
        assert!(close(t.update(Some(landscape_up(25., 20.)), 0.016), 0.5));
        assert!(close(t.update(Some(landscape_up(-5., 20.)), 0.016), -0.5));
        assert_eq!(t.update(Some(landscape_up(70., 20.)), 0.016), 1.);
        assert_eq!(t.update(Some(landscape_up(-70., 20.)), 0.016), -1.);
        assert!(close(t.raw(), 0.));
        t.recentre();
        assert_eq!(t.value(), 0.);
        assert!(close(
            t.update(Some(landscape_up(-40., 20.)), 0.016),
            1. / 1.
        ));
        assert_eq!(t.raw(), 0.5 + 0.5 * t.value());
        // A fixed neutral is used from the start.
        let mut t = Tilt::new(Settings {
            neutral: Some(-6.),
            ..settings
        });
        assert!(close(t.update(Some(landscape_up(9., 0.)), 0.016), 0.5));
    }

    #[test]
    fn smoothing_is_a_one_pole_low_pass() {
        let mut t = Tilt::new(Settings {
            max_angle: 30.,
            neutral: Some(0.),
            smoothing: 0.05,
        });
        let v = t.update(Some(landscape_up(30., 0.)), 0.05);
        assert!(close(v, 1. - (-1f32).exp()), "{v}");
        for _ in 0..100 {
            t.update(Some(landscape_up(30., 0.)), 0.016);
        }
        assert!(close(t.value(), 1.));
        // A non-finite step jumps straight to the target.
        let v = t.update(Some(landscape_up(0., 0.)), f32::NAN);
        assert_eq!(v, 0.);
    }
}
