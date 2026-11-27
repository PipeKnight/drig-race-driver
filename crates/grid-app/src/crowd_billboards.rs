//! Source population from each route, batched by occupied cell and billboard material.
#[path = "crowd_near.rs"]
mod near;
#[path = "crowd_near/flag_render.rs"]
mod flag_render;
use near::NEAR_FRAME_VERTEX_LIMIT;
use super::{
    crowd_population as population,
    crowd_preview::{self, Batch, BillboardLight, CrowdMaterial, Spec, Tint},
    effective_lighting::{EffectiveLighting, PackedLighting},
    le_mans_event::LeMansEvent,
    timeline_lighting::TimelineLighting,
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
    crowd_placements::{self, MovingActor, Placements, Standing},
    material, pssg, texture,
};
use grid_formats::track_lighting::Sun;
use grid_tools::{environment::Environment, route::Route, time_of_day::Mode};
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
    sun_visibility: f32,
}
#[derive(Debug)]
struct Omitted {
    group: Vec<u8>,
    records: u32,
    moving: bool,
    reason: String,
}
struct ActorModel {
    body: near::Baked,
    colours: [Vec4; 3],
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
    actors: Vec<Option<ActorModel>>,
    walkers: Vec<Walker>,
    skin_only: Vec<Row>,
    flags: Option<flag_render::Prepared>,
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
    Ok(Some(Spec {
        uv,
        dimensions,
        colours: source_colours(variant)?,
        instance_scale: crowd_preview::source_scale_fixture(h, w),
    }))
}

fn source_colours(variant: &population::Variant) -> AnyResult<[Vec4; 3]> {
    let colours = &variant
        .colour_sets
        .first()
        .ok_or("source colour sets absent")?
        .colours;
    if colours.len() != 3 {
        return Err("crowd material needs three saved tint colours".into());
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
    Ok(rgb)
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
            sun_visibility: 1.,
        });
    }
    Ok(true)
}
fn atlas(root: &Path, relative: &str) -> AnyResult<(Vec<u8>, Vec<u8>)> {
    let bytes = std::fs::read(grid_tools::install::resolve_case(root, relative))?;
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
    Ok((
        super::texture_upload::crowd_billboard_dds(diffuse),
        super::texture_upload::crowd_billboard_dds(tint),
    ))
}
#[cfg(test)]
fn prepare(root: &Path, route: &Route) -> AnyResult<Prepared> {
    prepare_scene(root, route, false)
}

