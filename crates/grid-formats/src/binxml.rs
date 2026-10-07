//! GRID sectioned binary XML. Values remain strings; game semantics are separate.
use crate::{Cursor, Result, fail, range};
use std::{collections::BTreeSet, ops::Range};
#[derive(Debug)]
pub struct Element<'a> {
    pub name: &'a str,
    pub value: Option<&'a str>,
    pub attributes: Vec<(&'a str, &'a str)>,
    pub children: Range<usize>,
    pub parent: Option<usize>,
}
#[derive(Debug)]
pub struct Document<'a> {
    pub strings: Vec<&'a str>,
    pub elements: Vec<Element<'a>>,
    pub string_padding: &'a [u8],
}

fn section<'a>(c: &mut Cursor<'a>, tag: u32) -> Result<&'a [u8]> {
    let offset = c.pos;
    if c.le()? != tag {
        return fail(offset, "unexpected binary XML section tag");
    }
    let size = c.le()? as usize;
    c.take(size)
}
fn word(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}
pub fn read(bytes: &[u8]) -> Result<Document<'_>> {
    let mut c = Cursor { bytes, pos: 0 };
    if c.le()? != 0x7252221a {
        return fail(0, "expected GRID binary XML magic");
    }
    if c.le()? as usize != bytes.len() - 8 {
        return fail(4, "binary XML size mismatch");
    }
    if c.le()? != 0x72522217 {
        return fail(8, "expected binary XML string-group tag");
    }
    let group_size = c.le()? as usize;
    let blob = section(&mut c, 0x7252221d)?;
    let offsets = section(&mut c, 0x7252221e)?;
    if !offsets.len().is_multiple_of(4)
        || group_size != blob.len() + offsets.len() + 16
        || offsets.len() / 4 > 1_000_000
    {
        return fail(12, "binary XML string sections disagree");
    }
    let mut strings = Vec::new();
    let mut next = 0;
    for offset in offsets.as_chunks::<4>().0 {
        let start = u32::from_le_bytes(*offset) as usize;
        if start != next || start >= blob.len() {
            return fail(24, "binary XML string offsets not contiguous");
        }
        let len = blob[start..]
            .iter()
            .position(|&v| v == 0)
            .ok_or(crate::Error {
                offset: 24 + start,
                message: "unterminated binary XML string",
            })?;
        let text = std::str::from_utf8(&blob[start..start + len]).map_err(|_| crate::Error {
            offset: 24 + start,
            message: "invalid binary XML UTF-8",
        })?;
        strings.push(text);
        next = start + len + 1;
    }
    let padding = &blob[next..];
    if strings.is_empty() || padding.len() > 15 || padding.iter().any(|&v| v != 0) {
        return fail(24 + next, "unexpected binary XML string padding");
    }
    let element_bytes = section(&mut c, 0x7252221b)?;
    let attribute_bytes = section(&mut c, 0x7252221c)?;
    if c.pos != bytes.len()
        || !element_bytes.len().is_multiple_of(24)
        || !attribute_bytes.len().is_multiple_of(8)
    {
        return fail(c.pos, "binary XML records or end mismatch");
    }
    let count = element_bytes.len() / 24;
    let attribute_count = attribute_bytes.len() / 8;
    if count == 0 || count > 250_000 || attribute_count > 1_000_000 {
        return fail(0, "binary XML count limit exceeded");
    }
    let resolve = |id: u32| {
        strings.get(id as usize).copied().ok_or(crate::Error {
            offset: 0,
            message: "binary XML string ID outside table",
        })
    };
    let mut all_attributes = Vec::with_capacity(attribute_count);
    for a in attribute_bytes.as_chunks::<8>().0 {
        all_attributes.push((resolve(word(a, 0))?, resolve(word(a, 4))?));
    }
    let mut used_attributes = vec![false; attribute_count];
    let mut parents = vec![None; count];
    let mut elements = Vec::with_capacity(count);
    for (i, e) in element_bytes.as_chunks::<24>().0.iter().enumerate() {
        let name = resolve(word(e, 0))?;
        if name.is_empty() {
            return fail(i * 24, "empty binary XML element name");
        }
        let value = if word(e, 4) == 0 {
            None
        } else {
            Some(resolve(word(e, 4))?)
        };
        let attrs = range(word(e, 12) as usize, word(e, 8) as usize, attribute_count)?;
        let children = range(word(e, 20) as usize, word(e, 16) as usize, count)?;
        let mut names = BTreeSet::new();
        for a in attrs.clone() {
            if used_attributes[a]
                || all_attributes[a].0.is_empty()
                || !names.insert(all_attributes[a].0)
            {
                return fail(a * 8, "shared or invalid binary XML attribute");
            }
            used_attributes[a] = true;
        }
        for child in children.clone() {
            if child == 0 || child == i || parents[child].replace(i).is_some() {
                return fail(child * 24, "shared or cyclic binary XML child");
            }
        }
        elements.push(Element {
            name,
            value,
            attributes: all_attributes[attrs].to_vec(),
            children,
            parent: None,
        });
    }
    if parents.iter().skip(1).any(Option::is_none) || used_attributes.iter().any(|v| !v) {
        return fail(0, "unowned binary XML element/attribute");
    }
    let mut pending = vec![(0, 0usize)];
    let mut reached = vec![false; count];
    while let Some((i, depth)) = pending.pop() {
        if depth > 128 || reached[i] {
            return fail(i * 24, "binary XML depth/cycle limit");
        }
        reached[i] = true;
        elements[i].parent = parents[i];
        pending.extend(elements[i].children.clone().map(|child| (child, depth + 1)));
    }
    if reached.iter().any(|v| !v) {
        return fail(0, "disconnected binary XML cycle");
    }
    Ok(Document {
        strings,
        elements,
        string_padding: padding,
    })
}
