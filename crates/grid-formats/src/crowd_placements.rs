//! Bounded observed crowd placement framing. Does not evaluate moving paths,
//! clocks, orientation conventions, animation, randomization, or LOD selection.
//! Observed format; unsupported moving payloads remain opaque.
use crate::{Result, fail, range};
#[derive(Debug, Clone)]
pub struct Cell {
    pub grid_index: u32,
    pub instances: u32,
    pub moving_instances: u32,
    pub bounds: [f32; 6],
}
#[derive(Debug, Clone)]
pub struct Standing {
    pub orientation_pair: [f32; 2],
    pub position: [f32; 3],
    pub opaque_scalar: f32,
    pub cell: u32,
}
#[derive(Debug)]
pub enum Placements<'a> {
    Standing(Vec<Standing>),
    /// Preserved, not interpreted. Count is the enclosing Group's count.
    Moving(&'a [u8]),
}
#[derive(Debug)]
pub struct Group<'a> {
    pub instance_set: &'a [u8],
    pub count: u32,
    pub placements: Placements<'a>,
}
#[derive(Debug)]
pub struct Crowd<'a> {
    pub origin_xz: [f32; 2],
    pub cell_size_xz: [f32; 2],
    pub dimensions: [u32; 2],
    pub cells: Vec<Cell>,
    pub groups: Vec<Group<'a>>,
    pub total_instances: u32,
}
fn word(b: &[u8], p: usize) -> Result<u32> {
    range(p, 4, b.len())?;
    Ok(u32::from_le_bytes(b[p..p + 4].try_into().unwrap()))
}
fn floats<const N: usize>(b: &[u8], p: usize) -> Result<[f32; N]> {
    range(p, N * 4, b.len())?;
    let f = std::array::from_fn(|i| f32::from_bits(word(b, p + i * 4).unwrap()));
    if f.iter().any(|v| !v.is_finite()) {
        return fail(p, "nonfinite crowd value");
    }
    Ok(f)
}
fn span(b: &[u8], start: usize, n: usize, stride: usize) -> Result<std::ops::Range<usize>> {
    range(
        start,
        n.checked_mul(stride).ok_or(crate::Error {
            offset: start,
            message: "crowd size overflow",
        })?,
        b.len(),
    )
}
/// Observed little-endian format. Moving groups are exposed opaquely and must
/// be explicitly rejected by a caller requiring complete animated placement.
pub fn read(b: &[u8]) -> Result<Crowd<'_>> {
    if word(b, 0)? != 12 || word(b, 4)? != 36 {
        return fail(0, "unsupported crowd header");
    }
    let types = word(b, 8)? as usize;
    let origin_xz = floats(b, 12)?;
    let cell_size_xz = floats::<2>(b, 20)?;
    let dimensions = [word(b, 28)?, word(b, 32)?];
    if cell_size_xz.iter().any(|v| *v <= 0.) || dimensions.contains(&0) {
        return fail(20, "invalid crowd grid");
    }
    let count = word(b, 36)? as usize;
    if count > 1_000_000 || word(b, 40)? != 48 || word(b, 44)? as usize != count {
        return fail(36, "invalid crowd cells");
    }
    let cells_range = span(b, 48, count, 40)?;
    if cells_range.end != types {
        return fail(8, "crowd cell/type offset mismatch");
    }
    let grid_size = u64::from(dimensions[0]) * u64::from(dimensions[1]);
    let mut cells = Vec::with_capacity(count);
    for i in 0..count {
        let p = 48 + i * 40;
        let grid_index = word(b, p + 4)?;
        let instances = word(b, p + 8)?;
        let moving_instances = word(b, p + 12)?;
        let bounds = floats::<6>(b, p + 16)?;
        if word(b, p)? as usize != i
            || u64::from(grid_index) >= grid_size
            || (moving_instances != 0 && moving_instances != instances)
            || (0..3).any(|j| bounds[j] > bounds[j + 3])
        {
            return fail(p, "invalid crowd cell");
        }
        cells.push(Cell {
            grid_index,
            instances,
            moving_instances,
            bounds,
        });
    }
    range(types, 24, b.len())?;
    let ng = word(b, types)? as usize;
    let total_instances = word(b, types + 4)?;
    let mp = word(b, types + 8)? as usize;
    let nm = word(b, types + 12)? as usize;
    let sp = word(b, types + 16)? as usize;
    let ns = word(b, types + 20)? as usize;
    if ng > 100_000
        || total_instances > 2_000_000
        || nm.checked_add(ns) != Some(ng)
        || mp != types + 24
        || span(b, mp, nm, 16)?.end != sp
    {
        return fail(types, "invalid crowd group header");
    }
    let headers = span(b, mp, ng, 16)?;
    if ng == 0 {
        return fail(types, "empty crowd catalogue unsupported");
    }
    let mut desc = Vec::with_capacity(ng);
    let mut string_start = b.len();
    for i in 0..ng {
        let p = mp + i * 16;
        let np = word(b, p)? as usize;
        let n = word(b, p + 4)?;
        let dp = word(b, p + 8)? as usize;
        if n != word(b, p + 12)? || dp < headers.end || np < headers.end || np >= b.len() {
            return fail(p, "crowd descriptor range");
        }
        string_start = string_start.min(np);
        desc.push((np, n, dp));
    }
    if desc[0].2 != headers.end {
        return fail(headers.start, "crowd body start");
    }
    let mut names = std::collections::BTreeMap::new();
    let mut seen = vec![0u32; count];
    let mut groups = Vec::with_capacity(ng);
    let mut total = 0u64;
    let mut moving_total = 0u64;
    for (i, &(np, n, dp)) in desc.iter().enumerate() {
        let end = if i + 1 < ng {
            desc[i + 1].2
        } else {
            string_start
        };
        let body = range(
            dp,
            end.checked_sub(dp).ok_or(crate::Error {
                offset: dp,
                message: "reversed crowd body",
            })?,
            string_start,
        )?;
        let sl = &b[np..];
        let len = sl
            .iter()
            .take(129)
            .position(|v| *v == 0)
            .ok_or(crate::Error {
                offset: np,
                message: "crowd name terminator",
            })?;
        let name = &sl[..len];
        if name.is_empty()
            || !name.iter().all(u8::is_ascii)
            || names.insert(np, np + len + 1).is_some()
        {
            return fail(np, "crowd name invalid/overlapping");
        }
        total += u64::from(n);
        // Enforce the declared bounded total before allocating a standing group.
        // Malformed input must not allocate/scan payloads that can only fail
        // the final total check (up to ~120GB from one u32 count otherwise).
        if total > u64::from(total_instances) {
            return fail(dp, "crowd group exceeds declared total");
        }
        let placements = if i < nm {
            moving_total += u64::from(n);
            Placements::Moving(&b[body])
        } else {
            if span(b, dp, n as usize, 28)? != body {
                return fail(dp, "crowd standing stride");
            }
            let mut rows = Vec::with_capacity(n as usize);
            for j in 0..n as usize {
                let p = dp + j * 28;
                let v = floats::<6>(b, p)?;
                let cell = word(b, p + 24)?;
                let c = cells.get(cell as usize).ok_or(crate::Error {
                    offset: p + 24,
                    message: "crowd cell reference",
                })?;
                if c.moving_instances != 0 {
                    return fail(p + 24, "standing crowd references moving cell");
                }
                let norm = v[0] * v[0] + v[1] * v[1];
                if (norm - 1.).abs() > 0.0002
                    || (0..3)
                        .any(|k| v[k + 2] < c.bounds[k] - 0.01 || v[k + 2] > c.bounds[k + 3] + 0.01)
                {
                    return fail(p, "crowd orientation/bounds");
                }
                // Verify authored cell membership in f64: four installed records lie
                // close enough to a boundary that f32 subtraction rounds across it.
                let col = ((f64::from(v[2]) - f64::from(origin_xz[0]))
                    / f64::from(cell_size_xz[0]))
                .floor();
                let row = ((f64::from(v[4]) - f64::from(origin_xz[1]))
                    / f64::from(cell_size_xz[1]))
                .floor();
                if col < 0.
                    || row < 0.
                    || col >= f64::from(dimensions[0])
                    || row >= f64::from(dimensions[1])
                    || (row as u64) * u64::from(dimensions[0]) + col as u64
                        != u64::from(c.grid_index)
                {
                    return fail(p, "crowd grid position");
                }
                seen[cell as usize] += 1;
                rows.push(Standing {
                    orientation_pair: [v[0], v[1]],
                    position: [v[2], v[3], v[4]],
                    opaque_scalar: v[5],
                    cell,
                });
            }
            Placements::Standing(rows)
        };
        groups.push(Group {
            instance_set: name,
            count: n,
            placements,
        });
    }
    let mut next = string_start;
    for (start, end) in names {
        if start != next {
            return fail(start, "crowd string table gap/overlap");
        }
        next = end;
    }
    if next != b.len()
        || total != u64::from(total_instances)
        || moving_total
            != cells
                .iter()
                .map(|c| u64::from(c.moving_instances))
                .sum::<u64>()
        || cells
            .iter()
            .enumerate()
            .any(|(i, c)| seen[i] != c.instances - c.moving_instances)
    {
        return fail(types, "crowd total/tail accounting");
    }
    Ok(Crowd {
        origin_xz,
        cell_size_xz,
        dimensions,
        cells,
        groups,
        total_instances,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn put(b: &mut [u8], p: usize, v: u32) {
        b[p..p + 4].copy_from_slice(&v.to_le_bytes());
    }
    fn fixture() -> Vec<u8> {
        let mut b = vec![0u8; 161];
        for (p, v) in [
            (0, 12),
            (4, 36),
            (8, 88),
            (28, 1),
            (32, 1),
            (36, 1),
            (40, 48),
            (44, 1),
            (56, 1),
            (88, 1),
            (92, 1),
            (96, 112),
            (104, 112),
            (108, 1),
            (112, 156),
            (116, 1),
            (120, 128),
            (124, 1),
        ] {
            put(&mut b, p, v);
        }
        for (p, v) in [
            (20, 20f32),
            (24, 20.),
            (76, 10.),
            (80, 2.),
            (84, 10.),
            (132, 1.),
            (136, 4.),
            (140, 1.),
            (144, 5.),
            (148, 7.5),
        ] {
            put(&mut b, p, v.to_bits());
        }
        b[156..].copy_from_slice(b"m_gr\0");
        b
    }
    #[test]
    fn source_offsets_and_standing_cell_accounting() {
        let b = fixture();
        let c = read(&b).unwrap();
        assert_eq!(c.total_instances, 1);
        assert_eq!(c.groups[0].instance_set, b"m_gr");
        let Placements::Standing(r) = &c.groups[0].placements else {
            panic!()
        };
        assert_eq!(r[0].position, [4., 1., 5.]);
        assert_eq!(r[0].opaque_scalar.to_bits(), 7.5f32.to_bits());
        for cut in 0..b.len() {
            assert!(read(&b[..cut]).is_err(), "cut {cut}");
        }
    }
    #[test]
    fn corrupt_ranges_counts_and_positions_reject() {
        for (p, v) in [
            (0, 0),
            (8, u32::MAX),
            (36, u32::MAX),
            (40, 49),
            (48, 1),
            (52, 1),
            (56, 2),
            (60, 1),
            (88, 2),
            (92, 2),
            (96, u32::MAX),
            (100, 1),
            (104, 113),
            (108, 2),
            (112, 0),
            (116, 2),
            (120, 129),
            (124, 2),
            (152, 1),
            (128, f32::NAN.to_bits()),
            (132, 0),
            (136, 40f32.to_bits()),
        ] {
            let mut b = fixture();
            put(&mut b, p, v);
            assert!(read(&b).is_err(), "mutation {p}");
        }
    }
    #[test]
    fn opaque_moving_payload_is_preserved_without_standing_decode() {
        let mut b = fixture();
        put(&mut b, 60, 1); // the one cell contains one moving actor
        put(&mut b, 100, 1); // one moving group
        put(&mut b, 104, 128); // static table follows the moving descriptor
        put(&mut b, 108, 0); // zero standing groups
        b[128..156].fill(0xff); // intentionally not finite Standing values
        let c = read(&b).unwrap();
        let Placements::Moving(raw) = &c.groups[0].placements else {
            panic!("expected opaque moving payload")
        };
        assert_eq!(*raw, &b[128..156]);
        assert_eq!(c.groups[0].count, 1);
    }
    #[test]
    fn group_count_exceeding_declared_total_rejects_before_rows() {
        let first = fixture();
        let mut b = vec![0u8; 189];
        b[..156].copy_from_slice(&first[..156]);
        b[156..184].copy_from_slice(&first[128..156]);
        b[184..].copy_from_slice(b"m_gr\0");
        put(&mut b, 56, 2); // valid cell capacity for the two real payload rows
        put(&mut b, 112, 184);
        put(&mut b, 116, 2);
        put(&mut b, 124, 2);
        // Header total remains one, even though the complete body holds two.
        let error = read(&b).unwrap_err();
        assert_eq!(error.offset, 128);
        assert_eq!(error.message, "crowd group exceeds declared total");
    }
    #[test]
    #[ignore = "requires purchased GRID_GAME_ROOT; CPU only, no GPU"]
    fn purchased_all_route_crowd_placements() {
        fn visit(dir: &std::path::Path, paths: &mut Vec<std::path::PathBuf>, depth: usize) {
            assert!(depth < 12, "unexpected purchased track directory depth");
            for entry in std::fs::read_dir(dir).unwrap() {
                let entry = entry.unwrap();
                let ty = entry.file_type().unwrap();
                if ty.is_dir() {
                    visit(&entry.path(), paths, depth + 1);
                } else if ty.is_file() && entry.file_name() == "crowd_standing.bin" {
                    paths.push(entry.path());
                }
            }
        }
        let root = std::path::PathBuf::from(
            std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required"),
        );
        let mut paths = Vec::new();
        visit(&root.join("tracks"), &mut paths, 0);
        paths.sort();
        assert_eq!(paths.len(), 97);
        let mut standing_records = 0u64;
        for path in paths {
            let bytes = std::fs::read(&path).unwrap();
            let c = read(&bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let types = word(&bytes, 8).unwrap() as usize;
            let descriptors = word(&bytes, types + 8).unwrap() as usize;
            let string_start = (0..c.groups.len())
                .map(|i| word(&bytes, descriptors + i * 16).unwrap() as usize)
                .min()
                .unwrap();
            for (i, g) in c.groups.iter().enumerate() {
                let begin = word(&bytes, descriptors + i * 16 + 8).unwrap() as usize;
                let end = if i + 1 < c.groups.len() {
                    word(&bytes, descriptors + (i + 1) * 16 + 8).unwrap() as usize
                } else {
                    string_start
                };
                match &g.placements {
                    Placements::Standing(rows) => {
                        assert_eq!(rows.len(), g.count as usize);
                        for (j, row) in rows.iter().enumerate() {
                            let offset = begin + j * 28;
                            let values = [
                                row.orientation_pair[0],
                                row.orientation_pair[1],
                                row.position[0],
                                row.position[1],
                                row.position[2],
                                row.opaque_scalar,
                            ];
                            for (k, value) in values.into_iter().enumerate() {
                                assert_eq!(
                                    value.to_bits(),
                                    word(&bytes, offset + k * 4).unwrap(),
                                    "{} row{j} component{k}",
                                    path.display()
                                );
                            }
                            assert_eq!(row.cell, word(&bytes, offset + 24).unwrap());
                        }
                        assert_eq!(end - begin, rows.len() * 28);
                        standing_records += rows.len() as u64;
                    }
                    Placements::Moving(raw) => assert_eq!(*raw, &bytes[begin..end]),
                }
            }
        }
        // f64 cell membership in read() also covers the four Jarama boundaries.
        assert_eq!(standing_records, 1_291_209);
    }
}
