//! The player's cockpit (F618, docs/CAMERA_VIEWS.md "Own car per view"):
//! `interiors/<stem>/int_<stem>.pssg`, loaded for the player only as the
//! original's race loader does (0x6138D0, `n12` when the car has none),
//! placed on the model frame like the body (its Scene Root is the identity)
//! and drawn through the car's material path. Which parts of the player car
//! a view draws is [`crate::camera::OwnCar`]. The rear-view mirror renders
//! the scene behind the car into its own target (docs/COCKPIT.md "Mirror").
use super::{Car, Part, read};
use bevy::{
    asset::RenderAssetUsages,
    camera::{
        Exposure, Hdr, RenderTarget,
        primitives::{Aabb, Frustum},
        visibility::RenderLayers,
    },
    core_pipeline::tonemapping::Tonemapping,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    render::render_resource::TextureFormat,
};
use grid_formats::{interior, material, mesh, pssg, scene, texture};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    path::Path,
};

/// Joint prefix of the cockpit's parts (each part's RENDERNODE follows).
pub const JOINT: &str = "interior/";

/// The layer of the cockpit's parts: the driving camera and the sun take
/// it, the mirror does not (0x5EA6A0 draws no interior).
pub const LAYER: usize = 28;

/// The exterior nodes the cockpit view keeps (`int_<stem>.xml`), without
/// their `x0_` prefix as `damage_parts::Joint` holds them, and the frame
/// the cockpit was placed in (model frame relative to the body pose).
#[derive(Resource, Clone, Debug, Default)]
pub struct Cockpit {
    pub active: Vec<String>,
    pub anchor: Mat4,
}

/// The rear-view mirror target: hardware settings `mirrors` width and height
/// (0x602400; 512 x 128, the default and every shipped preset), no MSAA.
const MIRROR_SIZE: UVec2 = UVec2::new(512, 128);
/// The mirror camera (0x5FA440): 15 degrees vertical (0xCE5204), the
/// target's aspect, near 0.5 m, far 1000 m, half a metre ahead of the
/// mirrors' mean centre; its visibility list is culled at 500 m.
const MIRROR_FOV: f32 = 0.261_799_4;
const MIRROR_NEAR: f32 = 0.5;
const MIRROR_FAR: f32 = 1000.;
const MIRROR_CULL: f32 = 500.;
const MIRROR_AHEAD: f32 = 0.5;
/// The pass's clear colour (base default, no override found) [I: raw
/// target values, read as sRGB].
const MIRROR_CLEAR: Color = Color::srgb(0.14, 0.11, 0.06);
/// mirror.fx with the tint the constructor 0xA135C0 sets (black, 0.5): the
/// reflection at half brightness.
const MIRROR_TINT: f32 = 0.5;

/// A cockpit mirror glass (int_reflection_l/m/r, mirror.fx).
#[derive(Component)]
pub struct MirrorGlass;

/// The rear-view mirror's camera.
#[derive(Component)]
pub struct MirrorCamera;

/// The mirror's target size and update interval in frames; `None` draws no
/// mirror. The original's own values unless `GRID_MIRROR` (AUTHORED
/// diagnostics: `off`, or `WIDTHxHEIGHT` with an optional `/FRAMES`).
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct Mirror(pub Option<(UVec2, u32)>);

impl Mirror {
    fn parse(value: &str) -> Option<Self> {
        if value == "off" {
            return Some(Self(None));
        }
        let (size, every) = value.split_once('/').unwrap_or((value, "1"));
        let (w, h) = size.split_once('x')?;
        let size = UVec2::new(w.parse().ok()?, h.parse().ok()?);
        let every: u32 = every.parse().ok()?;
        (size.min_element() > 0 && every > 0).then_some(Self(Some((size, every))))
    }
}

