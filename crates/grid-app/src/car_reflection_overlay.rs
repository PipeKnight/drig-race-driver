//! Each reflecting car part samples the environment cube
//! ([`crate::car_reflections`]) with its material's own saved controls
//! (docs/CAR_REFLECTIONS.md): `EnvironmentColour` and `FresnelMin/Max/Power`
//! of its shader instance, as every car's `*_highLOD.pssg` saves them
//! (paint, rims and metal of `carpaint_metallic`, glass of `carglass`, lamp
//! glass and pods, grills; matt parts and tyres save 0 and do not reflect).
//!
//! The original program mixes the lit colour toward the environment colour
//! by the Fresnel weight. The port draws that mix as a child of the part: the
//! same mesh again, in the transparent phase, with depth equal to the part's
//! and blend colour = env, alpha = weight. The part's own material and the
//! systems that swap it (batching, damage, lamps) are untouched; the overlay
//! follows the part's mesh (dents bend its normals), material (a cracked
//! pane), layers and visibility (a smashed pane is hidden).
//!
//! Parts of the paint program (`carpaint_metallic`) also take its gates: the
//! car's packed `main_m1` map's alpha gates the weight, its green occludes
//! the environment by `OcclusionMapScale`, and the sun's shadow darkens it.
//! [I] The program adds the `SpecularValues` z and w floors to the occlusion
//! and the shadow; only the debug material editor (0x9E7360) is seen setting
//! them, so the port takes 0.
//!
//! AUTHORED: glass weighs its reflection by the pane's alpha; a cracked pane
//! turns its facets and reflects less. The paint program's dust, scratch and
//! vertex-colour gates are not applied (a clean, unscratched car).
use bevy::{
    asset::uuid_handle,
    camera::visibility::RenderLayers,
    light::{NotShadowCaster, NotShadowReceiver},
    mesh::MeshVertexBufferLayoutRef,
    pbr::{Material, MaterialPipeline, MaterialPipelineKey},
    platform::collections::HashMap,
    prelude::*,
    render::render_resource::{
        AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, CompareFunction,
        RenderPipelineDescriptor, SpecializedMeshPipelineError,
    },
    shader::ShaderRef,
};

const SHADER: Handle<Shader> = uuid_handle!("6f1c2d8a-41b7-4f0e-9a53-2c7e1b9d4a60");

/// A car material's saved reflection controls.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Controls {
    pub colour: Vec3,
    pub power: f32,
    pub min: f32,
    pub max: f32,
    /// `OcclusionMapScale` of a paint-program part, `None` for other programs.
    pub paint: Option<f32>,
}

/// The paint program's `SpecularValues` z (occlusion) and w (shadow) floors.
const FLOORS: Vec2 = Vec2::ZERO;

impl Controls {
    /// From a shader instance's constants; `None` when it does not reflect.
    pub fn from_constants(
        group: &[u8],
        values: &std::collections::BTreeMap<&[u8], Vec<f32>>,
    ) -> Option<Self> {
        let scalar = |name: &[u8]| match values.get(name).map(Vec::as_slice) {
            Some([v]) => Some(*v),
            _ => None,
        };
        let colour = match values.get(b"EnvironmentColour".as_slice()).map(Vec::as_slice) {
            Some([r, g, b, ..]) => Vec3::new(*r, *g, *b),
            _ => return None,
        };
        let controls = Self {
            colour,
            power: scalar(b"FresnelPower")?,
            min: scalar(b"FresnelMin")?,
            max: scalar(b"FresnelMax")?,
            // The loader's fallback scale is 1 (0x99C300).
            paint: (group == b"carpaint_metallic.fx")
                .then(|| scalar(b"OcclusionMapScale").unwrap_or(1.)),
        };
        (controls.colour.max_element() > 0. && controls.min.max(controls.max) > 0.)
            .then_some(controls)
    }
}

/// How a reflecting car material responds.
#[derive(Clone, Debug)]
pub struct Response {
    pub controls: Controls,
    /// The part's alpha weighs its reflection (glass).
    pub glass: bool,
    /// Its car's packed paint map, linear.
    pub mask: Option<Handle<Image>>,
}

