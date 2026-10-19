//! Declarative startup components from ENS placements and an objecttype library.
//! This selects the explicit initial behaviour; it does not simulate behaviour.
use crate::{Result, fail, pssg::Tree, scene};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug)]
pub struct Entity<'a> {
    pub id: &'a [u8],
    pub template: &'a [u8],
    pub transform: [f32; 16],
    pub renderables: Vec<Renderable<'a>>,
    pub cloth: Vec<&'a [u8]>,
    pub links: usize,
}
#[derive(Debug)]
pub struct Renderable<'a> {
    pub component: &'a [u8],
    pub container: &'a [u8],
    pub node: &'a [u8],
    pub body_node: &'a [u8],
    pub body: &'a [u8],
    pub body_properties: &'a [u8],
    pub infinite_mass: bool,
    pub shapes: Vec<Shape<'a>>,
}
#[derive(Debug)]
pub struct Shape<'a> {
    pub id: &'a [u8],
    pub primitive: Primitive,
    pub transform: [f32; 16],
    pub shell: f32,
    pub phantom: bool,
    pub properties: &'a [u8],
    /// Shape-level damage behaviour, when the shape carries one.
    pub behaviour: Option<ShapeBehaviour<'a>>,
}
/// A collision shape's `TEMPLATEBEHAVIOUR`: its threshold properties and the
/// components its damage behaviour type changes (DYNAMIC_PROPS_RESEARCH.md).
#[derive(Debug)]
pub struct ShapeBehaviour<'a> {
    /// Qualified behaviour properties (`physparams.xml#bXxx`).
    pub properties: &'a [u8],
    pub behaviour_type: &'a [u8],
    /// Subentity component IDs the behaviour type deletes.
    pub deleted: Vec<&'a [u8]>,
    /// Subentities the behaviour type adds (their own shape behaviours are
    /// not resolved).
    pub added: Vec<Renderable<'a>>,
    /// Constraint components the behaviour type adds, in its change order.
    pub constraints: Vec<Constraint<'a>>,
}
/// A `TEMPLATECONSTRAINT` (the template reader 0x7CF4B0): the joint between
/// two rigid bodies of the template, its properties and its frame in the
/// entity.
#[derive(Debug)]
pub struct Constraint<'a> {
    pub id: &'a [u8],
    /// `#id` of a template rigid body (`body1uri`; "none" is an error in the
    /// original).
    pub body1: &'a [u8],
    /// `#id`, `none`/`world` (the world) or `link1` (the linked entity's body).
    pub body2: &'a [u8],
    /// Qualified constraint properties (`physparams.xml#cXxx`).
    pub properties: &'a [u8],
    /// `TEMPLATETRANSFORM` as read (the reader renormalises it).
    pub transform: [f32; 16],
    /// `display` (+0x140): a non-empty value.
    pub display: bool,
}
#[derive(Debug)]
pub enum Primitive {
    Box {
        width: f32,
        height: f32,
        length: f32,
    },
    Cylinder {
        radius: f32,
        height: f32,
    },
    /// `TEMPLATESHAPESPHERE` (Milan and Detroit `col_sphere_*` balloons, `sBalloon`).
    Sphere {
        radius: f32,
    },
}

fn index<'a>(tree: &Tree<'a>) -> Result<BTreeMap<&'a [u8], usize>> {
    let id_attributes: BTreeSet<_> = tree
        .framing
        .schema
        .iter()
        .flat_map(|e| &e.attributes)
        .filter(|a| a.name == b"id")
        .map(|a| a.id)
        .collect();
    let mut result = BTreeMap::new();
    for (i, node) in tree.nodes.iter().enumerate() {
        if node
            .attributes
            .iter()
            .any(|a| id_attributes.contains(&a.id))
        {
            let id = tree.string(i, b"id")?;
            if id.is_empty() || result.insert(id, i).is_some() {
                return fail(node.offset, "empty or duplicate entity-library ID");
            }
        }
    }
    Ok(result)
}
fn local(tree: &Tree<'_>, ids: &BTreeMap<&[u8], usize>, uri: &[u8], kind: &[u8]) -> Result<usize> {
    let target = uri
        .strip_prefix(b"#")
        .and_then(|id| ids.get(id))
        .copied()
        .ok_or(crate::Error {
            offset: 0,
            message: "unresolved local entity reference",
        })?;
    if tree.name(target) != kind {
        return fail(tree.nodes[target].offset, "entity reference kind mismatch");
    }
    Ok(target)
}
fn one(tree: &Tree<'_>, node: usize, name: &[u8]) -> Result<usize> {
    let children: Vec<_> = tree.nodes[node]
        .children
        .iter()
        .copied()
        .filter(|&i| tree.name(i) == name)
        .collect();
    if children.len() != 1 {
        return fail(tree.nodes[node].offset, "expected one typed entity child");
    }
    Ok(children[0])
}
fn transform(tree: &Tree<'_>, node: usize) -> Result<[f32; 16]> {
    let child = one(tree, node, b"TEMPLATETRANSFORM")?;
    scene::decode_matrix(
        tree.nodes[child].data.ok_or(crate::Error {
            offset: tree.nodes[child].offset,
            message: "template transform payload absent",
        })?,
        tree.nodes[child].offset,
    )
}
fn scalar(tree: &Tree<'_>, node: usize, name: &[u8], positive: bool) -> Result<f32> {
    let value = f32::from_bits(tree.integer(node, name)?);
    if !value.is_finite() || (positive && value <= 0.) || (!positive && value < 0.) {
        return fail(tree.nodes[node].offset, "invalid collision dimension");
    }
    Ok(value)
}
fn boolean(tree: &Tree<'_>, node: usize, name: &[u8]) -> Result<bool> {
    match tree.integer(node, name)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => fail(tree.nodes[node].offset, "invalid entity boolean"),
    }
}

