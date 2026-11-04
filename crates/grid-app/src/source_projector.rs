//! Bounded source RGB projector with an authored PBR diffuse adapter; default off.
//! No source normal/depth/focus, material-wide or physical-photometry parity.
#[path = "source_projector_witness.rs"]
mod witness;
use bevy::{
    asset::RenderAssetUsages,
    image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    light::SpotLightTexture,
    pbr::decal::clustered::{self, RenderClusteredDecals},
    prelude::*,
    render::{
        Extract, ExtractSchedule, Render, RenderApp, RenderSystems,
        render_asset::RenderAssets,
        render_resource::{CachedPipelineState, PipelineCache},
        renderer::{RenderAdapter, RenderDevice},
        sync_world::RenderEntity,
        texture::GpuImage,
    },
    shader::{ShaderImport, Source as ShaderSource},
};
use grid_formats::{pssg, texture};
use grid_tools::route::Route;
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
#[derive(Resource, Clone, Default)]
struct CookieGpuReady(Arc<AtomicBool>);

pub(crate) const SOURCE_PROJECTOR_TAG: u32 = 0x47524944;
const IMPORT_PATH: &str = "bevy_pbr::lighting";
const ADAPTER_MARKER: &str = "// GRID_SOURCE_PROJECTOR_AGGREGATE_V2";
// Filled from exact installed Bevy0.19.1 source before proposal delivery.
const FUNCTIONS_SHA256: &str = "13a5344ef4e0fa967e05978489d0909dd0c1862cf7a989ce0882dbc8de9a55dd";
const SUPPORTED_SHA256: &str = "ec609c97f380d1d18d180763ffffb253a1349a4ab4739435bccf7ca829f4c8b2";

#[derive(Component, Clone)]
pub(crate) struct SourceProjector {
    image: Handle<Image>,
    /// Explicit authored declaration until real focus-component identity joins.
    declared_range: f32,
}
#[derive(Resource, Default)]
struct Admission {
    device: bool,
    shader: bool,
}
/// Install only when caller explicitly requests the bounded diagnostic.
/// No auto-enablement from route/car identity, and no dependency enabling here.
pub(crate) struct SourceProjectorPlugin;
impl Plugin for SourceProjectorPlugin {
    fn build(&self, app: &mut App) {
        app.world()
            .get_resource::<Parameters>()
            .expect("explicit immutable projector snapshot required")
            .validate()
            .expect("invalid source snapshot");
        let gpu_ready = CookieGpuReady::default();
        app.insert_resource(gpu_ready.clone());
        app.init_resource::<Admission>()
            .add_systems(Startup, check_device)
            .add_systems(Update, patch_loaded_shader)
            .add_systems(
                Update,
                gate_not_ready
                    .after(patch_loaded_shader)
                    .after(crate::drive_present),
            );
        let render = app
            .get_sub_app_mut(RenderApp)
            .expect("source projector requires actual render app");
        render.insert_resource(gpu_ready);
        witness::install(render);
        render.add_systems(
            Render,
            refuse_pipeline_errors.in_set(RenderSystems::Cleanup),
        );
        render.add_systems(
            ExtractSchedule,
            extract_source_projectors.after(clustered::extract_decals),
        );
    }
}
fn check_device(
    device: Res<RenderDevice>,
    adapter: Res<RenderAdapter>,
    mut admission: ResMut<Admission>,
) {
    require_capabilities(
        cfg!(feature = "source-projector"),
        clustered::clustered_decals_are_usable(&device, &adapter),
    )
    .expect("source projector feature/device admission refused");
    admission.device = true;
}
fn require_capabilities(compiled: bool, usable: bool) -> Result<(), String> {
    if !(compiled && usable) {
        return Err("source projector requires compiled source-projector=>bevy/pbr_light_textures and usable clustered binding arrays".into());
    }
    Ok(())
}
fn gate_not_ready(
    admission: Res<Admission>,
    gpu_ready: Res<CookieGpuReady>,
    mut lights: Query<&mut SpotLight, With<SourceProjector>>,
) {
    if !(admission.device && admission.shader && gpu_ready.0.load(Ordering::Acquire)) {
        for mut light in &mut lights {
            light.intensity = 0.;
        }
    }
}
/// Carrier transport is WHITE and starts dark. Live presentation supplies 4PI*w.
/// Caller preserves activation, declared range and shadow policy.
pub(crate) fn attach(
    world: &mut World,
    entity: Entity,
    image: Handle<Image>,
    declared_range: f32,
) -> Result<(), String> {
    if !declared_range.is_finite() || declared_range <= 0. {
        return Err("source projector declared range must be finite positive".into());
    }
    if world.get::<SpotLightTexture>(entity).is_some() {
        return Err(
            "source marker owns its cookie carrier; duplicate native SpotLightTexture refused"
                .into(),
        );
    }
    let mut light = world
        .get_mut::<SpotLight>(entity)
        .ok_or("source projector requires an existing actual SpotLight")?;
    if light.shadow_maps_enabled {
        return Err("source projector prototype requires existing shadowless headlight; it does not override shadow policy".into());
    }
    light.color = Color::WHITE;
    light.intensity = 0.;
    // Enclose rectangular ±2 ratios, including diagonal corners, for native
    // cone clustering. Actual fragment shape is the strict source rectangle.
    light.range = declared_range;
    light.outer_angle = (2. * 2_f32.sqrt()).atan();
    light.inner_angle = 0.;
    world.entity_mut(entity).insert(SourceProjector {
        image,
        declared_range,
    });
    Ok(())
}
type SourceProjectors<'w, 's> = Query<
    'w,
    's,
    (
        RenderEntity,
        &'static SourceProjector,
        &'static GlobalTransform,
        &'static ViewVisibility,
        &'static SpotLight,
    ),
