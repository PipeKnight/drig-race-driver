//! Track ground cover: painted grass patches (`grass.grs`), the sprite set of
//! `ground_cover.pssg` and the draw distances of `ground_cover.xml`.
//! See docs/GROUND_EFFECTS_RESEARCH.md (loader 97EAB0, sprites 977200).
use crate::{Error, Result, binxml, fail, pssg::Tree};
use std::collections::BTreeMap;

/// One painted terrain patch: triangle list with per-vertex type densities.
#[derive(Clone, Debug, PartialEq)]
pub struct Patch {
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub positions: Vec<[f32; 3]>,
    /// Density 0..1 of ground-cover types `type_01`..`type_12` per vertex.
    pub densities: Vec<[f32; 12]>,
    pub triangles: Vec<[u16; 3]>,
}

fn u32_at(b: &[u8], o: usize) -> Result<u32> {
    b.get(o..o + 4)
        .map(|w| u32::from_le_bytes(w.try_into().unwrap()))
        .ok_or(Error {
            offset: o,
            message: "truncated grass patch file",
        })
}
fn f32_at(b: &[u8], o: usize) -> Result<f32> {
    let v = f32::from_bits(u32_at(b, o)?);
    if v.is_finite() {
        Ok(v)
    } else {
        fail(o, "nonfinite grass patch bound")
    }
}

/// `u32 count`, then per patch: AABB, vertex/triangle counts, 12-byte vertices
/// (u16 quantized xyz + 12 density nibbles), u16 triangles, 4-byte alignment.
pub fn read_patches(b: &[u8]) -> Result<Vec<Patch>> {
    if b.len() > 64 << 20 {
        return fail(0, "grass patch file exceeds bound");
    }
    let count = u32_at(b, 0)? as usize;
    if count > 100_000 {
        return fail(0, "too many grass patches");
    }
    let mut p = 4;
    let mut patches = Vec::with_capacity(count);
    for _ in 0..count {
        let min: [f32; 3] = [f32_at(b, p)?, f32_at(b, p + 4)?, f32_at(b, p + 8)?];
        let max: [f32; 3] = [f32_at(b, p + 12)?, f32_at(b, p + 16)?, f32_at(b, p + 20)?];
        let vertices = u32_at(b, p + 24)? as usize;
        let triangles = u32_at(b, p + 28)? as usize;
        if vertices > 65_536 || triangles > 1_000_000 {
            return fail(p, "grass patch counts exceed bound");
        }
        p += 32;
        let end = p + vertices * 12 + triangles * 6;
        if end > b.len() {
            return fail(p, "truncated grass patch");
        }
        let mut positions = Vec::with_capacity(vertices);
        let mut densities = Vec::with_capacity(vertices);
        for v in b[p..p + vertices * 12].as_chunks::<12>().0 {
            positions.push(std::array::from_fn(|k| {
                let q = f32::from(u16::from_le_bytes([v[k * 2], v[k * 2 + 1]]));
                min[k] + (max[k] - min[k]) * q / 65535.
            }));
            densities.push(std::array::from_fn(|i| {
                let byte = v[6 + i / 2];
                f32::from(if i % 2 == 0 { byte & 15 } else { byte >> 4 }) / 15.
            }));
        }
        p += vertices * 12;
        let mut list = Vec::with_capacity(triangles);
        for t in b[p..p + triangles * 6].as_chunks::<6>().0 {
            let tri: [u16; 3] =
                std::array::from_fn(|k| u16::from_le_bytes([t[k * 2], t[k * 2 + 1]]));
            if tri.iter().any(|&i| i as usize >= vertices) {
                return fail(p, "grass triangle index out of range");
            }
            list.push(tri);
        }
        p += triangles * 6;
        // Pad to 4 bytes; the final pad may be absent at end of file.
        p = (p + 3) & !3;
        patches.push(Patch {
            min,
            max,
            positions,
            densities,
            triangles: list,
        });
    }
    if p < b.len() {
        return fail(p, "trailing grass patch bytes");
    }
    Ok(patches)
}

/// `ground_cover.xml` draw distances (lod0..lod3, metres) and fade start fraction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Distances {
    pub lods: [f32; 4],
    pub fade_start: f32,
}

