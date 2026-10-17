//! Whole selected suspension-to-grip wrapper. Tyre table is inactive; world
//! queries and source material ownership are supplied by the caller.
use crate::{
    Error, contact_ray, grip_history,
    suspension_prepare::{Contact, State},
    suspension_vehicle, wheel_history,
};

pub const VIEW_WORDS: usize = 0x300 / 4;
fn f(v: &[u32; VIEW_WORDS], o: usize) -> f32 {
    f32::from_bits(v[o / 4])
}
fn set(v: &mut [u32; VIEW_WORDS], o: usize, x: f32) {
    v[o / 4] = x.to_bits();
}
fn vector(v: &[u32; VIEW_WORDS], o: usize) -> [f32; 4] {
    std::array::from_fn(|i| f(v, o + i * 4))
}
fn dot(a: [f32; 4], b: [f32; 4]) -> f32 {
    (a[1] * b[1] + a[0] * b[0]) + (a[3] * b[3] + a[2] * b[2])
}

/// The material callback resolves source tokens, including retained history
/// identities. Both buffers commit only after every stage succeeds.
#[allow(
    clippy::assign_op_pattern,
    reason = "Preserve observed multiplier-first arithmetic"
)]
pub fn apply(
    state: &mut State,
    view: &mut [u32; VIEW_WORDS],
    delta: f32,
    query: impl FnMut(usize, contact_ray::Ray) -> Result<Option<Contact>, Error>,
    mut material: impl FnMut(u32) -> Result<Option<wheel_history::Material>, Error>,
) -> Result<(), Error> {
    let mut s = state.clone();
    let mut v = *view;
    let scalars = [
        0xfcc, 0xfe4, 0xe1c, 0xe20, 0xe74, 0x1160, 0x1164, 0x1168, 0x116c, 0x1170, 0x1174, 0x11f4,
        0x12a8, 0x1354, 0x137c, 0x187c, 0x1efc, 0x1f10, 0x1f20, 0x1f24, 0x1f3c, 0x1f64, 0x1f8c,
        0x1f90, 0x1fa4, 0x1fa8, 0x1ffc, 0x21f8, 0x21fc, 0x2200,
    ];
    if s.words[0] != 0
        || s.words[0x1620 / 4] != 4
        || s.words[0x21ec / 4] > 4
        || scalars.into_iter().any(|o| !s.f(o).is_finite())
        || (0..10).any(|i| !s.f(0x11fc + i * 4).is_finite())
        || s.f(0x1648) < 0.
        || s.f(0x1648) > 1_000_000.
        || s.f(0x1f3c) <= 0.
        || s.f(0x1ffc) < 0.
        || s.f(0x137c) < 0.
        || s.f(0x137c) >= 1.
        || s.f(0xfcc) <= 0.
        || s.f(0xfcc) >= 1.
        || s.f(0x1160) == 0.
        || s.f(0x21f8) == s.f(0x21fc)
        || s.f(0x12a8) < 0.
        || s.f(0x12a8) > 4.
        || (0..4).any(|i| {
            [
                0xb0, 0xb4, 0xb8, 0xbc, 0x80, 0x104, 0xe4, 0xec, 0x10c, 0x11c, 0x120, 0x124,
            ]
            .into_iter()
            .any(|o| !s.f(0x1990 + i * 0x150 + o).is_finite())
        })
        || (0..4).any(|i| {
            [0x190, 0x1d0, 0x210]
                .into_iter()
                .any(|o| vector(&v, o + i * 16).iter().any(|x| !x.is_finite()))
                || !f(&v, 0x298 + i * 4).is_finite()
        })
    {
        return Err(Error("invalid selected suspension/grip wrapper input"));
    }
    for i in 0..4 {
        let token = s.words[(0x1990 + i * 0x150 + 0x128) / 4];
        if token != 0 {
            material(token)?.ok_or(Error("unknown retained material"))?;
        }
    }
    // The isolated reference's uninitialized gravity global is zero in this
    // boundary fixture. Suspension has its separate verified constant gravity.
    let inertia = suspension_vehicle::apply(&mut s, delta, query)?;
    for (i, x) in inertia.into_iter().enumerate() {
        set(&mut v, 0x48 + i * 4, x);
    }
    let mut notification = false;
    s.words[0x2204 / 4] &= !255;
    for i in 0..4 {
        let w = 0x1990 + i * 0x150;
        // Optional collision-shape publication; DB8 entries are presence flags.
        if s.words[(0xdb8 + i * 4) / 4] != 0 {
            let height = state.f(w + 0x40).max(s.f(w + 0x40)) + s.f(w + 0xe0) * 0.5;
            let anchor = s.vector(w);
            let local = s.vector(0x1650 + i * 16);
            let axis = s.vector(0x420 + i * 0x170);
            let mut point: [f32; 4] =
                std::array::from_fn(|j| height * axis[j] + (local[j] + anchor[j]));
            let shift = f32::from_bits(0x3e66_6666);
            point[0] = if point[0] > 0. {
                point[0] - shift
            } else {
                point[0] + shift
            };
            s.put(0x250 + i * 16, point);
        }
        // A00E10 (A0129B): a detached wheel (body +2278 + i) skips the tyre
        // history 9BA490 and every grip modifier; its grip is zero.
        if (s.words[0x2278 / 4] >> (i * 8)) & 255 != 0 {
            set(&mut v, 0x5c + i * 4, 0.);
            s.set(w + 0x78, 0.);
            continue;
        }
        let m = material(s.words[(w + 0xc0) / 4])?;
        let history = wheel_history::calculate_angled(
            wheel_history::Input {
                vehicle: wheel_history::Vehicle {
                    speed: s.f(0x1648),
                    mass: s.f(0x15a0),
                    scalar_1f3c: s.f(0x1f3c),
                    scalar_1ffc: s.f(0x1ffc),
                    scalar_1efc: s.f(0x1efc),
                    scalar_e20: s.f(0xe20),
                    scalar_187c: s.f(0x187c),
                    minimum_alignment: s.f(0x137c),
                    flag_b11: (s.words[0xb10 / 4] >> 8) & 255 != 0,
                    flag_e38: s.words[0xe38 / 4] & 255 != 0,
                    direction: s.vector(0x15e0),
                },
                wheel: wheel_history::Wheel {
                    air_clock: s.f(w + 0xb8),
                    return_clock: s.f(w + 0xbc),
                    // The original history helper overwrites +6c before reading it.
                    scalar_6c: 0.,
                    grip_scale: s.f(w + 0x80),
                    spin: s.f(w + 0x104),
                    radius: s.f(w + 0xe0),
                    scalar_138: s.f(w + 0x138),
                    scalar_e4: s.f(w + 0xe4),
                    pair_b0_b4: [s.f(w + 0xb0), s.f(w + 0xb4)],
                    scalar_10c: s.f(w + 0x10c),
                    flag_fc: s.words[(w + 0xfc) / 4] & 255 != 0,
                    flag_fd: (s.words[(w + 0xfc) / 4] >> 8) & 255 != 0,
                    history: [s.f(w + 0x11c), s.f(w + 0x120), s.f(w + 0x124)],
                    history_material: s.words[(w + 0x128) / 4],
                },
                material: m,
                velocity: vector(&v, 0x190 + i * 16),
                lateral_axis: vector(&v, 0x210 + i * 16),
                longitudinal_axis: vector(&v, 0x1d0 + i * 16),
                normal: s.vector(w + 0xd0),
                delta,
                scalar_argument_10: 0.,
            },
            s.f(w + 0xec),
            s.vector(0x15f0),
        )?;
        let wh = history.wheel;
        for (o, x) in [
            (0xb8, wh.air_clock),
            (0xbc, wh.return_clock),
            (0x6c, wh.scalar_6c),
            (0x11c, wh.history[0]),
            (0x120, wh.history[1]),
            (0x124, wh.history[2]),
        ] {
            s.set(w + o, x);
        }
        s.words[(w + 0xfc) / 4] = (s.words[(w + 0xfc) / 4] & !0xff00) | ((wh.flag_fd as u32) << 8);
        s.words[(w + 0x128) / 4] = wh.history_material;
        let axle = if s.f(w + 8) > 0. {
            s.f(0xfcc)
        } else {
            1. - s.f(0xfcc)
        };
        let denominator = (axle * (s.f(0x15a0) * f32::from_bits(0x411c_f5c3))) * 0.5;
        let load = s.f(w + 0x138) / denominator;
        let mut grip = history.grip;
        if load >= 0. {
            grip = (((load as f64).powf(s.f(0x12a8) as f64) as f32) / load) * grip;
        }
        if s.words[0x1558 / 4] & 255 != 0 {
            grip = (1. - s.f(if i < 2 { 0x1174 } else { 0x1170 })) * grip;
        }
        // Graded Braking Assist (AUTHORED): the lock-up needs the brake held
        // 1/p times as long.
        let lock = match crate::driver_aids::graded((s.words[0x1558 / 4] >> 8) as u8) {
            Some(p) => s.f(0xe74) > s.f(0x1168) / p,
            None => s.f(0xe74) > s.f(0x1168),
        };
        if lock {
            grip = (1. - s.f(0x116c)) * grip;
            s.set(w + 0x104, 0.);
        }
        let traction = crate::driver_aids::graded((s.words[0x1558 / 4] >> 16) as u8);
        if (s.words[0x1558 / 4] >> 16) & 255 != 0
            && s.f(0x185c) > f32::from_bits(0x3f66_6666)
            && s.f(w + 0x10c) > 0.
        {
            let speed = s.f(0x1648);
            let t = if speed <= 3. {
                (6. - speed) * f32::from_bits(0x3e2a_aaab)
            } else {
                (speed - 6.) / s.f(0x1160)
            };
            let factor = ((1. - t.clamp(0., 1.)) * s.f(0x1164))
                * ((s.f(0x185c) - f32::from_bits(0x3f66_6666)) * f32::from_bits(0x411f_fffe));
            // Graded Traction Control (AUTHORED): part of the grip penalty.
            let factor = traction.map_or(factor, |p| p * factor);
            grip = (1. - factor) * grip;
        }
        grip = f(&v, 0x298 + i * 4) * grip;
        let t = s.f(0x1648) * f32::from_bits(0x3db2_1643);
        let mut index = t as usize;
        let mut blend = t - index as f32;
        if index > 3 {
            index = 3;
            blend = 1.;
        }
        let table = if s.f(w + 8) > 0. { 0x11fc } else { 0x1210 };
        grip =
            ((1. - blend) * s.f(table + index * 4) + s.f(table + (index + 1) * 4) * blend) * grip;
        set(&mut v, 0x6c + i * 4, 1.);
        if s.f(w + 0x10c) > 0. {
            let velocity = s.vector(0x15c0);
            let squared = dot(velocity, velocity);
            let mut alignment = 1.;
            if squared > 0. {
                let inverse = 1. / squared.sqrt();
                alignment = ((dot(s.vector(0x1600), velocity.map(|x| inverse * x))
                    - f32::from_bits(0x3f7c_28f6))
                    * f32::from_bits(0x4285_555e))
                .clamp(0., 1.);
            }
            let input = s.f(0xe1c) as f64;
            set(
                &mut v,
                0x6c + i * 4,
                (1. - (1. - alignment as f64) * (input * s.f(0xfe4) as f64)) as f32,
            );
            grip = ((1. - input * s.f(0x11f4) as f64) * grip as f64) as f32;
        }
        if s.words[0x1640 / 4] & 255 != 0 && s.f(w + 8) < 0. {
            let t = ((s.f(0x1648) - 5.) * f32::from_bits(0x3dcc_cccd)).clamp(0., 1.);
            grip = (1. - s.f(0x1354) * t) * grip;
        }
        grip = s.f(0x1f20) * grip;
        grip = grip_history::apply(
            grip_history::Input {
                history: wh.history,
                scalar_1ffc: s.f(0x1ffc),
                scalar_1f24: s.f(0x1f24),
                scalar_1fa4: s.f(0x1fa4),
                scalar_1fa8: s.f(0x1fa8),
                wheel_coordinate_z: s.f(w + 8),
                flag_b12: (s.words[0xb10 / 4] >> 16) & 255 != 0,
                flag_227c: (s.words[0x227c / 4] >> (i * 8)) & 255 != 0,
            },
            grip,
        )?;
        let gear = s.words[0x196c / 4];
        if (gear == 1 || (gear == 9 && (s.words[0x2204 / 4] >> 8) & 255 != 0))
            && s.f(0xe1c) > f32::from_bits(0x3f66_6666)
        {
            // A zero +10c leaves the flag from the previous wheel intact.
            if s.f(w + 0x10c) != 0. {
                let speed = s.f(0x1648);
                if speed < s.f(0x21fc)
                    || (dot(s.vector(0x15c0), s.vector(0x1600)) < 0. && gear == 1)
                {
                    s.words[0x2204 / 4] |= 1;
                    let t = ((s.f(0x21fc) - speed) / (s.f(0x21fc) - s.f(0x21f8))).clamp(0., 1.);
                    grip = (1. - s.f(0x2200) * t) * grip;
                } else {
                    s.words[0x2204 / 4] &= !255;
                }
            }
        } else {
            s.words[0x2204 / 4] &= !255;
        }
        if !grip.is_finite() || !f(&v, 0x6c + i * 4).is_finite() {
            return Err(Error("suspension/grip wrapper exceeds finite range"));
        }
        set(&mut v, 0x5c + i * 4, grip);
        s.set(w + 0x78, grip);
    }
    for i in 0..4 {
        let w = 0x1990 + i * 0x150;
        if (s.words[(w + 0xfc) / 4] >> 8) & 255 != 0 {
            s.words[(w + 0xfc) / 4] &= !0xff00;
            notification = true;
        }
    }
    if notification && s.words[0x21ec / 4] < 4 {
        let count = s.words[0x21ec / 4] as usize;
        s.words[0x21dc / 4 + count] = 3;
        s.words[0x21ec / 4] += 1;
    }
    *state = s;
    *view = v;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn material_failure_after_suspension_rolls_back_both_buffers() {
        let mut s = State {
            words: [0; crate::suspension_prepare::WORDS],
        };
        let c = 0x3d0;
        s.words[(c + 0x5d8) / 4] = 4;
        s.words[0x1620 / 4] = 4;
        for (o, x) in [
            (c, 1.),
            (0x15a0, 1000.),
            (0x1160, 5.),
            (0xfcc, 0.53),
            (0x1f3c, 0.01),
            (0x21fc, 6.),
            (0x21b4, 2.),
            (0x21b8, 5.),
            (0x21bc, 10.),
            (0x21c0, 1.),
        ] {
            s.set(o, x);
        }
        for i in 0..3 {
            s.set(0x15a8 + i * 4, 600.);
        }
        for i in 0..7 {
            s.set(0x2198 + i * 4, 1.);
        }
        for (i, row) in [
            [1., 0., 0., 0.],
            [0., 1., 0., 0.],
            [0., 0., 1., 0.],
            [0., 1., 0., 1.],
        ]
        .into_iter()
        .enumerate()
        {
            s.put(0x15e0 + i * 16, row);
            s.put(0xb40 + i * 16, row);
        }
        for i in 0..4 {
            let w = 0x1990 + i * 0x150;
            s.put(c + i * 0x170 + 0x50, [0., 1., 0., 0.]);
            s.set(c + i * 0x170 + 0x6c, -1000.);
            for (o, x) in [(0x48, -0.1), (0x4c, 0.2), (0x50, 0.25), (0xe0, 0.3)] {
                s.set(w + o, x);
            }
            s.words[(w + 0x128) / 4] = 1;
        }
        let mut v = [0x12345678; VIEW_WORDS];
        for i in 0..4 {
            for base in [0x190, 0x1d0, 0x210] {
                for j in 0..4 {
                    set(&mut v, base + i * 16 + j * 4, 0.);
                }
            }
            set(&mut v, 0x298 + i * 4, 1.);
        }
        let before = s.words;
        let view_before = v;
        let mut queries = 0;
        let mut materials = 0;
        let error = apply(
            &mut s,
            &mut v,
            0.01,
            |_, _| {
                queries += 1;
                Ok(None)
            },
            |_| {
                materials += 1;
                if materials == 5 {
                    Err(Error("authored late material failure"))
                } else {
                    Ok(Some(wheel_history::Material {
                        wear: 1.,
                        speed_grip: [0., 1., 1.],
                        flag_70: true,
                        dirt_build_up: false,
                        token: 1,
                    }))
                }
            },
        )
        .unwrap_err();
        assert_eq!(error.0, "authored late material failure");
        assert_eq!(queries, 4);
        assert_eq!(materials, 5);
        assert_eq!(s.words, before);
        assert_eq!(v, view_before);
    }
}
