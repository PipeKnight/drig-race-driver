use crate::{Cursor, Result, fail, range};
use std::{collections::BTreeSet, ops::Range};

#[derive(Debug)]
pub struct AttributeDefinition<'a> {
    pub id: u32,
    pub name: &'a [u8],
}
#[derive(Debug)]
pub struct ElementDefinition<'a> {
    pub id: u32,
    pub name: &'a [u8],
    pub attributes: Vec<AttributeDefinition<'a>>,
}
#[derive(Debug, Clone)]
pub struct Attribute<'a> {
    pub id: u32,
    pub value: &'a [u8],
}

/// Flat preorder storage avoids recursive ownership/drop on untrusted trees.
#[derive(Debug)]
pub struct Node<'a> {
    pub id: u32,
    pub offset: usize,
    pub parent: Option<usize>,
    pub attributes: Vec<Attribute<'a>>,
    pub children: Vec<usize>,
    /// Present only for the explicitly supported raw-data element names.
    pub data: Option<&'a [u8]>,
}

#[derive(Debug)]
pub struct Tree<'a> {
    pub framing: Pssg<'a>,
    pub nodes: Vec<Node<'a>>,
}

impl<'a> Tree<'a> {
    pub fn name(&self, node: usize) -> &[u8] {
        self.framing
            .schema
            .iter()
            .find(|e| e.id == self.nodes[node].id)
            .unwrap()
            .name
    }

    pub fn attribute(&self, node: usize, name: &[u8]) -> Result<&'a [u8]> {
        let n = &self.nodes[node];
        let mut found = None;
        for a in &n.attributes {
            let matches = self
                .framing
                .schema
                .iter()
                .flat_map(|e| &e.attributes)
                .any(|d| d.id == a.id && d.name == name);
            if matches && found.replace(a.value).is_some() {
                return fail(n.offset, "duplicate named attribute");
            }
        }
        found.ok_or(crate::Error {
            offset: n.offset,
            message: "required attribute absent",
        })
    }

    pub fn integer(&self, node: usize, name: &[u8]) -> Result<u32> {
        let value = self.attribute(node, name)?;
        if value.len() != 4 {
            return fail(
                self.nodes[node].offset,
                "expected four-byte integer attribute",
            );
        }
        Ok(u32::from_be_bytes(value.try_into().unwrap()))
    }

    pub fn string(&self, node: usize, name: &[u8]) -> Result<&'a [u8]> {
        let value = self.attribute(node, name)?;
        if value.len() < 4 {
            return fail(self.nodes[node].offset, "truncated string attribute");
        }
        let length = u32::from_be_bytes(value[..4].try_into().unwrap()) as usize;
        if length != value.len() - 4 {
            return fail(self.nodes[node].offset, "string attribute length mismatch");
        }
        Ok(&value[4..])
    }
}

/// Explicit data-node policy, validated on selected GRID car/texture resources.
/// Unknown elements are parsed as containers; malformed children are errors,
/// never silently reclassified as data.
pub fn read_tree(bytes: &[u8]) -> Result<Tree<'_>> {
    let framing = inspect(bytes)?;
    let names: std::collections::BTreeMap<_, _> =
        framing.schema.iter().map(|e| (e.id, e.name)).collect();
    let attrs: BTreeSet<_> = framing
        .schema
        .iter()
        .flat_map(|e| &e.attributes)
        .map(|a| a.id)
        .collect();
    let mut nodes: Vec<Node<'_>> = Vec::new();
    let mut pending: Vec<(usize, usize, Option<usize>, usize)> =
        vec![(framing.root_offset, bytes.len(), None, 0usize)];
    let mut total_attributes = 0usize;
    while let Some((offset, limit, parent, depth)) = pending.pop() {
        if depth > 128 || nodes.len() >= 250_000 {
            return fail(offset, "tree depth/node inspector limit exceeded");
        }
        range(offset, 12, limit)?;
        let mut c = Cursor { bytes, pos: offset };
        let id = c.be()?;
        let Some(name) = names.get(&id) else {
            return fail(offset, "node ID absent from schema");
        };
        let size = c.be()? as usize;
        let end = range(c.pos, size, limit)?.end;
        if size < 4 {
            return fail(offset + 4, "node lacks attribute block size");
        }
        let attr_size = c.be()? as usize;
        let attr_end = range(c.pos, attr_size, end)?.end;
        let mut attributes = Vec::new();
        let mut used = BTreeSet::new();
        while c.pos < attr_end {
            range(c.pos, 8, attr_end)?;
            let aid = c.be()?;
            if !attrs.contains(&aid) || !used.insert(aid) {
                return fail(c.pos - 4, "unknown or duplicate node attribute ID");
            }
            let length = c.be()? as usize;
            range(c.pos, length, attr_end)?;
            total_attributes += 1;
            if total_attributes > 1_000_000 {
                return fail(c.pos, "tree attribute inspector limit exceeded");
            }
            attributes.push(Attribute {
                id: aid,
                value: c.take(length)?,
            });
        }
        let data_node = (*name == b"DATA"
            && parent.is_some_and(|p| names.get(&nodes[p].id).is_some_and(|n| *n == b"PNSTRING")))
            || matches!(
                *name,
                b"BOUNDINGBOX"
                    | b"DATABLOCKDATA"
                    | b"DATABLOCKBUFFERED"
                    | b"INDEXSOURCEDATA"
                    | b"INVERSEBINDMATRIX"
                    | b"MODIFIERNETWORKINSTANCEUNIQUEMODIFIERINPUT"
                    | b"RENDERINTERFACEBOUNDBUFFERED"
                    | b"SHADERINPUT"
                    | b"TEXTUREIMAGEBLOCKDATA"
                    | b"TRANSFORM"
                    | b"TEMPLATETRANSFORM"
                    | b"NEANIMKEYS"
                    | b"SHADERPROGRAMCODEBLOCK"
            )
            || name.starts_with(b"NeAnimPacketData_");
        let index = nodes.len();
        nodes.push(Node {
            id,
            offset,
            parent,
            attributes,
            children: Vec::new(),
            data: data_node.then_some(&bytes[attr_end..end]),
        });
        if let Some(parent) = parent {
            nodes[parent].children.push(index);
        }
        if !data_node {
            let mut children = Vec::new();
            let mut pos = attr_end;
            while pos < end {
                range(pos, 12, end)?;
                let length =
                    u32::from_be_bytes(bytes[pos + 4..pos + 8].try_into().unwrap()) as usize;
                if length < 4 {
                    return fail(pos + 4, "child lacks attribute block size");
                }
                let child_end = range(pos + 8, length, end)?.end;
                children.push((pos, child_end, Some(index), depth + 1));
                if children.len() + nodes.len() + pending.len() > 250_000 {
                    return fail(pos, "tree node inspector limit exceeded");
                }
                pos = child_end;
            }
            pending.extend(children.into_iter().rev());
        }
    }
    Ok(Tree { framing, nodes })
}
#[derive(Debug)]
pub struct Pssg<'a> {
    pub declared_size: u32,
    pub schema: Vec<ElementDefinition<'a>>,
    pub attribute_count: u32,
    pub root_id: u32,
    pub root_offset: usize,
    pub root_attributes: Vec<Attribute<'a>>,
    /// Opaque payload; no inferred recursive node/data distinction.
    pub root_payload: Range<usize>,
}

