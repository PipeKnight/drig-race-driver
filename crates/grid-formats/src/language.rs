//! Language tables (`language/language_*.lng`): string id -> UI text.
//!
//! Big-endian chunks after the `LNGT` header (its size word is the whole file
//! length): `HSHS` and `HSHT` (the id hash table, not needed for lookups by
//! name), then `SIDA` = count followed by `count` pairs (id offset into the
//! `SIDB` blob, text offset into the `LNGB` blob), then `SIDB` and `LNGB`
//! blobs of NUL-terminated strings. The SIDA size word covers the pairs only.
use crate::{Cursor, Result, fail};
use std::collections::HashMap;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Language {
    texts: HashMap<String, String>,
}

impl Language {
    /// UI text of string id `id`.
    pub fn get(&self, id: &str) -> Option<&str> {
        self.texts.get(id).map(String::as_str)
    }

    pub fn len(&self) -> usize {
        self.texts.len()
    }

    /// Every string id, in no particular order.
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.texts.keys().map(String::as_str)
    }

    pub fn is_empty(&self) -> bool {
        self.texts.is_empty()
    }
}

fn chunk<'a>(c: &mut Cursor<'a>, tag: &[u8; 4]) -> Result<&'a [u8]> {
    let offset = c.pos;
    if c.take(4)? != tag {
        return fail(offset, "unexpected language chunk tag");
    }
    let size = c.be()? as usize;
    c.take(size)
}

fn text(blob: &[u8], offset: usize, at: usize) -> Result<String> {
    let rest = blob.get(offset..).ok_or(crate::Error {
        offset: at,
        message: "language string offset outside its blob",
    })?;
    let len = rest.iter().position(|&b| b == 0).ok_or(crate::Error {
        offset: at,
        message: "unterminated language string",
    })?;
    // The shipped tables are UTF-8; a stray byte is replaced, not refused.
    Ok(String::from_utf8_lossy(&rest[..len]).into_owned())
}

pub fn read(bytes: &[u8]) -> Result<Language> {
    let mut c = Cursor { bytes, pos: 0 };
    if c.take(4)? != b"LNGT" {
        return fail(0, "expected LNGT language header");
    }
    if c.be()? as usize != bytes.len() {
        return fail(4, "language size mismatch");
    }
    chunk(&mut c, b"HSHS")?;
    chunk(&mut c, b"HSHT")?;
    let pairs_at = c.pos;
    if c.take(4)? != b"SIDA" {
        return fail(pairs_at, "expected SIDA string index");
    }
    let size = c.be()? as usize;
    let count = c.be()? as usize;
    if count.checked_mul(8) != Some(size) || count > 1_000_000 {
        return fail(pairs_at, "SIDA count and size disagree");
    }
    let pairs = c.take(size)?;
    let ids = chunk(&mut c, b"SIDB")?;
    let texts_blob = chunk(&mut c, b"LNGB")?;
    if c.pos != bytes.len() {
        return fail(c.pos, "trailing bytes after LNGB");
    }
    let mut texts = HashMap::with_capacity(count);
    for (i, pair) in pairs.as_chunks::<8>().0.iter().enumerate() {
        let at = pairs_at + 12 + i * 8;
        let id = u32::from_be_bytes(pair[..4].try_into().unwrap()) as usize;
        let value = u32::from_be_bytes(pair[4..].try_into().unwrap()) as usize;
        // Duplicate ids exist in the shipped tables; the first entry wins.
        texts
            .entry(text(ids, id, at)?)
            .or_insert(text(texts_blob, value, at + 4)?);
    }
    Ok(Language { texts })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(entries: &[(&str, &str)]) -> Vec<u8> {
        let (mut ids, mut texts, mut pairs) = (Vec::new(), Vec::new(), Vec::new());
        for (id, value) in entries {
            pairs.extend((ids.len() as u32).to_be_bytes());
            pairs.extend((texts.len() as u32).to_be_bytes());
            ids.extend(id.bytes().chain([0]));
            texts.extend(value.bytes().chain([0]));
        }
        let mut body = Vec::new();
        for (tag, data) in [(b"HSHS", &[0u8; 4][..]), (b"HSHT", &[][..])] {
            body.extend(tag);
            body.extend((data.len() as u32).to_be_bytes());
            body.extend(data);
        }
        body.extend(b"SIDA");
        body.extend((pairs.len() as u32).to_be_bytes());
        body.extend((entries.len() as u32).to_be_bytes());
        body.extend(&pairs);
        for (tag, data) in [(b"SIDB", &ids), (b"LNGB", &texts)] {
            body.extend(tag);
            body.extend((data.len() as u32).to_be_bytes());
            body.extend(data);
        }
        let mut file = b"LNGT".to_vec();
        file.extend(((body.len() + 8) as u32).to_be_bytes());
        file.extend(body);
        file
    }

    #[test]
    fn reads_ids_and_texts_first_entry_wins() {
        let bytes = table(&[("a", "Alpha"), ("b", "Beta"), ("a", "Again")]);
        let language = read(&bytes).unwrap();
        assert_eq!(language.len(), 2);
        assert_eq!(language.get("a"), Some("Alpha"));
        assert_eq!(language.get("b"), Some("Beta"));
        assert_eq!(language.get("c"), None);
    }

    #[test]
    fn refuses_bad_framing() {
        let mut bytes = table(&[("a", "Alpha")]);
        bytes.push(0);
        assert!(read(&bytes).is_err());
        let mut bytes = table(&[("a", "Alpha")]);
        bytes[0] = b'X';
        assert!(read(&bytes).is_err());
        assert!(read(b"LNGT").is_err());
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT purchased source"]
    fn reads_the_shipped_english_table() {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let path = std::path::Path::new(&root).join("language/language_eng.lng");
        let language = read(&std::fs::read(path).unwrap()).unwrap();
        // 4,907 pairs, a few ids repeated.
        assert!(language.len() > 4_800, "{}", language.len());
        assert_eq!(
            language.get("pause_mainmenu_screen_title"),
            Some("Pause Menu")
        );
        assert_eq!(language.get("overlay_popup_generic_accept"), Some("Yes"));
    }
}
