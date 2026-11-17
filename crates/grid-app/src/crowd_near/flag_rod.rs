//! Source rigid flag rods and their matching posed hand attachments.
use super::AnyResult;
use crate::{crowd_preview::unique_texture, graphics, texture_upload};
use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::NoFrustumCulling,
    light::NotShadowCaster,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use grid_formats::{binxml, material, mesh, pssg, scene, texture};
use grid_tools::crowd_flags::Role;

const PIVOT: &str = "RH_Prop_Helper";
const MATRIX_ERROR: f32 = 1e-4;

#[derive(PartialEq)]
struct Geometry {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
    indices: Vec<u16>,
    diffuse: Vec<u8>,
}

struct Attachment {
    geometry: usize,
    rod_to_pivot: Mat4,
    pin_to_pivot: [Vec3; 2],
}

pub(in crate::crowd_billboards) struct Rods {
    geometry: Vec<Geometry>,
    attachment: [Attachment; 3],
}

pub(in crate::crowd_billboards) struct Draws(Vec<(Handle<Mesh>, Handle<StandardMaterial>)>);

fn attr<'a>(element: &binxml::Element<'a>, name: &str) -> AnyResult<&'a str> {
    let mut values = element.attributes.iter().filter(|(key, _)| *key == name);
    let (_, value) = values.next().ok_or("missing rigid flag prop field")?;
    if values.next().is_some() || value.is_empty() {
        return Err("ambiguous or empty rigid flag prop field".into());
    }
    Ok(*value)
}

fn exact_node(tree: &pssg::Tree<'_>, kind: &[u8], name: &[u8]) -> AnyResult<usize> {
    let mut ids = (0..tree.nodes.len())
        .filter(|&i| tree.name(i) == kind && tree.string(i, b"id").ok() == Some(name));
    let id = ids.next().ok_or("missing rigid flag source node")?;
    if ids.next().is_some() {
        return Err("ambiguous rigid flag source node".into());
    }
    Ok(id)
}

fn exact_child(tree: &pssg::Tree<'_>, parent: usize, kind: &[u8], name: &[u8]) -> AnyResult<usize> {
    let child = exact_node(tree, kind, name)?;
    if tree.nodes[child].parent != Some(parent) {
        return Err("rigid flag node is outside its authored prop".into());
    }
    Ok(child)
}

fn prop_name(role: Role) -> &'static str {
    // Native prop enum slots 3/1/4 name these nodes in the authored bank.
    match role {
        Role::Start => "startflag",
        Role::Danger => "dangerflag",
        Role::Finish => "finishflag",
    }
}

fn affine(values: [f32; 16]) -> AnyResult<Mat4> {
    let matrix = Mat4::from_cols_array(&values);
    let determinant = matrix.determinant();
    if !matrix.is_finite()
        || [values[3], values[7], values[11], values[15]] != [0., 0., 0., 1.]
        || !determinant.is_finite()
        || determinant <= 0.
    {
        return Err("invalid rigid flag transform".into());
    }
    Ok(matrix)
}

fn max_error(a: Mat4, b: Mat4) -> f32 {
    a.to_cols_array()
        .into_iter()
        .zip(b.to_cols_array())
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f32::max)
}

fn child_transform(rod_to_pivot: Mat4, hand: [f32; 16]) -> AnyResult<Transform> {
    // PSSG stores row-vector matrices; Bevy reads the same arrays as columns.
    let matrix = affine(hand)? * rod_to_pivot;
    let output = Transform::from_matrix(affine(matrix.to_cols_array())?);
    let reconstructed = output.to_matrix();
    if !reconstructed.is_finite() || max_error(matrix, reconstructed) > MATRIX_ERROR {
        return Err("rigid flag transform cannot be represented as Bevy TRS".into());
    }
    Ok(output)
}

