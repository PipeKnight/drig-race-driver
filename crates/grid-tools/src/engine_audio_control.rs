//! Scoped original-style engine RPM mapping, twelve pitches and seek lookups.
//! This is playback control, not a PCM mixer or a claim of audible equivalence.
//! Gear wobble, native backend calls, voice gains and spatial processing are absent.

pub const CONTROLLER_BYTES: usize = 0x340;
pub const CURVE_WORDS: usize = 4096;

pub fn read_curves(ini: &str) -> Result<[f32; CURVE_WORDS], &'static str> {
    let mut result = [0.; CURVE_WORDS];
    for (block, (section, key)) in [
        ("Idle", "RevsFromTime"),
        ("Idle", "TimeFromRevs"),
        ("Throttle", "RevsFromTime"),
        ("Throttle", "TimeFromRevs"),
    ]
    .into_iter()
    .enumerate()
    {
        result[block * 1024..(block + 1) * 1024]
            .copy_from_slice(&super::engine_audio::curve(ini, section, key)?);
    }
    Ok(result)
}

#[derive(Clone, Copy, Debug)]
pub struct VehicleEngine {
    pub speed: f32,
    pub maximum: f32,
    pub idle: f32,
    pub delta: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VoiceLookup {
    pub recorded_rpm_bits: u32,
    pub seek_microseconds: u32,
}

fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}
fn scalar(bytes: &[u8], at: usize) -> f32 {
    f32::from_bits(word(bytes, at))
}
fn put(bytes: &mut [u8], at: usize, value: f32) {
    bytes[at..at + 4].copy_from_slice(&value.to_bits().to_le_bytes());
}
fn index(value: f32) -> usize {
    ((value * 1024.) as i32).clamp(0, 1023) as usize
}

