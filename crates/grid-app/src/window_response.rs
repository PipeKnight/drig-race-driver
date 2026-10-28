//! Optional window response controls; not a recovered GRID glass shader.
//! A render child follows the ordinary material owned by the damage presenter.
use bevy::{
    asset::{uuid_handle, AssetId},
    camera::{primitives::Aabb, visibility::{RenderLayers, VisibilitySystems}},
    mesh::MeshVertexBufferLayoutRef,
    pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline},
    prelude::*,
    render::render_resource::{AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError},
    shader::{ShaderImport, ShaderRef, Source},
};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Mode {
    #[default]
    Off,
    Control,
    Environment,
    Depth,
    Combined,
    /// Authored pane-only dielectric response; one stock PBR evaluation.
    Fresnel,
    /// Neutral wrapper using the native camera bias for the base RGBA atlas.
    BaseMipControl,
    /// Base RGBA atlas alone uses bias0; filtered alpha intentionally changes.
    BaseMipZero,
}
impl Mode {
    pub fn parse(value: &str) -> Result<Self, &'static str> {
        match value {
            "off" => Ok(Self::Off),
            "control" => Ok(Self::Control),
            "environment" => Ok(Self::Environment),
            "depth" => Ok(Self::Depth),
            "combined" => Ok(Self::Combined),
            "fresnel" => Ok(Self::Fresnel),
            "base-mip-control" => Ok(Self::BaseMipControl),
            "base-mip-zero" => Ok(Self::BaseMipZero),
            _ => Err("--glass-pilot needs off, control, environment, depth, combined, fresnel, base-mip-control or base-mip-zero"),
        }
    }
    pub(crate) fn base_mip(self) -> bool {
        matches!(self, Self::BaseMipControl | Self::BaseMipZero)
    }
    fn environment(self) -> bool {
        matches!(self, Self::Environment | Self::Combined)
    }
    fn depth(self) -> bool {
        matches!(self, Self::Depth | Self::Combined)
    }
}
const SHADER: Handle<Shader> = uuid_handle!("aba7a236-7189-4a8c-a47b-569ab0754806");
pub(crate) type Window = ExtendedMaterial<StandardMaterial, WindowLayer>;

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
#[bind_group_data(WindowKey)]
pub struct WindowLayer {
    // Legacy environment blend, roughness, pane F0 increment, inverse pane-alpha span.
    #[uniform(100)]
    controls: Vec4,
    write_depth: bool,
    base_mip: bool,
    base_mip_zero: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct WindowKey {
    write_depth: bool,
    base_mip: bool,
    base_mip_zero: bool,
}
impl From<&WindowLayer> for WindowKey {
    fn from(value: &WindowLayer) -> Self {
        Self { write_depth: value.write_depth, base_mip: value.base_mip, base_mip_zero: value.base_mip_zero }
    }
}
impl MaterialExtension for WindowLayer {
    fn fragment_shader() -> ShaderRef { SHADER.into() }
    fn specialize(
        _: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _: &MeshVertexBufferLayoutRef,
        key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if key.bind_group_data.base_mip {
            let fragment = descriptor.fragment.as_mut().expect("window diagnostic has a fragment shader");
            fragment.shader_defs.push("GRID_WINDOW_BASE_MIP_DIAGNOSTIC".into());
            if key.bind_group_data.base_mip_zero {
                fragment.shader_defs.push("GRID_WINDOW_BASE_MIP_ZERO".into());
            }
        }
        if key.bind_group_data.write_depth
            && let Some(depth) = descriptor.depth_stencil.as_mut()
        {
            depth.depth_write_enabled = Some(true);
        }
        Ok(())
    }
}
pub fn install(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    app.world_mut().resource_mut::<Assets<Shader>>().insert(
        SHADER.id(), Shader::from_wgsl(include_str!("window_response.wgsl"), "grid_window_response.wgsl"),
    )?;
    let base_mip = app.world().resource::<super::graphics::Settings>().window_pilot.base_mip();
    if base_mip {
        // Clone only after the existing pinned normal/clearcoat repair.
        app.add_systems(Update, prepare_base_mip_library.after(super::clearcoat_prepass::patch_loaded_library));
    }
    app.init_resource::<Cache>()
        .add_plugins(MaterialPlugin::<Window>::default())
        .add_systems(PostStartup, attach)
        .add_systems(PostUpdate, sync.before(VisibilitySystems::CheckVisibility));
    Ok(())
}

// The Bevy0.19.1 library is MIT OR Apache-2.0. Clone its loaded Shader (including
// source notices, dependencies and validation metadata), never the global asset.
// Only a new import name and ONE base-color sampling argument are changed.
const BASE_MIP_LIBRARY: Handle<Shader> = uuid_handle!("34845318-ed28-426b-8989-a04d686ed836");
const STOCK_IMPORT: &str = "#define_import_path bevy_pbr::pbr_fragment";
const BASE_MIP_IMPORT: &str = "#define_import_path grid_window_base_mip_pbr_fragment";
const BASE_SAMPLE_END: &str = "#else   // MESHLET_MESH_MATERIAL_PASS\n                bias.mip_bias,\n#endif  // MESHLET_MESH_MATERIAL_PASS\n        );\n\n#ifdef ALPHA_TO_COVERAGE";
const BASE_SAMPLE_ZERO: &str = "#else   // MESHLET_MESH_MATERIAL_PASS\n#ifdef GRID_WINDOW_BASE_MIP_ZERO\n                0.0,\n#else\n                bias.mip_bias,\n#endif\n#endif  // MESHLET_MESH_MATERIAL_PASS\n        );\n\n#ifdef ALPHA_TO_COVERAGE";

fn base_mip_library(stock: &Shader) -> Result<Shader, &'static str> {
    let Source::Wgsl(source) = &stock.source else { return Err("stock PBR library is not WGSL"); };
    if stock.import_path != ShaderImport::Custom("bevy_pbr::pbr_fragment".into())
        || source.matches(STOCK_IMPORT).count() != 1 || source.matches(BASE_SAMPLE_END).count() != 1
    {
        return Err("pinned Bevy0.19.1 base RGBA signature absent or ambiguous");
    }
    let mut shader = stock.clone();
    shader.source = Source::Wgsl(source.replacen(STOCK_IMPORT, BASE_MIP_IMPORT, 1)
        .replacen(BASE_SAMPLE_END, BASE_SAMPLE_ZERO, 1).into());
    shader.import_path = ShaderImport::Custom("grid_window_base_mip_pbr_fragment".into());
    shader.path = "grid_window_base_mip_bevy_0_19_1.wgsl".into();
    Ok(shader)
}

