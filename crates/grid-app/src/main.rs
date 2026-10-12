//! Source reference viewer and serial driving prototype.
use bevy::{
    app::{AppExit, ScheduleRunnerPlugin},
    asset::RenderAssetUsages,
    camera::RenderTarget,
    image::{
        CompressedImageFormats, ImageAddressMode, ImageSampler, ImageSamplerDescriptor, ImageType,
    },
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    render::{
        render_resource::{Face, TextureFormat, TextureUsages},
        view::screenshot::{Screenshot, ScreenshotCaptured},
    },
    window::ExitCondition,
    winit::WinitPlugin,
};
use grid_formats::{collision, entities, jpk, material, mesh, ornaments, pssg, scene, texture};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

#[derive(Resource)]
struct Car {
    parts: Vec<Part>,
    textures: BTreeMap<String, Vec<u8>>,
    bindings: BTreeMap<String, String>,
    center: Vec3,
    radius: f32,
    track: bool,
}
struct Part {
    name: String,
    material: String,
    mesh: Mesh,
    matrices: Vec<Mat4>,
    wheel: Option<usize>,
}
#[derive(Resource)]
struct Capture {
    path: PathBuf,
    target: Option<Handle<Image>>,
    frames: u32,
    requested: bool,
    started: Instant,
}
#[derive(Resource, Clone)]
struct Orbit {
    yaw: f32,
    pitch: f32,
    distance: f32,
    target: Vec3,
    scale: f32,
}
#[derive(Resource)]
struct InitialOrbit(Orbit);
#[derive(Component)]
struct ViewerCamera;

#[derive(Resource)]
struct Driving {
    runtime: grid_tools::source_vehicle::Runtime,
    geometry: Option<grid_tools::body_geometry::BodyGeometry>,
    paused: bool,
    failure: Option<String>,
    scripted_ticks: Option<u64>,
    scripted: grid_tools::source_vehicle::Controls,
    last_controls: Option<grid_tools::source_vehicle::Controls>,
    segment: u64,
    started: Instant,
}
impl Driving {
    fn report(&self, event: &str, controls: grid_tools::source_vehicle::Controls) {
        let b = &self.runtime.state.car.owner.vehicle;
        let brakes: [f32; 4] = std::array::from_fn(|i| {
            f32::from_bits(self.runtime.state.car.frame.view[0x28 / 4 + i])
        });
        let hits = (0..4)
            .filter(|i| b[(0x1990 + i * 0x150 + 0xc0) / 4] != 0)
            .count();
        println!(
            "drive_sample version=1 event={event} segment={} tick={} seconds={:.6} wall_seconds={:.6} throttle={} brake={} steering={} handbrake={} gear={} wheel_hits={hits} pose={:?} velocity={:?} wheel_brakes={brakes:?} body_geometry={} body_contacts={}",
            self.segment,
            self.runtime.ticks,
            self.runtime.ticks as f64 * f64::from(grid_tools::source_vehicle::DELTA),
            self.started.elapsed().as_secs_f64(),
            controls.throttle,
            controls.brake,
            controls.steering,
            controls.handbrake,
            b[0x196c / 4],
            self.runtime.pose(),
            self.runtime.velocity(),
            self.geometry.is_some(),
            self.runtime.last_body_contacts
        );
    }
}
#[derive(Component)]
struct MovingPart {
    rest: Mat4,
    wheel: Option<usize>,
}

fn wheel_joint(joint: &[u8]) -> Option<usize> {
    let name = std::str::from_utf8(joint).ok()?;
    let (_, name) = name.split_once('_')?;
    let (kind, side) = name.split_once('_')?;
    if !matches!(kind, "wheel" | "tyre" | "disc") {
        return None;
    }
    match side {
        "bl" => Some(0),
        "br" => Some(1),
        "fl" => Some(2),
        "fr" => Some(3),
        _ => None,
    }
}

/// Authored presentation composition using source wheel state. Retain each
/// mesh's bind offset around the shared physics pivot; never animate by input
/// command or integrate a second wheel clock. Camber/deformation are separate.
fn wheel_motion(wheel: grid_tools::source_vehicle::WheelPose) -> Mat4 {
    let anchor = Vec3::from_array(wheel.anchor);
    let center = anchor + Vec3::from_array(wheel.axis) * wheel.displacement;
    let yaw = wheel.steering[1].atan2(wheel.steering[0]);
    Mat4::from_translation(center)
        * Mat4::from_rotation_y(yaw)
        * Mat4::from_rotation_x(wheel.phase)
        * Mat4::from_translation(-anchor)
}

