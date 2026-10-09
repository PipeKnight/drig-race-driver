//! Ordered contact-to-grid record generation. Solver and collision response are separate.
use crate::{Error, spawn::reference_seed};

#[derive(Clone, Copy)]
pub struct Node {
    pub position: [f32; 4],
    pub marker: f32,
}

#[derive(Clone, Copy)]
pub struct Contact {
    pub enabled: bool,
    pub reverse: bool,
    pub point: [f32; 4],
    pub direction: [f32; 4],
    pub value_38: f32,
    pub value_3c: f32,
    pub value_40: f32,
    pub value_44: f32,
    pub counterpart_velocity: Option<[f32; 4]>,
}

pub struct Input<'a> {
    pub dimensions: [usize; 3],
    pub spacing: f32,
    pub minimum: [f32; 4],
    pub pose: [[f32; 4]; 4],
    pub body_offset: [f32; 4],
    pub body_b20: [f32; 4],
    pub velocity: [f32; 4],
    pub supplied_scalar: f32,
    pub state: f32,
    pub corners: [f32; 4],
    pub window: [f32; 10],
    pub corner_scale: f32,
    pub static_mass: f32,
    pub limit_scale: f32,
    pub flag_radius_scale: f32,
    pub record_radius_scale: f32,
    pub radial_scale: f32,
    pub damage_threshold: f32,
    pub damage_gain: f32,
    pub projection: f32,
    pub flags: &'a [u8],
    pub dirty: [u8; 5],
    pub nodes: &'a [Node],
    pub contacts: &'a [Contact],
}

pub struct Record {
    pub direction: [f32; 4],
    pub anchor: [f32; 4],
    pub node: u16,
    pub contact: u16,
    pub scalar: f32,
    pub weight: f32,
}

pub struct Output {
    pub records: Vec<Record>,
    pub flags: Vec<u8>,
    pub dirty: [u8; 5],
    pub state: f32,
}

fn dot(a: [f32; 4], b: [f32; 4]) -> f32 {
    (a[1] * b[1] + a[0] * b[0]) + (a[3] * b[3] + a[2] * b[2])
}

fn transform(m: [[f32; 4]; 4], v: [f32; 4]) -> [f32; 4] {
    std::array::from_fn(|i| (v[2] * m[2][i] + v[0] * m[0][i]) + (v[3] * m[3][i] + v[1] * m[1][i]))
}

fn root(value: f32) -> Result<f32, Error> {
    if value == 0. {
        return Ok(0.);
    }
    let seed = reference_seed(value, false)?;
    let product = seed * value;
    Ok((1. - product * seed) * (product * 0.5) + product)
}

