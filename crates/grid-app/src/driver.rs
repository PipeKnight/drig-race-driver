//! The player's driver in the cockpit (docs/COCKPIT.md "Driver"):
//! `characters/models/driver_model.pssg` skinned on its 75 joints, seated by
//! the cockpit's "Neutral" pose with the steering poses added, the arms
//! reaching the wheel's hand dummies and the legs the foot dummies by
//! two-bone IK (driver update 0x85DA80). The head-cam hides the head
//! (DisableDriverHead), so the model draws its `_x2` instances: the body
//! without helmet and visor (0x85D310).
use super::{Car, Part, read};
use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::NoFrustumCulling,
    mesh::{
        Indices, PrimitiveTopology, VertexAttributeValues,
        skinning::{SkinnedMesh, SkinnedMeshInverseBindposes},
    },
    prelude::*,
};
use grid_formats::{material, mesh, neanim, pssg, scene, texture};
use std::{error::Error, path::Path};

/// The model's level-of-detail slot with the head hidden: the instances
/// named `_x2` (0x84A9D0 reads the digit after `_x`, minus one; 0x85D310
/// picks slot 1 whenever the view sets DisableDriverHead).
const HEADLESS_SLOT: u32 = 1;

/// Weight below which the driver update adds no steering pose (0x852030)
/// and the evaluator skips an entry (0xACD760 keeps weights >= 0.001).
const STEER_MIN: f32 = 0.01;

/// The driver's skeleton, poses and IK limbs.
#[derive(Clone, Debug, Default)]
pub struct Driver {
    /// Every JOINTNODE and NODE of the model, parents first.
    joints: Vec<Joint>,
    /// The skin's joints in skin order (indices into `joints`).
    skin: Vec<usize>,
    inverse_bind: Vec<Mat4>,
    /// The model frame in the car frame: the cockpit anchor and the
    /// interior's `D_bip_position` (0x861C50).
    root: Mat4,
    /// int_<stem>_driver_anims "Neutral", weight 1 (0x84BA20).
    neutral: Pose,
    /// driver_anims set clips 1..4: steer to left, left, to right, right
    /// (additive: quaternion and translation differences).
    steer: [Pose; 4],
    limbs: Vec<Limb>,
}

#[derive(Clone, Debug)]
struct Joint {
    name: String,
    parent: Option<usize>,
    /// The file's local transform.
    local: Mat4,
}

/// One clip sampled at its first key: per joint, the rotation and
/// translation it writes.
type Pose = Vec<(Option<Vec4>, Option<Vec3>)>;

/// A two-bone limb (0x850A90) and the interior node it reaches for.
#[derive(Clone, Debug)]
struct Limb {
    upper: usize,
    lower: usize,
    end: usize,
    hint: usize,
    target: Target,
}

#[derive(Clone, Copy, Debug)]
enum Target {
    /// A node of the interior clips ([`crate::interior_anim::Interior`]): the
    /// hand dummies under the wheel, the foot dummies the pedal clips move.
    Keyed(usize),
    /// An interior node no clip keys, in the car frame.
    Fixed(Mat4),
}

/// One skin joint's entity and its index in the skeleton.
#[derive(Component)]
pub struct DriverJoint(usize);

