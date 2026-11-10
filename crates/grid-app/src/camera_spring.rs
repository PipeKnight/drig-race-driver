//! The front-end camera stick spring (docs/FRONT_END_FLOW.md §2, "Stick
//! spring"): the right stick turns a front-end camera about its target
//! within a small range, and a spring takes it back when the stick is
//! released. It is tuned by a `<camera_tweaks><NAME><spring_tweaks>` block of
//! `frontend/persistentdatarender.xml` (`default`, `pause`, `loading`,
//! `in_game_cameras`).
//!
//! [D, static analysis of the unpacked exe]:
//! - Input: 0x47F660 reads the actions `Fe View Tweak Right − Left` (0x2C −
//!   0x2B) and `Up − Down` (0x2D − 0x2E) each update and sends them as a
//!   message (type 0xDAACC4). The camera's spring component (vtable 0xCB9B9C)
//!   clamps each to ±`spring_limit` and scales it by −0.5 (0x47EAE0). The
//!   result is the spring's user target, and an axis pulls towards it only
//!   while its magnitude is above `spring_threshold` (0x47B7E0).
//! - Spring (0x47B970), in sub-steps of at most 0.1 s, semi-implicit Euler
//!   per axis: a = (user_tension (target − x) − user_damping v) w +
//!   centering_tension (0 − x) − centering_damping v; v += a h; x += v h.
//! - The component's update (0x47E890) turns the camera with 0x47C7F0 by
//!   `final_scale_y` x[1] about its side axis and `final_scale_x` x[0] about
//!   its up axis, times the dampen weight clamp(2f² − f³), f = 1 − share. The
//!   share moves at ±dt/`dampen_to_zero` in a direction that is 0 unless
//!   something requests a fade; no caller of that request is known, so the
//!   port keeps the weight at 1 [I].
//!
//! The z and w lanes of the original's four-lane spring get no input and no
//! weight, so they stay at 0 and are left out.
use crate::camera_noise::{attribute, section};

/// The `spring_tweaks` of one `camera_tweaks` block.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tweaks {
    pub centering_tension: f32,
    pub centering_damping: f32,
    pub user_tension: f32,
    pub user_damping: f32,
    /// `spring_threshold_x`, `_y`: the input magnitude above which the user
    /// target pulls.
    pub threshold: [f32; 2],
    /// `spring_limit_x`, `_y`: the input clamp.
    pub limit: [f32; 2],
    /// `final_scale_x`, `_y`: radians per unit of spring position.
    pub final_scale: [f32; 2],
    /// `dampen_to_zero duration` in seconds.
    pub dampen: f32,
}

/// The constructor's defaults (0x47B250, 0x47E770) [D].
pub const DEFAULT: Tweaks = Tweaks {
    centering_tension: 11.,
    centering_damping: 12.,
    user_tension: 15.,
    user_damping: 11.,
    threshold: [10.1; 2],
    limit: [10.1; 2],
    final_scale: [1.; 2],
    dampen: 1.,
};

impl Tweaks {
    /// The block `<NAME>` of `<camera_tweaks>`; a missing value keeps the
    /// constructor's default, a missing block gives `None`.
    pub fn read(xml: &str, name: &str) -> Option<Tweaks> {
        let spring = section(xml, name, "spring_tweaks")?;
        let scale = |tag: &str, default: f32| attribute(spring, tag, "scale", default);
        let normal = |tag: &str, default: f32| attribute(spring, tag, "normal", default);
        Some(Tweaks {
            centering_tension: scale("centering_tension", DEFAULT.centering_tension),
            centering_damping: scale("centering_damping", DEFAULT.centering_damping),
            user_tension: scale("user_tension", DEFAULT.user_tension),
            user_damping: scale("user_damping", DEFAULT.user_damping),
            threshold: [
                normal("spring_threshold_x", DEFAULT.threshold[0]),
                normal("spring_threshold_y", DEFAULT.threshold[1]),
            ],
            limit: [
                normal("spring_limit_x", DEFAULT.limit[0]),
                normal("spring_limit_y", DEFAULT.limit[1]),
            ],
            final_scale: [
                scale("final_scale_x", DEFAULT.final_scale[0]),
                scale("final_scale_y", DEFAULT.final_scale[1]),
            ],
            dampen: attribute(spring, "dampen_to_zero", "duration", DEFAULT.dampen),
        })
    }
}

/// A camera's spring state.
#[derive(Clone, Debug)]
pub struct CameraSpring {
    tweaks: Tweaks,
    /// Position and velocity per axis.
    x: [f32; 2],
    v: [f32; 2],
    /// The user target and whether it pulls (1) or not (0).
    target: [f32; 2],
    weight: [f32; 2],
}

impl CameraSpring {
    pub fn new(tweaks: Tweaks) -> CameraSpring {
        CameraSpring { tweaks, x: [0.; 2], v: [0.; 2], target: [0.; 2], weight: [0.; 2] }
    }