fn prepare_base_mip_library(
    mut shaders: ResMut<Assets<Shader>>,
    settings: Res<super::graphics::Settings>,
    mut prepared: Local<bool>,
    mut waiting: Local<bool>,
) {
    if *prepared { return; }
    let targets: Vec<_> = shaders.iter().filter_map(|(id, shader)|
        (shader.import_path == ShaderImport::Custom("bevy_pbr::pbr_fragment".into())).then_some(id)
    ).collect();
    if targets.is_empty() {
        if !*waiting {
            println!("window_base_mip library_ready=false reason=stock-shader-pending");
            *waiting = true;
        }
        return;
    }
    assert_eq!(targets.len(), 1, "window base-mip diagnostic requires one pinned stock PBR library");
    let shader = base_mip_library(shaders.get(targets[0]).expect("selected library exists"))
        .expect("window base-mip diagnostic must fail on an unrecognized Bevy shader");
    shaders.insert(BASE_MIP_LIBRARY.id(), shader).expect("authored library UUID is valid");
    *prepared = true;
    println!("window_base_mip library_ready=true mode={:?} bevy=0.19.1 scoped_module=true base_rgba_only=true other_texture_bias=camera alpha_blend_depth_state=unchanged filtered_alpha_may_change=true gpu_shader_compiled=unproved", settings.window_pilot);
}