/// Appends the driver's body to the cockpit parts and returns the driver,
/// or `None` when the cockpit has no `D_bip_position` or no Neutral pose.
pub fn append(
    root: &Path,
    stem: &str,
    interior: &pssg::Tree,
    anchor: Mat4,
    car: &mut Car,
    animated: Option<&crate::interior_anim::Interior>,
) -> Result<Option<Driver>, Box<dyn Error>> {
    let Some(seat) = crate::cockpit::named(interior, b"NODE", b"D_bip_position") else {
        return Ok(None);
    };
    let anims = root.join(format!("interiors/{stem}/int_{stem}_driver_anims.pssg"));
    if !anims.is_file() {
        return Ok(None);
    }
    let bytes = read(&root.join("characters/models/driver_model.pssg"))?;
    let tree = pssg::read_tree(&bytes)?;
    let mut joints = Vec::new();
    let mut index = std::collections::HashMap::new();
    // The IK hints (LA_hint, ...) and Bip01 Footsteps are plain NODEs in the
    // joint hierarchy.
    for i in (0..tree.nodes.len()).filter(|&i| matches!(tree.name(i), b"JOINTNODE" | b"NODE")) {
        let parent = tree.nodes[i].parent.and_then(|p| index.get(&p).copied());
        index.insert(i, joints.len());
        joints.push(Joint {
            name: String::from_utf8_lossy(tree.string(i, b"id")?).into_owned(),
            parent,
            local: Mat4::from_cols_array(&scene::matrix(&tree, i)?),
        });
    }
    let meshes = mesh::read_object_meshes(&tree)?;
    let skin = scene::read_skin(&tree, &meshes, b"body_x4")?;
    let skin_joints: Vec<usize> = skin.joints.iter().map(|j| index[j]).collect();
    let mut diffuse = std::collections::BTreeMap::new();
    for b in material::read_texture_bindings(&tree)? {
        if b.sampler == b"TDiffuseAlphaMap" && b.container.is_none() {
            diffuse.insert(b.shader, b.texture);
        }
    }
    let utf8 = |b: &[u8]| String::from_utf8_lossy(b).into_owned();
    let mut needed = Vec::new();
    for &(id, shader, source) in &skin.instances {
        if lod_slot(id) != HEADLESS_SLOT {
            continue;
        }
        let source = &meshes[source];
        let (bound, weights) = source.skin_weights.as_ref().expect("read_skin checked the weights");
        let mut geometry = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        );
        geometry.insert_attribute(Mesh::ATTRIBUTE_POSITION, source.positions.clone());
        geometry.insert_attribute(Mesh::ATTRIBUTE_NORMAL, source.normals.clone());
        geometry.insert_attribute(Mesh::ATTRIBUTE_UV_0, source.texcoords.clone());
        geometry.insert_attribute(
            Mesh::ATTRIBUTE_JOINT_INDEX,
            VertexAttributeValues::Uint16x4(bound.iter().map(|j| j.map(u16::from)).collect()),
        );
        geometry.insert_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT, weights.clone());
        geometry.insert_indices(Indices::U16(source.indices.clone()));
        let material = format!("car0/interior/driver/{}", utf8(shader));
        if let Some(texture) = diffuse.get(shader) {
            car.bindings.insert(material.clone(), format!("car0/interior/driver/{}", utf8(texture)));
            needed.push(*texture);
        }
        car.parts.push(Part {
            name: format!("interior/driver/{}", utf8(id)),
            joint: format!("{}body_x4", crate::cockpit::JOINT),
            material,
            mesh: geometry,
            matrices: vec![anchor * Mat4::from_cols_array(&scene::node_matrix(interior, seat)?)],
            wheel: None,
        });
    }
    for t in texture::read_named(&tree, &needed)? {
        car.textures.insert(format!("car0/interior/driver/{}", utf8(t.id)), t.validated_dds().0);
    }
    let sets = neanim::read_sets(&pssg::read_tree(&read(&anims)?)?)?;
    let Some(neutral) = sets.first().and_then(|s| s.clips.iter().find(|c| c.name == "Neutral")) else {
        return Ok(None);
    };
    let characters = neanim::read_sets(&pssg::read_tree(&read(&root.join("characters/anims/driver_anims.pssg"))?)?)?;
    let Some(clips) = characters.first().map(|s| &s.clips).filter(|c| c.len() > 4) else {
        return Err("driver_anims.pssg lacks the steering clips".into());
    };
    let find = |name: &str| joints.iter().position(|j| j.name == name).ok_or(format!("driver joint {name} absent"));
    let limb = |side: &str, upper: &str, lower: &str, end: &str, hint: &str, target| -> Result<Limb, String> {
        Ok(Limb {
            upper: find(&format!("Bip01 {side} {upper}"))?,
            lower: find(&format!("Bip01 {side} {lower}"))?,
            end: find(&format!("Bip01 {side} {end}"))?,
            hint: find(hint)?,
            target,
        })
    };
    let target = |name: &str| -> Result<Target, Box<dyn Error>> {
        if let Some(n) = animated.and_then(|c| c.node(name)) {
            return Ok(Target::Keyed(n));
        }
        let node = crate::cockpit::named(interior, b"NODE", name.as_bytes()).ok_or(format!("{name} absent"))?;
        Ok(Target::Fixed(anchor * Mat4::from_cols_array(&scene::node_matrix(interior, node)?)))
    };
    let limbs = vec![
        limb("L", "UpperArm", "Forearm", "Hand", "LA_hint", target("D_Lhand_Dummy")?)?,
        limb("R", "UpperArm", "Forearm", "Hand", "RA_hint", target("D_Rhand_Dummy")?)?,
        limb("L", "Thigh", "Calf", "Foot", "LL_hint", target("D_Lfoot_Dummy")?)?,
        limb("R", "Thigh", "Calf", "Foot", "RL_hint", target("D_Rfoot_Dummy")?)?,
    ];
    let pose = |clip: &neanim::Clip| -> Pose {
        let mut pose: Pose = vec![(None, None); joints.len()];
        for c in &clip.channels {
            let Some(j) = joints.iter().position(|j| j.name == c.target) else { continue };
            let v = Vec4::from_array(c.keys.sample(0.));
            match c.semantic.as_str() {
                "rotation_quaternion" => *pose[j].0.get_or_insert(Vec4::ZERO) += v,
                "translation" => *pose[j].1.get_or_insert(Vec3::ZERO) += v.truncate(),
                _ => {}
            }
        }
        pose
    };
    let driver = Driver {
        neutral: pose(neutral),
        steer: [pose(&clips[1]), pose(&clips[2]), pose(&clips[3]), pose(&clips[4])],
        root: anchor * Mat4::from_cols_array(&scene::node_matrix(interior, seat)?),
        joints,
        skin: skin_joints,
        inverse_bind: skin.inverse_bind.iter().map(Mat4::from_cols_array).collect(),
        limbs,
    };
    println!(
        "cockpit_driver joints={} skin={} parts={} limbs={}",
        driver.joints.len(),
        driver.skin.len(),
        car.parts.iter().filter(|p| p.name.starts_with("interior/driver/")).count(),
        driver.limbs.len(),
    );
    Ok(Some(driver))
}

