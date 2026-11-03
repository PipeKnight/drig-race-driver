//! Shared default-off static billboard subset; no native RNG, animation or LOD policy.
use super::{
    crowd_population as population,
    crowd_preview::{self, Batch, CrowdMaterial, Spec, Tint},
};
use bevy::{
    asset::RenderAssetUsages,
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
use std::{collections::BTreeMap, error::Error, path::Path};
type AnyResult<T> = Result<T, Box<dyn Error>>;
#[derive(Clone, Debug)]
struct Row {
    position: Vec3,
    orientation: [f32; 2],
    opaque: f32,
    cell: u32,
    source: [usize; 2],
    material: usize,
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
    draw: bool,
    rows: Vec<Row>,
    specs: Vec<Spec>,
    diffuse: Vec<u8>,
    tint: Vec<u8>,
    region: String,
    atlas: String,
    total: u32,
    omitted: Vec<Omitted>,
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
fn definitions_path(root: &Path) -> AnyResult<String> {
    let shared = grid_tools::tyre_smoke::read_xml(&root.join("anims/organism_static_dataset.xml"))?;
    definitions_from_dataset(&shared)
}
fn definitions_from_dataset(shared: &grid_tools::tyre_smoke::Node) -> AnyResult<String> {
    if shared.name != "dataset" {
        return Err("shared organism dataset root differs".into());
    }
    let mut selected = None;
    for entry in shared.children.iter().filter(|e| e.name == "xml") {
        let processor = field(&entry.attributes, "processor")?;
        if processor != "OrganismPlugin" {
            continue;
        }
        let user = field(&entry.attributes, "userdata")?;
        if user.eq_ignore_ascii_case("typeDefs") && selected.replace(entry).is_some() {
            return Err("ambiguous shared typeDefs resource".into());
        }
    }
    let source = selected.ok_or("shared typeDefs resource absent")?;
    let filename = field(&source.attributes, "filename")?;
    if filename.contains('&') {
        return Err("XML entity in typeDefs filename unsupported".into());
    }
    Ok(population::resource_path(filename)?)
}
// New population modes share the existing A2C material, which requires MSAA4.
// Preserve every legacy mode's admission; camera configuration remains authoritative.
pub(crate) fn validate_view(
    mode: crowd_preview::Mode,
    settings: &super::graphics::Settings,
) -> AnyResult<()> {
    if mode.is_population() && settings.anti_aliasing == 3 {
        return Err(
            "population crowd preview requires baseline MSAA4; AA3 is single-sample".into(),
        );
    }
    Ok(())
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
pub(crate) fn prepare(
    root: &Path,
    route: &Route,
    mode: crowd_preview::Mode,
) -> AnyResult<Prepared> {
    if !mode.is_population() {
        return Err("not a general static population mode".into());
    }
    let graph = grid_tools::tyre_smoke::read_xml(
        &root.join(route.track_file("organism_track_dataset.xml")),
    )?;
    let dataset = population::dataset(&graph)?;
    let (user, region) = context(&dataset.pssg)?;
    let raw = std::fs::read(root.join(definitions_path(root)?))?;
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
    Ok(Prepared {
        draw: mode == crowd_preview::Mode::PopulationBillboards,
        rows,
        specs,
        diffuse,
        tint,
        region,
        atlas: resource.relative_path.clone(),
        total: source.total_instances,
        omitted,
    })
}
pub(crate) fn install(app: &mut App, prepared: Prepared) -> AnyResult<()> {
    println!(
        "crowd_population_static region={} atlas={} source={} billboards={} unsupported_static={} moving={} choice=SourceOrderFirst(map,variant,colour) scale_fixture_draw=5000 orientation=initial-camera-facing native_rng=false animation=false native_lod=false source_parity=false",
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
    if !prepared.draw || prepared.rows.is_empty() {
        return Ok(());
    }
    crowd_preview::install_shader(app)?;
    app.insert_resource(prepared)
        .add_plugins(MaterialPlugin::<CrowdMaterial>::default())
        .add_systems(PostStartup, spawn);
    Ok(())
}
fn spawn(
    mut commands: Commands,
    prepared: Res<Prepared>,
    view: Query<(&Transform, &Msaa), With<Camera3d>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<CrowdMaterial>>,
    settings: Res<super::graphics::Settings>,
) {
    let (camera, msaa) = view.single().expect("one static population camera");
    assert_eq!(*msaa, Msaa::Sample4);
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
                    alpha_mode: AlphaMode::AlphaToCoverage,
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
    let mut batches: BTreeMap<(usize, u32), Batch> = BTreeMap::new();
    let mut raw = 0u64;
    for r in &prepared.rows {
        crowd_preview::card_values(
            batches.entry((r.material, r.cell)).or_default(),
            r.position,
            &prepared.specs[r.material],
            camera.translation,
        )
        .expect("validated source billboard card");
        raw ^= r.orientation[0].to_bits() as u64
            ^ r.orientation[1].to_bits() as u64
            ^ r.opaque.to_bits() as u64
            ^ ((r.source[0] as u64) << 32)
            ^ r.source[1] as u64;
    }
    println!(
        "crowd_population_static source_metadata_checksum={raw} cell_policy=batch-only-no-native-culling cards={}",
        prepared.rows.len()
    );
    for ((material, cell), b) in batches {
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, b.p);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, b.n);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, b.uv);
        mesh.insert_indices(Indices::U32(b.indices));
        commands.spawn((
            Name::new(format!("crowd-static/material{material}/cell{cell}")),
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(handles[material].clone()),
            Transform::IDENTITY,
            NotShadowCaster,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn userdata_selects_context_and_population_admission_is_not_a_route_whitelist() {
        fn entry(userdata: &str) -> population::DatasetPssg<'_> {
            population::DatasetPssg {
                userdata,
                filename: "/data/anims/arbitrary.pssg",
            }
        }
        assert_eq!(context(&[entry("prefixbillboardusa")]).unwrap().1, "usa");
        assert_eq!(context(&[entry("billboard")]).unwrap().1, "europe");
        assert_eq!(context(&[entry("billboardUndefined")]).unwrap().1, "europe");
        assert_eq!(context(&[entry("billboardjapan")]).unwrap().1, "japan");
        assert!(context(&[entry("BILLBOARDusa")]).is_err());
        assert!(context(&[entry("billboard"), entry("billboardjapan")]).is_err());
        assert!(
            crowd_preview::Mode::PopulationControl
                .validate(true, true, false, true, true, Some(10), true)
                .is_ok()
        );
        assert!(
            crowd_preview::Mode::StaticBillboards
                .validate(true, true, false, true, true, Some(10), true)
                .is_err()
        );
    }
    #[test]
    fn typedef_selection_ignores_unrelated_userdata_and_validates_selected_fields() {
        let selected = r#"<xml processor="OrganismPlugin" userdata="typeDefs" filename="/data/anims/crowdDefs.xml"/>"#;
        // Exercise the actual parser -> selector used by definitions_path, including
        // unrelated missing/duplicate userdata and an unselected malformed path.
        let text = format!(
            r#"<dataset>
            <xml processor="Other" filename="x"/>
            <xml processor="Other" userdata="one" userdata="two"/>
            <xml processor="OrganismPlugin" userdata="other" filename="../unused"/>
            {selected}</dataset>"#
        );
        let graph = grid_tools::tyre_smoke::parse_plain(&text).unwrap();
        assert_eq!(
            definitions_from_dataset(&graph).unwrap(),
            "anims/crowdDefs.xml"
        );
        for text in [
            format!("<dataset>{selected}{selected}</dataset>"),
            r#"<dataset><xml processor="OrganismPlugin" userdata="typeDefs"/></dataset>"#.into(),
            r#"<dataset><xml processor="OrganismPlugin" userdata="typeDefs" filename="../bad"/></dataset>"#.into(),
            r#"<dataset><xml processor="OrganismPlugin" userdata="typeDefs" userdata="other" filename="/data/anims/crowdDefs.xml"/></dataset>"#.into(),
        ] {
            let graph = grid_tools::tyre_smoke::parse_plain(&text).unwrap();
            assert!(definitions_from_dataset(&graph).is_err());
        }
    }
    #[test]
    fn population_preflight_tracks_actual_baseline_camera_msaa_without_changing_legacy() {
        for aa in 0..=3 {
            let settings = super::super::graphics::Settings {
                anti_aliasing: aa,
                ..default()
            };
            assert!(!settings.enhanced());
            let mut world = World::new();
            let camera = world.spawn((Camera3d::default(), Msaa::Sample4)).id();
            let mut media = Assets::<bevy::light::atmosphere::ScatteringMedium>::default();
            let mut queue = bevy::ecs::world::CommandQueue::default();
            {
                let mut commands = Commands::new(&mut queue, &world);
                super::super::graphics::configure(&mut commands, camera, &settings, &mut media);
            }
            queue.apply(&mut world);
            let actual = *world.entity(camera).get::<Msaa>().unwrap();
            for mode in [
                crowd_preview::Mode::PopulationControl,
                crowd_preview::Mode::PopulationBillboards,
            ] {
                assert_eq!(
                    validate_view(mode, &settings).is_ok(),
                    actual == Msaa::Sample4
                );
            }
            assert!(validate_view(crowd_preview::Mode::StaticBillboards, &settings).is_ok());
            assert!(validate_view(crowd_preview::Mode::Off, &settings).is_ok());
        }
    }
    #[test]
    #[ignore = "purchased GRID_GAME_ROOT, real CPU spawn ECS; no renderer/window"]
    fn purchased_three_regions_publish_actual_spawned_meshes_and_materials() {
        use bevy::mesh::VertexAttributeValues;
        let root =
            std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT"));
        for route_path in [
            "tracks/europe/donington/route_0",
            "tracks/usa/detroit/route_8",
            "tracks/japan/shibuya/route_0",
        ] {
            let route = Route::new(route_path).unwrap();
            let prepared =
                prepare(&root, &route, crowd_preview::Mode::PopulationBillboards).unwrap();
            assert!(prepared.draw);
            let eye = Vec3::new(10000., 10000., 10000.);
            let mut expected: BTreeMap<String, (usize, Batch)> = BTreeMap::new();
            for row in &prepared.rows {
                let key = format!("crowd-static/material{}/cell{}", row.material, row.cell);
                let (_, batch) = expected
                    .entry(key)
                    .or_insert_with(|| (row.material, Batch::default()));
                crowd_preview::card_values(batch, row.position, &prepared.specs[row.material], eye)
                    .unwrap();
            }
            let source_cards = prepared.rows.len();
            let palette: Vec<_> = prepared.specs.iter().map(|s| s.colours).collect();
            let mut app = App::new();
            app.insert_resource(Assets::<Mesh>::default())
                .insert_resource(Assets::<Image>::default())
                .insert_resource(Assets::<CrowdMaterial>::default())
                .insert_resource(super::super::graphics::Settings::default())
                .insert_resource(prepared)
                .add_systems(PostStartup, spawn);
            app.world_mut().spawn((
                Camera3d::default(),
                Transform::from_translation(eye),
                Msaa::Sample4,
            ));
            // Main schedule runs the ACTUAL production PostStartup spawn and flushes Commands.
            // No MaterialPlugin, renderer/window or copied spawn helper is installed.
            app.update();
            let spawned: Vec<_> = {
                let world = app.world_mut();
                let mut query = world.query::<(
                    &Name,
                    &Mesh3d,
                    &MeshMaterial3d<CrowdMaterial>,
                    &Transform,
                    &NotShadowCaster,
                )>();
                query
                    .iter(world)
                    .map(|(name, mesh, material, transform, _)| {
                        (
                            name.as_str().to_owned(),
                            mesh.0.clone(),
                            material.0.clone(),
                            *transform,
                        )
                    })
                    .collect()
            };
            assert_eq!(
                spawned.len(),
                expected.len(),
                "{route_path}: deferred entities missing/duplicated"
            );
            let world = app.world();
            let meshes = world.resource::<Assets<Mesh>>();
            let materials = world.resource::<Assets<CrowdMaterial>>();
            let images = world.resource::<Assets<Image>>();
            assert_eq!(meshes.len(), expected.len());
            assert_eq!(materials.len(), palette.len());
            assert_eq!(images.len(), 2);
            let mut seen_meshes = std::collections::HashSet::new();
            let mut per_material = BTreeMap::new();
            let mut texture_pair = None;
            let mut actual_cards = 0;
            for (name, mesh_handle, material_handle, transform) in spawned {
                let (material_index, batch) = expected
                    .remove(&name)
                    .expect("unexpected source batch entity");
                assert_eq!(transform.to_matrix(), Mat4::IDENTITY);
                assert!(
                    seen_meshes.insert(mesh_handle.id()),
                    "two batches alias one geometry asset"
                );
                let mesh = meshes
                    .get(&mesh_handle)
                    .expect("entity mesh handle unresolved");
                let Some(VertexAttributeValues::Float32x3(positions)) =
                    mesh.attribute(Mesh::ATTRIBUTE_POSITION)
                else {
                    panic!("position layout");
                };
                let Some(VertexAttributeValues::Float32x3(normals)) =
                    mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
                else {
                    panic!("normal layout");
                };
                let Some(VertexAttributeValues::Float32x2(uv)) =
                    mesh.attribute(Mesh::ATTRIBUTE_UV_0)
                else {
                    panic!("UV layout");
                };
                let Some(Indices::U32(indices)) = mesh.indices() else {
                    panic!("index layout");
                };
                assert_eq!(positions, &batch.p);
                assert_eq!(normals, &batch.n);
                assert_eq!(uv, &batch.uv);
                assert_eq!(indices, &batch.indices);
                assert_eq!(positions.len() % 4, 0);
                assert_eq!(indices.len(), positions.len() / 4 * 6);
                actual_cards += positions.len() / 4;
                if let Some(previous) = per_material.insert(material_index, material_handle.id()) {
                    assert_eq!(
                        previous,
                        material_handle.id(),
                        "source material changed across cells"
                    );
                }
                let material = materials
                    .get(&material_handle)
                    .expect("entity material handle unresolved");
                assert_eq!(material.base.alpha_mode, AlphaMode::AlphaToCoverage);
                assert_eq!(material.extension.colour0, palette[material_index][0]);
                assert_eq!(material.extension.colour1, palette[material_index][1]);
                assert_eq!(material.extension.colour2, palette[material_index][2]);
                let diffuse = material.base.base_color_texture.as_ref().unwrap();
                let tint = &material.extension.map;
                assert!(images.get(diffuse).is_some());
                assert!(images.get(tint).is_some());
                assert_ne!(diffuse.id(), tint.id());
                let pair = (diffuse.id(), tint.id());
                if let Some(previous) = texture_pair.replace(pair) {
                    assert_eq!(previous, pair);
                }
            }
            assert!(expected.is_empty());
            assert_eq!(
                actual_cards, source_cards,
                "{route_path}: source-to-actual-mesh cardinality"
            );
            assert_eq!(per_material.len(), palette.len());
            let unique_materials: std::collections::HashSet<_> = per_material.values().collect();
            assert_eq!(
                unique_materials.len(),
                palette.len(),
                "source palettes accidentally share material assets"
            );
        }
    }
    #[test]
    #[ignore = "purchased GRID_GAME_ROOT, CPU only"]
    fn atlas_rejects_duplicate_selected_texture_but_ignores_unselected_duplicate_ids() {
        let root =
            std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT"));
        let bytes = std::fs::read(root.join("anims/cr_euro_billboard.pssg")).unwrap();
        let mut unrelated_id = ("unused".len() as u32).to_be_bytes().to_vec();
        unrelated_id.extend_from_slice(b"unused");
        let mut tree = pssg::read_tree(&bytes).unwrap();
        let baseline = atlas_from_tree(&tree).unwrap();
        let selected = (0..tree.nodes.len())
            .find(|&i| tree.name(i) == b"TEXTURE")
            .unwrap();
        fn duplicate<'a>(n: &pssg::Node<'a>) -> pssg::Node<'a> {
            pssg::Node {
                id: n.id,
                offset: n.offset,
                parent: n.parent,
                attributes: n.attributes.clone(),
                children: n.children.clone(),
                data: n.data,
            }
        }
        let mut unrelated = duplicate(&tree.nodes[selected]);
        let id_attr = unrelated
            .attributes
            .iter()
            .find(|a| {
                tree.framing
                    .schema
                    .iter()
                    .flat_map(|e| &e.attributes)
                    .any(|def| def.id == a.id && def.name == b"id")
            })
            .unwrap()
            .id;
        unrelated
            .attributes
            .iter_mut()
            .find(|a| a.id == id_attr)
            .unwrap()
            .value = &unrelated_id;
        tree.nodes.push(duplicate(&unrelated));
        tree.nodes.push(unrelated);
        assert_eq!(atlas_from_tree(&tree).unwrap(), baseline);
        let same = duplicate(&tree.nodes[selected]);
        tree.nodes.push(same);
        assert!(atlas_from_tree(&tree).is_err());
    }
    #[test]
    #[ignore = "purchased GRID_GAME_ROOT, CPU only"]
    fn purchased_europe_usa_japan_source_roles_counts_bits_and_cards() {
        let root =
            std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT"));
        for (path, region, total, cards, unsupported, moving) in [
            (
                "tracks/europe/donington/route_0",
                "europe",
                16189,
                16165,
                22,
                2,
            ),
            ("tracks/usa/detroit/route_8", "usa", 6854, 6849, 5, 0),
            ("tracks/japan/shibuya/route_0", "japan", 15422, 15421, 0, 1),
        ] {
            let route = Route::new(path).unwrap();
            let p = prepare(&root, &route, crowd_preview::Mode::PopulationControl).unwrap();
            assert!(!p.draw);
            assert_eq!(p.region, region);
            assert_eq!(p.total, total);
            assert_eq!(p.rows.len(), cards);
            assert_eq!(
                p.omitted
                    .iter()
                    .filter(|o| !o.moving)
                    .map(|o| o.records)
                    .sum::<u32>(),
                unsupported
            );
            assert_eq!(
                p.omitted
                    .iter()
                    .filter(|o| o.moving)
                    .map(|o| o.records)
                    .sum::<u32>(),
                moving
            );
            let bytes = std::fs::read(root.join(route.file("crowd_standing.bin"))).unwrap();
            let source = crowd_placements::read(&bytes).unwrap();
            for row in &p.rows {
                let Placements::Standing(points) = &source.groups[row.source[0]].placements else {
                    panic!()
                };
                let src = &points[row.source[1]];
                assert_eq!(
                    row.position.to_array().map(f32::to_bits),
                    src.position.map(f32::to_bits)
                );
                assert_eq!(
                    row.orientation.map(f32::to_bits),
                    src.orientation_pair.map(f32::to_bits)
                );
                assert_eq!(row.opaque.to_bits(), src.opaque_scalar.to_bits());
                assert_eq!(row.cell, src.cell);
                let s = &p.specs[row.material];
                let mut b = Batch::default();
                crowd_preview::card_values(
                    &mut b,
                    row.position,
                    s,
                    row.position + Vec3::new(0., 2., 10.),
                )
                .unwrap();
                assert_eq!(b.p.len(), 4);
                assert_eq!(b.indices.len(), 6);
                assert_eq!(b.uv[0], [s.uv[2], s.uv[3]]);
                assert_eq!(b.uv[3], [s.uv[0], s.uv[1]]);
                assert!(
                    (b.p[1][1] - b.p[0][1] - s.dimensions[1] * s.instance_scale[1]).abs() < 0.0001
                );
            }
            let count = Path::new(&p.atlas).file_name().unwrap().to_str().unwrap();
            assert!(!p.diffuse.is_empty() && !p.tint.is_empty(), "{count}");
        }
    }
    #[test]
    #[ignore = "purchased GRID_GAME_ROOT, CPU only; real typed catalogue alternative control"]
    fn purchased_one_placement_is_not_multiplied_by_alternatives() {
        let root =
            std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT"));
        let route = Route::new("tracks/europe/donington/route_0").unwrap();
        let graph = grid_tools::tyre_smoke::read_xml(
            &root.join(route.track_file("organism_track_dataset.xml")),
        )
        .unwrap();
        let dataset = population::dataset(&graph).unwrap();
        let (user, region) = context(&dataset.pssg).unwrap();
        let raw = std::fs::read(root.join(definitions_path(&root).unwrap())).unwrap();
        let defs = binxml::read(&raw).unwrap();
        let b = std::fs::read(root.join(route.file("crowd_standing.bin"))).unwrap();
        let source = crowd_placements::read(&b).unwrap();
        let meta = population::prepare(
            &source,
            &defs,
            &dataset.pssg,
            population::DefinitionSelection::Exact {
                primary_definition_set: Some(&region),
                fallback_definition_set: Some("generic"),
                map_group: "generic",
                billboard_userdata: &user,
            },
        )
        .unwrap();
        let (group, templates) = meta
            .groups
            .iter()
            .find_map(|g| match &g.resolution {
                population::Resolution::StaticTemplates(t)
                    if t[0].variants[0].billboard_stance.is_some() =>
                {
                    Some((g, t))
                }
                _ => None,
            })
            .unwrap();
        let Placements::Standing(points) = &group.source.placements else {
            panic!()
        };
        let mut first = templates[0].clone();
        let mut bad = first.clone();
        bad.variants[0]
            .billboard_stance
            .as_mut()
            .unwrap()
            .iter_mut()
            .find(|(k, _)| k == "width")
            .unwrap()
            .1 = "NaN".into();
        first.variants.push(bad.variants[0].clone());
        let mut invalid_colour = first.variants[0].colour_sets[0].clone();
        invalid_colour.colours.clear();
        first.variants[0].colour_sets.push(invalid_colour);
        let alternatives = vec![first, bad];
        let mut rows = Vec::new();
        let mut specs = Vec::new();
        assert!(append_group(&points[..2], &alternatives, 0, &mut rows, &mut specs).unwrap());
        assert_eq!(rows.len(), 2);
        assert_eq!(specs.len(), 1);
        assert_eq!(rows[1].source, [0, 1]);
    }
}