>;
fn extract_source_projectors(
    admission: Extract<Res<Admission>>,
    query: Extract<SourceProjectors<'_, '_>>,
    mut decals: ResMut<RenderClusteredDecals>,
    images: Option<Res<RenderAssets<GpuImage>>>,
    gpu_ready: Res<CookieGpuReady>,
    mut compiled_witness: ResMut<witness::State>,
) {
    if !(admission.device && admission.shader) {
        return;
    }
    let owned_images: std::collections::HashSet<_> =
        query.iter().map(|(_, p, _, _, _)| p.image.id()).collect();
    let all_present = images
        .as_ref()
        .is_some_and(|assets| owned_images.iter().all(|id| assets.get(*id).is_some()));
    let prior = gpu_ready.0.load(Ordering::Acquire);
    let all_uploaded = cookie_transition(prior, !owned_images.is_empty(), all_present)
        .unwrap_or_else(|error| {
            gpu_ready.0.store(false, Ordering::Release);
            panic!("{error}");
        });
    gpu_ready.0.store(all_uploaded, Ordering::Release);
    if !all_uploaded {
        return;
    }
    if !prior {
        println!(
            "source_projector_cookie_gpu images={} ready=true pipeline_ready=false",
            owned_images.len()
        );
    }

    assert!(
        decals
            .binding_index_to_textures
            .iter()
            .all(|id| owned_images.contains(id)),
        "source projector diagnostic refuses mixed clustered-decal samplers until compatibility is verified"
    );
    for (entity, projector, transform, visible, light) in &query {
        if !visible.get() {
            continue;
        }
        assert_eq!(
            light.range.to_bits(),
            projector.declared_range.to_bits(),
            "source projector range changed without updating declaration"
        );
        let (scale, _, _) = transform.to_scale_rotation_translation();
        assert!(
            transform.affine().is_finite() && (scale - Vec3::ONE).abs().max_element() < 1e-4,
            "source projector prototype requires rigid unit-scale actual light transform"
        );
        // Native extract_decals has already cleared/built this frame's maps.
        // This own carrier has no SpotLightTexture, so no unused duplicate entry.
        decals.insert_decal(
            entity,
            [Some(projector.image.id()), None, None, None],
            transform.affine().inverse().into(),
            transform.translation(),
            projector.declared_range,
            SOURCE_PROJECTOR_TAG,
        );
        let index = decals
            .get(entity)
            .expect("just-inserted source decal public map");
        witness::inserted(
            &mut compiled_witness,
            entity,
            index,
            SOURCE_PROJECTOR_TAG,
            light.intensity,
        );
    }
}

