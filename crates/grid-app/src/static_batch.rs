//! Static world batching (F576). The track's static parts are merged into one
//! mesh per material and 64 m cell, so the per-entity passes (visibility,
//! shadow cascades, mesh collection, draw queues) see hundreds of entities
//! instead of thousands. Vertices are moved into the cell's frame exactly as
//! the vertex shader would place them, so the picture does not change. A part
//! alone in its material and cell gathers with the others of a 256 m cell
//! (F591).
//!
//! Intact breakable props are merged the same way in batches of their own.
//! Their original entities stay (without a mesh) and keep the visibility that
//! `dynamic_props::present` and the replay set; a batch drops a prop's
//! triangles while it is hidden (broken) and restores them when it shows
//! again (a restart). Foliage that faces the camera, car parts and pieces keep
//! their own entities. Common build; `GRID_STATIC_BATCH=0` keeps every part
//! separate (comparisons).
use crate::track_surface::{SOURCE_BINORMAL, SOURCE_TANGENT, SourcePart, SurfaceMaterial};
use bevy::{
    camera::{
        primitives::{Aabb, MeshAabb},
        visibility::{NoAutoAabb, RenderLayers, VisibilityClass, VisibilityRange, VisibilitySystems},
    },
    light::NotShadowCaster,
    mesh::{Indices, MeshTag, MeshVertexAttributeId, PrimitiveTopology, VertexAttributeValues, VertexFormat},
    prelude::*,
};
use std::collections::HashMap;
use std::ops::Range;

/// Edge of a batch cell on the ground plane, metres (`GRID_STATIC_BATCH_CELL`
/// overrides it for measurements).
const CELL_METRES: f32 = 64.;
/// Cells per side of the coarse cell that gathers single parts.
const COARSE: i32 = 4;

fn cell_metres() -> f32 {
    crate::dev::var("GRID_STATIC_BATCH_CELL")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|m: &f32| *m >= 8.)
        .unwrap_or(CELL_METRES)
}
/// Vertices per batch: 16-bit indices.
pub(crate) const MAX_VERTICES: usize = 1 << 16;
const INTACT: &str = "dynamic/intact/";
const PIECE: &str = "dynamic/piece/";

pub fn enabled() -> bool {
    crate::dev::var("GRID_STATIC_BATCH").map_or(true, |v| v != "0")
}

pub fn install(app: &mut App) {
    if !enabled() {
        println!("static_batch version=1 enabled=false");
        return;
    }
    app.add_systems(
        PostStartup,
        (merge::<StandardMaterial>, merge::<SurfaceMaterial>, merge::<crate::foundation_backing::RawMaterial>)
            .chain()
            .after(crate::track_surface::attach),
    )
    .add_systems(
        PostUpdate,
        refresh.before(VisibilitySystems::VisibilityPropagate),
    );
}

/// A merged mesh: its full index list and the index range of every part.
/// Parts with a source follow that entity's visibility (intact props).
#[derive(Component)]
pub struct Batch {
    indices: Indices,
    parts: Vec<(Option<Entity>, Range<usize>)>,
    shown: Vec<bool>,
}

type Plain = (
    With<SourcePart>,
    Without<crate::source_trees::Facing>,
    Without<crate::MovingPart>,
    Without<crate::OpponentPart>,
    Without<ChildOf>,
    Without<Children>,
    Without<RenderLayers>,
    Without<NotShadowCaster>,
    Without<VisibilityRange>,
);

pub(crate) const ATTRIBUTES: [MeshVertexAttributeId; 8] = [
    Mesh::ATTRIBUTE_POSITION.id,
    Mesh::ATTRIBUTE_NORMAL.id,
    Mesh::ATTRIBUTE_UV_0.id,
    Mesh::ATTRIBUTE_UV_1.id,
    Mesh::ATTRIBUTE_COLOR.id,
    Mesh::ATTRIBUTE_TANGENT.id,
    SOURCE_TANGENT.id,
    SOURCE_BINORMAL.id,
];

#[derive(Hash, PartialEq, Eq)]
struct Key {
    material: bevy::asset::UntypedAssetId,
    layout: Vec<(MeshVertexAttributeId, VertexFormat)>,
    intact: bool,
    cell: (i32, i32),
    mesh_tag: Option<u32>,
}

pub(crate) struct Part<M: Asset> {
    pub(crate) entity: Entity,
    pub(crate) material: Handle<M>,
    pub(crate) intact: bool,
    pub(crate) mesh: Handle<Mesh>,
    pub(crate) matrix: Mat4,
    pub(crate) vertices: usize,
}

