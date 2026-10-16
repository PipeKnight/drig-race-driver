//! Ground-placement prototype from the observed sampling/clearance contract.
//! Optional observed SIMD normalization matches controlled original numeric cases.
//! Caller-supplied whole-world query semantics remain separate and unverified.
use crate::Error;

#[derive(Debug, Clone, Copy)]
pub enum Normalization {
    Standard,
    /// Observed RSQRTPS seed/refinement, conditionally supported on x86/x86_64.
    ReferenceSse,
}

#[derive(Debug, Clone, Copy)]
pub struct Wheel {
    pub position: [f32; 3],
    pub radius: f32,
    /// Added to local Y in the final ordered clearance pass.
    pub vertical_displacement: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct Pose {
    pub side: [f32; 3],
    pub up: [f32; 3],
    pub forward: [f32; 3],
    pub position: [f32; 3],
    pub corner_misses: usize,
    pub clearance_hits: usize,
}

impl Pose {
    pub fn matrix(&self) -> [f32; 16] {
        [
            self.side[0],
            self.side[1],
            self.side[2],
            0.,
            self.up[0],
            self.up[1],
            self.up[2],
            0.,
            self.forward[0],
            self.forward[1],
            self.forward[2],
            0.,
            self.position[0],
            self.position[1],
            self.position[2],
            1.,
        ]
    }
    fn transform(&self, local: [f32; 3]) -> [f32; 3] {
        std::array::from_fn(|i| {
            (self.position[i] + self.up[i] * local[1])
                + (self.side[i] * local[0] + self.forward[i] * local[2])
        })
    }
}

pub(crate) fn reference_seed(value: f32, reciprocal: bool) -> Result<f32, Error> {
    #[cfg(all(
        any(target_arch = "x86", target_arch = "x86_64"),
        not(feature = "soft-sse")
    ))]
    {
        #[cfg(target_arch = "x86")]
        use std::arch::x86::{_mm_cvtss_f32, _mm_rcp_ps, _mm_rsqrt_ps, _mm_set1_ps};
        #[cfg(target_arch = "x86_64")]
        use std::arch::x86_64::{_mm_cvtss_f32, _mm_rcp_ps, _mm_rsqrt_ps, _mm_set1_ps};
        if !std::is_x86_feature_detected!("sse") {
            return Err(Error("reference arithmetic requires SSE"));
        }
        // SSE was checked; these intrinsics operate only on register values.
        Ok(unsafe {
            _mm_cvtss_f32(if reciprocal {
                _mm_rcp_ps(_mm_set1_ps(value))
            } else {
                _mm_rsqrt_ps(_mm_set1_ps(value))
            })
        })
    }
    #[cfg(any(
        not(any(target_arch = "x86", target_arch = "x86_64")),
        feature = "soft-sse"
    ))]
    {
        // Measured RCPPS/RSQRTPS of the validation CPU; equal bits on all 2^32 inputs.
        Ok(if reciprocal {
            crate::sse_approx::rcp(value)
        } else {
            crate::sse_approx::rsqrt(value)
        })
    }
}

/// Supported mode-one marker alignment with observed SSE length/reciprocal
/// arithmetic. Corners are A, B, C; front extent includes the body Z offset.
/// Random lateral selection, deformation and other alignment modes are separate.
pub fn reference_front_aligned_center(
    corners: [[f32; 3]; 3],
    front_extent: f32,
    lateral: f32,
) -> Result<[f32; 3], Error> {
    if corners
        .into_iter()
        .flatten()
        .chain([front_extent, lateral])
        .any(|v| !v.is_finite())
    {
        return Err(Error("invalid marker alignment input"));
    }
    let longitudinal: [f32; 3] = std::array::from_fn(|i| corners[0][i] - corners[2][i]);
    let transverse: [f32; 3] = std::array::from_fn(|i| corners[1][i] - corners[0][i]);
    let length = |direction: [f32; 3]| -> Result<f32, Error> {
        let squared = (direction[0] * direction[0] + direction[1] * direction[1])
            + direction[2] * direction[2];
        if !squared.is_finite() || squared <= f32::EPSILON * f32::EPSILON {
            return Err(Error("degenerate marker rectangle"));
        }
        let seed = reference_seed(squared, false)?;
        let root = seed * squared;
        Ok((1. - root * seed) * (root * 0.5) + root)
    };
    let long_length = length(longitudinal)?;
    let width = length(transverse)?;
    let long_inverse = reference_seed(long_length, true)?;
    let transverse_inverse = reference_seed(width, true)?;
    let displacement = long_length * 0.5 - front_extent;
    let result = std::array::from_fn(|i| {
        let center = (longitudinal[i] + transverse[i]) * 0.5 + corners[2][i];
        let across = transverse_inverse * (lateral * transverse[i]);
        let center = ((1. - transverse_inverse * width) * across + across) + center;
        let along = long_inverse * (displacement * longitudinal[i]);
        ((1. - long_inverse * long_length) * along + along) + center
    });
    if result.iter().any(|v| !v.is_finite()) {
        return Err(Error("marker result exceeds finite range"));
    }
    Ok(result)
}

