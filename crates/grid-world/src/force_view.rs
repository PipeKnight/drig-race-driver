//! Temporary four-wheel force vectors from supplied body and wheel state.
use crate::Error;

#[derive(Clone, Copy, Debug)]
pub struct Wheel {
    pub coordinate: [f32; 4],
    pub scalar_40: f32,
    pub scalar_e0: f32,
    pub blend_20: f32,
    pub blend_24: f32,
    /// Matrix reached through wheel+34, beginning at pointed object+40.
    pub matrix: [[f32; 4]; 4],
}
#[derive(Clone, Copy, Debug)]
pub struct Input {
    pub body_axis_0: [f32; 4],
    pub body_axis_2: [f32; 4],
    pub linear: [f32; 4],
    pub angular: [f32; 4],
    pub wheels: [Wheel; 4],
}
#[derive(Clone, Copy, Debug)]
pub struct Vectors {
    pub velocity: [f32; 4],
    pub longitudinal: [f32; 4],
    pub lateral: [f32; 4],
    pub lever: [f32; 4],
}
pub fn prepare(input: Input) -> Result<[Vectors; 4], Error> {
    if input
        .body_axis_0
        .into_iter()
        .chain(input.body_axis_2)
        .chain(input.linear)
        .chain(input.angular)
        .any(|v| !v.is_finite())
        || input.wheels.iter().any(|w| {
            w.coordinate
                .into_iter()
                .chain([w.scalar_40, w.scalar_e0, w.blend_20, w.blend_24])
                .chain(w.matrix.into_iter().flatten())
                .any(|v| !v.is_finite())
        })
    {
        return Err(Error("nonfinite force view input"));
    }
    let result = input.wheels.map(|w| {
        let local = [
            w.coordinate[0],
            (w.scalar_40 + w.coordinate[1]) - w.scalar_e0,
            w.coordinate[2],
            0.,
        ];
        let lever = std::array::from_fn(|i| {
            let first = local[3] * w.matrix[3][i] + local[1] * w.matrix[1][i];
            let second = local[2] * w.matrix[2][i] + local[0] * w.matrix[0][i];
            first + second
        });
        let longitudinal = std::array::from_fn(|i| {
            w.blend_24 * input.body_axis_0[i] + w.blend_20 * input.body_axis_2[i]
        });
        let negative_blend = -0.0 - w.blend_24;
        let lateral = std::array::from_fn(|i| {
            w.blend_20 * input.body_axis_0[i] + negative_blend * input.body_axis_2[i]
        });
        let a = input.angular;
        // The original cross helper explicitly writes positive zero in lane W.
        let rotation = [
            a[1] * lever[2] - a[2] * lever[1],
            a[2] * lever[0] - a[0] * lever[2],
            a[0] * lever[1] - a[1] * lever[0],
            0.,
        ];
        let velocity = std::array::from_fn(|i| rotation[i] + input.linear[i]);
        Vectors {
            velocity,
            longitudinal,
            lateral,
            lever,
        }
    });
    if result.iter().any(|v| {
        v.velocity
            .into_iter()
            .chain(v.longitudinal)
            .chain(v.lateral)
            .chain(v.lever)
            .any(|x| !x.is_finite())
    }) {
        return Err(Error("force view exceeds finite range"));
    }
    Ok(result)
}
