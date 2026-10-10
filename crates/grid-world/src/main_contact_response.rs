//! Main response preparation and all normal/friction iteration spans.
//! Inputs are constructed rows and caller-supplied body workspace/inverse properties.
use crate::{
    Error,
    contact_rows::Row as RawRow,
    physical_response::{self, Row},
};

#[derive(Clone)]
pub struct Body {
    pub workspace: [[f32; 4]; 2],
    pub inverse_mass: f32,
    /// The first three matrix rows are dot products; output W is positive zero.
    pub inverse_inertia: [[f32; 4]; 4],
}
#[derive(Clone)]
pub struct Input {
    pub rows: Vec<RawRow>,
    /// Ends of two normals, single normals, two friction, single friction spans.
    pub ends: [usize; 4],
    /// Friction references are row indices; normal rows have no reference.
    pub references: Vec<Option<usize>>,
    pub bodies: Vec<Body>,
    pub softness_scale: f32,
    pub bias_scale: f32,
    pub relaxation: f32,
    pub iterations: usize,
}
pub struct Output {
    pub biased: Vec<RawRow>,
    pub prepared: Vec<RawRow>,
    pub rows: Vec<RawRow>,
    pub bodies: Vec<[[f32; 4]; 2]>,
}
fn f(row: &RawRow, i: usize) -> f32 {
    f32::from_bits(row[i])
}
fn v(row: &RawRow, i: usize) -> [f32; 4] {
    std::array::from_fn(|j| f(row, i + j))
}
fn dot(a: [f32; 4], b: [f32; 4]) -> f32 {
    (a[0] * b[0] + a[1] * b[1]) + (a[2] * b[2] + a[3] * b[3])
}
fn projection(row: &RawRow, vectors: [[f32; 4]; 4], two: bool) -> f32 {
    let mut result = dot(v(row, 0), vectors[0]) + dot(v(row, 4), vectors[1]);
    if two {
        result += dot(v(row, 8), vectors[2]);
        result += dot(v(row, 12), vectors[3]);
    }
    result
}
fn typed(row: &RawRow) -> Row {
    Row {
        jacobian: std::array::from_fn(|i| v(row, i * 4)),
        response: std::array::from_fn(|i| v(row, 16 + i * 4)),
        softness_input: f(row, 33),
        lower: f(row, 34),
        upper: f(row, 35),
        bias: f(row, 36),
        impulse: f(row, 37),
        cumulative: f(row, 38),
        softness: f(row, 39),
        bodies: [row[40] as usize, row[41] as usize],
        normalization: f(row, 43),
        denominator: f(row, 44),
        batch_len: row[45] as usize,
    }
}

