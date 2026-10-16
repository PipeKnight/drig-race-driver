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
        let spin = angular_speed(runtime, i);
        wheel.phase = phase(wheel.phase, spin, remainder);
    }
    wheels
}

pub fn angular_speed(runtime: &Runtime, wheel: usize) -> f32 {
    assert!(wheel < 4);
    let speed =
        f32::from_bits(runtime.state.car.owner.vehicle[(0x1990 + wheel * 0x150 + 0x104) / 4]);
    if speed.is_finite() { speed } else { 0. }
}

/// Fixed box exposure, or an authored Gaussian temporal-filter floor. Seconds,
/// never a number of frames or subtraction of wrapped phase samples.
pub fn exposure(
    speed: f32,
    shutter: f32,
    frame_seconds: f32,
    adaptive: bool,
    floor: f32,
) -> (f32, bool) {
    if !speed.is_finite() || !shutter.is_finite() || shutter <= 0. {
        return (0., false);
    }
    let width = if adaptive {
        let frame = if frame_seconds.is_finite() {
            frame_seconds.clamp(0., 1. / 30.)
        } else {
            0.
        };
        // Same RMS width as the authored box at high FPS; extra filtering at
        // lower FPS. Three standard deviations each side of the centre.
        let floor = if floor.is_finite() {
            floor.clamp(0., 0.5)
        } else {
            0.5
        };
        6. * (shutter / 12_f32.sqrt()).max(frame * floor)
    } else {
        shutter
    };
    let angle = speed.abs() * width;
    // A full 3-sigma support is not a uniform circular distribution. Keep the
    // Gaussian width beyond one turn; shaders integrate a wrapped kernel on
    // at most one circle. Uniform is accurate once sigma itself reaches TAU
    // (first angular harmonic <3e-9), avoiding the old visible-policy jump.
    if adaptive && angle < 6. * std::f32::consts::TAU {
        (angle, true)
    } else {
        (angle.min(std::f32::consts::TAU), false)
    }
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

    #[test]
    fn adaptive_exposure_suppresses_a_low_fps_alias_without_blurring_standstill() {
        // Independent frequency-domain check of the sampled kernel: a16-fold
        // pattern at15rad/s aliases to8.2Hz at30FPS with a short box shutter.
        // Independent numerical circular convolution, compared with the
        // analytical Gaussian angular Fourier response below.
        let response = |angle: f32, gaussian: bool, mode: f64| {
            let (mut real, mut imaginary, mut mass) = (0_f64, 0_f64, 0_f64);
            let domain = f64::from(angle.min(std::f32::consts::TAU));
            let sigma = f64::from(angle) / 6.;
            for i in 0..4096 {
                let theta = ((i as f64 + 0.5) / 4096. - 0.5) * domain;
                let w = if gaussian {
                    (-8..=8)
                        .map(|k| {
                            (-0.5
                                * ((theta + f64::from(k) * std::f64::consts::TAU) / sigma).powi(2))
                            .exp()
                        })
                        .sum()
                } else {
                    1.
                };
                real += (mode * theta).cos() * w;
                imaginary += (mode * theta).sin() * w;
                mass += w;
            }
            real.hypot(imaginary) / mass
        };
        assert!(
            exposure(
                (std::f32::consts::TAU + 0.0001) / 0.05,
                0.008333,
                1. / 30.,
                true,
                0.25
            )
            .1
        );
        assert!(
            !exposure(
                (6. * std::f32::consts::TAU + 0.0001) / 0.05,
                0.008333,
                1. / 30.,
                true,
                0.25
            )
            .1
        );
        let before = response(std::f32::consts::TAU - 0.0001, true, 1.);
        let after = response(std::f32::consts::TAU + 0.0001, true, 1.);
        assert!((before - after).abs() < 0.0001);
        assert!((after - (-0.5 * (std::f64::consts::TAU / 6.).powi(2)).exp()).abs() < 0.0001);
        assert!(response(std::f32::consts::TAU * 6., true, 1.) < 1e-7);
        let fixed = exposure(15., 0.008333, 1. / 30., false, 0.5);
        let adaptive = exposure(15., 0.008333, 1. / 30., true, 0.5);
        assert!(response(fixed.0, fixed.1, 16.) > 0.8);
        assert!(response(adaptive.0, adaptive.1, 16.) < 0.01);
        // The gentler art-control deliberately retains some angular detail;
        // its weaker alias suppression is a visible tradeoff, not the default.
        let gentle = exposure(15., 0.008333, 1. / 30., true, 0.25);
        assert!((0.1..0.2).contains(&response(gentle.0, gentle.1, 16.)));
        assert_eq!(
            exposure(15., 0.008333, 1. / 300., true, 0.25),
            exposure(15., 0.008333, 1. / 300., true, 0.5)
        );
        assert_eq!(exposure(0., 0.008333, 1. / 30., true, 0.5).0, 0.);
        assert_eq!(exposure(-15., 0.008333, 1. / 30., true, 0.5), adaptive);
        assert_eq!(exposure(15., 0., 1. / 30., true, 0.5).0, 0.);
        assert!(exposure(15., 0.008333, 1. / 300., true, 0.5).0 < adaptive.0);
    }
}
