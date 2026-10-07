use crate::{Cursor, Result, fail, range};
use std::ops::Range;

#[derive(Debug)]
pub struct Entry<'a> {
    pub name: &'a [u8],
    pub uncompressed_size: u32,
    pub stored_data: Range<usize>,
    pub unknown: [u32; 4],
}
#[derive(Debug)]
pub struct Jpk<'a> {
    pub alignment: u32,
    pub unknown_header: [u32; 5],
    pub entries: Vec<Entry<'a>>,
}

/// Inspect the observed GRID JPAK table; does not decompress or extract entries.
pub fn inspect(bytes: &[u8]) -> Result<Jpk<'_>> {
    let mut c = Cursor { bytes, pos: 0 };
    if c.take(4)? != b"JPAK" {
        return fail(0, "expected JPAK magic");
    }
    let version = c.le()?;
    if version != 0 {
        return fail(4, "unsupported JPAK header variant");
    }
    let count = c.le()? as usize;
    let alignment = c.le()?;
    let unknown_header = [version, c.le()?, c.le()?, c.le()?, c.le()?];
    if count > 100_000 || count > (bytes.len() - c.pos) / 32 {
        return fail(8, "entry table cannot fit file or exceeds inspector limit");
    }
    let table_end = c.pos + count * 32;
    let mut entries = Vec::new();
    for _ in 0..count {
        let name_offset = c.le()? as usize;
        let uncompressed_size = c.le()?;
        let data_offset = c.le()? as usize;
        let stored_size = c.le()? as usize;
        let unknown = [c.le()?, c.le()?, c.le()?, c.le()?];
        if name_offset < table_end {
            return fail(name_offset, "entry name overlaps header or table");
        }
        let name_tail = bytes.get(name_offset..).ok_or(crate::Error {
            offset: name_offset,
            message: "entry name offset exceeds file",
        })?;
        // Bound the search too: avoid scanning the full file for every entry.
        let length = name_tail
            .iter()
            .take(4097)
            .position(|&b| b == 0)
            .ok_or(crate::Error {
                offset: name_offset,
                message: "entry name exceeds 4096 bytes or is not NUL terminated",
            })?;
        if data_offset < table_end && stored_size != 0 {
            return fail(data_offset, "entry payload overlaps header or table");
        }
        entries.push(Entry {
            name: &name_tail[..length],
            uncompressed_size,
            stored_data: range(data_offset, stored_size, bytes.len())?,
            unknown,
        });
    }
    Ok(Jpk {
        alignment,
        unknown_header,
        entries,
    })
}
