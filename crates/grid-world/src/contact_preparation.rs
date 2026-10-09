//! Collision descriptor activation and its mode-one deformation settling.
use crate::{Error, deformation_settle, spawn::reference_seed};

pub struct Policy {
    pub body_factor_a: f32,
    pub body_factor_b: f32,
    pub global_factor: f32,
    pub linked_mass: f32,
    pub scale: f32,
    pub table: [f32; 16],
}
pub struct Input<'a> {
    pub grid: deformation_settle::Input<'a>,
    pub intensity: f32,
    pub disabled: bool,
    pub up: [f32; 4],
    pub direction: [f32; 4],
    pub point: [f32; 4],
    pub inverse: [[f32; 4]; 4],
    pub policy: Policy,
}
pub struct Activated {
    pub strength: f32,
    pub point: [f32; 4],
    pub direction: [f32; 4],
    pub settled: deformation_settle::Output,
}
pub struct Output {
    /// Original x87 return rounded to f32; the production caller discards it.
    pub scalar: f32,
    /// Present only when mode changes to one; all other descriptor fields stay.
    pub activated: Option<Activated>,
}
fn dot(a: [f32; 4], b: [f32; 4]) -> f32 {
    (a[1] * b[1] + a[0] * b[0]) + (a[3] * b[3] + a[2] * b[2])
}
fn lookup(value: f32, policy: &Policy) -> f64 {
    let scaled = value * f32::from_bits(0x400ed917);
    if scaled < 0. {
        return policy.scale as f64 * policy.table[0] as f64;
    }
    if scaled >= 150. {
        return policy.table[15] as f64 * policy.scale as f64;
    }
    let step = f32::from_bits(0x3dcccccd);
    let index = ((scaled * step).floor() as usize).min(14);
    let fraction = (scaled as f64 - (index * 10) as f64) * step as f64;
    ((1. - fraction) * policy.table[index] as f64 + policy.table[index + 1] as f64 * fraction)
        * policy.scale as f64
}
pub fn calculate(input: Input<'_>) -> Result<Output, Error> {
    deformation_settle::validate(&input.grid)?;
    let p = &input.policy;
    if [
        p.body_factor_a,
        p.body_factor_b,
        p.global_factor,
        p.linked_mass,
    ]
    .into_iter()
    .any(|v| !v.is_finite() || v <= 0.)
        || [input.intensity, p.scale]
            .into_iter()
            .chain(p.table)
            .chain(input.up)
            .chain(input.direction)
            .chain(input.point)
            .chain(input.inverse.into_iter().flatten())
            .any(|v| !v.is_finite())
    {
        return Err(Error("invalid contact preparation input"));
    }
    let projection = dot(input.direction, input.up);
    let direction = if projection > 0. {
        std::array::from_fn(|i| input.direction[i] - projection * input.up[i])
    } else {
        input.direction
    };
    let factor = (p.body_factor_a * p.global_factor) * p.body_factor_b;
    let squared = dot(direction, direction);
    if !factor.is_finite() || factor <= 0. || !squared.is_finite() {
        return Err(Error("nonfinite contact preparation intermediate"));
    }
    let seed = if squared == 0. {
        0.
    } else {
        reference_seed(squared, false)?
    };
    let product = seed * squared;
    let magnitude = (1. - product * seed) * (product * 0.5) + product;
    let ratio = input.intensity / factor;
    // x87 PC53 retains double intermediates until the final f32 store.
    let scalar = ((magnitude as f64 / (p.linked_mass as f64 * factor as f64))
        * lookup(input.intensity, p)
        * ratio as f64) as f32;
    if !scalar.is_finite() {
        return Err(Error("nonfinite contact preparation scalar"));
    }
    if ratio < 1. || scalar <= 1. || input.disabled || input.grid.mode == 1 {
        return Ok(Output {
            scalar,
            activated: None,
        });
    }
    let inverse_length = (1. - (seed * squared) * seed) * (seed * 0.5) + seed;
    let mut normal = direction.map(|v| inverse_length * v);
    normal[3] = 0.;
    let mut point = input.point;
    point[3] = 1.;
    let m = input.inverse;
    let local_direction = std::array::from_fn(|i| {
        (normal[3] * m[3][i] + normal[1] * m[1][i]) + (normal[2] * m[2][i] + normal[0] * m[0][i])
    });
    let local_point = std::array::from_fn(|i| {
        (point[1] * m[1][i] + point[3] * m[3][i]) + (point[2] * m[2][i] + point[0] * m[0][i])
    });
    if local_direction
        .into_iter()
        .chain(local_point)
        .any(|v: f32| !v.is_finite())
    {
        return Err(Error("nonfinite contact preparation transform"));
    }
    let settled = deformation_settle::calculate(deformation_settle::Input {
        mode: 1,
        ..input.grid
    })?;
    Ok(Output {
        scalar,
        activated: Some(Activated {
            strength: scalar,
            point: local_point,
            direction: local_direction,
            settled,
        }),
    })
}
