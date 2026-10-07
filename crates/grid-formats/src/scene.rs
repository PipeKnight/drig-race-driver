//! Static MATRIXPALETTE draw assembly for the verified rigid car subset.
use crate::{Result, fail, mesh::Mesh, pssg::Tree, range};
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct Draw<'a> {
    pub id: &'a [u8],
    pub joint: &'a [u8],
    pub material: &'a [u8],
    pub source: usize,
    pub prototype: Option<&'a [u8]>,
    pub vertices: std::ops::Range<usize>,
    pub indices: Vec<u16>,
    /// File order is four rows; row-vector affine translation at 12..15.
    pub transform: [f32; 16],
}

pub fn transform_point(matrix: &[f32; 16], point: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|axis| {
        point[0] * matrix[axis]
            + point[1] * matrix[4 + axis]
            + point[2] * matrix[8 + axis]
            + matrix[12 + axis]
    })
}

/// Row-vector composition: a point traverses child first, then parent.
pub fn compose(child: &[f32; 16], parent: &[f32; 16]) -> [f32; 16] {
    std::array::from_fn(|i| {
        (0..4)
            .map(|k| child[(i / 4) * 4 + k] * parent[k * 4 + i % 4])
            .sum()
    })
}
const IDENTITY: [f32; 16] = [
    1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
];

/// Whole-source static instances, separate from the car palette subset.
pub fn read_static_draws<'a>(tree: &Tree<'a>, meshes: &[Mesh<'a>]) -> Result<Vec<Draw<'a>>> {
    read_scene_draws(tree, meshes, false, None)
}
/// Object-library closest LOD: direct children of LODVISIBLERENDERNODE only.
/// Lower LOD lists and physics/damage branches remain unselected.
pub fn read_object_draws<'a>(tree: &Tree<'a>, meshes: &[Mesh<'a>]) -> Result<Vec<Draw<'a>>> {
    read_scene_draws(tree, meshes, true, None)
}
/// Ordinary draws under an exact renderable NODE; exclude other template states.
pub fn read_renderable_draws<'a>(
    tree: &Tree<'a>,
    meshes: &[Mesh<'a>],
    id: &[u8],
) -> Result<Vec<Draw<'a>>> {
    let selected: Vec<_> = (0..tree.nodes.len())
        .filter(|&i| tree.name(i) == b"NODE" && tree.string(i, b"id").is_ok_and(|name| name == id))
        .collect();
    if selected.len() != 1 {
        return fail(0, "expected one selected renderable node");
    }
    read_scene_draws(tree, meshes, false, Some(selected[0]))
}
fn read_scene_draws<'a>(
    tree: &Tree<'a>,
    meshes: &[Mesh<'a>],
    object_subset: bool,
    selected: Option<usize>,
) -> Result<Vec<Draw<'a>>> {
    let sources: BTreeMap<_, _> = meshes.iter().enumerate().map(|(i, m)| (m.id, i)).collect();
    if sources.len() != meshes.len() {
        return fail(0, "duplicate mesh source ID");
    }
    let shaders: std::collections::BTreeSet<_> = (0..tree.nodes.len())
        .filter(|&i| tree.name(i) == b"SHADERINSTANCE")
        .map(|i| tree.string(i, b"id"))
        .collect::<Result<_>>()?;
    let mut result = Vec::new();
    for i in 0..tree.nodes.len() {
        if tree.name(i) != b"RENDERSTREAMINSTANCE" {
            continue;
        }
        if let Some(selected) = selected {
            let mut ancestor = tree.nodes[i].parent;
            let mut found = false;
            while let Some(a) = ancestor {
                if a == selected {
                    found = true;
                    break;
                }
                ancestor = tree.nodes[a].parent;
            }
            if !found {
                continue;
            }
        }
        // The same element also occurs inside car palettes; reject that context.
        let parent = tree.nodes[i].parent.ok_or(crate::Error {
            offset: tree.nodes[i].offset,
            message: "static draw parent absent",
        })?;
        if object_subset && tree.name(parent) != b"LODVISIBLERENDERNODE" {
            continue;
        }
        if !object_subset && tree.name(parent) != b"RENDERNODE" {
            return fail(
                tree.nodes[i].offset,
                "static draw requires render-node parent",
            );
        }
        if tree.integer(i, b"sourceCount")? != 1 || tree.integer(i, b"streamCount")? != 0 {
            return fail(
                tree.nodes[i].offset,
                "static draw requires one inherited source",
            );
        }
        let reference = tree.string(i, b"indices")?;
        let children = &tree.nodes[i].children;
        if children.len() != 1
            || tree.name(children[0]) != b"RENDERINSTANCESOURCE"
            || tree.string(children[0], b"source")? != reference
        {
            return fail(tree.nodes[i].offset, "static draw source references differ");
        }
        let source = *reference
            .strip_prefix(b"#")
            .and_then(|id| sources.get(id))
            .ok_or(crate::Error {
                offset: tree.nodes[i].offset,
                message: "unresolved static source",
            })?;
        let material = tree
            .string(i, b"shader")?
            .strip_prefix(b"#")
            .ok_or(crate::Error {
                offset: tree.nodes[i].offset,
                message: "expected local static shader",
            })?;
        if !shaders.contains(material) {
            return fail(tree.nodes[i].offset, "unresolved static shader");
        }
        let mut transform = IDENTITY;
        let mut ancestor = Some(parent);
        let mut root_seen = false;
        let mut prototype = None;
        while let Some(a) = ancestor {
            match tree.name(a) {
                b"RENDERNODE" | b"LODVISIBLERENDERNODE" | b"NODE" | b"ROOTNODE" => {
                    transform = compose(&transform, &matrix(tree, a)?);
                    if tree.name(a) == b"ROOTNODE" {
                        root_seen = true;
                        prototype = Some(tree.string(a, b"id")?);
                    }
                }
                b"LIBRARY" | b"PSSGDATABASE" | b"ROOT" => {}
                _ => {
                    return fail(
                        tree.nodes[a].offset,
                        "unsupported static transform ancestor",
                    );
                }
            }
            if selected == Some(a) {
                root_seen = true;
                break;
            }
            ancestor = tree.nodes[a].parent;
        }
        if !root_seen {
            return fail(tree.nodes[i].offset, "static draw has no scene root");
        }
        if transform.iter().any(|v| !v.is_finite()) {
            return fail(tree.nodes[i].offset, "composed transform nonfinite");
        }
        result.push(Draw {
            id: tree.string(i, b"id")?,
            joint: tree.string(parent, b"id")?,
            material,
            source,
            prototype,
            vertices: 0..meshes[source].positions.len(),
            indices: meshes[source].indices.clone(),
            transform,
        });
    }
    if result.is_empty() && selected.is_none() {
        return fail(0, "no supported static draws");
    }
    Ok(result)
}

