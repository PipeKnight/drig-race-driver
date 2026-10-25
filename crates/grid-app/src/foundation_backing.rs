//! Exact backing-only main-coverage experiment. No source D3D9level2 parity.
use bevy::{
    app::AppExit,
    asset::{AssetId, uuid_handle},
    light::{ClusteredDecal, NotShadowCaster},
    mesh::MeshVertexBufferLayoutRef,
    pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline},
    prelude::*,
    render::{batching::NoAutomaticBatching, render_resource::*},
    shader::{ShaderDefVal, ShaderRef},
};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
pub(crate) const TARGET: &str = "objects/building_grandstand_01_j_01";
pub(crate) const SHADER: Handle<Shader> = uuid_handle!("567492aa-d1c9-4f1f-b099-d577bb8012be");
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Mode {
    #[default]
    Off,
    Control,
    RawA2c,
}
impl Mode {
    pub(crate) fn parse(s: &str) -> Result<Self, &'static str> {
        match s {
            "off" => Ok(Self::Off),
            "control" => Ok(Self::Control),
            "raw-a2c" => Ok(Self::RawA2c),
            _ => Err("--foundation-backing needs off, control or raw-a2c"),
        }
    }
    pub(crate) fn validate(
        self,
        c: super::foundation_front::Context,
        front: super::foundation_front::Mode,
    ) -> Result<(), &'static str> {
        if self == Self::Off {
            return Ok(());
        }
        super::foundation_front::Mode::Control.validate(c)?;
        if front != super::foundation_front::Mode::SourceFront {
            return Err(
                "active backing requires explicit fixed SourceFront composite in both arms",
            );
        }
        Ok(())
    }
}
#[derive(Resource, Clone)]
pub(crate) struct Fixture {
    pub mode: Mode,
    pub stopping: Arc<AtomicBool>,
}
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub(crate) struct RawPass {
    #[uniform(100)]
    marker: Vec4,
}
pub(crate) type RawMaterial = ExtendedMaterial<StandardMaterial, RawPass>;
impl MaterialExtension for RawPass {
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
        _: &MaterialExtensionPipeline,
        d: &mut RenderPipelineDescriptor,
        _: &MeshVertexBufferLayoutRef,
        _: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        specialize_raw(d).expect("backing raw pipeline refused unsupported variant");
        Ok(())
    }
}
pub(crate) fn def_name(d: &ShaderDefVal) -> &str {
    match d {
        ShaderDefVal::Bool(s, _) | ShaderDefVal::Int(s, _) | ShaderDefVal::UInt(s, _) => s,
    }
}
fn forbidden(defs: &[ShaderDefVal]) -> bool {
    const BAD: [&str; 10] = [
        "MAY_DISCARD",
        "PREMULTIPLY_ALPHA",
        "OIT_ENABLED",
        "PREPASS_PIPELINE",
        "DEFERRED_PREPASS",
        "FORWARD_DECAL",
        "VISIBILITY_RANGE_DITHER",
        "MESHLET_MESH_MATERIAL_PASS",
        "READS_VIEW_TRANSMISSION_TEXTURE",
        "BLEND_PREMULTIPLIED_ALPHA",
    ];
    defs.iter().any(|d| BAD.contains(&def_name(d)))
}
fn attachment(d: &RenderPipelineDescriptor) -> Result<(), &'static str> {
    let f = d.fragment.as_ref().ok_or("no fragment")?;
    if f.targets.len() != 1 {
        return Err("one colour target required");
    }
    let t = f.targets[0].as_ref().ok_or("no colour target")?;
    if t.format != TextureFormat::Rgba8UnormSrgb
        || t.write_mask != ColorWrites::ALL
        || d.multisample.count != 4
    {
        return Err("native sRGB/all lanes/MSAA4 required");
    }
    let z = d.depth_stencil.as_ref().ok_or("no scene depth")?;
    if z.format != TextureFormat::Depth32Float
        || z.depth_compare != Some(CompareFunction::GreaterEqual)
        || z.depth_write_enabled != Some(true)
    {
        return Err("Depth32/reverseLEQUAL/write required");
    }
    if forbidden(&d.vertex.shader_defs) || forbidden(&f.shader_defs) {
        return Err("unsupported auxiliary/alpha variant");
    }
    Ok(())
}
fn source_blend() -> BlendState {
    BlendState {
        color: BlendComponent {
            src_factor: BlendFactor::One,
            dst_factor: BlendFactor::Zero,
            operation: BlendOperation::Add,
        },
        alpha: BlendComponent {
            src_factor: BlendFactor::Zero,
            dst_factor: BlendFactor::Zero,
            operation: BlendOperation::Add,
        },
    }
}
fn remaps(defs: &[ShaderDefVal]) -> usize {
    defs.iter()
        .filter(|d| def_name(d) == "ALPHA_TO_COVERAGE")
        .count()
}
fn active_remap(defs: &[ShaderDefVal]) -> bool {
    remaps(defs) == 1
        && defs
            .iter()
            .any(|d| matches!(d,ShaderDefVal::Bool(name,true) if name=="ALPHA_TO_COVERAGE"))
}
fn specialize_raw(d: &mut RenderPipelineDescriptor) -> Result<(), &'static str> {
    attachment(d)?;
    let f = d.fragment.as_ref().unwrap();
    if f.shader != SHADER
        || !d.multisample.alpha_to_coverage_enabled
        || !active_remap(&d.vertex.shader_defs)
        || !active_remap(&f.shader_defs)
    {
        return Err("expected exact A2C source variant/custom fragment");
    }
    // ALL checks above; keep every other define, shader import, cull and bias unchanged.
    d.vertex
        .shader_defs
        .retain(|v| def_name(v) != "ALPHA_TO_COVERAGE");
    let f = d.fragment.as_mut().unwrap();
    f.shader_defs.retain(|v| def_name(v) != "ALPHA_TO_COVERAGE");
    f.targets[0].as_mut().unwrap().blend = Some(source_blend());
    Ok(())
}
pub(crate) fn actual_pipeline(d: &RenderPipelineDescriptor, raw: bool) -> bool {
    if attachment(d).is_err() {
        return false;
    }
    let f = d.fragment.as_ref().unwrap();
    remaps(&d.vertex.shader_defs) == 0
        && remaps(&f.shader_defs) == 0
        && if raw {
            f.shader == SHADER
                && d.multisample.alpha_to_coverage_enabled
                && f.targets[0].as_ref().unwrap().blend == Some(source_blend())
        } else {
            f.shader != SHADER
                && !d.multisample.alpha_to_coverage_enabled
                && f.targets[0].as_ref().unwrap().blend.is_none()
        }
}
// Pinned PBR emission is multiplied by output alpha. Do not compare coverage
// with different emission radiance between Opaque and raw-alpha arms.
pub(crate) fn zero_material_emission(base: &StandardMaterial) -> bool {
    base.emissive.red == 0.
        && base.emissive.green == 0.
        && base.emissive.blue == 0.
        && base.emissive.alpha.is_finite()
        && base.emissive_exposure_weight.is_finite()
        && base.emissive_texture.is_none()
}
pub(crate) fn clustered_scene_admitted(decals: &Query<(), With<ClusteredDecal>>) -> bool {
    // Reject even invisible/texture-less decals: this is an explicit clean scene,
    // not an inference that a particular decal cannot reach this material.
    decals.is_empty()
}
fn admitted(base: &StandardMaterial) -> bool {
    base.alpha_mode == AlphaMode::Opaque
        && zero_material_emission(base)
        && base.base_color.alpha().to_bits() == 1f32.to_bits()
        && base.base_color_texture.is_some()
        && base.specular_transmission == 0.
        && base.diffuse_transmission == 0.
}
fn raw_material(base: &StandardMaterial) -> RawMaterial {
    assert!(admitted(base));
    let mut base = base.clone();
    base.alpha_mode = AlphaMode::AlphaToCoverage;
    RawMaterial {
        base,
        extension: RawPass { marker: Vec4::ZERO },
    }
}
#[derive(Component, Clone)]
struct Backup {
    original: Handle<StandardMaterial>,
    replacement: Option<AssetId<RawMaterial>>,
    no_batch: bool,
    no_shadow: bool,
}
fn still_owned(
    b: &Backup,
    standard: Option<AssetId<StandardMaterial>>,
    raw: Option<AssetId<RawMaterial>>,
) -> bool {
    match b.replacement {
        Some(expected) => standard.is_none() && raw == Some(expected),
        None => raw.is_none() && standard == Some(b.original.id()),
    }
}
#[derive(Resource, Default)]
struct Restoring(Vec<(Entity, Backup)>);
fn restored(
    b: &Backup,
    standard: Option<AssetId<StandardMaterial>>,
    raw: Option<AssetId<RawMaterial>>,
    has_backup: bool,
    no_batch: bool,
    no_shadow: bool,
) -> bool {
    standard == Some(b.original.id())
        && raw.is_none()
        && !has_backup
        && no_batch == b.no_batch
        && no_shadow == b.no_shadow
}
pub(crate) fn install(app: &mut App, mode: Mode) -> Result<(), Box<dyn std::error::Error>> {
    if mode == Mode::Off {
        return Ok(());
    }
    if mode == Mode::RawA2c {
        app.world_mut()
            .resource_mut::<Assets<Shader>>()
            .insert(
                SHADER.id(),
                Shader::from_wgsl(
                    include_str!("foundation_backing.wgsl"),
                    "foundation_backing_raw.wgsl",
                ),
            )
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        app.add_plugins(MaterialPlugin::<RawMaterial>::default());
    }
    app.insert_resource(Fixture {
        mode,
        stopping: Arc::default(),
    })
    .add_systems(PostStartup, attach);
    install_restoration(app);
    super::foundation_backing_audit::install(app);
    Ok(())
}
// ScreenshotCaptured is delivered in Update; restore before Bevy's PostUpdate
// material/mesh change collectors, so the terminal render sees one material type.
fn install_restoration(app: &mut App) {
    app.init_resource::<Restoring>().add_systems(
        PostUpdate,
        (restore, verify_restored)
            .chain()
            // Both private material-to-mesh markers are AFTER this public anchor.
            .before(bevy::mesh::mark_3d_meshes_as_changed_if_their_assets_changed)
            .before(bevy::pbr::check_entities_needing_specialization::<StandardMaterial>)
            .before(bevy::pbr::check_entities_needing_specialization::<RawMaterial>),
    );
}
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn attach(
    mut commands: Commands,
    fixture: Res<Fixture>,
    source: Res<Assets<StandardMaterial>>,
    meshes: Res<Assets<Mesh>>,
    mut extended: Option<ResMut<Assets<RawMaterial>>>,
    cameras: Query<&Msaa, With<super::ViewerCamera>>,
    decals: Query<(), With<ClusteredDecal>>,
    parts: Query<(
        Entity,
        &super::track_surface::SourcePart,
        &Mesh3d,
        &MeshMaterial3d<StandardMaterial>,
        Has<NoAutomaticBatching>,
        Has<NotShadowCaster>,
        Option<&Backup>,
    )>,
) {
    assert!(
        clustered_scene_admitted(&decals),
        "backing requires no main-world ClusteredDecal"
    );
    assert_eq!(
        *cameras.single().expect("one backing main camera"),
        Msaa::Sample4
    );
    let rows: Vec<_> = parts.iter().filter(|(_, p, ..)| p.0 == TARGET).collect();
    assert!(
        (1..=128).contains(&rows.len()),
        "exact backing target count bound"
    );
    for (_, _, mesh, material, _, _, backup) in &rows {
        assert!(backup.is_none(), "duplicate backing installation");
        assert!(
            admitted(source.get(&material.0).expect("backing source asset")),
            "backing source alpha/material contract"
        );
        let mesh = meshes.get(&mesh.0).expect("backing CPU mesh");
        assert!(
            mesh.attribute(Mesh::ATTRIBUTE_UV_0).is_some()
                && mesh.attribute(Mesh::ATTRIBUTE_COLOR).is_none(),
            "backing source UV/no vertex-alpha ambiguity"
        );
    }
    // Prepare all clones before scheduling any entity/component change.
    let mut handles: HashMap<AssetId<StandardMaterial>, Handle<RawMaterial>> = HashMap::new();
    if fixture.mode == Mode::RawA2c {
        let assets = extended.as_mut().expect("backing material plugin");
        for (_, _, _, material, ..) in &rows {
            handles
                .entry(material.0.id())
                .or_insert_with(|| assets.add(raw_material(source.get(&material.0).unwrap())));
        }
    }
    for (entity, _, _, material, no_batch, no_shadow, _) in &rows {
        let replacement = handles.get(&material.0.id());
        commands.entity(*entity).insert((
            NoAutomaticBatching,
            NotShadowCaster,
            Backup {
                original: material.0.clone(),
                replacement: replacement.map(Handle::id),
                no_batch: *no_batch,
                no_shadow: *no_shadow,
            },
        ));
        if let Some(handle) = replacement {
            commands
                .entity(*entity)
                .remove::<MeshMaterial3d<StandardMaterial>>()
                .insert(MeshMaterial3d(handle.clone()));
        }
    }
    println!(
        "backing_attach mode={:?} selected={} replaced={} matched_no_auto=true matched_no_shadow=true material_emission=zero emissive_texture=false main_clustered_decals=0 control_neutral_to_off=false geometry_texture_front_fence=unchanged scope=components_not_draws",
        fixture.mode,
        rows.len(),
        if fixture.mode == Mode::RawA2c {
            rows.len()
        } else {
            0
        }
    );
}
#[allow(clippy::type_complexity)]
fn restore(
    mut commands: Commands,
    mut exits: MessageReader<AppExit>,
    fixture: Res<Fixture>,
    mut pending: ResMut<Restoring>,
    parts: Query<(
        Entity,
        &Backup,
        Option<&MeshMaterial3d<StandardMaterial>>,
        Option<&MeshMaterial3d<RawMaterial>>,
    )>,
) {
    if fixture.stopping.load(Ordering::Acquire) || exits.read().next().is_none() {
        return;
    }
    fixture.stopping.store(true, Ordering::Release); // Observer stops BEFORE deferred restoration.
    for (_, b, standard, raw) in &parts {
        assert!(
            still_owned(b, standard.map(|h| h.0.id()), raw.map(|h| h.0.id())),
            "backing foreign material switch: will not overwrite"
        );
    }
    pending.0 = parts
        .iter()
        .map(|(entity, backup, _, _)| (entity, backup.clone()))
        .collect();
    let mut count = 0;
    for (entity, b, _, _) in &parts {
        let mut e = commands.entity(entity);
        e.remove::<MeshMaterial3d<RawMaterial>>()
            .insert(MeshMaterial3d(b.original.clone()))
            .remove::<Backup>();
        if !b.no_batch {
            e.remove::<NoAutomaticBatching>();
        }
        if !b.no_shadow {
            e.remove::<NotShadowCaster>();
        }
        count += 1;
    }
    println!(
        "backing_restore scheduled=true entities={count} original_handles_and_marker_presence=true note=deferred_commands_not_postapply_proof"
    );
}
#[allow(clippy::type_complexity)]
fn verify_restored(
    mut pending: ResMut<Restoring>,
    parts: Query<(
        Option<&MeshMaterial3d<StandardMaterial>>,
        Option<&MeshMaterial3d<RawMaterial>>,
        Has<Backup>,
        Has<NoAutomaticBatching>,
        Has<NotShadowCaster>,
    )>,
) {
    if pending.0.is_empty() {
        return;
    }
    let mut restored_count = 0;
    let mut despawned = 0;
    for (entity, b) in &pending.0 {
        if let Ok((standard, raw, backup, no_batch, no_shadow)) = parts.get(*entity) {
            assert!(
                restored(
                    b,
                    standard.map(|h| h.0.id()),
                    raw.map(|h| h.0.id()),
                    backup,
                    no_batch,
                    no_shadow
                ),
                "backing post-apply restoration failed"
            );
            restored_count += 1;
        } else {
            despawned += 1;
        }
    }
    println!(
        "backing_restore complete=true original_handles_and_marker_presence=true restored={restored_count} despawned={despawned} stage=after_deferred_apply"
    );
    pending.0.clear();
}
#[cfg(test)]
mod tests {
    use super::*;
    fn base() -> StandardMaterial {
        StandardMaterial {
            base_color_texture: Some(Handle::default()),
            ..default()
        }
    }
    fn pipe() -> RenderPipelineDescriptor {
        let defs = vec![
            "ALPHA_TO_COVERAGE".into(),
            "VERTEX_UVS".into(),
            ShaderDefVal::UInt("MAX_LIGHTS".into(), 16),
        ];
        let mut d = RenderPipelineDescriptor {
            vertex: VertexState {
                shader_defs: defs.clone(),
                ..default()
            },
            fragment: Some(FragmentState {
                shader: SHADER,
                shader_defs: defs,
                targets: vec![Some(ColorTargetState {
                    format: TextureFormat::Rgba8UnormSrgb,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })],
                ..default()
            }),
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(CompareFunction::GreaterEqual),
                stencil: default(),
                bias: default(),
            }),
            ..default()
        };
        d.multisample.count = 4;
        d.multisample.alpha_to_coverage_enabled = true;
        d
    }
    #[test]
    fn coverage_comparison_rejects_emission_texture_nonfinite_and_hidden_decals() {
        let old = base();
        assert!(admitted(&old));
        for component in 0..3 {
            for value in [1., -1., f32::NAN, f32::INFINITY] {
                let mut bad = old.clone();
                match component {
                    0 => bad.emissive.red = value,
                    1 => bad.emissive.green = value,
                    _ => bad.emissive.blue = value,
                }
                assert!(!admitted(&bad));
            }
        }
        let mut bad = old.clone();
        bad.emissive_texture = Some(Handle::default());
        assert!(!admitted(&bad));
        let mut bad = old.clone();
        bad.emissive_exposure_weight = f32::INFINITY;
        assert!(!admitted(&bad));
        let mut bad = old;
        bad.emissive.alpha = f32::NAN;
        assert!(!admitted(&bad));
        // Same typed check is used on both worlds; hidden decals are not ignored.
        let mut world = World::new();
        let mut state =
            bevy::ecs::system::SystemState::<Query<(), With<ClusteredDecal>>>::new(&mut world);
        assert!(clustered_scene_admitted(&state.get(&world).expect("fixture query validated")));
        let decal = world
            .spawn((ClusteredDecal::default(), Visibility::Hidden))
            .id();
        assert!(!clustered_scene_admitted(&state.get(&world).expect("fixture query validated")));
        world.despawn(decal);
        assert!(clustered_scene_admitted(&state.get(&world).expect("fixture query validated")));
    }
    #[test]
    fn real_deferred_restore_preserves_original_markers_in_all_four_combinations() {
        let mut app = App::new();
        app.add_message::<AppExit>();
        app.insert_resource(Fixture {
            mode: Mode::Control,
            stopping: Arc::default(),
        });
        install_restoration(&mut app);
        let original = Handle::<StandardMaterial>::default();
        let mut expected = Vec::new();
        for no_batch in [false, true] {
            for no_shadow in [false, true] {
                let entity = app
                    .world_mut()
                    .spawn((
                        MeshMaterial3d(original.clone()),
                        NoAutomaticBatching,
                        NotShadowCaster,
                        Backup {
                            original: original.clone(),
                            replacement: None,
                            no_batch,
                            no_shadow,
                        },
                    ))
                    .id();
                expected.push((entity, no_batch, no_shadow));
            }
        }
        let _ = app.world_mut().write_message(AppExit::Success);
        app.update();
        for (entity, no_batch, no_shadow) in expected {
            let e = app.world().entity(entity);
            assert_eq!(
                e.get::<MeshMaterial3d<StandardMaterial>>().unwrap().0.id(),
                original.id()
            );
            assert_eq!(e.contains::<NoAutomaticBatching>(), no_batch);
            assert_eq!(e.contains::<NotShadowCaster>(), no_shadow);
            assert!(!e.contains::<Backup>());
            assert!(!e.contains::<MeshMaterial3d<RawMaterial>>());
        }
        assert!(app.world().resource::<Restoring>().0.is_empty());
    }
    #[derive(Resource, Default)]
    struct DeliverCapture(Option<Entity>);
    fn deliver_capture_in_update(mut commands: Commands, mut delivery: ResMut<DeliverCapture>) {
        if let Some(entity) = delivery.0.take() {
            // CPU-delivered actual event type, not a claimed GPU image/readback.
            commands.trigger(bevy::render::view::screenshot::ScreenshotCaptured {
                entity,
                image: Image::default(),
            });
        }
    }
    #[derive(Resource, Default, Debug)]
    struct CollectorEvidence {
        observed: bool,
        standard_present: bool,
        raw_present: bool,
        std_changed: bool,
        raw_removed: bool,
        raw_changed: bool,
        mesh_changed: bool,
    }
    #[derive(Resource)]
    struct CheckedEntity(Entity);
    #[allow(clippy::type_complexity)]
    fn observe_native_collectors(
        fixture: Res<Fixture>,
        checked: Res<CheckedEntity>,
        standards: Res<bevy::pbr::EntitiesNeedingSpecialization<StandardMaterial>>,
        raws: Res<bevy::pbr::EntitiesNeedingSpecialization<RawMaterial>>,
        parts: Query<(
            Ref<Mesh3d>,
            Has<MeshMaterial3d<StandardMaterial>>,
            Has<MeshMaterial3d<RawMaterial>>,
        )>,
        mut evidence: ResMut<CollectorEvidence>,
    ) {
        // This real ECS system runs on both warmup updates as well, establishing
        // its own last-run tick before checking native Mesh3d change detection.
        if !fixture.stopping.load(Ordering::Acquire) {
            return;
        }
        let (mesh, standard_present, raw_present) = parts.get(checked.0).unwrap();
        *evidence = CollectorEvidence {
            observed: true,
            standard_present,
            raw_present,
            std_changed: standards.changed.contains(&checked.0),
            raw_removed: raws.removed.contains(&checked.0),
            raw_changed: raws.changed.contains(&checked.0),
            mesh_changed: mesh.is_changed(),
        };
    }
    #[test]
    fn actual_material_collectors_see_postupdate_restore_but_miss_old_last_restore() {
        use bevy::{asset::AssetPlugin, mesh::MeshPlugin};
        for earlier in [false, true] {
            for error_exit in [false, true] {
                let mut app = App::new();
                app.add_plugins((
                    MinimalPlugins,
                    AssetPlugin::default(),
                    MeshPlugin,
                    MaterialPlugin::<StandardMaterial>::default(),
                    MaterialPlugin::<RawMaterial>::default(),
                ));
                assert!(app.get_sub_app(bevy::render::RenderApp).is_none());
                app.insert_resource(Fixture {
                    mode: Mode::RawA2c,
                    stopping: Arc::default(),
                })
                .init_resource::<CollectorEvidence>()
                .init_resource::<DeliverCapture>()
                .add_systems(Update, deliver_capture_in_update);
                if earlier {
                    install_restoration(&mut app);
                    app.add_systems(Last, observe_native_collectors);
                } else {
                    // Exact previous lifecycle is the negative control. REAL
                    // MaterialPlugin collectors still run in PostUpdate.
                    app.init_resource::<Restoring>().add_systems(
                        Last,
                        (restore, verify_restored, observe_native_collectors).chain(),
                    );
                }
                let original = app
                    .world_mut()
                    .resource_mut::<Assets<StandardMaterial>>()
                    .add(base());
                let raw = app
                    .world_mut()
                    .resource_mut::<Assets<RawMaterial>>()
                    .add(raw_material(&base()));
                let mesh = app
                    .world_mut()
                    .resource_mut::<Assets<Mesh>>()
                    .add(Mesh::new(
                        PrimitiveTopology::TriangleList,
                        bevy::asset::RenderAssetUsages::MAIN_WORLD,
                    ));
                let entity = app
                    .world_mut()
                    .spawn((
                        Mesh3d(mesh),
                        MeshMaterial3d(raw.clone()),
                        NoAutomaticBatching,
                        NotShadowCaster,
                        Backup {
                            original: original.clone(),
                            replacement: Some(raw.id()),
                            no_batch: false,
                            no_shadow: false,
                        },
                    ))
                    .id();
                app.insert_resource(CheckedEntity(entity));
                let capture = app
                    .world_mut()
                    .spawn_empty()
                    .observe(
                        move |_: On<bevy::render::view::screenshot::ScreenshotCaptured>,
                              mut exits: MessageWriter<AppExit>| {
                            exits.write(if error_exit {
                                AppExit::error()
                            } else {
                                AppExit::Success
                            });
                        },
                    )
                    .id();
                app.finish();
                app.cleanup();
                app.update();
                app.update();
                assert!(!app.world().resource::<CollectorEvidence>().observed);
                app.world_mut().resource_mut::<DeliverCapture>().0 = Some(capture);
                app.update();
                let e = app.world().resource::<CollectorEvidence>();
                assert!(e.observed && e.standard_present && !e.raw_present, "{e:?}");
                assert_eq!(e.std_changed, earlier, "actual Std collector: {e:?}");
                assert_eq!(
                    e.raw_removed, earlier,
                    "actual Raw removal collector: {e:?}"
                );
                assert!(!e.raw_changed, "removed Raw is not still active: {e:?}");
                assert_eq!(
                    e.mesh_changed, earlier,
                    "actual private material mesh-marker: {e:?}"
                );
                let part = app.world().entity(entity);
                assert_eq!(
                    part.get::<MeshMaterial3d<StandardMaterial>>()
                        .unwrap()
                        .0
                        .id(),
                    original.id()
                );
                assert!(
                    !part.contains::<Backup>()
                        && !part.contains::<NoAutomaticBatching>()
                        && !part.contains::<NotShadowCaster>()
                );
                assert!(app.world().resource::<Restoring>().0.is_empty());
                let exit = app
                    .should_exit()
                    .expect("actual Update observer delivered AppExit");
                assert_eq!(exit.is_error(), error_exit);
            }
        }
    }
    #[test]
    fn off_has_no_resources_and_active_mode_requires_exact_composite_and_common_guards() {
        let mut app = App::new();
        install(&mut app, Mode::Off).unwrap();
        assert!(!app.world().contains_resource::<Fixture>());
        assert!(!app.world().contains_resource::<Assets<RawMaterial>>());
        assert_eq!(Mode::default(), Mode::Off);
        assert!(Mode::parse("blend").is_err());
        let c = super::super::foundation_front::Context {
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
        };
        for mode in [Mode::Control, Mode::RawA2c] {
            assert!(
                mode.validate(c, super::super::foundation_front::Mode::SourceFront)
                    .is_ok()
            );
            assert!(
                mode.validate(c, super::super::foundation_front::Mode::Off)
                    .is_err()
            );
            assert!(
                mode.validate(
                    super::super::foundation_front::Context { drive: true, ..c },
                    super::super::foundation_front::Mode::SourceFront
                )
                .is_err()
            );
        }
    }
    #[test]
    fn raw_pipeline_removes_only_exact_remap_and_preserves_cull_bias_shaders() {
        let mut d = pipe();
        let mut expected = d.clone();
        expected.vertex.shader_defs.remove(0);
        expected.fragment.as_mut().unwrap().shader_defs.remove(0);
        expected.fragment.as_mut().unwrap().targets[0]
            .as_mut()
            .unwrap()
            .blend = Some(source_blend());
        specialize_raw(&mut d).unwrap();
        assert_eq!(d, expected);
        assert!(actual_pipeline(&d, true));
        assert!(!actual_pipeline(&d, false));
        let mut wrong = d;
        wrong.vertex.shader_defs.push("PREMULTIPLY_ALPHA".into());
        assert!(!actual_pipeline(&wrong, true));
    }
    #[test]
    fn unsupported_variant_fails_before_mutation() {
        let mut cases = Vec::new();
        let mut d = pipe();
        d.multisample.count = 1;
        cases.push(d);
        let mut d = pipe();
        d.fragment.as_mut().unwrap().targets[0]
            .as_mut()
            .unwrap()
            .format = TextureFormat::Rgba16Float;
        cases.push(d);
        let mut d = pipe();
        d.depth_stencil = None;
        cases.push(d);
        let mut d = pipe();
        d.vertex.shader_defs.push("MAY_DISCARD".into());
        cases.push(d);
        let mut d = pipe();
        d.vertex.shader_defs[0] = ShaderDefVal::Bool("ALPHA_TO_COVERAGE".into(), false);
        cases.push(d);
        let mut d = pipe();
        d.fragment.as_mut().unwrap().targets.push(None);
        cases.push(d);
        let mut d = pipe();
        d.multisample.alpha_to_coverage_enabled = false;
        cases.push(d);
        let mut d = pipe();
        d.depth_stencil.as_mut().unwrap().depth_write_enabled = Some(false);
        cases.push(d);
        let mut d = pipe();
        d.fragment.as_mut().unwrap().shader_defs.clear();
        cases.push(d);
        let mut d = pipe();
        d.fragment
            .as_mut()
            .unwrap()
            .shader_defs
            .push("ALPHA_TO_COVERAGE".into());
        cases.push(d);
        for mut d in cases {
            let old = d.clone();
            assert!(specialize_raw(&mut d).is_err());
            assert_eq!(d, old);
        }
    }
    #[test]
    fn material_clone_only_routes_a2c_and_retains_original_asset() {
        let old = base();
        let text = format!("{old:?}");
        let mut new = raw_material(&old);
        assert_eq!(new.base.alpha_mode, AlphaMode::AlphaToCoverage);
        new.base.alpha_mode = AlphaMode::Opaque;
        assert_eq!(format!("{:?}", new.base), text);
        assert_eq!(format!("{old:?}"), text);
        let mut bad = old;
        bad.base_color = Color::srgba(1., 1., 1., 0.5);
        assert!(!admitted(&bad));
    }
    #[test]
    fn restoration_identity_and_original_marker_combinations() {
        let original = Handle::<StandardMaterial>::default();
        let replacement = Handle::<RawMaterial>::default();
        for no_batch in [false, true] {
            for no_shadow in [false, true] {
                let b = Backup {
                    original: original.clone(),
                    replacement: Some(replacement.id()),
                    no_batch,
                    no_shadow,
                };
                assert!(still_owned(&b, None, Some(replacement.id())));
                assert!(!still_owned(
                    &b,
                    Some(original.id()),
                    Some(replacement.id())
                ));
                assert!(!still_owned(&b, None, None));
                assert!(restored(
                    &b,
                    Some(original.id()),
                    None,
                    false,
                    no_batch,
                    no_shadow
                ));
                assert!(!restored(
                    &b,
                    Some(original.id()),
                    None,
                    true,
                    no_batch,
                    no_shadow
                ));
                assert!(!restored(
                    &b,
                    Some(original.id()),
                    None,
                    false,
                    !no_batch,
                    no_shadow
                ));
                assert!(!restored(
                    &b,
                    Some(original.id()),
                    None,
                    false,
                    no_batch,
                    !no_shadow
                ));
                let b = Backup {
                    replacement: None,
                    ..b
                };
                assert!(still_owned(&b, Some(original.id()), None));
                assert!(!still_owned(&b, None, None));
            }
        }
    }
}