#[allow(clippy::type_complexity)]
fn merge<M: Material>(
    mut commands: Commands,
    parts: Query<(Entity, &Name, &Mesh3d, &MeshMaterial3d<M>, &Transform, &Visibility, Option<&MeshTag>), Plain>,
    mut meshes: ResMut<Assets<Mesh>>,
    materials: Option<Res<Assets<M>>>,
) {
    let Some(materials) = materials else {
        return;
    };
    let cell_m = cell_metres();
    let mut groups: HashMap<Key, Vec<Part<M>>> = HashMap::new();
    let (mut blended, mut unsupported) = (0, 0);
    for (entity, name, mesh, material, transform, visibility, tag) in &parts {
        let name = name.as_str();
        if name.starts_with(PIECE) || *visibility != Visibility::Inherited {
            continue;
        }
        if !materials
            .get(&material.0)
            .is_some_and(|m| matches!(m.alpha_mode(), AlphaMode::Opaque | AlphaMode::Mask(_))
                || (std::any::TypeId::of::<M>() == std::any::TypeId::of::<crate::foundation_backing::RawMaterial>()
                    && m.alpha_mode() == AlphaMode::AlphaToCoverage))
        {
            blended += 1;
            continue;
        }
        let Some(source) = meshes.get(&mesh.0) else {
            unsupported += 1;
            continue;
        };
        let layout: Vec<_> = source
            .attributes()
            .map(|(a, v)| (a.id, VertexFormat::from(v)))
            .collect();
        let vertices = source.count_vertices();
        if source.primitive_topology() != PrimitiveTopology::TriangleList
            || source.indices().is_none()
            || vertices >= MAX_VERTICES
            || !layout.iter().all(|(a, f)| ATTRIBUTES.contains(a) && supported(*f))
        {
            unsupported += 1;
            continue;
        }
        let matrix = transform.to_matrix();
        let centre = source
            .compute_aabb()
            .map_or(Vec3::ZERO, |a| Vec3::from(a.center));
        let centre = matrix.transform_point3(centre);
        let cell = (
            (centre.x / cell_m).floor() as i32,
            (centre.z / cell_m).floor() as i32,
        );
        let intact = name.starts_with(INTACT);
        groups
            .entry(Key {
                material: material.0.id().untyped(),
                layout,
                intact,
                cell,
                mesh_tag: tag.map(|tag| tag.0),
            })
            .or_default()
            .push(Part {
                entity,
                material: material.0.clone(),
                intact,
                mesh: mesh.0.clone(),
                matrix,
                vertices,
            });
    }
    let mut stats = [0usize; 5]; // parts, batches, intact parts, intact batches, vertices
    // A part alone in its cell and material gathers with the others of a
    // cell COARSE times wider (F591): fewer entities for the cascade and
    // camera passes, the same triangles.
    let (mut pass, mut size, mut single) = (groups, cell_m, 0);
    for round in 0..2 {
        let mut leftovers: HashMap<Key, Vec<Part<M>>> = HashMap::new();
        let mut keys: Vec<_> = pass.into_iter().collect();
        keys.sort_by_key(|(k, _)| (k.intact, k.cell));
        for (key, mut group) in keys {
            group.sort_by_key(|p| p.entity);
            let origin = Vec3::new(
                (key.cell.0 as f32 + 0.5) * size,
                0.,
                (key.cell.1 as f32 + 0.5) * size,
            );
            let mut start = 0;
            while start < group.len() {
                let mut end = start;
                let mut count = 0;
                while end < group.len() && count + group[end].vertices <= MAX_VERTICES {
                    count += group[end].vertices;
                    end += 1;
                }
                let chunk = &group[start..end];
                start = end;
                if chunk.len() < 2 && !key.intact {
                    let part = &chunk[0];
                    if round == 0 {
                        leftovers
                            .entry(Key {
                                material: key.material,
                                layout: key.layout.clone(),
                                intact: false,
                                mesh_tag: key.mesh_tag,
                                cell: (key.cell.0.div_euclid(COARSE), key.cell.1.div_euclid(COARSE)),
                            })
                            .or_default()
                            .push(Part {
                                entity: part.entity,
                                material: part.material.clone(),
                                intact: false,
                                mesh: part.mesh.clone(),
                                matrix: part.matrix,
                                vertices: part.vertices,
                            });
                    } else if let Some(aabb) = meshes.get(&part.mesh).and_then(|m| m.compute_aabb()) {
                        // A single part stays, with fixed bounds.
                        single += 1;
                        commands.entity(part.entity).insert((aabb, NoAutoAabb));
                    }
                    continue;
                }
                let (mesh, indices, ranges) = build(chunk, &meshes, origin, &key.layout);
                let parts: Vec<_> = chunk
                    .iter()
                    .zip(ranges)
                    .map(|(p, r)| (p.intact.then_some(p.entity), r))
                    .collect();
                stats[if key.intact { 2 } else { 0 }] += chunk.len();
                stats[if key.intact { 3 } else { 1 }] += 1;
                stats[4] += count;
                let shown = vec![true; parts.len()];
                let bounds = mesh.compute_aabb().unwrap_or_default();
                let mut batch = commands.spawn((
                    Name::new(format!("static_batch/{}/{}_{}", if key.intact { "intact" } else { "track" }, key.cell.0, key.cell.1)),
                    Mesh3d(meshes.add(mesh)),
                    MeshMaterial3d(chunk[0].material.clone()),
                    Transform::from_translation(origin),
                    Batch { indices, parts, shown },
                    // Hiding a prop only drops triangles: the bounds stay valid.
                    bounds,
                    NoAutoAabb,
                ));
                if let Some(tag) = key.mesh_tag {
                    batch.insert(MeshTag(tag));
                }
                for part in chunk {
                    if part.intact {
                        commands
                            .entity(part.entity)
                            .remove::<(Mesh3d, MeshMaterial3d<M>, Aabb, VisibilityClass)>();
                    } else {
                        commands.entity(part.entity).despawn();
                    }
                }
            }
        }
        pass = leftovers;
        size *= COARSE as f32;
    }
    println!(
        "static_batch version=1 material={} parts={} batches={} intact_parts={} intact_batches={} vertices={} blended={blended} unsupported={unsupported} single={single} cell_m={cell_m}",
        std::any::type_name::<M>().rsplit("::").next().unwrap_or("?"),
        stats[0],
        stats[1],
        stats[2],
        stats[3],
        stats[4],
    );
}