pub fn install(app: &mut App) {
    let mut mirror = Mirror(Some((MIRROR_SIZE, 1)));
    if let Ok(value) = std::env::var("GRID_MIRROR") {
        match Mirror::parse(&value) {
            Some(m) => mirror = m,
            None => warn!("GRID_MIRROR: expected off or WIDTHxHEIGHT[/FRAMES], got {value:?}"),
        }
    }
    app.insert_resource(mirror)
        .add_systems(Update, (create_mirror, place_mirror).chain().after(crate::drive_present))
        .add_systems(
            PostUpdate,
            (
                cull_mirror.after(bevy::camera::visibility::VisibilitySystems::UpdateFrusta).before(
                    bevy::camera::visibility::VisibilitySystems::CheckVisibility,
                ),
                no_mirror_shadows
                    .after(bevy::light::SimulationLightSystems::UpdateLightFrusta)
                    .before(bevy::light::SimulationLightSystems::CheckLightVisibility),
            ),
        );
}

/// The mirror's visibility frustum ends at 500 m (0x5FA440 builds the cull
/// list with far 500 and draws with far 1000).
fn cull_mirror(mut camera: Query<(&Camera, &GlobalTransform, &mut Frustum), With<MirrorCamera>>) {
    for (camera, transform, mut frustum) in &mut camera {
        if !camera.is_active {
            continue;
        }
        let clip_from_world = camera.clip_from_view() * transform.to_matrix().inverse();
        *frustum = Frustum(bevy::math::primitives::ViewFrustum::from_clip_from_world_custom_far(
            &clip_from_world,
            &transform.translation(),
            &transform.back(),
            MIRROR_CULL,
        ));
    }
}

/// The original renders no cascade shadows for the mirror (no cascade pass,
/// 0x5EA6A0), and Bevy gives every active camera the sun's cascades: the
/// mirror's cascade frusta reject every caster, so its shadow maps stay
/// empty (lit, no shadow) and cost no draws.
fn no_mirror_shadows(
    mirror: Query<Entity, With<MirrorCamera>>,
    mut lights: Query<&mut bevy::camera::primitives::CascadesFrusta>,
) {
    let Ok(mirror) = mirror.single() else {
        return;
    };
    let none = bevy::math::primitives::HalfSpace::new(Vec4::new(1., 0., 0., f32::NEG_INFINITY));
    for mut frusta in &mut lights {
        if let Some(cascades) = frusta.frusta.get_mut(&mirror) {
            for frustum in cascades {
                frustum.half_spaces = [none; 6];
            }
        }
    }
}

