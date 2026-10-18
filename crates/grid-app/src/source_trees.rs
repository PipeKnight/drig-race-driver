//! Static closest-LOD source forest. No wind, collision or original shader claim.
use super::mesh_stream::{MeshStream, VertexLayout, put};
use super::{Car, Part, read};
use bevy::mesh::VertexAttributeValues;
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use grid_formats::{material, mesh, pssg, scene, texture, tree_placements};
use grid_tools::route::Route;
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    path::Path,
};

fn closest(tree: &pssg::Tree<'_>, node: usize) -> bool {
    let mut parent = tree.nodes[node].parent;
    while let Some(i) = parent {
        if tree.name(i) == b"NODE" && tree.string(i, b"id").is_ok_and(|s| s.ends_with(b"_x0")) {
            return true;
        }
        if tree.name(i) == b"ROOTNODE" {
            break;
        }
        parent = tree.nodes[i].parent;
    }
    false
}

pub fn append(
    root: &Path,
    route: &Route,
    car: &mut Car,
    coverage: bool,
    sheet_colours: bool,
) -> Result<(), Box<dyn Error>> {
    let bytes = read(&root.join(route.track_file("trees.pssg")))?;
    let tree = pssg::read_tree(&bytes)?;
    let sources = mesh::read_static_meshes(&tree)?;
    let draws = scene::read_static_draws(&tree, &sources)?;
    let library_draws = draws.len();
    let selected: BTreeSet<_> = (0..tree.nodes.len())
        .filter(|&i| tree.name(i) == b"RENDERSTREAMINSTANCE" && closest(&tree, i))
        .map(|i| tree.string(i, b"id"))
        .collect::<grid_formats::Result<_>>()?;
    let bytes = read(&root.join(route.file("trees.bin")))?;
    let placements = tree_placements::read(&bytes)?;
    let by_name: BTreeMap<_, _> = placements
        .prototypes
        .iter()
        .map(|p| Ok((format!("{} Root", std::str::from_utf8(p.name)?), p)))
        .collect::<Result<_, std::str::Utf8Error>>()?;
    let facing: BTreeSet<_> = (0..tree.nodes.len())
        .filter(|&i| {
            tree.name(i) == b"SHADERINSTANCE"
                && tree
                    .string(i, b"shaderGroup")
                    .is_ok_and(|s| s == b"#facing_foliage.fx")
        })
        .map(|i| tree.string(i, b"id"))
        .collect::<grid_formats::Result<_>>()?;
    let sheets: BTreeSet<_> = (0..tree.nodes.len())
        .filter(|&i| {
            tree.name(i) == b"SHADERINSTANCE"
                && tree
                    .string(i, b"shaderGroup")
                    .is_ok_and(|v| v == b"#treesheet_foliage.fx")
        })
        .map(|i| tree.string(i, b"id"))
        .collect::<grid_formats::Result<_>>()?;
    let leaves: BTreeSet<_> = (0..tree.nodes.len())
        .filter(|&i| {
            tree.name(i) == b"SHADERINSTANCE"
                && tree.string(i, b"shaderGroup").is_ok_and(|s| {
                    matches!(
                        s,
                        b"#facing_foliage.fx" | b"#foliage.fx" | b"#treesheet_foliage.fx"
                    )
                })
        })
        .map(|i| tree.string(i, b"id"))
        .collect::<grid_formats::Result<_>>()?;
    let mut diffuse = BTreeMap::new();
    for b in material::read_texture_bindings(&tree)? {
        if b.sampler != b"TDiffuseAlphaMap" {
            continue;
        }
        if b.container != Some(b"treestextures.pssg".as_slice())
            || diffuse.insert(b.shader, b.texture).is_some()
        {
            return Err("unsupported or duplicate tree diffuse binding".into());
        }
    }
    let (mut count, mut triangles, mut closest_draws) = (0usize, 0usize, 0usize);
    let mut matched = BTreeSet::new();
    let mut needed = BTreeSet::new();
    let mut leaf_textures = BTreeSet::new();
    // Stage CPU objects before appending: no partial forest if a source is invalid.
    let mut parts = Vec::new();
    let mut bindings = BTreeMap::new();
    let mut world_min = Vec3::splat(f32::INFINITY);
    let mut world_max = Vec3::splat(f32::NEG_INFINITY);
    for d in draws {
        if !selected.contains(d.id) {
            continue;
        }
        closest_draws += 1;
        let prototype = std::str::from_utf8(d.prototype.ok_or("tree draw prototype absent")?)?;
        let Some(p) = by_name.get(prototype) else {
            continue;
        };
        matched.insert(prototype.to_string());
        let tex = *diffuse
            .get(d.material)
            .ok_or("tree draw lacks diffuse map")?;
        if leaves.contains(d.material) {
            leaf_textures.insert(tex);
        }
        let name = format!(
            "trees/{}{id}",
            if leaves.contains(d.material) {
                "leaf/"
            } else {
                "bark/"
            },
            id = std::str::from_utf8(d.material)?
        );
        let key = format!("trees/{}", std::str::from_utf8(tex)?);
        bindings.insert(name.clone(), key);
        needed.insert(tex);
        let source = &sources[d.source];
        let mut geometry = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        );
        geometry.insert_attribute(Mesh::ATTRIBUTE_POSITION, source.positions.clone());
        geometry.insert_indices(Indices::U16(d.indices));
        // treesheet VS forwards saved v3.rgb; its PS multiplies the lit diffuse
        // by that RGB. Preserve raw UNORM values (no speculative sRGB transform).
        // Saved alpha is not the PS alpha source: instance v7.w is, so keep 1.
        if sheet_colours && sheets.contains(d.material) {
            if source.colors_argb.len() != source.positions.len() {
                return Err("treesheet RGB count differs from vertices".into());
            }
            let colours: Vec<[f32; 4]> = source.colors_argb.iter()
                .map(|[_, r, g, b]| [*r as f32 / 255., *g as f32 / 255., *b as f32 / 255., 1.])
                .collect();
            geometry.insert_attribute(Mesh::ATTRIBUTE_COLOR, colours);
        }
        if source.texcoords.is_empty() {
            return Err("tree geometry has no diffuse UV".into());
        }
        geometry.insert_attribute(Mesh::ATTRIBUTE_UV_0, source.texcoords.clone());
        if let Some(uv) = source.additional_texcoords.first() {
            geometry.insert_attribute(Mesh::ATTRIBUTE_UV_1, uv.clone());
        }
        if source.normals.is_empty() || source.nonfinite_normals != 0 {
            geometry.compute_smooth_normals();
        } else {
            geometry.insert_attribute(Mesh::ATTRIBUTE_NORMAL, source.normals.clone());
        }
        let matrices: Vec<_> = p
            .instances
            .iter()
            .map(|instance| {
                Mat4::from_cols_array(&scene::compose(&d.transform, &instance.transform))
            })
            .collect();
        let lo = source
            .positions
            .iter()
            .fold(Vec3::splat(f32::INFINITY), |v, p| {
                v.min(Vec3::from_array(*p))
            });
        let hi = source
            .positions
            .iter()
            .fold(Vec3::splat(f32::NEG_INFINITY), |v, p| {
                v.max(Vec3::from_array(*p))
            });
        for matrix in &matrices {
            for i in 0..8 {
                let p = matrix.transform_point3(Vec3::new(
                    if i & 1 == 0 { lo.x } else { hi.x },
                    if i & 2 == 0 { lo.y } else { hi.y },
                    if i & 4 == 0 { lo.z } else { hi.z },
                ));
                world_min = world_min.min(p);
                world_max = world_max.max(p);
            }
        }
        count += matrices.len();
        triangles += geometry.indices().unwrap().len() / 3 * matrices.len();
        parts.push(Part {
            name: format!(
                "trees/{}{id}",
                if facing.contains(d.material) {
                    "facing/"
                } else if sheets.contains(d.material) {
                    "sheet/"
                } else {
                    ""
                },
                id = std::str::from_utf8(d.id)?
            ),
            joint: String::new(),
            material: name,
            mesh: geometry,
            matrices,
            wheel: None,
        });
    }
    if matched.len() != placements.prototypes.len() || parts.is_empty() {
        return Err("tree placement prototype lacks closest source geometry".into());
    }
    let bytes = read(&root.join(route.track_file("treestextures.pssg")))?;
    let textures = pssg::read_tree(&bytes)?;
    let names: Vec<_> = needed.into_iter().collect();
    let mut images = BTreeMap::new();
    for t in texture::read_named(&textures, &names)? {
        let key = format!("trees/{}", std::str::from_utf8(t.id)?);
        let (dds, adjusted) = if coverage && leaf_textures.contains(t.id) {
            crate::foliage_mips::prepare(&t)?
        } else {
            (t.validated_dds().0, 0)
        };
        if adjusted > 0 {
            println!(
                "foliage_mip_coverage texture={key} adjusted_levels={adjusted} base=unchanged rgb=unchanged scale_max=2"
            );
        }
        images.insert(key, dds);
    }
    if bindings.values().any(|key| !images.contains_key(key)) {
        return Err("tree diffuse texture unresolved".into());
    }
    if bindings.keys().any(|key| car.bindings.contains_key(key))
        || images.keys().any(|key| car.textures.contains_key(key))
    {
        return Err("tree namespace already exists".into());
    }
    let sheet_draws = parts
        .iter()
        .filter(|p| p.name.starts_with("trees/sheet/"))
        .count();
    let sheet_triangles: usize = parts
        .iter()
        .filter(|p| p.name.starts_with("trees/sheet/"))
        .map(|p| p.mesh.indices().unwrap().len() / 3)
        .sum();
    println!(
        "source_tree_sheets draws={sheet_draws} triangles={sheet_triangles} expansion=cylindrical-world-up orientation=main-camera saved_rgb={sheet_colours} physics=unchanged"
    );
    println!(
        "source_trees enabled=true prototypes={} instances={} library_draws={library_draws} closest_draws={closest_draws} placed_draws={count} triangles={triangles} textures={} bounds={world_min:?}..{world_max:?} lod=source-x0 wind=false physics=unchanged alpha=authored-mask05 sampler=existing-repeat",
        matched.len(),
        placements.total_instances,
        images.len()
    );
    car.parts.extend(parts);
    car.bindings.extend(bindings);
    car.textures.extend(images);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn facing_offsets_expand_in_camera_plane_after_scaled_rotated_placement() {
        let matrix = Mat4::from_scale_rotation_translation(
            Vec3::splat(1.7),
            Quat::from_rotation_y(1.2),
            Vec3::new(10., -3., 20.),
        );
        let center = Vec3::new(0.5, 4., -0.7);
        let camera = Quat::from_euler(EulerRot::YXZ, -0.8, 0.3, 0.1);
        let right = camera * Vec3::X;
        let up = camera * Vec3::Y;
        let offset = Vec2::new(-1.3, 0.8);
        let actual =
            matrix.transform_point3(expanded(center, offset, right, up, matrix.inverse(), 1.7));
        let expected = matrix.transform_point3(center) + (right * offset.x + up * offset.y) * 1.7;
        assert!(actual.distance(expected) < 0.00001);
        assert!(
            (actual - matrix.transform_point3(center))
                .dot(camera * Vec3::Z)
                .abs()
                < 0.00001
        );
    }
    #[test]
    fn sheet_offsets_face_each_centre_without_camera_pitch_or_roll() {
        let matrix = Mat4::from_scale_rotation_translation(
            Vec3::splat(1.7),
            Quat::from_rotation_y(1.2),
            Vec3::new(10., -3., 20.),
        );
        let center = Vec3::new(0.5, 4., -0.7);
        let world_center = matrix.transform_point3(center);
        let offset = Vec2::new(-1.3, 0.8);
        for delta in [Vec3::new(0., 10., 30.), Vec3::new(30., -5., 0.)] {
            let eye = world_center + delta;
            let actual = matrix.transform_point3(sheet_expanded(
                center,
                offset,
                eye,
                matrix,
                matrix.inverse(),
                1.7,
            ));
            let right = Vec3::Y.cross(Vec3::new(delta.x, 0., delta.z)).normalize();
            let expected = world_center + (right * offset.x + Vec3::Y * offset.y) * 1.7;
            assert!(actual.distance(expected) < 0.00001);
        }
        assert!(
            sheet_expanded(
                center,
                offset,
                world_center + Vec3::Y,
                matrix,
                matrix.inverse(),
                1.7
            )
            .is_finite()
        );
    }
    #[test]
    fn sheet_presentation_uses_the_final_replay_camera_in_the_same_frame() {
        use crate::replay::{self, frame, stream};
        let mut snapshot = frame::Snapshot {
            tick: 0,
            cars: vec![frame::Car {
                pose: frame::Pose::from_mat4(Mat4::from_translation(Vec3::new(20., 0., 30.))),
                velocity: Vec3::X * 10.,
                wheels: [frame::Wheel::default(); 4],
                geometry: [frame::WheelGeometry {
                    anchor: [0.; 3],
                    axis: [0., 1., 0.],
                    deformation_translation: [0.; 3],
                    deformation_orientation: Mat4::IDENTITY.to_cols_array_2d(),
                }; 4],
                deformation: None,
                lost: [None; 4],
            }],
            props: None,
        };
        let mut encoder = frame::Encoder::default();
        let mut stream = stream::Stream::new(stream::Limits::default(), None);
        for (number, tick) in [0, 2].into_iter().enumerate() {
            snapshot.tick = tick;
            stream.push(tick, &encoder.encode(&snapshot, number as u32));
        }
        let views = crate::camera::Views::fallback();
        let mut replay = replay::Replay::new(stream, None);
        assert!(replay.start(replay::Kind::Full, replay::Origin::PauseMenu, false, &views));
        replay.session.as_mut().unwrap().shown = Some(snapshot);
        let center = Vec3::new(0., 4., 0.);
        let offset = Vec2::new(2., 3.);
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![center.to_array()]);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, vec![offset.to_array()]);
        let facing = FacingSource::from_mesh("trees/sheet/test", &mesh)
            .unwrap()
            .placed(Mat4::IDENTITY);
        let mut app = App::new();
        app.insert_resource(Assets::<Mesh>::default())
            .insert_resource(replay)
            .insert_resource(crate::camera::Cameras::new(views, "chase_close", None));
        let initial = Transform::from_xyz(-60., 10., -50.);
        let camera = app.world_mut().spawn((crate::ViewerCamera, initial)).id();
        let handle = app.world_mut().resource_mut::<Assets<Mesh>>().add(mesh);
        app.world_mut().spawn((facing, Mesh3d(handle.clone())));
        // Register the foliage first, as in production. Its explicit dependency
        // must still make it observe the replay's later camera writer.
        install(&mut app);
        app.add_systems(Update, replay::frame_camera);
        app.update();
        let camera = app.world().get::<Transform>(camera).unwrap();
        assert!(camera.translation.distance(initial.translation) > 1.);
        // POSITION, then UV_1: the first 12 bytes are the faced point.
        let bytes = app.world().resource::<MeshStream>().queued(handle.id()).unwrap();
        assert_eq!(bytes.len(), 12 + 8);
        let point: Vec<f32> = bytes[..12]
            .chunks(4)
            .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
            .collect();
        let delta = Vec3::from_slice(&point) - center;
        let toward = (camera.translation - center).with_y(0.).normalize();
        assert!(delta.with_y(0.).dot(toward).abs() < 0.00001);
        assert!((delta.y - offset.y).abs() < 0.00001);
        assert!((delta.with_y(0.).length() - offset.x).abs() < 0.00001);
    }
    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn purchased_donington_closest_forest_is_bound_without_billboard_duplicates() {
        let root = std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required");
        let mut car = Car {
            parts: Vec::new(),
            textures: BTreeMap::new(),
            bindings: BTreeMap::new(),
            center: Vec3::ZERO,
            radius: 1.,
            track: true,
            liveries: Vec::new(),
        };
        append(Path::new(&root), &Route::default(), &mut car, false, true).unwrap();
        assert_eq!(car.parts.len(), 21);
        let facing: Vec<_> = car
            .parts
            .iter()
            .filter_map(|p| FacingSource::from_mesh(&p.name, &p.mesh))
            .collect();
        assert_eq!(facing.len(), 7);
        assert_eq!(facing.iter().filter(|f| f.cylindrical).count(), 3);
        for part in car
            .parts
            .iter()
            .filter(|p| p.name.starts_with("trees/sheet/"))
        {
            let source = FacingSource::from_mesh(&part.name, &part.mesh).unwrap();
            assert!(source.cylindrical);
            let Some(VertexAttributeValues::Float32x4(colours)) = part.mesh.attribute(Mesh::ATTRIBUTE_COLOR) else {
                panic!("saved treesheet gradient absent")
            };
            assert_eq!(colours.len(), source.centers.len());
            assert!(colours.iter().all(|c| c[3] == 1. && c[0] == c[1] && c[1] == c[2]));
            let levels: BTreeSet<_> = colours.iter().map(|c| (c[0] * 255.).round() as u8).collect();
            assert_eq!(levels, BTreeSet::from([139, 201]));
            let points: Vec<_> = source
                .centers
                .iter()
                .zip(&source.offsets)
                .map(|(p, uv)| {
                    sheet_expanded(
                        Vec3::from_array(*p),
                        Vec2::from_array(*uv),
                        Vec3::new(-429., -12., -262.),
                        Mat4::IDENTITY,
                        Mat4::IDENTITY,
                        1.,
                    )
                })
                .collect();
            let Indices::U16(indices) = part.mesh.indices().unwrap() else {
                panic!("expected source indices")
            };
            let mut expanded_triangles = 0;
            for triangle in indices.as_chunks::<3>().0 {
                let raw = triangle.map(|i| Vec3::from_array(source.centers[i as usize]));
                assert!((raw[1] - raw[0]).cross(raw[2] - raw[0]).length_squared() < 1e-10);
                let p = triangle.map(|i| points[i as usize]);
                expanded_triangles +=
                    usize::from((p[1] - p[0]).cross(p[2] - p[0]).length_squared() > 1e-6);
            }
            assert_eq!(expanded_triangles, indices.len() / 3);
            assert!(source.radius() > 1.);
        }
        for part in car
            .parts
            .iter()
            .filter(|p| p.name.starts_with("trees/facing/"))
        {
            for matrix in &part.matrices {
                let axes = [
                    matrix.x_axis.truncate(),
                    matrix.y_axis.truncate(),
                    matrix.z_axis.truncate(),
                ];
                let scales = axes.map(|a| a.length());
                assert!(
                    scales[0] > 0.
                        && (scales[0] - scales[1]).abs() < 0.001
                        && (scales[0] - scales[2]).abs() < 0.001
                );
                assert!(axes[0].dot(axes[1]).abs() < 0.001 && axes[0].dot(axes[2]).abs() < 0.001);
            }
        }
        assert!(
            facing
                .iter()
                .all(|f| f.radius() > 0.1 && f.centers.len() == f.offsets.len())
        );
        assert!(
            car.parts
                .iter()
                .all(|p| !p.matrices.is_empty() && p.wheel.is_none())
        );
        assert!(
            car.parts
                .iter()
                .all(|p| p.matrices.iter().all(|m| m.is_finite()))
        );
        assert!(
            car.bindings
                .values()
                .all(|key| car.textures.contains_key(key))
        );
        assert!(append(Path::new(&root), &Route::default(), &mut car, false, true).is_err());
        assert_eq!(car.parts.len(), 21);
    }
}