/// The level-of-detail slot of a model instance (0x84A9D0): the number
/// after `_x`, minus one, at most 5; 0 without one.
fn lod_slot(id: &[u8]) -> u32 {
    let id = String::from_utf8_lossy(id);
    let Some((_, digits)) = id.rsplit_once("_x") else { return 0 };
    match digits.parse::<i64>() {
        Ok(n) if n >= 1 => (n - 1).min(5) as u32,
        _ => 0,
    }
}

impl Driver {
    /// The joints' model-frame transforms for the steering command `s`
    /// (+1 = full right), the hands reaching for `hands` (car frame) when
    /// the cockpit has them.
    fn pose(&self, s: f32, interior: &[Mat4]) -> Vec<Mat4> {
        let (left, right) = ((-s).max(0.), s.max(0.));
        // 0x851730 → 0x852030: to-left, left, to-right, right.
        let weights = [(2. * left).min(1.), (2. * left - 1.).max(0.), (2. * right).min(1.), (2. * right - 1.).max(0.)];
        let mut locals: Vec<Mat4> = Vec::with_capacity(self.joints.len());
        for (j, joint) in self.joints.iter().enumerate() {
            let (mut q, mut t) = self.neutral[j];
            for (pose, &w) in self.steer.iter().zip(&weights).filter(|(_, w)| **w > STEER_MIN) {
                if let Some(d) = pose[j].0 {
                    *q.get_or_insert(Vec4::ZERO) += w * d;
                }
                if let Some(d) = pose[j].1 {
                    *t.get_or_insert(Vec3::ZERO) += w * d;
                }
            }
            // A slot no entry writes keeps the node's own transform.
            let (_, file_q, file_t) = joint.local.to_scale_rotation_translation();
            let q = q.map_or(file_q, |q| Quat::from_vec4(q).normalize());
            locals.push(Mat4::from_rotation_translation(q, t.unwrap_or(file_t)));
        }
        let to_model = self.root.inverse();
        // Legs first (0x852EC0), then arms (0x852FD0).
        for limb in self.limbs.iter().rev() {
            let target = match limb.target {
                Target::Keyed(n) => interior[n],
                Target::Fixed(m) => m,
            };
            let world = self.world(&locals);
            ik::reach(&mut locals, &world, &self.parents(), limb, to_model * target);
        }
        self.world(&locals)
    }

    fn world(&self, locals: &[Mat4]) -> Vec<Mat4> {
        let mut world: Vec<Mat4> = Vec::with_capacity(locals.len());
        for (joint, local) in self.joints.iter().zip(locals) {
            world.push(joint.parent.map_or(*local, |p| world[p] * *local));
        }
        world
    }

    fn parents(&self) -> Vec<Option<usize>> {
        self.joints.iter().map(|j| j.parent).collect()
    }
}

pub fn install(app: &mut App) {
    app.add_systems(Update, (attach, pose).chain().after(crate::cockpit::animate_interior).before(crate::drive_present));
}

