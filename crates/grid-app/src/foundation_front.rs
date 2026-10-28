//! Default-off bounded front-sheet coverage/depth diagnostic, not original shader parity.
//! Inherits PBR lighting and fixes only selected saved main-pass states.
use bevy::{
    asset::AssetId,
    mesh::MeshVertexBufferLayoutRef,
    pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline},
    prelude::*,
    render::render_resource::{
        AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, ColorWrites,
        CompareFunction, RenderPipelineDescriptor, SpecializedMeshPipelineError, TextureFormat,
    },
};
use std::collections::HashMap;

pub(crate) const TARGET: &str = "objects/gg_building_grandstand_01_j_04";
const MAX_TARGETS: usize = 128;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Mode {
    #[default]
    Off,
    Control,
    SourceFront,
}
impl Mode {
    pub(crate) fn parse(s: &str) -> Result<Self, &'static str> {
        match s {
            "off" => Ok(Self::Off),
            "control" => Ok(Self::Control),
            "source-front" => Ok(Self::SourceFront),
            _ => Err("--foundation-front needs off, control or source-front"),
        }
    }
    pub(crate) fn validate(self, c: Context) -> Result<(), &'static str> {
        if self == Self::Off {
            return Ok(());
        }
        if c.enhanced
            || c.aa != 0
            || c.drive
            || !c.reference
            || c.collision
            || c.headless
            || !c.render
            || !c.route8
            || !c.lod0
            || !c.clean_settings
            || !c.source_sky
            || !c.source_track
            || !c.sky_reflections
            || !c.native1080
            || !c.other_pilots_off
            || !c.no_perf
            || !c.no_orbit
            || !c.no_menu
            || c.frames.is_none_or(|n| !(1..=600).contains(&n))
        {
            return Err(
                "foundation-front requires clean static rendered Detroit route8/sub0 LOD0 reference, baseline configAA0/actualMSAA4/native1080, source sky/sky reflections, benchmark1..600, no menu/orbit/other pilots/perf",
            );
        }
        Ok(())
    }
}
#[derive(Clone, Copy)]
pub(crate) struct Context {
    pub enhanced: bool,
    pub aa: u8,
    pub drive: bool,
    pub reference: bool,
    pub collision: bool,
    pub headless: bool,
    pub render: bool,
    pub route8: bool,
    pub lod0: bool,
    pub clean_settings: bool,
    pub source_sky: bool,
    pub source_track: bool,
    pub sky_reflections: bool,
    pub native1080: bool,
    pub other_pilots_off: bool,
    pub no_perf: bool,
    pub no_orbit: bool,
    pub no_menu: bool,
    pub frames: Option<u32>,
}
#[derive(Resource)]
struct Fixture(Mode);
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub(crate) struct FrontPass {
    // Nonempty extension layout; inherited PBR shaders do not read this binding.
    #[uniform(100)]
    marker: Vec4,
}
pub(crate) type FrontMaterial = ExtendedMaterial<StandardMaterial, FrontPass>;
impl MaterialExtension for FrontPass {
    // Source shadow/auxiliary masks are not emulated by this main-view diagnostic.
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn specialize(
        _: &MaterialExtensionPipeline,
        d: &mut RenderPipelineDescriptor,
        _: &MeshVertexBufferLayoutRef,
        _: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        source_front_pipeline(d).expect("foundation-front refused unsupported actual pipeline");
        static RECEIPTS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        if RECEIPTS
            .try_update(
                std::sync::atomic::Ordering::Relaxed,
                std::sync::atomic::Ordering::Relaxed,
                |count| (count < 8).then_some(count + 1),
            )
            .is_ok()
        {
            println!(
                "foundation_specialize format={:?} targets={} samples={} depth={:?} blend={:?} cull={:?} scope=specialized_descriptor_not_queued_or_pixels",
                d.fragment.as_ref().unwrap().targets[0]
                    .as_ref()
                    .unwrap()
                    .format,
                d.fragment.as_ref().unwrap().targets.len(),
                d.multisample.count,
                d.depth_stencil,
                d.fragment.as_ref().unwrap().targets[0]
                    .as_ref()
                    .unwrap()
                    .blend,
                d.primitive.cull_mode
            );
        }
        Ok(())
    }
}

