//! Supplied normal constraints: optional correction normalization and iteration.
use crate::Error;

#[derive(Clone, Debug)]
pub struct Row {
    pub jacobian: [[f32; 4]; 4],
    pub response: [[f32; 4]; 4],
    pub softness_input: f32,
    pub lower: f32,
    pub upper: f32,
    pub bias: f32,
    pub impulse: f32,
    pub cumulative: f32,
    pub softness: f32,
    pub bodies: [usize; 2],
    pub normalization: f32,
    pub denominator: f32,
    /// Only the first row in each contiguous batch supplies its length.
    pub batch_len: usize,
}

pub struct Input {
    pub rows: Vec<Row>,
    pub two_body_rows: usize,
    pub body_count: usize,
    pub iterations: usize,
    pub normalize: bool,
    pub relaxation: f32,
}

pub struct Output {
    pub rows: Vec<Row>,
    /// Linear four lanes followed by angular four lanes per supplied body.
    pub bodies: Vec<[[f32; 4]; 2]>,
}

fn finite(row: &Row) -> bool {
    row.jacobian
        .iter()
        .chain(&row.response)
        .flatten()
        .copied()
        .chain([
            row.softness_input,
            row.lower,
            row.upper,
            row.bias,
            row.impulse,
            row.cumulative,
            row.softness,
            row.normalization,
            row.denominator,
        ])
        .all(f32::is_finite)
}

fn validate(input: &Input) -> Result<(), Error> {
    if input.rows.len() > 256
        || input.two_body_rows > input.rows.len()
        || input.body_count > 64
        || input.iterations > 32
        || !input.relaxation.is_finite()
        || input.rows.iter().any(|r| !finite(r) || r.lower > r.upper)
    {
        return Err(Error("invalid physical response input"));
    }
    for (start, end, two) in [
        (0, input.two_body_rows, true),
        (input.two_body_rows, input.rows.len(), false),
    ] {
        let mut cursor = start;
        while cursor < end {
            let row = &input.rows[cursor];
            let next = cursor
                .checked_add(row.batch_len)
                .ok_or(Error("batch overflow"))?;
            if row.batch_len == 0
                || next > end
                || row.bodies[0] >= input.body_count
                || (two && row.bodies[1] >= input.body_count)
                || input.rows[cursor..next]
                    .iter()
                    .any(|r| r.bodies[0] != row.bodies[0] || (two && r.bodies[1] != row.bodies[1]))
            {
                return Err(Error("invalid physical response batch"));
            }
            cursor = next;
        }
    }
    Ok(())
}

fn dot(a: [f32; 4], b: [f32; 4]) -> f32 {
    // Original SSE rotates adjacent products, then adds the two pairs.
    (a[3] * b[3] + a[2] * b[2]) + (a[1] * b[1] + a[0] * b[0])
}

#[allow(clippy::assign_op_pattern)] // Keep increment * response + cached value order.
pub fn calculate(input: &Input) -> Result<Output, Error> {
    validate(input)?;
    let mut rows = input.rows.clone();
    if input.normalize {
        for (index, row) in rows.iter_mut().enumerate() {
            let divisor = row.denominator + row.softness_input;
            if divisor == 0. || row.normalization == 0. {
                return Err(Error("singular physical normalization"));
            }
            let factor = input.relaxation / divisor;
            let coefficient = factor / row.normalization;
            if !factor.is_finite() || !coefficient.is_finite() {
                return Err(Error("nonfinite physical normalization"));
            }
            let count = if index < input.two_body_rows { 4 } else { 2 };
            for vector in &mut row.jacobian[..count] {
                for v in vector {
                    *v = coefficient * *v;
                }
            }
            row.impulse = 0.;
            row.bias *= factor;
            row.softness = row.softness_input * factor;
            if !finite(row) {
                return Err(Error("nonfinite normalized constraint"));
            }
        }
    }
    let count = rows.len();
    iterate(
        rows,
        [input.two_body_rows, count, count, count],
        &vec![None; count],
        input.body_count,
        input.iterations,
    )
}