/// Gives each driver part its joints once it is spawned.
fn attach(
    mut commands: Commands,
    cockpit: Option<Res<crate::cockpit::Cockpit>>,
    parts: Query<(Entity, &Name), Added<crate::cockpit::InteriorPart>>,
    mut bindposes: ResMut<Assets<SkinnedMeshInverseBindposes>>,
) {
    let Some(driver) = cockpit.as_ref().and_then(|c| c.driver.as_ref()) else {
        return;
    };
    for (entity, name) in &parts {
        if !name.as_str().starts_with("interior/driver/") {
            continue;
        }
        let rest = driver.world(&driver.joints.iter().map(|j| j.local).collect::<Vec<_>>());
        let joints = (driver.skin.iter())
            .map(|&j| commands.spawn((DriverJoint(j), Transform::from_matrix(rest[j]), ChildOf(entity))).id())
            .collect();
        commands.entity(entity).insert((
            SkinnedMesh { inverse_bindposes: bindposes.add(SkinnedMeshInverseBindposes::from(driver.inverse_bind.clone())), joints },
            NoFrustumCulling,
        ));
    }
}

/// Poses the driver for the human steering command −(vehicle +0xE30), the
/// wheel's own input (docs/COCKPIT.md "Steering wheel"), reaching for this
/// frame's interior dummies.
fn pose(
    driving: Option<Res<crate::Driving>>,
    cockpit: Option<Res<crate::cockpit::Cockpit>>,
    interior: Res<crate::cockpit::InteriorPose>,
    mut joints: Query<(&DriverJoint, &mut Transform)>,
) {
    let (Some(driving), Some(cockpit)) = (driving, cockpit) else {
        return;
    };
    let Some(driver) = &cockpit.driver else {
        return;
    };
    let vehicle = &driving.runtime.state.car.owner.vehicle;
    let s = -vehicle.get(0xe30 / 4).copied().map_or(0., f32::from_bits);
    let world = driver.pose(s, &interior.0);
    for (DriverJoint(j), mut transform) in &mut joints {
        *transform = Transform::from_matrix(world[*j]);
    }
}

/// The two-bone solver NeAnimIKLimb (0xB00000) with the flags GRID never
/// changes from the constructor's (0xAFF960: rotateEffector on, the bend
/// direction and plane not inverted).
mod ik {
    use super::Limb;
    use bevy::prelude::*;

