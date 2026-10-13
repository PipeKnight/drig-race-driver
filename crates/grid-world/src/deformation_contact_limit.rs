//! Whole selected 9A0C90 for a registered first body and static counterpart.
//! The enclosing preparation dispatcher and physical scheduling are separate.
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
#[allow(clippy::assign_op_pattern)] // Retain original SSE/x87 operation order.
pub fn calculate(
    descriptor: &[u32],
    body: &[u32; PHYSICAL_WORDS],
    center: [f32; 4],
    contact: &[u32; 32],
    p: Parameters,
) -> Result<Output, Error> {
    if descriptor.len() != WORDS
        || byte(contact, 0xc) == 0
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
    let mut out = Output {
        descriptor: descriptor.to_vec(),
        contact: *contact,
        cap: f64::from(p.unlimited),
    };
    let normal = vec4(contact, 0x60);
    let point = vec4(contact, 0x50);
    let point_velocity = velocity(body, center, point);
    let relative: [f32; 4] = point_velocity.map(|v| 0. - v);
    let speed = dot(normal, relative).abs();
    let speed = if f(body, 0x3f0) < p.low_mass_threshold && speed > p.low_mass_speed_cap {
        p.low_mass_speed_cap
    } else {
        speed
    };
    let window_at = 0xa4c4 + descriptor[0xa4ec / 4] as usize * 4;
    if speed > f(&out.descriptor, window_at) {
        out.descriptor[window_at / 4] = speed.to_bits();
    }
    let mut maximum = f(&out.descriptor, 0xa4c4);
    for i in 1..10 {
        let v = f(&out.descriptor, 0xa4c4 + i * 4);
        if maximum < v {
            maximum = v;
        }
    }
    if byte(descriptor, 0xa4b0) == 0 || byte(descriptor, 0xa4af) != 0 || p.global_disabled {
        out.contact[0x40 / 4] = p.unlimited.to_bits();
        return Ok(out);
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
        out.contact[0x40 / 4] = p.unlimited.to_bits();
        return Ok(out);
    }
    let projected = dot(normal, vec4(body, 0x360)) * p.velocity_projection;
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
        out.contact[0x40 / 4] = (stiffness as f32).to_bits();
        out.cap = stiffness;
        return Ok(out);
    }
    let velocity = dot(normal, point_velocity).abs();
    let level = lookup(velocity, p);
    if f64::from(f(descriptor, 0xa490)) < level {
        out.descriptor[0xa490 / 4] = (level as f32).to_bits();
        out.descriptor[0xa498 / 4] = descriptor[0xa49c / 4];
        out.descriptor[0xa494 / 4] = velocity.to_bits();
    }
    let level = f(&out.descriptor, 0xa490);
    let multiplier = if level != 0. {
        (stiffness as f32) / level
    } else {
        p.unlimited
    };
    out.contact[0x40 / 4] = multiplier.to_bits();
    out.cap = if multiplier != p.unlimited {
        f64::from(multiplier) * f64::from(penetration)
    } else {
        f64::from(multiplier)
    };
    if !out.cap.is_finite() || !f(&out.contact, 0x40).is_finite() {
        return Err(Error("nonfinite static deformation cap"));
    }
    Ok(out)
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
    }
}