pub fn read<'a>(placements: &Tree<'a>, types: &Tree<'a>) -> Result<Vec<Entity<'a>>> {
    let placement_ids = index(placements)?;
    let type_ids = index(types)?;
    let mut entities = Vec::new();
    for i in 0..placements.nodes.len() {
        if placements.name(i) != b"TEMPLATEENTITYINSTANCE" {
            continue;
        }
        let reference = local(
            placements,
            &placement_ids,
            placements.string(i, b"uri")?,
            b"TEMPLATEENTITYREFERENCE",
        )?;
        let template_uri = placements.string(reference, b"uri")?;
        let template_id = template_uri
            .strip_prefix(b"objecttypes.pssg#")
            .ok_or(crate::Error {
                offset: placements.nodes[reference].offset,
                message: "unsupported entity template container",
            })?;
        let template = *type_ids.get(template_id).ok_or(crate::Error {
            offset: placements.nodes[reference].offset,
            message: "unresolved entity template",
        })?;
        if types.name(template) != b"TEMPLATEENTITY" {
            return fail(
                types.nodes[template].offset,
                "template reference kind mismatch",
            );
        }
        let initial = one(types, template, b"TEMPLATEBEHAVIOUR")?;
        let behaviour = local(
            types,
            &type_ids,
            types.string(initial, b"typeuri")?,
            b"TEMPLATEBEHAVIOURTYPE",
        )?;
        let mut active = BTreeSet::new();
        for &change in &types.nodes[behaviour].children {
            if types.name(change) != b"TEMPLATECOMPONENTCHANGE" {
                return fail(
                    types.nodes[change].offset,
                    "unsupported initial behaviour action",
                );
            }
            let uri = types.string(change, b"componenturi")?;
            let component = uri
                .strip_prefix(b"#")
                .and_then(|id| type_ids.get(id))
                .copied()
                .ok_or(crate::Error {
                    offset: types.nodes[change].offset,
                    message: "unresolved behaviour component",
                })?;
            if types.nodes[component].parent != Some(template) {
                return fail(
                    types.nodes[change].offset,
                    "behaviour component outside template",
                );
            }
            match types.string(change, b"operation")? {
                b"add" => {
                    active.insert(component);
                }
                b"delete" => {
                    active.remove(&component);
                }
                _ => {
                    return fail(
                        types.nodes[change].offset,
                        "unsupported component operation",
                    );
                }
            }
        }
        let mut entity = Entity {
            id: placements.string(i, b"id")?,
            template: template_id,
            transform: transform(placements, i)?,
            renderables: Vec::new(),
            cloth: Vec::new(),
            links: 0,
        };
        for &child in &placements.nodes[i].children {
            match placements.name(child) {
                b"TEMPLATETRANSFORM" => {}
                b"TEMPLATEENTITYLINK" => entity.links += 1,
                _ => {
                    return fail(
                        placements.nodes[child].offset,
                        "unsupported entity-instance child",
                    );
                }
            }
        }
        for component in active {
            if types.name(component) == b"TEMPLATECLOTH" {
                entity.cloth.push(types.string(component, b"id")?);
                continue;
            }
            if types.name(component) != b"TEMPLATESUBENTITY" {
                return fail(
                    types.nodes[component].offset,
                    "unsupported active component kind",
                );
            }
            entity
                .renderables
                .push(subentity(types, &type_ids, template, component, true)?);
        }
        entities.push(entity);
    }
    if entities.is_empty() {
        return fail(0, "no entity placements");
    }
    Ok(entities)
}

