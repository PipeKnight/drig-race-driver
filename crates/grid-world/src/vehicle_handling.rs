//! Whole selected 9F3E80 mutable handling update, before car publication.
//! Own the car/physical buffers and bounded optional component query records.
//! Four supplied impact values must remain below the original damage threshold.
use crate::{Error, steering_control, vehicle_publication::State, wheel_damage};
pub const MUTABLE: usize = 0xcdf0;
/// Only these two fields of each original 2E0-byte record are consumed here.
#[derive(Clone, Copy, Debug)]
pub struct QueryRecord {
    pub id: u32,
    pub status: u32,
}
fn active_query(queries: Option<&[QueryRecord]>, id: u32) -> bool {
    queries
        .and_then(|rows| rows.iter().find(|row| row.id == id))
        .is_some_and(|row| matches!(row.status, 4 | 5))
}
fn f(b: &[u32], o: usize) -> f32 {
    f32::from_bits(b[o / 4])
}
trait Store {
    fn store(&mut self, o: usize, x: f32);
    fn store_byte(&mut self, o: usize, x: u8);
}
impl Store for [u32] {
    fn store(&mut self, o: usize, x: f32) {
        self[o / 4] = x.to_bits();
    }
    fn store_byte(&mut self, o: usize, x: u8) {
        let shift = 8 * (o % 4);
        self[o / 4] = (self[o / 4] & !(255 << shift)) | (u32::from(x) << shift);
    }
}
fn byte(b: &[u32], o: usize) -> u8 {
    (b[o / 4] >> (8 * (o % 4))) as u8
}
fn bounded(b: &[u32], start: usize, end: usize) -> bool {
    b[start / 4..end / 4].iter().all(|&x| {
        let v = f32::from_bits(x);
        v.is_finite() && v.abs() <= 1_000_000.
    })
}
fn normalize(b: &mut [u32], first: usize) {
    let a = f(b, first);
    let c = f(b, first + 4);
    let sum = a + c;
    if sum > 1. {
        let inverse = 1. / sum;
        b.store(first, inverse * a);
        b.store(first + 4, inverse * c);
    }
}
fn fraction(level: f32, threshold: f32) -> f32 {
    if level < threshold {
        0.
    } else if threshold > 1. {
        1.
    } else {
        ((level - threshold) / (1. - threshold)).clamp(0., 1.)
    }
}
fn ramp(level: f32, start: f32, end: f32, low: f32) -> f32 {
    if level < start {
        0.
    } else if start > end {
        1.
    } else {
        (1. - low) * ((level - start) / (end - start)).clamp(0., 1.) + low
    }
}
#[allow(clippy::assign_op_pattern)] // Preserve the original SSE operand order.
fn effects(b: &mut [u32], queries: Option<&[QueryRecord]>) -> Result<(), Error> {
    let s = steering_control::calculate(steering_control::Input {
        wheel_levels_18_24: std::array::from_fn(|i| f(b, 0x18 + 4 * i)),
        cap_levels_30_34: [f(b, 0x30), f(b, 0x34)],
        cap_seeds_48_4c: [f(b, 0x48), f(b, 0x4c)],
        rotation_seeds_50_5c: std::array::from_fn(|i| f(b, 0x50 + 4 * i)),
        perturbation_gain_270: f(b, 0x270),
        ramp_start_274: f(b, 0x274),
        ramp_end_278: f(b, 0x278),
        low_gain_27c: f(b, 0x27c),
        cap_threshold_254: f(b, 0x254),
        wheel_views_active: true,
    })?;
    b.store_byte(0x3c4, byte(b, 0x440));
    let wear = f(b, 0xc);
    b.store(0x3b8, f(b, 0x320) * fraction(wear, f(b, 0x324)));
    b.store(0x3ac, s.perturbation_3ac);
    let mut cap = f(b, 0x310) * fraction(f(b, 8), f(b, 0x314));
    if queries.is_some() {
        // The original performs this even when the query list is empty.
        // Preserve its signed-zero addition and SSE operand order.
        cap = f(b, 0x300) * 0. + cap;
    }
    b.store(0x3a0, cap.clamp(0., 1.));
    b.store(0x3c0, f(b, 0x35c) * fraction(f(b, 0x14), f(b, 0x354)));
    let mut drag = f(b, 0x31c) * fraction(wear, f(b, 0x328));
    if !active_query(queries, 0x1f) {
        drag = f(b, 0x338) * fraction(f(b, 0x10), f(b, 0x33c)) + drag;
    }
    drag = f(b, 0x238) * fraction(f(b, 4), f(b, 0x23c)) + drag;
    b.store(0x3a4, 1. - drag.clamp(0., 1.));
    b.store(
        0x3a8,
        (1. - f(b, 0x224) * fraction(f(b, 0), f(b, 0x228))).clamp(0., 1.),
    );
    b.store(0x3b0, s.caps_3b0_3b4[0]);
    b.store(0x3b4, s.caps_3b0_3b4[1]);
    b.store(
        0x3bc,
        if active_query(queries, 0x46) || active_query(queries, 0x45) {
            (1. - f(b, 0x2fc)).clamp(0., 1.)
        } else {
            1.
        },
    );
    for i in 0..4 {
        let at = 0x3c8 + i * 0x18;
        let level = f(b, 0x18 + i * 4);
        let blend = ramp(level, f(b, 0x274), f(b, 0x278), f(b, 0x27c));
        b.store(
            at,
            (1. - f(b, 0x24c) * fraction(f(b, 0x28 + 4 * i), f(b, 0x254))).clamp(0., 1.),
        );
        b.store(
            at + 0x10,
            (1. - f(b, 0x2ec) * fraction(f(b, 0x38 + 4 * i), f(b, 0x2f8))).clamp(0., 1.),
        );
        b.store(at + 4, (f(b, 0x264) * blend).clamp(0., 1.));
        b.store(at + 8, blend * f(b, 0x268));
        b.store(at + 0xc, s.rotations_3d4_41c[i]);
        b.store_byte(at + 0x14, u8::from(s.frozen_3dc_424[i]));
    }
    let valid =
        bounded(b, 0x3a0, 0x3c4) && (0..4).all(|i| bounded(b, 0x3c8 + i * 0x18, 0x3dc + i * 0x18));
    if !valid {
        return Err(Error("nonfinite handling effects"));
    }
    Ok(())
}
/// The owned component has mode zero, an exact backlink, and query zero.
/// Pointer slots DB0 and mutable214 are zero tokens; no captured pointer is used.
#[allow(clippy::assign_op_pattern)] // Preserve the original SSE operand order.
pub fn calculate(input: &State, delta: f32) -> Result<State, Error> {
    if input.vehicle.len() == crate::vehicle_publication::VEHICLE_WORDS
        && byte(&input.vehicle, MUTABLE + 0x442) != 0
    {
        return Err(Error("unsupported handling branch"));
    }
    calculate_with_queries(input, delta, None)
}