pub(crate) fn supported(format: VertexFormat) -> bool {
    matches!(
        format,
        VertexFormat::Float32x2 | VertexFormat::Float32x3 | VertexFormat::Float32x4
    )
}

/// The parts' vertices in the frame of `origin`, as the vertex shader places
/// them: positions by the matrix, normals by its inverse transpose
/// (normalized, as the shader does), tangents by the matrix with the sign of
/// its determinant. Triangle order and winding are kept.
pub(crate) fn build<M: Asset>(
    parts: &[Part<M>],
    meshes: &Assets<Mesh>,
    origin: Vec3,
    layout: &[(MeshVertexAttributeId, VertexFormat)],
) -> (Mesh, Indices, Vec<Range<usize>>) {
    // A batch is drawn even when its parts' own meshes stay in the main
    // world (car_batch::Demoted).
    let source_usages = meshes.get(&parts[0].mesh).map(|m| m.asset_usage).unwrap_or_default()
        | bevy::asset::RenderAssetUsages::RENDER_WORLD;
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, source_usages);
    let mut indices = Vec::new();
    let mut ranges = Vec::with_capacity(parts.len());
    let mut values: Vec<VertexAttributeValues> = layout
        .iter()
        .map(|(_, f)| match f {
            VertexFormat::Float32x2 => VertexAttributeValues::Float32x2(Vec::new()),
            VertexFormat::Float32x3 => VertexAttributeValues::Float32x3(Vec::new()),
            _ => VertexAttributeValues::Float32x4(Vec::new()),
        })
        .collect();
    let mut base = 0u32;
    for part in parts {
        let source = meshes.get(&part.mesh).expect("batched source mesh");
        for (out, placed) in values.iter_mut().zip(place(part.matrix, source, origin, layout)) {
            match (out, placed) {
                (VertexAttributeValues::Float32x3(out), VertexAttributeValues::Float32x3(v)) => out.extend(v),
                (VertexAttributeValues::Float32x4(out), VertexAttributeValues::Float32x4(v)) => out.extend(v),
                (VertexAttributeValues::Float32x2(out), VertexAttributeValues::Float32x2(v)) => out.extend(v),
                _ => unreachable!("layout checked by merge"),
            }
        }
        let first = indices.len();
        indices.extend(source.indices().expect("indexed").iter().map(|i| base + i as u32));
        ranges.push(first..indices.len());
        base += source.count_vertices() as u32;
    }
    for ((id, _), v) in layout.iter().zip(values) {
        let attribute = ATTRIBUTE_BY_ID
            .iter()
            .find(|a| a.id == *id)
            .expect("known attribute");
        mesh.insert_attribute(*attribute, v);
    }
    let indices = Indices::U16(indices.into_iter().map(|i| i as u16).collect());
    mesh.insert_indices(indices.clone());
    (mesh, indices, ranges)
}

