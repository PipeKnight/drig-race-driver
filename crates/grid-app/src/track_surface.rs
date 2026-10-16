//! Source surface-layer reconstruction with authored Bevy lighting.
//! Only the statically examined surface_simple and infield variants are selected.
use bevy::{
    asset::{RenderAssetUsages, uuid_handle},
    image::{CompressedImageFormats, ImageType},
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};
use grid_formats::{material, mesh, pssg, scene, texture};
use std::{collections::BTreeMap, path::Path};

const SHADER: Handle<Shader> = uuid_handle!("ef412ff6-5acc-4977-bdb9-d5f314970b66");
type SurfaceMaterial = ExtendedMaterial<StandardMaterial, Surface>;

/// IDs are package-local; include the material identity when selecting draws.
#[derive(Component)]
pub struct SourcePart(pub String);

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct Surface {
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
    // A scale, B scale, C scale, A offset.
    #[uniform(108)]
    scales: Vec4,
    // B offset, skid enabled, authored macro AO strength, unused.
    #[uniform(109)]
    controls: Vec4,
    #[uniform(110)]
    skid_colour: Vec4,
    #[uniform(111)]
    skid_colour2: Vec4,
}
impl MaterialExtension for Surface {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }
    fn deferred_fragment_shader() -> ShaderRef {
        SHADER.into()
    }
}

struct Definition {
    textures: [String; 4],
    scales: Vec4,
    controls: Vec4,
    colours: [Vec4; 2],
}
struct Draw {
    material: String,
    uv: Vec<[f32; 2]>,
    masks: Vec<[f32; 4]>,
}
#[derive(Resource)]
struct Source {
    definitions: BTreeMap<String, Definition>,
    draws: BTreeMap<String, Draw>,
    textures: BTreeMap<String, Vec<u8>>,
}

fn read_source(root: &Path) -> Result<Source, Box<dyn std::error::Error>> {
    read_package(root, "tracksplit.pssg")
}

fn read_package(root: &Path, package: &str) -> Result<Source, Box<dyn std::error::Error>> {
    let bytes = super::read(&root.join("tracks/europe/donington").join(package))?;
    let tree = pssg::read_tree(&bytes)?;
    let bindings = material::read_texture_bindings(&tree)?;
    let mut definitions = BTreeMap::new();
    for i in 0..tree.nodes.len() {
        if tree.name(i) != b"SHADERINSTANCE" {
            continue;
        }
        let group_ref = tree.string(i, b"shaderGroup")?;
        let skidded = group_ref == b"#track_surface_simple.fx";
        if !skidded && group_ref != b"#track_infield.fx" {
            continue;
        }
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
        let second = texture_name(b"TDiffuseAlphaMap2").ok_or("surface second diffuse absent")?;
        let macro_map = texture_name(b"TMacroColour").ok_or("surface macro absent")?;
        let skids = texture_name(b"TSkidsMap");
        let definition = Definition {
            textures: [
                first,
                second,
                macro_map.clone(),
                skids.clone().unwrap_or(macro_map),
            ],
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
                1.,
                0.,
            ),
            colours: if skidded {
                [colour(b"SkidColour")?, colour(b"SkidColour2")?]
            } else {
                [Vec4::ZERO; 2]
            },
        };
        definitions.insert(String::from_utf8(id.to_vec())?, definition);
    }
    let sources = mesh::read_meshes(&tree)?;
    let mut draws = BTreeMap::new();
    for draw in scene::read_static_draws(&tree, &sources)? {
        let material = String::from_utf8(draw.material.to_vec())?;
        if !definitions.contains_key(&material) {
            continue;
        }
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
                },
            )
            .is_some()
        {
            return Err("duplicate surface draw name".into());
        }
    }
    let mut textures = BTreeMap::new();
    for t in texture::read_textures(&tree)? {
        let name = String::from_utf8(t.id.to_vec())?;
        if definitions.values().any(|d| d.textures.contains(&name)) {
            textures.insert(name, t.validated_dds().0);
        }
    }
    for d in definitions.values() {
        if d.textures.iter().any(|name| !textures.contains_key(name)) {
            return Err("surface texture unresolved".into());
        }
    }
    println!(
        "track_surface prepared materials={} draws={} textures={} lighting=authored macro_ao_scale=1",
        definitions.len(),
        draws.len(),
        textures.len()
    );
    Ok(Source {
        definitions,
        draws,
        textures,
    })
}

