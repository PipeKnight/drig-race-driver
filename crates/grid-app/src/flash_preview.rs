//! Default-off bounded Detroit static flash diagnostic adapter.
//! Source texture/FP blend contract, authored axis, attachment, index and frozen-phase fixture.
//! No source scheduler hookup, point/spot/scene lights, animation or live parity claim.
use super::flash_fixture::{self, Anchor};
use bevy::{
    asset::{RenderAssetUsages, uuid_handle},
    light::NotShadowCaster,
    mesh::{Indices, MeshVertexBufferLayoutRef, PrimitiveTopology},
    pbr::{Material, MaterialPipeline, MaterialPipelineKey},
    prelude::*,
    render::render_resource::{
        AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, CompareFunction,
        RenderPipelineDescriptor, SpecializedMeshPipelineError,
    },
    shader::ShaderRef,
};
use grid_formats::{pssg, texture};
use std::{error::Error, path::Path};
type AnyResult<T> = Result<T, Box<dyn Error>>;
const SHADER: Handle<Shader> = uuid_handle!("34f819e9-2955-4f18-8466-311dac26e356");
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Off,
    Control,
    FrozenPulse,
}
impl Mode {
    pub fn parse(s: &str) -> Result<Self, &'static str> {
        match s {
            "off" => Ok(Self::Off),
            "control" => Ok(Self::Control),
            "frozen-pulse" => Ok(Self::FrozenPulse),
            _ => Err("flash fixture needs off,control,frozen-pulse"),
        }
    }
    // Caller must first run existing crowd_preview::Mode::validate. Additional
    // render/route/frame/MSAA guards remain independent of optional owner feedback.
    pub fn validate(
        self,
        crowd: super::crowd_preview::Mode,
        frames: Option<u32>,
        msaa4: bool,
        reflections_sky: bool,
    ) -> Result<(), &'static str> {
        if self != Self::Off
            && !(crowd == super::crowd_preview::Mode::StaticBillboards
                && frames.is_some_and(|n| (1..=600).contains(&n))
                && msaa4
                && reflections_sky)
        {
            return Err(
                "flash fixture requires admitted bounded Detroit static crowd MSAA4 context",
            );
        };
        Ok(())
    }
}
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct FlashMaterial {
    #[texture(0)]
    #[sampler(1)]
    map: Handle<Image>,
}
impl Material for FlashMaterial {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }
    // Routing/sorting only; specialize supplies exact non-premultiplied source blend.
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn specialize(
        _: &MaterialPipeline,
        d: &mut RenderPipelineDescriptor,
        _: &MeshVertexBufferLayoutRef,
        _: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let fragment = d
            .fragment
            .as_mut()
            .expect("bounded flash needs fragment stage");
        assert_eq!(
            fragment.targets.len(),
            1,
            "bounded flash needs one colour target"
        );
        let target = fragment.targets[0]
            .as_mut()
            .expect("bounded flash colour target");
        let component = BlendComponent {
            src_factor: BlendFactor::SrcAlpha,
            dst_factor: BlendFactor::One,
            operation: BlendOperation::Add,
        };
        target.blend = Some(BlendState {
            color: component,
            alpha: component,
        });
        let depth = d
            .depth_stencil
            .as_mut()
            .expect("bounded flash needs scene depth");
        // Bevy reverse-Z GreaterEqual corresponds to saved conventional LEQUAL.
        depth.depth_compare = Some(CompareFunction::GreaterEqual);
        depth.depth_write_enabled = Some(true);
        d.multisample.alpha_to_coverage_enabled = false;
        // Authored double-sided index fixture, not recovered saved CW winding.
        d.primitive.cull_mode = None;
        Ok(())
    }
}
#[derive(Resource)]
pub struct Prepared {
    mode: Mode,
    dds: Vec<u8>,
    anchors: Vec<Anchor>,
}
// Caller supplies <=10 actual selected m_gr/f_gr source origins AFTER existing
// USA definitions' hasCamera check; attachment offset below is explicitly authored.
pub fn prepare(root: &Path, mode: Mode, anchors: &[Anchor]) -> AnyResult<Prepared> {
    if mode == Mode::Off {
        return Ok(Prepared {
            mode,
            dds: Vec::new(),
            anchors: Vec::new(),
        });
    }
    if anchors.len() != 3 {
        return Err("frozen flash fixture needs exactly three source anchors".into());
    }
    if anchors.iter().any(|a| {
        a.source_cell >= 106
            || a.source_index >= 4667
            || !a.position.into_iter().all(f32::is_finite)
    }) {
        return Err("unsupported flash source anchor".into());
    }
    if anchors.iter().enumerate().any(|(i, a)| {
        anchors[..i]
            .iter()
            .any(|b| b.source_index == a.source_index)
    }) {
        return Err("duplicate diagnostic flash source anchor".into());
    }
    let path = root.join("anims/cr_camera_flash.pssg");
    if std::fs::metadata(&path)?.len() > 1024 * 1024 {
        return Err("flash resource size limit".into());
    }
    let raw = std::fs::read(path)?;
    let tree = pssg::read_tree(&raw)?;
    let textures = texture::read_named(&tree, &[b"flash3.tga"])?;
    if textures.len() != 1 {
        return Err("flash source texture ambiguity".into());
    }
    let t = &textures[0];
    if t.width != 128 || t.height != 128 || t.format != texture::Format::Bc3 {
        return Err("flash source texture changed".into());
    }
    let (dds, levels) = t.validated_dds();
    // Saved declared7 means levels AFTER base. Require full validated128 chain.
    if levels != 8 {
        return Err("flash full source mip chain not admitted".into());
    }
    Ok(Prepared {
        mode,
        dds,
        anchors: anchors.to_vec(),
    })
}
pub fn install(app: &mut App, prepared: Prepared) -> AnyResult<()> {
    println!(
        "crowd_flash_fixture mode={:?} source_anchors={} phase=0.5 geometry=authored-unit-up-w0 attachment=source-origin-plus-authored-y1.2 scheduler=frozen colourspace=authored-srgb shader=raw-rgb-alpha-times-opacity blend=SrcAlpha-One depth_write=true reverse_z=GreaterEqual cull=authored-none emitted_lights=0 source_parity=false",
        prepared.mode,
        prepared.anchors.len()
    );
    if prepared.mode != Mode::FrozenPulse {
        return Ok(());
    }
    app.world_mut().resource_mut::<Assets<Shader>>().insert(
        SHADER.id(),
        Shader::from_wgsl(
            include_str!("flash_preview.wgsl"),
            "grid_flash_fixture.wgsl",
        ),
    )?;
    app.insert_resource(prepared)
        .add_plugins(MaterialPlugin::<FlashMaterial>::default())
        .add_systems(PostStartup, spawn);
    Ok(())
}
#[allow(clippy::too_many_arguments)] // Bevy ECS system keeps its explicit bounded view, asset and fixture resources.
fn spawn(
    mut commands: Commands,
    p: Res<Prepared>,
    viewer: Query<(&Transform, &Msaa), With<super::ViewerCamera>>,
    cameras: Query<&Camera, With<Camera3d>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<FlashMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    settings: Res<super::graphics::Settings>,
) {
    // Restrict to one active main3D view: no accidental envmap/reflection capture.
    assert_eq!(
        cameras.iter().filter(|c| c.is_active).count(),
        1,
        "flash fixture main-view only"
    );
    let (view, msaa) = viewer.single().expect("one admitted flash viewer");
    assert_eq!(*msaa, Msaa::Sample4);
    let image = super::texture_upload::image(
        &p.dds,
        settings.sampler(false),
        RenderAssetUsages::default(),
    )
    .expect("validated flash DDS");
    let handle = materials.add(FlashMaterial {
        map: images.add(image),
    });
    for anchor in &p.anchors {
        let mut attached = *anchor;
        attached.position[1] += 1.2; // AUTHORED sitting-camera fixture offset; no node binding claim.
        let quad = flash_fixture::quad(attached, view.translation.to_array(), [0., 1., 0.], 0.5)
            .expect("validated authored flash card");
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
        // Authored event-centred local mesh representation. Bevy0.19 sorts
        // transformed mesh AABB centres; static world-baked cards would also
        // have valid centres. This transform is not recovered source ABI.
        let center = Vec3::from_array(attached.position);
        let local = quad
            .positions
            .map(|v| (Vec3::from_array(v) - center).to_array());
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, local.to_vec());
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0., 0., 1.]; 4]);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, quad.uv.to_vec());
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[1., 1., 1., quad.opacity]; 4]);
        mesh.insert_indices(Indices::U32(vec![0, 1, 2, 1, 3, 2]));
        commands.spawn((
            Name::new(format!(
                "flash-fixture/cell{}/source{}",
                anchor.source_cell, anchor.source_index
            )),
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(handle.clone()),
            Transform::from_translation(center),
            NotShadowCaster,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires purchased GRID_GAME_ROOT; CPU only, no GPU"]
    fn purchased_source_flash_control_prepares_exact_three_anchors_and_full_dds() {
        let root = std::path::PathBuf::from(
            std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required"),
        );
        let route = grid_tools::route::Route::new("tracks/usa/detroit/route_8").unwrap();
        let crowd = super::super::crowd_preview::prepare(
            &root,
            &route,
            super::super::crowd_preview::Mode::StaticBillboards,
        )
        .unwrap();
        let anchors = super::super::crowd_preview::flash_fixture_anchors(&root, &crowd).unwrap();
        let p = prepare(&root, Mode::Control, &anchors).unwrap();
        assert_eq!(p.mode, Mode::Control);
        assert_eq!(p.anchors.len(), 3);
        assert!(p.dds.starts_with(b"DDS "));
    }

    #[test]
    fn admission_is_default_off_and_requires_each_static_context_guard() {
        use super::super::crowd_preview::Mode as Crowd;
        assert_eq!(Mode::default(), Mode::Off);
        assert!(Mode::Off.validate(Crowd::Off, None, false, false).is_ok());
        for mode in [Mode::Control, Mode::FrozenPulse] {
            assert!(
                mode.validate(Crowd::StaticBillboards, Some(120), true, true)
                    .is_ok()
            );
            for (crowd, frames, msaa, sky) in [
                (Crowd::Off, Some(120), true, true),
                (Crowd::Control, Some(120), true, true),
                (Crowd::StaticBillboards, None, true, true),
                (Crowd::StaticBillboards, Some(0), true, true),
                (Crowd::StaticBillboards, Some(601), true, true),
                (Crowd::StaticBillboards, Some(120), false, true),
                (Crowd::StaticBillboards, Some(120), true, false),
            ] {
                assert!(mode.validate(crowd, frames, msaa, sky).is_err());
            }
        }
    }
}