/// Validate the complete bounded attachment domain before mutating any field.
fn source_front_pipeline(d: &mut RenderPipelineDescriptor) -> Result<(), &'static str> {
    let fragment = d.fragment.as_ref().ok_or("missing fragment")?;
    if fragment.targets.len() != 1 {
        return Err("needs exactly one colour target");
    }
    let target = fragment.targets[0]
        .as_ref()
        .ok_or("missing colour target")?;
    if target.format != TextureFormat::Rgba8UnormSrgb || target.write_mask != ColorWrites::ALL {
        return Err("needs RGBA8sRGB/all colour lanes");
    }
    let depth = d.depth_stencil.as_ref().ok_or("missing scene depth")?;
    if depth.format != TextureFormat::Depth32Float
        || depth.depth_compare != Some(CompareFunction::GreaterEqual)
    {
        return Err("needs reverse-Z Depth32Float GreaterEqual");
    }
    if d.multisample.count != 4 {
        return Err("needs actual MSAA4");
    }
    // Explicit saved ordinary colour and separate-alpha factors; not generic Blend.
    d.fragment.as_mut().unwrap().targets[0]
        .as_mut()
        .unwrap()
        .blend = Some(BlendState {
        color: BlendComponent {
            src_factor: BlendFactor::SrcAlpha,
            dst_factor: BlendFactor::OneMinusSrcAlpha,
            operation: BlendOperation::Add,
        },
        alpha: BlendComponent {
            src_factor: BlendFactor::Zero,
            dst_factor: BlendFactor::Zero,
            operation: BlendOperation::Add,
        },
    });
    d.depth_stencil.as_mut().unwrap().depth_write_enabled = Some(true);
    d.multisample.alpha_to_coverage_enabled = false;
    // Keep existing raster cull/winding/bias. Saved CW mapping remains unproven.
    Ok(())
}
pub(crate) fn actual_pipeline(d: &RenderPipelineDescriptor) -> bool {
    let mut expected = d.clone();
    source_front_pipeline(&mut expected).is_ok() && expected == *d
}
fn base_admitted(base: &StandardMaterial) -> bool {
    base.alpha_mode == AlphaMode::Opaque
        && base.base_color.alpha().to_bits() == 1f32.to_bits()
        && base.base_color_texture.is_some()
}
fn candidate(mode: Mode, name: &str, base: &StandardMaterial) -> Option<FrontMaterial> {
    if mode != Mode::SourceFront || name != TARGET {
        return None;
    }
    assert!(
        base_admitted(base),
        "foundation front needs original solid textured unit-alpha material"
    );
    let mut clone = base.clone();
    clone.alpha_mode = AlphaMode::Blend; // Transparent routing, no shader alpha remap/discard.
    Some(FrontMaterial {
        base: clone,
        extension: FrontPass { marker: Vec4::ZERO },
    })
}
pub(crate) fn install(app: &mut App, mode: Mode) {
    if mode == Mode::Off {
        return;
    }
    app.insert_resource(Fixture(mode));
    if mode == Mode::SourceFront {
        app.add_plugins(MaterialPlugin::<FrontMaterial>::default());
    }
    app.add_systems(PostStartup, attach);
}
#[allow(clippy::type_complexity)] // Exact source draw ownership; no child/proxy entities.
fn attach(
    mut commands: Commands,
    fixture: Res<Fixture>,
    source: Res<Assets<StandardMaterial>>,
    mut extended: Option<ResMut<Assets<FrontMaterial>>>,
    sources: Query<
        (
            Entity,
            &super::track_surface::SourcePart,
            &MeshMaterial3d<StandardMaterial>,
        ),
        With<Mesh3d>,
    >,
    cameras: Query<&Msaa, With<super::ViewerCamera>>,
) {
    assert_eq!(
        *cameras
            .single()
            .expect("foundation front requires one main camera"),
        Msaa::Sample4
    );
    let selected: Vec<_> = sources
        .iter()
        .filter(|(_, part, _)| part.0 == TARGET)
        .map(|(entity, _, material)| (entity, material.0.clone()))
        .collect();
    assert!(
        (1..=MAX_TARGETS).contains(&selected.len()),
        "foundation front absent or target bound exceeded"
    );
    // Preflight ALL selected source assets before scheduling the first replacement.
    for (_, handle) in &selected {
        assert!(
            base_admitted(
                source
                    .get(handle)
                    .expect("foundation original material missing")
            ),
            "foundation unsupported original material"
        );
    }
    let mut cache: HashMap<AssetId<StandardMaterial>, Handle<FrontMaterial>> = HashMap::new();
    let mut changed = 0;
    if fixture.0 == Mode::SourceFront {
        let assets = extended
            .as_mut()
            .expect("SourceFront material plugin missing");
        for (entity, original) in &selected {
            let handle = cache
                .entry(original.id())
                .or_insert_with(|| {
                    assets.add(candidate(fixture.0, TARGET, source.get(original).unwrap()).unwrap())
                })
                .clone();
            commands
                .entity(*entity)
                .remove::<MeshMaterial3d<StandardMaterial>>()
                .insert(MeshMaterial3d(handle));
            changed += 1;
        }
    }
    let active = fixture.0 == Mode::SourceFront;
    println!(
        "foundation_front mode={:?} target={} selected={} replaced={} source_main_colour={} source_depth_write={} source_separate_alpha_zero={} alpha_source=diffuse_times_existing_PBR_base pbr_lighting=retained cull=existing shadow_aux=not_emulated geometry_texture=unchanged count_scope=material_components_not_queued_draws",
        fixture.0,
        TARGET,
        selected.len(),
        changed,
        active,
        active,
        active
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::render::render_resource::{ColorTargetState, DepthStencilState, FragmentState};
    fn context() -> Context {
        Context {
            enhanced: false,
            aa: 0,
            drive: false,
            reference: true,
            collision: false,
            headless: false,
            render: true,
            route8: true,
            lod0: true,
            clean_settings: true,
            source_sky: true,
            source_track: true,
            sky_reflections: true,
            native1080: true,
            other_pilots_off: true,
            no_perf: true,
            no_orbit: true,
            no_menu: true,
            frames: Some(120),
        }
    }
    fn pipeline() -> RenderPipelineDescriptor {
        let mut d = RenderPipelineDescriptor {
            label: Some("foundation pipeline fixture".into()),
            fragment: Some(FragmentState {
                targets: vec![Some(ColorTargetState {
                    format: TextureFormat::Rgba8UnormSrgb,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
                ..default()
            }),
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth32Float,
                depth_write_enabled: Some(false),
                depth_compare: Some(CompareFunction::GreaterEqual),
                stencil: default(),
                bias: default(),
            }),
            ..default()
        };
        d.multisample.count = 4;
        d
    }
    #[test]
    fn off_and_control_do_not_install_render_material_plugin() {
        let mut off = App::new();
        install(&mut off, Mode::Off);
        assert!(!off.world().contains_resource::<Fixture>());
        assert!(!off.is_plugin_added::<MaterialPlugin<FrontMaterial>>());
        let mut control = App::new();
        install(&mut control, Mode::Control);
        assert_eq!(control.world().resource::<Fixture>().0, Mode::Control);
        assert!(!control.is_plugin_added::<MaterialPlugin<FrontMaterial>>());
        assert!(!control.world().contains_resource::<Assets<FrontMaterial>>());
    }
    #[test]
    fn gate_keeps_off_unconditional_and_rejects_each_unsupported_context() {
        assert_eq!(Mode::default(), Mode::Off);
        assert!(Mode::parse("blend").is_err());
        for mode in [Mode::Control, Mode::SourceFront] {
            assert!(mode.validate(context()).is_ok());
            let c = context();
            let bad = [
                Context {
                    enhanced: true,
                    ..c
                },
                Context { aa: 1, ..c },
                Context { drive: true, ..c },
                Context {
                    reference: false,
                    ..c
                },
                Context {
                    collision: true,
                    ..c
                },
                Context {
                    headless: true,
                    ..c
                },
                Context { render: false, ..c },
                Context { route8: false, ..c },
                Context { lod0: false, ..c },
                Context {
                    clean_settings: false,
                    ..c
                },
                Context {
                    source_sky: false,
                    ..c
                },
                Context {
                    source_track: false,
                    ..c
                },
                Context {
                    sky_reflections: false,
                    ..c
                },
                Context {
                    native1080: false,
                    ..c
                },
                Context {
                    other_pilots_off: false,
                    ..c
                },
                Context {
                    no_perf: false,
                    ..c
                },
                Context {
                    no_orbit: false,
                    ..c
                },
                Context {
                    no_menu: false,
                    ..c
                },
                Context { frames: None, ..c },
                Context {
                    frames: Some(0),
                    ..c
                },
                Context {
                    frames: Some(601),
                    ..c
                },
            ];
            for c in bad {
                assert!(mode.validate(c).is_err());
                assert!(Mode::Off.validate(c).is_ok());
            }
        }
    }
    #[test]
    fn pipeline_changes_only_explicit_blend_write_and_a2c_and_keeps_cull() {
        let mut d = pipeline();
        d.multisample.alpha_to_coverage_enabled = true;
        let mut expected = d.clone();
        expected.fragment.as_mut().unwrap().targets[0]
            .as_mut()
            .unwrap()
            .blend = Some(BlendState {
            color: BlendComponent {
                src_factor: BlendFactor::SrcAlpha,
                dst_factor: BlendFactor::OneMinusSrcAlpha,
                operation: BlendOperation::Add,
            },
            alpha: BlendComponent {
                src_factor: BlendFactor::Zero,
                dst_factor: BlendFactor::Zero,
                operation: BlendOperation::Add,
            },
        });
        expected.depth_stencil.as_mut().unwrap().depth_write_enabled = Some(true);
        expected.multisample.alpha_to_coverage_enabled = false;
        source_front_pipeline(&mut d).unwrap();
        assert_eq!(d, expected);
        assert!(actual_pipeline(&d));
        let mut wrong = d.clone();
        wrong.depth_stencil.as_mut().unwrap().depth_write_enabled = Some(false);
        assert!(!actual_pipeline(&wrong));
        let mut wrong = d.clone();
        wrong.fragment.as_mut().unwrap().targets[0]
            .as_mut()
            .unwrap()
            .blend = Some(BlendState::ALPHA_BLENDING);
        assert!(!actual_pipeline(&wrong));
        assert!(matches!(
            <FrontPass as MaterialExtension>::fragment_shader(),
            bevy::shader::ShaderRef::Default
        ));
        assert!(matches!(
            <FrontPass as MaterialExtension>::vertex_shader(),
            bevy::shader::ShaderRef::Default
        ));
        assert!(!<FrontPass as MaterialExtension>::enable_prepass());
        assert!(!<FrontPass as MaterialExtension>::enable_shadows());
    }
    #[test]
    fn pipeline_refusals_are_transactional_before_any_mutation() {
        let mut bad = Vec::new();
        let mut d = pipeline();
        d.fragment = None;
        bad.push(d);
        let mut d = pipeline();
        d.fragment.as_mut().unwrap().targets.clear();
        bad.push(d);
        let mut d = pipeline();
        d.fragment.as_mut().unwrap().targets.push(None);
        bad.push(d);
        let mut d = pipeline();
        d.fragment.as_mut().unwrap().targets[0] = None;
        bad.push(d);
        let mut d = pipeline();
        d.fragment.as_mut().unwrap().targets[0]
            .as_mut()
            .unwrap()
            .format = TextureFormat::Rgba16Float;
        bad.push(d);
        let mut d = pipeline();
        d.fragment.as_mut().unwrap().targets[0]
            .as_mut()
            .unwrap()
            .write_mask = ColorWrites::RED;
        bad.push(d);
        let mut d = pipeline();
        d.depth_stencil = None;
        bad.push(d);
        let mut d = pipeline();
        d.depth_stencil.as_mut().unwrap().format = TextureFormat::Depth24Plus;
        bad.push(d);
        let mut d = pipeline();
        d.depth_stencil.as_mut().unwrap().depth_compare = Some(CompareFunction::Always);
        bad.push(d);
        let mut d = pipeline();
        d.multisample.count = 1;
        bad.push(d);
        for mut d in bad {
            let before = d.clone();
            assert!(source_front_pipeline(&mut d).is_err());
            assert_eq!(d, before);
        }
    }
    #[test]
    fn candidate_is_exact_target_only_and_preserves_every_base_field_except_alpha() {
        let original = StandardMaterial {
            base_color_texture: Some(Handle::default()),
            perceptual_roughness: 0.73,
            metallic: 0.17,
            depth_bias: 3.,
            double_sided: true,
            cull_mode: None,
            ..default()
        };
        let before = format!("{original:?}");
        for mode in [Mode::Off, Mode::Control] {
            assert!(candidate(mode, TARGET, &original).is_none());
        }
        for name in [
            "objects/building_grandstand_01_j_01",
            "objects/concrete_fence_white_end_01",
            "objects/gg_building_grandstand_01_j_04!0",
            "car0/Window Glass",
            "trees/leaf",
        ] {
            assert!(candidate(Mode::SourceFront, name, &original).is_none());
        }
        let mut made = candidate(Mode::SourceFront, TARGET, &original).unwrap();
        assert_eq!(made.base.alpha_mode, AlphaMode::Blend);
        made.base.alpha_mode = AlphaMode::Opaque;
        assert_eq!(format!("{:?}", made.base), before);
        assert_eq!(format!("{original:?}"), before);
        let mut bad = original.clone();
        bad.alpha_mode = AlphaMode::Mask(0.5);
        assert!(!base_admitted(&bad));
        let mut bad = original.clone();
        bad.base_color = Color::srgba(1., 1., 1., 0.5);
        assert!(!base_admitted(&bad));
        let mut bad = original;
        bad.base_color_texture = None;
        assert!(!base_admitted(&bad));
    }
}
