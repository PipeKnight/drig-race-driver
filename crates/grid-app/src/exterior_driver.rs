//! The drivers seen from outside the head-cam (docs/COCKPIT.md "Drivers
//! outside the head-cam"): the player's driver through the windows of the
//! exterior views and every opponent's driver. Each hangs under its car root
//! at `D_bip_position` (identity in every `<stem>_anim_dummies.pssg`), seated
//! by its car's "Neutral" clip (0x861C50, 0x862300). Each frame 0x85D310
//! gives every driver a level by its FOV-scaled distance, nearest first,
//! within the per-level caps of `characterlodsettings.xml`, and the level
//! picks the `_xN` meshes. Presentation only.
//!
//! The player's driver keeps the steering poses (animation level 2: no IK,
//! no lean, no reaches). The opponents' drivers hold their Neutral pose: the
//! animator's has_steering/gears/handbrake/pedals flags are cleared for them
//! [D, I: docs/COCKPIT.md], so their poses are baked into static meshes once.
//! They cast no shadow (0x84A810 draws them in the colour pass only).
//!
//! `GRID_EXTERIOR_DRIVERS=0` leaves them out (comparisons).
use crate::driver::Model;
use bevy::{
    camera::visibility::NoFrustumCulling,
    light::NotShadowCaster,
    mesh::{
        VertexAttributeValues,
        skinning::{SkinnedMesh, SkinnedMeshInverseBindposes},
    },
    prelude::*,
};
use grid_formats::{binxml, jpk};
use std::{collections::BTreeMap, error::Error, path::Path};

/// The `lod_setting` names, in the parser's set order (0x848E50).
const SETS: [&str; 7] = ["default", "low", "medium", "high", "ultra", "replay", "FE"];
/// characterQuality, the "vehicleDrivers" option: the shipped default 2,
/// "medium" (system/hardware_settings_config.xml).
const QUALITY: usize = 2;
/// The finest level a level-of-detail request can take (0x85D310 caps the
/// desired level at 4: `_x6` is never drawn).
const COARSEST: usize = 4;
/// The headless slot the views with DisableDriverHead draw.
const HEADLESS: u32 = 1;

/// A car type's Neutral pose, shared by its cars.
type Neutral = std::sync::Arc<Vec<Mat4>>;
/// One car type's baked meshes: slot, mesh, shader.
type Baked = Vec<(u32, Handle<Mesh>, String)>;

/// The caps of levels 0..4 of a character LOD set (cap 5 is never read).
pub type Caps = [i32; 5];

/// `raceload.jpk#characterlodsettings.xml`'s set `name`.
pub fn caps(root: &Path, name: &str) -> Result<Caps, Box<dyn Error>> {
    let bytes = std::fs::read(root.join("raceload.jpk"))?;
    let archive = jpk::inspect(&bytes)?;
    let entry = (archive.entries.iter())
        .find(|e| e.name == b"characterlodsettings.xml")
        .ok_or("raceload.jpk lacks characterlodsettings.xml")?;
    if entry.uncompressed_size as usize != entry.stored_data.len() {
        return Err("compressed characterlodsettings.xml unsupported".into());
    }
    let document = binxml::read(&bytes[entry.stored_data.clone()])?;
    let set = (document.elements.iter())
        .find(|e| e.name == "lod_setting" && e.attributes.iter().any(|&(k, v)| k == "name" && v == name))
        .ok_or(format!("characterlodsettings.xml lacks the set {name}"))?;
    let mut caps = [0; 5];
    for (i, cap) in caps.iter_mut().enumerate() {
        if let Some(&(_, v)) = set.attributes.iter().find(|&&(k, _)| k == format!("lod_cap_{i}")) {
            *cap = v.trim().parse()?;
        }
    }
    Ok(caps)
}

