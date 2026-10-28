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
    read_scene_draws(tree, meshes, Subset::Static, None)
}
/// Object-library closest LOD: direct children of LODVISIBLERENDERNODE only.
/// Lower LOD lists and physics/damage branches remain unselected.
pub fn read_object_draws<'a>(tree: &Tree<'a>, meshes: &[Mesh<'a>]) -> Result<Vec<Draw<'a>>> {
    read_scene_draws(tree, meshes, Subset::Closest, None)
}
/// Every object-library level with its number (docs/TRACK_LOD.md): level 0 is
/// the direct children of LODVISIBLERENDERNODE, levels 1 and 2 the
/// LODRENDERINSTANCELISTs of its LODRENDERINSTANCES in file order. Physics and
/// damage branches remain unselected.
pub fn read_object_level_draws<'a>(
    tree: &Tree<'a>,
    meshes: &[Mesh<'a>],
) -> Result<Vec<(u8, Draw<'a>)>> {
    let context = Context::new(tree, meshes)?;
    let mut result = Vec::new();
    for i in 0..tree.nodes.len() {
        if tree.name(i) != b"RENDERSTREAMINSTANCE" {
            continue;
        }
        let Some(draw) = instance_draw(tree, meshes, &context, i, Subset::Levels, None)? else {
            continue;
        };
        let parent = tree.nodes[i].parent.expect("checked by instance_draw");
        let level = if tree.name(parent) == b"LODRENDERINSTANCELIST" {
            let lists = tree.nodes[parent].parent.expect("list parent checked");
            1 + tree.nodes[lists]
                .children
                .iter()
                .filter(|&&c| tree.name(c) == b"LODRENDERINSTANCELIST")
                .position(|&c| c == parent)
                .expect("list among its parent's children") as u8
        } else {
            0
        };
        result.push((level, draw));
    }
    if result.is_empty() {
        return fail(0, "no supported static draws");
    }
    Ok(result)
}
/// Ordinary draws under an exact renderable NODE; exclude other template states.
pub fn read_renderable_draws<'a>(
    tree: &Tree<'a>,
    meshes: &[Mesh<'a>],
    id: &[u8],
) -> Result<Vec<Draw<'a>>> {
    Renderables::new(tree, meshes)?.draws(id)
}

/// [`read_renderable_draws`] for many ids of one tree: the source, shader and
/// node-id tables are built once, and each id walks only its own subtree
/// (F445: the Nurburgring's 11,962 breakable pieces scanned the whole object
/// library each, minutes of black screen before a race on the phone). Same
/// draws, order and errors as one call per id.
pub struct Renderables<'t, 'a> {
    tree: &'t Tree<'a>,
    meshes: &'t [Mesh<'a>],
    context: Context<'a>,
    nodes: std::collections::HashMap<&'a [u8], Vec<usize>>,
    /// BUNDLENODE ids: the damage renderables of tyre walls, Armco sections
    /// and flags ([`Self::draws_in`] roots).
    bundles: std::collections::HashMap<&'a [u8], Vec<usize>>,
    skeletons: std::collections::HashMap<&'a [u8], usize>,
    /// NODE ids with the exporter's `!N` suffix for a repeated name, by the
    /// name before it (Le Mans' blue and yellow tyre walls name their tyres
    /// `tyre01_obj_01!0`; the entity data names `tyre01_obj_01`).
    suffixed: std::collections::HashMap<&'a [u8], Vec<usize>>,
}

