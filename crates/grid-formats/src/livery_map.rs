//! A car's livery map `cars/<car>/<car>_livery.pssg`: a car model whose
//! render sources carry two ST streams and no positions. The team-livery
//! compositor draws the body-paint triangles in the first ST stream's space
//! (docs/TEAM_LIVERY.md).
use crate::{Result, fail, pssg::Tree};
use std::collections::BTreeMap;

/// One UV-only render source: each vertex's place on the car's body
/// texture (`uv0`) and on the livery pattern (`uv1`), and the triangles the
/// compositor draws from it.
#[derive(Debug)]
pub struct LiveryMap<'a> {
    pub id: &'a [u8],
    pub uv0: Vec<[f32; 2]>,
    pub uv1: Vec<[f32; 2]>,
    pub indices: Vec<u16>,
}

fn st(data: &[u8], count: usize, offset: usize, stride: usize) -> Result<Vec<[f32; 2]>> {
    if count == 0 || stride < offset + 8 || count.checked_mul(stride) != Some(data.len()) {
        return fail(0, "livery ST stream layout");
    }
    Ok((0..count)
        .map(|i| {
            let at = i * stride + offset;
            let f = |j: usize| f32::from_be_bytes(data[at + j..at + j + 4].try_into().unwrap());
            [f(0), f(4)]
        })
        .collect())
}

/// [V] The body parts whose joints the compositor's scene visitor
/// (0x6C4130, table 0xDB5308) accepts; a joint qualifies when its id also
/// contains `x0_` (0xCEE17C, the first level of detail). Matching is
/// `strstr` (0xA66880).
const BODY_PARTS: [&[u8]; 27] = [
    b"_bonnet", b"_boot", b"_bumper_bl", b"_bumper_bm", b"_bumper_br", b"_bumper_fl", b"_bumper_fm",
    b"_bumper_fr", b"_canopy_b", b"_canopy_bl", b"_canopy_br", b"_canopy_f", b"_canopy_fl", b"_canopy_fr",
    b"_door_bl", b"_door_br", b"_door_fl", b"_door_fr", b"_main_body", b"_under_f", b"_under_b",
    b"_wing_fl", b"_wing_fr", b"_wing_bl", b"_wing_br", b"_wing_mirror_l", b"_wing_mirror_r",
];

/// [V] The only material the visitor keeps (0xE4FD58 indexed by the single
/// byte at 0xCEDDF4, which is 0). Other materials sample other textures
/// (cabin, suspension, lights, wheels), so their first ST is not a place on
/// the body texture.
const BODY_PAINT: &[u8] = b"Car Body Paint";

fn contains(text: &[u8], part: &[u8]) -> bool {
    text.windows(part.len()).any(|w| w == part)
}

/// The triangles the compositor draws: for every `MATRIXPALETTERENDERINSTANCE`
/// on a level-0 body-part joint whose shader is `Car Body Paint`, the index
/// range it selects from its UV-only render source (two `float2` ST streams
/// plus `SkinIndices`, ushort triangles). Sources without such an instance
/// are left out.
pub fn read<'a>(tree: &Tree<'a>) -> Result<Vec<LiveryMap<'a>>> {
    let sources = sources(tree)?;
    let mut selected: BTreeMap<&[u8], Vec<u16>> = BTreeMap::new();
    for i in 0..tree.nodes.len() {
        if tree.name(i) != b"MATRIXPALETTERENDERINSTANCE" {
            continue;
        }
        let joint = tree.nodes[i].parent.map(|p| tree.string(p, b"id")).transpose()?.unwrap_or(b"");
        if !contains(joint, b"x0_") || !BODY_PARTS.iter().any(|part| contains(joint, part)) {
            continue;
        }
        if !contains(tree.string(i, b"shader")?, BODY_PAINT) {
            continue;
        }
        let reference = tree.string(i, b"indices")?;
        let Some((&id, source)) = reference.strip_prefix(b"#").and_then(|r| sources.get_key_value(r)) else {
            // Body paint drawn from a mesh with positions is not a livery map.
            continue;
        };
        let first = tree.integer(i, b"streamOffset")? as usize;
        let count = tree.integer(i, b"elementCountFromOffset")? as usize;
        let index_first = tree.integer(i, b"indexOffset")? as usize;
        let index_count = tree.integer(i, b"indicesCountFromOffset")? as usize;
        let Some(range) = source.indices.get(index_first..index_first.saturating_add(index_count)) else {
            return fail(tree.nodes[i].offset, "livery instance index range");
        };
        if !index_count.is_multiple_of(3) || range.iter().any(|&v| !(first..first + count).contains(&(v as usize))) {
            return fail(tree.nodes[i].offset, "livery instance vertex range");
        }
        selected.entry(id).or_default().extend_from_slice(range);
    }
    Ok(sources
        .into_iter()
        .filter_map(|(id, source)| {
            let indices = selected.remove(id)?;
            Some(LiveryMap { id, uv0: source.uv0, uv1: source.uv1, indices })
        })
        .collect())
}

