//! Static-only window response controls; not a recovered GRID glass shader.
//! Driving is rejected until cracked/reset material ownership is integrated.
use bevy::{
    asset::uuid_handle,
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
}
impl Mode {
    pub fn parse(value: &str) -> Result<Self, &'static str> {
        match value {
            "off" => Ok(Self::Off),
            "control" => Ok(Self::Control),
            "environment" => Ok(Self::Environment),
            "depth" => Ok(Self::Depth),
            "combined" => Ok(Self::Combined),
            _ => Err("--glass-pilot needs off, control, environment, depth or combined"),
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
    // Environment blend, perceptual roughness, padding.
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
    app.add_plugins(MaterialPlugin::<Window>::default()).add_systems(PostStartup, attach);
    Ok(())
}
fn attach(
    mut commands: Commands,
    settings: Res<super::graphics::Settings>,
    parts: Query<(Entity, &super::track_surface::SourcePart, &MeshMaterial3d<StandardMaterial>), Without<super::OpponentPart>>,
    source: Res<Assets<StandardMaterial>>,
    mut materials: ResMut<Assets<Window>>,
) {
    if settings.window_pilot == Mode::Off { return; }
    let mut handles = HashMap::new();
    let mut count = 0;
    for (entity, part, original) in &parts {
        if part.0 != "car0/Window Glass" { continue; }
        let Some(base) = source.get(&original.0) else { continue; };
        let handle = handles.entry(original.0.id()).or_insert_with(|| materials.add(Window {
            base: base.clone(),
            extension: WindowLayer {
                // Saved window environment colour * midpoint of saved Fresnel
                // bounds motivates this static probe, not an exact BRDF port.
                controls: Vec4::new(if settings.window_pilot.environment() { 0.24313727 } else { 0. }, settings.window_roughness, 0., 0.),
                write_depth: settings.window_pilot.depth(),
            },
        })).clone();
        commands.entity(entity).remove::<MeshMaterial3d<StandardMaterial>>().insert(MeshMaterial3d(handle));
        count += 1;
    }
    println!("window_response attached_parts={count} mode={:?} source_parity=false textures_alpha_geometry=unchanged scope=static-player-window-only", settings.window_pilot);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn window_attachment_keeps_alpha_textures_and_excludes_lamps_paint_and_opponents() {
        let mut app = App::new();
        app.insert_resource(super::super::graphics::Settings { window_pilot: Mode::Combined, ..default() })
            .init_resource::<Assets<StandardMaterial>>().init_resource::<Assets<Window>>().add_systems(Update, attach);
        let texture = Handle::<Image>::default();
        let original = app.world_mut().resource_mut::<Assets<StandardMaterial>>().add(StandardMaterial {
            base_color: Color::srgba(0.2, 0.2, 0.2, 0.65), base_color_texture: Some(texture.clone()), alpha_mode: AlphaMode::Blend,
            double_sided: true, cull_mode: None, ..default()
        });
        let mut ids = Vec::new();
        for name in ["car0/Window Glass", "car0/Window Glass", "car0/Head Lights Glass", "car0/Car Body Paint", "car1/Window Glass"] {
            ids.push(app.world_mut().spawn((super::super::track_surface::SourcePart(name.into()), MeshMaterial3d(original.clone()), Mesh3d(Handle::default()))).id());
        }
        app.update();
        let a = app.world().get::<MeshMaterial3d<Window>>(ids[0]).unwrap();
        let b = app.world().get::<MeshMaterial3d<Window>>(ids[1]).unwrap();
        assert_eq!(a.0.id(), b.0.id());
        let window = app.world().resource::<Assets<Window>>().get(&a.0).unwrap();
        assert_eq!(window.base.base_color, Color::srgba(0.2, 0.2, 0.2, 0.65));
        assert_eq!(window.base.base_color_texture, Some(texture));
        assert_eq!(window.base.alpha_mode, AlphaMode::Blend);
        assert!(window.base.double_sided);
        assert_eq!(window.base.cull_mode, None);
        assert!(window.extension.write_depth);
        for id in &ids[2..] {
            assert_eq!(app.world().get::<MeshMaterial3d<StandardMaterial>>(*id).unwrap().0.id(), original.id());
            assert!(app.world().get::<MeshMaterial3d<Window>>(*id).is_none());
        }
        assert_eq!(app.world().resource::<Assets<StandardMaterial>>().get(&original).unwrap().alpha_mode, AlphaMode::Blend);
    }
}
