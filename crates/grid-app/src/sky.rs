//! The route's source sky dome (`sky.pssg`, docs/SKY.md) with an independently
//! authored background shader: `sky.fx` on every track, Le Mans' six-slot
//! `nightsky.fx` blended by the time of day.
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
use grid_formats::pssg;
use grid_tools::{
    route::Route,
    time_of_day::{self, Mode, Timeline},
};
use std::path::Path;

const SHADER: Handle<Shader> = uuid_handle!("90c5b5ee-1eb2-4cec-8499-b83994cc6df2");

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct SkyMaterial {
    #[texture(0)]
    #[sampler(1)]
    first: Handle<Image>,
    #[texture(3)]
    #[sampler(4)]
    second: Handle<Image>,
    /// skyScaleAndInvGamma.xy, then the nightsky blend timeOfDay.z.
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
    // Bevy sorts view-space Z plus depth_bias ascending: negative draws first.
    // Keep the background ahead of transparent geometry, including depth-writing flashes.
    fn depth_bias(&self) -> f32 {
        -1.0e8
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

/// Sky state at one time of day: slot pair, blend and tone.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Look {
    first: usize,
    second: usize,
    blend: f32,
    scale: f32,
    exponent: f32,
}

#[derive(Resource)]
struct SourceSky {
    mesh: Mesh,
    /// Base-level DDS per slot (one, or six for nightsky.fx).
    dds: Vec<Vec<u8>>,
    environment: Option<Image>,
    /// Le Mans: the daynight timeline drives slots and tone.
    timeline: Option<Timeline>,
    /// lighting.xml tone; `None` keeps the raw texture (`--sky-tone raw`).
    saved: Option<[f32; 2]>,
    look: Look,
}

impl SourceSky {
    /// The look at day fraction `t` [V 0x95FCE0, 0x9ACE90]: slots from
    /// `skyTime`, tone from `skyScale`/`skygamma` keys over lighting.xml.
    fn look_at(&self, t: f64) -> Look {
        let (first, second, blend) = match (&self.timeline, self.dds.len()) {
            (Some(tl), 6) => tl.value("skyTime", t).map_or((0, 0, 0.), time_of_day::sky_textures),
            _ => (0, 0, 0.),
        };
        let [mut scale, mut exponent] = self.saved.unwrap_or([1., 1.]);
        if let (Some(tl), Some(_)) = (&self.timeline, self.saved) {
            scale = tl.value("skyScale", t).unwrap_or(scale);
            exponent = tl.value("skygamma", t).unwrap_or(exponent);
        }
        Look {
            first,
            second,
            blend,
            scale: scale.clamp(0., 16.),
            exponent: exponent.clamp(0.01, 16.),
        }
    }
}

/// The route's own `sky.pssg` (the Haruna night routes ship one), else the
/// track's [I: same order as lighting.xml, grid_tools::environment].
pub fn sky_file(root: &Path, route: &Route) -> String {
    let own = route.file("sky.pssg");
    if root.join(&own).is_file() {
        own
    } else {
        route.track_file("sky.pssg")
    }
}

pub fn install(
    app: &mut App,
    root: &Path,
    route: &Route,
    source_ibl: bool,
    source_tone: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let environment = grid_tools::environment::Environment::for_route(root, route)?;
    let saved = if source_tone {
        let l = &environment.lighting;
        let scale = l.scalar("skyscale").ok_or("lighting skyscale missing")?;
        let exponent = l.scalar("skygamma").ok_or("lighting skygamma missing")?;
        if !(0.0..=16.).contains(&scale) || !(0.01..=16.).contains(&exponent) {
            return Err("sky controls outside supported range".into());
        }
        Some([scale, exponent])
    } else {
        None
    };
    let timeline = match environment.mode() {
        Mode::Timeline => Timeline::load(root, route)?,
        _ => None,
    };
    let file = sky_file(root, route);
    let bytes = super::read(&root.join(&file))?;
    let tree = pssg::read_tree(&bytes)?;
    let sky = grid_formats::sky::read(&tree).map_err(|e| format!("{file}: {e}"))?;
    let dds: Vec<_> = (0..sky.slots.len()).map(|i| sky.texture(i).base_dds()).collect();
    let mut source = SourceSky {
        mesh: Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default()),
        dds,
        environment: None,
        timeline,
        saved,
        look: Look {
            first: 0,
            second: 0,
            blend: 0.,
            scale: 1.,
            exponent: 1.,
        },
    };
    // Diagnostic: GRID_SKY_HOUR (0..24) sets the start look of a static
    // render; a Le Mans drive follows its own clock.
    let start = std::env::var("GRID_SKY_HOUR")
        .ok()
        .and_then(|h| h.parse::<f64>().ok())
        .filter(|h| (0.0..24.0).contains(h))
        .map_or_else(|| time_of_day::day_fraction(0., None), |h| h / 24.);
    source.look = source.look_at(start);
    let look = source.look;
    println!(
        "source_sky file={file} slots={} lighting={} tone_source={source_tone} timeline={} start_slots={},{} blend={:.3} scale={} exponent={} space=linear-adaptation original-sampler-space=unverified",
        sky.slots.len(),
        environment.lighting_file,
        source.timeline.is_some(),
        look.first,
        look.second,
        look.blend,
        look.scale,
        look.exponent,
    );
    // The bake takes the start look's first slot; it is not re-baked as the
    // Le Mans day turns.
    source.environment = source_ibl
        .then(|| {
            super::sky_environment::bake(
                &sky.positions,
                &sky.texcoords,
                &sky.indices,
                sky.texture(look.first),
                [look.scale, look.exponent],
            )
        })
        .transpose()?;
    source.mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, sky.positions.clone());
    source.mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, sky.texcoords.clone());
    source.mesh.insert_indices(Indices::U16(sky.indices.clone()));
    let tex = sky.texture(0);
    println!(
        "source_sky vertices={} triangles={} texture={}x{}",
        sky.positions.len(),
        sky.indices.len() / 3,
        tex.width,
        tex.height
    );
    app.world_mut().resource_mut::<Assets<Shader>>().insert(
        SHADER.id(),
        Shader::from_wgsl(include_str!("sky.wgsl"), "grid_source_sky.wgsl"),
    )?;
    let animated = source.timeline.is_some();
    app.insert_resource(source)
        .add_plugins(MaterialPlugin::<SkyMaterial>::default())
        .add_systems(Startup, setup)
        .add_systems(PostStartup, setup_environment)
        .add_systems(
            PostUpdate,
            follow_camera.before(TransformSystems::Propagate),
        );
    if animated {
        app.add_systems(Update, follow_time_of_day);
    }
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

