//! Selected PSSG render buffers; no shader or animation behavior inferred.
use crate::{Result, fail, pssg::Tree, range};
use std::collections::BTreeMap;

#[derive(Debug)]
pub struct Mesh<'a> {
    pub id: &'a [u8],
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    /// Saved basis vectors, including nonfinite values for renderer fallback.
    pub tangents: Vec<[f32; 3]>,
    pub binormals: Vec<[f32; 3]>,
    /// Raw saved A,R,G,B bytes; these may control damage, not albedo tint.
    pub colors_argb: Vec<[u8; 4]>,
    /// Source values are preserved; invalid normals need a renderer fallback.
    pub nonfinite_normals: usize,
    /// Vertices whose half2 ST pairs decode to NaN or infinity in any ST
    /// stream (0xFFFF/0x7FFF words in Yokohama, Spa, Detroit and Haruna
    /// objects); preserved exactly like normals.
    pub nonfinite_texcoords: usize,
    pub texcoords: Vec<[f32; 2]>,
    /// Further ST streams, in render-stream order; their shader use is separate.
    pub additional_texcoords: Vec<Vec<[f32; 2]>>,
    pub skin_indices: Option<Vec<f32>>,
    pub indices: Vec<u16>,
}

fn child(tree: &Tree<'_>, node: usize, name: &[u8]) -> Result<usize> {
    let mut found = tree.nodes[node]
        .children
        .iter()
        .copied()
        .filter(|&i| tree.name(i) == name);
    let Some(index) = found.next() else {
        return fail(tree.nodes[node].offset, "required child absent");
    };
    if found.next().is_some() {
        return fail(tree.nodes[node].offset, "expected exactly one child");
    }
    Ok(index)
}

fn floats<const N: usize>(
    data: &[u8],
    count: usize,
    offset: usize,
    stride: usize,
    require_finite: bool,
) -> Result<Vec<[f32; N]>> {
    if count == 0 || count > 1_000_000 || stride < offset + N * 4 {
        return fail(0, "unsupported vertex count/stream layout");
    }
    let total = count.checked_mul(stride).ok_or(crate::Error {
        offset: 0,
        message: "stream size overflow",
    })?;
    if total != data.len() {
        return fail(0, "vertex count times stride differs from buffer size");
    }
    let mut result = Vec::with_capacity(count);
    for i in 0..count {
        let start = i * stride + offset;
        range(start, N * 4, data.len())?;
        let mut value = [0.0; N];
        for (j, v) in value.iter_mut().enumerate() {
            *v = f32::from_be_bytes(data[start + j * 4..start + j * 4 + 4].try_into().unwrap());
            if require_finite && !v.is_finite() {
                return fail(start + j * 4, "nonfinite vertex component");
            }
        }
        result.push(value);
    }
    Ok(result)
}

/// IEEE binary16 decoded exactly into binary32, including signed zero.
pub fn half_to_f32(value: u16) -> f32 {
    let sign = (value as u32 & 0x8000) << 16;
    let exponent = (value >> 10) & 31;
    let mut mantissa = value as u32 & 1023;
    let bits = match exponent {
        0 if mantissa == 0 => sign,
        0 => {
            let mut exponent = 113u32;
            while mantissa & 1024 == 0 {
                mantissa <<= 1;
                exponent -= 1;
            }
            sign | (exponent << 23) | ((mantissa & 1023) << 13)
        }
        31 => sign | 0x7f80_0000 | (mantissa << 13),
        _ => sign | (((exponent as u32) + 112) << 23) | (mantissa << 13),
    };
    f32::from_bits(bits)
}
/// The first `N` halves of each element, decoded exactly (nonfinite kept).
fn halves<const N: usize>(
    data: &[u8],
    count: usize,
    offset: usize,
    stride: usize,
) -> Result<Vec<[f32; N]>> {
    if count == 0
        || count > 1_000_000
        || stride < offset + N * 2
        || count.checked_mul(stride) != Some(data.len())
    {
        return fail(0, "unsupported half stream layout");
    }
    Ok((0..count)
        .map(|i| {
            let start = i * stride + offset;
            std::array::from_fn(|j| {
                half_to_f32(u16::from_be_bytes([
                    data[start + j * 2],
                    data[start + j * 2 + 1],
                ]))
            })
        })
        .collect())
}
fn color_words(data: &[u8], count: usize, offset: usize, stride: usize) -> Result<Vec<[u8; 4]>> {
    if count == 0
        || count > 1_000_000
        || offset.checked_add(4).is_none_or(|end| end > stride)
        || count.checked_mul(stride) != Some(data.len())
    {
        return fail(0, "unsupported ARGB color layout");
    }
    Ok((0..count)
        .map(|i| {
            data[i * stride + offset..i * stride + offset + 4]
                .try_into()
                .unwrap()
        })
        .collect())
}

