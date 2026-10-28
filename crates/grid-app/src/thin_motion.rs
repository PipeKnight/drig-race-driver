//! Optional velocity-only conservative raster for the player's thin antenna.
//! Visible geometry, color and depth remain owned by the ordinary PBR passes.
use bevy::{
    asset::{AssetId, uuid_handle},
    core_pipeline::{
        Core3dSystems,
        prepass::{PreviousViewData, ViewPrepassTextures},
        schedule::Core3d,
    },
    mesh::{VertexAttributeValues, VertexBufferLayout},
    prelude::*,
    render::{
        RenderApp, RenderStartup,
        camera::TemporalJitter,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_resource::{binding_types::uniform_buffer, *},
        renderer::{RenderContext, RenderDevice, RenderQueue, ViewQuery},
        view::{ExtractedView, ViewDepthTexture},
    },
};
use std::sync::Arc;
const SHADER: Handle<Shader> = uuid_handle!("69b1964a-6d3c-4a6d-94f7-b981d7b4e946");
#[derive(Clone)]
struct Geometry {
    vertices: Vec<[f32; 3]>,
    indices: Vec<u32>,
}
#[derive(Resource, Clone, Default, ExtractResource)]
struct Source {
    key: Option<AssetId<Mesh>>,
    visible: bool,
    geometry: Option<Arc<Geometry>>,
    current: Mat4,
    previous: Mat4,
}
#[derive(Clone, Copy, ShaderType)]
struct Parameters {
    jittered_clip: Mat4,
    current_clip: Mat4,
    previous_clip: Mat4,
    current_model: Mat4,
    previous_model: Mat4,
}
#[derive(Resource)]
struct Pipeline {
    layout: BindGroupLayoutDescriptor,
    id: CachedRenderPipelineId,
}
#[derive(Default)]
struct Buffers {
    key: Option<AssetId<Mesh>>,
    vertices: Option<Buffer>,
    indices: Option<Buffer>,
    count: u32,
}
type ViewInputs = (
    &'static ExtractedView,
    &'static ViewPrepassTextures,
    &'static ViewDepthTexture,
    Option<&'static TemporalJitter>,
    Option<&'static PreviousViewData>,
    Has<super::native_dlaa::DlaaView>,
);

