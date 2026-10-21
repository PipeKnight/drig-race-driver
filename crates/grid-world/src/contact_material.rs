//! Supplied contact result/material arithmetic; collision traversal remains separate.
use crate::{Error, contact_ray, wheel_radius};

#[derive(Clone, Copy, Debug)]
pub struct Material {
    pub time_based: bool,
    pub wavelength: f32,
    pub magnitude: f32,
    pub real_proportion: f32,
    pub car_proportion: f32,
    /// Runtime material +a4, loaded from surface MECHANICS depth.
    pub height_offset: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Input {
    pub ray: contact_ray::Ray,
    pub body_y: [f32; 4],
    pub suspension_axis: [f32; 4],
    pub query_normal: [f32; 4],
    pub fraction: f32,
    pub hit_present: bool,
    pub material: Material,
    pub wheel_ordinal: usize,
    pub speed: f32,
    pub clock: f32,
    /// Runtime body +12ac; provenance belongs to the caller.
    pub car_factor: f32,
    pub calculate_bump: bool,
    pub previous_bump: f32,
    pub new_bump_before: f32,
    pub wheel_angle: f32,
    pub wheel_radius: f32,
    pub radius_samples: [[f32; 2]; 5],
    /// Runtime wheel +140; provenance belongs to the caller.
    pub wheel_height_offset: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Output {
    pub accepted: bool,
    pub floor: f32,
    pub new_bump: f32,
    pub wheel_normal: [f32; 4],
    pub solver_normal: [f32; 4],
}

fn byte_noise(x: i32, z: i32) -> Result<i32, Error> {
    let squared = (x * 256 + z) as f32 + f32::from_bits(0x400a_2a52);
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        #[cfg(target_arch = "x86")]
        use std::arch::x86::{_mm_cvtss_f32, _mm_set_ss, _mm_sqrt_ss};
        #[cfg(target_arch = "x86_64")]
        use std::arch::x86_64::{_mm_cvtss_f32, _mm_set_ss, _mm_sqrt_ss};
        if !std::is_x86_feature_detected!("sse") {
            return Err(Error("reference material noise requires SSE"));
        }
        // SSE was checked; these intrinsics operate only on register values.
        let root = unsafe { _mm_cvtss_f32(_mm_sqrt_ss(_mm_set_ss(squared))) };
        Ok(((root * f32::from_bits(0x4971_0d1a)) as i32) & 255)
    }
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    {
        // SQRTSS is the correctly rounded IEEE square root, as is f32::sqrt.
        Ok(((squared.sqrt() * f32::from_bits(0x4971_0d1a)) as i32) & 255)
    }
}

/// Periodic bilinear integer noise; two stages of truncation are intentional.
fn noise(x: f32, z: f32) -> Result<f32, Error> {
    let x = x * 256.;
    let z = z * 256.;
    if !x.is_finite() || !z.is_finite() || x.abs() >= 2_147_483_520. || z.abs() >= 2_147_483_520. {
        return Err(Error(
            "material coordinates exceed integer conversion range",
        ));
    }
    let x = (x as i32) & 65535;
    let z = (z as i32) & 65535;
    let (ix, iz, fx, fz) = (x >> 8, z >> 8, x & 255, z & 255);
    let next_x = (ix + 1) & 255;
    let next_z = (iz + 1) & 255;
    let a = (byte_noise(ix, iz)? * (256 - fx) + byte_noise(next_x, iz)? * fx) >> 8;
    let b = (byte_noise(ix, next_z)? * (256 - fx) + byte_noise(next_x, next_z)? * fx) >> 8;
    Ok((((a * (256 - fz) + b * fz) >> 8) - 128) as f32)
}

pub fn resolve(input: Input) -> Result<Output, Error> {
    let m = input.material;
    if input.wheel_ordinal >= 4
        || input
            .body_y
            .iter()
            .chain(&input.suspension_axis)
            .chain(&input.query_normal)
            .chain(&input.ray.start)
            .any(|v| !v.is_finite())
        || [
            input.ray.low,
            input.ray.high,
            input.fraction,
            input.speed,
            input.clock,
            input.car_factor,
            input.previous_bump,
            input.new_bump_before,
            input.wheel_height_offset,
            m.wavelength,
            m.magnitude,
            m.real_proportion,
            m.car_proportion,
            m.height_offset,
        ]
        .iter()
        .any(|v| !v.is_finite())
        || m.wavelength <= 0.
        // The wheel ray keeps the query's tolerance fractions (a hit at the
        // ray start can be -8.5e-8 for an upside-down car, F458); only the
        // port's own guard rejected them.
        || !(crate::triangle::FRACTION_LOW..=crate::triangle::FRACTION_HIGH)
            .contains(&input.fraction)
    {
        return Err(Error("invalid contact material inputs"));
    }
    let n = input.query_normal;
    let a = input.suspension_axis;
    let dot = (a[1] * n[1] + a[0] * n[0]) + (a[3] * n[3] + a[2] * n[2]);
    let accepted = input.hit_present && dot > f32::from_bits(0x3ed8_616c);
    let mut floor = -1000.;
    let mut new_bump = input.new_bump_before;
    let wheel_normal = if accepted { n } else { input.body_y };
    if accepted {
        floor = (input.ray.low - input.ray.high) * input.fraction + input.ray.high;
        let bump = if input.calculate_bump {
            let (x, z) = if m.time_based {
                (input.clock / m.wavelength, input.wheel_ordinal as f32)
            } else {
                let inverse = 1. / m.wavelength;
                (inverse * input.ray.start[0], inverse * input.ray.start[2])
            };
            let amplitude = (input.speed * f32::from_bits(0x3dcc_cccd)).min(1.);
            let mut bump = noise(x, z)? * m.magnitude;
            bump *= amplitude;
            bump *= f32::from_bits(0x3c00_0000);
            bump *= input.car_factor * m.car_proportion + (1. - m.car_proportion);
            new_bump = bump;
            bump
        } else {
            input.previous_bump
        };
        floor += m.real_proportion * bump;
        floor -= m.height_offset;
        let radius_delta = wheel_radius::adjustment_wide(
            input.wheel_angle,
            input.wheel_radius,
            input.radius_samples,
        )?;
        floor = ((radius_delta + f64::from(floor)) - f64::from(input.wheel_height_offset)) as f32;
    }
    Ok(Output {
        accepted,
        floor,
        new_bump,
        wheel_normal,
        solver_normal: contact_ray::regularize(wheel_normal, input.suspension_axis)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn periodic_noise_wraps_and_retains_integer_interpolation() {
        assert_eq!(noise(0., 0.).unwrap(), noise(256., 256.).unwrap());
        let a = noise(1., 2.).unwrap();
        assert!((-128. ..=127.).contains(&a));
        assert!(noise(f32::MAX, 0.).is_err());
    }

    #[test]
    fn strict_acceptance_and_supplied_offsets_determine_floor() {
        let input = Input {
            ray: contact_ray::Ray {
                start: [0., 1., 0., 1.],
                end: [0., 0., 0., 1.],
                low: -0.25,
                high: 0.75,
            },
            body_y: [0., 1., 0., 0.],
            suspension_axis: [0., 1., 0., 0.],
            query_normal: [0., 1., 0., 0.],
            fraction: 0.5,
            hit_present: true,
            material: Material {
                time_based: false,
                wavelength: 1.,
                magnitude: 0.,
                real_proportion: 0.5,
                car_proportion: 0.,
                height_offset: 0.125,
            },
            wheel_ordinal: 0,
            speed: 0.,
            clock: 0.,
            car_factor: 1.,
            calculate_bump: false,
            previous_bump: 0.25,
            new_bump_before: 0.2,
            wheel_angle: 0.,
            wheel_radius: 0.5,
            radius_samples: [[0., 0.75]; 5],
            wheel_height_offset: 0.125,
        };
        let result = resolve(input).unwrap();
        assert!(result.accepted);
        // .25 ray height + .125 material bump - .125 material offset
        // + .25 selected radius change - .125 wheel offset.
        assert_eq!(result.floor, 0.375);
        assert_eq!(result.new_bump, 0.2);
        let threshold = f32::from_bits(0x3ed8_616c);
        let rejected = resolve(Input {
            query_normal: [0., threshold, 0., 0.],
            ..input
        })
        .unwrap();
        assert!(!rejected.accepted);
        assert_eq!(rejected.floor, -1000.);
        assert_eq!(rejected.wheel_normal, input.body_y);
        assert!(
            !resolve(Input {
                hit_present: false,
                ..input
            })
            .unwrap()
            .accepted
        );
        // The wheel ray keeps the query's tolerance fractions: a hit at the
        // ray start of an upside-down car (Sarthe soak, F458) is accepted;
        // fractions outside the query's range are still rejected.
        assert!(
            resolve(Input {
                fraction: -8.527_584e-8,
                ..input
            })
            .is_ok()
        );
        assert!(
            resolve(Input {
                fraction: -2e-6,
                ..input
            })
            .is_err()
        );
    }
}
