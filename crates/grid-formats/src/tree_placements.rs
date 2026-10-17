//! Observed route trees.bin: compact affine placements, not ornaments.bin.
//! The instance word and prototype word remain uninterpreted.
use crate::{Result, fail, range};

#[derive(Debug)]
pub struct Instance {
    pub transform: [f32; 16],
    pub metadata: u32,
}
#[derive(Debug)]
pub struct Prototype<'a> {
    pub name: &'a [u8],
    pub bounds: [f32; 6],
    pub first_instance: u32,
    pub instances: Vec<Instance>,
    pub opaque: u32,
}
#[derive(Debug)]
pub struct Trees<'a> {
    pub bounds: [f32; 6],
    pub total_instances: u32,
    pub prototypes: Vec<Prototype<'a>>,
}
fn word(bytes: &[u8], offset: usize) -> Result<u32> {
    let data = range(offset, 4, bytes.len())?;
    Ok(u32::from_le_bytes(bytes[data].try_into().unwrap()))
}
fn floats<const N: usize>(bytes: &[u8], offset: usize) -> Result<[f32; N]> {
    range(offset, N * 4, bytes.len())?;
    let values = std::array::from_fn(|i| f32::from_bits(word(bytes, offset + i * 4).unwrap()));
    if values.iter().any(|v| !v.is_finite()) {
        return fail(offset, "nonfinite tree placement value");
    }
    Ok(values)
}
fn bounds(bytes: &[u8], offset: usize) -> Result<[f32; 6]> {
    let b = floats::<6>(bytes, offset)?;
    if (0..3).any(|i| b[i] > b[i + 3]) {
        return fail(offset, "inverted tree bounds");
    }
    Ok(b)
}
pub fn read(bytes: &[u8]) -> Result<Trees<'_>> {
    let bounds = bounds(bytes, 0)?;
    let total_instances = word(bytes, 24)?;
    let count = word(bytes, 32)? as usize;
    if word(bytes, 28)? != 36 || count == 0 || count > 100_000 || total_instances > 1_000_000 {
        return fail(24, "unsupported tree placement header/counts");
    }
    let headers = range(36, count * 48, bytes.len())?;
    let string_start = (0..count)
        .map(|i| word(bytes, 36 + i * 48).map(|v| v as usize))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .min()
        .unwrap();
    if string_start < headers.end || string_start >= bytes.len() {
        return fail(36, "tree name table outside file");
    }
    let mut names = std::collections::BTreeSet::new();
    let (mut next_instance, mut next_data) = (0u32, headers.end);
    let mut prototypes = Vec::new();
    for i in 0..count {
        let offset = 36 + i * 48;
        let name_at = word(bytes, offset)? as usize;
        if name_at < string_start || name_at >= bytes.len() {
            return fail(offset, "tree name offset outside table");
        }
        let len = bytes[name_at..]
            .iter()
            .take(4097)
            .position(|&b| b == 0)
            .ok_or(crate::Error {
                offset: name_at,
                message: "tree name lacks bounded terminator",
            })?;
        let name = &bytes[name_at..name_at + len];
        if name.is_empty() || !names.insert(name) {
            return fail(name_at, "tree name empty or duplicated");
        }
        let local_bounds = self::bounds(bytes, offset + 4)?;
        let total = word(bytes, offset + 28)?;
        let first = word(bytes, offset + 32)?;
        let data_at = word(bytes, offset + 36)? as usize;
        let stored = word(bytes, offset + 40)? as usize;
        if first != next_instance || stored != total as usize || data_at != next_data {
            return fail(
                offset,
                "tree instance ranges not contiguous or stored count differs",
            );
        }
        next_instance = next_instance.checked_add(total).ok_or(crate::Error {
            offset,
            message: "tree instance total overflow",
        })?;
        let data = range(
            data_at,
            stored.checked_mul(52).ok_or(crate::Error {
                offset,
                message: "tree placement size overflow",
            })?,
            string_start,
        )?;
        next_data = data.end;
        let mut instances = Vec::new();
        for j in 0..stored {
            let v = floats::<12>(bytes, data.start + j * 52)?;
            instances.push(Instance {
                transform: [
                    v[0], v[1], v[2], 0., v[3], v[4], v[5], 0., v[6], v[7], v[8], 0., v[9], v[10],
                    v[11], 1.,
                ],
                metadata: word(bytes, data.start + j * 52 + 48)?,
            });
        }
        prototypes.push(Prototype {
            name,
            bounds: local_bounds,
            first_instance: first,
            instances,
            opaque: word(bytes, offset + 44)?,
        });
    }
    if next_instance != total_instances || next_data != string_start {
        return fail(24, "tree totals differ from table boundaries");
    }
    Ok(Trees {
        bounds,
        total_instances,
        prototypes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn put(bytes: &mut [u8], at: usize, value: u32) {
        bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    fn fixture(names: &[&[u8]]) -> Vec<u8> {
        let data = 36 + names.len() * 48;
        let strings = data + names.len() * 52;
        let mut b = vec![0; strings + names.iter().map(|n| n.len() + 1).sum::<usize>()];
        for (i, v) in [-5_f32, -5., -5., 5., 5., 5.].into_iter().enumerate() {
            put(&mut b, i * 4, v.to_bits());
        }
        put(&mut b, 24, names.len() as u32);
        put(&mut b, 28, 36);
        put(&mut b, 32, names.len() as u32);
        let mut at = strings;
        for (i, name) in names.iter().enumerate() {
            let p = 36 + i * 48;
            put(&mut b, p, at as u32);
            for (k, v) in [-1_f32, 0., -1., 1., 5., 1.].into_iter().enumerate() {
                put(&mut b, p + 4 + k * 4, v.to_bits());
            }
            put(&mut b, p + 28, 1);
            put(&mut b, p + 32, i as u32);
            put(&mut b, p + 36, (data + i * 52) as u32);
            put(&mut b, p + 40, 1);
            put(&mut b, p + 44, 0xaabbccdd);
            for (k, v) in [1_f32, 0., 0., 0., 1., 0., 0., 0., 1., 2., 3., 4.]
                .into_iter()
                .enumerate()
            {
                put(&mut b, data + i * 52 + k * 4, v.to_bits());
            }
            put(&mut b, data + i * 52 + 48, 0x10203040);
            b[at..at + name.len()].copy_from_slice(name);
            at += name.len() + 1;
        }
        b
    }
    #[test]
    fn affine_positions_and_uninterpreted_words_survive() {
        let b = fixture(&[b"oak", b"pine"]);
        let trees = read(&b).unwrap();
        assert_eq!(trees.total_instances, 2);
        let p = &trees.prototypes[1];
        assert_eq!(p.first_instance, 1);
        assert_eq!(p.opaque, 0xaabbccdd);
        assert_eq!(p.instances[0].metadata, 0x10203040);
        assert_eq!(&p.instances[0].transform[12..], &[2., 3., 4., 1.]);
    }
    #[test]
    fn rejects_bad_envelopes_counts_ranges_names_and_nonfinite_transforms() {
        let original = fixture(&[b"oak"]);
        for (at, value) in [
            (24, 2),
            (28, 40),
            (32, u32::MAX),
            (36, u32::MAX),
            (68, 1),
            (72, 85),
            (76, 2),
            (84, f32::NAN.to_bits()),
            (0, f32::INFINITY.to_bits()),
        ] {
            let mut b = original.clone();
            put(&mut b, at, value);
            assert!(read(&b).is_err(), "accepted offset{at}");
        }
        let mut b = original.clone();
        b.pop();
        assert!(read(&b).is_err());
        assert!(read(&fixture(&[b"oak", b"oak"])).is_err());
        assert!(read(&fixture(&[b""])).is_err());
        assert!(read(&original[..83]).is_err());
    }
    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn purchased_donington_placements_are_complete() {
        let root = std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required");
        let b = std::fs::read(
            std::path::Path::new(&root).join("tracks/europe/donington/route_0/trees.bin"),
        )
        .unwrap();
        let trees = read(&b).unwrap();
        assert_eq!(trees.prototypes.len(), 9);
        assert_eq!(trees.total_instances, 646);
        assert_eq!(
            trees
                .prototypes
                .iter()
                .map(|p| p.instances.len())
                .sum::<usize>(),
            646
        );
    }
}