fn prepare_scene(root: &Path, route: &Route, flags_enabled: bool) -> AnyResult<Prepared> {
    let graph = grid_tools::tyre_smoke::read_xml(
        &root.join(route.track_file("organism_track_dataset.xml")),
    )?;
    let dataset = population::dataset(&graph)?;
    let (user, region) = context(&dataset.pssg)?;
    let shared = grid_tools::tyre_smoke::read_xml(&root.join("anims/organism_static_dataset.xml"))?;
    let raw = std::fs::read(grid_tools::install::resolve_case(root, &shared_resource(&shared, "xml", "typeDefs")?))?;
    let defs = binxml::read(&raw)?;
    let bytes = std::fs::read(root.join(route.file("crowd_standing.bin")))?;
    let (source, moving) = match crowd_placements::read_with_paths(&bytes) {
        Ok(source) => (source.crowd, source.moving),
        Err(error) => {
            let standing = crowd_placements::read(&bytes)?;
            eprintln!("crowd moving paths ignored on {}: {error}", route.path());
            (standing, Vec::new())
        }
    };
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
        .and_then(|resource| std::fs::read(grid_tools::install::resolve_case(root, &resource.relative_path)).ok());
    let animation_bank = animation_bank_bytes
        .as_deref()
        .and_then(|bytes| pssg::read_tree(bytes).ok());
    let selector_bytes = shared_resource(&shared, "binary", "hla")
        .ok()
        .and_then(|path| std::fs::read(grid_tools::install::resolve_case(root, &path)).ok());
    let selector = selector_bytes
        .as_deref()
        .and_then(|bytes| binxml::read(bytes).ok());
    let library = selector
        .as_ref()
        .and_then(|doc| grid_tools::crowd_selection::Library::parse(doc).ok());
    let mut omitted = Vec::new();
    // Reserve the shared sample budget for paths first. Stationary actors can
    // fall back to a billboard where a walker has no authored impostor.
    let moving_groups = moving.len();
    let mut uncarded_models = Vec::new();
    let mut walkers = Vec::new();
    for (group, path_group) in metadata.groups.iter().zip(moving) {
        let group = group.source;
        let model = (|| -> AnyResult<_> {
            let (library, bank) = library
                .as_ref()
                .zip(animation_bank.as_ref())
                .ok_or("missing walker animation library")?;
            let templates = metadata.moving_templates(group)?;
            let template = templates.first().ok_or("missing moving actor map")?;
            let rate = metadata.moving_time_scale(template)?;
            let colours = source_colours(
                template
                    .variants
                    .first()
                    .ok_or("missing walking skin palette")?,
            )?;
            let body = near::bake(
                root,
                template,
                &metadata,
                library,
                bank,
                near::Motion::Walking,
            )?
            .ok_or("missing animated walking skin")?;
            if near_vertex_count + body.frame_vertices() > NEAR_FRAME_VERTEX_LIMIT {
                return Err("walking skin exceeds shared sample-vertex budget".into());
            }
            Ok((ActorModel { body, colours }, rate))
        })();
        match model {
            Ok((model, rate)) => {
                near_vertex_count += model.body.frame_vertices();
                let index = uncarded_models.len();
                uncarded_models.push(model);
                walkers.extend(
                    path_group
                        .actors
                        .into_iter()
                        .map(|path| Walker::new(path, index, rate)),
                );
            }
            Err(error) => omitted.push(Omitted {
                group: path_group.instance_set.to_vec(),
                records: group.count,
                moving: true,
                reason: error.to_string(),
            }),
        }
    }
    // The same near-body budget covers the authored skins that have no
    // fallback atlas entry. Prepare them before optional billboard upgrades.
    let mut skin_only = Vec::new();
    let mut prepared_skin_groups = std::collections::BTreeSet::new();
    for (index, g) in metadata.groups.iter().enumerate() {
        let (Placements::Standing(points), population::Resolution::StaticTemplates(templates)) =
            (&g.source.placements, &g.resolution)
        else {
            continue;
        };
        let [template, ..] = templates.as_slice() else {
            continue;
        };
        let Some(variant) = template.variants.first() else {
            continue;
        };
        if variant.billboard_stance.is_some() {
            continue;
        }
        let model = (|| -> AnyResult<Option<ActorModel>> {
            if !metadata.unpropped_standing_skin(template)? {
                return Ok(None);
            }
            let (library, bank) = library
                .as_ref()
                .zip(animation_bank.as_ref())
                .ok_or("missing standing skin animation library")?;
            let colours = source_colours(variant)?;
            let body = near::bake(
                root,
                template,
                &metadata,
                library,
                bank,
                near::Motion::StandingIdle,
            )?
            .ok_or("missing authored standing skin pose")?;
            if near_vertex_count + body.frame_vertices() > NEAR_FRAME_VERTEX_LIMIT {
                return Err("standing skin exceeds shared sample-vertex budget".into());
            }
            Ok(Some(ActorModel { body, colours }))
        })();
        match model {
            Ok(Some(model)) => {
                let material = uncarded_models.len();
                near_vertex_count += model.body.frame_vertices();
                uncarded_models.push(model);
                skin_only.extend(points.iter().enumerate().map(|(ordinal, point)| Row {
                    position: Vec3::from_array(point.position),
                    orientation: point.orientation_pair,
                    opaque: point.opaque_scalar,
                    cell: point.cell,
                    source: [index, ordinal],
                    material,
                    has_camera: false,
                    sun_visibility: 1.,
                }));
                prepared_skin_groups.insert(index);
            }
            Ok(None) => {}
            Err(error) => eprintln!("crowd skin for {}: {error}", template.map_name),
        }
    }
    let flags = if flags_enabled {
        let candidate = (|| -> AnyResult<Option<flag_render::Prepared>> {
            let (Some(library), Some(animation_bank)) = (&library, &animation_bank) else {
                return Ok(None);
            };
            let Some(bank) = near::flag_bank::prepare(
                root, &metadata, library, animation_bank, near_vertex_count,
            )? else {
                return Ok(None);
            };
            let props = grid_tools::install::resolve_case(root, &shared_resource(&shared, "pssg", "props")?);
            let definitions = grid_tools::install::resolve_case(root, &shared_resource(&shared, "binary", "props")?);
            let rods = near::flag_rod::Rods::read(
                &std::fs::read(props)?, &std::fs::read(definitions)?,
            )?;
            Ok(Some(flag_render::Prepared::new(bank, rods)?))
        })();
        match candidate {
            Ok(Some(flags)) => {
                near_vertex_count = flags.total_vertices();
                prepared_skin_groups.extend(flags.source_groups());
                Some(flags)
            }
            Ok(None) => None,
            Err(error) => {
                eprintln!("crowd flag officials unavailable on {}: {error}", route.path());
                None
            }
        }
    } else {
        None
    };
    for (index, g) in metadata.groups.iter().enumerate() {
        if prepared_skin_groups.contains(&index) {
            continue;
        }
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
                        match near::bake(
                            root,
                            template,
                            &metadata,
                            library,
                            bank,
                            near::Motion::Standing,
                        ) {
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
                    actors.push(actor.map(|body| ActorModel {
                        body,
                        colours: specs[specs.len() - 1].colours,
                    }));
                    continue;
                }
                (false,"selected source variant has no declared billboard stance; skinned/other role remains undrawn".to_owned())
            }
            (Placements::Moving(_), population::Resolution::UnsupportedMoving { .. }) => {
                if index < moving_groups {
                    continue;
                }
                (
                    true,
                    "moving path decoder did not admit route payload".into(),
                )
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
    for walker in &mut walkers {
        walker.model += actors.len();
    }
    for row in &mut skin_only {
        row.material += actors.len();
    }
    actors.extend(uncarded_models.into_iter().map(Some));
    if rows.len() as u64
        + walkers.len() as u64
        + skin_only.len() as u64
        + flags.as_ref().map_or(0, |flags| flags.rows.len() as u64)
        + omitted.iter().map(|o| u64::from(o.records)).sum::<u64>()
        != u64::from(source.total_instances)
    {
        return Err("billboard source accounting differs".into());
    }
    let (diffuse, tint) = atlas(root, &resource.relative_path)?;
    let flash_dds = if let Some(resource) = metadata.dataset_resources.get("camFlash")
        && rows.iter().any(|row| row.has_camera)
    {
        match super::flash_preview::source_dds(&grid_tools::install::resolve_case(root, &resource.relative_path)) {
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
        walkers,
        skin_only,
        flags,
    })
}
fn install(app: &mut App, prepared: Prepared) -> AnyResult<()> {
    println!(
        "crowd_population_static region={} atlas={} source={} billboards={} unsupported_static={} unsupported_moving={} choice=SourceOrderFirst(map,variant,colour) scale_fixture_draw=5000 orientation=camera-facing animation=upper-edge-sway native_rng=false native_lod=false",
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
    println!(
        "crowd_population_moving walkers={} clock=race-or-replay",
        prepared.walkers.len()
    );
    println!(
        "crowd_population_standing_skins={}",
        prepared.skin_only.len()
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
    if prepared.rows.is_empty() && prepared.walkers.is_empty()
        && prepared.skin_only.is_empty() && prepared.flags.is_none()
    {
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
                .after(crate::replay::frame_camera)
                .after(crate::ground_effect_assets::validation_camera),
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
    (mut meshes, mut images): (ResMut<Assets<Mesh>>, ResMut<Assets<Image>>),
    mut materials: ResMut<Assets<CrowdMaterial>>,
    (settings, source_light, adapter): (
        Res<super::graphics::Settings>,
        Option<Res<CardLighting>>,
        Option<Res<bevy::render::renderer::RenderAdapterInfo>>,
    ),
    (mut replay, mut rod_materials): (
        Option<ResMut<super::replay::Replay>>,
        Option<ResMut<Assets<StandardMaterial>>>,
    ),
) {
    let Ok((camera, msaa)) = view.single() else {
        return;
    };
    let source_sample_coverage = source_light.is_some()
        && adapter.is_some_and(|info| info.backend == wgpu::Backend::Vulkan);
    if source_sample_coverage && matches!(msaa, Msaa::Sample2 | Msaa::Sample4) {
        println!("crowd_source_coverage samples={}", msaa.samples());
    }

    let diffuse_upload = if source_light.is_some() {
        super::texture_upload::image_linear
    } else {
        super::texture_upload::image
    };
    let d = images.add(diffuse_upload(
        &prepared.diffuse, settings.sampler(false), RenderAssetUsages::default(),
    ).expect("validated billboard diffuse"));
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
                    billboard_light: source_light.as_deref()
                        .map_or(BillboardLight::default(), |light| light.initial),
                    source_sample_coverage,
                },
            })
        })
        .collect();
    if source_light.as_ref().is_some_and(|light| light.timeline) {
        commands.insert_resource(TimelineCards { handles: handles.clone() });
    }
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
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, batch.light_uv);
        mesh.insert_indices(Indices::U32(batch.indices));
        commands.spawn((
            Name::new(format!("crowd/material{material}/cell{cell}")),
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(handles[material].clone()),
            Transform::IDENTITY,
            Aabb::from_min_max(min, max),
            // These bounds hold for every card direction; the streamed
            // vertices never update the asset's own (F854).
            bevy::camera::visibility::NoAutoAabb,
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
        .map(|actor| {
            let ActorModel { body, colours } = actor.as_ref()?;
            let diffuse = super::texture_upload::image(
                &body.diffuse,
                settings.sampler(false),
                RenderAssetUsages::default(),
            )
            .ok()?;
            let tint = super::texture_upload::image_linear(
                &body.tint,
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
                    colour0: colours[0],
                    colour1: colours[1],
                    colour2: colours[2],
                    billboard_light: BillboardLight::default(),
                    source_sample_coverage: false,
                },
            });
            Some(NearModel {
                frames: body.meshes(&mut meshes),
                foot_offset: body.foot_offset,
                material,
            })
        })
        .collect::<Vec<_>>();
    println!(
        "crowd_animated_templates={}",
        models.iter().flatten().count()
    );
    commands.insert_resource(NearModels(models));
    if let (Some(flags), Some(replay), Some(rod_materials)) = (
        &prepared.flags, replay.as_deref_mut(), rod_materials.as_deref_mut(),
    ) {
        match flags.install(
            replay, &settings, &mut meshes, &mut images, &mut materials, rod_materials,
        ) {
            Ok(installed) => {
                commands.insert_resource(installed);
            }
            Err(error) => eprintln!("crowd flag asset installation failed: {error}"),
        }
    }
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
    tick: u64,
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
    clock.tick = drive.runtime.ticks;
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
        let row = &source.rows[index];
        let position = row.position;
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
        batch.light_uv.extend([[row.sun_visibility, 0.]; 4]);
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
const NATIVE_PATH_KEYS: usize = 100;

