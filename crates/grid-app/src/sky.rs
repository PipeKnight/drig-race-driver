//! Source Donington sky geometry/UVs with an independently authored background shader.
use bevy::{
    asset::{RenderAssetUsages, uuid_handle},
    camera::visibility::NoFrustumCulling,
    image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    light::{NotShadowCaster, NotShadowReceiver},
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
    transform::TransformSystems,
};
use grid_formats::{material, mesh, pssg, scene, texture};
use std::path::Path;

const SHADER: Handle<Shader> = uuid_handle!("90c5b5ee-1eb2-4cec-8499-b83994cc6df2");

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct SkyMaterial {
    #[texture(0)]
    #[sampler(1)]
    texture: Handle<Image>,
    #[uniform(2)]
    tone: Vec4,
}

impl Material for SkyMaterial {
    fn vertex_shader() -> ShaderRef {
        SHADER.into()
    }
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
    // Draw first among transparencies so glass can still appear in front of sky.
    fn depth_bias(&self) -> f32 {
        1.0e8
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
}

#[derive(Component)]
struct SkyDome;

#[derive(Resource)]
struct SourceSky {
    mesh: Mesh,
    dds: Vec<u8>,
    environment: Option<Image>,
    tone: Vec4,
}

pub fn install(
    app: &mut App,
    root: &Path,
    source_ibl: bool,
    source_tone: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let tone = if source_tone {
        let bytes = super::read(&root.join("tracks/europe/donington/lighting.xml"))?;
        let saved = grid_formats::track_lighting::read_sky(&bytes)?;
        Vec4::new(saved.scale, saved.exponent, 0., 0.)
    } else {
        Vec4::new(1., 1., 0., 0.)
    };
    println!(
        "source_sky_tone scale={} exponent={} source_controls={} space=linear-adaptation original-sampler-space=unverified",
        tone.x, tone.y, source_tone
    );
    let bytes = super::read(&root.join("tracks/europe/donington/sky.pssg"))?;
    let tree = pssg::read_tree(&bytes)?;
    let sources = mesh::read_meshes(&tree)?;
    let draws = scene::read_static_draws(&tree, &sources)?;
    if draws.len() != 1 {
        return Err("expected one Donington sky draw".into());
    }
    let draw = &draws[0];
    let source = &sources[draw.source];
    if source.texcoords.len() != source.positions.len() {
        return Err("sky UV count mismatch".into());
    }
    let bindings = material::read_texture_bindings(&tree)?;
    let binding = bindings
        .iter()
        .find(|b| b.shader == draw.material && b.sampler == b"TDiffuseAlphaMap")
        .ok_or("sky diffuse binding missing")?;
    let textures = texture::read_textures(&tree)?;
    let tex = textures
        .iter()
        .find(|t| t.id == binding.texture)
        .ok_or("sky texture missing")?;
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    let positions: Vec<_> = source.positions[draw.vertices.clone()]
        .iter()
        .map(|p| scene::transform_point(&draw.transform, *p))
        .collect();
    let environment = source_ibl
        .then(|| {
            super::sky_environment::bake(
                &positions,
                &source.texcoords[draw.vertices.clone()],
                &draw.indices,
                tex,
                [tone.x, tone.y],
            )
        })
        .transpose()?;
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_UV_0,
        source.texcoords[draw.vertices.clone()].to_vec(),
    );
    mesh.insert_indices(Indices::U16(draw.indices.clone()));
    println!(
        "source_sky vertices={} triangles={} texture={}x{}",
        draw.vertices.len(),
        draw.indices.len() / 3,
        tex.width,
        tex.height
    );
    app.world_mut().resource_mut::<Assets<Shader>>().insert(
        SHADER.id(),
        Shader::from_wgsl(include_str!("sky.wgsl"), "grid_source_sky.wgsl"),
    )?;
    app.insert_resource(SourceSky {
        mesh,
        dds: tex.base_dds(),
        environment,
        tone,
    })
    .add_plugins(MaterialPlugin::<SkyMaterial>::default())
    .add_systems(Startup, setup)
    .add_systems(PostStartup, setup_environment)
    .add_systems(
        PostUpdate,
        follow_camera.before(TransformSystems::Propagate),
    );
    if source_ibl {
        use bevy::{
            pbr::generate,
            render::{Render, RenderApp, RenderSystems},
        };
        app.sub_app_mut(RenderApp).add_systems(
            Render,
            environment_dispatch_ready
                .after(generate::downsampling_system)
                .after(generate::filtering_system)
                .before(RenderSystems::Render),
        );
    }
    Ok(())
}

fn environment_dispatch_ready(
    mut reported: Local<bool>,
    pipelines: Option<Res<bevy::pbr::generate::GeneratorPipelines>>,
    cache: Res<bevy::render::render_resource::PipelineCache>,
    maps: Query<(
        &bevy::pbr::generate::GeneratorBindGroups,
        &bevy::pbr::generate::RenderEnvironmentMap,
    )>,
) {
    if *reported {
        return;
    }
    let Some(pipelines) = pipelines else { return };
    if [
        pipelines.downsample_first,
        pipelines.downsample_second,
        pipelines.copy,
        pipelines.radiance,
        pipelines.irradiance,
    ]
    .iter()
    .any(|&id| cache.get_compute_pipeline(id).is_none())
    {
        return;
    }
    for (_, map) in &maps {
        println!(
            "source_sky_ibl gpu_dispatch_ready=true source_size={} diffuse_size={} specular_mips={} scope=pipelines-and-bind-groups-not-readback",
            map.environment_map.texture_descriptor.size.width,
            map.diffuse_map.texture_descriptor.size.width,
            map.specular_map.texture_descriptor.mip_level_count
        );
        *reported = true;
    }
}

fn setup_environment(
    mut commands: Commands,
    source: Res<SourceSky>,
    camera: Query<Entity, With<super::ViewerCamera>>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(environment) = &source.environment else {
        return;
    };
    let Ok(camera) = camera.single() else { return };
    commands
        .entity(camera)
        .remove::<bevy::light::AtmosphereEnvironmentMapLight>()
        .insert(bevy::light::GeneratedEnvironmentMapLight {
            environment_map: images.add(environment.clone()),
            intensity: 8000.,
            ..default()
        });
    println!(
        "source_sky_ibl requested=true intensity_cd_m2=8000 filtering=Bevy-runtime source=sky-mesh-uv lower=authored-ground"
    );
}

fn setup(
    mut commands: Commands,
    source: Res<SourceSky>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<SkyMaterial>>,
) {
    let image = super::texture_upload::image(
        &source.dds,
        ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: ImageAddressMode::Repeat,
            ..ImageSamplerDescriptor::linear()
        }),
        RenderAssetUsages::default(),
    )
    .expect("validated source sky DDS");
    commands.spawn((
        SkyDome,
        Name::new("Donington source sky"),
        NoFrustumCulling,
        NotShadowCaster,
        NotShadowReceiver,
        Mesh3d(meshes.add(source.mesh.clone())),
        MeshMaterial3d(materials.add(SkyMaterial {
            texture: images.add(image),
            tone: source.tone,
        })),
        Transform::default(),
    ));
}

fn follow_camera(
    camera: Query<&Transform, With<super::ViewerCamera>>,
    mut skies: Query<&mut Transform, (With<SkyDome>, Without<super::ViewerCamera>)>,
) {
    let Ok(camera) = camera.single() else {
        return;
    };
    for mut sky in &mut skies {
        sky.translation = camera.translation;
    }
}