/// The render sources made of exactly two `float2` ST streams (plus
/// `SkinIndices`) and ushort triangles, with their whole index buffer.
/// Sources with positions are the car's ordinary meshes and are skipped.
fn sources<'a>(tree: &Tree<'a>) -> Result<BTreeMap<&'a [u8], LiveryMap<'a>>> {
    let mut blocks = BTreeMap::new();
    for i in 0..tree.nodes.len() {
        if tree.name(i) == b"DATABLOCK" {
            blocks.insert(tree.string(i, b"id")?, i);
        }
    }
    let mut out = BTreeMap::new();
    'sources: for source in 0..tree.nodes.len() {
        if tree.name(source) != b"RENDERDATASOURCE" {
            continue;
        }
        let mut uvs = Vec::new();
        let mut vertices = None;
        for &stream in &tree.nodes[source].children {
            if tree.name(stream) != b"RENDERSTREAM" {
                continue;
            }
            let reference = tree.string(stream, b"dataBlock")?;
            let Some(&block) = reference.strip_prefix(b"#").and_then(|r| blocks.get(r)) else {
                return fail(tree.nodes[stream].offset, "unresolved livery data block");
            };
            let descriptors: Vec<usize> = tree.nodes[block]
                .children
                .iter()
                .copied()
                .filter(|&i| tree.name(i) == b"DATABLOCKSTREAM")
                .collect();
            let ordinal = tree.integer(stream, b"subStream")? as usize;
            let Some(&descriptor) = descriptors.get(ordinal) else {
                return fail(tree.nodes[stream].offset, "livery substream out of range");
            };
            match (tree.string(descriptor, b"renderType")?, tree.string(descriptor, b"dataType")?) {
                (b"ST", b"float2") => {
                    let Some(data) = tree.nodes[block]
                        .children
                        .iter()
                        .find(|&&i| tree.name(i) == b"DATABLOCKDATA")
                        .and_then(|&i| tree.nodes[i].data)
                    else {
                        return fail(tree.nodes[block].offset, "livery data block without data");
                    };
                    let count = tree.integer(block, b"elementCount")? as usize;
                    uvs.push(st(
                        data,
                        count,
                        tree.integer(descriptor, b"offset")? as usize,
                        tree.integer(descriptor, b"stride")? as usize,
                    )?);
                    vertices = Some(count);
                }
                (b"SkinIndices", _) => {}
                _ => continue 'sources,
            }
        }
        let [uv0, uv1] = <[_; 2]>::try_from(uvs).map_err(|_| crate::Error {
            offset: tree.nodes[source].offset,
            message: "livery source needs two ST streams",
        })?;
        let n = vertices.unwrap_or(0);
        if uv0.len() != uv1.len() {
            return fail(tree.nodes[source].offset, "livery ST streams differ in length");
        }
        let Some(&index_source) = tree.nodes[source]
            .children
            .iter()
            .find(|&&i| tree.name(i) == b"RENDERINDEXSOURCE")
        else {
            return fail(tree.nodes[source].offset, "livery source without indices");
        };
        if tree.string(index_source, b"primitive")? != b"triangles" || tree.string(index_source, b"format")? != b"ushort" {
            return fail(tree.nodes[index_source].offset, "livery indices are not ushort triangles");
        }
        let Some(data) = tree.nodes[index_source]
            .children
            .iter()
            .find(|&&i| tree.name(i) == b"INDEXSOURCEDATA")
            .and_then(|&i| tree.nodes[i].data)
        else {
            return fail(tree.nodes[index_source].offset, "livery index data absent");
        };
        let indices: Vec<u16> = data.as_chunks::<2>().0.iter().map(|b| u16::from_be_bytes(*b)).collect();
        if indices.len() != tree.integer(index_source, b"count")? as usize
            || !indices.len().is_multiple_of(3)
            || indices.iter().any(|&i| i as usize >= n)
        {
            return fail(tree.nodes[index_source].offset, "livery index buffer");
        }
        let id = tree.string(source, b"id")?;
        out.insert(id, LiveryMap { id, uv0, uv1, indices });
    }
    Ok(out)
}