/// 0x85D310: each driver's level from its FOV-scaled distance in metres
/// (the focus car's counts as 0). Drivers are served nearest first (a
/// stable sort); each takes its desired level (one per 10 m, at most 4) or
/// the first coarser one with a cap left. `None`: no level left, hidden.
pub fn levels(metres: &[f32], caps: Caps) -> Vec<Option<usize>> {
    let mut order: Vec<usize> = (0..metres.len()).collect();
    order.sort_by(|&a, &b| metres[a].total_cmp(&metres[b]));
    let mut left = caps;
    let mut out = vec![None; metres.len()];
    for i in order {
        let desired = ((metres[i] * 0.1) as i32).clamp(0, 5) as usize;
        out[i] = (desired.min(COARSEST)..=COARSEST).find(|&l| left[l] >= 1);
        if let Some(l) = out[i] {
            left[l] -= 1;
        }
    }
    out
}

/// The mesh slot of a level (0x85D310, 0x84A930): the finest slot at or
/// below the level that has meshes, skipping the headless slot 1; else the
/// first coarser one.
pub fn slot(level: usize, slots: &[u32]) -> Option<u32> {
    let level = level as u32;
    let usable = |s: &u32| *s != HEADLESS && slots.contains(s);
    (0..=level.min(5)).rev().find(usable).or_else(|| (level..6).find(usable))
}

/// The loaded model and each car's placement, until the drivers spawn.
#[derive(Resource)]
pub struct Pending {
    model: Model,
    caps: Caps,
    /// Opponent cars: index, model-frame placement (the car's model offset),
    /// Neutral pose; `None` when the car has no driver clips.
    opponents: Vec<(usize, Mat4, Option<Neutral>)>,
}

/// Reads the model, the caps and each opponent's Neutral pose. `stems` and
/// `offsets` list the opponents (cars 1..), the player excluded.
pub fn prepare(root: &Path, stems: &[&str], offsets: &[Mat4]) -> Result<Option<Pending>, Box<dyn Error>> {
    if std::env::var("GRID_EXTERIOR_DRIVERS").is_ok_and(|v| v == "0") {
        return Ok(None);
    }
    let model = Model::read(root)?;
    let caps = caps(root, SETS[QUALITY])?;
    let mut poses: BTreeMap<&str, Option<Neutral>> = BTreeMap::new();
    let mut opponents = Vec::new();
    for (k, (&stem, &offset)) in stems.iter().zip(offsets).enumerate() {
        if !poses.contains_key(stem) {
            poses.insert(stem, model.neutral(root, stem)?.map(std::sync::Arc::new));
        }
        opponents.push((k + 1, offset, poses[stem].clone()));
    }
    println!(
        "exterior_drivers version=1 set={} caps={caps:?} opponents={} posed={}",
        SETS[QUALITY],
        opponents.len(),
        opponents.iter().filter(|o| o.2.is_some()).count()
    );
    Ok(Some(Pending { model, caps, opponents }))
}

/// One driver: its car, its placement in the car's model frame, its root
/// entity and its meshes by slot.
struct Seat {
    car: usize,
    rest: Mat4,
    root: Entity,
    meshes: Vec<(u32, Entity)>,
}

/// The spawned drivers: the player's (car 0, skinned) first when it has one.
#[derive(Resource)]
pub struct Drivers {
    caps: Caps,
    slots: Vec<u32>,
    seats: Vec<Seat>,
    /// The player's skin joints: entity and pose index.
    joints: Vec<(Entity, usize)>,
}

pub fn install(app: &mut App) {
    app.add_systems(Update, (spawn, present).chain().after(crate::drive_present));
}