/// Supports raw PSSG only. Gzip/XML variants are deliberately unsupported.
pub fn inspect(bytes: &[u8]) -> Result<Pssg<'_>> {
    let mut c = Cursor { bytes, pos: 0 };
    if c.take(4)? != b"PSSG" {
        return fail(0, "expected raw PSSG magic");
    }
    let declared_size = c.be()?;
    if declared_size as usize != bytes.len() - 8 {
        return fail(
            4,
            "PSSG declared size does not match file length minus eight",
        );
    }
    let attribute_count = c.be()?;
    let element_count = c.be()?;
    // These are limits on this research inspector, not format constants.
    if element_count == 0 || element_count > 100_000 || attribute_count > 1_000_000 {
        return fail(8, "unsupported schema counts");
    }
    if element_count as usize > (bytes.len() - c.pos) / 12 {
        return fail(12, "schema count cannot fit remaining bytes");
    }
    let mut schema = Vec::new();
    let mut element_ids = BTreeSet::new();
    let mut attribute_ids = BTreeSet::new();
    for _ in 0..element_count {
        let id_offset = c.pos;
        let id = c.be()?;
        if id == 0 || id > element_count || !element_ids.insert(id) {
            return fail(id_offset, "invalid or duplicate element ID");
        }
        let name = c.name()?;
        let count = c.be()? as usize;
        if count > (bytes.len() - c.pos) / 8 || count > attribute_count as usize {
            return fail(
                c.pos - 4,
                "attribute definitions cannot fit declared bounds",
            );
        }
        let mut attributes = Vec::new();
        for _ in 0..count {
            let id_offset = c.pos;
            let id = c.be()?;
            if id == 0 || id > attribute_count || !attribute_ids.insert(id) {
                return fail(id_offset, "invalid or duplicate attribute ID");
            }
            attributes.push(AttributeDefinition {
                id,
                name: c.name()?,
            });
        }
        schema.push(ElementDefinition {
            id,
            name,
            attributes,
        });
    }
    if attribute_ids.len() != attribute_count as usize {
        return fail(c.pos, "attribute definition count mismatch");
    }
    let root_offset = c.pos;
    let root_id = c.be()?;
    if !element_ids.contains(&root_id) {
        return fail(root_offset, "root ID absent from schema");
    }
    let root_size = c.be()? as usize;
    let root_end = range(c.pos, root_size, bytes.len())?.end;
    if root_end != bytes.len() || root_size < 4 {
        return fail(
            c.pos - 4,
            "root must contain attribute size and cover rest of file",
        );
    }
    let attribute_size = c.be()? as usize;
    let attr_end = range(c.pos, attribute_size, root_end)?.end;
    let mut root_attributes = Vec::new();
    while c.pos < attr_end {
        range(c.pos, 8, attr_end)?;
        let id = c.be()?;
        if !attribute_ids.contains(&id) {
            return fail(c.pos - 4, "root attribute ID absent from schema");
        }
        let length = c.be()? as usize;
        range(c.pos, length, attr_end)?;
        root_attributes.push(Attribute {
            id,
            value: c.take(length)?,
        });
    }
    Ok(Pssg {
        declared_size,
        schema,
        attribute_count,
        root_id,
        root_offset,
        root_attributes,
        root_payload: attr_end..root_end,
    })
}
