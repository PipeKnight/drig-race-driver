//! Source surface-layer reconstruction with authored Bevy lighting, on the
//! route's track (docs/SKY.md "Road surface"). The six road/infield shader
//! groups share the examined albedo arithmetic. The nonreflective
//! track_surface program also uses its independently authored normal basis.
use bevy::{
    asset::{RenderAssetUsages, uuid_handle},
    ecs::system::SystemParamItem,
    mesh::{MeshVertexAttribute, MeshVertexBufferLayoutRef},
    pbr::{
        ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline,
    },
    prelude::*,
    render::{
        render_resource::{
            AsBindGroup, AsBindGroupError, BindGroupLayout, BindGroupLayoutEntry, BindingType,
            RenderPipelineDescriptor, ShaderStages, SpecializedMeshPipelineError,
            UnpreparedBindGroup, VertexFormat,
        },
        renderer::RenderDevice,
        storage::ShaderBuffer,
    },
    shader::{ShaderDefVal, ShaderRef},
};
use grid_formats::{material, mesh, pssg, scene, texture};
use grid_tools::route::Route;
use std::{collections::BTreeMap, path::Path};

/// Optional authored GGX base-F0 weighting; not the source Phong model.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Specular {
    #[default]
    Off,
    SourceBase,
    WhiteControl,
}
impl Specular {
    pub fn parse(value: &str) -> Result<Self, &'static str> {
        match value {
            "off" => Ok(Self::Off),
            "source-base" => Ok(Self::SourceBase),
            "white-control" => Ok(Self::WhiteControl),
            _ => Err("--surface-specular needs off, source-base or white-control"),
        }
    }
    /// Surface-only diagnostic: independent of the enhanced lighting preset.
    /// Off must preserve all previously valid scenes, including car/collision.
    pub fn validate_context(self, layered_track: bool, track_scene: bool, collision: bool) -> Result<(), &'static str> {
        if self != Self::Off && (!layered_track || !track_scene || collision) {
            Err("--surface-specular requires a non-collision track scene with --track-materials source")
        } else {
            Ok(())
        }
    }
    fn control(self) -> f32 {
        match self {
            Self::Off => 0.,
            Self::SourceBase => 1.,
            Self::WhiteControl => 2.,
        }
    }
}

/// Groups whose pixel programs blend TDiffuseAlphaMap/TDiffuseAlphaMap2 by
/// mask red, TMacroColour (UV1) by mask green; `true`: also the skid layers.
/// `track_infield_wsm` takes its diffuse UVs from the world position.
const GROUPS: [(&[u8], bool); 6] = [
    (b"#track_surface_simple.fx", true),
    (b"#track_surface.fx", true),
    (b"#track_surface_reflective.fx", true),
    (b"#track_infield.fx", false),
    (b"#track_infield_dxt5nm.fx", false),
    (b"#track_infield_wsm.fx", false),
];

const SHADER: Handle<Shader> = uuid_handle!("ef412ff6-5acc-4977-bdb9-d5f314970b66");
const BASIS_SHADER: Handle<Shader> = uuid_handle!("81c59e94-1394-4d97-9544-e05baf815837");
const NORMAL_BINDING: u32 = 109;
const AO_TEXTURE_BINDING: u32 = 111;
const AO_BUFFER_BINDING: u32 = 112;
const AO_DEF: &str = "GRID_TRACK_DYNAMIC_AO";
const NORMAL_DEF: &str = "GRID_TRACK_SOURCE_NORMAL";

fn fragment_textures_supported(device: &RenderDevice, optional: u32) -> bool {
    let limits = device.limits();
    let standard_textures: u32 = StandardMaterial::bind_group_layout_entries(device, true)
        .iter()
        .filter(|entry| {
            entry.visibility.contains(ShaderStages::FRAGMENT)
                && matches!(entry.ty, BindingType::Texture { .. })
        })
        .map(|entry| entry.count.map_or(1, std::num::NonZeroU32::get))
        .sum();
    // The four material textures and Bevy's view textures are bound even
    // when the optional source normal or vehicle AO are disabled.
    const VIEW_TEXTURE_BUDGET: u32 = 10;
    limits.max_sampled_textures_per_shader_stage >= standard_textures + VIEW_TEXTURE_BUDGET + 4 + optional
}

fn normal_layout_supported(device: &RenderDevice) -> bool {
    let limits = device.limits();
    limits.max_bindings_per_bind_group > NORMAL_BINDING
        && fragment_textures_supported(device, 1)
        && limits.max_vertex_attributes >= 10
        && limits.max_inter_stage_shader_variables >= 12
        && limits.max_vertex_buffer_array_stride >= 80
}

fn dynamic_ao_layout_supported(device: &RenderDevice) -> bool {
    let limits = device.limits();
    limits.max_bindings_per_bind_group > AO_BUFFER_BINDING
        && limits.max_storage_buffers_per_shader_stage >= 8
        && fragment_textures_supported(device, 2)
}
pub(crate) type SurfaceMaterial = ExtendedMaterial<StandardMaterial, Surface>;

/// Source track directions are placed in world space before static batching.
/// Use independent raw lanes; Bevy's standard tangent is a signed vec4.
pub(crate) const SOURCE_TANGENT: MeshVertexAttribute =
    MeshVertexAttribute::new("SourceTrackTangent", 0x5354_544e, VertexFormat::Float32x3);
pub(crate) const SOURCE_BINORMAL: MeshVertexAttribute =
    MeshVertexAttribute::new("SourceTrackBinormal", 0x5354_424e, VertexFormat::Float32x3);


/// IDs are package-local; include the material identity when selecting draws.
#[derive(Component)]
pub struct SourcePart(pub String);

#[derive(Asset, TypePath, Clone, Debug)]
pub(crate) struct Surface {
    bindings: SurfaceBindings,
}

