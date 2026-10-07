//! Named saved texture bindings. Original shader execution is not implemented.
use crate::{Result, fail, pssg::Tree};
use std::collections::BTreeMap;
#[derive(Debug)]
pub struct TextureBinding<'a> {
    pub shader: &'a [u8],
    pub sampler: &'a [u8],
    pub texture: &'a [u8],
    pub container: Option<&'a [u8]>,
}
pub fn read_texture_bindings<'a>(tree: &Tree<'a>) -> Result<Vec<TextureBinding<'a>>> {
    let mut groups = BTreeMap::new();
    for i in 0..tree.nodes.len() {
        if tree.name(i) == b"SHADERGROUP" {
            let id = tree.string(i, b"id")?;
            let definitions: Vec<_> = tree.nodes[i]
                .children
                .iter()
                .copied()
                .filter(|&j| tree.name(j) == b"SHADERINPUTDEFINITION")
                .collect();
            if definitions.len() != tree.integer(i, b"parameterCount")? as usize
                || groups.insert(id, definitions).is_some()
            {
                return fail(
                    tree.nodes[i].offset,
                    "shader parameter count or group ID invalid",
                );
            }
        }
    }
    let mut result = Vec::new();
    let mut shaders = std::collections::BTreeSet::new();
    for i in 0..tree.nodes.len() {
        if tree.name(i) != b"SHADERINSTANCE" {
            continue;
        }
        let shader = tree.string(i, b"id")?;
        if !shaders.insert(shader) {
            return fail(tree.nodes[i].offset, "duplicate shader ID");
        }
        let group = tree
            .string(i, b"shaderGroup")?
            .strip_prefix(b"#")
            .and_then(|id| groups.get(id))
            .ok_or(crate::Error {
                offset: tree.nodes[i].offset,
                message: "unresolved shader group",
            })?;
        let mut parameters = std::collections::BTreeSet::new();
        for &child in &tree.nodes[i].children {
            if tree.name(child) != b"SHADERINPUT" {
                return fail(tree.nodes[child].offset, "unexpected shader instance child");
            }
            let parameter = tree.integer(child, b"parameterID")? as usize;
            let Some(&definition) = group.get(parameter) else {
                return fail(
                    tree.nodes[child].offset,
                    "shader input parameter outside definition list",
                );
            };
            if !parameters.insert(parameter)
                || tree.string(child, b"type")? != tree.string(definition, b"type")?
            {
                return fail(
                    tree.nodes[child].offset,
                    "shader input duplicate or type mismatch",
                );
            }
            if tree.string(child, b"type")? == b"texture" {
                let reference = tree.string(child, b"texture")?;
                let separator = reference
                    .iter()
                    .position(|&b| b == b'#')
                    .ok_or(crate::Error {
                        offset: tree.nodes[child].offset,
                        message: "expected texture fragment reference",
                    })?;
                let texture = &reference[separator + 1..];
                if texture.is_empty() || texture.contains(&b'#') {
                    return fail(
                        tree.nodes[child].offset,
                        "texture fragment empty or ambiguous",
                    );
                }
                result.push(TextureBinding {
                    shader,
                    sampler: tree.string(definition, b"name")?,
                    texture,
                    container: (separator != 0).then_some(&reference[..separator]),
                });
            }
        }
        if parameters.len() != tree.integer(i, b"parameterSavedCount")? as usize {
            return fail(tree.nodes[i].offset, "saved shader parameter count differs");
        }
    }
    Ok(result)
}