/// The pose of a skinned mesh baked into its vertices: each vertex by the
/// weighted skin matrices (the original skins on the GPU; static poses give
/// the same vertices).
fn bake(mesh: &Mesh, skin: &[Mat4]) -> Option<Mesh> {
    let (
        Some(VertexAttributeValues::Float32x3(positions)),
        Some(VertexAttributeValues::Float32x3(normals)),
        Some(VertexAttributeValues::Uint16x4(joints)),
        Some(VertexAttributeValues::Float32x4(weights)),
    ) = (
        mesh.attribute(Mesh::ATTRIBUTE_POSITION),
        mesh.attribute(Mesh::ATTRIBUTE_NORMAL),
        mesh.attribute(Mesh::ATTRIBUTE_JOINT_INDEX),
        mesh.attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT),
    )
    else {
        return None;
    };
    let mut out = mesh.clone();
    let mut p = Vec::with_capacity(positions.len());
    let mut n = Vec::with_capacity(normals.len());
    for (((position, normal), joint), weight) in positions.iter().zip(normals).zip(joints).zip(weights) {
        let mut m = Mat4::ZERO;
        for (&j, &w) in joint.iter().zip(weight) {
            m += *skin.get(usize::from(j))? * w;
        }
        p.push(m.transform_point3(Vec3::from_array(*position)).to_array());
        n.push(m.transform_vector3(Vec3::from_array(*normal)).normalize_or_zero().to_array());
    }
    out.insert_attribute(Mesh::ATTRIBUTE_POSITION, p);
    out.insert_attribute(Mesh::ATTRIBUTE_NORMAL, n);
    out.remove_attribute(Mesh::ATTRIBUTE_JOINT_INDEX);
    out.remove_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT);
    Some(out)
}

