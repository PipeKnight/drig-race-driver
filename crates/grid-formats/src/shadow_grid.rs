//! Recorded static track shading from the route's CLM hierarchy.
use crate::{Result, fail, range};
use std::collections::BTreeSet;

const HEADER: usize = 24;
const RADIX: u32 = 10;
const NODE_SIZE: usize = 104;

pub struct ShadowGrid<'a> {
    data: &'a [u8],
    origin: [f32; 2],
    inverse_cell: [f32; 2],
    size: u32,
    padded: u32,
}

pub fn read(bytes: &[u8]) -> Result<ShadowGrid<'_>> {
    range(0, HEADER + NODE_SIZE, bytes.len())?;
    let origin = [scalar(bytes, 0), scalar(bytes, 4)];
    let inverse_cell = [scalar(bytes, 8), scalar(bytes, 12)];
    let size = word(bytes, 16);
    if word(bytes, 20) != RADIX
        || size <= 1
        || origin.iter().any(|v| !v.is_finite())
        || inverse_cell.iter().any(|v| !v.is_finite() || *v <= 0.)
        || !(bytes.len() - HEADER).is_multiple_of(NODE_SIZE)
    {
        return fail(0, "invalid static shadow grid");
    }
    let mut padded = RADIX;
    while padded < size {
        let Some(next) = padded.checked_mul(RADIX) else {
            return fail(16, "shadow grid dimensions overflow");
        };
        padded = next;
    }
    validate(bytes, HEADER, padded, &mut BTreeSet::new())?;
    Ok(ShadowGrid {
        data: bytes,
        origin,
        inverse_cell,
        size,
        padded,
    })
}

impl ShadowGrid<'_> {
    /// Positive source samples describe static shadow. Outside the grid is lit.
    pub fn shaded(&self, x: f32, z: f32) -> bool {
        let coordinates = [x, z];
        let scaled = std::array::from_fn::<_, 2, _>(|i| {
            ((coordinates[i] - self.origin[i]) * self.inverse_cell[i]).trunc()
        });
        if scaled
            .iter()
            .any(|&v| !(0. ..self.size as f32).contains(&v))
        {
            return false;
        }
        let [mut col, mut row] = scaled.map(|v| v as u32);
        let mut at = HEADER;
        let mut scale = self.padded;
        loop {
            scale /= RADIX;
            let index = (row / scale) * RADIX + col / scale;
            let code = self.data[at + 4 + index as usize] as i8;
            if code >= 0 {
                return code > 0;
            }
            at = child_offset(self.data, at, code);
            col %= scale;
            row %= scale;
        }
    }
}

fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

fn scalar(bytes: &[u8], at: usize) -> f32 {
    f32::from_bits(word(bytes, at))
}

fn child_offset(bytes: &[u8], node: usize, token: i8) -> usize {
    // Negative table entries index a contiguous child span relative to the file.
    word(bytes, node) as usize + (token.unsigned_abs() as usize - 1) * NODE_SIZE
}

fn validate(bytes: &[u8], at: usize, scale: u32, seen: &mut BTreeSet<(usize, u32)>) -> Result<()> {
    if at < HEADER || !(at - HEADER).is_multiple_of(NODE_SIZE) {
        return fail(at, "shadow node alignment");
    }
    let node = range(at, NODE_SIZE, bytes.len())?;
    if !seen.insert((at, scale)) {
        return Ok(());
    }
    for &value in &bytes[node.start + 4..node.end] {
        let code = value as i8;
        if code >= 0 {
            continue;
        }
        if scale == RADIX {
            return fail(at, "shadow child at terminal depth");
        }
        let base = word(bytes, at) as usize;
        let index = (code.unsigned_abs() as usize - 1) * NODE_SIZE;
        let child = range(base, index, bytes.len())?.end;
        validate(bytes, child, scale / RADIX, seen)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walks_signed_child_addresses_and_rejects_short_buffers() {
        let mut bytes = vec![0; HEADER + 2 * NODE_SIZE];
        for offset in [8, 12] {
            bytes[offset..offset + 4].copy_from_slice(&1f32.to_le_bytes());
        }
        bytes[16..20].copy_from_slice(&20u32.to_le_bytes());
        bytes[20..24].copy_from_slice(&RADIX.to_le_bytes());
        let child = HEADER + NODE_SIZE;
        bytes[HEADER..HEADER + 4].copy_from_slice(&(child as u32).to_le_bytes());
        bytes[HEADER + 4] = 255;
        bytes[child + 4 + 34] = 1;
        let grid = read(&bytes).unwrap();
        assert!(grid.shaded(4.2, 3.2));
        assert!(!grid.shaded(3.2, 4.2));
        assert!(!grid.shaded(20., 3.2));
        assert!(read(&bytes[..bytes.len() - 1]).is_err());
        bytes[child + 4] = 255;
        assert!(read(&bytes).is_err());
    }
}
