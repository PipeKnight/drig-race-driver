//! Positive-count CQTC boundary geometry. Node/list framing is admitted here;
//! traversal and original collision dispatch are separate behavior contracts.
use crate::{Result, collision::Triangle, fail, range};
use std::collections::BTreeMap;

pub struct Chunk<'a> {
    pub bounds: [f32; 6],
    pub materials: Vec<[u8; 4]>,
    pub packed_vertices: &'a [u8],
    pub positions: Vec<[f32; 3]>,
    pub triangles: Vec<Triangle>,
    pub nodes: Vec<u32>,
    pub references: BTreeMap<usize, Vec<u32>>,
}
fn be24(b: &[u8]) -> u32 {
    u32::from_be_bytes([0, b[0], b[1], b[2]])
}

/// Selected bounded positive-count / 8-byte-vertex / 7-byte-triangle variant.
pub fn read(bytes: &[u8]) -> Result<Chunk<'_>> {
    range(0, 56, bytes.len())?;
    if bytes.len() > 2 * 1024 * 1024 {
        return fail(0, "CQTC source exceeds bound");
    }
    let word = |p| u32::from_le_bytes(bytes[p..p + 4].try_into().unwrap());
    let bounds = std::array::from_fn(|i| f32::from_bits(word(i * 4)));
    if bounds.iter().any(|v| !v.is_finite()) || (0..3).any(|i| bounds[i] >= bounds[i + 3]) {
        return fail(0, "invalid CQTC quantization bounds");
    }
    let [nt, nv, nm]: [usize; 3] = std::array::from_fn(|i| word(24 + i * 4) as usize);
    if !(1..=32768).contains(&nt) || !(1..=65535).contains(&nv) || !(1..=8).contains(&nm) {
        return fail(24, "unsupported CQTC count variant");
    }
    let [vo, no, to, ro]: [usize; 4] = std::array::from_fn(|i| word(36 + i * 4) as usize);
    if vo != 52 + nm * 4
        || no != vo + nv * 8
        || to <= no
        || !(to - no).is_multiple_of(3)
        || ro != to + nt * 7
    {
        return fail(36, "CQTC section boundaries disagree");
    }
    range(0, ro, bytes.len())?;
    let materials = bytes[52..vo].as_chunks::<4>().0.to_vec();
    let packed_vertices = &bytes[vo..no];
    let scale = [
        (bounds[3] - bounds[0]) / 16777216.,
        (bounds[4] - bounds[1]) / 65536.,
        (bounds[5] - bounds[2]) / 16777216.,
    ];
    if scale.iter().any(|v| !v.is_finite()) {
        return fail(0, "CQTC bounds overflow");
    }
    let positions = packed_vertices
        .as_chunks::<8>()
        .0
        .iter()
        .map(|v| {
            let p = [
                be24(v),
                u16::from_be_bytes([v[3], v[4]]) as u32,
                be24(&v[5..]),
            ];
            std::array::from_fn(|j| p[j] as f32 * scale[j] + bounds[j])
        })
        .collect();
    let mut triangles = Vec::with_capacity(nt);
    for (i, t) in bytes[to..ro].as_chunks::<7>().0.iter().enumerate() {
        let a = be24(t);
        let indices = [
            a,
            a + t[4] as u32 + ((t[3] as u32 & 240) << 4),
            a + t[5] as u32 + ((t[3] as u32 & 15) << 8),
        ];
        if indices.iter().any(|&x| x as usize >= nv) || t[6] as usize >= nm {
            return fail(to + i * 7, "CQTC triangle index/material outside table");
        }
        triangles.push(Triangle {
            indices: indices.map(|x| x as u16),
            material: t[6],
            flags: 0,
        });
    }
    let nodes: Vec<_> = bytes[no..to]
        .as_chunks::<3>()
        .0
        .iter()
        .map(|n| be24(n))
        .collect();
    let mut references = BTreeMap::new();
    let mut cursor = ro;
    while cursor < bytes.len() {
        let start = cursor - ro;
        range(cursor, 3, bytes.len())?;
        let first = be24(&bytes[cursor..]);
        cursor += 3;
        let base = first & 0x7fffff;
        if base as usize >= nt {
            return fail(cursor - 3, "CQTC leaf base outside triangle table");
        }
        let mut list = vec![base];
        if first & 0x800000 == 0 {
            loop {
                range(cursor, 2, bytes.len())?;
                let delta = u16::from_be_bytes(bytes[cursor..cursor + 2].try_into().unwrap());
                cursor += 2;
                // Offsets are relative to the initial base, not cumulative.
                let id = base + u32::from(delta & 0x7fff);
                if id as usize >= nt {
                    return fail(cursor - 2, "CQTC leaf delta outside triangle table");
                }
                list.push(id);
                if delta & 0x8000 != 0 {
                    break;
                }
            }
        }
        references.insert(start, list);
    }
    for (i, &node) in nodes.iter().enumerate() {
        if node == 0xffffff {
            continue;
        }
        if node & 0x800000 != 0 {
            if !references.contains_key(&((node & 0x7fffff) as usize)) {
                return fail(no + i * 3, "CQTC node references absent leaf list");
            }
        } else if node as usize + 4 > nodes.len() {
            return fail(no + i * 3, "CQTC child span outside node table");
        }
    }
    Ok(Chunk {
        bounds,
        materials,
        packed_vertices,
        positions,
        triangles,
        nodes,
        references,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source() -> Vec<u8> {
        let mut b = Vec::new();
        for x in [0f32, 0., 0., 1., 1., 1.] {
            b.extend(x.to_le_bytes());
        }
        for x in [3u32, 4, 1, 56, 88, 91, 112] {
            b.extend(x.to_le_bytes());
        }
        b.extend(*b"TEST");
        b.extend([0u8; 32]);
        b.extend([0x80, 0, 0]);
        for _ in 0..3 {
            b.extend([0, 0, 0, 0, 1, 2, 0]);
        }
        b.extend([0, 0, 0, 0, 1, 0x80, 2]);
        b
    }
    #[test]
    fn base_relative_lists_and_rejected_unbounded_references() {
        let b = source();
        let c = read(&b).unwrap();
        assert_eq!(c.references[&0], [0, 1, 2]);
        assert_eq!(c.triangles[0].indices, [0, 1, 2]);
        assert_eq!(c.positions, [[0.; 3]; 4]);
        for n in [0, 55, 111, b.len() - 1] {
            assert!(read(&b[..n]).is_err());
        }
        let mut bad = b.clone();
        bad[90] = 1;
        assert!(read(&bad).is_err());
        let mut bad = b.clone();
        bad[91] = 1;
        assert!(read(&bad).is_err());
        let mut bad = b.clone();
        bad[118] = 3;
        assert!(read(&bad).is_err());
        let mut bad = b.clone();
        bad[88..91].copy_from_slice(&[0, 0, 1]);
        assert!(read(&bad).is_err());
        let mut singleton = b[..112].to_vec();
        singleton.extend([0x80, 0, 2]);
        assert_eq!(read(&singleton).unwrap().references[&0], [2]);
    }
}
