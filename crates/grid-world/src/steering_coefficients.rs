//! Whole four-wheel coefficient update with assistance disabled.
use crate::{Error, steering_input};

#[derive(Clone, Copy, Debug)]
pub struct Wheel {
    pub x: f32,
    pub toe_e8: f32,
    pub rotation_offset: f32,
    pub frozen: bool,
    pub blend_20: f32,
    pub blend_24: f32,
    pub smooth_28: f32,
    pub smooth_2c: f32,
    pub limit_f0: f32,
    pub rotation_144: f32,
}
#[derive(Clone, Copy, Debug)]
pub struct Input {
    pub command: steering_input::Input,
    pub scale_e54: f32,
    pub span_21d4: f32,
    pub span_21d8: f32,
    pub cap_2298: f32,
    pub cap_229c: f32,
    pub mode_1f74_is_one: bool,
    pub delta: f32,
    /// Wheel ordinals reached through body+21cc/+21d0, supplied explicitly.
    pub pair: [usize; 2],
    pub wheels: [Wheel; 4],
}
#[derive(Clone, Copy, Debug)]
pub struct State {
    pub blend_20: f32,
    pub blend_24: f32,
    pub smooth_28: f32,
    pub smooth_2c: f32,
    /// Wheel+38 captures the coefficient before the final rotation/restoration.
    pub history_38: f32,
}
fn trig(angle: f32) -> (f32, f32) {
    (f64::from(angle).sin() as f32, f64::from(angle).cos() as f32)
}
pub(crate) fn validate(input: Input) -> Result<(), Error> {
    if input.pair.into_iter().any(|i| i >= 4)
        || input.pair[0] == input.pair[1]
        || input.delta < 0.
        || input.span_21d8 <= 0.
        || input.span_21d4 < 0.
        || input.scale_e54 < 0.
        || input.cap_2298 < 0.
        || input.cap_229c < 0.
        || [
            input.scale_e54,
            input.span_21d4,
            input.span_21d8,
            input.cap_2298,
            input.cap_229c,
            input.delta,
        ]
        .into_iter()
        .any(|v| !v.is_finite())
        || input.wheels.iter().any(|w| {
            w.limit_f0 < 0.
                || w.limit_f0 > 1.
                || [
                    w.x,
                    w.toe_e8,
                    w.rotation_offset,
                    w.blend_20,
                    w.blend_24,
                    w.smooth_28,
                    w.smooth_2c,
                    w.limit_f0,
                    w.rotation_144,
                ]
                .into_iter()
                .any(|v| !v.is_finite())
        })
    {
        return Err(Error("invalid steering coefficient input"));
    }
    Ok(())
}
pub fn unassisted(input: Input) -> Result<[State; 4], Error> {
    validate(input)?;
    let command = steering_input::condition(input.command)?;
    let angle = input.scale_e54 * command;
    update(input, angle, None)
}
pub(crate) struct AssistedTail {
    pub slew_step: Option<f32>,
    pub prior_38: [f32; 4],
    pub smoothing_factor: f32,
}
pub(crate) fn update(
    input: Input,
    angle: f32,
    tail: Option<AssistedTail>,
) -> Result<[State; 4], Error> {
    let mut result = input.wheels.map(|w| State {
        blend_20: w.blend_20,
        blend_24: w.blend_24,
        smooth_28: w.smooth_28,
        smooth_2c: w.smooth_2c,
        history_38: 0.,
    });
    if angle.abs() <= 1e-7 {
        for i in input.pair {
            result[i].blend_20 = 1.;
            result[i].blend_24 = 0.;
        }
    } else {
        let (sin, cos) = trig(angle);
        let center = -0.0 - (cos / sin) * input.span_21d8;
        let half = input.span_21d4 * 0.5;
        let inverse = 1. / input.span_21d8;
        for (slot, slope, cap) in [
            (input.pair[0], (center - half) * inverse, input.cap_229c),
            (input.pair[1], (half + center) * inverse, input.cap_2298),
        ] {
            let norm = (slope * slope + 1.).sqrt();
            let norm = if slope < 0. { -norm } else { norm };
            result[slot].blend_20 = slope / norm;
            result[slot].blend_24 = -1. / norm;
            if cap < 1. {
                let bound = (f64::from(input.scale_e54).tan() as f32) * cap;
                let old = result[slot].blend_24;
                if bound < old.abs() {
                    let sign = if old > 0. {
                        1.
                    } else if old < 0. {
                        -1.
                    } else {
                        0.
                    };
                    result[slot].blend_24 = sign * bound;
                    let v = result[slot].blend_24;
                    result[slot].blend_20 = (1. - v * v).sqrt();
                }
            }
        }
    }
    if angle.abs() > 1e-7 && input.mode_1f74_is_one {
        if input.command.speed_scalar < 15. {
            let factor = -0.0 - (1. - input.command.speed_scalar * f32::from_bits(0x3d888889));
            for i in 0..2 {
                let v = factor * result[i + 2].blend_24;
                result[i].blend_24 = v;
                result[i].blend_20 = (1. - v * v).sqrt();
            }
        }
    } else {
        for w in &mut result[..2] {
            w.blend_20 = 1.;
            w.blend_24 = 0.;
        }
    }
    for (w, state) in input.wheels.iter().zip(&mut result) {
        let toe = if w.x < 0. { -0.0 - w.toe_e8 } else { w.toe_e8 };
        let offset = toe + w.rotation_offset;
        if offset != 0. {
            let (sin, cos) = trig(offset);
            let old = state.blend_24;
            state.blend_24 = old * cos - state.blend_20 * sin;
            state.blend_20 = state.blend_20 * cos + old * sin;
        }
    }
    if let Some(tail) = &tail
        && let Some(step) = tail.slew_step
    {
        for (state, prior) in result.iter_mut().zip(tail.prior_38) {
            // Original COMISS branches leave the coefficient alone for unordered bounds.
            let v = if step.is_nan() {
                state.blend_24
            } else {
                state.blend_24.clamp(prior - step, prior + step)
            };
            state.blend_24 = v;
            state.blend_20 = (1. - v * v).sqrt();
        }
    }
    let step = input.delta * 5.;
    let step = if let Some(tail) = &tail {
        tail.smoothing_factor * step
    } else {
        step
    };
    for (w, state) in input.wheels.iter().zip(&mut result) {
        state.history_38 = state.blend_24;
        let before_rotation = state.blend_24;
        let target = if w.frozen {
            w.blend_24
        } else {
            before_rotation
        };
        let smooth = target
            .clamp(w.smooth_2c - step, step + w.smooth_2c)
            .clamp(-0.0 - w.limit_f0, w.limit_f0);
        if smooth != w.smooth_2c {
            state.smooth_2c = smooth;
            state.smooth_28 = if smooth == 0. {
                1.
            } else {
                (1. - smooth * smooth).sqrt()
            };
        }
        let rotation = w.rotation_144 * 2.;
        if rotation.abs() > 1e-7 {
            let (sin, cos) = trig(rotation);
            let old = state.blend_20;
            state.blend_24 = sin * old + cos * before_rotation;
            state.blend_20 = cos * old - sin * before_rotation;
        }
        if w.frozen {
            state.blend_20 = w.blend_20;
            state.blend_24 = w.blend_24;
        }
    }
    if result.iter().any(|s| {
        [
            s.blend_20,
            s.blend_24,
            s.smooth_28,
            s.smooth_2c,
            s.history_38,
        ]
        .into_iter()
        .any(|v| !v.is_finite())
    }) {
        return Err(Error("nonfinite unassisted steering result"));
    }
    Ok(result)
}