pub fn read_meshes<'a>(tree: &Tree<'a>) -> Result<Vec<Mesh<'a>>> {
    read_meshes_impl(tree, false)
}
/// Explicit static subset: skinnable sources are excluded, never rigidified.
pub fn read_static_meshes<'a>(tree: &Tree<'a>) -> Result<Vec<Mesh<'a>>> {
    read_meshes_impl(tree, true)
}
fn read_meshes_impl<'a>(tree: &Tree<'a>, skip_skinned: bool) -> Result<Vec<Mesh<'a>>> {
    let mut blocks = BTreeMap::new();
    for i in 0..tree.nodes.len() {
        if tree.name(i) == b"DATABLOCK" {
            let id = tree.string(i, b"id")?;
            if blocks.insert(id, i).is_some() {
                return fail(tree.nodes[i].offset, "duplicate data block ID");
            }
        }
    }
    let mut result = Vec::new();
    let mut total_vertices = 0usize;
    let mut total_indices = 0usize;
    'sources: for source in 0..tree.nodes.len() {
        if tree.name(source) != b"RENDERDATASOURCE" {
            continue;
        }
        let id = tree.string(source, b"id")?;
        let streams: Vec<_> = tree.nodes[source]
            .children
            .iter()
            .copied()
            .filter(|&i| tree.name(i) == b"RENDERSTREAM")
            .collect();
        if streams.len() != tree.integer(source, b"streamCount")? as usize {
            return fail(tree.nodes[source].offset, "render stream count mismatch");
        }
        let mut positions = None;
        let mut normals = None;
        let mut tangents = None;
        let mut binormals = None;
        let mut colors = None;
        let mut texcoords = Vec::new();
        let mut skin_indices = None;
        for stream in streams {
            let reference = tree.string(stream, b"dataBlock")?;
            let Some(reference) = reference.strip_prefix(b"#") else {
                return fail(
                    tree.nodes[stream].offset,
                    "expected local data block reference",
                );
            };
            let Some(&block) = blocks.get(reference) else {
                return fail(tree.nodes[stream].offset, "unresolved data block reference");
            };
            let descriptors: Vec<_> = tree.nodes[block]
                .children
                .iter()
                .copied()
                .filter(|&i| tree.name(i) == b"DATABLOCKSTREAM")
                .collect();
            if descriptors.len() != tree.integer(block, b"streamCount")? as usize {
                return fail(tree.nodes[block].offset, "data block stream count mismatch");
            }
            let ordinal = tree.integer(stream, b"subStream")? as usize;
            let Some(&descriptor) = descriptors.get(ordinal) else {
                return fail(tree.nodes[stream].offset, "substream ordinal out of range");
            };
            let data_node = child(tree, block, b"DATABLOCKDATA")?;
            let data = tree.nodes[data_node].data.ok_or(crate::Error {
                offset: tree.nodes[data_node].offset,
                message: "expected raw vertex buffer",
            })?;
            if tree.integer(block, b"size")? as usize != data.len() {
                return fail(tree.nodes[block].offset, "data block size mismatch");
            }
            let kind = tree.string(descriptor, b"renderType")?;
            let format = tree.string(descriptor, b"dataType")?;
            let count = tree.integer(block, b"elementCount")? as usize;
            let offset = tree.integer(descriptor, b"offset")? as usize;
            let stride = tree.integer(descriptor, b"stride")? as usize;
            match (kind, format) {
                (b"SkinnableVertex", b"float3") if skip_skinned => continue 'sources,
                (b"Vertex", b"float3") => {
                    total_vertices = total_vertices.checked_add(count).ok_or(crate::Error {
                        offset: 0,
                        message: "vertex count overflow",
                    })?;
                    if total_vertices > 4_000_000 {
                        return fail(
                            tree.nodes[block].offset,
                            "total vertex inspector limit exceeded",
                        );
                    }
                    if positions
                        .replace(floats(data, count, offset, stride, true)?)
                        .is_some()
                    {
                        return fail(tree.nodes[stream].offset, "duplicate position stream");
                    }
                }
                (b"Normal", b"float3") => {
                    if normals
                        .replace(floats(data, count, offset, stride, false)?)
                        .is_some()
                    {
                        return fail(tree.nodes[stream].offset, "duplicate normal stream");
                    }
                }
                // Detroit's interleaved 48-byte objects: x, y, z of four halves.
                (b"Normal", b"half4") => {
                    if normals
                        .replace(halves::<3>(data, count, offset, stride)?)
                        .is_some()
                    {
                        return fail(tree.nodes[stream].offset, "duplicate normal stream");
                    }
                }
                (b"ST", b"float2") => {
                    texcoords.push(floats(data, count, offset, stride, true)?);
                }
                (b"ST", b"half2") => {
                    texcoords.push(halves::<2>(data, count, offset, stride)?);
                }
                // Retained in the tree, but not interpreted by this mesh subset.
                (b"SkinIndices", b"float") => {
                    let values: Vec<f32> = floats::<1>(data, count, offset, stride, true)?
                        .into_iter()
                        .map(|v| v[0])
                        .collect();
                    if values
                        .iter()
                        .any(|v| *v < 0.0 || *v > 65535.0 || v.fract() != 0.0)
                        || skin_indices.replace(values).is_some()
                    {
                        return fail(
                            tree.nodes[descriptor].offset,
                            "invalid or duplicate rigid skin index stream",
                        );
                    }
                }
                (b"Tangent", b"float3") | (b"Binormal", b"float3") => {
                    let target = if kind == b"Tangent" {
                        &mut tangents
                    } else {
                        &mut binormals
                    };
                    if target
                        .replace(floats(data, count, offset, stride, false)?)
                        .is_some()
                    {
                        return fail(
                            tree.nodes[stream].offset,
                            "duplicate tangent/binormal stream",
                        );
                    }
                }
                (b"Color", b"uint_color_argb") => {
                    if colors
                        .replace(color_words(data, count, offset, stride)?)
                        .is_some()
                    {
                        return fail(tree.nodes[stream].offset, "duplicate color stream");
                    }
                }
                // Other optional encodings keep the previous unconsumed behavior.
                (b"Color", _) | (b"Tangent", _) | (b"Binormal", _) => {}
                _ => {
                    return fail(
                        tree.nodes[descriptor].offset,
                        "unsupported mesh stream kind/format",
                    );
                }
            }
        }
        let Some(positions) = positions else {
            return fail(tree.nodes[source].offset, "position stream absent");
        };
        let normals = normals.unwrap_or_default();
        let tangents = tangents.unwrap_or_default();
        let binormals = binormals.unwrap_or_default();
        let colors_argb = colors.unwrap_or_default();
        if (!normals.is_empty() && normals.len() != positions.len())
            || (!tangents.is_empty() && tangents.len() != positions.len())
            || (!binormals.is_empty() && binormals.len() != positions.len())
            || (!colors_argb.is_empty() && colors_argb.len() != positions.len())
            || texcoords.iter().any(|uv| uv.len() != positions.len())
        {
            return fail(tree.nodes[source].offset, "mesh stream lengths differ");
        }
        let texcoords_first = if texcoords.is_empty() {
            vec![]
        } else {
            texcoords.remove(0)
        };
        if skin_indices
            .as_ref()
            .is_some_and(|s| s.len() != positions.len())
        {
            return fail(
                tree.nodes[source].offset,
                "skin index stream length differs",
            );
        }
        let index_source = child(tree, source, b"RENDERINDEXSOURCE")?;
        if tree.string(index_source, b"primitive")? != b"triangles"
            || tree.string(index_source, b"format")? != b"ushort"
        {
            return fail(
                tree.nodes[index_source].offset,
                "only ushort triangle indices supported",
            );
        }
        let count = tree.integer(index_source, b"count")? as usize;
        total_indices += count;
        if count == 0 || !count.is_multiple_of(3) || total_indices > 12_000_000 {
            return fail(
                tree.nodes[index_source].offset,
                "unsupported triangle index count",
            );
        }
        let index_data = child(tree, index_source, b"INDEXSOURCEDATA")?;
        let data = tree.nodes[index_data].data.unwrap();
        if count.checked_mul(2) != Some(data.len()) {
            return fail(
                tree.nodes[index_data].offset,
                "index count differs from buffer size",
            );
        }
        let mut indices = Vec::with_capacity(count);
        for bytes in data.as_chunks::<2>().0 {
            let index = u16::from_be_bytes(*bytes);
            if index as usize >= positions.len() {
                return fail(tree.nodes[index_data].offset, "index exceeds vertex count");
            }
            indices.push(index);
        }
        if indices.iter().copied().max().unwrap() as u32
            != tree.integer(index_source, b"maximumIndex")?
        {
            return fail(
                tree.nodes[index_source].offset,
                "declared maximum index differs from buffer",
            );
        }
        let nonfinite_normals = normals
            .iter()
            .filter(|n| n.iter().any(|v| !v.is_finite()))
            .count();
        let nonfinite_texcoords = (0..positions.len())
            .filter(|&i| {
                std::iter::once(&texcoords_first)
                    .chain(&texcoords)
                    .any(|uv| {
                        uv.get(i)
                            .is_some_and(|uv| uv.iter().any(|v| !v.is_finite()))
                    })
            })
            .count();
        result.push(Mesh {
            id,
            positions,
            normals,
            tangents,
            binormals,
            colors_argb,
            nonfinite_normals,
            nonfinite_texcoords,
            texcoords: texcoords_first,
            additional_texcoords: texcoords,
            skin_indices,
            indices,
        });
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn color_layout_preserves_mask_order_and_rejects_bad_ranges() {
        let data = [
            1, 2, 3, 4, 0xff, 0x00, 0xff, 0xff, 9, 8, 7, 6, 0xfe, 0xff, 0xff, 0xff,
        ];
        assert_eq!(
            color_words(&data, 2, 4, 8).unwrap(),
            vec![[255, 0, 255, 255], [254, 255, 255, 255]]
        );
        assert!(color_words(&data, 2, 5, 8).is_err());
        assert!(color_words(&data[..15], 2, 4, 8).is_err());
        assert!(color_words(&data, 0, 4, 8).is_err());
        assert!(color_words(&data, 2, usize::MAX, 8).is_err());
    }
}
