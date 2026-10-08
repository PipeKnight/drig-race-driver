//! Supplied assisted command shaping and motion feedback, followed by four wheels.
use crate::{
    Error, spawn,
    steering_coefficients::{self, AssistedTail, Input, State},
    steering_feedback, steering_input,
};

#[derive(Clone, Copy, Debug)]
pub struct Motion {
    pub matrix: [[f32; 4]; 4],
    pub linear: [f32; 4],
    pub angular: [f32; 4],
    pub wheel_points_2_3: [[f32; 4]; 2],
}
#[derive(Clone, Copy, Debug)]
pub struct Parameters {
    pub command_deadzone_fdc: f32,
    pub shaping_138c: f32,
    pub high_scale_123c: f32,
    pub scale_start_1240: f32,
    pub scale_end_1244: f32,
    pub speed_cap_e70: f32,
    pub minimum_scale_e5c: f32,
    pub feedback_gain_e58: f32,
    pub low_gain_101c: f32,
    pub gain_speed_1020: f32,
    pub maximum_angle_1024: f32,
    pub minimum_angle_1028: f32,
    pub feedback_deadzone_fec: f32,
    pub linear_response_fe8: f32,
    pub slew_low_1248: f32,
    pub slew_high_1250: f32,
    pub slew_start_124c: f32,
    pub slew_end_1254: f32,
    pub input_factor_1388: f32,
    pub smoothing_factor_1f2c: f32,
}
impl Parameters {
    pub fn scalars(self) -> [f32; 20] {
        [
            self.command_deadzone_fdc,
            self.shaping_138c,
            self.high_scale_123c,
            self.scale_start_1240,
            self.scale_end_1244,
            self.speed_cap_e70,
            self.minimum_scale_e5c,
            self.feedback_gain_e58,
            self.low_gain_101c,
            self.gain_speed_1020,
            self.maximum_angle_1024,
            self.minimum_angle_1028,
            self.feedback_deadzone_fec,
            self.linear_response_fe8,
            self.slew_low_1248,
            self.slew_high_1250,
            self.slew_start_124c,
            self.slew_end_1254,
            self.input_factor_1388,
            self.smoothing_factor_1f2c,
        ]
    }
}
// Retain the original operand order at each f32 store.
#[allow(clippy::assign_op_pattern)]
pub fn calculate(
    mut input: Input,
    p: Parameters,
    motion: Motion,
    prior_38: [f32; 4],
) -> Result<[State; 4], Error> {
    steering_coefficients::validate(input)?;
    if p.scalars()
        .into_iter()
        .chain(prior_38)
        .any(|v| !v.is_finite())
        || p.command_deadzone_fdc < 0.
        || p.command_deadzone_fdc > 1.
        || p.smoothing_factor_1f2c < 0.
        || (p.high_scale_123c != 0. && p.scale_end_1244 <= p.scale_start_1240)
        || (p.maximum_angle_1024 > 0. && p.maximum_angle_1024 == p.minimum_angle_1028)
        || (p.slew_low_1248 != 0. && p.slew_high_1250 != 0. && p.slew_end_1254 <= p.slew_start_124c)
    {
        return Err(Error("invalid assisted steering parameters"));
    }
    let raw = input.command.command;
    let conditioned = steering_input::condition(input.command)?;
    let exponent = if p.shaping_138c >= 0. {
        p.shaping_138c + 1.
    } else {
        1. / (1. - p.shaping_138c)
    };
    let deadzone = p.command_deadzone_fdc * f32::from_bits(0x3f7d70a4);
    let shaped = if conditioned < 0. {
        let base = -0.0 - ((deadzone + conditioned).min(0.) / (1. - deadzone));
        -0.0 - (f64::from(base).powf(f64::from(exponent)) as f32)
    } else {
        let base = (conditioned - deadzone).max(0.) / (1. - deadzone);
        f64::from(base).powf(f64::from(exponent)) as f32
    };
    let speed = input.command.speed_scalar;
    let mut scale = input.scale_e54;
    if p.high_scale_123c != 0. && p.scale_start_1240 < speed {
        let fraction =
            ((speed - p.scale_start_1240) / (p.scale_end_1244 - p.scale_start_1240)).min(1.);
        scale = (p.high_scale_123c - input.scale_e54) * fraction + input.scale_e54;
    }
    if p.speed_cap_e70 < speed && speed > 0. {
        scale = (p.speed_cap_e70 / speed) * scale;
    }
    if scale < p.minimum_scale_e5c {
        scale = p.minimum_scale_e5c;
    }
    let feedback = steering_feedback::calculate(steering_feedback::Input {
        speed_scalar: speed,
        scale_e54: input.scale_e54,
        command_e30: raw,
        matrix: motion.matrix,
        linear: motion.linear,
        angular: motion.angular,
        wheel_points_2_3: motion.wheel_points_2_3,
    })?;
    let mut gain = p.feedback_gain_e58;
    if p.gain_speed_1020 > 0. {
        let fraction = (speed / p.gain_speed_1020).min(1.);
        gain = (gain - p.low_gain_101c) * fraction + p.low_gain_101c;
    }
    if p.maximum_angle_1024 > 0. {
        let v = motion.linear;
        let sq = v.map(|v| v * v);
        let squared = (sq[3] + sq[2]) + (sq[1] + sq[0]);
        let seed = spawn::reference_seed(squared, false)?;
        let inverse = (1. - (seed * squared) * seed) * (seed * 0.5) + seed;
        let dot = std::array::from_fn::<_, 4, _>(|i| (inverse * v[i]) * motion.matrix[2][i]);
        let cosine = ((dot[1] + dot[0]) + (dot[3] + dot[2])).clamp(-1., 1.);
        let angle = f64::from(cosine).acos() as f32;
        let fraction = ((angle - p.minimum_angle_1028)
            / (p.maximum_angle_1024 - p.minimum_angle_1028))
            .clamp(0., 1.);
        gain = fraction * gain;
    }
    let mut center = gain * feedback;
    if p.feedback_deadzone_fec > 0. {
        let threshold = p.feedback_deadzone_fec * scale;
        let negative = -0.0 - threshold;
        center = if center > threshold {
            center - threshold
        } else if center < negative {
            center - negative
        } else {
            0.
        };
    }
    let mut angle = if p.linear_response_fe8 != 0. {
        scale * shaped + center
    } else {
        let positive = (center + scale).clamp(-0.0 - input.scale_e54, input.scale_e54);
        let negative = (center - scale).clamp(-0.0 - input.scale_e54, input.scale_e54);
        let mut middle = (positive + negative) * 0.5 * 0.25;
        let first = (negative * 7. + positive) * 0.125;
        if middle < first {
            middle = first;
        } else {
            let second = (positive * 7. + negative) * 0.125;
            if second < middle {
                middle = second;
            }
        }
        if shaped > 0. {
            (positive - middle) * shaped + middle
        } else {
            middle - (negative - middle) * shaped
        }
    };
    angle = angle.clamp(-0.0 - input.scale_e54, input.scale_e54);
    let slew_step = if p.slew_low_1248 != 0. && p.slew_high_1250 != 0. {
        let fraction =
            ((speed - p.slew_start_124c) / (p.slew_end_1254 - p.slew_start_124c)).clamp(0., 1.);
        let inverted = 1. - raw.abs();
        let factor = p.input_factor_1388 * inverted + (1. - inverted);
        Some(
            (factor * ((p.slew_high_1250 - p.slew_low_1248) * fraction + p.slew_low_1248))
                * input.delta,
        )
    } else {
        None
    };
    if !angle.is_finite()
        || !(p.smoothing_factor_1f2c * (input.delta * 5.)).is_finite()
        || slew_step.is_some_and(|v| !v.is_finite() || v < 0.)
    {
        return Err(Error("nonfinite or negative assisted steering step"));
    }
    for i in 0..2 {
        input.wheels[i + 2].x = motion.wheel_points_2_3[i][0];
    }
    steering_coefficients::update(
        input,
        angle,
        Some(AssistedTail {
            slew_step,
            prior_38,
            smoothing_factor: p.smoothing_factor_1f2c,
        }),
    )
}
