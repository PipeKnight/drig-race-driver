//! Source population from each route, batched by occupied cell and billboard material.
#[path = "crowd_near.rs"]
mod near;
use super::{
    crowd_population as population,
    crowd_preview::{self, Batch, CrowdMaterial, Spec, Tint},
};
use bevy::{
    asset::RenderAssetUsages,
    camera::primitives::Aabb,
    light::NotShadowCaster,
    mesh::{Indices, PrimitiveTopology},
    pbr::{ExtendedMaterial, MaterialPlugin},
    prelude::*,
};
use grid_formats::{
    binxml,
    crowd_placements::{self, Placements, Standing},
    material, pssg, texture,
};
use grid_tools::route::Route;
use std::{
    collections::{BTreeMap, VecDeque},
    error::Error,
    path::Path,
};
type AnyResult<T> = Result<T, Box<dyn Error>>;
#[derive(Clone, Debug)]
struct Row {
    position: Vec3,
    orientation: [f32; 2],
    opaque: f32,
    cell: u32,
    source: [usize; 2],
    material: usize,
    has_camera: bool,
}
#[derive(Debug)]
struct Omitted {
    group: Vec<u8>,
    records: u32,
    moving: bool,
    reason: String,
}
#[derive(Resource)]
pub(crate) struct Prepared {
    rows: Vec<Row>,
    specs: Vec<Spec>,
    diffuse: Vec<u8>,
    tint: Vec<u8>,
    region: String,
    atlas: String,
    total: u32,
    omitted: Vec<Omitted>,
    flash_dds: Option<Vec<u8>>,
    flash_cells: Vec<FlashCell>,
    actors: Vec<Option<near::Baked>>,
}
fn field<'a>(fields: &'a population::Fields, name: &str) -> AnyResult<&'a str> {
    let mut found = fields.iter().filter(|(k, _)| k == name);
    let value = found
        .next()
        .ok_or_else(|| format!("billboard missing {name}"))?;
    if found.next().is_some() {
        return Err(format!("duplicate billboard {name}").into());
    }
    Ok(&value.1)
}
fn number(fields: &population::Fields, name: &str) -> AnyResult<f32> {
    let x: f32 = field(fields, name)?.parse()?;
    if !x.is_finite() {
        return Err("nonfinite billboard field".into());
    }
    Ok(x)
}
// Matches the initialized source userdata producer44 on this explicitly admitted ASCII domain.
fn context(entries: &[population::DatasetPssg<'_>]) -> AnyResult<(String, String)> {
    let found: Vec<_> = entries
        .iter()
        .filter_map(|e| e.userdata.find("billboard").map(|at| (e, at)))
        .collect();
    let [(entry, at)] = found.as_slice() else {
        return Err("static billboard consumer needs one unambiguous billboard userdata".into());
    };
    if !entry.userdata.is_ascii() {
        return Err("non-ASCII billboard context unsupported".into());
    }
    let suffix = entry.userdata[*at + "billboard".len()..]
        .split_ascii_whitespace()
        .next()
        .unwrap_or("undefined");
    let region = if suffix.is_empty() || suffix.eq_ignore_ascii_case("undefined") {
        "europe"
    } else {
        suffix
    };
    Ok((entry.userdata.into(), region.into()))
}
fn shared_resource(
    shared: &grid_tools::tyre_smoke::Node,
    kind: &str,
    alias: &str,
) -> AnyResult<String> {
    if shared.name != "dataset" {
        return Err("shared organism dataset root differs".into());
    }
    let mut selected = None;
    for entry in shared.children.iter().filter(|e| e.name == kind) {
        let processor = field(&entry.attributes, "processor")?;
        if processor != "OrganismPlugin" {
            continue;
        }
        let user = field(&entry.attributes, "userdata")?;
        if user.eq_ignore_ascii_case(alias) && selected.replace(entry).is_some() {
            return Err("ambiguous shared crowd resource".into());
        }
    }
    let source = selected.ok_or("shared crowd resource absent")?;
    let filename = field(&source.attributes, "filename")?;
    if filename.contains('&') {
        return Err("XML entity in shared crowd filename unsupported".into());
    }
    Ok(population::resource_path(filename)?)
}
fn source_spec(template: &population::Template) -> AnyResult<Option<Spec>> {
    // Authored SourceOrderFirst, not an attempt to replay native RNG/weights.
    let variant = template.variants.first().ok_or("source variants absent")?;
    let Some(stance) = &variant.billboard_stance else {
        return Ok(None);
    };
    let uv = [
        number(stance, "u0")?,
        number(stance, "v0")?,
        number(stance, "u1")?,
        number(stance, "v1")?,
    ];
    let dimensions = [number(stance, "width")?, number(stance, "height")?];
    if !uv.iter().all(|x| (0. ..=1.).contains(x))
        || uv[0] >= uv[2]
        || uv[1] >= uv[3]
        || !dimensions.iter().all(|x| *x > 0. && *x < 5.)
    {
        return Err("unsupported billboard rectangle/dimensions".into());
    }
    let h = number(&template.stance_fields, "randomHeightScale")?;
    let w = number(&template.stance_fields, "randomWidthScale")?;
    if !(0. ..0.25).contains(&h) || !(0. ..0.25).contains(&w) {
        return Err("billboard fixed scale-fixture domain exceeded".into());
    }
    let colours = &variant
        .colour_sets
        .first()
        .ok_or("source colour sets absent")?
        .colours;
    if colours.len() != 3 {
        return Err("billboard adapter needs three saved tint colours".into());
    }
    let mut rgb = [Vec4::ZERO; 3];
    for (dst, c) in rgb.iter_mut().zip(colours) {
        *dst = Vec4::new(
            number(c, "red")?,
            number(c, "green")?,
            number(c, "blue")?,
            1.,
        );
    }
    Ok(Some(Spec {
        uv,
        dimensions,
        colours: rgb,
        instance_scale: crowd_preview::source_scale_fixture(h, w),
    }))
}
fn append_group(
    points: &[Standing],
    alternatives: &[population::Template],
    group: usize,
    rows: &mut Vec<Row>,
    specs: &mut Vec<Spec>,
) -> AnyResult<bool> {
    let template = alternatives
        .first()
        .ok_or("static map alternatives absent")?;
    let Some(spec) = source_spec(template)? else {
        return Ok(false);
    };
    if rows
        .len()
        .checked_add(points.len())
        .is_none_or(|n| n > 100_000)
    {
        return Err("static billboard row cap exceeded".into());
    }
    if points
        .iter()
        .any(|point| !point.position.iter().all(|v| v.is_finite()))
    {
        return Err("nonfinite source billboard position".into());
    }
    let material = specs.len();
    specs.push(spec);
    for (ordinal, p) in points.iter().enumerate() {
        rows.push(Row {
            position: Vec3::from_array(p.position),
            orientation: p.orientation_pair,
            opaque: p.opaque_scalar,
            cell: p.cell,
            source: [group, ordinal],
            material,
            has_camera: template
                .definition_fields
                .iter()
                .any(|(key, value)| key == "hasCamera" && value == "1"),
        });
    }
    Ok(true)
}
fn atlas(root: &Path, relative: &str) -> AnyResult<(Vec<u8>, Vec<u8>)> {
    let bytes = std::fs::read(root.join(relative))?;
    let tree = pssg::read_tree(&bytes)?;
    atlas_from_tree(&tree)
}
fn atlas_from_tree(tree: &pssg::Tree<'_>) -> AnyResult<(Vec<u8>, Vec<u8>)> {
    let bindings = material::read_texture_bindings(tree)?;
    let ds: Vec<_> = bindings
        .iter()
        .filter(|b| b.sampler == b"TDiffuseAlphaMap")
        .collect();
    let ms: Vec<_> = bindings
        .iter()
        .filter(|b| b.sampler == b"TTintMap")
        .collect();
    let ([d], [m]) = (ds.as_slice(), ms.as_slice()) else {
        return Err("billboard atlas texture-role binding absent/ambiguous".into());
    };
    if d.shader != m.shader || d.container.is_some() || m.container.is_some() {
        return Err("billboard roles need one local shader/texture container".into());
    }
    let shaders: Vec<_> = (0..tree.nodes.len())
        .filter(|&i| {
            tree.name(i) == b"SHADERINSTANCE" && tree.string(i, b"id").ok() == Some(d.shader)
        })
        .collect();
    let [shader] = shaders.as_slice() else {
        return Err("billboard shader instance ambiguous".into());
    };
    if tree.string(*shader, b"shaderGroup")? != b"#crowd_billboard.fx" {
        return Err("unsupported billboard material contract".into());
    }
    for selected in [d.texture, m.texture] {
        let count = (0..tree.nodes.len())
            .filter(|&i| tree.name(i) == b"TEXTURE" && tree.string(i, b"id").ok() == Some(selected))
            .count();
        if count != 1 {
            return Err("selected billboard texture ID absent/ambiguous".into());
        }
    }
    let textures = texture::read_named(tree, &[d.texture, m.texture])?;
    let diffuse = textures
        .iter()
        .find(|t| t.id == d.texture)
        .ok_or("bound diffuse texture absent")?;
    let tint = textures
        .iter()
        .find(|t| t.id == m.texture)
        .ok_or("bound tint texture absent")?;
    if diffuse.format != texture::Format::Bc3
        || tint.format != texture::Format::Bc1
        || diffuse.width != tint.width
        || diffuse.height != tint.height
    {
        return Err("unsupported/mismatched billboard atlas formats".into());
    }
    Ok((diffuse.validated_dds().0, tint.validated_dds().0))
}
fn prepare(root: &Path, route: &Route) -> AnyResult<Prepared> {
    let graph = grid_tools::tyre_smoke::read_xml(
        &root.join(route.track_file("organism_track_dataset.xml")),
    )?;
    let dataset = population::dataset(&graph)?;
    let (user, region) = context(&dataset.pssg)?;
    let shared = grid_tools::tyre_smoke::read_xml(&root.join("anims/organism_static_dataset.xml"))?;
    let raw = std::fs::read(root.join(shared_resource(&shared, "xml", "typeDefs")?))?;
    let defs = binxml::read(&raw)?;
    let bytes = std::fs::read(root.join(route.file("crowd_standing.bin")))?;
    let source = crowd_placements::read(&bytes)?;
    let metadata = population::prepare(
        &source,
        &defs,
        &dataset.pssg,
        population::DefinitionSelection::Exact {
            primary_definition_set: Some(&region),
            fallback_definition_set: Some("generic"),
            map_group: "generic",
            billboard_userdata: &user,
        },
    )?;
    let resource = metadata
        .dataset_resources
        .get(&user)
        .ok_or("selected billboard resource absent")?;
    let mut rows = Vec::new();
    let mut specs = Vec::new();
    let mut actors = Vec::new();
    let mut near_vertex_count = 0usize;
    let animation_bank_bytes = metadata
        .dataset_resources
        .get("hla")
        .and_then(|resource| std::fs::read(root.join(&resource.relative_path)).ok());
    let animation_bank = animation_bank_bytes
        .as_deref()
        .and_then(|bytes| pssg::read_tree(bytes).ok());
    let selector_bytes = shared_resource(&shared, "binary", "hla")
        .ok()
        .and_then(|path| std::fs::read(root.join(path)).ok());
    let selector = selector_bytes
        .as_deref()
        .and_then(|bytes| binxml::read(bytes).ok());
    let library = selector
        .as_ref()
        .and_then(|doc| grid_tools::crowd_selection::Library::parse(doc).ok());
    let mut omitted = Vec::new();
    for (index, g) in metadata.groups.iter().enumerate() {
        let (moving, reason) = match (&g.source.placements, &g.resolution) {
            (
                Placements::Standing(points),
                population::Resolution::StaticTemplates(alternatives),
            ) => {
                if alternatives.iter().any(|t| t.billboard != *resource) {
                    return Err("map alternative atlas differs from selected context".into());
                }
                if append_group(points, alternatives, index, &mut rows, &mut specs)? {
                    let actor = if let (Some(library), Some(bank), [template, ..]) = (
                        library.as_ref(),
                        animation_bank.as_ref(),
                        alternatives.as_slice(),
                    ) {
                        match near::bake(root, template, &metadata, library, bank) {
                            Ok(actor) => actor,
                            Err(error) => {
                                eprintln!("crowd source clip for {}: {error}", template.map_name);
                                None
                            }
                        }
                    } else {
                        None
                    };
                    let actor = actor.filter(|body| {
                        let wanted = near_vertex_count + body.frame_vertices();
                        if wanted > NEAR_FRAME_VERTEX_LIMIT {
                            return false;
                        }
                        near_vertex_count = wanted;
                        true
                    });
                    actors.push(actor);
                    continue;
                }
                (false,"selected source variant has no declared billboard stance; skinned/other role remains undrawn".to_owned())
            }
            (Placements::Moving(_), population::Resolution::UnsupportedMoving { .. }) => {
                (true, "moving payload unsupported".into())
            }
            (Placements::Standing(_), population::Resolution::Unresolved(error)) => {
                (false, format!("unresolved source metadata: {error}"))
            }
            _ => return Err("population resolution/placement classification differs".into()),
        };
        omitted.push(Omitted {
            group: g.source.instance_set.to_vec(),
            records: g.source.count,
            moving,
            reason,
        });
    }
    if rows.len() as u64 + omitted.iter().map(|o| u64::from(o.records)).sum::<u64>()
        != u64::from(source.total_instances)
    {
        return Err("billboard source accounting differs".into());
    }
    let (diffuse, tint) = atlas(root, &resource.relative_path)?;
    let flash_dds = if let Some(resource) = metadata.dataset_resources.get("camFlash")
        && rows.iter().any(|row| row.has_camera)
    {
        match super::flash_preview::source_dds(&root.join(&resource.relative_path)) {
            Ok(dds) => Some(dds),
            Err(error) => {
                eprintln!("crowd camera flash disabled on {}: {error}", route.path());
                None
            }
        }
    } else {
        None
    };
    let mut cameras = BTreeMap::<u32, Vec<usize>>::new();
    for (index, row) in rows.iter().enumerate().filter(|(_, row)| row.has_camera) {
        let eligible = cameras.entry(row.cell).or_default();
        if eligible.len() < 300 {
            eligible.push(index);
        }
    }
    let flash_cells = source
        .cells
        .iter()
        .enumerate()
        .map(|(index, cell)| {
            let [x0, y0, z0, x1, y1, z1] = cell.bounds;
            FlashCell {
                center: Vec3::new((x0 + x1) * 0.5, (y0 + y1) * 0.5, (z0 + z1) * 0.5),
                instances: cell.instances,
                eligible: cameras.remove(&(index as u32)).unwrap_or_default(),
            }
        })
        .collect();
    Ok(Prepared {
        rows,
        specs,
        diffuse,
        tint,
        region,
        atlas: resource.relative_path.clone(),
        total: source.total_instances,
        omitted,
        flash_dds,
        flash_cells,
        actors,
    })
}
fn install(app: &mut App, prepared: Prepared) -> AnyResult<()> {
    println!(
        "crowd_population_static region={} atlas={} source={} billboards={} unsupported_static={} moving={} choice=SourceOrderFirst(map,variant,colour) scale_fixture_draw=5000 orientation=camera-facing animation=upper-edge-sway native_rng=false native_lod=false",
        prepared.region,
        prepared.atlas,
        prepared.total,
        prepared.rows.len(),
        prepared
            .omitted
            .iter()
            .filter(|o| !o.moving)
            .map(|o| o.records)
            .sum::<u32>(),
        prepared
            .omitted
            .iter()
            .filter(|o| o.moving)
            .map(|o| o.records)
            .sum::<u32>()
    );
    for o in &prepared.omitted {
        println!(
            "crowd_population_unsupported group={:?} records={} moving={} reason={}",
            String::from_utf8_lossy(&o.group),
            o.records,
            o.moving,
            o.reason
        );
    }
    if prepared.rows.is_empty() {
        return Ok(());
    }
    crowd_preview::install_shader(app)?;
    if prepared.flash_dds.is_some() {
        super::flash_preview::register_material(app)?;
        app.add_systems(PostStartup, install_flash).add_systems(
            PostUpdate,
            update_flash.before(bevy::camera::visibility::VisibilitySystems::CheckVisibility),
        );
    }
    app.init_resource::<CrowdClock>()
        .init_resource::<NearRuntime>()
        .add_systems(
            Update,
            (advance_clock, update_actors)
                .chain()
                .after(crate::drive_present)
                .after(crate::replay::control)
                .after(crate::replay::frame_camera),
        )
        .add_systems(
            PostUpdate,
            update_cards.before(bevy::camera::visibility::VisibilitySystems::CheckVisibility),
        );
    app.insert_resource(prepared)
        .add_plugins(MaterialPlugin::<CrowdMaterial>::default())
        .add_systems(PostStartup, spawn);
    Ok(())
}
fn spawn(
    mut commands: Commands,
    prepared: Res<Prepared>,
    view: Query<(&Transform, &Msaa), With<super::ViewerCamera>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<CrowdMaterial>>,
    settings: Res<super::graphics::Settings>,
) {
    let Ok((camera, msaa)) = view.single() else {
        return;
    };

    let d = images.add(
        super::texture_upload::image(
            &prepared.diffuse,
            settings.sampler(false),
            RenderAssetUsages::default(),
        )
        .expect("validated billboard diffuse"),
    );
    let m = images.add(
        super::texture_upload::image_linear(
            &prepared.tint,
            settings.sampler(false),
            RenderAssetUsages::default(),
        )
        .expect("validated billboard tint"),
    );
    let handles: Vec<_> = prepared
        .specs
        .iter()
        .map(|s| {
            materials.add(ExtendedMaterial {
                base: StandardMaterial {
                    base_color_texture: Some(d.clone()),
                    alpha_mode: if *msaa == Msaa::Off {
                        AlphaMode::Mask(0.45)
                    } else {
                        AlphaMode::AlphaToCoverage
                    },
                    perceptual_roughness: 1.,
                    reflectance: 0.,
                    cull_mode: None,
                    double_sided: true,
                    ..default()
                },
                extension: Tint {
                    map: m.clone(),
                    colour0: s.colours[0],
                    colour1: s.colours[1],
                    colour2: s.colours[2],
                },
            })
        })
        .collect();
    let mut batches: BTreeMap<(usize, u32), Vec<usize>> = BTreeMap::new();
    for (index, row) in prepared.rows.iter().enumerate() {
        batches
            .entry((row.material, row.cell))
            .or_default()
            .push(index);
    }
    let mut raw = 0u64;
    for row in &prepared.rows {
        raw ^= row.orientation[0].to_bits() as u64
            ^ row.orientation[1].to_bits() as u64
            ^ row.opaque.to_bits() as u64
            ^ ((row.source[0] as u64) << 32)
            ^ row.source[1] as u64;
    }
    for ((material, cell), rows) in batches {
        let spec = &prepared.specs[material];
        let Ok(batch) = make_batch(&prepared, &rows, spec, camera.translation, 0.0) else {
            eprintln!("crowd billboard geometry rejected for cell {cell}");
            continue;
        };
        // Bounds account for every possible camera-facing width and sway.
        let xz_radius = Vec3::new(
            2.0 * spec.dimensions[0] * spec.instance_scale[0],
            0.,
            2.0 * spec.dimensions[0] * spec.instance_scale[0],
        );
        let height = Vec3::Y * spec.dimensions[1] * spec.instance_scale[1];
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for &index in &rows {
            let p = prepared.rows[index].position;
            min = min.min(p - xz_radius);
            max = max.max(p + xz_radius + height);
        }
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, batch.p);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, batch.n);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, batch.uv);
        mesh.insert_indices(Indices::U32(batch.indices));
        commands.spawn((
            Name::new(format!("crowd/material{material}/cell{cell}")),
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(handles[material].clone()),
            Transform::IDENTITY,
            Aabb::from_min_max(min, max),
            NotShadowCaster,
            CardCell {
                material,
                rows,
                min,
                max,
                last_frame: u64::MAX,
                last_eye: camera.translation,
                omitted: Vec::new(),
            },
        ));
    }
    let models = prepared
        .actors
        .iter()
        .zip(&prepared.specs)
        .map(|(actor, spec)| {
            let actor = actor.as_ref()?;
            let diffuse = super::texture_upload::image(
                &actor.diffuse,
                settings.sampler(false),
                RenderAssetUsages::default(),
            )
            .ok()?;
            let tint = super::texture_upload::image_linear(
                &actor.tint,
                settings.sampler(false),
                RenderAssetUsages::default(),
            )
            .ok()?;
            let material = materials.add(ExtendedMaterial {
                base: StandardMaterial {
                    base_color_texture: Some(images.add(diffuse)),
                    perceptual_roughness: 1.,
                    reflectance: 0.,
                    cull_mode: None,
                    double_sided: true,
                    ..default()
                },
                extension: Tint {
                    map: images.add(tint),
                    colour0: spec.colours[0],
                    colour1: spec.colours[1],
                    colour2: spec.colours[2],
                },
            });
            Some(NearModel {
                frames: actor.meshes(&mut meshes),
                foot_offset: actor.foot_offset,
                material,
            })
        })
        .collect::<Vec<_>>();
    println!(
        "crowd_animated_templates={}",
        models.iter().flatten().count()
    );
    commands.insert_resource(NearModels(models));
    println!(
        "crowd_population_spawn rows={} source_metadata_checksum={raw}",
        prepared.rows.len()
    );
}

