//! Source Type2 bloom, luminance adaptation and final merge for the race view.
use std::{io, path::Path};

use bevy::{
    asset::uuid_handle,
    core_pipeline::{Core3dSystems, FullscreenShader, schedule::Core3d, tonemapping::tonemapping},
    diagnostic::FrameCount,
    post_process::motion_blur::motion_blur,
    prelude::*,
    render::{
        RenderApp, RenderStartup,
        extract_component::{
            ComponentUniforms, DynamicUniformIndex, ExtractComponent, ExtractComponentPlugin,
            UniformComponentPlugin,
        },
        render_resource::{
            binding_types::{sampler, texture_2d, uniform_buffer},
            *,
        },
        renderer::{RenderContext, RenderDevice, ViewQuery},
        view::ViewTarget,
    },
};
use grid_formats::race_postprocess::{self, RaceEffect, RaceEffects};
use grid_tools::route::Route;

const SHADER: Handle<Shader> = uuid_handle!("7869d3a3-9cc4-40e0-938b-5f80a0886655");
const COLOUR_FORMAT: TextureFormat = TextureFormat::Rgba8Unorm;
const LUMINANCE_FORMAT: TextureFormat = TextureFormat::Rgba16Float;

pub fn load(root: &Path, route: &Route) -> Result<RaceEffects, Box<dyn std::error::Error>> {
    let global = std::fs::read(root.join("postprocess/effects.xml"))?;
    let mut selected = None;
    for path in [route.file("effects.xml"), route.track_file("effects.xml")] {
        match std::fs::read(root.join(&path)) {
            Ok(bytes) => {
                selected = Some((path, bytes));
                break;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    let render = std::fs::read(root.join("frontend/persistentdatarender.xml"))?;
    let common = std::fs::read(root.join("frontend/persistentdatacommon.xml"))?;
    let effects = race_postprocess::read(
        &global,
        selected.as_ref().map(|(_, bytes)| bytes.as_slice()),
        Some(race_postprocess::PauseSources {
            common: &common,
            render: &render,
            track_name: route.track_name(),
        }),
    )?;
    let source = selected
        .as_ref()
        .map_or("global", |(path, _)| path.as_str());
    println!(
        "graphics_grade chain=Type2 track={source} exterior={:?} interior={:?}",
        effects.exterior, effects.interior
    );
    Ok(effects)
}

#[derive(Component, Clone, Copy)]
pub struct TrackGrade {
    effects: RaceEffects,
    motion_blur: bool,
}

impl TrackGrade {
    pub fn new(effects: RaceEffects, motion_blur: bool) -> Self {
        Self {
            effects,
            motion_blur,
        }
    }
}

#[derive(Component, Clone, Copy, PartialEq, ExtractComponent, ShaderType)]
struct EffectUniform {
    thresholds: Vec4,
    mix_amounts: Vec4,
    exposure: Vec4,
    balance: Vec4,
    tint: Vec4,
    vignette: Vec4,
    frame: Vec4,
    rotation: Vec4,
    tone_threshold: Vec4,
}

impl From<RaceEffect> for EffectUniform {
    fn from(effect: RaceEffect) -> Self {
        let balance = effect.balance;
        let tint = Vec3::ONE.lerp(Vec3::from_array(balance.tint_rgb), balance.tint_amount);
        let angle = balance.vignette_rotation.to_radians();
        Self {
            thresholds: Vec4::new(
                effect.bloom_lower,
                effect.bloom_upper,
                effect.bloom_gain,
                effect.bloom_history,
            ),
            mix_amounts: Vec4::new(
                effect.scene_amount,
                effect.bloom_amount,
                effect.softening_amount,
                effect.bloom_medium,
            ),
            exposure: Vec4::new(
                effect.luminance_min,
                effect.luminance_max,
                effect.adapt_max,
                effect.bloom_large,
            ),
            balance: Vec4::new(
                balance.contrast,
                balance.brightness,
                balance.maximum,
                balance.colour,
            ),
            tint: tint.extend(balance.vignette_amount),
            vignette: Vec3::from_array(balance.vignette_rgb).extend(balance.vignette_shape),
            frame: Vec4::new(
                balance.vignette_scale[0],
                balance.vignette_scale[1],
                balance.vignette_offset[0],
                balance.vignette_offset[1],
            ),
            rotation: Vec4::new(angle.cos(), angle.sin(), balance.vignette_transition, 1.),
            tone_threshold: Vec4::new(
                effect.luminance_threshold,
                threshold_scale(effect.luminance_threshold),
                f32::from(u8::from(effect.threshold_enabled)),
                0.,
            ),
        }
    }
}

// 7471C0 clamps the reciprocal calculation just below its singularity.
fn threshold_scale(threshold: f32) -> f32 {
    if threshold < 0.99 {
        1. / (1. - threshold)
    } else {
        100.0001
    }
}

/// Sample the native named destinations using the existing event clock.
fn timeline_effect(
    mut effect: RaceEffect,
    timeline: &grid_tools::time_of_day::Timeline,
    day_fraction: f64,
    head_camera: bool,
) -> (RaceEffect, f32) {
    let balance = &mut effect.balance;
    let sample = |name: &str| {
        timeline
            .curves
            .get(name)
            .filter(|curve| curve.has_keys())
            .map(|curve| curve.value(day_fraction))
            .filter(|value| value.is_finite())
    };
    // ToneMapFast slots: 744270. BloomFromDsX4 slots: 7447E0.
    let minimum = sample("lummin").unwrap_or(effect.luminance_min);
    let maximum = sample("lummax").unwrap_or(effect.luminance_max);
    if minimum >= 0. && maximum >= minimum {
        effect.luminance_min = minimum;
        effect.luminance_max = maximum;
    }
    for (field, destination) in [
        ("adaptmax", &mut effect.adapt_max),
        ("lumthreshold", &mut effect.luminance_threshold),
        ("intensity", &mut effect.bloom_gain),
        ("mixmedium", &mut effect.bloom_medium),
        ("mixlarge", &mut effect.bloom_large),
        ("contrast", &mut balance.contrast),
        ("colour", &mut balance.colour),
        ("tintamount", &mut balance.tint_amount),
        ("vignetteamount", &mut balance.vignette_amount),
    ] {
        if let Some(value) = sample(field) {
            *destination = value;
        }
    }
    if let Some(value) = sample("togglethreshold") {
        effect.threshold_enabled = value > grid_tools::time_of_day::THRESHOLD;
    }
    for (fields, colour) in [
        (
            ["tintcolour_r", "tintcolour_g", "tintcolour_b"],
            &mut balance.tint_rgb,
        ),
        (
            ["vignettecolour_r", "vignettecolour_g", "vignettecolour_b"],
            &mut balance.vignette_rgb,
        ),
    ] {
        for (name, channel) in fields.into_iter().zip(colour) {
            if let Some(value) = sample(name) {
                *channel = (value + 0.5).trunc().clamp(0., 255.) / 255.;
            }
        }
    }
    // The original reads this scalar from E5EBE8 in 618440; 9ACFB0
    // writes the current luminanceFactor or luminanceFactorHeadcam there.
    let scale = sample(if head_camera {
        "luminancefactorheadcam"
    } else {
        "luminancefactor"
    })
    .filter(|scale| *scale >= 0.)
    .unwrap_or(1.);
    (effect, scale)
}

fn set_effect(
    mut commands: Commands,
    views: Query<(Entity, &TrackGrade, Option<&EffectUniform>)>,
    cameras: Option<Res<crate::camera::Cameras>>,
    replay: Option<Res<crate::replay::Replay>>,
    (menu, event): (
        Option<Res<crate::pause_menu::PauseMenu>>,
        Option<Res<crate::le_mans_event::LeMansEvent>>,
    ),
    time: Res<Time<Real>>,
    mut share: Local<f32>,
) {
    let open = menu.as_ref().is_some_and(|menu| menu.open);
    *share = if open {
        (*share + time.delta_secs() / 0.3).min(1.)
    } else {
        (*share - time.delta_secs() / 0.29).max(0.)
    };
    let inside = crate::camera::own_car(cameras.as_deref(), replay.as_deref(), true)
        == crate::camera::OwnCar::Interior;
    // The existing audio camera classification's Internal is exactly the
    // render view with Rig::Head. Replay detaches this view.
    let head_camera = !replay
        .as_ref()
        .is_some_and(|replay| replay.session.is_some())
        && cameras.as_ref().is_some_and(|cameras| {
            cameras.audio_camera() == grid_tools::audio_mix::Camera::Internal
        });
    let current_timeline = event
        .as_ref()
        .filter(|e| e.mode == grid_tools::time_of_day::Mode::Timeline)
        .and_then(|e| {
            e.timeline
                .as_ref()
                .map(|timeline| (timeline, e.time_of_day))
        });
    for (view, effects, current) in &views {
        let saved = if inside {
            effects.effects.interior
        } else {
            effects.effects.exterior
        };
        let (effect, scale) = current_timeline.map_or((saved, 1.), |(timeline, day_fraction)| {
            timeline_effect(saved, timeline, day_fraction, head_camera)
        });
        let effect = effects
            .effects
            .pause
            .map_or(effect, |pause| pause.blend(effect, *share));
        let mut value = EffectUniform::from(effect);
        value.rotation.w = scale;
        value.tone_threshold.w = f32::from(u8::from(!effects.motion_blur));
        if current != Some(&value) {
            commands.entity(view).insert(value);
        }
    }
}

#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub(super) struct RaceMerge;

pub fn install(app: &mut App) {
    if app.get_sub_app(RenderApp).is_none() {
        return;
    }
    app.world_mut()
        .resource_mut::<Assets<Shader>>()
        .insert(
            SHADER.id(),
            Shader::from_wgsl(include_str!("track_grade.wgsl"), "grid_track_grade.wgsl"),
        )
        .expect("UUID shader asset");
    app.add_systems(Last, set_effect).add_plugins((
        ExtractComponentPlugin::<EffectUniform>::default(),
        UniformComponentPlugin::<EffectUniform>::default(),
    ));
    if let Some(render) = app.get_sub_app_mut(RenderApp) {
        render
            .init_resource::<Scratch>()
            .add_systems(RenderStartup, initialize)
            .add_systems(
                Core3d,
                (
                    prepare_scene
                        .in_set(Core3dSystems::PostProcess)
                        .before(motion_blur),
                    draw.in_set(Core3dSystems::PostProcess)
                        .in_set(RaceMerge)
                        .after(tonemapping),
                ),
            );
    }
}

struct Stage {
    name: &'static str,
    format: TextureFormat,
}

const STAGES: [Stage; 12] = [
    Stage {
        name: "display_x4",
        format: COLOUR_FORMAT,
    },
    Stage {
        name: "down_x4",
        format: COLOUR_FORMAT,
    },
    Stage {
        name: "luminance",
        format: COLOUR_FORMAT,
    },
    Stage {
        name: "adapt",
        format: LUMINANCE_FORMAT,
    },
    Stage {
        name: "bloom_x2",
        format: COLOUR_FORMAT,
    },
    Stage {
        name: "gauss9_h",
        format: COLOUR_FORMAT,
    },
    Stage {
        name: "gauss9_v",
        format: COLOUR_FORMAT,
    },
    Stage {
        name: "bloom_large",
        format: COLOUR_FORMAT,
    },
    Stage {
        name: "bloom_merge",
        format: COLOUR_FORMAT,
    },
    Stage {
        name: "soften",
        format: COLOUR_FORMAT,
    },
    Stage {
        name: "final_merge",
        format: TextureFormat::Rgba16Float,
    },
    Stage {
        name: "bound_main_colour",
        format: TextureFormat::Rgba16Float,
    },
];

#[derive(Resource)]
struct Pipeline {
    layout: BindGroupLayoutDescriptor,
    sampler: Sampler,
    ids: [CachedRenderPipelineId; STAGES.len()],
}

fn initialize(
    mut commands: Commands,
    device: Res<RenderDevice>,
    fullscreen: Res<FullscreenShader>,
    cache: Res<PipelineCache>,
) {
    let image = || texture_2d(TextureSampleType::Float { filterable: true });
    let layout = BindGroupLayoutDescriptor::new(
        "Type2 race postprocess",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                image(),
                image(),
                image(),
                sampler(SamplerBindingType::Filtering),
                uniform_buffer::<EffectUniform>(true),
                image(),
            ),
        ),
    );
    let ids = STAGES.map(|stage| {
        cache.queue_render_pipeline(RenderPipelineDescriptor {
            label: Some(format!("Type2 {}", stage.name).into()),
            layout: vec![layout.clone()],
            vertex: fullscreen.to_vertex_state(),
            fragment: Some(FragmentState {
                shader: SHADER,
                entry_point: Some(stage.name.into()),
                targets: vec![Some(ColorTargetState {
                    format: stage.format,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })],
                ..default()
            }),
            ..default()
        })
    });
    commands.insert_resource(Pipeline {
        layout,
        sampler: device.create_sampler(&SamplerDescriptor {
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..default()
        }),
        ids,
    });
}

#[derive(Default, Resource)]
struct Scratch(Option<Images>);

impl Scratch {
    fn images(&mut self, device: &RenderDevice, view: Entity, target: &ViewTarget) -> &mut Images {
        let texture = target.main_texture();
        let size = UVec2::new(texture.width(), texture.height());
        if self
            .0
            .as_ref()
            .is_none_or(|old| old.view != view || old.size != size)
        {
            self.0 = Some(Images::new(device, view, size));
        }
        self.0.as_mut().expect("Type2 scratch allocated")
    }
}

struct Images {
    view: Entity,
    size: UVec2,
    initialized: bool,
    bounded_frame: Option<u32>,
    current: usize,
    scene: TextureView,
    meter_ds: TextureView,
    meter_32: TextureView,
    meter_8: TextureView,
    meter_1: TextureView,
    luminance: [TextureView; 2],
    bright: TextureView,
    medium_h: TextureView,
    medium: TextureView,
    large: TextureView,
    bloom: [TextureView; 2],
    softened: TextureView,
    soft_h: TextureView,
    soft: TextureView,
}

impl Images {
    fn new(device: &RenderDevice, view: Entity, size: UVec2) -> Self {
        let texture = |name, size: UVec2, format| {
            let size = size.max(UVec2::ONE);
            device
                .create_texture(&TextureDescriptor {
                    label: Some(name),
                    size: Extent3d {
                        width: size.x,
                        height: size.y,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: TextureDimension::D2,
                    format,
                    usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&TextureViewDescriptor::default())
        };
        let colour = |name, size| texture(name, size, COLOUR_FORMAT);
        let quarter = size / 4;
        let eighth = size / 8;
        Self {
            view,
            size,
            initialized: false,
            bounded_frame: None,
            current: 0,
            scene: colour("Type2 scene x4", quarter),
            meter_ds: colour("Type2 reduced luminance source", quarter / 4),
            meter_32: colour("Type2 meter 32x32", UVec2::splat(32)),
            meter_8: colour("Type2 meter 8x8", UVec2::splat(8)),
            meter_1: colour("Type2 meter 1x1", UVec2::ONE),
            luminance: std::array::from_fn(|_| {
                texture("Type2 adapted luminance", UVec2::ONE, LUMINANCE_FORMAT)
            }),
            bright: colour("Type2 bright x8", eighth),
            medium_h: colour("Type2 horizontal medium bloom", eighth),
            medium: colour("Type2 medium bloom", eighth),
            large: colour("Type2 large bloom", eighth),
            bloom: std::array::from_fn(|_| colour("Type2 bloom history", eighth)),
            softened: colour("Type2 exposed softening", quarter),
            soft_h: colour("Type2 horizontal softening", quarter),
            soft: colour("Type2 softening", quarter),
        }
    }
}

type RenderView<'w> = (
    Entity,
    &'w ViewTarget,
    &'w EffectUniform,
    &'w DynamicUniformIndex<EffectUniform>,
);

struct StageBindings<'a> {
    layout: BindGroupLayout,
    sampler: &'a Sampler,
    uniform: BindingResource<'a>,
    index: u32,
}

impl StageBindings<'_> {
    fn draw(
        &self,
        ctx: &mut RenderContext,
        stage: &RenderPipeline,
        [source, other, previous, adapted]: [&TextureView; 4],
        output: &TextureView,
    ) {
        let group = ctx.render_device().create_bind_group(
            "Type2 stage",
            &self.layout,
            &BindGroupEntries::sequential((
                source,
                other,
                previous,
                self.sampler,
                self.uniform.clone(),
                adapted,
            )),
        );
        let mut pass = ctx
            .command_encoder()
            .begin_render_pass(&RenderPassDescriptor {
                label: Some("Type2 stage"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: output,
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations::default(),
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        pass.set_pipeline(stage);
        pass.set_bind_group(0, &group, &[self.index]);
        pass.draw(0..3, 0..1);
    }
}

/// Match the normalized range of the original MainColourTarget before
/// any filtering. Swig also needs its quarter scene before motion blur.
fn prepare_scene(
    view: ViewQuery<RenderView>,
    pipeline: Res<Pipeline>,
    cache: Res<PipelineCache>,
    uniforms: Res<ComponentUniforms<EffectUniform>>,
    frame: Res<FrameCount>,
    mut ctx: RenderContext,
    mut scratch: ResMut<Scratch>,
) {
    let (view, target, effect, index) = view.into_inner();
    if target.main_texture_format() != TextureFormat::Rgba16Float {
        return;
    }
    let Some(bound) = cache.get_render_pipeline(pipeline.ids[STAGES.len() - 1]) else {
        return;
    };
    let Some(quarter) = cache.get_render_pipeline(pipeline.ids[0]) else {
        return;
    };
    let Some(binding) = uniforms.uniforms().binding() else {
        return;
    };
    let bindings = StageBindings {
        layout: cache.get_bind_group_layout(&pipeline.layout),
        sampler: &pipeline.sampler,
        uniform: binding,
        index: index.index(),
    };
    let images = scratch.images(ctx.render_device(), view, target);
    let write = target.post_process_write();
    bindings.draw(&mut ctx, bound, [write.source; 4], write.destination);
    if effect.tone_threshold.w <= 0. {
        bindings.draw(&mut ctx, quarter, [write.destination; 4], &images.scene);
    }
    images.bounded_frame = Some(frame.0);
}

fn draw(
    view: ViewQuery<RenderView>,
    pipeline: Res<Pipeline>,
    cache: Res<PipelineCache>,
    uniforms: Res<ComponentUniforms<EffectUniform>>,
    frame: Res<FrameCount>,
    mut ctx: RenderContext,
    mut scratch: ResMut<Scratch>,
) {
    let (view, target, effect, index) = view.into_inner();
    if target.main_texture_format() != TextureFormat::Rgba16Float {
        return;
    }
    let Some(binding) = uniforms.uniforms().binding() else {
        return;
    };
    let ids = pipeline.ids.map(|id| cache.get_render_pipeline(id));
    let [
        Some(display_x4),
        Some(down_x4),
        Some(luminance),
        Some(adapt),
        Some(bloom_x2),
        Some(gauss_h),
        Some(gauss_v),
        Some(bloom_large),
        Some(bloom_merge),
        Some(soften),
        Some(final_merge),
        Some(_),
    ] = ids
    else {
        return;
    };
    let no_blur = effect.tone_threshold.w > 0.;
    let images = scratch.images(ctx.render_device(), view, target);
    if images.bounded_frame != Some(frame.0) {
        return;
    }
    let cur = images.current;
    let prev = 1 - cur;
    if !images.initialized {
        let (merge, downsample) = if no_blur {
            ("FinalMergeNoBlur", "main")
        } else {
            ("CheapFinalMerge", "pre-motion")
        };
        println!(
            "graphics_source_postprocess active=Type2 size={}x{} domain=display merge={merge} downsample={downsample} main_colour_range=unorm",
            images.size.x, images.size.y
        );
        // 605CB0 seeds the source luminance history with 0.5 in the
        // RGB lanes. FastLumAdapt reads rate in R and exposure in G.
        ctx.command_encoder()
            .begin_render_pass(&RenderPassDescriptor {
                label: Some("initialize Type2 luminance history"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &images.luminance[prev],
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(LinearRgba::new(0.5, 0.5, 0.5, 1.).into()),
                        store: StoreOp::Store,
                    },
                })],
                ..default()
            });
    }
    let bindings = StageBindings {
        layout: cache.get_bind_group_layout(&pipeline.layout),
        sampler: &pipeline.sampler,
        uniform: binding,
        index: index.index(),
    };
    let write = target.post_process_write();
    let mut pass = |stage: &RenderPipeline,
                    source: &TextureView,
                    other: &TextureView,
                    previous: &TextureView,
                    adapted: &TextureView,
                    output: &TextureView| {
        bindings.draw(&mut ctx, stage, [source, other, previous, adapted], output);
    };
    let mut from = |stage: &RenderPipeline, source: &TextureView, output: &TextureView| {
        pass(stage, source, source, source, source, output);
    };
    if no_blur {
        from(display_x4, write.source, &images.scene);
    }
    from(down_x4, &images.scene, &images.meter_ds);
    from(down_x4, &images.meter_ds, &images.meter_32);
    from(down_x4, &images.meter_32, &images.meter_8);
    from(luminance, &images.meter_8, &images.meter_1);
    from(bloom_x2, &images.scene, &images.bright);
    from(gauss_h, &images.bright, &images.medium_h);
    from(gauss_v, &images.medium_h, &images.medium);
    from(bloom_large, &images.bright, &images.large);
    pass(
        adapt,
        &images.meter_1,
        &images.meter_1,
        &images.luminance[prev],
        &images.meter_1,
        &images.luminance[cur],
    );
    pass(
        bloom_merge,
        &images.medium,
        &images.large,
        &images.bloom[prev],
        &images.luminance[cur],
        &images.bloom[cur],
    );
    if effect.mix_amounts.z > 0. {
        pass(
            soften,
            &images.scene,
            &images.luminance[cur],
            &images.luminance[cur],
            &images.luminance[cur],
            &images.softened,
        );
        pass(
            gauss_h,
            &images.softened,
            &images.softened,
            &images.softened,
            &images.softened,
            &images.soft_h,
        );
        pass(
            gauss_v,
            &images.soft_h,
            &images.soft_h,
            &images.soft_h,
            &images.soft_h,
            &images.soft,
        );
    }
    pass(
        final_merge,
        write.source,
        &images.soft,
        &images.bloom[cur],
        &images.luminance[cur],
        write.destination,
    );
    images.current = prev;
    images.initialized = true;
}

