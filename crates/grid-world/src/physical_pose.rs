//! Apply supplied physical corrections and publish body, shape and derived state.
//! Contact creation, velocity integration and frame scheduling are external.
use crate::{Error, pose, spawn::reference_seed};

pub type Matrix = [[f32; 4]; 4];

pub struct Input {
    pub basis: Matrix,
    pub offset: [f32; 4],
    pub local_matrices: [Matrix; 2],
    /// Group heads orthonormalize their three basis rows before publication.
    pub group_head: bool,
    /// None publishes the body matrix directly; Some applies a shape transform.
    pub shapes: Vec<Option<Matrix>>,
    pub linear: [f32; 4],
    pub angular: [f32; 4],
}

pub struct Output {
    pub corrected: Matrix,
    pub published: Matrix,
    pub derived: [Matrix; 2],
    pub shapes: Vec<Matrix>,
}

fn dot(a: [f32; 4], b: [f32; 4]) -> f32 {
    (a[1] * b[1] + a[0] * b[0]) + (a[3] * b[3] + a[2] * b[2])
}

fn normalized(v: [f32; 4]) -> Result<[f32; 4], Error> {
    let squared = dot(v, v);
    if !squared.is_finite() || squared < f32::MIN_POSITIVE {
        return Err(Error("degenerate physical basis"));
    }
    let seed = reference_seed(squared, false)?;
    let inverse = (1. - (seed * squared) * seed) * (seed * 0.5) + seed;
    Ok(v.map(|x| inverse * x))
}

fn multiply(a: Matrix, b: Matrix) -> Matrix {
    std::array::from_fn(|r| {
        std::array::from_fn(|i| {
            (a[r][2] * b[2][i] + a[r][0] * b[0][i]) + (a[r][3] * b[3][i] + a[r][1] * b[1][i])
        })
    })
}

pub fn calculate(input: &Input) -> Result<Output, Error> {
    if input.shapes.len() > 64
        || input
            .basis
            .iter()
            .flatten()
            .chain(input.offset.iter())
            .chain(input.local_matrices.iter().flatten().flatten())
            .chain(input.shapes.iter().flatten().flatten().flatten())
            .chain(input.linear.iter())
            .chain(input.angular.iter())
            .any(|v| !v.is_finite())
    {
        return Err(Error("invalid physical pose input"));
    }
    let mut corrected = input.basis;
    for (position, delta) in corrected[3].iter_mut().zip(input.linear) {
        *position += delta;
    }
    let squared = dot(input.angular, input.angular);
    // The original unordered comparison skips a subnormal RSQRT refinement.
    let length = if squared < f32::MIN_POSITIVE {
        0.
    } else {
        let seed = reference_seed(squared, false)?;
        let root = seed * squared;
        (1. - root * seed) * (root * 0.5) + root
    };
    if !length.is_finite() || length > 64. {
        return Err(Error("physical angular correction exceeds bound"));
    }
    if length > f32::from_bits(0x2338_77aa) {
        let inverse = 1. / length;
        let rotation = pose::rotation(input.angular.map(|v| inverse * v), length)?;
        let position = corrected[3];
        corrected[3] = [0., 0., 0., 1.];
        corrected = multiply(corrected, rotation);
        corrected[3] = position;
    }
    if input.group_head {
        corrected[0] = normalized(corrected[0])?;
        let projection = dot(corrected[0], corrected[1]);
        corrected[1] = normalized(std::array::from_fn(|i| {
            corrected[1][i] - projection * corrected[0][i]
        }))?;
        let (a, b, previous) = (corrected[0], corrected[1], corrected[2]);
        corrected[2] = normalized([
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
            0.,
        ])?;
        if dot(previous, corrected[2]) < 0. {
            // Multiplication by -1 has this exact sign flip for finite inputs.
            corrected[2] = corrected[2].map(|v| -v);
        }
    }
    let mut published = corrected;
    let o = input.offset;
    published[3] = std::array::from_fn(|i| {
        corrected[3][i]
            - ((o[3] * corrected[3][i] + o[1] * corrected[1][i])
                + (o[2] * corrected[2][i] + o[0] * corrected[0][i]))
    });
    let shapes: Vec<_> = input
        .shapes
        .iter()
        .map(|local| local.map_or(published, |m| multiply(m, published)))
        .collect();
    let mut orientation = corrected;
    orientation[3] = [0., 0., 0., 1.];
    let derived = input.local_matrices.map(|local| {
        let middle = multiply(local, orientation);
        std::array::from_fn(|r| {
            std::array::from_fn(|i| {
                (middle[1][i] * orientation[1][r] + middle[3][i] * orientation[3][r])
                    + (middle[0][i] * orientation[0][r] + middle[2][i] * orientation[2][r])
            })
        })
    });
    if corrected
        .iter()
        .chain(&published)
        .chain(derived.iter().flatten())
        .chain(shapes.iter().flatten())
        .flatten()
        .any(|v| !v.is_finite())
    {
        return Err(Error("physical pose exceeds finite range"));
    }
    Ok(Output {
        corrected,
        published,
        derived,
        shapes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    const IDENTITY: Matrix = [
        [1., 0., 0., 0.],
        [0., 1., 0., 0.],
        [0., 0., 1., 0.],
        [0., 0., 0., 1.],
    ];
    fn input() -> Input {
        Input {
            basis: IDENTITY,
            offset: [0.; 4],
            local_matrices: [IDENTITY; 2],
            group_head: false,
            shapes: vec![None, Some(IDENTITY)],
            linear: [1., 2., 3., 0.],
            angular: [0.; 4],
        }
    }
    #[test]
    fn translation_publishes_without_changing_derived_orientation() {
        let out = calculate(&input()).unwrap();
        assert_eq!(out.corrected[3], [1., 2., 3., 1.]);
        assert_eq!(out.published, out.corrected);
        assert_eq!(out.shapes, vec![out.published; 2]);
        assert_eq!(out.derived, [IDENTITY; 2]);
    }
    #[test]
    fn center_offset_changes_published_position() {
        let mut v = input();
        v.offset = [0.25, 0.5, 0.75, 0.];
        let out = calculate(&v).unwrap();
        assert_eq!(out.corrected[3], [1., 2., 3., 1.]);
        assert_eq!(out.published[3], [0.75, 1.5, 2.25, 1.]);
    }
    #[test]
    fn group_head_preserves_handedness_and_rejects_parallel_rows() {
        let mut v = input();
        v.group_head = true;
        v.basis[2][2] = -1.;
        let out = calculate(&v).unwrap();
        assert!(out.corrected[2][2] < 0.);
        v.basis[1] = v.basis[0];
        assert!(calculate(&v).is_err());
    }
}
