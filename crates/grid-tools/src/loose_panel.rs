//! A detached body panel as a rigid body (VISUAL_DAMAGE_PARTS_RESEARCH.md
//! "Detached parts as bodies"). The original's scene update turns every part
//! the 9EC9B0 rules detach into debris (9DC270 state 4 -> 9DB0E0 -> 9D9F80):
//!
//! - the bounds of the part node give a box (9A87B0): centre and half extents
//!   of the bounds, rounded by min(0.2, half the smallest half extent);
//! - mass = bounds volume x 600 (9DB0E0 passes CE5C1C for a detached part),
//!   at most 0.015 (CB5068) x car +15A0, as for a lost wheel;
//! - the lost wheel's body words (angular damping 0.2, 0.5 for the mirrors
//!   0x58/0x59, air resistance 0.01 x mass, stationary 0.1/0.1, turnOffTime 1);
//! - the body starts with the car's angular velocity and its velocity at the
//!   part (A69820, A697C0); 9DC270 writes a kick and spin only for wheel-like
//!   parts and 0x60/0x61, so a panel takes the car's motion only.
use crate::{
    car::Car,
    dynamic_props::PieceSpec,
    loose_parts::PARTS,
    loose_wheel::{CarMotion, DENSITY, MASS_CAP, Spawn},
};
use grid_formats::{mesh, pssg, scene};
use grid_world::{Error, physical_pose::Matrix};
use std::{collections::BTreeMap, path::Path};

type V3 = [f32; 3];

/// Rest bounds in car-model space of the body parts the car model has
/// (`loose_parts::PARTS` names; LOD0 draws).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PanelNodes {
    pub bounds: BTreeMap<&'static str, (V3, V3)>,
}

impl PanelNodes {
    /// From the car's `_highLOD.pssg`, LOD0 draws: every draw of joint
    /// `<lod>_<part>` through its rest matrix. Parts without draws, or with
    /// empty or implausibly large bounds, are left out.
    pub fn read(root: &Path, car: &Car) -> Result<Self, Box<dyn std::error::Error>> {
        let bytes = std::fs::read(root.join(car.file("_highLOD.pssg")))?;
        let tree = pssg::read_tree(&bytes)?;
        let sources = mesh::read_meshes(&tree)?;
        let draws = scene::read_draws(&tree, &sources, b"LOD0_")?;
        let mut bounds: BTreeMap<&'static str, (V3, V3)> = BTreeMap::new();
        for draw in &draws {
            let joint = String::from_utf8_lossy(draw.joint);
            let Some((_, name)) = joint.split_once('_') else {
                continue;
            };
            let Some(part) = PARTS.iter().find(|p| **p == name) else {
                continue;
            };
            let entry = bounds
                .entry(part)
                .or_insert(([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]));
            for p in &sources[draw.source].positions[draw.vertices.clone()] {
                let q = scene::transform_point(&draw.transform, *p);
                for (j, v) in q.iter().enumerate() {
                    entry.0[j] = entry.0[j].min(*v);
                    entry.1[j] = entry.1[j].max(*v);
                }
            }
        }
        bounds.retain(|_, (lo, hi)| (0..3).all(|j| (0. ..=5.).contains(&(hi[j] - lo[j]))));
        Ok(Self { bounds })
    }
}

fn rows(m: &[f32; 16]) -> Matrix {
    std::array::from_fn(|r| std::array::from_fn(|c| m[r * 4 + c]))
}