struct Walker {
    path: MovingActor,
    model: usize,
    rate: f32,
    period: f64,
}

impl Walker {
    fn new(mut path: MovingActor, model: usize, rate: f32) -> Self {
        // 0x769D50 copies up to 100 keys on each channel before 0x986960 sorts them.
        path.positions.truncate(NATIVE_PATH_KEYS);
        path.yaws.truncate(NATIVE_PATH_KEYS);
        path.positions
            .sort_by(|a, b| a.seconds.total_cmp(&b.seconds));
        path.yaws.sort_by(|a, b| a.seconds.total_cmp(&b.seconds));
        let period = f64::from(
            path.positions
                .last()
                .unwrap()
                .seconds
                .max(path.yaws.last().unwrap().seconds),
        );
        Self {
            path,
            model,
            rate,
            period,
        }
    }

    fn pose(&self, seconds: f64) -> (Vec3, f32) {
        // A global race/replay clock keeps the port deterministic after seeking.
        // Original paths integrate the deltas of their admitted culling updates.
        let time = seconds * f64::from(self.rate);
        let wrapped = if self.period <= 0. {
            0.
        } else if (0. ..=self.period).contains(&time) {
            time
        } else {
            let wrapped = time.rem_euclid(self.period);
            if wrapped == 0. && time > 0. {
                self.period
            } else {
                wrapped
            }
        } as f32;
        let (a, b, t) = path_span(&self.path.positions, wrapped, |key| key.seconds);
        let position = Vec3::from_array(a.position).lerp(Vec3::from_array(b.position), t);
        let (a, b, t) = path_span(&self.path.yaws, wrapped, |key| key.seconds);
        let mut turn = b.degrees - a.degrees;
        if turn > 180. {
            turn -= 360.;
        } else if turn < -180. {
            turn += 360.;
        }
        (position, a.degrees + turn * t)
    }
}

fn path_span<T>(keys: &[T], seconds: f32, time: impl Fn(&T) -> f32) -> (&T, &T, f32) {
    let after = keys.partition_point(|key| time(key) <= seconds);
    let left = &keys[after.saturating_sub(1)];
    let right = keys.get(after).unwrap_or(left);
    let interval = time(right) - time(left);
    let progress = if interval >= 1e-5 {
        (seconds - time(left)) / interval
    } else {
        0.
    };
    (left, right, progress)
}

#[derive(Resource)]
struct NearModels(Vec<Option<NearModel>>);

struct NearModel {
    frames: near::PoseMeshes,
    foot_offset: Vec3,
    material: Handle<CrowdMaterial>,
}

#[derive(Resource, Default)]
struct NearRuntime {
    bodies: BTreeMap<usize, Entity>,
    flags: BTreeMap<usize, flag_render::Live>,
}