/// The base level's mean alpha (the reflection gate) and green (occlusion),
/// and the share of texels whose alpha opens the gate fully (≥ 0.5).
fn log_mask(car: usize, dds: &[u8]) {
    let word = |i: usize| u32::from_le_bytes(dds[i..i + 4].try_into().unwrap()) as usize;
    let texels = word(12) * word(16);
    let Some(base) = dds.get(128..128 + texels * 4) else {
        return;
    };
    let (mut a, mut g, mut open) = (0u64, 0u64, 0usize);
    for p in base.as_chunks::<4>().0 {
        g += u64::from(p[1]);
        a += u64::from(p[3]);
        open += usize::from(p[3] >= 128);
    }
    let n = texels.max(1) as f64;
    println!(
        "car_reflections paint_mask car={car} mean_alpha={:.3} mean_green={:.3} alpha_open={:.3}",
        a as f64 / n / 255.,
        g as f64 / n / 255.,
        open as f64 / n
    );
}

/// Reflecting car materials by their Bevy material.
#[derive(Resource, Default)]
pub struct Reflective(pub HashMap<AssetId<StandardMaterial>, Response>);

/// Each car's packed paint map (`main_m1`) staged at load by car index. Green
/// and alpha keep their place; the shared re-encoding swaps red and blue.
#[derive(Resource, Default)]
pub struct Masks(crate::body_occlusion::Prepared);

impl Masks {
    /// Every car's own map; none (the paint gates open) when one is missing.
    pub fn prepare(car: &crate::Car, field: &crate::Field) -> Self {
        let raw = |dds: &[u8]| crate::body_occlusion::derive_with(dds, |p| p);
        match crate::body_occlusion::prepare_with(car, field, raw) {
            Ok(prepared) => Self(prepared),
            Err(error) => {
                println!("car_reflections paint_masks=none reason={error}");
                Self::default()
            }
        }
    }

