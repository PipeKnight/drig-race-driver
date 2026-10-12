//! Localized handling damage distribution for one enabled source box.
//! Caller transforms the impact point and schedules channels/RNG separately.
use crate::Error;

#[derive(Clone, Copy)]
pub struct Input {
    pub previous: f32,
    pub point: [f32; 4],
    pub impact: f32,
    /// Magnitude, second probability, first probability, respectively.
    pub coefficients: [f32; 3],
    pub gain: f32,
    pub center: [f32; 4],
    pub half: [f32; 4],
    pub enabled: bool,
    pub radius: f32,
    pub factors: [f32; 2],
    pub random_state: u32,
}
#[derive(Clone, Copy, Debug)]
pub struct Output {
    pub level: f32,
    pub random_state: u32,
}

pub fn calculate(input: Input) -> Result<Output, Error> {
    if !input.previous.is_finite()
        || !(0. ..=1.5).contains(&input.previous)
        || !(0. ..=1.).contains(&input.impact)
        || !(0. ..=1000.).contains(&input.gain)
        || input
            .point
            .iter()
            .chain(input.center.iter())
            .any(|x| !x.is_finite() || x.abs() > 10000.)
        || input.point[3] != 1.
        || ![0., 1.].contains(&input.center[3])
        || ![0., 1.].contains(&input.half[3])
        || input.half[..3].iter().any(|x| !(0. ..=100.).contains(x))
        || !(0. ..=1000.).contains(&input.radius)
        || input.factors.iter().any(|x| !(0. ..=1000.).contains(x))
        || !(0. ..=1000.).contains(&input.coefficients[0])
        || input.coefficients[1..]
            .iter()
            .any(|x| !(0. ..=1.).contains(x))
    {
        return Err(Error("unsupported localized damage inputs"));
    }
    let mut out = Output {
        level: input.previous,
        random_state: input.random_state,
    };
    if !input.enabled {
        return Ok(out);
    }
    let delta: [f32; 4] = std::array::from_fn(|j| input.point[j] - input.center[j]);
    let mut clamped = delta;
    let mut outside = false;
    for j in 0..3 {
        if input.half[j] < delta[j].abs() {
            outside = true;
            clamped[j] = if delta[j] > 0. {
                input.half[j]
            } else {
                f32::from_bits(input.half[j].to_bits() ^ 0x80000000)
            };
        }
    }
    let mut attenuation = 1.;
    if outside {
        let d: [f32; 4] = std::array::from_fn(|j| delta[j] - clamped[j]);
        let square = (d[3] * d[3] + d[2] * d[2]) + (d[1] * d[1] + d[0] * d[0]);
        if input.radius * input.radius < square {
            return Ok(out);
        }
        attenuation = 1. - square.sqrt() / input.radius;
        // COMISS/JBE also skips an unordered 0/0 after squared underflow.
        if attenuation <= 0. || attenuation.is_nan() {
            return Ok(out);
        }
    }
    let mut increment = ((attenuation * input.impact) * input.coefficients[0]) * input.gain;
    out.random_state = input
        .random_state
        .wrapping_mul(0x41c64e6d)
        .wrapping_add(0x40584c0f);
    let quotient = (out.random_state as i32) / 65535;
    let byte = (out.random_state.wrapping_add(quotient as u32) & 255) as f64;
    // Original RNG multiplies on x87, then the caller stores the sample to f32.
    let sample = (byte * f32::from_bits(0x3b808081) as f64) as f32;
    if input.coefficients[2] > sample {
        increment *= input.factors[0];
    } else if input.coefficients[1] + input.coefficients[2] > sample {
        increment *= input.factors[1];
    }
    out.level = input.previous + increment;
    if out.level > 1. {
        out.level = 1.;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn skip_radius_boundary_rng_and_cap_order() {
        let mut i = Input {
            previous: 0.2,
            point: [0., 0., 0., 1.],
            impact: 0.5,
            coefficients: [0.4, 0., 1.],
            gain: 1.,
            center: [0., 0., 0., 1.],
            half: [1., 1., 1., 0.],
            enabled: false,
            radius: 1.,
            factors: [2., 3.],
            random_state: 0,
        };
        assert_eq!(calculate(i).unwrap().random_state, 0);
        i.enabled = true;
        let inside = calculate(i).unwrap();
        assert_eq!(inside.level, 0.6);
        assert_ne!(inside.random_state, 0);
        i.point[0] = 2.;
        assert_eq!(calculate(i).unwrap().random_state, 0);
        i.point[0] = 1.5;
        assert_eq!(calculate(i).unwrap().level, 0.4);
        i.point[0] = 0.;
        i.impact = 0.;
        i.previous = 1.2;
        assert_eq!(calculate(i).unwrap().level, 1.);
        assert_ne!(calculate(i).unwrap().random_state, 0);
        i.enabled = false;
        assert_eq!(calculate(i).unwrap().level, 1.2);
        i.enabled = true;
        i.point[0] = f32::MIN_POSITIVE;
        i.half = [0., 0., 0., 1.];
        i.radius = 0.;
        let skipped = calculate(i).unwrap();
        assert_eq!(skipped.level, 1.2);
        assert_eq!(skipped.random_state, 0);
        i.point[0] = f32::NAN;
        assert!(calculate(i).is_err());
    }
}