impl<'t, 'a> Renderables<'t, 'a> {
    pub fn new(tree: &'t Tree<'a>, meshes: &'t [Mesh<'a>]) -> Result<Self> {
        let mut nodes: std::collections::HashMap<&'a [u8], Vec<usize>> = Default::default();
        let mut bundles: std::collections::HashMap<&'a [u8], Vec<usize>> = Default::default();
        let mut skeletons = std::collections::HashMap::new();
        let mut suffixed: std::collections::HashMap<&'a [u8], Vec<usize>> = Default::default();
        for i in 0..tree.nodes.len() {
            if tree.name(i) == b"NODE"
                && let Ok(name) = tree.string(i, b"id")
                && let Some(at) = name.iter().rposition(|&c| c == b'!')
                && at > 0
                && name[at + 1..].iter().all(u8::is_ascii_digit)
            {
                suffixed.entry(&name[..at]).or_default().push(i);
            }
            if tree.name(i) == b"SKELETON"
                && let Ok(name) = tree.string(i, b"id")
            {
                skeletons.insert(name, i);
            }
            let index = match tree.name(i) {
                b"NODE" => &mut nodes,
                b"BUNDLENODE" => &mut bundles,
                _ => continue,
            };
            if let Ok(name) = tree.string(i, b"id") {
                index.entry(name).or_default().push(i);
            }
        }
        Ok(Self {
            tree,
            meshes,
            context: Context::new(tree, meshes)?,
            nodes,
            bundles,
            skeletons,
            suffixed,
        })
    }

    /// Ordinary draws under the one NODE named `id`.
    pub fn draws(&self, id: &[u8]) -> Result<Vec<Draw<'a>>> {
        let selected = match self.nodes.get(id).map(Vec::as_slice) {
            Some([one]) => *one,
            _ => return fail(0, "expected one selected renderable node"),
        };
        // Descendants in index order: the instances whose ancestor chain
        // holds `selected`, as the whole-tree scan visits them.
        let mut instances = Vec::new();
        let mut stack = self.tree.nodes[selected].children.clone();
        while let Some(n) = stack.pop() {
            if self.tree.name(n) == b"RENDERSTREAMINSTANCE" {
                instances.push(n);
            }
            stack.extend_from_slice(&self.tree.nodes[n].children);
        }
        instances.sort_unstable();
        let mut result = Vec::new();
        for i in instances {
            if let Some(draw) = instance_draw(
                self.tree,
                self.meshes,
                &self.context,
                i,
                Subset::Static,
                Some(selected),
            )? {
                result.push(draw);
            }
        }
        Ok(result)
    }

    /// Draws under the NODE `node`, placed in the frame of its ancestor NODE
    /// or BUNDLENODE `root` (`node` may carry the exporter's `!N` suffix): the transforms of `node` and of every node between it and
    /// `root` (inclusive) are composed. [`Self::draws`] stops at the selected
    /// node, so a sub-node's draws (a breakable piece's body anchor inside
    /// its damage renderable) lacked the transforms above it. `node` must be
    /// the one NODE of that id under `root`; other renderables may reuse the
    /// id (Nürburgring tyre walls name their tyres `tyre01_obj_01` and on).
    pub fn draws_in(&self, root: &[u8], node: &[u8]) -> Result<Vec<Draw<'a>>> {
        let root = match (self.nodes.get(root), self.bundles.get(root)) {
            (Some(n), None) | (None, Some(n)) if n.len() == 1 => n[0],
            _ => return fail(0, "expected one selected renderable node"),
        };
        let under = |n: usize| {
            let mut ancestor = Some(n);
            while ancestor.is_some_and(|a| a != root) {
                ancestor = ancestor.and_then(|a| self.tree.nodes[a].parent);
            }
            ancestor.is_some()
        };
        let candidates = (self.nodes.get(node).into_iter().flatten())
            .chain(self.suffixed.get(node).into_iter().flatten());
        let node = match candidates.filter(|&&n| under(n)).collect::<Vec<_>>()[..] {
            [one] => *one,
            _ => return fail(0, "expected one node under the renderable root"),
        };
        let mut instances = Vec::new();
        let mut stack = self.tree.nodes[node].children.clone();
        while let Some(n) = stack.pop() {
            if self.tree.name(n) == b"RENDERSTREAMINSTANCE" {
                instances.push(n);
            }
            stack.extend_from_slice(&self.tree.nodes[n].children);
        }
        instances.sort_unstable();
        if instances.is_empty() {
            return self.skin_draws(root, node);
        }
        let mut result = Vec::new();
        for i in instances {
            if let Some(draw) =
                instance_draw(self.tree, self.meshes, &self.context, i, Subset::Static, Some(root))?
            {
                result.push(draw);
            }
        }
        Ok(result)
    }

    /// The triangles of `root`'s skinned meshes (SKINNODE, network
    /// `skinTransformLocal`) whose main joint is a JOINTNODE at or under
    /// `node`, one draw per joint, in `root`'s frame: a bind-space vertex goes
    /// through the joint's INVERSEBINDMATRIX, then the joint's and its
    /// ancestors' transforms up to `root`. Tyre walls, Armco sections and
    /// flags keep their damage geometry in such a mesh (F566). AUTHORED
    /// approximation: the original blends up to four joints per vertex
    /// (Armco rails bend smoothly); here each triangle follows one joint
    /// (its vertices' main joint, [`crate::mesh::read_object_meshes`]), exact
    /// at the bind pose and with seams once the pieces separate.
    fn skin_draws(&self, root: usize, node: usize) -> Result<Vec<Draw<'a>>> {
        let tree = self.tree;
        let descendants = |from: usize| {
            let mut out = Vec::new();
            let mut stack = vec![from];
            while let Some(n) = stack.pop() {
                out.push(n);
                stack.extend(tree.nodes[n].children.iter().rev());
            }
            out
        };
        let under_node = |mut n: usize| loop {
            if n == node {
                break true;
            }
            match tree.nodes[n].parent {
                Some(p) if n != root => n = p,
                _ => break false,
            }
        };
        let in_root = descendants(root);
        let mut result = Vec::new();
        for &skin in in_root.iter().filter(|&&n| tree.name(n) == b"SKINNODE") {
            let children = &tree.nodes[skin].children;
            let skeleton = tree.string(skin, b"skeleton")?.strip_prefix(b"#").unwrap_or_default();
            let Some(&skeleton) = self.skeletons.get(skeleton) else {
                return fail(tree.nodes[skin].offset, "unresolved skin skeleton");
            };
            let inverse: Vec<[f32; 16]> = (tree.nodes[skeleton].children.iter())
                .filter(|&&c| tree.name(c) == b"INVERSEBINDMATRIX")
                .map(|&c| decode_bind_matrix(tree.nodes[c].data.unwrap_or_default(), tree.nodes[c].offset))
                .collect::<Result<_>>()?;
            let mut joints = Vec::new();
            for &c in children.iter().filter(|&&c| tree.name(c) == b"SKINJOINT") {
                let id = tree.string(c, b"joint")?.strip_prefix(b"#").unwrap_or_default();
                let found: Vec<usize> = (in_root.iter().copied())
                    .filter(|&n| tree.name(n) == b"JOINTNODE" && tree.string(n, b"id").ok() == Some(id))
                    .collect();
                let [joint] = found[..] else {
                    return fail(tree.nodes[c].offset, "expected one skin joint under the renderable root");
                };
                joints.push(joint);
            }
            if joints.len() != inverse.len() {
                return fail(tree.nodes[skin].offset, "skin joint and bind matrix counts differ");
            }
            for &instance in children.iter().filter(|&&c| tree.name(c) == b"MODIFIERNETWORKINSTANCE") {
                if !tree.string(instance, b"network")?.ends_with(b"#skinTransformLocal") {
                    continue;
                }
                let reference = tree.string(instance, b"indices")?.strip_prefix(b"#").unwrap_or_default();
                // Not rigid, so not read: no draw.
                let Some(&source) = self.context.sources.get(reference) else {
                    continue;
                };
                let mesh = &self.meshes[source];
                let Some(bound) = &mesh.skin_indices else {
                    return fail(tree.nodes[instance].offset, "skinned source without joints");
                };
                let material = tree.string(instance, b"shader")?.strip_prefix(b"#").ok_or(crate::Error {
                    offset: tree.nodes[instance].offset,
                    message: "expected local skin shader",
                })?;
                if !self.context.shaders.contains(material) {
                    return fail(tree.nodes[instance].offset, "unresolved skin shader");
                }
                for (j, &joint) in joints.iter().enumerate() {
                    if !under_node(joint) {
                        continue;
                    }
                    // A triangle follows the main joint of two of its
                    // vertices, else of its first.
                    let main = |t: &[u16; 3]| {
                        let [a, b, c] = t.map(|v| bound[v as usize]);
                        if a == b || a == c { a } else if b == c { b } else { a }
                    };
                    let indices: Vec<u16> = (mesh.indices.as_chunks::<3>().0.iter())
                        .filter(|t| main(t) == j as f32)
                        .flatten()
                        .copied()
                        .collect();
                    if indices.is_empty() {
                        continue;
                    }
                    let mut chain = IDENTITY;
                    let mut a = joint;
                    loop {
                        chain = compose(&chain, &matrix(tree, a)?);
                        if a == root {
                            break;
                        }
                        a = tree.nodes[a].parent.ok_or(crate::Error {
                            offset: tree.nodes[a].offset,
                            message: "skin joint outside its root",
                        })?;
                    }
                    result.push(Draw {
                        id: tree.string(instance, b"id")?,
                        joint: tree.string(joint, b"id")?,
                        material,
                        source,
                        prototype: None,
                        vertices: 0..mesh.positions.len(),
                        indices,
                        transform: compose(&inverse[j], &chain),
                    });
                }
            }
        }
        Ok(result)
    }
}

