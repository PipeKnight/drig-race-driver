//! In-place vertex updates for meshes whose size stays fixed (P12, F392,
//! docs/PERFORMANCE.md). Replacing a `Mesh` asset makes Bevy clone it into
//! the render world, free and reallocate its slab space and repack it on the
//! render thread. Systems here instead pack the new vertex bytes in the main
//! world exactly as `Mesh::write_packed_vertex_buffer_data` would, and the
//! render world writes them over the mesh's existing allocation. The asset
//! itself is left unchanged, so it must already have the final vertex count,
//! attribute set and indices. Presentation only.
//!
//! Bevy sorts alpha-blended meshes by the centre of the mesh asset's bounding
//! box (`RenderMesh::aabb_center`), taken when the asset is prepared and
//! copied into the retained `Transparent3d` item when the entity is queued.
//! A streamed asset is neither, so its padding and its old vertices would set
//! the order. [`MeshStream::sort_centre`] gives the centre a replaced asset
//! would have had; it is written into the queued item before Bevy sorts,
//! transformed exactly as Bevy's queue does. It is written again whenever
//! Bevy re-queues the entity (which writes the stale asset's centre), so the
//! order changes exactly when it changed with replaced assets (F398).
use bevy::{
    asset::RenderAssetUsages,
    core_pipeline::core_3d::{Transparent3d, TransparentSortingInfo3d},
    mesh::{Indices, MeshVertexAttribute, MeshVertexAttributeId, PrimitiveTopology},
    prelude::*,
    render::{
        Extract, ExtractSchedule, Render, RenderApp, RenderSystems,
        mesh::allocator::{MeshAllocator, allocate_and_free_meshes},
        render_phase::{ViewSortedRenderPhases, sort_phase_system},
        renderer::RenderQueue,
        sync_world::MainEntity,
    },
};
use bevy::{
    pbr::{MeshInputUniform, MeshUniform, RenderMeshInstances, get_mesh_instance_world_from_local},
    render::batching::gpu_preprocessing::BatchedInstanceBuffers,
};
use std::{
    collections::{HashMap, HashSet},
    ops::Range,
    sync::{Arc, Mutex},
};

/// Vertex bytes written this frame, handed from the main world to the render
/// world at extraction. Both sides keep their buffers, so after the first
/// frames nothing is allocated.
/// One write: the mesh, its bytes, and for a part of the buffer its first
/// vertex and stride.
type Write = (AssetId<Mesh>, Range<usize>, Option<(u32, usize)>);

#[derive(Default)]
struct Writes {
    bytes: Vec<u8>,
    meshes: Vec<Write>,
    /// Local-space transparent sort centres by main-world entity.
    centres: Vec<(Entity, Vec3)>,
}

impl Writes {
    fn clear(&mut self) {
        self.bytes.clear();
        self.meshes.clear();
        self.centres.clear();
    }
}

#[derive(Resource, Clone, Default)]
pub(crate) struct MeshStream(Arc<Mutex<Writes>>);

#[derive(Resource, Default)]
struct RenderWrites(Writes);

impl MeshStream {
    /// Queue `mesh`'s whole vertex buffer: `fill` appends exactly the packed
    /// bytes (vertex count × [`VertexLayout::stride`]).
    pub(crate) fn write(&self, mesh: AssetId<Mesh>, fill: impl FnOnce(&mut Vec<u8>)) {
        let mut writes = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let start = writes.bytes.len();
        fill(&mut writes.bytes);
        let end = writes.bytes.len();
        writes.meshes.push((mesh, start..end, None));
    }

    /// Queue part of `mesh`'s vertex buffer from vertex `first`: `fill`
    /// appends whole packed vertices of `stride` bytes. The rest of the
    /// buffer keeps what was written before (Bevy's slab allocator copies a
    /// slab's contents when it grows it) (F592).
    pub(crate) fn write_at(
        &self,
        mesh: AssetId<Mesh>,
        first: u32,
        stride: usize,
        fill: impl FnOnce(&mut Vec<u8>),
    ) {
        let mut writes = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let start = writes.bytes.len();
        fill(&mut writes.bytes);
        let end = writes.bytes.len();
        writes.meshes.push((mesh, start..end, Some((first, stride))));
    }