/// Default-off scoped diagnostic: refresh a retained transparent item's world
/// centre through MeshStream, without changing its material or depth state.
pub(crate) fn install_current_sort(app: &mut App) {
    // Last follows source transforms/deformation and proxy sync in PostUpdate.
    // Extraction then hands the local centre to the existing pre-sort adapter.
    app.add_systems(Last, queue_current_sort_centres);
    println!("window_sort enabled=true scope=car0-window-glass current_local_bounds=true source_or_proxy=true pbr_alpha_depth_geometry=unchanged");
}
#[allow(clippy::type_complexity)] // Explicit source-window ownership, excluding opponents.
fn queue_current_sort_centres(
    stream: Res<super::mesh_stream::MeshStream>,
    sources: Query<(Entity, &super::track_surface::SourcePart, &Aabb),
        (With<Mesh3d>, Without<super::OpponentPart>)>,
    proxies: Query<(Entity, &Proxy, &Aabb)>,
) {
    let mut selected: HashMap<Entity, Vec3> = sources.iter()
        .filter(|(_, part, _)| part.0 == "car0/Window Glass")
        .map(|(entity, _, bounds)| (entity, bounds.center.into()))
        .collect();
    for (entity, proxy, bounds) in &proxies {
        if selected.remove(&proxy.0).is_some() {
            // A proxy uses the source's mesh with child-local identity transform.
            // Pass LOCAL coordinates; MeshStream applies the current render-world
            // world_from_local. Passing a world point here would transform twice.
            stream.sort_centre(entity, bounds.center.into());
        }
    }
    for (entity, local_centre) in selected {
        // Off material pilot: the original source is the actual rendered draw.
        // Hidden sources are not also queued when their proxy was selected above.
        stream.sort_centre(entity, local_centre);
    }
}

/// Bounded diagnostic for the standard-material car path, including lenses.
/// Keep extended glass on its independent pilot; no material/depth changes.
pub(crate) fn install_current_car_sort(app: &mut App) {
    app.add_systems(Last, queue_current_car_sort_centres);
    println!("car_transparent_sort enabled=true scope=car0-standard-blend current_local_bounds=true material_alpha_depth_geometry=unchanged");
}
#[allow(clippy::type_complexity)] // Material alpha and explicit source ownership.
fn queue_current_car_sort_centres(
    stream: Res<super::mesh_stream::MeshStream>,
    materials: Res<Assets<StandardMaterial>>,
    sources: Query<(Entity, &super::track_surface::SourcePart, &Aabb, &MeshMaterial3d<StandardMaterial>),
        (With<Mesh3d>, Without<super::OpponentPart>)>,
) {
    for (entity, part, bounds, material) in &sources {
        if part.0.starts_with("car0/")
            && materials.get(&material.0).is_some_and(|m| m.alpha_mode == AlphaMode::Blend)
        {
            stream.sort_centre(entity, bounds.center.into());
        }
    }
}

