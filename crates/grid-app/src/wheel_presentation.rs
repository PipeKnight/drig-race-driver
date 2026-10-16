//! Read-only render-time wheel phase. No second persistent wheel clock.
//! The source port integrates angle += spin * delta in vehicle_propulsion;
//! vehicle_force stores signed wheel spin (rad/s) at wheel+0x104.
use grid_tools::source_vehicle::{DELTA, Runtime, WheelPose};

/// Predict within the outstanding fixed-step remainder. This is bounded
/// extrapolation, not interpolation of wrapped angles or physical feedback.
fn phase(physical: f32, radians_per_second: f32, remainder: f32) -> f32 {
    if !physical.is_finite() || !radians_per_second.is_finite() || !remainder.is_finite() {
        return physical;
    }
    physical + radians_per_second * remainder.clamp(0., DELTA)
}

pub fn wheels(runtime: &Runtime, remainder: f32) -> [WheelPose; 4] {
    let mut wheels = runtime.wheels();
    if runtime.ticks == 0 {
        return wheels;
    }
    for (i, wheel) in wheels.iter_mut().enumerate() {
        let spin =
            f32::from_bits(runtime.state.car.owner.vehicle[(0x1990 + i * 0x150 + 0x104) / 4]);
        wheel.phase = phase(wheel.phase, spin, remainder);
    }
    wheels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_rates_do_not_change_continuous_spin_or_lose_turns() {
        // Includes multiple complete turns per physics step, both directions.
        for speed in [0., 15., -15., 190., -190., 2000., -2000.] {
            for fps in [30, 60, 144, 300] {
                let mut previous = 0.;
                for frame in 0..=fps {
                    let t = frame as f64 / f64::from(fps);
                    let dt = f64::from(DELTA);
                    let tick = (t / dt).floor();
                    let physical = (tick * dt * f64::from(speed)) as f32;
                    let shown = phase(physical, speed, (t - tick * dt) as f32);
                    assert!((shown - (t * f64::from(speed)) as f32).abs() < 0.0003);
                    if frame > 0 {
                        assert!(((shown - previous) - speed / fps as f32).abs() < 0.0003);
                    }
                    previous = shown;
                }
            }
        }
    }

    #[test]
    fn bounded_prediction_and_stationary_phase() {
        assert_eq!(phase(1., 100., 0.), 1.);
        assert_eq!(phase(1., 100., -1.), 1.);
        assert_eq!(phase(1., 100., 2.), phase(1., 100., DELTA));
        assert_eq!(phase(1., 0., 0.01), 1.);
        assert_eq!(phase(1., f32::NAN, 0.01), 1.);
        assert_eq!(phase(1., 100., f32::NAN), 1.);
    }
}