    /// Sort the alpha-blended `entity` as if its mesh asset had just been
    /// replaced: `centre` is the local-space centre of the bounding box of
    /// the drawn vertices (`Aabb::enclosing`, without the padding), as
    /// `RenderMesh::aabb_center`. It stays in force until the next call.
    pub(crate) fn sort_centre(&self, entity: Entity, centre: Vec3) {
        let mut writes = self.0.lock().unwrap_or_else(|e| e.into_inner());
        writes.centres.push((entity, centre));
    }

    /// Sort centre queued for `entity` since the last extraction (tests).
    #[cfg(test)]
    pub(crate) fn queued_centre(&self, entity: Entity) -> Option<Vec3> {
        let writes = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let (_, centre) = writes.centres.iter().rev().find(|(e, _)| *e == entity)?;
        Some(*centre)
    }

    /// Bytes queued for `mesh` since the last extraction (tests).
    #[cfg(test)]
    pub(crate) fn queued(&self, mesh: AssetId<Mesh>) -> Option<Vec<u8>> {
        let writes = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let (_, range, _) = writes.meshes.iter().rev().find(|(id, ..)| *id == mesh)?;
        Some(writes.bytes[range.clone()].to_vec())
    }
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<MeshStream>();
    if let Some(render) = app.get_sub_app_mut(RenderApp) {
        render
            .init_resource::<RenderWrites>()
            .add_systems(ExtractSchedule, extract)
            .add_systems(
                Render,
                (
                    upload
                        .in_set(RenderSystems::PrepareAssets)
                        .after(allocate_and_free_meshes),
                    resort
                        .in_set(RenderSystems::PhaseSort)
                        .before(sort_phase_system::<Transparent3d>),
                ),
            );
    } else {
        // Nothing extracts without a render world: drop each frame's bytes.
        app.add_systems(First, |stream: Res<MeshStream>| {
            stream.0.lock().unwrap_or_else(|e| e.into_inner()).clear();
        });
    }
}

fn extract(main: Extract<Option<Res<MeshStream>>>, mut render: ResMut<RenderWrites>) {
    let Some(main) = main.as_ref() else { return };
    let mut writes = main.0.lock().unwrap_or_else(|e| e.into_inner());
    // The render side consumed its buffers last frame; swap them back empty.
    render.0.clear();
    std::mem::swap(&mut *writes, &mut render.0);
}

/// After Bevy (re)allocated this frame's extracted meshes, so a mesh that was
/// also re-extracted keeps the streamed bytes (queue writes apply in order).
fn upload(writes: Res<RenderWrites>, allocator: Res<MeshAllocator>, queue: Res<RenderQueue>) {
    for (mesh, range, at) in &writes.0.meshes {
        // Not allocated yet (first frame of a new asset): its own data is drawn.
        let Some(slice) = allocator.mesh_vertex_slice(mesh) else {
            continue;
        };
        let bytes = &writes.0.bytes[range.clone()];
        let vertices = slice.range.end - slice.range.start;
        let (first, stride) = match *at {
            Some((first, stride)) => {
                if stride == 0
                    || !bytes.len().is_multiple_of(stride)
                    || first as usize + bytes.len() / stride > vertices as usize
                {
                    continue;
                }
                (u64::from(first), stride as u64)
            }
            None => {
                if vertices == 0 || !bytes.len().is_multiple_of(vertices as usize) {
                    continue;
                }
                (0, (bytes.len() / vertices as usize) as u64)
            }
        };
        queue.write_buffer(slice.buffer, (u64::from(slice.range.start) + first) * stride, bytes);
    }
}

