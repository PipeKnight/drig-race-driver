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

/// One shader instance's saved constant inputs, by input name, as floats.
#[derive(Debug)]
pub struct ShaderConstants<'a> {
    pub shader: &'a [u8],
    /// The shader group without its `#`, e.g. `carpaint_metallic.fx`.
    pub group: &'a [u8],
    pub values: BTreeMap<&'a [u8], Vec<f32>>,
}

/// The saved constants of every shader instance (car reflection controls,
/// docs/CAR_REFLECTIONS.md): big-endian floats, one to four per input.
pub fn read_constants<'a>(tree: &Tree<'a>) -> Result<Vec<ShaderConstants<'a>>> {
    let mut groups = BTreeMap::new();
    for i in 0..tree.nodes.len() {
        if tree.name(i) == b"SHADERGROUP" {
            let definitions: Vec<_> = tree.nodes[i]
                .children
                .iter()
                .copied()
                .filter(|&j| tree.name(j) == b"SHADERINPUTDEFINITION")
                .collect();
            groups.insert(tree.string(i, b"id")?, definitions);
        }
    }
    let mut result = Vec::new();
    for i in 0..tree.nodes.len() {
        if tree.name(i) != b"SHADERINSTANCE" {
            continue;
        }
        let group = tree
            .string(i, b"shaderGroup")?
            .strip_prefix(b"#")
            .ok_or(crate::Error {
                offset: tree.nodes[i].offset,
                message: "external shader group",
            })?;
        let definitions = groups.get(group).ok_or(crate::Error {
            offset: tree.nodes[i].offset,
            message: "unresolved shader group",
        })?;
        let mut values = BTreeMap::new();
        for &child in &tree.nodes[i].children {
            if tree.name(child) != b"SHADERINPUT" || tree.string(child, b"type")? != b"constant" {
                continue;
            }
            let Some(&definition) = definitions.get(tree.integer(child, b"parameterID")? as usize)
            else {
                return fail(tree.nodes[child].offset, "shader constant outside definition list");
            };
            let data = tree.nodes[child].data.unwrap_or_default();
            if data.is_empty() || data.len() > 16 || !data.len().is_multiple_of(4) {
                return fail(tree.nodes[child].offset, "unsupported shader constant size");
            }
            let floats: Vec<f32> = data
                .as_chunks::<4>()
                .0
                .iter()
                .map(|c| f32::from_be_bytes(*c))
                .collect();
            if floats.iter().any(|f| !f.is_finite()) {
                return fail(tree.nodes[child].offset, "nonfinite shader constant");
            }
            values.insert(tree.string(definition, b"name")?, floats);
        }
        result.push(ShaderConstants {
            shader: tree.string(i, b"id")?,
            group,
            values,
        });
    }
    Ok(result)
}
