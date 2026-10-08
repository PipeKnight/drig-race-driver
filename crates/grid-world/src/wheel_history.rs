//! Supplied production wheel history/grip calculation.
//! Offset-qualified fields have observed arithmetic roles, not established units.
use crate::{Error, tyre_grip};

#[derive(Clone, Copy, Debug)]
pub struct Vehicle {
    pub speed: f32,
    pub mass: f32,
    pub scalar_1f3c: f32,
    pub scalar_1ffc: f32,
    pub scalar_1efc: f32,
    pub scalar_e20: f32,
    pub scalar_187c: f32,
    pub minimum_alignment: f32,
    pub flag_b11: bool,
    pub flag_e38: bool,
    pub direction: [f32; 4],
}
#[derive(Clone, Copy, Debug)]
pub struct Material {
    pub wear: f32,
    pub speed_grip: [f32; 3],
    pub flag_70: bool,
    pub dirt_build_up: bool,
    /// Caller-owned opaque material identity, not an original process pointer.
    pub token: u32,
}
#[derive(Clone, Copy, Debug)]
pub struct Wheel {
    pub air_clock: f32,
    pub return_clock: f32,
    pub scalar_6c: f32,
    pub grip_scale: f32,
    pub spin: f32,
    pub radius: f32,
    pub scalar_138: f32,
    pub scalar_e4: f32,
    pub pair_b0_b4: [f32; 2],
    pub scalar_10c: f32,
    pub flag_fc: bool,
    pub flag_fd: bool,
    /// Original +11c/+120/+124. Exact state meanings remain separate research.
    pub history: [f32; 3],
    pub history_material: u32,
}
#[derive(Clone, Copy, Debug)]
pub struct Input {
    pub vehicle: Vehicle,
    pub wheel: Wheel,
    pub material: Option<Material>,
    pub velocity: [f32; 4],
    pub lateral_axis: [f32; 4],
    pub longitudinal_axis: [f32; 4],
    pub normal: [f32; 4],
    pub delta: f32,
    pub scalar_argument_10: f32,
}
#[derive(Clone, Copy, Debug)]
pub struct Output {
    pub wheel: Wheel,
    pub grip: f32,
}
fn dot(a: [f32; 4], b: [f32; 4]) -> f32 {
    (a[0] * b[0] + a[1] * b[1]) + (a[2] * b[2] + a[3] * b[3])
}

/// Compatibility entry with zero wheel +ec angle and second body axis Y.
pub fn calculate(input: Input) -> Result<Output, Error> {
    calculate_angled(input, 0., [0., 1., 0., 0.])
}

