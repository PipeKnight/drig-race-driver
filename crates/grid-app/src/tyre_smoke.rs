//! Tyre smoke presentation (TYRE_SMOKE_RESEARCH.md): the source-data particle
//! simulation in `grid_tools::tyre_smoke` advances once per successful physics
//! tick for the player and every opponent; this module draws its sprites as
//! camera-facing (or horizon-aligned) quads, one alpha-blended mesh per shipped
//! texture, sorted back to front. Presentation only, never feeds physics.
use super::mesh_stream::{MeshStream, Quad, QuadMesh};
use bevy::{
    asset::RenderAssetUsages,
    camera::{primitives::Aabb, visibility::NoFrustumCulling},
    image::ImageSampler,
    light::{NotShadowCaster, NotShadowReceiver},
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use grid_tools::tyre_smoke::{self as smoke, Library, Smoke, Sprite};
use std::sync::Arc;

#[derive(Resource)]
pub(crate) struct TyreSmoke {
    smoke: Smoke,
    last: Option<(u64, u64)>,
    meshes: Vec<Handle<Mesh>>,
    /// The group entities (identity transform), for their sort centre.
    entities: Vec<Entity>,
    /// Each group's capacity: the sprites are streamed into it (F394).
    quads: Vec<QuadMesh>,
    /// Groups whose mesh already draws nothing: an empty group leaves it
    /// untouched instead of rewriting it every frame (F392).
    hidden: Vec<bool>,
    /// Camera position of the last drawn frame (distance level of detail).
    eye: [f32; 3],
    /// Frame counter for the periodic log line.
    frames: u64,
}

impl TyreSmoke {
    /// The smoke group meshes (Performance test, `perf_test`).
    pub(crate) fn mesh_handles(&self) -> &[Handle<Mesh>] {
        &self.meshes
    }
}

pub(crate) fn install(app: &mut App, root: &std::path::Path) {
    let loaded = Library::load(root).and_then(|library| {
        let textures = smoke::load_textures(root, &library.texture_names())?;
        Ok((Arc::new(library), textures))
    });
    match loaded {
        Ok((library, textures)) => {
            println!(
                "tyre_smoke version=1 event=loaded systems={} textures={} particle_limit={} fidelity=experimental",
                library.system_count(),
                textures.len(),
                library.particle_limit()
            );
            app.insert_resource(TyreSmoke {
                smoke: Smoke::new(library.clone()),
                last: None,
                meshes: Vec::new(),
                entities: Vec::new(),
                quads: Vec::new(),
                hidden: Vec::new(),
                eye: [0.; 3],
                frames: 0,
            })
            .insert_resource(Pending(Some((library, textures))))
            .add_systems(Startup, setup)
            .add_systems(FixedUpdate, tick.after(super::drive_tick))
            .add_systems(PostUpdate, draw.after(TransformSystems::Propagate));
        }
        Err(error) => warn!("Tyre smoke unavailable; driving continues: {error}"),
    }
}

type Loaded = (
    Arc<Library>,
    std::collections::HashMap<String, (Vec<u8>, u32)>,
);
#[derive(Resource)]
struct Pending(Option<Loaded>);

fn setup(
    mut commands: Commands,
    mut pending: ResMut<Pending>,
    mut state: ResMut<TyreSmoke>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some((library, textures)) = pending.0.take() else {
        return;
    };
    for name in library.texture_names() {
        let (dds, _) = &textures[name];
        let image = match super::texture_upload::image(
            dds,
            ImageSampler::linear(),
            RenderAssetUsages::RENDER_WORLD,
        ) {
            Ok(image) => images.add(image),
            Err(error) => {
                warn!("tyre smoke texture {name}: {error}");
                Handle::default()
            }
        };
        let mesh = meshes.add(empty_mesh());
        let material = materials.add(StandardMaterial {
            base_color_texture: Some(image),
            // AUTHORED: scene-lit diffuse billboards (normals face the camera)
            // with alpha blending; the original pfx_half_smoke shader, its
            // per-particle light sample and half-resolution buffer are not
            // reproduced.
            perceptual_roughness: 1.,
            reflectance: 0.,
            alpha_mode: AlphaMode::Blend,
            double_sided: true,
            cull_mode: None,
            fog_enabled: false,
            ..default()
        });
        let entity = commands.spawn((
            Name::new(format!("Tyre smoke {name}")),
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material),
            Transform::IDENTITY,
            NoFrustumCulling,
            NotShadowCaster,
            NotShadowReceiver,
        ));
        state.entities.push(entity.id());
        state.quads.push(QuadMesh::default());
        state.meshes.push(mesh);
        state.hidden.push(true);
    }
}

fn empty_mesh() -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0f32; 3]; 4]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0f32, 1., 0.]; 4]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0f32; 2]; 4]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[0f32; 4]; 4]);
    mesh.insert_indices(Indices::U32(vec![0, 1, 2, 0, 2, 3]));
    mesh
}