fn update_actors(
    mut commands: Commands,
    (prepared, clock): (Res<Prepared>, Res<CrowdClock>),
    models: Option<Res<NearModels>>,
    (flag_models, mut replay): (
        Option<Res<flag_render::Installed>>,
        Option<ResMut<super::replay::Replay>>,
    ),
    mut live: ResMut<NearRuntime>,
    (view, cells): (
        Single<&Transform, With<super::ViewerCamera>>,
        Query<&CardCell>,
    ),
    mut actors: Query<(&mut Mesh3d, &mut Transform), Without<super::ViewerCamera>>,
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
    let first_walker = prepared.rows.len();
    let walker_poses: Vec<_> = prepared
        .walkers
        .iter()
        .map(|walker| walker.pose(clock.seconds))
        .collect();
    for (index, (walker, &(position, _))) in prepared.walkers.iter().zip(&walker_poses).enumerate()
    {
        let distance = eye.distance_squared(position);
        if distance <= ACTIVE_RANGE * ACTIVE_RANGE && models.0[walker.model].is_some() {
            candidates.push((distance, first_walker + index));
        }
    }
    let first_skin = first_walker + prepared.walkers.len();
    for (index, row) in prepared.skin_only.iter().enumerate() {
        let distance = eye.distance_squared(row.position);
        if distance <= ACTIVE_RANGE * ACTIVE_RANGE && models.0[row.material].is_some() {
            candidates.push((distance, first_skin + index));
        }
    }
    let first_flag = first_skin + prepared.skin_only.len();
    let flag_scene = prepared.flags.as_ref().zip(flag_models.as_deref());
    if let Some((flags, _)) = flag_scene
        && replay.is_some()
    {
        for (row, flag) in flags.rows.iter().enumerate() {
            let distance = eye.distance_squared(flag.position);
            if distance <= ACTIVE_RANGE * ACTIVE_RANGE {
                candidates.push((distance, first_flag + row));
            }
        }
    }
    candidates.sort_unstable_by(|(a, i), (b, j)| a.total_cmp(b).then(i.cmp(j)));
    candidates.truncate(NEAR_LIMIT);
    let selected = candidates
        .iter()
        .map(|(_, row)| *row)
        .collect::<std::collections::BTreeSet<_>>();
    live.bodies.retain(|row, entity| {
        if selected.contains(row) {
            true
        } else {
            commands.entity(*entity).despawn();
            false
        }
    });
    live.flags.retain(|row, pair| {
        if selected.contains(row) {
            true
        } else {
            commands.entity(pair.body).despawn();
            false
        }
    });
    if let (Some((flags, _)), Some(replay)) = (flag_scene, replay.as_deref_mut()) {
        flags.publish(
            replay, clock.segment, clock.tick,
            selected.iter().filter_map(|index| index.checked_sub(first_flag)),
        );
    }
    let mut flag_choices = BTreeMap::new();
    for (_, index) in candidates {
        if let Some(row) = index.checked_sub(first_flag) {
            let (Some((flags, installed)), Some(replay)) = (flag_scene, replay.as_deref()) else {
                continue;
            };
            let group = flags.group(row);
            let choice = flag_choices.entry(group).or_insert_with(|| flags.choice(replay, group));
            let Some(pose) = (*choice).and_then(|choice| flags.pose(installed, row, choice)) else {
                continue;
            };
            if let Some(pair) = live.flags.get(&index) {
                if let Ok([(mut body, _), (_, mut rod)]) = actors.get_many_mut([pair.body, pair.rod]) {
                    if body.id() != pose.mesh.id() {
                        body.0 = pose.mesh.clone();
                    }
                    if *rod != pose.hand {
                        *rod = pose.hand;
                    }
                }
            } else {
                let pair = flags.spawn(installed, &mut commands, &pose, clock.tick);
                live.flags.insert(index, pair);
            }
            continue;
        }
        let skin = index
            .checked_sub(first_skin)
            .map(|index| &prepared.skin_only[index]);
        let walker = (first_walker..first_skin)
            .contains(&index)
            .then(|| index - first_walker);
        let (model, position, yaw) = if let Some(walker) = walker {
            let (position, degrees) = walker_poses[walker];
            (
                prepared.walkers[walker].model,
                position,
                Quat::from_rotation_y(degrees.to_radians()),
            )
        } else {
            let row = skin.unwrap_or_else(|| &prepared.rows[index]);
            // The saved two-vector represents actor local X in world XZ.
            let [x, z] = row.orientation;
            (
                row.material,
                row.position,
                Quat::from_rotation_y((-z).atan2(x)),
            )
        };
        let Some(model) = &models.0[model] else {
            continue;
        };
        let unit_phase = f64::from(phase(index) / std::f32::consts::TAU);
        let handle = if let Some(walker) = walker {
            // Start the selected clip at its first key. The global, rate-scaled
            // phase rewinds with the path using the same shown race coordinate.
            model.frames.walking_frame(
                clock.seconds * f64::from(prepared.walkers[walker].rate),
                unit_phase,
            )
        } else {
            model.frames.frame(clock.seconds, unit_phase)
        };
        let transform =
            Transform::from_translation(position + yaw * model.foot_offset).with_rotation(yaw);
        if let Some(&entity) = live.bodies.get(&index) {
            if let Ok((mut mesh, mut current)) = actors.get_mut(entity) {
                if mesh.id() != handle.id() {
                    mesh.0 = handle.clone();
                }
                if walker.is_some() && *current != transform {
                    *current = transform;
                }
            }
        } else {
            if let Some(walker) = walker {
                println!(
                    "crowd_walker_visible index={walker} source_cell={} seconds={:.3} position={position:?} yaw={:.3}",
                    prepared.walkers[walker].path.cell, clock.seconds, walker_poses[walker].1
                );
            }
            let entity = commands
                .spawn((
                    Name::new(if walker.is_some() {
                        "crowd/source-walk"
                    } else if skin.is_some() {
                        "crowd/source-standing-skin"
                    } else {
                        "crowd/source-animation"
                    }),
                    Mesh3d(handle.clone()),
                    MeshMaterial3d(model.material.clone()),
                    transform,
                    bevy::camera::visibility::NoFrustumCulling,
                    NotShadowCaster,
                ))
                .id();
            live.bodies.insert(index, entity);
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
    stream: Option<Res<super::mesh_stream::MeshStream>>,
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
                    .filter(|row| live.bodies.contains_key(row))
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
            .filter(|row| !live.bodies.contains_key(row))
            .collect();
        if visible.is_empty() {
            *visibility = Visibility::Hidden;
        } else {
            let Ok(mut batch) = make_batch(
                &prepared,
                &visible,
                &prepared.specs[cell.material],
                eye,
                seconds,
            ) else {
                continue;
            };
            // Cards a live body replaces become zero-area padding, so the
            // asset keeps one card per row and its vertices are streamed:
            // a replaced or modified asset makes Bevy revisit every mesh
            // entity that frame (F854).
            pad_cards(&mut batch, cell.rows.len());
            *visibility = Visibility::Inherited;
            // The cards face the camera, so their corners change with every
            // move of the eye. Same cards: their bytes go over the mesh's GPU
            // allocation (mesh_stream), as replacing the asset would upload
            // and repack the whole mesh every frame.
            let same = meshes
                .get(&mesh.0)
                .is_some_and(|m| m.count_vertices() == batch.p.len());
            match (same, stream.as_deref()) {
                (true, Some(stream)) => {
                    let layout = meshes.get(&mesh.0).map(super::mesh_stream::VertexLayout::of);
                    if let Some(layout) = layout {
                        stream.write(mesh.0.id(), |bytes| pack_cards(&batch, &layout, bytes));
                    }
                }
                _ => {
                    if let Some(mut mesh) = meshes.get_mut(&mesh.0) {
                        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, batch.p);
                        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, batch.n);
                        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, batch.uv);
                        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, batch.light_uv);
                        if !same {
                            mesh.insert_indices(Indices::U32(batch.indices));
                        }
                    }
                }
            }
        }
        cell.omitted = omitted;
        cell.last_frame = frame;
        cell.last_eye = eye;
    }
}

