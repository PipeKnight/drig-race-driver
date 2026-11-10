//! Each reflecting car part samples the environment cube ([`Environment`]:
//! the dynamic cube or the track's static map, [`crate::car_reflections`]) with its material's own saved controls
//! (docs/CAR_REFLECTIONS.md): `EnvironmentColour` and `FresnelMin/Max/Power`
//! of its shader instance, as every car's `*_highLOD.pssg` saves them or the
//! original's material tuning overrides them
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
//!
//! The original overrides the saved Fresnel and occlusion controls of every
//! material whose name starts with one of 35 names ([`Tuning`],
//! `raceload.jpk#materials.xml`, read by 0x99C300 and submitted by 0x9E7360
//! as `SpecularValues`, `MetallicSpecularValues` and `FresnelValues`). The
//! same submission sets the `SpecularValues` z and w floors that the paint
//! program adds to the occlusion and the shadow ([`FLOORS`]).
//!
//! A cracked pane reflects like an intact one through its crack-map colour
//! and alpha: the original's glass program (`car_basic` `fp_glass_windows`)
//! keeps the interpolated normal and the Fresnel weight.
//!
//! The same overlay draws the tuned sun terms of the paint and window glass
//! programs ([`Lighting`], docs/CAR_REFLECTIONS.md 1.7): the highlight
//! `SpecularValues.y · pow(sat(sun · r), SpecularValues.x)` and, on paint,
//! the metallic tint and darkening of `MetallicSpecularValues`. They replace
//! the part's own specular, which is turned off when its overlay is made.
//!
//! AUTHORED: glass weighs its reflection by the pane's alpha. The paint
//! program's dust, scratch and vertex-colour gates are not applied (a clean,
//! unscratched car).
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
    /// The paint program's `SpecularValues` z (occlusion) and w (shadow)
    /// floors: [`FLOORS`] for a tuned material, 0 otherwise.
    pub floors: Vec2,
    /// The tuned sun terms of a paint or window glass program part.
    pub lighting: Option<Lighting>,
}

/// A tuned material's sun terms (0x9E7360 submits them from its record):
/// the paint program (`carpaint_metallic`) and the window glass program
/// (`carglass`) light with them; docs/CAR_REFLECTIONS.md 1.7.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lighting {
    /// `SpecularValues` x and y: the highlight's power and strength.
    pub specular: Vec2,
    /// `MetallicSpecularValues` x, y and z: the metallic tint's power, its
    /// scale of the diffuse colour and the falloff of the metallic darkening.
    pub metallic: Vec3,
}

/// The `SpecularValues` z and w that 0x9E7360 submits with a tuned
/// material: the vehicle manager's +4 and +0, which its constructor 0xA135C0
/// sets to 0.25 and 0.5 (only the debug material editor changes them).
pub const FLOORS: Vec2 = Vec2::new(0.25, 0.5);

/// The 35 material names the original tunes (the table at 0xE4FD58), in its
/// order.
const TUNED: [&str; 35] = [
    "Car Body Paint",
    "Matt",
    "Head Lights Pod",
    "Rear Lights Pod",
    "Lights Pod Matt",
    "Head Lights Glass",
    "Rear Lights Glass",
    "Shiny Metal",
    "Bare Metal",
    "Car Body Carbon Fibre",
    "Discs",
    "Car Body Plastic",
    "Suspension",
    "Aerials",
    "Window Glass",
    "Cabin",
    "Grills",
    "Grills Glass",
    "Wheel",
    "Tyre Wall",
    "Tyre Tread",
    "Chrome",
    "carbon fibre",
    "Leather",
    "dials",
    "Shiny Painted Metal",
    "shiny plastic",
    "dials dials dials dials dials matt",
    "dials shny dials shiny",
    "dials shny dials shiny low spec",
    "interior glass windows",
    "mirror",
    "bare metal bare",
    "matt clipping shader",
    "shiny clipping shader",
];