/// After the queue systems (which re-add an entity whose asset changed, with
/// the prepared asset's centre) and before the transparent sort, which
/// recomputes every item's distance from its stored centre.
///
/// `known` keeps, per entity, the latest local centre and the world centre
/// last written. A centre sent this frame is written as a replaced asset
/// would have been queued. Otherwise an item whose centre differs from the
/// one written was re-queued by Bevy from the stale asset (it became visible
/// again or its material changed), and gets the current centre; an item
/// left alone keeps its old centre, as a retained item of an unchanged asset.
fn resort(
    writes: Res<RenderWrites>,
    mut phases: ResMut<ViewSortedRenderPhases<Transparent3d>>,
    instances: Res<RenderMeshInstances>,
    buffers: Option<Res<BatchedInstanceBuffers<MeshUniform, MeshInputUniform>>>,
    mut known: Local<HashMap<Entity, (Vec3, Option<Vec3>)>>,
) {
    let mut sent = HashSet::new();
    for (entity, local) in &writes.0.centres {
        known.insert(*entity, (*local, None));
        sent.insert(*entity);
    }
    if known.is_empty() {
        return;
    }
    for (entity, (local, written)) in known.iter_mut() {
        let main = MainEntity::from(*entity);
        let Some(data) = instances.render_mesh_queue_data(main) else {
            continue;
        };
        let world = get_mesh_instance_world_from_local(
            main,
            data.current_uniform_index,
            &instances,
            buffers.as_deref(),
        )
        .transform_point3(*local);
        let mut wrote = false;
        for phase in phases.values_mut() {
            // Mesh items are keyed with a placeholder render entity (queue_material_meshes).
            let key = (Entity::PLACEHOLDER, main);
            if let Some(item) = phase.items.get_mut(&key)
                && let TransparentSortingInfo3d::Sorted { mesh_center, .. } = &mut item.sorting_info
                && (sent.contains(entity) || Some(*mesh_center) != *written)
            {
                *mesh_center = world;
                wrote = true;
            }
        }
        if wrote {
            *written = Some(world);
        }
    }
}

/// Byte layout of a mesh's packed vertex buffer: attributes interleaved per
/// vertex in attribute-id order, as `Mesh::write_packed_vertex_buffer_data`.
#[derive(Clone, Debug)]
pub(crate) struct VertexLayout {
    pub stride: usize,
    offsets: Vec<(MeshVertexAttributeId, usize)>,
}

impl VertexLayout {
    pub(crate) fn of(mesh: &Mesh) -> Self {
        let mut stride = 0;
        let mut offsets = Vec::new();
        for (attribute, _) in mesh.attributes() {
            offsets.push((attribute.id, stride));
            stride += attribute.format.size() as usize;
        }
        Self { stride, offsets }
    }

    pub(crate) fn offset(&self, attribute: MeshVertexAttribute) -> Option<usize> {
        self.offsets
            .iter()
            .find(|(id, _)| *id == attribute.id)
            .map(|(_, offset)| *offset)
    }
}

/// Little-endian f32 components at `at` (wgpu vertex data).
pub(crate) fn put<const N: usize>(bytes: &mut [u8], at: usize, value: [f32; N]) {
    for (k, v) in value.into_iter().enumerate() {
        bytes[at + 4 * k..at + 4 * k + 4].copy_from_slice(&v.to_le_bytes());
    }
}

/// One billboard: corners in draw order, one normal and one colour.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Quad {
    pub positions: [[f32; 3]; 4],
    pub normal: [f32; 3],
    pub uvs: [[f32; 2]; 4],
    pub colour: [f32; 4],
}

/// Unused capacity: zero-area quads, which draw nothing.
const PAD: Quad = Quad {
    positions: [[0.; 3]; 4],
    normal: [0., 1., 0.],
    uvs: [[0.; 2]; 4],
    colour: [0.; 4],
};

/// A billboard mesh (POSITION, NORMAL, UV_0, COLOR, u32 indices) whose asset
/// holds `capacity` quads: the drawn ones first, then zero-area padding, so
/// the picture is that of the drawn quads alone. The asset is replaced only
/// when the quads outgrow it; otherwise their vertices are streamed.
#[derive(Default)]
pub(crate) struct QuadMesh {
    capacity: usize,
    layout: Option<VertexLayout>,
}