/// Spawns the drivers once the race and the cockpit exist.
#[allow(clippy::too_many_arguments)] // Explicit Bevy resource ownership.
fn spawn(
    mut commands: Commands,
    pending: Option<Res<Pending>>,
    driving: Option<Res<crate::Driving>>,
    cockpit: Option<Res<crate::cockpit::Cockpit>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut bindposes: ResMut<Assets<SkinnedMeshInverseBindposes>>,
) {
    let (Some(pending), Some(driving)) = (pending, driving) else {
        return;
    };
    commands.remove_resource::<Pending>();
    let model = &pending.model;
    // The car material path's rule for interior parts (as the head-cam's
    // driver): the diffuse map, roughness 0.7, opaque, back faces culled.
    let mut by_shader = BTreeMap::new();
    for instance in &model.instances {
        by_shader.entry(instance.shader.clone()).or_insert_with(|| {
            let texture = (instance.texture.as_ref())
                .and_then(|t| model.textures.get(t))
                .and_then(|dds| crate::ground_effect_assets::image(dds, true).ok())
                .map(|image| images.add(image));
            materials.add(StandardMaterial {
                base_color_texture: texture,
                perceptual_roughness: 0.7,
                ..default()
            })
        });
    }
    let mut slots: Vec<u32> = model.instances.iter().map(|i| i.slot).collect();
    slots.sort_unstable();
    slots.dedup();
    let mut seats = Vec::new();
    let mut joints = Vec::new();
    // The player's driver: the cockpit's skeleton, skinned, posed each frame.
    if let Some(driver) = cockpit.as_ref().and_then(|c| c.driver.as_ref()) {
        let pose = Mat4::from_cols_array(&driving.runtime.pose());
        let root = commands.spawn((Name::new("driver/exterior"), Transform::from_matrix(pose * driver.root()), Visibility::Hidden)).id();
        let world = driver.exterior_pose(0.);
        let skin: Vec<Entity> = (driver.skin().iter())
            .map(|&j| {
                let e = commands.spawn((Transform::from_matrix(world[j]), ChildOf(root))).id();
                joints.push((e, j));
                e
            })
            .collect();
        let inverse = bindposes.add(SkinnedMeshInverseBindposes::from(driver.inverse_bind().to_vec()));
        let mut parts = Vec::new();
        for instance in model.instances.iter().filter(|i| i.slot <= COARSEST as u32) {
            let e = commands
                .spawn((
                    Name::new(format!("driver/exterior/{}", instance.id)),
                    Mesh3d(meshes.add(instance.mesh.clone())),
                    MeshMaterial3d(by_shader[&instance.shader].clone()),
                    SkinnedMesh { inverse_bindposes: inverse.clone(), joints: skin.clone() },
                    // The bind-pose bounds do not follow the seated pose.
                    NoFrustumCulling,
                    Transform::default(),
                    Visibility::Hidden,
                    ChildOf(root),
                ))
                .id();
            parts.push((instance.slot, e));
        }
        seats.push(Seat { car: 0, rest: driver.root(), root, meshes: parts });
    }
    // The opponents' drivers: Neutral baked once per car model.
    let skin_of = |world: &[Mat4]| -> Vec<Mat4> {
        (model.skin().iter().zip(model.inverse_bind())).map(|(&j, inverse)| world[j] * *inverse).collect()
    };
    let mut baked: BTreeMap<*const Vec<Mat4>, Baked> = BTreeMap::new();
    let cars = driving.opponents.as_ref().map(|o| &o.multi.cars);
    for (car, rest, neutral) in &pending.opponents {
        let Some(neutral) = neutral else { continue };
        let Some(runtime) = cars.and_then(|c| c.get(*car)) else { continue };
        let set = baked.entry(std::sync::Arc::as_ptr(neutral)).or_insert_with(|| {
            let skin = skin_of(neutral);
            (model.instances.iter())
                .filter(|i| i.slot <= COARSEST as u32)
                .filter_map(|i| Some((i.slot, meshes.add(bake(&i.mesh, &skin)?), i.shader.clone())))
                .collect()
        });
        let pose = Mat4::from_cols_array(&runtime.pose());
        let root = commands.spawn((Name::new(format!("driver/car{car}")), Transform::from_matrix(pose * *rest), Visibility::Visible)).id();
        let parts = (set.iter())
            .map(|(slot, mesh, shader)| {
                let e = commands
                    .spawn((
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(by_shader[shader].clone()),
                        NotShadowCaster,
                        Transform::default(),
                        Visibility::Hidden,
                        ChildOf(root),
                    ))
                    .id();
                (*slot, e)
            })
            .collect();
        seats.push(Seat { car: *car, rest: *rest, root, meshes: parts });
    }
    println!(
        "exterior_drivers event=spawn seats={} player={} slots={slots:?}",
        seats.len(),
        seats.first().is_some_and(|s| s.car == 0)
    );
    commands.insert_resource(Drivers { caps: pending.caps, slots, seats, joints });
}