#[derive(AsBindGroup, Clone, Debug)]
pub(crate) struct SurfaceBindings {
    #[texture(100)]
    #[sampler(101)]
    first: Handle<Image>,
    #[texture(102)]
    #[sampler(103)]
    second: Handle<Image>,
    #[texture(104)]
    #[sampler(105)]
    macro_map: Handle<Image>,
    #[texture(106)]
    #[sampler(107)]
    skids: Handle<Image>,
    #[texture(109, visibility(fragment))]
    normal: Option<Handle<Image>>,
    #[texture(111, dimension = "2d_array", visibility(fragment))]
    dynamic_ao_atlas: Option<Handle<Image>>,
    #[storage(112, read_only, visibility(fragment))]
    dynamic_ao_volumes: Handle<ShaderBuffer>,
    // AsBindGroup packs fields at binding 108 in declaration order. Keep
    // SurfaceUniforms in track_surface.wgsl in sync. Separate buffers here can
    // exhaust the Metal vertex-stage slots, even though only the fragment
    // shader reads these values.
    // A scale, B scale, C scale, A offset.
    #[uniform(108)]
    scales: Vec4,
    // B offset, skid enabled, authored macro AO strength, 1/PScale (wsm) or 0.
    #[uniform(108)]
    controls: Vec4,
    #[uniform(108)]
    skid_colour: Vec4,
    #[uniform(108)]
    skid_colour2: Vec4,
    // Per-layer response codes:0road,1standalone terrain,2optional U-band.
    // Z/W are authored roughness/reflectance, not recovered source values.
    #[uniform(108)]
    dry_ground: Vec4,
    // Camera mip bias opt-in, specular mode, speed low-pass bias, asphalt layer bits.
    // controls.w remains the source world-projected UV scale.
    #[uniform(108)]
    sampling: Vec4,
    // X: RGB mixture; Y: standalone-road authored grazing F0 gain or0; ZW reserved.
    // Alpha stays on original taps.
    #[uniform(108)]
    rgb_filter: Vec4,
    // The source Phong lobe: PhongPower, Fresnel min, max, power (all 0: none).
    #[uniform(108)]
    phong: Vec4,
    // specularColour as radiance per unit lobe; W 1 replaces Bevy's specular.
    #[uniform(108)]
    phong_light: Vec4,
    // specularDirection, toward the light.
    #[uniform(108)]
    phong_direction: Vec4,
}

impl AsBindGroup for Surface {
    type Data = (bool, bool);
    type Param = <SurfaceBindings as AsBindGroup>::Param;

    fn label() -> &'static str { "Surface" }

    fn bind_group_data(&self) -> Self::Data {
        (self.bindings.normal.is_some(), self.bindings.dynamic_ao_atlas.is_some())
    }

    fn unprepared_bind_group(
        &self,
        layout: &BindGroupLayout,
        device: &RenderDevice,
        param: &mut SystemParamItem<'_, '_, Self::Param>,
        force_no_bindless: bool,
    ) -> Result<UnpreparedBindGroup, AsBindGroupError> {
        let mut bindings = self.bindings.unprepared_bind_group(
            layout, device, param, force_no_bindless,
        )?;
        if !normal_layout_supported(device) {
            bindings.bindings.0.retain(|(slot, _)| *slot != NORMAL_BINDING);
        }
        if !dynamic_ao_layout_supported(device) {
            bindings.bindings.0.retain(|(slot, _)| !matches!(*slot, AO_TEXTURE_BINDING | AO_BUFFER_BINDING));
        }
        Ok(bindings)
    }

    fn bind_group_layout_entries(
        device: &RenderDevice,
        force_no_bindless: bool,
    ) -> Vec<BindGroupLayoutEntry> {
        let mut entries = SurfaceBindings::bind_group_layout_entries(device, force_no_bindless);
        if !normal_layout_supported(device) {
            entries.retain(|entry| entry.binding != NORMAL_BINDING);
        }
        if !dynamic_ao_layout_supported(device) {
            entries.retain(|entry| !matches!(entry.binding, AO_TEXTURE_BINDING | AO_BUFFER_BINDING));
        }
        entries
    }
}

/// Bounded shared-lighting diagnostic only. Existing sourcePhong semantics,
/// weights, source directions, textures and material controls are preserved.
pub(crate) fn set_trial_phong_colour(materials: &mut Assets<SurfaceMaterial>, colour: [f32; 3]) -> usize {
    set_phong_lighting(materials, colour, None).0
}

/// Set a sampled source lobe, counting participating and actually changed assets.
pub(crate) fn set_phong_lighting(
    materials: &mut Assets<SurfaceMaterial>,
    colour: [f32; 3],
    direction: Option<[f32; 3]>,
) -> (usize, usize) {
    let colour = Vec3::from_array(colour);
    let direction = direction.map(|value| Vec3::from_array(value).normalize());
    let mut participating = 0;
    let updates: Vec<_> = materials.iter().filter_map(|(id, material)| {
        let surface = &material.extension.bindings;
        if surface.phong_light.w <= 0. {
            return None;
        }
        participating += 1;
        let light = colour.extend(surface.phong_light.w);
        let direction = direction.map_or(surface.phong_direction, |value| {
            value.extend(surface.phong_direction.w)
        });
        (light != surface.phong_light || direction != surface.phong_direction)
            .then_some((id, light, direction))
    }).collect();
    for &(id, light, direction) in &updates {
        // AssetMut emits Modified only after an actual mutable dereference.
        if let Some(mut material) = materials.get_mut(id) {
            material.extension.bindings.phong_light = light;
            material.extension.bindings.phong_direction = direction;
        }
    }
    (participating, updates.len())
}
#[cfg(test)]
pub(crate) fn trial_fixture() -> SurfaceMaterial {
    trial_fixture_with_strength(1.)
}

#[cfg(test)]
pub(crate) fn trial_fixture_with_strength(strength: f32) -> SurfaceMaterial {
    SurfaceMaterial {base: StandardMaterial::default(),extension: Surface { bindings: SurfaceBindings {
        first: default(),second: default(),macro_map: default(),skids:default(),normal:None,
        dynamic_ao_atlas:None,dynamic_ao_volumes:default(),
        scales:Vec4::ZERO,controls:Vec4::ZERO,skid_colour:Vec4::ZERO,skid_colour2:Vec4::ZERO,
        dry_ground:Vec4::ZERO,sampling:Vec4::ZERO,rgb_filter:Vec4::ZERO,phong:Vec4::ZERO,
        phong_light:Vec4::new(3.,2.,1.,strength),phong_direction:Vec4::new(0.2,0.8,0.1,0.),
    }}}
}
#[cfg(test)]
pub(crate) fn trial_snapshot(material:&SurfaceMaterial)->(Vec4,Vec4) {
    (material.extension.bindings.phong_light,material.extension.bindings.phong_direction)
}

