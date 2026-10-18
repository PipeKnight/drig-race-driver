//! Graded Traction Control and Braking Assist (AUTHORED, docs/DRIVER_AIDS.md).
//!
//! The original's aid bytes (vehicle +1559 braking, +155A traction) are 0 for
//! the aid on and 1 for off; every read site only tests them for non-zero
//! (docs/DRIVING_ASSISTS_RESEARCH.md 3). Those two values keep the original
//! code paths exactly. The port adds partial aid levels as byte values with
//! the top bit set: `0x80 | penalty` where penalty is the percentage of the
//! "off" behaviour that applies (75 for a 25 % aid, 50 for a 50 % aid). The
//! original never writes such values.

/// The original's "off" pedal scale, 1/0.9 (0x3F8E38E4).
pub const OFF_SCALE: u32 = 0x3f8e_38e4;

/// A graded aid byte: the fraction of the "off" behaviour, 0 < p < 1.
/// `None` for the original's 0 (on) and 1 (off) and for any other value.
pub fn graded(byte: u8) -> Option<f32> {
    let percent = byte & 0x7f;
    (byte & 0x80 != 0 && (1..100).contains(&percent)).then(|| f32::from(percent) / 100.)
}

/// The aid byte for an aid level in percent: 100 is the original's on (0),
/// 0 its off (1), anything between is graded.
pub fn byte(level: u8) -> u8 {
    match level {
        100.. => 0,
        0 => 1,
        l => 0x80 | (100 - l),
    }
}

/// The "off" pedal scale applied to a fraction `p` of its effect:
/// 1 + p (1/0.9 - 1).
pub fn scale(p: f32) -> f32 {
    1. + p * (f32::from_bits(OFF_SCALE) - 1.)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_values_stay_and_levels_encode() {
        assert_eq!(byte(100), 0);
        assert_eq!(byte(0), 1);
        assert_eq!(byte(25), 0x80 | 75);
        assert_eq!(byte(50), 0x80 | 50);
        assert_eq!(graded(0), None);
        assert_eq!(graded(1), None);
        assert_eq!(graded(byte(25)), Some(0.75));
        assert_eq!(graded(byte(50)), Some(0.5));
        assert_eq!(scale(1.), 1. + (f32::from_bits(OFF_SCALE) - 1.));
        assert_eq!(scale(0.), 1.);
        assert!(scale(0.5) > 1. && scale(0.5) < f32::from_bits(OFF_SCALE));
    }
}
