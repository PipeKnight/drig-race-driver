//! Steering subset of the mutable-state producer; component queries are inactive.
use crate::Error;

#[derive(Clone, Copy, Debug)]
pub struct Input {
    pub wheel_levels_18_24: [f32; 4],
    pub cap_levels_30_34: [f32; 2],
    pub cap_seeds_48_4c: [f32; 2],
    pub rotation_seeds_50_5c: [f32; 4],
    pub perturbation_gain_270: f32,
    pub ramp_start_274: f32,
    pub ramp_end_278: f32,
    pub low_gain_27c: f32,
    pub cap_threshold_254: f32,
    pub wheel_views_active: bool,
}
#[derive(Clone, Copy, Debug)]
pub struct Output {
    pub perturbation_3ac: f32,
    pub caps_3b0_3b4: [f32; 2],
    pub rotations_3d4_41c: [f32; 4],
    pub frozen_3dc_424: [bool; 4],
}
impl Output {
    pub fn scalars(self) -> [f32; 7] {
        [
            self.perturbation_3ac,
            self.caps_3b0_3b4[0],
            self.caps_3b0_3b4[1],
            self.rotations_3d4_41c[0],
            self.rotations_3d4_41c[1],
            self.rotations_3d4_41c[2],
            self.rotations_3d4_41c[3],
        ]
    }
}
// COMISS/JBE preserves unordered fractions. Final output validation rejects them.
fn fraction(v: f32) -> f32 {
    v.clamp(0., 1.)
}
fn ramp(level: f32, start: f32, end: f32, low: f32) -> f32 {
    if level < start {
        0.
    } else if start > end {
        1.
    } else {
        (1. - low) * fraction((level - start) / (end - start)) + low
    }
}
pub fn calculate(input: Input) -> Result<Output, Error> {
    if input
        .wheel_levels_18_24
        .into_iter()
        .chain(input.cap_levels_30_34)
        .chain(input.cap_seeds_48_4c)
        .chain(input.rotation_seeds_50_5c)
        .chain([
            input.perturbation_gain_270,
            input.ramp_start_274,
            input.ramp_end_278,
            input.low_gain_27c,
            input.cap_threshold_254,
        ])
        .any(|v| !v.is_finite())
        || input
            .cap_seeds_48_4c
            .iter()
            .any(|v| !(0. ..=1.).contains(v))
    {
        return Err(Error("invalid steering control input"));
    }
    let wheels = input.wheel_levels_18_24;
    let maximum = if wheels[2] < wheels[3] {
        wheels[3]
    } else {
        wheels[2]
    };
    let perturbation = input.perturbation_gain_270
        * ramp(
            maximum,
            input.ramp_start_274,
            input.ramp_end_278,
            input.low_gain_27c,
        );
    let caps = std::array::from_fn(|i| {
        let threshold = input.cap_threshold_254;
        let level = input.cap_levels_30_34[i];
        let blend = if level < threshold {
            0.
        } else if threshold > 1. {
            1.
        } else {
            fraction((level - threshold) / (1. - threshold))
        };
        (input.cap_seeds_48_4c[i] - 1.) * blend + 1.
    });
    let rotations = if input.wheel_views_active {
        std::array::from_fn(|i| {
            input.rotation_seeds_50_5c[i]
                * ramp(
                    wheels[i],
                    input.ramp_start_274,
                    input.ramp_end_278,
                    input.low_gain_27c,
                )
        })
    } else {
        [0.; 4]
    };
    let result = Output {
        perturbation_3ac: if perturbation > 2. { 2. } else { perturbation },
        caps_3b0_3b4: caps,
        rotations_3d4_41c: rotations,
        frozen_3dc_424: wheels.map(|level| input.wheel_views_active && input.ramp_end_278 <= level),
    };
    if result.scalars().iter().any(|v| !v.is_finite()) {
        return Err(Error("nonfinite steering control result"));
    }
    Ok(result)
}