// Provisional diffuse bindings recovered from executable static tuples.
fn diffuse(material: &str) -> Option<&'static str> {
    match material.trim() {
        "Car Body Paint" | "Matt" | "Shiny Metal" | "Bare Metal" | "Aerials" => {
            Some("lac_main.tga")
        }
        "Window Glass" | "Grills Glass" => Some("lac_glass.tga"),
        "Cabin" => Some("lac_cabin.tga"),
        "Wheel" | "Tyre Wall" => Some("lac_wheel_tm.tga"),
        "Tyre Tread" => Some("lac_tread_tm.tga"),
        "Suspension" => Some("lac_susp.tga"),
        "Head Lights Pod" | "Rear Lights Pod" | "Lights Pod Matt" | "Head Lights Glass"
        | "Rear Lights Glass" | "Discs" | "Grills" => Some("lac_lights.tga"),
        _ => None,
    }
}
fn read(path: &Path) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use std::io::Read;
    let file = std::fs::File::open(path)?;
    let limit = 256 * 1024 * 1024;
    if file.metadata()?.len() > limit {
        return Err("resource exceeds viewer limit".into());
    }
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err("resource grew beyond viewer limit".into());
    }
    Ok(bytes)
}
fn load(
    root: &Path,
    lod: &str,
    track: bool,
    include_batches: bool,
) -> Result<Car, Box<dyn std::error::Error>> {
    let mesh_path = if track {
        "tracks/europe/donington/tracksplit.pssg"
    } else {
        "cars/lac/lac_highLOD.pssg"
    };
    let bytes = read(&root.join(mesh_path))?;
    let tree = pssg::read_tree(&bytes)?;
    let sources = mesh::read_meshes(&tree)?;
    let draws = if track {
        scene::read_static_draws(&tree, &sources)?
    } else {
        scene::read_draws(&tree, &sources, lod.as_bytes())?
    };
    let mut bindings = BTreeMap::new();
    if track {
        for binding in material::read_texture_bindings(&tree)? {
            if binding.sampler == b"TDiffuseAlphaMap"
                && bindings
                    .insert(
                        String::from_utf8(binding.shader.to_vec())?,
                        String::from_utf8(binding.texture.to_vec())?,
                    )
                    .is_some()
            {
                return Err("duplicate diffuse binding".into());
            }
        }
    }
    let mut parts = Vec::new();
    let mut bounds = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
    let mut triangle_count = 0;
    let mut fallback_count = 0;
    let mut omitted_draws = 0;
    for draw in draws {
        if track && !include_batches && draw.material == b"batchmaterial" {
            omitted_draws += 1;
            continue;
        }
        let source = &sources[draw.source];
        let positions = source.positions[draw.vertices.clone()].to_vec();
        let normals = if source.normals.is_empty() {
            vec![]
        } else {
            source.normals[draw.vertices.clone()].to_vec()
        };
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        );
        for p in &positions {
            let p = Vec3::from_array(scene::transform_point(&draw.transform, *p));
            bounds.0 = bounds.0.min(p);
            bounds.1 = bounds.1.max(p);
        }
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        if !source.texcoords.is_empty() {
            mesh.insert_attribute(
                Mesh::ATTRIBUTE_UV_0,
                source.texcoords[draw.vertices.clone()].to_vec(),
            );
        }
        triangle_count += draw.indices.len() / 3;
        mesh.insert_indices(Indices::U16(draw.indices));
        if normals.is_empty() || normals.iter().any(|n| n.iter().any(|v| !v.is_finite())) {
            mesh.compute_smooth_normals();
            fallback_count += 1;
        } else {
            mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        }
        parts.push(Part {
            name: String::from_utf8(draw.id.to_vec())?,
            material: String::from_utf8(draw.material.to_vec())?,
            mesh,
            matrices: vec![Mat4::from_cols_array(&draw.transform)],
            wheel: if track { None } else { wheel_joint(draw.joint) },
        });
    }
    let mut textures = BTreeMap::new();
    let texture_paths = if track {
        vec![mesh_path]
    } else {
        vec![
            "cars/lac/livery_00/textures_high/lac_tex_high_00.pssg",
            "cars/lac/lac_gen_high.pssg",
        ]
    };
    for path in texture_paths {
        let bytes = read(&root.join(path))?;
        let tree = pssg::read_tree(&bytes)?;
        for texture in texture::read_textures(&tree)? {
            let (dds, levels) = texture.validated_dds();
            println!(
                "texture {:?} validated_mip_levels={levels}",
                String::from_utf8_lossy(texture.id)
            );
            if textures
                .insert(String::from_utf8(texture.id.to_vec())?, dds)
                .is_some()
            {
                return Err("duplicate texture ID".into());
            }
        }
    }
    for part in &parts {
        if !track {
            let name = diffuse(&part.material)
                .ok_or_else(|| format!("unknown car material {:?}", part.material))?;
            bindings.insert(part.material.clone(), name.to_string());
        }
        let Some(name) = bindings.get(&part.material) else {
            if track && part.material == "batchmaterial" {
                continue;
            }
            return Err(format!("unresolved diffuse material {:?}", part.material).into());
        };
        if !textures.contains_key(name) {
            return Err(format!("missing diffuse texture: {name}").into());
        }
    }
    println!(
        "scene track={track} LOD={lod} draws={} omitted_unresolved_batch_draws={omitted_draws} triangles={triangle_count} normal_fallback_draws={fallback_count} bounds={:?}..{:?} textures={}",
        parts.len(),
        bounds.0,
        bounds.1,
        textures.len()
    );
    let mut car = Car {
        parts,
        textures,
        bindings,
        center: (bounds.0 + bounds.1) * 0.5,
        radius: (bounds.1 - bounds.0).length() * 0.5,
        track,
    };
    if track {
        append_objects(root, &mut car)?;
    }
    Ok(car)
}
fn append_objects(root: &Path, car: &mut Car) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = read(&root.join("tracks/europe/donington/objects.pssg"))?;
    let tree = pssg::read_tree(&bytes)?;
    let sources = mesh::read_static_meshes(&tree)?;
    let draws = scene::read_object_draws(&tree, &sources)?;
    let placement_bytes = read(&root.join("tracks/europe/donington/route_0/ornaments.bin"))?;
    let placements = ornaments::read(&placement_bytes)?;
    let transforms: BTreeMap<_, _> = placements
        .prototypes
        .iter()
        .map(|p| {
            (
                format!("{} Root", String::from_utf8_lossy(p.name)),
                &p.transforms,
            )
        })
        .collect();
    let mut selected = std::collections::BTreeSet::new();
    let mut unplaced = std::collections::BTreeSet::new();
    let mut draw_count = 0;
    let mut triangles = 0;
    for binding in material::read_texture_bindings(&tree)? {
        if binding.sampler == b"TDiffuseAlphaMap" {
            let name = format!("objects/{}", String::from_utf8(binding.shader.to_vec())?);
            let container = binding
                .container
                .ok_or("object diffuse binding needs a container")?;
            if ![
                b"objectstextures.pssg".as_slice(),
                b"lemans_sponsors.pssg",
                b"sponsor_minor.pssg",
                b"sponsor_event.pssg",
            ]
            .contains(&container)
            {
                return Err("unsupported object texture container".into());
            }
            if car
                .bindings
                .insert(
                    name,
                    format!(
                        "{}#{}",
                        String::from_utf8(container.to_vec())?,
                        String::from_utf8(binding.texture.to_vec())?
                    ),
                )
                .is_some()
            {
                return Err("duplicate object diffuse binding".into());
            }
        }
    }
    for draw in draws {
        let prototype =
            String::from_utf8(draw.prototype.ok_or("object prototype absent")?.to_vec())?;
        let Some(placements) = transforms.get(&prototype) else {
            unplaced.insert(prototype);
            continue;
        };
        selected.insert(prototype);
        if placements.is_empty() {
            continue;
        }
        let source = &sources[draw.source];
        let name = format!("objects/{}", String::from_utf8(draw.material.to_vec())?);
        if !car.bindings.contains_key(&name) {
            return Err(format!("unresolved object diffuse binding {name}").into());
        }
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, source.positions.clone());
        if !source.texcoords.is_empty() {
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, source.texcoords.clone());
        }
        mesh.insert_indices(Indices::U16(draw.indices));
        if source.normals.is_empty() || source.nonfinite_normals != 0 {
            mesh.compute_smooth_normals();
        } else {
            mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, source.normals.clone());
        }
        let matrices = placements
            .iter()
            .map(|placement| Mat4::from_cols_array(&scene::compose(&draw.transform, placement)))
            .collect::<Vec<_>>();
        draw_count += matrices.len();
        triangles += mesh.count_vertices();
        car.parts.push(Part {
            name: String::from_utf8(draw.id.to_vec())?,
            material: name,
            mesh,
            matrices,
            wheel: None,
        });
    }
    for prototype in &placements.prototypes {
        let name = format!("{} Root", String::from_utf8_lossy(prototype.name));
        if !prototype.transforms.is_empty() && !selected.contains(&name) {
            return Err(format!("static prototype has no selected geometry {name}").into());
        }
    }
    for (container, path) in [
        (
            "objectstextures.pssg",
            "tracks/europe/donington/patchup_ot.pssg",
        ),
        (
            "lemans_sponsors.pssg",
            "tracks/europe/donington/lemans_sponsors.pssg",
        ),
        (
            "sponsor_minor.pssg",
            "track_sponsors/tag_heuer/sponsor_minor.pssg",
        ),
        (
            "sponsor_event.pssg",
            "track_sponsors/bridgestone/sponsor_event.pssg",
        ),
    ] {
        let bytes = read(&root.join(path))?;
        let tree = pssg::read_tree(&bytes)?;
        for texture in texture::read_textures(&tree)? {
            let name = format!("{container}#{}", String::from_utf8(texture.id.to_vec())?);
            let (dds, _) = texture.validated_dds();
            if let Some(existing) = car.textures.get(&name) {
                if existing != &dds {
                    return Err(format!("conflicting shared texture {name}").into());
                }
            } else {
                car.textures.insert(name, dds);
            }
        }
    }
    for (material, texture) in &car.bindings {
        if material.starts_with("objects/") && !car.textures.contains_key(texture) {
            return Err(format!("object texture unresolved {texture}").into());
        }
    }
    println!(
        "objects static_sources={} placed_draws={draw_count} unique_geometry_vertices={triangles} static_placements={} total_header_instances={} omitted_entity_instances={} textures={}",
        sources.len(),
        placements
            .prototypes
            .iter()
            .map(|p| p.transforms.len())
            .sum::<usize>(),
        placements.total_instances,
        placements.total_instances as usize
            - placements
                .prototypes
                .iter()
                .map(|p| p.transforms.len())
                .sum::<usize>(),
        car.textures.len()
    );
    println!("unplaced library prototypes={unplaced:?}");
    append_entities(root, car, &tree, &sources)?;
    Ok(())
}
fn append_entities(
    root: &Path,
    car: &mut Car,
    objects: &pssg::Tree<'_>,
    sources: &[mesh::Mesh<'_>],
) -> Result<(), Box<dyn std::error::Error>> {
    let placement_bytes = read(&root.join("tracks/europe/donington/route_0/objects.ens"))?;
    let type_bytes = read(&root.join("tracks/europe/donington/objecttypes.pssg"))?;
    let placement_tree = pssg::read_tree(&placement_bytes)?;
    let type_tree = pssg::read_tree(&type_bytes)?;
    let entities = entities::read(&placement_tree, &type_tree)?;
    let mut placements: BTreeMap<&[u8], Vec<[f32; 16]>> = BTreeMap::new();
    let mut cloth = 0;
    let mut boxes = 0;
    let mut cylinders = 0;
    let mut links = 0;
    for entity in &entities {
        cloth += entity.cloth.len();
        links += entity.links;
        for renderable in &entity.renderables {
            if renderable.container != b"objects.pssg" {
                return Err("unsupported entity renderable container".into());
            }
            placements
                .entry(renderable.node)
                .or_default()
                .push(entity.transform);
            for shape in &renderable.shapes {
                match shape.primitive {
                    entities::Primitive::Box { .. } => boxes += 1,
                    entities::Primitive::Cylinder { .. } => cylinders += 1,
                }
            }
        }
    }
    let mut draw_count = 0;
    let mut triangles = 0;
    let closest_lod = scene::read_object_draws(objects, sources)?;
    let mut provisional_lod_instances = 0;
    for (node, placements) in &placements {
        let mut draws = scene::read_renderable_draws(objects, sources, node)?;
        if draws.is_empty() {
            // Exact startup renderables containing anchors but no mesh.
            // Preview intact closest LOD; runtime substitution remains untraced.
            let prototype: &[u8] = match *node {
                b"dcushion_green_a01_default_01" => b"cushion_blue Root",
                b"tyrestack_black_default_01" => b"tyrestack_black Root",
                _ => {
                    return Err(format!(
                        "empty entity renderable {}",
                        String::from_utf8_lossy(node)
                    )
                    .into());
                }
            };
            draws = closest_lod
                .iter()
                .filter(|d| d.prototype == Some(prototype))
                .cloned()
                .collect();
            if draws.is_empty() {
                return Err("provisional intact LOD unavailable".into());
            }
            provisional_lod_instances += placements.len();
            println!(
                "provisional entity intact LOD {:?} instances={}",
                String::from_utf8_lossy(prototype),
                placements.len()
            );
        }
        for draw in draws {
            let source = &sources[draw.source];
            let material = format!("objects/{}", String::from_utf8(draw.material.to_vec())?);
            if !car.bindings.contains_key(&material) {
                return Err(format!("unresolved entity material {material}").into());
            }
            let mut mesh = Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
            );
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, source.positions.clone());
            if !source.texcoords.is_empty() {
                mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, source.texcoords.clone());
            }
            triangles += draw.indices.len() / 3;
            mesh.insert_indices(Indices::U16(draw.indices));
            if source.normals.is_empty() || source.nonfinite_normals != 0 {
                mesh.compute_smooth_normals();
            } else {
                mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, source.normals.clone());
            }
            let matrices = placements
                .iter()
                .map(|placement| Mat4::from_cols_array(&scene::compose(&draw.transform, placement)))
                .collect::<Vec<_>>();
            draw_count += matrices.len();
            car.parts.push(Part {
                name: format!(
                    "entity/{}/{}",
                    String::from_utf8_lossy(node),
                    String::from_utf8_lossy(draw.id)
                ),
                material,
                mesh,
                matrices,
                wheel: None,
            });
        }
    }
    println!(
        "entities placements={} initial_renderables={} unique_renderables={} placed_draws={draw_count} unique_geometry_triangles={triangles} collision_boxes={boxes} collision_cylinders={cylinders} omitted_cloth_components={cloth} unimplemented_entity_links={links} provisional_lod_instances={provisional_lod_instances}",
        entities.len(),
        placements.values().map(Vec::len).sum::<usize>(),
        placements.len()
    );
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("grid-app: {error}");
        std::process::exit(1);
    }
}
fn load_collision(root: &Path) -> Result<Car, Box<dyn std::error::Error>> {
    let bytes = read(&root.join("tracks/europe/donington/route_0/track.jpk"))?;
    let archive = jpk::inspect(&bytes)?;
    let mut parts = Vec::new();
    let mut bounds = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
    let mut vertices = 0;
    let mut triangles = 0;
    let mut materials = std::collections::BTreeSet::new();
    for entry in &archive.entries {
        if !entry.name.ends_with(b".vcqtc")
            || entry.uncompressed_size as usize != entry.stored_data.len()
        {
            return Err("unsupported track collision entry".into());
        }
        let chunk = collision::read_vcqtc(&bytes[entry.stored_data.clone()])?;
        vertices += chunk.positions.len();
        triangles += chunk.triangles.len();
        for &p in &chunk.positions {
            let p = Vec3::from_array(p);
            bounds.0 = bounds.0.min(p);
            bounds.1 = bounds.1.max(p);
        }
        for (i, material) in chunk.materials.iter().enumerate() {
            let indices: Vec<u16> = chunk
                .triangles
                .iter()
                .filter(|t| t.material as usize == i)
                .flat_map(|t| t.indices)
                .collect();
            if indices.is_empty() {
                continue;
            }
            let material = format!(
                "collision/{}",
                std::str::from_utf8(material)?.trim_end_matches('\0')
            );
            materials.insert(material.clone());
            let mut mesh = Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
            );
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, chunk.positions.clone());
            mesh.insert_indices(Indices::U16(indices));
            mesh.compute_smooth_normals();
            parts.push(Part {
                name: format!("collision/{}", String::from_utf8_lossy(entry.name)),
                material,
                mesh,
                matrices: vec![Mat4::IDENTITY],
                wheel: None,
            });
        }
    }
    println!(
        "collision chunks={} vertices={vertices} triangles={triangles} material_groups={} codes={materials:?} bounds={:?}..{:?}",
        archive.entries.len(),
        parts.len(),
        bounds.0,
        bounds.1
    );
    Ok(Car {
        parts,
        textures: BTreeMap::new(),
        bindings: BTreeMap::new(),
        center: (bounds.0 + bounds.1) * 0.5,
        radius: (bounds.1 - bounds.0).length() * 0.5,
        track: true,
    })
}
fn vector(value: &str) -> Result<Vec3, Box<dyn std::error::Error>> {
    let coordinates = value
        .split(',')
        .map(str::parse::<f32>)
        .collect::<Result<Vec<_>, _>>()?;
    if coordinates.len() != 3
        || coordinates
            .iter()
            .any(|v| !v.is_finite() || v.abs() > 100_000.)
    {
        return Err("camera vector needs three finite coordinates".into());
    }
    Ok(Vec3::new(coordinates[0], coordinates[1], coordinates[2]))
}
fn grid_slot(root: &Path, name: &str) -> Result<Mat4, Box<dyn std::error::Error>> {
    let bytes = read(&root.join("tracks/europe/donington/route_0/grids.pssg"))?;
    let tree = pssg::read_tree(&bytes)?;
    let marker = grid_formats::grid::read(&tree, name.as_bytes())?;
    let transform = marker.horizontal_preview()?;
    println!(
        "grid marker {name} raw_matrix={:?} corners={:?} forward={:?}; horizontal preview only, final contact height/offset pending",
        marker.raw_transform, marker.corners, marker.forward
    );
    Ok(Mat4::from_cols_array(&transform))
}
fn append_car(
    root: &Path,
    lod: &str,
    car: &mut Car,
    anchor: Mat4,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut lacetti = load(root, lod, false, false)?;
    for part in &mut lacetti.parts {
        part.material = format!("lacetti/{}", part.material);
        for matrix in &mut part.matrices {
            *matrix = anchor * *matrix;
        }
    }
    for (name, data) in lacetti.textures {
        car.textures.insert(format!("lacetti/{name}"), data);
    }
    for (name, texture) in lacetti.bindings {
        car.bindings
            .insert(format!("lacetti/{name}"), format!("lacetti/{texture}"));
    }
    car.parts.extend(lacetti.parts);
    Ok(())
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut root = PathBuf::from(r"C:\Program Files (x86)\Steam\steamapps\common\Grid");
    let mut output = None;
    let mut lod = String::from("LOD0_");
    let mut track = false;
    let mut include_batches = false;
    let mut reference = false;
    let mut collision = false;
    let mut start_view = false;
    let mut eye = None;
    let mut target = None;
    let mut drive = false;
    let mut drive_ticks = None;
    let mut body_contacts = None;
    let mut scripted = grid_tools::source_vehicle::Controls::default();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--drive" => drive = true,
            "--body-contacts" => {
                body_contacts = Some(PathBuf::from(
                    args.next()
                        .ok_or("--body-contacts needs a private CD01 path")?,
                ));
            }
            "--drive-ticks" => {
                drive_ticks = Some(args.next().ok_or("missing tick count")?.parse::<u64>()?)
            }
            "--throttle" => scripted.throttle = args.next().ok_or("missing throttle")?.parse()?,
            "--steering" => scripted.steering = args.next().ok_or("missing steering")?.parse()?,
            "--game-root" => root = args.next().ok_or("--game-root needs a path")?.into(),
            "--render" => {
                output = Some(PathBuf::from(
                    args.next().ok_or("--render needs a PNG path")?,
                ))
            }
            "--lod" => lod = args.next().ok_or("--lod needs LOD0_ or LOD1_")?,
            "--include-batches" => include_batches = true,
            "--camera" => eye = Some(vector(&args.next().ok_or("--camera needs X,Y,Z")?)?),
            "--look-at" => target = Some(vector(&args.next().ok_or("--look-at needs X,Y,Z")?)?),
            "--view" => {
                start_view = match args
                    .next()
                    .ok_or("--view needs overview or start")?
                    .as_str()
                {
                    "overview" => false,
                    "start" => true,
                    _ => return Err("unknown view".into()),
                };
            }
            "--scene" => {
                let scene = args.next().ok_or("--scene needs lacetti or donington")?;
                track = match scene.as_str() {
                    "lacetti" => {
                        collision = false;
                        reference = false;
                        false
                    }
                    "donington" => {
                        collision = false;
                        reference = false;
                        true
                    }
                    "reference" => {
                        collision = false;
                        reference = true;
                        true
                    }
                    "collision" => {
                        collision = true;
                        reference = false;
                        true
                    }
                    _ => return Err("unknown scene".into()),
                };
            }
            "--help" => {
                println!(
                    "grid-app [--game-root PATH] [--scene lacetti|donington|reference|collision] [--lod LOD0_|LOD1_] [--view overview|start] [--camera X,Y,Z --look-at X,Y,Z] [--include-batches] [--render local/PATH.png]\n--drive: source Lacetti/Donington driving prototype. Arrows/WASD drive; Space handbrake; P pause; R reset; Escape exit. Full body collisions and race rules are not implemented.\n--body-contacts PRIVATE_CD01: experimental Donington packed-track and CQTC boundary contacts; shared material, boundary-first order, above-threshold damage pauses driving.\n--drive-ticks N --throttle 0..1 --steering -1..1: bounded scripted render validation.\nStatic viewer: arrows orbit; WASD pan; Q/E vertical; Shift faster; PageUp/PageDown zoom; Home reset."
                );
                return Ok(());
            }
            _ => return Err(format!("unknown argument {arg}").into()),
        }
    }
    if drive_ticks.is_some_and(|n| n > 36_000) || (drive_ticks.is_some() && !drive) {
        return Err("scripted ticks require --drive and must be <=36000".into());
    }
    if body_contacts.is_some() && !drive {
        return Err("--body-contacts requires --drive".into());
    }
    if !scripted.throttle.is_finite()
        || !(0. ..=1.).contains(&scripted.throttle)
        || !scripted.steering.is_finite()
        || !(-1. ..=1.).contains(&scripted.steering)
    {
        return Err("invalid scripted controls".into());
    }
    let driving = if drive {
        track = true;
        reference = true;
        collision = false;
        start_view = true;
        let mut runtime = grid_tools::source_vehicle::Runtime::lacetti(&root, "slot_00", 200679)?;
        let geometry = body_contacts.map(|path| -> Result<_, Box<dyn std::error::Error>> {
            if std::fs::metadata(&path)?.len() != 712 {
                return Err("invalid private body sampling extent".into());
            }
            let table = std::fs::read(path)?;
            let geometry = runtime.prepare_body_geometry(&root, &table)?;
            println!("drive_body_geometry enabled=true pool=20 boundary=cqtc order=boundary-first-then-archive materials=shared damage=guarded");
            Ok(geometry)
        }).transpose()?;
        Some(Driving {
            runtime,
            geometry,
            paused: false,
            failure: None,
            scripted_ticks: drive_ticks,
            scripted,
            last_controls: None,
            segment: 0,
            started: Instant::now(),
        })
    } else {
        None
    };
    if lod != "LOD0_" && lod != "LOD1_" {
        return Err("unsupported LOD selection".into());
    }
    let output = output
        .map(|path| -> Result<PathBuf, Box<dyn std::error::Error>> {
            if path.extension().is_none_or(|ext| ext != "png") {
                return Err("render output must be PNG".into());
            }
            let parent = path
                .parent()
                .ok_or("output needs a parent directory")?
                .canonicalize()?;
            if !parent.starts_with(Path::new("local").canonicalize()?) {
                return Err("game-derived renders must stay under local/".into());
            }
            let result = parent.join(path.file_name().ok_or("missing output filename")?);
            if result.exists() {
                return Err("refusing to overwrite an existing render".into());
            }
            Ok(result)
        })
        .transpose()?;
    if eye.is_some() != target.is_some() {
        return Err("provide both --camera and --look-at".into());
    }
    if start_view && !track {
        return Err("start view requires a Donington scene".into());
    }
    let mut car = if collision {
        load_collision(&root)?
    } else {
        load(&root, &lod, track, include_batches)?
    };
    let anchor = if let Some(driving) = &driving {
        Some(Mat4::from_cols_array(&driving.runtime.pose()))
    } else if reference || start_view {
        Some(grid_slot(&root, "slot_00")?)
    } else {
        None
    };
    if reference {
        let model = if let Some(driving) = &driving {
            let owner = &driving.runtime.state.car.owner.vehicle;
            // Model wheel coordinates are NED coordinates; constructor shifts
            // them into its local physical frame. Apply that same translation.
            Mat4::from_translation(Vec3::new(
                f32::from_bits(owner[0x1630 / 4]),
                f32::from_bits(owner[0x1634 / 4]) - f32::from_bits(owner[0x1564 / 4]),
                f32::from_bits(owner[0x1638 / 4]),
            ))
        } else {
            anchor.unwrap()
        };
        append_car(&root, &lod, &mut car, model)?;
    }
    let mut orbit = if track {
        Orbit {
            yaw: -0.4,
            pitch: 0.85,
            distance: car.radius * 2.4,
            target: car.center,
            scale: car.radius,
        }
    } else {
        Orbit {
            yaw: -0.75,
            pitch: 0.25,
            distance: 7.0,
            target: Vec3::new(0., 0.65, 0.),
            scale: 3.0,
        }
    };
    if start_view || reference {
        let anchor = anchor.unwrap();
        let at = anchor.transform_point3(Vec3::new(0., 1.0, 0.));
        let position = anchor.transform_point3(Vec3::new(-8., 6., -12.));
        let offset = position - at;
        orbit = Orbit {
            yaw: offset.x.atan2(offset.z),
            pitch: (offset.y / offset.length()).asin(),
            distance: offset.length(),
            target: at,
            scale: 6.0,
        };
    }
    if let (Some(position), Some(at)) = (eye, target) {
        let offset = position - at;
        if offset.length() < 0.1 {
            return Err("camera and target too close".into());
        }
        orbit = Orbit {
            yaw: offset.x.atan2(offset.z),
            pitch: (offset.y / offset.length()).asin(),
            distance: offset.length(),
            target: at,
            scale: (offset.length() / 2.).max(1.),
        };
    }
    let mut app = App::new();
    if let Some(path) = output {
        app.add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: None,
                    exit_condition: ExitCondition::DontExit,
                    ..default()
                })
                .disable::<WinitPlugin>(),
        )
        .add_plugins(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
            1.0 / 60.0,
        )))
        .insert_resource(Capture {
            path,
            target: None,
            frames: 0,
            requested: false,
            started: Instant::now(),
        })
        .add_systems(Update, capture);
    } else {
        app.add_plugins(
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: if drive {
                        "GRID independent driving prototype"
                    } else {
                        "GRID independent static resource viewer"
                    }
                    .into(),
                    resolution: (1280, 800).into(),
                    ..default()
                }),
                ..default()
            }),
        );
        if !drive {
            app.add_systems(Update, update_orbit);
        }
    }
    if let Some(driving) = driving {
        app.insert_resource(driving)
            .insert_resource(Time::<Fixed>::from_seconds(f64::from(
                grid_tools::source_vehicle::DELTA,
            )))
            .add_systems(FixedUpdate, drive_tick)
            .add_systems(Last, drive_exit_log)
            .add_systems(Update, (drive_actions, drive_present).chain());
    }
    let exit = app
        .insert_resource(car)
        .insert_resource(InitialOrbit(orbit.clone()))
        .insert_resource(orbit)
        .insert_resource(ClearColor(Color::srgb(0.055, 0.07, 0.095)))
        .add_systems(Startup, setup)
        .run();
    if !matches!(exit, AppExit::Success) {
        return Err("renderer exited with an error".into());
    }
    Ok(())
}
fn camera_transform(orbit: &Orbit) -> Transform {
    let target = orbit.target;
    let offset = Vec3::new(
        orbit.yaw.sin() * orbit.pitch.cos(),
        orbit.pitch.sin(),
        orbit.yaw.cos() * orbit.pitch.cos(),
    ) * orbit.distance;
    Transform::from_translation(target + offset).looking_at(target, Vec3::Y)
}
// Bevy system parameters are separate ECS borrows, not a public call interface.
#[allow(clippy::too_many_arguments)]
fn setup(
    mut commands: Commands,
    mut car: ResMut<Car>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    orbit: Res<Orbit>,
    capture: Option<ResMut<Capture>>,
    adapter: Res<bevy::render::renderer::RenderAdapterInfo>,
    device: Res<bevy::render::renderer::RenderDevice>,
    driving: Option<Res<Driving>>,
) {
    println!(
        "renderer adapter={:?} backend={:?} driver={:?} driver_info={:?} features={:?}",
        adapter.name,
        adapter.backend,
        adapter.driver,
        adapter.driver_info,
        device.features()
    );
    assert!(
        device
            .features()
            .contains(bevy::render::render_resource::WgpuFeatures::TEXTURE_COMPRESSION_BC),
        "viewer requires GPU BC texture support"
    );
    let mut texture_handles = BTreeMap::new();
    for (name, dds) in &car.textures {
        let image = Image::from_buffer(
            dds,
            ImageType::Extension("dds"),
            CompressedImageFormats::BC,
            true,
            if car.track {
                ImageSampler::Descriptor(ImageSamplerDescriptor {
                    address_mode_u: ImageAddressMode::Repeat,
                    address_mode_v: ImageAddressMode::Repeat,
                    ..ImageSamplerDescriptor::linear()
                })
            } else {
                ImageSampler::Default
            },
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        )
        .expect("validated DDS image");
        texture_handles.insert(name.clone(), images.add(image));
    }
    let mut material_handles = BTreeMap::new();
    let bindings = car.bindings.clone();
    let track = car.track;
    for part in car.parts.drain(..) {
        let material = material_handles
            .entry(part.material.clone())
            .or_insert_with(|| {
                let name = part
                    .material
                    .strip_prefix("lacetti/")
                    .unwrap_or(&part.material)
                    .trim();
                let glass = matches!(
                    name,
                    "Window Glass" | "Grills Glass" | "Head Lights Glass" | "Rear Lights Glass"
                );
                materials.add(StandardMaterial {
                    base_color_texture: bindings
                        .get(&part.material)
                        .map(|texture| texture_handles[texture].clone()),
                    base_color: if name.starts_with("collision/") {
                        let rgb = match name.strip_prefix("collision/").unwrap() {
                            "STDR" => [0.23, 0.25, 0.28],
                            "GRAS" => [0.3, 0.47, 0.17],
                            "LGDR" => [0.63, 0.49, 0.32],
                            "MPDR" => [0.48, 0.51, 0.56],
                            "RMBL" => [0.82, 0.25, 0.23],
                            "ROCK" => [0.55, 0.47, 0.39],
                            "MUD" => [0.27, 0.18, 0.12],
                            "CONC" => [0.67, 0.68, 0.68],
                            _ => [1., 0., 1.],
                        };
                        Color::srgb(rgb[0], rgb[1], rgb[2])
                    } else if track && name == "batchmaterial" {
                        Color::srgb(0.28, 0.32, 0.38)
                    } else {
                        Color::WHITE
                    },
                    perceptual_roughness: if name == "Car Body Paint" { 0.32 } else { 0.7 },
                    metallic: if matches!(name, "Wheel" | "Bare Metal" | "Shiny Metal" | "Discs") {
                        0.6
                    } else {
                        0.0
                    },
                    alpha_mode: if name.starts_with("objects/") {
                        AlphaMode::Mask(0.5)
                    } else if glass {
                        AlphaMode::Blend
                    } else {
                        AlphaMode::Opaque
                    },
                    unlit: name.starts_with("collision/"),
                    double_sided: glass || name.starts_with("collision/"),
                    cull_mode: if glass || name.starts_with("collision/") {
                        None
                    } else {
                        Some(Face::Back)
                    },
                    ..default()
                })
            })
            .clone();
        let mesh = meshes.add(part.mesh);
        for matrix in part.matrices {
            let moving = driving.is_some() && part.material.starts_with("lacetti/");
            let transform = if moving {
                let runtime = &driving.as_ref().unwrap().runtime;
                let motion = part
                    .wheel
                    .map_or(Mat4::IDENTITY, |i| wheel_motion(runtime.wheels()[i]));
                Mat4::from_cols_array(&runtime.pose()) * motion * matrix
            } else {
                matrix
            };
            let mut entity = commands.spawn((
                Name::new(part.name.clone()),
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
                Transform::from_matrix(transform),
            ));
            if moving {
                entity.insert(MovingPart {
                    rest: matrix,
                    wheel: part.wheel,
                });
            }
        }
    }
    if !track {
        commands.spawn((
            Mesh3d(meshes.add(Plane3d::default().mesh().size(200.0, 200.0))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.16, 0.19, 0.23),
                perceptual_roughness: 0.9,
                ..default()
            })),
            Transform::from_xyz(0.0, -0.05, 0.0),
        ));
    }
    commands.spawn((
        DirectionalLight {
            illuminance: 18_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(-4.0, 8.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.insert_resource(GlobalAmbientLight {
        color: Color::WHITE,
        brightness: 450.0,
        ..default()
    });
    let camera = commands
        .spawn((Camera3d::default(), camera_transform(&orbit), ViewerCamera))
        .id();
    if let Some(mut capture) = capture {
        let mut image = Image::new_target_texture(1280, 800, TextureFormat::Rgba8UnormSrgb, None);
        image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
        let target = images.add(image);
        commands
            .entity(camera)
            .insert(RenderTarget::Image(target.clone().into()));
        capture.target = Some(target);
    }
}
fn capture(
    mut commands: Commands,
    mut capture: ResMut<Capture>,
    mut exit: MessageWriter<AppExit>,
    driving: Option<Res<Driving>>,
) {
    capture.frames += 1;
    if capture.started.elapsed() > Duration::from_secs(120) {
        error!("render capture timed out");
        exit.write(AppExit::error());
        return;
    }
    if let Some(driving) = &driving {
        if driving.failure.is_some() {
            exit.write(AppExit::error());
            return;
        }
        if driving
            .scripted_ticks
            .is_some_and(|n| driving.runtime.ticks < n)
        {
            return;
        }
    }
    if capture.frames >= 180 && !capture.requested {
        capture.requested = true;
        let path = capture.path.clone();
        commands
            .spawn(Screenshot::image(capture.target.as_ref().unwrap().clone()))
            .observe(
                move |event: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
                    let result = event
                        .image
                        .clone()
                        .try_into_dynamic()
                        .map_err(|e| format!("{e:?}"))
                        .and_then(|image| image.to_rgb8().save(&path).map_err(|e| e.to_string()));
                    match result {
                        Ok(()) => {
                            println!("saved {}", path.display());
                            exit.write(AppExit::Success);
                        }
                        Err(e) => {
                            error!("capture: {e}");
                            exit.write(AppExit::error());
                        }
                    }
                },
            );
    }
}
fn drive_tick(mut driving: ResMut<Driving>, input: Res<ButtonInput<KeyCode>>) {
    if driving.paused
        || driving.failure.is_some()
        || driving
            .scripted_ticks
            .is_some_and(|n| driving.runtime.ticks >= n)
    {
        return;
    }
    let controls = if driving.scripted_ticks.is_some() {
        driving.scripted
    } else {
        let pressed = |a, b| input.pressed(a) || input.pressed(b);
        grid_tools::source_vehicle::Controls {
            throttle: f32::from(pressed(KeyCode::ArrowUp, KeyCode::KeyW)),
            brake: f32::from(pressed(KeyCode::ArrowDown, KeyCode::KeyS)),
            steering: f32::from(pressed(KeyCode::ArrowLeft, KeyCode::KeyA))
                - f32::from(pressed(KeyCode::ArrowRight, KeyCode::KeyD)),
            handbrake: input.pressed(KeyCode::Space),
        }
    };
    let driving = &mut *driving;
    let result = if let Some(geometry) = &driving.geometry {
        driving.runtime.tick_with_body_geometry(controls, geometry)
    } else {
        driving.runtime.tick(controls)
    };
    if let Err(error) = result {
        let text = format!("tick {}: {error}; R resets", driving.runtime.ticks + 1);
        error!("driving paused: {text}");
        driving.failure = Some(text);
        driving.report("failure", controls);
    } else {
        if driving.last_controls != Some(controls)
            || driving.runtime.ticks.is_multiple_of(16)
            || driving.runtime.last_body_contacts > 0
            || driving.scripted_ticks == Some(driving.runtime.ticks)
        {
            driving.report("tick", controls);
        }
        driving.last_controls = Some(controls);
    }
    if driving.failure.is_none() && driving.scripted_ticks == Some(driving.runtime.ticks) {
        println!(
            "drive_script_complete ticks={} pose={:?} velocity={:?}",
            driving.runtime.ticks,
            driving.runtime.pose(),
            driving.runtime.velocity()
        );
    }
}
fn drive_actions(
    mut driving: ResMut<Driving>,
    input: Res<ButtonInput<KeyCode>>,
    mut exit: MessageWriter<AppExit>,
) {
    if input.just_pressed(KeyCode::KeyR) {
        driving.report("reset", driving.last_controls.unwrap_or_default());
        driving.runtime.reset();
        driving.segment += 1;
        driving.last_controls = None;
        driving.failure = None;
        driving.paused = false;
    }
    if input.just_pressed(KeyCode::KeyP) {
        driving.paused = !driving.paused;
        driving.report(
            if driving.paused { "pause" } else { "resume" },
            driving.last_controls.unwrap_or_default(),
        );
    }
    if input.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
}
fn drive_exit_log(driving: Res<Driving>, mut exits: MessageReader<AppExit>) {
    if exits.read().next().is_some() {
        driving.report("exit", driving.last_controls.unwrap_or_default());
    }
}
fn drive_present(
    driving: Res<Driving>,
    mut parts: Query<(&MovingPart, &mut Transform), Without<ViewerCamera>>,
    mut camera: Single<&mut Transform, With<ViewerCamera>>,
    mut windows: Query<&mut Window>,
) {
    let pose = Mat4::from_cols_array(&driving.runtime.pose());
    let wheels = driving.runtime.wheels().map(wheel_motion);
    for (part, mut transform) in &mut parts {
        let motion = part.wheel.map_or(Mat4::IDENTITY, |i| wheels[i]);
        *transform = Transform::from_matrix(pose * motion * part.rest);
    }
    let at = pose.transform_point3(Vec3::new(0., 1., 0.));
    let eye = pose.transform_point3(Vec3::new(0., 3., -9.));
    **camera = Transform::from_translation(eye).looking_at(at, Vec3::Y);
    let v = driving.runtime.velocity();
    let speed = Vec3::new(v[0], v[1], v[2]).length() * 3.6;
    let gear = driving.runtime.state.car.owner.vehicle[0x196c / 4];
    let gear = match gear {
        0 => "N".into(),
        9 => "R".into(),
        n => n.to_string(),
    };
    let handbrake = if driving.last_controls.is_some_and(|c| c.handbrake) {
        " | HB"
    } else {
        ""
    };
    let geometry = if driving.geometry.is_some() {
        format!(" | BODY {}", driving.runtime.last_body_contacts)
    } else {
        String::new()
    };
    for mut window in &mut windows {
        window.title = format!(
            "GRID prototype | {speed:.1} km/h | gear {gear}{handbrake}{geometry} | tick {} | {}",
            driving.runtime.ticks,
            driving.failure.as_deref().unwrap_or(if driving.paused {
                "Paused; P resumes; R resets"
            } else {
                "Arrows drive; Space handbrake; P pause; R reset"
            })
        );
    }
}
fn update_orbit(
    input: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut orbit: ResMut<Orbit>,
    mut camera: Single<&mut Transform, With<ViewerCamera>>,
    mut exit: MessageWriter<AppExit>,
    initial: Res<InitialOrbit>,
) {
    let dt = time.delta_secs().min(0.1);
    let speed = dt
        * orbit.scale.clamp(1.0, 80.0)
        * if input.pressed(KeyCode::ShiftLeft) || input.pressed(KeyCode::ShiftRight) {
            5.0
        } else {
            1.0
        };
    let forward = *camera.forward();
    let right = *camera.right();
    if input.pressed(KeyCode::KeyW) {
        orbit.target += forward * speed;
    }
    if input.pressed(KeyCode::KeyS) {
        orbit.target -= forward * speed;
    }
    if input.pressed(KeyCode::KeyD) {
        orbit.target += right * speed;
    }
    if input.pressed(KeyCode::KeyA) {
        orbit.target -= right * speed;
    }
    if input.pressed(KeyCode::KeyE) {
        orbit.target.y += speed;
    }
    if input.pressed(KeyCode::KeyQ) {
        orbit.target.y -= speed;
    }
    if input.pressed(KeyCode::ArrowLeft) {
        orbit.yaw -= dt;
    }
    if input.pressed(KeyCode::ArrowRight) {
        orbit.yaw += dt;
    }
    if input.pressed(KeyCode::ArrowUp) {
        orbit.pitch += dt;
    }
    if input.pressed(KeyCode::ArrowDown) {
        orbit.pitch -= dt;
    }
    if input.pressed(KeyCode::PageUp) {
        orbit.distance -= dt * orbit.scale;
    }
    if input.pressed(KeyCode::PageDown) {
        orbit.distance += dt * orbit.scale;
    }
    orbit.pitch = orbit.pitch.clamp(-0.1, 1.4);
    orbit.distance = orbit.distance.clamp(orbit.scale * 0.5, orbit.scale * 8.0);
    if input.just_pressed(KeyCode::Home) {
        *orbit = initial.0.clone();
    }
    if input.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
    **camera = camera_transform(&orbit);
}

#[cfg(test)]
mod driving_tests {
    use super::*;

    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn source_wheel_joints_follow_physics_without_feedback() {
        let root = std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required");
        let root = Path::new(&root);
        let car = load(root, "LOD0_", false, false).unwrap();
        let mut runtime =
            grid_tools::source_vehicle::Runtime::lacetti(root, "slot_00", 200679).unwrap();
        let mut counts = [0; 4];
        for part in &car.parts {
            if let Some(i) = part.wheel {
                counts[i] += 1;
            }
        }
        // Rim, two tyre materials and rotating brake disc per corner.
        assert_eq!(counts, [4; 4]);
        let initial = runtime.wheels().map(wheel_motion);
        let b = &runtime.state.car.owner.vehicle;
        let model_offset = Mat4::from_translation(Vec3::new(
            f32::from_bits(b[0x1630 / 4]),
            f32::from_bits(b[0x1634 / 4]) - f32::from_bits(b[0x1564 / 4]),
            f32::from_bits(b[0x1638 / 4]),
        ));
        for part in &car.parts {
            if let Some(i) = part.wheel {
                // Source NED and PSSG were authored with small distinct pivot
                // offsets. Preserve them, while detecting ordinal/axis swaps.
                let pivot = (model_offset * part.matrices[0]).w_axis.truncate();
                assert!(pivot.distance(Vec3::from_array(runtime.wheels()[i].anchor)) < 0.003);
            }
        }
        let mut moved = [false; 4];
        for tick in 0..160 {
            runtime
                .tick(grid_tools::source_vehicle::Controls {
                    throttle: 1.,
                    steering: 0.2,
                    ..Default::default()
                })
                .unwrap();
            let owner_before = runtime.state.car.owner.vehicle.clone();
            let body_before = runtime.state.car.owner.physical;
            for (i, wheel) in runtime.wheels().into_iter().enumerate() {
                let motion = wheel_motion(wheel);
                assert!(motion.is_finite());
                let anchor = Vec3::from_array(wheel.anchor);
                let center = anchor + Vec3::from_array(wheel.axis) * wheel.displacement;
                assert!((motion.transform_point3(anchor) - center).length() < 1e-5);
                // Spin leaves the axle invariant; steered axle is the source
                // [cos,0,-sin] used by the tyre frame, after normalization.
                let expected = Vec3::new(wheel.steering[0], 0., -wheel.steering[1]).normalize();
                assert!((motion.transform_vector3(Vec3::X) - expected).length() < 1e-5);
                assert!((motion.determinant() - 1.).abs() < 1e-5);
                moved[i] |= motion != initial[i];
                // Rim and tyre bind offsets must rotate around the common
                // physics center rather than orbit around the car origin.
                if tick == 159 {
                    for part in car.parts.iter().filter(|p| p.wheel == Some(i)) {
                        let bind = model_offset * part.matrices[0];
                        let before = bind.w_axis.truncate().distance(anchor);
                        let after = (motion * bind).w_axis.truncate().distance(center);
                        assert!((before - after).abs() < 1e-5);
                    }
                }
            }
            assert_eq!(runtime.state.car.owner.vehicle, owner_before);
            assert_eq!(runtime.state.car.owner.physical, body_before);
        }
        assert_eq!(moved, [true; 4]);
        runtime.reset();
        assert_eq!(runtime.wheels().map(wheel_motion), initial);
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn keyboard_pause_reset_and_failure_recovery() {
        exercise_keyboard_recovery(None);
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS"]
    fn body_geometry_keyboard_pause_reset_and_failure_recovery() {
        let table = std::fs::read(std::env::var_os("GRID_CONTACT_DIRECTIONS").unwrap()).unwrap();
        exercise_keyboard_recovery(Some(&table));
    }

    fn exercise_keyboard_recovery(table: Option<&[u8]>) {
        let root = std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required");
        let mut runtime =
            grid_tools::source_vehicle::Runtime::lacetti(Path::new(&root), "slot_00", 200679)
                .unwrap();
        let geometry = table.map(|table| {
            runtime
                .prepare_body_geometry(Path::new(&root), table)
                .unwrap()
        });
        let mut app = App::new();
        app.add_message::<AppExit>()
            .insert_resource(ButtonInput::<KeyCode>::default())
            .insert_resource(Driving {
                runtime,
                geometry,
                paused: false,
                failure: None,
                scripted_ticks: None,
                scripted: Default::default(),
                last_controls: None,
                segment: 0,
                started: Instant::now(),
            })
            .add_systems(Update, (drive_actions, drive_tick).chain());
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowUp);
        for _ in 0..120 {
            app.update();
        }
        assert_eq!(app.world().resource::<Driving>().runtime.ticks, 120);
        assert!(
            app.world().resource::<Driving>().runtime.velocity()[..3]
                .iter()
                .map(|v| v * v)
                .sum::<f32>()
                > 1.
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowLeft);
        app.update();
        assert_eq!(
            app.world()
                .resource::<Driving>()
                .runtime
                .state
                .car
                .owner
                .vehicle[0xe30 / 4],
            1f32.to_bits()
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::ArrowLeft);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowRight);
        app.update();
        assert_eq!(
            app.world()
                .resource::<Driving>()
                .runtime
                .state
                .car
                .owner
                .vehicle[0xe30 / 4],
            (-1f32).to_bits()
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyP);
        app.update();
        assert!(app.world().resource::<Driving>().paused);
        assert_eq!(app.world().resource::<Driving>().runtime.ticks, 122);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyR);
        app.update();
        assert_eq!(app.world().resource::<Driving>().runtime.ticks, 1);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        // An inconsistent authoritative owner must pause, preserve its tick,
        // and allow the same R path to restore a valid source world.
        app.world_mut()
            .resource_mut::<Driving>()
            .runtime
            .state
            .car
            .owner
            .vehicle[0x15a0 / 4] = 0;
        app.update();
        assert!(app.world().resource::<Driving>().failure.is_some());
        assert_eq!(app.world().resource::<Driving>().runtime.ticks, 1);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyR);
        app.update();
        assert!(app.world().resource::<Driving>().failure.is_none());
        assert_eq!(app.world().resource::<Driving>().runtime.ticks, 1);
        // Exercise the actual keyboard path, not just the runtime control flag.
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowUp);
        for _ in 0..120 {
            app.update();
        }
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Space);
        app.update();
        let driving = app.world().resource::<Driving>();
        assert!(driving.last_controls.unwrap().handbrake);
        assert!(
            driving.runtime.state.car.frame.view[0x28 / 4..0x30 / 4]
                .iter()
                .all(|v| f32::from_bits(*v) > 1000.)
        );
        for _ in 0..59 {
            app.update();
        }
        assert!(
            app.world().resource::<Driving>().runtime.velocity()[..3]
                .iter()
                .map(|v| v * v)
                .sum::<f32>()
                < 0.01
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyR);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowDown);
        for _ in 0..120 {
            app.update();
        }
        let driving = app.world().resource::<Driving>();
        assert_eq!(driving.segment, 3);
        assert_eq!(driving.runtime.state.car.owner.vehicle[0x196c / 4], 9);
        let p = driving.runtime.pose();
        let v = driving.runtime.velocity();
        assert!(v[0] * p[8] + v[1] * p[9] + v[2] * p[10] < -1.);
    }
}