/// The debris body of `part`. `car.offset` is the car-model to physical
/// translation the part meshes are drawn with (+1630, +1634 - +1564, +1638).
pub fn spawn(nodes: &PanelNodes, part: &str, car: &CarMotion) -> Result<Spawn, Error> {
    let (lo, hi) = *nodes
        .bounds
        .get(part)
        .ok_or(Error("loose panel without node bounds"))?;
    let half: V3 = std::array::from_fn(|j| ((hi[j] - lo[j]) * 0.5).max(0.005));
    let centre: V3 = std::array::from_fn(|j| (hi[j] + lo[j]) * 0.5);
    let radius = (half[0].min(half[1]).min(half[2]) * 0.5).min(0.2);
    let mass = (8. * half[0] * half[1] * half[2] * DENSITY).min(car.mass * MASS_CAP);
    let damping = if part.starts_with("wing_mirror") {
        0.5
    } else {
        f32::from_bits(0x3e4c_cccd) // 0.2 (CB870C)
    };
    let pose_rows = rows(&car.pose);
    let local: V3 = std::array::from_fn(|j| centre[j] + car.offset[j]);
    let mut pose = pose_rows;
    pose[3] = std::array::from_fn(|c| {
        (0..3).map(|k| local[k] * pose_rows[k][c]).sum::<f32>() + pose_rows[3][c]
    });
    let spec =
        PieceSpec::loose_panel(half.map(|h| 2. * (h - radius)), radius, mass, damping, pose)?;
    let at = pose[3];
    let r: V3 = std::array::from_fn(|j| at[j] - car.centre[j]);
    let w = car.angular;
    let linear = [
        car.velocity[0] + (w[1] * r[2] - w[2] * r[1]),
        car.velocity[1] + (w[2] * r[0] - w[0] * r[2]),
        car.velocity[2] + (w[0] * r[1] - w[1] * r[0]),
        0.,
    ];
    let angular = [w[0], w[1], w[2], 0.];
    if linear.iter().chain(&angular).any(|v| !v.is_finite()) {
        return Err(Error("nonfinite loose panel launch"));
    }
    Ok(Spawn {
        spec,
        linear,
        angular,
    })
}

/// AUTHORED lying rule (owner's reports of 23 October 2026, F563): the
/// original's box can come to rest on any face, edge or end, but a body
/// panel is a thin shell that does not stand. While a panel's body touches
/// the track, moves slowly and its broadest face is tilted more than 20
/// degrees from level, it is turned over its support edge (a rotation about
/// the lowest corner that every other corner rises from, up to `TIP_RATE`),
/// woken if asleep; the world's contacts and gravity do the rest. After
/// `GIVE_UP` s of that (a panel wedged against a wall) it is left alone.
/// A small panel lying flat on the track rocks on its corners above the
/// angular stationary threshold and never sleeps; lying flat, touching and
/// slower than `ROCKING` rad/s and the linear threshold, its angular
/// velocity is cleared so the world's sleep rule can take it.
const FLAT_COS: f32 = 0.94;
const TIP: f32 = 40.;
const TIP_RATE: f32 = 3.;
const GIVE_UP: f32 = 20.;
/// Corner above the track below it by less than this touches it (the road
/// contact margin is 0.02).
const TOUCH: f32 = 0.05;
const ROCKING: f32 = 1.;
/// 9D9F80's linear stationary threshold (CAFCD4).
const STATIONARY: f32 = 0.1;

