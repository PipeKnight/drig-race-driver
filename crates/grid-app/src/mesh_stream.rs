//! In-place vertex updates for meshes whose size stays fixed (P12, F392,
//! docs/PERFORMANCE.md). Replacing a `Mesh` asset makes Bevy clone it into
//! the render world, free and reallocate its slab space and repack it on the
//! render thread. Systems here instead pack the new vertex bytes in the main
//! world exactly as `Mesh::write_packed_vertex_buffer_data` would, and the
//! render world writes them over the mesh's existing allocation. The asset
//! itself is left unchanged, so it must already have the final vertex count,
//! attribute set and indices. Presentation only.
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, MeshVertexAttribute, MeshVertexAttributeId, PrimitiveTopology},
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems,
        mesh::allocator::{MeshAllocator, allocate_and_free_meshes},
        renderer::RenderQueue,
        Extract, ExtractSchedule,
    },
};
use std::{
    ops::Range,
    sync::{Arc, Mutex},
};

/// Vertex bytes written this frame, handed from the main world to the render
/// world at extraction. Both sides keep their buffers, so after the first
/// frames nothing is allocated.
#[derive(Default)]
struct Writes {
    bytes: Vec<u8>,
    meshes: Vec<(AssetId<Mesh>, Range<usize>)>,
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
        writes.meshes.push((mesh, start..end));
    }

    /// Bytes queued for `mesh` since the last extraction (tests).
    #[cfg(test)]
    pub(crate) fn queued(&self, mesh: AssetId<Mesh>) -> Option<Vec<u8>> {
        let writes = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let (_, range) = writes.meshes.iter().rev().find(|(id, _)| *id == mesh)?;
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
                upload
                    .in_set(RenderSystems::PrepareAssets)
                    .after(allocate_and_free_meshes),
            );
    } else {
        // Nothing extracts without a render world: drop each frame's bytes.
        app.add_systems(First, |stream: Res<MeshStream>| {
            let mut writes = stream.0.lock().unwrap_or_else(|e| e.into_inner());
            writes.bytes.clear();
            writes.meshes.clear();
        });
    }
}

fn extract(main: Extract<Option<Res<MeshStream>>>, mut render: ResMut<RenderWrites>) {
    let Some(main) = main.as_ref() else { return };
    let mut writes = main.0.lock().unwrap_or_else(|e| e.into_inner());
    // The render side consumed its buffers last frame; swap them back empty.
    render.0.bytes.clear();
    render.0.meshes.clear();
    std::mem::swap(&mut *writes, &mut render.0);
}

/// After Bevy (re)allocated this frame's extracted meshes, so a mesh that was
/// also re-extracted keeps the streamed bytes (queue writes apply in order).
fn upload(writes: Res<RenderWrites>, allocator: Res<MeshAllocator>, queue: Res<RenderQueue>) {
    for (mesh, range) in &writes.0.meshes {
        // Not allocated yet (first frame of a new asset): its own data is drawn.
        let Some(slice) = allocator.mesh_vertex_slice(mesh) else {
            continue;
        };
        let bytes = &writes.0.bytes[range.clone()];
        let vertices = slice.range.end - slice.range.start;
        if vertices == 0 || !bytes.len().is_multiple_of(vertices as usize) {
            continue;
        }
        let stride = (bytes.len() / vertices as usize) as u64;
        queue.write_buffer(slice.buffer, slice.range.start as u64 * stride, bytes);
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