/// Source-facing foliage stores repeated centres and signed offsets in UV1.
/// The zero-wind original VS expands those offsets along inverse-view X/Y,
/// after applying the source instance scale. This CPU adaptation uses the main
/// camera for all passes; shadow/reflection-view orientation is not 1:1 yet.
#[derive(Clone)]
pub struct FacingSource {
    cylindrical: bool,
    centers: Vec<[f32; 3]>,
    offsets: Vec<[f32; 2]>,
    /// The mesh's packed vertex buffer; presentation rewrites only POSITION
    /// and NORMAL in it (mesh_stream, F392).
    packed: std::sync::Arc<Vec<u8>>,
    layout: VertexLayout,
}
impl FacingSource {
    pub fn from_mesh(name: &str, mesh: &Mesh) -> Option<Self> {
        let cylindrical = name.starts_with("trees/sheet/");
        if !cylindrical && !name.starts_with("trees/facing/") {
            return None;
        }
        let Some(VertexAttributeValues::Float32x3(centers)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            return None;
        };
        let Some(VertexAttributeValues::Float32x2(offsets)) = mesh.attribute(Mesh::ATTRIBUTE_UV_1)
        else {
            return None;
        };
        assert_eq!(centers.len(), offsets.len(), "validated foliage streams");
        Some(Self {
            cylindrical,
            centers: centers.clone(),
            offsets: offsets.clone(),
            packed: std::sync::Arc::new(mesh.create_packed_vertex_buffer_data()),
            layout: VertexLayout::of(mesh),
        })
    }
    pub fn radius(&self) -> f32 {
        self.offsets
            .iter()
            .map(|v| Vec2::from_array(*v).length())
            .fold(0., f32::max)
    }
    pub fn placed(&self, matrix: Mat4) -> Facing {
        let inverse = matrix.inverse();
        // Original zero-wind facing VS uses a radial local normal, not the
        // intentionally nonfinite saved NORMAL stream of collapsed quads.
        // Neither normal depends on the camera, so it is written once here.
        let mut vertices = self.packed.to_vec();
        if let Some(at) = self.layout.offset(Mesh::ATTRIBUTE_NORMAL) {
            for (i, p) in self.centers.iter().enumerate() {
                let normal = (if self.cylindrical {
                    inverse.transform_vector3(Vec3::Y)
                } else {
                    Vec3::from_array(*p)
                })
                .try_normalize()
                .unwrap_or(Vec3::Y);
                put(&mut vertices, i * self.layout.stride + at, normal.to_array());
            }
        }
        Facing {
            source: self.clone(),
            world: matrix,
            inverse,
            scale: matrix.x_axis.truncate().length(),
            vertices,
        }
    }
}
#[derive(Component)]
pub struct Facing {
    source: FacingSource,
    world: Mat4,
    inverse: Mat4,
    scale: f32,
    /// Packed vertex buffer with this placement's normals.
    vertices: Vec<u8>,
}
fn expanded(center: Vec3, offset: Vec2, right: Vec3, up: Vec3, inverse: Mat4, scale: f32) -> Vec3 {
    center + inverse.transform_vector3((right * offset.x + up * offset.y) * scale)
}
// Original treesheet VS faces each centre about world Y, unlike the
// spherical inverse-view X/Y offsets used by the nearer facing foliage.
fn sheet_expanded(
    center: Vec3,
    offset: Vec2,
    eye: Vec3,
    world: Mat4,
    inverse: Mat4,
    scale: f32,
) -> Vec3 {
    let world_center = world.transform_point3(center);
    let toward_eye = Vec3::new(eye.x - world_center.x, 0., eye.z - world_center.z);
    let right = Vec3::Y.cross(toward_eye).try_normalize().unwrap_or(Vec3::X);
    expanded(center, offset, right, Vec3::Y, inverse, scale)
}
pub fn install(app: &mut App) {
    app.init_resource::<MeshStream>().add_systems(
        Update,
        present
            .after(super::drive_present)
            .after(super::replay::frame_camera),
    );
}
/// Re-faces every billboard toward the camera each frame. The vertex bytes are
/// written over the mesh's GPU allocation (mesh_stream); the asset keeps its
/// source vertices, so Bevy does not reallocate and repack it every frame.
fn present(
    camera: Query<&Transform, With<super::ViewerCamera>>,
    foliage: Query<(&Facing, &Mesh3d)>,
    stream: Res<MeshStream>,
) {
    let Ok(camera) = camera.single() else {
        return;
    };
    let (right, up) = (camera.right().as_vec3(), camera.up().as_vec3());
    for (facing, handle) in &foliage {
        let source = &facing.source;
        let Some(at) = source.layout.offset(Mesh::ATTRIBUTE_POSITION) else {
            continue;
        };
        stream.write(handle.id(), |bytes| {
            let start = bytes.len();
            bytes.extend_from_slice(&facing.vertices);
            let vertices = &mut bytes[start..];
            for (i, (p, uv)) in source.centers.iter().zip(&source.offsets).enumerate() {
                let point = if source.cylindrical {
                    sheet_expanded(
                        Vec3::from_array(*p),
                        Vec2::from_array(*uv),
                        camera.translation,
                        facing.world,
                        facing.inverse,
                        facing.scale,
                    )
                } else {
                    expanded(
                        Vec3::from_array(*p),
                        Vec2::from_array(*uv),
                        right,
                        up,
                        facing.inverse,
                        facing.scale,
                    )
                };
                put(vertices, i * source.layout.stride + at, point.to_array());
            }
        });
    }
}

/// Authored PBR adaptation, not the original wrapped-normal/backlight shader.
pub fn configure_material(name: &str, material: &mut StandardMaterial) {
    if !name.starts_with("trees/") {
        return;
    }
    // Original foliage vertex lighting has no front-face normal inversion.
    // In Bevy, double_sided changes lighting, whereas cull_mode controls visibility.
    material.double_sided = false;
    material.perceptual_roughness = 1.;
    material.reflectance = 0.1;
    material.diffuse_transmission = if name.starts_with("trees/leaf/") {
        0.3
    } else {
        0.
    };
}
