//! Four-wheel suspension subset of the setup consumer. Ancillary brake,
//! gearing and tyre-angle writes are deliberately outside this result.
use crate::{Error, suspension};

#[derive(Clone, Copy, Debug)]
pub struct Input {
    pub upper: [f32; 30],
    pub lower: [f32; 30],
    pub sliders: [f32; 30],
    /// Front, rear, matching the decoded tuning record.
    pub base_damper: [f32; 2],
    pub base_threshold: [f32; 2],
    pub base_high_ratio: [f32; 2],
    pub height: f32,
    pub denominator: [f32; 4],
    pub local_y: [f32; 4],
    pub local_z: [f32; 4],
    pub mass: f32,
    pub gravity: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Wheel {
    pub spring: f32,
    pub damper: f32,
    pub threshold: f32,
    pub high_ratio: f32,
    pub negative_scale: f32,
    pub coupling: f32,
    pub denominator: f32,
    pub local_y: f32,
    pub preload: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Output {
    pub height: f32,
    pub wheels: [Wheel; 4],
}

fn blend(base: [f32; 2], target: f32, low: f32, high: f32) -> [f32; 2] {
    let mean = (base[0] + base[1]) * 0.5;
    let endpoint = if mean <= target { high } else { low };
    let difference = mean - endpoint;
    let weight = if difference.abs() >= f32::from_bits(0x3a83_126f) {
        (target - endpoint) / difference
    } else {
        0.
    };
    base.map(|value| (value - endpoint) * weight + endpoint)
}

/// Select thirty supplied setup ranges and return only suspension/height writes.
/// Negative sliders disable their selected value; caller range provenance and
/// source default-slider construction are separate. Standard wheel order is
/// rear 0/1, front 2/3, with axle selected by nonpositive local Z.
pub fn prepare(input: Input) -> Result<Output, Error> {
    if input
        .upper
        .iter()
        .chain(&input.lower)
        .chain(&input.sliders)
        .chain(&input.base_damper)
        .chain(&input.base_threshold)
        .chain(&input.base_high_ratio)
        .chain(&input.denominator)
        .chain(&input.local_y)
        .chain(&input.local_z)
        .any(|v| !v.is_finite())
        || !input.height.is_finite()
        || input.base_damper.iter().any(|v| *v <= 0.)
        || input.sliders.iter().any(|v| !(-1. ..=1.).contains(v))
    {
        return Err(Error("invalid suspension setup inputs"));
    }
    let selected: [f32; 30] = std::array::from_fn(|i| {
        if input.sliders[i] < 0. {
            0.
        } else {
            (input.upper[i] - input.lower[i]) * input.sliders[i] + input.lower[i]
        }
    });
    let high = blend(
        std::array::from_fn(|i| input.base_high_ratio[i] * input.base_damper[i]),
        selected[10],
        input.lower[10],
        input.upper[10],
    );
    let threshold = blend(
        input.base_threshold,
        selected[11],
        input.lower[11],
        input.upper[11],
    );
    let mut height = selected[12];
    for denominator in input.denominator {
        let minimum = f32::from_bits(0x3c23_d70a);
        if (height - input.height) + denominator < minimum {
            height = (minimum - denominator) + input.height;
        }
    }
    let delta = height - input.height;
    let spring = std::array::from_fn(|i| selected[if input.local_z[i] <= 0. { 5 } else { 4 }]);
    let preload = suspension::rest_preloads(input.mass, input.gravity, input.local_z, spring)?;
    let wheels = std::array::from_fn(|i| {
        let axle = usize::from(input.local_z[i] <= 0.);
        let damping_index = if axle == 1 { 8 } else { 6 };
        Wheel {
            spring: spring[i],
            damper: selected[damping_index],
            threshold: threshold[axle],
            high_ratio: high[axle] / input.base_damper[axle],
            negative_scale: selected[damping_index + 1] / selected[damping_index],
            coupling: selected[28 + axle],
            denominator: if delta != 0. {
                delta + input.denominator[i]
            } else {
                input.denominator[i]
            },
            local_y: if delta != 0. {
                input.local_y[i] - delta
            } else {
                input.local_y[i]
            },
            preload: preload[i],
        }
    });
    if !height.is_finite()
        || wheels.iter().any(|w| {
            [
                w.spring,
                w.damper,
                w.threshold,
                w.high_ratio,
                w.negative_scale,
                w.coupling,
                w.denominator,
                w.local_y,
                w.preload,
            ]
            .iter()
            .any(|v| !v.is_finite())
                || w.damper <= 0.
                || w.denominator <= 0.
                || w.threshold < 0.
        })
    {
        return Err(Error("suspension setup exceeds supported finite range"));
    }
    Ok(Output { height, wheels })
}

/// Constructor cap after setup/deformation preparation. The dimension operand
/// is supplied: its physical interpretation and deformation update are separate.
pub fn cap_denominators(denominator: [f32; 4], dimension: f32) -> Result<[f32; 4], Error> {
    let cap = dimension + f32::from_bits(0x3d4c_cccd);
    if !cap.is_finite() || cap <= 0. || denominator.iter().any(|v| !v.is_finite() || *v <= 0.) {
        return Err(Error("invalid constructor travel cap"));
    }
    Ok(denominator.map(|v| if cap < v { cap } else { v }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn height_guard_preserves_clearance_and_threshold_endpoint() {
        let mut input = Input {
            upper: [1.; 30],
            lower: [0.; 30],
            sliders: [0.; 30],
            base_damper: [100.; 2],
            base_threshold: [400.; 2],
            base_high_ratio: [0.5; 2],
            height: 0.,
            denominator: [0.25; 4],
            local_y: [-0.5; 4],
            local_z: [-1., -1., 1., 1.],
            mass: 100.,
            gravity: 10.,
        };
        for i in [4, 5, 6, 7, 8, 9, 10] {
            input.lower[i] = 100.;
            input.upper[i] = 100.;
        }
        input.lower[11] = 0.001;
        input.upper[11] = 0.14;
        input.sliders[11] = 1.;
        input.lower[12] = -1.;
        input.upper[12] = -1.;
        let result = prepare(input).unwrap();
        assert_eq!(result.height, f32::from_bits(0x3c23_d70a) - 0.25);
        for wheel in result.wheels {
            assert_eq!(wheel.threshold, 0.14);
            assert!(wheel.denominator >= 0.00999999);
            assert_eq!(wheel.negative_scale, 1.);
        }
        assert_eq!(
            cap_denominators([0.3, 0.1, 0.4, 0.15], 0.1).unwrap(),
            [0.15, 0.1, 0.15, 0.15]
        );
        input.base_damper[0] = 0.;
        assert!(prepare(input).is_err());
    }
}