/// One tick of the lying rule over every panel body of `props`. `ground` is
/// the track height below a point; `tipping` keeps each panel body's time
/// spent being turned (for `GIVE_UP`). True when a body was woken (waking
/// relinks the active list, so the caller refreshes its cars' copies).
pub fn settle(
    props: &crate::dynamic_props::PropWorld,
    world: &mut grid_world::grouped_physical_frame::Output,
    ground: impl Fn([f32; 3]) -> Option<f32>,
    tipping: &mut BTreeMap<usize, f32>,
    delta: f32,
) -> bool {
    use grid_world::{contact_groups, grouped_physical_frame};
    let word = |b: &contact_groups::Body, o: usize| -> [f32; 3] {
        std::array::from_fn(|j| f32::from_bits(b[o / 4 + j]))
    };
    let mut woke = false;
    for w in props.wheels.iter().filter(|w| w.panel.is_some()) {
        let Some(b) = world.groups.bodies.get(w.body) else {
            continue;
        };
        let crate::dynamic_props::Primitive::Box(size) = w.spec.primitive else {
            continue;
        };
        if (b[0x410 / 4] >> 24) & 255 != 0 || tipping.get(&w.body).is_some_and(|t| *t >= GIVE_UP) {
            continue; // parked far from the player, or given up
        }
        let pose = crate::dynamic_props::body_pose(b);
        let half = size.map(|v| v * 0.5 + w.spec.shell);
        let Some(thin) = (0..3).min_by(|&i, &j| half[i].total_cmp(&half[j])) else {
            continue;
        };
        let up = pose[thin][1];
        let flat = up.abs() >= FLAT_COS;
        if flat && grouped_physical_frame::asleep(world, w.body) {
            continue;
        }
        let (linear, angular) = (word(b, 0x220), word(b, 0x210));
        let length = |v: [f32; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        if flat && (length(linear) >= STATIONARY || length(angular) >= ROCKING) {
            continue;
        }
        if length(linear) > 1.5 || length(angular) > 4. {
            continue; // still tumbling: no touch yet
        }
        let corners: Vec<[f32; 3]> = (0..8)
            .map(|k| {
                std::array::from_fn(|c| {
                    pose[3][c]
                        + (0..3)
                            .map(|a| if k >> a & 1 == 0 { -half[a] } else { half[a] } * pose[a][c])
                            .sum::<f32>()
                })
            })
            .collect();
        let lowest = corners.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
        let Some(low) = corners.iter().find(|p| p[1] == lowest).copied() else {
            continue;
        };
        if ground(low).is_none_or(|y| low[1] - y > TOUCH) {
            continue; // in the air
        }
        if flat {
            let b = &mut world.groups.bodies[w.body];
            for j in 0..3 {
                b[(0x210 / 4) + j] = 0f32.to_bits();
            }
            continue;
        }
        // Turn the broadest face's upward normal toward up.
        let s = if up < 0. { -1. } else { 1. };
        let face = [pose[thin][0] * s, pose[thin][1] * s, pose[thin][2] * s];
        let axis = [-face[2], 0., face[0]];
        let n = length(axis);
        if n < 1e-4 {
            continue;
        }
        let axis = axis.map(|v| v / n);
        if angular.iter().zip(axis).map(|(a, b)| a * b).sum::<f32>() >= TIP_RATE {
            continue;
        }
        let side = [axis[2], 0., -axis[0]];
        let centre = word(b, 0x1c0);
        let Some(pivot) = corners
            .iter()
            .filter(|p| p[1] - lowest < TOUCH)
            .map(|p| std::array::from_fn::<f32, 3, _>(|j| p[j] - centre[j]))
            .min_by(|a, b| {
                let along = |r: &[f32; 3]| r[0] * side[0] + r[2] * side[2];
                along(a).total_cmp(&along(b))
            })
        else {
            continue;
        };
        *tipping.entry(w.body).or_insert(0.) += delta;
        if grouped_physical_frame::asleep(world, w.body) {
            grouped_physical_frame::wake(world, w.body);
            woke = true;
        }
        let b = &mut world.groups.bodies[w.body];
        let (linear, angular) = (word(b, 0x220), word(b, 0x210));
        let turn = axis.map(|v| v * TIP * delta);
        // The pivot keeps still: v -= turn x r.
        let cross = [
            turn[1] * pivot[2] - turn[2] * pivot[1],
            turn[2] * pivot[0] - turn[0] * pivot[2],
            turn[0] * pivot[1] - turn[1] * pivot[0],
        ];
        for j in 0..3 {
            b[(0x210 / 4) + j] = (angular[j] + turn[j]).to_bits();
            b[(0x220 / 4) + j] = (linear[j] - cross[j]).to_bits();
        }
    }
    woke
}

/// How a detached panel's body lies ([`poses`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PanelPose {
    pub car: usize,
    pub part: &'static str,
    pub body: usize,
    pub asleep: bool,
    pub parked: bool,
    pub speed: f32,
    pub spin: f32,
    /// World angular velocity and the full box (outer) size.
    pub angular: [f32; 3],
    pub size: [f32; 3],
    /// |normal of the broadest face . up| (1 = lying flat).
    pub face: f32,
    /// Height above the track below it of the lowest and the fourth-lowest
    /// corner of the rounded box's outer box (a face on the road: both 0).
    pub low: f32,
    pub fourth: f32,
}

