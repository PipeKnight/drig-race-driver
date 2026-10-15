//! Source reference viewer and serial driving prototype.
mod body_damage;
mod car_materials;
mod engine_sound;
mod free_look;
mod graphics;
mod graphics_benchmark;
mod sky;
mod sky_environment;
mod time_trial;
mod track_grade;
mod tyre_sound;
mod tyre_visual;
use bevy::{
    app::{AppExit, ScheduleRunnerPlugin},
    asset::RenderAssetUsages,
    camera::RenderTarget,
    camera::primitives::{Aabb, MeshAabb},
    image::{CompressedImageFormats, ImageType},
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

const BUILD_REVISION: &str = match option_env!("GRID_BUILD_REVISION") {
    Some(value) => value,
    None => "unversioned",
};

#[derive(Resource)]
struct Car {
    parts: Vec<Part>,
    textures: BTreeMap<String, Vec<u8>>,
    bindings: BTreeMap<String, String>,
    center: Vec3,
    radius: f32,
    track: bool,
    /// Opponent livery textures (car k uses entry k-1), keyed like `textures`.
    liveries: Vec<BTreeMap<String, Vec<u8>>>,
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
#[derive(Component)]
struct DrivingHud;

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
    opponents: Option<Opponents>,
}
/// Shared-world opponents. Car 0 of `multi` is a placeholder that is swapped
/// with the player runtime for each common tick.
struct Opponents {
    multi: grid_tools::source_vehicle::multi::MultiRuntime,
    drivers: Vec<grid_tools::ai_driver::AiDriver>,
    ai: grid_tools::ai_driver::AiData,
    /// Race positions during a timed event (authored classification).
    standings: grid_tools::race::Standings,
    names: Vec<String>,
    /// Scripted validation only (GRID_AUTOPILOT=1 with --drive-ticks): the
    /// ported AI drives the player's car.
    autopilot: Option<grid_tools::ai_driver::AiDriver>,
    /// Last throttle of each opponent (car k at k-1), for engine sound.
    throttles: Vec<f32>,
}
impl Opponents {
    /// Positions of every car; car 0 is the player runtime held outside `multi`.
    fn positions(&self, player: &grid_tools::source_vehicle::Runtime) -> Vec<[f32; 3]> {
        std::iter::once(player)
            .chain(self.multi.cars.iter().skip(1))
            .map(|c| {
                let p = c.pose();
                [p[12], p[13], p[14]]
            })
            .collect()
    }
}
#[derive(Component)]
struct OpponentPart {
    car: usize,
    rest: Mat4,
    wheel: Option<usize>,
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
        let damage_levels = [0, 4, 8, 0x10, 0x14, 0x28, 0x2c, 0x30, 0x34]
            .map(|o| f32::from_bits(b[(0xcdf0 + o) / 4]));
        println!(
            "drive_sample version=1 event={event} segment={} tick={} seconds={:.6} wall_seconds={:.6} throttle={} brake={} steering={} handbrake={} gear={} wheel_hits={hits} pose={:?} velocity={:?} wheel_brakes={brakes:?} body_geometry={} body_contacts={} body_events={} damage_rng_owned={} damage_rng={:08x} impact_flag={} suspension_gain={} damage_levels={damage_levels:?}",
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
            self.runtime.last_body_contacts,
            self.runtime.body_events.is_some(),
            self.runtime.state.car.owner.damage_random_state.is_some(),
            self.runtime
                .state
                .car
                .owner
                .damage_random_state
                .unwrap_or(0),
            (b[0xba5 / 4] >> 8) & 255,
            f32::from_bits(b[0x3d0 / 4])
        );
        let m = 0xcdf0;
        let effects = (0..8)
            .map(|i| f32::from_bits(b[(m + 0x3a0) / 4 + i]))
            .collect::<Vec<_>>();
        let wheel_levels =
            std::array::from_fn::<_, 4, _>(|i| f32::from_bits(b[(m + 0x18) / 4 + i]));
        println!(
            "drive_handling_sample version=1 event={event} segment={} tick={} status={} terminal_wheel={} effects_3a0_3bc={effects:?} wheel_levels={wheel_levels:?} controls_disabled={}",
            self.segment,
            self.runtime.ticks,
            b[0xd234 / 4],
            self.runtime
                .terminal_wheel()
                .map_or_else(|| "none".into(), |i| i.to_string()),
            f32::from_bits(b[0x226c / 4])
        );
        if let Some(registration) = &self.runtime.deformation_registration {
            let count = (0..3)
                .map(|i| b[0xc714 / 4 + i] as usize)
                .product::<usize>();
            let moved = (0..count)
                .filter(|&i| {
                    (0..3).any(|a| b[0x4880 / 4 + i * 20 + a] != b[0x4890 / 4 + i * 20 + a])
                })
                .count();
            let maximum = (0..count)
                .map(|i| {
                    (0..3)
                        .map(|a| {
                            let d = f32::from_bits(b[0x4880 / 4 + i * 20 + a])
                                - f32::from_bits(b[0x4890 / 4 + i * 20 + a]);
                            d * d
                        })
                        .sum::<f32>()
                        .sqrt()
                })
                .fold(0f32, f32::max);
            println!(
                "drive_deformation_sample version=1 event={event} segment={} tick={} bulk_calls={} records={} crt_seed={:08x} mode={} pending_geometry={} moved_positions={moved} maximum_displacement={maximum}",
                self.segment,
                self.runtime.ticks,
                registration.bulk_calls,
                registration.records,
                registration.random_seed,
                b[0xc7a0 / 4],
                b[0xc7f0 / 4] >> 16 & 255
            );
        }
        if let Some(lap) = &self.runtime.lap {
            let elapsed = lap
                .elapsed()
                .map_or_else(|| "none".into(), |s| format!("{s:.9}"));
            let best = lap
                .best_lap
                .map_or_else(|| "none".into(), |s| format!("{s:.9}"));
            let status = if lap.elapsed().is_none() {
                "waiting"
            } else if lap.current_valid {
                "running"
            } else {
                "invalid"
            };
            println!(
                "drive_lap_sample version=1 event={event} segment={} tick={} lap={} status={status} elapsed={elapsed} next_gate={} passed_gates={} total_gates={} required_gates={} valid_laps={} attempts={} best={best}",
                self.segment,
                self.runtime.ticks,
                lap.lap_number(),
                lap.next_gate,
                lap.passed_gates(),
                self.runtime.lap_gate_count(),
                self.runtime.lap_required_gate_count(),
                lap.valid_laps,
                lap.attempts
            );
        }
    }
}
#[derive(Component)]
struct MovingPart {
    rest: Mat4,
    wheel: Option<usize>,
}
#[derive(Component)]
struct DeformingPart {
    source: Mesh,
    raw_bind: Mat4,
}
#[derive(Resource, Default)]
struct DeformationPresentation(Option<Vec<u32>>);
/// Donington start-light board (`start_lights.xml`: object
/// startlights_lights_horizontal, 6 stages over 4.0 s). The emissive texture
/// is swapped per stage; `stage` 0 is the idle board.
#[derive(Resource)]
struct StartLights {
    material: Handle<StandardMaterial>,
    /// Idle, then stages 1..6 (textures 1..5 and 7).
    textures: Vec<Handle<Image>>,
    stage: usize,
}
const START_LIGHT_TEXTURES: [&str; 7] = [
    "start_lights_e.tga",
    "start_lights_1_e.tga",
    "start_lights_2_e.tga",
    "start_lights_3_e.tga",
    "start_lights_4_e.tga",
    "start_lights_5_e.tga",
    "start_lights_7_e.tga",
];
const START_SEQUENCE_SECONDS: f64 = 4.0;
/// Body dents of one opponent part, projected from that car's own field.
#[derive(Component)]
struct OpponentDeforming(DeformingPart);

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
/// command or integrate a second wheel clock. Deformation uses the owned whole
/// wheel kernel; this render composition remains an independent presentation.
fn wheel_motion(wheel: grid_tools::source_vehicle::WheelPose) -> Mat4 {
    let anchor = Vec3::from_array(wheel.anchor);
    let center = anchor
        + Vec3::from_array(wheel.deformation_translation)
        + Vec3::from_array(wheel.axis) * wheel.displacement;
    let yaw = wheel.steering[1].atan2(wheel.steering[0]);
    Mat4::from_translation(center)
        * Mat4::from_cols_array_2d(&wheel.deformation_orientation)
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
        liveries: Vec::new(),
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
        liveries: Vec::new(),
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
/// Lacetti livery package `livery_NN` (shipped 00..09), keyed `lacetti/<texture>`.
fn load_livery(
    root: &Path,
    index: usize,
) -> Result<BTreeMap<String, Vec<u8>>, Box<dyn std::error::Error>> {
    let path = format!("cars/lac/livery_{index:02}/textures_high/lac_tex_high_{index:02}.pssg");
    let bytes = read(&root.join(path))?;
    let tree = pssg::read_tree(&bytes)?;
    let mut out = BTreeMap::new();
    for texture in texture::read_textures(&tree)? {
        let (dds, _) = texture.validated_dds();
        out.insert(
            format!("lacetti/{}", String::from_utf8(texture.id.to_vec())?),
            dds,
        );
    }
    Ok(out)
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    println!("prototype_build revision={BUILD_REVISION}");
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
    let mut audio_check = false;
    let mut graphics = graphics::Settings::default();
    let mut trial_laps = None;
    let mut drive_ticks = None;
    let mut body_contacts = None;
    let mut body_events = None;
    let mut deformation = None;
    let mut opponents = None;
    let mut grid_position = None;
    let mut ai_characteristics = None;
    let mut scripted = grid_tools::source_vehicle::Controls::default();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--drive" => drive = true,
            "--audio-check" => audio_check = true,
            "--graphics" => {
                graphics.preset =
                    graphics::Preset::parse(&args.next().ok_or("missing graphics preset")?)?;
            }
            "--resolution" => graphics.resolution(&args.next().ok_or("missing resolution")?)?,
            "--car-materials" => {
                graphics.layered_car = match args.next().as_deref() {
                    Some("legacy") => false,
                    Some("layered") => true,
                    _ => return Err("--car-materials needs legacy or layered".into()),
                }
            }
            "--ibl" => {
                graphics.source_ibl = match args.next().as_deref() {
                    Some("source") => true,
                    Some("procedural") => false,
                    _ => return Err("--ibl needs source or procedural".into()),
                }
            }
            "--tyre-detail" => {
                graphics.smooth_tyres = match args.next().as_deref() {
                    Some("source") => false,
                    Some("smooth") => true,
                    _ => return Err("--tyre-detail needs source or smooth".into()),
                }
            }
            "--grade" => {
                graphics.track_grade = match args.next().as_deref() {
                    Some("authored") => false,
                    Some("track") => true,
                    _ => return Err("--grade needs authored or track".into()),
                }
            }
            "--benchmark-frames" => {
                let count = args
                    .next()
                    .ok_or("missing benchmark frame count")?
                    .parse::<u32>()?;
                if !(1..=3600).contains(&count) {
                    return Err("benchmark frame count must be 1..3600".into());
                }
                graphics.benchmark_frames = Some(count);
            }
            "--sky" => {
                graphics.source_sky = match args.next().as_deref() {
                    Some("source") => true,
                    Some("procedural") => false,
                    _ => return Err("--sky needs source or procedural".into()),
                }
            }
            "--time-trial-laps" => {
                trial_laps = Some(
                    args.next()
                        .ok_or("--time-trial-laps needs 1..99")?
                        .parse::<u32>()?,
                )
            }
            "--deformation" => {
                deformation = Some(PathBuf::from(
                    args.next()
                        .ok_or("--deformation needs a private DF01 path")?,
                ))
            }
            "--opponents" => {
                opponents = Some(
                    args.next()
                        .ok_or("--opponents needs 1..11")?
                        .parse::<usize>()?,
                )
            }
            "--ai-characteristics" => {
                ai_characteristics = Some(PathBuf::from(
                    args.next()
                        .ok_or("--ai-characteristics needs a private TSV path")?,
                ))
            }
            "--grid-position" => {
                grid_position = Some(
                    args.next()
                        .ok_or("--grid-position needs 1..opponents+1")?
                        .parse::<usize>()?,
                )
            }
            "--body-events" => {
                body_events = Some(PathBuf::from(
                    args.next()
                        .ok_or("--body-events needs a private BP01 path")?,
                ));
            }
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
                    "--grade authored|track: enhanced color treatment; track reads saved exterior controls, with an authored display-domain mapping (not 1:1 original postprocessing)."
                );
                println!(
                    "--benchmark-frames 1..3600: main-schedule wall intervals after 240 warmup frames; requires --render; stops scripted driving early at benchmark completion. Not GPU/presentation FPS."
                );
                println!(
                    "--sky source|procedural: enhanced Donington background (source is default; sky IBL remains procedural)."
                );
                println!(
                    "--graphics baseline|enhanced: independent rendering presets; baseline is the current prototype.\n--resolution WIDTHxHEIGHT: window or offscreen size, e.g. 3840x2160."
                );
                println!(
                    "grid-app [--game-root PATH] [--scene lacetti|donington|reference|collision] [--lod LOD0_|LOD1_] [--view overview|start] [--camera X,Y,Z --look-at X,Y,Z] [--include-batches] [--render local/PATH.png]\n--drive: source Lacetti/Donington driving prototype. Arrows/WASD drive; Space handbrake; P pause; R reset; Escape exit. Free practice is the default.\n--time-trial-laps 1..99: independent finite event, 3-second countdown, results and private personal bests.\n--body-contacts PRIVATE_CD01: experimental Donington packed-track and CQTC boundary contacts with selected collision damage; shared material and boundary-first order. Terminal wheel damage disables driving; R resets.\n--drive-ticks N --throttle 0..1 --steering -1..1: bounded scripted render validation.\nStatic viewer: arrows orbit; WASD pan; Q/E vertical; Shift faster; PageUp/PageDown zoom; Home reset."
                );
                println!(
                    "--opponents 1..11: AI cars in the shared world (needs --body-contacts); --grid-position N: player grid slot (1 = pole)."
                );
                println!(
                    "--body-events PRIVATE_BP01: experimental registered impacts after solving, including wheel contacts. Single car/static peers and empty notifications; EVENTS appears in the title."
                );
                println!(
                    "--deformation PRIVATE_DF01: experimental source grid, wheel/shape rebuilding, physical callbacks and independently mapped body dents. Requires --drive and --body-contacts."
                );
                return Ok(());
            }
            _ => return Err(format!("unknown argument {arg}").into()),
        }
    }
    if audio_check {
        return engine_sound::check(&root);
    }
    if graphics.benchmark_frames.is_some() && output.is_none() {
        return Err("--benchmark-frames requires --render for bounded unattended exit".into());
    }
    if graphics.track_grade {
        if !graphics.enhanced() {
            return Err("--grade track requires --graphics enhanced".into());
        }
        let bytes = read(&root.join("tracks/europe/donington/effects.xml"))?;
        let grade = grid_formats::graphics_grade::read(&bytes, "Effect_0000")?;
        println!(
            "graphics_grade source=donington/effects.xml effect=Effect_0000 mapping=track-inspired-display controls={grade:?}"
        );
        graphics.saved_grade = Some(grade);
    }
    if drive_ticks.is_some_and(|n| n > 36_000) || (drive_ticks.is_some() && !drive) {
        return Err("scripted ticks require --drive and must be <=36000".into());
    }
    if trial_laps.is_some_and(|n| !(1..=99).contains(&n)) || (trial_laps.is_some() && !drive) {
        return Err("time trial requires --drive and 1..99 laps".into());
    }
    if (body_contacts.is_some() || body_events.is_some()) && !drive {
        return Err("--body-contacts and --body-events require --drive".into());
    }
    if deformation.is_some() && (!drive || body_contacts.is_none()) {
        return Err("--deformation requires --drive and --body-contacts".into());
    }
    if opponents.is_some_and(|n| !(1..=11).contains(&n))
        || (opponents.is_some() && (!drive || body_contacts.is_none()))
    {
        return Err("--opponents 1..11 requires --drive and --body-contacts".into());
    }
    if let Some(position) = grid_position
        && !opponents.is_some_and(|n| (1..=n + 1).contains(&position))
    {
        return Err("--grid-position needs --opponents and 1..opponents+1".into());
    }
    // The player's grid slot; the opponents fill the remaining slots in order.
    let player_slot = grid_position.map_or(0, |p| p - 1);
    let contacts_path = body_contacts.clone();
    if !scripted.throttle.is_finite()
        || !(0. ..=1.).contains(&scripted.throttle)
        || !scripted.steering.is_finite()
        || !(-1. ..=1.).contains(&scripted.steering)
    {
        return Err("invalid scripted controls".into());
    }
    let trial_mode = {
        use grid_tools::time_trial::Mode;
        if deformation.is_some() {
            Mode::Deformation
        } else if body_events.is_some() {
            Mode::Events
        } else if body_contacts.is_some() {
            Mode::Body
        } else {
            Mode::Wheels
        }
    };
    let driving = if drive {
        track = true;
        reference = true;
        collision = false;
        start_view = true;
        let mut runtime = grid_tools::source_vehicle::Runtime::lacetti(
            &root,
            &format!("slot_{player_slot:02}"),
            200679,
        )?;
        runtime.enable_lap_timing(&root)?;
        println!(
            "drive_lap_timing enabled=true gates={} required_gates={} finish=0 clock=successful-simulation-ticks policy=independent-ordered-route-splits",
            runtime.lap_gate_count(),
            runtime.lap_required_gate_count()
        );
        if let Some(path) = body_events {
            if std::fs::metadata(&path)?.len() != 28 {
                return Err("invalid private body event parameter extent".into());
            }
            runtime.body_events = Some(grid_tools::body_impact_parameters::read(&std::fs::read(
                path,
            )?)?);
            println!(
                "drive_body_events enabled=true scope=static-and-car-peers notifications=empty persistent-chains=absent"
            );
        }
        let geometry = body_contacts.map(|path| -> Result<_, Box<dyn std::error::Error>> {
            if std::fs::metadata(&path)?.len() != 712 {
                return Err("invalid private body sampling extent".into());
            }
            let table = std::fs::read(path)?;
            let mut geometry = runtime.prepare_body_geometry(&root, &table)?;
            if let Some(path)=&deformation {
                if std::fs::metadata(path)?.len()!=252{return Err("invalid private deformation parameter extent".into());}
                let (limit,bulk)=grid_tools::deformation_parameters::read(&std::fs::read(path)?)?;
                runtime.enable_deformation(&root,&mut geometry)?;
                runtime.register_deformation_callbacks(limit,bulk,1)?;
                println!("drive_deformation enabled=true scope=static-and-car-peers source=player-lod1 wheel-view=source-wheel-anchors crt_seed=00000001 body_graphics=independent-grid-projection");
            }
            println!("drive_body_geometry enabled=true pool=20 boundary=cqtc order=boundary-first-then-archive materials=shared damage=owned-rng notifications=guarded");
            Ok(geometry)
        }).transpose()?;
        let opponents = opponents
            .map(|n| -> Result<Opponents, Box<dyn std::error::Error>> {
                let table = std::fs::read(contacts_path.as_ref().ok_or("missing CD01")?)?;
                let slots: Vec<String> = std::iter::once(player_slot)
                    .chain((0..=n).filter(|&i| i != player_slot))
                    .map(|i| format!("slot_{i:02}"))
                    .collect();
                let refs: Vec<&str> = slots.iter().map(String::as_str).collect();
                let parameters = deformation
                    .as_ref()
                    .map(|path| -> Result<_, Box<dyn std::error::Error>> {
                        Ok(grid_tools::deformation_parameters::read(&std::fs::read(path)?)?)
                    })
                    .transpose()?;
                let multi = grid_tools::source_vehicle::multi::MultiRuntime::lacetti_deforming(
                    &root,
                    &refs,
                    &table,
                    runtime.body_events,
                    parameters,
                )?;
                use grid_tools::ai_driver::{AiData, AiDriver, NORMAL_DIFFICULTY, Parameters};
                let ai = AiData::load(&root, "tracks/europe/donington/route_0", "touring", "lac")?;
                let parameters = Parameters::new(&ai, NORMAL_DIFFICULTY, |_| 1.)?;
                let characteristics = match &ai_characteristics {
                    Some(path) => grid_tools::ai_driver::read_characteristics(
                        &std::fs::read_to_string(path)?,
                    )?,
                    None => Vec::new(),
                };
                // Opponent k uses driver row k (row 0 is the player's slot).
                let drivers = (0..n)
                    .map(|i| -> Result<AiDriver, String> {
                        let row = characteristics.get((i + 1) % characteristics.len().max(1));
                        let p = Parameters::new(&ai, NORMAL_DIFFICULTY, |name| {
                            row.and_then(|r| r.get(name).copied()).unwrap_or(1.)
                        })?;
                        Ok(AiDriver::new(p, 0x5eed + i as u32 + 1))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                if !characteristics.is_empty() {
                    println!(
                        "drive_ai_characteristics drivers={} source=private-table",
                        characteristics.len()
                    );
                }
                println!(
                    "drive_opponents enabled=true count={n} world=shared car_contacts=two-body driver=original-ai-port difficulty={NORMAL_DIFFICULTY} deformation={}",
                    if deformation.is_some() { "per-car-static-peers" } else { "disabled" }
                );
                let start = {
                    let gate = &grid_tools::lap_progress::load(&root)?.progress.gates[0];
                    std::array::from_fn(|k| (gate.left[k] + gate.right[k]) * 0.5)
                };
                let mut positions = vec![{
                    let p = runtime.pose();
                    [p[12], p[13], p[14]]
                }];
                positions.extend(multi.cars.iter().skip(1).map(|c| {
                    let p = c.pose();
                    [p[12], p[13], p[14]]
                }));
                let standings = grid_tools::race::Standings::new(
                    &ai.line,
                    trial_laps.unwrap_or(1),
                    start,
                    &positions,
                );
                let names = std::iter::once("You".to_string())
                    .chain((1..=n).map(|i| format!("AI {i}")))
                    .collect();
                let autopilot = (drive_ticks.is_some()
                    && std::env::var_os("GRID_AUTOPILOT").is_some())
                .then(|| AiDriver::new(parameters.clone(), 0x5eed));
                if autopilot.is_some() {
                    println!("drive_autopilot enabled=true scope=scripted-validation");
                }
                Ok(Opponents {
                    multi,
                    drivers,
                    ai,
                    standings,
                    names,
                    autopilot,
                    throttles: vec![0.; n],
                })
            })
            .transpose()?;
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
            opponents,
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
        // Each opponent wears its own shipped livery (01, 02, ...); the player keeps 00.
        if let Some(opponents) = driving.as_ref().and_then(|d| d.opponents.as_ref()) {
            for k in 1..opponents.multi.cars.len() {
                car.liveries.push(load_livery(&root, k % 10)?);
            }
            println!("drive_opponent_liveries count={}", car.liveries.len());
        }
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
    if let Some(laps) = trial_laps {
        // Races with opponents never write the solo time-trial best times.
        let race = driving.as_ref().is_some_and(|d| d.opponents.is_some());
        let mut trial = time_trial::Trial::new(laps, trial_mode, output.is_none() && !race)?;
        trial.race = race;
        if race {
            // Races use the shipped start-light sequence time.
            trial.state = grid_tools::time_trial::State::new(
                laps,
                f64::from(grid_tools::source_vehicle::DELTA),
                START_SEQUENCE_SECONDS,
            )?;
        }
        app.insert_resource(trial);
    }
    if let Some(path) = output {
        app.add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: None,
                    exit_condition: ExitCondition::DontExit,
                    ..default()
                })
                .disable::<WinitPlugin>()
                .disable::<bevy::audio::AudioPlugin>(),
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
                    resolution: (graphics.width, graphics.height).into(),
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
        // Offscreen captures never open an audio device or play sound.
        if app.is_plugin_added::<bevy::audio::AudioPlugin>() {
            match grid_tools::engine_audio_continuous::Bank::lacetti(&root) {
                Ok(bank) => engine_sound::install(&mut app, bank),
                Err(error) => warn!("Engine audio unavailable; driving continues: {error}"),
            }
            match grid_tools::tyre_audio::Bank::load(&root) {
                Ok(bank) => tyre_sound::install(&mut app, bank),
                Err(error) => warn!("Tyre audio unavailable; driving continues: {error}"),
            }
        }
        app.insert_resource(driving)
            .insert_resource(Time::<Fixed>::from_seconds(f64::from(
                grid_tools::source_vehicle::DELTA,
            )))
            .add_systems(FixedUpdate, drive_tick)
            .add_systems(Last, drive_exit_log)
            .init_resource::<DeformationPresentation>()
            .init_resource::<free_look::FreeLook>()
            .add_systems(
                Update,
                (drive_actions, free_look::update, drive_present).chain(),
            )
            .add_systems(Update, update_start_lights);
    }
    if graphics.source_ibl && (!graphics.enhanced() || !graphics.source_sky || !track) {
        return Err(
            "--ibl source requires enhanced graphics and a source-sky Donington scene".into(),
        );
    }
    if graphics.enhanced() && graphics.source_sky && track {
        sky::install(&mut app, &root, graphics.source_ibl)?;
    }
    if let Some(frames) = graphics.benchmark_frames {
        graphics_benchmark::install(&mut app, frames);
    }
    if graphics.track_grade {
        track_grade::install(&mut app);
    }
    let exit = app
        .insert_resource(graphics)
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
    graphics: Res<graphics::Settings>,
    mut scattering_media: ResMut<Assets<bevy::light::atmosphere::ScatteringMedium>>,
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
            graphics.sampler(car.track),
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        )
        .expect("validated DDS image");
        texture_handles.insert(name.clone(), images.add(image));
    }
    let mut material_handles = BTreeMap::new();
    let liveries = std::mem::take(&mut car.liveries);
    let mut livery_images: BTreeMap<(usize, String), Handle<Image>> = BTreeMap::new();
    let mut livery_materials: BTreeMap<(usize, String), Handle<StandardMaterial>> = BTreeMap::new();
    let bindings = car.bindings.clone();
    let track = car.track;
    let start_light_images: Vec<Option<Handle<Image>>> = START_LIGHT_TEXTURES
        .iter()
        .map(|name| {
            texture_handles
                .get(&format!("objectstextures.pssg#{name}"))
                .cloned()
        })
        .collect();
    for mut part in car.parts.drain(..) {
        if graphics.enhanced()
            && graphics.smooth_tyres
            && part.wheel.is_some()
            && matches!(
                part.material
                    .strip_prefix("lacetti/")
                    .unwrap_or(&part.material)
                    .trim(),
                "Tyre Wall" | "Tyre Tread"
            )
        {
            match tyre_visual::refine(&mut part.mesh) {
                Ok((before, after)) => println!(
                    "tyre_visual part={:?} triangles={before}->{after} physics=unchanged",
                    part.name
                ),
                Err(error) => eprintln!("tyre_visual skipped part={:?}: {error}", part.name),
            }
        }
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
                let mut material = StandardMaterial {
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
                };
                if graphics.enhanced()
                    && graphics.layered_car
                    && (!track || part.material.starts_with("lacetti/"))
                {
                    car_materials::apply(name, &mut material);
                }
                materials.add(material)
            })
            .clone();
        let deformable = driving
            .as_ref()
            .is_some_and(|d| d.runtime.deformation_enabled)
            && part.material.starts_with("lacetti/")
            && part.wheel.is_none()
            && part.material.strip_prefix("lacetti/") != Some("Aerials");
        let original = deformable.then(|| part.mesh.clone());
        let initial_bounds = part
            .mesh
            .compute_aabb()
            .expect("validated source mesh bounds");
        let shared = if deformable {
            None
        } else {
            Some(meshes.add(part.mesh))
        };
        for matrix in part.matrices {
            let mesh = shared
                .clone()
                .unwrap_or_else(|| meshes.add(original.as_ref().unwrap().clone()));
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
                initial_bounds,
            ));
            if moving {
                entity.insert(MovingPart {
                    rest: matrix,
                    wheel: part.wheel,
                });
                if let Some(source) = &original {
                    let owner = &driving.as_ref().unwrap().runtime.state.car.owner.vehicle;
                    let model = Mat4::from_translation(Vec3::new(
                        f32::from_bits(owner[0x1630 / 4]),
                        f32::from_bits(owner[0x1634 / 4]) - f32::from_bits(owner[0x1564 / 4]),
                        f32::from_bits(owner[0x1638 / 4]),
                    ));
                    entity.insert(DeformingPart {
                        source: source.clone(),
                        raw_bind: model.inverse() * matrix,
                    });
                }
            }
            if moving && let Some(opponents) = driving.as_ref().and_then(|d| d.opponents.as_ref()) {
                for car in 1..opponents.multi.cars.len() {
                    let runtime = &opponents.multi.cars[car];
                    let motion = part
                        .wheel
                        .map_or(Mat4::IDENTITY, |i| wheel_motion(runtime.wheels()[i]));
                    // Deforming body parts own their mesh per car; others share it.
                    let own = original.as_ref().filter(|_| runtime.deformation_enabled);
                    let opponent_mesh = own.map_or_else(|| mesh.clone(), |m| meshes.add(m.clone()));
                    // Swap in this car's livery texture where the package has it.
                    let opponent_material = bindings
                        .get(&part.material)
                        .and_then(|texture| {
                            let dds = liveries.get(car - 1)?.get(texture)?;
                            Some((texture, dds))
                        })
                        .map_or_else(
                            || material.clone(),
                            |(texture, dds)| {
                                let image = livery_images
                                    .entry((car, texture.clone()))
                                    .or_insert_with(|| {
                                        images.add(
                                            Image::from_buffer(
                                                dds,
                                                ImageType::Extension("dds"),
                                                CompressedImageFormats::BC,
                                                true,
                                                graphics.sampler(false),
                                                RenderAssetUsages::MAIN_WORLD
                                                    | RenderAssetUsages::RENDER_WORLD,
                                            )
                                            .expect("validated livery DDS image"),
                                        )
                                    })
                                    .clone();
                                livery_materials
                                    .entry((car, part.material.clone()))
                                    .or_insert_with(|| {
                                        let mut m = materials
                                            .get(&material)
                                            .expect("part material")
                                            .clone();
                                        m.base_color_texture = Some(image);
                                        materials.add(m)
                                    })
                                    .clone()
                            },
                        );
                    let mut opponent = commands.spawn((
                        Name::new(format!("opponent{car} {}", part.name)),
                        Mesh3d(opponent_mesh),
                        MeshMaterial3d(opponent_material),
                        Transform::from_matrix(
                            Mat4::from_cols_array(&runtime.pose()) * motion * matrix,
                        ),
                        initial_bounds,
                        OpponentPart {
                            car,
                            rest: matrix,
                            wheel: part.wheel,
                        },
                    ));
                    if let Some(source) = own {
                        let owner = &runtime.state.car.owner.vehicle;
                        let model = Mat4::from_translation(Vec3::new(
                            f32::from_bits(owner[0x1630 / 4]),
                            f32::from_bits(owner[0x1634 / 4]) - f32::from_bits(owner[0x1564 / 4]),
                            f32::from_bits(owner[0x1638 / 4]),
                        ));
                        opponent.insert(OpponentDeforming(DeformingPart {
                            source: source.clone(),
                            raw_bind: model.inverse() * matrix,
                        }));
                    }
                }
            }
        }
    }
    if let Some(material) = material_handles.get("objects/start_lights").cloned()
        && start_light_images.iter().all(Option::is_some)
    {
        let textures: Vec<Handle<Image>> = start_light_images.into_iter().flatten().collect();
        if let Some(mut m) = materials.get_mut(&material) {
            m.emissive = LinearRgba::rgb(3., 3., 3.);
            m.emissive_texture = Some(textures[0].clone());
        }
        commands.insert_resource(StartLights {
            material,
            textures,
            stage: 0,
        });
        println!("start_lights enabled=true stages=6 sequence_seconds={START_SEQUENCE_SECONDS}");
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
            illuminance: graphics.sun_illuminance(),
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(-4.0, 8.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        graphics.cascades(),
    ));
    commands.insert_resource(GlobalAmbientLight {
        color: Color::WHITE,
        brightness: 450.0,
        ..default()
    });
    let camera = commands
        .spawn((Camera3d::default(), camera_transform(&orbit), ViewerCamera))
        .id();
    graphics::configure(&mut commands, camera, &graphics, &mut scattering_media);
    if driving.is_some() {
        commands.spawn((
            DrivingHud,
            Text::new("Lacetti / Donington\nPreparing practice timer"),
            TextFont {
                font_size: FontSize::Px(22.),
                ..default()
            },
            TextColor(Color::srgb(0.93, 0.96, 1.)),
            BackgroundColor(Color::srgba(0.025, 0.04, 0.065, 0.9)),
            Node {
                position_type: PositionType::Absolute,
                top: px(18.),
                left: px(18.),
                width: px(360.),
                padding: UiRect::all(px(14.)),
                ..default()
            },
            UiTargetCamera(camera),
        ));
    }
    if let Some(mut capture) = capture {
        let mut image = Image::new_target_texture(
            graphics.width,
            graphics.height,
            TextureFormat::Rgba8UnormSrgb,
            None,
        );
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
    benchmark: Option<Res<graphics_benchmark::Benchmark>>,
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
        if benchmark.is_none()
            && driving
                .scripted_ticks
                .is_some_and(|n| driving.runtime.ticks < n)
        {
            return;
        }
    }
    if benchmark.as_ref().is_some_and(|b| !b.finished) {
        return;
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
fn drive_tick(
    mut driving: ResMut<Driving>,
    input: Res<ButtonInput<KeyCode>>,
    mut trial: Option<ResMut<time_trial::Trial>>,
) {
    if driving.paused
        || driving.failure.is_some()
        || driving
            .scripted_ticks
            .is_some_and(|n| driving.runtime.ticks >= n)
    {
        return;
    }
    // During the countdown the engine keeps running and the player may rev.
    // Authored grid hold, not a recovered original race-start rule: the
    // reconstructed controls engage the clutch with the handbrake (1640 ->
    // E34), so the car stays put and releasing it at GO launches with wheelspin.
    let held = trial.as_ref().is_some_and(|t| !t.driving());
    if let Some(trial) = trial.as_mut()
        && held
    {
        if trial.finished() {
            return;
        }
        trial.countdown();
    }
    let driving = &mut *driving;
    let autopilot = if !held
        && let Some(opponents) = &mut driving.opponents
        && let Some(pilot) = &mut opponents.autopilot
    {
        use grid_tools::ai_driver::{Other, View};
        let car = &driving.runtime;
        let others: Vec<Other> = opponents
            .multi
            .cars
            .iter()
            .enumerate()
            .skip(1)
            .map(|(i, c)| Other::from_pose(i as u32 + 1, c.pose(), c.velocity()))
            .collect();
        let view = View::of(car, opponents.multi.obstruction(0));
        Some(pilot.controls(
            &opponents.ai,
            &view,
            &others,
            grid_tools::source_vehicle::DELTA,
        ))
    } else {
        None
    };
    let mut controls = if let Some(controls) = autopilot {
        controls
    } else if driving.scripted_ticks.is_some() {
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
    if held {
        controls = grid_tools::source_vehicle::Controls {
            throttle: controls.throttle,
            handbrake: true,
            ..Default::default()
        };
    }
    let driving = &mut *driving;
    let result = if let Some(opponents) = &mut driving.opponents {
        // One common shared-world tick: the player runtime occupies car slot 0.
        use grid_tools::ai_driver::{Other, View};
        let mut all = vec![controls];
        // Car 0 of the shared world is the player runtime, held outside it here.
        let cars: Vec<Other> = std::iter::once(&driving.runtime)
            .chain(opponents.multi.cars.iter().skip(1))
            .enumerate()
            .map(|(i, c)| Other::from_pose(i as u32 + 1, c.pose(), c.velocity()))
            .collect();
        // Start state while held [I]: gentle blips first, hard revving in the
        // second half of the countdown.
        let rev_state = match trial.as_ref().map(|t| t.state.phase()) {
            Some(grid_tools::time_trial::Phase::Countdown(left))
                if f64::from(*left) * f64::from(grid_tools::source_vehicle::DELTA)
                    > START_SEQUENCE_SECONDS / 2. =>
            {
                2
            }
            _ => 3,
        };
        for (i, driver) in opponents.drivers.iter_mut().enumerate() {
            let car = &opponents.multi.cars[i + 1];
            all.push(if held {
                grid_tools::source_vehicle::Controls {
                    throttle: driver.grid_rev(rev_state, grid_tools::source_vehicle::DELTA),
                    handbrake: true,
                    ..Default::default()
                }
            } else {
                let others: Vec<Other> = cars
                    .iter()
                    .enumerate()
                    .filter(|&(j, _)| j != i + 1)
                    .map(|(_, o)| *o)
                    .collect();
                let view = View::of(car, opponents.multi.obstruction(i + 1));
                driver.controls(
                    &opponents.ai,
                    &view,
                    &others,
                    grid_tools::source_vehicle::DELTA,
                )
            });
        }
        opponents.throttles = all[1..].iter().map(|c| c.throttle).collect();
        std::mem::swap(&mut driving.runtime, &mut opponents.multi.cars[0]);
        let result = opponents.multi.tick(&all);
        std::mem::swap(&mut driving.runtime, &mut opponents.multi.cars[0]);
        if result.is_ok() {
            driving.runtime.last_body_contacts =
                opponents.multi.last_body_contacts + opponents.multi.last_car_contacts;
        }
        result
    } else if let Some(geometry) = &mut driving.geometry {
        if driving.runtime.deformation_enabled {
            driving
                .runtime
                .tick_with_deforming_body_geometry(controls, geometry)
        } else {
            driving.runtime.tick_with_body_geometry(controls, geometry)
        }
    } else {
        driving.runtime.tick(controls)
    };
    if let Err(error) = result {
        let text = format!("tick {}: {error}; R resets", driving.runtime.ticks + 1);
        error!("driving paused: {text}");
        driving.failure = Some(text);
        driving.report("failure", controls);
    } else {
        if !held
            && let Some(trial) = trial.as_mut()
            && let Err(error) = trial.tick(driving.runtime.ticks, &driving.runtime.lap_events)
        {
            driving.failure = Some(format!("{error}; R restarts"));
            driving.report("failure", controls);
            return;
        }
        if !held
            && let (Some(trial), Some(opponents)) = (trial.as_ref(), driving.opponents.as_mut())
        {
            let positions = opponents.positions(&driving.runtime);
            let already = opponents.standings.finished(0).is_some();
            opponents
                .standings
                .update(&opponents.ai.line, &positions, trial.state.elapsed(), &[0]);
            if let grid_tools::time_trial::Phase::Finished(result) = trial.state.phase()
                && !already
            {
                opponents.standings.finish(0, result.seconds);
                let speeds: Vec<f64> = std::iter::once(&driving.runtime)
                    .chain(opponents.multi.cars.iter().skip(1))
                    .map(|c| {
                        let v = c.velocity();
                        f64::from((v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt())
                    })
                    .collect();
                opponents
                    .standings
                    .estimate_remaining(&speeds, result.seconds);
                for line in opponents.standings.classification(&opponents.names) {
                    println!("drive_race_result {line}");
                }
            }
        }
        for event in &driving.runtime.lap_events {
            use grid_tools::lap_progress::Event;
            let lap = driving.runtime.lap.as_ref().unwrap().lap_number();
            let segment = driving.segment;
            let tick = driving.runtime.ticks;
            match event {
                Event::Started { when } => {
                    println!(
                        "drive_lap_event version=1 segment={segment} tick={tick} type=start lap={lap} when={when:.9}"
                    )
                }
                Event::Checkpoint { gate, when } => println!(
                    "drive_lap_event version=1 segment={segment} tick={tick} type=checkpoint lap={lap} gate={gate} when={when:.9}"
                ),
                Event::Sector { sector, seconds } => println!(
                    "drive_lap_event version=1 segment={segment} tick={tick} type=sector lap={lap} sector={} seconds={seconds:.9}",
                    sector + 1
                ),
                Event::Finished(result) => println!(
                    "drive_lap_event version=1 segment={segment} tick={tick} type=finish lap={} seconds={:.9} valid={} missed_gate={} sectors={}",
                    result.number,
                    result.seconds,
                    result.valid,
                    result
                        .missed_gate
                        .map_or_else(|| "none".into(), |g| g.to_string()),
                    result
                        .sectors
                        .iter()
                        .map(|s| s.map_or_else(|| "none".into(), |s| format!("{s:.9}")))
                        .collect::<Vec<_>>()
                        .join(",")
                ),
                Event::JumpInvalidated => {
                    println!(
                        "drive_lap_event version=1 segment={segment} tick={tick} type=position-jump lap={lap}"
                    )
                }
            }
        }
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
    mut trial: Option<ResMut<time_trial::Trial>>,
) {
    if input.just_pressed(KeyCode::KeyR) {
        driving.report("reset", driving.last_controls.unwrap_or_default());
        driving.runtime.reset();
        let driving = &mut *driving;
        if let Some(opponents) = &mut driving.opponents {
            opponents.multi.reset();
            for driver in &mut opponents.drivers {
                driver.reset();
            }
            let positions = opponents.positions(&driving.runtime);
            opponents.standings.reset(&opponents.ai.line, &positions);
        }
        driving.segment += 1;
        driving.last_controls = None;
        driving.failure = None;
        driving.paused = false;
        if let Some(trial) = trial.as_mut() {
            trial.reset();
        }
    }
    if input.just_pressed(KeyCode::KeyP) && !trial.as_ref().is_some_and(|t| t.finished()) {
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
#[allow(clippy::type_complexity, clippy::too_many_arguments)] // Explicit Bevy resource/component ownership.
fn drive_present(
    mut driving: ResMut<Driving>,
    mut parts: Query<
        (
            &MovingPart,
            &mut Transform,
            Option<&DeformingPart>,
            &Mesh3d,
            &mut Aabb,
        ),
        Without<ViewerCamera>,
    >,
    mut opponent_parts: Query<
        (
            &OpponentPart,
            &mut Transform,
            Option<&OpponentDeforming>,
            &Mesh3d,
            &mut Aabb,
        ),
        (Without<MovingPart>, Without<ViewerCamera>),
    >,
    mut camera: Single<&mut Transform, With<ViewerCamera>>,
    mut windows: Query<&mut Window>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut presentation: ResMut<DeformationPresentation>,
    mut hud: Query<&mut Text, With<DrivingHud>>,
    sound: Option<Res<engine_sound::EngineSound>>,
    trial: Option<Res<time_trial::Trial>>,
    input: Option<Res<ButtonInput<KeyCode>>>,
    mut look_back: Local<Option<bool>>,
    mut opponent_keys: Local<Vec<Option<Vec<u32>>>>,
    free_look: Option<Res<free_look::FreeLook>>,
) {
    if driving.runtime.deformation_enabled && driving.failure.is_none() {
        let update = (|| -> Result<(), &'static str> {
            let field =
                grid_tools::body_deformation::Field::read(&driving.runtime.state.car.owner.vehicle)
                    .map_err(|e| e.0)?;
            let key = field.key();
            if presentation.0.as_ref() == Some(&key) {
                return Ok(());
            }
            let mut staged: Vec<(Handle<Mesh>, Mesh, Aabb)> = Vec::new();
            let mut changed = 0;
            for (_, _, source, handle, _) in &parts {
                if let Some(source) = source {
                    if meshes.get(&handle.0).is_none()
                        || staged.iter().any(|(h, _, _)| h == &handle.0)
                    {
                        return Err("body damage mesh ownership differs");
                    }
                    let mesh = body_damage::project(&source.source, source.raw_bind, &field)?;
                    changed += usize::from(
                        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                            != source.source.attribute(Mesh::ATTRIBUTE_POSITION),
                    );
                    let bounds = mesh.compute_aabb().ok_or("invalid projected body bounds")?;
                    staged.push((handle.0.clone(), mesh, bounds));
                }
            }
            for (handle, mesh, _) in &staged {
                *meshes.get_mut(handle).expect("staged owned mesh") = mesh.clone();
            }
            for (_, _, _, handle, mut bounds) in &mut parts {
                if let Some((_, _, next)) = staged.iter().find(|(h, _, _)| h == &handle.0) {
                    *bounds = *next;
                }
            }
            presentation.0 = Some(key);
            println!(
                "drive_body_projection event=update tick={} meshes={} changed_meshes={changed} policy=independent-current-minus-rest",
                driving.runtime.ticks,
                staged.len()
            );
            Ok(())
        })();
        if let Err(error) = update {
            driving.failure = Some(format!("body presentation: {error}; R resets"));
            driving.report("failure", driving.last_controls.unwrap_or_default());
        }
    }
    let pose = Mat4::from_cols_array(&driving.runtime.pose());
    let wheels = driving.runtime.wheels().map(wheel_motion);
    for (part, mut transform, _, _, _) in &mut parts {
        let motion = part.wheel.map_or(Mat4::IDENTITY, |i| wheels[i]);
        *transform = Transform::from_matrix(pose * motion * part.rest);
    }
    if let Some(opponents) = &driving.opponents {
        let cars: Vec<(Mat4, [Mat4; 4])> = opponents
            .multi
            .cars
            .iter()
            .map(|car| {
                (
                    Mat4::from_cols_array(&car.pose()),
                    car.wheels().map(wheel_motion),
                )
            })
            .collect();
        // Opponent body dents: re-project a car's parts when its field changes.
        opponent_keys.resize(opponents.multi.cars.len(), None);
        let mut dirty = Vec::new();
        for (car, runtime) in opponents.multi.cars.iter().enumerate().skip(1) {
            if !runtime.deformation_enabled {
                continue;
            }
            if let Ok(field) =
                grid_tools::body_deformation::Field::read(&runtime.state.car.owner.vehicle)
                && opponent_keys[car].as_ref() != Some(&field.key())
            {
                opponent_keys[car] = Some(field.key());
                dirty.push((car, field));
            }
        }
        for (part, mut transform, deforming, handle, mut bounds) in &mut opponent_parts {
            let (pose, wheels) = &cars[part.car];
            let motion = part.wheel.map_or(Mat4::IDENTITY, |i| wheels[i]);
            *transform = Transform::from_matrix(*pose * motion * part.rest);
            if let (Some(OpponentDeforming(source)), Some((_, field))) =
                (deforming, dirty.iter().find(|(car, _)| *car == part.car))
                && let Ok(mesh) = body_damage::project(&source.source, source.raw_bind, field)
                && let Some(next) = mesh.compute_aabb()
                && let Some(mut target) = meshes.get_mut(&handle.0)
            {
                *target = mesh;
                *bounds = next;
            }
        }
    }
    // C holds a look-back chase view; GRID_LOOK_BACK forces it for scripted renders.
    let back = input.is_some_and(|i| i.pressed(KeyCode::KeyC))
        || *look_back.get_or_insert_with(|| std::env::var_os("GRID_LOOK_BACK").is_some());
    let side = if back { -1. } else { 1. };
    // Right mouse button orbits the chase camera (AUTHORED free look).
    let target = Vec3::new(0., 1., 0.);
    let offset = free_look::offset(free_look.as_deref(), Vec3::new(0., 2., -9. * side), side);
    let at = pose.transform_point3(target);
    let eye = pose.transform_point3(target + offset);
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
    let mut geometry = if driving.geometry.is_some() {
        format!(" | BODY {}", driving.runtime.last_body_contacts)
    } else {
        String::new()
    };
    if driving.runtime.body_events.is_some() {
        geometry.push_str(" | EVENTS");
    }
    if driving.runtime.deformation_enabled {
        geometry.push_str(" | DAMAGE");
    }
    let terminal = terminal_damage_message(&driving.runtime);

    let lap = driving
        .runtime
        .lap
        .as_ref()
        .map_or_else(String::new, |lap| {
            let time = lap.elapsed().map_or_else(
                || "Cross start line".into(),
                |s| {
                    format!(
                        "LAP {} {} | SECTOR {}/{}{}",
                        lap.lap_number(),
                        lap_time(s),
                        lap.passed_gates(),
                        driving.runtime.lap_required_gate_count(),
                        if lap.current_valid { "" } else { " INVALID" }
                    )
                },
            );
            format!(
                " | {time}{}",
                lap.best_lap
                    .map_or_else(String::new, |s| format!(" | BEST {}", lap_time(s)))
            )
        });
    let progress = driving
        .runtime
        .lap
        .as_ref()
        .map_or_else(String::new, |lap| {
            let current = lap.elapsed().map_or_else(
                || "Cross the start line".into(),
                |s| {
                    format!(
                        "Lap {}   {}\nSector {}/{}{}",
                        lap.lap_number(),
                        lap_time(s),
                        lap.passed_gates(),
                        driving.runtime.lap_required_gate_count(),
                        if lap.current_valid {
                            ""
                        } else {
                            " - invalid lap"
                        }
                    )
                },
            );
            let last = lap.last_lap.as_ref().map_or_else(
                || "Last  --".into(),
                |s| {
                    format!(
                        "Last  {}{}",
                        lap_time(s.seconds),
                        if s.valid { "" } else { " (invalid)" }
                    )
                },
            );
            let best = lap
                .best_lap
                .map_or_else(|| "Best  --".into(), |s| format!("Best  {}", lap_time(s)));
            format!("{current}\n{last}\n{best}")
        });
    let status = if driving.failure.is_some() {
        "Simulation paused after an error. R resets."
    } else if driving.paused {
        "Paused - P resumes; R resets"
    } else {
        terminal
            .as_deref()
            .unwrap_or("Space: handbrake | C: look back | RMB: look around | P: pause | R: reset")
    };
    let audio = sound.map_or("", |s| {
        if s.muted {
            "\nSound muted - M enables"
        } else {
            "\nM: mute engine sound"
        }
    });
    let heading = if trial.as_ref().is_some_and(|t| t.race) {
        "Lacetti / Donington race"
    } else if trial.is_some() {
        "Lacetti / Donington time trial"
    } else {
        "Lacetti / Donington practice"
    };
    // Race position or classification above the event HUD.
    let race = match (&driving.opponents, &trial) {
        (Some(o), Some(t)) if t.finished() => format!(
            "RACE RESULT\n{}\n\n",
            o.standings.classification(&o.names).join("\n")
        ),
        (Some(o), Some(t)) if t.driving() => {
            // Gaps as time at the player's speed (floor 10 m/s).
            let v = driving.runtime.velocity();
            let speed = f64::from((v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()).max(10.);
            let (ahead, behind) = o.standings.neighbour_gaps(0);
            let gap = |label: &str, sign: &str, d: Option<f64>| {
                d.map_or_else(String::new, |d| {
                    format!("\n{label} {sign}{:.1} s", d / speed)
                })
            };
            format!(
                "Position {}/{}{}{}\n",
                o.standings.position(0),
                o.standings.len(),
                gap("Ahead ", "-", ahead),
                gap("Behind", "+", behind)
            )
        }
        (Some(o), Some(_)) => format!("Grid {}/{}\n", o.standings.position(0), o.standings.len()),
        _ => String::new(),
    };
    let progress = trial.as_ref().map_or_else(
        || progress.clone(),
        |t| {
            if t.finished() || !t.driving() {
                format!("{race}{}", t.hud())
            } else {
                format!("{race}{}\n\n{progress}", t.hud())
            }
        },
    );
    let text = format!(
        "{heading}\n{speed:.0} km/h   Gear {gear}{handbrake}\n\n{progress}\n\n{status}{audio}"
    );
    for mut value in &mut hud {
        if value.0 != text {
            value.0.clone_from(&text);
        }
    }
    let lap = trial.as_ref().map_or(lap.clone(), |t| t.title());
    for mut window in &mut windows {
        window.title = format!(
            "GRID prototype {BUILD_REVISION} | {speed:.1} km/h | gear {gear}{handbrake}{geometry}{lap} | tick {} | {}",
            driving.runtime.ticks,
            driving.failure.as_deref().unwrap_or(if driving.paused {
                "Paused; P resumes; R resets"
            } else if let Some(message) = terminal.as_deref() {
                message
            } else {
                "Arrows drive; Space handbrake; P pause; R reset"
            })
        );
    }
}
fn terminal_damage_message(runtime: &grid_tools::source_vehicle::Runtime) -> Option<String> {
    runtime.terminal_wheel().map(|i| {
        let wheel = ["rear-left", "rear-right", "front-left", "front-right"]
            .get(i)
            .copied()
            .unwrap_or("unknown");
        format!("Terminal {wheel} wheel damage; controls disabled; R resets")
    })
}
/// Stage of the start-light board: lights 1..5 come on across the race
/// countdown, stage 6 (texture 7) shows for 3 s from GO, otherwise idle.
/// Mapping of the shipped "6 stages over 4.0 s" to the countdown is [I].
fn update_start_lights(
    trial: Option<Res<time_trial::Trial>>,
    lights: Option<ResMut<StartLights>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    use grid_tools::time_trial::Phase;
    let Some(mut lights) = lights else {
        return;
    };
    let stage = match trial.as_ref().map(|t| (t.state.phase(), t.state.elapsed())) {
        Some((Phase::Countdown(left), _)) => {
            let remaining = f64::from(*left) * f64::from(grid_tools::source_vehicle::DELTA);
            let elapsed = (START_SEQUENCE_SECONDS - remaining).max(0.);
            ((elapsed / (START_SEQUENCE_SECONDS / 5.)) as usize + 1).min(5)
        }
        Some((Phase::Running, elapsed)) if elapsed < 3. => 6,
        _ => 0,
    };
    if stage != lights.stage {
        lights.stage = stage;
        if let Some(mut m) = materials.get_mut(&lights.material) {
            m.emissive_texture = Some(lights.textures[stage].clone());
        }
    }
}
fn lap_time(seconds: f64) -> String {
    let milliseconds = (seconds.max(0.) * 1000.).round() as u64;
    format!(
        "{}:{:02}.{:03}",
        milliseconds / 60000,
        milliseconds / 1000 % 60,
        milliseconds % 1000
    )
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
        exercise_keyboard_recovery(None, false);
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS"]
    fn body_geometry_keyboard_pause_reset_and_failure_recovery() {
        let table = std::fs::read(std::env::var_os("GRID_CONTACT_DIRECTIONS").unwrap()).unwrap();
        exercise_keyboard_recovery(Some(&table), false);
    }
    #[test]
    #[ignore = "requires purchased sources and private body/deformation inputs"]
    fn deformation_keyboard_pause_reset_and_failure_recovery() {
        let table = std::fs::read(std::env::var_os("GRID_CONTACT_DIRECTIONS").unwrap()).unwrap();
        exercise_keyboard_recovery(Some(&table), true);
    }
    #[test]
    #[ignore = "requires purchased sources and private body/deformation inputs"]
    fn deformation_wheel_presentation_keeps_pivots_and_reset() {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let root = Path::new(&root);
        let read_env = |name| std::fs::read(std::env::var_os(name).unwrap()).unwrap();
        let mut runtime =
            grid_tools::source_vehicle::Runtime::lacetti(root, "slot_00", 200679).unwrap();
        let mut geometry = runtime
            .prepare_body_geometry(root, &read_env("GRID_CONTACT_DIRECTIONS"))
            .unwrap();
        runtime.enable_deformation(root, &mut geometry).unwrap();
        let (limit, bulk) =
            grid_tools::deformation_parameters::read(&read_env("GRID_DEFORMATION_PARAMETERS"))
                .unwrap();
        runtime
            .register_deformation_callbacks(limit, bulk, 1)
            .unwrap();
        runtime.body_events = Some(
            grid_tools::body_impact_parameters::read(&read_env("GRID_BODY_EVENT_PARAMETERS"))
                .unwrap(),
        );
        let initial = runtime.wheels().map(wheel_motion);
        let car = load(root, "LOD0_", false, false).unwrap();
        let pristine_field =
            grid_tools::body_deformation::Field::read(&runtime.state.car.owner.vehicle).unwrap();
        let selected: Vec<_> = car
            .parts
            .iter()
            .filter(|p| p.wheel.is_none() && p.material != "Aerials")
            .collect();
        assert!(!selected.is_empty());
        for part in &selected {
            assert_eq!(
                body_damage::project(&part.mesh, part.matrices[0], &pristine_field).unwrap(),
                part.mesh
            );
        }
        let mut damaged = false;
        for _ in 0..600 {
            runtime
                .tick_with_deforming_body_geometry(
                    grid_tools::source_vehicle::Controls {
                        throttle: 1.,
                        steering: 0.25,
                        ..Default::default()
                    },
                    &mut geometry,
                )
                .unwrap();
            let before = runtime.state.car.owner.vehicle.clone();
            for wheel in runtime.wheels() {
                let motion = wheel_motion(wheel);
                assert!(motion.is_finite());
                let anchor = Vec3::from_array(wheel.anchor);
                let center = anchor
                    + Vec3::from_array(wheel.deformation_translation)
                    + Vec3::from_array(wheel.axis) * wheel.displacement;
                assert!((motion.transform_point3(anchor) - center).length() < 1e-5);
                assert!((motion.determinant() - 1.).abs() < 1e-4);
                damaged |= wheel
                    .deformation_translation
                    .into_iter()
                    .any(|v| v.abs() > 1e-5);
            }
            assert_eq!(runtime.state.car.owner.vehicle, before);
        }
        assert!(damaged);
        let before = runtime.state.car.owner.vehicle.clone();
        let field = grid_tools::body_deformation::Field::read(&before).unwrap();
        let mut changed = 0;
        for part in &selected {
            let mesh = body_damage::project(&part.mesh, part.matrices[0], &field).unwrap();
            changed += usize::from(
                mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                    != part.mesh.attribute(Mesh::ATTRIBUTE_POSITION),
            );
            assert_eq!(mesh.indices(), part.mesh.indices());
            assert_eq!(
                mesh.attribute(Mesh::ATTRIBUTE_UV_0),
                part.mesh.attribute(Mesh::ATTRIBUTE_UV_0)
            );
            assert!(mesh.compute_aabb().is_some());
        }
        assert!(changed > 0);
        assert_eq!(runtime.state.car.owner.vehicle, before);
        // Exercise actual asset publication: a bad second bind fails after a
        // valid first projection, retaining both meshes/bounds and owner state.
        let mut assets = Assets::<Mesh>::default();
        let mut entries = Vec::new();
        for part in selected.iter().take(2) {
            entries.push((
                assets.add(part.mesh.clone()),
                part.mesh.clone(),
                part.matrices[0],
            ));
        }
        assert_eq!(entries.len(), 2);
        let mut app = App::new();
        app.insert_resource(assets)
            .init_resource::<DeformationPresentation>()
            .insert_resource(Driving {
                runtime,
                geometry: Some(geometry),
                paused: false,
                failure: None,
                scripted_ticks: None,
                scripted: Default::default(),
                last_controls: None,
                segment: 0,
                started: Instant::now(),
                opponents: None,
            })
            .add_systems(Update, drive_present);
        app.world_mut().spawn((ViewerCamera, Transform::default()));
        let mut ids = Vec::new();
        for (i, (handle, source, bind)) in entries.iter().enumerate() {
            ids.push(
                app.world_mut()
                    .spawn((
                        MovingPart {
                            rest: *bind,
                            wheel: None,
                        },
                        Transform::default(),
                        Mesh3d(handle.clone()),
                        source.compute_aabb().unwrap(),
                        DeformingPart {
                            source: source.clone(),
                            raw_bind: if i == 1 {
                                Mat4::from_cols_array(&[f32::NAN; 16])
                            } else {
                                *bind
                            },
                        },
                    ))
                    .id(),
            );
        }
        app.update();
        assert!(app.world().resource::<Driving>().failure.is_some());
        for (handle, source, _) in &entries {
            assert_eq!(
                app.world().resource::<Assets<Mesh>>().get(handle).unwrap(),
                source
            );
        }
        assert_eq!(
            app.world()
                .resource::<Driving>()
                .runtime
                .state
                .car
                .owner
                .vehicle,
            before
        );
        app.world_mut()
            .entity_mut(ids[1])
            .get_mut::<DeformingPart>()
            .unwrap()
            .raw_bind = entries[1].2;
        app.world_mut().resource_mut::<Driving>().failure = None;
        app.update();
        assert!(app.world().resource::<Driving>().failure.is_none());
        assert!(entries.iter().any(|(handle, source, _)| {
            app.world().resource::<Assets<Mesh>>().get(handle).unwrap() != source
        }));
        app.world_mut().resource_mut::<Driving>().runtime.reset();
        app.update();
        for (handle, source, _) in &entries {
            assert_eq!(
                app.world().resource::<Assets<Mesh>>().get(handle).unwrap(),
                source
            );
        }
        assert_eq!(
            app.world()
                .resource::<Driving>()
                .runtime
                .wheels()
                .map(wheel_motion),
            initial
        );
        eprintln!(
            "source_body_projection source_parts={} changed_parts={changed} pristine_mesh_exact=1 indices_uv_preserved=1 late_second_mesh_failure_atomic=1 reset_mesh_exact=1 physics_feedback=0",
            selected.len()
        );
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT purchased source; completion events are authored"]
    fn trial_countdown_pause_failure_finish_freeze_and_restart() {
        use grid_tools::time_trial::{Mode, Phase};
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let mut runtime =
            grid_tools::source_vehicle::Runtime::lacetti(Path::new(&root), "slot_00", 200679)
                .unwrap();
        runtime.enable_lap_timing(Path::new(&root)).unwrap();
        let initial = runtime.pose();
        let mut app = App::new();
        app.add_message::<AppExit>()
            .insert_resource(ButtonInput::<KeyCode>::default())
            .insert_resource(time_trial::Trial::new(1, Mode::Wheels, false).unwrap())
            .insert_resource(Driving {
                runtime,
                geometry: None,
                paused: false,
                failure: None,
                scripted_ticks: None,
                scripted: Default::default(),
                last_controls: None,
                segment: 0,
                started: Instant::now(),
                opponents: None,
            })
            .add_systems(Update, (drive_actions, drive_tick).chain());
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowUp);
        app.update();
        // The countdown ticks the held car: the engine runs and revs, the car
        // stays on its grid slot and the event clock has not started.
        assert_eq!(app.world().resource::<Driving>().runtime.ticks, 1);
        let held_distance = |app: &App| {
            let pose = app.world().resource::<Driving>().runtime.pose();
            (12..15)
                .map(|i| (pose[i] - initial[i]).powi(2))
                .sum::<f32>()
                .sqrt()
        };
        assert!(held_distance(&app) < 0.01);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyP);
        app.update();
        let frozen = app.world().resource::<time_trial::Trial>().state.clone();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        for _ in 0..5 {
            app.update();
        }
        assert_eq!(app.world().resource::<time_trial::Trial>().state, frozen);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyP);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        let mut count = 2;
        while !app.world().resource::<time_trial::Trial>().driving() {
            app.update();
            count += 1;
            assert!(count <= 188);
        }
        assert_eq!(count, 188);
        assert_eq!(app.world().resource::<Driving>().runtime.ticks, 188);
        assert!(held_distance(&app) < 0.01);
        assert_eq!(
            app.world().resource::<time_trial::Trial>().state.elapsed(),
            0.
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowUp);
        app.update();
        assert_eq!(app.world().resource::<Driving>().runtime.ticks, 189);
        assert_eq!(
            app.world().resource::<time_trial::Trial>().state.elapsed(),
            f64::from(grid_tools::source_vehicle::DELTA)
        );
        // Successful physics and event clocks advance together. A physics failure
        // preserves the event clock; R resets both and restores the countdown.
        app.world_mut()
            .resource_mut::<Driving>()
            .runtime
            .state
            .car
            .owner
            .vehicle[0x15a0 / 4] = 0;
        let before_failure = app.world().resource::<time_trial::Trial>().state.clone();
        app.update();
        assert!(app.world().resource::<Driving>().failure.is_some());
        assert_eq!(
            app.world().resource::<time_trial::Trial>().state,
            before_failure
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyR);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        assert!(app.world().resource::<Driving>().failure.is_none());
        // The reset frame already performs the first held countdown tick.
        assert_eq!(app.world().resource::<Driving>().runtime.ticks, 1);
        assert!(held_distance(&app) < 0.01);
        while !app.world().resource::<time_trial::Trial>().driving() {
            app.update();
        }
        // Inject valid authored crossing events only to exercise the Bevy finish
        // gate. Real three-lap physics is checked separately by the diagnostic.
        {
            use grid_tools::lap_progress::Event;
            let mut trial = app.world_mut().resource_mut::<time_trial::Trial>();
            trial.tick(1, &[Event::Started { when: 0.008 }]).unwrap();
            trial
                .tick(
                    2,
                    &[Event::Finished(grid_tools::time_trial::Completed {
                        number: 1,
                        seconds: 0.016,
                        sectors: vec![Some(0.016)],
                        valid: true,
                        missed_gate: None,
                    })],
                )
                .unwrap();
            assert!(matches!(trial.state.phase(), Phase::Finished(_)));
            assert!(trial.hud().contains("FINISHED"));
        }
        let car = app
            .world()
            .resource::<Driving>()
            .runtime
            .state
            .car
            .owner
            .vehicle
            .clone();
        let finished_ticks = app.world().resource::<Driving>().runtime.ticks;
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowUp);
        for _ in 0..4 {
            app.update();
        }
        assert_eq!(
            app.world().resource::<Driving>().runtime.ticks,
            finished_ticks
        );
        assert_eq!(
            app.world()
                .resource::<Driving>()
                .runtime
                .state
                .car
                .owner
                .vehicle,
            car
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyR);
        app.update();
        assert!(!app.world().resource::<time_trial::Trial>().finished());
        assert_eq!(app.world().resource::<Driving>().runtime.ticks, 1);
        assert!(held_distance(&app) < 0.01);
    }

    fn exercise_keyboard_recovery(table: Option<&[u8]>, deformation: bool) {
        let root = std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required");
        let mut runtime =
            grid_tools::source_vehicle::Runtime::lacetti(Path::new(&root), "slot_00", 200679)
                .unwrap();
        runtime.enable_lap_timing(Path::new(&root)).unwrap();
        let mut geometry = table.map(|table| {
            runtime
                .prepare_body_geometry(Path::new(&root), table)
                .unwrap()
        });
        if table.is_some() {
            let parameters =
                std::fs::read(std::env::var_os("GRID_BODY_EVENT_PARAMETERS").unwrap()).unwrap();
            runtime.body_events =
                Some(grid_tools::body_impact_parameters::read(&parameters).unwrap());
        }
        if deformation {
            let (limit, bulk) = grid_tools::deformation_parameters::read(
                &std::fs::read(std::env::var_os("GRID_DEFORMATION_PARAMETERS").unwrap()).unwrap(),
            )
            .unwrap();
            runtime
                .enable_deformation(Path::new(&root), geometry.as_mut().unwrap())
                .unwrap();
            runtime
                .register_deformation_callbacks(limit, bulk, 1)
                .unwrap();
        }
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
                opponents: None,
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
        let before_pause = app.world().resource::<Driving>().runtime.lap.clone();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyP);
        app.update();
        assert!(app.world().resource::<Driving>().paused);
        assert_eq!(app.world().resource::<Driving>().runtime.ticks, 122);
        // This minimal test app has no input plugin to clear just-pressed P
        // between updates; model the release rather than toggling each frame.
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        for _ in 0..4 {
            app.update();
        }
        assert_eq!(app.world().resource::<Driving>().runtime.lap, before_pause);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyR);
        app.update();
        assert_eq!(app.world().resource::<Driving>().runtime.ticks, 1);
        assert!(
            app.world()
                .resource::<Driving>()
                .runtime
                .lap
                .as_ref()
                .unwrap()
                .elapsed()
                .is_none()
        );
        assert!(
            app.world()
                .resource::<Driving>()
                .runtime
                .lap
                .as_ref()
                .unwrap()
                .best_lap
                .is_none()
        );
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
        let before_failure = app.world().resource::<Driving>().runtime.lap.clone();
        app.update();
        assert!(app.world().resource::<Driving>().failure.is_some());
        assert_eq!(app.world().resource::<Driving>().runtime.ticks, 1);
        app.update();
        assert_eq!(
            app.world().resource::<Driving>().runtime.lap,
            before_failure
        );
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
        // The idling engine launches immediately, so 120 throttle ticks reach a
        // higher speed than the former zero-RPM crank; allow a bounded stop.
        let speed_squared = |app: &App| {
            app.world().resource::<Driving>().runtime.velocity()[..3]
                .iter()
                .map(|v| v * v)
                .sum::<f32>()
        };
        let mut stop_ticks = 1;
        while speed_squared(&app) >= 0.01 {
            app.update();
            stop_ticks += 1;
            assert!(stop_ticks <= 300, "handbrake did not stop the car");
        }
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
        // A terminal car keeps advancing instead of entering the failure pause.
        // Original terminal production is separately compared against 9F3E80;
        // here exercise the actual app controls, source sync and reset consumer.
        {
            let mut driving = app.world_mut().resource_mut::<Driving>();
            driving.runtime.state.car.owner.vehicle[0xd234 / 4] = 3;
            driving.runtime.state.car.owner.vehicle[0xd238 / 4] = 2;
        }
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowUp);
        let before = app.world().resource::<Driving>().runtime.ticks;
        for _ in 0..3 {
            app.update();
        }
        let driving = app.world().resource::<Driving>();
        assert!(driving.failure.is_none());
        assert_eq!(driving.runtime.ticks, before + 3);
        assert_eq!(
            driving.runtime.state.car.owner.vehicle[0x226c / 4],
            1f32.to_bits()
        );
        assert_eq!(
            driving.runtime.state.car.owner.vehicle[0xe1c / 4],
            0f32.to_bits()
        );
        assert_eq!(
            terminal_damage_message(&driving.runtime).as_deref(),
            Some("Terminal front-left wheel damage; controls disabled; R resets")
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyR);
        app.update();
        let driving = app.world().resource::<Driving>();
        assert!(driving.failure.is_none());
        assert_eq!(driving.runtime.ticks, 1);
        assert_eq!(driving.runtime.terminal_wheel(), None);
        assert_eq!(terminal_damage_message(&driving.runtime), None);
    }
}
