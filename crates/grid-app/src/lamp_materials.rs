//! Default-off authored Lacetti inner reflectors, not a recovered source BRDF.
//! StandardMaterial is retained so existing cracked-lamp/reset code keeps working.
use bevy::{asset::RenderAssetUsages, image::ImageSampler, prelude::*};
use std::collections::HashMap;

#[derive(Resource)]
pub struct Prepared(pub Option<Handle<Image>>);

pub fn install(app: &mut App) {
    app.add_systems(PostStartup, attach);
}

// Clone only the player's draw material after shared-model/opponent setup.
// Applying this earlier would leak changes into same-model opponents.
fn attach(
    mut commands: Commands,
    settings: Res<super::graphics::Settings>,
    normal: Res<Prepared>,
    parts: Query<
        (
            Entity,
            &super::track_surface::SourcePart,
            &MeshMaterial3d<StandardMaterial>,
        ),
        Without<super::OpponentPart>,
    >,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut handles = HashMap::new();
    let mut count = 0;
    for (entity, part, original) in &parts {
        let Some(name) = part.0.strip_prefix("car0/").filter(|s| reflector(s)) else {
            continue;
        };
        // A shared base handle can need different head/rear response constants.
        let handle = handles
            .entry((original.0.id(), name.to_owned()))
            .or_insert_with(|| {
                let mut base = materials
                    .get(&original.0)
                    .expect("source lamp material")
                    .clone();
                apply(name, settings.lamp_reflectors, normal.0.as_ref(), &mut base);
                materials.add(base)
            })
            .clone();
        commands.entity(entity).insert(MeshMaterial3d(handle));
        count += 1;
    }
    println!(
        "lamp_reflector_materials attached_parts={count} standard=true opponents=unchanged covers=unchanged matt=unchanged"
    );
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Off,
    Pbr,
    Normals,
    Combined,
}
impl Mode {
    pub fn parse(value: &str) -> Result<Self, &'static str> {
        match value {
            "off" => Ok(Self::Off),
            "pbr" => Ok(Self::Pbr),
            "normals" => Ok(Self::Normals),
            "combined" => Ok(Self::Combined),
            _ => Err("--lamp-reflectors needs off, pbr, normals or combined"),
        }
    }
    fn normals(self) -> bool {
        matches!(self, Self::Normals | Self::Combined)
    }
    fn pbr(self) -> bool {
        matches!(self, Self::Pbr | Self::Combined)
    }
}
fn reflector(name: &str) -> bool {
    matches!(name, "Head Lights Pod" | "Rear Lights Pod")
}
fn player_part(name: &str) -> bool {
    reflector(name.strip_prefix("car0/").unwrap_or(name))
}
fn normal_key(car: &super::Car) -> &'static str {
    if car.track {
        "car0/lac_lights_nm.tga"
    } else {
        "lac_lights_nm.tga"
    }
}
/// Keep disabled/PBR-only mesh layouts unchanged. Only the selected player's
/// normal pilot may repair invalid saved lamp frames from geometry and UV.
pub fn prepare_frames(car: &mut super::Car, mode: Mode) -> Result<(), String> {
    for part in &mut car.parts {
        let name = part.material.rsplit('/').next().unwrap_or(&part.material);
        if !reflector(name) {
            continue;
        }
        if !mode.normals() || !player_part(&part.material) {
            part.mesh.remove_attribute(Mesh::ATTRIBUTE_TANGENT);
        } else if part.mesh.attribute(Mesh::ATTRIBUTE_TANGENT).is_none() {
            part.mesh
                .generate_tangents()
                .map_err(|e| format!("lamp tangent {}: {e:?}", part.name))?;
            println!(
                "lamp_tangent draw={:?} frame=mikktspace authored=true",
                part.name
            );
        }
    }
    Ok(())
}
/// Normal-only comparisons must not silently drop missing tangent frames.
pub fn validate(car: &super::Car, mode: Mode) -> Result<(), &'static str> {
    if !mode.normals() {
        return Ok(());
    }
    let parts: Vec<_> = car
        .parts
        .iter()
        .filter(|p| player_part(&p.material))
        .collect();
    if parts.is_empty() || !car.textures.contains_key(normal_key(car)) {
        return Err("lamp reflector normal resource/parts absent");
    }
    if parts
        .iter()
        .any(|p| p.mesh.attribute(Mesh::ATTRIBUTE_TANGENT).is_none())
    {
        return Err("lamp reflector source tangent frame unavailable");
    }
    Ok(())
}
pub fn upload_normal(
    car: &super::Car,
    mode: Mode,
    images: &mut Assets<Image>,
    sampler: ImageSampler,
) -> Option<Handle<Image>> {
    if mode == Mode::Off {
        return None;
    }
    let parts = car
        .parts
        .iter()
        .filter(|p| player_part(&p.material))
        .count();
    println!(
        "lamp_reflectors mode={mode:?} parts={parts} scope=player-inner-pods covers=unchanged matt=unchanged state_material=standard source_parity=false"
    );
    mode.normals().then(|| {
        let image = super::texture_upload::image_linear(
            &car.textures[normal_key(car)], sampler, RenderAssetUsages::default(),
        ).expect("validated linear lamp normal DDS");
        println!("lamp_reflector_normal srgb=false format={:?} frame=source-or-mikktspace approximate=true upload=shared-helper", image.texture_descriptor.format);
        images.add(image)
    })
}
fn apply(name: &str, mode: Mode, normal: Option<&Handle<Image>>, material: &mut StandardMaterial) {
    if !reflector(name) {
        return;
    }
    if mode.pbr() {
        // Authored reflector response: saved source Fresnel is not metallicness.
        // Keep the existing diffuse lens atlas and every damage/emission property.
        material.perceptual_roughness = if name == "Head Lights Pod" {
            0.18
        } else {
            0.24
        };
        material.metallic = if name == "Head Lights Pod" { 0.8 } else { 0.55 };
    }
    if mode.normals() {
        material.normal_map_texture = normal.cloned();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scope_and_controls_preserve_damage_inputs() {
        assert_eq!(Mode::default(), Mode::Off);
        for value in ["", "source", "NaN", "1"] {
            assert!(Mode::parse(value).is_err());
        }
        let image = Handle::<Image>::default();
        let base = StandardMaterial {
            base_color: Color::srgb(0.8, 0.2, 0.1),
            alpha_mode: AlphaMode::Mask(0.5),
            emissive: LinearRgba::rgb(0.2, 0.1, 0.0),
            perceptual_roughness: 0.7,
            ..default()
        };
        for mode in [Mode::Off, Mode::Pbr, Mode::Normals, Mode::Combined] {
            let mut material = base.clone();
            apply("Head Lights Pod", mode, Some(&image), &mut material);
            assert_eq!(material.base_color, base.base_color);
            assert_eq!(material.alpha_mode, base.alpha_mode);
            assert_eq!(material.emissive, base.emissive);
            assert_eq!(material.normal_map_texture.is_some(), mode.normals());
            assert_eq!(
                material.perceptual_roughness,
                if mode.pbr() { 0.18 } else { 0.7 }
            );
        }
        for name in [
            "Head Lights Glass",
            "Rear Lights Glass",
            "Lights Pod Matt",
            "Car Body Paint",
            "car1/Head Lights Pod",
        ] {
            let mut material = base.clone();
            apply(name, Mode::Combined, Some(&image), &mut material);
            assert_eq!(material.perceptual_roughness, base.perceptual_roughness);
            assert_eq!(material.normal_map_texture, base.normal_map_texture);
        }
        assert!(!player_part("car1/Head Lights Pod"));
        assert!(player_part("car0/Head Lights Pod"));
    }
    #[test]
    fn shared_base_handles_do_not_leak_to_opponents_or_rear_class() {
        let mut app = App::new();
        app.insert_resource(super::super::graphics::Settings {
            lamp_reflectors: Mode::Pbr,
            ..default()
        })
        .insert_resource(Prepared(None))
        .init_resource::<Assets<StandardMaterial>>()
        .add_systems(Update, attach);
        let original = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial {
                perceptual_roughness: 0.7,
                ..default()
            });
        let mut entities = Vec::new();
        for name in [
            "car0/Head Lights Pod",
            "car0/Head Lights Pod",
            "car0/Rear Lights Pod",
            "car0/Head Lights Glass",
            "car0/Head Lights Pod",
        ] {
            entities.push(
                app.world_mut()
                    .spawn((
                        super::super::track_surface::SourcePart(name.to_owned()),
                        MeshMaterial3d(original.clone()),
                    ))
                    .id(),
            );
        }
        app.world_mut()
            .entity_mut(entities[4])
            .insert(super::super::OpponentPart {
                car: 1,
                rest: Mat4::IDENTITY,
                wheel: None,
                lost_with_wheel: false,
            });
        app.update();
        let handles: Vec<_> = entities
            .iter()
            .map(|&e| {
                app.world()
                    .get::<MeshMaterial3d<StandardMaterial>>(e)
                    .unwrap()
                    .0
                    .clone()
            })
            .collect();
        assert_eq!(handles[0], handles[1]);
        assert_ne!(handles[0], handles[2]);
        assert_eq!(handles[3], original);
        assert_eq!(handles[4], original);
        let materials = app.world().resource::<Assets<StandardMaterial>>();
        assert_eq!(
            materials.get(&handles[0]).unwrap().perceptual_roughness,
            0.18
        );
        assert_eq!(
            materials.get(&handles[2]).unwrap().perceptual_roughness,
            0.24
        );
        assert_eq!(materials.get(&original).unwrap().perceptual_roughness, 0.7);
    }
    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn lacetti_reflector_frames_and_normal_resource() {
        let root = std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required");
        let mut car = super::super::load(
            std::path::Path::new(&root),
            &grid_tools::route::Route::default(),
            &grid_tools::car::Car::default(),
            "LOD0_",
            false,
            false,
        )
        .unwrap();
        prepare_frames(&mut car, Mode::Combined).unwrap();
        validate(&car, Mode::Combined).unwrap();
        assert_eq!(
            car.parts
                .iter()
                .filter(|p| player_part(&p.material))
                .count(),
            8
        );
    }
}