#[derive(Component)]
struct CardCell {
    material: usize,
    rows: Vec<usize>,
    min: Vec3,
    max: Vec3,
    last_frame: u64,
    last_eye: Vec3,
    omitted: Vec<usize>,
}

#[derive(Default, Resource)]
struct CrowdClock {
    seconds: f64,
    segment: u64,
    replaying: bool,
}

fn advance_clock(
    mut clock: ResMut<CrowdClock>,
    drive: Res<super::Driving>,
    replay: Option<Res<super::replay::Replay>>,
) {
    let shown = replay.as_deref().and_then(|replay| replay.session.as_ref());
    clock.segment = drive.segment;
    clock.replaying = shown.is_some();
    clock.seconds = shown.map_or_else(
        || super::replay::seconds(drive.runtime.ticks),
        |session| session.player.time,
    );
}

// Distant impostors use a low-amplitude top-corner deformation. The bottom
// edge stays at the saved position. The port uses elapsed game time and a
// per-placement phase until the original sway clock and selection are known.
const UPDATES_PER_SECOND: f64 = 15.;
const ACTIVE_RANGE: f32 = 160.;

fn phase(index: usize) -> f32 {
    (index as u32).wrapping_mul(2_654_435_761) as f32 * (std::f32::consts::TAU / 4_294_967_296.)
}

