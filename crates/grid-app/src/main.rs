// Source reference viewer and serial driving prototype. A plain comment, not
// `//!`: src/lib.rs includes this file for the Android library (P9).

/// One stdout log line for the per-tick paths. Unlike `println!`, a failed
/// write (closed pipe, full disk) is dropped instead of panicking.
macro_rules! log_line {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        let _ = writeln!(std::io::stdout().lock(), $($arg)*);
    }};
}

#[cfg(target_os = "android")]
mod android;
#[cfg(target_os = "android")]
mod android_display;
#[cfg(target_os = "android")]
mod android_import;
// Mapping tested on the desktop; the JNI bridge is Android-only.
mod android_pad;
#[cfg(target_os = "android")]
mod android_sensor;
mod assists;
mod body_damage;
mod body_occlusion;
#[cfg(feature = "paint-specular")]
mod body_specular;
mod camera;
mod car_materials;
mod car_numbers;
mod career_screen;
mod clearcoat_prepass;
mod damage_hud;
mod damage_parts;
mod drift_event;
mod dynamic_props;
mod engine_sound;
mod event_menu;
mod flashback;
mod free_look;
mod hud_sound;
mod geometry_ray;
mod graphics;
mod graphics_benchmark;
mod perf_test;
mod perf_timing;
#[cfg(feature = "profile")]
mod span_profile;
mod ground_detail;
mod ground_effect_assets;
mod input;
mod mesh_stream;
mod motion_capture;
mod particles;
mod pause_menu;
mod post_race;
mod osd_scene;
mod race_hud;
mod race_osd;
mod render_scale;
mod replay;
mod scene_probe;
mod settings;
mod sidewall_filter;
mod skid_marks;
mod sky;
mod sky_environment;
mod source_trees;
mod foliage_mips;
mod fps_counter;
mod front_end;
mod texture_cache;
mod touge_event;
mod demolition_event;
mod le_mans_event;
mod texture_overrides;
mod texture_upload;
mod tilt;
mod time_trial;
mod touch_controls;
mod track_environment;
mod track_grade;
mod track_surface;
mod tyre_smoke;
mod tyre_sound;
mod tyre_visual;
mod wheel_debris;
mod wheel_exposure;
mod wheel_ground;
mod wheel_presentation;
use bevy::{
    app::{AppExit, ScheduleRunnerPlugin},
    asset::RenderAssetUsages,
    camera::RenderTarget,
    camera::primitives::{Aabb, MeshAabb},
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
use grid_tools::{car::Car as CarModel, route::Route};
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
/// Material/texture key prefix of the field's car model `m` (0 = the
/// player's model; `Field::models` order).
fn model_prefix(m: usize) -> String {
    format!("car{m}/")
}
/// The model index of a prefixed car material, e.g. `car1/Wheel` -> 1.
fn part_model(material: &str) -> Option<usize> {
    let rest = material.strip_prefix("car")?;
    let (index, _) = rest.split_once('/')?;
    index.parse().ok()
}
struct Part {
    name: String,
    /// Source joint (car draws only), e.g. `x0_bonnet`.
    joint: String,
    material: String,
    mesh: Mesh,
    matrices: Vec<Mat4>,
    wheel: Option<usize>,
}
/// The field's distinct car models (0 = the player's) and each car's model
/// index (car 0 is the player, car k opponent k).
#[derive(Resource)]
struct Field {
    models: Vec<CarModel>,
    car_model: Vec<usize>,
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
    /// Cars retired after their own tick stage failed (HUD; cleared by R).
    notices: Vec<String>,
    scripted_ticks: Option<u64>,
    scripted: grid_tools::source_vehicle::Controls,
    /// Scripted handbrake from this tick on (validation renders).
    handbrake_from: Option<u64>,
    last_controls: Option<grid_tools::source_vehicle::Controls>,
    segment: u64,
    started: Instant,
    opponents: Option<Opponents>,
    /// The player's auto-reset onto the racing line (ResetLinesManager, F341).
    auto_reset: grid_tools::source_vehicle::player_reset::PlayerReset,
    /// Its line; None disables it (unit tests).
    reset_line: Option<grid_tools::ai_driver::Line>,
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
    /// Scripted validation only (diagnostics autopilot with --drive-ticks): the
    /// ported AI drives the player's car.
    autopilot: Option<grid_tools::ai_driver::AiDriver>,
    /// Drives the player's car after the finish while the opponents race on
    /// (the original enables AI control of the finished car [I]).
    after_finish: Option<grid_tools::ai_driver::AiDriver>,
    /// Last throttle of each opponent (car k at k-1), for engine sound.
    throttles: Vec<f32>,
    /// Original race manager: catch-up and choreographer (`--ai`).
    pacing: grid_tools::ai_pacing::Field,
    /// Each opponent's AI data with its own car's statistics (car k at k-1).
    car_ai: Vec<grid_tools::ai_driver::AiData>,
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
    /// Speed (m/s) of every car; car 0 is the player runtime.
    fn speeds(&self, player: &grid_tools::source_vehicle::Runtime) -> Vec<f64> {
        std::iter::once(player)
            .chain(self.multi.cars.iter().skip(1))
            .map(|c| {
                let v = c.velocity();
                f64::from((v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt())
            })
            .collect()
    }
}
#[derive(Component)]
struct OpponentPart {
    car: usize,
    rest: Mat4,
    wheel: Option<usize>,
    /// Wheel rim or tyre: drawn in flight once the wheel is lost.
    lost_with_wheel: bool,
}
impl Driving {
    fn report(&self, event: &str, controls: grid_tools::source_vehicle::Controls) {
        // Performance test: Sample logging off drops the periodic samples only.
        if event == "tick" && perf_test::quiet_samples() {
            return;
        }
        let b = &self.runtime.state.car.owner.vehicle;
        let brakes: [f32; 4] = std::array::from_fn(|i| {
            f32::from_bits(self.runtime.state.car.frame.view[0x28 / 4 + i])
        });
        let hits = (0..4)
            .filter(|i| b[(0x1990 + i * 0x150 + 0xc0) / 4] != 0)
            .count();
        let damage_levels = [0, 4, 8, 0x10, 0x14, 0x28, 0x2c, 0x30, 0x34]
            .map(|o| f32::from_bits(b[(0xcdf0 + o) / 4]));
        log_line!(
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
        log_line!(
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
            log_line!(
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
            log_line!(
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
    /// Wheel rim or tyre: drawn in flight once the wheel is lost.
    lost_with_wheel: bool,
}
#[derive(Component)]
struct DeformingPart {
    source: Mesh,
    raw_bind: Mat4,
}
/// The deformation keys drive_present last projected: the player's, then
/// each opponent's (cleared when a replay hands the meshes back).
#[derive(Resource, Default)]
struct DeformationPresentation(Option<Vec<u32>>, Vec<Option<Vec<u32>>>);
/// Start-light board of the route's track (`start_lights.xml`). The emissive
/// texture is swapped per stage; `stage` 0 is the idle board.
#[derive(Resource)]
struct StartLights {
    material: Handle<StandardMaterial>,
    /// Idle, then the shipped stages.
    textures: Vec<Handle<Image>>,
    stage: usize,
}
/// The route's shipped start-light sequence: stage textures after the idle
/// board and its time, which is also the race countdown. Defaults to
/// Donington (startlights_lights_horizontal, 6 stages over 4.0 s); Nürburgring
/// and Istanbul Park ship 7 stages over 5.0 s.
#[derive(Resource, Clone, Debug)]
struct StartSequence {
    textures: Vec<String>,
    seconds: f64,
}
impl Default for StartSequence {
    fn default() -> Self {
        Self {
            textures: START_LIGHT_TEXTURES.map(String::from).to_vec(),
            seconds: START_SEQUENCE_SECONDS,
        }
    }
}
impl StartSequence {
    fn load(root: &Path, route: &Route) -> Result<Self, Box<dyn std::error::Error>> {
        let shipped = grid_tools::start_lights::load(root, route)?;
        Ok(Self {
            textures: std::iter::once(START_LIGHT_TEXTURES[0].to_owned())
                .chain(shipped.stages)
                .collect(),
            seconds: f64::from(shipped.seconds),
        })
    }
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

/// Wheel rim and tyre joints leave with a detached wheel (9FA6E0 detaches the
/// wheel part; 9D9F80 sizes its body by the tyre); discs stay on the hub.
fn lost_with_wheel(joint: &str) -> bool {
    joint
        .split_once('_')
        .is_some_and(|(_, name)| name.starts_with("wheel_") || name.starts_with("tyre_"))
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

// Provisional diffuse bindings recovered from executable static tuples. The
// texture names are per model (`lac_main.tga`, `bmw_main.tga`, ...).
fn diffuse(material: &str, model: &CarModel) -> Option<String> {
    let name = match material.trim() {
        "Car Body Paint" | "Matt" | "Shiny Metal" | "Bare Metal" | "Aerials" => "main",
        "Window Glass" | "Grills Glass" => "glass",
        "Cabin" => "cabin",
        "Wheel" | "Tyre Wall" => "wheel_tm",
        "Tyre Tread" => "tread_tm",
        "Suspension" => "susp",
        "Head Lights Pod" | "Rear Lights Pod" | "Lights Pod Matt" | "Head Lights Glass"
        | "Rear Lights Glass" | "Discs" | "Grills" => "lights",
        _ => return None,
    };
    Some(model.texture(name))
}
fn read(path: &Path) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use std::io::Read;
    // The path in the error: on a phone a missing pushed file must be named.
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
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
/// HUD name of a route's track folder, e.g. `istanbul_park` -> `Istanbul Park`.
fn track_label(route: &Route) -> String {
    route
        .track_name()
        .split('_')
        .map(|w| {
            let mut c = w.chars();
            c.next().map_or_else(String::new, |f| {
                f.to_ascii_uppercase().to_string() + c.as_str()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}
fn load(
    root: &Path,
    route: &Route,
    model: &CarModel,
    lod: &str,
    track: bool,
    include_batches: bool,
) -> Result<Car, Box<dyn std::error::Error>> {
    let mesh_path = if track {
        route.track_file("tracksplit.pssg")
    } else {
        model.file("_highLOD.pssg")
    };
    load_package(
        root,
        (route, model),
        lod,
        track,
        include_batches,
        &mesh_path,
        track,
    )
}

fn load_package(
    root: &Path,
    (route, model): (&Route, &CarModel),
    lod: &str,
    track: bool,
    include_batches: bool,
    mesh_path: &str,
    objects: bool,
) -> Result<Car, Box<dyn std::error::Error>> {
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
        if !track
            && lod == "LOD0_"
            && draw.material == b"Tyre Tread"
            && let Some(tangents) = car_materials::tread_tangents(source, draw.vertices.clone())
        {
            mesh.insert_attribute(Mesh::ATTRIBUTE_TANGENT, tangents);
        }
        parts.push(Part {
            name: String::from_utf8(draw.id.to_vec())?,
            joint: String::from_utf8(draw.joint.to_vec())?,
            material: String::from_utf8(draw.material.to_vec())?,
            mesh,
            matrices: vec![Mat4::from_cols_array(&draw.transform)],
            wheel: if track { None } else { wheel_joint(draw.joint) },
        });
    }
    let mut textures = BTreeMap::new();
    let texture_paths = if track {
        vec![mesh_path.to_owned()]
    } else {
        vec![model.livery(0), model.file("_gen_high.pssg")]
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
            let name = diffuse(&part.material, model)
                .ok_or_else(|| format!("unknown car material {:?}", part.material))?;
            bindings.insert(part.material.clone(), name);
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
    if objects {
        append_objects(root, route, &mut car)?;
    }
    Ok(car)
}
fn append_land(
    root: &Path,
    route: &Route,
    model: &CarModel,
    car: &mut Car,
) -> Result<(), Box<dyn std::error::Error>> {
    let land = load_package(
        root,
        (route, model),
        "LOD0_",
        true,
        false,
        &route.track_file("land.pssg"),
        false,
    )?;
    let draws = land.parts.len();
    for mut part in land.parts {
        part.name = format!("land/{}", part.name);
        part.material = format!("land/{}", part.material);
        car.parts.push(part);
    }
    for (name, texture) in land.bindings {
        car.bindings
            .insert(format!("land/{name}"), format!("land/{texture}"));
    }
    for (name, texture) in land.textures {
        car.textures.insert(format!("land/{name}"), texture);
    }
    println!(
        "source_land appended=true draws={draws} source=land.pssg namespace=land physics=unchanged"
    );
    Ok(())
}
fn append_objects(
    root: &Path,
    route: &Route,
    car: &mut Car,
) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = read(&root.join(route.track_file("objects.pssg")))?;
    let tree = pssg::read_tree(&bytes)?;
    let sources = mesh::read_static_meshes(&tree)?;
    let draws = scene::read_object_draws(&tree, &sources)?;
    let placement_bytes = read(&root.join(route.file("ornaments.bin")))?;
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
            joint: String::new(),
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
        ("objectstextures.pssg", route.track_file("patchup_ot.pssg")),
        (
            "lemans_sponsors.pssg",
            route.track_file("lemans_sponsors.pssg"),
        ),
        (
            "sponsor_minor.pssg",
            "track_sponsors/tag_heuer/sponsor_minor.pssg".into(),
        ),
        (
            "sponsor_event.pssg",
            "track_sponsors/bridgestone/sponsor_event.pssg".into(),
        ),
    ] {
        // Not every track ships the Le Mans sponsor package (Long Beach has
        // none); a binding that needs it still fails as unresolved below.
        if container == "lemans_sponsors.pssg" && !root.join(&path).is_file() {
            continue;
        }
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
    append_entities(root, route, car, &tree, &sources)?;
    Ok(())
}
fn append_entities(
    root: &Path,
    route: &Route,
    car: &mut Car,
    objects: &pssg::Tree<'_>,
    sources: &[mesh::Mesh<'_>],
) -> Result<(), Box<dyn std::error::Error>> {
    let placement_bytes = read(&root.join(route.file("objects.ens")))?;
    let type_bytes = read(&root.join(route.track_file("objecttypes.pssg")))?;
    let placement_tree = pssg::read_tree(&placement_bytes)?;
    let type_tree = pssg::read_tree(&type_bytes)?;
    let entities = entities::read(&placement_tree, &type_tree)?;
    // Breakable entities draw their intact state as their own parts.
    let catalog = dynamic_props::catalog(root, route);
    let breakables = catalog
        .as_ref()
        .map(dynamic_props::by_entity)
        .unwrap_or_default();
    let mut placements: BTreeMap<&[u8], Vec<dynamic_props::Placement>> = BTreeMap::new();
    let mut cloth = 0;
    let mut boxes = 0;
    let mut cylinders = 0;
    let mut spheres = 0;
    let mut links = 0;
    for (index, entity) in entities.iter().enumerate() {
        cloth += entity.cloth.len();
        links += entity.links;
        for renderable in &entity.renderables {
            if renderable.container != b"objects.pssg" {
                return Err("unsupported entity renderable container".into());
            }
            placements
                .entry(renderable.node)
                .or_default()
                .push((entity.transform, breakables.get(&index).copied()));
            for shape in &renderable.shapes {
                match shape.primitive {
                    entities::Primitive::Box { .. } => boxes += 1,
                    entities::Primitive::Cylinder { .. } => cylinders += 1,
                    entities::Primitive::Sphere { .. } => spheres += 1,
                }
            }
        }
    }
    let mut draw_count = 0;
    let mut triangles = 0;
    let closest_lod = scene::read_object_draws(objects, sources)?;
    let mut provisional_lod_instances = 0;
    let mut omitted_empty = 0;
    let mut omitted_unbound = 0;
    for (node, placements) in &placements {
        let mut draws = scene::read_renderable_draws(objects, sources, node)?;
        if draws.is_empty() {
            // Exact startup renderables containing anchors but no mesh.
            // Preview intact closest LOD; runtime substitution remains untraced.
            // Donington pairs are explicit; other routes try the renderable's
            // `<stem>_default_NN` stem as `<stem> Root` and otherwise omit it.
            let parts: Vec<&[u8]> = node.rsplitn(3, |&c| c == b'_').collect();
            let stem = (parts.len() == 3 && parts[1] == b"default")
                .then(|| [parts[2], b" Root".as_slice()].concat());
            let prototype: &[u8] = match *node {
                b"dcushion_green_a01_default_01" => b"cushion_blue Root",
                b"tyrestack_black_default_01" => b"tyrestack_black Root",
                _ if !route.is_default() => match &stem {
                    Some(stem) => stem,
                    None => {
                        omitted_empty += placements.len();
                        continue;
                    }
                },
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
            if draws.is_empty() && !route.is_default() {
                omitted_empty += placements.len();
                continue;
            }
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
                // Haruna's flag_a_01 has no diffuse binding; other routes omit
                // the draw (counted), the default route stays strict.
                if route.is_default() {
                    return Err(format!("unresolved entity material {material}").into());
                }
                omitted_unbound += 1;
                continue;
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
            for (placement, breakable) in placements {
                if let Some(b) = breakable {
                    draw_count += 1;
                    car.parts.push(Part {
                        name: dynamic_props::intact_name(*b, node, draw.id),
                        material: material.clone(),
                        mesh: mesh.clone(),
                        matrices: vec![Mat4::from_cols_array(&scene::compose(
                            &draw.transform,
                            placement,
                        ))],
                        wheel: None,
                        joint: String::new(),
                    });
                }
            }
            let matrices = placements
                .iter()
                .filter(|(_, breakable)| breakable.is_none())
                .map(|(placement, _)| {
                    Mat4::from_cols_array(&scene::compose(&draw.transform, placement))
                })
                .collect::<Vec<_>>();
            if matrices.is_empty() {
                continue;
            }
            draw_count += matrices.len();
            car.parts.push(Part {
                name: format!(
                    "entity/{}/{}",
                    String::from_utf8_lossy(node),
                    String::from_utf8_lossy(draw.id)
                ),
                joint: String::new(),
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
    if omitted_unbound > 0 {
        println!("entities omitted_unbound_material_draws={omitted_unbound}");
    }
    if spheres > 0 {
        // Static spheres (Milan and Detroit balloons) are route collision.
        println!("entities collision_spheres={spheres}");
    }
    if omitted_empty > 0 {
        println!("entities omitted_empty_renderable_instances={omitted_empty}");
    }
    if let Some(catalog) = &catalog {
        dynamic_props::append_pieces(car, catalog, objects, sources)?;
    }
    Ok(())
}
#[bevy_main]
fn main() {
    #[cfg(target_os = "android")]
    android::init();
    // Bundled data still being copied (F451): the copy screen took over.
    #[cfg(target_os = "android")]
    if !android::data_ready() {
        android::log(false, "data not ready: the copy screen opened instead");
        return;
    }
    #[cfg(not(target_os = "android"))]
    let args: Vec<String> = std::env::args().skip(1).collect();
    // No command line on Android: the event menu, or the race it chose.
    #[cfg(target_os = "android")]
    let args = event_menu::android_args();
    // `--menu`: the event menu and the races it starts, each its own
    // process (event_menu.rs).
    #[cfg(not(target_os = "android"))]
    if args.first().is_some_and(|a| a == "--menu") {
        match event_menu::supervise(&args[1..]) {
            Ok(code) => std::process::exit(code),
            Err(error) => {
                eprintln!("grid-app: {error}");
                std::process::exit(1);
            }
        }
    }
    // `--career`: the career screen and its races (career_screen.rs).
    #[cfg(not(target_os = "android"))]
    if args.first().is_some_and(|a| a == "--career") {
        match career_screen::supervise(&args[1..]) {
            Ok(code) => std::process::exit(code),
            Err(error) => {
                eprintln!("grid-app: {error}");
                std::process::exit(1);
            }
        }
    }
    let result = if args.iter().any(|a| a == "--menu-screen") {
        let rest: Vec<String> = args.into_iter().filter(|a| a != "--menu-screen").collect();
        event_menu::screen(&rest)
    } else if args.iter().any(|a| a == "--career-screen") {
        let rest: Vec<String> = args.into_iter().filter(|a| a != "--career-screen").collect();
        career_screen::screen(&rest)
    } else {
        run(args)
    };
    if let Err(error) = result {
        eprintln!("grid-app: {error}");
        #[cfg(target_os = "android")]
        {
            // On screen and in the log file; give the start of the error
            // screen's process a moment before this one exits.
            android::fatal(&format!("grid-app: {error}"));
            std::thread::sleep(Duration::from_millis(500));
        }
        std::process::exit(1);
    }
}
/// The game folder: `[game] root` / `--game-root`, else on Android the data
/// pushed or imported into the app's storage (docs/ANDROID.md), else the
/// Steam libraries.
fn game_root(root: Option<PathBuf>) -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(match root {
        Some(root) => root,
        None if cfg!(target_os = "android") => settings::data_dir().join("Grid"),
        None => grid_tools::install::discover(&grid_tools::install::steam_roots()).map_err(
            |tried| {
                format!(
                    "GRID was not found in the Steam libraries ({tried:?}); set [game] root in the settings file or pass --game-root"
                )
            },
        )?,
    })
}
fn load_collision(root: &Path, route: &Route) -> Result<Car, Box<dyn std::error::Error>> {
    let bytes = read(&root.join(route.file("track.jpk")))?;
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
                joint: String::new(),
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
fn grid_slot(root: &Path, route: &Route, name: &str) -> Result<Mat4, Box<dyn std::error::Error>> {
    let bytes = read(&root.join(route.file("grids.pssg")))?;
    let tree = pssg::read_tree(&bytes)?;
    let marker = grid_formats::grid::read_slot(&tree, name.as_bytes(), route.subroute())?;
    let transform = marker.horizontal_preview()?;
    println!(
        "grid marker {name} raw_matrix={:?} corners={:?} forward={:?}; horizontal preview only, final contact height/offset pending",
        marker.raw_transform, marker.corners, marker.forward
    );
    Ok(Mat4::from_cols_array(&transform))
}
/// Append field model `m`'s parts, materials and textures under
/// [`model_prefix`]; `anchor` places that model's NED coordinates.
fn append_car(
    root: &Path,
    lod: &str,
    car: &mut Car,
    anchor: Mat4,
    model: &CarModel,
    m: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let prefix = model_prefix(m);
    let mut source = load(root, &Route::default(), model, lod, false, false)?;
    for part in &mut source.parts {
        part.material = format!("{prefix}{}", part.material);
        for matrix in &mut part.matrices {
            *matrix = anchor * *matrix;
        }
    }
    for (name, data) in source.textures {
        car.textures.insert(format!("{prefix}{name}"), data);
    }
    for (name, texture) in source.bindings {
        car.bindings
            .insert(format!("{prefix}{name}"), format!("{prefix}{texture}"));
    }
    car.parts.extend(source.parts);
    Ok(())
}
/// Shipped high-detail liveries `livery_00..` of a model (Lacetti 10, BMW 13).
fn livery_count(root: &Path, model: &CarModel) -> usize {
    (0..100)
        .take_while(|&i| root.join(model.livery(i)).is_file())
        .count()
}
/// Livery package `livery_NN` of field model `m`, keyed like its textures.
fn load_livery(
    root: &Path,
    model: &CarModel,
    m: usize,
    index: usize,
) -> Result<BTreeMap<String, Vec<u8>>, Box<dyn std::error::Error>> {
    let bytes = read(&root.join(model.livery(index)))?;
    let tree = pssg::read_tree(&bytes)?;
    let mut out = BTreeMap::new();
    for texture in texture::read_textures(&tree)? {
        let (dds, _) = texture.validated_dds();
        out.insert(
            format!(
                "{}{}",
                model_prefix(m),
                String::from_utf8(texture.id.to_vec())?
            ),
            dds,
        );
    }
    Ok(out)
}
/// NED-to-physical translation of a runtime's car model: model wheel
/// coordinates are NED coordinates and the constructor shifts them into its
/// local physical frame.
fn model_offset(runtime: &grid_tools::source_vehicle::Runtime) -> Mat4 {
    let owner = &runtime.state.car.owner.vehicle;
    Mat4::from_translation(Vec3::new(
        f32::from_bits(owner[0x1630 / 4]),
        f32::from_bits(owner[0x1634 / 4]) - f32::from_bits(owner[0x1564 / 4]),
        f32::from_bits(owner[0x1638 / 4]),
    ))
}
/// Grid-fill seed of a series event: GRID_SERIES_SEED, else the clock
/// (docs/SERIES.md "Grid fill").
fn series_seed() -> u32 {
    std::env::var("GRID_SERIES_SEED")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_millis() as u32)
        })
}
fn run(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    println!("prototype_build revision={BUILD_REVISION}");
    // Settings files first (docs/SETTINGS.md); the flags below override them.
    let profile = settings::load(&args)?;
    let mut root = profile.root.clone();
    let mut route = match &profile.route {
        Some(path) => Route::new(path)?,
        None => Route::default(),
    }
    .with_subroute(profile.subroute.unwrap_or(0))?;
    let mut player_car = match &profile.car {
        Some(stem) => CarModel::new(stem)?,
        None => CarModel::default(),
    };
    let mut opponent_cars = profile
        .opponent_cars
        .as_deref()
        .map(CarModel::list)
        .transpose()?;
    let mut series = profile.series.clone();
    let mut car_chosen = profile.car.is_some();
    // Opponent k's livery package in a series event (docs/SERIES.md).
    let mut series_liveries: Vec<Option<usize>> = profile
        .opponent_liveries
        .as_ref()
        .map(|v| v.iter().map(|&i| Some(i)).collect())
        .unwrap_or_default();
    let opponent_names = profile.opponent_names.clone();
    // A Le Mans Series race's class name keys by car (results Class column).
    let mut car_classes: Vec<String> = Vec::new();
    let mut driver_aids = profile.assists()?;
    let mut input_settings = profile.input()?;
    let mut ai_settings = profile.ai()?;
    let mut output = None;
    let mut lod = String::from("LOD0_");
    let mut track = false;
    let mut include_batches = false;
    let mut from_menu = false;
    let mut career_result: Option<PathBuf> = None;
    let mut reference = false;
    let mut collision = false;
    let mut start_view = false;
    let mut eye = None;
    let mut target = None;
    let mut drive = profile.drive;
    let mut audio_check = false;
    let mut graphics = graphics::Settings::default();
    let mut source_haze = false;
    let mut reflection_calibration = false;
    let mut geometry_pixel = None;
    let mut render_fixed_step = false;
    let mut render_fps = None;
    let mut capture_orbit_rate = None;
    let mut motion_sequence = None;
    let mut texture_override_path = None;
    let mut trial_laps = profile.laps;
    let mut drive_ticks = None;
    let mut body_contacts = profile.body_contacts.clone();
    let mut body_events = profile.body_events.clone();
    let mut deformation = profile.deformation.clone();
    let mut discipline = profile.discipline.clone();
    let mut opponents = profile.opponents;
    let mut grid_position = profile.grid_position;
    // A settings-file table is optional: used when the private file exists.
    let mut ai_characteristics = profile.ai_characteristics.clone().filter(|p| p.is_file());
    let mut scripted = grid_tools::source_vehicle::Controls::default();
    let mut handbrake_from = None;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            // Already read by settings::load.
            "--settings" | "--data-dir" => {
                args.next();
            }
            "--drive" => drive = true,
            // Started by the event menu: leaving the event goes back to it.
            "--from-menu" => from_menu = true,
            // A career race: its result goes to this file (career_screen.rs).
            "--career-result" => {
                career_result = Some(PathBuf::from(args.next().ok_or("missing career result path")?));
            }
            "--audio-check" => audio_check = true,
            "--graphics" => {
                graphics.preset =
                    graphics::Preset::parse(&args.next().ok_or("missing graphics preset")?)?;
            }
            "--resolution" => graphics.resolution(&args.next().ok_or("missing resolution")?)?,
            "--clearcoat-prepass" => {
                graphics.clearcoat_prepass_fix = match args.next().as_deref() {
                    Some("fixed") => true,
                    Some("stock") => false,
                    _ => return Err("--clearcoat-prepass needs fixed or stock".into()),
                }
            }
            "--normal-detail" => {
                graphics.tread_normals = match args.next().as_deref() {
                    Some("off") => false,
                    Some("tread") => true,
                    _ => return Err("--normal-detail needs off or tread".into()),
                }
            }
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
            "--sun" => {
                let (direction, colour) = match args.next().as_deref() {
                    Some("authored") => (false, false),
                    Some("source") => (true, false),
                    Some("source-colour") => (true, true),
                    _ => return Err("--sun needs authored, source or source-colour".into()),
                };
                graphics.source_sun = direction;
                graphics.source_sun_colour = colour;
            }
            "--sky-tone" => {
                graphics.source_sky_tone = match args.next().as_deref() {
                    Some("raw") => false,
                    Some("source") => true,
                    _ => return Err("--sky-tone needs raw or source".into()),
                };
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
                deformation = Some(settings::Private::from_cli(
                    args.next()
                        .ok_or("--deformation needs a private DF01 path or derive")?,
                ))
            }
            "--discipline" => discipline = Some(args.next().ok_or("--discipline needs race, drift_battle or drift_grand_prix")?),
            "--opponents" => {
                opponents = Some(
                    args.next()
                        .ok_or("--opponents needs 1..19")?
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
                body_events = Some(settings::Private::from_cli(
                    args.next()
                        .ok_or("--body-events needs a private BP01 path or derive")?,
                ));
            }
            "--body-contacts" => {
                body_contacts = Some(settings::Private::from_cli(
                    args.next()
                        .ok_or("--body-contacts needs a private CD01 path or derive")?,
                ));
            }
            "--drive-ticks" => {
                drive_ticks = Some(args.next().ok_or("missing tick count")?.parse::<u64>()?)
            }
            "--throttle" => scripted.throttle = args.next().ok_or("missing throttle")?.parse()?,
            "--steering" => scripted.steering = args.next().ok_or("missing steering")?.parse()?,
            "--handbrake" => {
                handbrake_from = Some(args.next().ok_or("--handbrake needs a tick")?.parse()?)
            }
            "--game-root" => root = Some(args.next().ok_or("--game-root needs a path")?.into()),
            "--assists" => {
                driver_aids = grid_tools::source_vehicle::assists::Assists::parse(
                    &args.next().ok_or("--assists needs key=value items")?,
                )?
            }
            "--assists-file" => {
                (driver_aids, input_settings) =
                    assists::read_file(&args.next().ok_or("--assists-file needs a path")?)?
            }
            "--input" => {
                input_settings =
                    input::Settings::parse(&args.next().ok_or("--input needs key=value items")?)?
            }
            "--ai" => {
                ai_settings = grid_tools::ai_pacing::Settings::parse(
                    &args.next().ok_or("--ai needs key=value items")?,
                )?
            }
            "--car" => {
                player_car = CarModel::new(&args.next().ok_or("--car needs a cars/<stem> name")?)?;
                car_chosen = true;
            }
            "--series" => {
                let name = args.next().ok_or("--series needs a series name")?.to_ascii_lowercase();
                if !grid_tools::series::CIRCUIT_SERIES.contains(&name.as_str()) {
                    return Err(format!(
                        "--series needs one of {}",
                        grid_tools::series::CIRCUIT_SERIES.join(", ")
                    )
                    .into());
                }
                series = Some(name);
            }
            "--opponent-cars" => {
                opponent_cars = Some(CarModel::list(
                    &args.next().ok_or("--opponent-cars needs STEM[,STEM...]")?,
                )?)
            }
            "--route" => {
                let subroute = route.subroute();
                route = Route::new(
                    &args
                        .next()
                        .ok_or("--route needs tracks/<region>/<track>/<route>")?,
                )?
                .with_subroute(subroute)?
            }
            "--subroute" => {
                let subroute = args.next().ok_or("--subroute needs 0 or 1")?.parse()?;
                route = route.with_subroute(subroute)?
            }
            "--render" => {
                output = Some(PathBuf::from(
                    args.next().ok_or("--render needs a PNG path")?,
                ))
            }
            "--texture-overrides" => {
                texture_override_path = Some(PathBuf::from(
                    args.next().ok_or("missing private texture manifest")?,
                ))
            }
            "--motion-capture" => {
                motion_sequence = Some(motion_capture::Config::parse(
                    &args.next().ok_or("missing motion capture window")?,
                )?)
            }
            "--reflection-calibration" => reflection_calibration = true,
            "--render-fixed-step" => render_fixed_step = true,
            "--render-fps" => {
                let fps: u32 = args.next().ok_or("missing render FPS")?.parse()?;
                if !(30..=300).contains(&fps) {
                    return Err("diagnostic render FPS must be 30..300".into());
                }
                render_fps = Some(fps);
            }
            "--motion-orbit-rate" => {
                let rate: f32 = args.next().ok_or("missing motion orbit rate")?.parse()?;
                if !rate.is_finite() || rate.abs() > 3. {
                    return Err("motion orbit rate must be finite, -3..3 rad/s".into());
                }
                capture_orbit_rate = Some(rate);
            }
            "--haze" => {
                source_haze = match args.next().as_deref() {
                    Some("source") => true,
                    Some("off") => false,
                    _ => return Err("--haze needs source or off".into()),
                };
            }
            "--shadow-distance" => {
                graphics.shadow_distance(&args.next().ok_or("--shadow-distance needs metres")?)?;
            }
            "--capture-timeout" => {
                graphics.capture_timeout(&args.next().ok_or("--capture-timeout needs seconds")?)?;
            }
            "--grass-alpha" => {
                graphics.grass_soft_alpha = match args.next().as_deref() {
                    Some("mask") => false,
                    Some("blend") => true,
                    _ => return Err("--grass-alpha needs mask or blend".into()),
                };
            }
            "--ao" | "--grass-shadows" => {
                let enabled = match args.next().as_deref() {
                    Some("on") => true,
                    Some("off") => false,
                    _ => return Err("--ao/--grass-shadows needs on or off".into()),
                };
                if arg == "--ao" {
                    graphics.ambient_occlusion = enabled;
                } else {
                    graphics.grass_receive_shadows = enabled;
                }
            }
            "--aa" => {
                graphics.anti_aliasing = match args.next().as_deref() {
                    Some("taa") => 0,
                    Some("smaa") => 1,
                    Some("off") => 2,
                    _ => return Err("--aa needs taa, smaa or off".into()),
                };
            }
            "--wheel-motion" => {
                graphics.smooth_wheel_motion = match args.next().as_deref() {
                    Some("smooth") => true,
                    Some("source") => false,
                    _ => return Err("--wheel-motion needs smooth or source".into()),
                };
            }
            "--wheel-shutter" => {
                let ms: f32 = args
                    .next()
                    .ok_or("missing wheel shutter milliseconds")?
                    .parse()?;
                if !ms.is_finite() || !(0.0..=33.333).contains(&ms) {
                    return Err("wheel shutter must be finite, 0..33.333 milliseconds".into());
                }
                graphics.wheel_shutter_seconds = ms * 0.001;
            }
            "--wheel-geometry-shutter" => {
                let ms: f32 = args
                    .next()
                    .ok_or("missing geometry shutter milliseconds")?
                    .parse()?;
                if !ms.is_finite() || !(0.0..=33.333).contains(&ms) {
                    return Err("geometry shutter must be finite, 0..33.333 milliseconds".into());
                }
                graphics.wheel_geometry_shutter_seconds = ms * 0.001;
            }
            "--wheel-max-arc" => {
                let degrees: f32 = args.next().ok_or("missing wheel maximum arc degrees")?.parse()?;
                if !degrees.is_finite() || !(0.0..=90.0).contains(&degrees) {
                    return Err("wheel maximum arc must be finite,0..90 degrees".into());
                }
                graphics.wheel_max_arc = Some(degrees.to_radians());
                println!("wheel_max_arc degrees={degrees} scope=sidewall-and-player-hull authored=true tradeoff=detail-over-alias-suppression");
            }
            "--wheel-filter-floor" => {
                let floor: f32 = args.next().ok_or("missing wheel filter floor")?.parse()?;
                if !floor.is_finite() || !(0.0..=0.5).contains(&floor) { return Err("wheel filter floor must be0..0.5 of frame seconds".into()); }
                graphics.wheel_filter_floor = floor;
                println!("wheel_filter_floor sigma_frame_fraction={floor} scope=adaptive-only authored=true");
            }
            "--wheel-exposure" => {
                graphics.adaptive_wheel_exposure = match args.next().as_deref() {
                    Some("fixed") => false,
                    Some("adaptive") => true,
                    _ => return Err("--wheel-exposure needs fixed or adaptive".into()),
                };
            }
            "--body-specular" => {
                let value = args.next().ok_or("--body-specular needs off, source-base or white-control")?;
                graphics.source_body_specular = match value.as_str() {
                    "off" => false,
                    "source-base" | "white-control" if cfg!(feature = "paint-specular") => true,
                    "source-base" | "white-control" => return Err("body specular masks need a build with --features paint-specular".into()),
                    _ => return Err("--body-specular needs off, source-base or white-control".into()),
                };
                graphics.white_body_specular = value == "white-control";
            }
            "--body-occlusion" => {
                graphics.source_body_occlusion = match args.next().as_deref() {
                    Some("source-ambient") => true,
                    Some("off") => false,
                    _ => return Err("--body-occlusion needs source-ambient or off".into()),
                };
            }
            "--numbers" => {
                graphics.source_race_numbers = match args.next().as_deref() {
                    Some("source") => true, Some("off") => false,
                    _ => return Err("--numbers needs source or off".into()),
                };
            }
            "--race-number" => {
                graphics.race_number(&args.next().ok_or("--race-number needs 0..999")?)?;
            }
            "--glass-reflectance" => {
                graphics.glass_reflectance(&args.next().ok_or("--glass-reflectance needs 0..1")?)?;
            }
            "--glass-roughness" => {
                graphics.glass_roughness(&args.next().ok_or("--glass-roughness needs 0.04..1")?)?;
            }
            "--exposure" => {
                graphics.exposure(&args.next().ok_or("--exposure needs EV100")?)?;
            }
            "--trees" => {
                let mode = args.next().ok_or("--trees needs source, coverage, coverage-shaded or off")?;
                graphics.source_trees = match mode.as_str() {
                    "source" | "coverage" | "coverage-shaded" => true,
                    "off" => false,
                    _ => return Err("--trees needs source, coverage, coverage-shaded or off".into()),
                };
                graphics.foliage_coverage = matches!(mode.as_str(), "coverage" | "coverage-shaded");
                graphics.tree_sheet_colours = mode == "coverage-shaded";
            }
            "--land" => {
                graphics.source_land = match args.next().as_deref() {
                    Some("source") => true,
                    Some("off") => false,
                    _ => return Err("--land needs source or off".into()),
                };
            }
            "--geometry-ray" => {
                let value = args.next().ok_or("--geometry-ray needs X,Y")?;
                let (x, y) = value.split_once(',').ok_or("--geometry-ray needs X,Y")?;
                let pixel = Vec2::new(x.parse()?, y.parse()?);
                if !pixel.is_finite() || pixel.min_element() < 0. {
                    return Err("invalid geometry pixel".into());
                }
                geometry_pixel = Some(pixel);
            }
            "--probe-update" => {
                graphics.moving_probe = match args.next().as_deref() {
                    Some("static") => false,
                    Some("moving") => true,
                    _ => return Err("--probe-update needs static or moving".into()),
                };
            }
            "--probe-size" => {
                let size = args
                    .next()
                    .ok_or("--probe-size needs 256, 512 or 1024")?
                    .parse()?;
                if !matches!(size, 256 | 512 | 1024) {
                    return Err("--probe-size needs 256, 512 or 1024".into());
                }
                graphics.probe_size = size;
            }
            "--ssr-thickness" => {
                let value: f32 = args
                    .next()
                    .ok_or("--ssr-thickness needs 0.005..1")?
                    .parse()?;
                if !value.is_finite() || !(0.005..=1.).contains(&value) {
                    return Err("--ssr-thickness needs a finite value in 0.005..1".into());
                }
                graphics.ssr_thickness = value;
            }
            "--reflections" => {
                graphics.reflection_mode = match args.next().as_deref() {
                    Some("sky") => 0,
                    Some("deferred") => 1,
                    Some("ssr") => 2,
                    Some("probe") => 3,
                    _ => return Err("--reflections needs sky, deferred, ssr or probe".into()),
                };
            }
            "--track-materials" => {
                graphics.layered_track = match args.next().as_deref() {
                    Some("source") => true,
                    Some("single") => false,
                    _ => return Err("--track-materials needs source or single".into()),
                };
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
                let scene = args.next().ok_or("--scene needs car or donington")?;
                track = match scene.as_str() {
                    "car" | "lacetti" => {
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
                #[cfg(feature = "paint-specular")]
                println!("--body-specular off|source-base|white-control: authored source-alpha base dielectric F0 mask; coat unchanged, off default, not original shader parity.");
                println!("--body-occlusion off|source-ambient: optional source green paint mask adapted to PBR ambient occlusion; off default, not the original full lighting model.");
                println!(
                    "--numbers source|off: compose saved per-livery race numbers in enhanced graphics; off default.\n--race-number DIGITS: override the player's number with 1..3 decimal digits; leading zeroes are preserved."
                );
                println!(
                    "--grade authored|track: enhanced color treatment; track reads saved exterior controls, with an authored display-domain mapping (not 1:1 original postprocessing)."
                );
                println!(
                    "--benchmark-frames 1..3600: main-schedule wall intervals after 240 warmup frames; requires --render; stops scripted driving early at benchmark completion. Not GPU/presentation FPS."
                );
                println!(
                    "--sky source|procedural: enhanced Donington background (source is default; sky IBL remains procedural).\n--sky-tone raw|source: optional saved scale/exponent for the sky and source IBL; raw default.\n--sun authored|source|source-colour: recovered diffuse-sun direction, optionally saved colour ratios; radiometry remains authored."
                );
                println!(
                    "--graphics baseline|enhanced: independent rendering presets; baseline is the current prototype.\n--resolution WIDTHxHEIGHT: window or offscreen size, e.g. 3840x2160."
                );
                println!(
                    "--reflections sky|deferred|ssr|probe: experimental reflections; sky is default.\n--probe-size 256|512|1024: HDR capture/specular face size (1024 default for probe).\n--probe-update static|moving: freeze near start or follow the player; static default.\n--reflection-calibration: diagnostic mirror sphere, excluded from probe and physics."
                );
                println!(
                    "grid-app [--game-root PATH] [--route tracks/REGION/TRACK/ROUTE] [--car STEM] [--scene car|donington|reference|collision] [--lod LOD0_|LOD1_] [--view overview|start] [--camera X,Y,Z --look-at X,Y,Z] [--include-batches] [--render local/PATH.png]\n--drive: source driving prototype of the selected car and route. Arrows/WASD drive; Space handbrake; Escape/P (pad Start) pause menu: Continue, Restart, Options, Retire from Race (races) or Finish Test Drive (practice); R restarts. Free practice is the default.\n--time-trial-laps 1..99: independent finite event, 3-second countdown, results and private personal bests.\n--body-contacts PRIVATE_CD01|derive: experimental Donington packed-track and CQTC boundary contacts with selected collision damage; shared material and boundary-first order. Terminal wheel damage disables driving; R resets.\n--drive-ticks N --throttle 0..1 --steering -1..1 [--handbrake FROM_TICK]: bounded scripted render validation.\nStatic viewer: arrows orbit; WASD pan; Q/E vertical; Shift faster; PageUp/PageDown zoom; Home reset."
                );
                println!(
                    "--route: purchased circuit route folder (default tracks/europe/donington/route_0); the donington/reference/collision scenes and driving use it. Time-trial bests are kept for the default route and car only."
                );
                println!(
                    "--car STEM: purchased car folder cars/STEM for the player and the car scene (default lac; bmw is the other Touring_cars model). --opponent-cars STEM[,STEM...]: opponent models, cycled over the field (default: the player's car; bmw,lac alternates as in the euro_touring 6+6 roster)."
                );
                println!("{}", settings::HELP);
                println!("{}", assists::HELP);
                println!("{}", input::HELP);
                println!("{}", grid_tools::ai_pacing::Settings::HELP);
                println!(
                    "--opponents 1..19: AI cars in the shared world (needs --body-contacts); --grid-position N: player grid slot (1 = pole)."
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
    let root = game_root(root)?;
    println!("game_root path={}", root.display());
    // The first loader's error names only an OS error; say what is missing.
    #[cfg(target_os = "android")]
    if !root.join("tracks").is_dir() {
        return Err(format!(
            "no game data in {}: this APK carries none; start the app again to import your GRID folder, or push the data (docs/ANDROID.md)",
            root.display()
        )
        .into());
    }
    if let Some(name) = &series {
        // Le Mans Series grid option (AUTHORED mapping of the settings):
        // `grid_position = back` is the back of the player's class group,
        // anything else its front.
        let event = grid_tools::series::Event::prepare_on_grid(
            &root,
            name,
            &route,
            car_chosen.then_some(&player_car),
            opponents,
            opponent_cars.is_none(),
            series_seed(),
            if profile.grid_back { 1 } else { 0 },
        )?;
        if let Some(slot) = event.player_slot {
            grid_position = Some(slot + 1);
        }
        player_car = event.player.clone();
        opponents = Some(event.opponent_count);
        if opponent_cars.is_none() {
            opponent_cars = Some(event.cars());
            series_liveries = event.opponents.iter().map(|e| e.livery_index).collect();
            car_classes = event.classes.clone();
        }
        println!("{}", event.log_line());
    }
    // Le Mans 24 Hours: a time-limited race (race type 3) of `time_limit`
    // real seconds (720 in the career [D]) running 24 h of day [V 0x669700];
    // the career events' 8 laps only cap the lap count (docs/LE_MANS.md).
    let race_limit = series
        .as_deref()
        .is_some_and(grid_tools::series::is_le_mans_24)
        .then(|| profile.time_limit.unwrap_or(720.));
    if let Some(limit) = race_limit {
        if !drive || opponents.is_none() {
            return Err("the Le Mans 24 Hours needs --drive and opponents (docs/LE_MANS.md)".into());
        }
        trial_laps.get_or_insert(8);
        println!(
            "drive_le_mans version=1 time_limit={limit} clock_scale={} laps={}",
            grid_tools::time_of_day::clock_scale(limit),
            trial_laps.unwrap_or(8)
        );
    }
    let start_sequence = StartSequence::load(&root, &route)?;
    if graphics.enhanced() && (!route.is_default() || !player_car.is_default()) {
        return Err("enhanced graphics currently requires Lacetti and Donington route_0; baseline supports other cars/routes".into());
    }
    if profile.grid_back && grid_position.is_none() {
        grid_position = opponents.map(|n| n + 1);
    }
    if audio_check {
        return engine_sound::check(&root);
    }
    if graphics.benchmark_frames.is_some() && output.is_none() {
        return Err("--benchmark-frames requires --render for bounded unattended exit".into());
    }
    if graphics.source_sun {
        if !graphics.enhanced() || (!track && !drive) {
            return Err("--sun source requires an enhanced track scene".into());
        }
        let bytes = read(&root.join(route.track_file("lighting.xml")))?;
        let sun = grid_formats::track_lighting::read_sun(&bytes)?;
        println!("source_sun angles={sun:?} direction_to_light={:?} mapping=source-basis radiometry=authored", sun.direction_to_light());
        graphics.saved_sun = Some(sun);
        if graphics.source_sun_colour {
            let rgb = grid_formats::track_lighting::read_sun_colour(&bytes)?;
            println!("source_sun_colour normalized_rgb={rgb:?} radiometry=authored colour_space=assumed-linear");
            graphics.saved_sun_colour = Some(rgb);
        }
    }
    if source_haze {
        if !graphics.enhanced() || (!track && !drive) {
            return Err("--haze source requires an enhanced track scene".into());
        }
        let bytes = read(&root.join(route.track_file("lighting.xml")))?;
        graphics.saved_fog = Some(grid_formats::track_lighting::read_fog(&bytes)?);
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
    // A point-to-point route (Haruna) is one run from its start to its
    // finish: touge events run one lap [D event_track_pool num_laps 1].
    if trial_laps.is_some()
        && grid_tools::lap_progress::load_route(&root, &route)
            .is_ok_and(|p| p.progress.kind == grid_formats::progress::TrackKind::PointToPoint)
    {
        trial_laps = Some(1);
    }
    if (body_contacts.is_some() || body_events.is_some()) && !drive {
        return Err("--body-contacts and --body-events require --drive".into());
    }
    if deformation.is_some() && (!drive || body_contacts.is_none()) {
        return Err("--deformation requires --drive and --body-contacts".into());
    }
    if opponents.is_some_and(|n| !(1..=19).contains(&n))
        || (opponents.is_some() && (!drive || body_contacts.is_none()))
    {
        return Err("--opponents 1..19 requires --drive and --body-contacts".into());
    }
    use grid_tools::virtual_race::Format;
    // A Drift GP runs the player alone; its rivals are virtual (docs/DISCIPLINES.md).
    let drift_format = match discipline.as_deref() {
        None | Some("race") => None,
        Some("drift_battle") if trial_laps.is_some() && opponents.is_some() => Some(Format::DriftBattle),
        Some("drift_battle") => return Err("drift_battle needs laps and opponents (docs/DISCIPLINES.md)".into()),
        Some("drift_grand_prix") if trial_laps.is_some() && opponents.is_none() => Some(Format::DriftGrandPrix),
        Some("drift_grand_prix") => {
            return Err("drift_grand_prix needs laps and no opponents (docs/DISCIPLINES.md)".into());
        }
        // Freestyle drift: the player alone for a time limit, no progress.
        Some("drift_freestyle") if drive && opponents.is_none() => Some(Format::DriftFreestyle),
        Some("drift_freestyle") => {
            return Err("drift_freestyle needs --drive and no opponents (docs/DISCIPLINES.md)".into());
        }
        Some("pro_touge" | "demolition_derby") => None,
        Some(other) => {
            return Err(format!("unknown discipline {other}: race, drift_battle, drift_grand_prix, drift_freestyle or pro_touge").into());
        }
    };
    let freestyle = drift_format == Some(Format::DriftFreestyle);
    // Pro Touge: two runs against one AI car on a point-to-point road.
    let pro_touge = discipline.as_deref() == Some("pro_touge");
    // Demolition Derby: a lap race of the field with disqualification
    // (docs/DEMOLITION_DERBY.md).
    let demolition = discipline.as_deref() == Some("demolition_derby");
    if demolition && (opponents.is_none() || trial_laps.is_none()) {
        return Err("demolition_derby needs laps and opponents (docs/DEMOLITION_DERBY.md)".into());
    }
    // The AI config section of the event's format [D ai_driver_config.xml;
    // I: sections are named by format, F290].
    let ai_format = if demolition {
        Some("single_demolition_race")
    } else if pro_touge {
        Some("pro_touge")
    } else {
        drift_format.and_then(|f| (f == Format::DriftBattle).then_some("drift_battle"))
    };
    if pro_touge && (opponents != Some(1) || !drive) {
        return Err("pro_touge needs --drive and one opponent (docs/TOUGE.md)".into());
    }
    if freestyle {
        // [V] drift_freestyle ignores race progress (Route::ignoring_progress);
        // the event is one timed run that ends at its time limit.
        route = route.ignoring_progress();
        trial_laps = Some(1);
    }
    let drift = drift_format == Some(Format::DriftBattle);
    if let Some(position) = grid_position
        && !opponents.is_some_and(|n| (1..=n + 1).contains(&position))
    {
        return Err("--grid-position needs --opponents and 1..opponents+1".into());
    }
    // The player's grid slot; the opponents fill the remaining slots in order.
    let player_slot = grid_position.map_or(0, |p| p - 1);
    let contacts = body_contacts
        .map(|source| source.bytes(&root, "CD01"))
        .transpose()?;
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
        } else if contacts.is_some() {
            Mode::Body
        } else {
            Mode::Wheels
        }
    };
    let deformation = deformation
        .map(|source| source.bytes(&root, "DF01"))
        .transpose()?;
    let mut drive_input = None;
    // Phone controls (P11): `None` when off, else the tilt settings (if any).
    let mut drive_touch: Option<Option<tilt::Settings>> = None;
    let mut touge_run = None;
    let driving = if drive {
        track = true;
        reference = true;
        collision = false;
        start_view = true;
        // The device class decides assisted steering (record +0x33 -> +E6C).
        input_settings.apply_class(&mut driver_aids);
        #[cfg(target_os = "android")]
        let touch_screen = android::touch_screen();
        #[cfg(not(target_os = "android"))]
        let touch_screen = false;
        let touch = input_settings.touch_active(touch_screen);
        let (actions, pad) = input::load(&root, &input_settings, touch)?;
        if touch {
            let tilt = input_settings.tilt_settings();
            println!(
                "drive_input {input_settings} pad_preset={pad} touch=on touch_screen={touch_screen} tilt={tilt:?}"
            );
            drive_touch = Some(tilt);
        } else {
            println!("drive_input {input_settings} pad_preset={pad}");
        }
        drive_input = Some(actions);
        // One race of the event: the player's runtime and the AI field on
        // `route` from `player_slot` (built again for a Pro Touge's second
        // run, docs/TOUGE.md).
        #[allow(clippy::type_complexity)]
        let build_race = |route: &Route,
                          player_slot: usize|
         -> Result<
            (
                grid_tools::source_vehicle::Runtime,
                Option<grid_tools::body_geometry::BodyGeometry>,
                Option<Opponents>,
            ),
            Box<dyn std::error::Error>,
        > {
        let mut runtime = grid_tools::source_vehicle::Runtime::load(
            &root,
            route,
            &player_car,
            &format!("slot_{player_slot:02}"),
            200679,
        )?;
        println!("drive_car car={player_car} label={:?}", player_car.label());
        runtime.set_assists(driver_aids)?;
        println!("drive_assists {driver_aids}");
        runtime.enable_lap_timing(&root)?;
        if route.ignores_progress() {
            println!("drive_lap_timing enabled=false reason=progress-ignored");
        } else {
            println!(
                "drive_lap_timing enabled=true gates={} required_gates={} finish=0 clock=successful-simulation-ticks policy=independent-ordered-route-splits",
                runtime.lap_gate_count(),
                runtime.lap_required_gate_count()
            );
        }
        if let Some(source) = &body_events {
            let bytes = source.bytes(&root, "BP01")?;
            if bytes.len() != 28 {
                return Err("invalid private body event parameter extent".into());
            }
            runtime.body_events = Some(grid_tools::body_impact_parameters::read(&bytes)?);
            println!(
                "drive_body_events enabled=true scope=static-and-car-peers notifications=empty persistent-chains=absent"
            );
        }
        let geometry = contacts.as_ref().map(|table| -> Result<_, Box<dyn std::error::Error>> {
            if table.len() != 712 {
                return Err("invalid private body sampling extent".into());
            }
            let mut geometry = runtime.prepare_body_geometry(&root, table)?;
            if let Some(bytes)=&deformation {
                if bytes.len()!=252{return Err("invalid private deformation parameter extent".into());}
                let (limit,bulk)=grid_tools::deformation_parameters::read(bytes)?;
                runtime.enable_deformation(&root,&mut geometry)?;
                runtime.register_deformation_callbacks(limit,bulk,1)?;
                println!("drive_deformation enabled=true scope=static-and-car-peers source=player-lod1 wheel-view=source-wheel-anchors crt_seed=00000001 body_graphics=independent-grid-projection");
            }
            println!("drive_body_geometry enabled=true pool=20 boundary=cqtc order=boundary-first-then-archive materials=shared damage=owned-rng notifications=guarded");
            runtime.enable_dynamic_props(&root)?;
            println!("drive_dynamic_props enabled=true scope=constraint-free-breakables thresholds=physparams pieces=shared-world-bodies");
            Ok(geometry)
        }).transpose()?;
        let opponents = opponents
            .map(|n| -> Result<Opponents, Box<dyn std::error::Error>> {
                let table = contacts.as_deref().ok_or("missing CD01")?;
                let slots: Vec<String> = std::iter::once(player_slot)
                    .chain((0..=n).filter(|&i| i != player_slot))
                    .map(|i| format!("slot_{i:02}"))
                    .collect();
                let refs: Vec<&str> = slots.iter().map(String::as_str).collect();
                let parameters = deformation
                    .as_ref()
                    .map(|bytes| grid_tools::deformation_parameters::read(bytes))
                    .transpose()?;
                // Car 0 is the player's model; opponent k cycles the list.
                let field: Vec<CarModel> = std::iter::once(player_car.clone())
                    .chain((0..n).map(|i| {
                        opponent_cars.as_ref().map_or_else(
                            || player_car.clone(),
                            |list| list[i % list.len()].clone(),
                        )
                    }))
                    .collect();
                let entries: Vec<(&str, &CarModel)> =
                    refs.iter().copied().zip(field.iter()).collect();
                let mut multi = grid_tools::source_vehicle::multi::MultiRuntime::load_deforming_on(
                    &root,
                    route,
                    &entries,
                    table,
                    runtime.body_events,
                    parameters,
                )?;
                multi.enable_dynamic_props(&root)?;
                use grid_tools::ai_driver::{AiData, AiDriver, Parameters};
                // Every model's own AI classes and statistics (database vehicle row).
                let mut by_model = BTreeMap::new();
                for model in &field {
                    if !by_model.contains_key(model) {
                        // A Drift Battle reads the drift_battle AI config (docs/DISCIPLINES.md).
                        let data = AiData::load_route(
                            &root,
                            route,
                            model,
                            ai_format,
                        )?;
                        by_model.insert(model.clone(), data);
                    }
                }
                let ai = by_model[&player_car].clone();
                let car_ai: Vec<AiData> = field[1..].iter().map(|m| by_model[m].clone()).collect();
                let difficulty = ai_settings.difficulty;
                let parameters = Parameters::new(&ai, difficulty, |_| 1.)?;
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
                        let p = Parameters::new(&car_ai[i], difficulty, |name| {
                            row.and_then(|r| r.get(name).copied()).unwrap_or(1.)
                        })?;
                        Ok(AiDriver::new(p, 0x5eed + i as u32 + 1))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let mistakes = (0..=n)
                    .map(|k| {
                        characteristics
                            .get(k % characteristics.len().max(1))
                            .and_then(|r| r.get("mistakes").copied())
                            .unwrap_or(1.)
                    })
                    .collect();
                if !characteristics.is_empty() {
                    println!(
                        "drive_ai_characteristics drivers={} source=private-table",
                        characteristics.len()
                    );
                }
                println!(
                    "drive_opponents enabled=true count={n} world=shared car_contacts=two-body driver=original-ai-port difficulty={difficulty} deformation={}",
                    if deformation.is_some() { "per-car-static-peers" } else { "disabled" }
                );
                println!("drive_ai_pacing {ai_settings}");
                let progress = grid_tools::lap_progress::load_route(&root, route)?.progress;
                let centre = |gate: &grid_formats::progress::Gate| -> [f32; 3] {
                    std::array::from_fn(|k| (gate.left[k] + gate.right[k]) * 0.5)
                };
                let start = centre(&progress.gates[0]);
                // A point-to-point run (Haruna) finishes at its last gate.
                let run_finish = (progress.kind
                    == grid_formats::progress::TrackKind::PointToPoint)
                    .then(|| centre(progress.gates.last().unwrap()));
                let mut positions = vec![{
                    let p = runtime.pose();
                    [p[12], p[13], p[14]]
                }];
                positions.extend(multi.cars.iter().skip(1).map(|c| {
                    let p = c.pose();
                    [p[12], p[13], p[14]]
                }));
                let standings = match run_finish {
                    Some(finish) => {
                        grid_tools::race::Standings::new_run(&ai.line, start, finish, &positions)
                    }
                    None => {
                        let standings = grid_tools::race::Standings::new(
                            &ai.line,
                            trial_laps.unwrap_or(1),
                            start,
                            &positions,
                        );
                        match race_limit {
                            Some(limit) => standings.with_time_limit(limit),
                            None => standings,
                        }
                    }
                };
                let pacing = grid_tools::ai_pacing::Field::new(
                    grid_tools::ai_pacing::Data::load_for(
                        &root,
                        &ai_settings,
                        if drift { "drift_battle" } else { "circuit" },
                    )?,
                    ai_settings.clone(),
                    &ai.line,
                    trial_laps.unwrap_or(1),
                    start,
                    &positions,
                    0,
                    mistakes,
                    0x5eed,
                );
                let names = std::iter::once("You".to_string())
                    .chain((1..=n).map(|i| {
                        opponent_names
                            .as_ref()
                            .and_then(|v| v.get(i - 1))
                            .filter(|s| !s.is_empty())
                            .cloned()
                            .unwrap_or_else(|| format!("AI {i}"))
                    }))
                    .collect();
                let autopilot = (drive_ticks.is_some()
                    && crate::settings::diagnostics().autopilot)
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
                    after_finish: None,
                    throttles: vec![0.; n],
                    pacing,
                    car_ai,
                })
            })
            .transpose()?;
            Ok((runtime, geometry, opponents))
        };
        let (runtime, geometry, opponents) = build_race(&route, player_slot)?;
        // A Pro Touge's second run: the reverse subroute, the slots swapped
        // [I] (docs/TOUGE.md §7).
        if pro_touge {
            let reverse = route.clone().with_subroute(1)?;
            let (runtime, geometry, opponents) = build_race(&reverse, 1 - player_slot)?;
            let length = |r: &Route| -> Result<f64, Box<dyn std::error::Error>> {
                Ok(f64::from(grid_tools::lap_progress::load_route(&root, r)?.progress.total_distance))
            };
            touge_run = Some((
                touge_event::NextRun {
                    runtime,
                    geometry,
                    opponents,
                    reset_line: Some(grid_tools::ai_driver::Line::load_route(&root, &reverse)?),
                    length: length(&reverse)?,
                },
                length(&route)?,
            ));
            println!("drive_touge version=1 event=setup runs=2 run2=subroute_1 slots=swapped");
        }
        // Set-up for headless replays of this log (grid-soak replay-log).
        println!(
            "drive_replay version=1 route={} opponents={} grid_position={}{}",
            route.path(),
            opponents.as_ref().map_or(0, |o| o.drivers.len()),
            player_slot + 1,
            // Only a non-default field is named, so default logs keep their form.
            match (&opponent_cars, player_car.is_default()) {
                (None, true) => String::new(),
                (list, _) => format!(
                    " car={player_car} opponent_cars={}",
                    list.as_ref().map_or(player_car.to_string(), |l| l
                        .iter()
                        .map(CarModel::stem)
                        .collect::<Vec<_>>()
                        .join(","))
                ),
            }
        );
        Some(Driving {
            runtime,
            geometry,
            paused: false,
            failure: None,
            notices: Vec::new(),
            scripted_ticks: drive_ticks,
            scripted,
            handbrake_from,
            last_controls: None,
            segment: 0,
            started: Instant::now(),
            opponents,
            // A derby disqualifies a flipped player instead (0x60D590).
            auto_reset: grid_tools::source_vehicle::player_reset::PlayerReset {
                demolition,
                ..Default::default()
            },
            // The Yokohama arenas have no AI line: no auto-reset there.
            reset_line: if route.ignores_progress() {
                grid_tools::ai_driver::Line::load_route(&root, &route).ok()
            } else {
                Some(grid_tools::ai_driver::Line::load_route(&root, &route)?)
            },
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
            if !parent.starts_with(settings::data_dir().canonicalize()?) {
                return Err("game-derived renders must stay under the data dir (local/)".into());
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
        return Err("start view requires a track scene".into());
    }
    let mut car = if collision {
        load_collision(&root, &route)?
    } else {
        load(&root, &route, &player_car, &lod, track, include_batches)?
    };
    // Distinct car models of the field (0 = the player's) and each car's model.
    let mut field = Field {
        models: vec![player_car.clone()],
        car_model: vec![0],
    };
    if let Some(opponents) = driving.as_ref().and_then(|d| d.opponents.as_ref()) {
        for runtime in &opponents.multi.cars[1..] {
            let m = match field.models.iter().position(|c| c == runtime.car()) {
                Some(m) => m,
                None => {
                    field.models.push(runtime.car().clone());
                    field.models.len() - 1
                }
            };
            field.car_model.push(m);
        }
    }
    if track && !collision && graphics.enhanced() && graphics.source_land {
        append_land(&root, &route, &player_car, &mut car)?;
    }
    if track && !collision && graphics.enhanced() && graphics.source_trees {
        source_trees::append(&root, &route, &mut car, graphics.foliage_coverage, graphics.tree_sheet_colours)?;
    }
    let anchor = if let Some(driving) = &driving {
        Some(Mat4::from_cols_array(&driving.runtime.pose()))
    } else if reference || start_view {
        Some(grid_slot(&root, &route, "slot_00")?)
    } else {
        None
    };
    if reference {
        let model = if let Some(driving) = &driving {
            model_offset(&driving.runtime)
        } else {
            anchor.unwrap()
        };
        append_car(&root, &lod, &mut car, model, &player_car, 0)?;
        if let Some(opponents) = driving.as_ref().and_then(|d| d.opponents.as_ref()) {
            // Other models are placed by the first opponent that drives them.
            for (m, model) in field.models.iter().enumerate().skip(1) {
                let k = field.car_model.iter().position(|&c| c == m).unwrap();
                append_car(
                    &root,
                    &lod,
                    &mut car,
                    model_offset(&opponents.multi.cars[k]),
                    model,
                    m,
                )?;
            }
            // Each opponent wears its own shipped livery (01, 02, ...); the player keeps 00.
            for k in 1..opponents.multi.cars.len() {
                let m = field.car_model[k];
                let model = &field.models[m];
                let count = livery_count(&root, model).max(1);
                let index = series_liveries
                    .get(k - 1)
                    .copied()
                    .flatten()
                    .filter(|&i| i < count)
                    .unwrap_or(k % count);
                car.liveries.push(load_livery(&root, model, m, index)?);
            }
            println!(
                "drive_opponent_liveries count={} models={}",
                car.liveries.len(),
                field
                    .models
                    .iter()
                    .map(CarModel::stem)
                    .collect::<Vec<_>>()
                    .join(",")
            );
        }
    }
    let texture_override_keys = if let Some(path) = texture_override_path {
        if !graphics.enhanced() {
            return Err("texture overrides require enhanced graphics".into());
        }
        let used = car
            .parts
            .iter()
            .filter(|p| {
                !p.matrices.is_empty()
                    && (!car.track
                        || p.material.starts_with("objects/")
                        || part_model(&p.material).is_some())
            })
            .filter_map(|p| car.bindings.get(&p.material).cloned())
            .collect();
        let applied = texture_overrides::apply(&path, &mut car.textures, &used)?;
        geometry_ray::describe_texture_usage(&car, &applied);
        Some(applied)
    } else { None };
    if graphics.source_race_numbers || graphics.player_race_number.is_some() {
        if !graphics.enhanced() { return Err("race-number pilot requires enhanced graphics".into()); }
        if graphics.source_race_numbers {
            car_numbers::apply_source(&root, &field, &mut car, graphics.player_race_number)?;
        } else if let Some(number) = graphics.player_race_number {
            car_numbers::apply(&root, &player_car, &mut car, number)?;
        }
    }
    #[cfg(feature = "paint-specular")]
    let body_specular = if graphics.source_body_specular {
        if !graphics.enhanced() { return Err("body specular pilot requires enhanced graphics".into()); }
        body_specular::prepare(&car, &field, graphics.white_body_specular)?
    } else { body_specular::Prepared::default() };
    let body_occlusion = if graphics.source_body_occlusion {
        if !graphics.enhanced() { return Err("body occlusion pilot requires enhanced graphics".into()); }
        body_occlusion::prepare(&car, &field)?
    } else { body_occlusion::Prepared::default() };
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
    if let Some(pixel) = geometry_pixel {
        let size = Vec2::new(graphics.width as f32, graphics.height as f32);
        if pixel.x >= size.x || pixel.y >= size.y {
            return Err("geometry pixel exceeds resolution".into());
        }
        geometry_ray::audit(&car, camera_transform(&orbit), size, pixel);
        return Ok(());
    }
    if (render_fixed_step || render_fps.is_some() || capture_orbit_rate.is_some() || motion_sequence.is_some())
        && (!drive || output.is_none())
    {
        return Err("fixed-step/motion-orbit diagnostics require --drive and --render".into());
    }
    if graphics.capture_timeout_seconds.is_some() && output.is_none() {
        return Err("--capture-timeout requires --render".into());
    }
    if render_fixed_step && render_fps.is_some() {
        return Err("choose --render-fixed-step or --render-fps, not both".into());
    }
    let mut app = App::new();
    app.insert_resource(start_sequence.clone());
    if let Some(keys) = texture_override_keys { app.insert_resource(texture_overrides::Active(keys)); }
    if let Some(rate) = capture_orbit_rate {
        app.insert_resource(free_look::CaptureOrbitRate(rate));
    }
    if let Some(laps) = trial_laps {
        // Races with opponents never write the solo time-trial best times.
        let race = driving.as_ref().is_some_and(|d| d.opponents.is_some());
        // The private best-time file is keyed to the default route and car.
        let mut trial = time_trial::Trial::new(
            laps,
            trial_mode,
            output.is_none() && !race && route.is_default() && player_car.is_default(),
        )?;
        trial.race = race;
        if race {
            // Races use the shipped start-light sequence time.
            trial.state = grid_tools::time_trial::State::new(
                laps,
                f64::from(grid_tools::source_vehicle::DELTA),
                start_sequence.seconds,
            )?;
        }
        if let Some(limit) = race_limit {
            // The player's event ends at the limit, not after its laps.
            trial.state = grid_tools::time_trial::State::new(
                99,
                f64::from(grid_tools::source_vehicle::DELTA),
                start_sequence.seconds,
            )?
            .with_time_limit(limit)?;
        }
        if freestyle {
            let seconds = profile.time_limit.unwrap_or(90.);
            trial.state = trial.state.clone().with_time_limit(seconds)?;
            println!("drive_freestyle time_limit={seconds} progress=ignored");
        }
        app.insert_resource(trial);
    }
    if let Some(path) = output {
        if let Some(config) = motion_sequence {
            motion_capture::install(
                &mut app,
                config,
                &path,
                render_fps,
                drive_ticks,
                (graphics.width, graphics.height),
            )?;
        }
        println!("capture_pipeline_compilation synchronous=true scope=offscreen-only");
        app.add_plugins(with_profile(
            DefaultPlugins
                // A capture's frame counter must not outrun background shader
                // compilation. Interactive windows keep Bevy's async default.
                .set(bevy::render::RenderPlugin {
                    synchronous_pipeline_compilation: true,
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: None,
                    exit_condition: ExitCondition::DontExit,
                    ..default()
                })
                .disable::<WinitPlugin>()
                .disable::<bevy::audio::AudioPlugin>(),
        ))
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
        if render_fixed_step {
            app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                Duration::from_secs_f64(f64::from(grid_tools::source_vehicle::DELTA)),
            ));
            println!(
                "capture_fixed_step=true dt={} scope=offscreen-only",
                grid_tools::source_vehicle::DELTA
            );
        }
        if let Some(fps) = render_fps {
            app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                Duration::from_secs_f64(1. / f64::from(fps)),
            ));
            println!("capture_render_clock virtual_fps={fps} measured_display_fps=false");
        }
    } else {
        app.add_plugins(with_profile(
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: if drive {
                        "GRID independent driving prototype"
                    } else {
                        "GRID independent static resource viewer"
                    }
                    .into(),
                    resolution: (graphics.width, graphics.height).into(),
                    // [video] present_mode / frame_latency (default Bevy's Fifo, wgpu's 2).
                    present_mode: profile.present_mode.unwrap_or_default(),
                    desired_maximum_frame_latency: profile.frame_latency,
                    ..default()
                }),
                ..default()
            }),
        ));
        if !drive {
            app.add_systems(Update, update_orbit);
        }
        // [video] render_height: the 3D view renders at most that many rows
        // (Android default 1080) and is stretched over the window.
        render_scale::install(
            &mut app,
            profile
                .render_height
                .unwrap_or_else(render_scale::RenderHeight::platform_default),
        );
        // Window size from the native surface, optional refresh-rate hint.
        #[cfg(target_os = "android")]
        android_display::install(&mut app, profile.refresh_hz.flatten());
    }
    if let Some(mut driving) = driving {
        // Diagnostics prop_script=TEMPLATE:INDEX with --drive-ticks: authored
        // validation drive into a breakable route prop.
        if driving.scripted_ticks.is_some()
            && let Some(target) = crate::settings::diagnostics().prop_script.clone()
        {
            let catalog = driving
                .runtime
                .dynamic_props()
                .ok_or("prop_script needs --body-contacts")?
                .catalog
                .clone();
            let pursuit = grid_tools::dynamic_props::PropPursuit::new(
                &root,
                &route,
                &catalog,
                &target,
                crate::settings::diagnostics()
                    .prop_script_speed
                    .unwrap_or(15.),
            )?;
            println!("drive_prop_script target={target} goal={:?}", pursuit.goal);
            app.insert_resource(dynamic_props::PropScript(pursuit));
        }
        // Offscreen captures never open an audio device or play sound.
        if app.is_plugin_added::<bevy::audio::AudioPlugin>() {
            // The original group mix (docs/AUDIO_RESEARCH.md); without it the
            // mixers keep the Normal/Chase levels.
            let mix = match grid_tools::audio_mix::Table::load(&root) {
                Ok(table) => Some(std::sync::Arc::new(table)),
                Err(error) => {
                    warn!("Audio group mix unavailable; Chase levels used: {error}");
                    None
                }
            };
            match field
                .models
                .iter()
                .map(|m| grid_tools::engine_audio_continuous::Bank::load(&root, m))
                .collect::<Result<Vec<_>, _>>()
            {
                Ok(banks) => engine_sound::install(&mut app, banks, mix.clone()),
                Err(error) => warn!("Engine audio unavailable; driving continues: {error}"),
            }
            match grid_tools::hud_audio::Bank::load(&root) {
                Ok(bank) => hud_sound::install(&mut app, bank, mix.clone()),
                Err(error) => warn!("HUD audio unavailable; driving continues: {error}"),
            }
            match grid_tools::tyre_audio::Bank::load(&root) {
                Ok(bank) => tyre_sound::install(&mut app, bank, mix),
                Err(error) => warn!("Tyre audio unavailable; driving continues: {error}"),
            }
        }
        tyre_smoke::install(&mut app, &root);
        if ground_effect_assets::enabled() {
            wheel_ground::install(&mut app);
            if let Err(error) = skid_marks::install(&mut app, &root) {
                warn!("Skid marks unavailable; driving continues: {error}");
            }
            if let Err(error) = wheel_debris::install(&mut app, &root) {
                warn!("Wheel debris unavailable; driving continues: {error}");
            }
        }
        if let Some(actions) = drive_input.take() {
            // The original's pause menu (docs/PAUSE_MENU.md): Race Day free
            // race for timed events, test drive for free practice.
            let menu = pause_menu::PauseMenu::new(
                if trial_laps.is_some() {
                    pause_menu::Variant::FreeRace
                } else {
                    pause_menu::Variant::TestDrive
                },
                input_settings.device == input::Device::Wheel,
                driving.reset_line.is_some(),
                pause_menu::initial_options(&profile, Some(&actions)),
            );
            let variant = menu.variant;
            input::install(&mut app, actions);
            #[cfg(target_os = "android")]
            android_pad::install(&mut app);
            if let Some(tilt) = drive_touch {
                // device=auto: a connected pad hides the touch zones and tilt.
                touch_controls::install(
                    &mut app,
                    tilt,
                    input_settings.device == input::Device::Auto,
                );
            }
            pause_menu::install(&mut app, menu, pause_menu::Labels::load(&root));
            perf_test::install(&mut app);
            // The Race Options front end with Driver Aids before the race
            // (docs/DRIVER_AIDS.md); not for scripted runs.
            if profile.front_end != Some(false)
                && driving.scripted_ticks.is_none()
                && !settings::diagnostics().autopilot
            {
                driving.paused = true;
                front_end::install(&mut app, driving.runtime.assists, front_end::Video::from_settings(&profile));
            }
            // The original HUD and post-race screens (docs/RACE_HUD.md): the
            // OSD scene from the install when it loads, else plain panels.
            if let Err(error) = osd_scene::install(&mut app, &root) {
                warn!("Original OSD unavailable, plain HUD panels used: {error}");
            }
            race_hud::install(&mut app);
            // Frame-rate counter ([osd] fps; on by default on Android, F9 on the desktop).
            fps_counter::install(
                &mut app,
                profile.osd_fps.unwrap_or(cfg!(target_os = "android")),
            );
            post_race::install(&mut app, variant);
        }
        // Drift Battle on top of the race, Drift GP on top of the solo run
        // (docs/DISCIPLINES.md).
        if let Some(format) = drift_format {
            let cars = driving.opponents.as_ref().map_or(1, |o| o.multi.cars.len());
            // The virtual race's driver skill: the player 1.0 (0x62B170 without a
            // driver row), opponent k its characteristics row's virtual_performance.
            let characteristics = match &ai_characteristics {
                Some(path) => grid_tools::ai_driver::read_characteristics(&std::fs::read_to_string(path)?)?,
                None => Vec::new(),
            };
            let performances: Vec<f32> = (0..12)
                .map(|k| {
                    characteristics
                        .get(k % characteristics.len().max(1))
                        .filter(|_| k > 0)
                        .and_then(|r| r.get("virtual_performance").copied())
                        .unwrap_or(1.)
                })
                .collect();
            drift_event::install(
                &mut app,
                &root,
                &route,
                (format, cars, trial_laps.unwrap_or(1)),
                ai_settings.difficulty,
                &performances,
            )?;
            if let Some(mut post) = app.world_mut().get_resource_mut::<post_race::PostRace>() {
                post.drift = true;
            }
        }
        if !car_classes.is_empty() {
            app.insert_resource(post_race::CarClasses(std::mem::take(&mut car_classes)));
        }
        if demolition {
            let cars = driving.opponents.as_ref().map_or(1, |o| o.multi.cars.len());
            let settings = grid_tools::demolition::Settings::load(&root)?;
            println!("drive_demolition version=1 event=setup cars={cars} settings={settings:?}");
            app.insert_resource(demolition_event::DemolitionEvent::new(settings, cars))
                .add_systems(FixedUpdate, demolition_event::tick.after(drive_tick));
        }
        // Le Mans: the time-limited race, its class results and the time of
        // day on the route that has one (docs/LE_MANS.md).
        let mode = grid_tools::time_of_day::Mode::for_route(&root, &route)
            .unwrap_or(grid_tools::time_of_day::Mode::None);
        if race_limit.is_some() || mode == grid_tools::time_of_day::Mode::Timeline {
            let schema = std::fs::read(root.join("database/schema.bin"))?;
            let data = std::fs::read(root.join("database/database.bin"))?;
            let db = grid_formats::database::read(&schema, &data)?;
            let mut cars = vec![driving.runtime.car().clone()];
            if let Some(o) = &driving.opponents {
                cars.extend(o.multi.cars.iter().skip(1).map(|c| c.car().clone()));
            }
            let classes = cars
                .iter()
                .map(|c| grid_tools::series::lemans_class(&db, c))
                .collect::<Result<Vec<_>, _>>()?;
            let timeline = grid_tools::time_of_day::Timeline::load(&root, &route)?;
            println!(
                "drive_time_of_day version=1 event=setup mode={mode:?} curves={} classes={}",
                timeline.as_ref().map_or(0, |t| t.curves.len()),
                classes.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(",")
            );
            app.insert_resource(le_mans_event::LeMansEvent::new(race_limit, mode, classes, timeline))
                .add_systems(Startup, le_mans_event::spawn_hud)
                .add_systems(Update, le_mans_event::show_hud)
                .add_systems(FixedUpdate, le_mans_event::tick.after(drive_tick));
        }
        if let Some((next, length)) = touge_run.take() {
            app.insert_resource(touge_event::TougeEvent::new(
                grid_tools::touge::Settings::load(&root)?,
                length,
                next,
            ))
            .add_systems(FixedUpdate, touge_event::tick.after(drive_tick));
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
                (
                    drive_actions,
                    free_look::update,
                    drive_present,
                    dynamic_props::tag,
                    dynamic_props::present,
                )
                    .chain(),
            )
            .add_systems(Update, update_start_lights);
        damage_parts::install(&mut app, &root, &field.models, &field.car_model);
        camera::install(&mut app, &root, field.models[0].stem());
        track_environment::install(&mut app, &root, &route);
        damage_hud::install(&mut app);
        if app.world().contains_resource::<input::Actions>() {
            replay::install(&mut app, &root, &route);
            flashback::install(&mut app, ai_settings.difficulty);
        }
    }
    if track && graphics.source_trees {
        source_trees::install(&mut app);
    }
    if track
        && !collision
        && ground_effect_assets::enabled()
        && let Err(error) = ground_detail::install(&mut app, &root, &route)
    {
        warn!("Ground cover unavailable; continuing: {error}");
    }
    if graphics.source_sky_tone && (!graphics.enhanced() || !graphics.source_sky || !track) {
        return Err("--sky-tone source requires enhanced source-sky track rendering".into());
    }
    if graphics.source_ibl && (!graphics.enhanced() || !graphics.source_sky || !track) {
        return Err(
            "--ibl source requires enhanced graphics and a source-sky Donington scene".into(),
        );
    }
    if graphics.enhanced() && graphics.source_sky && track {
        sky::install(&mut app, &root, graphics.source_ibl, graphics.source_sky_tone)?;
    }
    if let Some(frames) = graphics.benchmark_frames {
        graphics_benchmark::install(&mut app, frames);
    }
    perf_timing::install(&mut app);
    #[cfg(feature = "profile")]
    span_profile::install(&mut app);
    mesh_stream::install(&mut app);
    if graphics.track_grade {
        track_grade::install(&mut app);
    }
    if matches!(graphics.reflection_mode, 1 | 2) {
        if !graphics.enhanced() {
            return Err("deferred/SSR reflections require enhanced graphics".into());
        }
        app.insert_resource(bevy::pbr::DefaultOpaqueRendererMethod::deferred());
    }
    if graphics.enhanced() && graphics.layered_track && track && !collision {
        track_surface::install(&mut app, &root, graphics.source_land)?;
    }
    if graphics.reflection_mode == 3 {
        if !graphics.enhanced()
            || !graphics.source_ibl
            || !track
            || collision
            || !(reference || drive)
        {
            return Err("--reflections probe requires enhanced source IBL and a Donington reference/drive scene".into());
        }
        scene_probe::install(&mut app, &root)?;
    }
    if graphics.moving_probe && graphics.reflection_mode != 3 {
        return Err("--probe-update moving requires --reflections probe".into());
    }
    if reflection_calibration {
        if !track || collision || !(reference || drive) {
            return Err("reflection calibration requires Donington reference/drive".into());
        }
        scene_probe::install_calibration(&mut app, &root)?;
    }
    if graphics.enhanced() && graphics.clearcoat_prepass_fix {
        clearcoat_prepass::install(&mut app);
    }
    if graphics.enhanced() && graphics.wheel_shutter_seconds > 0. {
        sidewall_filter::install(&mut app);
    }
    if graphics.enhanced() && drive && graphics.wheel_geometry_shutter_seconds > 0. {
        wheel_exposure::install(&mut app);
    }
    if graphics.enhanced() {
        app.add_systems(Last, graphics_texture_state);
    }
    #[cfg(feature = "paint-specular")]
    app.insert_resource(body_specular);
    if from_menu {
        app.insert_resource(event_menu::FromMenu);
    }
    if let Some(path) = career_result {
        if !drive {
            return Err("--career-result requires a race".into());
        }
        app.insert_resource(career_screen::ResultPath(path))
            .init_resource::<career_screen::TopSpeed>()
            .add_systems(Update, career_screen::track_top_speed)
            .add_systems(Last, career_screen::write_result);
    }
    let exit = app
        .insert_resource(body_occlusion)
        .insert_resource(field)
        .insert_resource(graphics)
        .insert_resource(car)
        .insert_resource(InitialOrbit(orbit.clone()))
        .insert_resource(orbit)
        .insert_resource(ClearColor(Color::srgb(0.055, 0.07, 0.095)))
        .add_systems(PreStartup, texture_upload::configure)
        .add_systems(Startup, setup)
        .run();
    // Leaving an event started from the menu (event_menu::leave).
    if exit == AppExit::from_code(event_menu::BACK_TO_MENU as u8) {
        std::process::exit(event_menu::BACK_TO_MENU);
    }
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
fn graphics_texture_state(images: Res<Assets<Image>>, mut emitted: Local<bool>) {
    if *emitted {
        return;
    }
    *emitted = true;
    let bc = images
        .iter()
        .filter(|(_, image)| {
            matches!(
                image.texture_descriptor.format,
                TextureFormat::Bc1RgbaUnorm
                    | TextureFormat::Bc1RgbaUnormSrgb
                    | TextureFormat::Bc3RgbaUnorm
                    | TextureFormat::Bc3RgbaUnormSrgb
            )
        })
        .count();
    println!(
        "graphics_texture_state poststartup=true bc1_bc3_assets={bc} {}",
        texture_upload::summary()
    );
}

#[derive(bevy::ecs::system::SystemParam)]
struct PaintMaskResources<'w> {
    occlusion: ResMut<'w, body_occlusion::Prepared>,
    #[cfg(feature = "paint-specular")]
    specular: ResMut<'w, body_specular::Prepared>,
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
    field: Res<Field>,
    graphics: Res<graphics::Settings>,
    mut scattering_media: ResMut<Assets<bevy::light::atmosphere::ScatteringMedium>>,
    overrides: Option<Res<texture_overrides::Active>>,
    start: Option<Res<StartSequence>>,
    mut paint_masks: PaintMaskResources,
) {
    println!(
        "renderer adapter={:?} backend={:?} driver={:?} driver_info={:?} features={:?}",
        adapter.name,
        adapter.backend,
        adapter.driver,
        adapter.driver_info,
        device.features()
    );
    let body_occlusion_handles = body_occlusion::upload(&mut paint_masks.occlusion, &mut images, graphics.sampler(false));
    #[cfg(feature = "paint-specular")]
    let body_specular_handles = body_specular::upload(&mut paint_masks.specular, &mut images, graphics.sampler(false));
    let mut occluded_liveries = BTreeMap::new();
    let mut texture_handles = BTreeMap::new();
    for (name, dds) in &car.textures {
        let image = texture_upload::image(
            dds,
            graphics.sampler(car.track),
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        )
        .expect("validated DDS image");
        if overrides.as_ref().is_some_and(|o| o.0.contains(name)) {
            println!("texture_override_image key={name:?} format={:?} size={:?} mip_levels={} scope=after-device-selection-shared-helper",image.texture_descriptor.format,image.texture_descriptor.size,image.texture_descriptor.mip_level_count);
        }
        texture_handles.insert(name.clone(), images.add(image));
    }
    let mut material_handles = BTreeMap::new();
    let tread_ready = car
        .parts
        .iter()
        .filter(|p| {
            p.material
                .strip_prefix("car0/")
                .unwrap_or(&p.material)
                .trim()
                == "Tyre Tread"
        })
        .all(|p| p.mesh.attribute(Mesh::ATTRIBUTE_TANGENT).is_some());
    let tread_key = if car.track {
        "car0/lac_tread_tm_nm.tga"
    } else {
        "lac_tread_tm_nm.tga"
    };
    let tread_normal = if graphics.enhanced() && graphics.tread_normals && tread_ready {
        car.textures.get(tread_key).map(|dds| {
            let image=texture_upload::image_linear(dds,graphics.sampler(false),RenderAssetUsages::default()).expect("validated linear tread normal DDS");
            println!("tread_normal active=true srgb=false format={:?} frame=orthonormalized-source approximate=true",image.texture_descriptor.format);
            images.add(image)
        })
    } else {
        None
    };
    let liveries = std::mem::take(&mut car.liveries);
    let mut livery_images: BTreeMap<(usize, String), Handle<Image>> = BTreeMap::new();
    let mut livery_materials: BTreeMap<(usize, String), Handle<StandardMaterial>> = BTreeMap::new();
    let bindings = car.bindings.clone();
    let track = car.track;
    let start = start.map(|s| s.clone()).unwrap_or_default();
    let start_light_images: Vec<Option<Handle<Image>>> = start
        .textures
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
                    .strip_prefix("car0/")
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
                let name = match part_model(&part.material) {
                    Some(_) => part.material.split_once('/').unwrap().1,
                    None => &part.material,
                }
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
                    alpha_mode: if name.starts_with("objects/") || name.starts_with("trees/") {
                        AlphaMode::Mask(0.5)
                    } else if glass {
                        AlphaMode::Blend
                    } else {
                        AlphaMode::Opaque
                    },
                    unlit: name.starts_with("collision/"),
                    double_sided: glass || name.starts_with("collision/") || name.starts_with("trees/"),
                    cull_mode: if glass || name.starts_with("collision/") || name.starts_with("trees/") {
                        None
                    } else {
                        Some(Face::Back)
                    },
                    ..default()
                };
                if name == "Car Body Paint" && (!track || part_model(&part.material) == Some(0)) {
                    material.occlusion_texture = body_occlusion_handles.get(&0).cloned();
                    #[cfg(feature = "paint-specular")]
                    body_specular::apply(&mut material, body_specular_handles.get(&0));
                }
                source_trees::configure_material(name, &mut material);
                if graphics.enhanced()
                    && graphics.layered_car
                    && (!track || part.material.starts_with("car0/"))
                {
                    car_materials::apply(name, &mut material);
                    if name == "Window Glass" {
                        material.perceptual_roughness = graphics.window_roughness;
                        material.reflectance = graphics.window_reflectance;
                    }
                }
                if name == "Tyre Tread" && (!track || part.material.starts_with("car0/")) {
                    material.normal_map_texture = tread_normal.clone();
                }
                materials.add(material)
            })
            .clone();
        let model = part_model(&part.material);
        // Deforming body parts own their mesh per car (any car of the model).
        let deformable = driving.as_ref().is_some_and(|d| {
            d.runtime.deformation_enabled
                || d.opponents
                    .as_ref()
                    .is_some_and(|o| o.multi.cars.iter().any(|c| c.deformation_enabled))
        }) && model.is_some()
            && part.wheel.is_none()
            && part.material.split_once('/').map(|x| x.1) != Some("Aerials");
        // The player's model is drawn on the player; other models only on
        // the opponents that drive them.
        let player_part = model.is_none_or(|m| m == 0);
        let original = deformable.then(|| part.mesh.clone());
        let facing = source_trees::FacingSource::from_mesh(&part.name, &part.mesh);
        let mut initial_bounds = part
            .mesh
            .compute_aabb()
            .expect("validated source mesh bounds");
        if let Some(source) = &facing {
            initial_bounds.half_extents += source.radius();
        }
        let shared = if deformable {
            None
        } else {
            Some(meshes.add(part.mesh))
        };
        for matrix in part.matrices {
            let mesh = shared
                .clone()
                .unwrap_or_else(|| meshes.add(original.as_ref().unwrap().clone()));
            let mesh = if facing.is_some() {
                let own = meshes.get(&mesh).expect("source foliage mesh").clone();
                meshes.add(own)
            } else {
                mesh
            };
            let moving = driving.is_some() && model.is_some();
            let transform = if moving {
                let runtime = &driving.as_ref().unwrap().runtime;
                let motion = part
                    .wheel
                    .map_or(Mat4::IDENTITY, |i| wheel_motion(runtime.wheels()[i]));
                Mat4::from_cols_array(&runtime.pose()) * motion * matrix
            } else {
                matrix
            };
            let player_deforms = driving
                .as_ref()
                .is_some_and(|d| d.runtime.deformation_enabled);
            let mut entity = if player_part {
                Some(commands.spawn((
                    Name::new(part.name.clone()),
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material.clone()),
                    Transform::from_matrix(transform),
                    initial_bounds,
                )))
            } else {
                None
            };
            if let Some(source) = &facing
                && let Some(entity) = &mut entity
            {
                entity.insert(source.placed(matrix));
            }
            if track && let Some(entity) = &mut entity {
                entity.insert(track_surface::SourcePart(part.material.clone()));
            }
            if moving && let Some(entity) = &mut entity {
                entity.insert((
                    MovingPart {
                        rest: matrix,
                        wheel: part.wheel,
                        lost_with_wheel: lost_with_wheel(&part.joint),
                    },
                    damage_parts::Joint::new(&part.joint),
                ));
                if part.material == "car0/Tyre Wall"
                    && let Some(wheel) = part.wheel
                {
                    entity.insert(sidewall_filter::Sidewall { car: 0, wheel });
                }
                if let Some(source) = original.as_ref().filter(|_| player_deforms) {
                    let model = model_offset(&driving.as_ref().unwrap().runtime);
                    entity.insert(DeformingPart {
                        source: source.clone(),
                        raw_bind: model.inverse() * matrix,
                    });
                }
            }
            if moving && let Some(opponents) = driving.as_ref().and_then(|d| d.opponents.as_ref()) {
                for car in 1..opponents.multi.cars.len() {
                    if model != Some(field.car_model[car]) {
                        continue;
                    }
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
                                            texture_upload::image(
                                                dds,
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
                    let opponent_material = if (graphics.source_body_occlusion || graphics.source_body_specular)
                        && part.material.rsplit('/').next() == Some("Car Body Paint")
                    {
                        occluded_liveries.entry((car, part.material.clone())).or_insert_with(|| {
                            let mut own = materials.get(&opponent_material).expect("opponent material").clone();
                            own.occlusion_texture = body_occlusion_handles.get(&car).cloned();
                            #[cfg(feature = "paint-specular")]
                            body_specular::apply(&mut own, body_specular_handles.get(&car));
                            materials.add(own)
                        }).clone()
                    } else { opponent_material };
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
                            lost_with_wheel: lost_with_wheel(&part.joint),
                        },
                        damage_parts::Joint::new(&part.joint),
                    ));
                    if part.material == "car0/Tyre Wall"
                        && let Some(wheel) = part.wheel
                    {
                        opponent.insert(sidewall_filter::Sidewall { car, wheel });
                    }
                    if let Some(source) = own {
                        let model = model_offset(runtime);
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
        println!(
            "start_lights enabled=true stages={} sequence_seconds={}",
            start.textures.len() - 1,
            start.seconds
        );
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
            color: graphics.sun_colour(),
            shadow_maps_enabled: true,
            ..default()
        },
        graphics.sun_transform(),
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
            Text::new(format!(
                "{} / {}\nPreparing practice timer",
                driving.as_ref().map_or("", |d| d.runtime.car().label()),
                driving
                    .as_ref()
                    .map_or_else(String::new, |d| track_label(d.runtime.route()))
            )),
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
// Separate ECS borrows for the bounded offscreen capture system.
#[allow(clippy::too_many_arguments)]
fn capture(
    mut commands: Commands,
    mut capture: ResMut<Capture>,
    mut exit: MessageWriter<AppExit>,
    driving: Option<Res<Driving>>,
    benchmark: Option<Res<graphics_benchmark::Benchmark>>,
    probe: Option<Res<scene_probe::Cube>>,
    graphics: Res<graphics::Settings>,
    motion: Option<Res<motion_capture::Motion>>,
    replay: Option<Res<replay::Replay>>,
) {
    capture.frames += 1;
    // Diagnostics replay_capture: the replay plays first.
    if replay.is_some_and(|r| r.capture_pending()) {
        return;
    }
    // Scripted drives tick in real time; allow their length plus headroom.
    let limit = driving
        .as_ref()
        .and_then(|d| d.scripted_ticks)
        .map_or(120., |t| {
            (t as f64 * f64::from(grid_tools::source_vehicle::DELTA) * 1.5 + 30.).max(120.)
        });
    let limit = if graphics.reflection_mode == 3 {
        limit.max(300.)
    } else {
        limit
    };
    let limit = graphics.capture_timeout_seconds.map_or(limit, |extra| limit.max(extra));
    if capture.frames == 1
        && (graphics.reflection_mode != 3 || probe.as_ref().is_some_and(|p| p.ready()))
    {
        println!("render_capture_budget seconds={limit} explicit_extension={:?} scope=offscreen-only", graphics.capture_timeout_seconds);
    }
    if capture.started.elapsed() > Duration::from_secs_f64(limit) {
        error!("render capture timed out");
        exit.write(AppExit::error());
        return;
    }
    if graphics.reflection_mode == 3 && probe.as_ref().is_none_or(|p| !p.ready()) {
        // Keep the normal warm-up after activation as well, so TAA/shadows do
        // not retain the pre-probe state in the comparison image.
        capture.frames = 0;
        return;
    }
    if let Some(driving) = &driving {
        if driving.failure.is_some() {
            exit.write(AppExit::error());
            return;
        }
        // A finished race freezes before the scripted tick count.
        if benchmark.is_none()
            && driving
                .scripted_ticks
                .is_some_and(|n| driving.runtime.ticks < n)
            && !driving
                .opponents
                .as_ref()
                .is_some_and(|o| o.standings.over())
        {
            return;
        }
    }
    if benchmark.as_ref().is_some_and(|b| !b.finished) {
        return;
    }
    if capture.frames >= 180 && !capture.requested && motion.as_ref().is_none_or(|m| m.finished()) {
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
                            println!("{}", texture_upload::summary());
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
#[allow(clippy::too_many_arguments)]
fn drive_tick(
    mut driving: ResMut<Driving>,
    actions: Res<input::Actions>,
    mut shifts: ResMut<input::ShiftQueue>,
    mut trial: Option<ResMut<time_trial::Trial>>,
    mut prop_script: Option<ResMut<dynamic_props::PropScript>>,
    start: Option<Res<StartSequence>>,
    // Last `drive_input` record (segment, controls, hold) for `grid-soak replay-log`.
    mut recorded: Local<Option<(u64, grid_tools::source_vehicle::Controls, bool)>>,
    // The player's inputs for Flashback (logged, or given back when ticks run again).
    mut inputs: Option<ResMut<flashback::Inputs>>,
) {
    let _tick_timer = grid_tools::perf::Timer::start(grid_tools::perf::Stage::DriveTick);
    // One queued gear edge per fixed tick, dropped when the tick does not run.
    let shift = shifts.pop();
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
    // Race over (every car finished or out, or ended with F): frozen result.
    if trial.is_some()
        && driving
            .opponents
            .as_ref()
            .is_some_and(|o| o.standings.over())
    {
        return;
    }
    // After the player's finish the opponents race on until every car has
    // finished [V: a single-player lap race ends when all cars complete the
    // laps, docs/RACE_RULES_RESEARCH.md].
    let cooldown = trial.as_ref().is_some_and(|t| t.finished()) && driving.opponents.is_some();
    let held = trial.as_ref().is_some_and(|t| !t.driving()) && !cooldown;
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
        && let Some(pilot) = match &mut opponents.autopilot {
            Some(pilot) => Some(pilot),
            None if cooldown => Some(opponents.after_finish.get_or_insert_with(|| {
                let mut pilot = opponents.drivers[0].clone();
                pilot.reset();
                pilot
            })),
            None => None,
        } {
        use grid_tools::ai_driver::{Other, View};
        let car = &driving.runtime;
        let others: Vec<Other> = opponents
            .multi
            .cars
            .iter()
            .enumerate()
            .skip(1)
            .map(|(i, c)| Other::of(i as u32 + 1, c))
            .collect();
        let view = View::of(car, opponents.multi.obstruction(0));
        pilot.race = grid_tools::ai_driver::RaceView {
            place: opponents.standings.position(0).saturating_sub(1) as u32,
            cars: opponents.standings.len() as u32,
            progress: opponents.standings.laps_covered(0) as f32,
        };
        Some(pilot.controls(
            &opponents.ai,
            &view,
            &others,
            grid_tools::source_vehicle::DELTA,
        ))
    } else {
        None
    };
    let mut applied = None;
    let mut controls = if let Some(controls) = autopilot {
        controls
    } else if driving.scripted_ticks.is_some()
        && let Some(script) = prop_script.as_mut()
    {
        let broken_at = driving
            .opponents
            .as_ref()
            .map_or(driving.runtime.dynamic_props(), |o| o.multi.dynamic_props())
            .and_then(|p| p.events.first())
            .map(|e| e.tick);
        script.0.controls(
            driving.runtime.pose(),
            driving.runtime.velocity(),
            broken_at,
            driving.runtime.ticks,
        )
    } else if driving.scripted_ticks.is_some() {
        grid_tools::source_vehicle::Controls {
            handbrake: driving
                .handbrake_from
                .is_some_and(|t| driving.runtime.ticks >= t),
            ..driving.scripted
        }
    } else {
        let (controls, shift) = flashback::player(
            inputs.as_deref_mut(),
            driving.runtime.ticks + 1,
            actions.controls(),
            shift,
        );
        // The request reaches the car only with a manual gearbox; it is
        // recorded for grid-soak replay-log.
        if driving.runtime.assists.transmission
            == grid_tools::source_vehicle::assists::Transmission::Manual
        {
            applied = shift;
        }
        assists::request_shift(shift, &mut driving.runtime);
        controls
    };
    if held {
        controls = grid_tools::source_vehicle::Controls {
            throttle: controls.throttle,
            handbrake: true,
            ..Default::default()
        };
    }
    // Player input record: every change, gear request and segment start.
    let now = (driving.segment, controls, held);
    if *recorded != Some(now) || applied.is_some() {
        log_line!(
            "drive_input version=1 segment={} tick={} throttle={} brake={} steering={} handbrake={} shift={} hold={}",
            driving.segment,
            driving.runtime.ticks + 1,
            controls.throttle,
            controls.brake,
            controls.steering,
            controls.handbrake,
            grid_tools::soak::shift_name(applied),
            u8::from(held)
        );
        *recorded = Some(now);
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
            .map(|(i, c)| Other::of(i as u32 + 1, c))
            .collect();
        // Start state while held [I]: gentle blips first, hard revving in the
        // second half of the countdown.
        let rev_state = match trial.as_ref().map(|t| t.state.phase()) {
            Some(grid_tools::time_trial::Phase::Countdown(left))
                if f64::from(*left) * f64::from(grid_tools::source_vehicle::DELTA)
                    > start.as_ref().map_or(START_SEQUENCE_SECONDS, |s| s.seconds) / 2. =>
            {
                2
            }
            _ => 3,
        };
        let ai_timer = grid_tools::perf::Timer::start(grid_tools::perf::Stage::AiDrivers);
        // Race manager (catch-up, choreographer) before the AI update.
        if !held {
            use grid_tools::race::Status;
            let racing: Vec<bool> = (0..opponents.standings.len())
                .map(|i| {
                    opponents.standings.status(i) == Status::Racing
                        && opponents.multi.retirement(i).is_none()
                })
                .collect();
            opponents.pacing.tick_with_player(
                &opponents.ai.line,
                &driving.runtime,
                &mut opponents.multi,
                &mut opponents.drivers,
                &racing,
                grid_tools::source_vehicle::DELTA,
            );
            for line in &opponents.pacing.log {
                log_line!(
                    "drive_ai_event version=1 segment={} tick={} {line}",
                    driving.segment,
                    driving.runtime.ticks + 1
                );
            }
        }
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
                // Race place for the drift test (read only in drift events).
                driver.race = grid_tools::ai_driver::RaceView {
                    place: opponents.standings.position(i + 1).saturating_sub(1) as u32,
                    cars: opponents.standings.len() as u32,
                    progress: opponents.standings.laps_covered(i + 1) as f32,
                };
                driver.controls(
                    &opponents.car_ai[i],
                    &view,
                    &others,
                    grid_tools::source_vehicle::DELTA,
                )
            });
        }
        drop(ai_timer);
        std::mem::swap(&mut driving.runtime, &mut opponents.multi.cars[0]);
        let result = opponents.multi.tick(&all);
        std::mem::swap(&mut driving.runtime, &mut opponents.multi.cars[0]);
        if result.is_ok() {
            // The player's own records: the report follows the player's
            // contacts, not every contact in the field.
            driving.runtime.last_body_contacts = opponents.multi.body_contacts_of(0);
            // A car whose own stage failed is held and out of the race; the
            // rest of the field races on (S3, docs/RUNTIME_LIMITS_AUDIT.md).
            for &i in opponents.multi.newly_retired() {
                let Some(retirement) = opponents.multi.retirement(i) else {
                    continue;
                };
                let name = opponents.names.get(i).map_or("car", String::as_str);
                log_line!(
                    "drive_car_retired version=1 segment={} tick={} car={i} name={name:?} error={:?}",
                    driving.segment,
                    retirement.tick,
                    retirement.error.0
                );
                opponents
                    .standings
                    .retire(i, grid_tools::race::Out::Retired);
                driving.notices.push(if i == 0 {
                    format!("Your car retired: {}. R restarts", retirement.error)
                } else {
                    format!("{name} retired: {}", retirement.error)
                });
            }
        }
        opponents.throttles = all[1..]
            .iter()
            .enumerate()
            .map(|(k, c)| {
                if opponents.multi.retirement(k + 1).is_some() {
                    0.
                } else {
                    c.throttle
                }
            })
            .collect();
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
        // Nearly still on the side or roof for 2 s: back onto the line. A
        // failed placement is logged and the car drives on where it is.
        if let Some(line) = &driving.reset_line {
            let player = &mut driving.runtime;
            let shared = driving.opponents.as_mut().map(|o| &mut o.multi);
            match driving.auto_reset.update(player, shared, line) {
                Ok(Some(reset)) => log_line!(
                    "drive_auto_reset version=1 segment={} tick={} point={:?} heading={:?}",
                    driving.segment,
                    driving.runtime.ticks,
                    reset.point,
                    reset.heading
                ),
                Ok(None) => {}
                Err(error) => log_line!(
                    "drive_auto_reset version=1 segment={} tick={} error={:?}",
                    driving.segment,
                    driving.runtime.ticks,
                    error.0
                ),
            }
        }
        // Trial bookkeeping never stops physics: a refused tick invalidates
        // the lap and the trial clock runs on.
        if !held
            && let Some(trial) = trial.as_mut()
            && trial.driving()
            && let Err(error) = trial.tick(driving.runtime.ticks, &driving.runtime.lap_events)
        {
            log_line!(
                "drive_trial_event version=1 type=refused tick={} error={:?}",
                driving.runtime.ticks,
                error.message
            );
            if !trial.invalidate(driving.runtime.ticks, &driving.runtime.lap_events)
                && let Some(lap) = &mut driving.runtime.lap
                && lap.elapsed().is_some()
            {
                lap.current_valid = false;
            }
        }
        if !held
            && let (Some(trial), Some(opponents)) = (trial.as_ref(), driving.opponents.as_mut())
        {
            let positions = opponents.positions(&driving.runtime);
            let time = trial.race_time(driving.runtime.ticks).unwrap_or(0.);
            opponents
                .standings
                .update(&opponents.ai.line, &positions, time, &[0]);
            if let grid_tools::time_trial::Phase::Finished(result) = trial.state.phase() {
                opponents.standings.finish(0, result.seconds);
            }
            // Terminal damage: the player's race is over (the original's
            // wrecked screen retires and ends the race [V]); opponents are
            // retired on it (AUTHORED).
            use grid_tools::race::{Out, Status};
            if driving.runtime.terminal_wheel().is_some()
                && opponents.standings.status(0) == Status::Racing
            {
                opponents.standings.retire(0, Out::Wrecked);
                let speeds = opponents.speeds(&driving.runtime);
                opponents.standings.finalize(&speeds, time, &[0]);
            }
            for i in 1..opponents.standings.len() {
                if opponents.multi.cars[i].terminal_wheel().is_some()
                    && opponents.standings.status(i) == Status::Racing
                {
                    opponents.standings.retire(i, Out::Wrecked);
                }
                // Stopped by the AI's failure/damage retire drive (AUTHORED).
                if opponents.drivers[i - 1].removal_requested()
                    && opponents.standings.status(i) == Status::Racing
                {
                    opponents.standings.retire(i, Out::Wrecked);
                    log_line!(
                        "drive_ai_event version=1 segment={} tick={} car={i} retired=failure",
                        driving.segment,
                        driving.runtime.ticks
                    );
                    driving
                        .notices
                        .push(format!("{} retired", opponents.names[i]));
                }
            }
            if opponents.standings.over() {
                for line in opponents.standings.classification(&opponents.names) {
                    log_line!("drive_race_result {line}");
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
                    log_line!(
                        "drive_lap_event version=1 segment={segment} tick={tick} type=start lap={lap} when={when:.9}"
                    )
                }
                Event::Checkpoint { gate, when } => log_line!(
                    "drive_lap_event version=1 segment={segment} tick={tick} type=checkpoint lap={lap} gate={gate} when={when:.9}"
                ),
                Event::Sector { sector, seconds } => log_line!(
                    "drive_lap_event version=1 segment={segment} tick={tick} type=sector lap={lap} sector={} seconds={seconds:.9}",
                    sector + 1
                ),
                Event::Finished(result) => log_line!(
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
                    log_line!(
                        "drive_lap_event version=1 segment={segment} tick={tick} type=position-jump lap={lap}"
                    )
                }
            }
        }
        // Bounded reports: every 16th tick, the player's control changes (not
        // the after-finish AI's continuous ones) and contacts while still
        // racing, and the end of a script.
        let player_out = driving
            .opponents
            .as_ref()
            .is_some_and(|o| o.multi.retirement(0).is_some());
        if (!cooldown && driving.last_controls != Some(controls))
            || driving.runtime.ticks.is_multiple_of(16)
            || (!player_out && driving.runtime.last_body_contacts > 0)
            || driving.scripted_ticks == Some(driving.runtime.ticks)
        {
            driving.report("tick", controls);
        }
        driving.last_controls = Some(controls);
    }
    if driving.failure.is_none() && driving.scripted_ticks == Some(driving.runtime.ticks) {
        log_line!(
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
    actions: Option<Res<input::Actions>>,
    mut exit: MessageWriter<AppExit>,
    mut trial: Option<ResMut<time_trial::Trial>>,
    replay: Option<Res<replay::Replay>>,
    front: Option<Res<front_end::FrontEnd>>,
) {
    // A replay owns the keys while it plays, the front end before the race.
    if replay.is_some_and(|r| r.owns_input()) || front.is_some_and(|f| f.open) {
        return;
    }
    if input.just_pressed(KeyCode::KeyR) {
        restart(&mut driving, trial.as_deref_mut());
    }
    // F after the player's finish: "Finish Race" ends the race now; cars
    // still racing are estimated or, below 5 mph, retired [V 0x90FCD0].
    if input.just_pressed(KeyCode::KeyF)
        && let Some(trial) = trial.as_ref()
        && trial.finished()
    {
        let driving = &mut *driving;
        if let Some(opponents) = &mut driving.opponents
            && !opponents.standings.over()
        {
            let speeds = opponents.speeds(&driving.runtime);
            let time = trial.race_time(driving.runtime.ticks).unwrap_or(0.);
            opponents.standings.finalize(&speeds, time, &[0]);
            for line in opponents.standings.classification(&opponents.names) {
                log_line!("drive_race_result {line}");
            }
        }
    }
    // Without the pause menu (no input layer) Escape still exits.
    if actions.is_none() && input.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
}
/// Restart Race / R: the whole event from its start (`StateRestartRace`):
/// every car, the shared world with its props, damage, the AI field and
/// race manager, timing and the countdown.
fn restart(driving: &mut Driving, trial: Option<&mut time_trial::Trial>) {
    driving.report("reset", driving.last_controls.unwrap_or_default());
    driving.runtime.reset();
    driving.auto_reset.reset();
    if let Some(opponents) = &mut driving.opponents {
        opponents.multi.reset();
        for driver in &mut opponents.drivers {
            driver.reset();
        }
        let positions = opponents.positions(&driving.runtime);
        opponents.standings.reset(&opponents.ai.line, &positions);
        opponents.pacing.reset(&opponents.ai.line, &positions);
        opponents.after_finish = None;
        if let Some(pilot) = &mut opponents.autopilot {
            pilot.reset();
        }
        opponents.throttles.iter_mut().for_each(|t| *t = 0.);
    }
    driving.segment += 1;
    driving.last_controls = None;
    driving.failure = None;
    driving.notices.clear();
    driving.paused = false;
    if let Some(trial) = trial {
        trial.reset();
    }
}
fn drive_exit_log(driving: Res<Driving>, mut exits: MessageReader<AppExit>) {
    if exits.read().next().is_some() {
        driving.report("exit", driving.last_controls.unwrap_or_default());
    }
}
#[allow(clippy::type_complexity, clippy::too_many_arguments)] // Explicit Bevy resource/component ownership.
fn drive_present(
    driving: Res<Driving>,
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
    mut camera: Single<(&mut Transform, Option<&mut Projection>), With<ViewerCamera>>,
    mut windows: Query<&mut Window>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut presentation: ResMut<DeformationPresentation>,
    mut hud: Query<&mut Text, With<DrivingHud>>,
    sound: Option<Res<engine_sound::EngineSound>>,
    trial: Option<Res<time_trial::Trial>>,
    actions: Option<Res<input::Actions>>,
    mut look_back: Local<Option<bool>>,
    (free_look, mut cameras): (
        Option<Res<free_look::FreeLook>>,
        Option<ResMut<camera::Cameras>>,
    ),
    mut refused: Local<Option<&'static str>>,
    render_clock: (
        Option<Res<Time<Fixed>>>,
        Option<Res<graphics::Settings>>,
        Option<Res<pause_menu::PauseMenu>>,
        Option<Res<race_hud::Active>>,
        Option<Res<drift_event::DriftEvent>>,
    ),
) {
    let (fixed, graphics, menu, race_hud, drift) = render_clock;
    // Pause menu Screen Setup (lap timer, race position, speed unit).
    let osd = menu
        .as_ref()
        .map_or_else(pause_menu::Options::default, |m| m.values);
    if driving.runtime.deformation_enabled && driving.failure.is_none() {
        let update = (|| -> Result<(), &'static str> {
            let field =
                grid_tools::body_deformation::Field::read(&driving.runtime.state.car.owner.vehicle)
                    .map_err(|e| e.0)?;
            let key = field.key();
            if presentation.0.as_ref() == Some(&key) {
                return Ok(());
            }
            // A refused field is not projected again until it changes.
            presentation.0 = Some(key);
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
            log_line!(
                "drive_body_projection event=update tick={} meshes={} changed_meshes={changed} policy=independent-current-minus-rest",
                driving.runtime.ticks,
                staged.len()
            );
            Ok(())
        })();
        // Presentation never stops physics: a refused field keeps the
        // previous mesh, as for the opponents, with one log line.
        match update {
            Ok(()) => *refused = None,
            Err(error) if *refused != Some(error) => {
                *refused = Some(error);
                log_line!(
                    "drive_body_projection event=refused tick={} error={error:?} policy=keep-previous-mesh",
                    driving.runtime.ticks
                );
            }
            Err(_) => {}
        }
    }
    let pose = Mat4::from_cols_array(&driving.runtime.pose());
    let remainder = if graphics.is_some_and(|g| g.enhanced() && g.smooth_wheel_motion)
        && !driving.paused
        && driving.failure.is_none()
        && !driving
            .scripted_ticks
            .is_some_and(|n| driving.runtime.ticks >= n)
    {
        fixed.map_or(0., |t| t.overstep().as_secs_f32())
    } else {
        0.
    };
    let wheels = wheel_presentation::wheels(&driving.runtime, remainder).map(wheel_motion);
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
                    wheel_presentation::wheels(car, remainder).map(wheel_motion),
                )
            })
            .collect();
        // Opponent body dents: re-project a car's parts when its field changes.
        let opponent_keys = &mut presentation.1;
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
    // Change View (C, pad LB) cycles the car's views; Look Back (End, pad Y)
    // holds the view's reverse camera; diagnostics look_back forces it.
    let back = actions.as_ref().is_some_and(|a| a.down(input::Action::LookBack))
        || *look_back.get_or_insert_with(|| crate::settings::diagnostics().look_back);
    let mut fallback = None;
    let cameras = match cameras.as_deref_mut() {
        Some(c) => c,
        None => fallback.insert(camera::Cameras::new(camera::Views::fallback(), "", None)),
    };
    if actions.as_ref().is_some_and(|a| a.pressed(input::Action::ChangeView)) {
        cameras.change_view();
        log_line!("drive_camera event=change view={} tick={}", cameras.ident(), driving.runtime.ticks);
    }
    let sample = camera::Sample::of(&driving.runtime, driving.segment);
    // Right mouse button orbits a chase view (AUTHORED free look).
    let side = if back { -1. } else { 1. };
    let shot = cameras.frame(&sample, back, |o| free_look::offset(free_look.as_deref(), o, side));
    let (transform, projection) = &mut *camera;
    **transform = Transform::from_translation(shot.eye).with_rotation(shot.rotation);
    if let Some(Projection::Perspective(p)) = projection.as_deref_mut() {
        p.fov = shot.fov;
        p.near = shot.near.unwrap_or(PerspectiveProjection::default().near);
    }
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
    let status = if let Some(failure) = &driving.failure {
        format!("Simulation paused: {failure}")
    } else if driving.paused {
        String::new()
    } else {
        terminal
            .as_deref()
            .unwrap_or(
                "Space: handbrake | C: view | End: look back | RMB: look around | Esc/P: pause | R: restart",
            )
            .into()
    };
    // The latest retirements after a car's own tick stage failed.
    let skip = driving.notices.len().saturating_sub(3);
    let status: String = driving.notices[skip..]
        .iter()
        .map(|n| format!("{n}\n"))
        .chain(std::iter::once(status))
        .collect();
    let audio = sound.map_or("", |s| {
        if s.muted {
            "\nSound muted - M enables"
        } else {
            "\nM: mute engine sound"
        }
    });
    let heading = format!(
        "{} / {} {}",
        driving.runtime.car().label(),
        track_label(driving.runtime.route()),
        if let Some(d) = &drift {
            match d.format {
                grid_tools::virtual_race::Format::DriftGrandPrix => "drift gp",
                _ => "drift battle",
            }
        } else if trial.as_ref().is_some_and(|t| t.race) {
            "race"
        } else if trial.is_some() {
            "time trial"
        } else {
            "practice"
        }
    );
    // Race position or classification above the event HUD.
    let race = match (&driving.opponents, &trial) {
        (Some(o), Some(t)) if t.finished() || o.standings.over() => format!(
            "RACE RESULT{}\n{}\n\n",
            if o.standings.over() {
                ""
            } else {
                " - racing on, F: finish race"
            },
            o.standings.classification(&o.names).join("\n")
        ),
        (Some(_), Some(t)) if !osd.position && t.driving() => String::new(),
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
                "Position {}/{}{}{}{}\n",
                o.standings.position(0),
                o.standings.len(),
                gap("Ahead ", "-", ahead),
                gap("Behind", "+", behind),
                if o.standings.leader_flag().is_some() {
                    "\nChequered flag"
                } else {
                    ""
                }
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
            } else if !osd.lap_timer {
                race
            } else {
                format!("{race}{}\n\n{progress}", t.hud())
            }
        },
    );
    let progress = if osd.lap_timer || trial.is_some() {
        progress
    } else {
        String::new()
    };
    let (shown_speed, unit) = if osd.mph {
        (speed / 1.609_344, "mph")
    } else {
        (speed, "km/h")
    };
    // The original-layout HUD shows speed, laps and position itself.
    let text = if race_hud.is_some() {
        format!("{heading}{handbrake}\n{status}{audio}")
    } else {
        format!(
            "{heading}\n{shown_speed:.0} {unit}   Gear {gear}{handbrake}\n\n{progress}\n\n{status}{audio}"
        )
    };
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
                "Paused"
            } else if let Some(message) = terminal.as_deref() {
                message
            } else {
                "Arrows drive; Space handbrake; Esc/P pause; R restart"
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
        if runtime.lost_wheel() {
            format!("Lost the {wheel} wheel; controls disabled; R resets")
        } else {
            format!("Terminal {wheel} wheel damage; controls disabled; R resets")
        }
    })
}
/// Stage of the start-light board: stages 1..n-1 come on across the race
/// countdown, the last stage (the green row) shows for 3 s from GO, otherwise
/// idle. Mapping of the shipped "n stages over t s" to the countdown is [I]:
/// Donington 1..5 then texture 7, Nürburgring and Istanbul Park 1..6 then 7.
fn start_light_stage(
    phase: Option<(&grid_tools::time_trial::Phase, f64)>,
    seconds: f64,
    stages: usize,
) -> usize {
    use grid_tools::time_trial::Phase;
    let lit = stages.saturating_sub(1).max(1);
    match phase {
        Some((Phase::Countdown(left), _)) => {
            let remaining = f64::from(*left) * f64::from(grid_tools::source_vehicle::DELTA);
            let elapsed = (seconds - remaining).max(0.);
            ((elapsed / (seconds / lit as f64)) as usize + 1).min(lit)
        }
        Some((Phase::Running, elapsed)) if elapsed < 3. => stages,
        _ => 0,
    }
}
fn update_start_lights(
    trial: Option<Res<time_trial::Trial>>,
    lights: Option<ResMut<StartLights>>,
    start: Option<Res<StartSequence>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(mut lights) = lights else {
        return;
    };
    let seconds = start.map_or(START_SEQUENCE_SECONDS, |s| s.seconds);
    let stage = start_light_stage(
        trial.as_ref().map(|t| (t.state.phase(), t.state.elapsed())),
        seconds,
        lights.textures.len() - 1,
    );
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

/// Bevy's plugins; with the `profile` feature its log plugin also carries the
/// per-system span profile (docs/PERFORMANCE.md "Phone bisection").
fn with_profile(plugins: bevy::app::PluginGroupBuilder) -> bevy::app::PluginGroupBuilder {
    #[cfg(feature = "profile")]
    let plugins = plugins.set(bevy::log::LogPlugin {
        custom_layer: |_| Some(Box::new(span_profile::SpanProfile)),
        ..default()
    });
    plugins
}

#[cfg(test)]
mod driving_tests {
    use super::*;
    #[test]
    fn start_light_stages_follow_the_shipped_sequence() {
        use grid_tools::time_trial::Phase;
        let ticks = |seconds: f64| (seconds / f64::from(grid_tools::source_vehicle::DELTA)) as u32;
        let at = |left: f64, seconds, stages| {
            start_light_stage(Some((&Phase::Countdown(ticks(left)), 0.)), seconds, stages)
        };
        // Donington: 6 stages over 4.0 s, lights 1..5 across the countdown.
        assert_eq!([4.0, 3.1, 2.3, 1.5, 0.7, 0.].map(|l| at(l, 4.0, 6)), [1, 2, 3, 4, 5, 5]);
        // Nürburgring and Istanbul Park: 7 stages over 5.0 s, lights 1..6.
        assert_eq!([5.0, 4.1, 3.1, 2.1, 1.1, 0.1].map(|l| at(l, 5.0, 7)), [1, 2, 3, 4, 5, 6]);
        assert_eq!(start_light_stage(Some((&Phase::Running, 1.)), 5.0, 7), 7);
        assert_eq!(start_light_stage(Some((&Phase::Running, 3.5)), 5.0, 7), 0);
        assert_eq!(start_light_stage(None, 4.0, 6), 0);
        let default = StartSequence::default();
        assert_eq!((default.textures.len(), default.seconds), (7, 4.0));
    }

    #[test]
    fn only_rims_and_tyres_leave_with_a_lost_wheel() {
        assert!(lost_with_wheel("x0_wheel_fl"));
        assert!(lost_with_wheel("x1_tyre_br"));
        assert!(!lost_with_wheel("x0_disc_fl"));
        assert!(!lost_with_wheel("x0_hub_caliper_fl"));
        assert!(!lost_with_wheel("x0_bonnet"));
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn bmw_wheel_meshes_sit_on_its_source_wheel_anchors() {
        let root = std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required");
        let root = Path::new(&root);
        let bmw = CarModel::new(grid_tools::car::BMW).unwrap();
        let car = load(root, &Route::default(), &bmw, "LOD0_", false, false).unwrap();
        let runtime =
            grid_tools::source_vehicle::Runtime::load(root, &Route::default(), &bmw, "slot_00", 1)
                .unwrap();
        let mut counts = [0; 4];
        for part in &car.parts {
            if let Some(i) = part.wheel {
                counts[i] += 1;
                let pivot = (model_offset(&runtime) * part.matrices[0])
                    .w_axis
                    .truncate();
                // The BMW meshes sit about 11 mm inboard of the NED wheel
                // positions (the Lacetti's within 3 mm); height and axle agree.
                let anchor = Vec3::from_array(runtime.wheels()[i].anchor);
                let d = pivot - anchor;
                assert!(d.y.abs() < 0.003 && d.z.abs() < 0.003, "wheel {i} {d}");
                assert!(
                    pivot.x * anchor.x > 0. && d.x.abs() < 0.015,
                    "wheel {i} {d}"
                );
            }
        }
        assert!(counts.iter().all(|&n| n > 0), "{counts:?}");
        assert_eq!(livery_count(root, &bmw), 13);
        assert_eq!(livery_count(root, &CarModel::default()), 10);
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn wheel_render_remainder_pause_reset_and_source_control() {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let mut runtime =
            grid_tools::source_vehicle::Runtime::lacetti(Path::new(&root), "slot_00", 200679)
                .unwrap();
        for _ in 0..160 {
            runtime
                .tick(grid_tools::source_vehicle::Controls {
                    throttle: 1.,
                    ..Default::default()
                })
                .unwrap();
        }
        let mut fixed = Time::<Fixed>::from_seconds(0.016);
        fixed.accumulate_overstep(Duration::from_secs_f32(0.008));
        let mut app = App::new();
        app.insert_resource(Assets::<Mesh>::default())
            .insert_resource(fixed)
            .insert_resource(graphics::Settings {
                preset: graphics::Preset::Enhanced,
                ..default()
            })
            .init_resource::<DeformationPresentation>()
            .insert_resource(Driving {
                runtime,
                geometry: None,
                paused: false,
                failure: None,
                notices: Vec::new(),
                handbrake_from: None,
                auto_reset: Default::default(),
                reset_line: None,
                scripted_ticks: None,
                scripted: Default::default(),
                last_controls: None,
                segment: 0,
                started: Instant::now(),
                opponents: None,
            })
            .add_systems(Update, drive_present);
        app.world_mut().spawn((ViewerCamera, Transform::default()));
        let part = app
            .world_mut()
            .spawn((
                MovingPart {
                    rest: Mat4::IDENTITY,
                    wheel: Some(0),
                    lost_with_wheel: true,
                },
                Transform::default(),
                Mesh3d::default(),
                Aabb::default(),
            ))
            .id();
        let before = app
            .world()
            .resource::<Driving>()
            .runtime
            .state
            .car
            .owner
            .vehicle
            .clone();
        app.update();
        let moving = *app.world().get::<Transform>(part).unwrap();
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
        app.world_mut().resource_mut::<Driving>().paused = true;
        app.update();
        let paused = *app.world().get::<Transform>(part).unwrap();
        assert_ne!(moving, paused);
        app.update();
        assert_eq!(*app.world().get::<Transform>(part).unwrap(), paused);
        app.world_mut().resource_mut::<Driving>().paused = false;
        app.world_mut()
            .resource_mut::<graphics::Settings>()
            .smooth_wheel_motion = false;
        app.update();
        assert_eq!(*app.world().get::<Transform>(part).unwrap(), paused);
        app.world_mut().resource_mut::<Driving>().runtime.reset();
        app.world_mut()
            .resource_mut::<graphics::Settings>()
            .smooth_wheel_motion = true;
        app.update();
        let reset = *app.world().get::<Transform>(part).unwrap();
        assert_ne!(reset, moving);
        app.update();
        assert_eq!(*app.world().get::<Transform>(part).unwrap(), reset);
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn source_wheel_joints_follow_physics_without_feedback() {
        let root = std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required");
        let root = Path::new(&root);
        let car = load(
            root,
            &Route::default(),
            &CarModel::default(),
            "LOD0_",
            false,
            false,
        )
        .unwrap();
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
            // Render-time samples must never mutate the source wheel clock,
            // even with several display frames per physics step.
            for fps in [30_f32, 60., 144., 300.] {
                let shown = wheel_presentation::wheels(&runtime, (1. / fps).min(0.016));
                for (i, wheel) in shown.into_iter().enumerate() {
                    assert!(wheel_motion(wheel).is_finite());
                    assert_eq!(wheel.anchor, runtime.wheels()[i].anchor);
                    assert_eq!(wheel.displacement, runtime.wheels()[i].displacement);
                    assert_eq!(wheel.steering, runtime.wheels()[i].steering);
                }
            }
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
        let car = load(
            root,
            &Route::default(),
            &CarModel::default(),
            "LOD0_",
            false,
            false,
        )
        .unwrap();
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
                notices: Vec::new(),
                scripted_ticks: None,
                scripted: Default::default(),
                handbrake_from: None,
                last_controls: None,
                segment: 0,
                started: Instant::now(),
                opponents: None,
                auto_reset: Default::default(),
                reset_line: None,
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
                            lost_with_wheel: false,
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
        // A refused projection keeps the previous meshes and never pauses
        // physics (S3); the refused field is not projected again.
        assert!(app.world().resource::<Driving>().failure.is_none());
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
        // The next field change projects again.
        app.world_mut().resource_mut::<DeformationPresentation>().0 = None;
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
                notices: Vec::new(),
                scripted_ticks: None,
                scripted: Default::default(),
                handbrake_from: None,
                last_controls: None,
                segment: 0,
                started: Instant::now(),
                opponents: None,
                auto_reset: Default::default(),
                reset_line: None,
            })
            .insert_resource(input::Actions::new(
                input::ActionMap::keyboard(),
                input::Pads::Off,
            ))
            .init_resource::<input::ShiftQueue>()
            .insert_resource(pause_menu::PauseMenu::new(
                pause_menu::Variant::FreeRace,
                false,
                false,
                Default::default(),
            ))
            .add_systems(
                Update,
                (input::sample, pause_menu::update, drive_actions, drive_tick).chain(),
            );
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
        // A refused trial tick (here an event clock pushed one tick ahead)
        // invalidates the lap and never pauses physics (S3).
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowUp);
        while !app.world().resource::<time_trial::Trial>().driving() {
            app.update();
        }
        app.update();
        let ticks = app.world().resource::<Driving>().runtime.ticks;
        app.world_mut()
            .resource_mut::<time_trial::Trial>()
            .tick(ticks + 1, &[])
            .unwrap();
        app.update();
        app.update();
        assert!(app.world().resource::<Driving>().failure.is_none());
        assert_eq!(app.world().resource::<Driving>().runtime.ticks, ticks + 2);
        let trial = app.world().resource::<time_trial::Trial>();
        assert!(trial.driving());
        assert!((trial.state.elapsed() - trial.race_time(ticks + 2).unwrap()).abs() < 1e-9);
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
                notices: Vec::new(),
                scripted_ticks: None,
                scripted: Default::default(),
                handbrake_from: None,
                last_controls: None,
                segment: 0,
                started: Instant::now(),
                opponents: None,
                auto_reset: Default::default(),
                reset_line: None,
            })
            .insert_resource(input::Actions::new(
                input::ActionMap::keyboard(),
                input::Pads::Off,
            ))
            .init_resource::<input::ShiftQueue>()
            .insert_resource(pause_menu::PauseMenu::new(
                pause_menu::Variant::FreeRace,
                false,
                false,
                Default::default(),
            ))
            .add_systems(
                Update,
                (input::sample, pause_menu::update, drive_actions, drive_tick).chain(),
            );
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
