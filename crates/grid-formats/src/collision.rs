//! Observed GRID VCQTC chunk geometry; quadtree traversal semantics stay opaque.
use crate::{Result, fail, range};
use std::collections::BTreeMap;

#[derive(Debug)]
pub struct Chunk<'a> {
    pub bounds: [f32; 6],
    pub signed_counts: [i32; 3],
    pub materials: Vec<[u8; 4]>,
    pub packed_vertices: &'a [u8],
    pub positions: Vec<[f32; 3]>,
    pub triangles: Vec<Triangle>,
    pub nodes: Vec<u16>,
    pub references: BTreeMap<usize, Vec<u16>>,
}
#[derive(Debug)]
pub struct Triangle {
    pub indices: [u16; 3],
    pub material: u8,
    /// Reserved report field, always zero in the supported subset.
    /// Original packed-index consumers do not establish independent flag bits.
    pub flags: u8,
}

/// Only the old GRID negative-count / 8-byte-vertex / 4-byte-triangle variant.
pub fn read_vcqtc(bytes: &[u8]) -> Result<Chunk<'_>> {
    range(0, 52, bytes.len())?;
    let word = |p| u32::from_le_bytes(bytes[p..p + 4].try_into().unwrap());
    let bounds = std::array::from_fn(|i| f32::from_bits(word(i * 4)));
    if bounds.iter().any(|v| !v.is_finite()) || (0..3).any(|i| bounds[i] >= bounds[i + 3]) {
        return fail(0, "invalid collision quantization bounds");
    }
    let signed_counts: [i32; 3] = std::array::from_fn(|i| word(24 + i * 4) as i32);
    if signed_counts.iter().any(|&n| n >= 0 || n == i32::MIN) {
        return fail(24, "unsupported VCQTC count variant");
    }
    let [triangles, vertices, materials] = signed_counts.map(|n| n.unsigned_abs() as usize);
    if vertices > 1024 || materials > 8 || triangles > 32768 {
        return fail(24, "VCQTC packed count limit exceeded");
    }
    let [
        vertex_offset,
        node_offset,
        triangle_offset,
        reference_offset,
    ]: [usize; 4] = std::array::from_fn(|i| word(36 + i * 4) as usize);
    if vertex_offset != 52 + materials * 4
        || node_offset != vertex_offset + vertices * 8
        || triangle_offset < node_offset
        || !(triangle_offset - node_offset).is_multiple_of(2)
        || reference_offset != triangle_offset + triangles * 4
    {
        return fail(36, "VCQTC section boundaries disagree");
    }
    range(0, reference_offset, bytes.len())?;
    let materials = bytes[52..vertex_offset].as_chunks::<4>().0.to_vec();
    let packed_vertices = &bytes[vertex_offset..node_offset];
    let scale = [
        (bounds[3] - bounds[0]) / 16777216.,
        (bounds[4] - bounds[1]) / 65536.,
        (bounds[5] - bounds[2]) / 16777216.,
    ];
    if scale.iter().any(|v| !v.is_finite()) {
        return fail(0, "collision bounds overflow");
    }
    let mut positions = Vec::with_capacity(vertices);
    for v in packed_vertices.as_chunks::<8>().0 {
        let packed = [
            u32::from_be_bytes([0, v[0], v[1], v[2]]),
            u16::from_be_bytes([v[3], v[4]]) as u32,
            u32::from_be_bytes([0, v[5], v[6], v[7]]),
        ];
        positions.push(std::array::from_fn(|i| {
            packed[i] as f32 * scale[i] + bounds[i]
        }));
    }
    let mut triangles_out = Vec::with_capacity(triangles);
    for (i, v) in bytes[triangle_offset..reference_offset]
        .as_chunks::<4>()
        .0
        .iter()
        .enumerate()
    {
        // The observed original decoder uses the full first byte as index bits.
        // Our <=1024-vertex subset therefore requires its upper two bits to be zero.
        if v[0] & 0xc0 != 0 {
            return fail(
                triangle_offset + i * 4,
                "unsupported triangle upper-index bits",
            );
        }
        let first = ((v[0] as u16) << 4) | ((v[1] as u16) >> 4);
        let indices = [first, first + v[2] as u16, first + v[3] as u16];
        if indices.iter().any(|&v| v as usize >= vertices)
            || (v[1] & 15) as usize >= materials.len()
        {
            return fail(
                triangle_offset + i * 4,
                "collision triangle index/material outside table",
            );
        }
        triangles_out.push(Triangle {
            indices,
            material: v[1] & 15,
            flags: 0,
        });
    }
    let nodes: Vec<_> = bytes[node_offset..triangle_offset]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|v| u16::from_be_bytes(*v))
        .collect();
    let mut references = BTreeMap::new();
    let mut cursor = reference_offset;
    while cursor < bytes.len() {
        let start = cursor - reference_offset;
        range(cursor, 2, bytes.len())?;
        let mut current =
            u16::from_be_bytes(bytes[cursor..cursor + 2].try_into().unwrap()) as usize;
        cursor += 2;
        if current >= triangles {
            return fail(cursor - 2, "collision leaf triangle outside table");
        }
        let mut list = vec![current as u16];
        loop {
            range(cursor, 1, bytes.len())?;
            let delta = bytes[cursor];
            cursor += 1;
            if delta == 255 {
                break;
            }
            current += delta as usize;
            if current >= triangles {
                return fail(cursor - 1, "collision leaf delta outside table");
            }
            if delta != 254 {
                list.push(current as u16);
            }
        }
        references.insert(start, list);
    }
    for (i, &node) in nodes.iter().enumerate() {
        if node & 0x8000 != 0
            && node != u16::MAX
            && !references.contains_key(&((node & 0x7fff) as usize))
        {
            return fail(
                node_offset + i * 2,
                "collision node references absent leaf list",
            );
        }
    }
    Ok(Chunk {
        bounds,
        signed_counts,
        materials,
        packed_vertices,
        positions,
        triangles: triangles_out,
        nodes,
        references,
    })
}