/// A tuning record's fields, as 0x99C300 reads them from each `param`.
const FIELDS: [&str; 9] = [
    "specularHighlightPower",
    "specularHighlightStrength",
    "metallicSpecularPower",
    "metallicSpecularScale",
    "metallicFalloff",
    "occlusionMapScale",
    "fresnelPower",
    "fresnelMin",
    "fresnelMax",
];

/// 0x99C300's record before it reads a name's element.
const FALLBACK: [f32; 9] = [300., 64., 10., 4., 0.2, 1., 2., 0.1, 0.9];

/// The original's car material tuning, `raceload.jpk#materials.xml`.
///
/// 0x99C300 reads one nine-field record per name of [`TUNED`] from the root
/// `materials` element's children of that name (spaces as underscores) and
/// sets the vehicle manager's reload flag (+0x558, also set by its
/// constructor). 0x9E7360, called every frame by the vehicle manager's update
/// (0xA10310), sees the flag and, for every material of the 20 cars whose
/// name starts with a tuned name (case-insensitive, the first in table
/// order), submits `SpecularValues` (highlight power and strength,
/// [`FLOORS`]), `MetallicSpecularValues` and `FresnelValues` (power, min,
/// max) to it, then clears the flag.
///
/// [I] The cars are loaded when the flag is consumed: the update does not
/// tick during the race load.
#[derive(Clone, Debug, PartialEq)]
pub struct Tuning(pub [[f32; 9]; 35]);

impl Tuning {
    /// From the installation's `raceload.jpk`.
    pub fn read(root: &std::path::Path) -> Result<Self, Box<dyn std::error::Error>> {
        use grid_formats::{binxml, jpk};
        let bytes = std::fs::read(root.join("raceload.jpk"))?;
        let archive = jpk::inspect(&bytes)?;
        let entry = (archive.entries.iter())
            .find(|e| e.name == b"materials.xml")
            .ok_or("raceload.jpk lacks materials.xml")?;
        if entry.uncompressed_size as usize != entry.stored_data.len() {
            return Err("compressed materials.xml unsupported".into());
        }
        Self::parse(&binxml::read(&bytes[entry.stored_data.clone()])?)
    }

    /// From the decoded document, as 0x99C300 reads it: later elements and
    /// params override earlier ones.
    pub fn parse(
        document: &grid_formats::binxml::Document,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let elements = &document.elements;
        let root = (elements.iter())
            .find(|e| e.parent.is_none())
            .filter(|e| e.name == "materials")
            .ok_or("materials.xml's root is not `materials`")?;
        let named = |range: std::ops::Range<usize>, name: String| {
            range.filter(move |&i| elements[i].name == name)
        };
        let mut records = [FALLBACK; 35];
        for (record, name) in records.iter_mut().zip(TUNED) {
            for material in named(root.children.clone(), name.replace(' ', "_")) {
                for param in named(elements[material].children.clone(), "param".into()) {
                    for &(key, value) in &elements[param].attributes {
                        if let Some(k) = FIELDS.iter().position(|f| *f == key) {
                            record[k] = value.trim().parse()?;
                        }
                    }
                }
            }
        }
        Ok(Self(records))
    }

    /// The record 0x9E7360 submits to a material of this name, if any.
    pub fn record(&self, material: &str) -> Option<[f32; 9]> {
        let bytes = material.as_bytes();
        let i = TUNED.iter().position(|t| {
            bytes.len() >= t.len() && bytes[..t.len()].eq_ignore_ascii_case(t.as_bytes())
        })?;
        Some(self.0[i])
    }
}

/// Where an overlay sorts among the transparent draws (Bevy sorts by the
/// mesh's bounds centre plus the material's bias). The original reflects in
/// each part's own program, so an opaque part's reflection is in place before
/// any glass is blended over it: its overlay goes before every transparent
/// draw. Sorted by bounds centres instead, a pane in front and the cabin,
/// cage or lamp reflector behind it swapped order with small camera moves,
/// and the reflection was laid over the glass. A blended pane's overlay comes
/// just after the pane.
fn sort_bias(alpha: Option<AlphaMode>) -> f32 {
    match alpha {
        Some(AlphaMode::Opaque | AlphaMode::Mask(_)) | None => -1.0e6,
        Some(_) => 0.01,
    }
}

