//! The original's light-shaft cards (`god_rays.fx`, `god_rays_single_sided.fx`;
//! docs/LIGHT_SHAFTS.md): `object_light_shafts` programs' arithmetic on the
//! saved constants, the card texture's alpha/green/blue, lighting.xml's
//! `lightShaft_*` and `backlight_suncolour`. AUTHORED: the fade distance is
//! the pixel's (the original's is the card origin's), the dust has no
//! per-card phase, timer is seconds, and the colour is used as linear HDR.
use bevy::{
    asset::uuid_handle,
    mesh::MeshVertexBufferLayoutRef,
    pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline},
    prelude::*,
    render::render_resource::{AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError},
    shader::ShaderRef,
};
use grid_formats::{material, pssg};
use grid_tools::route::Route;
use std::{collections::BTreeMap, path::Path};

const SHADER: Handle<Shader> = uuid_handle!("8c41d2e7-5b93-4f0a-a6d8-2e7b19c4f650");
type ShaftMaterial = ExtendedMaterial<StandardMaterial, Shaft>;

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct Shaft {
    // GodColour * trackLightShaftColour * backlightSunColour.
    #[uniform(100)]
    colour: Vec4,
    // GodStrength * strength, GodDensity * density, smoke_scale.
    #[uniform(101)]
    controls: Vec4,
    // DustSpeedX/Y, distanceStart/EndFadeOut.
    #[uniform(102)]
    motion: Vec4,
}
impl MaterialExtension for Shaft {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }
    fn specialize(
        _: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _: &MeshVertexBufferLayoutRef,
        _: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // Translucent cards: no depth write (the original's blend pass).
        if let Some(depth) = &mut descriptor.depth_stencil {
            depth.depth_write_enabled = Some(false);
        }
        Ok(())
    }
}

#[derive(Resource)]
struct Source {
    // Material name -> (extension, two-sided).
    materials: BTreeMap<String, (Shaft, bool)>,
}

pub fn install(app: &mut App, root: &Path, route: &Route) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = super::read(&root.join(route.track_file("objects.pssg")))?;
    let tree = pssg::read_tree(&bytes)?;
    let shaders: Vec<_> = material::read_constants(&tree)?
        .into_iter()
        .filter(|s| s.group.starts_with(b"god_rays"))
        .collect();
    if shaders.is_empty() {
        return Ok(());
    }
    let lighting = grid_tools::environment::Environment::for_route(root, route)?.lighting;
    let track_colour = Vec3::from(lighting.rgb("lightShaft_colour").unwrap_or([1.; 3]));
    let strength = lighting.scalar("lightShaft_strength").unwrap_or(1.);
    let density = lighting.scalar("lightShaft_density").unwrap_or(1.);
    let sun = Vec3::from(lighting.rgb("backlight_suncolour").unwrap_or([1.; 3]));
    let materials = shaders.iter()
        .map(|s| {
            let c = |key: &[u8], i: usize| s.values.get(key).and_then(|v| v.get(i)).copied();
            let god = Vec3::new(c(b"GodColour", 0).unwrap_or(1.), c(b"GodColour", 1).unwrap_or(1.), c(b"GodColour", 2).unwrap_or(1.));
            let shaft = Shaft {
                colour: (god * track_colour * sun).extend(1.),
                controls: Vec4::new(
                    c(b"GodStrength", 0).unwrap_or(1.) * strength,
                    c(b"GodDensity", 0).unwrap_or(1.) * density,
                    c(b"smoke_scale", 0).unwrap_or(1.),
                    0.,
                ),
                motion: Vec4::new(
                    c(b"DustSpeedX", 0).unwrap_or(0.),
                    c(b"DustSpeedY", 0).unwrap_or(0.),
                    c(b"distanceStartFadeOut", 0).unwrap_or(1e6),
                    c(b"distanceEndFadeOut", 0).unwrap_or(1e6 + 1.),
                ),
            };
            let name = format!("objects/{}", String::from_utf8_lossy(s.shader));
            (name, (shaft, s.group == b"god_rays.fx"))
        })
        .collect::<BTreeMap<_, _>>();
    println!(
        "light_shafts prepared materials={} lightShaft_colour={track_colour:?} strength={strength} density={density} sun={sun:?}",
        materials.len()
    );
    app.world_mut().resource_mut::<Assets<Shader>>().insert(
        SHADER.id(),
        Shader::from_wgsl(include_str!("light_shafts.wgsl"), "grid_light_shafts.wgsl"),
    )?;
    app.insert_resource(Source { materials })
        .add_plugins(MaterialPlugin::<ShaftMaterial>::default())
        .add_systems(PostStartup, attach);
    Ok(())
}

fn attach(
    mut commands: Commands,
    source: Res<Source>,
    query: Query<(Entity, &super::track_surface::SourcePart, &MeshMaterial3d<StandardMaterial>)>,
    standard: Res<Assets<StandardMaterial>>,
    mut materials: ResMut<Assets<ShaftMaterial>>,
) {
    let mut handles = BTreeMap::new();
    let mut attached = 0;
    for (entity, part, base) in &query {
        let Some((shaft, two_sided)) = source.materials.get(&part.0) else {
            continue;
        };
        let handle = handles.entry(part.0.clone()).or_insert_with(|| {
            let mut base = standard.get(&base.0).expect("light-shaft base material").clone();
            base.alpha_mode = AlphaMode::Blend;
            base.unlit = true;
            base.cull_mode = if *two_sided { None } else { base.cull_mode };
            base.double_sided = *two_sided;
            materials.add(ExtendedMaterial { base, extension: shaft.clone() })
        }).clone();
        commands.entity(entity)
            .remove::<MeshMaterial3d<StandardMaterial>>()
            .insert((MeshMaterial3d(handle), bevy::light::NotShadowCaster));
        attached += 1;
    }
    println!("light_shafts attached draws={attached} materials={}", handles.len());
}
