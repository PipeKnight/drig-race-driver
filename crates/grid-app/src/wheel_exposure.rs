//! Experimental wheel-local scene-colour reconstruction. Main view only.
//! A snug swept hull reads the opaque scene; original wheel meshes stay intact.
use bevy::{
    asset::uuid_handle,
    camera::visibility::RenderLayers,
    light::NotShadowCaster,
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};

const SHADER: Handle<Shader> = uuid_handle!("87b97081-7ef3-469a-a9dd-fdaee93b1749");
type HullMaterial = ExtendedMaterial<StandardMaterial, Exposure>;
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct Exposure {
    #[uniform(100)]
    world: Mat4,
    #[uniform(101)]
    inverse: Mat4,
    #[uniform(102)]
    controls: Vec4,
}
impl MaterialExtension for Exposure {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn specialize(
        _: &bevy::pbr::MaterialExtensionPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _: &bevy::mesh::MeshVertexBufferLayoutRef,
        _: bevy::pbr::MaterialExtensionKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        // Opaque alpha mode selects Bevy's transmissive phase and scene copy;
        // hull depth must still leave the actual scene depth untouched.
        if let Some(depth) = descriptor.depth_stencil.as_mut() {
            depth.depth_write_enabled = Some(false);
        }
        Ok(())
    }
}
#[derive(Component)]
struct Hull {
    source: Entity,
    wheel: usize,
    bind: Mat4,
}

pub fn install(app: &mut App) {
    app.world_mut()
        .resource_mut::<Assets<Shader>>()
        .insert(
            SHADER.id(),
            Shader::from_wgsl(
                include_str!("wheel_exposure.wgsl"),
                "grid_wheel_exposure.wgsl",
            ),
        )
        .expect("UUID shader asset");
    app.add_plugins(MaterialPlugin::<HullMaterial>::default())
        .add_systems(PostStartup, attach)
        .add_systems(
            PostUpdate,
            update.before(bevy::transform::TransformSystems::Propagate),
        );
}

fn attach(
    mut commands: Commands,
    camera: Query<Entity, With<super::ViewerCamera>>,
    source: Query<(
        Entity,
        &super::sidewall_filter::Sidewall,
        &Mesh3d,
        &Transform,
    )>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<HullMaterial>>,
) {
    // Layer1 is main-car-only when scene probes are enabled; capture views use0.
    // The ordinary main camera otherwise defaults to0 and would hide the hull.
    for camera in &camera {
        commands
            .entity(camera)
            .insert(RenderLayers::from_layers(&[0, 1]));
    }
    let mut staged = Vec::new();
    for (entity, sidewall, mesh, transform) in &source {
        if sidewall.car != 0 {
            continue;
        }
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) = meshes
            .get(&mesh.0)
            .and_then(|m| m.attribute(Mesh::ATTRIBUTE_POSITION))
        else {
            continue;
        };
        let lo = positions.iter().fold(Vec3::splat(f32::INFINITY), |a, p| {
            a.min(Vec3::from_array(*p))
        });
        let hi = positions
            .iter()
            .fold(Vec3::splat(f32::NEG_INFINITY), |a, p| {
                a.max(Vec3::from_array(*p))
            });
        let center = (lo + hi) * 0.5;
        let radius = positions
            .iter()
            .map(|p| Vec2::new(p[1] - center.y, p[2] - center.z).length())
            .fold(0_f32, f32::max)
            + 0.0005;
        let width = hi.x - lo.x + 0.001;
        if !(0.1..0.6).contains(&radius) || !(0.05..0.5).contains(&width) {
            continue;
        }
        let bind =
            Mat4::from_translation(center) * Mat4::from_rotation_z(std::f32::consts::FRAC_PI_2);
        staged.push((
            entity,
            sidewall.wheel,
            bind,
            transform.to_matrix() * bind,
            radius,
            width,
        ));
    }
    let count = staged.len();
    for (source, wheel, bind, world, radius, width) in staged {
        let material = materials.add(HullMaterial {
            base: StandardMaterial {
                alpha_mode: AlphaMode::Opaque,
                specular_transmission: 1.,
                unlit: true,
                cull_mode: Some(bevy::render::render_resource::Face::Back),
                ..default()
            },
            extension: Exposure {
                world,
                inverse: world.inverse(),
                controls: Vec4::ZERO,
            },
        });
        commands.spawn((
            Mesh3d(meshes.add(Cylinder::new(radius, width).mesh().resolution(128))),
            MeshMaterial3d(material),
            Transform::from_matrix(world),
            Hull {
                source,
                wheel,
                bind,
            },
            RenderLayers::layer(1),
            NotShadowCaster,
        ));
    }
    println!(
        "wheel_exposure hulls={count} scope=player-main-view approximation=opaque-scene-radial depth_write=false"
    );
}

fn update(
    driving: Res<super::Driving>,
    graphics: Res<super::graphics::Settings>,
    time: Res<Time<Real>>,
    sources: Query<(&Transform, &Visibility), Without<Hull>>,
    mut hulls: Query<(
        &Hull,
        &mut Transform,
        &mut Visibility,
        &MeshMaterial3d<HullMaterial>,
    )>,
    mut materials: ResMut<Assets<HullMaterial>>,
) {
    for (hull, mut transform, mut visibility, handle) in &mut hulls {
        let Ok((source, source_visibility)) = sources.get(hull.source) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        *visibility = *source_visibility;
        let world = source.to_matrix() * hull.bind;
        *transform = Transform::from_matrix(world);
        let (angle, gaussian) = if driving.paused || driving.failure.is_some() {
            (0., false)
        } else {
            super::wheel_presentation::exposure(
                super::wheel_presentation::angular_speed(&driving.runtime, hull.wheel),
                graphics.wheel_geometry_shutter_seconds,
                time.delta_secs(),
                graphics.adaptive_wheel_exposure,
                graphics.wheel_filter_floor,
            )
        };
        if let Some(mut material) = materials.get_mut(&handle.0) {
            material.extension.world = world;
            material.extension.inverse = world.inverse();
            material.extension.controls.x = angle;
            material.extension.controls.y = f32::from(gaussian);
        }
    }
}