/// Whole selected update with an owned, immutable component lookup list.
/// None is an absent query object; Some(empty) is a present empty object.
/// Notification442 may be on only while every unclamped deformation stays at
/// or below the configured threshold. Event production remains unsupported.
#[allow(clippy::assign_op_pattern)] // Preserve the original SSE operand order.
pub fn calculate_with_queries(
    input: &State,
    delta: f32,
    queries: Option<&[QueryRecord]>,
) -> Result<State, Error> {
    let car = &input.vehicle;
    if car.len() != crate::vehicle_publication::VEHICLE_WORDS
        || !delta.is_finite()
        || !(0. ..=0.1).contains(&delta)
        || queries.is_some_and(|rows| rows.len() > 64)
    {
        return Err(Error("invalid handling framing"));
    }
    let b = &car[MUTABLE / 4..(MUTABLE + 0x460) / 4];
    let p = &input.physical;
    if car[0xdb0 / 4] != 0
        || b[0x214 / 4] != 0
        || car[0x1620 / 4] != 4
        || byte(car, 0xb13) != 1
        || byte(car, 0xb14) != 1
        || car[0x22e4 / 4] != 0
        || byte(b, 0x442) > 1
        || b[0x444 / 4] != 7
        || byte(b, 0x440) > 1
        || byte(car, 0xc7ae) > 1
        || car[0x2118 / 4] > 1
        || car[0x211c / 4] > 2
        || !bounded(b, 0, 0x60)
        || !bounded(b, 0x218, 0x37c)
        || !bounded(b, 0x3a0, 0x3a4)
        || !bounded(b, 0x42c, 0x43c)
        || !bounded(p, 0x1d0, 0x230)
        || !bounded(p, 0x3f0, 0x3f4)
        || f(p, 0x3f0) <= 0.
        || f(b, 0x368) < 0.
        || !(0. ..=1.).contains(&f(b, 0x48))
        || !(0. ..=1.).contains(&f(b, 0x4c))
        || f(b, 0x280) < 0.
        || (f(b, 0x280) != 0. && f(b, 0x284) <= 0.)
        || !bounded(car, 0x1650, 0x1690)
        || !bounded(car, 0x1790, 0x17a0)
        || !bounded(car, 0xc740, 0xc750)
        || !bounded(car, 0xeefc, 0xef00)
        || !bounded(car, 0x18c4, 0x18c8)
    {
        return Err(Error("unsupported handling branch"));
    }
    let delta = delta.min(f32::from_bits(0x3ca3d70a));
    // Original x87 product is stored to f32 before comparison. Two f32
    // operands multiply exactly in f64, matching the selected 53-bit FPU mode.
    let threshold = (f(p, 0x3f0) as f64 * f(b, 0x368) as f64) as f32;
    for i in 0..4 {
        if !bounded(car, 0x1aa8 + i * 0x150, 0x1aac + i * 0x150) {
            return Err(Error("out-of-bound handling wheel wear"));
        }
        // Integrated wheel forces can exceed the old diagnostic's arbitrary
        // one-million bound. Delta is <= .02, so any finite f32 operand has a
        // finite product; retain the original damage-threshold guard below.
        if !f(car, 0x1ac8 + i * 0x150).is_finite() {
            return Err(Error("nonfinite handling wheel impact"));
        }
        if !bounded(b, 0x60 + i * 0x30, 0x70 + i * 0x30) {
            return Err(Error("out-of-bound handling collision point"));
        }
        if f(car, 0x1ac8 + i * 0x150) * delta > threshold {
            return Err(Error("unsupported handling collision damage"));
        }
        if f(car, 0x1aa8 + i * 0x150) > f(b, 0x2f0) && f(b, 0x2f4) <= 0. {
            return Err(Error("invalid active handling wheel-wear divisor"));
        }
    }
    let mut next = input.clone();
    let car = &mut next.vehicle;
    // Separate owned mutable storage prevents aliases during the full update.
    let mut b = car[MUTABLE / 4..(MUTABLE + 0x460) / 4].to_vec();
    for i in 0..4 {
        let wheel = wheel_damage::Input {
            previous_level: f(&b, 0x18 + 4 * i),
            angular_deformation_1790: f(car, 0x1790 + 4 * i),
            displacement_1650: std::array::from_fn(|j| f(car, 0x1650 + 16 * i + 4 * j)),
            correction_enabled_c7ae: byte(car, 0xc7ae) != 0,
            correction_scale_eefc: f(car, 0xeefc),
            correction_vector_c740: std::array::from_fn(|j| f(car, 0xc740 + 4 * j)),
            angular_limit_280: f(&b, 0x280),
            displacement_limit_284: f(&b, 0x284),
        };
        if byte(&b, 0x442) != 0
            && wheel_damage::raw_ratio(wheel)?
                .is_some_and(|ratio| ratio > f(&b, 0x288) * f32::from_bits(0x3c23d70a))
        {
            return Err(Error("unsupported handling notification event"));
        }
        let level = wheel_damage::calculate(wheel)?;
        b.store(0x18 + 4 * i, level);
        for at in [4, 0x10, 0x1c, 0x28, 0x34, 0x40] {
            normalize(&mut b, 0x28c + at);
        }
        let wear = f(car, 0x1aa8 + i * 0x150);
        if wear > f(&b, 0x2f0) {
            let ratio = (wear / f(&b, 0x2f4)).min(1.);
            let at = 0x28c + (car[0x2118 / 4] + 2 * car[0x211c / 4]) as usize * 12;
            let mut amount = ((f(&b, 0x438) * f(&b, 0x434)) * f(&b, 0x430)) * f(&b, at);
            amount = (amount * ratio) * delta;
            let random = car[0x1644 / 4]
                .wrapping_mul(0x41c64e6d)
                .wrapping_add(0x40584c0f);
            car[0x1644 / 4] = random;
            let sample = ((random % 65535) & 255) as f32 * f32::from_bits(0x3b808081);
            if f(&b, at + 8) > sample {
                amount = f(&b, 0x374) * amount;
            } else if f(&b, at + 4) + f(&b, at + 8) > sample {
                amount = f(&b, 0x378) * amount;
            }
            let level = (f(&b, 0x38 + i * 4) + amount).min(1.);
            b.store(0x38 + i * 4, level);
        }
        normalize(&mut b, 0x244);
    }
    for at in [0x21c, 0x230, 0x308] {
        normalize(&mut b, at);
    }
    let cap = f(&b, 0x3a0);
    let mut wear = f(&b, 0xc);
    if cap > wear {
        wear += (((f(&b, 0x438) * f(&b, 0x434)) * f(&b, 0x430)) * f(&b, 0x318)) * delta;
        if wear > cap {
            wear = cap;
        }
    } else {
        wear = cap;
    }
    b.store(0xc, wear);
    if f(p, 0x1e4) < 0. {
        b.store_byte(0x440, 1);
    } else if byte(&b, 0x440) != 0 {
        let v: [f32; 4] = std::array::from_fn(|i| f(p, 0x210 + 4 * i));
        let squared = (v[0] * v[0] + v[1] * v[1]) + (v[2] * v[2] + v[3] * v[3]);
        if squared < f32::from_bits(0x3dcccccd) {
            b.store_byte(0x440, 0);
        }
    }
    normalize(&mut b, 0x330);
    if active_query(queries, 0x1f) {
        b.store(0x10, 1.);
    }
    normalize(&mut b, 0x344);
    if f(car, 0x18c4) > 0. {
        let mut amount = ((f(&b, 0x438) * f(&b, 0x434)) * f(&b, 0x430)) * f(&b, 0x358);
        amount = (amount * f(car, 0x18c4)) * delta;
        let level = (amount + f(&b, 0x14)).min(1.);
        b.store(0x14, level);
    }
    if car[0x196c / 4] != b[0x428 / 4] {
        b[0x428 / 4] = car[0x196c / 4];
        b[0x42c / 4] = b[0x34c / 4];
        let amount = ((f(&b, 0x438) * f(&b, 0x434)) * f(&b, 0x430)) * f(&b, 0x350);
        let level = (amount + f(&b, 0x14)).min(1.);
        b.store(0x14, level);
    } else {
        let timer = f(&b, 0x42c) - delta;
        b.store(0x42c, if timer < 0. { 0. } else { timer });
    }
    effects(&mut b, queries)?;
    if !bounded(&b, 0, 0x60) || !bounded(&b, 0x42c, 0x430) {
        return Err(Error("nonfinite handling result"));
    }
    car[MUTABLE / 4..(MUTABLE + 0x460) / 4].copy_from_slice(&b);
    Ok(next)
}