    pub fn upload(
        &mut self,
        images: &mut Assets<Image>,
        sampler: bevy::image::ImageSampler,
    ) -> std::collections::BTreeMap<usize, Handle<Image>> {
        self.0
            .drain()
            .into_iter()
            .filter_map(|(car, dds)| {
                log_mask(car, &dds);
                let image = crate::texture_upload::image_linear(
                    &dds,
                    sampler.clone(),
                    bevy::asset::RenderAssetUsages::RENDER_WORLD,
                )
                .ok()?;
                Some((car, images.add(image)))
            })
            .collect()
    }
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct Overlay {
    #[uniform(0)]
    environment: Vec4,
    #[uniform(1)]
    fresnel: Vec4,
    #[texture(2, dimension = "cube")]
    #[sampler(3)]
    cube: Handle<Image>,
    #[texture(4)]
    #[sampler(5)]
    base: Option<Handle<Image>>,
    #[texture(6)]
    #[sampler(7)]
    mask: Option<Handle<Image>>,
    #[uniform(8)]
    paint: Vec4,
}

impl Material for Overlay {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
    /// Sorted just in front of the part it covers (a pane drawn first).
    fn depth_bias(&self) -> f32 {
        0.01
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn specialize(
        _: &MaterialPipeline,
        d: &mut RenderPipelineDescriptor,
        _: &MeshVertexBufferLayoutRef,
        _: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if let Some(target) = d
            .fragment
            .as_mut()
            .and_then(|f| f.targets.first_mut())
            .and_then(Option::as_mut)
        {
            target.blend = Some(BlendState {
                color: BlendComponent {
                    src_factor: BlendFactor::SrcAlpha,
                    dst_factor: BlendFactor::OneMinusSrcAlpha,
                    operation: BlendOperation::Add,
                },
                alpha: BlendComponent {
                    src_factor: BlendFactor::Zero,
                    dst_factor: BlendFactor::One,
                    operation: BlendOperation::Add,
                },
            });
        }
        if let Some(depth) = d.depth_stencil.as_mut() {
            // The part's own depth (reverse Z): the same vertices, the same
            // transform.
            depth.depth_write_enabled = Some(false);
            depth.depth_compare = Some(CompareFunction::GreaterEqual);
        }
        Ok(())
    }
}

/// The overlay child of a part.
#[derive(Component)]
pub struct OverlayOf;

/// The part's overlay entity.
#[derive(Component)]
struct HasOverlay(Entity);

pub fn install(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    app.world_mut().resource_mut::<Assets<Shader>>().insert(
        SHADER.id(),
        Shader::from_wgsl(
            include_str!("car_reflection_overlay.wgsl"),
            "grid_car_reflection_overlay.wgsl",
        ),
    )?;
    app.add_plugins(MaterialPlugin::<Overlay>::default())
        .init_resource::<Reflective>()
        .add_systems(
            PostUpdate,
            follow.before(bevy::camera::visibility::VisibilitySystems::CheckVisibility),
        );
    Ok(())
}

/// Create, update or remove each part's overlay when its material, mesh or
/// layers change.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn follow(
    mut commands: Commands,
    reflective: Res<Reflective>,
    cube: Option<Res<crate::car_reflections::Cube>>,
    parts: Query<
        (
            Entity,
            &MeshMaterial3d<StandardMaterial>,
            &Mesh3d,
            Option<&RenderLayers>,
            Option<&HasOverlay>,
            Option<&crate::damage_parts::Original>,
        ),
        (
            Without<OverlayOf>,
            Or<(
                Changed<MeshMaterial3d<StandardMaterial>>,
                Changed<Mesh3d>,
                Changed<RenderLayers>,
            )>,
        ),
    >,
    standard: Res<Assets<StandardMaterial>>,
    mut overlays: ResMut<Assets<Overlay>>,
    mut cache: Local<HashMap<AssetId<StandardMaterial>, Handle<Overlay>>>,
) {
    let Some(cube) = cube else {
        return;
    };
    for (entity, material, mesh, layers, existing, original) in &parts {
        // A cracked pane's material is a copy of its intact one with the
        // crack map (damage_parts).
        let source = original.map_or(material.id(), |o| o.0.id());
        let Some(response) = reflective.0.get(&source) else {
            if let Some(HasOverlay(child)) = existing {
                commands.entity(*child).despawn();
                commands.entity(entity).remove::<HasOverlay>();
            }
            continue;
        };
        let (controls, glass) = (response.controls, response.glass);
        let cracked = original.is_some();
        let handle = cache
            .entry(material.id())
            .or_insert_with(|| {
                overlays.add(Overlay {
                    environment: controls.colour.extend(controls.power),
                    fresnel: Vec4::new(
                        controls.min,
                        controls.max,
                        if glass || cracked { 1. } else { 0. },
                        if cracked { 1. } else { 0. },
                    ),
                    cube: cube.image.clone(),
                    base: standard
                        .get(material.id())
                        .and_then(|m| m.base_color_texture.clone()),
                    mask: response.mask.clone(),
                    paint: controls
                        .paint
                        .map_or(Vec4::ZERO, |scale| Vec4::new(1., scale, FLOORS.x, FLOORS.y)),
                })
            })
            .clone();
        // Drawn wherever the part is, except by the reflection cameras.
        let layers = layers
            .cloned()
            .unwrap_or_default()
            .without(crate::car_reflections::LAYER);
        let components = (
            Mesh3d(mesh.0.clone()),
            MeshMaterial3d(handle),
            layers,
        );
        match existing {
            Some(HasOverlay(child)) => {
                commands.entity(*child).insert(components);
            }
            None => {
                let child = commands
                    .spawn((
                        OverlayOf,
                        components,
                        Transform::default(),
                        Visibility::Inherited,
                        NotShadowCaster,
                        NotShadowReceiver,
                        ChildOf(entity),
                    ))
                    .id();
                commands.entity(entity).insert(HasOverlay(child));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controls_need_a_colour_and_a_weight() {
        let mut values = std::collections::BTreeMap::new();
        values.insert(b"EnvironmentColour".as_slice(), vec![0.537, 0.537, 0.537]);
        values.insert(b"FresnelPower".as_slice(), vec![1.]);
        values.insert(b"FresnelMin".as_slice(), vec![0.3]);
        values.insert(b"FresnelMax".as_slice(), vec![0.6]);
        let paint = Controls::from_constants(b"carpaint_metallic.fx", &values).unwrap();
        assert_eq!((paint.min, paint.max, paint.power), (0.3, 0.6, 1.));
        assert_eq!(paint.paint, Some(1.));
        let glass = Controls::from_constants(b"carglass.fx", &values).unwrap();
        assert_eq!(glass.paint, None);
        values.insert(b"EnvironmentColour".as_slice(), vec![0., 0., 0.]);
        assert_eq!(Controls::from_constants(b"carpaint_metallic.fx", &values), None);
    }
}