impl Rods {
    pub(in crate::crowd_billboards) fn read(model: &[u8], metadata: &[u8]) -> AnyResult<Self> {
        let metadata = binxml::read(metadata)?;
        let root = metadata
            .elements
            .first()
            .filter(|node| node.name == "props")
            .ok_or("missing source prop definitions")?;
        let top = metadata
            .elements
            .get(root.children.clone())
            .ok_or("invalid prop child range")?;
        let mut sections = top.iter().filter(|node| node.name == "clothprops");
        let section = sections.next().ok_or("missing flag prop definitions")?;
        if sections.next().is_some() {
            return Err("ambiguous flag prop definitions".into());
        }
        let records = metadata
            .elements
            .get(section.children.clone())
            .ok_or("invalid flag prop child range")?;
        let tree = pssg::read_tree(model)?;
        let catalogue = mesh::read_static_meshes(&tree)?;
        let renderables = scene::Renderables::new(&tree, &catalogue)?;
        let bindings = material::read_texture_bindings(&tree)?;
        let mut geometries: Vec<Geometry> = Vec::new();
        let mut attachments = Vec::new();
        for role in [Role::Start, Role::Danger, Role::Finish] {
            let name = prop_name(role);
            let mut matching = records
                .iter()
                .filter(|n| n.name == "prop" && attr(n, "nodename").ok() == Some(name));
            let record = matching
                .next()
                .ok_or("missing authored marshal flag prop")?;
            if matching.next().is_some() || attr(record, "pivotnode")? != PIVOT {
                return Err("ambiguous flag prop or unsupported hand pivot".into());
            }
            let prop = exact_node(&tree, b"NODE", name.as_bytes())?;
            let pivot = exact_node(&tree, b"NODE", attr(record, "pivotnode")?.as_bytes())?;
            let inverse_pivot = affine(scene::node_matrix(&tree, pivot)?)?.inverse();

            // The authored cloth surface is a sibling of this rigid subtree.
            let draws = renderables.draws(name.as_bytes())?;
            let [draw] = draws.as_slice() else {
                return Err("expected one authored rigid rod draw".into());
            };
            let joint = exact_child(&tree, prop, b"RENDERNODE", draw.joint)?;
            let rod_to_pivot = inverse_pivot * affine(scene::node_matrix(&tree, joint)?)?;
            let shader = exact_node(&tree, b"SHADERINSTANCE", draw.material)?;
            if tree.string(shader, b"shaderGroup")? != b"#props_simple.fx" {
                return Err("unsupported rigid rod shader".into());
            }
            let textures: Vec<_> = bindings
                .iter()
                .filter(|binding| binding.shader == draw.material)
                .collect();
            let [binding] = textures.as_slice() else {
                return Err("ambiguous rigid rod texture binding".into());
            };
            if binding.container.is_some() {
                return Err("external rod texture container unsupported".into());
            }
            let source = catalogue.get(draw.source).ok_or("rod mesh source absent")?;
            let n = source.positions.len();
            if n == 0
                || draw.vertices != (0..n)
                || source.normals.len() != n
                || source.texcoords.len() != n
                || source
                    .positions
                    .iter()
                    .chain(&source.normals)
                    .flatten()
                    .any(|x| !x.is_finite())
                || source.texcoords.iter().flatten().any(|x| !x.is_finite())
                || draw.indices.is_empty()
                || !draw.indices.len().is_multiple_of(3)
                || draw.indices.iter().any(|&i| usize::from(i) >= n)
            {
                return Err("invalid rigid flag source mesh".into());
            }
            let tex = texture::read_named(&tree, &[binding.texture])?;
            let diffuse = unique_texture(&tex, binding.texture)?.validated_dds().0;
            let geometry = Geometry {
                positions: source.positions.clone(),
                normals: source.normals.clone(),
                uv: source.texcoords.clone(),
                indices: draw.indices.clone(),
                diffuse,
            };
            let geometry = if let Some(index) = geometries.iter().position(|old| *old == geometry) {
                index
            } else {
                let index = geometries.len();
                geometries.push(geometry);
                index
            };

            let children = metadata
                .elements
                .get(record.children.clone())
                .ok_or("invalid flag pin range")?;
            let pin = |label| -> AnyResult<Vec3> {
                let mut choices = children.iter().filter(|node| node.name == label);
                let one = choices.next().ok_or("missing authored flag pin")?;
                if choices.next().is_some() {
                    return Err("ambiguous authored flag pin".into());
                }
                let pin = exact_child(&tree, prop, b"NODE", attr(one, "node")?.as_bytes())?;
                Ok(inverse_pivot
                    .transform_point3(affine(scene::node_matrix(&tree, pin)?)?.w_axis.truncate()))
            };
            attachments.push(Attachment {
                geometry,
                rod_to_pivot,
                pin_to_pivot: [pin("attachpoint1")?, pin("attachpoint2")?],
            });
        }
        let attachment = attachments
            .try_into()
            .map_err(|_| "missing prepared role attachment")?;
        Ok(Self {
            geometry: geometries,
            attachment,
        })
    }

