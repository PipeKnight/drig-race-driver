//! Whole selected 9A0C90 for a registered body and a static or dynamic peer
//! (docs/CAR_TO_CAR_DEFORMATION_RESEARCH.md). The enclosing preparation
//! dispatcher and physical scheduling are separate.
use crate::{Error, contact_inverse, deformation_layout::WORDS};
pub const PHYSICAL_WORDS: usize = 0x424 / 4;
#[derive(Clone, Copy)]
pub struct Parameters {
    pub low_mass_threshold: f32,
    pub low_mass_speed_cap: f32,
    pub corner_scale: f32,
    pub velocity_projection: f32,
    pub stiffness_scale: f32,
    pub unlimited: f32,
    pub damage_scale: f32,
    pub table: [f32; 16],
    pub global_disabled: bool,
}
/// The other body of a two-body contact.
#[derive(Clone, Copy)]
pub struct Peer<'a> {
    pub body: &'a [u32; PHYSICAL_WORDS],
    pub center: [f32; 4],
    /// The peer is itself deformation-registered (A14F10 result equals A484).
    pub deformable: bool,
}
/// The owner's role in the contact: `second` when the contact's second body
/// (+08, with +0C == 0) is the owner.
#[derive(Clone, Copy, Default)]
pub struct Side<'a> {
    pub second: bool,
    pub peer: Option<Peer<'a>>,
}
pub struct Output {
    pub descriptor: Vec<u32>,
    pub contact: [u32; 32],
    pub cap: f64,
}
fn f(b: &[u32], at: usize) -> f32 {
    f32::from_bits(b[at / 4])
}
fn vec4(b: &[u32], at: usize) -> [f32; 4] {
    std::array::from_fn(|i| f(b, at + i * 4))
}
fn byte(b: &[u32], at: usize) -> u8 {
    (b[at / 4] >> (at % 4 * 8)) as u8
}
fn dot(a: [f32; 4], b: [f32; 4]) -> f32 {
    (a[1] * b[1] + a[0] * b[0]) + (a[3] * b[3] + a[2] * b[2])
}
fn velocity(body: &[u32], center: [f32; 4], point: [f32; 4]) -> [f32; 4] {
    let d: [f32; 4] = std::array::from_fn(|i| point[i] - center[i]);
    let a = vec4(body, 0x210);
    let l = vec4(body, 0x220);
    [
        l[0] + (a[1] * d[2] - a[2] * d[1]),
        l[1] + (a[2] * d[0] - a[0] * d[2]),
        l[2] + (a[0] * d[1] - a[1] * d[0]),
        l[3] + 0.,
    ]
}
fn lookup(value: f32, p: Parameters) -> f64 {
    let scaled = value * f32::from_bits(0x400ed917);
    if scaled < 0. {
        return f64::from(p.damage_scale) * f64::from(p.table[0]);
    }
    if scaled >= 150. {
        return f64::from(p.table[15]) * f64::from(p.damage_scale);
    }
    let step = f32::from_bits(0x3dcccccd);
    let index = ((scaled * step).floor() as usize).min(14);
    let fraction = (f64::from(scaled) - (index * 10) as f64) * f64::from(step);
    ((1. - fraction) * f64::from(p.table[index]) + f64::from(p.table[index + 1]) * fraction)
        * f64::from(p.damage_scale)
}
/// Pointers are explicit owned roles: first contact body equals this owner,
/// +0C is nonzero, and no second body, shape graph or wake call is followed.
/// Returns the node count. `grid` holds [`validate_grid`]'s verdict once
/// computed: [`calculate_side_in`] never writes the grid words (+A414
/// dimensions, +A420 spacing, +A430 origin, the nodes below +A3B8), so one
/// verdict serves every contact of a descriptor.
pub(crate) fn validate_cached(
    descriptor: &[u32],
    grid: &mut Option<Result<usize, Error>>,
    body: &[u32; PHYSICAL_WORDS],
    center: [f32; 4],
    contact: &[u32; 32],
    p: Parameters,
) -> Result<usize, Error> {
    validate_contact(descriptor, body, center, contact, p)?;
    grid.get_or_insert_with(|| validate_grid(descriptor))
        .clone()
}

