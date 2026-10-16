//! Player auto-reset arithmetic of the original ResetLinesManager
//! (docs/RESET_AND_RESPAWN_RESEARCH.md 3): the nearly-still-and-on-its-side
//! detector 0x60D590, the placement height limit of 0x9145D0 and the speed-0
//! tail of the grid-spawn setter 0x9EF040 (wheel spin and gear). The caller
//! owns the ground placement (`spawn::align_below`) and the pose setter.
use crate::sse_approx::rsqrt;

/// 0xDB29F0: speed plus spin at or below this counts as still.
pub const STILL: f32 = 0.8;
/// 0xDB29EC: an up-axis Y at or below this counts as on the side or roof.
pub const TILTED: f32 = 0.17;
/// 0xDB29E8: seconds the car must stay still and tilted.
pub const DELAY: f32 = 2.0;
/// Upper timer bound of the firing branch (research 3.1).
const LATCH: f32 = 3.0;
/// 0xCAFBE8 / 0xCAFBE4: half span and span of the limit's vertical cast.
pub const LIMIT_REACH: f32 = 10000.;
const LIMIT_SPAN: f32 = 20000.;

/// Vector length as the game's inline SSE helper computes it (seen in
/// 0x5EC0E0): z² + (y² + x²), one RSQRTPS seed refined once, zero kept. The
/// seed comes from the measured table, never the host instruction (F320).
pub fn length(v: [f32; 3]) -> f32 {
    let square = v[2] * v[2] + (v[1] * v[1] + v[0] * v[0]);
    if square == 0. {
        return 0.;
    }
    let seed = rsqrt(square);
    let root = seed * square;
    (1. - root * seed) * root * 0.5 + root
}

/// Auto-reset timer (manager +0x214).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Detector {
    pub timer: f32,
}

impl Detector {
    /// One update with the body's linear and angular velocity, the pose's
    /// up-axis Y (row 1 . y) and the frame time. True once per still-and-
    /// tilted stretch, after [`DELAY`] seconds: the reset request.
    pub fn update(&mut self, linear: [f32; 3], angular: [f32; 3], up_y: f32, delta: f32) -> bool {
        let speed = length(linear) + length(angular);
        // A non-finite state is never still; it restarts the stretch.
        let still = speed <= STILL && up_y <= TILTED;
        if !still {
            self.timer = 0.;
        } else if self.timer < DELAY {
            self.timer += delta;
        } else if self.timer < LATCH {
            self.timer += DELAY;
            return true;
        }
        false
    }
}

/// Placement limit of 0x9145D0 for a line point at height `y`: max(y + 1,
/// ground + 1), where the ground is the hit of a vertical cast from
/// y + 1 - 10000 up to y + 1 + 10000 at `fraction` (None: no hit).
pub fn placement_limit(y: f32, fraction: Option<f32>) -> f32 {
    let top = y + 1.;
    let start = top - LIMIT_REACH;
    match fraction {
        Some(f) if top < f * LIMIT_SPAN + start + 1. => f * LIMIT_SPAN + start + 1.,
        _ => top,
    }
}

/// Start and end Y of the limit cast for a line point at height `y`.
pub fn limit_cast(y: f32) -> [f32; 2] {
    let top = y + 1.;
    [top - LIMIT_REACH, top + LIMIT_REACH]
}

fn get(words: &[u32], offset: usize) -> f32 {
    f32::from_bits(words[offset / 4])
}
fn set(words: &mut [u32], offset: usize, value: f32) {
    words[offset / 4] = value.to_bits();
}

/// 0xD15028 / 0xD1502C: torque-curve index scale and its top speed.
const CURVE_SCALE: f32 = f32::from_bits(0x3c97_9102);
const CURVE_TOP: f32 = f32::from_bits(0x44d1_7084);

