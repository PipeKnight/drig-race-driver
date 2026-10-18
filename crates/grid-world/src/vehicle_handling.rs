//! Whole selected 9F3E80 mutable handling update, before car publication.
//! Own the car/physical buffers and bounded optional component query records.
//! Reached impacts require an independently owned global damage RNG.
use crate::{Error, steering_control, vehicle_publication::State, wheel_damage};
pub const MUTABLE: usize = 0xcdf0;
pub struct ResetOutput {
    pub owner: [u32; 280],
    pub random_state: u32,
}
/// Whole selected 9E6070 reset and 9C5B90 effects. Optional secondary component
/// and query list are absent; wheel-view ownership is supplied independently.
pub fn reset(
    input: &[u32; 280],
    random_state: u32,
    wheel_views_active: bool,
) -> Result<ResetOutput, Error> {
    if input[0x210 / 4] != 0
        || input[0x214 / 4] != 0
        || !input[0x218 / 4..0x380 / 4]
            .iter()
            .all(|&word| f32::from_bits(word).is_finite())
    {
        return Err(Error("invalid handling reset owner"));
    }
    let f = |o| f(input, o);
    let seeds = steering_control::reset(
        random_state,
        steering_control::ResetParameters {
            maximum_rotation_26c: f(0x26c),
            maximum_cap_250: f(0x250),
            perturbation_gain_270: f(0x270),
            ramp_start_274: f(0x274),
            ramp_end_278: f(0x278),
            low_gain_27c: f(0x27c),
            cap_threshold_254: f(0x254),
            wheel_views_active,
        },
    )?;
    let mut b = *input;
    b[..0x60 / 4].fill(0);
    for (i, value) in seeds.rotation_seeds_50_5c.into_iter().enumerate() {
        b.store(0x50 + i * 4, value);
    }
    for (i, value) in seeds.cap_seeds_48_4c.into_iter().enumerate() {
        b.store(0x48 + i * 4, value);
    }
    b[0x428 / 4] = u32::MAX;
    b.store(0x42c, 0.);
    b.store_byte(0x440, 0);
    effects(&mut b, None, wheel_views_active)?;
    b[0x444 / 4] = 7;
    b.store_byte(0x448, 0);
    Ok(ResetOutput {
        owner: b,
        random_state: seeds.random_state,
    })
}
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
/// The 9C5B90 damage ramp. At `level == threshold == 1` (the Standard.hdm
/// suspension, wheel and cap thresholds are 1.0, and damage levels saturate
/// at 1.0) the original divides 0/0 and its COMISS/JBE clamp passes the NaN
/// on. Its readers of the suspension effect (+0x3C0 -> +0x2280: 0x9B2C5A,
/// 0x9B42CD) and of the caps (0x9B37F7, 0x9B388B) compare with JBE and so
/// treat the NaN as zero blend; the port stores that zero (F342) instead of
/// retiring the car. The per-wheel effect reader (0x9B2F50) does arithmetic
/// on it, so there the original would spread the NaN; zero is the deviation.
fn fraction(level: f32, threshold: f32) -> f32 {
    if level < threshold {
        0.
    } else if threshold > 1. {
        1.
    } else {
        let blend = (level - threshold) / (1. - threshold);
        if blend.is_nan() {
            0.
        } else {
            blend.clamp(0., 1.)
        }
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
pub(crate) fn effects(
    b: &mut [u32],
    queries: Option<&[QueryRecord]>,
    wheel_views_active: bool,
) -> Result<(), Error> {
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
        wheel_views_active,
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
        if !wheel_views_active {
            for (offset, value) in [(0, 1.), (4, 0.), (8, 0.), (12, 0.), (16, 1.)] {
                b.store(at + offset, value);
            }
            b.store_byte(at + 0x14, 0);
            continue;
        }
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
    calculate_owned(input, delta, queries, false)
}

/// Complete wheel-terminal transition with an owned component whose event
/// receiver (+84) is absent. The source changes status 7 to 3 and records the
/// first failing wheel; no external notification callback is discarded.
pub fn calculate_with_empty_notification_receiver(
    input: &State,
    delta: f32,
    queries: Option<&[QueryRecord]>,
) -> Result<State, Error> {
    calculate_owned(input, delta, queries, true)
}

#[allow(clippy::assign_op_pattern)] // Preserve the original SSE operand order.
fn calculate_owned(
    input: &State,
    delta: f32,
    queries: Option<&[QueryRecord]>,
    empty_notification_receiver: bool,
) -> Result<State, Error> {
    let car = &input.vehicle;
    if car.len() != crate::vehicle_publication::VEHICLE_WORDS
        || !delta.is_finite()
        || !(0. ..=0.1).contains(&delta)
        || queries.is_some_and(|rows| rows.len() > 64)
    {
        return Err(Error("invalid handling framing"));
    }
    // 9F3EAC..9F3EB7: either activation byte clear returns at once. The damage
    // option's mode 0 (mechanical_damage::Damage::Off) clears +B14.
    if byte(car, 0xb13) == 0 || byte(car, 0xb14) == 0 {
        return Ok(input.clone());
    }
    let b = &car[MUTABLE / 4..(MUTABLE + 0x460) / 4];
    let p = &input.physical;
    if car[0xdb0 / 4] != 0
        || b[0x214 / 4] != 0
        || car[0x1620 / 4] != 4
        || byte(car, 0xb13) != 1
        || byte(car, 0xb14) != 1
        || (0..4).any(|i| byte(car, 0x22e4 + i) > 1)
        || byte(b, 0x442) > 1
        // Status 6: a disqualified car (0x9E64E0(6, 0); Demolition Derby).
        || !(b[0x444 / 4] == 7
            || (empty_notification_receiver && matches!(b[0x444 / 4], 3 | 4 | 6)))
        || (matches!(b[0x444 / 4], 3 | 4) && byte(b, 0x448) > 3)
        || byte(b, 0x440) > 1
        || byte(car, 0xc7ae) > 1
        || car[0x2118 / 4] > 1
        || car[0x211c / 4] > 2
        || !bounded(b, 0, 0x60)
        // Source Standard HDM's wear-force divisor exceeds ten million.
        // These two force parameters are compared/divided, not accumulated as
        // bounded level values. Keep their finite and active-divisor checks.
        || !bounded(b, 0x218, 0x2f0)
        || !f(b, 0x2f0).is_finite()
        || !f(b, 0x2f4).is_finite()
        || !bounded(b, 0x2f8, 0x37c)
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
        if input.damage_random_state.is_none() && f(car, 0x1ac8 + i * 0x150) * delta > threshold {
            return Err(Error("unsupported handling collision damage"));
        }
        if f(car, 0x1aa8 + i * 0x150) > f(b, 0x2f0) && f(b, 0x2f4) <= 0. {
            return Err(Error("invalid active handling wheel-wear divisor"));
        }
    }
    let mut next = input.clone();
    if let Some(mut random_state) = input.damage_random_state {
        // Original order: all four collision dispatches precede wheel wear.
        // Transform grouping follows 9F3F7D..9F3FD0, including homogeneous W.
        for i in 0..4 {
            let impact = f(car, 0x1ac8 + i * 0x150) * delta;
            if impact <= threshold {
                continue;
            }
            let local: [f32; 4] = std::array::from_fn(|j| f(b, 0x60 + i * 0x30 + j * 4));
            let point = std::array::from_fn(|j| {
                (local[3] * f(p, 0x200 + j * 4) + local[1] * f(p, 0x1e0 + j * 4))
                    + (local[2] * f(p, 0x1f0 + j * 4) + local[0] * f(p, 0x1d0 + j * 4))
            });
            let output = crate::collision_damage::calculate_with_queries(
                &next,
                point,
                impact,
                true,
                random_state,
                queries,
            )?;
            random_state = output.random_state;
            next = output.state;
        }
        next.damage_random_state = Some(random_state);
    }
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
            && b[0x444 / 4] == 7
            && wheel_damage::raw_ratio(wheel)?
                .is_some_and(|ratio| ratio > f(&b, 0x288) * f32::from_bits(0x3c23d70a))
        {
            if !empty_notification_receiver {
                return Err(Error("unsupported handling notification event"));
            }
            // A14E40 tests car B13/B14, validated above. 9F2F80 commits
            // status and ordinal before checking the absent component receiver.
            b[0x444 / 4] = 3;
            b.store_byte(0x448, i as u8);
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
    effects(&mut b, queries, true)?;
    // 9E65B0 (called at 9F4246): while racing (status 7), the first detached
    // wheel (car +22E4 + i, set by the loose-part rule 9EC9B0) is terminal
    // damage 9E64E0(4, wheel). 9E64E0 needs +442 and A14E40 (car B13/B14,
    // validated above); status and wheel commit before the absent receiver.
    // The second 9E65B0 loop only returns early and changes nothing.
    if b[0x444 / 4] == 7
        && byte(&b, 0x442) != 0
        && let Some(i) = (0..4).find(|&i| byte(car, 0x22e4 + i) != 0)
    {
        if !empty_notification_receiver {
            return Err(Error("unsupported handling notification event"));
        }
        b[0x444 / 4] = 4;
        b.store_byte(0x448, i as u8);
    }
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

pub fn publish_with_empty_notification_receiver(
    input: &State,
    delta: f32,
    queries: Option<&[QueryRecord]>,
) -> Result<State, Error> {
    let published = crate::vehicle_publication::calculate(input, delta)?;
    calculate_with_empty_notification_receiver(&published, delta, queries)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> State {
        let mut s = State {
            damage_random_state: None,
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
    fn activate_damage(s: &mut State) {
        s.damage_random_state = Some(0x8291_abcd);
        for at in [0x1d0, 0x1e4, 0x1f8, 0x20c] {
            s.physical.store(at, 1.);
        }
        s.vehicle.store(MUTABLE + 0x368, 0.);
        s.vehicle.store(MUTABLE + 0x36c, 1.);
        s.vehicle.store(MUTABLE + 0x370, 1.);
        s.vehicle.store(MUTABLE + 0x240, 0.7);
        s.vehicle.store(MUTABLE + 0x254, 0.5);
        for at in [0x60, 0x90, 0xc0, 0xf0, 0x120, 0x150, 0x180, 0x1b0, 0x1e0] {
            s.vehicle.store(MUTABLE + at + 12, 1.);
            for j in 0..4 {
                s.vehicle.store(MUTABLE + at + 16 + j * 4, 1.);
            }
            s.vehicle.store_byte(MUTABLE + at + 32, 1);
        }
        s.vehicle.store(0x1ac8, 100_000.);
    }
    #[test]
    fn owned_damage_rng_is_distinct_and_failed_late_event_rolls_back() {
        let mut s = fixture();
        activate_damage(&mut s);
        s.vehicle.store(MUTABLE + 0x280, 27.);
        s.vehicle.store(MUTABLE + 0x284, 0.45);
        s.vehicle.store(MUTABLE + 0x288, 105.);
        s.vehicle.store_byte(MUTABLE + 0x442, 1);
        s.vehicle.store(MUTABLE + 0x2f0, 1.);
        s.vehicle.store(MUTABLE + 0x2f4, 10.);
        s.vehicle.store(MUTABLE + 0x28c, 1.);
        s.vehicle.store(0x1aa8, 2.);
        let good = publish_with_queries(&s, 0.016, Some(&[])).unwrap();
        assert_ne!(good.damage_random_state, s.damage_random_state);
        assert_ne!(good.vehicle[0x1644 / 4], s.vehicle[0x1644 / 4]);
        assert!(f(&good.vehicle, MUTABLE + 0x28) > 0.);
        // Collision RNG and wheel0 wear precede the unsupported fourth event.
        s.vehicle.store(0x1790 + 3 * 4, 1.);
        let before = s.clone();
        assert_eq!(
            publish_with_queries(&s, 0.016, Some(&[])).err().unwrap().0,
            "unsupported handling notification event"
        );
        assert_eq!(s.damage_random_state, before.damage_random_state);
        assert_eq!(s.vehicle, before.vehicle);
        assert_eq!(s.physical, before.physical);
    }
    #[test]
    fn inactive_damage_returns_unchanged() {
        let mut s = fixture();
        activate_damage(&mut s);
        s.vehicle[0x196c / 4] = 2;
        s.vehicle.store_byte(0xb14, 0);
        let out = calculate_with_queries(&s, 0.016, Some(&[])).unwrap();
        assert_eq!(out.vehicle, s.vehicle);
        assert_eq!(out.damage_random_state, s.damage_random_state);
        s.vehicle.store_byte(0xb14, 1);
        let active = calculate_with_queries(&s, 0.016, Some(&[])).unwrap();
        assert_ne!(active.vehicle, s.vehicle);
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
    fn saturated_damage_at_unit_thresholds_gives_zero_blend() {
        // Seed 2177 (long Nuerburgring): suspension level 0x14 reached the
        // Standard.hdm threshold 0x354 = 1 and 0x3C0 became 0/0.
        let mut s = fixture();
        let m = MUTABLE / 4;
        let b = &mut s.vehicle[m..m + 280];
        for (o, v) in [
            (0x14, 1.),
            (0x354, 1.),
            (0x35c, 1.),
            (0x2f8, 1.),
            (0x2ec, 0.5),
        ] {
            b.store(o, v);
        }
        for i in 0..4 {
            b.store(0x28 + 4 * i, 1.);
            b.store(0x38 + 4 * i, 1.);
        }
        b.store(0x254, 1.);
        b.store(0x24c, 0.4);
        b.store(0x30, 1.);
        b.store(0x34, 1.);
        b.store(0x48, 0.5);
        b.store(0x4c, 0.5);
        effects(b, None, true).unwrap();
        assert_eq!(f(b, 0x3c0), 0.);
        assert_eq!([f(b, 0x3b0), f(b, 0x3b4)], [1., 1.]);
        for i in 0..4 {
            assert_eq!([f(b, 0x3c8 + i * 0x18), f(b, 0x3d8 + i * 0x18)], [1., 1.]);
        }
        // Past a unit threshold the original's x/0 = inf clamps to 1.
        assert_eq!(fraction(1.5, 1.), 1.);
        assert_eq!(fraction(0.5, 0.25), 0.25f32 / 0.75);
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

    #[test]
    fn terminal_wheel_transition_is_once_ordered_and_atomic() {
        let mut s = fixture();
        activate_damage(&mut s);
        s.vehicle.store(MUTABLE + 0x280, 27.);
        s.vehicle.store(MUTABLE + 0x284, 0.45);
        s.vehicle.store(MUTABLE + 0x288, 105.);
        s.vehicle.store_byte(MUTABLE + 0x442, 1);
        s.vehicle.store(0x1790 + 4, 1.);
        s.vehicle.store(0x1790 + 8, 2.);
        let before = s.clone();
        let terminal = calculate_with_empty_notification_receiver(&s, 0.016, Some(&[])).unwrap();
        assert_eq!(terminal.vehicle[(MUTABLE + 0x444) / 4], 3);
        assert_eq!(byte(&terminal.vehicle, MUTABLE + 0x448), 1);
        assert_eq!(f(&terminal.vehicle, MUTABLE + 0x1c), 1.);
        assert_ne!(terminal.damage_random_state, s.damage_random_state);
        let continued =
            calculate_with_empty_notification_receiver(&terminal, 0.016, Some(&[])).unwrap();
        assert_eq!(continued.vehicle[(MUTABLE + 0x444) / 4], 3);
        assert_eq!(byte(&continued.vehicle, MUTABLE + 0x448), 1);
        assert_eq!(s.vehicle, before.vehicle);
        assert_eq!(s.damage_random_state, before.damage_random_state);
        // A later invalid effect cannot commit even an already reached terminal
        // transition, collision RNG advance or previous wheel updates.
        s.vehicle.store(MUTABLE + 0x274, 1.);
        s.vehicle.store(MUTABLE + 0x278, 1.);
        let before = s.clone();
        assert!(calculate_with_empty_notification_receiver(&s, 0.016, Some(&[])).is_err());
        assert_eq!(s.vehicle, before.vehicle);
        assert_eq!(s.damage_random_state, before.damage_random_state);
    }

    #[test]
    fn a_detached_wheel_is_terminal_damage_for_the_first_flagged_wheel() {
        let mut s = fixture();
        activate_damage(&mut s);
        s.vehicle.store_byte(MUTABLE + 0x442, 1);
        s.vehicle.store_byte(0x22e4 + 2, 1);
        s.vehicle.store_byte(0x22e4 + 3, 1);
        let before = s.clone();
        let lost = calculate_with_empty_notification_receiver(&s, 0.016, Some(&[])).unwrap();
        assert_eq!(lost.vehicle[(MUTABLE + 0x444) / 4], 4);
        assert_eq!(byte(&lost.vehicle, MUTABLE + 0x448), 2);
        assert_eq!(s.vehicle, before.vehicle);
        // Status 4 is not 7, so 9E65B0 does nothing on later ticks.
        let continued =
            calculate_with_empty_notification_receiver(&lost, 0.016, Some(&[])).unwrap();
        assert_eq!(continued.vehicle[(MUTABLE + 0x444) / 4], 4);
        assert_eq!(byte(&continued.vehicle, MUTABLE + 0x448), 2);
        // A healthy receiver would get a notification, which is not ported.
        assert_eq!(
            calculate_with_queries(&s, 0.016, Some(&[]))
                .err()
                .unwrap()
                .0,
            "unsupported handling notification event"
        );
        // 9E64E0 ignores the request while +442 is clear.
        let mut quiet = s.clone();
        quiet.vehicle.store_byte(MUTABLE + 0x442, 0);
        let quiet = calculate_with_empty_notification_receiver(&quiet, 0.016, Some(&[])).unwrap();
        assert_eq!(quiet.vehicle[(MUTABLE + 0x444) / 4], 7);
    }
}
