//! Source-sidewall angular exposure filter; not a rim-geometry blur.
use bevy::{
    asset::uuid_handle,
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};
const SHADER: Handle<Shader> = uuid_handle!("41b1a37f-69f0-40c4-a79d-b70a8a2411a0");
type SidewallMaterial = ExtendedMaterial<StandardMaterial, Filter>;
#[derive(Component)]
pub struct Sidewall {
    pub car: usize,
    pub wheel: usize,
}
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct Filter {
    #[texture(100)]
    #[sampler(101)]
    texture: Handle<Image>,
    #[uniform(102)]
    exposure: Vec4,
}
impl MaterialExtension for Filter {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }
    fn deferred_fragment_shader() -> ShaderRef {
        SHADER.into()
    }
}
pub fn install(app: &mut App) {
    app.world_mut()
        .resource_mut::<Assets<Shader>>()
        .insert(
            SHADER.id(),
            Shader::from_wgsl(
                include_str!("sidewall_filter.wgsl"),
                "grid_sidewall_filter.wgsl",
            ),
        )
        .expect("UUID shader asset");
    app.add_plugins(MaterialPlugin::<SidewallMaterial>::default())
        .add_systems(PostStartup, attach)
        .add_systems(Update, update.after(super::drive_present));
}
fn attach(
    mut commands: Commands,
    parts: Query<(Entity, &Sidewall, &MeshMaterial3d<StandardMaterial>)>,
    standard: Res<Assets<StandardMaterial>>,
    mut filtered: ResMut<Assets<SidewallMaterial>>,
) {
    let mut count = 0;
    for (entity, _, handle) in &parts {
        let Some(base) = standard.get(&handle.0) else {
            continue;
        };
        let Some(texture) = base.base_color_texture.clone() else {
            continue;
        };
        let material = filtered.add(SidewallMaterial {
            base: base.clone(),
            extension: Filter {
                texture,
                exposure: Vec4::ZERO,
            },
        });
        commands
            .entity(entity)
            .remove::<MeshMaterial3d<StandardMaterial>>()
            .insert(MeshMaterial3d(material));
        count += 1;
    }
    println!(
        "sidewall_filter attached={count} samples=64 scope=Lacetti-sidewall-only shutter=fixed-seconds"
    );
}
fn update(
    driving: Option<Res<super::Driving>>,
    graphics: Res<super::graphics::Settings>,
    parts: Query<(&Sidewall, &MeshMaterial3d<SidewallMaterial>)>,
    mut materials: ResMut<Assets<SidewallMaterial>>,
    mut final_report: Local<Option<(u64, u64)>>,
) {
    let Some(driving) = driving else {
        return;
    };
    if driving.scripted_ticks == Some(driving.runtime.ticks)
        && *final_report != Some((driving.segment, driving.runtime.ticks))
    {
        *final_report = Some((driving.segment, driving.runtime.ticks));
        let speeds: [f32; 4] =
            std::array::from_fn(|i| super::wheel_presentation::angular_speed(&driving.runtime, i));
        println!(
            "sidewall_filter_final tick={} shutter_seconds={} signed_spin_rad_s={speeds:?} exposure_rad={:?}",
            driving.runtime.ticks,
            graphics.wheel_shutter_seconds,
            speeds.map(|s| (s.abs() * graphics.wheel_shutter_seconds).min(std::f32::consts::TAU))
        );
    }
    for (part, handle) in &parts {
        let runtime = if part.car == 0 {
            Some(&driving.runtime)
        } else {
            driving
                .opponents
                .as_ref()
                .and_then(|o| o.multi.cars.get(part.car))
        };
        let angle = runtime
            .filter(|_| !driving.paused && driving.failure.is_none())
            .map_or(0., |r| {
                super::wheel_presentation::angular_speed(r, part.wheel).abs()
                    * graphics.wheel_shutter_seconds
            })
            .min(std::f32::consts::TAU);
        if let Some(mut material) = materials.get_mut(&handle.0)
            && material.extension.exposure.x != angle
        {
            material.extension.exposure.x = angle;
        }
    }
}
