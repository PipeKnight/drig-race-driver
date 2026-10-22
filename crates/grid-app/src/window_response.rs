//! Optional window response controls; not a recovered GRID glass shader.
//! A render child follows the ordinary material owned by the damage presenter.
use bevy::{
    asset::{uuid_handle, AssetId},
    camera::{primitives::Aabb, visibility::{RenderLayers, VisibilitySystems}},
    mesh::MeshVertexBufferLayoutRef,
    pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline},
    prelude::*,
    render::render_resource::{AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError},
    shader::ShaderRef,
};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Off,
    Control,
    Environment,
    Depth,
    Combined,
    /// Authored pane-only dielectric response; one stock PBR evaluation.
    Fresnel,
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
            _ => Err("--glass-pilot needs off, control, environment, depth, combined or fresnel"),
        }
    }
    fn environment(self) -> bool {
        matches!(self, Self::Environment | Self::Combined)
    }
    fn depth(self) -> bool {
        matches!(self, Self::Depth | Self::Combined)
    }
}
const SHADER: Handle<Shader> = uuid_handle!("aba7a236-7189-4a8c-a47b-569ab0754806");
type Window = ExtendedMaterial<StandardMaterial, WindowLayer>;

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
#[bind_group_data(WindowKey)]
pub struct WindowLayer {
    // Legacy environment blend, roughness, pane F0 increment, inverse pane-alpha span.
    #[uniform(100)]
    controls: Vec4,
    write_depth: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct WindowKey {
    write_depth: bool,
}
impl From<&WindowLayer> for WindowKey {
    fn from(value: &WindowLayer) -> Self {
        Self { write_depth: value.write_depth }
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
    app.init_resource::<Cache>()
        .add_plugins(MaterialPlugin::<Window>::default())
        .add_systems(PostStartup, attach)
        .add_systems(PostUpdate, sync.before(VisibilitySystems::CheckVisibility));
    Ok(())
}
#[derive(Component)]
struct HiddenSource;
#[derive(Component)]
struct Proxy(Entity);
#[derive(Resource, Default)]
struct Cache(HashMap<AssetId<StandardMaterial>, Handle<Window>>);
impl Cache {
    fn material(&mut self, original: &Handle<StandardMaterial>, settings: &super::graphics::Settings,
        source: &Assets<StandardMaterial>, materials: &mut Assets<Window>) -> Option<Handle<Window>> {
        if let Some(handle) = self.0.get(&original.id()) { return Some(handle.clone()); }
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
            },
        });
        self.0.insert(original.id(), handle.clone());
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
    fn proxy_tracks_crack_reset_mesh_bounds_and_keeps_source_ownership() {
        // The new response must retain the same damage-owned source material,
        // proxy identity and reset cache as the existing combined reference.
        for mode in [Mode::Combined, Mode::Fresnel] {
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