#[allow(clippy::assign_op_pattern)] // Retain original SSE multiply/subtract ordering.
#[allow(clippy::neg_multiply)] // The original reverses all lanes with MULPS by -1.
pub fn calculate(input: Input<'_>) -> Result<Output, Error> {
    if input.dimensions.iter().any(|d| !(2..=16).contains(d))
        || input.dimensions.iter().product::<usize>() > 400
        || input.nodes.len() != 400
        || input.flags.len() != 400
        || input.contacts.is_empty()
        || input.contacts.len() > 80
        || input.dirty.iter().any(|v| *v > 1)
        || input.spacing <= 0.
        || input.record_radius_scale <= 0.
        || input.radial_scale <= 0.
        || input.static_mass <= 0.
        || [
            input.spacing,
            input.supplied_scalar,
            input.state,
            input.corner_scale,
            input.static_mass,
            input.limit_scale,
            input.flag_radius_scale,
            input.record_radius_scale,
            input.radial_scale,
            input.damage_threshold,
            input.damage_gain,
            input.projection,
        ]
        .into_iter()
        .chain(input.minimum)
        .chain(input.pose.into_iter().flatten())
        .chain(input.body_offset)
        .chain(input.body_b20)
        .chain(input.velocity)
        .chain(input.corners)
        .chain(input.window)
        .any(|v| !v.is_finite())
        || input.nodes.iter().any(|n| {
            n.position
                .into_iter()
                .chain([n.marker])
                .any(|v| !v.is_finite())
        })
        || input.contacts.iter().any(|c| {
            c.point
                .into_iter()
                .chain(c.direction)
                .chain([c.value_38, c.value_3c, c.value_40, c.value_44])
                .chain(c.counterpart_velocity.into_iter().flatten())
                .any(|v| !v.is_finite())
                || (c.reverse && c.counterpart_velocity.is_none())
                || (c.enabled
                    && c.value_3c > 0.
                    && (if c.reverse {
                        c.value_44 <= 0. || c.value_40 < 0.
                    } else {
                        c.value_40 <= 0. || (c.counterpart_velocity.is_some() && c.value_44 < 0.)
                    }))
        })
    {
        return Err(Error("invalid contact-to-record input"));
    }
    let p = input.pose;
    let b = input.body_offset;
    let center: [f32; 4] = std::array::from_fn(|i| {
        ((b[1] * p[1][i] + b[3] * p[3][i]) + (b[0] * p[0][i] + b[2] * p[2][i])) + p[3][i]
    });
    let mut inverse = [[0.; 4]; 4];
    for (axis, column) in inverse.iter_mut().take(3).enumerate() {
        for (i, value) in column.iter_mut().take(3).enumerate() {
            *value = p[i][axis];
        }
    }
    inverse[3] = std::array::from_fn(|i| {
        if i == 3 {
            1.
        } else {
            ((-center[1]) * inverse[1][i] + (-center[0]) * inverse[0][i])
                + (-center[2]) * inverse[2][i]
        }
    });
    let spacing_squared = input.spacing * input.spacing;
    let flag_radius = input.flag_radius_scale * spacing_squared;
    let record_radius = input.record_radius_scale * spacing_squared;
    let inverse_radius = 1. / (input.radial_scale * record_radius);
    let mut maximum = input.window[0];
    for value in &input.window[1..] {
        if maximum < *value {
            maximum = *value;
        }
    }
    let count = input.dimensions.iter().product::<usize>();
    let mut result = Output {
        records: Vec::new(),
        flags: input.flags.to_vec(),
        dirty: input.dirty,
        state: input.state,
    };
    for (ordinal, contact) in input.contacts.iter().enumerate() {
        if !contact.enabled || contact.value_3c <= 0. {
            continue;
        }
        let point = transform(inverse, contact.point);
        let mut direction = transform(inverse, contact.direction);
        let mut world_direction = contact.direction;
        let x = (input.dimensions[0] - 1) as f32 * input.spacing * 0.5;
        let z = (input.dimensions[2] - 1) as f32 * input.spacing * 0.5;
        let corner = usize::from(x < point[0] - input.minimum[0])
            + 2 * usize::from(z < point[2] - input.minimum[2]);
        let threshold = input.corners[corner] * input.corner_scale;
        let mut relative = input.velocity;
        if let Some(other) = contact.counterpart_velocity {
            relative = std::array::from_fn(|i| relative[i] - other[i]);
        }
        let (mass, other_mass) = if contact.reverse {
            direction = direction.map(|v| -1. * v);
            world_direction = world_direction.map(|v| -1. * v);
            (contact.value_44, contact.value_40)
        } else {
            (
                contact.value_40,
                if contact.counterpart_velocity.is_some() {
                    contact.value_44
                } else {
                    input.static_mass
                },
            )
        };
        for axis in [0, 2] {
            if (point[axis] < -0.3 && direction[axis] < 0.)
                || (point[axis] > 0.3 && direction[axis] > 0.)
            {
                direction[axis] = 0.;
            }
        }
        if direction[1] > 0. {
            direction[1] = 0.;
        }
        let speed = dot(world_direction, relative);
        let available = contact.value_38 + if speed < 0. { 0. } else { speed };
        let mut limit = contact.value_3c / mass;
        let total = contact.value_3c / other_mass + limit;
        if available * input.limit_scale < total {
            limit = (available / total) * input.limit_scale * limit;
        }
        let projection = dot(world_direction, input.body_b20);
        if input.state == -1. {
            if projection < 0. {
                let mut value = -0.0 - available / projection;
                if value < 0. {
                    value = 0.;
                } else if input.supplied_scalar < value {
                    value = input.supplied_scalar;
                }
                if result.state < value {
                    result.state = value;
                }
            }
        } else {
            result.state = 2.;
        }
        let scale = 1. - input.projection;
        let projected: [f32; 4] =
            std::array::from_fn(|i| input.body_b20[i] - (projection * world_direction[i]) * scale);
        let magnitude = root(dot(projected, projected))?;
        for (node_index, node) in input.nodes[..count].iter().enumerate() {
            if node.marker == 0. {
                continue;
            }
            let mut delta: [f32; 4] = std::array::from_fn(|i| node.position[i] - point[i]);
            delta[3] = 0.;
            let depth = dot(direction, delta);
            let capped = if limit < depth { limit } else { depth };
            let residual: [f32; 4] = std::array::from_fn(|i| delta[i] - capped * direction[i]);
            let radial = dot(residual, residual);
            if radial < record_radius && threshold <= maximum {
                if result.records.len() >= 400 {
                    break;
                }
                let nonnegative = if capped < 0. { 0. } else { capped };
                let value = limit - nonnegative;
                let scalar = if value < limit { value } else { limit };
                let weight = (2. - radial * inverse_radius).clamp(0., 1.);
                result.records.push(Record {
                    direction,
                    anchor: node.position,
                    node: node_index as u16,
                    contact: ordinal as u16,
                    scalar: scalar * weight,
                    weight,
                });
            }
            if radial < flag_radius && magnitude > input.damage_threshold && capped < 0.1 {
                let decrease = (input.damage_gain * magnitude) * 255.;
                if !decrease.is_finite() || decrease < -2147483648. || decrease >= 2147483648. {
                    return Err(Error("damage conversion outside bounded i32 domain"));
                }
                let next = i32::from(result.flags[node_index])
                    .wrapping_sub(decrease as i32)
                    .max(0) as u8;
                if next != result.flags[node_index] {
                    result.flags[node_index] = next;
                    result.dirty = [1; 5];
                }
            }
        }
    }
    if !result.state.is_finite()
        || result.records.iter().any(|r| {
            r.direction
                .into_iter()
                .chain(r.anchor)
                .chain([r.scalar, r.weight])
                .any(|v| !v.is_finite())
        })
    {
        return Err(Error("nonfinite contact-generation result"));
    }
    Ok(result)
}
