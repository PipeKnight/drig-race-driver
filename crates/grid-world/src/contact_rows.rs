//! Initial impact constraints from supplied contacts and body/material operands.
//! Geometric contact production and the subsequent response solver are external.
use crate::{Error, spawn::reference_seed};

pub type Row = [u32; 48];
pub type Material = [f32; 13];

#[derive(Clone)]
pub struct Body {
    pub position: [f32; 4],
    pub angular: [f32; 4],
    pub linear: [f32; 4],
    pub scale_angular: bool,
    pub directional_friction: bool,
}

#[derive(Clone)]
pub struct Contact {
    pub two_bodies: bool,
    pub bodies: [usize; 2],
    pub materials: [usize; 2],
    /// None means no shape; Some is its angular-scaling permission.
    pub shapes: [Option<bool>; 2],
    pub special: bool,
    pub group_end: bool,
    /// Retained for the downstream solver; construction does not test it.
    pub disabled: bool,
    pub accumulated: f32,
    pub previous_impulse: f32,
    pub point: [f32; 4],
    pub normal: [f32; 4],
}

#[derive(Clone)]
pub struct Input {
    /// Inverse time, fallback bias, threshold, compliance scale, angular scale,
    /// and angular normal-Y threshold. Last two are mutable reference globals.
    pub parameters: [f32; 6],
    pub bodies: Vec<Body>,
    pub materials: Vec<Material>,
    pub contacts: Vec<Contact>,
    /// Single normals, single tangents, two-body normals, two-body tangents.
    /// Capacities are N, 3N, N, 3N where N is the contact count (at least one).
    pub rows: [Vec<Row>; 4],
    pub cached_pair: Option<[usize; 2]>,
    pub cached_material: Material,
}

pub struct Output {
    pub rows: [Vec<Row>; 4],
    /// Counts in each unfinished body batch; final batches are not flushed here.
    pub batch_counts: [u32; 4],
    pub normal_count: u32,
    pub cursors: [usize; 4],
    pub heads: [Option<usize>; 4],
    pub point_sum: [f32; 4],
    pub normal_sum: [f32; 4],
    pub friction_sum: f32,
    pub group_count: u32,
    pub cached_pair: Option<[usize; 2]>,
    pub cached_material: Material,
}

fn dot(a: [f32; 4], b: [f32; 4]) -> f32 {
    (a[0] * b[0] + a[1] * b[1]) + (a[2] * b[2] + a[3] * b[3])
}
fn cross(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
        0.,
    ]
}
fn difference(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    std::array::from_fn(|i| a[i] - b[i])
}
fn length(v: [f32; 4]) -> Result<f32, Error> {
    let squared = dot(v, v);
    if squared == 0. {
        return Ok(0.);
    }
    if !squared.is_finite() || squared < f32::MIN_POSITIVE {
        return Err(Error("unsupported contact vector length"));
    }
    let seed = reference_seed(squared, false)?;
    let root = seed * squared;
    Ok((1. - root * seed) * (root * 0.5) + root)
}
fn vector(row: &mut Row, offset: usize, v: [f32; 4]) {
    row[offset..offset + 4].copy_from_slice(&v.map(f32::to_bits));
}
fn mixed(input: &Input, output: &mut Output, pair: [usize; 2]) -> Result<Material, Error> {
    let a = input.materials[pair[0]];
    let b = input.materials[pair[1]];
    if pair[0] == pair[1] {
        return Ok(a);
    }
    if output.cached_pair == Some(pair) || output.cached_pair == Some([pair[1], pair[0]]) {
        return Ok(output.cached_material);
    }
    output.cached_pair = Some(pair);
    for i in 1..=10 {
        if i >= 6 && !(a[6] > 0. || a[7] > 0. || b[6] > 0. || b[7] > 0.) {
            continue;
        }
        let v = if a[i] == b[i] {
            a[i]
        } else if i >= 6 && a[i] == 0. {
            b[i]
        } else if i >= 6 && b[i] == 0. {
            a[i]
        } else {
            (b[i] * a[i]).sqrt()
        };
        if !v.is_finite() {
            return Err(Error("invalid material mixture"));
        }
        output.cached_material[i] = v;
    }
    output.cached_material[11] = 0.;
    if a[11] > 0. {
        output.cached_material[11..13].copy_from_slice(&a[11..13]);
    } else if b[11] > 0. {
        output.cached_material[11..13].copy_from_slice(&b[11..13]);
    }
    Ok(output.cached_material)
}
fn take(output: &mut Output, pool: usize, contact: &Contact) -> usize {
    let index = output.cursors[pool];
    if let Some(head) = output.heads[pool] {
        let row = output.rows[pool][head];
        if row[40] != contact.bodies[0] as u32
            || (contact.two_bodies && row[41] != contact.bodies[1] as u32)
        {
            output.rows[pool][head][45] = output.batch_counts[pool];
            output.batch_counts[pool] = 0;
            output.heads[pool] = Some(index);
        }
    } else {
        output.heads[pool] = Some(index);
    }
    output.cursors[pool] += 1;
    output.batch_counts[pool] += 1;
    index
}
fn angular(input: &Input, c: &Contact, body: usize, v: [f32; 4], y: f32) -> [f32; 4] {
    if input.bodies[c.bodies[body]].scale_angular
        && c.shapes[body] == Some(true)
        && y.abs() < input.parameters[5]
    {
        v.map(|x| input.parameters[4] * x)
    } else {
        v
    }
}