pub fn install(app: &mut App, root: &Path, land: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut source = read_source(root)?;
    if land {
        let land = read_package(root, "land.pssg")?;
        for (name, mut definition) in land.definitions {
            for texture in &mut definition.textures {
                *texture = format!("land/{texture}");
            }
            source
                .definitions
                .insert(format!("land/{name}"), definition);
        }
        for (name, mut draw) in land.draws {
            draw.material = format!("land/{}", draw.material);
            source.draws.insert(format!("land/{name}"), draw);
        }
        for (name, texture) in land.textures {
            source.textures.insert(format!("land/{name}"), texture);
        }
    }
    app.world_mut().resource_mut::<Assets<Shader>>().insert(
        SHADER.id(),
        Shader::from_wgsl(
            include_str!("track_surface.wgsl"),
            "grid_track_surface.wgsl",
        ),
    )?;
    app.insert_resource(source)
        .add_plugins(MaterialPlugin::<SurfaceMaterial>::default())
        .add_systems(PostStartup, attach);
    Ok(())
}

// uint_color_argb is stored A,R,G,B. It is a linear control, not sRGB albedo.
fn mask([a, r, g, b]: [u8; 4]) -> [f32; 4] {
    let mut controls = [r, g, b, a].map(|v| f32::from(v) / 255.);
    controls[1] = controls[1].min(0.5);
    controls
}

#[allow(clippy::too_many_arguments)]
fn attach(
    mut commands: Commands,
    source: Res<Source>,
    settings: Res<super::graphics::Settings>,
    query: Query<(
        Entity,
        &Name,
        &SourcePart,
        &Mesh3d,
        &MeshMaterial3d<StandardMaterial>,
    )>,
    mut meshes: ResMut<Assets<Mesh>>,
    standard: Res<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<SurfaceMaterial>>,
) {
    let textures: BTreeMap<_, _> = source
        .textures
        .iter()
        .map(|(name, dds)| {
            let image = Image::from_buffer(
                dds,
                ImageType::Extension("dds"),
                CompressedImageFormats::BC,
                true,
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
            let image = Image::from_buffer(
                &source.textures[name],
                ImageType::Extension("dds"),
                CompressedImageFormats::BC,
                false,
                settings.sampler(true),
                RenderAssetUsages::default(),
            )
            .expect("validated linear skid DDS");
            (name.clone(), images.add(image))
        })
        .collect();
    let mut handles = BTreeMap::new();
    let mut attached = 0;
    for (entity, name, identity, mesh, base) in &query {
        let Some(draw) = source.draws.get(name.as_str()) else {
            continue;
        };
        if draw.material != identity.0 {
            continue;
        }
        let mut target = meshes.get_mut(&mesh.0).expect("surface mesh ready");
        assert_eq!(target.count_vertices(), draw.uv.len());
        target.insert_attribute(Mesh::ATTRIBUTE_UV_1, draw.uv.clone());
        target.insert_attribute(Mesh::ATTRIBUTE_COLOR, draw.masks.clone());
        let handle = handles
            .entry(draw.material.clone())
            .or_insert_with(|| {
                let d = &source.definitions[&draw.material];
                let mut base = standard
                    .get(&base.0)
                    .expect("source base material ready")
                    .clone();
                // The extension composes albedo explicitly; never tint by mask colors.
                base.base_color_texture = None;
                materials.add(ExtendedMaterial {
                    base,
                    extension: Surface {
                        first: textures[&d.textures[0]].clone(),
                        second: textures[&d.textures[1]].clone(),
                        macro_map: textures[&d.textures[2]].clone(),
                        skids: skid_textures[&d.textures[3]].clone(),
                        scales: d.scales,
                        controls: d.controls,
                        skid_colour: d.colours[0],
                        skid_colour2: d.colours[1],
                    },
                })
            })
            .clone();
        commands
            .entity(entity)
            .remove::<MeshMaterial3d<StandardMaterial>>()
            .insert(MeshMaterial3d(handle));
        attached += 1;
    }
    println!(
        "track_surface attached draws={attached} materials={} uv1=true masks=linear-ARGB-to-RGBA",
        handles.len()
    );
    assert_eq!(
        attached,
        source.draws.len(),
        "surface draw attachment incomplete"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires owner's purchased GRID installation"]
    fn purchased_surfaces_resolve_both_uv_sets_and_all_layers() {
        let source = read_source(Path::new(
            r"C:\Program Files (x86)\Steam\steamapps\common\Grid",
        ))
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
    fn mask_channels_are_linear_controls() {
        assert_eq!(mask([255, 0, 255, 128]), [0., 0.5, 128. / 255., 1.]);
        // Clamping after interpolation differs: midpoint of 0 and 255 must be
        // 0.25, not 0.5. The source clamps in the vertex program.
        let midpoint = (mask([255, 0, 0, 0])[1] + mask([255, 0, 255, 0])[1]) / 2.;
        assert_eq!(midpoint, 0.25);
    }
}