/// Pads `batch` to `cards` cards with zero-area cards at the origin, which
/// draw nothing, in the cards' own index pattern.
fn pad_cards(batch: &mut Batch, cards: usize) {
    while batch.p.len() < cards * 4 {
        let first = batch.p.len() as u32;
        batch.p.extend([[0.; 3]; 4]);
        batch.n.extend([[0., 1., 0.]; 4]);
        batch.uv.extend([[0.; 2]; 4]);
        batch.light_uv.extend([[0.; 2]; 4]);
        batch
            .indices
            .extend([first, first + 1, first + 2, first + 3, first + 2, first + 1]);
    }
}

/// A card batch packed as `Mesh::write_packed_vertex_buffer_data` packs the
/// cell's asset (POSITION, NORMAL, UV_0, UV_1 in `layout`'s order).
fn pack_cards(batch: &Batch, layout: &super::mesh_stream::VertexLayout, bytes: &mut Vec<u8>) {
    use super::mesh_stream::put;
    let at = |attribute| layout.offset(attribute).expect("card attribute");
    let (p, n, uv, light) = (
        at(Mesh::ATTRIBUTE_POSITION),
        at(Mesh::ATTRIBUTE_NORMAL),
        at(Mesh::ATTRIBUTE_UV_0),
        at(Mesh::ATTRIBUTE_UV_1),
    );
    let start = bytes.len();
    bytes.resize(start + batch.p.len() * layout.stride, 0);
    for (k, vertex) in (start..bytes.len()).step_by(layout.stride).enumerate() {
        put(bytes, vertex + p, batch.p[k]);
        put(bytes, vertex + n, batch.n[k]);
        put(bytes, vertex + uv, batch.uv[k]);
        put(bytes, vertex + light, batch.light_uv[k]);
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

#[derive(Clone, Copy)]
struct FlashCheckpoint {
    seconds: f64,
    next: f64,
    random: u32,
}

const FLASH_FIRST_SECONDS: f64 = 0.1;
const FLASH_INITIAL_RANDOM: u32 = 1;
const FLASH_HISTORY_LIMIT: usize = 65_536;

#[derive(Resource)]
struct Flashes {
    segment: u64,
    next: f64,
    last_live_time: f64,
    random: u32,
    history: VecDeque<FlashEvent>,
    schedule: VecDeque<FlashCheckpoint>,
}

/// A flash card's slot and its streamed quad, which changes every frame it
/// shows (F854).
#[derive(Component)]
struct FlashCard(usize, super::mesh_stream::SoupMesh);

impl Flashes {
    fn visible_at(&self, seconds: f64) -> std::ops::Range<usize> {
        let first = self
            .history
            .partition_point(|event| event.born + FLASH_LIFETIME <= seconds);
        let last = self.history.partition_point(|event| event.born <= seconds);
        first..last
    }

    fn remember_schedule(&mut self, seconds: f64) {
        if self.schedule.len() == FLASH_HISTORY_LIMIT {
            self.schedule.pop_front();
        }
        self.schedule.push_back(FlashCheckpoint {
            seconds,
            next: self.next,
            random: self.random,
        });
    }

    fn cut(&mut self, seconds: f64) {
        let retained = self.history.partition_point(|event| event.born <= seconds);
        self.history.truncate(retained);
        let retained = self.schedule.partition_point(|step| step.seconds <= seconds);
        self.schedule.truncate(retained);
        (self.next, self.random) = self
            .schedule
            .back()
            .map_or((FLASH_FIRST_SECONDS, FLASH_INITIAL_RANDOM), |step| {
                (step.next, step.random)
            });
    }
}

fn install_flash(
    mut commands: Commands,
    prepared: Res<Prepared>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<super::flash_preview::FlashMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
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
        next: FLASH_FIRST_SECONDS,
        last_live_time: 0.,
        random: FLASH_INITIAL_RANDOM,
        history: VecDeque::new(),
        schedule: VecDeque::new(),
    });
    for index in 0..10 {
        commands.spawn((
            Name::new("crowd/source-camera-flash"),
            FlashCard(index, default()),
            // Its own asset (the pulse rewrites it in place) with the flash's
            // layout, so the warm-up builds the flash pipelines under the cover.
            Mesh3d(meshes.add(super::flash_preview::idle_mesh())),
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
// intervals .5–.1 and .15 lifetime. The flash job receives the full vertical
// field of view before scene projection halves it (0x0063A819–0x0063A84D).
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
        (Entity, &mut FlashCard, &mut Transform, &mut Visibility, &mut Mesh3d),
        Without<super::ViewerCamera>,
    >,
    mut meshes: ResMut<Assets<Mesh>>,
    stream: Option<Res<super::mesh_stream::MeshStream>>,
) {
    let Some(flashes) = flashes.as_deref_mut() else {
        return;
    };
    let (camera, projection) = *view;
    let eye = camera.translation;
    let now = clock.seconds;
    if flashes.segment != clock.segment {
        flashes.segment = clock.segment;
        flashes.next = FLASH_FIRST_SECONDS;
        flashes.last_live_time = 0.;
        flashes.random = FLASH_INITIAL_RANDOM;
        flashes.history.clear();
        flashes.schedule.clear();
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
            Projection::Perspective(perspective)
                if perspective.fov > 0. && perspective.fov < std::f32::consts::PI =>
            {
                perspective.fov
            }
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
                if flashes.history.len() == FLASH_HISTORY_LIMIT {
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
        // Searching can consume random choices even when this attempt makes
        // no flash. Flashback restores the scheduler at the selected time.
        flashes.remember_schedule(now);
    }

    let active = flashes.visible_at(now);
    let up = (*camera.up()).to_array();
    for (entity, mut card, mut transform, mut visibility, mut handle) in &mut cards {
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
            super::flash_preview::event_mesh(event.anchor, eye.to_array(), up, phase)
        {
            if let Some(added) =
                card.1
                    .update(mesh, &handle.0, entity, &mut meshes, stream.as_deref())
            {
                handle.0 = added;
            }
            transform.translation = center;
            *visibility = Visibility::Visible;
        }
    }
}
/// Called by ordinary rendered driving. Routes with no standing population
/// need no crowd material registration or asset lookup.
#[derive(Resource)]
struct CardLighting {
    initial: BillboardLight,
    timeline: bool,
}

#[derive(Resource)]
struct TimelineCards {
    handles: Vec<Handle<CrowdMaterial>>,
}

fn set_card_sun(
    mut light: BillboardLight,
    rgb: EffectiveLighting,
    angles: Sun,
) -> BillboardLight {
    light.sun = Vec3::from_array(rgb.sun).extend(light.sun.w);
    light.back = Vec3::from_array(rgb.back).extend(1.);
    light.ambient = Vec3::from_array(rgb.ambient).extend(light.ambient.w);
    light.direction = Vec3::from_array(angles.direction_to_light()).extend(light.direction.w);
    light
}

fn install_card_lighting(
    app: &mut App,
    root: &Path,
    route: &Route,
    prepared: &mut Prepared,
) -> AnyResult<()> {
    let environment = Environment::for_route(root, route)?;
    let raw = &environment.lighting;
    let [height_degrees, rotation_degrees] = raw.sun().ok_or("crowd lighting sun angles missing")?;
    let angles = Sun { height_degrees, rotation_degrees };
    let (_, rgb) = PackedLighting::from_static(raw)?.resolve(Mode::None, None, 0.)?;
    let fog = Vec3::from_array(raw.rgb("fogColour").unwrap_or([0.; 3]))
        * raw.scalar("fogColourBrightness").unwrap_or(1.);
    let defaults = BillboardLight {
        sun: Vec3::ZERO.extend(raw.scalar("shadow_strength").unwrap_or(1.)),
        ambient: Vec3::ZERO.extend(raw.scalar("fogDistance").unwrap_or(0.)),
        direction: Vec3::ZERO.extend(raw.scalar("fogDensityCap").unwrap_or(0.)),
        fog: fog.extend(0.),
        ..default()
    };
    let timeline = environment.mode() == Mode::Timeline;
    let initial = if timeline {
        let source = app.world().get_resource::<TimelineLighting>()
            .ok_or("crowd lighting timeline unavailable")?;
        let (rgb, lights) = source.sample(super::sky::static_start())?;
        set_card_sun(defaults, rgb, lights.sun_direction.unwrap_or(angles))
    } else {
        set_card_sun(defaults, rgb, angles)
    };
    if !initial.direction.is_finite() || !initial.fog.is_finite() {
        return Err("nonfinite crowd card lighting".into());
    }
    let path = grid_tools::install::resolve_case(root, &route.file("route.clm"));
    let bytes = super::read(&path)?;
    let grid = grid_formats::shadow_grid::read(&bytes)?;
    let mut shaded = 0;
    for row in &mut prepared.rows {
        if grid.shaded(row.position.x, row.position.z) {
            row.sun_visibility = 0.;
            shaded += 1;
        }
    }
    println!(
        "crowd_card_light source=route.clm shaded={shaded} cards={} timeline={timeline}",
        prepared.rows.len(),
    );
    app.insert_resource(CardLighting { initial, timeline });
    if timeline {
        app.add_systems(PostUpdate, follow_card_lighting);
    }
    Ok(())
}

fn follow_card_lighting(
    cards: Option<Res<TimelineCards>>,
    source: Res<TimelineLighting>,
    initial: Res<CardLighting>,
    event: Option<Res<LeMansEvent>>,
    mut materials: ResMut<Assets<CrowdMaterial>>,
    mut last_time: Local<Option<f64>>,
) {
    let Some(cards) = cards else { return };
    let time = event.as_deref().map_or_else(super::sky::static_start, |event| event.time_of_day);
    if *last_time == Some(time) {
        return;
    }
    *last_time = Some(time);
    let Ok((rgb, lights)) = source.sample(time) else { return };
    let Some(angles) = lights.sun_direction else { return };
    let mut light = set_card_sun(initial.initial, rgb, angles);
    if let Some(timeline) = event.as_deref().and_then(|event| event.timeline.as_ref()) {
        light.sun.w = timeline.value("shadow_strength", time).unwrap_or(light.sun.w);
    }
    for handle in &cards.handles {
        if materials.get(handle).is_some_and(|mat| mat.extension.billboard_light != light)
            && let Some(mut mat) = materials.get_mut(handle)
        {
            mat.extension.billboard_light = light;
        }
    }
}

pub(crate) fn install_route(app: &mut App, root: &Path, route: &Route) -> AnyResult<()> {
    match std::fs::metadata(root.join(route.file("crowd_standing.bin"))) {
        Ok(_) => {
            let flags = app.world().contains_resource::<super::replay::Replay>();
            let mut prepared = prepare_scene(root, route, flags)?;
            if !prepared.rows.is_empty()
                && let Err(error) = install_card_lighting(app, root, route, &mut prepared)
            {
                eprintln!("crowd card lighting falls back to scene lighting: {error}");
            }
            install(app, prepared)
        }
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
                sun_visibility: 0.,
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
            walkers: Vec::new(),
            skin_only: Vec::new(),
            flags: None,
        };
        let eye = Vec3::new(3., 5., 10.);
        let a = make_batch(&source, &[0], &spec, eye, 0.).unwrap();
        let b = make_batch(&source, &[0], &spec, eye, 0.5).unwrap();
        assert_eq!(a.uv, b.uv);
        assert_eq!(a.light_uv, vec![[0.; 2]; 4]);
        assert_eq!(a.light_uv, b.light_uv);
        assert_eq!(a.indices, b.indices);
        assert_eq!(a.p[0], b.p[0]);
        assert_eq!(a.p[2], b.p[2]);
        assert_ne!(a.p[1], b.p[1]);
        assert_ne!(a.p[3], b.p[3]);
        let footing = (Vec3::from_array(b.p[0]) + Vec3::from_array(b.p[2])) * 0.5;
        assert_eq!(footing, source.rows[0].position);
    }

    #[test]
    fn streamed_cards_equal_the_packed_asset() {
        let batch = Batch {
            p: (0..8).map(|i| [i as f32, 0.5 * i as f32, -1.25]).collect(),
            n: (0..8).map(|i| [0., 1., i as f32 * 0.1]).collect(),
            uv: (0..8).map(|i| [i as f32 * 0.125, 1.]).collect(),
            light_uv: (0..8).map(|i| [f32::from(u8::from(i < 4)), 0.]).collect(),
            indices: vec![0, 1, 2, 3, 2, 1, 4, 5, 6, 7, 6, 5],
        };
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, batch.p.clone());
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, batch.n.clone());
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, batch.uv.clone());
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, batch.light_uv.clone());
        mesh.insert_indices(Indices::U32(batch.indices.clone()));
        let layout = crate::mesh_stream::VertexLayout::of(&mesh);
        let expected = mesh.create_packed_vertex_buffer_data();
        let mut streamed = vec![7];
        pack_cards(&batch, &layout, &mut streamed);
        assert_eq!(streamed[0], 7);
        assert_eq!(&streamed[1..], &expected[..]);
    }

    #[test]
    fn walking_pose_seeks_and_holds_the_native_loop_endpoint() {
        use crowd_placements::{PositionKey, YawKey};

        let actor = Walker::new(
            MovingActor {
                cell: 0,
                positions: (0..101)
                    .map(|i| PositionKey {
                        seconds: i as f32,
                        position: [i as f32, 0., 0.],
                    })
                    .collect(),
                yaws: vec![
                    YawKey {
                        seconds: 2.,
                        degrees: 10.,
                    },
                    YawKey {
                        seconds: 0.,
                        degrees: 350.,
                    },
                ],
            },
            0,
            0.5,
        );
        let at_start = actor.pose(0.);
        let at_turn = actor.pose(2.);
        assert_eq!(at_start, (Vec3::ZERO, 350.));
        assert_eq!(at_turn, (Vec3::X, 360.));
        assert_eq!(actor.pose(198.), (Vec3::X * 99., 10.));
        assert_eq!(actor.pose(396.), (Vec3::X * 99., 10.));
        assert_eq!(actor.pose(200.), at_turn);
        assert_eq!(actor.pose(-2.), (Vec3::X * 98., 10.));
        assert_eq!(actor.pose(0.), at_start);
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
            schedule: VecDeque::new(),
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
    fn flashback_resumes_the_last_retained_attempt_even_if_it_emitted_nothing() {
        let mut flashes = Flashes {
            segment: 0,
            next: FLASH_FIRST_SECONDS,
            last_live_time: 0.,
            random: FLASH_INITIAL_RANDOM,
            history: VecDeque::new(),
            schedule: VecDeque::new(),
        };
        for (now, interval) in [(0.1, 0.3), (0.4, 0.5), (0.9, 0.1)] {
            flash_draw(&mut flashes.random, 21);
            flashes.next = now + interval;
            flashes.remember_schedule(now);
        }
        let retained = flashes.schedule[1];
        assert_eq!(retained.seconds, 0.4);
        let mut expected_random = retained.random;
        let future_choice = flash_draw(&mut expected_random, 21);
        flashes.cut(0.45);
        assert!(flashes.history.is_empty());
        assert_eq!(flashes.schedule.len(), 2);
        assert_eq!(flashes.next, retained.next);
        assert_eq!(flashes.random, retained.random);
        assert_eq!(flash_draw(&mut flashes.random, 21), future_choice);
        flashes.cut(0.05);
        assert!(flashes.schedule.is_empty());
        assert_eq!((flashes.next, flashes.random), (FLASH_FIRST_SECONDS, FLASH_INITIAL_RANDOM));
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
                    .any(|model| model.body.has_animated_action()),
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

    #[test]
    #[ignore = "requires purchased GRID_GAME_ROOT, CPU only"]
    fn purchased_standing_flag_banks_fit_with_existing_actor_reservations() {
        use crate::replay::{flag_demand::Demand, flag_phase::{Phase, State}};
        use grid_tools::crowd_flags::{Cue, Role};
        let root = std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").unwrap());
        let shared = grid_tools::tyre_smoke::read_xml(
            &root.join("anims/organism_static_dataset.xml"),
        ).unwrap();
        let props_path = shared_resource(&shared, "pssg", "props").unwrap();
        let definitions_path = shared_resource(&shared, "binary", "props").unwrap();
        let rods = near::flag_rod::Rods::read(
            &std::fs::read(root.join(props_path)).unwrap(),
            &std::fs::read(root.join(definitions_path)).unwrap(),
        ).unwrap();
        let mut meshes = Assets::<Mesh>::default();
        let mut images = Assets::<Image>::default();
        let mut materials = Assets::<StandardMaterial>::default();
        let rod_draws = rods.install(
            &crate::graphics::Settings::default(), &mut meshes, &mut images, &mut materials,
        ).unwrap();
        assert_eq!((meshes.len(), images.len(), materials.len()), (1, 1, 1));
        for (route, roles) in [
            ("tracks/europe/donington/route_0", 2),
            ("tracks/europe/jarama/route_0", 3),
            ("tracks/europe/milan/route_0", 2),
        ] {
            let route = Route::new(route).unwrap();
            let control = prepare(&root, &route).unwrap();
            let prepared = prepare_scene(&root, &route, true).unwrap();
            let flags = prepared.flags.as_ref().unwrap();
            let bank = flags.bank();
            assert_eq!(control.rows.len(), prepared.rows.len());
            assert_eq!(control.walkers.len(), prepared.walkers.len());
            assert_eq!(control.skin_only.len(), prepared.skin_only.len());
            let actor_reserve: usize = prepared.actors.iter().flatten()
                .map(|actor| actor.body.frame_vertices()).sum();
            let used: usize = bank.clips.values()
                .map(|clip| clip.frames.len() * bank.uv.len()).sum();
            assert!(actor_reserve + used <= NEAR_FRAME_VERTEX_LIMIT);
            assert!(bank.total_vertices <= actor_reserve + used);
            assert_eq!(bank.groups.len(), roles);
            assert_eq!(bank.clips.len(), 1 + roles * 3);

            let mut replay = crate::replay::Replay::new(
                crate::replay::stream::Stream::new(Default::default(), None), None,
            );
            let mut body_materials = Assets::<CrowdMaterial>::default();
            let installed = flags.install(
                &mut replay, &crate::graphics::Settings::default(),
                &mut meshes, &mut images, &mut body_materials, &mut materials,
            ).unwrap();
            assert_eq!(body_materials.len(), roles);
            let idle = flags.choice(&replay, flags.group(0)).unwrap();
            let idle_pose = flags.pose(&installed, 0, idle).unwrap();
            assert_eq!((idle.phase, idle.sample), (Phase::Idle, 0));
            let mut world = World::new();
            let pair = flags.spawn(&installed, &mut world.commands(), &idle_pose, 0);
            world.flush();
            assert_eq!(world.get::<ChildOf>(pair.rod).unwrap().parent(), pair.body);
            assert_eq!(world.get::<Mesh3d>(pair.body).unwrap().id(), idle_pose.mesh.id());
            assert_eq!(*world.get::<Transform>(pair.rod).unwrap(), idle_pose.hand);
            let next = flags.pose(&installed, 0, crate::replay::flag_phase::Frame {
                phase: Phase::Start, sample: 0,
            }).unwrap();
            assert_ne!(idle_pose.mesh.id(), next.mesh.id());
            world.despawn(pair.body);
            assert!(world.get_entity(pair.rod).is_err());
            assert!(bank.modifier.windows(7).any(|w| w == b"mar_LOD"));
            assert!(!bank.diffuse.is_empty() && !bank.tint.is_empty());
            assert!(bank.foot_offset.is_finite());
            assert!(bank.indices.iter().all(|&i| usize::from(i) < bank.uv.len()));
            let mut source_groups = std::collections::BTreeSet::new();
            for group in &bank.groups {
                assert!(source_groups.insert(group.source_group));
                let clips = match group.role {
                    Role::Start => [183, 107, 70, 106],
                    Role::Finish => [183, 103, 71, 72],
                    Role::Danger => [183, 105, 108, 104],
                };
                assert_eq!(group.clips, clips);
                let phase_clips = bank.phase_clips(group).unwrap();
                let demand = Demand::observed(1 << (group.role as u8 - 1)).unwrap();
                let mut state = State::default();
                state.step(&phase_clips, group.role, demand, &[Cue::Begin(group.role)]);
                let choice = state.frame(&phase_clips);
                assert_eq!(choice.phase, Phase::Start);
                let selected = bank.chosen_frame(group, choice).unwrap();
                assert!(std::ptr::eq(selected, &bank.clips[&group.clips[1]].frames[0]));
                let point = &group.placements[0];
                let [x, z] = point.orientation_pair;
                let yaw = Quat::from_rotation_y((-z).atan2(x));
                let body = Transform::from_translation(
                    Vec3::from_array(point.position) + yaw * bank.foot_offset,
                ).with_rotation(yaw);
                let mut world = World::new();
                let actor = world.spawn(body).id();
                let child = world.spawn(rods.child(
                    &rod_draws, group.role, actor, rods.pose(group.role, selected.hand).unwrap(),
                )).id();
                assert_eq!(world.get::<ChildOf>(child).unwrap().parent(), actor);
                for id in group.clips {
                    for frame in &bank.clips[&id].frames {
                        rods.check_pose(group.role, frame.hand, body).unwrap();
                        *world.get_mut::<Transform>(child).unwrap() =
                            rods.pose(group.role, frame.hand).unwrap();
                    }
                }
                world.despawn(actor);
                assert!(world.get_entity(child).is_err());
                assert!(group.colours.iter().all(|colour| colour.is_finite()));
                let expected = if group.role == Role::Danger { 6 } else { 1 };
                assert_eq!(group.placements.len(), expected);
                let idle = &bank.clips[&group.clips[0]];
                assert_eq!(idle.id, "m_np_s1_id_01");
                assert_eq!(idle.frames.len(), 2);
                for ordinal in &group.clips[1..] {
                    let clip = &bank.clips[ordinal];
                    assert!(clip.first.is_finite() && clip.duration > 0.);
                    assert!(clip.frames.iter().all(|frame| {
                        frame.body.positions.len() == bank.uv.len()
                            && frame.hand.iter().all(|value| value.is_finite())
                    }));
                    assert!(clip.frames.windows(2).any(|f| f[0].hand != f[1].hand));
                }
            }
        }
    }

    #[test]
    #[ignore = "requires purchased GRID_GAME_ROOT, CPU only"]
    fn purchased_routes_prepare_off_atlas_standing_officials() {
        let root = std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").unwrap());
        for (route, count, unresolved) in [
            ("tracks/europe/donington/route_0", 20, 0),
            ("tracks/europe/spa_francorchamps/route_0", 12, 10),
            ("tracks/usa/detroit/route_8", 3, 0),
        ] {
            let prepared = prepare(&root, &Route::new(route).unwrap()).unwrap();
            assert_eq!(
                prepared.skin_only.len(),
                count,
                "{route}: {:?}",
                prepared.omitted
            );
            for row in &prepared.skin_only {
                let body = &prepared.actors[row.material].as_ref().unwrap().body;
                assert!(
                    body.has_animated_idle(),
                    "{route}: missing authored marshal idle"
                );
                assert!(
                    !body.has_animated_action(),
                    "{route}: unexpected action fallback"
                );
            }
            assert!(!prepared.omitted.iter().any(|entry| entry.group == b"ma_np"));
            assert_eq!(
                prepared
                    .omitted
                    .iter()
                    .filter(|entry| !entry.moving)
                    .map(|entry| entry.records)
                    .sum::<u32>(),
                2 + unresolved
            );
        }
    }

    #[test]
    #[ignore = "requires purchased GRID_GAME_ROOT, CPU only"]
    fn purchased_two_regions_prepare_walking_spectators_and_marshals() {
        let root = std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").unwrap());
        for (route, spectators, marshals) in [
            ("tracks/europe/milan/route_0", 3, 1),
            ("tracks/japan/yokohama_docks/route_5", 1, 0),
        ] {
            let prepared = prepare(&root, &Route::new(route).unwrap()).unwrap();
            assert_eq!(
                prepared.walkers.len(),
                spectators + marshals,
                "{route}: {:?}",
                prepared.omitted
            );
            assert_eq!(
                prepared
                    .walkers
                    .iter()
                    .filter(|actor| actor.rate == 0.547)
                    .count(),
                spectators
            );
            assert_eq!(
                prepared
                    .walkers
                    .iter()
                    .filter(|actor| actor.rate == 0.33)
                    .count(),
                marshals
            );
            for actor in prepared.walkers {
                let model = prepared.actors[actor.model].as_ref().unwrap();
                assert!(
                    model.body.has_animated_action(),
                    "{route}: second walking clip missing"
                );
                assert_ne!(
                    actor.pose(0.),
                    actor.pose(1.),
                    "{route}: path stays at start"
                );
            }
        }
    }
}
