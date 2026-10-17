//! Static closest-LOD source forest. No wind, collision or original shader claim.
use super::{Car, Part, read};
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use grid_formats::{material, mesh, pssg, scene, texture, tree_placements};
use grid_tools::route::Route;
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    path::Path,
};

fn closest(tree: &pssg::Tree<'_>, node: usize) -> bool {
    let mut parent = tree.nodes[node].parent;
    while let Some(i) = parent {
        if tree.name(i) == b"NODE" && tree.string(i, b"id").is_ok_and(|s| s.ends_with(b"_x0")) {
            return true;
        }
        if tree.name(i) == b"ROOTNODE" {
            break;
        }
        parent = tree.nodes[i].parent;
    }
    false
}

pub fn append(root: &Path, route: &Route, car: &mut Car) -> Result<(), Box<dyn Error>> {
    let bytes = read(&root.join(route.track_file("trees.pssg")))?;
    let tree = pssg::read_tree(&bytes)?;
    let sources = mesh::read_static_meshes(&tree)?;
    let draws = scene::read_static_draws(&tree, &sources)?;
    let library_draws = draws.len();
    let selected: BTreeSet<_> = (0..tree.nodes.len())
        .filter(|&i| tree.name(i) == b"RENDERSTREAMINSTANCE" && closest(&tree, i))
        .map(|i| tree.string(i, b"id"))
        .collect::<grid_formats::Result<_>>()?;
    let bytes = read(&root.join(route.file("trees.bin")))?;
    let placements = tree_placements::read(&bytes)?;
    let by_name: BTreeMap<_, _> = placements
        .prototypes
        .iter()
        .map(|p| Ok((format!("{} Root", std::str::from_utf8(p.name)?), p)))
        .collect::<Result<_, std::str::Utf8Error>>()?;
    let mut diffuse = BTreeMap::new();
    for b in material::read_texture_bindings(&tree)? {
        if b.sampler != b"TDiffuseAlphaMap" {
            continue;
        }
        if b.container != Some(b"treestextures.pssg".as_slice())
            || diffuse.insert(b.shader, b.texture).is_some()
        {
            return Err("unsupported or duplicate tree diffuse binding".into());
        }
    }
    let (mut count, mut triangles, mut closest_draws) = (0usize, 0usize, 0usize);
    let mut matched = BTreeSet::new();
    let mut needed = BTreeSet::new();
    // Stage CPU objects before appending: no partial forest if a source is invalid.
    let mut parts = Vec::new();
    let mut bindings = BTreeMap::new();
    let mut world_min = Vec3::splat(f32::INFINITY);
    let mut world_max = Vec3::splat(f32::NEG_INFINITY);
    for d in draws {
        if !selected.contains(d.id) {
            continue;
        }
        closest_draws += 1;
        let prototype = std::str::from_utf8(d.prototype.ok_or("tree draw prototype absent")?)?;
        let Some(p) = by_name.get(prototype) else {
            continue;
        };
        matched.insert(prototype.to_string());
        let tex = *diffuse
            .get(d.material)
            .ok_or("tree draw lacks diffuse map")?;
        let name = format!("trees/{}", std::str::from_utf8(d.material)?);
        let key = format!("trees/{}", std::str::from_utf8(tex)?);
        bindings.insert(name.clone(), key);
        needed.insert(tex);
        let source = &sources[d.source];
        let mut geometry = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        );
        geometry.insert_attribute(Mesh::ATTRIBUTE_POSITION, source.positions.clone());
        geometry.insert_indices(Indices::U16(d.indices));
        if source.texcoords.is_empty() {
            return Err("tree geometry has no diffuse UV".into());
        }
        geometry.insert_attribute(Mesh::ATTRIBUTE_UV_0, source.texcoords.clone());
        if let Some(uv) = source.additional_texcoords.first() {
            geometry.insert_attribute(Mesh::ATTRIBUTE_UV_1, uv.clone());
        }
        if source.normals.is_empty() || source.nonfinite_normals != 0 {
            geometry.compute_smooth_normals();
        } else {
            geometry.insert_attribute(Mesh::ATTRIBUTE_NORMAL, source.normals.clone());
        }
        let matrices: Vec<_> = p
            .instances
            .iter()
            .map(|instance| {
                Mat4::from_cols_array(&scene::compose(&d.transform, &instance.transform))
            })
            .collect();
        let lo = source
            .positions
            .iter()
            .fold(Vec3::splat(f32::INFINITY), |v, p| {
                v.min(Vec3::from_array(*p))
            });
        let hi = source
            .positions
            .iter()
            .fold(Vec3::splat(f32::NEG_INFINITY), |v, p| {
                v.max(Vec3::from_array(*p))
            });
        for matrix in &matrices {
            for i in 0..8 {
                let p = matrix.transform_point3(Vec3::new(
                    if i & 1 == 0 { lo.x } else { hi.x },
                    if i & 2 == 0 { lo.y } else { hi.y },
                    if i & 4 == 0 { lo.z } else { hi.z },
                ));
                world_min = world_min.min(p);
                world_max = world_max.max(p);
            }
        }
        count += matrices.len();
        triangles += geometry.indices().unwrap().len() / 3 * matrices.len();
        parts.push(Part {
            name: format!("trees/{}", std::str::from_utf8(d.id)?),
            joint: String::new(),
            material: name,
            mesh: geometry,
            matrices,
            wheel: None,
        });
    }
    if matched.len() != placements.prototypes.len() || parts.is_empty() {
        return Err("tree placement prototype lacks closest source geometry".into());
    }
    let bytes = read(&root.join(route.track_file("treestextures.pssg")))?;
    let textures = pssg::read_tree(&bytes)?;
    let names: Vec<_> = needed.into_iter().collect();
    let mut images = BTreeMap::new();
    for t in texture::read_named(&textures, &names)? {
        let key = format!("trees/{}", std::str::from_utf8(t.id)?);
        images.insert(key, t.validated_dds().0);
    }
    if bindings.values().any(|key| !images.contains_key(key)) {
        return Err("tree diffuse texture unresolved".into());
    }
    if bindings.keys().any(|key| car.bindings.contains_key(key))
        || images.keys().any(|key| car.textures.contains_key(key))
    {
        return Err("tree namespace already exists".into());
    }
    println!(
        "source_trees enabled=true prototypes={} instances={} library_draws={library_draws} closest_draws={closest_draws} placed_draws={count} triangles={triangles} textures={} bounds={world_min:?}..{world_max:?} lod=source-x0 wind=false physics=unchanged alpha=authored-mask05 sampler=existing-repeat",
        matched.len(),
        placements.total_instances,
        images.len()
    );
    car.parts.extend(parts);
    car.bindings.extend(bindings);
    car.textures.extend(images);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn purchased_donington_closest_forest_is_bound_without_billboard_duplicates() {
        let root = std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required");
        let mut car = Car {
            parts: Vec::new(),
            textures: BTreeMap::new(),
            bindings: BTreeMap::new(),
            center: Vec3::ZERO,
            radius: 1.,
            track: true,
            liveries: Vec::new(),
        };
        append(Path::new(&root), &Route::default(), &mut car).unwrap();
        assert_eq!(car.parts.len(), 21);
        assert!(
            car.parts
                .iter()
                .all(|p| !p.matrices.is_empty() && p.wheel.is_none())
        );
        assert!(
            car.parts
                .iter()
                .all(|p| p.matrices.iter().all(|m| m.is_finite()))
        );
        assert!(
            car.bindings
                .values()
                .all(|key| car.textures.contains_key(key))
        );
        assert!(append(Path::new(&root), &Route::default(), &mut car).is_err());
        assert_eq!(car.parts.len(), 21);
    }
}