fn validate_contact(
    descriptor: &[u32],
    body: &[u32; PHYSICAL_WORDS],
    center: [f32; 4],
    contact: &[u32; 32],
    p: Parameters,
) -> Result<(), Error> {
    if descriptor.len() != WORDS
        || byte(contact, 0xc) > 1
        || contact[1] != 0
        || contact[2] != 0
        || descriptor[0xa4ec / 4] >= 10
        || p.unlimited != f32::MAX
        || [
            p.low_mass_threshold,
            p.low_mass_speed_cap,
            p.corner_scale,
            p.velocity_projection,
            p.stiffness_scale,
            p.damage_scale,
        ]
        .into_iter()
        .chain(p.table)
        .any(|v| !v.is_finite() || v.abs() > 1e6)
        || [0x1d0, 0x1e0, 0x1f0, 0x200, 0x210, 0x220, 0x360]
            .iter()
            .flat_map(|at| vec4(body, *at))
            .chain(center)
            .chain(vec4(contact, 0x50))
            .chain(vec4(contact, 0x60))
            .any(|v| !v.is_finite() || v.abs() > 1e6)
        || !f(body, 0x3f0).is_finite()
        || f(body, 0x3f0) <= 0.
        || f(body, 0x3f0) > 10000.
        || [0xa490, 0xa494, 0xa49c, 0xa420]
            .iter()
            .any(|at| !f(descriptor, *at).is_finite())
        || (0..10).any(|i| !f(descriptor, 0xa4c4 + i * 4).is_finite())
        || (0..4).any(|i| !f(descriptor, 0xa4b4 + i * 4).is_finite())
        || !f(contact, 0x38).is_finite()
    {
        return Err(Error("invalid static deformation contact limit"));
    }
    Ok(())
}

/// The grid and node checks of [`validate_cached`] (after its per-contact checks,
/// which guarantee the descriptor length).
fn validate_grid(descriptor: &[u32]) -> Result<usize, Error> {
    let dimensions: [usize; 3] = std::array::from_fn(|i| descriptor[0xa414 / 4 + i] as usize);
    if dimensions.iter().any(|d| !(2..=16).contains(d))
        || dimensions.iter().product::<usize>() > 400
        || f(descriptor, 0xa420) <= 0.
    {
        return Err(Error("invalid contact limit grid"));
    }
    let count = dimensions.iter().product::<usize>();
    if (0..count).any(|i| {
        vec4(descriptor, 0x2580 + i * 80)
            .into_iter()
            .chain([f(descriptor, 0x25c0 + i * 80)])
            .any(|v| !v.is_finite() || v.abs() > 1e6)
    }) || vec4(descriptor, 0xa430).iter().any(|v| !v.is_finite())
    {
        return Err(Error("invalid contact limit nodes"));
    }
    Ok(count)
}

pub fn calculate(
    descriptor: &[u32],
    body: &[u32; PHYSICAL_WORDS],
    center: [f32; 4],
    contact: &[u32; 32],
    p: Parameters,
) -> Result<Output, Error> {
    calculate_side(descriptor, body, center, contact, p, Side::default())
}

/// Two-body form: relative point velocity and low-mass check use both bodies,
/// the penetration term uses the owner-oriented normal and `own.360 - peer.360`,
/// the coefficient is scaled by A488 (deformable peer, also setting contact
/// +0E) or A48C, and the side-B coefficient slot is +44.
pub fn calculate_side(
    descriptor: &[u32],
    body: &[u32; PHYSICAL_WORDS],
    center: [f32; 4],
    contact: &[u32; 32],
    p: Parameters,
    side: Side<'_>,
) -> Result<Output, Error> {
    let mut descriptor = descriptor.to_vec();
    let (contact, cap) =
        calculate_side_in(&mut descriptor, &mut None, body, center, contact, p, side)?;
    Ok(Output {
        descriptor,
        contact,
        cap,
    })
}

