//! Scoped collision body adjustment with explicit original-CRT random state.
use crate::{Error, spawn::reference_seed};
pub struct Input {
    pub angular: [f32; 4],
    pub linear: [f32; 4],
    pub frame: [[f32; 4]; 4],
    pub point: [f32; 4],
    pub direction: [f32; 4],
    pub center: [f32; 4],
    pub history: [f32; 10],
    pub history_extra: f32,
    pub mass: f32,
    pub gain: f32,
    pub random_gain: f32,
    pub history_range: f32,
    pub history_threshold: f32,
    pub speed_threshold: f32,
    pub random_seed: u32,
}
pub struct Output {
    pub angular: [f32; 4],
    pub linear: [f32; 4],
    pub history: [f32; 10],
    pub history_extra: f32,
    pub random_seed: u32,
}
fn dot(a: [f32; 4], b: [f32; 4]) -> f32 {
    (a[1] * b[1] + a[0] * b[0]) + (a[3] * b[3] + a[2] * b[2])
}
fn cross(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
        0.,
    ]
}
fn unit(v: [f32; 4]) -> Result<[f32; 4], Error> {
    let squared = dot(v, v);
    if squared == 0. {
        return Ok([0.; 4]);
    }
    let seed = reference_seed(squared, false)?;
    let factor = (1. - (seed * squared) * seed) * (seed * 0.5) + seed;
    Ok(v.map(|v| factor * v))
}
fn point_velocity(
    angular: [f32; 4],
    linear: [f32; 4],
    point: [f32; 4],
    center: [f32; 4],
) -> [f32; 4] {
    let lever = std::array::from_fn(|i| point[i] - center[i]);
    let rotation = cross(angular, lever);
    std::array::from_fn(|i| linear[i] + rotation[i])
}
pub fn calculate(input: Input) -> Result<Output, Error> {
    if input.mass <= 0.
        || input.history_range <= 0.
        || input
            .angular
            .into_iter()
            .chain(input.linear)
            .chain(input.frame.into_iter().flatten())
            .chain(input.point)
            .chain(input.direction)
            .chain(input.center)
            .chain(input.history)
            .chain([
                input.history_extra,
                input.mass,
                input.gain,
                input.random_gain,
                input.history_range,
                input.history_threshold,
                input.speed_threshold,
            ])
            .any(|v| !v.is_finite())
    {
        return Err(Error("invalid collision body input"));
    }
    let mut output = Output {
        angular: input.angular,
        linear: input.linear,
        history: input.history,
        history_extra: input.history_extra,
        random_seed: input.random_seed,
    };
    let squared = dot(input.linear, input.linear);
    let speed = if squared == 0. {
        0.
    } else {
        let seed = reference_seed(squared, false)?;
        let product = seed * squared;
        (1. - product * seed) * (product * 0.5) + product
    };
    if speed >= input.speed_threshold {
        return Ok(output);
    }
    let mut maximum = input.history[0];
    for value in &input.history[1..] {
        if maximum <= *value {
            maximum = *value;
        }
    }
    let intensity = ((maximum - input.history_threshold) / input.history_range).min(1.);
    if intensity <= 0. {
        return Ok(output);
    }
    let b = input.direction;
    let m = input.frame;
    let local = std::array::from_fn(|i| {
        (b[3] * m[3][i] + b[1] * m[1][i]) + (b[2] * m[2][i] + b[0] * m[0][i])
    });
    let normalized = unit(local)?;
    let side = if normalized[2] > f32::from_bits(0x3f333333)
        || normalized[0].abs() > f32::from_bits(0x3f333333)
    {
        1.5
    } else {
        1.
    };
    output.history = [0.; 10];
    output.history_extra = 0.;
    let mut axis = cross(input.direction, [0., 1., 0., 0.]);
    let squared = dot(axis, axis);
    if squared <= f32::from_bits(0x29e12e13) {
        return Ok(output);
    }
    let inverse = 1. / squared.sqrt();
    axis = axis.map(|v| inverse * v);
    output.random_seed = input.random_seed.wrapping_mul(214013).wrapping_add(2531011);
    let byte = ((output.random_seed >> 16) & 255) as f32;
    axis[1] = ((byte - 127.5) * input.random_gain) * f32::from_bits(0x3c008081);
    axis = unit(axis)?;
    let mass_scale = (1000. / input.mass).min(1.);
    let gain = ((intensity * mass_scale) * input.gain) * side;
    let before = point_velocity(input.angular, input.linear, input.point, input.center);
    if dot(axis, input.angular) < gain * 0.5 {
        output.angular = std::array::from_fn(|i| gain * axis[i] + input.angular[i]);
    }
    let after = point_velocity(output.angular, input.linear, input.point, input.center);
    output.linear = std::array::from_fn(|i| input.linear[i] - (after[i] - before[i]));
    if output
        .angular
        .into_iter()
        .chain(output.linear)
        .any(|v| !v.is_finite())
    {
        return Err(Error("nonfinite collision body result"));
    }
    Ok(output)
}