/// Places every driver on its car, picks the levels and poses the player's.
fn present(
    driving: Option<Res<crate::Driving>>,
    drivers: Option<Res<Drivers>>,
    cockpit: Option<Res<crate::cockpit::Cockpit>>,
    cameras: Option<Res<crate::camera::Cameras>>,
    camera: Query<(&Transform, &Projection), With<crate::ViewerCamera>>,
    mut transforms: Query<&mut Transform, Without<crate::ViewerCamera>>,
    mut visibility: Query<&mut Visibility>,
) {
    let (Some(driving), Some(drivers)) = (driving, drivers) else {
        return;
    };
    let cars = driving.opponents.as_ref().map(|o| &o.multi.cars);
    let pose = |car: usize| -> Option<Mat4> {
        match car {
            0 => Some(Mat4::from_cols_array(&driving.runtime.pose())),
            k => cars.and_then(|c| c.get(k)).map(|r| Mat4::from_cols_array(&r.pose())),
        }
    };
    let Ok((eye, Projection::Perspective(lens))) = camera.single() else {
        return;
    };
    // The focus car (the player) is served first, at distance 0.
    let mut metres = Vec::with_capacity(drivers.seats.len());
    for seat in &drivers.seats {
        let world = pose(seat.car).unwrap_or(Mat4::IDENTITY) * seat.rest;
        if let Ok(mut t) = transforms.get_mut(seat.root) {
            *t = Transform::from_matrix(world);
        }
        metres.push(match seat.car {
            0 => 0.,
            _ => crate::car_lod::scaled(world.w_axis.truncate().distance(eye.translation), lens.fov),
        });
    }
    let levels = levels(&metres, drivers.caps);
    let own = cameras.as_ref().map_or(crate::camera::OwnCar::Whole, |c| c.own_car());
    for (seat, level) in drivers.seats.iter().zip(levels) {
        let shown = seat.car != 0 || own == crate::camera::OwnCar::Whole;
        let chosen = match level {
            Some(_) if seat.car == 0 && cameras.as_ref().is_some_and(|c| c.driver_head_hidden()) => Some(HEADLESS),
            Some(l) => slot(l, &drivers.slots),
            None => None,
        };
        if let Ok(mut v) = visibility.get_mut(seat.root) {
            v.set_if_neq(if shown && chosen.is_some() { Visibility::Inherited } else { Visibility::Hidden });
        }
        for &(s, e) in &seat.meshes {
            if let Ok(mut v) = visibility.get_mut(e) {
                v.set_if_neq(if Some(s) == chosen { Visibility::Inherited } else { Visibility::Hidden });
            }
        }
    }
    // The player's steering poses, while its driver shows.
    let Some(driver) = cockpit.as_ref().and_then(|c| c.driver.as_ref()) else {
        return;
    };
    if own != crate::camera::OwnCar::Whole || drivers.joints.is_empty() {
        return;
    }
    let world = driver.exterior_pose(crate::cockpit::steering(&driving.runtime.state.car.owner.vehicle));
    for &(e, j) in &drivers.joints {
        if let Ok(mut t) = transforms.get_mut(e) {
            *t = Transform::from_matrix(world[j]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Medium caps (0, 0, 1, 5, 99): the focus car takes the one level-2
    /// slot, the five nearest opponents level 3, the rest level 4.
    #[test]
    fn drivers_are_served_nearest_first_within_the_caps() {
        let caps = [0, 0, 1, 5, 99];
        let mut metres = vec![0., 12., 25., 8., 33., 14., 19., 90.];
        let levels = levels(&metres, caps);
        assert_eq!(levels[0], Some(2));
        assert_eq!(levels[7], Some(4));
        assert_eq!(levels.iter().filter(|l| **l == Some(3)).count(), 5);
        // Ultra (1, 0, 2, 8, 99): the focus car level 0, the next two 2.
        metres.truncate(4);
        assert_eq!(super::levels(&metres, [1, 0, 2, 8, 99]), vec![Some(0), Some(2), Some(3), Some(2)]);
        // A far opponent goes straight to level 4 although level 3 has room.
        assert_eq!(super::levels(&[0., 90.], caps), vec![Some(2), Some(4)]);
        // The front-end set hides everyone.
        assert_eq!(super::levels(&[0., 5.], [0; 5]), vec![None, None]);
    }

    #[test]
    fn levels_draw_the_finest_slot_but_never_the_headless_one() {
        let slots = [0, 1, 2, 3, 4, 5];
        assert_eq!([0, 1, 2, 3, 4].map(|l| slot(l, &slots)), [Some(0), Some(0), Some(2), Some(3), Some(4)]);
        assert_eq!(slot(1, &[1, 3]), Some(3));
    }

    /// The shipped medium set (GRID_GAME_ROOT).
    #[test]
    #[ignore = "needs GRID_GAME_ROOT"]
    fn shipped_character_lod_caps() {
        let root = std::path::PathBuf::from(std::env::var("GRID_GAME_ROOT").expect("GRID_GAME_ROOT"));
        assert_eq!(caps(&root, "medium").unwrap(), [0, 0, 1, 5, 99]);
        assert_eq!(caps(&root, "ultra").unwrap(), [1, 0, 2, 8, 99]);
    }
}