/// [`calculate_side`] on a descriptor updated in place (the per-contact
/// window and level words), returning the contact and the cap; `grid` caches
/// the grid verdict as in [`validate_cached`]. On an error the descriptor may
/// hold this call's window update, so callers discard it.
#[allow(clippy::assign_op_pattern)] // Retain original SSE/x87 operation order.
#[allow(clippy::too_many_arguments)]
pub(crate) fn calculate_side_in(
    descriptor: &mut [u32],
    grid: &mut Option<Result<usize, Error>>,
    body: &[u32; PHYSICAL_WORDS],
    center: [f32; 4],
    contact: &[u32; 32],
    p: Parameters,
    side: Side<'_>,
) -> Result<([u32; 32], f64), Error> {
    let mut tokenless = *contact;
    if side.peer.is_some() || side.second {
        tokenless[1] = 0;
        tokenless[2] = 0;
    }
    let count = validate_cached(descriptor, grid, body, center, &tokenless, p)?;
    if let Some(peer) = side.peer
        && ([0x210, 0x220, 0x360]
            .iter()
            .flat_map(|at| vec4(peer.body, *at))
            .chain(peer.center)
            .any(|v| !v.is_finite() || v.abs() > 1e6)
            || !f(peer.body, 0x3f0).is_finite()
            || [0xa488, 0xa48c]
                .iter()
                .any(|at| !f(descriptor, *at).is_finite()))
    {
        return Err(Error("invalid deformation contact peer"));
    }
    if side.second && side.peer.is_none() {
        return Err(Error("second-body deformation side requires a peer"));
    }
    let slot = if side.second { 0x44 } else { 0x40 };
    let dimensions: [usize; 3] = std::array::from_fn(|i| descriptor[0xa414 / 4 + i] as usize);
    // In place, every read sees what it saw when this wrote a copy: the copy
    // differed only in the window slot and +A490..+A498, read after their
    // writes (+A490 also just before its own).
    let mut out_contact = *contact;
    let mut cap = f64::from(p.unlimited);
    let normal = vec4(contact, 0x60);
    let point = vec4(contact, 0x50);
    let point_velocity = velocity(body, center, point);
    let peer_velocity = side.peer.map(|q| velocity(q.body, q.center, point));
    // |n . (vB - vA)| with A the contact's first body.
    let relative: [f32; 4] = match peer_velocity {
        None => point_velocity.map(|v| 0. - v),
        Some(q) if side.second => std::array::from_fn(|i| point_velocity[i] - q[i]),
        Some(q) => std::array::from_fn(|i| q[i] - point_velocity[i]),
    };
    let speed = dot(normal, relative).abs();
    let low_mass = f(body, 0x3f0) < p.low_mass_threshold
        || side
            .peer
            .is_some_and(|q| f(q.body, 0x3f0) < p.low_mass_threshold);
    let speed = if low_mass && speed > p.low_mass_speed_cap {
        p.low_mass_speed_cap
    } else {
        speed
    };
    let window_at = 0xa4c4 + descriptor[0xa4ec / 4] as usize * 4;
    if speed > f(descriptor, window_at) {
        descriptor[window_at / 4] = speed.to_bits();
    }
    let mut maximum = f(descriptor, 0xa4c4);
    for i in 1..10 {
        let v = f(descriptor, 0xa4c4 + i * 4);
        if maximum < v {
            maximum = v;
        }
    }
    if byte(descriptor, 0xa4b0) == 0 || byte(descriptor, 0xa4af) != 0 || p.global_disabled {
        out_contact[slot / 4] = p.unlimited.to_bits();
        return Ok((out_contact, cap));
    }
    let pose = std::array::from_fn(|i| vec4(body, 0x1d0 + i * 16));
    let inverse = contact_inverse::calculate(pose)?;
    let local: [f32; 4] = std::array::from_fn(|i| {
        (point[3] * inverse[3][i] + point[1] * inverse[1][i])
            + (point[2] * inverse[2][i] + point[0] * inverse[0][i])
    });
    let spacing = f(descriptor, 0xa420);
    let half_x = ((dimensions[0] - 1) as f32 * spacing) * 0.5;
    let half_z = ((dimensions[2] - 1) as f32 * spacing) * 0.5;
    let corner = usize::from(half_x < local[0] - f(descriptor, 0xa430))
        + 2 * usize::from(half_z < local[2] - f(descriptor, 0xa438));
    if maximum < f(descriptor, 0xa4b4 + corner * 4) * p.corner_scale {
        out_contact[slot / 4] = p.unlimited.to_bits();
        return Ok((out_contact, cap));
    }
    let own = vec4(body, 0x360);
    let projected = match side.peer {
        None => dot(normal, own),
        Some(q) => {
            let other = vec4(q.body, 0x360);
            let oriented = if side.second {
                normal.map(|v| -v)
            } else {
                normal
            };
            dot(oriented, std::array::from_fn(|i| own[i] - other[i]))
        }
    } * p.velocity_projection;
    let projected = if projected < 0. { 0. } else { projected };
    let penetration = f(contact, 0x38) + projected;
    let radius = spacing * spacing;
    let mut nearest = None;
    let mut nearest_square = f32::MAX;
    let mut total = 0f32;
    let mut weighted = 0f32;
    for i in 0..count {
        let marker = f(descriptor, 0x25c0 + i * 80);
        if marker == 0. {
            continue;
        }
        let d: [f32; 4] =
            std::array::from_fn(|a| f(descriptor, 0x2580 + i * 80 + a * 4) - local[a]);
        let square = (d[2] * d[2] + d[3] * d[3]) + (d[0] * d[0] + d[1] * d[1]);
        if square < nearest_square {
            nearest_square = square;
            nearest = Some(marker);
        }
        if square < radius {
            let weight = radius - square;
            total = weight + total;
            weighted = marker * weight + weighted;
        }
    }
    let marker = if total > 0. {
        weighted / total
    } else {
        nearest.unwrap_or(0.)
    };
    let stiffness = if marker > 0. {
        (f64::from(f(body, 0x3f0)) / f64::from(marker)) * f64::from(p.stiffness_scale)
    } else {
        f64::from(p.unlimited)
    };
    if stiffness == f64::from(p.unlimited) {
        out_contact[slot / 4] = (stiffness as f32).to_bits();
        cap = stiffness;
        return Ok((out_contact, cap));
    }
    let velocity = match peer_velocity {
        None => dot(normal, point_velocity).abs(),
        Some(q) => dot(normal, std::array::from_fn(|i| point_velocity[i] - q[i])).abs(),
    };
    let level = lookup(velocity, p);
    if f64::from(f(descriptor, 0xa490)) < level {
        descriptor[0xa490 / 4] = (level as f32).to_bits();
        descriptor[0xa498 / 4] = descriptor[0xa49c / 4];
        descriptor[0xa494 / 4] = velocity.to_bits();
    }
    let level = f(descriptor, 0xa490);
    let mut multiplier = if level != 0. {
        (stiffness as f32) / level
    } else {
        p.unlimited
    };
    if let Some(q) = side.peer
        && multiplier != p.unlimited
    {
        multiplier = multiplier * f(descriptor, if q.deformable { 0xa488 } else { 0xa48c });
        if q.deformable {
            // B0CD10(contact, 1): disables the optional correction rows.
            out_contact[0xc / 4] = (out_contact[0xc / 4] & !0xff_0000) | 0x1_0000;
        }
    }
    out_contact[slot / 4] = multiplier.to_bits();
    cap = if multiplier != p.unlimited {
        f64::from(multiplier) * f64::from(penetration)
    } else {
        f64::from(multiplier)
    };
    if !cap.is_finite() || !f(&out_contact, slot).is_finite() {
        return Err(Error("nonfinite static deformation cap"));
    }
    Ok((out_contact, cap))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn disabled_cap_still_updates_window_and_active_cap_uses_marker_mass() {
        let mut d = vec![0; WORDS];
        for at in [0xa414, 0xa418, 0xa41c] {
            d[at / 4] = 2;
        }
        d[0xa420 / 4] = 2f32.to_bits();
        d[0xa4b0 / 4] = 1;
        d[0xa49c / 4] = 1f32.to_bits();
        for (a, v) in [-1f32, -1., -1., 1.].into_iter().enumerate() {
            d[0xa430 / 4 + a] = v.to_bits();
        }
        let mut index = 0;
        for z in 0..2 {
            for y in 0..2 {
                for x in 0..2 {
                    for (a, v) in [
                        x as f32 * 2. - 1.,
                        y as f32 * 2. - 1.,
                        z as f32 * 2. - 1.,
                        1.,
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        d[(0x2580 + index * 80) / 4 + a] = v.to_bits();
                    }
                    d[(0x25c0 + index * 80) / 4] = 2f32.to_bits();
                    index += 1;
                }
            }
        }
        let mut b = [0; PHYSICAL_WORDS];
        for i in 0..4 {
            b[(0x1d0 + i * 20) / 4] = 1f32.to_bits();
        }
        b[0x220 / 4] = (-4f32).to_bits();
        b[0x3f0 / 4] = 1000f32.to_bits();
        let mut c = [0; 32];
        c[0xc / 4] = 1;
        c[0x5c / 4] = 1f32.to_bits();
        c[0x60 / 4] = 1f32.to_bits();
        c[0x38 / 4] = 0.5f32.to_bits();
        c[0x3c / 4] = 0x7fc01234;
        let p = Parameters {
            low_mass_threshold: 100.,
            low_mass_speed_cap: 5.,
            corner_scale: 25.,
            velocity_projection: 1.,
            stiffness_scale: 15.,
            unlimited: f32::MAX,
            damage_scale: 1.,
            table: [1.; 16],
            global_disabled: false,
        };
        let active = calculate(&d, &b, [0., 0., 0., 1.], &c, p).unwrap();
        assert_eq!(active.cap, 3750.); // (1000 / 2) * 15 * 0.5
        assert_eq!(f(&active.contact, 0x40), 7500.);
        assert_eq!(active.contact[0x3c / 4], 0x7fc01234);
        let disabled = calculate(
            &d,
            &b,
            [0., 0., 0., 1.],
            &c,
            Parameters {
                global_disabled: true,
                ..p
            },
        )
        .unwrap();
        assert_eq!(disabled.cap, f64::from(f32::MAX));
        assert_eq!(f(&disabled.descriptor, 0xa4c4), 4.);
        assert_eq!(disabled.descriptor[0xa490 / 4], 0);
        assert_eq!(d[0xa4c4 / 4], 0);
        assert_eq!(c[0x40 / 4], 0);

        // Two-body: a resting deformable peer gives the same speeds, scales the
        // coefficient by A488, sets +0E and uses the side's own slot.
        d[0xa488 / 4] = 0.75f32.to_bits();
        d[0xa48c / 4] = 0.5f32.to_bits();
        let mut peer = b;
        peer[0x220 / 4] = 0;
        c[0x8 / 4] = 2;
        c[1] = 1;
        c[0xc / 4] = 0x0100_0000;
        for (second, slot, other) in [(false, 0x40, 0x44), (true, 0x44, 0x40)] {
            let side = Side {
                second,
                peer: Some(Peer {
                    body: &peer,
                    center: [0., 0., 0., 1.],
                    deformable: true,
                }),
            };
            let r = calculate_side(&d, &b, [0., 0., 0., 1.], &c, p, side).unwrap();
            assert_eq!(f(&r.contact, slot), 5625.);
            assert_eq!(r.contact[other / 4], c[other / 4]);
            assert_eq!(r.cap, 2812.5);
            assert_eq!(byte(&r.contact, 0xe), 1);
        }
        let dynamic = Side {
            second: false,
            peer: Some(Peer {
                body: &peer,
                center: [0., 0., 0., 1.],
                deformable: false,
            }),
        };
        let r = calculate_side(&d, &b, [0., 0., 0., 1.], &c, p, dynamic).unwrap();
        assert_eq!(f(&r.contact, 0x40), 3750.);
        assert_eq!(byte(&r.contact, 0xe), 0);
        // A light peer caps the relative speed for the window.
        let mut light = peer;
        light[0x3f0 / 4] = 50f32.to_bits();
        let light_side = Side {
            second: false,
            peer: Some(Peer {
                body: &light,
                center: [0., 0., 0., 1.],
                deformable: false,
            }),
        };
        let mut fast = b;
        fast[0x220 / 4] = (-40f32).to_bits();
        let r = calculate_side(&d, &fast, [0., 0., 0., 1.], &c, p, light_side).unwrap();
        assert_eq!(f(&r.descriptor, 0xa4c4), 5.);
    }
}