/// One part's vertices as [`build`] places them, one value list per `layout`
/// attribute (car_batch patches a dented part's vertices with it).
pub(crate) fn place(
    matrix: Mat4,
    source: &Mesh,
    origin: Vec3,
    layout: &[(MeshVertexAttributeId, VertexFormat)],
) -> Vec<VertexAttributeValues> {
    let m3 = Mat3::from_mat4(matrix);
    let normal = m3.inverse().transpose();
    let sign = m3.determinant().signum();
    layout
        .iter()
        .map(|(id, _)| {
            let attribute = source
                .attributes()
                .find(|(a, _)| a.id == *id)
                .map(|(_, v)| v)
                .expect("batched layout");
            match attribute {
                VertexAttributeValues::Float32x3(v) if *id == Mesh::ATTRIBUTE_POSITION.id => {
                    VertexAttributeValues::Float32x3(
                        v.iter()
                            .map(|p| (matrix.transform_point3(Vec3::from(*p)) - origin).to_array())
                            .collect(),
                    )
                }
                VertexAttributeValues::Float32x3(v) if *id == Mesh::ATTRIBUTE_NORMAL.id => {
                    VertexAttributeValues::Float32x3(
                        v.iter().map(|n| (normal * Vec3::from(*n)).normalize_or_zero().to_array()).collect(),
                    )
                }
                VertexAttributeValues::Float32x4(v) if *id == Mesh::ATTRIBUTE_TANGENT.id => {
                    VertexAttributeValues::Float32x4(
                        v.iter()
                            .map(|t| {
                                let xyz = (m3 * Vec3::new(t[0], t[1], t[2])).normalize_or_zero();
                                [xyz.x, xyz.y, xyz.z, t[3] * sign]
                            })
                            .collect(),
                    )
                }
                other => other.clone(),
            }
        })
        .collect()
}

const ATTRIBUTE_BY_ID: [bevy::mesh::MeshVertexAttribute; 8] = [
    Mesh::ATTRIBUTE_POSITION,
    Mesh::ATTRIBUTE_NORMAL,
    Mesh::ATTRIBUTE_UV_0,
    Mesh::ATTRIBUTE_UV_1,
    Mesh::ATTRIBUTE_COLOR,
    Mesh::ATTRIBUTE_TANGENT,
    SOURCE_TANGENT,
    SOURCE_BINORMAL,
];

