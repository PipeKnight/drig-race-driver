//! Torque-curve shift thresholds, before runtime controls. Whole 9AD6B0 boundary.
use crate::{Error, vehicle_force::Frame, vehicle_propulsion::curve53};

fn f(b: &[u32], o: usize) -> f32 {
    f32::from_bits(b[o / 4])
}
fn set(b: &mut [u32], o: usize, x: f32) {
    b[o / 4] = x.to_bits();
}

/// Build thresholds from already initialized engine/ratios/radii. Does not
/// decode the source engine, apply setup scaling or initialize the full car.
/// A bounded RPM scan and owned driven-wheel indices exclude unsafe inputs.
pub fn initialize(s: &mut Frame) -> Result<(), Error> {
    let b = &s.body;
    let rpm = f(b, 0x17a4);
    if s.driven.is_empty()
        || s.driven.len() > 4
        || s.driven.iter().enumerate().any(|(i, &w)| {
            w >= 4
                || s.driven[..i].contains(&w)
                || !f(b, 0x1990 + w * 0x150 + 0xe0).is_finite()
                || f(b, 0x1990 + w * 0x150 + 0xe0) <= 0.
        })
        || !rpm.is_finite()
        || !(0. ..=20_000.).contains(&rpm)
        || !f(b, 0x2218).is_finite()
        || (0..32).any(|i| !f(b, 0x17ac + i * 4).is_finite())
        || (0..10).any(|i| !f(b, 0x18f4 + i * 4).is_finite())
        || (1..10).any(|i| !f(b, 0x191c + i * 4).is_finite() || !f(b, 0x1944 + i * 4).is_finite())
    {
        return Err(Error("invalid shift-threshold initialization inputs"));
    }
    let mut next = s.clone();
    let b = &mut next.body;
    for gear in 1..9 {
        let ratio = f(b, 0x18f4 + gear * 4);
        if ratio <= 0. {
            continue;
        }
        set(b, 0x191c + gear * 4, 10_000.);
        let following = f(b, 0x18f8 + gear * 4);
        let mut speed = rpm * f32::from_bits(0x3f7ae148);
        if following > 0. && speed >= 1. {
            let factor = 1. / (following / ratio);
            if !factor.is_finite() || !(factor * speed).is_finite() {
                return Err(Error("unbounded shift-threshold curve lookup"));
            }
            while speed >= 1. {
                let current = curve53(b, speed) as f32;
                let after = curve53(b, factor * speed) * f32::from_bits(0x3f7d70a4) as f64;
                if current as f64 > after {
                    let radius = f(b, 0x1990 + next.driven[0] * 0x150 + 0xe0);
                    set(b, 0x191c + gear * 4, (radius * speed) * ratio);
                    break;
                }
                speed -= 1.;
            }
        }
        let threshold = if gear == 1 {
            0.
        } else {
            f(b, 0x1918 + gear * 4) * f32::from_bits(0x3f733333)
        };
        set(b, 0x1944 + gear * 4, threshold);
    }
    let second = f(b, 0x2218) * f(b, 0x194c);
    set(b, 0x194c, second);
    let mut min = f32::MAX;
    let mut max = 0f32;
    for &i in &next.driven {
        let radius = f(b, 0x1990 + i * 0x150 + 0xe0);
        if radius < min {
            min = radius;
        }
        if radius > max {
            max = radius;
        }
    }
    let scale = min / max;
    for gear in 1..10 {
        for base in [0x1944, 0x191c] {
            let value = f(b, base + gear * 4) * scale;
            set(b, base + gear * 4, value);
            if !f(b, base + gear * 4).is_finite() {
                return Err(Error("nonfinite shift-threshold result"));
            }
        }
    }
    *s = next;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn overflow_after_threshold_work_preserves_complete_frame() {
        let mut s = Frame {
            body: [0; crate::vehicle_force::BODY_WORDS],
            view: [0; crate::vehicle_force::VIEW_WORDS],
            solver: [0; 884],
            driven: vec![0],
            material_multipliers: [None; 4],
            diagnostics: [0.; 8],
        };
        set(&mut s.body, 0x1990 + 0xe0, 1.);
        set(&mut s.body, 0x18f8, 1.);
        set(&mut s.body, 0x18fc, 1.);
        set(&mut s.body, 0x2218, f32::MAX);
        let before = s.clone();
        assert_eq!(
            initialize(&mut s).unwrap_err().0,
            "nonfinite shift-threshold result"
        );
        assert_eq!(s.body, before.body);
        assert_eq!(s.view, before.view);
        assert_eq!(s.solver, before.solver);
        assert_eq!(s.diagnostics, before.diagnostics);
    }
}