/// Mesh sources by id and the tree's shader instance ids.
struct Context<'a> {
    sources: BTreeMap<&'a [u8], usize>,
    shaders: std::collections::BTreeSet<&'a [u8]>,
}

impl<'a> Context<'a> {
    fn new(tree: &Tree<'a>, meshes: &[Mesh<'a>]) -> Result<Self> {
        let sources: BTreeMap<_, _> = meshes.iter().enumerate().map(|(i, m)| (m.id, i)).collect();
        if sources.len() != meshes.len() {
            return fail(0, "duplicate mesh source ID");
        }
        let shaders = (0..tree.nodes.len())
            .filter(|&i| tree.name(i) == b"SHADERINSTANCE")
            .map(|i| tree.string(i, b"id"))
            .collect::<Result<_>>()?;
        Ok(Self { sources, shaders })
    }
}

/// Which RENDERSTREAMINSTANCEs a scene read takes.
#[derive(Clone, Copy, PartialEq)]
enum Subset {
    /// Under RENDERNODEs (track, renderables).
    Static,
    /// The object library's closest level.
    Closest,
    /// Every object-library level.
    Levels,
}

fn read_scene_draws<'a>(
    tree: &Tree<'a>,
    meshes: &[Mesh<'a>],
    subset: Subset,
    selected: Option<usize>,
) -> Result<Vec<Draw<'a>>> {
    let context = Context::new(tree, meshes)?;
    let mut result = Vec::new();
    for i in 0..tree.nodes.len() {
        if tree.name(i) != b"RENDERSTREAMINSTANCE" {
            continue;
        }
        if let Some(draw) = instance_draw(tree, meshes, &context, i, subset, selected)? {
            result.push(draw);
        }
    }
    if result.is_empty() && selected.is_none() {
        return fail(0, "no supported static draws");
    }
    Ok(result)
}

