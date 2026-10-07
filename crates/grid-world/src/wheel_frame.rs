//! Four-wheel world geometry and supplied active tyre-axis preparation.
use crate::Error;

#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub point: [f32; 4],
    pub anchor: [f32; 4],
    pub axis: [f32; 4],
}

/// Active +70c branch: transform wheel +20/+24 coefficients into two axes.
/// Their source/input provenance remains the caller's responsibility.
pub fn tyre_axes(
    basis: [[f32; 4]; 4],
    coefficients: [[f32; 2]; 4],
) -> Result<[[[f32; 4]; 2]; 4], Error> {
    if basis
        .iter()
        .flatten()
        .chain(coefficients.iter().flatten())
        .any(|v| !v.is_finite())
    {
        return Err(Error("invalid tyre axis inputs"));
    }
    let transform = |v: [f32; 4]| {
        std::array::from_fn(|i| {
            (v[3] * basis[3][i] + v[1] * basis[1][i]) + (v[2] * basis[2][i] + v[0] * basis[0][i])
        })
    };
    let axes: [[[f32; 4]; 2]; 4] =
        coefficients.map(|[a, b]| [transform([a, 0., -0. - b, 0.]), transform([b, 0., a, 0.])]);
    if axes.iter().flatten().flatten().any(|v| !v.is_finite()) {
        return Err(Error("tyre axes exceed finite range"));
    }
    Ok(axes)
}

/// Supplied affine basis, local wheel centers/axes and displacements. Centers
/// explicitly receive W=1; axes retain all four supplied lanes. No ground query.
pub fn prepare(
    basis: [[f32; 4]; 4],
    points: [[f32; 4]; 4],
    axes: [[f32; 4]; 4],
    displacement: [f32; 4],
) -> Result<[Frame; 4], Error> {
    if basis
        .iter()
        .flatten()
        .chain(points.iter().flatten())
        .chain(axes.iter().flatten())
        .chain(&displacement)
        .any(|v| !v.is_finite())
    {
        return Err(Error("invalid wheel frame inputs"));
    }
    let transform = |v: [f32; 4]| {
        std::array::from_fn(|i| {
            (v[3] * basis[3][i] + v[1] * basis[1][i]) + (v[2] * basis[2][i] + v[0] * basis[0][i])
        })
    };
    let frames: [Frame; 4] = std::array::from_fn(|w| {
        let axis: [f32; 4] = transform(axes[w]);
        let mut local = points[w];
        local[3] = 1.;
        let anchor: [f32; 4] = transform(local);
        let point = std::array::from_fn(|i| displacement[w] * axis[i] + anchor[i]);
        Frame {
            point,
            anchor,
            axis,
        }
    });
    if frames.iter().any(|f| {
        f.point
            .iter()
            .chain(&f.anchor)
            .chain(&f.axis)
            .any(|v| !v.is_finite())
    }) {
        return Err(Error("wheel frame exceeds finite range"));
    }
    Ok(frames)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn center_is_affine_and_displacement_follows_axis() {
        let basis = [
            [1., 0., 0., 0.],
            [0., 1., 0., 0.],
            [0., 0., 1., 0.],
            [10., 20., 30., 1.],
        ];
        let frames = prepare(
            basis,
            [[1., 2., 3., 7.]; 4],
            [[0., 1., 0., 0.]; 4],
            [0.5; 4],
        )
        .unwrap();
        assert_eq!(frames[0].anchor, [11., 22., 33., 1.]);
        assert_eq!(frames[0].point, [11., 22.5, 33., 1.]);
        assert_eq!(frames[0].axis, [0., 1., 0., 0.]);
    }
}
