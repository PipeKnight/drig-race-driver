//! Ordered response to supplied, already-solved deformation records.
use crate::{Error, spawn::reference_seed};

pub struct Contact {
    pub enabled: bool,
    pub reverse: bool,
    pub baseline: f32,
    pub positive_gate: f32,
    pub normal: [f32; 4],
}

pub struct Record {
    pub direction: [f32; 4],
    pub anchor: [f32; 4],
    pub node: u16,
    pub contact: u16,
    pub weight: f32,
}

pub struct Input<'a> {
    pub nodes: &'a [[f32; 4]],
    pub records: &'a [Record],
    pub contacts: &'a [Contact],
    pub response: Option<&'a [f32]>,
    pub normalization_vector: [f32; 4],
    pub accumulator: [f32; 4],
    pub dirty: bool,
    pub scale: f32,
    pub correction_gain: f32,
    pub subtract_baseline: bool,
}

pub struct Output {
    pub response: Option<Vec<f32>>,
    pub accumulator: [f32; 4],
    pub dirty: bool,
}

fn dot(a: [f32; 4], b: [f32; 4]) -> f32 {
    (a[1] * b[1] + a[0] * b[0]) + (a[3] * b[3] + a[2] * b[2])
}

#[allow(clippy::neg_multiply)] // Original MULPS reverses all lanes before clearing W.
fn normal(contact: &Contact) -> [f32; 4] {
    let mut result = contact.normal;
    if contact.reverse {
        result = result.map(|v| v * -1.);
    }
    result[3] = 0.;
    result
}

#[allow(clippy::assign_op_pattern)] // Preserve the original first-pass ADDSS operand order.
fn mean(input: &Input<'_>, contact: usize, cursor: &mut usize) -> Result<Option<f32>, Error> {
    let start = *cursor;
    let (mut sum, mut weights) = (0., 0.);
    while let Some(record) = input.records.get(*cursor) {
        if usize::from(record.contact) != contact {
            break;
        }
        let position = input.nodes[usize::from(record.node)];
        let delta = std::array::from_fn(|i| position[i] - record.anchor[i]);
        sum = record.weight * dot(record.direction, delta) + sum;
        weights = record.weight + weights;
        *cursor += 1;
    }
    let value = if weights > 0. { sum / weights } else { sum };
    if !weights.is_finite() || !value.is_finite() {
        return Err(Error("nonfinite deformation response mean"));
    }
    Ok((*cursor != start).then_some(value))
}

pub fn calculate(input: Input<'_>) -> Result<Output, Error> {
    if input.nodes.is_empty()
        || input.nodes.len() > 400
        || input.records.len() > 400
        || input.contacts.is_empty()
        || input.contacts.len() > 80
        || input
            .response
            .is_some_and(|r| r.len() != input.contacts.len())
        || input.nodes.iter().flatten().any(|v| !v.is_finite())
        || input.contacts.iter().any(|c| {
            c.normal
                .into_iter()
                .chain([c.baseline, c.positive_gate])
                .any(|v| !v.is_finite())
        })
        || input.records.iter().any(|r| {
            usize::from(r.node) >= input.nodes.len()
                || input
                    .contacts
                    .get(usize::from(r.contact))
                    .is_none_or(|c| !c.enabled || c.positive_gate <= 0.)
                || r.direction
                    .into_iter()
                    .chain(r.anchor)
                    .chain([r.weight])
                    .any(|v| !v.is_finite())
        })
        || input
            .records
            .windows(2)
            .any(|r| r[0].contact > r[1].contact)
        || input
            .normalization_vector
            .into_iter()
            .chain(input.accumulator)
            .chain([input.scale, input.correction_gain])
            .chain(input.response.into_iter().flatten().copied())
            .any(|v| !v.is_finite())
    {
        return Err(Error("invalid deformation response input"));
    }
    let mut output = Output {
        response: input.response.map(<[f32]>::to_vec),
        accumulator: input.accumulator,
        dirty: input.dirty,
    };
    if input.records.is_empty() || input.response.is_none() {
        return Ok(output);
    }
    let squared = dot(input.normalization_vector, input.normalization_vector);
    if !squared.is_finite() {
        return Err(Error("nonfinite deformation response magnitude"));
    }
    let mut unit = [0.; 4];
    if squared > f32::from_bits(0x358637bd) {
        let seed = reference_seed(squared, false)?;
        let factor = (1. - (seed * squared) * seed) * (seed * 0.5) + seed;
        unit = input.normalization_vector.map(|v| factor * v);
    }
    let (mut cursor, mut minimum, mut chosen) = (0, 0., [0.; 4]);
    for (i, contact) in input.contacts.iter().enumerate() {
        if let Some(mean) = mean(&input, i, &mut cursor)? {
            let baseline = if input.subtract_baseline {
                0.
            } else {
                contact.baseline
            };
            let deficit = baseline - mean * input.scale;
            if deficit < 0. {
                let normal = normal(contact);
                let alignment = dot(normal, unit);
                if alignment > 0. {
                    let candidate = alignment * deficit;
                    if minimum > candidate {
                        minimum = candidate;
                        chosen = normal;
                    }
                }
            }
        }
    }
    let scaled = if minimum < 0. {
        let scaled = minimum * input.correction_gain;
        output.accumulator = std::array::from_fn(|i| input.accumulator[i] + scaled * chosen[i]);
        output.dirty = true;
        scaled
    } else {
        0.
    };
    cursor = 0;
    let response = output.response.as_mut().unwrap();
    for (i, contact) in input.contacts.iter().enumerate() {
        if let Some(mean) = mean(&input, i, &mut cursor)? {
            let residual =
                (dot(normal(contact), chosen) * scaled + contact.baseline) - mean * input.scale;
            let reduction = if !input.subtract_baseline && residual >= 0. {
                contact.baseline - residual
            } else {
                contact.baseline
            };
            response[i] -= reduction;
        }
    }
    if output
        .accumulator
        .into_iter()
        .chain(response.iter().copied())
        .any(|v| !v.is_finite())
    {
        return Err(Error("nonfinite deformation response result"));
    }
    Ok(output)
}
