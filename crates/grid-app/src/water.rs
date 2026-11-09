//! The original's `water.fx` (land.pssg sea, rivers and Haruna's wet patches;
//! docs/WATER.md): its programs' arithmetic on the saved constants, normal map
//! and vertex colour. The reflection cube is the original's `WaterEnvMap` of
//! the draw's child of the `water` node (water_probe.rs); a draw outside that
//! node samples the route's source sky baked at 64 texels a face
//! (sky_environment.rs) [authored]. AUTHORED: colours are decoded from sRGB,
//! time is seconds, and the port's fog replaces the program's own.
use bevy::{
    asset::{RenderAssetUsages, uuid_handle},
    mesh::{Indices, PrimitiveTopology},
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};
use grid_formats::{material, mesh, pssg, scene, texture};
use grid_tools::route::Route;
use std::{collections::BTreeMap, path::Path};

const SHADER: Handle<Shader> = uuid_handle!("3f0e4c52-8a7b-4d61-9b1e-6c2d5a7f9e13");
type WaterMaterial = ExtendedMaterial<StandardMaterial, Water>;

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct Water {
    #[texture(100)]
    #[sampler(101)]
    normal_map: Handle<Image>,
    // deepColour, waterAmount.
    #[uniform(102)]
    deep: Vec4,
    // shallowColour, bumpHeight.
    #[uniform(103)]
    shallow: Vec4,
    // reflectionColour, reflectionAmount.
    #[uniform(104)]
    reflection: Vec4,
    // textureScaleX/Y, bumpSpeedX/Y.
    #[uniform(105)]
    motion: Vec4,
    // fresnelBias, fresnelPower.
    #[uniform(106)]
    fresnel: Vec4,
    #[texture(107, dimension = "cube")]
    #[sampler(108)]
    environment: Handle<Image>,
}
impl MaterialExtension for Water {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }
}

struct Definition {
    normal_map: String,
    constants: BTreeMap<String, Vec<f32>>,
}
struct Draw {
    material: String,
    mesh: Mesh,
    transform: Mat4,
    /// The `water` child whose environment map it samples.
    probe: Option<usize>,
}
#[derive(Resource)]
struct Source {
    definitions: BTreeMap<String, Definition>,
    draws: Vec<Draw>,
    textures: BTreeMap<String, Vec<u8>>,
    environment: Option<Image>,
    /// World origins of the `water` node's children (the probes' centres).
    origins: Vec<Vec3>,
    cubes: Vec<Handle<Image>>,
}

/// The `water` node's children: their world origins, and each scene id at or
/// under a child mapped to its index (0x985550 makes one `WaterEnvMap` per
/// child, at its first scene child's translation, else at its own).
fn probe_nodes(tree: &pssg::Tree<'_>, file: &str) -> (Vec<Vec3>, BTreeMap<Vec<u8>, usize>) {
    let mut origins = Vec::new();
    let mut ids = BTreeMap::new();
    let Some(water) = (0..tree.nodes.len()).find(|&i| {
        matches!(tree.name(i), b"NODE" | b"ROOTNODE") && tree.string(i, b"id").is_ok_and(|v| v == b"water")
    }) else {
        return (origins, ids);
    };
    for &child in &tree.nodes[water].children {
        if !matches!(tree.name(child), b"NODE" | b"RENDERNODE" | b"ROOTNODE" | b"LODVISIBLERENDERNODE") {
            continue;
        }
        // 0x985550 places the probe at the child's first scene child (the
        // authored `*_cube_01` marker) when it has one, else at the child.
        let marker = tree.nodes[child].children.iter().copied()
            .find(|&c| matches!(tree.name(c), b"NODE" | b"RENDERNODE" | b"ROOTNODE"))
            .unwrap_or(child);
        let m = match scene::node_matrix(tree, marker) {
            Ok(m) => m,
            Err(error) => {
                eprintln!("water_probe child={child} skipped file={file} error={error}");
                continue;
            }
        };
        let index = origins.len();
        origins.push(Vec3::new(m[12], m[13], m[14]));
        let mut stack = vec![child];
        while let Some(node) = stack.pop() {
            if let Ok(id) = tree.string(node, b"id") {
                ids.entry(id.to_vec()).or_insert(index);
            }
            stack.extend(tree.nodes[node].children.iter().copied());
        }
    }
    (origins, ids)
}