fn make_batch(
    source: &Prepared,
    rows: &[usize],
    spec: &Spec,
    eye: Vec3,
    seconds: f32,
) -> Result<Batch, &'static str> {
    let mut batch = Batch::default();
    for &index in rows {
        let position = source.rows[index].position;
        let delta = eye - position;
        let horizontal = Vec3::new(delta.x, 0., delta.z);
        let forward = horizontal.try_normalize().unwrap_or(Vec3::Z);
        let camera_point = if horizontal.length_squared() < 1e-8 {
            position + forward
        } else {
            eye
        };
        let first = batch.p.len();
        crowd_preview::card_values(&mut batch, position, spec, camera_point)?;
        let right = Vec3::new(-forward.z, 0., forward.x);
        let movement = (seconds * 2.2 + phase(index)).sin()
            * 0.10
            * spec.dimensions[0]
            * spec.instance_scale[0];
        for offset in [1, 3] {
            let vertex = &mut batch.p[first + offset];
            *vertex = (Vec3::from_array(*vertex) + right * movement).to_array();
        }
    }
    Ok(batch)
}

const NEAR_DISTANCE: f32 = 20.;
const NEAR_LIMIT: usize = 64;
// 320k resident sample vertices ≈ 10 MiB of position/normal/UV data.
const NEAR_FRAME_VERTEX_LIMIT: usize = 320_000;

