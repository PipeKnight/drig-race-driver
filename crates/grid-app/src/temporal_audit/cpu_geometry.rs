//! Bounded current-CPU-mesh candidate observer; no GPU primitive/visibility claim.
use super::Snapshot;
use bevy::{
    mesh::VertexAttributeValues,
    prelude::*,
    render::{Extract, ExtractSchedule, RenderApp, render_asset::AssetExtractionSystems},
};
use std::{io::Write, path::Path, sync::Arc};
const MAX_DRAWS: usize = 512;
const MAX_VERTICES: usize = 200_000;
const MAX_TRIANGLES: usize = 400_000;
const MAX_BYTES: usize = 16 * 1024 * 1024;
const MAX_PIXELS: usize = 32;
const MAX_HITS: usize = 32;
#[derive(Resource)]
pub(super) struct Geometry {
    pixels: Vec<UVec2>,
    current: Option<Result<Arc<Frame>, String>>,
}
struct Draw {
    entity: Entity,
    name: String,
    asset: AssetId<Mesh>,
    world: Mat4,
    positions: Vec<[f32; 3]>,
    indices: Vec<u32>,
    digest: String,
}
struct Frame {
    source_frame: u32,
    tick: u64,
    body: [u32; 16],
    draws: Vec<Draw>,
}
#[derive(Clone)]
pub(super) struct Frozen {
    frame: Arc<Frame>,
    pixels: Vec<UVec2>,
    inverse_jittered: Mat4,
    size: UVec2,
    origin: UVec2,
}
fn pixels(value: &str) -> Result<Vec<UVec2>, String> {
    let result = value
        .split(';')
        .map(|s| {
            let (x, y) = s.split_once(',').ok_or("CPU geometry pixel needs x,y")?;
            let x = x.parse::<u32>().map_err(|e| e.to_string())?;
            let y = y.parse::<u32>().map_err(|e| e.to_string())?;
            if x >= super::WIDTH || y >= super::HEIGHT {
                return Err("CPU geometry pixel exceeds audit ROI".into());
            }
            Ok(UVec2::new(x, y))
        })
        .collect::<Result<Vec<_>, String>>()?;
    if result.is_empty() || result.len() > MAX_PIXELS {
        return Err("CPU geometry needs 1..32 ROI pixels".into());
    }
    let mut sorted = result.clone();
    sorted.sort_unstable_by_key(|p| (p.x, p.y));
    sorted.dedup();
    if sorted.len() != result.len() {
        return Err("CPU geometry pixels duplicate".into());
    }
    Ok(result)
}
pub(super) fn requested() -> Result<Option<Vec<UVec2>>, String> {
    match std::env::var("GRID_TEMPORAL_CPU_GEOMETRY_PIXELS") {
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(e) => Err(e.to_string()),
        Ok(value) => pixels(&value).map(Some),
    }
}
pub(super) fn install(app: &mut App, points: Vec<UVec2>) -> Result<(), String> {
    let render = app
        .get_sub_app_mut(RenderApp)
        .ok_or("CPU geometry render app absent")?;
    render
        .insert_resource(Geometry {
            pixels: points,
            current: None,
        })
        .add_systems(ExtractSchedule, extract.before(AssetExtractionSystems));
    Ok(())
}
fn digest(positions: &[[f32; 3]], indices: &[u32]) -> String {
    use grid_tools::private_parameters as p;
    let mut b = Vec::with_capacity(positions.len() * 12 + indices.len() * 4 + 16);
    b.extend((positions.len() as u64).to_le_bytes());
    b.extend((indices.len() as u64).to_le_bytes());
    b.extend(
        positions
            .iter()
            .flatten()
            .flat_map(|f| f.to_bits().to_le_bytes()),
    );
    b.extend(indices.iter().flat_map(|i| i.to_le_bytes()));
    p::hex(&p::sha256(&b))
}
type Candidates<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Name,
        &'static Mesh3d,
        &'static GlobalTransform,
    ),
    With<crate::MovingPart>,