/// One subentity's renderable, rigid body and collision shapes. With
/// `behaviours`, shape-level damage behaviours are resolved one level deep.
fn subentity<'a>(
    types: &Tree<'a>,
    type_ids: &BTreeMap<&'a [u8], usize>,
    template: usize,
    component: usize,
    behaviours: bool,
) -> Result<Renderable<'a>> {
    let transformable = one(types, component, b"TEMPLATETRANSFORMABLE")?;
    if !types.nodes[transformable].children.is_empty() {
        return fail(
            types.nodes[transformable].offset,
            "unhandled transformable child",
        );
    }
    let renderable = local(
        types,
        type_ids,
        types.string(transformable, b"uri")?,
        b"TEMPLATERENDERABLE",
    )?;
    let uri = types.string(renderable, b"uri")?;
    let split = uri.iter().position(|&v| v == b'#').ok_or(crate::Error {
        offset: types.nodes[renderable].offset,
        message: "unqualified renderable URI",
    })?;
    if split == 0 || split + 1 == uri.len() || uri[split + 1..].contains(&b'#') {
        return fail(types.nodes[renderable].offset, "invalid renderable URI");
    }
    let body = one(types, component, b"TEMPLATERIGIDBODY")?;
    let mut result = Renderable {
        component: types.string(component, b"id")?,
        container: &uri[..split],
        node: &uri[split + 1..],
        body_node: types.string(transformable, b"sid")?,
        body: types.string(body, b"id")?,
        body_properties: types.string(body, b"props")?,
        infinite_mass: boolean(types, body, b"infMass")?,
        shapes: Vec::new(),
    };
    for &child in &types.nodes[body].children {
        if types.name(child) == b"TEMPLATELINKPOINT" {
            continue;
        }
        if types.name(child) != b"TEMPLATERIGIDBODYSHAPE" {
            return fail(types.nodes[child].offset, "unsupported rigid-body child");
        }
        let uri = types.string(child, b"uri")?;
        let target = uri
            .strip_prefix(b"#")
            .and_then(|id| type_ids.get(id))
            .copied()
            .ok_or(crate::Error {
                offset: types.nodes[child].offset,
                message: "unresolved collision shape",
            })?;
        let primitive = match types.name(target) {
            b"TEMPLATESHAPEBOX" => Primitive::Box {
                width: scalar(types, target, b"width", true)?,
                height: scalar(types, target, b"height", true)?,
                length: scalar(types, target, b"length", true)?,
            },
            b"TEMPLATESHAPECYLINDER" => Primitive::Cylinder {
                radius: scalar(types, target, b"radius", true)?,
                height: scalar(types, target, b"height", true)?,
            },
            b"TEMPLATESHAPESPHERE" => Primitive::Sphere {
                radius: scalar(types, target, b"radius", true)?,
            },
            _ => {
                return fail(
                    types.nodes[target].offset,
                    "unsupported collision primitive",
                );
            }
        };
        result.shapes.push(Shape {
            id: types.string(target, b"id")?,
            primitive,
            transform: transform(types, child)?,
            shell: scalar(types, target, b"shell", false)?,
            phantom: boolean(types, target, b"phantom")?,
            properties: types.string(target, b"props")?,
            behaviour: if behaviours {
                shape_behaviour(types, type_ids, template, child)?
            } else {
                None
            },
        });
    }
    Ok(result)
}

fn shape_behaviour<'a>(
    types: &Tree<'a>,
    type_ids: &BTreeMap<&'a [u8], usize>,
    template: usize,
    shape: usize,
) -> Result<Option<ShapeBehaviour<'a>>> {
    let nodes: Vec<_> = types.nodes[shape]
        .children
        .iter()
        .copied()
        .filter(|&i| types.name(i) == b"TEMPLATEBEHAVIOUR")
        .collect();
    let node = match nodes[..] {
        [] => return Ok(None),
        [node] => node,
        _ => return fail(types.nodes[shape].offset, "several shape behaviours"),
    };
    let behaviour_type = types.string(node, b"typeuri")?;
    let kind = local(types, type_ids, behaviour_type, b"TEMPLATEBEHAVIOURTYPE")?;
    let mut result = ShapeBehaviour {
        properties: types.string(node, b"impulse")?,
        behaviour_type,
        deleted: Vec::new(),
        added: Vec::new(),
        constraints: Vec::new(),
    };
    for &change in &types.nodes[kind].children {
        if types.name(change) != b"TEMPLATECOMPONENTCHANGE" {
            return fail(types.nodes[change].offset, "unsupported behaviour action");
        }
        let id = types
            .string(change, b"componenturi")?
            .strip_prefix(b"#")
            .ok_or(crate::Error {
                offset: types.nodes[change].offset,
                message: "unqualified behaviour component",
            })?;
        let component = *type_ids.get(id).ok_or(crate::Error {
            offset: types.nodes[change].offset,
            message: "unresolved behaviour component",
        })?;
        if types.nodes[component].parent != Some(template) {
            return fail(
                types.nodes[change].offset,
                "behaviour component outside template",
            );
        }
        match (types.string(change, b"operation")?, types.name(component)) {
            (b"delete", _) => result.deleted.push(id),
            (b"add", b"TEMPLATESUBENTITY") => {
                result
                    .added
                    .push(subentity(types, type_ids, template, component, false)?);
            }
            (b"add", b"TEMPLATECONSTRAINT") => result.constraints.push(Constraint {
                id,
                body1: types.string(component, b"body1uri")?,
                body2: types.string(component, b"body2uri")?,
                properties: types.string(component, b"props")?,
                transform: transform(types, component)?,
                display: types
                    .attribute(component, b"display")
                    .is_ok_and(|v| v.iter().any(|&b| b != 0)),
            }),
            _ => {
                return fail(
                    types.nodes[change].offset,
                    "unsupported damage component change",
                );
            }
        }
    }
    Ok(Some(result))
}
