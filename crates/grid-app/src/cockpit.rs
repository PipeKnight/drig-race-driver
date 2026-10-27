//! The player's cockpit (F618, docs/CAMERA_VIEWS.md "Own car per view"):
//! `interiors/<stem>/int_<stem>.pssg`, loaded for the player only as the
//! original's race loader does (0x6138D0, `n12` when the car has none),
//! placed on the model frame like the body (its Scene Root is the identity)
//! and drawn through the car's material path. Which parts of the player car
//! a view draws is [`crate::camera::OwnCar`].
use super::{Car, Part, read};
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use grid_formats::{interior, material, mesh, pssg, scene, texture};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    path::Path,
};

/// Joint prefix of the cockpit's parts (each part's RENDERNODE follows).
pub const JOINT: &str = "interior/";

/// The exterior nodes the cockpit view keeps (`int_<stem>.xml`), without
/// their `x0_` prefix as `damage_parts::Joint` holds them.
#[derive(Resource, Clone, Debug, Default)]
pub struct Cockpit {
    pub active: Vec<String>,
}

/// A cockpit part: drawn on its own, never merged into the car's batches.
#[derive(Component)]
pub struct InteriorPart;

/// Appends the cockpit of the player's model `stem` to `car` (material and
/// texture keys under `car0/interior/`), placed by `anchor` like the body's
/// parts. Shader groups other than interior.fx, interior_alpha.fx and
/// interior_windscreen.fx (the LCD and motec displays, the mirror) are not
/// drawn yet.
pub fn append(root: &Path, stem: &str, car: &mut Car, anchor: Mat4) -> Result<Cockpit, Box<dyn Error>> {
    let file = |stem: &str, ext: &str| root.join(format!("interiors/{stem}/int_{stem}.{ext}"));
    let stem = if file(stem, "pssg").is_file() { stem } else { "n12" };
    let bytes = read(&file(stem, "pssg"))?;
    let tree = pssg::read_tree(&bytes)?;
    let sources = mesh::read_static_meshes(&tree)?;
    let draws = scene::read_static_draws(&tree, &sources)?;
    let mut groups = BTreeMap::new();
    for i in (0..tree.nodes.len()).filter(|&i| tree.name(i) == b"SHADERINSTANCE") {
        groups.insert(tree.string(i, b"id")?, tree.string(i, b"shaderGroup")?);
    }
    let mut diffuse = BTreeMap::new();
    for b in material::read_texture_bindings(&tree)? {
        if b.sampler == b"TDiffuseAlphaMap" && b.container.is_none() && diffuse.insert(b.shader, b.texture).is_some() {
            return Err("duplicate cockpit diffuse binding".into());
        }
    }
    let utf8 = |b: &[u8]| String::from_utf8_lossy(b).into_owned();
    let (mut parts, mut bindings, mut needed) = (Vec::new(), BTreeMap::new(), BTreeSet::new());
    let (mut skipped, mut triangles) = (0usize, 0usize);
    for d in draws {
        let group = groups.get(d.material).copied().unwrap_or_default();
        let kind = match group.strip_prefix(b"#").unwrap_or(group) {
            b"interior.fx" => "",
            b"interior_alpha.fx" => "alpha/",
            b"interior_windscreen.fx" => "glass/",
            _ => {
                skipped += 1;
                continue;
            }
        };
        let material = format!("car0/interior/{kind}{}", utf8(d.material));
        if let Some(texture) = diffuse.get(d.material) {
            bindings.insert(material.clone(), format!("car0/interior/{}", utf8(texture)));
            needed.insert(*texture);
        }
        let source = &sources[d.source];
        let mut geometry = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        );
        geometry.insert_attribute(Mesh::ATTRIBUTE_POSITION, source.positions.clone());
        triangles += d.indices.len() / 3;
        geometry.insert_indices(Indices::U16(d.indices));
        let uv = match source.texcoords.is_empty() {
            true => vec![[0f32; 2]; source.positions.len()],
            false => source.texcoords.clone(),
        };
        geometry.insert_attribute(Mesh::ATTRIBUTE_UV_0, uv);
        if source.normals.is_empty() || source.nonfinite_normals != 0 {
            geometry.compute_smooth_normals();
        } else {
            geometry.insert_attribute(Mesh::ATTRIBUTE_NORMAL, source.normals.clone());
        }
        parts.push(Part {
            name: format!("interior/{}", utf8(d.id)),
            joint: format!("{JOINT}{}", utf8(d.joint)),
            material,
            mesh: geometry,
            matrices: vec![anchor * Mat4::from_cols_array(&d.transform)],
            wheel: None,
        });
    }
    if parts.is_empty() {
        return Err("cockpit without drawable parts".into());
    }
    let names: Vec<_> = needed.into_iter().collect();
    let mut images = BTreeMap::new();
    for t in texture::read_named(&tree, &names)? {
        images.insert(format!("car0/interior/{}", utf8(t.id)), t.validated_dds().0);
    }
    if bindings.values().any(|key| !images.contains_key(key)) {
        return Err("cockpit diffuse texture unresolved".into());
    }
    let active = interior::read_active_nodes(&std::fs::read_to_string(file(stem, "xml"))?)?;
    let active: Vec<String> = active.iter().map(|n| n.strip_prefix("x0_").unwrap_or(n).to_string()).collect();
    println!(
        "cockpit stem={stem} parts={} skipped={skipped} triangles={triangles} textures={} active_nodes={}",
        parts.len(),
        images.len(),
        active.join(",")
    );
    car.parts.extend(parts);
    car.bindings.extend(bindings);
    car.textures.extend(images);
    Ok(Cockpit { active })
}
