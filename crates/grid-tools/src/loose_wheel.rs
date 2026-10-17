//! A lost wheel as a rigid body (WHEEL_DETACHMENT.md "The detached wheel as
//! a body"). The original's scene update turns wheel part 0x2E + i into
//! debris (9FA6E0 -> 9DC270 state 4 -> 9DB0E0 -> 9D9F80):
//!
//! - the bounds of the tyre node 0x2A + i (the wheel node when the tyre is
//!   deflated, car +22E0 + i) give a cylinder about the axle (9A8BB0: radius
//!   half the Z extent, half length half the X extent, shell 0.02);
//! - mass = box volume x 600 (CE5C1C), at most 0.015 (CB5068) x car +15A0;
//! - the body starts with the car's angular velocity and its velocity at the
//!   node (A69820, A697C0); 9DB0E0 then adds the part's kick (+30) and spin
//!   (+40), both car-frame vectors (car +15E0.. rows).
use crate::{car::Car, dynamic_props::PieceSpec, source_vehicle::WheelPose};
use grid_formats::{mesh, pssg, scene};
use grid_world::{Error, physical_pose::Matrix};
use std::path::Path;

type V3 = [f32; 3];

/// Rest bounds in car-model space of the tyre and wheel nodes, by wheel
/// record (RL, RR, FL, FR = parts `_bl`, `_br`, `_fl`, `_fr`).
#[derive(Clone, Debug, PartialEq)]
pub struct WheelNodes {
    pub tyres: [(V3, V3); 4],
    pub wheels: [(V3, V3); 4],
}

const SUFFIX: [&str; 4] = ["bl", "br", "fl", "fr"];
/// 9D9F80 density for a detached part (9DB0E0 passes CE5C1C, 600).
pub const DENSITY: f32 = 600.;
/// 9D9F80 mass cap as a fraction of car +15A0 (CB5068, 0.015).
pub const MASS_CAP: f32 = 0.015;

impl WheelNodes {
    /// From the car's `_highLOD.pssg`, LOD0 draws: every draw of joint
    /// `<lod>_tyre_<suffix>` / `<lod>_wheel_<suffix>` through its rest matrix.
    pub fn read(root: &Path, car: &Car) -> Result<Self, Box<dyn std::error::Error>> {
        let bytes = std::fs::read(root.join(car.file("_highLOD.pssg")))?;
        let tree = pssg::read_tree(&bytes)?;
        let sources = mesh::read_meshes(&tree)?;
        let draws = scene::read_draws(&tree, &sources, b"LOD0_")?;
        let empty = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
        let mut tyres = [empty; 4];
        let mut wheels = [empty; 4];
        for draw in &draws {
            let joint = String::from_utf8_lossy(draw.joint);
            let Some((_, name)) = joint.split_once('_') else {
                continue;
            };
            let Some((kind, side)) = name.split_once('_') else {
                continue;
            };
            let Some(i) = SUFFIX.iter().position(|s| *s == side) else {
                continue;
            };
            let target = match kind {
                "tyre" => &mut tyres[i],
                "wheel" => &mut wheels[i],
                _ => continue,
            };
            for p in &sources[draw.source].positions[draw.vertices.clone()] {
                let q = scene::transform_point(&draw.transform, *p);
                for (j, v) in q.iter().enumerate() {
                    target.0[j] = target.0[j].min(*v);
                    target.1[j] = target.1[j].max(*v);
                }
            }
        }
        for (lo, hi) in tyres.iter().chain(&wheels) {
            // Empty (inverted or NaN) or implausibly large extents.
            if (0..3).any(|j| !(f32::MIN_POSITIVE..=5.).contains(&(hi[j] - lo[j]))) {
                return Err("missing or invalid wheel node bounds".into());
            }
        }
        Ok(Self { tyres, wheels })
    }
}

/// A detached wheel's body, ready for `PropWorld::add_wheel`.
pub struct Spawn {
    pub spec: PieceSpec,
    pub linear: [f32; 4],
    pub angular: [f32; 4],
}