impl MaterialExtension for Surface {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }
    fn deferred_fragment_shader() -> ShaderRef {
        SHADER.into()
    }

    fn specialize(
        _: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let Some(fragment) = descriptor.fragment.as_mut() else {
            return Ok(());
        };
        let def = |name: &str| {
            descriptor.vertex.shader_defs.iter()
                .any(|value| matches!(value, ShaderDefVal::Bool(key, true) if key == name))
        };
        if fragment.shader == SHADER && key.bind_group_data.1 && !def("PREPASS_PIPELINE") {
            fragment.shader_defs.push(AO_DEF.into());
        }
        // Default depth, shadow, normal and motion prepasses use their stock
        // fragment handle. The deferred color pass keeps its prepass IO.
        if !key.bind_group_data.0
            || fragment.shader != SHADER
            || (def("PREPASS_PIPELINE") && !def("DEFERRED_PREPASS"))
            || ["SKINNED", "MORPH_TARGETS", "VERTEX_TANGENTS", "MESHLET_MESH_MATERIAL_PASS"]
                .into_iter().any(def)
            || !["VERTEX_POSITIONS", "VERTEX_NORMALS", "VERTEX_UVS_A", "VERTEX_UVS_B", "VERTEX_COLORS"]
                .into_iter().all(|name| {
                    // Bevy prepass Position is unconditional, unlike forward.
                    (name == "VERTEX_POSITIONS" && def("PREPASS_PIPELINE")) || def(name)
                })
            || !layout.0.contains(SOURCE_TANGENT.id)
            || !layout.0.contains(SOURCE_BINORMAL.id)
        {
            return Ok(());
        }
        let extra = layout.0.get_layout(&[
            SOURCE_TANGENT.at_shader_location(8),
            SOURCE_BINORMAL.at_shader_location(9),
        ])?;
        let Some(buffer) = descriptor.vertex.buffers.first_mut() else {
            return Ok(());
        };
        if buffer.array_stride != extra.array_stride
            || buffer.attributes.iter().any(|attribute| attribute.shader_location >= 8)
        {
            return Ok(());
        }
        buffer.attributes.extend(extra.attributes);
        descriptor.vertex.shader = BASIS_SHADER;
        fragment.shader_defs.push(NORMAL_DEF.into());
        Ok(())
    }
}

struct Definition {
    dry_ground: Vec4,
    phong: Vec4,
    textures: [String; 4],
    normal_map: Option<String>,
    scales: Vec4,
    controls: Vec4,
    colours: [Vec4; 2],
}
struct Draw {
    material: String,
    uv: Vec<[f32; 2]>,
    masks: Vec<[f32; 4]>,
    normal_basis: Option<NormalBasis>,
}

/// The nonreflective track_surface VS exports both authored vectors as-is.
/// Do not reconstruct either via a cross product or infer handedness.
struct NormalBasis {
    tangent: Vec<[f32; 3]>,
    binormal: Vec<[f32; 3]>,
}

impl NormalBasis {
    fn from_draw(
        source: &mesh::Mesh<'_>,
        vertices: std::ops::Range<usize>,
        transform: &[f32; 16],
    ) -> Option<Self> {
        let normal = source.normals.get(vertices.clone())?;
        let linear = Mat3::from_mat4(Mat4::from_cols_array(transform));
        let to_world = |values: &[[f32; 3]]| {
            if linear == Mat3::IDENTITY {
                values.to_vec()
            } else {
                values.iter().map(|&v| (linear * Vec3::from_array(v)).to_array()).collect()
            }
        };
        let tangent = to_world(source.tangents.get(vertices.clone())?);
        let binormal = to_world(source.binormals.get(vertices)?);
        let finite_basis = [normal, &tangent, &binormal]
            .into_iter()
            .flatten()
            .flatten()
            .all(|component| component.is_finite());
        (finite_basis && !normal.is_empty()).then_some(Self { tangent, binormal })
    }
}
#[derive(Resource)]
pub(crate) struct Source {
    definitions: BTreeMap<String, Definition>,
    draws: BTreeMap<String, Draw>,
    textures: BTreeMap<String, Vec<u8>>,
    normal_maps: BTreeMap<String, Vec<u8>>,
}

/// The track's tracksplit plus the route's routesplit (main.rs
/// `append_route_split`), whose materials name the tracksplit's textures.
fn read_source(root: &Path, route: &Route) -> Result<Source, Box<dyn std::error::Error>> {
    let file = route.track_file("tracksplit.pssg");
    let bytes = super::read(&root.join(&file))?;
    let tree = pssg::read_tree(&bytes)?;
    let mut source = read_surfaces(&tree, &tree).map_err(|e| format!("{file}: {e}"))?;
    let split = route.file("routesplit.pssg");
    if root.join(&split).is_file() {
        let bytes = super::read(&root.join(&split))?;
        let split_tree = pssg::read_tree(&bytes)?;
        let part = read_surfaces(&split_tree, &tree).map_err(|e| format!("{split}: {e}"))?;
        merge(&mut source, part, "route", false);
    }
    Ok(source)
}

fn read_optional_package(
    root: &Path,
    route: &Route,
    package: &str,
) -> Result<Source, Box<dyn std::error::Error>> {
    let file = route.track_file(package);
    read_optional_bytes(&super::read(&root.join(&file))?)
        .map_err(|e| format!("{file}: {e}").into())
}

/// Adds `other`'s materials and draws under `namespace/`, as main.rs names
/// them; its textures too when they are its own.
fn merge(source: &mut Source, other: Source, namespace: &str, own_textures: bool) {
    let texture = |name: String| {
        if own_textures {
            format!("{namespace}/{name}")
        } else {
            name
        }
    };
    for (name, mut definition) in other.definitions {
        for t in &mut definition.textures {
            *t = texture(std::mem::take(t));
        }
        definition.normal_map = definition.normal_map.map(texture);
        source
            .definitions
            .insert(format!("{namespace}/{name}"), definition);
    }
    for (name, mut draw) in other.draws {
        draw.material = format!("{namespace}/{}", draw.material);
        source.draws.insert(format!("{namespace}/{name}"), draw);
    }
    for (name, dds) in other.textures {
        source.textures.insert(texture(name), dds);
    }
    for (name, dds) in other.normal_maps {
        source.normal_maps.insert(texture(name), dds);
    }
}

fn read_optional_bytes(bytes: &[u8]) -> Result<Source, Box<dyn std::error::Error>> {
    let tree = pssg::read_tree(bytes)?;
    if scene::is_empty_optional_static_package(&tree)? {
        return Ok(Source {
            definitions: BTreeMap::new(),
            draws: BTreeMap::new(),
            textures: BTreeMap::new(),
            normal_maps: BTreeMap::new(),
        });
    }
    read_surfaces(&tree, &tree)
}