fn normalize(v: [f32; 3], mode: Normalization) -> Result<[f32; 3], Error> {
    let squared = (v[0] * v[0] + v[1] * v[1]) + v[2] * v[2];
    if !squared.is_finite() || squared <= f32::EPSILON * f32::EPSILON {
        return Err(Error("degenerate placement direction"));
    }
    match mode {
        Normalization::Standard => Ok(v.map(|x| x / squared.sqrt())),
        Normalization::ReferenceSse => {
            let seed = reference_seed(squared, false)?;
            let inverse = (1. - (squared * seed) * seed) * (seed * 0.5) + seed;
            Ok(v.map(|x| inverse * x))
        }
    }
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Query callback receives X, Z, upper Y and lower Y, returning a finite hit height.
/// This supported prototype requires the center cast to hit; the original's
/// prior-pose fallback, zero-heading fallback and steep-frame branch are unimplemented.
pub fn align(
    center: [f32; 3],
    heading: [f32; 3],
    wheels: [Wheel; 4],
    query: impl FnMut(f32, f32, f32, f32) -> Result<Option<f32>, Error>,
) -> Result<Pose, Error> {
    align_with_normalization(center, heading, wheels, Normalization::Standard, query)
}

pub fn align_with_normalization(
    center: [f32; 3],
    heading: [f32; 3],
    wheels: [Wheel; 4],
    normalization: Normalization,
    mut query: impl FnMut(f32, f32, f32, f32) -> Result<Option<f32>, Error>,
) -> Result<Pose, Error> {
    if center.iter().chain(&heading).any(|v| !v.is_finite())
        || wheels.iter().any(|w| {
            w.position.iter().any(|v| !v.is_finite())
                || !w.radius.is_finite()
                || w.radius <= 0.
                || !w.vertical_displacement.is_finite()
        })
    {
        return Err(Error("invalid ground placement input"));
    }
    let mut cast = |x, z, upper, lower| -> Result<Option<f32>, Error> {
        if [x, z, upper, lower].iter().any(|v: &f32| !v.is_finite()) || upper <= lower {
            return Err(Error("invalid ground placement cast"));
        }
        let hit = query(x, z, upper, lower)?;
        if hit.is_some_and(|v| !v.is_finite() || v > upper || v < lower) {
            return Err(Error("invalid ground placement hit"));
        }
        Ok(hit)
    };
    let horizontal = normalize([heading[0], 0., heading[2]], normalization)?;
    let transverse = [horizontal[2], 0., -horizontal[0]];
    let width = (wheels[0].position[0] - wheels[1].position[0]).abs();
    let length = (wheels[0].position[2] - wheels[2].position[2]).abs();
    if !width.is_finite() || !length.is_finite() || width <= f32::EPSILON || length <= f32::EPSILON
    {
        return Err(Error("degenerate placement wheel span"));
    }
    let ground = cast(center[0], center[2], 10000., -2000.)?.ok_or(Error(
        "center ground cast missed; fallback is unimplemented",
    ))?;
    let mut heights = [[0.; 2]; 2];
    let mut corner_misses = 0;
    for (i, row) in heights.iter_mut().enumerate() {
        let along = (i as f32 - 0.5) * length;
        for (j, height) in row.iter_mut().enumerate() {
            let across = (j as f32 - 0.5) * width;
            let sample: [f32; 3] = std::array::from_fn(|axis| {
                (along * horizontal[axis] + center[axis]) + across * transverse[axis]
            });
            *height = match cast(sample[0], sample[2], 10000., -2000.)? {
                Some(value) => value,
                None => {
                    corner_misses += 1;
                    ground
                }
            };
        }
    }
    let longitudinal = ((heights[0][1] + heights[0][0]) - (heights[1][1] + heights[1][0])) / length;
    let lateral = ((heights[1][0] + heights[0][0]) - (heights[1][1] + heights[0][1])) / width;
    let up = normalize(
        std::array::from_fn(|i| {
            (transverse[i] * lateral + if i == 1 { 2. } else { 0. }) + horizontal[i] * longitudinal
        }),
        normalization,
    )?;
    if up[1] <= 0.5 {
        return Err(Error("steep placement frame is unsupported"));
    }
    let lift = -wheels
        .iter()
        .map(|w| w.position[1] - w.radius)
        .fold(f32::INFINITY, f32::min);
    let mut y = up[1] * lift + ground;
    y += ((up[2] * up[2] + up[0] * up[0]) / up[1]) * lift;
    let side = normalize(cross(up, normalize(heading, normalization)?), normalization)?;
    let forward = cross(side, up);
    let mut pose = Pose {
        side,
        up,
        forward,
        position: [center[0], y, center[2]],
        corner_misses,
        clearance_hits: 0,
    };
    for wheel in wheels {
        let point = pose.transform([
            wheel.position[0],
            wheel.position[1] + wheel.vertical_displacement - wheel.radius,
            wheel.position[2],
        ]);
        if let Some(height) = cast(point[0], point[2], point[1] + 0.5, -10000.)? {
            pose.clearance_hits += 1;
            let clearance = height + 0.005;
            if point[1] < clearance {
                pose.position[1] += clearance - point[1];
            }
        }
    }
    if pose.matrix().iter().any(|v| !v.is_finite()) {
        return Err(Error("placement result exceeds finite f32 range"));
    }
    Ok(pose)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn wheels() -> [Wheel; 4] {
        [[1., 0., -2.], [-1., 0., -2.], [1., 0., 2.], [-1., 0., 2.]].map(|position| Wheel {
            position,
            radius: 0.5,
            vertical_displacement: 0.,
        })
    }
    #[test]
    fn flat_ground_and_corner_misses_have_explicit_behavior() {
        let mut calls = 0;
        let pose = align([3., 99., 4.], [0., 0., 1.], wheels(), |_, _, _, _| {
            calls += 1;
            Ok(if calls == 2 { None } else { Some(2.) })
        })
        .unwrap();
        assert_eq!(calls, 9);
        assert_eq!(pose.position, [3., 2.505, 4.]);
        assert_eq!(pose.up, [0., 1., 0.]);
        assert_eq!(pose.side, [1., 0., 0.]);
        assert_eq!(pose.forward, [0., 0., 1.]);
        assert_eq!(pose.corner_misses, 1);
    }
    #[test]
    fn planar_slope_gives_analytic_normal_and_wheel_clearance() {
        let pose = align([0.; 3], [0., 0., 1.], wheels(), |x, z, _, _| {
            Ok(Some(x * 0.1 + z * 0.2))
        })
        .unwrap();
        let expected = normalize([-0.1, 1., -0.2], Normalization::Standard).unwrap();
        for (&actual, expected) in pose.up.iter().zip(expected) {
            assert!((actual - expected).abs() < 1e-6);
        }
        for wheel in wheels() {
            let bottom = pose.transform([wheel.position[0], -wheel.radius, wheel.position[2]]);
            assert!(bottom[1] - (bottom[0] * 0.1 + bottom[2] * 0.2) >= 0.004999);
        }
    }
    #[test]
    fn ordered_clearance_updates_cast_origins_and_rejects_invalid_inputs() {
        let mut calls = 0;
        let mut upper_heights = Vec::new();
        let pose = align([0.; 3], [0., 0., 1.], wheels(), |_, _, upper, _| {
            calls += 1;
            if calls > 5 {
                upper_heights.push(upper);
            }
            Ok(Some(if calls == 6 { 0.2 } else { 0. }))
        })
        .unwrap();
        assert!((pose.position[1] - 0.705).abs() < 1e-6);
        assert!((upper_heights[1] - upper_heights[0] - 0.205).abs() < 1e-6);
        assert!(align([0.; 3], [0., 1., 0.], wheels(), |_, _, _, _| Ok(Some(0.))).is_err());
        assert!(align([0.; 3], [0., 0., 1.], wheels(), |_, _, _, _| Ok(None)).is_err());
        assert!(
            align([0.; 3], [0., 0., 1.], wheels(), |_, _, _, _| Ok(Some(
                f32::NAN
            )))
            .is_err()
        );
        let mut invalid = wheels();
        invalid[0].radius = -1.;
        assert!(align([0.; 3], [0., 0., 1.], invalid, |_, _, _, _| Ok(Some(0.))).is_err());
    }
}