/// Speed-0 tail of 0x9EF040 on the car words: every wheel's spin
/// (+0x104) = speed / radius (+0xE0), then the gear (+0x196C) with the most
/// torque at the resulting engine speed and the engine speed (+0x1894).
/// [I] The engine speed source *(+0x18D8)+0x104 is a wheel spin, so it is 0
/// once the spins are cleared.
pub fn stop_drivetrain(words: &mut [u32]) {
    const SPEED: f32 = 0.;
    let wheels = (words[0x1620 / 4] as usize).min(4);
    for i in 0..wheels {
        let w = 0x1990 + i * 0x150;
        set(words, w + 0x104, SPEED / get(words, w + 0xe0));
    }
    let spin = 0f32;
    let mut best = 0f32;
    let mut gear = 0;
    for i in 0..10 {
        let ratio = get(words, 0x18f4 + i * 4);
        if ratio == 0. {
            continue;
        }
        let mut speed = spin / ratio;
        let torque = if speed > CURVE_TOP {
            0.
        } else {
            // Negative or NaN reads node 0 (the original's else branch).
            if speed < 0. || speed.is_nan() {
                speed = 0.;
            }
            let index = (speed * CURVE_SCALE) as i32;
            let fraction = speed * CURVE_SCALE - index as f32;
            let index = index.clamp(0, 0x1e) as usize;
            let low = get(words, 0x17ac + index * 4);
            if low == 0. {
                0.
            } else {
                (get(words, 0x17b0 + index * 4) - low) * fraction + low
            }
        };
        if best < torque {
            gear = i;
            best = torque;
        }
    }
    words[0x196c / 4] = gear as u32;
    let ratio = get(words, 0x18f4 + gear * 4);
    let engine = if ratio == 0. {
        get(words, 0x17a8)
    } else {
        spin / ratio
    };
    set(words, 0x1894, engine);
    if gear == 0 {
        words[0x196c / 4] = 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_match_the_executable() {
        assert_eq!(STILL.to_bits(), 0x3f4c_cccd);
        assert_eq!(TILTED.to_bits(), 0x3e2e_147b);
        assert_eq!(DELAY.to_bits(), 0x4000_0000);
        assert_eq!(CURVE_SCALE, 0.018_501_762);
        assert_eq!(CURVE_TOP, 1_675.516_1);
    }

    #[test]
    fn length_follows_the_refined_seed() {
        assert_eq!(length([0.; 3]), 0.);
        for v in [[3f32, 4., 0.], [0.1, -0.2, 0.3], [40., 1., -7.]] {
            let exact = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            assert!((length(v) - exact).abs() <= exact * 1e-5, "{v:?}");
        }
    }

    #[test]
    fn detector_fires_once_after_two_still_tilted_seconds() {
        let mut d = Detector::default();
        let mut fired = Vec::new();
        for tick in 0..400 {
            if d.update([0.1, 0., 0.], [0., 0.2, 0.], -1., 0.016) {
                fired.push(tick);
            }
        }
        // 2.0 / 0.016 = 125 accumulating updates (f32 sum reaches 2.0 on the
        // 126th), the next one fires; then the timer stays latched above 3.
        assert_eq!(fired.len(), 1);
        assert!((125..=127).contains(&fired[0]), "{fired:?}");
        // Upright or moving restarts the stretch.
        assert!(!d.update([0.; 3], [0.; 3], 0.9, 0.016));
        assert_eq!(d.timer, 0.);
        d.timer = 1.9;
        assert!(!d.update([0.9, 0., 0.], [0.; 3], -1., 0.016));
        assert_eq!(d.timer, 0.);
        assert!(!d.update([f32::NAN; 3], [0.; 3], -1., 0.016));
        assert_eq!(d.timer, 0.);
        // Exactly at the thresholds still counts.
        d.timer = 0.;
        assert!(!d.update([0.8, 0., 0.], [0.; 3], 0.17, 0.016));
        assert_eq!(d.timer, 0.016);
    }

    #[test]
    fn limit_is_the_higher_of_point_and_ground() {
        assert_eq!(placement_limit(5., None), 6.);
        assert_eq!(limit_cast(5.), [-9994., 10006.]);
        // Ground at 4 (below the point): the point wins.
        assert_eq!(placement_limit(5., Some(0.4999)), 6.);
        // Ground at 10: limit 11.
        let f = (10. - limit_cast(5.)[0]) / LIMIT_SPAN;
        assert!((placement_limit(5., Some(f)) - 11.).abs() < 1e-3);
    }

    #[test]
    fn drivetrain_stops_in_the_first_gear_with_torque() {
        let mut words = vec![0u32; 0x2000 / 4];
        words[0x1620 / 4] = 4;
        for i in 0..4 {
            let w = 0x1990 + i * 0x150;
            set(&mut words, w + 0xe0, 0.3);
            set(&mut words, w + 0x104, 55.);
        }
        set(&mut words, 0x17a8, 230.);
        set(&mut words, 0x17ac, 100.);
        for (i, r) in [0., 3.5, 2.1, 1.4, 1.0, 0.8, 0., 0., 0., -3.2]
            .into_iter()
            .enumerate()
        {
            set(&mut words, 0x18f4 + i * 4, r);
        }
        words[0x196c / 4] = 4;
        set(&mut words, 0x1894, 600.);
        stop_drivetrain(&mut words);
        for i in 0..4 {
            assert_eq!(get(&words, 0x1990 + i * 0x150 + 0x104), 0.);
        }
        assert_eq!(words[0x196c / 4], 1);
        assert_eq!(get(&words, 0x1894), 0.);
        // No torque at rest: gear 0 is chosen, then gear 1 forced, and the
        // neutral ratio 0 gives the idle speed.
        set(&mut words, 0x17ac, 0.);
        stop_drivetrain(&mut words);
        assert_eq!(words[0x196c / 4], 1);
        assert_eq!(get(&words, 0x1894), 230.);
    }
}
