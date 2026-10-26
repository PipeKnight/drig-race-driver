//! The load inputs of 451770 (docs/AUDIO_RESEARCH.md §23) on the engine
//! controller's bytes: 4148A0 (the applied throttle, its change per frame and
//! the lift-off RPM burble) and 414970 (the rev limiter). 422A40
//! (`engine_audio_control::update`) plays the burble and the limiter's RPM
//! cut; the backfire 451260 (`mech_audio::Backfire`) reads the change and the
//! limiter flag.

fn f(c: &[u8], at: usize) -> f32 {
    f32::from_le_bytes(c[at..at + 4].try_into().unwrap())
}
fn put(c: &mut [u8], at: usize, value: f32) {
    c[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

/// 4148A0 with the applied throttle (vehicle +0xE1C, 9961D0(1)): +0xB0 the
/// throttle, +0x304 its change since the last frame (+0x308), +0x24 a
/// smoothed throttle. Lifting below 0.1 from a smoothed throttle above 0.55
/// raises the burble level +0x28 to +0x6C · clamp((smoothed − 0.55) / 0.35)
/// · 0.1 (+0x6C is 1.0 from the reset 422470) and restarts the smoothing.
pub fn throttle(c: &mut [u8], applied: f32, dt: f32) {
    put(c, 0xb0, applied);
    put(c, 0x304, applied - f(c, 0x308));
    put(c, 0x308, applied);
    let smoothed = (1. - dt) * f(c, 0x24) + applied * dt;
    put(c, 0x24, smoothed);
    if smoothed > 0.55 && applied < 0.1 {
        let t = ((smoothed - 0.55) * 2.857_143_2).clamp(0., 1.);
        let level = f(c, 0x6c) * t * 0.1;
        if f(c, 0x28) < level {
            put(c, 0x28, level);
        }
        put(c, 0x24, applied);
    }
}

/// 414970, the rev limiter: at +0x38 ≥ 0.99 in gears up to fourth, +0xD0 is
/// set and +0x64 toggles every second frame; while it is set the RPM cut +0xB4
/// (subtracted from the mapped RPM in 422A40) grows by 0.05 a frame, capped
/// at 0.63, and it is 0 otherwise. +0x324 times the exhaust-flame state
/// (vehicle +0x21DC, not ported).
pub fn limiter(c: &mut [u8], gear: u32, dt: f32) {
    put(c, 0x324, dt + f(c, 0x324));
    if f(c, 0x38) < 0.99 || gear >= 5 {
        c[0x64] = 0;
        c[0xd0] = 0;
        put(c, 0xb4, 0.);
        return;
    }
    c[0xd0] = 1;
    if f(c, 0xb4) > 0.63 {
        put(c, 0xb4, 0.63);
    }
    let count = u32::from_le_bytes(c[0xd4..0xd8].try_into().unwrap()) + 1;
    c[0xd4..0xd8].copy_from_slice(&count.to_le_bytes());
    if count > 1 {
        c[0xd4..0xd8].fill(0);
        c[0x64] = u8::from(c[0x64] == 0);
        if c[0x64] != 0 {
            put(c, 0x324, 0.);
        }
    }
    let cut = if c[0x64] != 0 { f(c, 0xb4) + 0.05 } else { 0. };
    put(c, 0xb4, cut);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lift_from_full_throttle_starts_the_burble_once() {
        let mut c = [0u8; 0x400];
        put(&mut c, 0x6c, 1.);
        for _ in 0..200 {
            throttle(&mut c, 1., 0.016);
        }
        assert!(f(&c, 0x24) > 0.95 && f(&c, 0x28) == 0. && f(&c, 0x304) == 0.);
        throttle(&mut c, 0., 0.016);
        assert_eq!(f(&c, 0x304), -1.);
        // smoothed 0.984·s + 0: t = (s − 0.55)/0.35 → 1, level 0.1.
        assert_eq!((f(&c, 0x28), f(&c, 0x24)), (0.1, 0.));
        throttle(&mut c, 0., 0.016);
        assert_eq!(f(&c, 0x304), 0.);
    }

    #[test]
    fn the_limiter_cuts_two_frames_in_four_below_fifth_gear() {
        let mut c = [0u8; 0x400];
        put(&mut c, 0x38, 0.995);
        let cuts: Vec<f32> = (0..8)
            .map(|_| {
                limiter(&mut c, 3, 0.016);
                f(&c, 0xb4)
            })
            .collect();
        assert_eq!(c[0xd0], 1);
        assert_eq!(cuts, [0., 0.05, 0.1, 0., 0., 0.05, 0.1, 0.]);
        limiter(&mut c, 5, 0.016);
        assert_eq!((c[0xd0], f(&c, 0xb4)), (0, 0.));
    }
}