    fn attachment(&self, role: Role) -> &Attachment {
        &self.attachment[role as usize - 1]
    }

    pub(in crate::crowd_billboards) fn pose(
        &self,
        role: Role,
        hand: [f32; 16],
    ) -> AnyResult<Transform> {
        child_transform(self.attachment(role).rod_to_pivot, hand)
    }

    pub(in crate::crowd_billboards) fn install(
        &self,
        settings: &graphics::Settings,
        meshes: &mut Assets<Mesh>,
        images: &mut Assets<Image>,
        materials: &mut Assets<StandardMaterial>,
    ) -> AnyResult<Draws> {
        let mut draws = Vec::new();
        for geometry in &self.geometry {
            let image = texture_upload::image(
                &geometry.diffuse,
                settings.sampler(false),
                RenderAssetUsages::default(),
            )?;
            let mut mesh = Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::default(),
            );
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, geometry.positions.clone());
            mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, geometry.normals.clone());
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, geometry.uv.clone());
            mesh.insert_indices(Indices::U16(geometry.indices.clone()));
            draws.push((
                meshes.add(mesh),
                materials.add(StandardMaterial {
                    base_color_texture: Some(images.add(image)),
                    perceptual_roughness: 1.0,
                    reflectance: 0.0,
                    ..default()
                }),
            ));
        }
        Ok(Draws(draws))
    }

    /// The body and attachment use the same admitted actor and posed sample.
    pub(in crate::crowd_billboards) fn child(
        &self,
        draws: &Draws,
        role: Role,
        body: Entity,
        transform: Transform,
    ) -> impl Bundle + 'static {
        let (mesh, material) = &draws.0[self.attachment(role).geometry];
        (
            Name::new("crowd/source-flag-rod"),
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            ChildOf(body),
            transform,
            NotShadowCaster,
            NoFrustumCulling,
        )
    }

    pub(in crate::crowd_billboards) fn check_pose(
        &self,
        role: Role,
        hand: [f32; 16],
        body: Transform,
    ) -> AnyResult<()> {
        let attachment = self.attachment(role);
        let hand_matrix = affine(hand)?;
        let authored = body.to_matrix() * hand_matrix * attachment.rod_to_pivot;
        let presented = body.to_matrix() * self.pose(role, hand)?.to_matrix();
        let [a, b] = attachment.pin_to_pivot;
        let span = (body.to_matrix() * hand_matrix)
            .transform_vector3(b - a)
            .length();
        if max_error(authored, presented) > MATRIX_ERROR
            || !span.is_finite()
            || (b - a).length() <= f32::EPSILON
            || (hand_matrix.transform_vector3(b - a).length() - (b - a).length()).abs()
                > MATRIX_ERROR
        {
            return Err("invalid presented flag attachment geometry".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn child_uses_hand_before_body_and_refuses_non_trs_shear() {
        let hand = Mat4::from_rotation_x(0.4) * Mat4::from_translation(Vec3::new(1., 2., 3.));
        let rest = Mat4::from_rotation_y(0.3) * Mat4::from_translation(Vec3::new(3., 1., 2.));
        let chosen = child_transform(rest, hand.to_cols_array())
            .unwrap()
            .to_matrix();
        assert!(max_error(chosen, hand * rest) < MATRIX_ERROR);
        assert!(max_error(chosen, rest * hand) > 0.01);
        let mut shear = hand.to_cols_array();
        shear[1] += 0.25;
        assert!(child_transform(rest, shear).is_err());
    }
}