/// The physical meaning of wheel +ec is not yet established.
#[allow(
    clippy::assign_op_pattern,
    reason = "Preserve observed change-first floating-point operations"
)]
pub fn calculate_angled(
    input: Input,
    angle: f32,
    second_body_axis: [f32; 4],
) -> Result<Output, Error> {
    let b = input.vehicle;
    let mut w = input.wheel;
    let m = input.material;
    let scalars = [
        b.speed,
        b.mass,
        b.scalar_1f3c,
        b.scalar_1ffc,
        b.scalar_1efc,
        b.scalar_e20,
        b.scalar_187c,
        b.minimum_alignment,
        w.air_clock,
        w.return_clock,
        w.scalar_6c,
        w.grip_scale,
        w.spin,
        w.radius,
        w.scalar_138,
        w.scalar_e4,
        w.pair_b0_b4[0],
        w.pair_b0_b4[1],
        w.scalar_10c,
        w.history[0],
        w.history[1],
        w.history[2],
        input.delta,
        input.scalar_argument_10,
        angle,
    ];
    if scalars
        .iter()
        .chain(b.direction.iter())
        .chain(input.velocity.iter())
        .chain(input.lateral_axis.iter())
        .chain(input.longitudinal_axis.iter())
        .chain(input.normal.iter())
        .chain(second_body_axis.iter())
        .any(|v| !v.is_finite())
        || input.delta < 0.
        || input.delta > 0.1
        || b.mass <= 0.
        || b.scalar_1f3c <= 0.
        || b.minimum_alignment < 0.
        || b.minimum_alignment >= 1.
        || b.scalar_1ffc < 0.
        || w.radius <= 0.
    {
        return Err(Error("invalid supplied wheel history inputs"));
    }
    let mut grip = 0.;
    let mut activity = 0.;
    let mut heat_factor = 1.;
    w.scalar_6c = 0.;
    if let Some(m) = m {
        if !m.wear.is_finite() {
            return Err(Error("invalid material wear"));
        }
        if w.air_clock > 0.5 {
            w.flag_fd = true;
        }
        w.air_clock = 0.;
        let lateral = dot(input.lateral_axis, input.velocity);
        let longitudinal = dot(input.longitudinal_axis, input.velocity) - w.spin * w.radius;
        let speed = (lateral * lateral + longitudinal * longitudinal).sqrt();
        let g = tyre_grip::extended(m.speed_grip[0], m.speed_grip[1], m.speed_grip[2], speed, 1.)?;
        grip = ((w.scalar_138 as f64 * w.scalar_e4 as f64) * g) as f32;
        if w.return_clock > 0. {
            w.return_clock -= input.delta;
        }
        let minimum = (b.speed * f32::from_bits(0x3d4c_cccd)).clamp(0., 1.);
        let scale = ((b.speed - 3.) * 0.5).clamp(0., 1.);
        activity = (w.pair_b0_b4[0].abs() + w.pair_b0_b4[1].abs())
            * (b.mass * f32::from_bits(0x3ed0_c43d));
        activity = activity.max(minimum).min(f32::from_bits(0x3fac_cccd)) * scale;
        if w.scalar_10c > 0.
            && w.spin * w.radius > b.speed + 2.
            && (b.scalar_e20 > 0.1 || b.speed < 8.)
            && ((b.flag_e38 && b.speed > 5. && w.flag_fc) || b.scalar_187c < 0.25)
            && m.flag_70
        {
            activity = f32::from_bits(0x3d08_8889) / b.scalar_1f3c + activity;
            heat_factor = f32::from_bits(0x3e9e_b852);
        }
        activity *= f32::from_bits(0x3f4c_cccd);
    } else {
        w.air_clock = input.delta + w.air_clock;
        w.return_clock = (w.return_clock + input.delta).min(5.);
    }
    let wear = m.map_or(0., |m| m.wear);
    let heat = (((1. - (b.scalar_1ffc - 0.5) * f32::from_bits(0x3ecc_cccd)) * activity)
        * heat_factor)
        * f32::from_bits(0x3bda_740e);
    if b.flag_b11 {
        let increment = (((b.scalar_1f3c * wear) * activity) * b.scalar_1efc)
            * (input.scalar_argument_10 * 2. + 1.);
        let increment =
            (increment / (b.scalar_1ffc * f32::from_bits(0x3faa_aaab) + 0.25)) * input.delta;
        w.history[1] = increment + w.history[1];
    }
    if m.is_some() {
        w.history[0] = heat * input.delta + w.history[0];
    }
    w.history[0] = ((1. - input.delta * f32::from_bits(0x3bbc_bad5)) * w.history[0]).clamp(0., 1.);
    w.history[1] = w.history[1].clamp(0., 1.);
    if let Some(m) = m {
        if m.dirt_build_up {
            let increase =
                ((w.spin.abs() * f32::from_bits(0x3daa_aaab)).clamp(0., 1.) * input.delta) * 2.;
            w.history[2] = (increase + w.history[2]).min(1.);
            w.history_material = m.token;
        } else {
            let decrease =
                ((w.spin.abs() * f32::from_bits(0x3d2a_aaab)).clamp(0., 1.) * input.delta) * 0.25;
            w.history[2] = (w.history[2] - decrease).max(0.);
        }
        // Original double trigonometric returns are converted before vector arithmetic.
        let sine = (angle as f64).sin() as f32;
        let cosine = (angle as f64).cos() as f32;
        let direction =
            std::array::from_fn(|i| cosine * b.direction[i] - sine * second_body_axis[i]);
        let alignment_dot = dot(direction, input.normal);
        let alignment = (1. - alignment_dot * alignment_dot).sqrt();
        grip = if alignment <= b.minimum_alignment {
            0.
        } else {
            ((alignment - b.minimum_alignment) / (1. - b.minimum_alignment)) * grip
        };
    }
    let result = ((w.grip_scale as f64) * (grip as f64)) as f32;
    if [
        result,
        w.history[0],
        w.history[1],
        w.history[2],
        w.air_clock,
        w.return_clock,
    ]
    .iter()
    .any(|v| !v.is_finite())
    {
        return Err(Error("wheel history result exceeds finite range"));
    }
    Ok(Output {
        wheel: w,
        grip: result,
    })
}