pub fn matrix(tree: &Tree<'_>, node: usize) -> Result<[f32; 16]> {
    let children: Vec<_> = tree.nodes[node]
        .children
        .iter()
        .copied()
        .filter(|&i| tree.name(i) == b"TRANSFORM")
        .collect();
    if children.len() != 1 {
        return fail(tree.nodes[node].offset, "expected one joint transform");
    }
    decode_matrix(
        tree.nodes[children[0]].data.unwrap(),
        tree.nodes[children[0]].offset,
    )
}
pub fn decode_matrix(data: &[u8], offset: usize) -> Result<[f32; 16]> {
    if data.len() != 64 {
        return fail(offset, "expected 16-float transform");
    }
    let mut result = [0.0; 16];
    for (value, bytes) in result.iter_mut().zip(data.as_chunks::<4>().0) {
        *value = f32::from_be_bytes(*bytes);
        if !value.is_finite() {
            return fail(offset, "nonfinite transform");
        }
    }
    if result[3] != 0.0 || result[7] != 0.0 || result[11] != 0.0 || result[15] != 1.0 {
        return fail(offset, "only affine transforms supported");
    }
    Ok(result)
}

/// Compose one ordinary scene node through its affine NODE/ROOTNODE ancestry.
pub fn node_matrix(tree: &Tree<'_>, node: usize) -> Result<[f32; 16]> {
    if node >= tree.nodes.len() {
        return fail(node, "scene node index outside tree");
    }
    let mut transform = IDENTITY;
    let mut ancestor = Some(node);
    let mut root = false;
    while let Some(i) = ancestor {
        match tree.name(i) {
            b"NODE" | b"ROOTNODE" => {
                transform = compose(&transform, &matrix(tree, i)?);
                root |= tree.name(i) == b"ROOTNODE";
            }
            b"LIBRARY" | b"PSSGDATABASE" | b"ROOT" => {}
            _ => return fail(tree.nodes[i].offset, "unsupported named-node ancestry"),
        }
        ancestor = tree.nodes[i].parent;
    }
    if !root || transform.iter().any(|v| !v.is_finite()) {
        return fail(
            tree.nodes[node].offset,
            "named node lacks finite rooted transform",
        );
    }
    Ok(transform)
}