#[derive(Resource)]
struct NearModels(Vec<Option<NearModel>>);

struct NearModel {
    frames: near::PoseMeshes,
    foot_offset: Vec3,
    material: Handle<CrowdMaterial>,
}

#[derive(Resource, Default)]
struct NearRuntime(BTreeMap<usize, Entity>);

fn update_actors(
    mut commands: Commands,
    prepared: Res<Prepared>,
    clock: Res<CrowdClock>,
    models: Option<Res<NearModels>>,
    mut live: ResMut<NearRuntime>,
    (view, cells): (
        Single<&Transform, With<super::ViewerCamera>>,
        Query<&CardCell>,
    ),
    mut meshes: Query<&mut Mesh3d, Without<CardCell>>,
) {
    let Some(models) = models else { return };
    let eye = view.translation;
    let range = NEAR_DISTANCE * NEAR_DISTANCE;
    let mut candidates = Vec::new();
    for cell in &cells {
        if models
            .0
            .get(cell.material)
            .and_then(Option::as_ref)
            .is_none()
            || eye.distance_squared(eye.clamp(cell.min, cell.max)) > range
        {
            continue;
        }
        candidates.extend(cell.rows.iter().filter_map(|&row| {
            let d = eye.distance_squared(prepared.rows[row].position);
            (d <= range).then_some((d, row))
        }));
    }
    candidates.sort_unstable_by(|(a, i), (b, j)| a.total_cmp(b).then(i.cmp(j)));
    candidates.truncate(NEAR_LIMIT);
    let selected = candidates
        .iter()
        .map(|(_, row)| *row)
        .collect::<std::collections::BTreeSet<_>>();
    live.0.retain(|row, entity| {
        if selected.contains(row) {
            true
        } else {
            commands.entity(*entity).despawn();
            false
        }
    });
    for (_, index) in candidates {
        let row = &prepared.rows[index];
        let Some(model) = &models.0[row.material] else {
            continue;
        };
        let unit_phase = f64::from(phase(index) / std::f32::consts::TAU);
        let handle = model.frames.frame(clock.seconds, unit_phase);
        if let Some(&entity) = live.0.get(&index) {
            if let Ok(mut mesh) = meshes.get_mut(entity)
                && mesh.id() != handle.id()
            {
                mesh.0 = handle.clone();
            }
        } else {
            // The saved two-vector represents actor local X in world XZ.
            let [x, z] = row.orientation;
            let yaw = Quat::from_rotation_y((-z).atan2(x));
            let entity = commands
                .spawn((
                    Name::new("crowd/source-animation"),
                    Mesh3d(handle.clone()),
                    MeshMaterial3d(model.material.clone()),
                    Transform::from_translation(row.position + yaw * model.foot_offset)
                        .with_rotation(yaw),
                    bevy::camera::visibility::NoFrustumCulling,
                    NotShadowCaster,
                ))
                .id();
            live.0.insert(index, entity);
        }
    }
}