/// Shared complete iterator: two/single normal spans, then two/single friction.
/// Friction bounds scale by the referenced normal row's current prefix impulse.
#[allow(clippy::assign_op_pattern)] // Preserve response multiplication/addition order.
pub(crate) fn iterate(
    mut rows: Vec<Row>,
    ends: [usize; 4],
    references: &[Option<usize>],
    body_count: usize,
    iterations: usize,
) -> Result<Output, Error> {
    if rows.len() > 256
        || body_count > 64
        || iterations > 32
        || references.len() != rows.len()
        || ends[3] != rows.len()
        || ends.windows(2).any(|w| w[0] > w[1])
        || rows.iter().any(|r| !finite(r) || r.lower > r.upper)
    {
        return Err(Error("invalid physical iteration input"));
    }
    let mut start = 0;
    for (span, &end) in ends.iter().enumerate() {
        let two = span % 2 == 0;
        let mut cursor = start;
        while cursor < end {
            let row = &rows[cursor];
            let next = cursor
                .checked_add(row.batch_len)
                .ok_or(Error("batch overflow"))?;
            if row.batch_len == 0
                || next > end
                || row.bodies[0] >= body_count
                || (two && row.bodies[1] >= body_count)
                || rows[cursor..next]
                    .iter()
                    .any(|r| r.bodies[0] != row.bodies[0] || (two && r.bodies[1] != row.bodies[1]))
                || references[cursor..next].iter().any(|r| {
                    if span >= 2 {
                        r.is_none_or(|i| i >= ends[1])
                    } else {
                        r.is_some()
                    }
                })
            {
                return Err(Error("invalid physical iteration batch/reference"));
            }
            cursor = next;
        }
        start = end;
    }
    let mut bodies = vec![[[0.; 4]; 2]; body_count];
    for _ in 0..iterations {
        let mut start = 0;
        for (span, &end) in ends.iter().enumerate() {
            let two = span % 2 == 0;
            let mut cursor = start;
            while cursor < end {
                let first = &rows[cursor];
                let next = cursor + first.batch_len;
                let [a, b] = first.bodies;
                let mut left = bodies[a];
                let mut right = if two { bodies[b] } else { [[0.; 4]; 2] };
                let mut cumulative = 0.;
                for index in cursor..next {
                    let scale = references[index].map(|r| rows[r].cumulative);
                    let row = &mut rows[index];
                    let mut projection =
                        dot(row.jacobian[0], left[0]) + dot(row.jacobian[1], left[1]);
                    if two {
                        projection += dot(row.jacobian[2], right[0]);
                        projection += dot(row.jacobian[3], right[1]);
                    }
                    let candidate =
                        ((row.bias - row.softness * row.impulse) - projection) + row.impulse;
                    if !candidate.is_finite() {
                        return Err(Error("nonfinite physical impulse"));
                    }
                    let lower = scale.map_or(row.lower, |s| row.lower * s);
                    let upper = scale.map_or(row.upper, |s| row.upper * s);
                    if !lower.is_finite() || !upper.is_finite() {
                        return Err(Error("nonfinite physical friction bounds"));
                    }
                    let impulse = if candidate < lower {
                        lower
                    } else if candidate > upper {
                        upper
                    } else {
                        candidate
                    };
                    let change = impulse - row.impulse;
                    row.impulse = impulse;
                    if span < 2 {
                        cumulative = impulse + cumulative;
                        row.cumulative = cumulative;
                    }
                    for v in 0..2 {
                        for i in 0..4 {
                            left[v][i] = change * row.response[v][i] + left[v][i];
                            if two {
                                right[v][i] = change * row.response[v + 2][i] + right[v][i];
                            }
                        }
                    }
                    if !finite(row) || left.iter().chain(&right).flatten().any(|v| !v.is_finite()) {
                        return Err(Error("nonfinite physical response"));
                    }
                }
                // The original caches both sides and publishes left, then right.
                // This also preserves its overwrite behavior for aliased indices.
                bodies[a] = left;
                if two {
                    bodies[b] = right;
                }
                cursor = next;
            }
            start = end;
        }
    }
    Ok(Output { rows, bodies })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn row() -> Row {
        Row {
            jacobian: [[0.; 4]; 4],
            response: [[1.; 4]; 4],
            softness_input: 0.,
            lower: 0.,
            upper: 2.,
            bias: 3.,
            impulse: 0.,
            cumulative: -1.,
            softness: 0.,
            bodies: [0, 1],
            normalization: 1.,
            denominator: 1.,
            batch_len: 1,
        }
    }
    #[test]
    fn batch_cumulative_is_prefix_of_clamped_impulses() {
        let mut first = row();
        first.batch_len = 2;
        let mut second = row();
        second.bias = 1.;
        let o = calculate(&Input {
            rows: vec![first, second],
            two_body_rows: 0,
            body_count: 1,
            iterations: 1,
            normalize: true,
            relaxation: 1.,
        })
        .unwrap();
        assert_eq!(o.rows[0].cumulative, 2.);
        assert_eq!(o.rows[1].cumulative, 3.);
        assert_eq!(o.bodies[0], [[3.; 4]; 2]);
    }
    #[test]
    fn zero_iterations_still_normalizes_and_clears_workspace() {
        let o = calculate(&Input {
            rows: vec![row()],
            two_body_rows: 0,
            body_count: 2,
            iterations: 0,
            normalize: true,
            relaxation: 0.5,
        })
        .unwrap();
        assert_eq!(o.rows[0].bias, 1.5);
        assert_eq!(o.rows[0].cumulative, -1.);
        assert_eq!(o.bodies, vec![[[0.; 4]; 2]; 2]);
    }
    #[test]
    fn two_body_alias_publishes_second_cached_side_last() {
        let mut r = row();
        r.bodies = [0, 0];
        r.response[0] = [10.; 4];
        r.response[2] = [-1.; 4];
        let o = calculate(&Input {
            rows: vec![r],
            two_body_rows: 1,
            body_count: 1,
            iterations: 1,
            normalize: false,
            relaxation: 1.,
        })
        .unwrap();
        assert_eq!(o.bodies[0][0], [-2.; 4]);
    }
}