/// Surfaces of `tree` with their textures taken from `textures`.
fn read_surfaces(
    tree: &pssg::Tree,
    textures_from: &pssg::Tree,
) -> Result<Source, Box<dyn std::error::Error>> {
    let bindings = material::read_texture_bindings(tree)?;
    let mut definitions = BTreeMap::new();
    for i in 0..tree.nodes.len() {
        if tree.name(i) != b"SHADERINSTANCE" {
            continue;
        }
        let group_ref = tree.string(i, b"shaderGroup")?;
        let Some(&(_, skidded)) = GROUPS.iter().find(|(g, _)| *g == group_ref) else {
            continue;
        };
        let world_mapped = group_ref == b"#track_infield_wsm.fx";
        let reflective = group_ref == b"#track_surface_reflective.fx";
        let id = tree.string(i, b"id")?;
        let group = (0..tree.nodes.len())
            .find(|&j| {
                tree.name(j) == b"SHADERGROUP"
                    && tree.string(j, b"id").is_ok_and(|s| s == &group_ref[1..])
            })
            .ok_or("surface shader group absent")?;
        let inputs: Vec<_> = tree.nodes[group]
            .children
            .iter()
            .copied()
            .filter(|&j| tree.name(j) == b"SHADERINPUTDEFINITION")
            .collect();
        let mut constants = BTreeMap::new();
        for &j in &tree.nodes[i].children {
            if tree.string(j, b"type")? != b"constant" {
                continue;
            }
            let definition = *inputs
                .get(tree.integer(j, b"parameterID")? as usize)
                .ok_or("surface constant index invalid")?;
            let data = tree.nodes[j]
                .data
                .ok_or("surface constant payload absent")?;
            if data.is_empty() || !data.len().is_multiple_of(4) || data.len() > 16 {
                return Err("unsupported surface constant size".into());
            }
            let values: Vec<_> = data
                .as_chunks::<4>()
                .0
                .iter()
                .map(|c| f32::from_be_bytes(*c))
                .collect();
            if values.iter().any(|f| !f.is_finite()) {
                return Err("nonfinite surface constant".into());
            }
            constants.insert(tree.string(definition, b"name")?, values);
        }
        let scalar = |name: &[u8]| -> Result<f32, &'static str> {
            match constants.get(name).map(Vec::as_slice) {
                Some([value]) => Ok(*value),
                _ => Err("surface scalar missing or wrong size"),
            }
        };
        let colour = |name: &[u8]| -> Result<Vec4, &'static str> {
            match constants.get(name).map(Vec::as_slice) {
                Some([r, g, b]) => Ok(Vec4::new(*r, *g, *b, 1.)),
                Some([r, g, b, a]) => Ok(Vec4::new(*r, *g, *b, *a)),
                _ => Err("surface colour missing or wrong size"),
            }
        };
        let texture_name = |sampler: &[u8]| -> Option<String> {
            bindings
                .iter()
                .find(|b| b.shader == id && b.sampler == sampler)
                .map(|b| String::from_utf8_lossy(b.texture).into_owned())
        };
        let first = texture_name(b"TDiffuseAlphaMap").ok_or("surface diffuse absent")?;
        // Some materials leave inputs unsaved (Jarama's edge strips have no
        // TMacroColour). AUTHORED [I: the runtime binding of an unsaved
        // sampler is untraced]: no second layer, or no macro blend and AO.
        let second = texture_name(b"TDiffuseAlphaMap2").unwrap_or_else(|| first.clone());
        let macro_map = texture_name(b"TMacroColour");
        let has_macro = macro_map.is_some();
        let macro_map = macro_map.unwrap_or_else(|| first.clone());
        let skids = texture_name(b"TSkidsMap");
        let normal_map = if matches!(
            group_ref,
            b"#track_surface.fx" | b"#track_infield_dxt5nm.fx"
        ) {
            texture_name(b"TNormalMap")
        } else {
            None
        };
        // The pixel programs' direct specular: pow(sat(dot(reflect(I, N),
        // specularDirection)), PhongPower) * lerp(FresnelMin, FresnelMax,
        // pow_sat(1 + dot(I, N), FresnelPower)). track_infield_wsm.cg.fp
        // takes FresnelMax alone; track_surface_reflective.cg.fp has no lobe
        // (its ReflectionMap is not ported).
        let phong = if reflective {
            Vec4::ZERO
        } else if world_mapped {
            let max = scalar(b"FresnelMax")?;
            Vec4::new(scalar(b"PhongPower")?, max, max, 1.)
        } else {
            Vec4::new(
                scalar(b"PhongPower")?,
                scalar(b"FresnelMin")?,
                scalar(b"FresnelMax")?,
                scalar(b"FresnelPower")?,
            )
        };
        let definition = Definition {
            phong,
            dry_ground: Vec4::new(
                dry_ground_layer(&first, !skidded),
                dry_ground_layer(&second, !skidded),
                1.,   // authored dry-ground perceptual roughness
                0.25, // dielectric normal-incidence F0 = 0.16 * 0.25^2 = 1%
            ),
            textures: [
                first,
                second,
                macro_map.clone(),
                skids.clone().unwrap_or(macro_map),
            ],
            normal_map,
            scales: if skidded {
                Vec4::new(
                    scalar(b"Skid_A_Scale")?,
                    scalar(b"Skid_B_Scale")?,
                    scalar(b"Skid_C_Scale")?,
                    scalar(b"A_Offset")?,
                )
            } else {
                Vec4::ZERO
            },
            controls: Vec4::new(
                if skidded { scalar(b"B_Offset")? } else { 0. },
                if skidded && skids.is_some() { 1. } else { 0. },
                // Macro AO strength; -1 marks a material without a macro map.
                if has_macro { 1. } else { -1. },
                if world_mapped {
                    let scale = scalar(b"PScale")?;
                    if !scale.is_finite() || scale <= 0. {
                        return Err("surface PScale not positive".into());
                    }
                    1. / scale
                } else {
                    0.
                },
            ),
            colours: if skidded {
                [colour(b"SkidColour")?, colour(b"SkidColour2")?]
            } else {
                [Vec4::ZERO; 2]
            },
        };
        definitions.insert(String::from_utf8(id.to_vec())?, definition);
    }
    let sources = mesh::read_meshes(tree)?;
    let mut draws = BTreeMap::new();
    for draw in scene::read_static_draws(tree, &sources)? {
        let material = String::from_utf8(draw.material.to_vec())?;
        let Some(definition) = definitions.get(&material) else {
            continue;
        };
        let source = &sources[draw.source];
        let uv = source
            .additional_texcoords
            .first()
            .ok_or("surface UV1 absent")?;
        if uv.len() != source.positions.len() || source.colors_argb.len() != source.positions.len()
        {
            return Err("surface UV/mask count mismatch".into());
        }
        let uv = uv[draw.vertices.clone()].to_vec();
        if uv.iter().flatten().any(|v| !v.is_finite()) {
            return Err("surface UV1 nonfinite".into());
        }
        let normal_basis = definition
            .normal_map
            .as_ref()
            .and_then(|_| NormalBasis::from_draw(source, draw.vertices.clone(), &draw.transform));
        let masks = source.colors_argb[draw.vertices]
            .iter()
            .map(|&argb| mask(argb))
            .collect();
        if draws
            .insert(
                String::from_utf8(draw.id.to_vec())?,
                Draw {
                    material,
                    uv,
                    masks,
                    normal_basis,
                },
            )
            .is_some()
        {
            return Err("duplicate surface draw name".into());
        }
    }
    let mut textures = BTreeMap::new();
    let mut normal_maps = BTreeMap::new();
    for t in texture::read_textures(textures_from)? {
        let name = String::from_utf8(t.id.to_vec())?;
        if definitions.values().any(|d| d.textures.contains(&name)) {
            textures.insert(name.clone(), t.validated_dds().0);
        }
        if t.format == texture::Format::Bc3
            && definitions.values().any(|d| d.normal_map.as_ref() == Some(&name))
        {
            normal_maps.insert(name, t.validated_dds().0);
        }
    }
    for d in definitions.values() {
        if d.textures.iter().any(|name| !textures.contains_key(name)) {
            return Err("surface texture unresolved".into());
        }
    }
    println!(
        "track_surface prepared materials={} draws={} textures={} lighting=authored macro_ao_scale=sao_scale",
        definitions.len(),
        draws.len(),
        textures.len()
    );
    Ok(Source {
        definitions,
        draws,
        textures,
        normal_maps,
    })
}