fn update_cards(
    prepared: Res<Prepared>,
    clock: Res<CrowdClock>,
    live: Res<NearRuntime>,
    view: Single<&Transform, With<super::ViewerCamera>>,
    mut cells: Query<(&Mesh3d, &mut CardCell, &mut Visibility)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let eye = view.translation;
    let frame = (clock.seconds * UPDATES_PER_SECOND) as u64;
    let seconds = (frame as f64 / UPDATES_PER_SECOND) as f32;
    for (mesh, mut cell, mut visibility) in &mut cells {
        let distance = eye.distance_squared(eye.clamp(cell.min, cell.max));
        if distance > ACTIVE_RANGE * ACTIVE_RANGE && cell.omitted.is_empty() {
            continue;
        }
        let omitted: Vec<_> =
            if distance <= NEAR_DISTANCE * NEAR_DISTANCE || !cell.omitted.is_empty() {
                cell.rows
                    .iter()
                    .copied()
                    .filter(|row| live.0.contains_key(row))
                    .collect()
            } else {
                Vec::new()
            };
        let topology_changed = omitted != cell.omitted;
        if !topology_changed
            && cell.last_frame == frame
            && eye.distance_squared(cell.last_eye) < 0.25
        {
            continue;
        }
        let visible: Vec<_> = cell
            .rows
            .iter()
            .copied()
            .filter(|row| !live.0.contains_key(row))
            .collect();
        if visible.is_empty() {
            *visibility = Visibility::Hidden;
        } else {
            let Ok(batch) = make_batch(
                &prepared,
                &visible,
                &prepared.specs[cell.material],
                eye,
                seconds,
            ) else {
                continue;
            };
            *visibility = Visibility::Inherited;
            if let Some(mut mesh) = meshes.get_mut(&mesh.0) {
                mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, batch.p);
                mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, batch.n);
                if topology_changed {
                    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, batch.uv);
                    mesh.insert_indices(Indices::U32(batch.indices));
                }
            }
        }
        cell.omitted = omitted;
        cell.last_frame = frame;
        cell.last_eye = eye;
    }
}

