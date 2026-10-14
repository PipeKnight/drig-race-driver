//! Whole 4371C0 scheduling/publication while all voices exist and restarts wait.
//! The cooldown boundary deliberately excludes native backend play/seek calls.
use crate::{
    engine_audio_control::{CONTROLLER_BYTES, CURVE_WORDS},
    engine_audio_publication::{self, Environment, VOICE_BYTES},
};

fn f(bytes: &[u8], at: usize) -> f32 {
    f32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}
fn u(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}
fn put(bytes: &mut [u8], at: usize, value: f32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

/// Transactional scheduler for the existing-voices, no-restart service boundary.
/// Positive cooldowns must remain nonnegative after this frame's decrement.
pub fn publish_waiting(
    controller: &mut [u8; CONTROLLER_BYTES],
    voices: &mut [[u8; VOICE_BYTES]; 12],
    environment: Environment,
    curves: &[f32; CURVE_WORDS],
    delta: f64,
) -> Result<(), &'static str> {
    if !delta.is_finite() || !(0.0..=1.).contains(&delta) {
        return Err("engine scheduler delta domain");
    }
    let delta = delta as f32;
    if curves.iter().enumerate().any(|(i, value)| {
        !value.is_finite() || !(0.0..=1.).contains(value) || (i % 2048 < 1024 && *value <= 0.)
    }) || !f(controller, 0xc8).is_finite()
        || !(0.0..=1.).contains(&f(controller, 0xc8))
        || !f(controller, 0x30c).is_finite()
        || f(controller, 0x30c).abs() > 1.
    {
        return Err("engine scheduler curve/pitch domain");
    }
    for category in 0..3 {
        let cooldown = f(controller, 0x134 + category * 4);
        let fade = f(controller, 0x140 + category * 4);
        if !cooldown.is_finite()
            || !(delta..=60.).contains(&cooldown)
            || !fade.is_finite()
            || fade.abs() > 1.
            || controller[0x14c + category] > 1
            || controller[0x14f + category] > 1
        {
            return Err("engine scheduler waiting domain");
        }
    }
    for slot in 0..12 {
        let position = f(controller, 0x230 + slot * 4);
        if !position.is_finite()
            || position.abs() > 1_000_000_000.
            || !(1..=10_000_000).contains(&u(controller, 0x2c0 + slot * 4))
        {
            return Err("engine scheduler position domain");
        }
    }
    let mut next = *controller;
    let mut next_voices = *voices;
    for category in 0..3 {
        let outgoing = if next[0x14f + category] == 0 {
            2_u32
        } else {
            0
        };
        next[0x1d4 + category * 4..0x1d8 + category * 4].copy_from_slice(&outgoing.to_le_bytes());
        next[0x1e0 + category * 4..0x1e4 + category * 4]
            .copy_from_slice(&(2 - outgoing).to_le_bytes());
    }
    for category in 0..3 {
        let base = if category == 0 { 0 } else { 2048 };
        let minimum = curves[if category == 0 { 1023 } else { 2048 }] * 0.5;
        let requested = f(&next, 0xc8).clamp(minimum, 1.);
        put(&mut next, 0xc8, requested);
        for bank in 0..4 {
            let slot = bank * 3 + category;
            let position = f(&next, 0x230 + slot * 4) as i32;
            let time =
                ((f64::from(position) / f64::from(u(&next, 0x2c0 + slot * 4))) as f32).min(1.);
            let index = ((time * 1024.) as i32).clamp(0, 1023) as usize;
            let pitch = (requested / curves[base + index] + f(&next, 0x30c)).clamp(0.5, 2.);
            put(&mut next, 0x1a4 + slot * 4, pitch);
            put(&mut next, 0x260 + slot * 4, pitch);
        }
    }
    for category in 0..3 {
        let outgoing = u(&next, 0x1d4 + category * 4) as usize;
        let pitch = f(&next, 0x1a4 + (outgoing * 3 + category) * 4);
        if pitch <= 0.95 || pitch >= 1.05 {
            next[0x14c + category] = 1;
        }
        let fade = f(&next, 0x140 + category * 4);
        let step = delta * 16.;
        let fade = if next[0x14f + category] == 0 {
            fade + step
        } else {
            fade - step
        };
        put(&mut next, 0x140 + category * 4, fade.clamp(-1., 1.));
        let cooldown = f(&next, 0x134 + category * 4) - delta;
        put(&mut next, 0x134 + category * 4, cooldown);
    }
    engine_audio_publication::publish(&mut next, &mut next_voices, environment)?;
    *controller = next;
    *voices = next_voices;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn backend_boundary_and_late_publication_failure_are_atomic() {
        let mut controller = [0; CONTROLLER_BYTES];
        for at in [0x134, 0x138, 0x13c, 0x2f8] {
            put(&mut controller, at, 1.);
        }
        for slot in 0..12 {
            controller[0x2c0 + slot * 4..0x2c4 + slot * 4].copy_from_slice(&100_u32.to_le_bytes());
        }
        let mut voices = [[0x6b; VOICE_BYTES]; 12];
        let env = Environment {
            motion_210: [0.; 4],
            motion_220: [0.; 4],
            ranges: [[1., 50.]; 2],
            trims: [0.; 2],
            treble: 0.,
            player: true,
        };
        publish_waiting(
            &mut controller,
            &mut voices,
            env,
            &[0.5; CURVE_WORDS],
            0.016,
        )
        .unwrap();
        let before = (controller, voices);
        let bad = Environment {
            treble: f32::NAN,
            ..env
        };
        assert!(
            publish_waiting(
                &mut controller,
                &mut voices,
                bad,
                &[0.5; CURVE_WORDS],
                0.016
            )
            .is_err()
        );
        assert_eq!((controller, voices), before);
        put(&mut controller, 0x13c, 0.);
        let before = (controller, voices);
        assert!(
            publish_waiting(
                &mut controller,
                &mut voices,
                env,
                &[0.5; CURVE_WORDS],
                0.016
            )
            .is_err()
        );
        assert_eq!((controller, voices), before);
    }
}