/// Curves are idle RevsFromTime/TimeFromRevs, then throttle equivalents.
/// All consumed fields are checked before mutation. Opaque bytes are retained.
pub fn update(
    controller: &mut [u8; CONTROLLER_BYTES],
    engine: VehicleEngine,
    curves: &[f32; CURVE_WORDS],
) -> Result<[VoiceLookup; 12], &'static str> {
    if !engine.speed.is_finite()
        || !engine.maximum.is_finite()
        || !engine.idle.is_finite()
        || !engine.delta.is_finite()
        || engine.maximum <= 0.
        || engine.speed < 0.
        || engine.speed > engine.maximum * 2.
        || engine.idle <= 0.
        || engine.idle >= engine.maximum
        || !(0.0..=1.).contains(&engine.delta)
    {
        return Err("engine audio vehicle domain");
    }
    for (i, &value) in curves.iter().enumerate() {
        if !value.is_finite() || !(0.0..=1.).contains(&value) || (i % 2048 < 1024 && value <= 0.) {
            return Err("engine audio curve domain");
        }
    }
    for at in [0, 0x38, 0xb0, 0xb4, 0xc8, 0x30c] {
        if !scalar(controller, at).is_finite() {
            return Err("engine audio controller scalar");
        }
    }
    if word(controller, 0x28) != 0
        || !(0.0..=2.).contains(&scalar(controller, 0x38))
        || !(0.0..=2.).contains(&scalar(controller, 0))
        || scalar(controller, 0xb0).abs() > 2.
        || scalar(controller, 0xb4).abs() > 1.
        || scalar(controller, 0x30c).abs() > 1.
    {
        return Err("engine audio no-wobble domain");
    }
    let physical_idle = engine.idle / engine.maximum;
    if (curves[1023] - physical_idle).abs() > 0.39 {
        return Err("engine audio logger-free remap domain");
    }
    // A voice's carried pitch is the scheduler's clamped [0.5, 2] value, or
    // the raw pitch a pair restart stores (engine_audio_restart: up to 1.5,
    // below 0.5 for the BMW 320si at idle, 0.4955; the voice setter 42C150
    // clamps only what the backend plays). It only advances the position
    // here before the pitch is recomputed.
    for slot in 0..12 {
        let position = scalar(controller, 0x230 + slot * 4);
        let length = word(controller, 0x2c0 + slot * 4);
        let rate = word(controller, 0x290 + slot * 4);
        let pitch = scalar(controller, 0x260 + slot * 4);
        if !position.is_finite()
            || position.abs() > 1_000_000_000.
            || !(1..=10_000_000).contains(&length)
            || !(8000..=192000).contains(&rate)
            || !pitch.is_finite()
            || pitch <= 0.
            || pitch > 2.
        {
            return Err("engine audio voice domain");
        }
    }
    let mut next = *controller;
    let weight = 1. - (1. - scalar(&next, 0xb0)) * 0.1;
    let ratio = (engine.speed / engine.maximum) * scalar(&next, 0);
    let old = scalar(&next, 0x38);
    let increment = ((weight * ratio - old) * 0.25).clamp(-0.2, 0.2);
    let smoothed = old + increment;
    put(&mut next, 0x38, smoothed);
    let rpm = smoothed.min(1.);
    put(&mut next, 0x14, rpm);
    put(&mut next, 0xbc, 0.);
    let recorded_idle = curves[1023];
    put(&mut next, 0xc0, recorded_idle);
    put(&mut next, 0xc4, physical_idle);
    let mapped = if physical_idle <= rpm {
        ((rpm - physical_idle) / (1. - physical_idle)) * (1. - recorded_idle) + recorded_idle
    } else {
        (recorded_idle / physical_idle) * rpm
    };
    let mapped = (mapped - scalar(&next, 0xb4)).clamp(0., 1.);
    put(&mut next, 0x48, mapped);
    put(&mut next, 0xc8, mapped);
    // Advance at the previous voice pitch, before the scheduler updates pitch.
    // Bank zero multiplies rate by f64 delta first; the other banks do pitch first.
    for category in 0..3 {
        for bank in 0..4 {
            let slot = bank * 3 + category;
            let rate = f64::from(word(&next, 0x290 + slot * 4));
            let pitch = f64::from(scalar(&next, 0x260 + slot * 4));
            let step = if bank == 0 {
                (rate * engine.delta) * pitch
            } else {
                (rate * pitch) * engine.delta
            };
            let position = step + f64::from(scalar(&next, 0x230 + slot * 4));
            put(&mut next, 0x230 + slot * 4, position as f32);
            let end = f64::from(word(&next, 0x2c0 + slot * 4)) * f64::from(0.9_f32);
            if position > end {
                next[0x14c + category] = 1;
            }
        }
    }
    // Preserve the scheduler's category order: the shared C8 clamp is carried.
    for category in 0..3 {
        let base = if category == 0 { 0 } else { 2048 };
        let minimum = curves[if category == 0 { 1023 } else { 2048 }] * 0.5;
        let requested = scalar(&next, 0xc8).clamp(minimum, 1.);
        put(&mut next, 0xc8, requested);
        for bank in 0..4 {
            let slot = bank * 3 + category;
            // 415500 uses cvttss2si on a FLOAT position, then x87 division.
            let position = scalar(&next, 0x230 + slot * 4) as i32;
            let length = word(&next, 0x2c0 + slot * 4);
            let time = ((f64::from(position) / f64::from(length)) as f32).min(1.);
            let pitch =
                (requested / curves[base + index(time)] + scalar(&next, 0x30c)).clamp(0.5, 2.);
            put(&mut next, 0x1a4 + slot * 4, pitch);
            put(&mut next, 0x260 + slot * 4, pitch);
        }
    }
    let mut queries = [VoiceLookup::default(); 12];
    for (slot, query) in queries.iter_mut().enumerate() {
        let base = if slot % 3 == 0 { 0 } else { 2048 };
        let length = word(&next, 0x2c0 + slot * 4);
        let position = scalar(&next, 0x230 + slot * 4) as i32;
        let time = ((f64::from(position) / f64::from(length)) as f32).min(1.);
        query.recorded_rpm_bits = curves[base + index(time)].to_bits();
        // Native x87 runs at 53-bit precision. Do not round intermediates to f32.
        let time = curves[base + 1024 + index(scalar(&next, 0xc8))];
        let rate = word(&next, 0x290 + slot * 4);
        query.seek_microseconds =
            ((f64::from(length) * f64::from(time) / f64::from(rate)) * 1_000_000.) as u32;
    }
    *controller = next;
    Ok(queries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn late_voice_failure_preserves_entire_carried_controller() {
        let mut controller = [0x6b; CONTROLLER_BYTES];
        for (at, value) in [
            (0, 1.),
            (0x28, 0.),
            (0x38, 0.),
            (0xb0, 1.),
            (0xb4, 0.),
            (0xc8, 0.),
            (0x30c, 0.),
        ] {
            put(&mut controller, at, value);
        }
        for slot in 0..12 {
            put(&mut controller, 0x230 + slot * 4, 0.);
            put(&mut controller, 0x260 + slot * 4, 1.);
            controller[0x290 + slot * 4..0x294 + slot * 4]
                .copy_from_slice(&32000_u32.to_le_bytes());
            controller[0x2c0 + slot * 4..0x2c4 + slot * 4]
                .copy_from_slice(&319294_u32.to_le_bytes());
        }
        let engine = VehicleEngine {
            speed: 400.,
            maximum: 800.,
            idle: 240.,
            delta: 0.016,
        };
        let curves = [0.5; CURVE_WORDS];
        update(&mut controller, engine, &curves).unwrap();
        put(&mut controller, 0x28c, f32::NAN);
        let before = controller;
        assert!(update(&mut controller, engine, &curves).is_err());
        assert_eq!(controller, before);
        put(&mut controller, 0x28c, 1.);
        assert!(update(&mut controller, engine, &curves).is_ok());
    }
}