#[cfg(test)]
mod timeline_tests {
    use super::*;
    use crate::le_mans_event::LeMansEvent;
    use grid_tools::time_of_day::{Mode, Timeline};

    #[test]
    #[ignore = "requires GRID_GAME_ROOT and original pause resources"]
    fn purchased_pause_fade_updates_the_actual_race_view_uniform() {
        use crate::pause_menu::{Command, Nav, Options, PauseMenu, Variant};
        use std::time::Duration;

        let root = std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").unwrap());
        for (path, target) in [
            ("tracks/europe/donington/route_0", Vec3::new(0.2, 0.6, 0.4)),
            ("tracks/japan/haruna/route_1", Vec3::new(0.3526, 1., 0.9364)),
            (
                "tracks/japan/shibuya/route_0",
                Vec3::new(0.2, 0.0983, 0.2659),
            ),
        ] {
            let route = Route::new(path).unwrap();
            let data = load(&root, &route).unwrap();
            let mut app = App::new();
            app.insert_resource(Time::<Real>::default())
                .insert_resource(PauseMenu::new(
                    Variant::TestDrive,
                    false,
                    false,
                    Options::default(),
                ))
                .add_systems(Last, set_effect);
            let camera = app.world_mut().spawn(TrackGrade::new(data, false)).id();
            let sample = |app: &mut App, millis| {
                app.world_mut()
                    .resource_mut::<Time<Real>>()
                    .advance_by(Duration::from_millis(millis));
                app.update();
                let values = app.world().get::<EffectUniform>(camera).unwrap();
                Vec3::new(values.exposure.x, values.exposure.y, values.mix_amounts.y)
            };
            let assert_sample = |actual: Vec3, expected: Vec3| {
                assert!(
                    actual.abs_diff_eq(expected, 1e-6),
                    "{path}: actual {actual:?}, expected {expected:?}"
                );
            };

            let initial = sample(&mut app, 0);
            assert_sample(
                initial,
                Vec3::new(
                    data.exterior.luminance_min,
                    data.exterior.luminance_max,
                    data.exterior.bloom_amount,
                ),
            );
            let menu_step =
                |app: &mut App, nav: Nav| app.world_mut().resource_mut::<PauseMenu>().step(nav);
            assert_eq!(
                menu_step(
                    &mut app,
                    Nav {
                        pause: true,
                        ..default()
                    }
                ),
                Some(Command::Open)
            );
            let midpoint = initial.lerp(target, 0.5);
            assert_sample(sample(&mut app, 150), midpoint);
            assert_sample(sample(&mut app, 150), target);
            assert_sample(sample(&mut app, 500), target);
            assert_eq!(
                menu_step(
                    &mut app,
                    Nav {
                        back: true,
                        ..default()
                    }
                ),
                Some(Command::Resume)
            );
            assert_sample(sample(&mut app, 145), midpoint);
            assert_sample(sample(&mut app, 145), initial);
            assert_eq!(
                menu_step(
                    &mut app,
                    Nav {
                        pause: true,
                        ..default()
                    }
                ),
                Some(Command::Open)
            );
            assert_sample(sample(&mut app, 150), midpoint);
        }
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT and original Le Mans resources"]
    fn purchased_timeline_updates_the_actual_race_view_uniform() {
        let root = std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").unwrap());
        let route = Route::new("tracks/europe/le_mans/route_0").unwrap();
        let data = load(&root, &route).unwrap();
        let timeline = Timeline::load(&root, &route).unwrap().unwrap();
        let mut app = App::new();
        app.insert_resource(Time::<Real>::default())
            .insert_resource(LeMansEvent::new(
                None,
                Mode::Timeline,
                Vec::new(),
                Some(timeline),
            ))
            .add_systems(Last, set_effect);
        let camera = app.world_mut().spawn(TrackGrade::new(data, true)).id();

        let evaluate = |app: &mut App, hour: u32, minute: u32| {
            app.world_mut().resource_mut::<LeMansEvent>().time_of_day =
                f64::from(60 * hour + minute) / 1440.;
            app.update();
            *app.world().get::<EffectUniform>(camera).unwrap()
        };

        // Exact original keyframes at 09:05 and 21:27. Fields with no
        // registered timeline write still come from the selected race slot.
        let day = evaluate(&mut app, 9, 5);
        let night = evaluate(&mut app, 21, 27);
        assert!((day.balance.x - 1.).abs() < 1e-6);
        assert!((day.balance.w - 0.8052).abs() < 1e-6);
        assert_eq!(day.tint.w, 0.);
        assert!((night.balance.x - 1.0909).abs() < 1e-6);
        assert!((night.balance.w - 0.6753).abs() < 1e-6);
        assert!((night.tint.w - 0.2338).abs() < 1e-6);
        assert_eq!(night.balance.y, data.exterior.balance.brightness);
        assert!((day.exposure.x - 0.499958).abs() < 1e-6);
        assert!((night.exposure.x - 0.2597).abs() < 1e-6);
        assert!((night.exposure.y - 0.8610903).abs() < 1e-6);
        assert!((night.exposure.z - 0.035714).abs() < 1e-6);
        assert!((day.thresholds.z - 1.4905977).abs() < 1e-6);
        assert!((night.exposure.w - 0.8958476).abs() < 1e-6);
        assert!((night.mix_amounts.w - 0.4131796).abs() < 1e-6);
        assert_eq!(night.thresholds.w, data.exterior.bloom_history);
        assert_eq!(night.mix_amounts.x, data.exterior.scene_amount);
        assert!((night.rotation.w - 0.9984019).abs() < 1e-6);
        let event = app.world().resource::<LeMansEvent>();
        let (_, head_scale) = timeline_effect(
            data.interior,
            event.timeline.as_ref().unwrap(),
            event.time_of_day,
            true,
        );
        assert!((head_scale - 0.706255).abs() < 1e-6);
    }
}