/// The mirror's target and camera, once the glass exists; the camera takes
/// the driving camera's exposure and tonemapping so both pictures match.
#[allow(clippy::type_complexity)]
fn create_mirror(
    mut commands: Commands,
    glass: Query<Entity, Added<MirrorGlass>>,
    existing: Query<(), With<MirrorCamera>>,
    mirror: Res<Mirror>,
    viewer: Query<(Option<&Exposure>, Option<&Tonemapping>, Has<Hdr>), With<crate::ViewerCamera>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if glass.is_empty() || !existing.is_empty() {
        return;
    }
    let Mirror(Some((size, every))) = *mirror else {
        return;
    };
    let target = images.add(Image::new_target_texture(
        size.x,
        size.y,
        TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    let material = materials.add(StandardMaterial {
        base_color: Color::linear_rgb(MIRROR_TINT, MIRROR_TINT, MIRROR_TINT),
        base_color_texture: Some(target.clone()),
        unlit: true,
        ..default()
    });
    for entity in &glass {
        commands.entity(entity).insert(MeshMaterial3d(material.clone()));
    }
    let mut camera = commands.spawn((
        MirrorCamera,
        Name::new("rear view mirror"),
        Camera3d::default(),
        Camera {
            order: -1,
            is_active: false,
            clear_color: ClearColorConfig::Custom(MIRROR_CLEAR),
            ..default()
        },
        RenderTarget::Image(target.into()),
        Projection::Perspective(PerspectiveProjection {
            fov: MIRROR_FOV,
            aspect_ratio: size.x as f32 / size.y as f32,
            near: MIRROR_NEAR,
            far: MIRROR_FAR,
            ..default()
        }),
        Msaa::Off,
        RenderLayers::layer(0),
    ));
    if let Ok((exposure, tonemapping, hdr)) = viewer.single() {
        if let Some(exposure) = exposure {
            camera.insert(*exposure);
        }
        if let Some(tonemapping) = tonemapping {
            camera.insert(*tonemapping);
        }
        if hdr {
            camera.insert(Hdr);
        }
    }
    println!(
        "cockpit_mirror size={}x{} every={every} fov_deg=15 near={MIRROR_NEAR} far={MIRROR_FAR} glass={}",
        size.x,
        size.y,
        glass.iter().count()
    );
}

/// 0x5FA440 per frame: the eye half a metre ahead of the mirrors' mean
/// world centre, looking back along the cockpit frame; on while the cockpit
/// is shown and a mirror is inside the driving camera's frustum, otherwise
/// the target keeps its last picture. With an update interval above one
/// frame it renders only on every that many frames.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn place_mirror(
    mirror: Res<Mirror>,
    frame: Res<bevy::diagnostic::FrameCount>,
    test: Option<Res<crate::perf_test::PerfTest>>,
    driving: Option<Res<crate::Driving>>,
    cockpit: Option<Res<Cockpit>>,
    own: (Option<Res<crate::camera::Cameras>>, Option<Res<crate::replay::Replay>>),
    glass: Query<(&Transform, &Aabb), (With<MirrorGlass>, Without<MirrorCamera>)>,
    viewer: Query<&Frustum, With<crate::ViewerCamera>>,
    mut camera: Query<(&mut Camera, &mut Transform), With<MirrorCamera>>,
) {
    let (Some(driving), Some(cockpit), Ok((mut camera, mut transform))) = (driving, cockpit, camera.single_mut()) else {
        return;
    };
    let shown = crate::camera::own_car(own.0.as_deref(), own.1.as_deref(), true) == crate::camera::OwnCar::Interior;
    let seen = viewer.single().is_ok_and(|frustum| {
        glass.iter().any(|(t, aabb)| frustum.intersects_obb(aabb, &t.compute_affine(), true, true))
    });
    let due = mirror.0.is_some_and(|(_, every)| frame.0.is_multiple_of(every))
        && !test.is_some_and(|t| t.switches.mirror);
    let active = shown && seen && due && !glass.is_empty();
    camera.is_active = active;
    if !active {
        return;
    }
    let centre = glass.iter().map(|(t, aabb)| t.transform_point(aabb.center.into())).sum::<Vec3>() / glass.iter().count() as f32;
    let frame = Mat4::from_cols_array(&driving.runtime.pose()) * cockpit.anchor;
    let (forward, up) = (frame.z_axis.truncate().normalize(), frame.y_axis.truncate().normalize());
    *transform = Transform::from_translation(centre + MIRROR_AHEAD * forward).looking_to(-forward, up);
}

/// A cockpit part: drawn on its own, never merged into the car's batches.
#[derive(Component)]
pub struct InteriorPart;

/// Appends the cockpit of the player's model `stem` to `car` (material and
/// texture keys under `car0/interior/`), placed by `anchor` like the body's
/// parts. Shader groups other than interior.fx, interior_alpha.fx and
/// interior_windscreen.fx (the LCD and motec displays, the mirror) are not
/// drawn yet.
pub fn append(root: &Path, stem: &str, car: &mut Car, anchor: Mat4) -> Result<Cockpit, Box<dyn Error>> {
    let file = |stem: &str, ext: &str| root.join(format!("interiors/{stem}/int_{stem}.{ext}"));
    let stem = if file(stem, "pssg").is_file() { stem } else { "n12" };
    let bytes = read(&file(stem, "pssg"))?;
    let tree = pssg::read_tree(&bytes)?;
    let sources = mesh::read_static_meshes(&tree)?;
    let draws = scene::read_static_draws(&tree, &sources)?;
    let mut groups = BTreeMap::new();
    for i in (0..tree.nodes.len()).filter(|&i| tree.name(i) == b"SHADERINSTANCE") {
        groups.insert(tree.string(i, b"id")?, tree.string(i, b"shaderGroup")?);
    }
    let mut diffuse = BTreeMap::new();
    for b in material::read_texture_bindings(&tree)? {
        if b.sampler == b"TDiffuseAlphaMap" && b.container.is_none() && diffuse.insert(b.shader, b.texture).is_some() {
            return Err("duplicate cockpit diffuse binding".into());
        }
    }
    let utf8 = |b: &[u8]| String::from_utf8_lossy(b).into_owned();
    let (mut parts, mut bindings, mut needed) = (Vec::new(), BTreeMap::new(), BTreeSet::new());
    let (mut skipped, mut triangles) = (0usize, 0usize);
    for d in draws {
        let group = groups.get(d.material).copied().unwrap_or_default();
        let kind = match group.strip_prefix(b"#").unwrap_or(group) {
            b"interior.fx" => "",
            b"interior_alpha.fx" => "alpha/",
            b"interior_windscreen.fx" => "glass/",
            b"mirror.fx" => "mirror/",
            _ => {
                skipped += 1;
                continue;
            }
        };
        let material = format!("car0/interior/{kind}{}", utf8(d.material));
        let mirror = kind == "mirror/";
        // A mirror samples its own target, not the file's textures.
        if !mirror && let Some(texture) = diffuse.get(d.material) {
            bindings.insert(material.clone(), format!("car0/interior/{}", utf8(texture)));
            needed.insert(*texture);
        }
        let source = &sources[d.source];
        let mut geometry = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        );
        geometry.insert_attribute(Mesh::ATTRIBUTE_POSITION, source.positions.clone());
        triangles += d.indices.len() / 3;
        geometry.insert_indices(Indices::U16(d.indices));
        let uv = match source.texcoords.is_empty() {
            true => vec![[0f32; 2]; source.positions.len()],
            // mirror.fx's vertex program flips the reflection: (1 - u, v).
            false if mirror => source.texcoords.iter().map(|[u, v]| [1. - u, *v]).collect(),
            false => source.texcoords.clone(),
        };
        geometry.insert_attribute(Mesh::ATTRIBUTE_UV_0, uv);
        if source.normals.is_empty() || source.nonfinite_normals != 0 {
            geometry.compute_smooth_normals();
        } else {
            geometry.insert_attribute(Mesh::ATTRIBUTE_NORMAL, source.normals.clone());
        }
        parts.push(Part {
            name: format!("interior/{}", utf8(d.id)),
            joint: format!("{JOINT}{}", utf8(d.joint)),
            material,
            mesh: geometry,
            matrices: vec![anchor * Mat4::from_cols_array(&d.transform)],
            wheel: None,
        });
    }
    if parts.is_empty() {
        return Err("cockpit without drawable parts".into());
    }
    let names: Vec<_> = needed.into_iter().collect();
    let mut images = BTreeMap::new();
    for t in texture::read_named(&tree, &names)? {
        images.insert(format!("car0/interior/{}", utf8(t.id)), t.validated_dds().0);
    }
    if bindings.values().any(|key| !images.contains_key(key)) {
        return Err("cockpit diffuse texture unresolved".into());
    }
    let active = interior::read_active_nodes(&std::fs::read_to_string(file(stem, "xml"))?)?;
    let active: Vec<String> = active.iter().map(|n| n.strip_prefix("x0_").unwrap_or(n).to_string()).collect();
    println!(
        "cockpit stem={stem} parts={} skipped={skipped} triangles={triangles} textures={} active_nodes={}",
        parts.len(),
        images.len(),
        active.join(",")
    );
    car.parts.extend(parts);
    car.bindings.extend(bindings);
    car.textures.extend(images);
    Ok(Cockpit { active, anchor })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mirror_setting_parses_off_size_and_interval() {
        assert_eq!(Mirror::parse("off"), Some(Mirror(None)));
        assert_eq!(Mirror::parse("256x64"), Some(Mirror(Some((UVec2::new(256, 64), 1)))));
        assert_eq!(Mirror::parse("512x128/2"), Some(Mirror(Some((UVec2::new(512, 128), 2)))));
        assert_eq!(Mirror::parse("0x64"), None);
        assert_eq!(Mirror::parse("256x64/0"), None);
        assert_eq!(Mirror::parse("on"), None);
    }
}
