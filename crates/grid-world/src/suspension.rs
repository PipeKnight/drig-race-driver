//! Supported four-wheel suspension impulse calculation. Contact limits, tyre
//! impulses, coefficient preparation and drivetrain coupling are separate.
use crate::Error;

#[derive(Clone, Copy, Debug)]
pub struct Wheel {
    pub displacement: f32,
    pub speed: f32,
    pub spring: f32,
    pub preload: f32,
    pub travel_denominator: f32,
    pub damper: f32,
    pub speed_threshold: f32,
    pub high_speed_ratio: f32,
    pub negative_speed_scale: f32,
    pub bump: f32,
    pub axle_coupling: f32,
}

/// Standard four-wheel preload builder with supplied gravity magnitude and
/// local wheel Z. Matches the observed x87 path at 53-bit precision/nearest;
/// running-game control-word and later deformation updates remain separate.
pub fn rest_preloads(
    mass: f32,
    gravity: f32,
    z: [f32; 4],
    spring: [f32; 4],
) -> Result<[f32; 4], Error> {
    if !mass.is_finite()
        || mass <= 0.
        || !gravity.is_finite()
        || gravity < 0.
        || z.iter().any(|v| !v.is_finite())
        || spring.iter().any(|v| !v.is_finite() || *v <= 0.)
    {
        return Err(Error("invalid Standard preload inputs"));
    }
    let rear = (z[0] + z[1]).abs() * 0.5;
    let front = (z[2] + z[3]).abs() * 0.5;
    let span = front + rear;
    if !span.is_finite() || span <= 0. {
        return Err(Error("degenerate preload axle span"));
    }
    // The front load is stored as f32 before its two divisions. Rear load
    // remains in x87 through both divisions, so preserve that asymmetry.
    let front_load = ((((gravity as f64 * mass as f64) * rear as f64) * 0.5) / span as f64) as f32;
    let rear_load = (((gravity as f64 * mass as f64) * front as f64) * 0.5) / span as f64;
    let result = std::array::from_fn(|i| {
        let load = if i < 2 { rear_load } else { front_load as f64 };
        (-(load / spring[i] as f64)) as f32
    });
    if result.iter().any(|v| !v.is_finite()) {
        return Err(Error("preload exceeds finite range"));
    }
    Ok(result)
}

/// Supplied coefficients and displacement/velocity, four active wheels only.
/// Scale provenance/units and simulation delta must be established by the caller.
#[allow(clippy::assign_op_pattern)] // Preserve the observed multiplication operand order.
pub fn impulses(wheels: [Wheel; 4], scale: f32, delta: f32) -> Result<[f32; 4], Error> {
    crate::dynamics::substeps(delta)?;
    if !scale.is_finite() || scale <= 0. {
        return Err(Error("invalid suspension scale"));
    }
    for w in wheels {
        if [
            w.displacement,
            w.speed,
            w.spring,
            w.preload,
            w.travel_denominator,
            w.damper,
            w.speed_threshold,
            w.high_speed_ratio,
            w.negative_speed_scale,
            w.bump,
            w.axle_coupling,
        ]
        .into_iter()
        .any(|v| !v.is_finite())
            || w.travel_denominator <= 0.
            || w.speed_threshold < 0.
        {
            return Err(Error("invalid suspension inputs"));
        }
    }
    let means = [
        (wheels[0].displacement + wheels[1].displacement) * 0.5,
        (wheels[2].displacement + wheels[3].displacement) * 0.5,
    ];
    let result = std::array::from_fn(|i| {
        let w = wheels[i];
        let ratio = (w.displacement / w.travel_denominator).min(1.);
        let spring = (w.spring * scale) * (w.displacement - w.preload / scale);
        let mut damping = w.damper * scale;
        if w.speed <= 0. {
            if w.speed_threshold < w.speed.abs() {
                damping = (w.high_speed_ratio
                    - (w.speed_threshold / w.speed) * (1. - w.high_speed_ratio))
                    * damping;
            }
            damping = w.negative_speed_scale * damping;
        } else if w.speed_threshold < w.speed {
            damping = ((w.speed_threshold / w.speed) * (1. - w.high_speed_ratio)
                + w.high_speed_ratio)
                * damping;
        }
        let ratio2 = ratio * ratio;
        let ratio4 = ratio2 * ratio2;
        let bump = if ratio > 0. {
            ((ratio4 * ratio4) * ratio4) * w.bump
        } else {
            0.
        };
        let coupled = (w.displacement - means[i / 2]) * w.axle_coupling;
        (coupled + ((w.speed * damping + spring) + bump)) * delta
    });
    if result.iter().any(|v: &f32| !v.is_finite()) {
        return Err(Error("suspension impulse exceeds finite range"));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn static_preload_balances_weight_and_opposite_axle_lever() {
        let result =
            rest_preloads(100., 10., [-1., -1., 3., 3.], [100., 100., 100., 100.]).unwrap();
        assert_eq!(result, [-3.75, -3.75, -1.25, -1.25]);
        assert_eq!(result.iter().sum::<f32>() * -100., 1000.);
        assert!(rest_preloads(100., 10., [0.; 4], [100.; 4]).is_err());
        assert!(rest_preloads(100., 10., [-1., -1., 3., 3.], [0.; 4]).is_err());
    }
    #[test]
    fn axle_coupling_cancels_equal_and_opposes_unequal_displacement() {
        let wheel = Wheel {
            displacement: 0.,
            speed: 0.,
            spring: 0.,
            preload: 0.,
            travel_denominator: 1.,
            damper: 0.,
            speed_threshold: 1.,
            high_speed_ratio: 1.,
            negative_speed_scale: 1.,
            bump: 0.,
            axle_coupling: 8.,
        };
        assert_eq!(impulses([wheel; 4], 1., 0.125).unwrap(), [0.; 4]);
        let mut wheels = [wheel; 4];
        wheels[0].displacement = 2.;
        assert_eq!(impulses(wheels, 1., 0.125).unwrap(), [1., -1., 0., 0.]);
        wheels[0].travel_denominator = 0.;
        assert!(impulses(wheels, 1., 0.125).is_err());
    }
}