/// Advance once per successful player tick (opponents tick in the same
/// FixedUpdate step); reset on a new driving segment.
fn tick(mut state: ResMut<TyreSmoke>, driving: Res<super::Driving>) {
    let now = (driving.segment, driving.runtime.ticks);
    match state.last {
        Some(last) if last == now => return,
        Some((segment, ticks)) if segment == now.0 && ticks + 1 == now.1 => {}
        _ => state.smoke.clear(),
    }
    state.last = Some(now);
    if now.1 == 0 || driving.paused || driving.failure.is_some() {
        return;
    }
    let mut cars = vec![smoke::CarInput::read(&driving.runtime, false)];
    if let Some(opponents) = &driving.opponents {
        cars.extend(
            opponents
                .multi
                .cars
                .iter()
                .skip(1)
                .map(|car| smoke::CarInput::read(car, true)),
        );
    }
    let eye = state.eye;
    state
        .smoke
        .tick(&cars, eye, grid_tools::source_vehicle::DELTA);
    if now.1.is_multiple_of(25) {
        let player = state
            .smoke
            .wheel_report()
            .first()
            .copied()
            .unwrap_or_default();
        println!(
            "tyre_smoke event=tick tick={} particles={} emitting_wheels={} player_slip_rate={:?}",
            now.1,
            state.smoke.particle_count(),
            state.smoke.emitting_wheels(),
            player.map(|(s, r)| [(s * 100.).round() / 100., (r * 100.).round() / 100.])
        );
    }
}

fn draw(
    mut state: ResMut<TyreSmoke>,
    camera: Single<&GlobalTransform, With<crate::ViewerCamera>>,
    mut meshes: ResMut<Assets<Mesh>>,
    stream: Res<MeshStream>,
) {
    let state = &mut *state;
    let eye = camera.translation();
    state.eye = eye.to_array();
    let right = camera.right().as_vec3();
    let up = camera.up().as_vec3();
    let library = state.smoke.library().clone();
    let mut groups: Vec<Vec<(f32, &Sprite)>> = vec![Vec::new(); state.meshes.len()];
    for sprite in state.smoke.sprites() {
        let p = Vec3::from_array(sprite.position);
        if let Some(group) = groups.get_mut(sprite.texture) {
            group.push((p.distance_squared(eye), sprite));
        }
    }
    let mut total = 0;
    let mut quads = Vec::new();
    for (k, group) in groups.iter_mut().enumerate() {
        group.sort_by(|a, b| b.0.total_cmp(&a.0));
        total += group.len();
        if group.is_empty() && state.hidden[k] {
            continue;
        }
        state.hidden[k] = group.is_empty();
        quads.clear();
        for (_, s) in group.iter() {
            let centre = Vec3::from_array(s.position);
            let (axis_x, axis_y, normal) = if s.horizon {
                (Vec3::X, Vec3::Z, Vec3::Y)
            } else {
                (right, up, (eye - centre).normalize_or(Vec3::Y))
            };
            // The original puts the corners at ±size (886BD0, 0x88B3DB): the
            // stored size is the half extent, so the drawn edge is 2 × size.
            let (sin, cos) = s.rotation.sin_cos();
            let hx = (axis_x * cos + axis_y * sin) * s.size[0];
            let hy = (axis_y * cos - axis_x * sin) * s.size[1];
            let [u0, v0, u1, v1] = library.frame_uv(s.texture, s.frame);
            // Data colours are display-referred; vertex colours are linear.
            let c = Color::srgba(s.colour[0], s.colour[1], s.colour[2], s.colour[3]).to_linear();
            quads.push(Quad {
                positions: [-hx - hy, hx - hy, hx + hy, -hx + hy]
                    .map(|corner| (centre + corner).to_array()),
                normal: normal.to_array(),
                uvs: [[u0, v1], [u1, v1], [u1, v0], [u0, v0]],
                colour: [c.red, c.green, c.blue, c.alpha],
            });
        }
        state.quads[k].update(
            &quads,
            [0, 1, 2, 0, 2, 3],
            &state.meshes[k],
            &mut meshes,
            &stream,
        );
        // The order between the groups (and other blended meshes) is that of
        // a mesh holding only these sprites; an empty group was one
        // transparent degenerate quad at the origin.
        let corners = quads.iter().flat_map(|q| q.positions.map(Vec3::from_array));
        let centre = Aabb::enclosing(corners).map_or(Vec3::ZERO, |aabb| aabb.center.into());
        stream.sort_centre(state.entities[k], centre);
    }
    state.frames += 1;
    if state.frames.is_multiple_of(600) {
        println!(
            "tyre_smoke event=sample frame={} sprites={total} particles={} emitting_wheels={} dropped={}",
            state.frames,
            state.smoke.particle_count(),
            state.smoke.emitting_wheels(),
            state.smoke.dropped()
        );
    }
}