#[derive(Resource, Clone, Copy, Debug)]
pub(crate) struct Parameters {
    pub working_unit: f32,
    pub global_colour: Vec3,
}
impl Parameters {
    pub(crate) fn validate(self) -> Result<Self, String> {
        if !self.working_unit.is_finite()
            || self.working_unit <= 0.
            || !self.global_colour.is_finite()
            || self.global_colour.min_element() < 0.
            || !(self.global_colour * self.working_unit).is_finite()
        {
            return Err("source projector snapshot needs finite positive U, finite nonnegative RGB and finite product".into());
        }
        Ok(self)
    }
}
fn sha(source: &str) -> String {
    grid_tools::private_parameters::hex(&grid_tools::private_parameters::sha256(source.as_bytes()))
}
fn unique_replace(source: &str, anchor: &str, replacement: &str) -> Result<String, String> {
    if source.matches(anchor).count() != 1 {
        return Err("Bevy aggregate projector unique shader anchor changed".into());
    }
    Ok(source.replacen(anchor, replacement, 1))
}
fn patch_pair(
    lighting: &str,
    functions: &str,
    enabled: bool,
    params: Parameters,
) -> Result<(String, String), String> {
    if !enabled {
        return Ok((lighting.into(), functions.into()));
    }
    let params = params.validate()?;
    if lighting.contains(ADAPTER_MARKER)
        || functions.contains(ADAPTER_MARKER)
        || sha(lighting) != SUPPORTED_SHA256
        || sha(functions) != FUNCTIONS_SHA256
    {
        return Err("unsupported/already patched Bevy aggregate projector shader pair".into());
    }
    let l = unique_replace(
        lighting,
        "fn point_light(\n",
        &(include_str!("lighting_helpers.wgsl").to_owned() + "\nfn point_light(\n"),
    )?;
    let mut f = unique_replace(
        functions,
        "    // Spot lights (direct)\n",
        &format!(
            "{ADAPTER_MARKER}\n    var grid_source_direct = vec3<f32>(0.0);\n#ifdef STANDARD_MATERIAL_DIFFUSE_TRANSMISSION\n    var grid_source_transmitted = vec3<f32>(0.0);\n#endif\n    // Spot lights (direct)\n"
        ),
    )?;
    let start = f
        .find("    // Spot lights (direct)\n")
        .ok_or("spot loop absent")?;
    let end = f[start..]
        .find("    // directional lights (direct)\n")
        .ok_or("directional boundary absent")?
        + start;
    let spot = &f[start..end];
    let anchor = "        var shadow: f32 = 1.0;";
    let branch = r#"        if lighting::grid_source_projector(light_id) {
            if enable_diffuse {
                grid_source_direct += lighting::grid_source_raw(light_id, in.world_position.xyz);
#ifdef STANDARD_MATERIAL_DIFFUSE_TRANSMISSION
                grid_source_transmitted += lighting::grid_source_raw(light_id, diffuse_transmissive_lobe_world_position.xyz);
#endif
            }
            // This source tag has no native PBR diffuse, specular or shadow work.
            continue;
        }
        var shadow: f32 = 1.0;"#;
    let changed = unique_replace(spot, anchor, branch)?;
    f.replace_range(start..end, &changed);
    let c = params.global_colour;
    // Immutable bounded-run snapshot, not runtime colour timeline or source SI units.
    let final_adapter = format!(
        r#"    // AUTHORED PBR-diffuse policy: clamp aggregate once BEFORE source colour.
    let grid_source_scale = vec3<f32>({:.9e}, {:.9e}, {:.9e}) * {:.9e} / 3.141592653589793;
    direct_light += clamp(grid_source_direct, vec3(0.0), vec3(1.0)) * grid_source_scale * diffuse_color * diffuse_occlusion;
#ifdef STANDARD_MATERIAL_DIFFUSE_TRANSMISSION
    transmitted_light += clamp(grid_source_transmitted, vec3(0.0), vec3(1.0)) * grid_source_scale * diffuse_transmissive_color * diffuse_occlusion;
#endif

    // directional lights (direct)
"#,
        c.x, c.y, c.z, params.working_unit
    );
    f = unique_replace(&f, "    // directional lights (direct)\n", &final_adapter)?;
    Ok((l, f))
}
fn patch_loaded_shader(
    mut shaders: ResMut<Assets<Shader>>,
    params: Res<Parameters>,
    mut admission: ResMut<Admission>,
) {
    if admission.shader {
        return;
    }
    // Both must be present and both must validate BEFORE modifying either asset.
    let mut selected = Vec::new();
    for path in [IMPORT_PATH, "bevy_pbr::pbr_functions"] {
        let ids: Vec<_> = shaders
            .iter()
            .filter(|(_, s)| s.import_path == ShaderImport::Custom(path.into()))
            .map(|(id, _)| id)
            .collect();
        assert!(ids.len() <= 1, "duplicate Bevy shader import asset");
        let Some(id) = ids.first().copied() else {
            return;
        };
        selected.push((id, shaders.get(id).expect("stable shader id").clone()));
    }
    fn source(s: &Shader) -> &str {
        match &s.source {
            ShaderSource::Wgsl(x) => x.as_ref(),
            _ => panic!("Bevy module is not WGSL"),
        }
    }
    let (l, f) = patch_pair(
        source(&selected[0].1),
        source(&selected[1].1),
        true,
        *params,
    )
    .expect("aggregate shader version/anchor refusal");
    for ((id, mut original), replacement) in selected.into_iter().zip([l, f]) {
        original.source = ShaderSource::Wgsl(replacement.into());
        shaders
            .insert(id, original)
            .expect("same-id Bevy shader replacement");
    }
    admission.shader = true;
    println!(
        "source_projector version=2 optics=source-RGB-rect-linear-aggregate adapter=AUTHORED-PBR-diffuse U={} global_colour={:?} snapshot=true source_normal_policy=false pipeline_ready_unproven=true",
        params.working_unit, params.global_colour
    );
}
/// Transport, not source photometry or a physical-lumens boost.
pub(crate) fn transport_intensity(normalized_source_weight: f32) -> Result<f32, String> {
    if !normalized_source_weight.is_finite() || !(0. ..=2.5).contains(&normalized_source_weight) {
        return Err("source projector normalized weight outside 0..2.5".into());
    }
    Ok(4. * std::f32::consts::PI * normalized_source_weight)
}

