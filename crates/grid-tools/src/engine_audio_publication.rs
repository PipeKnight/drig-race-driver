//! Scoped whole-style 42DE60 publication into twelve owned voice property records.
//! Camera mode 3, all voices present, supplied motion/ranges/trims/Treble.
//! No device backend, factory/camera selection, filtering or PCM rendering.
use super::{engine_audio_control::CONTROLLER_BYTES, engine_audio_gain::crossfade_db};

pub const VOICE_BYTES: usize = 0x80;
#[derive(Clone, Copy)]
pub struct Environment {
    pub motion_210: [f32; 4],
    pub motion_220: [f32; 4],
    pub ranges: [[f32; 2]; 2],
    pub trims: [f32; 2],
    pub treble: f32,
    pub player: bool,
}
fn scalar(bytes: &[u8], at: usize) -> f32 {
    f32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}
fn put(bytes: &mut [u8], at: usize, value: f32) {
    bytes[at..at + 4].copy_from_slice(&value.to_bits().to_le_bytes());
}
fn root(value: f32) -> Result<f32, &'static str> {
    if value == 0. {
        return Ok(0.);
    }
    // Measured RSQRTPS of the validation CPU (grid_world::sse_approx) on every CPU (F320);
    // feature hw-sse executes the instruction instead, for hardware comparison only.
    #[cfg(not(all(feature = "hw-sse", any(target_arch = "x86", target_arch = "x86_64"))))]
    let seed = grid_world::sse_approx::rsqrt(value);
    #[cfg(all(feature = "hw-sse", any(target_arch = "x86", target_arch = "x86_64")))]
    let seed = {
        #[cfg(target_arch = "x86")]
        use std::arch::x86::{_mm_cvtss_f32, _mm_rsqrt_ps, _mm_set1_ps};
        #[cfg(target_arch = "x86_64")]
        use std::arch::x86_64::{_mm_cvtss_f32, _mm_rsqrt_ps, _mm_set1_ps};
        if !std::is_x86_feature_detected!("sse") {
            return Err("engine gain requires SSE");
        }
        // Checked SSE; this intrinsic accesses register values only.
        unsafe { _mm_cvtss_f32(_mm_rsqrt_ps(_mm_set1_ps(value))) }
    };
    let product = seed * value;
    Ok((1. - product * seed) * (product * 0.5) + product)
}