/// The `water.fx` draws of the route's land.pssg; none on tracks without.
fn read_source(root: &Path, route: &Route) -> Result<Source, Box<dyn std::error::Error>> {
    let file = route.track_file("land.pssg");
    let bytes = super::read(&root.join(&file))?;
    let tree = pssg::read_tree(&bytes)?;
    if scene::is_empty_optional_static_package(&tree)? {
        return Ok(Source {
            definitions: BTreeMap::new(),
            draws: Vec::new(),
            textures: BTreeMap::new(),
            environment: None,
            origins: Vec::new(),
            cubes: Vec::new(),
        });
    }
    let mut definitions = BTreeMap::new();
    for s in material::read_constants(&tree)? {
        if s.group != b"water.fx" {
            continue;
        }
        let constants = s.values.iter()
            .map(|(k, v)| (String::from_utf8_lossy(k).into_owned(), v.clone()))
            .collect();
        definitions.insert(String::from_utf8(s.shader.to_vec())?, Definition { normal_map: String::new(), constants });
    }
    let mut containers = BTreeMap::new();
    for b in material::read_texture_bindings(&tree)? {
        if b.sampler == b"TNormalMap"
            && let Some(d) = definitions.get_mut(std::str::from_utf8(b.shader)?)
        {
            d.normal_map = String::from_utf8(b.texture.to_vec())?;
            let container = b.container.map_or("land.pssg".to_owned(), |c| String::from_utf8_lossy(c).into_owned());
            containers.insert(d.normal_map.clone(), container);
        }
    }
    if let Some((name, _)) = definitions.iter().find(|(_, d)| d.normal_map.is_empty()) {
        return Err(format!("{file}: water material {name:?} has no TNormalMap").into());
    }
    let sources = mesh::read_meshes(&tree)?;
    let (origins, probe_ids) = probe_nodes(&tree, &file);
    let mut draws = Vec::new();
    for draw in scene::read_static_draws(&tree, &sources)? {
        let material = std::str::from_utf8(draw.material)?;
        if !definitions.contains_key(material) {
            continue;
        }
        let source = &sources[draw.source];
        let range = draw.vertices.clone();
        if source.texcoords.is_empty() || source.normals.is_empty() {
            return Err(format!("{file}: water draw without UVs or normals").into());
        }
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        let transform = Mat4::from_cols_array(&draw.transform);
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, source.positions[range.clone()].to_vec());
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, source.normals[range.clone()].to_vec());
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, source.texcoords[range.clone()].to_vec());
        // The vertex program scales the tangent by bumpHeight * (1 - green)
        // and passes alpha through; white (no bump, opaque) without colours.
        let colours: Vec<[f32; 4]> = if source.colors_argb.is_empty() {
            vec![[1.; 4]; range.len()]
        } else {
            source.colors_argb[range.clone()].iter()
                .map(|&[a, r, g, b]| [r, g, b, a].map(|c| f32::from(c) / 255.))
                .collect()
        };
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colours);
        mesh.insert_indices(Indices::U16(draw.indices));
        if source.tangents.is_empty() {
            mesh.generate_tangents()?;
        } else {
            let tangents: Vec<[f32; 4]> = source.tangents[range].iter().map(|t| [t[0], t[1], t[2], 1.]).collect();
            mesh.insert_attribute(Mesh::ATTRIBUTE_TANGENT, tangents);
        }
        draws.push(Draw {
            material: material.to_owned(),
            mesh,
            transform,
            probe: probe_ids.get(draw.joint).copied(),
        });
    }
    let mut textures = BTreeMap::new();
    let files: std::collections::BTreeSet<&String> = containers.values().collect();
    for container in files {
        let other = if container == "land.pssg" {
            None
        } else {
            Some(super::read(&root.join(route.track_file(container)))?)
        };
        let other_tree = other.as_deref().map(pssg::read_tree).transpose()?;
        for t in texture::read_textures(other_tree.as_ref().unwrap_or(&tree))? {
            let id = String::from_utf8(t.id.to_vec())?;
            if containers.get(&id) == Some(container) {
                textures.insert(id, t.validated_dds().0);
            }
        }
    }
    if let Some(name) = containers.keys().find(|n| !textures.contains_key(*n)) {
        return Err(format!("{file}: water normal map {name:?} unresolved").into());
    }
    println!(
        "water prepared materials={} draws={} textures={} probes={} probed_draws={} source=land.pssg shader=water.fx",
        definitions.len(), draws.len(), textures.len(), origins.len(),
        draws.iter().filter(|d| d.probe.is_some()).count()
    );
    Ok(Source { definitions, draws, textures, environment: None, origins, cubes: Vec::new() })
}