/// Actual route resource, original filename order, base-level BC3 only.
/// No fallback to a particular track and no generated replacement cookie.
pub(crate) fn load_cookie(root: &Path, route: &Route) -> Result<(Image, String), String> {
    let relative = route.track_file("headlights.pssg");
    let bytes = std::fs::read(root.join(&relative)).map_err(|e| format!("{relative}: {e}"))?;
    if bytes.len() > 4 * 1024 * 1024 {
        return Err("source headlight container exceeds diagnostic bound".into());
    }
    let tree = pssg::read_tree(&bytes).map_err(|e| e.to_string())?;
    let textures = texture::read_textures(&tree).map_err(|e| e.to_string())?;
    let source = match named_texture(&textures, b"headlight.dds")? {
        Some(source) => source,
        None => named_texture(&textures, b"headlight.tga")?
            .ok_or("source cookie headlight.dds/tga absent")?,
    };
    if source.format != texture::Format::Bc3 {
        return Err("source cookie prototype admits actual BC3 atlas only".into());
    }
    let (cropped, width, height) = left_bc3_base(source.base_blocks, source.width, source.height)?;
    let cropped = texture::Texture {
        id: source.id,
        width,
        height,
        format: source.format,
        declared_mip_levels: 0,
        base_blocks: &cropped,
        remaining_data: &[],
    };
    // Explicit AUTHORED sampler/colour-space adaptation, original backendstate unknown.
    let sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        ..ImageSamplerDescriptor::linear()
    });
    let image = crate::texture_upload::image_linear(
        &cropped.base_dds(),
        sampler,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
    .map_err(|e| e.to_string())?;
    let sha = grid_tools::private_parameters::hex(&grid_tools::private_parameters::sha256(&bytes));
    println!(
        "source_projector_cookie file={relative} sha={sha} id={} source={}x{} crop={}x{} levels=1 rgb=true alpha_unused=true colour_space=AUTHORED-linear sampler=AUTHORED-clamp-linear original_sampler_parity=false",
        String::from_utf8_lossy(source.id),
        source.width,
        source.height,
        width,
        height
    );
    Ok((image, relative))
}
fn named_texture<'a, 'data>(
    textures: &'a [texture::Texture<'data>],
    name: &[u8],
) -> Result<Option<&'a texture::Texture<'data>>, String> {
    let matches: Vec<_> = textures
        .iter()
        .filter(|t| t.id.eq_ignore_ascii_case(name))
        .collect();
    if matches.len() > 1 {
        return Err("duplicate selected source headlight texture id".into());
    }
    Ok(matches.first().copied())
}
fn left_bc3_base(data: &[u8], width: u32, height: u32) -> Result<(Vec<u8>, u32, u32), String> {
    if width < 8
        || height < 4
        || !width.is_multiple_of(8)
        || !height.is_multiple_of(4)
        || width / 2 != height
    {
        return Err("source cookie prototype needs aligned 2:1 atlas".into());
    }
    let row = (width / 4) as usize * 16;
    let rows = (height / 4) as usize;
    if data.len() != row * rows {
        return Err("source cookie BC3 base length mismatch".into());
    }
    let mut out = Vec::with_capacity(data.len() / 2);
    for blocks in data.chunks_exact(row) {
        out.extend_from_slice(&blocks[..row / 2]);
    }
    Ok((out, width / 2, height))
}
/// Unit-free source fade descriptor only; never mapped to lumens automatically.
pub(crate) fn source_mask_weight(fade: f32) -> Result<f32, String> {
    if !fade.is_finite() || !(0. ..=1.).contains(&fade) {
        return Err("source fade outside normalized admission".into());
    }
    Ok((fade * 1.5).clamp(0., 1.) * 2.5)
}
/// CPU reference for the owned shader shape, identity rigid source frame.
#[cfg(test)]
fn shape_reference(local: Vec3, range: f32) -> Result<(Vec2, f32), String> {
    if !local.is_finite() || !range.is_finite() || range <= 0. {
        return Err("invalid source projector reference domain".into());
    }
    let z = -local.z;
    if z <= 0. || local.x.abs() * 0.5 >= z || local.y.abs() * 0.5 >= z {
        return Ok((Vec2::ZERO, 0.));
    }
    Ok((
        local.truncate() / z * Vec2::new(0.25, -0.25) + Vec2::splat(0.5),
        (1. - local.length() / range).clamp(0., 1.),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn params() -> Parameters {
        Parameters {
            working_unit: 2.,
            global_colour: Vec3::new(2., 1., 0.5),
        }
    }
    fn adapted(samples: &[Vec3], c: Vec3, u: f32, diffuse: Vec3, ao: Vec3) -> Vec3 {
        samples
            .iter()
            .copied()
            .sum::<Vec3>()
            .clamp(Vec3::ZERO, Vec3::ONE)
            * c
            * (u / std::f32::consts::PI)
            * diffuse
            * ao
    }
    #[test]
    fn snapshot_refuses_missing_domain_without_defaults() {
        assert!(params().validate().is_ok());
        for u in [0., -1., f32::NAN, f32::INFINITY] {
            assert!(
                Parameters {
                    working_unit: u,
                    ..params()
                }
                .validate()
                .is_err()
            );
        }
        assert!(
            Parameters {
                global_colour: Vec3::splat(-1.),
                ..params()
            }
            .validate()
            .is_err()
        );
    }
    #[test]
    fn pure_aggregate_has_no_order_dependent_material_budget() {
        let a = [Vec3::splat(0.75), Vec3::splat(0.75)];
        let b = [a[1], a[0]];
        assert_eq!(
            adapted(&a, Vec3::ONE, 2., Vec3::splat(0.2), Vec3::splat(0.5)),
            adapted(&b, Vec3::ONE, 2., Vec3::splat(0.2), Vec3::splat(0.5))
        );
        assert_eq!(
            adapted(
                &a,
                Vec3::splat(2.),
                std::f32::consts::PI,
                Vec3::ONE,
                Vec3::ONE
            ),
            Vec3::splat(2.)
        );
    }
    #[test]
    fn separate_lobes_and_ao_albedo_policy() {
        let direct = adapted(
            &[Vec3::splat(0.8)],
            Vec3::ONE,
            std::f32::consts::PI,
            Vec3::ONE,
            Vec3::ONE,
        );
        let transmitted = adapted(
            &[Vec3::splat(0.2)],
            Vec3::ONE,
            std::f32::consts::PI,
            Vec3::ONE,
            Vec3::ONE,
        );
        assert!((direct.x - 0.8).abs() < 1e-6 && (transmitted.x - 0.2).abs() < 1e-6);
        assert_eq!(
            adapted(&[Vec3::ONE], Vec3::ONE, 1., Vec3::ZERO, Vec3::ONE),
            Vec3::ZERO
        );
        assert_eq!(
            adapted(&[Vec3::ONE], Vec3::ONE, 1., Vec3::ONE, Vec3::ZERO),
            Vec3::ZERO
        );
    }
    #[test]
    fn carrier_transport_normalizes_source_weight() {
        assert!(
            (transport_intensity(2.5).unwrap() / (4. * std::f32::consts::PI) - 2.5).abs() < 1e-6
        );
        assert_eq!(transport_intensity(0.).unwrap(), 0.);
        assert!(transport_intensity(2.5001).is_err());
    }
    #[test]
    fn disabled_pair_preserves_exact_bytes_and_unknown_version_refuses() {
        let (l, f) =
            patch_pair("unknown lighting\r\n", "unknown functions", false, params()).unwrap();
        assert_eq!(l, "unknown lighting\r\n");
        assert_eq!(f, "unknown functions");
        assert!(patch_pair(&l, &f, true, params()).is_err());
    }
    #[test]
    fn rectangle_radial_and_crop_cpu_reference() {
        assert_eq!(
            shape_reference(Vec3::new(0., 0., -5.), 10.).unwrap(),
            (Vec2::splat(0.5), 0.5)
        );
        assert_eq!(shape_reference(Vec3::new(2., 0., -1.), 10.).unwrap().1, 0.);
        assert!(shape_reference(Vec3::new(1.9, 1.9, -1.), 10.).unwrap().1 > 0.);
        let data: Vec<u8> = (0..8).flat_map(|x| std::iter::repeat_n(x, 16)).collect();
        let (crop, _, _) = left_bc3_base(&data, 16, 8).unwrap();
        assert_eq!(
            crop,
            [0, 1, 4, 5]
                .into_iter()
                .flat_map(|x| std::iter::repeat_n(x, 16))
                .collect::<Vec<_>>()
        );
    }
    #[test]
    #[ignore = "requires explicit installed Bevy primary shader directory; no native/GPU work"]
    fn pinned_primary_pair_anchors_preserve_native_code_and_split_lobes() {
        let dir = std::path::PathBuf::from(
            std::env::var_os("GRID_BEVY_SHADER_DIR").expect("set primary shader path"),
        );
        let l = std::fs::read_to_string(dir.join("pbr_lighting.wgsl")).unwrap();
        let f = std::fs::read_to_string(dir.join("pbr_functions.wgsl")).unwrap();
        let (pl, pf) = patch_pair(&l, &f, true, params()).unwrap();
        assert_eq!(pl.matches(ADAPTER_MARKER).count(), 1);
        assert_eq!(pf.matches(ADAPTER_MARKER).count(), 1);
        assert!(pf.contains("grid_source_transmitted += lighting::grid_source_raw(light_id, diffuse_transmissive_lobe_world_position.xyz)"));
        assert!(pf.contains(
            "grid_source_direct += lighting::grid_source_raw(light_id, in.world_position.xyz)"
        ));
        assert!(pf.contains(
            "let light_contrib = lighting::spot_light(light_id, &lighting_input, enable_diffuse);"
        ));
        assert!(pf.contains("continue;\n        }\n        var shadow: f32 = 1.0;"));
        assert!(pl.contains("fn spot_light("));
        assert!(patch_pair(&pl, &pf, true, params()).is_err());
        assert!(patch_pair(&(l + "\n"), &f, true, params()).is_err());
    }
}

// Global error refusal in this bounded diagnostic is deliberately stronger than
// readiness. Empty/quiet cache is NOT positive proof that tagged source draws ran.
// The native cache explicitly retries these two loading states. They must not
// be confused with permanent shader-processing/device-module failures.
fn shader_is_pending(error: &bevy::shader::ShaderCacheError) -> bool {
    matches!(
        error,
        bevy::shader::ShaderCacheError::ShaderNotLoaded(_)
            | bevy::shader::ShaderCacheError::ShaderImportNotYetAvailable
    )
}
fn refuse_pipeline_errors(cache: Res<PipelineCache>) {
    for pipeline in cache.pipelines() {
        if let CachedPipelineState::Err(error) = &pipeline.state
            && !shader_is_pending(error)
        {
            panic!("source projector diagnostic refuses pipeline error: {error:?}");
        }
    }
}

/// Pure consumed readiness transition: image-presence only, NOT pipeline proof.
fn cookie_transition(
    was_ready: bool,
    any_owned: bool,
    all_present: bool,
) -> Result<bool, &'static str> {
    let ready = any_owned && all_present;
    if was_ready && !ready {
        return Err("retained source cookie GPU image disappeared; refuse fallback render");
    }
    Ok(ready)
}
#[cfg(test)]
mod cookie_readiness_tests {
    use super::cookie_transition;
    #[test]
    fn empty_owned_set_is_not_vacuous_success() {
        assert_eq!(cookie_transition(false, false, true), Ok(false));
        assert_eq!(cookie_transition(false, false, false), Ok(false));
    }
    #[test]
    fn upload_wait_then_ready_then_stable_uses_actual_presence() {
        let mut ready = false;
        for (present, expected) in [(false, false), (false, false), (true, true), (true, true)] {
            ready = cookie_transition(ready, true, present).unwrap();
            assert_eq!(ready, expected);
        }
    }
    #[test]
    fn loss_of_retained_cookie_or_owned_set_refuses_after_admission() {
        assert!(cookie_transition(true, true, false).is_err());
        assert!(cookie_transition(true, false, true).is_err());
        assert!(cookie_transition(true, false, false).is_err());
    }
}

#[cfg(test)]
mod actual_shader_asset_ecs_tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    fn parameters() -> Parameters {
        Parameters {
            working_unit: 2.,
            global_colour: Vec3::ONE,
        }
    }
    fn primary(name: &str) -> String {
        let dir = std::path::PathBuf::from(
            std::env::var_os("GRID_BEVY_SHADER_DIR").expect("GRID_BEVY_SHADER_DIR required"),
        );
        std::fs::read_to_string(dir.join(name)).unwrap()
    }
    fn wgsl(shader: &Shader) -> &str {
        match &shader.source {
            ShaderSource::Wgsl(x) => x.as_ref(),
            _ => panic!("actual fixture must be WGSL"),
        }
    }
    fn preserved(before: &Shader, after: &Shader) {
        assert_eq!(before.path, after.path);
        assert_eq!(before.import_path, after.import_path);
        assert_eq!(before.imports, after.imports);
        assert_eq!(before.additional_imports, after.additional_imports);
        assert_eq!(before.shader_defs, after.shader_defs);
        assert_eq!(before.file_dependencies, after.file_dependencies);
        assert_eq!(
            std::mem::discriminant(&before.validate_shader),
            std::mem::discriminant(&after.validate_shader)
        );
    }
    fn shader(source: String, path: &str, dependency: Handle<Shader>) -> Shader {
        let mut s = Shader::from_wgsl(source, path.to_owned());
        // Non-default metadata sentinels verify cloning actual asset settings,
        // not just reconstructing equivalent imports from the changed source.
        s.shader_defs.push(bevy::shader::ShaderDefVal::Bool(
            "TEST_PRESERVE_DEF".into(),
            true,
        ));
        s.additional_imports.push(Default::default());
        s.additional_imports.last_mut().unwrap().import = "test_preserve_extra_import".into();
        s.file_dependencies.push(dependency);
        s.validate_shader = bevy::shader::ValidateShader::Enabled;
        s
    }
    fn world() -> World {
        let mut w = World::new();
        w.insert_resource(Assets::<Shader>::default());
        w.insert_resource(parameters());
        w.insert_resource(Admission::default());
        w
    }
    #[test]
    #[ignore = "reads explicit installed Bevy primary assets; actual ECS update only, no device/GPU/compiler"]
    fn actual_asset_system_waits_for_both_then_patches_same_ids_and_is_idempotent() {
        let mut w = world();
        let (lh, pending_f, original_l, original_f) = {
            let mut assets = w.resource_mut::<Assets<Shader>>();
            let dep = assets.add(Shader::from_wgsl(
                "fn fixture_dependency() {}",
                "test-dependency",
            ));
            let l = shader(
                primary("pbr_lighting.wgsl"),
                "actual-primary-lighting",
                dep.clone(),
            );
            let f = shader(
                primary("pbr_functions.wgsl"),
                "actual-primary-functions",
                dep,
            );
            assert_eq!(sha(wgsl(&l)), SUPPORTED_SHA256);
            assert_eq!(sha(wgsl(&f)), FUNCTIONS_SHA256);
            let lh = assets.add(l.clone());
            (lh, f.clone(), l, f)
        };
        // Only lighting is present: actual system must leave it byte-identical.
        w.run_system_once(patch_loaded_shader).unwrap();
        assert!(!w.resource::<Admission>().shader);
        {
            let assets = w.resource::<Assets<Shader>>();
            let after = assets.get(lh.id()).unwrap();
            preserved(&original_l, after);
            assert_eq!(wgsl(&original_l), wgsl(after));
        }
        let fh = w.resource_mut::<Assets<Shader>>().add(pending_f);
        w.run_system_once(patch_loaded_shader).unwrap();
        assert!(w.resource::<Admission>().shader);
        let (patched_l, patched_f) = {
            let assets = w.resource::<Assets<Shader>>();
            let l = assets.get(lh.id()).expect("same lighting ID retained");
            let f = assets.get(fh.id()).expect("same functions ID retained");
            preserved(&original_l, l);
            preserved(&original_f, f);
            assert_ne!(wgsl(&original_l), wgsl(l));
            assert_ne!(wgsl(&original_f), wgsl(f));
            assert_eq!(wgsl(l).matches(ADAPTER_MARKER).count(), 1);
            assert_eq!(wgsl(f).matches(ADAPTER_MARKER).count(), 1);
            (wgsl(l).to_owned(), wgsl(f).to_owned())
        };
        // Actual second system update uses Admission early return, no double patch.
        w.run_system_once(patch_loaded_shader).unwrap();
        let assets = w.resource::<Assets<Shader>>();
        assert_eq!(wgsl(assets.get(lh.id()).unwrap()), patched_l);
        assert_eq!(wgsl(assets.get(fh.id()).unwrap()), patched_f);
    }
    #[test]
    #[ignore = "reads explicit installed Bevy primary assets; catches actual admission panic before any mutation"]
    fn actual_asset_system_bad_second_source_refuses_before_mutating_either_asset() {
        let mut w = world();
        let (lh, fh, original_l, original_f) = {
            let mut assets = w.resource_mut::<Assets<Shader>>();
            let dep = assets.add(Shader::from_wgsl(
                "fn fixture_dependency() {}",
                "test-dependency",
            ));
            let l = shader(
                primary("pbr_lighting.wgsl"),
                "actual-primary-lighting",
                dep.clone(),
            );
            let f = shader(
                primary("pbr_functions.wgsl") + "\n// intentional exact-version mismatch\n",
                "actual-primary-functions",
                dep,
            );
            let lh = assets.add(l.clone());
            let fh = assets.add(f.clone());
            (lh, fh, l, f)
        };
        let refused = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            w.run_system_once(patch_loaded_shader).unwrap();
        }));
        assert!(refused.is_err());
        assert!(!w.resource::<Admission>().shader);
        let assets = w.resource::<Assets<Shader>>();
        let l = assets.get(lh.id()).unwrap();
        let f = assets.get(fh.id()).unwrap();
        preserved(&original_l, l);
        preserved(&original_f, f);
        assert_eq!(wgsl(&original_l), wgsl(l));
        assert_eq!(wgsl(&original_f), wgsl(f));
    }
}