#[derive(Component)]
struct HiddenSource;
#[derive(Component)]
struct Proxy(Entity);
#[derive(Resource, Default)]
struct Cache(HashMap<(AssetId<StandardMaterial>, Mode), Handle<Window>>);
impl Cache {
    fn material(&mut self, original: &Handle<StandardMaterial>, settings: &super::graphics::Settings,
        source: &Assets<StandardMaterial>, materials: &mut Assets<Window>) -> Option<Handle<Window>> {
        let key = (original.id(), settings.window_pilot);
        if let Some(handle) = self.0.get(&key) { return Some(handle.clone()); }
        let base = source.get(original)?.clone();
        let handle = materials.add(Window {
            base,
            extension: WindowLayer {
                // Authored, source-motivated probe; not an exact GRID BRDF.
                controls: Vec4::new(
                    if settings.window_pilot.environment() { 0.24313727 } else { 0. },
                    settings.window_roughness,
                    // Bounded artistic F0 increment, not saved GRID shader parity
                    // or physical IOR1.5: default pane F0 .04 becomes .08.
                    if settings.window_pilot == Mode::Fresnel { 0.04 } else { 0. },
                    // Saved clear-pane alpha165/255; opaque atlas borders must
                    // retain their original response. Filtering fades this gate.
                    if settings.window_pilot == Mode::Fresnel { 255. / 90. } else { 0. },
                ),
                write_depth: settings.window_pilot.depth(),
                base_mip: settings.window_pilot.base_mip(),
                base_mip_zero: settings.window_pilot == Mode::BaseMipZero,
            },
        });
        self.0.insert(key, handle.clone());
        Some(handle)
    }
}
#[allow(clippy::type_complexity)] // Explicit source/proxy ownership and opponent exclusion in Bevy query.
fn attach(
    mut commands: Commands,
    settings: Res<super::graphics::Settings>,
    parts: Query<(Entity, &super::track_surface::SourcePart, &MeshMaterial3d<StandardMaterial>, &Mesh3d, &Aabb), (Without<super::OpponentPart>, Without<HiddenSource>)>,
    source: Res<Assets<StandardMaterial>>,
    mut materials: ResMut<Assets<Window>>,
    mut cache: ResMut<Cache>,
) {
    if settings.window_pilot == Mode::Off { return; }
    let mut count = 0;
    for (entity, part, original, mesh, bounds) in &parts {
        if part.0 != "car0/Window Glass" { continue; }
        let Some(handle) = cache.material(&original.0, &settings, &source, &mut materials) else { continue; };
        // Retain Mesh3d and StandardMaterial on the source entity so the
        // established damage/reset/deformation queries retain their ownership.
        commands.entity(entity).insert((HiddenSource, RenderLayers::none()));
        commands.spawn((
            Name::new("window response render proxy"), Proxy(entity), ChildOf(entity),
            Mesh3d(mesh.0.clone()), MeshMaterial3d(handle), *bounds,
            Transform::IDENTITY, Visibility::Inherited,
            if settings.reflection_mode == 3 { RenderLayers::layer(1) } else { RenderLayers::default() },
        ));
        count += 1;
    }
    println!("window_response attached_parts={count} mode={:?} source_parity=false textures_alpha_geometry=unchanged scope=player-window-render-proxy", settings.window_pilot);
    if settings.window_pilot == Mode::Fresnel {
        println!("window_fresnel stock_pbr_evaluations=1 normal_f0_increment=0.04 f0_max=0.16 clear_alpha_reference=165/255 opaque_frame_increment=0 depth_write=false physical_ior_parity=false");
    }
}
#[allow(clippy::type_complexity)] // Source and proxy access are explicitly disjoint.
fn sync(
    settings: Res<super::graphics::Settings>,
    source_materials: Res<Assets<StandardMaterial>>,
    mut materials: ResMut<Assets<Window>>,
    mut cache: ResMut<Cache>,
    mut sources: Query<(&MeshMaterial3d<StandardMaterial>, &Mesh3d, &Aabb, &mut RenderLayers), With<HiddenSource>>,
    mut proxies: Query<(&Proxy, &mut MeshMaterial3d<Window>, &mut Mesh3d, &mut Aabb), Without<HiddenSource>>,
) {
    for (proxy, mut material, mut mesh, mut bounds) in &mut proxies {
        let Ok((original, source_mesh, source_bounds, mut layers)) = sources.get_mut(proxy.0) else { continue; };
        // Probe setup runs in Update and may assign the source's layers once.
        // Hide it before visibility extraction; only the child renders.
        if *layers != RenderLayers::none() { *layers = RenderLayers::none(); }
        if let Some(handle) = cache.material(&original.0, &settings, &source_materials, &mut materials)
            && material.0 != handle { material.0 = handle; }
        if mesh.0 != source_mesh.0 { mesh.0 = source_mesh.0.clone(); }
        if bounds.center != source_bounds.center || bounds.half_extents != source_bounds.half_extents { *bounds = *source_bounds; }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scoped_base_mip_shader_preserves_stock_and_unrelated_sampling() {
        let input = format!("{STOCK_IMPORT}\n#import bevy_pbr::pbr_types\n// upstream notice remains\n{BASE_SAMPLE_END}\nnormal = textureSampleBias(normal_map, normal_sampler, uv, bias.mip_bias);\n");
        let mut stock = Shader::from_wgsl(input.clone(), "pinned-pbr-test.wgsl");
        stock.shader_defs.push("UNCHANGED_TEST_DEFINITION".into());
        stock.file_dependencies.push(uuid_handle!("794d1627-bdb9-4fd6-8261-695dc17c6f81"));
        let variant = base_mip_library(&stock).unwrap();
        let Source::Wgsl(original) = &stock.source else { panic!("WGSL expected"); };
        let Source::Wgsl(changed) = &variant.source else { panic!("WGSL expected"); };
        assert_eq!(original.as_ref(), input);
        assert_eq!(changed.matches("0.0,").count(), 1);
        assert!(changed.contains("normal = textureSampleBias(normal_map, normal_sampler, uv, bias.mip_bias);"));
        assert_eq!(changed.replace(BASE_MIP_IMPORT, STOCK_IMPORT).replace(BASE_SAMPLE_ZERO, BASE_SAMPLE_END), input);
        assert_eq!(variant.imports, stock.imports);
        assert_eq!(variant.shader_defs, stock.shader_defs);
        assert_eq!(variant.file_dependencies, stock.file_dependencies);
        assert_eq!(std::mem::discriminant(&variant.validate_shader), std::mem::discriminant(&stock.validate_shader));
        assert_ne!(variant.import_path, stock.import_path);
        let duplicate = Shader::from_wgsl(format!("{input}\n{BASE_SAMPLE_END}"), "wrong-version.wgsl");
        assert!(base_mip_library(&duplicate).is_err());
        assert!(base_mip_library(&Shader::from_wgsl(STOCK_IMPORT, "missing-base-sample.wgsl")).is_err());
    }

    #[test]
    fn base_mip_modes_do_not_alias_the_same_source_material_in_the_cache() {
        let mut source = Assets::<StandardMaterial>::default();
        let original = source.add(StandardMaterial { alpha_mode: AlphaMode::Blend, ..default() });
        let mut materials = Assets::<Window>::default();
        let mut cache = Cache::default();
        let mut settings = super::super::graphics::Settings { window_pilot: Mode::BaseMipControl, ..default() };
        let control = cache.material(&original, &settings, &source, &mut materials).unwrap();
        settings.window_pilot = Mode::BaseMipZero;
        let zero = cache.material(&original, &settings, &source, &mut materials).unwrap();
        assert_ne!(control, zero);
        let c = &materials.get(&control).unwrap().extension;
        let z = &materials.get(&zero).unwrap().extension;
        assert_eq!(c.controls, z.controls);
        assert!(!c.write_depth && !z.write_depth);
        assert!(c.base_mip && z.base_mip && !c.base_mip_zero && z.base_mip_zero);
        assert_ne!(WindowKey::from(c), WindowKey::from(z));
        assert_eq!(materials.get(&control).unwrap().base.alpha_mode, AlphaMode::Blend);
        assert_eq!(materials.get(&zero).unwrap().base.alpha_mode, AlphaMode::Blend);
        settings.window_pilot = Mode::BaseMipControl;
        assert_eq!(cache.material(&original, &settings, &source, &mut materials).unwrap(), control);
    }

    #[test]
    fn car_sort_tracks_lenses_and_material_reset_without_touching_opaque_or_other_cars() {
        let mut app = App::new();
        super::super::mesh_stream::install(&mut app);
        app.init_resource::<Assets<StandardMaterial>>();
        app.add_systems(Last, queue_current_car_sort_centres);
        let blend = app.world_mut().resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial { alpha_mode: AlphaMode::Blend, ..default() });
        let opaque = app.world_mut().resource_mut::<Assets<StandardMaterial>>().add(StandardMaterial::default());
        let bounds = Aabb { center: Vec3::new(1., 2., 3.).into(), half_extents: Vec3::ONE.into() };
        let mut ids = Vec::new();
        for (part, material) in [("car0/Rear Lights Glass", blend.clone()),
            ("car0/Window Glass", blend.clone()), ("car0/Paint", opaque.clone()),
            ("car1/Window Glass", blend.clone()), ("track/Window Glass", blend.clone())] {
            ids.push(app.world_mut().spawn((super::super::track_surface::SourcePart(part.into()),
                Mesh3d(Handle::default()), MeshMaterial3d(material), bounds, Transform::from_xyz(100., 0., 200.))).id());
        }
        app.update();
        let stream = app.world().resource::<super::super::mesh_stream::MeshStream>();
        for id in &ids[..2] { assert_eq!(stream.queued_centre(*id), Some(Vec3::new(1., 2., 3.))); }
        for id in &ids[2..] { assert_eq!(stream.queued_centre(*id), None); }
        app.world_mut().get_mut::<Aabb>(ids[0]).unwrap().center = Vec3::new(4., 5., 6.).into();
        app.world_mut().get_mut::<MeshMaterial3d<StandardMaterial>>(ids[1]).unwrap().0 = opaque;
        app.update();
        let stream = app.world().resource::<super::super::mesh_stream::MeshStream>();
        assert_eq!(stream.queued_centre(ids[0]), Some(Vec3::new(4., 5., 6.)));
        assert_eq!(stream.queued_centre(ids[1]), None);
        app.world_mut().get_mut::<MeshMaterial3d<StandardMaterial>>(ids[1]).unwrap().0 = blend;
        app.update();
        assert_eq!(app.world().resource::<super::super::mesh_stream::MeshStream>().queued_centre(ids[1]), Some(Vec3::new(1., 2., 3.)));
    }

    #[test]
    fn current_sort_routes_only_player_windows_to_one_render_entity() {
        let mut app = App::new();
        // The real no-render MeshStream adapter clears pending writes in First.
        // Thus a later assertion cannot accidentally pass on last frame's entry.
        super::super::mesh_stream::install(&mut app);
        app.add_systems(Last, queue_current_sort_centres);
        let bounds = Aabb { center: Vec3::new(1., 2., 3.).into(), half_extents: Vec3::ONE.into() };
        let direct = app.world_mut().spawn((
            super::super::track_surface::SourcePart("car0/Window Glass".into()),
            Mesh3d(Handle::default()), bounds, Transform::IDENTITY,
        )).id();
        let hidden_source = app.world_mut().spawn((
            super::super::track_surface::SourcePart("car0/Window Glass".into()),
            Mesh3d(Handle::default()), bounds, Transform::IDENTITY,
        )).id();
        let proxy_bounds = Aabb { center: Vec3::new(4., 5., 6.).into(), ..bounds };
        let proxy = app.world_mut().spawn((
            Proxy(hidden_source), proxy_bounds, ChildOf(hidden_source), Transform::IDENTITY,
        )).id();
        let opponent = app.world_mut().spawn((
            super::super::track_surface::SourcePart("car1/Window Glass".into()),
            Mesh3d(Handle::default()), bounds,
        )).id();
        let lamp = app.world_mut().spawn((
            super::super::track_surface::SourcePart("car0/Head Lights Glass".into()),
            Mesh3d(Handle::default()), bounds,
        )).id();
        app.update();
        let stream = app.world().resource::<super::super::mesh_stream::MeshStream>();
        assert_eq!(stream.queued_centre(direct), Some(Vec3::new(1., 2., 3.)));
        assert_eq!(stream.queued_centre(proxy), Some(Vec3::new(4., 5., 6.)));
        assert_eq!(stream.queued_centre(hidden_source), None);
        assert_eq!(stream.queued_centre(opponent), None);
        assert_eq!(stream.queued_centre(lamp), None);
        // Moving the car must not turn the LOCAL centre into a world point.
        app.world_mut().get_mut::<Transform>(direct).unwrap().translation = Vec3::new(100., -20., 300.);
        app.world_mut().get_mut::<Transform>(hidden_source).unwrap().translation = Vec3::new(-50., -10., 200.);
        app.world_mut().get_mut::<Aabb>(direct).unwrap().center = Vec3::new(7., 8., 9.).into();
        app.update();
        let stream = app.world().resource::<super::super::mesh_stream::MeshStream>();
        assert_eq!(stream.queued_centre(direct), Some(Vec3::new(7., 8., 9.)));
        assert_eq!(stream.queued_centre(proxy), Some(Vec3::new(4., 5., 6.)));
        assert_eq!(stream.queued_centre(hidden_source), None);
        // Reset presentation to its initial transform/bounds. First clears each
        // previous frame, so unchanged proxy bounds still need a fresh emission.
        *app.world_mut().get_mut::<Transform>(direct).unwrap() = Transform::IDENTITY;
        *app.world_mut().get_mut::<Transform>(hidden_source).unwrap() = Transform::IDENTITY;
        *app.world_mut().get_mut::<Aabb>(direct).unwrap() = bounds;
        app.update();
        let stream = app.world().resource::<super::super::mesh_stream::MeshStream>();
        assert_eq!(stream.queued_centre(direct), Some(Vec3::new(1., 2., 3.)));
        assert_eq!(stream.queued_centre(proxy), Some(Vec3::new(4., 5., 6.)));
        assert_eq!(stream.queued_centre(hidden_source), None);
    }

    #[test]
    fn proxy_tracks_crack_reset_mesh_bounds_and_keeps_source_ownership() {
        // The new response must retain the same damage-owned source material,
        // proxy identity and reset cache as the existing combined reference.
        for mode in [Mode::Combined, Mode::Fresnel, Mode::BaseMipControl, Mode::BaseMipZero] {
            check_proxy_contract(mode);
        }
    }

    fn check_proxy_contract(mode: Mode) {
        let mut app = App::new();
        app.insert_resource(super::super::graphics::Settings { window_pilot: mode, reflection_mode: 3, ..default() })
            .init_resource::<Assets<StandardMaterial>>().init_resource::<Assets<Window>>().init_resource::<Cache>()
            .add_systems(Update, attach).add_systems(PostUpdate, sync);
        let original = app.world_mut().resource_mut::<Assets<StandardMaterial>>().add(StandardMaterial {
            base_color: Color::srgba(0.2, 0.2, 0.2, 0.65), base_color_texture: Some(Handle::default()), alpha_mode: AlphaMode::Blend,
            double_sided: true, cull_mode: None, ..default()
        });
        let mut ids = Vec::new();
        for name in ["car0/Window Glass", "car0/Window Glass", "car0/Head Lights Glass", "car0/Car Body Paint", "car1/Window Glass"] {
            ids.push(app.world_mut().spawn((super::super::track_surface::SourcePart(name.into()), MeshMaterial3d(original.clone()), Mesh3d(Handle::default()), Visibility::Inherited, Aabb::from_min_max(Vec3::ZERO, Vec3::ONE))).id());
        }
        app.update();
        let children: Vec<_> = app.world_mut().query::<(Entity, &Proxy)>().iter(app.world()).map(|(e,p)| (e,p.0)).collect();
        assert_eq!(children.len(), 2);
        let child = children.iter().find(|(_, p)| *p == ids[0]).unwrap().0;
        let other = children.iter().find(|(_, p)| *p == ids[1]).unwrap().0;
        let baseline = app.world().get::<MeshMaterial3d<Window>>(child).unwrap().0.clone();
        assert_eq!(baseline, app.world().get::<MeshMaterial3d<Window>>(other).unwrap().0);
        assert_eq!(app.world().get::<ChildOf>(child).unwrap().parent(), ids[0]);
        assert_eq!(*app.world().get::<Transform>(child).unwrap(), Transform::IDENTITY);
        assert_eq!(*app.world().get::<RenderLayers>(child).unwrap(), RenderLayers::layer(1));
        for id in &ids {
            assert_eq!(app.world().get::<MeshMaterial3d<StandardMaterial>>(*id).unwrap().0, original);
            assert!(app.world().get::<Mesh3d>(*id).is_some());
        }
        for id in &ids[2..] { assert!(app.world().get::<HiddenSource>(*id).is_none()); }
        let base = &app.world().resource::<Assets<Window>>().get(&baseline).unwrap().base;
        assert_eq!(base.base_color, Color::srgba(0.2, 0.2, 0.2, 0.65));
        assert_eq!(base.base_color_texture, Some(Handle::default()));
        assert_eq!(base.alpha_mode, AlphaMode::Blend);
        assert!(base.double_sided);
        assert_eq!(base.cull_mode, None);
        assert_eq!(base.reflectance, 0.5); // Only the fragment's resolved input changes.
        let layer = &app.world().resource::<Assets<Window>>().get(&baseline).unwrap().extension;
        assert_eq!(layer.write_depth, mode == Mode::Combined);
        assert_eq!(layer.controls.x > 0., mode == Mode::Combined);
        assert_eq!(layer.controls.z > 0., mode == Mode::Fresnel);
        assert_eq!(layer.base_mip, mode.base_mip());
        assert_eq!(layer.base_mip_zero, mode == Mode::BaseMipZero);
        let cracked = app.world_mut().resource_mut::<Assets<StandardMaterial>>().add(StandardMaterial { base_color: Color::srgba(0.4,0.4,0.4,0.9), ..default() });
        app.world_mut().get_mut::<MeshMaterial3d<StandardMaterial>>(ids[0]).unwrap().0 = cracked;
        let replacement: Handle<Mesh> = uuid_handle!("4b9e28c4-67eb-4adb-83f7-f82ee847e58c");
        app.world_mut().get_mut::<Mesh3d>(ids[0]).unwrap().0 = replacement.clone();
        *app.world_mut().get_mut::<Aabb>(ids[0]).unwrap() = Aabb::from_min_max(Vec3::ZERO, Vec3::splat(2.));
        *app.world_mut().get_mut::<Visibility>(ids[0]).unwrap() = Visibility::Hidden;
        app.update();
        let changed = &app.world().get::<MeshMaterial3d<Window>>(child).unwrap().0;
        assert_ne!(*changed, baseline);
        assert_eq!(app.world().resource::<Assets<Window>>().get(changed).unwrap().base.base_color, Color::srgba(0.4,0.4,0.4,0.9));
        assert_eq!(app.world().get::<Mesh3d>(child).unwrap().0, replacement);
        assert_eq!(app.world().get::<Aabb>(child).unwrap().half_extents, bevy::math::Vec3A::ONE);
        assert_eq!(*app.world().get::<Visibility>(child).unwrap(), Visibility::Inherited);
        // The damage presenter restores the original source handle and visibility.
        app.world_mut().get_mut::<MeshMaterial3d<StandardMaterial>>(ids[0]).unwrap().0 = original;
        *app.world_mut().get_mut::<Visibility>(ids[0]).unwrap() = Visibility::Inherited;
        app.update();
        assert_eq!(app.world().get::<MeshMaterial3d<Window>>(child).unwrap().0, baseline);
        assert_eq!(app.world().resource::<Assets<Window>>().len(), 2);
        assert_eq!(app.world_mut().query::<&Proxy>().iter(app.world()).count(), 2);
    }
}
