//! Supplied drive, paired/central exchange and brake increments before tyre slip.
use crate::Error;

#[derive(Clone, Copy, Debug)]
pub struct Wheel {
    pub spin: f32,
    pub rate: f32,
    pub inertia: f32,
    pub drive_delta: f32,
    pub brake_delta: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Exchange {
    pub lower: f32,
    pub upper: f32,
    pub strength: f32,
}

fn impulse(difference: f32, exchange: Exchange, gain: f32, delta: f32) -> f32 {
    let sign = if difference > 0. { 1. } else { -1. };
    let ratio =
        ((sign * difference - exchange.lower) / (exchange.upper - exchange.lower)).clamp(0., 1.);
    (((exchange.strength * ratio) * gain) * sign) * delta
}

#[allow(clippy::assign_op_pattern)] // Original adds the change before stored spin.
fn apply(wheel: &mut Wheel, impulse: f32, positive: bool) {
    let change = if positive {
        impulse / wheel.inertia
    } else {
        (-1. / wheel.inertia) * impulse
    };
    wheel.spin = change + wheel.spin;
    wheel.rate -= change;
}

pub fn prepare(
    mut wheels: [Wheel; 4],
    exchanges: [Exchange; 3],
    gain: f32,
    delta: f32,
) -> Result<[Wheel; 4], Error> {
    if !gain.is_finite()
        || !delta.is_finite()
        || delta <= 0.
        || delta > 0.01
        || wheels
            .iter()
            .flat_map(|w| [w.spin, w.rate, w.inertia, w.drive_delta, w.brake_delta])
            .any(|v| !v.is_finite())
        || wheels.iter().any(|w| w.inertia <= 0. || w.brake_delta < 0.)
        || exchanges.iter().any(|e| {
            ![e.lower, e.upper, e.strength].iter().all(|v| v.is_finite()) || e.upper <= e.lower
        })
    {
        return Err(Error("invalid supported wheel rotation inputs"));
    }
    for wheel in &mut wheels {
        wheel.spin += wheel.drive_delta;
        wheel.rate -= wheel.drive_delta;
    }
    let rear = impulse(wheels[1].spin - wheels[0].spin, exchanges[0], gain, delta);
    apply(&mut wheels[0], rear, true);
    apply(&mut wheels[1], rear, false);
    let front = impulse(wheels[3].spin - wheels[2].spin, exchanges[1], gain, delta);
    apply(&mut wheels[2], front, true);
    apply(&mut wheels[3], front, false);
    let central = impulse(
        (wheels[3].spin + wheels[2].spin) - (wheels[1].spin + wheels[0].spin),
        exchanges[2],
        gain,
        delta,
    );
    for (w, wheel) in wheels.iter_mut().enumerate() {
        apply(wheel, central, w < 2);
    }
    for wheel in &mut wheels {
        let positive = wheel.spin > 0.;
        if positive {
            wheel.spin -= wheel.brake_delta;
            wheel.rate -= -0. - wheel.brake_delta;
        } else {
            wheel.spin += wheel.brake_delta;
            wheel.rate -= wheel.brake_delta;
        }
        if (positive && wheel.spin < 0.) || (!positive && wheel.spin > 0.) {
            let overshoot = wheel.spin;
            wheel.spin -= overshoot;
            wheel.rate -= -0. - overshoot;
        }
    }
    if wheels
        .iter()
        .any(|w| !w.spin.is_finite() || !w.rate.is_finite())
    {
        return Err(Error("wheel rotation exceeds finite range"));
    }
    Ok(wheels)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn braking_overshoot_stops_spin_and_preserves_relative_total() {
        let wheel = Wheel {
            spin: 2.,
            rate: 3.,
            inertia: 1.,
            drive_delta: 0.,
            brake_delta: 5.,
        };
        let exchange = Exchange {
            lower: 0.,
            upper: 1.,
            strength: 0.,
        };
        let result = prepare([wheel; 4], [exchange; 3], 1., 0.001).unwrap();
        for wheel in result {
            assert_eq!(wheel.spin, 0.);
            assert_eq!(wheel.rate, 5.);
        }
        let result = prepare([Wheel { spin: -2., ..wheel }; 4], [exchange; 3], 1., 0.001).unwrap();
        for wheel in result {
            assert_eq!(wheel.spin, 0.);
            assert_eq!(wheel.rate, 1.);
        }
    }
    #[test]
    fn singular_exchange_and_negative_brake_increment_are_rejected() {
        let wheel = Wheel {
            spin: 0.,
            rate: 0.,
            inertia: 1.,
            drive_delta: 0.,
            brake_delta: 0.,
        };
        let exchange = Exchange {
            lower: 1.,
            upper: 1.,
            strength: 1.,
        };
        assert!(prepare([wheel; 4], [exchange; 3], 1., 0.001).is_err());
        let exchange = Exchange {
            upper: 2.,
            ..exchange
        };
        assert!(
            prepare(
                [Wheel {
                    brake_delta: -1.,
                    ..wheel
                }; 4],
                [exchange; 3],
                1.,
                0.001
            )
            .is_err()
        );
    }
}