/// Explicit LOD bundle selection. No guessed automatic LOD or damage state.
pub fn read_draws<'a>(
    tree: &Tree<'a>,
    meshes: &[Mesh<'a>],
    bundle: &[u8],
) -> Result<Vec<Draw<'a>>> {
    let sources: BTreeMap<_, _> = meshes.iter().enumerate().map(|(i, m)| (m.id, i)).collect();
    if sources.len() != meshes.len() {
        return fail(0, "duplicate mesh source ID");
    }
    let bundles: Vec<_> = (0..tree.nodes.len())
        .filter(|&i| {
            tree.name(i) == b"MATRIXPALETTEBUNDLENODE"
                && tree.string(i, b"id").is_ok_and(|id| id == bundle)
        })
        .collect();
    if bundles.len() != 1 {
        return fail(0, "expected exactly one selected LOD bundle");
    }
    let bundle_node = bundles[0];
    // Currently verified bundles/root are identity. Refuse unsupported ancestry
    // rather than dropping nonidentity parent transforms silently.
    let mut ancestor = Some(bundle_node);
    while let Some(i) = ancestor {
        if tree.name(i) == b"ROOTNODE" || tree.name(i) == b"MATRIXPALETTEBUNDLENODE" {
            let m = matrix(tree, i)?;
            for (j, value) in m.iter().enumerate() {
                if *value
                    != if [0, 5, 10, 15].contains(&j) {
                        1.0
                    } else {
                        0.0
                    }
                {
                    return fail(
                        tree.nodes[i].offset,
                        "nonidentity car root/bundle transform unsupported",
                    );
                }
            }
        }
        ancestor = tree.nodes[i].parent;
    }
    let mut draws = Vec::new();
    for i in 0..tree.nodes.len() {
        if tree.name(i) != b"MATRIXPALETTERENDERINSTANCE" {
            continue;
        }
        let joint_node = tree.nodes[i].parent.ok_or(crate::Error {
            offset: tree.nodes[i].offset,
            message: "draw parent absent",
        })?;
        if tree.nodes[joint_node].parent != Some(bundle_node) {
            continue;
        }
        if tree.name(joint_node) != b"MATRIXPALETTEJOINTNODE" {
            return fail(tree.nodes[i].offset, "expected rigid joint draw parent");
        }
        if tree.integer(i, b"sourceCount")? != 1 || tree.integer(i, b"streamCount")? != 0 {
            return fail(
                tree.nodes[i].offset,
                "only one inherited render source supported",
            );
        }
        let reference = tree.string(i, b"indices")?;
        let Some(reference) = reference.strip_prefix(b"#") else {
            return fail(
                tree.nodes[i].offset,
                "expected local index source reference",
            );
        };
        let Some(&source) = sources.get(reference) else {
            return fail(tree.nodes[i].offset, "unresolved render source");
        };
        let children = &tree.nodes[i].children;
        if children.len() != 1
            || tree.name(children[0]) != b"RENDERINSTANCESOURCE"
            || tree.string(children[0], b"source")? != tree.string(i, b"indices")?
        {
            return fail(
                tree.nodes[i].offset,
                "draw stream and index source must agree",
            );
        }
        let mesh = &meshes[source];
        let first = tree.integer(i, b"streamOffset")? as usize;
        let count = tree.integer(i, b"elementCountFromOffset")? as usize;
        let vertices = range(first, count, mesh.positions.len())?;
        let joint_id = tree.integer(joint_node, b"jointID")?;
        let Some(skin) = &mesh.skin_indices else {
            return fail(tree.nodes[i].offset, "rigid skin index stream absent");
        };
        if joint_id > 65535 || skin[vertices.clone()].iter().any(|v| *v != joint_id as f32) {
            return fail(
                tree.nodes[i].offset,
                "draw vertices do not all reference parent joint",
            );
        }
        let index_first = tree.integer(i, b"indexOffset")? as usize;
        let index_count = tree.integer(i, b"indicesCountFromOffset")? as usize;
        if count == 0
            || index_count == 0
            || !index_first.is_multiple_of(3)
            || !index_count.is_multiple_of(3)
        {
            return fail(tree.nodes[i].offset, "invalid triangle draw range");
        }
        let selected = range(index_first, index_count, mesh.indices.len())?;
        let mut indices = Vec::with_capacity(index_count);
        for &index in &mesh.indices[selected] {
            if !(vertices.start..vertices.end).contains(&(index as usize)) {
                return fail(
                    tree.nodes[i].offset,
                    "draw index outside selected vertex range",
                );
            }
            indices.push((index as usize - first) as u16);
        }
        let material = tree.string(i, b"shader")?;
        let Some(material) = material.strip_prefix(b"#") else {
            return fail(tree.nodes[i].offset, "expected local shader reference");
        };
        if !(0..tree.nodes.len()).any(|j| {
            tree.name(j) == b"SHADERINSTANCE"
                && tree.string(j, b"id").is_ok_and(|id| id == material)
        }) {
            return fail(tree.nodes[i].offset, "unresolved draw shader");
        }
        draws.push(Draw {
            id: tree.string(i, b"id")?,
            joint: tree.string(joint_node, b"id")?,
            material,
            source,
            prototype: None,
            vertices,
            indices,
            transform: matrix(tree, joint_node)?,
        });
    }
    if draws.is_empty() {
        return fail(
            tree.nodes[bundle_node].offset,
            "selected bundle has no supported draws",
        );
    }
    Ok(draws)
}