/// Build complete normal and grouped friction rows, preserving untouched words.
/// References at row+A8 are pool-relative tokens: (normal pool+1)<<24 | byte offset.
pub fn calculate(input: &Input) -> Result<Output, Error> {
    let n = input.contacts.len().max(1);
    if input.contacts.len() > crate::MAX_WORLD_CONTACTS
        || input.bodies.is_empty()
        || input.bodies.len() > crate::MAX_WORLD_BODIES
        || input.materials.is_empty()
        || input.materials.len() > 8
        || input
            .rows
            .iter()
            .enumerate()
            .any(|(p, r)| r.len() != n * if p % 2 == 0 { 1 } else { 3 })
        || input
            .parameters
            .iter()
            .chain(&input.cached_material)
            .chain(input.materials.iter().flatten())
            .chain(
                input
                    .bodies
                    .iter()
                    .flat_map(|b| b.position.iter().chain(&b.angular).chain(&b.linear)),
            )
            .any(|v| !v.is_finite())
        || input.parameters[0] <= 0.
        || input
            .cached_pair
            .is_some_and(|pair| pair.iter().any(|&i| i >= input.materials.len()))
    {
        return Err(Error("invalid contact row input"));
    }
    for c in &input.contacts {
        if c.bodies.iter().any(|&i| i >= input.bodies.len())
            || c.materials.iter().any(|&i| i >= input.materials.len())
            || c.point
                .iter()
                .chain(&c.normal)
                .chain([c.accumulated, c.previous_impulse].iter())
                .any(|v| !v.is_finite())
        {
            return Err(Error("invalid contact row operand"));
        }
    }
    let mut output = Output {
        rows: input.rows.clone(),
        batch_counts: [0; 4],
        normal_count: 0,
        cursors: [0; 4],
        heads: [None; 4],
        point_sum: [0.; 4],
        normal_sum: [0.; 4],
        friction_sum: 0.,
        group_count: 0,
        cached_pair: input.cached_pair,
        cached_material: input.cached_material,
    };
    let [time, fallback, threshold, scale, _, _] = input.parameters;
    for c in &input.contacts {
        let p = if c.two_bodies { 2 } else { 0 };
        let index = take(&mut output, p, c);
        output.normal_count += 1;
        let m = mixed(input, &mut output, c.materials)?;
        let a = &input.bodies[c.bodies[0]];
        let b = &input.bodies[c.bodies[1]];
        let (bias_factor, softness) = if c.special {
            (fallback, m[3])
        } else {
            let speed = length(if c.two_bodies {
                difference(a.linear, b.linear)
            } else {
                a.linear
            })?;
            let (stiffness, damping) = if speed < m[10] {
                let fraction = speed / m[10];
                (
                    (m[8] - m[6]) * fraction + m[6],
                    (m[9] - m[7]) * fraction + m[7],
                )
            } else {
                (m[8], m[9])
            };
            let stiffness = scale * stiffness;
            let damping = scale * damping;
            if stiffness != f32::MAX && damping != f32::MAX && (stiffness != 0. || damping != 0.) {
                let stiffness = (1. / time) * stiffness;
                let inverse = 1. / (stiffness + damping);
                (inverse * stiffness, inverse)
            } else {
                (fallback, m[3])
            }
        };
        let angular_a = angular(
            input,
            c,
            0,
            cross(difference(c.point, a.position), c.normal),
            c.normal[1],
        );
        let angular_b = angular(
            input,
            c,
            1,
            cross(c.normal, difference(c.point, b.position)),
            c.normal[1],
        );
        let row = &mut output.rows[p][index];
        vector(row, 0, c.normal);
        vector(row, 4, angular_a);
        row[40] = c.bodies[0] as u32;
        row[37] = 0;
        if c.two_bodies {
            vector(row, 8, c.normal.map(|v| -v));
            vector(row, 12, angular_b);
            row[41] = c.bodies[1] as u32;
        }
        let mut delta = c.accumulated - threshold;
        if delta < 0. {
            delta = 0.;
        }
        let mut bias = (time * bias_factor) * delta;
        if c.special && bias > m[4] {
            bias = m[4];
        }
        if softness > 0. {
            row[33] = softness.to_bits();
        }
        if m[5] >= 0. && m[1] > 0. {
            let mut projection = dot(a.angular, angular_a) + dot(a.linear, c.normal);
            if c.two_bodies {
                projection -= dot(b.linear, c.normal);
                projection += dot(b.angular, angular_b);
            }
            if m[5] < 0. - projection {
                let bounce = 0. - m[1] * projection;
                if bounce > bias {
                    bias = bounce;
                }
            }
        }
        row[32] = bias.to_bits();
        row[34] = 0;
        let mut upper = if c.previous_impulse == f32::MAX {
            f32::MAX
        } else {
            time * c.previous_impulse
        };
        if upper != f32::MAX {
            upper *= scale;
        }
        row[35] = upper.to_bits();
        for i in 0..4 {
            output.point_sum[i] += c.point[i];
            output.normal_sum[i] += c.normal[i];
        }
        output.friction_sum += m[2];
        output.group_count += 1;
        if !c.group_end {
            continue;
        }
        let reciprocal = 1. / output.group_count as f32;
        let point = output.point_sum.map(|v| reciprocal * v);
        // The reference retains the x87 reciprocal for this one product.
        let friction = (f64::from(output.friction_sum) / f64::from(output.group_count)) as f32;
        let size = length(output.normal_sum)?;
        let normal = if size <= f32::from_bits(0x38d1_b717) {
            [0., 1., 0., 0.]
        } else {
            output.normal_sum.map(|v| (1. / size) * v)
        };
        let use_y = normal[1].abs() <= f32::from_bits(0x3f34_fdf4);
        let tangent = cross(
            if use_y {
                [0., 1., 0., 0.]
            } else {
                [1., 0., 0., 0.]
            },
            normal,
        );
        let squared = dot(tangent, tangent);
        if !squared.is_finite() || squared < f32::MIN_POSITIVE {
            return Err(Error("degenerate contact tangent"));
        }
        let seed = reference_seed(squared, false)?;
        let inverse = (1. - (squared * seed) * seed) * (seed * 0.5) + seed;
        let tangent = tangent.map(|v| inverse * v);
        let second = cross(tangent, normal);
        for (t, direction) in [normal, tangent, second].into_iter().enumerate() {
            let ti = if t == 0 {
                take(&mut output, p + 1, c)
            } else {
                let ti = output.cursors[p + 1];
                output.cursors[p + 1] += 1;
                output.batch_counts[p + 1] += 1;
                ti
            };
            let row = &mut output.rows[p + 1][ti];
            vector(row, 0, if t == 0 { [0.; 4] } else { direction });
            vector(
                row,
                4,
                if t == 0 {
                    normal
                } else {
                    angular(
                        input,
                        c,
                        0,
                        cross(difference(point, a.position), direction),
                        normal[1],
                    )
                },
            );
            row[40] = c.bodies[0] as u32;
            if c.two_bodies {
                vector(
                    row,
                    8,
                    if t == 0 {
                        [0.; 4]
                    } else {
                        direction.map(|v| -v)
                    },
                );
                vector(
                    row,
                    12,
                    if t == 0 {
                        normal.map(|v| -v)
                    } else {
                        angular(
                            input,
                            c,
                            1,
                            cross(direction, difference(point, b.position)),
                            normal[1],
                        )
                    },
                );
                row[41] = c.bodies[1] as u32;
            }
            row[37] = 0;
            row[34] = (-friction).to_bits();
            row[35] = friction.to_bits();
            row[42] = ((p as u32 + 1) << 24) | (index as u32 * 192 + 0x98);
            if t == 2 && !c.two_bodies && use_y && a.directional_friction {
                if second[1] < 0. {
                    row[34] = 0;
                    row[35] = (friction * 2.).to_bits();
                } else {
                    row[34] = (-friction * 2.).to_bits();
                    row[35] = 0;
                }
            }
        }
        output.point_sum = [0.; 4];
        output.normal_sum = [0.; 4];
        output.friction_sum = 0.;
        output.group_count = 0;
    }
    // Reject overflow in produced arithmetic while preserving arbitrary opaque words.
    for p in 0..4 {
        for row in &output.rows[p][..output.cursors[p]] {
            if row[..if p >= 2 { 16 } else { 8 }]
                .iter()
                .chain(&row[if p % 2 == 0 { 32 } else { 34 }..36])
                .any(|&v| !f32::from_bits(v).is_finite())
            {
                return Err(Error("nonfinite constructed row"));
            }
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input() -> Input {
        Input {
            parameters: [60., 0.2, 0.125, 1., 0.2, 0.2],
            bodies: vec![Body {
                position: [0., 0., 0., 1.],
                angular: [0.; 4],
                linear: [0., -10., 0., 0.],
                scale_angular: false,
                directional_friction: false,
            }],
            materials: vec![[0., 0.5, 0., 0.01, 100., 0., 0., 0., 0., 0., 100., 0., 0.]],
            contacts: vec![Contact {
                two_bodies: false,
                bodies: [0, 0],
                materials: [0, 0],
                shapes: [None; 2],
                special: false,
                group_end: true,
                disabled: false,
                accumulated: 0.125,
                previous_impulse: f32::MAX,
                point: [0., 0., 0., 1.],
                normal: [0., 1., 0., 0.],
            }],
            rows: std::array::from_fn(|p| vec![[0x3e00_0000; 48]; if p % 2 == 0 { 1 } else { 3 }]),
            cached_pair: None,
            cached_material: [0.; 13],
        }
    }
    #[test]
    fn restitution_group_rows_negative_zero_and_opaque_response_survive() {
        let i = input();
        let o = calculate(&i).unwrap();
        assert_eq!(f32::from_bits(o.rows[0][0][32]), 5.);
        assert_eq!(o.cursors, [1, 3, 0, 0]);
        for r in &o.rows[1] {
            assert_eq!(r[34], (-0.0f32).to_bits());
            assert_eq!(r[35], 0);
            assert_eq!(r[42], 0x0100_0098);
            assert_eq!(&r[16..32], &i.rows[1][0][16..32]);
        }
        assert_eq!(o.rows[0][0][45], i.rows[0][0][45]);
        assert_eq!(o.group_count, 0);
    }
    #[test]
    fn disabled_flag_does_not_disable_initial_construction() {
        let i = input();
        let normal = calculate(&i).unwrap();
        let mut disabled = i.clone();
        disabled.contacts[0].disabled = true;
        assert_eq!(calculate(&disabled).unwrap().rows, normal.rows);
    }
    #[test]
    fn open_group_preserves_sums_without_tangents() {
        let mut i = input();
        i.contacts[0].group_end = false;
        let o = calculate(&i).unwrap();
        assert_eq!(o.cursors, [1, 0, 0, 0]);
        assert_eq!(o.point_sum, i.contacts[0].point);
        assert_eq!(o.normal_sum, i.contacts[0].normal);
        assert_eq!(o.group_count, 1);
        assert_eq!(o.rows[1], i.rows[1]);
    }
    #[test]
    fn reversed_cached_pair_retains_cached_material_and_softness_fallback() {
        let mut i = input();
        i.materials.push(i.materials[0]);
        i.contacts[0].materials = [1, 0];
        i.cached_pair = Some([0, 1]);
        i.cached_material[3] = 0.;
        i.cached_material[5] = -1.;
        let o = calculate(&i).unwrap();
        assert_eq!(o.cached_pair, i.cached_pair);
        assert_eq!(o.cached_material, i.cached_material);
        assert_eq!(o.rows[0][0][33], i.rows[0][0][33]);
        assert_eq!(o.rows[0][0][32], 0);
    }
    #[test]
    fn body_change_flushes_previous_batch_only() {
        let mut i = input();
        i.bodies.push(i.bodies[0].clone());
        let mut next = i.contacts[0].clone();
        next.bodies = [1, 0];
        i.contacts.push(next);
        i.rows = std::array::from_fn(|p| vec![[0x3e00_0000; 48]; if p % 2 == 0 { 2 } else { 6 }]);
        let o = calculate(&i).unwrap();
        assert_eq!(o.rows[0][0][45], 1);
        assert_eq!(o.rows[1][0][45], 3);
        assert_eq!(o.rows[0][1][45], i.rows[0][1][45]);
        assert_eq!(o.rows[1][3][45], i.rows[1][3][45]);
        assert_eq!(o.heads, [Some(1), Some(3), None, None]);
        assert_eq!(o.batch_counts, [1, 3, 0, 0]);
    }
}