/// Intact batches follow their props: a hidden prop's triangles leave the
/// index list, and a batch with nothing shown is hidden.
fn refresh(
    mut batches: Query<(&mut Batch, &Mesh3d, &mut Visibility)>,
    sources: Query<&Visibility, Without<Batch>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    for (mut batch, mesh, mut visibility) in &mut batches {
        let batch = &mut *batch;
        let mut changed = false;
        for ((source, _), shown) in batch.parts.iter().zip(&mut batch.shown) {
            let Some(source) = source else { continue };
            let now = sources.get(*source).is_ok_and(|v| *v != Visibility::Hidden);
            changed |= now != *shown;
            *shown = now;
        }
        if !changed {
            continue;
        }
        let any = batch.shown.iter().any(|s| *s);
        visibility.set_if_neq(if any { Visibility::Inherited } else { Visibility::Hidden });
        if !any {
            continue;
        }
        let Indices::U16(all) = &batch.indices else {
            continue;
        };
        let kept: Vec<u16> = batch
            .parts
            .iter()
            .zip(&batch.shown)
            .filter(|(_, s)| **s)
            .flat_map(|((_, r), _)| all[r.clone()].iter().copied())
            .collect();
        if let Some(mut target) = meshes.get_mut(&mesh.0) {
            target.insert_indices(Indices::U16(kept));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::RenderAssetUsages;

    fn triangle() -> Mesh {
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(
                Mesh::ATTRIBUTE_POSITION,
                vec![[0., 0., 0.], [1., 0., 0.], [0., 0., 1.]],
            )
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0., 1., 0.]; 3])
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0., 0.], [1., 0.], [0., 1.]])
            .with_inserted_indices(Indices::U16(vec![0, 2, 1]))
    }

    #[test]
    fn source_track_vectors_keep_their_world_space_and_original_colour() {
        let mut meshes = Assets::<Mesh>::default();
        let t = [[0.5, 0., 1.], [-1., 0., 0.], [1., 2., 3.]];
        let b = [[0., 0.25, -1.], [1., 1., 2.], [3., 0., 1.]];
        let colour = [[0., 0.2, 0.35, 1.], [0.1, 0., 0.85, 0.6], [1., 0.3, 0.4, 0.]];
        let mesh = triangle()
            .with_inserted_attribute(SOURCE_TANGENT, t.to_vec())
            .with_inserted_attribute(SOURCE_BINORMAL, b.to_vec())
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colour.to_vec());
        let layout: Vec<_> = mesh
            .attributes()
            .map(|(attribute, values)| (attribute.id, VertexFormat::from(values)))
            .collect();
        assert!(layout.iter().all(|(id, format)| ATTRIBUTES.contains(id) && supported(*format)));
        let handle = meshes.add(mesh);
        let transform = Mat4::from_scale_rotation_translation(
            Vec3::new(-1., 2., 1.5),
            Quat::from_rotation_y(0.7),
            Vec3::new(12., 1., -8.),
        );
        let part = |matrix| Part::<SurfaceMaterial> {
            entity: Entity::PLACEHOLDER,
            material: Handle::default(),
            intact: false,
            mesh: handle.clone(),
            matrix,
            vertices: 3,
        };
        let origin = Vec3::new(10., 5., -7.);
        let (merged, indices, ranges) =
            build(&[part(Mat4::IDENTITY), part(transform)], &meshes, origin, &layout);
        assert_eq!(ranges, vec![0..3, 3..6]);
        assert_eq!(indices.iter().collect::<Vec<_>>(), vec![0, 2, 1, 3, 5, 4]);
        assert_eq!(
            merged.attribute(SOURCE_TANGENT),
            Some(&VertexAttributeValues::Float32x3([t, t].concat())),
        );
        assert_eq!(
            merged.attribute(SOURCE_BINORMAL),
            Some(&VertexAttributeValues::Float32x3([b, b].concat())),
        );
        assert_eq!(
            merged.attribute(Mesh::ATTRIBUTE_COLOR),
            Some(&VertexAttributeValues::Float32x4([colour, colour].concat())),
        );
        let Some(VertexAttributeValues::Float32x3(positions)) =
            merged.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("positions");
        };
        assert!(
            (Vec3::from(positions[4]) + origin)
                .abs_diff_eq(transform.transform_point3(Vec3::X), 1e-5),
        );
    }

    #[test]
    fn parts_keep_their_shader_placement_and_order() {
        let mut meshes = Assets::<Mesh>::default();
        let mesh = meshes.add(triangle());
        let layout: Vec<_> = triangle()
            .attributes()
            .map(|(a, v)| (a.id, VertexFormat::from(v)))
            .collect();
        // A rotated, non-uniformly scaled part and a mirrored one.
        let a = Mat4::from_scale_rotation_translation(
            Vec3::new(2., 1., 3.),
            Quat::from_rotation_z(0.3),
            Vec3::new(100., 5., -40.),
        );
        let b = Mat4::from_scale_rotation_translation(Vec3::new(-1., 1., 1.), Quat::IDENTITY, Vec3::new(90., 0., -30.));
        let part = |matrix| Part::<StandardMaterial> {
            entity: Entity::PLACEHOLDER,
            material: Handle::default(),
            intact: false,
            mesh: mesh.clone(),
            matrix,
            vertices: 3,
        };
        let origin = Vec3::new(96., 0., -32.);
        let (merged, indices, ranges) = build(&[part(a), part(b)], &meshes, origin, &layout);
        assert_eq!(ranges, vec![0..3, 3..6]);
        // Winding kept, second part offset by 3 vertices.
        assert_eq!(indices.iter().collect::<Vec<_>>(), vec![0, 2, 1, 3, 5, 4]);
        let Some(VertexAttributeValues::Float32x3(p)) = merged.attribute(Mesh::ATTRIBUTE_POSITION) else {
            panic!("positions");
        };
        let Some(VertexAttributeValues::Float32x3(n)) = merged.attribute(Mesh::ATTRIBUTE_NORMAL) else {
            panic!("normals");
        };
        for (i, (matrix, local)) in [(a, [1., 0., 0.]), (b, [0., 0., 1.])].iter().enumerate() {
            let world = Vec3::from(p[i * 3 + 1 + i]) + origin;
            assert!(world.abs_diff_eq(matrix.transform_point3(Vec3::from(*local)), 1e-4), "{world}");
            let expected = (Mat3::from_mat4(*matrix).inverse().transpose() * Vec3::Y).normalize();
            assert!(Vec3::from(n[i * 3]).abs_diff_eq(expected, 1e-6));
        }
    }
}