/// Diagnostic `GRID_CAR_REFLECTIONS_PAINT=x,y,z,w`: the paint gates' share
/// of the alpha gate, occlusion scale and the two floors, for comparisons
/// with original frames (docs/CAR_REFLECTIONS.md 4.2).
fn paint_override() -> Option<Vec4> {
    let text = std::env::var("GRID_CAR_REFLECTIONS_PAINT").ok()?;
    let v: Vec<f32> = text.split(',').filter_map(|t| t.trim().parse().ok()).collect();
    (v.len() == 4).then(|| Vec4::new(v[0].max(1e-3), v[1], v[2], v[3]))
}

impl Controls {
    /// From a shader instance's constants and, for a tuned material, its
    /// [`Tuning`] record; `None` when it does not reflect.
    pub fn from_constants(
        group: &[u8],
        values: &std::collections::BTreeMap<&[u8], Vec<f32>>,
        tuned: Option<[f32; 9]>,
    ) -> Option<Self> {
        let scalar = |name: &[u8]| match values.get(name).map(Vec::as_slice) {
            Some([v]) => Some(*v),
            _ => None,
        };
        let colour = match values.get(b"EnvironmentColour".as_slice()).map(Vec::as_slice) {
            Some([r, g, b, ..]) => Vec3::new(*r, *g, *b),
            _ => return None,
        };
        let paint = group == b"carpaint_metallic.fx";
        let lit = paint || group == b"carglass.fx";
        let controls = match tuned {
            Some(r) => Self {
                colour,
                power: r[6],
                min: r[7],
                max: r[8],
                paint: paint.then_some(r[5]),
                floors: FLOORS,
                lighting: lit.then(|| Lighting {
                    specular: Vec2::new(r[0], r[1]),
                    metallic: Vec3::new(r[2], r[3], r[4]),
                }),
            },
            None => Self {
                colour,
                power: scalar(b"FresnelPower")?,
                min: scalar(b"FresnelMin")?,
                max: scalar(b"FresnelMax")?,
                // The loader's fallback scale is 1 (0x99C300).
                paint: paint.then(|| scalar(b"OcclusionMapScale").unwrap_or(1.)),
                floors: Vec2::ZERO,
                lighting: None,
            },
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
    /// `SpecularValues` x, y and `MetallicSpecularValues` x, y.
    #[uniform(9)]
    sun_terms: Vec4,
    /// The metallic falloff, and the program: 0 none, 1 paint, 2 glass.
    #[uniform(10)]
    sun_program: Vec4,
    /// The transparent phase's sort bias (`Material::depth_bias`).
    bias: f32,
}

impl Material for Overlay {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
    fn depth_bias(&self) -> f32 {
        self.bias
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
                    // Premultiplied: the shader returns the weighted
                    // environment, the metallic tint and the highlight, and
                    // the share of the part that stays in alpha.
                    src_factor: BlendFactor::One,
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

/// A part whose overlay draws its program's sun terms keeps only its diffuse
/// lighting: no specular F0 (direct or environment) and no clear coat.
fn unlit_specular(part: &mut StandardMaterial) {
    part.reflectance = 0.;
    part.metallic = 0.;
    part.clearcoat = 0.;
    #[cfg(feature = "paint-specular")]
    {
        part.specular_texture = None;
    }
}

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

/// The cube the overlays sample: the dynamic cube of a probe level or the
/// track's static map ([`crate::car_reflections::Level`]).
#[derive(Resource, Clone)]
pub struct Environment(pub Handle<Image>);

/// Create, update or remove each part's overlay when its material, mesh or
/// layers change.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn follow(
    mut commands: Commands,
    reflective: Res<Reflective>,
    cube: Option<Res<Environment>>,
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
    mut standard: ResMut<Assets<StandardMaterial>>,
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
                if controls.lighting.is_some()
                    && let Some(mut part) = standard.get_mut(material.id())
                {
                    unlit_specular(&mut part);
                }
                let part = standard.get(material.id());
                overlays.add(Overlay {
                    environment: controls.colour.extend(controls.power),
                    fresnel: Vec4::new(
                        controls.min,
                        controls.max,
                        if glass || cracked { 1. } else { 0. },
                        0.,
                    ),
                    cube: cube.0.clone(),
                    base: part.and_then(|m| m.base_color_texture.clone()),
                    mask: response.mask.clone(),
                    // Glass takes only the floors (its occlusion term has no
                    // map, so only the shadow floor acts).
                    paint: controls.paint.map_or(
                        Vec4::new(0., 0., controls.floors.x, controls.floors.y),
                        |scale| {
                            paint_override()
                                .unwrap_or(Vec4::new(1., scale, controls.floors.x, controls.floors.y))
                        },
                    ),
                    sun_terms: controls.lighting.map_or(Vec4::ZERO, |l| {
                        l.specular.extend(l.metallic.x).extend(l.metallic.y)
                    }),
                    sun_program: Vec4::new(
                        controls.lighting.map_or(0., |l| l.metallic.z),
                        match (controls.lighting, controls.paint) {
                            (None, _) => 0.,
                            (Some(_), Some(_)) => 1.,
                            (Some(_), None) => 2.,
                        },
                        0.,
                        0.,
                    ),
                    bias: sort_bias(part.map(|m| m.alpha_mode)),
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
    fn an_opaque_part_reflects_before_the_glass_over_it() {
        let (opaque, masked, pane) = (
            sort_bias(Some(AlphaMode::Opaque)),
            sort_bias(Some(AlphaMode::Mask(0.5))),
            sort_bias(Some(AlphaMode::Blend)),
        );
        // A cage whose bounds centre is 1 m nearer than the pane's still
        // sorts before it (Bevy draws ascending view-space z + bias).
        assert!(opaque + 1.0 < pane && masked + 1.0 < pane);
        assert!(pane > 0.);
    }

    #[test]
    fn controls_need_a_colour_and_a_weight() {
        let mut values = std::collections::BTreeMap::new();
        values.insert(b"EnvironmentColour".as_slice(), vec![0.537, 0.537, 0.537]);
        values.insert(b"FresnelPower".as_slice(), vec![1.]);
        values.insert(b"FresnelMin".as_slice(), vec![0.3]);
        values.insert(b"FresnelMax".as_slice(), vec![0.6]);
        let paint = Controls::from_constants(b"carpaint_metallic.fx", &values, None).unwrap();
        assert_eq!((paint.min, paint.max, paint.power), (0.3, 0.6, 1.));
        assert_eq!((paint.paint, paint.floors), (Some(1.), Vec2::ZERO));
        let glass = Controls::from_constants(b"carglass.fx", &values, None).unwrap();
        assert_eq!(glass.paint, None);
        // A tuned material takes its record's Fresnel, occlusion and floors.
        let record = [35.65, 1.16, 14.16, 1.2, 0.25, 1.05, 2.13, 0.27, 0.86];
        let tuned = Controls::from_constants(b"carpaint_metallic.fx", &values, Some(record));
        let tuned = tuned.unwrap();
        assert_eq!((tuned.power, tuned.min, tuned.max), (2.13, 0.27, 0.86));
        assert_eq!((tuned.paint, tuned.floors), (Some(1.05), FLOORS));
        // Its sun terms: SpecularValues x, y and MetallicSpecularValues x, y, z.
        let lighting = Lighting {
            specular: Vec2::new(35.65, 1.16),
            metallic: Vec3::new(14.16, 1.2, 0.25),
        };
        assert_eq!(tuned.lighting, Some(lighting));
        let glass = Controls::from_constants(b"carglass.fx", &values, Some(record)).unwrap();
        assert_eq!((glass.paint, glass.lighting), (None, Some(lighting)));
        // Untuned materials and other programs keep their own lighting.
        assert_eq!(paint.lighting, None);
        let lamp = Controls::from_constants(b"car_light_glass.fx", &values, Some(record));
        assert_eq!(lamp.unwrap().lighting, None);
        // A record of no weight turns the reflection off.
        let off = [1., 0., 1., 0., 0.19, 1.04, 3.95, 0., 0.];
        assert_eq!(Controls::from_constants(b"carglass.fx", &values, Some(off)), None);
        values.insert(b"EnvironmentColour".as_slice(), vec![0., 0., 0.]);
        let dark = Controls::from_constants(b"carpaint_metallic.fx", &values, Some(record));
        assert_eq!(dark, None);
    }

    #[test]
    fn a_material_takes_the_first_tuned_name_it_starts_with() {
        let mut records = [FALLBACK; 35];
        for (i, r) in records.iter_mut().enumerate() {
            r[0] = i as f32;
        }
        let tuning = Tuning(records);
        let index = |name: &str| tuning.record(name).map(|r| r[0] as usize);
        assert_eq!(index("Car Body Paint"), Some(0));
        assert_eq!(index("car body paint_livery"), Some(0));
        assert_eq!(index("Window Glass"), Some(14));
        // "dials" comes before the longer dials names in the table.
        assert_eq!(index("dials shny dials shiny"), Some(24));
        assert_eq!(index("Body"), None);
    }

    /// The installed `raceload.jpk#materials.xml`, and every reflecting
    /// material of every car's high LOD: how many are tuned.
    #[test]
    #[ignore = "needs GRID_GAME_ROOT"]
    fn the_installed_tuning_covers_the_cars_reflecting_materials() {
        let root = std::path::PathBuf::from(std::env::var("GRID_GAME_ROOT").unwrap());
        let tuning = Tuning::read(&root).unwrap();
        let paint = tuning.record("Car Body Paint").unwrap();
        assert_eq!(paint[5..], [1.05, 2.13, 0.27, 0.86]);
        // "Grills" comes first and is a prefix of "Grills Glass" (and "Matt"
        // of "matt clipping shader"): their own records are never used.
        assert_eq!(tuning.record("Grills Glass"), tuning.record("Grills"));
        assert_eq!(tuning.record("shiny clipping shader"), Some(FALLBACK));
        let (mut tuned, mut paints, mut off) = (0, 0, 0);
        let mut untuned = Vec::new();
        let mut cars: Vec<_> = std::fs::read_dir(root.join("cars")).unwrap().flatten().collect();
        cars.sort_by_key(|e| e.file_name());
        for car in cars {
            let id = car.file_name().to_string_lossy().into_owned();
            let Ok(bytes) = std::fs::read(car.path().join(format!("{id}_highLOD.pssg"))) else {
                continue;
            };
            let tree = grid_formats::pssg::read_tree(&bytes).unwrap();
            for shader in grid_formats::material::read_constants(&tree).unwrap() {
                let name = String::from_utf8_lossy(shader.shader).into_owned();
                if Controls::from_constants(shader.group, &shader.values, None).is_none() {
                    continue;
                }
                match tuning.record(&name) {
                    Some(r) => {
                        tuned += 1;
                        paints += usize::from(shader.group == b"carpaint_metallic.fx");
                        let now = Controls::from_constants(shader.group, &shader.values, Some(r));
                        off += usize::from(now.is_none());
                    }
                    None => untuned.push(format!("{id}:{name}")),
                }
            }
        }
        println!(
            "tuning reflecting_tuned={tuned} paint_program={paints} tuned_off={off} untuned={} {:?}",
            untuned.len(),
            &untuned[..untuned.len().min(40)]
        );
        assert!(tuned > 0 && paints > 0);
    }
    #[test]
    fn a_lit_overlay_takes_the_parts_specular() {
        let mut app = App::new();
        app.init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<Image>>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<Overlay>>()
            .init_resource::<Reflective>()
            .add_systems(Update, follow);
        let cube = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(Image::default());
        app.insert_resource(Environment(cube));
        let mesh = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(Mesh::from(Cuboid::new(1., 1., 1.)));
        let lighting = Lighting {
            specular: Vec2::new(35.65, 1.16),
            metallic: Vec3::new(14.16, 1.2, 0.25),
        };
        // Paint, window glass, and an untuned part.
        let cases = [
            (Some(1.05), Some(lighting)),
            (None, Some(lighting)),
            (Some(1.), None),
        ];
        let parts: Vec<_> = cases
            .iter()
            .map(|&(paint, lighting)| {
                let material = app
                    .world_mut()
                    .resource_mut::<Assets<StandardMaterial>>()
                    .add(StandardMaterial {
                        reflectance: 0.5,
                        metallic: 0.4,
                        clearcoat: 1.,
                        ..Default::default()
                    });
                let controls = Controls {
                    colour: Vec3::splat(0.5),
                    power: 2.13,
                    min: 0.27,
                    max: 0.86,
                    paint,
                    floors: if lighting.is_some() { FLOORS } else { Vec2::ZERO },
                    lighting,
                };
                let response = Response {
                    controls,
                    glass: paint.is_none(),
                    mask: None,
                };
                app.world_mut()
                    .resource_mut::<Reflective>()
                    .0
                    .insert(material.id(), response);
                let entity = app
                    .world_mut()
                    .spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone())))
                    .id();
                (entity, material)
            })
            .collect();
        app.update();
        let overlay = |entity| {
            let child = app.world().get::<HasOverlay>(entity).unwrap().0;
            let handle = &app.world().get::<MeshMaterial3d<Overlay>>(child).unwrap().0;
            app.world().resource::<Assets<Overlay>>().get(handle).unwrap().clone()
        };
        let base = |material: &Handle<StandardMaterial>| {
            let m = app.world().resource::<Assets<StandardMaterial>>().get(material).unwrap();
            (m.reflectance, m.metallic, m.clearcoat)
        };
        let paint = overlay(parts[0].0);
        assert_eq!(paint.sun_terms, Vec4::new(35.65, 1.16, 14.16, 1.2));
        assert_eq!(paint.sun_program, Vec4::new(0.25, 1., 0., 0.));
        assert_eq!(base(&parts[0].1), (0., 0., 0.));
        // Glass: program 2 and only the floors in the paint vector.
        let glass = overlay(parts[1].0);
        assert_eq!(glass.sun_program.y, 2.);
        assert_eq!(glass.paint, Vec4::new(0., 0., FLOORS.x, FLOORS.y));
        assert_eq!(base(&parts[1].1), (0., 0., 0.));
        // Untuned: no sun terms, the part keeps its own specular.
        let untuned = overlay(parts[2].0);
        assert_eq!((untuned.sun_terms, untuned.sun_program), (Vec4::ZERO, Vec4::ZERO));
        assert_eq!(base(&parts[2].1), (0.5, 0.4, 1.));
    }

    #[test]
    fn actual_follow_retains_lamp_owned_optics_across_damage_and_bank_readiness() {
        use crate::lamp_association::{BankKey, Receipt};
        use crate::lamp_signals::{Atlas, Mode, runtime};
        let mut app = App::new();
        app.init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<Image>>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<Overlay>>()
            .init_resource::<Reflective>()
            .init_resource::<runtime::State>()
            .init_resource::<runtime::Owners>();
        let image = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(Image::default());
        let crack_image = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(Image::default());
        let mask = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(Image::default());
        app.insert_resource(Environment(image.clone()));
        let high = BankKey {
            source: "fixture_high".into(),
            resource: "same_lights_on.tga".into(),
        };
        let low = BankKey {
            source: "fixture_low".into(),
            resource: "same_lights_on.tga".into(),
        };
        app.insert_resource(Atlas([(high.clone(), image.clone())].into_iter().collect()));
        app.world_mut()
            .resource_mut::<runtime::State>()
            .constant(Mode::Brake, 1);
        let source = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial {
                base_color_texture: Some(image.clone()),
                perceptual_roughness: 0.37,
                ..Default::default()
            });
        let response = Response {
            controls: Controls {
                colour: Vec3::new(0.2, 0.3, 0.4),
                power: 0.7,
                min: 0.02,
                max: 0.9,
                paint: None,
                floors: Vec2::ZERO,
                lighting: None,
            },
            glass: true,
            mask: Some(mask),
        };
        app.world_mut()
            .resource_mut::<Reflective>()
            .0
            .insert(source.id(), response.clone());
        let section = crate::lamp_sections::Section {
            semantic: 121,
            palette: 9,
            role: crate::lamp_sections::Role::Brake,
            cluster: 6,
            lod: 0,
            node: 3,
            bundle: "LOD0_".into(),
            draw: "fixture_draw".into(),
            material: "Rear Lights Pod".into(),
            joint: "x0_light_brake_pod_bl".into(),
        };
        let receipt = Receipt {
            bank: high.clone(),
            section,
            geometry_source: "fixture_geometry".into(),
            namespaced_texture: "fixture_namespace/same_lights_on.tga".into(),
        };
        let mesh = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(Mesh::from(Cuboid::new(1., 1., 1.)));
        let entity = app
            .world_mut()
            .spawn((
                runtime::Receiver::from_receipt(0, &receipt).unwrap(),
                Mesh3d(mesh),
                MeshMaterial3d(source.clone()),
                crate::track_surface::SourcePart("car0/Rear Lights Pod".into()),
                Transform::IDENTITY,
                Visibility::Inherited,
            ))
            .id();
        // Exercise the actual first optical clone too, rather than fabricate its alias.
        let settings = crate::graphics::Settings {
            lamp_reflectors: crate::lamp_materials::Mode::Pbr,
            ..Default::default()
        };
        app.insert_resource(settings)
            .insert_resource(crate::lamp_materials::Prepared(None));
        app.add_systems(PostStartup, crate::lamp_materials::attach)
            .add_systems(Update, runtime::project)
            .add_systems(PostUpdate, follow);
        app.update();
        let current = |app: &App| {
            app.world()
                .get::<MeshMaterial3d<StandardMaterial>>(entity)
                .unwrap()
                .0
                .clone()
        };
        let check = |app: &App, expected: &Response, base: &Handle<Image>| {
            let child = app
                .world()
                .get::<HasOverlay>(entity)
                .expect("actual follow must create/retain a real child")
                .0;
            assert!(app.world().get::<OverlayOf>(child).is_some());
            let material = &app.world().get::<MeshMaterial3d<Overlay>>(child).unwrap().0;
            let overlay = app
                .world()
                .resource::<Assets<Overlay>>()
                .get(material)
                .unwrap();
            assert_eq!(
                overlay.environment,
                expected.controls.colour.extend(expected.controls.power)
            );
            assert_eq!(
                overlay.fresnel,
                Vec4::new(expected.controls.min, expected.controls.max, 1., 0.)
            );
            assert_eq!(overlay.mask, expected.mask);
            assert_eq!(overlay.base.as_ref(), Some(base));
            child
        };
        let intact = current(&app);
        assert_ne!(intact, source);
        let child = check(&app, &response, &image);
        let original_source = app
            .world()
            .resource::<Assets<StandardMaterial>>()
            .get(&source)
            .unwrap();
        assert_eq!(original_source.perceptual_roughness, 0.37);
        assert_eq!(original_source.emissive, LinearRgba::BLACK);
        // Raw damage clone has NO response row. Original supplies actual intact identity.
        let mut damaged = app
            .world()
            .resource::<Assets<StandardMaterial>>()
            .get(&intact)
            .unwrap()
            .clone();
        damaged.base_color_texture = Some(crack_image.clone());
        let damaged = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(damaged);
        assert!(
            !app.world()
                .resource::<Reflective>()
                .0
                .contains_key(&damaged.id())
        );
        app.world_mut().entity_mut(entity).insert((
            MeshMaterial3d(damaged.clone()),
            crate::damage_parts::Original(intact.clone()),
        ));
        app.world_mut().resource_mut::<Atlas>().0.clear();
        app.world_mut()
            .resource_mut::<runtime::State>()
            .constant(Mode::Off, 0);
        app.update();
        let cracked_owned = current(&app);
        assert_ne!(cracked_owned, damaged);
        assert_eq!(check(&app, &response, &crack_image), child);
        assert_eq!(
            app.world()
                .resource::<Assets<StandardMaterial>>()
                .get(&cracked_owned)
                .unwrap()
                .emissive,
            LinearRgba::BLACK
        );
        assert_eq!(
            app.world()
                .get::<crate::damage_parts::Original>(entity)
                .unwrap()
                .0,
            intact
        );
        assert_eq!(
            app.world()
                .resource::<Assets<StandardMaterial>>()
                .get(&intact)
                .unwrap()
                .emissive,
            LinearRgba::rgb(0.7, 0.7, 0.7)
        );
        // Lose only our cached alias rows; canonical optical-source controls remain.
        // Actual follow must retain its child, so BOTH cracked and Original-owned
        // rows need refreshing even while state and image are unavailable.
        app.world_mut()
            .resource_mut::<Reflective>()
            .0
            .remove(&intact.id());
        app.world_mut()
            .resource_mut::<Reflective>()
            .0
            .remove(&cracked_owned.id());
        app.update();
        assert_eq!(current(&app), cracked_owned);
        assert_eq!(check(&app, &response, &crack_image), child);
        assert_eq!(
            app.world()
                .resource::<Reflective>()
                .0
                .get(&intact.id())
                .unwrap()
                .controls,
            response.controls
        );
        // Restore actual Original contract, then switch to a distinct pending LOW bank.
        app.world_mut()
            .entity_mut(entity)
            .remove::<crate::damage_parts::Original>()
            .insert(MeshMaterial3d(intact.clone()));
        app.world_mut()
            .resource_mut::<runtime::State>()
            .constant(Mode::Brake, 1);
        app.world_mut()
            .resource_mut::<Atlas>()
            .0
            .insert(high, image.clone());
        app.update();
        assert_eq!(current(&app), intact);
        assert_eq!(check(&app, &response, &image), child);
        let mut low_receipt = receipt.clone();
        low_receipt.bank = low.clone();
        low_receipt.section.lod = 2;
        low_receipt.section.bundle = "LOD2_".into();
        low_receipt.section.joint = "x2_light_brake_pod_bl".into();
        app.world_mut()
            .entity_mut(entity)
            .insert(runtime::Receiver::from_receipt(0, &low_receipt).unwrap());
        app.update();
        let low_owned = current(&app);
        assert_ne!(low_owned, intact);
        assert_eq!(check(&app, &response, &image), child);
        assert_eq!(
            app.world()
                .resource::<Assets<StandardMaterial>>()
                .get(&low_owned)
                .unwrap()
                .emissive,
            LinearRgba::BLACK
        );
        app.world_mut()
            .resource_mut::<Atlas>()
            .0
            .insert(low, image.clone());
        app.update();
        assert_eq!(current(&app), low_owned);
        assert_eq!(check(&app, &response, &image), child);
        assert_eq!(
            app.world()
                .resource::<Assets<StandardMaterial>>()
                .get(&low_owned)
                .unwrap()
                .emissive,
            LinearRgba::rgb(0.7, 0.7, 0.7)
        );
    }

}