pub fn install(app: &mut App, root: &Path, route: &Route) -> Result<(), Box<dyn std::error::Error>> {
    let mut source = read_source(root, route)?;
    if source.draws.is_empty() {
        return Ok(());
    }
    source.environment = Some(sky_cube(root, route)?);
    // GRID_WATER_PROBE=0 reflects the baked sky cube only (F705's look; comparisons).
    if std::env::var_os("GRID_WATER_PROBE").is_some_and(|v| v == "0") {
        for d in &mut source.draws {
            d.probe = None;
        }
    }
    // Probes no draw samples are not drawn.
    let used: std::collections::BTreeSet<usize> = source.draws.iter().filter_map(|d| d.probe).collect();
    if used.len() != source.origins.len() {
        let remap: BTreeMap<usize, usize> = used.iter().enumerate().map(|(new, &old)| (old, new)).collect();
        source.origins = used.iter().map(|&i| source.origins[i]).collect();
        for d in &mut source.draws {
            d.probe = d.probe.map(|p| remap[&p]);
        }
    }
    source.cubes = super::water_probe::install(app, source.origins.clone());
    app.world_mut().resource_mut::<Assets<Shader>>().insert(
        SHADER.id(),
        Shader::from_wgsl(include_str!("water.wgsl"), "grid_water.wgsl"),
    )?;
    app.insert_resource(source)
        .add_plugins(MaterialPlugin::<WaterMaterial>::default())
        .add_systems(Startup, spawn);
    Ok(())
}

/// The route's sky (slot 0, lighting.xml's skyscale/skygamma, as sky.rs's
/// start look on every route without a timeline) as a cube.
fn sky_cube(root: &Path, route: &Route) -> Result<Image, Box<dyn std::error::Error>> {
    let file = super::sky::sky_file(root, route);
    let bytes = super::read(&root.join(&file))?;
    let tree = pssg::read_tree(&bytes)?;
    let sky = grid_formats::sky::read(&tree).map_err(|e| format!("{file}: {e}"))?;
    let lighting = grid_tools::environment::Environment::for_route(root, route)?.lighting;
    let tone = [
        lighting.scalar("skyscale").unwrap_or(1.).clamp(0., 16.),
        lighting.scalar("skygamma").unwrap_or(1.).clamp(0.01, 16.),
    ];
    Ok(super::sky_environment::bake_sized(&sky.positions, &sky.texcoords, &sky.indices, sky.texture(0), tone, 64)?)
}

