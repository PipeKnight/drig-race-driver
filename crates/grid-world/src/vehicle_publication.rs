//! Whole selected A03220: decay state and linked velocity publication.
//! Rendering/UI/setup rebuild and notification are inactive; manager list empty.
use crate::Error;
pub const VEHICLE_WORDS: usize = 0x21000 / 4;
pub const PHYSICAL_WORDS: usize = 0x424 / 4;
#[derive(Clone)]
pub struct State {
    pub vehicle: Vec<u32>,
    pub physical: [u32; PHYSICAL_WORDS],
}
fn f(b: &[u32], o: usize) -> f32 {
    f32::from_bits(b[o / 4])
}
fn byte(b: &[u32], o: usize) -> u8 {
    (b[o / 4] >> (o % 4 * 8)) as u8
}
/// Own all storage; DB0 is zero, never a captured pointer to follow.
pub fn calculate(input: &State, delta: f32) -> Result<State, Error> {
    let b = &input.vehicle;
    if b.len() != VEHICLE_WORDS || !delta.is_finite() || !(0. ..=0.1).contains(&delta) {
        return Err(Error("invalid vehicle publication framing"));
    }
    let independent = b[0x1554 / 4] == 0;
    if b[0xdb0 / 4] != 0
        || b[0x1620 / 4] != 4
        || b[0xdb8 / 4..0xdc8 / 4].iter().any(|&x| x != 0)
        || b[0xdd4 / 4] != 0
        || byte(b, 4) != 0
        || b[0xc7a0 / 4] == 1
        || byte(b, 0xc7f2) != 0
        || b[0xba0 / 4] != if independent { 2 } else { 1 }
        || byte(b, 0x20fb4) != u8::from(independent)
        || [0xc798, 0xc790].iter().any(|&o| !f(b, o).is_finite())
        || [0x15c0, 0x15d0]
            .iter()
            .any(|&o| (0..3).any(|i| !f(b, o + 4 * i).is_finite()))
    {
        return Err(Error("unsupported vehicle publication branch"));
    }
    let mut next = input.clone();
    let b = &mut next.vehicle;
    let mut timer = f(b, 0xc798);
    if timer > 0. {
        timer -= delta;
        b[0xc798 / 4] = timer.to_bits();
    }
    if timer <= 0. {
        let amount = f(b, 0xc790) * 0.5;
        b[0xc790 / 4] = if amount < f32::from_bits(0x3a83126f) {
            0
        } else {
            amount.to_bits()
        };
        b[0xc794 / 4] = 0;
        b[0xc780 / 4] = (-1f32).to_bits();
    }
    // Opaque original callback identity, retained for comparison; never executed.
    b[0xc784 / 4] = 0x9a0c90;
    for (car, physical) in [(0x15c0, 0x220), (0x15d0, 0x210)] {
        b[car / 4 + 3] = 0;
        next.physical[physical / 4..physical / 4 + 4].copy_from_slice(&b[car / 4..car / 4 + 4]);
    }
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zero_delta_still_decays_expired_state_and_clears_unused_w() {
        let mut s = State {
            vehicle: vec![0; VEHICLE_WORDS],
            physical: [0; PHYSICAL_WORDS],
        };
        s.vehicle[0x1620 / 4] = 4;
        s.vehicle[0xba0 / 4] = 2;
        s.vehicle[0x20fb4 / 4] = 1;
        s.vehicle[0xc790 / 4] = 0.5f32.to_bits();
        s.vehicle[0x15cc / 4] = f32::NAN.to_bits();
        let next = calculate(&s, 0.).unwrap();
        assert_eq!(f(&next.vehicle, 0xc790), 0.25);
        assert_eq!(next.vehicle[0x15cc / 4], 0);
        assert_eq!(next.physical[0x22c / 4], 0);
        assert_eq!(s.vehicle[0x15cc / 4], f32::NAN.to_bits());
    }
}