#[derive(Resource)]
struct SlotImages(Vec<Handle<Image>>);

fn setup(
    mut commands: Commands,
    source: Res<SourceSky>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<SkyMaterial>>,
) {
    let slots: Vec<_> = source
        .dds
        .iter()
        .map(|dds| {
            let image = super::texture_upload::image(
                dds,
                ImageSampler::Descriptor(ImageSamplerDescriptor {
                    address_mode_u: ImageAddressMode::Repeat,
                    ..ImageSamplerDescriptor::linear()
                }),
                RenderAssetUsages::default(),
            )
            .expect("validated source sky DDS");
            images.add(image)
        })
        .collect();
    let look = source.look;
    commands.spawn((
        SkyDome,
        Name::new("Source sky"),
        NoFrustumCulling,
        NotShadowCaster,
        NotShadowReceiver,
        Mesh3d(meshes.add(source.mesh.clone())),
        MeshMaterial3d(materials.add(SkyMaterial {
            first: slots[look.first].clone(),
            second: slots[look.second].clone(),
            tone: Vec4::new(look.scale, look.exponent, look.blend, 0.),
        })),
        Transform::default(),
    ));
    commands.insert_resource(SlotImages(slots));
}

/// Le Mans: follow the event's clock (15:00 outside a time-limited race).
fn follow_time_of_day(
    event: Option<Res<super::le_mans_event::LeMansEvent>>,
    slots: Option<Res<SlotImages>>,
    mut source: ResMut<SourceSky>,
    domes: Query<&MeshMaterial3d<SkyMaterial>, With<SkyDome>>,
    mut materials: ResMut<Assets<SkyMaterial>>,
) {
    let (Some(event), Some(slots)) = (event, slots) else {
        return;
    };
    let look = source.look_at(event.time_of_day);
    if look == source.look {
        return;
    }
    source.look = look;
    for handle in &domes {
        if let Some(mut m) = materials.get_mut(&handle.0) {
            m.first = slots.0[look.first].clone();
            m.second = slots.0[look.second].clone();
            m.tone = Vec4::new(look.scale, look.exponent, look.blend, 0.);
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sky_precedes_near_transparencies_in_bevy_sort_keys() {
        use bevy::{
            core_pipeline::core_3d::{Transparent3d, TransparentSortingInfo3d},
            math::{Affine3A, FloatOrd},
            render::render_phase::{SortedPhaseItem, ViewRangefinder3d},
        };
        let material = SkyMaterial {
            first: Handle::default(),
            second: Handle::default(),
            tone: Vec4::ONE,
        };
        assert_eq!(material.alpha_mode(), AlphaMode::Blend);
        assert!(!SkyMaterial::enable_prepass());
        assert!(!SkyMaterial::enable_shadows());
        let camera = Vec3::new(120., 40., -75.);
        let rangefinder =
            ViewRangefinder3d::from_world_from_view(&Affine3A::from_translation(camera));
        // Use the actual Transparent3d associated sort-key type and Bevy's
        // distance calculation; reversing the sky bias must break this ordering.
        let key = |center, bias| -> <Transparent3d as SortedPhaseItem>::SortKey {
            FloatOrd(
                TransparentSortingInfo3d::Sorted {
                    mesh_center: center,
                    depth_bias: bias,
                }
                .sort_distance(&rangefinder),
            )
        };
        let mut items = [
            (key(camera + Vec3::new(0., 0., -2.), 0.), "near-glass"),
            (key(camera, material.depth_bias()), "sky"),
            (key(camera + Vec3::new(0., 0., -10_000.), 0.), "far-glass"),
            (
                key(camera + Vec3::new(1., 1., -12.), 0.),
                "depth-writing-flash",
            ),
        ];
        items.sort_by_key(|(key, _)| *key);
        assert_eq!(
            items.map(|(_, name)| name),
            ["sky", "far-glass", "depth-writing-flash", "near-glass"]
        );
    }

    const ROOT: &str = r"C:\Program Files (x86)\Steam\steamapps\common\Grid";

    #[test]
    #[ignore = "requires owner's purchased GRID installation"]
    fn purchased_skies_load_on_every_track_and_le_mans_turns_dark_at_night() {
        let root = Path::new(ROOT);
        for track in [
            "europe/donington",
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
            let file = sky_file(root, &route);
            let bytes = std::fs::read(root.join(&file)).unwrap();
            let tree = pssg::read_tree(&bytes).unwrap();
            let sky = grid_formats::sky::read(&tree).unwrap();
            let slots = if track == "europe/le_mans" { 6 } else { 1 };
            assert_eq!(sky.slots.len(), slots, "{track}");
            let names: Vec<_> = sky.slots.iter().map(|s| String::from_utf8_lossy(s)).collect();
            println!("sky {track} {file} slots={names:?}");
        }
        let night = Route::new("tracks/japan/haruna/route_1").unwrap();
        assert_eq!(sky_file(root, &night), "tracks/japan/haruna/route_1/sky.pssg");
        let route = Route::new("tracks/europe/le_mans/route_0").unwrap();
        let source = SourceSky {
            mesh: Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default()),
            dds: vec![vec![]; 6],
            environment: None,
            timeline: Timeline::load(root, &route).unwrap(),
            saved: Some([1., 0.735]),
            look: Look {
                first: 0,
                second: 0,
                blend: 0.,
                scale: 1.,
                exponent: 1.,
            },
        };
        let day = source.look_at(15. / 24.);
        let night = source.look_at(1. / 24.);
        println!("le_mans 15:00 {day:?} 01:00 {night:?}");
        // skyTime 1/3 at 15:00: f = 2.0, slot 2 (sky_lemans_2) [V log].
        assert_eq!((day.first, day.second), (2, 3));
        assert_eq!(night.first, 5);
        assert!(night.scale < day.scale);
    }
}