pub fn calculate(input: &Input) -> Result<Output, Error> {
    if input.rows.len() > 256
        || input.bodies.len() > 64
        || input.iterations > 32
        || input.references.len() != input.rows.len()
        || input.ends[3] != input.rows.len()
        || input.ends.windows(2).any(|w| w[0] > w[1])
        || [input.softness_scale, input.bias_scale, input.relaxation]
            .iter()
            .any(|v| !v.is_finite())
        || input.bodies.iter().any(|b| {
            !b.inverse_mass.is_finite()
                || b.workspace
                    .iter()
                    .flatten()
                    .chain(b.inverse_inertia.iter().flatten())
                    .any(|v| !v.is_finite())
        })
    {
        return Err(Error("invalid main contact response input"));
    }
    // Validate all batches/references before indexing body operands.
    physical_response::iterate(
        input.rows.iter().map(typed).collect(),
        input.ends,
        &input.references,
        input.bodies.len(),
        0,
    )?;
    let mut rows = input.rows.clone();
    let mut start = 0;
    for (span, &end) in input.ends.iter().enumerate() {
        let two = span % 2 == 0;
        for row in &mut rows[start..end] {
            if !f(row, 32).is_finite() {
                return Err(Error("nonfinite provisional bias"));
            }
            let a = &input.bodies[row[40] as usize];
            let b = if two {
                &input.bodies[row[41] as usize]
            } else {
                a
            };
            let projection = projection(
                row,
                [
                    a.workspace[0],
                    a.workspace[1],
                    b.workspace[0],
                    b.workspace[1],
                ],
                two,
            );
            row[36] = (f(row, 32) * input.bias_scale - projection).to_bits();
            row[33] = (f(row, 33) * input.softness_scale).to_bits();
        }
        start = end;
    }
    let biased = rows.clone();
    start = 0;
    for (span, &end) in input.ends.iter().enumerate() {
        let two = span % 2 == 0;
        for row in &mut rows[start..end] {
            for side in 0..if two { 2 } else { 1 } {
                let b = &input.bodies[row[40 + side] as usize];
                let linear = v(row, side * 8).map(|v| b.inverse_mass * v);
                let angular = v(row, side * 8 + 4);
                let angular: [f32; 4] = std::array::from_fn(|i| {
                    if i == 3 {
                        0.
                    } else {
                        dot(angular, b.inverse_inertia[i])
                    }
                });
                row[16 + side * 8..20 + side * 8].copy_from_slice(&linear.map(f32::to_bits));
                row[20 + side * 8..24 + side * 8].copy_from_slice(&angular.map(f32::to_bits));
            }
            let denominator = projection(row, std::array::from_fn(|i| v(row, 16 + i * 4)), two);
            let divisor = denominator + f(row, 33);
            let factor = input.relaxation / divisor;
            if divisor == 0. || !factor.is_finite() {
                return Err(Error("singular main contact normalization"));
            }
            for word in &mut row[..if two { 16 } else { 8 }] {
                *word = (factor * f32::from_bits(*word)).to_bits();
            }
            row[36] = (factor * f(row, 36)).to_bits();
            row[37] = 0;
            row[39] = (f(row, 33) * factor).to_bits();
            row[43] = factor.to_bits();
            row[44] = denominator.to_bits();
        }
        start = end;
    }
    let prepared = rows.clone();
    let solved = physical_response::iterate(
        rows.iter().map(typed).collect(),
        input.ends,
        &input.references,
        input.bodies.len(),
        input.iterations,
    )?;
    for (raw, r) in rows.iter_mut().zip(solved.rows) {
        raw[37] = r.impulse.to_bits();
        raw[38] = r.cumulative.to_bits();
    }
    Ok(Output {
        biased,
        prepared,
        rows,
        bodies: solved.bodies,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input() -> Input {
        let mut normal = [0; 48];
        normal[0] = 1f32.to_bits();
        normal[8] = (-1f32).to_bits();
        normal[32] = 2f32.to_bits();
        normal[35] = f32::MAX.to_bits();
        normal[41] = 1;
        normal[43] = 1f32.to_bits();
        normal[44] = 1f32.to_bits();
        normal[45] = 1;
        let mut friction = normal;
        friction[0] = 0;
        friction[8] = 0;
        friction[1] = 1f32.to_bits();
        friction[9] = (-1f32).to_bits();
        friction[32] = 5f32.to_bits();
        friction[34] = (-0.5f32).to_bits();
        friction[35] = 0.5f32.to_bits();
        friction[38] = 0.75f32.to_bits();
        let body = Body {
            workspace: [[0.; 4]; 2],
            inverse_mass: 1.,
            inverse_inertia: std::array::from_fn(|r| {
                std::array::from_fn(|c| if r == c { 1. } else { 0. })
            }),
        };
        Input {
            rows: vec![normal, friction],
            ends: [1, 1, 2, 2],
            references: vec![None, Some(0)],
            bodies: vec![body.clone(), body],
            softness_scale: 1.,
            bias_scale: 1.,
            relaxation: 1.,
            iterations: 1,
        }
    }
    #[test]
    fn normal_prefix_scales_friction_and_friction_prefix_survives() {
        let o = calculate(&input()).unwrap();
        assert_eq!(f(&o.rows[0], 37), 1.);
        assert_eq!(f(&o.rows[0], 38), 1.);
        assert_eq!(f(&o.rows[1], 37), 0.5);
        assert_eq!(f(&o.rows[1], 38), 0.75);
        assert_eq!(o.bodies[0][0], [1., 0.5, 0., 0.]);
        assert_eq!(o.bodies[1][0], [-1., -0.5, 0., 0.]);
        assert_eq!(f(&o.prepared[0], 43), 0.5);
        assert_eq!(f(&o.prepared[0], 44), 2.);
    }
    #[test]
    fn initial_workspace_changes_bias_before_solver_clear() {
        let mut i = input();
        i.bodies[0].workspace[0][0] = 1.;
        let o = calculate(&i).unwrap();
        assert_eq!(f(&o.biased[0], 36), 1.);
        assert_eq!(f(&o.rows[0], 37), 0.5);
        assert_eq!(f(&o.rows[1], 37), 0.25);
        assert_eq!(o.bodies[0][0], [0.5, 0.25, 0., 0.]);
    }
    #[test]
    fn reference_guard_preserves_input() {
        let mut i = input();
        i.references[1] = Some(1);
        let before = i.rows.clone();
        assert!(calculate(&i).is_err());
        assert_eq!(i.rows, before);
    }
    #[test]
    fn empty_rows_still_clear_all_body_workspace() {
        let mut i = input();
        i.rows.clear();
        i.references.clear();
        i.ends = [0; 4];
        i.iterations = 0;
        i.bodies[0].workspace = [[9.; 4]; 2];
        assert_eq!(calculate(&i).unwrap().bodies, vec![[[0.; 4]; 2]; 2]);
    }
}