impl QuadMesh {
    /// `triangles`: the two triangles of quad corners 0..4.
    pub(crate) fn update(
        &mut self,
        quads: &[Quad],
        triangles: [u32; 6],
        mesh: &Handle<Mesh>,
        meshes: &mut Assets<Mesh>,
        stream: &MeshStream,
    ) {
        if quads.len() > self.capacity {
            // AUTHORED headroom, so the asset is rarely replaced again.
            self.capacity = quads.len() + quads.len() / 4;
            let asset = quad_mesh(quads, self.capacity, triangles);
            self.layout = Some(VertexLayout::of(&asset));
            if let Some(mut target) = meshes.get_mut(mesh) {
                *target = asset;
            }
        } else if let Some(layout) = &self.layout {
            stream.write(mesh.id(), |bytes| {
                pack_quads(quads, self.capacity, layout, bytes)
            });
        }
    }
}

fn padded(quads: &[Quad], capacity: usize) -> impl Iterator<Item = &Quad> {
    quads
        .iter()
        .chain(std::iter::repeat_n(&PAD, capacity.saturating_sub(quads.len())))
}

fn quad_mesh(quads: &[Quad], capacity: usize, triangles: [u32; 6]) -> Mesh {
    let mut positions = Vec::with_capacity(capacity * 4);
    let mut normals = Vec::with_capacity(capacity * 4);
    let mut uvs = Vec::with_capacity(capacity * 4);
    let mut colours = Vec::with_capacity(capacity * 4);
    let mut indices = Vec::with_capacity(capacity * 6);
    for q in padded(quads, capacity) {
        let base = positions.len() as u32;
        positions.extend(q.positions);
        normals.extend([q.normal; 4]);
        uvs.extend(q.uvs);
        colours.extend([q.colour; 4]);
        indices.extend(triangles.map(|k| base + k));
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colours);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// The packed vertex bytes of `quad_mesh(quads, capacity, _)`.
fn pack_quads(quads: &[Quad], capacity: usize, layout: &VertexLayout, bytes: &mut Vec<u8>) {
    let at = |attribute| layout.offset(attribute).expect("billboard attribute");
    let (p, n, uv, c) = (
        at(Mesh::ATTRIBUTE_POSITION),
        at(Mesh::ATTRIBUTE_NORMAL),
        at(Mesh::ATTRIBUTE_UV_0),
        at(Mesh::ATTRIBUTE_COLOR),
    );
    let start = bytes.len();
    bytes.resize(start + capacity * 4 * layout.stride, 0);
    let mut vertex = start;
    for q in padded(quads, capacity) {
        for k in 0..4 {
            put(bytes, vertex + p, q.positions[k]);
            put(bytes, vertex + n, q.normal);
            put(bytes, vertex + uv, q.uvs[k]);
            put(bytes, vertex + c, q.colour);
            vertex += layout.stride;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streamed_quads_equal_the_padded_asset() {
        let quads: Vec<_> = (0..3)
            .map(|i| Quad {
                positions: std::array::from_fn(|k| [i as f32, k as f32 * 0.5, -1.25]),
                normal: [0.1, 0.9, 0.2],
                uvs: std::array::from_fn(|k| [k as f32 * 0.25, 1. - i as f32 * 0.125]),
                colour: [1., 0.5, 0.25, 0.75 - i as f32 * 0.25],
            })
            .collect();
        let mesh = quad_mesh(&quads, 5, [0, 1, 2, 2, 1, 3]);
        assert_eq!(mesh.count_vertices(), 20);
        let Some(Indices::U32(indices)) = mesh.indices() else {
            panic!("u32 indices")
        };
        assert_eq!(indices[6..12], [4, 5, 6, 6, 5, 7]);
        let layout = VertexLayout::of(&mesh);
        let mut bytes = vec![7];
        pack_quads(&quads, 5, &layout, &mut bytes);
        assert_eq!(bytes[0], 7);
        assert_eq!(&bytes[1..], mesh.create_packed_vertex_buffer_data());
        // Fewer quads later: the rest of the capacity is zero-area padding.
        let mut fewer = Vec::new();
        pack_quads(&quads[..1], 5, &layout, &mut fewer);
        let padded = quad_mesh(&quads[..1], 5, [0, 1, 2, 2, 1, 3]);
        assert_eq!(fewer, padded.create_packed_vertex_buffer_data());
    }

    #[test]
    fn quad_mesh_grows_once_then_streams() {
        let mut meshes = Assets::<Mesh>::default();
        let handle = meshes.add(quad_mesh(&[], 0, [0, 1, 2, 0, 2, 3]));
        let stream = MeshStream::default();
        let mut quads = QuadMesh::default();
        let one = [PAD; 4].map(|q| Quad { colour: [1.; 4], ..q });
        quads.update(&one, [0, 1, 2, 0, 2, 3], &handle, &mut meshes, &stream);
        assert_eq!(meshes.get(&handle).unwrap().count_vertices(), 20);
        assert!(stream.queued(handle.id()).is_none());
        quads.update(&one[..2], [0, 1, 2, 0, 2, 3], &handle, &mut meshes, &stream);
        assert_eq!(meshes.get(&handle).unwrap().count_vertices(), 20);
        assert_eq!(stream.queued(handle.id()).unwrap().len(), 20 * (12 + 12 + 8 + 16));
    }

    #[test]
    fn padding_moves_the_sort_centre_so_it_is_queued_separately() {
        use bevy::camera::primitives::MeshAabb;
        let quad = Quad {
            positions: [[10., 2., -30.], [12., 2., -30.], [12., 4., -30.], [10., 4., -30.]],
            ..PAD
        };
        let centre = |mesh: Mesh| Vec3::from(mesh.compute_aabb().unwrap().center);
        let drawn = centre(quad_mesh(&[quad], 1, [0, 1, 2, 0, 2, 3]));
        assert_eq!(drawn, Vec3::new(11., 3., -30.));
        // Bevy sorts blended meshes by this centre: the padding at the origin
        // moves it, which reordered the smoke groups (F394).
        assert_ne!(centre(quad_mesh(&[quad], 4, [0, 1, 2, 0, 2, 3])), drawn);
        let stream = MeshStream::default();
        let entity = Entity::from_raw_u32(7).unwrap();
        stream.sort_centre(entity, drawn);
        assert_eq!(stream.queued_centre(entity), Some(drawn));
    }

    #[test]
    fn layout_and_put_match_bevy_packing() {
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::all());
        // Inserted out of id order on purpose.
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[0.5f32, 0.25, 1., 0.75]; 3]);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.1f32, 0.9]; 3]);
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            vec![[1f32, 2., 3.], [4., 5., 6.], [-7., 8.5, 9.]],
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0f32, 1., 0.]; 3]);
        mesh.insert_indices(Indices::U32(vec![0, 1, 2]));
        let layout = VertexLayout::of(&mesh);
        assert_eq!(layout.stride, 12 + 12 + 8 + 16);
        let mut bytes = vec![0; 3 * layout.stride];
        let positions = [[1f32, 2., 3.], [4., 5., 6.], [-7., 8.5, 9.]];
        for (i, p) in positions.into_iter().enumerate() {
            let at = i * layout.stride;
            put(&mut bytes, at + layout.offset(Mesh::ATTRIBUTE_POSITION).unwrap(), p);
            put(&mut bytes, at + layout.offset(Mesh::ATTRIBUTE_NORMAL).unwrap(), [0., 1., 0.]);
            put(&mut bytes, at + layout.offset(Mesh::ATTRIBUTE_UV_0).unwrap(), [0.1, 0.9]);
            put(
                &mut bytes,
                at + layout.offset(Mesh::ATTRIBUTE_COLOR).unwrap(),
                [0.5, 0.25, 1., 0.75],
            );
        }
        assert_eq!(bytes, mesh.create_packed_vertex_buffer_data());
        assert_eq!(layout.offset(Mesh::ATTRIBUTE_UV_1), None);
    }
}