    /// Rotates the limb's upper and lower bones so its end reaches
    /// `target`, bent in the plane through the hint joint, and gives the
    /// end the target's rotation. Translations stay; the bones' old twist
    /// is replaced by the bend-plane frame. `world` is `locals` composed.
    pub fn reach(locals: &mut [Mat4], world: &[Mat4], parents: &[Option<usize>], limb: &Limb, target: Mat4) {
        let position = |m: &Mat4| m.w_axis.truncate();
        let rotation = |m: &Mat4| Mat3::from_quat(m.to_scale_rotation_translation().1);
        let p0 = position(&world[limb.upper]);
        let (l1, l2) = (position(&locals[limb.lower]).length(), position(&locals[limb.end]).length());
        let t = position(&target);
        let d = t - p0;
        let distance = d.length();
        let dir = d / distance;
        let n = dir.cross((position(&world[limb.hint]) - p0).normalize()).normalize();
        let u = n.cross(dir).normalize();
        let (elbow, end) = if distance > l1 + l2 {
            // Out of reach: the limb points straight at the target.
            (p0 + l1 * dir, p0 + (l1 + l2) * dir)
        } else {
            let c = (distance * distance + l1 * l1 - l2 * l2) / (l1 * distance * 2.);
            if !(-1. ..=1.).contains(&c) {
                // Too close: folded along the target direction.
                let fold = if l1 <= l2 { -dir } else { dir };
                let e = p0 + l1 * fold;
                (e, e - l2 * fold)
            } else {
                // acos and sin in double precision (0xBF4067, 0xBF4E44).
                let perp = (f64::from(f64::from(c).acos() as f32).sin() as f32) * l1;
                let along = (l1 * l1 - perp * perp).sqrt().copysign(c);
                (p0 + along * dir + perp * u, t)
            }
        };
        let (a, b) = ((elbow - p0).normalize(), (end - elbow).normalize());
        // Bone frames: X along the bone, Y the bend-plane normal.
        let upper = Mat3::from_cols(a, n, a.cross(n));
        let lower = Mat3::from_cols(b, n, b.cross(n));
        let parent = parents[limb.upper].map_or(Mat3::IDENTITY, |p| rotation(&world[p]));
        let set = |m: &mut Mat4, r: Mat3| *m = Mat4::from_rotation_translation(Quat::from_mat3(&r).normalize(), position(m));
        set(&mut locals[limb.upper], parent.transpose() * upper);
        set(&mut locals[limb.lower], upper.transpose() * lower);
        set(&mut locals[limb.end], lower.transpose() * rotation(&target));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_of_detail_slot_reads_the_digit_after_x() {
        assert_eq!(lod_slot(b"!w_body_x1"), 0);
        assert_eq!(lod_slot(b"!7_body_x2"), HEADLESS_SLOT);
        assert_eq!(lod_slot(b"!GC_body_x6"), 5);
        assert_eq!(lod_slot(b"body"), 0);
    }

    /// Lacetti: the headless body is drawn, and at full left, centre and
    /// full right both hands reach their wheel dummies with the dummies'
    /// rotation.
    #[test]
    #[ignore = "needs GRID_GAME_ROOT"]
    fn lacetti_hands_reach_the_wheel_dummies() {
        let root = std::path::PathBuf::from(std::env::var("GRID_GAME_ROOT").expect("GRID_GAME_ROOT"));
        let mut car = Car {
            parts: Vec::new(),
            textures: Default::default(),
            bindings: Default::default(),
            decal_materials: Default::default(),
            object_glass_materials: Default::default(),
            object_two_sided_materials: Default::default(),
            center: Vec3::ZERO,
            radius: 0.,
            track: false,
            liveries: Vec::new(),
            low_lods: Vec::new(),
            ornament_lods: Default::default(),
        };
        let cockpit = crate::cockpit::append(&root, "lac", &mut car, Mat4::IDENTITY).unwrap();
        let (driver, clips) = (cockpit.driver.unwrap(), cockpit.interior.unwrap());
        let drawn: Vec<_> = car.parts.iter().filter(|p| p.name.starts_with("interior/driver/")).collect();
        assert_eq!(drawn.len(), 1);
        assert!(drawn[0].name.ends_with("_body_x2"));
        assert_eq!((driver.joints.len(), driver.skin.len()), (80, 75));
        let steering = clips.clip("steering").unwrap();
        let hands = [clips.node("D_Lhand_Dummy").unwrap(), clips.node("D_Rhand_Dummy").unwrap()];
        for s in [-1., -0.5, 0., 0.5, 1.] {
            let interior = clips.worlds(&[(steering, (s + 1.) * 0.5 * clips.duration(steering), 1.)]);
            let world = driver.pose(s, &interior);
            for (limb, hand) in driver.limbs[..2].iter().zip(hands.map(|h| interior[h])) {
                let got = driver.root * world[limb.end];
                let gap = got.w_axis.distance(hand.w_axis);
                assert!(gap < 1e-3, "s={s} {}: hand {gap} m from its dummy", driver.joints[limb.end].name);
                let turn = got.to_scale_rotation_translation().1.angle_between(hand.to_scale_rotation_translation().1);
                assert!(turn < 1e-3, "s={s}: hand rotation off by {turn} rad");
            }
            assert!(world.iter().all(|m| m.is_finite()));
        }
        // The pedals: full throttle and full brake press their pedal, and
        // the right foot follows D_Rfoot_Dummy.
        let vehicle = |throttle: f32, brake: f32| {
            let mut v = vec![0u32; 0xe40 / 4];
            v[0xe1c / 4] = throttle.to_bits();
            v[0xe20 / 4] = brake.to_bits();
            v
        };
        let foot = clips.node("D_Rfoot_Dummy").unwrap();
        let rest = clips.worlds(&crate::cockpit::play(&clips, &vehicle(0., 0.)));
        for (throttle, brake, pedal) in [(1., 0., "D_accelerator_Dummy"), (0.3, 1., "D_brake_Dummy")] {
            let interior = clips.worlds(&crate::cockpit::play(&clips, &vehicle(throttle, brake)));
            let n = clips.node(pedal).unwrap();
            let moved = interior[n].w_axis.distance(rest[n].w_axis);
            let turned = interior[n].to_scale_rotation_translation().1.angle_between(rest[n].to_scale_rotation_translation().1);
            println!("pedal {pedal}: moved {moved} m, turned {turned} rad");
            assert!(moved > 1e-3 || turned > 1e-2, "{pedal} did not move");
            let world = driver.pose(0., &interior);
            let gap = (driver.root * world[driver.limbs[3].end]).w_axis.distance(interior[foot].w_axis);
            assert!(gap < 1e-3, "right foot {gap} m from its dummy");
        }
    }
}