/// The car's state when the wheel comes off.
pub struct CarMotion {
    /// Published car pose (physical +1D0, rows).
    pub pose: [f32; 16],
    /// Mass centre (physical +1C0).
    pub centre: V3,
    /// World linear and angular velocity (physical +220, +210).
    pub velocity: V3,
    pub angular: V3,
    /// Car +15A0 (the mass the cap is taken from).
    pub mass: f32,
    /// Car +1630..+1638: car-model (NED) coordinates to the physical frame;
    /// the wheel anchors are model coordinates plus this offset.
    pub offset: V3,
}

/// Kick of 9DC270 state 4 while a deformation impact is active (+C7A0 = 1):
/// (2.5 + 5 r/255) x clamp(+C754 - 0.25, 0.75, 4) along the normalised
/// (|r - 127.5|, r - 127.5, r - 127.5); car frame. AUTHORED: the random
/// bytes come from a seeded LCG in place of the CRT `rand`.
pub fn kick(strength: Option<f32>, seed: u32) -> V3 {
    let Some(strength) = strength else {
        return [0.; 3];
    };
    let mut state = seed;
    let mut random = || {
        state = state.wrapping_mul(0x41c6_4e6d).wrapping_add(12345);
        ((state >> 16) & 255) as f32
    };
    let k = f32::from_bits(0x3acd_9a67); // 0.0015686 = 0.2 / 127.5
    let x = ((random() - 127.5) * k).abs();
    let y = (random() - 127.5) * k;
    let z = (random() - 127.5) * k;
    let magnitude =
        (random() * f32::from_bits(0x3b80_8081) * 5. + 2.5) * (strength - 0.25).clamp(0.75, 4.);
    let length = (x * x + y * y + z * z).sqrt();
    if length > 0. {
        [x, y, z].map(|v| v / length * magnitude)
    } else {
        [0.; 3]
    }
}

fn rows(m: &[f32; 16]) -> Matrix {
    std::array::from_fn(|r| std::array::from_fn(|c| m[r * 4 + c]))
}
fn product(a: &Matrix, b: &Matrix) -> Matrix {
    std::array::from_fn(|r| std::array::from_fn(|c| (0..4).map(|k| a[r][k] * b[k][c]).sum()))
}
fn translation(v: V3) -> Matrix {
    [
        [1., 0., 0., 0.],
        [0., 1., 0., 0.],
        [0., 0., 1., 0.],
        [v[0], v[1], v[2], 1.],
    ]
}

/// Car-model rest space to car space for wheel `w` (row vectors): the
/// presentation's wheel motion (anchor, suspension, deformation, steering,
/// spin phase).
pub fn wheel_motion(w: &WheelPose) -> Matrix {
    let centre: V3 = std::array::from_fn(|j| {
        w.anchor[j] + w.deformation_translation[j] + w.axis[j] * w.displacement
    });
    let yaw = w.steering[1].atan2(w.steering[0]);
    let (sy, cy) = yaw.sin_cos();
    let (sx, cx) = w.phase.sin_cos();
    let rx = [
        [1., 0., 0., 0.],
        [0., cx, sx, 0.],
        [0., -sx, cx, 0.],
        [0., 0., 0., 1.],
    ];
    let ry = [
        [cy, 0., -sy, 0.],
        [0., 1., 0., 0.],
        [sy, 0., cy, 0.],
        [0., 0., 0., 1.],
    ];
    let mut m = translation(w.anchor.map(|v| -v));
    for next in [rx, ry, w.deformation_orientation, translation(centre)] {
        m = product(&m, &next);
    }
    m
}