>;
fn extract(
    state: Extract<Res<Snapshot>>,
    audit: Extract<Res<super::Audit>>,
    meshes: Extract<Res<Assets<Mesh>>>,
    candidates: Extract<Candidates>,
    mut geometry: ResMut<Geometry>,
) {
    if state.tick < audit.tick
        || audit.failed()
        || audit
            .submitted
            .iter()
            .all(|n| n.load(std::sync::atomic::Ordering::Acquire))
    {
        geometry.current = None;
        return;
    }
    geometry.current = Some(gather(&state, &meshes, candidates.iter()));
}
fn gather<'a>(
    state: &Snapshot,
    meshes: &Assets<Mesh>,
    candidates: impl IntoIterator<Item = (Entity, &'a Name, &'a Mesh3d, &'a GlobalTransform)>,
) -> Result<Arc<Frame>, String> {
    let mut draws = Vec::new();
    let (mut vertices, mut triangles, mut bytes) = (0usize, 0usize, 0usize);
    for (entity, name, handle, world) in candidates {
        if draws.len() == MAX_DRAWS {
            return Err("CPU geometry draw budget".into());
        }
        let mesh = meshes
            .get(&handle.0)
            .ok_or("CPU geometry current Mesh absent; no rest fallback")?;
        if mesh.primitive_topology()
            != bevy::render::render_resource::PrimitiveTopology::TriangleList
        {
            return Err("CPU geometry topology unsupported".into());
        }
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            return Err("CPU geometry current positions absent".into());
        };
        let indices = mesh.indices().ok_or("CPU geometry indices absent")?;
        vertices = vertices
            .checked_add(positions.len())
            .ok_or("CPU vertex overflow")?;
        triangles = triangles
            .checked_add(indices.len() / 3)
            .ok_or("CPU triangle overflow")?;
        bytes = bytes
            .checked_add(positions.len() * 12 + indices.len() * 4)
            .ok_or("CPU byte overflow")?;
        if vertices > MAX_VERTICES || triangles > MAX_TRIANGLES || bytes > MAX_BYTES {
            return Err("CPU geometry aggregate budget".into());
        }
        if name.as_str().len() > 256 {
            return Err("CPU draw name exceeds metadata bound".into());
        }
        let world = world.to_matrix();
        if !world.is_finite()
            || world.determinant().abs() < 1e-12
            || positions.iter().flatten().any(|v| !v.is_finite())
        {
            return Err("CPU geometry invalid transform/position".into());
        }
        let indices = indices
            .iter()
            .map(|i| u32::try_from(i).map_err(|e| e.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        if indices.is_empty()
            || !indices.len().is_multiple_of(3)
            || indices.iter().any(|&i| i as usize >= positions.len())
        {
            return Err("CPU geometry invalid triangle indices".into());
        }
        draws.push(Draw {
            entity,
            name: name.as_str().into(),
            asset: handle.0.id(),
            world,
            digest: digest(positions, &indices),
            positions: positions.clone(),
            indices,
        });
    }
    if draws.is_empty() {
        return Err("CPU geometry model-instance set empty".into());
    }
    draws.sort_by_key(|d| d.entity.to_bits());
    Ok(Arc::new(Frame {
        source_frame: state.frame,
        tick: state.tick,
        body: state.current_body.to_cols_array().map(f32::to_bits),
        draws,
    }))
}
impl Geometry {
    pub(super) fn freeze(
        &self,
        state: &Snapshot,
        inverse_jittered: Mat4,
        size: UVec2,
        origin: UVec2,
    ) -> Result<Frozen, String> {
        let frame = self
            .current
            .as_ref()
            .ok_or("CPU geometry extract absent")?
            .as_ref()
            .map_err(Clone::clone)?;
        if frame.source_frame != state.frame
            || frame.tick != state.tick
            || frame.body != state.current_body.to_cols_array().map(f32::to_bits)
        {
            return Err("CPU geometry exact source snapshot mismatch".into());
        }
        Ok(Frozen {
            frame: frame.clone(),
            pixels: self.pixels.clone(),
            inverse_jittered,
            size,
            origin,
        })
    }
}
impl Frozen {
    pub(super) fn summary(&self, header: [u32; 12], data: &[u8]) -> Result<String, String> {
        if header[1] != self.frame.source_frame
            || header[2] as u64 != self.frame.tick
            || header[4..6] != self.size.to_array()
            || header[6..8] != self.origin.to_array()
        {
            return Err("CPU geometry raw-header identity mismatch".into());
        }
        let clip = self.inverse_jittered.inverse();
        if !clip.is_finite() {
            return Err("CPU geometry singular captured projection".into());
        }
        let mut out = format!(
            "cpu_effective_mesh_candidate=true gpu_primitive_verified=false opaque_visibility_verified=false gpu_upload_verified=false source_frame={} source_tick={} draws={} hash_encoding=f32bits_le_positions_then_u32indices_with_u64counts\n",
            self.frame.source_frame,
            self.frame.tick,
            self.frame.draws.len()
        );
        for d in &self.frame.draws {
            out.push_str(&format!("cpu_mesh entity={:?} source_draw_name_unqualified={:?} asset={:?} world_bits={:?} positions={} triangles={} current_shape_sha256={}\n",d.entity,d.name,d.asset,d.world.to_cols_array().map(f32::to_bits),d.positions.len(),d.indices.len()/3,d.digest));
        }
        for point in &self.pixels {
            if point.x >= header[8] || point.y >= header[9] {
                return Err("CPU pixel outside actual raw extent".into());
            }
            let pixel = self.origin + *point;
            let ndc = Vec2::new(
                2. * (pixel.x as f32 + 0.5) / self.size.x as f32 - 1.,
                1. - 2. * (pixel.y as f32 + 0.5) / self.size.y as f32,
            );
            let near = self.inverse_jittered.project_point3(ndc.extend(1.));
            let far = self.inverse_jittered.project_point3(ndc.extend(0.5));
            let ray = (far - near).try_normalize().ok_or("CPU ray singular")?;
            if !near.is_finite() {
                return Err("CPU ray near nonfinite".into());
            }
            let at = 48 + ((point.y * header[8] + point.x) as usize) * 16 + 8;
            let raw_depth = f32::from_le_bytes(
                data.get(at..at + 4)
                    .ok_or("CPU depth offset invalid")?
                    .try_into()
                    .unwrap(),
            );
            let depth_point = self.inverse_jittered.project_point3(ndc.extend(raw_depth));
            if !depth_point.is_finite() {
                return Err("CPU depth unprojection nonfinite".into());
            }
            let mut hits = Vec::new();
            for d in &self.frame.draws {
                for (triangle, ix) in d.indices.as_chunks::<3>().0.iter().enumerate() {
                    let points = ix.map(|i| {
                        d.world
                            .transform_point3(Vec3::from_array(d.positions[i as usize]))
                    });
                    if points.iter().any(|p| !p.is_finite()) {
                        return Err("CPU world vertex nonfinite".into());
                    }
                    if let Some((distance, front, bary)) =
                        crate::geometry_ray::triangle_hit(near, ray, points)
                    {
                        if !distance.is_finite() {
                            return Err("CPU intersection distance nonfinite".into());
                        }
                        if hits.len() == MAX_HITS {
                            return Err("CPU ray hit budget; no truncation".into());
                        }
                        let hit = near + ray * distance;
                        let projected = clip.project_point3(hit);
                        if !projected.is_finite() {
                            return Err("CPU hit reprojection nonfinite".into());
                        }
                        let world_distance = (hit - depth_point).length();
                        if !world_distance.is_finite() {
                            return Err("CPU depth-distance metric nonfinite".into());
                        }
                        hits.push((
                            distance,
                            d.entity,
                            triangle,
                            front,
                            bary,
                            projected.z - raw_depth,
                            world_distance,
                            projected.truncate() - ndc,
                        ));
                    }
                }
            }
            hits.sort_by(|a, b| a.0.total_cmp(&b.0));
            out.push_str(&format!("cpu_ray roi={point:?} pixel={pixel:?} raw_depth={raw_depth:?} hits={} interpretation=CPU_candidates_only_no_first_hit_admission\n",hits.len()));
            for hit in hits {
                out.push_str(&format!("cpu_hit distance={} entity={:?} triangle={} front={} bary={:?} depth_residual={} depth_world_distance={} ndc_residual={:?}\n",hit.0,hit.1,hit.2,hit.3,hit.4,hit.5,hit.6,hit.7));
            }
        }
        Ok(out)
    }
    pub(super) fn write(
        &self,
        directory: &Path,
        slot: usize,
        header: [u32; 12],
        data: &[u8],
    ) -> Result<(), String> {
        let text = self.summary(header, data)?;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(directory.join(format!("cpu-geometry-{slot}.txt")))
            .map_err(|e| e.to_string())?;
        file.write_all(text.as_bytes()).map_err(|e| e.to_string())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_unique_roi_pixel_contract() {
        assert!(pixels("231,144;232,156").is_ok());
        for bad in ["", "512,0", "0,384", "1,1;1,1", "-1,0"] {
            assert!(pixels(bad).is_err());
        }
    }
    #[test]
    fn current_shape_hash_is_sensitive_to_one_position_bit_and_index_order() {
        let mut p = [[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]];
        let first = digest(&p, &[0, 1, 2]);
        p[0][0] = f32::from_bits(1);
        assert_ne!(first, digest(&p, &[0, 1, 2]));
        p[0][0] = 0.;
        assert_ne!(first, digest(&p, &[0, 2, 1]));
    }
    #[test]
    fn ecs_current_mesh_and_actual_global_are_copied_not_rest_guessed() {
        use bevy::{
            asset::RenderAssetUsages, mesh::Indices, render::render_resource::PrimitiveTopology,
        };
        let mut assets = Assets::<Mesh>::default();
        let mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_POSITION,
            vec![[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]],
        )
        .with_inserted_indices(Indices::U16(vec![0, 1, 2]));
        let handle = assets.add(mesh);
        assets.get_mut(&handle).unwrap().insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            vec![[0., 0., 0.75], [1., 0., 0.75], [0., 1., 0.75]],
        );
        let mut world = World::new();
        let matrix = Mat4::from_translation(Vec3::new(2., 3., 4.));
        world.spawn((
            Name::new("actual source draw"),
            Mesh3d(handle.clone()),
            GlobalTransform::from(matrix),
            crate::MovingPart {
                rest: Mat4::IDENTITY,
                wheel: None,
                lost_with_wheel: false,
            },
        ));
        let state = Snapshot {
            frame: 7,
            tick: 9,
            current_body: Mat4::IDENTITY,
            previous_body: Mat4::IDENTITY,
        };
        let mut query = world
            .query_filtered::<(Entity, &Name, &Mesh3d, &GlobalTransform), With<crate::MovingPart>>(
            );
        let frame = gather(&state, &assets, query.iter(&world)).unwrap();
        assert_eq!(frame.draws[0].positions[0], [0., 0., 0.75]);
        assert_eq!(frame.draws[0].world.to_cols_array(), matrix.to_cols_array());
        let first = frame.draws[0].digest.clone();
        assets.get_mut(&handle).unwrap().insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            vec![[0., 0., 0.5], [1., 0., 0.5], [0., 1., 0.5]],
        );
        let later = gather(&state, &assets, query.iter(&world)).unwrap();
        assert_ne!(first, later.draws[0].digest);
        assert_eq!(frame.draws[0].positions[0], [0., 0., 0.75]);
        let geometry = Geometry {
            pixels: vec![UVec2::ZERO],
            current: Some(Ok(frame)),
        };
        assert!(
            geometry
                .freeze(&state, Mat4::IDENTITY, UVec2::ONE, UVec2::ZERO)
                .is_ok()
        );
        let changed = Snapshot { frame: 8, ..state };
        assert!(
            geometry
                .freeze(&changed, Mat4::IDENTITY, UVec2::ONE, UVec2::ZERO)
                .is_err()
        );
    }
    #[test]
    fn frozen_raw_depth_ray_finds_mutated_cpu_triangle_and_refuses_header_drift() {
        let positions = vec![[-1., -1., 0.75], [1., -1., 0.75], [0., 1., 0.75]];
        let indices = vec![0, 1, 2];
        let draw = Draw {
            entity: Entity::PLACEHOLDER,
            name: "current CPU source candidate".into(),
            asset: AssetId::invalid(),
            world: Mat4::IDENTITY,
            digest: digest(&positions, &indices),
            positions,
            indices,
        };
        let frozen = Frozen {
            frame: Arc::new(Frame {
                source_frame: 7,
                tick: 9,
                body: Mat4::IDENTITY.to_cols_array().map(f32::to_bits),
                draws: vec![draw],
            }),
            pixels: vec![UVec2::ZERO],
            inverse_jittered: Mat4::IDENTITY,
            size: UVec2::ONE,
            origin: UVec2::ZERO,
        };
        let header = [1, 7, 9, 0, 1, 1, 0, 0, 1, 1, 0, 0];
        let mut data = vec![0; 64];
        data[56..60].copy_from_slice(&0.75f32.to_le_bytes());
        let summary = frozen.summary(header, &data).unwrap();
        assert!(summary.contains("cpu_hit distance=0.25"));
        assert!(summary.contains("depth_residual=0 "));
        assert!(summary.contains("gpu_primitive_verified=false"));
        let mut wrong = header;
        wrong[1] += 1;
        assert!(frozen.summary(wrong, &data).is_err());
        // Finite matrices/positions can still overflow a squared-distance metric.
        let mut extreme = frozen;
        extreme.inverse_jittered = Mat4::from_cols(
            Vec4::X, Vec4::Y, Vec4::new(0., 0., 0., 1.), Vec4::new(0., 0., 1., 1e-30),
        );
        for position in &mut Arc::get_mut(&mut extreme.frame).unwrap().draws[0].positions {
            position[2] = 1.5;
        }
        data[56..60].copy_from_slice(&0.0f32.to_le_bytes());
        assert!(extreme.summary(header, &data).unwrap_err().contains("metric nonfinite"));
    }
    #[derive(Resource, Default)]
    struct ExtractCalls(usize);
    fn publish_actual_clock(frame: Res<bevy::diagnostic::FrameCount>, mut state: ResMut<Snapshot>) {
        let prior = state.current_body;
        state.frame = frame.0;
        state.tick += 1;
        state.previous_body = prior;
        state.current_body = Mat4::from_translation(Vec3::new(state.tick as f32, 2., 3.));
    }
    fn asset_extraction_set_sentinel(
        mut main: ResMut<bevy::render::MainWorld>,
        geometry: Res<Geometry>,
        mut calls: ResMut<ExtractCalls>,
    ) {
        let state = main.resource::<Snapshot>();
        let frame = geometry
            .current
            .as_ref()
            .expect("observer must precede asset extraction")
            .as_ref()
            .unwrap();
        assert_eq!(frame.source_frame, state.frame);
        assert_eq!(frame.tick, state.tick);
        assert_eq!(
            frame.body,
            state.current_body.to_cols_array().map(f32::to_bits)
        );
        assert_eq!(
            main.resource::<bevy::diagnostic::FrameCount>().0,
            state.frame + 1
        );
        assert_eq!(
            frame.draws[0].positions[0][2],
            if calls.0 == 0 { 0.75 } else { 0.5 }
        );
        assert_eq!(
            frame.draws[0].world.to_cols_array(),
            Mat4::from_translation(Vec3::new(2., 3., 4.)).to_cols_array()
        );
        // An asset-extraction-stage mutation proves actual ordering and independent snapshot ownership.
        // No RenderDevice, WGPU or renderer plugin is installed in this fixture.
        let mut query = main.query_filtered::<&Mesh3d, With<crate::MovingPart>>();
        let handle = query.single(&main).unwrap().0.clone();
        let next = if calls.0 == 0 { 0.5 } else { 0.25 };
        main.resource_mut::<Assets<Mesh>>()
            .get_mut(&handle)
            .unwrap()
            .insert_attribute(
                Mesh::ATTRIBUTE_POSITION,
                vec![[0., 0., next], [1., 0., next], [0., 1., next]],
            );
        calls.0 += 1;
    }
    #[test]
    fn actual_extract_schedule_copies_main_clock_geometry_before_asset_stage_and_freezes_exact() {
        use bevy::{
            asset::RenderAssetUsages,
            mesh::Indices,
            render::{extract_plugin::ExtractPlugin, render_resource::PrimitiveTopology},
        };
        let mut app = App::new();
        app.add_plugins((bevy::diagnostic::FrameCountPlugin, ExtractPlugin::default()));
        let mut assets = Assets::<Mesh>::default();
        let mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_POSITION,
            vec![[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]],
        )
        .with_inserted_indices(Indices::U16(vec![0, 1, 2]));
        let handle = assets.add(mesh);
        // Actual current data deliberately differs from source rest before production extraction.
        assets.get_mut(&handle).unwrap().insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            vec![[0., 0., 0.75], [1., 0., 0.75], [0., 1., 0.75]],
        );
        app.insert_resource(assets).init_resource::<Snapshot>();
        app.insert_resource(super::super::Audit {
            #[cfg(feature = "dlaa")]
            companion: None,
            tick: 1,
            centre: None,
            directory: std::path::PathBuf::new(),
            buffers: vec![Handle::default(), Handle::default()],
            submitted: vec![Arc::default(), Arc::default()],
            readback_queued: vec![false, false],
            complete: vec![Arc::default(), Arc::default()],
            failed: Arc::default(),
            frozen: Arc::new(std::sync::Mutex::new(vec![None, None])),
            pair: Some(super::super::PairCapture::default()),
        });
        app.add_systems(PostUpdate, publish_actual_clock);
        app.world_mut().spawn((
            Name::new("actual source draw"),
            Mesh3d(handle.clone()),
            GlobalTransform::from(Mat4::from_translation(Vec3::new(2., 3., 4.))),
            crate::MovingPart {
                rest: Mat4::IDENTITY,
                wheel: None,
                lost_with_wheel: false,
            },
        ));
        app.sub_app_mut(RenderApp)
            .init_resource::<ExtractCalls>()
            .add_systems(
                ExtractSchedule,
                asset_extraction_set_sentinel.in_set(AssetExtractionSystems),
            );
        // Sentinel was registered first; production before(AssetExtractionSystems) supplies the order.
        install(&mut app, vec![UVec2::ZERO]).unwrap(); // Actual production registration, not direct gather().
        app.update(); // Actual Main/PostUpdate -> Last FrameCount -> ExtractPlugin -> ExtractSchedule.
        let first_state = app.world().resource::<Snapshot>().clone();
        let first = app
            .sub_app(RenderApp)
            .world()
            .resource::<Geometry>()
            .freeze(&first_state, Mat4::IDENTITY, UVec2::ONE, UVec2::ZERO)
            .unwrap();
        assert_eq!(first.frame.draws[0].positions[0][2], 0.75);
        let Some(VertexAttributeValues::Float32x3(now)) = app
            .world()
            .resource::<Assets<Mesh>>()
            .get(&handle)
            .unwrap()
            .attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("current positions")
        };
        assert_eq!(now[0][2], 0.5); // Asset stage truly ran AFTER the independent copied geometry.
        app.update();
        let second_state = app.world().resource::<Snapshot>().clone();
        let geometry = app.sub_app(RenderApp).world().resource::<Geometry>();
        let second = geometry
            .freeze(&second_state, Mat4::IDENTITY, UVec2::ONE, UVec2::ZERO)
            .unwrap();
        assert_eq!(second.frame.source_frame, first.frame.source_frame + 1);
        assert_eq!(second.frame.tick, first.frame.tick + 1);
        assert_ne!(second.frame.body, first.frame.body);
        assert_eq!(second.frame.draws[0].positions[0][2], 0.5);
        assert_eq!(first.frame.draws[0].positions[0][2], 0.75); // Frozen Arc unchanged after second main mutation.
        assert_ne!(first.frame.draws[0].digest, second.frame.draws[0].digest);
        assert!(
            geometry
                .freeze(&first_state, Mat4::IDENTITY, UVec2::ONE, UVec2::ZERO)
                .is_err()
        );
        let wrong_tick = Snapshot {
            tick: second_state.tick + 1,
            ..second_state.clone()
        };
        assert!(
            geometry
                .freeze(&wrong_tick, Mat4::IDENTITY, UVec2::ONE, UVec2::ZERO)
                .is_err()
        );
        let wrong_body = Snapshot {
            current_body: Mat4::IDENTITY,
            ..second_state
        };
        assert!(
            geometry
                .freeze(&wrong_body, Mat4::IDENTITY, UVec2::ONE, UVec2::ZERO)
                .is_err()
        );
        assert_eq!(
            app.sub_app(RenderApp).world().resource::<ExtractCalls>().0,
            2
        );
    }
    #[test]
    fn factored_triangle_helper_keeps_known_front_back_miss_and_degenerate_semantics() {
        let p = [
            Vec3::new(-1., -1., 0.75),
            Vec3::new(1., -1., 0.75),
            Vec3::new(0., 1., 0.75),
        ];
        let front = crate::geometry_ray::triangle_hit(Vec3::Z, -Vec3::Z, p).unwrap();
        assert_eq!(front, (0.25, true, [0.25, 0.5]));
        let back =
            crate::geometry_ray::triangle_hit(Vec3::Z, -Vec3::Z, [p[0], p[2], p[1]]).unwrap();
        assert_eq!(back.0, 0.25);
        assert!(!back.1);
        assert!(crate::geometry_ray::triangle_hit(Vec3::new(3., 0., 1.), -Vec3::Z, p).is_none());
        assert!(crate::geometry_ray::triangle_hit(Vec3::Z, Vec3::Z, p).is_none());
        assert!(crate::geometry_ray::triangle_hit(Vec3::Z, -Vec3::Z, [p[0]; 3]).is_none());
    }
}