struct FlashCell {
    center: Vec3,
    instances: u32,
    eligible: Vec<usize>,
}

#[derive(Clone, Copy)]
struct FlashEvent {
    anchor: super::flash_fixture::Anchor,
    born: f64,
}

#[derive(Resource)]
struct Flashes {
    segment: u64,
    next: f64,
    last_live_time: f64,
    random: u32,
    history: VecDeque<FlashEvent>,
}

#[derive(Component)]
struct FlashCard(usize);

impl Flashes {
    fn visible_at(&self, seconds: f64) -> std::ops::Range<usize> {
        let first = self
            .history
            .partition_point(|event| event.born + FLASH_LIFETIME <= seconds);
        let last = self.history.partition_point(|event| event.born <= seconds);
        first..last
    }

    fn cut(&mut self, seconds: f64) {
        let retained = self.history.partition_point(|event| event.born <= seconds);
        self.history.truncate(retained);
        self.next = seconds + 0.1;
    }
}

fn install_flash(
    mut commands: Commands,
    prepared: Res<Prepared>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<super::flash_preview::FlashMaterial>>,
    settings: Res<super::graphics::Settings>,
) {
    let Some(dds) = &prepared.flash_dds else {
        return;
    };
    let Ok(image) =
        super::texture_upload::image(dds, settings.sampler(false), RenderAssetUsages::default())
    else {
        eprintln!("crowd camera flash texture could not be decoded");
        return;
    };
    let handle = materials.add(super::flash_preview::FlashMaterial {
        map: images.add(image),
    });
    commands.insert_resource(Flashes {
        segment: 0,
        next: 0.1,
        last_live_time: 0.,
        random: 1,
        history: VecDeque::new(),
    });
    for index in 0..10 {
        commands.spawn((
            Name::new("crowd/source-camera-flash"),
            FlashCard(index),
            Mesh3d(default()),
            MeshMaterial3d(handle.clone()),
            // The pulse owns a tiny changing view-facing mesh.
            bevy::camera::visibility::NoFrustumCulling,
            Visibility::Hidden,
            Transform::IDENTITY,
            NotShadowCaster,
        ));
    }
}