pub fn install(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    app.world_mut().resource_mut::<Assets<Shader>>().insert(
        SHADER.id(),
        Shader::from_wgsl(include_str!("thin_motion.wgsl"), "thin_motion.wgsl"),
    )?;
    app.init_resource::<Source>()
        .add_plugins(ExtractResourcePlugin::<Source>::default())
        .add_systems(
            PostUpdate,
            snapshot
                .after(bevy::transform::TransformSystems::Propagate)
                .after(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate),
        );
    if let Some(render) = app.get_sub_app_mut(RenderApp) {
        render.add_systems(RenderStartup, initialize).add_systems(
            Core3d,
            draw.after(Core3dSystems::Prepass)
                .before(Core3dSystems::MainPass),
        );
    }
    Ok(())
}
type AntennaInputs = (
    &'static super::track_surface::SourcePart,
    &'static Mesh3d,
    &'static GlobalTransform,
    &'static InheritedVisibility,
);
fn snapshot(mut source: ResMut<Source>, meshes: Res<Assets<Mesh>>, parts: Query<AntennaInputs>) {
    let Some((_, mesh, transform, visibility)) = parts
        .iter()
        .find(|(part, _, _, _)| part.0 == "car0/Aerials")
    else {
        source.visible = false;
        source.key = None;
        return;
    };
    source.visible = false;
    let current = transform.to_matrix();
    if !current.is_finite() {
        return;
    }
    if source.key != Some(mesh.0.id()) {
        let Some(mesh_asset) = meshes.get(&mesh.0) else {
            return;
        };
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh_asset.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            return;
        };
        let Some(indices) = mesh_asset.indices() else {
            return;
        };
        let indices: Vec<u32> = indices.iter().map(|i| i as u32).collect();
        if positions.is_empty()
            || positions.len() > 65536
            || indices.len() > 150000
            || !indices.len().is_multiple_of(3)
            || positions.iter().flatten().any(|v| !v.is_finite())
            || indices.iter().any(|&i| i as usize >= positions.len())
        {
            return;
        }
        source.geometry = Some(Arc::new(Geometry {
            vertices: positions.clone(),
            indices,
        }));
        source.key = Some(mesh.0.id());
        source.previous = current;
    } else {
        source.previous = source.current;
    }
    source.current = current;
    source.visible = visibility.get();
}
fn initialize(mut commands: Commands, device: Res<RenderDevice>, cache: Res<PipelineCache>) {
    if !device
        .features()
        .contains(WgpuFeatures::CONSERVATIVE_RASTERIZATION)
    {
        println!("thin_motion active=false reason=device-conservative-raster-unavailable");
        return;
    }
    let layout = BindGroupLayoutDescriptor::new(
        "thin antenna motion",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::VERTEX,
            (uniform_buffer::<Parameters>(false),),
        ),
    );
    let id = cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("antenna motion only conservative".into()),
        layout: vec![layout.clone()],
        vertex: VertexState {
            shader: SHADER,
            buffers: vec![VertexBufferLayout {
                array_stride: 12,
                step_mode: VertexStepMode::Vertex,
                attributes: vec![VertexAttribute {
                    format: VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                }],
            }],
            ..default()
        },
        fragment: Some(FragmentState {
            shader: SHADER,
            targets: vec![Some(ColorTargetState {
                format: TextureFormat::Rg16Float,
                blend: None,
                write_mask: ColorWrites::ALL,
            })],
            ..default()
        }),
        primitive: PrimitiveState {
            conservative: true,
            cull_mode: None,
            ..default()
        },
        depth_stencil: Some(DepthStencilState {
            format: TextureFormat::Depth32Float,
            depth_write_enabled: Some(false),
            depth_compare: Some(CompareFunction::GreaterEqual),
            stencil: default(),
            bias: default(),
        }),
        ..default()
    });
    commands.insert_resource(Pipeline { layout, id });
}
#[allow(clippy::too_many_arguments)]
fn draw(
    view: ViewQuery<ViewInputs>,
    source: Res<Source>,
    pipeline: Option<Res<Pipeline>>,
    cache: Res<PipelineCache>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    mut ctx: RenderContext,
    mut buffers: Local<Buffers>,
    mut reported: Local<bool>,
) {
    let Some(pipeline) = pipeline else { return };
    let Some(compiled) = cache.get_render_pipeline(pipeline.id) else {
        return;
    };
    let (view, prepass, view_depth, jitter, previous, dlaa) = view.into_inner();
    if !dlaa || !source.visible {
        return;
    }
    let (Some(motion), Some(previous), Some(geometry)) = (
        prepass.motion_vectors_view(),
        previous,
        source.geometry.as_ref(),
    ) else {
        return;
    };
    if buffers.key != source.key {
        let vertices: Vec<u8> = geometry
            .vertices
            .iter()
            .flatten()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let indices: Vec<u8> = geometry
            .indices
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        buffers.vertices = Some(device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("purchased antenna vertices"),
            contents: &vertices,
            usage: BufferUsages::VERTEX,
        }));
        buffers.indices = Some(device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("purchased antenna indices"),
            contents: &indices,
            usage: BufferUsages::INDEX,
        }));
        buffers.count = geometry.indices.len() as u32;
        buffers.key = source.key;
    }
    let mut projection = view.clip_from_view;
    if let Some(jitter) = jitter {
        jitter.jitter_projection(
            &mut projection,
            Vec2::new(prepass.size.width as f32, prepass.size.height as f32),
        );
    }
    let view_from_world = view.world_from_view.to_matrix().inverse();
    let data = Parameters {
        jittered_clip: projection * view_from_world,
        current_clip: view.clip_from_view * view_from_world,
        previous_clip: previous.clip_from_world,
        current_model: source.current,
        previous_model: source.previous,
    };
    if [
        data.jittered_clip,
        data.current_clip,
        data.previous_clip,
        data.current_model,
        data.previous_model,
    ]
    .iter()
    .any(|m| !m.is_finite())
    {
        return;
    }
    let mut uniform = UniformBuffer::from(data);
    uniform.write_buffer(&device, &queue);
    let group = device.create_bind_group(
        "thin antenna motion",
        &cache.get_bind_group_layout(&pipeline.layout),
        &BindGroupEntries::sequential((uniform.binding().unwrap(),)),
    );
    let mut pass = ctx
        .command_encoder()
        .begin_render_pass(&RenderPassDescriptor {
            label: Some("antenna conservative velocity only"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: motion,
                depth_slice: None,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Load,
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                view: view_depth.view(),
                depth_ops: None,
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
    pass.set_pipeline(compiled);
    pass.set_bind_group(0, &group, &[]);
    pass.set_vertex_buffer(0, *buffers.vertices.as_ref().unwrap().slice(..));
    pass.set_index_buffer(
        *buffers.indices.as_ref().unwrap().slice(..),
        IndexFormat::Uint32,
    );
    pass.set_viewport(
        view.viewport.x as f32,
        view.viewport.y as f32,
        view.viewport.z as f32,
        view.viewport.w as f32,
        0.,
        1.,
    );
    pass.draw_indexed(0..buffers.count, 0, 0..1);
    if !*reported {
        println!(
            "thin_motion active=true conservative=true motion_only=true depth_write=false player_antenna_vertices={} triangles={} stage=after-prepass-before-main",
            geometry.vertices.len(),
            geometry.indices.len() / 3
        );
        *reported = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extracted_motion_tracks_render_frames_and_respects_hidden_parts() {
        let mut app = App::new();
        app.init_resource::<Source>()
            .init_resource::<Assets<Mesh>>()
            .add_systems(Update, snapshot);
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            bevy::asset::RenderAssetUsages::default(),
        );
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            vec![[0f32, 0., 0.], [0.01, 0., 0.], [0., 0.2, 0.]],
        );
        mesh.insert_indices(bevy::mesh::Indices::U32(vec![0, 1, 2]));
        let handle = app.world_mut().resource_mut::<Assets<Mesh>>().add(mesh);
        let first = GlobalTransform::from_translation(Vec3::new(1., 2., 3.));
        let entity = app
            .world_mut()
            .spawn((
                super::super::track_surface::SourcePart("car0/Aerials".into()),
                Mesh3d(handle.clone()),
                first,
                InheritedVisibility::VISIBLE,
            ))
            .id();
        app.update();
        let source = app.world().resource::<Source>();
        assert!(source.visible);
        assert_eq!(source.current, first.to_matrix());
        assert_eq!(source.previous, first.to_matrix());
        let second = GlobalTransform::from_translation(Vec3::new(2., 2., 3.));
        app.world_mut().entity_mut(entity).insert(second);
        app.update();
        let source = app.world().resource::<Source>();
        assert_eq!(source.current, second.to_matrix());
        assert_eq!(source.previous, first.to_matrix());
        app.world_mut()
            .entity_mut(entity)
            .insert(InheritedVisibility::HIDDEN);
        app.update();
        assert!(!app.world().resource::<Source>().visible);
        assert_eq!(
            app.world().resource::<Source>().previous,
            second.to_matrix()
        );
        app.world_mut().entity_mut(entity).despawn();
        app.update();
        assert!(!app.world().resource::<Source>().visible);
        assert!(app.world().resource::<Source>().key.is_none());
        assert_eq!(
            app.world()
                .resource::<Assets<Mesh>>()
                .get(&handle)
                .unwrap()
                .count_vertices(),
            3
        );
    }
}