    /// The stick message: `[Right − Left, Up − Down]` of the `Fe View Tweak`
    /// actions (0x47EAE0, 0x47B7E0).
    pub fn input(&mut self, stick: [f32; 2]) {
        for (i, &raw) in stick.iter().enumerate() {
            let limit = self.tweaks.limit[i];
            let mut s = raw;
            if limit < s {
                s = limit;
            }
            if s < -limit {
                s = -limit;
            }
            let target = s * -0.5;
            self.target[i] = target;
            let threshold = self.tweaks.threshold[i];
            self.weight[i] = if threshold < target || target < -threshold { 1. } else { 0. };
        }
    }

    /// The (pitch, yaw) angles in radians after `dt` more seconds (0x47E890
    /// with the dampen share at 0).
    pub fn angles(&mut self, dt: f32) -> (f32, f32) {
        let t = self.tweaks;
        let mut left = dt;
        while left > 0. {
            let h = left.min(0.1);
            for i in 0..2 {
                let user = (t.user_tension * (self.target[i] - self.x[i]) - t.user_damping * self.v[i]) * self.weight[i];
                let centre = t.centering_tension * (0. - self.x[i]) - t.centering_damping * self.v[i];
                self.v[i] += (user + centre) * h;
                self.x[i] += h * self.v[i];
            }
            left -= h;
        }
        (t.final_scale[1] * self.x[1], t.final_scale[0] * self.x[0])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOADING: &str = r#"<camera_tweaks><loading>
      <spring_tweaks>
        <centering_tension  scale="1.0" />
        <centering_damping  scale="2.0" />
        <user_tension   scale="5.0" />
        <user_damping   scale="1.0" />
        <spring_threshold_x normal="0.1" />
        <spring_threshold_y normal="0.1" />
        <spring_limit_x   normal="0.35" />
        <spring_limit_y   normal="0.35" />
        <dampen_to_zero   duration="0.5" />
        <final_scale_x    scale="0.25" />
        <final_scale_y    scale="0.25" />
      </spring_tweaks>
      <noise_tweaks><rotation_rate scale="0.066" /><dampen_to_zero duration="0.2" /></noise_tweaks>
    </loading><pause><spring_tweaks><user_tension scale="7" /></spring_tweaks></pause></camera_tweaks>"#;

    #[test]
    fn tweaks_read_by_block_with_constructor_defaults() {
        let t = Tweaks::read(LOADING, "loading").unwrap();
        assert_eq!((t.centering_tension, t.centering_damping, t.user_tension, t.user_damping), (1., 2., 5., 1.));
        assert_eq!((t.threshold, t.limit, t.final_scale, t.dampen), ([0.1; 2], [0.35; 2], [0.25; 2], 0.5));
        let p = Tweaks::read(LOADING, "pause").unwrap();
        assert_eq!(p, Tweaks { user_tension: 7., ..DEFAULT });
        assert!(Tweaks::read(LOADING, "default").is_none());
        // The noise reader shares the section parser and still finds its own.
        assert_eq!(crate::camera_noise::Tweaks::read(LOADING, "loading").unwrap().rate, 0.066);
    }

    #[test]
    fn input_is_clamped_halved_negated_and_gated_by_the_threshold() {
        let mut s = CameraSpring::new(Tweaks::read(LOADING, "loading").unwrap());
        s.input([1., -0.1]);
        assert_eq!((s.target, s.weight), ([-0.175, 0.05], [1., 0.]));
        s.input([-0.3, 0.]);
        assert_eq!((s.target, s.weight), ([0.15, -0.], [1., 0.]));
    }

    #[test]
    fn held_stick_settles_and_released_stick_returns() {
        let t = Tweaks::read(LOADING, "loading").unwrap();
        let mut s = CameraSpring::new(t);
        assert_eq!(s.angles(1. / 60.), (0., 0.));
        s.input([1., 1.]);
        let mut a = (0., 0.);
        for _ in 0..60 * 20 {
            a = s.angles(1. / 60.);
        }
        // Equilibrium: x = ku·t / (ku + kc) = 5 · −0.175 / 6, times 0.25.
        let settled = 0.25 * 5. * -0.175 / 6.;
        assert!((a.0 - settled).abs() < 1e-4 && (a.1 - settled).abs() < 1e-4, "{a:?}");
        s.input([0., 0.]);
        for _ in 0..60 * 20 {
            a = s.angles(1. / 60.);
        }
        assert!(a.0.abs() < 1e-3 && a.1.abs() < 1e-3, "{a:?}");
    }

    #[test]
    fn a_long_frame_is_split_into_tenth_second_steps() {
        let t = Tweaks::read(LOADING, "loading").unwrap();
        let (mut one, mut many) = (CameraSpring::new(t), CameraSpring::new(t));
        one.input([1., 0.]);
        many.input([1., 0.]);
        let long = one.angles(0.25);
        many.angles(0.1);
        many.angles(0.1);
        let short = many.angles(0.05);
        assert!((long.1 - short.1).abs() < 1e-6, "{long:?} {short:?}");
    }
}
