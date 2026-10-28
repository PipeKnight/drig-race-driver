//! Default-off authored environment reflection experiment for player paint.
//! Source constants motivate a separate lobe; this is not a recovered GRID BRDF.
use bevy::{
    asset::uuid_handle,
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};
use std::collections::HashMap;

const SHADER: Handle<Shader> = uuid_handle!("e5c3fc7a-25d5-42f6-9182-bfab9aeb0c20");
type Paint = ExtendedMaterial<StandardMaterial, EnvironmentLayer>;

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct EnvironmentLayer {
    // Gain, authored normal/grazing weights, perceptual roughness.
    #[uniform(100)]
    controls: Vec4,
}
impl MaterialExtension for EnvironmentLayer {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }
}

pub fn install(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    app.world_mut().resource_mut::<Assets<Shader>>().insert(
        SHADER.id(),
        Shader::from_wgsl(
            include_str!("paint_environment.wgsl"),
            "grid_paint_environment.wgsl",
        ),
    )?;
    app.add_plugins(MaterialPlugin::<Paint>::default())
        .add_systems(PostStartup, attach);
    Ok(())
}

fn attach(
    mut commands: Commands,
    settings: Res<super::graphics::Settings>,
    parts: Query<(
        Entity,
        &super::track_surface::SourcePart,
        &MeshMaterial3d<StandardMaterial>,
    )>,
    source: Res<Assets<StandardMaterial>>,
    mut materials: ResMut<Assets<Paint>>,
) {
    let mut handles = HashMap::new();
    let mut count = 0;
    for (entity, part, original) in &parts {
        if part.0 != "car0/Car Body Paint" {
            continue;
        }
        let Some(base) = source.get(&original.0) else {
            continue;
        };
        let handle = handles
            .entry(original.0.id())
            .or_insert_with(|| {
                materials.add(Paint {
                    base: base.clone(),
                    extension: EnvironmentLayer {
                        // GRID's saved environment colour/Fresnel values are a starting
                        // point only: direct-light response and exact source gates differ.
                        controls: Vec4::new(
                            settings.paint_environment,
                            0.16117648,
                            0.32235296,
                            settings.paint_coat_roughness,
                        ),
                    },
                })
            })
            .clone();
        commands
            .entity(entity)
            .remove::<MeshMaterial3d<StandardMaterial>>()
            .insert(MeshMaterial3d(handle));
        count += 1;
    }
    println!(
        "paint_environment attached_parts={count} gain={} scope=player-paint-environment-only source_parity=false stock_brdf=retained top_layer=blend depth_motion=stock textures=shared",
        settings.paint_environment
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attachment_preserves_shared_materials_and_only_replaces_player_paint() {
        let mut app = App::new();
        let settings = super::super::graphics::Settings {
            paint_environment: 0.5,
            ..default()
        };
        app.insert_resource(settings)
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<Paint>>()
            .add_systems(Update, attach);
        let original = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial {
                base_color: Color::srgb(0.8, 0.1, 0.2),
                clearcoat: 0.35,
                perceptual_roughness: 0.3,
                ..default()
            });
        let mut entities = Vec::new();
        for part in [
            "car0/Car Body Paint",
            "car0/Car Body Paint",
            "car1/Car Body Paint",
            "car0/Tyre Wall",
        ] {
            entities.push(
                app.world_mut()
                    .spawn((
                        super::super::track_surface::SourcePart(part.to_owned()),
                        MeshMaterial3d(original.clone()),
                        Mesh3d(Handle::default()),
                    ))
                    .id(),
            );
        }
        app.update();
        let a = app
            .world()
            .get::<MeshMaterial3d<Paint>>(entities[0])
            .unwrap();
        let b = app
            .world()
            .get::<MeshMaterial3d<Paint>>(entities[1])
            .unwrap();
        assert_eq!(a.0.id(), b.0.id());
        let own = app.world().resource::<Assets<Paint>>().get(&a.0).unwrap();
        let old = app
            .world()
            .resource::<Assets<StandardMaterial>>()
            .get(&original)
            .unwrap();
        assert_eq!(own.base.base_color, old.base_color);
        assert_eq!(own.base.clearcoat, old.clearcoat);
        assert_eq!(own.base.perceptual_roughness, old.perceptual_roughness);
        for &entity in &entities[..2] {
            assert!(
                app.world()
                    .get::<MeshMaterial3d<StandardMaterial>>(entity)
                    .is_none()
            );
            assert!(app.world().get::<Mesh3d>(entity).is_some());
        }
        for &entity in &entities[2..] {
            assert_eq!(
                app.world()
                    .get::<MeshMaterial3d<StandardMaterial>>(entity)
                    .unwrap()
                    .0
                    .id(),
                original.id()
            );
            assert!(app.world().get::<MeshMaterial3d<Paint>>(entity).is_none());
        }
    }
}