/// Code defaults when attributes are absent are 150/40/20/10 and 0.7 (733DE0).
pub fn read_distances(doc: &binxml::Document<'_>) -> Result<Distances> {
    let mut d = Distances {
        lods: [150., 40., 20., 10.],
        fade_start: 0.7,
    };
    let e = doc
        .elements
        .iter()
        .find(|e| e.name == "drawdistances")
        .ok_or(Error {
            offset: 0,
            message: "ground cover draw distances absent",
        })?;
    for (key, value) in &e.attributes {
        let v: f32 = value.trim().parse().map_err(|_| Error {
            offset: 0,
            message: "invalid ground cover distance",
        })?;
        if !v.is_finite() || v < 0. {
            return fail(0, "invalid ground cover distance");
        }
        match *key {
            "lod0" => d.lods[0] = v,
            "lod1" => d.lods[1] = v,
            "lod2" => d.lods[2] = v,
            "lod3" => d.lods[3] = v,
            "fadestart" => d.fade_start = v,
            _ => {}
        }
    }
    Ok(d)
}

/// One sprite of a ground-cover type: its quad corners and userdata ranges.
#[derive(Clone, Debug, PartialEq)]
pub struct Sprite {
    pub name: String,
    pub weight: f32,
    pub height_range: [f32; 2],
    pub width_range: [f32; 2],
    /// Quad corner positions (x across, y up, metres) and their UVs.
    pub corners: [[f32; 2]; 4],
    pub uvs: [[f32; 2]; 4],
}

#[derive(Clone, Debug, PartialEq)]
pub struct CoverType {
    /// 1-based `type_NN` index (density nibble NN-1).
    pub index: usize,
    /// Userdata mode: 0 billboard, 1 fixed mesh with random yaw.
    pub mode: u32,
    pub sprites: Vec<Sprite>,
}

fn reference<'a>(tree: &Tree<'a>, node: usize, key: &[u8]) -> Result<&'a [u8]> {
    let value = tree.string(node, key)?;
    value.strip_prefix(b"#").ok_or(Error {
        offset: tree.nodes[node].offset,
        message: "expected local ground cover reference",
    })
}