/// Diagnostics: every detached panel's body and how it lies on the track
/// (corner heights by `ground` casts; NaN where a cast misses).
pub fn poses(
    props: &crate::dynamic_props::PropWorld,
    bodies: &[grid_world::contact_groups::Body],
    ground: impl Fn([f32; 3]) -> Option<f32>,
) -> Vec<PanelPose> {
    use grid_world::contact_groups;
    props
        .wheels
        .iter()
        .filter_map(|w| {
            let part = w.panel?;
            let b = bodies.get(w.body)?;
            let crate::dynamic_props::Primitive::Box(size) = w.spec.primitive else {
                return None;
            };
            let pose = crate::dynamic_props::body_pose(b);
            let half = size.map(|v| v * 0.5 + w.spec.shell);
            let thin = (0..3).min_by(|&i, &j| half[i].total_cmp(&half[j]))?;
            let mut gaps: Vec<f32> = (0..8)
                .map(|k| {
                    let p: [f32; 3] = std::array::from_fn(|c| {
                        pose[3][c]
                            + (0..3)
                                .map(|a| if k >> a & 1 == 0 { -half[a] } else { half[a] } * pose[a][c])
                                .sum::<f32>()
                    });
                    ground(p).map_or(f32::NAN, |y| p[1] - y)
                })
                .collect();
            gaps.sort_by(f32::total_cmp);
            let v = [0x220, 0x224, 0x228].map(|o| f32::from_bits(b[o / 4]));
            Some(PanelPose {
                car: w.car,
                part,
                body: w.body,
                asleep: b[0x400 / 4] == contact_groups::ASLEEP,
                parked: (b[0x410 / 4] >> 24) & 255 != 0,
                speed: (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt(),
                spin: [0x210, 0x214, 0x218]
                    .map(|o| f32::from_bits(b[o / 4]).powi(2))
                    .iter()
                    .sum::<f32>()
                    .sqrt(),
                angular: [0x210, 0x214, 0x218].map(|o| f32::from_bits(b[o / 4])),
                size: half.map(|h| 2. * h),
                face: pose[thin][1].abs(),
                low: gaps[0],
                fourth: gaps[3],
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_panel_body_is_the_rounded_box_of_its_node_at_the_car() {
        let mut nodes = PanelNodes::default();
        nodes
            .bounds
            .insert("bonnet", ([-0.7, 0.6, 0.5], [0.7, 0.68, 1.7]));
        let car = CarMotion {
            pose: [
                0., 0., -1., 0., 0., 1., 0., 0., 1., 0., 0., 0., 10., 2., 30., 1.,
            ],
            centre: [10., 2., 30.],
            velocity: [5., 0., 0.],
            angular: [0., 1., 0.],
            mass: 1400.,
            offset: [0., -0.1, 0.],
        };
        let s = spawn(&nodes, "bonnet", &car).unwrap();
        // Rounding: half the smallest half extent (0.04 / 2), below 0.2.
        assert!((s.spec.shell - 0.02).abs() < 1e-6);
        match s.spec.primitive {
            crate::dynamic_props::Primitive::Box(size) => {
                for (got, want) in size.iter().zip([1.36, 0.04, 1.16]) {
                    assert!((got - want).abs() < 1e-5, "{size:?}");
                }
            }
            _ => panic!("a box"),
        }
        // 1.4 x 0.08 x 1.2 x 600 = 80.6 kg, above 0.015 x 1400 = 21 kg.
        assert!((s.spec.mass - 21.).abs() < 1e-4);
        // Centre (0, 0.64, 1.1) + offset, through the car's turned frame.
        let at = s.spec.pose[3];
        for (got, want) in at.iter().zip([11.1, 2.54, 30., 1.]) {
            assert!((got - want).abs() < 1e-5, "{at:?}");
        }
        // The car's velocity at the part: v + w x r, r = (1.1, 0.54, 0).
        for (got, want) in s.linear.iter().zip([5., 0., -1.1, 0.]) {
            assert!((got - want).abs() < 1e-5, "{:?}", s.linear);
        }
        assert_eq!(s.angular, [0., 1., 0., 0.]);
        assert!(spawn(&nodes, "boot", &car).is_err());
    }
}