pub fn publish(
    controller: &mut [u8; CONTROLLER_BYTES],
    voices: &mut [[u8; VOICE_BYTES]; 12],
    environment: Environment,
) -> Result<(), &'static str> {
    for at in [0xcc, 0x140, 0x144, 0x148, 0x1a0, 0x2f8, 0x330, 0xc8] {
        if !scalar(controller, at).is_finite() {
            return Err("engine gain controller scalar");
        }
    }
    if (0..3).any(|i| scalar(controller, 0x140 + i * 4).abs() > 1.)
        || !(0.0..=1.).contains(&scalar(controller, 0xcc))
        // 451580 stores the clamped offset minus 0.5 during an upshift.
        || scalar(controller, 0x1a0).abs() > 1.5
        || scalar(controller, 0x330).abs() > 48.
        || !(0.5..=2.).contains(&scalar(controller, 0x2f8))
        || !(0.0..=1.).contains(&scalar(controller, 0xc8))
        || controller[0xd8] > 1
    {
        return Err("engine gain controller domain");
    }
    for slot in 0..12 {
        let value = scalar(controller, 0x1a4 + slot * 4);
        if !value.is_finite() || !(0.5..=2.).contains(&value) {
            return Err("engine gain pitch domain");
        }
    }
    for value in environment
        .motion_210
        .into_iter()
        .chain(environment.motion_220)
    {
        if !value.is_finite() || value.abs() > 1000. {
            return Err("engine gain motion domain");
        }
    }
    for emitter in 0..2 {
        for axis in 0..3 {
            let value = scalar(controller, 0xe0 + emitter * 16 + axis * 4);
            if !value.is_finite() || value.abs() > 1_000_000. {
                return Err("engine gain position domain");
            }
        }
    }
    for range in environment.ranges {
        if range
            .iter()
            .any(|v| !v.is_finite() || *v <= 0. || *v > 100_000.)
            || range[0] > range[1]
        {
            return Err("engine gain range domain");
        }
    }
    if environment
        .trims
        .iter()
        .any(|v| !v.is_finite() || v.abs() > 48.)
        || !environment.treble.is_finite()
        || environment.treble.abs() > 96.
    {
        return Err("engine gain trim/Treble domain");
    }
    let fade: [[f32; 2]; 3] = std::array::from_fn(|category| {
        crossfade_db((scalar(controller, 0x140 + category * 4) + 1.) * 0.5).unwrap()
    });
    let load_phase =
        (((scalar(controller, 0xcc) * 2. - 1.) + scalar(controller, 0x1a0)) + 1.) * 0.5;
    let load = crossfade_db(load_phase.clamp(0., 1.))?;
    let [x, _, z, w] = environment.motion_210;
    let squared = (x * x + 0.) + (z * z + w * w);
    if squared > 0. && squared < f32::MIN_POSITIVE {
        return Err("engine gain subnormal motion square");
    }
    let mut adjustment = root(squared)? * 0.1;
    if adjustment > 0.1 {
        adjustment = 0.1 - (adjustment - 0.1);
    }
    adjustment = adjustment.max(-0.1);
    let mut layers = [[0.; 2]; 3];
    for bank in 0..2 {
        layers[0][bank] = (fade[0][bank] + (load[0] - 6.)) - scalar(controller, 0x330);
        layers[1][bank] = fade[1][bank] + load[1];
        layers[2][bank] = (fade[2][bank] + load[1]) - 48.;
    }
    let mut next_controller = *controller;
    if controller[0xd8] != 0 {
        layers = [[-48.; 2]; 3];
        put(&mut next_controller, 8, 0.);
    }
    let attenuation =
        ((1. - scalar(controller, 0xc8)) * if environment.player { 18. } else { 36. }).min(7.);
    let trims = environment.trims.map(|trim| trim - attenuation);
    let mut next = *voices;
    for (slot, voice) in next.iter_mut().enumerate() {
        let bank = slot / 3;
        let category = slot % 3;
        let emitter = bank % 2;
        let pitch = (scalar(controller, 0x1a4 + slot * 4) * scalar(controller, 0x2f8) + adjustment)
            .clamp(0.5, 2.);
        put(voice, 0x18, pitch);
        let volume = (trims[emitter] + layers[category][bank / 2]).clamp(-48., 0.);
        put(voice, 0x14, volume);
        for axis in 0..3 {
            put(
                voice,
                0x1c + axis * 4,
                scalar(controller, 0xe0 + emitter * 16 + axis * 4),
            );
            put(voice, 0x2c + axis * 4, environment.motion_220[axis]);
        }
        put(voice, 0x28, 1.);
        put(voice, 0x38, 1.);
        put(voice, 0x50, environment.ranges[emitter][0]);
        put(voice, 0x54, environment.ranges[emitter][1]);
        voice[0x78] = 1;
        put(voice, 0x58, 1.);
        put(voice, 0x5c, environment.treble);
    }
    *controller = next_controller;
    *voices = next;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_last_pitch_preserves_controller_and_all_voice_records() {
        let mut controller = [0x6b; CONTROLLER_BYTES];
        controller[0xd8] = 0;
        for (at, value) in [
            (0xcc, 0.5),
            (0x140, 0.),
            (0x144, 0.),
            (0x148, 0.),
            (0x1a0, 0.),
            (0x2f8, 1.),
            (0x330, 0.),
            (0xc8, 0.8),
        ] {
            put(&mut controller, at, value);
        }
        for slot in 0..12 {
            put(&mut controller, 0x1a4 + slot * 4, 1.);
        }
        for emitter in 0..2 {
            for axis in 0..3 {
                put(&mut controller, 0xe0 + emitter * 16 + axis * 4, 0.);
            }
        }
        let env = Environment {
            motion_210: [0.; 4],
            motion_220: [0.; 4],
            ranges: [[1., 50.]; 2],
            trims: [0.; 2],
            treble: 0.,
            player: true,
        };
        let mut voices = [[0x6b; VOICE_BYTES]; 12];
        publish(&mut controller, &mut voices, env).unwrap();
        put(&mut controller, 0x1d0, f32::NAN);
        let before_controller = controller;
        let before_voices = voices;
        assert!(publish(&mut controller, &mut voices, env).is_err());
        assert_eq!(controller, before_controller);
        assert_eq!(voices, before_voices);
        put(&mut controller, 0x1d0, 1.);
        assert!(publish(&mut controller, &mut voices, env).is_ok());
    }
}