/// One RENDERSTREAMINSTANCE `i` (under `selected`, when given): its draw,
/// `None` when the object subset skips it, or the error that fails the read.
fn instance_draw<'a>(
    tree: &Tree<'a>,
    meshes: &[Mesh<'a>],
    context: &Context<'a>,
    i: usize,
    subset: Subset,
    selected: Option<usize>,
) -> Result<Option<Draw<'a>>> {
    let Context { sources, shaders } = context;
    {
        // The same element also occurs inside car palettes; reject that context.
        let parent = tree.nodes[i].parent.ok_or(crate::Error {
            offset: tree.nodes[i].offset,
            message: "static draw parent absent",
        })?;
        let lod_list = tree.name(parent) == b"LODRENDERINSTANCELIST";
        let object_subset = subset != Subset::Static;
        if object_subset
            && tree.name(parent) != b"LODVISIBLERENDERNODE"
            && !(subset == Subset::Levels && lod_list)
        {
            return Ok(None);
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
                b"RENDERNODE" | b"LODVISIBLERENDERNODE" | b"NODE" | b"ROOTNODE" | b"BUNDLENODE" => {
                    transform = compose(&transform, &matrix(tree, a)?);
                    if tree.name(a) == b"ROOTNODE" {
                        root_seen = true;
                        prototype = Some(tree.string(a, b"id")?);
                    }
                }
                b"LIBRARY" | b"PSSGDATABASE" | b"ROOT" => {}
                // A lower level's lists carry no transform.
                b"LODRENDERINSTANCES" | b"LODRENDERINSTANCELIST" if lod_list => {}
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
        Ok(Some(Draw {
            id: tree.string(i, b"id")?,
            // A lower level's draws name their LODVISIBLERENDERNODE.
            joint: tree.string(
                if lod_list {
                    tree.nodes[parent]
                        .parent
                        .and_then(|l| tree.nodes[l].parent)
                        .unwrap_or(parent)
                } else {
                    parent
                },
                b"id",
            )?,
            material,
            source,
            prototype,
            vertices: 0..meshes[source].positions.len(),
            indices: meshes[source].indices.clone(),
            transform,
        }))
    }
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
/// An INVERSEBINDMATRIX: affine up to float noise (the tyre walls' carry
/// -0.0 and values near 1e-8 in the last column), which is cleared.
fn decode_bind_matrix(data: &[u8], offset: usize) -> Result<[f32; 16]> {
    if data.len() != 64 {
        return fail(offset, "expected 16-float bind matrix");
    }
    let mut m = [0.0; 16];
    for (value, bytes) in m.iter_mut().zip(data.as_chunks::<4>().0) {
        *value = f32::from_be_bytes(*bytes);
    }
    if m.iter().any(|v| !v.is_finite())
        || [m[3], m[7], m[11], m[15] - 1.].iter().any(|v| v.abs() > 1e-4)
    {
        return fail(offset, "only affine bind matrices supported");
    }
    [m[3], m[7], m[11], m[15]] = [0., 0., 0., 1.];
    Ok(m)
}
/// A SKINNODE drawn with its joints (`characters/models/driver_model.pssg`).
pub struct Skin<'a> {
    /// The SKINJOINTs' JOINTNODEs, in skin order (a vertex's joint numbers
    /// index this list).
    pub joints: Vec<usize>,
    /// Each joint's INVERSEBINDMATRIX: bind space to the joint.
    pub inverse_bind: Vec<[f32; 16]>,
    /// The `skinTransformLocal` instances: id, shader id and mesh source.
    pub instances: Vec<(&'a [u8], &'a [u8], usize)>,
}