fn flash_draw(state: &mut u32, count: usize) -> usize {
    *state = state.wrapping_mul(214_013).wrapping_add(2_531_011);
    ((*state >> 16) & 0x7fff) as usize % count
}

// Source constructor: 40–100 m at the reference angle, 50–1000 actors,
// intervals .5–.1 and .15 lifetime. The port uses ordinary game time and
// the viewer's vertical half-FOV for the still-unresolved original caller.
const FLASH_LIFETIME: f64 = f32::from_bits(0x3e19_999a) as f64;
const FLASH_REFERENCE_ANGLE: f32 = f32::from_bits(0x3f75_be0b);

fn flash_interval(visible: u32) -> f64 {
    let ratio = visible.clamp(50, 1000).saturating_sub(50) as f64 / 950.;
    0.5 - 0.4 * ratio
}

fn update_flash(
    prepared: Res<Prepared>,
    clock: Res<CrowdClock>,
    mut flashes: Option<ResMut<Flashes>>,
    view: Single<(&Transform, &Projection), With<super::ViewerCamera>>,
    mut cards: Query<
        (&FlashCard, &mut Transform, &mut Visibility, &mut Mesh3d),
        Without<super::ViewerCamera>,
    >,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Some(flashes) = flashes.as_deref_mut() else {
        return;
    };
    let (camera, projection) = *view;
    let eye = camera.translation;
    let now = clock.seconds;
    if flashes.segment != clock.segment {
        flashes.segment = clock.segment;
        flashes.next = 0.1;
        flashes.last_live_time = 0.;
        flashes.random = 1;
        flashes.history.clear();
    }
    if !clock.replaying && now < flashes.last_live_time {
        // A Flashback cut abandons future effects along with future car state.
        flashes.cut(now);
    }
    if !clock.replaying {
        flashes.last_live_time = now;
    }
    if !clock.replaying && now >= flashes.next {
        let angle = match projection {
            Projection::Perspective(perspective) => (perspective.fov * 0.5).clamp(0.1, 1.5),
            _ => FLASH_REFERENCE_ANGLE,
        };
        let scale = FLASH_REFERENCE_ANGLE / angle;
        let distance = (40. * scale)..=(100. * scale);
        let cone = angle.cos();
        let forward = camera.forward();
        let cells: Vec<_> = prepared
            .flash_cells
            .iter()
            .filter(|cell| {
                let delta = cell.center - eye;
                let length = delta.length();
                distance.contains(&length) && delta.dot(*forward) >= cone * length
            })
            .take(1000)
            .collect();
        let population = cells.iter().map(|cell| cell.instances).sum();
        flashes.next = now + flash_interval(population);
        if !cells.is_empty() {
            let cell = cells[flash_draw(&mut flashes.random, cells.len())];
            if !cell.eligible.is_empty() && flashes.visible_at(now).len() < 10 {
                let index = cell.eligible[flash_draw(&mut flashes.random, cell.eligible.len())];
                let row = &prepared.rows[index];
                // Saved fallback homogeneous local position is (0, 1.7, 0, 1).
                let position = row.position + Vec3::Y * 1.7;
                if flashes.history.len() == 65_536 {
                    flashes.history.pop_front();
                }
                flashes.history.push_back(FlashEvent {
                    anchor: super::flash_fixture::Anchor {
                        source_index: index as u32,
                        source_cell: row.cell,
                        position: position.to_array(),
                    },
                    born: now,
                });
            }
        }
    }

    let active = flashes.visible_at(now);
    for (card, mut transform, mut visibility, mut handle) in &mut cards {
        *visibility = Visibility::Hidden;
        let index = active.start + card.0;
        if index >= active.end {
            continue;
        }
        let Some(event) = flashes.history.get(index) else {
            continue;
        };
        let phase = ((now - event.born) / FLASH_LIFETIME) as f32;
        if let Ok((mesh, center)) =
            super::flash_preview::event_mesh(event.anchor, eye.to_array(), phase)
        {
            if let Some(mut existing) = meshes.get_mut(&handle.0) {
                *existing = mesh;
            } else {
                handle.0 = meshes.add(mesh);
            }
            transform.translation = center;
            *visibility = Visibility::Visible;
        }
    }
}
/// Called by ordinary rendered driving. Routes with no standing population
/// need no crowd material registration or asset lookup.
pub(crate) fn install_route(app: &mut App, root: &Path, route: &Route) -> AnyResult<()> {
    match std::fs::metadata(root.join(route.file("crowd_standing.bin"))) {
        Ok(_) => install(app, prepare(root, route)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_billboard_deforms_only_the_top_and_keeps_its_saved_footing() {
        let spec = Spec {
            uv: [0., 0., 1., 1.],
            dimensions: [0.6, 1.4],
            colours: [Vec4::ONE; 3],
            instance_scale: [1., 1.],
        };
        let source = Prepared {
            rows: vec![Row {
                position: Vec3::new(3., 4., 5.),
                orientation: [0., 1.],
                opaque: 0.,
                cell: 0,
                source: [0, 0],
                material: 0,
                has_camera: false,
            }],
            specs: vec![spec.clone()],
            diffuse: Vec::new(),
            tint: Vec::new(),
            region: "test".to_owned(),
            atlas: "test".to_owned(),
            total: 1,
            omitted: Vec::new(),
            flash_dds: None,
            flash_cells: Vec::new(),
            actors: Vec::new(),
        };
        let eye = Vec3::new(3., 5., 10.);
        let a = make_batch(&source, &[0], &spec, eye, 0.).unwrap();
        let b = make_batch(&source, &[0], &spec, eye, 0.5).unwrap();
        assert_eq!(a.uv, b.uv);
        assert_eq!(a.indices, b.indices);
        assert_eq!(a.p[0], b.p[0]);
        assert_eq!(a.p[2], b.p[2]);
        assert_ne!(a.p[1], b.p[1]);
        assert_ne!(a.p[3], b.p[3]);
        let footing = (Vec3::from_array(b.p[0]) + Vec3::from_array(b.p[2])) * 0.5;
        assert_eq!(footing, source.rows[0].position);
    }

    #[test]
    fn replay_flashes_seek_both_directions_and_flashback_drops_the_future() {
        let event = |born| FlashEvent {
            born,
            anchor: super::super::flash_fixture::Anchor {
                source_index: 0,
                source_cell: 0,
                position: [0.; 3],
            },
        };
        let mut flashes = Flashes {
            segment: 0,
            next: 1.5,
            last_live_time: 1.4,
            random: 1,
            history: [event(1.0), event(1.1), event(1.4)].into(),
        };
        assert_eq!(flashes.visible_at(1.12), 0..2);
        assert_eq!(flashes.visible_at(1.41), 2..3);
        assert_eq!(flashes.visible_at(1.03), 0..1);
        assert_eq!(flashes.visible_at(1.25), 1..2);
        flashes.cut(1.12);
        assert_eq!(flashes.history.len(), 2);
        assert!(flashes.visible_at(1.41).is_empty());
        assert_eq!(flashes.visible_at(1.12), 0..2);
    }

    #[test]
    #[ignore = "requires purchased GRID_GAME_ROOT, CPU only"]
    fn purchased_three_regions_prepare_standing_crowds_and_flash_sources() {
        let root = std::path::PathBuf::from(
            std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required"),
        );
        for (route, region, total, drawn) in [
            ("tracks/europe/donington/route_0", "europe", 16189, 16165),
            ("tracks/usa/detroit/route_8", "usa", 6854, 6849),
            ("tracks/japan/shibuya/route_0", "japan", 15422, 15421),
        ] {
            let route = Route::new(route).unwrap();
            let p = prepare(&root, &route).unwrap();
            assert_eq!(p.region, region);
            assert_eq!(p.total, total);
            assert_eq!(p.rows.len(), drawn);
            assert!(
                p.actors
                    .iter()
                    .flatten()
                    .any(near::Baked::has_animated_action),
                "{region}: no distinct source action reached the actor consumer"
            );
            assert!(p.diffuse.starts_with(b"DDS "));
            assert!(p.tint.starts_with(b"DDS "));
            assert!(
                p.flash_dds
                    .as_deref()
                    .is_some_and(|d| d.starts_with(b"DDS "))
            );
            let eligible = p
                .flash_cells
                .iter()
                .map(|cell| cell.eligible.len())
                .sum::<usize>();
            assert!(eligible > 0 && eligible <= drawn);
            for (index, cell) in p.flash_cells.iter().enumerate() {
                assert!(cell.eligible.len() <= 300);
                for &row in &cell.eligible {
                    assert_eq!(p.rows[row].cell as usize, index);
                    assert!(p.rows[row].has_camera);
                }
            }
        }
    }
}
