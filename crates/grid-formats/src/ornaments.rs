//! Observed GRID static ornament table. Other words remain opaque.
use crate::{Result, fail, range};
#[derive(Debug)]
pub struct Prototype<'a> {
    pub name: &'a [u8],
    pub bounds: [f32; 6],
    pub total_instances: u32,
    pub first_instance: u32,
    pub transforms: Vec<[f32; 16]>,
    pub transform_metadata: Vec<[u32; 2]>,
    pub opaque_words: [u32; 2],
}
#[derive(Debug)]
pub struct Ornaments<'a> {
    pub bounds: [f32; 6],
    pub total_instances: u32,
    pub prototypes: Vec<Prototype<'a>>,
}
fn word(bytes: &[u8], offset: usize) -> Result<u32> {
    range(offset, 4, bytes.len())?;
    Ok(u32::from_le_bytes(
        bytes[offset..offset + 4].try_into().unwrap(),
    ))
}
fn floats<const N: usize>(bytes: &[u8], offset: usize) -> Result<[f32; N]> {
    range(offset, N * 4, bytes.len())?;
    let result = std::array::from_fn(|i| {
        f32::from_le_bytes(
            bytes[offset + i * 4..offset + i * 4 + 4]
                .try_into()
                .unwrap(),
        )
    });
    if result.iter().any(|v| !v.is_finite()) {
        return fail(offset, "nonfinite ornament value");
    }
    Ok(result)
}
pub fn read(bytes: &[u8]) -> Result<Ornaments<'_>> {
    if word(bytes, 0)? != 0 || word(bytes, 32)? != 40 {
        return fail(0, "unsupported ornament header");
    }
    let bounds = floats(bytes, 4)?;
    let total_instances = word(bytes, 28)?;
    let count = word(bytes, 36)? as usize;
    if count == 0 || count > 100_000 || total_instances > 1_000_000 {
        return fail(28, "ornament count limit exceeded");
    }
    let headers = range(
        40,
        count.checked_mul(52).ok_or(crate::Error {
            offset: 36,
            message: "ornament header overflow",
        })?,
        bytes.len(),
    )?;
    let mut prototypes = Vec::new();
    let mut next_instance = 0;
    let mut next_transform = headers.end;
    let string_start = (0..count)
        .map(|i| word(bytes, 40 + i * 52).map(|v| v as usize))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .min()
        .unwrap();
    if string_start < headers.end || string_start >= bytes.len() {
        return fail(40, "ornament name table outside file");
    }
    let mut names = std::collections::BTreeSet::new();
    for i in 0..count {
        let offset = 40 + i * 52;
        let name_offset = word(bytes, offset)? as usize;
        if name_offset < string_start || name_offset >= bytes.len() {
            return fail(offset, "ornament name offset invalid");
        }
        let end = bytes[name_offset..]
            .iter()
            .take(4097)
            .position(|&v| v == 0)
            .ok_or(crate::Error {
                offset: name_offset,
                message: "ornament name lacks bounded terminator",
            })?;
        let name = &bytes[name_offset..name_offset + end];
        if name.is_empty() || !names.insert(name) {
            return fail(name_offset, "ornament name empty or duplicated");
        }
        let bounds = floats(bytes, offset + 4)?;
        let total = word(bytes, offset + 28)?;
        let first = word(bytes, offset + 32)?;
        let data_offset = word(bytes, offset + 36)? as usize;
        let stored = word(bytes, offset + 40)? as usize;
        if first != next_instance || stored > total as usize || data_offset != next_transform {
            return fail(offset, "ornament ranges not contiguous or exceed total");
        }
        next_instance = next_instance.checked_add(total).ok_or(crate::Error {
            offset,
            message: "ornament instance count overflow",
        })?;
        let data = range(
            data_offset,
            stored.checked_mul(56).ok_or(crate::Error {
                offset,
                message: "ornament transform count overflow",
            })?,
            string_start,
        )?;
        next_transform = data.end;
        let mut transforms = Vec::new();
        let mut transform_metadata = Vec::new();
        for j in 0..stored {
            let values = floats::<12>(bytes, data.start + j * 56)?;
            transform_metadata.push([
                word(bytes, data.start + j * 56 + 48)?,
                word(bytes, data.start + j * 56 + 52)?,
            ]);
            transforms.push([
                values[0], values[1], values[2], 0., values[3], values[4], values[5], 0.,
                values[6], values[7], values[8], 0., values[9], values[10], values[11], 1.,
            ]);
        }
        prototypes.push(Prototype {
            name,
            bounds,
            total_instances: total,
            first_instance: first,
            transforms,
            transform_metadata,
            opaque_words: [word(bytes, offset + 44)?, word(bytes, offset + 48)?],
        });
    }
    if next_instance != total_instances || next_transform != string_start {
        return fail(28, "ornament totals do not match table boundaries");
    }
    Ok(Ornaments {
        bounds,
        total_instances,
        prototypes,
    })
}