fn numbers(text: &str) -> Vec<f32> {
    text.split(|c: char| c == ',' || c == '(' || c == ')' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect()
}

fn floats_be(data: &[u8], count: usize, stride: usize) -> Result<Vec<[f32; 3]>> {
    if stride != 12 || data.len() != count * stride {
        return fail(0, "unsupported ground cover data block");
    }
    Ok(data
        .as_chunks::<12>()
        .0
        .iter()
        .map(|c| {
            std::array::from_fn(|k| f32::from_be_bytes(c[k * 4..k * 4 + 4].try_into().unwrap()))
        })
        .collect())
}

/// Types (`NODE` named `type_NN`) and their billboard sprites (`RENDERNODE`).
pub fn read_types(tree: &Tree<'_>) -> Result<Vec<CoverType>> {
    let mut ids = BTreeMap::new();
    for i in 0..tree.nodes.len() {
        if let Ok(id) = tree.string(i, b"id") {
            ids.insert(id, i);
        }
    }
    let lookup = |id: &[u8], at: usize| {
        ids.get(id).copied().ok_or(Error {
            offset: at,
            message: "unresolved ground cover reference",
        })
    };
    let userdata = |node: usize| -> Result<String> {
        let Some(&u) = tree.nodes[node]
            .children
            .iter()
            .find(|&&c| tree.name(c) == b"USERDATA")
        else {
            return Ok(String::new());
        };
        let string = lookup(reference(tree, u, b"object")?, tree.nodes[u].offset)?;
        let data = tree.nodes[string]
            .children
            .iter()
            .find_map(|&c| tree.nodes[c].data)
            .unwrap_or(&[]);
        let end = data.iter().position(|&b| b == 0).unwrap_or(data.len());
        Ok(String::from_utf8_lossy(&data[..end]).into_owned())
    };
    let mut types = Vec::new();
    for i in 0..tree.nodes.len() {
        if tree.name(i) != b"NODE" {
            continue;
        }
        let Some(index) = tree
            .string(i, b"id")
            .ok()
            .and_then(|id| id.strip_prefix(b"type_"))
            .and_then(|n| std::str::from_utf8(n).ok()?.parse::<usize>().ok())
        else {
            continue;
        };
        if !(1..=12).contains(&index) {
            return fail(tree.nodes[i].offset, "ground cover type index out of range");
        }
        let mode = numbers(&userdata(i)?).get(1).copied().unwrap_or(0.) as u32;
        let mut sprites = Vec::new();
        for &r in &tree.nodes[i].children {
            if tree.name(r) != b"RENDERNODE" {
                continue;
            }
            let at = tree.nodes[r].offset;
            let values = numbers(&userdata(r)?);
            if values.len() < 5 {
                return fail(at, "incomplete ground cover sprite userdata");
            }
            let instance = tree.nodes[r]
                .children
                .iter()
                .copied()
                .find(|&c| tree.name(c) == b"RENDERSTREAMINSTANCE")
                .ok_or(Error {
                    offset: at,
                    message: "ground cover sprite without geometry",
                })?;
            let source = lookup(reference(tree, instance, b"indices")?, at)?;
            let mut corners = None;
            let mut uvs = None;
            for &stream in &tree.nodes[source].children {
                if tree.name(stream) != b"RENDERSTREAM" {
                    continue;
                }
                let block = lookup(reference(tree, stream, b"dataBlock")?, at)?;
                let descriptor = tree.nodes[block]
                    .children
                    .iter()
                    .copied()
                    .find(|&c| tree.name(c) == b"DATABLOCKSTREAM")
                    .ok_or(Error {
                        offset: at,
                        message: "ground cover block without stream",
                    })?;
                let data = tree.nodes[block]
                    .children
                    .iter()
                    .find(|&&c| tree.name(c) == b"DATABLOCKDATA")
                    .and_then(|&c| tree.nodes[c].data)
                    .ok_or(Error {
                        offset: at,
                        message: "ground cover block without data",
                    })?;
                let count = tree.integer(block, b"elementCount")? as usize;
                if count != 4 || tree.string(descriptor, b"dataType")? != b"float3" {
                    return fail(at, "ground cover sprite is not a float3 quad");
                }
                let values = floats_be(data, count, tree.integer(descriptor, b"stride")? as usize)?;
                let pairs: [[f32; 2]; 4] = std::array::from_fn(|k| [values[k][0], values[k][1]]);
                if pairs.iter().flatten().any(|v| !v.is_finite()) {
                    return fail(at, "nonfinite ground cover sprite");
                }
                match tree.string(descriptor, b"renderType")? {
                    b"Vertex" => corners = Some(pairs),
                    b"ST" => uvs = Some(pairs),
                    _ => {}
                }
            }
            sprites.push(Sprite {
                name: String::from_utf8_lossy(tree.string(r, b"id")?).into_owned(),
                weight: values[0],
                height_range: [values[1], values[2]],
                width_range: [values[3], values[4]],
                corners: corners.ok_or(Error {
                    offset: at,
                    message: "ground cover sprite without positions",
                })?,
                uvs: uvs.ok_or(Error {
                    offset: at,
                    message: "ground cover sprite without UVs",
                })?,
            });
        }
        // Shipped order lists sprites by node order; keep it deterministic by name.
        sprites.sort_by(|a, b| a.name.cmp(&b.name));
        types.push(CoverType {
            index,
            mode,
            sprites,
        });
    }
    types.sort_by_key(|t| t.index);
    Ok(types)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_quantized_patches_and_nibbles() {
        let mut b = Vec::new();
        b.extend(1u32.to_le_bytes());
        for v in [0f32, 0., 0., 10., 2., 20.] {
            b.extend(v.to_le_bytes());
        }
        b.extend(3u32.to_le_bytes());
        b.extend(1u32.to_le_bytes());
        for (q, w) in [
            ([0u16, 0, 0], [0x0fu8, 0, 0, 0, 0, 0x70]),
            ([65535, 65535, 0], [0x08, 0, 0, 0, 0, 0]),
            ([0, 0, 65535], [0x00, 0, 0, 0, 0, 0]),
        ] {
            for x in q {
                b.extend(x.to_le_bytes());
            }
            b.extend(w);
        }
        for i in [0u16, 1, 2] {
            b.extend(i.to_le_bytes());
        }
        // 4 + 32 + 36 + 6 = 78: final pad omitted at end of file.
        let patches = read_patches(&b).unwrap();
        assert_eq!(patches.len(), 1);
        let p = &patches[0];
        assert_eq!(p.positions[1], [10., 2., 0.]);
        assert_eq!(p.positions[2], [0., 0., 20.]);
        assert_eq!(p.densities[0][0], 1.);
        assert_eq!(p.densities[0][11], 7. / 15.);
        assert_eq!(p.densities[1][0], 8. / 15.);
        assert_eq!(p.triangles, vec![[0, 1, 2]]);
        let mut bad = b.clone();
        bad[b.len() - 2] = 9;
        assert!(read_patches(&bad).is_err());
        assert!(read_patches(&b[..50]).is_err());
        b.extend([0, 0, 0]);
        assert!(read_patches(&b).is_err());
    }

    #[test]
    fn parses_userdata_numbers() {
        assert_eq!(
            numbers("1.0, (1.3, 2.3), (1.0, 1.5), 90.0, 0.8, 1.0"),
            vec![1.0, 1.3, 2.3, 1.0, 1.5, 90.0, 0.8, 1.0]
        );
        assert_eq!(numbers("16.0, 0"), vec![16.0, 0.0]);
    }
}
