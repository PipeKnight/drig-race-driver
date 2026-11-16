//! Bounded owned text/binary XML view for shared Document semantic readers.
//! Uses quick_xml once for text and delegates binary bytes to existing binxml::read.
use crate::{
    Error, Result,
    binxml::{self, Document, Element},
    fail,
};
use quick_xml::{
    Reader,
    events::{BytesStart, Event},
};
use std::{
    collections::{BTreeSet, VecDeque},
    ops::Range,
};
const LIMIT: usize = 1024 * 1024;
const NODES: usize = 250_000;
#[derive(Debug)]
struct OwnedElement {
    name: String,
    value: Option<String>,
    attributes: Vec<(String, String)>,
    children: Range<usize>,
    parent: Option<usize>,
}
#[derive(Debug)]
pub struct OwnedDocument {
    elements: Vec<OwnedElement>,
    strings: Vec<String>,
    padding: Vec<u8>,
}
impl OwnedDocument {
    pub fn document(&self) -> Document<'_> {
        Document {
            strings: self.strings.iter().map(String::as_str).collect(),
            string_padding: &self.padding,
            elements: self
                .elements
                .iter()
                .map(|e| Element {
                    name: &e.name,
                    value: e.value.as_deref(),
                    attributes: e
                        .attributes
                        .iter()
                        .map(|(n, v)| (n.as_str(), v.as_str()))
                        .collect(),
                    children: e.children.clone(),
                    parent: e.parent,
                })
                .collect(),
        }
    }
    fn own(document: &Document<'_>) -> Self {
        Self {
            strings: document.strings.iter().map(|s| (*s).to_owned()).collect(),
            padding: document.string_padding.to_vec(),
            elements: document
                .elements
                .iter()
                .map(|e| OwnedElement {
                    name: e.name.to_owned(),
                    value: e.value.map(str::to_owned),
                    attributes: e
                        .attributes
                        .iter()
                        .map(|(n, v)| ((*n).to_owned(), (*v).to_owned()))
                        .collect(),
                    children: e.children.clone(),
                    parent: e.parent,
                })
                .collect(),
        }
    }
}
#[derive(Debug)]
struct Pending {
    name: String,
    value: Option<String>,
    attributes: Vec<(String, String)>,
    children: Vec<usize>,
}
fn error(offset: usize, message: &'static str) -> Error {
    Error { offset, message }
}
fn push(
    node: &BytesStart<'_>,
    offset: usize,
    stack: &[usize],
    nodes: &mut Vec<Pending>,
    root: &mut Option<usize>,
    attributes: &mut usize,
) -> Result<usize> {
    if nodes.len() >= NODES || stack.len() > 128 {
        return fail(offset, "XML node/depth bound");
    }
    let name = std::str::from_utf8(node.name().as_ref())
        .map_err(|_| error(offset, "non-UTF8 XML name"))?
        .to_owned();
    if name.is_empty() {
        return fail(offset, "empty XML name");
    }
    let mut attrs = Vec::new();
    let mut names = BTreeSet::new();
    for a in node.attributes() {
        let a = a.map_err(|_| error(offset, "invalid/duplicate XML attribute"))?;
        *attributes += 1;
        if *attributes > 1_000_000 {
            return fail(offset, "XML attribute bound");
        }
        let key = std::str::from_utf8(a.key.as_ref())
            .map_err(|_| error(offset, "non-UTF8 XML attribute"))?
            .to_owned();
        if key.is_empty() || !names.insert(key.clone()) {
            return fail(offset, "duplicate/empty XML attribute");
        }
        let value = a
            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
            .map_err(|_| error(offset, "invalid XML attribute value"))?
            .into_owned();
        attrs.push((key, value));
    }
    let index = nodes.len();
    nodes.push(Pending {
        name,
        value: None,
        attributes: attrs,
        children: Vec::new(),
    });
    if let Some(&parent) = stack.last() {
        nodes[parent].children.push(index)
    } else if root.replace(index).is_some() {
        return fail(offset, "multiple XML roots");
    }
    Ok(index)
}
/// Strict supported text subset preserves original grade's text/attribute policies:
/// no DTD/CDATA/text references, no entity expansion engine; comments/PI are ignored.
pub fn read(bytes: &[u8]) -> Result<OwnedDocument> {
    if bytes.len() > LIMIT {
        return fail(0, "XML source exceeds bound");
    }
    if bytes.starts_with(&0x7252221au32.to_le_bytes()) {
        return binxml::read(bytes).map(|d| OwnedDocument::own(&d));
    }
    let mut reader = Reader::from_reader(bytes);
    let mut nodes = Vec::new();
    let mut stack = Vec::new();
    let mut root = None;
    let mut attributes = 0usize;
    loop {
        let offset = reader.buffer_position() as usize;
        let event = reader
            .read_event()
            .map_err(|_| error(offset, "invalid text XML"))?;
        match event {
            Event::Start(node) => {
                let i = push(
                    &node,
                    offset,
                    &stack,
                    &mut nodes,
                    &mut root,
                    &mut attributes,
                )?;
                stack.push(i);
            }
            Event::Empty(node) => {
                push(
                    &node,
                    offset,
                    &stack,
                    &mut nodes,
                    &mut root,
                    &mut attributes,
                )?;
            }
            Event::End(node) => {
                let i = stack
                    .pop()
                    .ok_or(error(offset, "unmatched XML closing tag"))?;
                if node.name().as_ref() != nodes[i].name.as_bytes() {
                    return fail(offset, "mismatched XML closing tag");
                }
            }
            Event::Text(text) => {
                if let Some(&i) = stack.last() {
                    let value = std::str::from_utf8(text.as_ref())
                        .map_err(|_| error(offset, "non-UTF8 XML text"))?;
                    nodes[i]
                        .value
                        .get_or_insert_with(String::new)
                        .push_str(value);
                } else if !text.as_ref().iter().all(u8::is_ascii_whitespace) {
                    return fail(offset, "XML text outside root");
                }
            }
            Event::Decl(_) | Event::Comment(_) | Event::PI(_) => {}
            Event::DocType(_) | Event::GeneralRef(_) | Event::CData(_) => {
                return fail(offset, "unsupported XML content");
            }
            Event::Eof => break,
        }
    }
    if !stack.is_empty() {
        return fail(bytes.len(), "unclosed XML element");
    }
    let root = root.ok_or(error(0, "XML root absent"))?;
    // Match Document's CONTIGUOUS DIRECT-child ranges; event order was depth-first.
    let mut pending: Vec<_> = nodes.into_iter().map(Some).collect();
    let mut queue = VecDeque::from([(root, None)]);
    let mut elements = Vec::new();
    while let Some((old, parent)) = queue.pop_front() {
        let node = pending[old].take().ok_or(error(old, "XML node reused"))?;
        let index = elements.len();
        // Existing queued siblings precede the children we append now.
        let first = index + 1 + queue.len();
        let count = node.children.len();
        for child in node.children {
            queue.push_back((child, Some(index)));
        }
        elements.push(OwnedElement {
            name: node.name,
            value: node.value,
            attributes: node.attributes,
            parent,
            children: first..first + count,
        });
    }
    Ok(OwnedDocument {
        elements,
        strings: Vec::new(),
        padding: Vec::new(),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn direct_ranges_values_and_attribute_normalization_survive_owned_bridge() {
        let owned =
            read(br#"<root flag="a&amp;b"><a><inside/></a><b>1<!--join-->.25</b></root>"#).unwrap();
        let d = owned.document();
        assert_eq!(
            d.elements.iter().map(|e| e.name).collect::<Vec<_>>(),
            ["root", "a", "b", "inside"]
        );
        assert_eq!(d.elements[0].children, 1..3);
        assert_eq!(d.elements[1].children, 3..4);
        assert_eq!(d.elements[2].value, Some("1.25"));
        assert_eq!(d.elements[3].parent, Some(1));
        assert_eq!(d.elements[0].attributes, [("flag", "a&b")]);
    }
    #[test]
    fn unsupported_or_unbalanced_inputs_refuse_without_partial_document() {
        for xml in [
            "<a/><b/>",
            "<a></b>",
            "<a>",
            "outside<a/>",
            "<a k='1' k='2'/>",
            "<!DOCTYPE a><a/>",
            "<a><![CDATA[x]]></a>",
            "<a>&amp;</a>",
        ] {
            assert!(read(xml.as_bytes()).is_err(), "{xml}");
        }
        assert!(read(&vec![b' '; LIMIT + 1]).is_err());
    }
    #[test]
    fn exact_depth_boundary_is_explicit() {
        let good = format!("{}{}", "<a>".repeat(129), "</a>".repeat(129));
        assert!(read(good.as_bytes()).is_ok());
        let bad = format!("{}{}", "<a>".repeat(130), "</a>".repeat(130));
        assert!(read(bad.as_bytes()).is_err());
    }
}
