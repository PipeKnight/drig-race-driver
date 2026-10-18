//! Chunk-name grid population; source geometry and world query remain separate.
use crate::{
    Error,
    candidate_grid::{Grid, Tile},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Path {
    pub x: u32,
    pub z: u32,
    pub depth: u8,
}

impl Path {
    /// Supported canonical qt_XX[_XX...].vcqtc names, with depth 1 through 11.
    /// The original parser treats each '1' as an axis bit, other characters as zero;
    /// this API deliberately rejects noncanonical names and excessive subdivision.
    pub fn from_name(name: &str) -> Result<Self, Error> {
        let suffix = name
            .strip_prefix("qt_")
            .and_then(|s| s.strip_suffix(".vcqtc"))
            .ok_or(Error("invalid grid chunk name"))?;
        let mut result = Self {
            x: 0,
            z: 0,
            depth: 0,
        };
        let mut step = 32768;
        for pair in suffix.split('_') {
            if result.depth >= 11
                || pair.len() != 2
                || !pair.bytes().all(|b| b == b'0' || b == b'1')
            {
                return Err(Error("unsupported grid chunk path"));
            }
            result.x += u32::from(pair.as_bytes()[0] == b'1') * step;
            result.z += u32::from(pair.as_bytes()[1] == b'1') * step;
            result.depth += 1;
            step /= 2;
        }
        Ok(result)
    }
}

/// Allocate subdivisions selected by the deepest starting path in each tile,
/// then insert chunk IDs in supplied source order. Later entries overwrite cells.
pub fn build(bounds: [f32; 4], entries: &[(u32, Path)]) -> Result<Grid, Error> {
    if entries.is_empty()
        || entries.len() > 4096
        || bounds.iter().any(|v| !v.is_finite())
        || (0..2).any(|i| !((bounds[i + 2] - bounds[i]).is_finite() && bounds[i] < bounds[i + 2]))
    {
        return Err(Error("invalid grid population bounds or count"));
    }
    let mut exponents = [0u8; 1024];
    for (i, (id, path)) in entries.iter().enumerate() {
        if *id == 0
            || entries[..i].iter().any(|(old, _)| old == id)
            || !(1..=11).contains(&path.depth)
        {
            return Err(Error("invalid population ID or depth"));
        }
        let size = 65536u32 >> path.depth;
        if path.x >= 65536
            || path.z >= 65536
            || !path.x.is_multiple_of(size)
            || !path.z.is_multiple_of(size)
        {
            return Err(Error("invalid population coordinates"));
        }
        let tile = ((path.z >> 11) * 32 + (path.x >> 11)) as usize;
        exponents[tile] = exponents[tile].max(path.depth.saturating_sub(5));
    }
    let total: usize = exponents.iter().map(|e| 1usize << (2 * e)).sum();
    if total > 1024 * 1024 {
        return Err(Error("population cell allocation bound exceeded"));
    }
    let mut tiles: Vec<_> = exponents
        .into_iter()
        .map(|exponent| Tile {
            exponent,
            cells: vec![0; 1usize << (2 * exponent)],
        })
        .collect();
    for (id, path) in entries {
        let size = 65536u32 >> path.depth;
        let high = [path.x + size - 1, path.z + size - 1];
        for z in (path.z >> 11)..=(high[1] >> 11) {
            for x in (path.x >> 11)..=(high[0] >> 11) {
                let tile = &mut tiles[(z * 32 + x) as usize];
                let width = 1u32 << tile.exponent;
                let shift = 11 - tile.exponent;
                let low = [
                    (path.x >> shift).saturating_sub(x * width),
                    (path.z >> shift).saturating_sub(z * width),
                ];
                let high = [
                    (high[0] >> shift).saturating_sub(x * width).min(width - 1),
                    (high[1] >> shift).saturating_sub(z * width).min(width - 1),
                ];
                for cz in low[1]..=high[1] {
                    for cx in low[0]..=high[0] {
                        tile.cells[(cz * width + cx) as usize] = *id;
                    }
                }
            }
        }
    }
    Ok(Grid {
        bounds,
        active: true,
        tiles,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn axis_bits_subdivision_and_ordered_overwrite() {
        let a = Path::from_name("qt_10.vcqtc").unwrap();
        assert_eq!(
            a,
            Path {
                x: 32768,
                z: 0,
                depth: 1
            }
        );
        let b = Path::from_name("qt_10_00_00_00_00_00.vcqtc").unwrap();
        let grid = build([0., 0., 1., 1.], &[(7, a), (9, b)]).unwrap();
        assert_eq!(grid.tiles[16].exponent, 1);
        assert_eq!(grid.tiles[16].cells, vec![9, 7, 7, 7]);
        let reversed = build([0., 0., 1., 1.], &[(9, b), (7, a)]).unwrap();
        assert_eq!(reversed.tiles[16].cells, vec![7; 4]);
        assert!(Path::from_name("qt_02.vcqtc").is_err());
        assert!(build([0., 0., 1., 1.], &[(7, a), (7, b)]).is_err());
    }
}