/// Whole selected A03220(delta,1), including this mutable producer.
/// Stable setup makes the following A03030 call an already-validated no-op.
pub fn publish(input: &State, delta: f32) -> Result<State, Error> {
    let published = crate::vehicle_publication::calculate(input, delta)?;
    calculate(&published, delta)
}

/// Whole selected A03220(delta,1) with the same query/healthy-notification scope.
pub fn publish_with_queries(
    input: &State,
    delta: f32,
    queries: Option<&[QueryRecord]>,
) -> Result<State, Error> {
    let published = crate::vehicle_publication::calculate(input, delta)?;
    calculate_with_queries(&published, delta, queries)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> State {
        let mut s = State {
            vehicle: vec![0; crate::vehicle_publication::VEHICLE_WORDS],
            physical: [0; crate::vehicle_publication::PHYSICAL_WORDS],
        };
        s.vehicle[0x1620 / 4] = 4;
        s.vehicle[0xba0 / 4] = 2;
        s.vehicle.store_byte(0x20fb4, 1);
        s.vehicle.store(0xc798, 0.1);
        s.vehicle.store(0x15d0, 0.1);
        s.vehicle.store_byte(0xb13, 1);
        s.vehicle.store_byte(0xb14, 1);
        s.vehicle[(MUTABLE + 0x444) / 4] = 7;
        for (o, v) in [
            (0x278, 0.95),
            (0x368, 0.8),
            (0x3a0, 1.),
            (0x310, 1.),
            (0x318, 0.2),
            (0x430, 1.),
            (0x434, 1.),
            (0x438, 1.),
        ] {
            s.vehicle.store(MUTABLE + o, v);
        }
        s.physical.store(0x3f0, 1140.);
        s.physical.store(0x1e4, 1.);
        s
    }
    #[test]
    fn finite_large_impact_keeps_original_damage_threshold_guard() {
        let mut s = fixture();
        s.vehicle.store(MUTABLE + 0x368, 10000.);
        s.vehicle.store(0x1ac8, f32::from_bits(0x4a222f9b));
        assert!(calculate(&s, 0.001).is_ok());
        let before = s.clone();
        s.vehicle.store(MUTABLE + 0x368, 0.8);
        assert_eq!(
            calculate(&s, 0.001).err().unwrap().0,
            "unsupported handling collision damage"
        );
        assert_eq!(s.physical, before.physical);
        s.vehicle.store(0x1ac8, f32::INFINITY);
        assert_eq!(
            calculate(&s, 0.001).err().unwrap().0,
            "nonfinite handling wheel impact"
        );
    }

    #[test]
    fn late_effect_failure_rolls_back_earlier_wear_and_gear_history() {
        let mut s = fixture();
        s.vehicle[0x196c / 4] = 2;
        let good = calculate(&s, 0.016).unwrap();
        assert!(f(&good.vehicle, MUTABLE + 0xc) > 0.);
        assert_eq!(good.vehicle[(MUTABLE + 0x428) / 4], 2);
        // A valid finite input reaches a 0/0 ramp in the final producer,
        // after wheel wear/body wear/gear history have already advanced.
        s.vehicle.store(MUTABLE + 0x274, 0.95);
        s.vehicle.store(MUTABLE + 0x278, 0.95);
        for i in 0..4 {
            s.vehicle.store(MUTABLE + 0x18 + i * 4, 0.95);
        }
        let before = s.clone();
        assert_eq!(
            calculate(&s, 0.016).err().unwrap().0,
            "nonfinite steering control result"
        );
        assert_eq!(s.vehicle, before.vehicle);
        assert_eq!(s.physical, before.physical);
        assert_eq!(
            publish(&s, 0.016).err().unwrap().0,
            "nonfinite steering control result"
        );
        assert_eq!(s.vehicle, before.vehicle);
        assert_eq!(s.physical, before.physical);
    }

    #[test]
    fn fourth_wheel_event_rolls_back_wear_and_publication() {
        let mut s = fixture();
        s.vehicle.store(MUTABLE + 0x280, 27.);
        s.vehicle.store(MUTABLE + 0x284, 0.45);
        s.vehicle.store(MUTABLE + 0x288, 105.);
        s.vehicle.store_byte(MUTABLE + 0x442, 1);
        s.vehicle.store(MUTABLE + 0x2f0, 1.);
        s.vehicle.store(MUTABLE + 0x2f4, 10.);
        s.vehicle.store(MUTABLE + 0x28c, 1.);
        s.vehicle.store(0x1aa8, 2.);
        let queries = [QueryRecord {
            id: 0x1f,
            status: 5,
        }];
        let good = publish_with_queries(&s, 0.016, Some(&queries)).unwrap();
        assert_ne!(good.vehicle[0x1644 / 4], s.vehicle[0x1644 / 4]);
        assert_ne!(good.physical, s.physical);
        assert_eq!(f(&good.vehicle, MUTABLE + 0x10), 1.);
        // Three earlier wheel iterations have run before the last wheel
        // reaches an unsupported event, including an RNG update on wheel0.
        s.vehicle.store(0x1790 + 3 * 4, 1.);
        let before = s.clone();
        assert_eq!(
            publish_with_queries(&s, 0.016, Some(&queries))
                .err()
                .unwrap()
                .0,
            "unsupported handling notification event"
        );
        assert_eq!(s.vehicle, before.vehicle);
        assert_eq!(s.physical, before.physical);
    }
}
