//! Supplied tyre-table lookup and material slip-speed grip expression.
use crate::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Code([u8; 4]),
    Index(usize),
}

/// Original lookup caches the first exact code match, retains unresolved misses,
/// and returns one while inactive. The caller supplies the selected row values.
pub fn factor(keys: &[[u8; 4]], values: &[f32], active: bool, key: &mut Key) -> Result<f32, Error> {
    if !active {
        return Ok(1.);
    }
    let index = match *key {
        Key::Code(code) => {
            let Some(index) = keys.iter().position(|k| *k == code) else {
                return Ok(1.);
            };
            *key = Key::Index(index);
            index
        }
        Key::Index(index) => index,
    };
    if keys.len() != values.len() || values.iter().any(|v| !v.is_finite()) {
        return Err(Error("invalid supplied tyre table row"));
    }
    values
        .get(index)
        .copied()
        .ok_or(Error("cached tyre table index is out of bounds"))
}

/// Result at the consumer's f32 store. Preserve the original x87 double-precision
/// intermediate rather than rounding slope*speed+base to f32 before the factor.
pub fn grip(slope: f32, maximum: f32, base: f32, speed: f32, factor: f32) -> Result<f32, Error> {
    let value = extended(slope, maximum, base, speed, factor)? as f32;
    if !value.is_finite() {
        return Err(Error("material grip exceeds finite f32 range"));
    }
    Ok(value)
}

/// Preserve the original returned intermediate when a consumer multiplies it
/// before its own f32 store, as in the wheel-history calculation.
pub fn extended(
    slope: f32,
    maximum: f32,
    base: f32,
    speed: f32,
    factor: f32,
) -> Result<f64, Error> {
    if [slope, maximum, base, speed, factor]
        .iter()
        .any(|v| !v.is_finite())
        || maximum < 0.
    {
        return Err(Error("invalid material grip inputs"));
    }
    let speed = if speed < 0. {
        0.
    } else if speed > maximum {
        maximum
    } else {
        speed
    };
    let value = ((slope as f64 * speed as f64) + base as f64) * factor as f64;
    if !value.is_finite() {
        return Err(Error("material grip exceeds finite f32 range"));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lookup_caches_first_match_but_preserves_inactive_and_missing_codes() {
        let keys = [*b"TEST", *b"TEST"];
        let mut key = Key::Code(*b"TEST");
        assert_eq!(factor(&keys, &[2., 3.], false, &mut key).unwrap(), 1.);
        assert_eq!(key, Key::Code(*b"TEST"));
        assert_eq!(factor(&keys, &[2., 3.], true, &mut key).unwrap(), 2.);
        assert_eq!(key, Key::Index(0));
        assert_eq!(factor(&keys, &[4., 5.], true, &mut key).unwrap(), 4.);
        let mut absent = Key::Code(*b"MISS");
        assert_eq!(factor(&keys, &[2., 3.], true, &mut absent).unwrap(), 1.);
        assert_eq!(absent, Key::Code(*b"MISS"));
        let mut invalid = Key::Index(2);
        assert!(factor(&keys, &[2., 3.], true, &mut invalid).is_err());
        assert_eq!(factor(&[], &[], false, &mut invalid).unwrap(), 1.);
    }
}
