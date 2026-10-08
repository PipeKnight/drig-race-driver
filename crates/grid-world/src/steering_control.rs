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

#[derive(Clone, Copy, Debug)]
pub struct ResetParameters {
    pub maximum_rotation_26c: f32,
    pub maximum_cap_250: f32,
    pub perturbation_gain_270: f32,
    pub ramp_start_274: f32,
    pub ramp_end_278: f32,
    pub low_gain_27c: f32,
    pub cap_threshold_254: f32,
    pub wheel_views_active: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct ResetOutput {
    pub random_state: u32,
    pub cap_seeds_48_4c: [f32; 2],
    pub rotation_seeds_50_5c: [f32; 4],
    pub steering: Output,
}

/// Reset with the optional secondary component absent and supplied RNG state.
pub fn reset(mut random_state: u32, p: ResetParameters) -> Result<ResetOutput, Error> {
    if !p.maximum_rotation_26c.is_finite() || !(0. ..=1.).contains(&p.maximum_cap_250) {
        return Err(Error("invalid steering reset parameters"));
    }
    let mut sample = || {
        random_state = random_state
            .wrapping_mul(0x41c64e6d)
            .wrapping_add(0x40584c0f);
        // Signed division truncates toward zero. The original then adds the
        // quotient back to the wrapping state before selecting its low byte.
        let quotient = (random_state as i32) / 65535;
        (random_state.wrapping_add(quotient as u32) & 255) as f32
    };
    let rotations = std::array::from_fn(|_| {
        (sample() * f32::from_bits(0x3c008081) - 1.) * p.maximum_rotation_26c
    });
    let caps = std::array::from_fn(|_| {
        (sample() * f32::from_bits(0x3b808081)) * (p.maximum_cap_250 - 1.) + 1.
    });
    let steering = calculate(Input {
        wheel_levels_18_24: [0.; 4],
        cap_levels_30_34: [0.; 2],
        cap_seeds_48_4c: caps,
        rotation_seeds_50_5c: rotations,
        perturbation_gain_270: p.perturbation_gain_270,
        ramp_start_274: p.ramp_start_274,
        ramp_end_278: p.ramp_end_278,
        low_gain_27c: p.low_gain_27c,
        cap_threshold_254: p.cap_threshold_254,
        wheel_views_active: p.wheel_views_active,
    })?;
    Ok(ResetOutput {
        random_state,
        cap_seeds_48_4c: caps,
        rotation_seeds_50_5c: rotations,
        steering,
    })
}
impl Output {
    /// Steering-only transfer observed in the production body synchronization.
    pub fn apply(
        self,
        mut input: crate::steering_coefficients::Input,
    ) -> crate::steering_coefficients::Input {
        input.command.perturbation = self.perturbation_3ac;
        input.cap_2298 = self.caps_3b0_3b4[0];
        input.cap_229c = self.caps_3b0_3b4[1];
        for i in 0..4 {
            input.wheels[i].rotation_offset = self.rotations_3d4_41c[i];
            input.wheels[i].frozen = self.frozen_3dc_424[i];
        }
        input
    }

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
