//! Supplied motion feedback used by the assisted steering coefficient writer.
use crate::Error;

#[derive(Clone, Copy, Debug)]
pub struct Input {
    pub speed_scalar: f32,
    pub scale_e54: f32,
    /// Original body+e30, before the private conditioned command copy.
    pub command_e30: f32,
    pub matrix: [[f32; 4]; 4],
    pub linear: [f32; 4],
    pub angular: [f32; 4],
    pub wheel_points_2_3: [[f32; 4]; 2],
}
pub fn calculate(input: Input) -> Result<f32, Error> {
    if [input.speed_scalar, input.scale_e54, input.command_e30]
        .into_iter()
        .chain(input.matrix.into_iter().flatten())
        .chain(input.linear)
        .chain(input.angular)
        .chain(input.wheel_points_2_3.into_iter().flatten())
        .any(|v| !v.is_finite())
    {
        return Err(Error("nonfinite steering feedback input"));
    }
    let blend = ((input.speed_scalar.abs() - 2.) * 0.5).clamp(0., 1.);
    let requested = input.scale_e54 * input.command_e30;
    if blend == 0. {
        return if requested.is_finite() {
            Ok(requested)
        } else {
            Err(Error("nonfinite steering feedback output"))
        };
    }
    let local: [f32; 4] = std::array::from_fn(|i| {
        if i == 3 {
            1.
        } else {
            (input.wheel_points_2_3[1][i] + input.wheel_points_2_3[0][i]) * 0.5
        }
    });
    let lever: [f32; 4] = std::array::from_fn(|i| {
        let a = local[3] * input.matrix[3][i] + local[1] * input.matrix[1][i];
        let b = local[2] * input.matrix[2][i] + local[0] * input.matrix[0][i];
        (a + b) - input.matrix[3][i]
    });
    let a = input.angular;
    let rotation = [
        a[1] * lever[2] - a[2] * lever[1],
        a[2] * lever[0] - a[0] * lever[2],
        a[0] * lever[1] - a[1] * lever[0],
        0.,
    ];
    let velocity: [f32; 4] = std::array::from_fn(|i| input.linear[i] + rotation[i]);
    let p0: [f32; 4] = std::array::from_fn(|i| input.matrix[0][i] * velocity[i]);
    let p2: [f32; 4] = std::array::from_fn(|i| input.matrix[2][i] * velocity[i]);
    let lateral = (p0[1] + p0[0]) + (p0[3] + p0[2]);
    let longitudinal = (p2[3] + p2[2]) + (p2[1] + p2[0]);
    let angle = (f64::from(lateral) / f64::from(longitudinal)).atan();
    let blend = f64::from(blend);
    let result = ((1. - blend) * f64::from(requested) + angle * blend) as f32;
    if result.is_finite() {
        Ok(result)
    } else {
        Err(Error("nonfinite steering feedback output"))
    }
}