pub fn install(
    app: &mut App,
    root: &Path,
    route: &Route,
    land: bool,
    rgb_overrides: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut source = read_source(root, route)?;
    if land {
        let land = read_optional_package(root, route, "land.pssg")?;
        merge(&mut source, land, "land", true);
    }
    if let Some(manifest) = rgb_overrides {
        let used = source
            .draws
            .values()
            .flat_map(|draw| {
                source.definitions[&draw.material].textures[..2]
                    .iter()
                    .cloned()
            })
            .collect();
        super::surface_rgb::apply(manifest, &mut source.textures, &used)?;
    }
    app.world_mut().resource_mut::<Assets<Shader>>().insert(
        SHADER.id(),
        Shader::from_wgsl(
            include_str!("track_surface.wgsl"),
            "grid_track_surface.wgsl",
        ),
    )?;
    if !source.normal_maps.is_empty() {
        app.world_mut().resource_mut::<Assets<Shader>>().insert(
            BASIS_SHADER.id(),
            Shader::from_wgsl(
                include_str!("track_surface_basis.wgsl"),
                "grid_track_surface_basis.wgsl",
            ),
        )?;
    }
    app.insert_resource(source)
        .add_plugins(MaterialPlugin::<SurfaceMaterial>::default())
        .add_systems(PostStartup, attach)
        .add_systems(FixedUpdate, update_asphalt_softness.after(super::drive_tick));
    Ok(())
}

// Select only known standalone terrain layers. Mixed road-edge atlases are not
// classified wholesale: that would also dull their asphalt/paint texels.
fn dry_ground_layer(texture: &str, infield_shader: bool) -> f32 {
    let texture = texture.strip_prefix("land/").unwrap_or(texture);
    f32::from(
        infield_shader
            || matches!(
                texture,
                "don_infield1_d.tga"
                    | "don_infield2_d.tga"
                    | "don_infield2_varb_d.tga"
                    | "don_infield3_d.tga"
                    | "don_infield4_d.tga"
                    | "don_infield5_d.tga"
                    | "don_outfield_terrain_01_d.tga"
                    | "don_outfield_terrain_02_d.tga"
            ),
    )
}

// Code2 requests an authored U-band, not a whole-atlas terrain classification.
// Native source atlas review puts ground/gravel left of the white road line.
fn edge_ground_layer(texture: &str, standalone: f32, enabled: bool) -> f32 {
    let texture = texture.strip_prefix("land/").unwrap_or(texture);
    if enabled
        && standalone < 0.5
        && matches!(
            texture,
            "don_edge1to1_d.tga"
                | "don_edge1to1varb_d.tga"
                | "don_edge2to2_d.tga"
                | "don_edge3to1_d.tga"
        )
    {
        2.
    } else {
        standalone
    }
}

// Exact audited standalone layers only; edge/paint atlases must never be blurred.
fn asphalt_layer(name: &str) -> bool {
    matches!(name, "don_track1_d.tga" | "don_track2_d.tga" | "don_track3_d.tga" | "don_track4_d.tga")
}

// Narrow host-only lighting experiment: both layers must be audited pure road.
// Keep mixed atlases and every infield-family definition on their existing response.
fn standalone_asphalt(definition: &Definition) -> bool {
    definition.dry_ground.x == 0.
        && definition.dry_ground.y == 0.
        && asphalt_layer(&definition.textures[0])
        && asphalt_layer(&definition.textures[1])
}

// A direct function of physical speed, not render delta or displacement per frame.
// Fade in between 18 and108 km/h; reverse gets the same filtering at equal speed.
fn asphalt_bias(velocity: [f32; 4], maximum: f32) -> f32 {
    let speed = velocity[..3].iter().map(|v| v * v).sum::<f32>().sqrt();
    if !speed.is_finite() { return 0.; }
    let t = ((speed - 5.) / 25.).clamp(0., 1.);
    maximum * t * t * (3. - 2. * t)
}

fn update_asphalt_softness(
    driving: Option<Res<super::Driving>>,
    settings: Res<super::graphics::Settings>,
    mut materials: ResMut<Assets<SurfaceMaterial>>,
) {
    if settings.asphalt_soften == 0. { return; }
    let bias = driving.as_ref().map_or(0., |d| asphalt_bias(d.runtime.velocity(), settings.asphalt_soften));
    for (_, material) in materials.iter_mut() {
        if material.extension.bindings.sampling.w > 0. {
            material.extension.bindings.sampling.z = bias;
        }
    }
}