fn spawn(
    mut commands: Commands,
    mut source: ResMut<Source>,
    settings: Res<super::graphics::Settings>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<WaterMaterial>>,
) {
    let environment = images.add(source.environment.take().expect("water sky cube"));
    let textures: BTreeMap<_, _> = source.textures.iter()
        .map(|(name, dds)| {
            // The program decodes 2t - 1: linear data, repeated.
            let image = super::texture_upload::image_linear(dds, settings.sampler(true), RenderAssetUsages::default())
                .expect("validated water normal map");
            (name.clone(), images.add(image))
        })
        .collect();
    let mut keys: Vec<(String, Option<usize>)> = source.draws.iter().map(|d| (d.material.clone(), d.probe)).collect();
    keys.sort();
    keys.dedup();
    let handles: BTreeMap<_, _> = keys.into_iter()
        .map(|(name, probe)| {
            let d = &source.definitions[&name];
            let c = |key: &str, i: usize| d.constants.get(key).and_then(|v| v.get(i)).copied().unwrap_or(0.);
            let colour = |key: &str| Color::srgb(c(key, 0), c(key, 1), c(key, 2)).to_linear().to_vec3();
            let base = StandardMaterial {
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                ..default()
            };
            let extension = Water {
                normal_map: textures[&d.normal_map].clone(),
                deep: colour("deepColour").extend(c("waterAmount", 0)),
                shallow: colour("shallowColour").extend(c("bumpHeight", 0)),
                reflection: colour("reflectionColour").extend(c("reflectionAmount", 0)),
                motion: Vec4::new(c("textureScaleX", 0), c("textureScaleY", 0), c("bumpSpeedX", 0), c("bumpSpeedY", 0)),
                fresnel: Vec4::new(c("fresnelBias", 0), c("fresnelPower", 0), 0., 0.),
                environment: probe.map_or_else(|| environment.clone(), |p| source.cubes[p].clone()),
            };
            ((name, probe), materials.add(ExtendedMaterial { base, extension }))
        })
        .collect();
    let draws = std::mem::take(&mut source.draws);
    let count = draws.len();
    for (i, draw) in draws.into_iter().enumerate() {
        commands.spawn((
            Name::new(format!("land/water/{}#{i}", draw.material)),
            Mesh3d(meshes.add(draw.mesh)),
            MeshMaterial3d(handles[&(draw.material, draw.probe)].clone()),
            Transform::from_matrix(draw.transform),
            bevy::light::NotShadowCaster,
            super::water_probe::WaterSurface,
        ));
    }
    let probed = handles.keys().filter(|(_, p)| p.is_some()).count();
    println!(
        "water spawned draws={count} materials={} reflection=water-env-probe-{} probed_materials={probed} sky_cube_materials={}",
        handles.len(), super::water_probe::SIZE, handles.len() - probed
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn purchased_optional_land_water_adapter_preserves_empty_and_nonempty_sources() {
        let root = std::path::PathBuf::from(
            std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required"),
        );
        for path in [grid_tools::route::SHIBUYA, grid_tools::route::MILAN] {
            let route = Route::new(path).unwrap();
            let source = read_source(&root, &route).unwrap();
            assert!(source.definitions.is_empty(), "{path}");
            assert!(source.draws.is_empty(), "{path}");
            assert!(source.textures.is_empty(), "{path}");
            assert!(source.environment.is_none(), "{path}");
        }
        let route = Route::new("tracks/usa/detroit/route_8").unwrap();
        let source = read_source(&root, &route).unwrap();
        assert!(!source.definitions.is_empty());
        assert!(!source.draws.is_empty());
        assert!(!source.textures.is_empty());
        assert!(source.environment.is_none());
        // One WaterEnvMap probe at the `sea_mesh_01_cube_01` marker under the
        // `water` node (lifted 8.9 m below its parent), and every water draw
        // lies under it.
        assert_eq!(source.origins.len(), 1);
        let expected = Vec3::new(330.52, 273.47 - 8.89, 948.03);
        assert!(source.origins[0].distance(expected) < 0.05, "{:?}", source.origins);
        assert!(source.draws.iter().all(|d| d.probe == Some(0)));
    }
}