/// The debris body of wheel record `wheel`.
pub fn spawn(
    nodes: &WheelNodes,
    wheel: usize,
    deflated: bool,
    car: &CarMotion,
    motion: &Matrix,
    spin: f32,
    kick: V3,
) -> Result<Spawn, Error> {
    let (lo, hi) = if deflated {
        nodes.wheels[wheel]
    } else {
        nodes.tyres[wheel]
    };
    let size: V3 = std::array::from_fn(|j| hi[j] - lo[j]);
    let centre: V3 = std::array::from_fn(|j| (hi[j] + lo[j]) * 0.5);
    let mass = (size[0] * size[1] * size[2] * DENSITY).min(car.mass * MASS_CAP);
    // Cylinder local Y along the model X axle; local X = model Y, local Z =
    // -model Z keeps the frame right-handed.
    let local = [
        [0., 1., 0., 0.],
        [1., 0., 0., 0.],
        [0., 0., -1., 0.],
        [centre[0], centre[1], centre[2], 1.],
    ];
    let pose_rows = rows(&car.pose);
    let pose = product(
        &product(&product(&local, &translation(car.offset)), motion),
        &pose_rows,
    );
    let spec = PieceSpec::loose_wheel(size[2] * 0.5, size[0], mass, pose)?;
    let at = pose[3];
    let r: V3 = std::array::from_fn(|j| at[j] - car.centre[j]);
    let w = car.angular;
    let along = |v: V3| -> V3 {
        std::array::from_fn(|j| {
            v[0] * pose_rows[0][j] + v[1] * pose_rows[1][j] + v[2] * pose_rows[2][j]
        })
    };
    let kick = along(kick);
    let linear = [
        car.velocity[0] + (w[1] * r[2] - w[2] * r[1]) + kick[0],
        car.velocity[1] + (w[2] * r[0] - w[0] * r[2]) + kick[1],
        car.velocity[2] + (w[0] * r[1] - w[1] * r[0]) + kick[2],
        0.,
    ];
    let axle = along([spin, 0., 0.]);
    let angular = [w[0] + axle[0], w[1] + axle[1], w[2] + axle[2], 0.];
    if linear.iter().chain(&angular).any(|v| !v.is_finite()) {
        return Err(Error("nonfinite loose wheel launch"));
    }
    Ok(Spawn {
        spec,
        linear,
        angular,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> [f32; 16] {
        [
            1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
        ]
    }

    fn nodes() -> WheelNodes {
        let tyre = ([0.65, 0.0, 1.0], [0.85, 0.62, 1.62]);
        WheelNodes {
            tyres: [tyre; 4],
            wheels: [([0.68, 0.06, 1.06], [0.82, 0.56, 1.56]); 4],
        }
    }

    #[test]
    fn a_wheel_body_is_a_capped_cylinder_about_the_axle_at_the_tyre() {
        let car = CarMotion {
            pose: identity(),
            centre: [0.; 3],
            velocity: [0., 0., 20.],
            angular: [0., 1., 0.],
            mass: 1140.,
            offset: [0.; 3],
        };
        let motion = translation([0.; 3]);
        let s = spawn(&nodes(), 2, false, &car, &motion, 50., [0.; 3]).unwrap();
        // 0.2 x 0.62 x 0.62 x 600 = 46 kg, capped at 0.015 x 1140.
        assert!((s.spec.mass - 17.1).abs() < 1e-4);
        let crate::dynamic_props::Primitive::Cylinder { radius, height } = s.spec.primitive else {
            panic!("a cylinder");
        };
        assert!((radius - 0.31).abs() < 1e-6 && (height - 0.2).abs() < 1e-6);
        assert!((s.spec.pose[1][0] - 1.).abs() < 1e-6, "axle along car X");
        assert!((s.spec.pose[3][0] - 0.75).abs() < 1e-6 && (s.spec.pose[3][2] - 1.31).abs() < 1e-6);
        // Point velocity: v + w x r, r = (0.75, 0.31, 1.31): w x r = (1.31, 0, -0.75).
        assert!((s.linear[0] - 1.31).abs() < 1e-5 && (s.linear[2] - 19.25).abs() < 1e-5);
        assert_eq!(s.angular, [50., 1., 0., 0.]);
        // The body offset moves the model-space tyre into the car frame.
        let shifted = CarMotion {
            offset: [0., -0.39, -0.15],
            ..car
        };
        let moved = spawn(&nodes(), 2, false, &shifted, &motion, 50., [0.; 3]).unwrap();
        assert!((moved.spec.pose[3][1] - (0.31 - 0.39)).abs() < 1e-6);
        let deflated = spawn(&nodes(), 2, true, &car, &motion, 0., [0.; 3]).unwrap();
        assert!(deflated.spec.mass < s.spec.mass + 1e-6);
    }

    #[test]
    fn the_kick_needs_an_active_impact_and_is_seeded() {
        assert_eq!(kick(None, 7), [0.; 3]);
        let a = kick(Some(1.), 7);
        assert_eq!(a, kick(Some(1.), 7));
        assert!(a[0] >= 0.);
        let m = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
        assert!((2.5 * 0.75..=7.5 * 0.75 + 1e-4).contains(&m), "{m}");
    }
}