// uint_color_argb is stored A,R,G,B. It is a linear control, not sRGB albedo.
fn mask([a, r, g, b]: [u8; 4]) -> [f32; 4] {
    let mut controls = [r, g, b, a].map(|v| f32::from(v) / 255.);
    controls[1] = controls[1].min(0.5);
    controls
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn attach(
    mut commands: Commands,
    source: Res<Source>,
    settings: Res<super::graphics::Settings>,
    dynamic_ao: Option<Res<super::ground_ao::GroundAo>>,
    mut ao_buffers: ResMut<Assets<ShaderBuffer>>,
    query: Query<(
        Entity,
        &Name,
        &SourcePart,
        &Mesh3d,
        &MeshMaterial3d<StandardMaterial>,
    )>,
    mut meshes: ResMut<Assets<Mesh>>,
    standard: Res<Assets<StandardMaterial>>,
    render_device: Option<Res<RenderDevice>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<SurfaceMaterial>>,
) {
    // Refuse a no-target comparison before mutating meshes, images or materials.
    if settings.asphalt_roughness.is_some() || settings.asphalt_grazing_gain.is_some() {
        let eligible = query.iter().any(|(_, name, identity, mesh, _)| {
            let Some(draw) = source.draws.get(name.as_str()) else { return false; };
            draw.material == identity.0
                && standalone_asphalt(&source.definitions[&draw.material])
                && meshes.get(&mesh.0).is_some_and(|mesh| mesh.count_vertices() == draw.uv.len())
        });
        assert!(eligible, "asphalt material pilot has no eligible attached standalone road draw");
    }
    let textures: BTreeMap<_, _> = source
        .textures
        .iter()
        .map(|(name, dds)| {
            let image = super::texture_upload::image(
                dds,
                settings.sampler(true),
                RenderAssetUsages::default(),
            )
            .expect("validated surface DDS");
            (name.clone(), images.add(image))
        })
        .collect();
    // Skid red is a weight, not a color. Alpha in the macro map stays linear
    // even though its RGB color is decoded from sRGB by the GPU.
    let skid_textures: BTreeMap<_, _> = source
        .definitions
        .values()
        .map(|d| &d.textures[3])
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .map(|name| {
            let image = super::texture_upload::image_linear(
                &source.textures[name],
                settings.sampler(true),
                RenderAssetUsages::default(),
            )
            .expect("validated linear skid DDS");
            (name.clone(), images.add(image))
        })
        .collect();
    let normal_supported = render_device.as_deref().is_some_and(normal_layout_supported);
    let mut normal_maps = BTreeMap::new();
    if normal_supported {
        for (name, dds) in &source.normal_maps {
            match super::texture_upload::image_linear(
                dds, settings.sampler(true), RenderAssetUsages::default(),
            ) {
                Ok(image) => { normal_maps.insert(name.as_str(), images.add(image)); }
                Err(error) => { warn!("track surface normal {name} unavailable: {error}"); }
            }
        }
    }
    let mut basis_draws = 0;
    let mut basis_fallbacks = 0;
    let dynamic_ao = dynamic_ao.as_deref()
        .filter(|_| render_device.as_deref().is_some_and(dynamic_ao_layout_supported));
    let ao_buffer = dynamic_ao.map_or_else(
        || ao_buffers.add(ShaderBuffer::with_size(super::ground_ao::BUFFER_BYTES, RenderAssetUsages::RENDER_WORLD)),
        |ao| ao.buffer.clone(),
    );
    let mut handles = BTreeMap::new();
    let mut attached = 0;
    let mut mismatched = 0;
    let mut roughness_draws = 0;
    let mut roughness_materials = 0;
    let mut grazing_draws = 0;
    let mut grazing_materials = 0;
    for (entity, name, identity, mesh, base) in &query {
        let Some(draw) = source.draws.get(name.as_str()) else {
            continue;
        };
        if draw.material != identity.0 {
            continue;
        }
        let mut target = meshes.get_mut(&mesh.0).expect("surface mesh ready");
        // IDs are package-local: a same-named part of another package with
        // another vertex count keeps its plain material.
        if target.count_vertices() != draw.uv.len() {
            mismatched += 1;
            continue;
        }
        target.insert_attribute(Mesh::ATTRIBUTE_UV_1, draw.uv.clone());
        target.insert_attribute(Mesh::ATTRIBUTE_COLOR, draw.masks.clone());
        let d = &source.definitions[&draw.material];
        let normal_map = d.normal_map.as_deref().and_then(|name| normal_maps.get(name));
        if normal_map.is_some() {
            let (tangent, binormal) = if let Some(basis) = &draw.normal_basis {
                basis_draws += 1;
                (basis.tangent.clone(), basis.binormal.clone())
            } else {
                basis_fallbacks += 1;
                // Match all draws sharing the material's batch layout.
                let zero = vec![[0.; 3]; draw.uv.len()];
                (zero.clone(), zero)
            };
            target.insert_attribute(SOURCE_TANGENT, tangent);
            target.insert_attribute(SOURCE_BINORMAL, binormal);
        }
        let roughness = settings.asphalt_roughness.filter(|_| standalone_asphalt(d));
        let grazing_gain = settings.asphalt_grazing_gain.filter(|_| standalone_asphalt(d));
        if grazing_gain.is_some() { grazing_draws += 1; }
        if roughness.is_some() { roughness_draws += 1; }
        let handle = handles
            .entry(draw.material.clone())
            .or_insert_with(|| {
                let mut base = standard
                    .get(&base.0)
                    .expect("source base material ready")
                    .clone();
                // The extension composes albedo explicitly; never tint by mask colors.
                base.base_color_texture = None;
                if dynamic_ao.is_some() {
                    base.opaque_render_method = bevy::material::OpaqueRendererMethod::Forward;
                }
                if let Some(roughness) = roughness {
                    base.perceptual_roughness = roughness;
                    roughness_materials += 1;
                }
                if grazing_gain.is_some() { grazing_materials += 1; }
                materials.add(ExtendedMaterial {
                    base,
                    extension: Surface { bindings: SurfaceBindings {
                        normal: normal_map.cloned(),
                        dynamic_ao_atlas: dynamic_ao.map(|ao| ao.atlas.clone()),
                        dynamic_ao_volumes: ao_buffer.clone(),
                        first: textures[&d.textures[0]].clone(),
                        second: textures[&d.textures[1]].clone(),
                        macro_map: textures[&d.textures[2]].clone(),
                        skids: skid_textures[&d.textures[3]].clone(),
                        scales: d.scales,
                        sampling: Vec4::new(
                            if settings.surface_view_mip_bias { 1. } else { 0. },
                            settings.surface_specular.control(),
                            0.,
                            if settings.asphalt_soften > 0. {
                                f32::from(asphalt_layer(&d.textures[0]))
                                    + 2. * f32::from(asphalt_layer(&d.textures[1]))
                            } else { 0. },
                        ),
                        rgb_filter: Vec4::new(settings.asphalt_filter_strength, grazing_gain.unwrap_or(0.), 0., 0.),
                        phong: d.phong,
                        phong_light: settings.baseline_specular.map_or(Vec4::ZERO, |p| {
                            Vec4::new(p.colour[0], p.colour[1], p.colour[2], 1.)
                        }),
                        phong_direction: settings.baseline_specular.map_or(Vec4::ZERO, |p| {
                            Vec3::from_array(p.direction_to_light).normalize().extend(0.)
                        }),
                        // Macro AO strength from the route's lighting (`controls.z` > 0).
                        controls: if d.controls.z > 0. {
                            Vec4::new(d.controls.x, d.controls.y, settings.surface_ao_scale, d.controls.w)
                        } else {
                            d.controls
                        },
                        skid_colour: d.colours[0],
                        skid_colour2: d.colours[1],
                        dry_ground: Vec4::new(
                            edge_ground_layer(
                                &d.textures[0],
                                d.dry_ground.x,
                                settings.ground_edge_pilot,
                            ),
                            edge_ground_layer(
                                &d.textures[1],
                                d.dry_ground.y,
                                settings.ground_edge_pilot,
                            ),
                            d.dry_ground.z,
                            d.dry_ground.w,
                        ),
                    }},
                })
            })
            .clone();
        commands
            .entity(entity)
            .remove::<MeshMaterial3d<StandardMaterial>>()
            .insert(MeshMaterial3d(handle));
        attached += 1;
    }
    if !source.normal_maps.is_empty() {
        println!(
            "track_surface_normal layout_supported={normal_supported} maps={} basis_draws={basis_draws} basis_fallbacks={basis_fallbacks}",
            normal_maps.len(),
        );
    }
    println!("asphalt_soften maximum={} scope=don_track1-4-rgb-only speed_mps=5..30 default=off alpha_macro_skids=preserved", settings.asphalt_soften);
    println!("asphalt_filter_strength value={} scope=rgb-only alpha_macro_skids=preserved", settings.asphalt_filter_strength);
    if let Some(value) = settings.asphalt_roughness {
        println!("asphalt_roughness value={value} source_materials={roughness_materials} source_draws={roughness_draws} before_static_batch=true scope=standalone-don-track1-4-both-layers authored=true default=off masks_rgb_ground_motion=unchanged");
    }
    if let Some(gain) = settings.asphalt_grazing_gain {
        println!("asphalt_grazing gain={gain} ndotv_full=0.10 ndotv_off=0.35 source_materials={grazing_materials} source_draws={grazing_draws} before_static_batch=true scope=standalone-don-track1-4-both-layers-source-base authored_view_dependent_F0=true independent_physical_F90=false default=off alpha_rgb_macro_skids_ground_roughness_motion=unchanged");
    }
    println!("track_surface_sampling view_mip_bias={} default=false scope=source-layer-macro-skid-samples", settings.surface_view_mip_bias);
    println!(
        "track_surface attached draws={attached} of={} mismatched={mismatched} materials={} specular={:?} source_phong={} uv1=true masks=linear-ARGB-to-RGBA",
        source.draws.len(),
        handles.len(),
        settings.surface_specular,
        settings.baseline_specular.is_some()
    );
    if attached != source.draws.len() {
        warn!(
            "track surface: {} of {} source draws attached",
            attached,
            source.draws.len()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn specular_diagnostic_eligibility_preserves_off_and_requires_source_track() {
        for layered in [false, true] {
            for track_scene in [false, true] {
                for collision in [false, true] {
                    assert!(Specular::Off.validate_context(layered, track_scene, collision).is_ok());
                    for mode in [Specular::SourceBase, Specular::WhiteControl] {
                        assert_eq!(mode.validate_context(layered, track_scene, collision).is_ok(),
                                   layered && track_scene && !collision);
                    }
                }
            }
        }
    }
    #[test]
    fn specular_parser_accepts_only_explicit_modes_and_keeps_off_default() {
        assert_eq!(Specular::default(), Specular::Off);
        assert_eq!(Specular::parse("off"), Ok(Specular::Off));
        assert_eq!(Specular::parse("source-base"), Ok(Specular::SourceBase));
        assert_eq!(Specular::parse("white-control"), Ok(Specular::WhiteControl));
        for invalid in ["", "source", "on", "SourceBase", "source-base "] {
            assert!(Specular::parse(invalid).is_err());
        }
    }

    #[test]
    fn asphalt_softness_protects_mixed_atlases_and_unrelated_routes() {
        for name in ["don_track1_d.tga", "don_track2_d.tga", "don_track3_d.tga", "don_track4_d.tga"] {
            assert!(asphalt_layer(name));
        }
        for name in ["don_edge1to1_d.tga", "don_decals_01_d.tga", "don_rumble_strip_03_d.tga", "don_infield1_d.tga", "land/don_track1_d.tga", "other_track1_d.tga"] {
            assert!(!asphalt_layer(name));
        }
        assert_eq!(asphalt_bias([0.; 4], 4.), 0.);
        assert_eq!(asphalt_bias([5., 0., 0., 0.], 4.), 0.);
        assert_eq!(asphalt_bias([60., 0., 0., 0.], 4.), 4.);
        assert_eq!(asphalt_bias([-60., 0., 0., 0.], 4.), 4.);
        assert_eq!(asphalt_bias([f32::NAN, 0., 0., 0.], 4.), 0.);
        let samples: Vec<_> = (0..=120).map(|v| asphalt_bias([v as f32, 0., 0., 0.], 4.)).collect();
        assert!(samples.windows(2).all(|p| p[0] <= p[1]));
        assert!(samples.iter().all(|v| (0.0..=4.0).contains(v)));
    }

    #[test]
    #[ignore = "requires owner's purchased GRID installation"]
    fn purchased_surfaces_resolve_both_uv_sets_and_all_layers() {
        let source = read_source(
            Path::new(r"C:\Program Files (x86)\Steam\steamapps\common\Grid"),
            &Route::default(),
        )
        .unwrap();
        assert_eq!(source.definitions.len(), 94);
        assert!(!source.draws.is_empty());
        assert!(source.draws.values().all(|d| d.uv.len() == d.masks.len()));
        assert!(
            source
                .draws
                .values()
                .flat_map(|d| &d.masks)
                .any(|m| m[0] > 0. && m[0] < 1.)
        );
        println!(
            "surface_asset_validation draws={} materials={} textures={}",
            source.draws.len(),
            source.definitions.len(),
            source.textures.len()
        );
    }

    #[test]
    #[ignore = "requires owner's purchased GRID installation"]
    fn purchased_surfaces_resolve_on_every_track() {
        let root = Path::new(r"C:\Program Files (x86)\Steam\steamapps\common\Grid");
        for track in [
            "europe/istanbul_park",
            "europe/jarama",
            "europe/le_mans",
            "europe/milan",
            "europe/nurburgring",
            "europe/spa_francorchamps",
            "japan/haruna",
            "japan/okutama",
            "japan/shibuya",
            "japan/yokohama_docks",
            "usa/detroit",
            "usa/long_beach",
            "usa/san_fran",
            "usa/washington",
        ] {
            let route = Route::new(&format!("tracks/{track}/route_0")).unwrap();
            let source = read_source(root, &route).unwrap();
            // land.pssg is layered in enhanced only (Donington); Milan's has no
            // static draws at all.
            let land = read_optional_package(root, &route, "land.pssg");
            let world_mapped = source.definitions.values().filter(|d| d.controls.w > 0.).count();
            let without_macro = source.definitions.values().filter(|d| d.controls.z < 0.).count();
            // Every road and infield material saves its Phong constants.
            let phong = source.definitions.values().filter(|d| d.phong.x > 0.).count();
            println!(
                "surface {track} materials={} draws={} textures={} wsm={world_mapped} no_macro={without_macro} phong={phong} land={}",
                source.definitions.len(),
                source.draws.len(),
                source.textures.len(),
                land.map_or_else(|e| e.to_string(), |l| format!("{}/{}", l.definitions.len(), l.draws.len()))
            );
            assert!(!source.draws.is_empty(), "{track}");
            assert!(source.draws.values().all(|d| d.uv.len() == d.masks.len()));
            if track == "japan/haruna" {
                assert_eq!(world_mapped, 36);
            }
            // Every route's own road (routesplit.pssg, docs/SKY.md section 8) on the track's
            // textures.
            for r in 1..10 {
                let route = Route::new(&format!("tracks/{track}/route_{r}")).unwrap();
                if !root.join(route.file("routesplit.pssg")).is_file() {
                    continue;
                }
                let source = read_source(root, &route).unwrap();
                let split = source.draws.keys().filter(|k| k.starts_with("route/")).count();
                println!("surface {} route_draws={split}", route.path());
            }
            let split = source.draws.keys().filter(|k| k.starts_with("route/")).count();
            println!("surface {track}/route_0 route_draws={split}");
            if track == "usa/washington" {
                assert_eq!(split, 180);
            }
        }
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn purchased_surface_normals_resolve_route_and_track_bases() {
        let root = std::path::PathBuf::from(
            std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required"),
        );
        for (path, track_positive, route_positive) in [
            ("tracks/europe/donington/route_0", false, false),
            ("tracks/usa/detroit/route_8", true, true),
            ("tracks/usa/washington/route_0", true, true),
            ("tracks/europe/le_mans/route_0", true, false),
            ("tracks/europe/milan/route_0", false, true),
            ("tracks/japan/shibuya/route_0", false, false),
        ] {
            let route = Route::new(path).unwrap();
            let source = read_source(&root, &route).unwrap();
            let mut accepted = [0; 2];
            for (id, draw) in &source.draws {
                let Some(name) = &source.definitions[&draw.material].normal_map else {
                    continue;
                };
                assert_eq!(&source.normal_maps[name][84..88], b"DXT5", "{path} {name}");
                let Some(basis) = &draw.normal_basis else {
                    continue;
                };
                assert_eq!(basis.tangent.len(), draw.uv.len(), "{path} {id}");
                assert_eq!(basis.binormal.len(), draw.uv.len(), "{path} {id}");
                accepted[usize::from(id.starts_with("route/"))] += 1;
            }
            assert_eq!(accepted.map(|count| count > 0), [track_positive, route_positive], "{path}");
        }
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn purchased_infield_normals_resolve_with_source_bases_or_fallback() {
        let root = std::path::PathBuf::from(
            std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required"),
        );
        for (path, draw_id, map, finite_basis) in [
            ("tracks/europe/le_mans/route_0", "!Wp", "lem_edge_n.tga", true),
            (
                "tracks/usa/washington/route_0",
                "route/!cK",
                "was_feature1_n.tga",
                true,
            ),
            (
                "tracks/usa/washington/route_2",
                "route/!Q",
                "was_feature1_n.tga",
                false,
            ),
        ] {
            let route = Route::new(path).unwrap();
            let source = read_source(&root, &route).unwrap();
            let draw = &source.draws[draw_id];
            assert_eq!(
                source.definitions[&draw.material].normal_map.as_deref(),
                Some(map)
            );
            assert_eq!(&source.normal_maps[map][84..88], b"DXT5");
            assert_eq!(draw.normal_basis.is_some(), finite_basis, "{path} {draw_id}");
        }
    }

    #[test]
    fn mask_channels_are_linear_controls() {
        assert_eq!(mask([255, 0, 255, 128]), [0., 0.5, 128. / 255., 1.]);
        // Clamping after interpolation differs: midpoint of 0 and 255 must be
        // 0.25, not 0.5. The source clamps in the vertex program.
        let midpoint = (mask([255, 0, 0, 0])[1] + mask([255, 0, 255, 0])[1]) / 2.;
        assert_eq!(midpoint, 0.25);
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn purchased_empty_land_surface_adapter_returns_no_resources() {
        let root = std::path::PathBuf::from(
            std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required"),
        );
        let bytes = std::fs::read(root.join("tracks/japan/shibuya/land.pssg")).unwrap();
        let surface = read_optional_bytes(&bytes).unwrap();
        assert!(surface.definitions.is_empty());
        assert!(surface.draws.is_empty());
        assert!(surface.textures.is_empty());
    }
}