/// The SKINNODE `id` of `tree`, with `meshes` from
/// [`crate::mesh::read_object_meshes`]; every instance needs its joints and
/// weights.
pub fn read_skin<'a>(tree: &Tree<'a>, meshes: &[Mesh<'a>], id: &[u8]) -> Result<Skin<'a>> {
    let named = |kind: &[u8], id: &[u8]| {
        (0..tree.nodes.len()).find(|&i| tree.name(i) == kind && tree.string(i, b"id").ok() == Some(id))
    };
    let Some(skin) = named(b"SKINNODE", id) else {
        return fail(0, "skin node absent");
    };
    let skeleton = tree.string(skin, b"skeleton")?.strip_prefix(b"#").unwrap_or_default();
    let Some(skeleton) = named(b"SKELETON", skeleton) else {
        return fail(tree.nodes[skin].offset, "unresolved skin skeleton");
    };
    let inverse_bind: Vec<[f32; 16]> = (tree.nodes[skeleton].children.iter())
        .filter(|&&c| tree.name(c) == b"INVERSEBINDMATRIX")
        .map(|&c| decode_bind_matrix(tree.nodes[c].data.unwrap_or_default(), tree.nodes[c].offset))
        .collect::<Result<_>>()?;
    let mut joints = Vec::new();
    let mut instances = Vec::new();
    for &c in &tree.nodes[skin].children {
        match tree.name(c) {
            b"SKINJOINT" => {
                let joint = tree.string(c, b"joint")?.strip_prefix(b"#").unwrap_or_default();
                let Some(joint) = named(b"JOINTNODE", joint) else {
                    return fail(tree.nodes[c].offset, "unresolved skin joint");
                };
                joints.push(joint);
            }
            b"MODIFIERNETWORKINSTANCE" if tree.string(c, b"network")?.ends_with(b"#skinTransformLocal") => {
                let reference = tree.string(c, b"indices")?.strip_prefix(b"#").unwrap_or_default();
                let Some(source) = meshes.iter().position(|m| m.id == reference) else {
                    return fail(tree.nodes[c].offset, "unresolved skin source");
                };
                let Some((bound, _)) = &meshes[source].skin_weights else {
                    return fail(tree.nodes[c].offset, "skinned source without joints");
                };
                if bound.iter().flatten().any(|&j| j as usize >= inverse_bind.len()) {
                    return fail(tree.nodes[c].offset, "skin joint number out of range");
                }
                let shader = tree.string(c, b"shader")?.strip_prefix(b"#").unwrap_or_default();
                instances.push((tree.string(c, b"id")?, shader, source));
            }
            _ => {}
        }
    }
    if joints.len() != inverse_bind.len() || joints.len() != tree.integer(skin, b"jointCount")? as usize {
        return fail(tree.nodes[skin].offset, "skin joint and bind matrix counts differ");
    }
    Ok(Skin { joints, inverse_bind, instances })
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

/// Compose one ordinary scene node through its affine NODE/RENDERNODE/ROOTNODE
/// ancestry (grid slots can be RENDERNODEs: Haruna route 9, garages).
pub fn node_matrix(tree: &Tree<'_>, node: usize) -> Result<[f32; 16]> {
    if node >= tree.nodes.len() {
        return fail(node, "scene node index outside tree");
    }
    let mut transform = IDENTITY;
    let mut ancestor = Some(node);
    let mut root = false;
    while let Some(i) = ancestor {
        match tree.name(i) {
            b"NODE" | b"RENDERNODE" | b"ROOTNODE" => {
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

#[cfg(test)]
mod renderables_tests {
    use super::*;

    /// The pre-F445 per-id read: one whole-tree scan per renderable id.
    fn one_scan<'a>(tree: &Tree<'a>, meshes: &[Mesh<'a>], id: &[u8]) -> Result<Vec<Draw<'a>>> {
        let selected: Vec<_> = (0..tree.nodes.len())
            .filter(|&i| {
                tree.name(i) == b"NODE" && tree.string(i, b"id").is_ok_and(|name| name == id)
            })
            .collect();
        if selected.len() != 1 {
            return fail(0, "expected one selected renderable node");
        }
        // The old loop: every RENDERSTREAMINSTANCE of the tree, kept when
        // its ancestor chain holds the selected node.
        let context = Context::new(tree, meshes)?;
        let mut result = Vec::new();
        for i in 0..tree.nodes.len() {
            if tree.name(i) != b"RENDERSTREAMINSTANCE" {
                continue;
            }
            let mut ancestor = tree.nodes[i].parent;
            let mut found = false;
            while let Some(a) = ancestor {
                if a == selected[0] {
                    found = true;
                    break;
                }
                ancestor = tree.nodes[a].parent;
            }
            if found
                && let Some(draw) =
                    instance_draw(tree, meshes, &context, i, Subset::Static, Some(selected[0]))?
            {
                result.push(draw);
            }
        }
        Ok(result)
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT"]
    fn renderables_match_one_scan_per_id_on_the_nurburgring_objects() {
        let root = std::path::PathBuf::from(std::env::var("GRID_GAME_ROOT").unwrap());
        let bytes = std::fs::read(root.join("tracks/europe/nurburgring/objects.pssg")).unwrap();
        let tree = crate::pssg::read_tree(&bytes).unwrap();
        let meshes = crate::mesh::read_static_meshes(&tree).unwrap();
        let index = Renderables::new(&tree, &meshes).unwrap();
        let ids: Vec<&[u8]> = (0..tree.nodes.len())
            .filter(|&i| tree.name(i) == b"NODE")
            .filter_map(|i| tree.string(i, b"id").ok())
            .collect();
        assert!(ids.len() > 500, "{}", ids.len());
        // Every node id once (the app asks per breakable piece, 11,962 times), plus
        // an id that is not there.
        let mut checked = 0;
        for id in ids.iter().copied().chain([&b"no such node"[..]]) {
            let (a, b) = (index.draws(id), one_scan(&tree, &meshes, id));
            assert_eq!(
                a.as_ref().map(Vec::len).map_err(|e| e.message),
                b.as_ref().map(Vec::len).map_err(|e| e.message),
                "{}",
                String::from_utf8_lossy(id)
            );
            if let (Ok(a), Ok(b)) = (a, b) {
                assert_eq!(a.len(), b.len());
                for (x, y) in a.iter().zip(&b) {
                    assert_eq!(
                        (x.id, x.joint, x.material, x.source, x.prototype),
                        (y.id, y.joint, y.material, y.source, y.prototype)
                    );
                    assert_eq!(x.transform.map(f32::to_bits), y.transform.map(f32::to_bits));
                    assert_eq!((&x.vertices, &x.indices), (&y.vertices, &y.indices));
                }
                checked += 1;
            }
        }
        assert!(checked > 100, "{checked}");
    }
}
