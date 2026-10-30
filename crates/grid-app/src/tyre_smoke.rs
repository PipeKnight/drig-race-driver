//! Tyre smoke presentation (TYRE_SMOKE_RESEARCH.md): the source-data particle
//! simulation in `grid_tools::tyre_smoke` advances once per successful physics
//! tick for the player and every opponent; this module draws its sprites as
//! camera-facing (or horizon-aligned) quads, one alpha-blended mesh per shipped
//! texture, sorted back to front, lit once per particle as the original's
//! particle shaders do (`tyre_smoke.wgsl`). Presentation only, never feeds
//! physics.
use super::mesh_stream::{MeshStream, Quad, QuadMesh};
use bevy::{
    asset::{RenderAssetUsages, uuid_handle},
    camera::{
        primitives::Aabb,
        visibility::{NoFrustumCulling, VisibilitySystems},
    },
    image::ImageSampler,
    light::{NotShadowCaster, NotShadowReceiver},
    mesh::{Indices, PrimitiveTopology},
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};
use grid_formats::track_lighting::ParticleLight;
use grid_tools::tyre_smoke::{self as smoke, Library, Smoke, Sprite};
use std::sync::Arc;

const SHADER: Handle<Shader> = uuid_handle!("6d1f3b52-0c47-4a8e-9b1e-5f2a7c9d4e31");

/// An unlit material whose fragment applies the original particle light.
pub(crate) type SmokeMaterial = ExtendedMaterial<StandardMaterial, SmokeLight>;

/// The particle light colours (track lighting.xml), see `tyre_smoke.wgsl`.
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug, Default)]
pub(crate) struct SmokeLight {
    /// `pfxSunColour`; w = 1 for the lit smoke stream, 0 for the fakelight
    /// burnout rings.
    #[uniform(100)]
    sun: Vec4,
    /// `pfxPreMultipliedAmbientColour`.
    #[uniform(101)]
    ambient: Vec4,
    /// `pfxUnlitAmbientColour`.
    #[uniform(102)]
    unlit: Vec4,
}

impl MaterialExtension for SmokeLight {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }
}

impl SmokeLight {
    /// Burnout_circles (the only Wheel_spin emitter) is `basicDepthAlphaBlendHalf`,
    /// shader `pfx_unlit_half_depth` (fakelight); every other smoke emitter is
    /// `smokeHalf`, `pfx_half_smoke` with the lit stream (effects/config.xml).
    fn new(light: &ParticleLight, texture: &str) -> Self {
        let lit = texture != "Wheel_spin.tga";
        Self {
            sun: Vec3::from_array(light.sun).extend(if lit { 1. } else { 0. }),
            ambient: Vec3::from_array(light.ambient).extend(0.),
            unlit: Vec3::from_array(light.unlit_ambient).extend(0.),
        }
    }
}

/// Default keeps the original five texture groups and draw system unchanged.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SmokeOrder {
    #[default]
    Groups,
    Sprites,
}

#[derive(Resource)]
pub(crate) struct TyreSmoke {
    smoke: Smoke,
    pool: Option<SpritePool>,
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
        self.pool.as_ref().map_or(&self.meshes, |pool| &pool.meshes)
    }
}

pub(crate) fn install(
    app: &mut App,
    root: &std::path::Path,
    mode: SmokeOrder,
    light: ParticleLight,
) {
    let loaded = Library::load(root).and_then(|library| {
        let textures = smoke::load_textures(root, &library.texture_names())?;
        Ok((Arc::new(library), textures))
    });
    match loaded {
        Ok((library, textures)) => {
            println!(
                "tyre_smoke version=1 event=loaded systems={} textures={} particle_limit={} fidelity=experimental light=per-particle pfx_sun={:?} pfx_ambient={:?} pfx_unlit={:?}",
                library.system_count(),
                textures.len(),
                library.particle_limit(),
                light.sun,
                light.ambient,
                light.unlit_ambient
            );
            app.world_mut()
                .resource_mut::<Assets<Shader>>()
                .insert(
                    SHADER.id(),
                    Shader::from_wgsl(include_str!("tyre_smoke.wgsl"), "grid_tyre_smoke.wgsl"),
                )
                .expect("tyre smoke shader handle");
            app.add_plugins(MaterialPlugin::<SmokeMaterial>::default());
            app.insert_resource(TyreSmoke {
                smoke: Smoke::new(library.clone()),
                pool: (mode == SmokeOrder::Sprites).then(SpritePool::default),
                last: None,
                meshes: Vec::new(),
                entities: Vec::new(),
                quads: Vec::new(),
                hidden: Vec::new(),
                eye: [0.; 3],
                frames: 0,
            })
            .insert_resource(Pending(Some((library, textures, light))))
            .add_systems(Startup, setup)
            .add_systems(FixedUpdate, tick.after(super::drive_tick));
            match mode {
                SmokeOrder::Groups => {
                    app.add_systems(PostUpdate, draw.after(TransformSystems::Propagate));
                }
                SmokeOrder::Sprites => {
                    app.add_systems(
                        PostUpdate,
                        draw_sprites
                            .after(TransformSystems::Propagate)
                            .before(VisibilitySystems::VisibilityPropagate),
                    );
                }
            }
        }
        Err(error) => warn!("Tyre smoke unavailable; driving continues: {error}"),
    }
}

type Loaded = (
    Arc<Library>,
    std::collections::HashMap<String, (Vec<u8>, u32)>,
    ParticleLight,
);
#[derive(Resource)]
struct Pending(Option<Loaded>);

fn setup(
    mut commands: Commands,
    mut pending: ResMut<Pending>,
    mut state: ResMut<TyreSmoke>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<SmokeMaterial>>,
) {
    let Some((library, textures, light)) = pending.0.take() else {
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

        let material = materials.add(SmokeMaterial {
            base: StandardMaterial {
                base_color_texture: Some(image),
                // Lit in the extension's fragment, once per particle; the
                // original's half-resolution buffer is not reproduced.
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                double_sided: true,
                cull_mode: None,
                fog_enabled: false,
                ..default()
            },
            extension: SmokeLight::new(&light, name),
        });
        if let Some(pool) = &mut state.pool {
            pool.materials.push(material);
            continue;
        }
        let mesh = meshes.add(empty_mesh());
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
        quads.extend(
            group
                .iter()
                .map(|(_, s)| sprite_quad(s, right, up, library.frame_uv(s.texture, s.frame))),
        );
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


/// A slot has a unique mesh and entity but uses one of the existing texture materials.
/// Slot identity follows array position, not particle identity: births/deaths can
/// shift slots, so material and all vertex attributes are refreshed on reuse.
struct SpriteSlot {
    entity: Entity,
    quad: QuadMesh,
    texture: usize,
    hidden: bool,
}

#[derive(Default, Resource)]
struct SpritePool {
    materials: Vec<Handle<SmokeMaterial>>,
    meshes: Vec<Handle<Mesh>>,
    slots: Vec<SpriteSlot>,
}

impl SpritePool {
    fn update(
        &mut self,
        sprites: &[(usize, Quad)],
        commands: &mut Commands,
        meshes: &mut Assets<Mesh>,
        stream: &MeshStream,
    ) {
        for (k, (texture, quad)) in sprites.iter().enumerate() {
            // The caller filters unknown texture indices exactly as Groups does.
            let material = &self.materials[*texture];
            if k == self.slots.len() {
                let mesh = meshes.add(empty_mesh());
                let entity = commands
                    .spawn((
                        Name::new(format!("Tyre smoke sprite {k}")),
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(material.clone()),
                        Transform::IDENTITY,
                        NoFrustumCulling,
                        NotShadowCaster,
                        NotShadowReceiver,
                    ))
                    .id();
                self.meshes.push(mesh);
                self.slots.push(SpriteSlot {
                    entity,
                    quad: QuadMesh::default(),
                    texture: *texture,
                    hidden: false,
                });
            }
            let slot = &mut self.slots[k];
            if slot.texture != *texture {
                commands
                    .entity(slot.entity)
                    .insert(MeshMaterial3d(material.clone()));
                slot.texture = *texture;
            }
            if slot.hidden {
                commands.entity(slot.entity).insert(Visibility::Inherited);
                slot.hidden = false;
            }
            slot.quad.update(
                std::slice::from_ref(quad),
                [0, 1, 2, 0, 2, 3],
                &self.meshes[k],
                meshes,
                stream,
            );
            stream.sort_centre(slot.entity, quad_centre(quad));
        }
        for (k, slot) in self.slots.iter_mut().enumerate().skip(sprites.len()) {
            if !slot.hidden {
                // Also zero the geometry: no stale sprite even if a future schedule
                // edit were to delay visibility propagation. Never reallocates.
                slot.quad
                    .update(&[], [0, 1, 2, 0, 2, 3], &self.meshes[k], meshes, stream);
                commands.entity(slot.entity).insert(Visibility::Hidden);
                stream.sort_centre(slot.entity, Vec3::ZERO);
                slot.hidden = true;
            }
        }
    }
}

fn quad_centre(quad: &Quad) -> Vec3 {
    Aabb::enclosing(quad.positions.map(Vec3::from_array))
        .map_or(Vec3::ZERO, |aabb| aabb.center.into())
}

/// The original lit stream's corner normals (886BD0, normal stream branch
/// 0x88C221, constants 0x88C2AB..0x88C38C): (±1, ±1, 1)/√3 in the billboard
/// basis, the in-plane signs those of the corner and +Z towards the camera,
/// so the interpolated normal rounds the sprite like a ball.
fn ball_normals(axis_x: Vec3, axis_y: Vec3, back: Vec3) -> [[f32; 3]; 4] {
    [(-1., -1.), (1., -1.), (1., 1.), (-1., 1.)]
        .map(|(sx, sy)| ((axis_x * sx + axis_y * sy + back) / 3f32.sqrt()).to_array())
}

fn sprite_quad(s: &Sprite, right: Vec3, up: Vec3, uv: [f32; 4]) -> Quad {
    let centre = Vec3::from_array(s.position);
    let (axis_x, axis_y) = if s.horizon {
        (Vec3::X, Vec3::Z)
    } else {
        (right, up)
    };
    // The original puts the corners at ±size (886BD0, 0x88B3DB): the
    // stored size is the half extent, so the drawn edge is 2 × size.
    let (sin, cos) = s.rotation.sin_cos();
    let (unit_x, unit_y) = (axis_x * cos + axis_y * sin, axis_y * cos - axis_x * sin);
    let (hx, hy) = (unit_x * s.size[0], unit_y * s.size[1]);
    let [u0, v0, u1, v1] = uv;
    // Data colours are display-referred; vertex colours are linear.
    let c = Color::srgba(s.colour[0], s.colour[1], s.colour[2], s.colour[3]).to_linear();
    Quad {
        positions: [-hx - hy, hx - hy, hx + hy, -hx + hy]
            .map(|corner| (centre + corner).to_array()),
        // The horizon-aligned rings use the unlit fakelight shader: no normal is read.
        normals: if s.horizon {
            [[0., 1., 0.]; 4]
        } else {
            ball_normals(unit_x, unit_y, right.cross(up))
        },
        uvs: [[u0, v1], [u1, v1], [u1, v0], [u0, v0]],
        colour: [c.red, c.green, c.blue, c.alpha],
    }
}

fn draw_sprites(
    mut commands: Commands,
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
    let library = state.smoke.library();
    let Some(pool) = &mut state.pool else { return };
    let quads: Vec<_> = state
        .smoke
        .sprites()
        .iter()
        .filter(|sprite| sprite.texture < pool.materials.len())
        .map(|s| {
            (
                s.texture,
                sprite_quad(s, right, up, library.frame_uv(s.texture, s.frame)),
            )
        })
        .collect();
    pool.update(&quads, &mut commands, &mut meshes, &stream);
    state.frames += 1;
    if state.frames.is_multiple_of(600) {
        println!(
            "tyre_smoke event=sample frame={} sprites={} particles={} emitting_wheels={} dropped={}",
            state.frames,
            quads.len(),
            state.smoke.particle_count(),
            state.smoke.emitting_wheels(),
            state.smoke.dropped()
        );
    }
}

#[cfg(test)]
mod sprite_pool_tests {
    use super::*;

    #[derive(Resource, Default)]
    struct Frame(Vec<(usize, Quad)>);

    fn sync(
        mut pool: ResMut<SpritePool>,
        frame: Res<Frame>,
        mut commands: Commands,
        mut meshes: ResMut<Assets<Mesh>>,
        stream: Res<MeshStream>,
    ) {
        pool.update(&frame.0, &mut commands, &mut meshes, &stream);
    }

    fn quad(frame: u8, horizon: bool) -> Quad {
        sprite_quad(
            &Sprite {
                position: [13., 2., -27.],
                size: [2., 0.75],
                rotation: 0.63,
                colour: [0.8, 0.24, 0.03, 0.37],
                texture: 0,
                frame,
                horizon,
            },
            Vec3::X,
            Vec3::Y,
            [
                frame as f32 * 0.125,
                0.25,
                (frame as f32 + 1.) * 0.125,
                0.75,
            ],
        )
    }

    fn app() -> App {
        let mut app = App::new();
        let mut materials = Assets::<SmokeMaterial>::default();
        let handles = (0..5)
            .map(|_| materials.add(SmokeMaterial::default()))
            .collect();
        app.insert_resource(SpritePool {
            materials: handles,
            ..default()
        })
        .insert_resource(materials)
        .init_resource::<Assets<Mesh>>()
        .init_resource::<MeshStream>()
        .init_resource::<Frame>()
        .add_systems(Update, sync);
        app
    }

    fn set(app: &mut App, values: Vec<(usize, Quad)>) {
        app.world_mut().resource_mut::<Frame>().0 = values;
        app.update();
    }

    #[test]
    fn sprite_pool_reuses_slots_through_shrink_empty_reset_and_material_frame_swap() {
        let mut app = app();
        set(&mut app, vec![]);
        assert!(app.world().resource::<SpritePool>().meshes.is_empty());
        set(&mut app, vec![(0, quad(0, false)), (2, quad(1, true))]);
        let world = app.world();
        let pool = world.resource::<SpritePool>();
        let entities: Vec<_> = pool.slots.iter().map(|s| s.entity).collect();
        let handles = pool.meshes.clone();
        for (k, entity) in entities.iter().enumerate() {
            assert_eq!(
                *world.get::<Transform>(*entity).unwrap(),
                Transform::IDENTITY
            );
            assert!(world.get::<NoFrustumCulling>(*entity).is_some());
            assert!(world.get::<NotShadowCaster>(*entity).is_some());
            assert!(world.get::<NotShadowReceiver>(*entity).is_some());
            assert_eq!(
                world.resource::<MeshStream>().queued_centre(*entity),
                Some(quad_centre(&quad(k as u8, k == 1)))
            );
            assert_eq!(
                world
                    .resource::<Assets<Mesh>>()
                    .get(&handles[k])
                    .unwrap()
                    .count_vertices(),
                4
            );
        }
        set(&mut app, vec![(4, quad(3, false))]);
        let world = app.world();
        let pool = world.resource::<SpritePool>();
        assert_eq!(pool.meshes, handles);
        assert_eq!(
            world
                .get::<MeshMaterial3d<SmokeMaterial>>(entities[0])
                .unwrap()
                .0,
            pool.materials[4]
        );
        assert_eq!(
            *world.get::<Visibility>(entities[1]).unwrap(),
            Visibility::Hidden
        );
        assert_eq!(
            world.resource::<MeshStream>().queued_centre(entities[1]),
            Some(Vec3::ZERO)
        );
        // Tail is also transparent zero-area geometry, without asset replacement.
        let padded = world
            .resource::<MeshStream>()
            .queued(handles[1].id())
            .unwrap();
        let layout = super::super::mesh_stream::VertexLayout::of(
            world.resource::<Assets<Mesh>>().get(&handles[1]).unwrap(),
        );
        for vertex in padded.chunks_exact(layout.stride) {
            let pos = layout.offset(Mesh::ATTRIBUTE_POSITION).unwrap();
            let colour = layout.offset(Mesh::ATTRIBUTE_COLOR).unwrap();
            assert!(vertex[pos..pos + 12].iter().all(|b| *b == 0));
            assert!(vertex[colour..colour + 16].iter().all(|b| *b == 0));
        }
        set(&mut app, vec![]);
        assert!(
            app.world()
                .resource::<SpritePool>()
                .slots
                .iter()
                .all(|s| s.hidden)
        );
        set(
            &mut app,
            vec![(1, quad(4, true)), (3, quad(5, false)), (0, quad(6, true))],
        );
        let world = app.world();
        let pool = world.resource::<SpritePool>();
        assert_eq!(pool.slots.len(), 3);
        assert_eq!(&pool.meshes[..2], &handles);
        assert_eq!(pool.slots[0].entity, entities[0]);
        assert_eq!(pool.slots[1].entity, entities[1]);
        assert_eq!(
            *world.get::<Visibility>(entities[1]).unwrap(),
            Visibility::Inherited
        );
        assert_eq!(world.resource::<Assets<Mesh>>().len(), 3);
        assert_eq!(world.resource::<Assets<SmokeMaterial>>().len(), 5);
        assert_eq!(
            world.resource::<MeshStream>().queued_centre(entities[1]),
            Some(quad_centre(&quad(5, false)))
        );
        // Updated frame/colour/normal/UV/position bytes equal a freshly constructed
        // single-quad asset; mesh IDs are unchanged after shrink and regrowth.
        let mut expected_meshes = Assets::<Mesh>::default();
        let h = expected_meshes.add(empty_mesh());
        QuadMesh::default().update(
            &[quad(5, false)],
            [0, 1, 2, 0, 2, 3],
            &h,
            &mut expected_meshes,
            &MeshStream::default(),
        );
        assert_eq!(
            world
                .resource::<MeshStream>()
                .queued(handles[1].id())
                .unwrap(),
            expected_meshes
                .get(&h)
                .unwrap()
                .create_packed_vertex_buffer_data()
        );
    }

    #[test]
    fn horizon_quad_has_literal_original_size_uv_colour_and_normal() {
        let q = sprite_quad(&Sprite {
            position: [3., 4., 5.], size: [2., 3.], rotation: 0.,
            colour: [1., 0., 1., 0.25], texture: 0, frame: 0, horizon: true,
        }, Vec3::Z, Vec3::Y, [0.125, 0.25, 0.5, 0.75]);
        assert_eq!(q.positions, [[1., 4., 2.], [5., 4., 2.], [5., 4., 8.], [1., 4., 8.]]);
        assert_eq!(q.normals, [[0., 1., 0.]; 4]);
        assert_eq!(q.uvs, [[0.125, 0.75], [0.5, 0.75], [0.5, 0.25], [0.125, 0.25]]);
        assert_eq!(q.colour, [1., 0., 1., 0.25]);
    }

    #[test]
    fn billboard_corner_normals_are_the_original_ball_normals() {
        // Camera right +X, up +Y, so the billboard faces +Z; no spin.
        let q = sprite_quad(&Sprite {
            position: [0., 1., -6.], size: [1., 2.], rotation: 0.,
            colour: [1.; 4], texture: 0, frame: 0, horizon: false,
        }, Vec3::X, Vec3::Y, [0., 0., 1., 1.]);
        let k = 1. / 3f32.sqrt();
        assert_eq!(
            q.normals,
            [[-k, -k, k], [k, -k, k], [k, k, k], [-k, k, k]]
        );
        for (p, n) in q.positions.iter().zip(q.normals) {
            // Each normal leans towards its own corner.
            assert!((p[0] * n[0]) > 0. && ((p[1] - 1.) * n[1]) > 0.);
        }
        // A spun sprite keeps unit normals leaning to the spun corners.
        let spun = sprite_quad(&Sprite { rotation: 0.7, ..Sprite {
            position: [0.; 3], size: [1., 1.], rotation: 0.,
            colour: [1.; 4], texture: 0, frame: 0, horizon: false,
        } }, Vec3::X, Vec3::Y, [0., 0., 1., 1.]);
        for (p, n) in spun.positions.iter().zip(spun.normals) {
            let (p, n) = (Vec3::from_array(*p), Vec3::from_array(n));
            assert!((n.length() - 1.).abs() < 1e-6);
            assert!((n.truncate() - p.truncate().normalize() * (2f32 / 3.).sqrt()).length() < 1e-5);
        }
    }

    #[test]
    fn wheel_spin_rings_use_the_fakelight_and_other_smoke_the_lit_stream() {
        let light = ParticleLight {
            sun: [0.75, 0.71, 0.66],
            ambient: [1.06, 1.08, 1.13],
            unlit_ambient: [1.; 3],
        };
        let lit = SmokeLight::new(&light, "SM33.tga");
        assert_eq!(lit.sun, Vec4::new(0.75, 0.71, 0.66, 1.));
        assert_eq!(lit.ambient, Vec4::new(1.06, 1.08, 1.13, 0.));
        assert_eq!(SmokeLight::new(&light, "Wheel_spin.tga").sun.w, 0.);
    }

    #[test]
    fn sprite_meshes_have_the_same_packed_quad_bytes_and_winding_as_group_mesh() {
        let input = [quad(0, false), quad(1, true), quad(2, false)];
        let mut meshes = Assets::<Mesh>::default();
        let stream = MeshStream::default();
        let grouped = meshes.add(empty_mesh());
        QuadMesh::default().update(&input, [0, 1, 2, 0, 2, 3], &grouped, &mut meshes, &stream);
        let mut singles = Vec::new();
        for q in input {
            let h = meshes.add(empty_mesh());
            QuadMesh::default().update(&[q], [0, 1, 2, 0, 2, 3], &h, &mut meshes, &stream);
            let asset = meshes.get(&h).unwrap();
            singles.extend(asset.create_packed_vertex_buffer_data());
            let Some(Indices::U32(indices)) = asset.indices() else {
                panic!("indices")
            };
            assert_eq!(indices, &[0, 1, 2, 0, 2, 3]);
        }
        assert_eq!(
            singles,
            meshes
                .get(&grouped)
                .unwrap()
                .create_packed_vertex_buffer_data()
        );
    }
}

// Append inside tyre_smoke.rs after integration. Private proposal; NOT RUN.
#[cfg(test)]
mod sprite_visibility_reset_regression {
    use super::*;
    use bevy::{
        asset::AssetPlugin, camera::visibility::VisibilityPlugin,
        mesh::skinning::SkinnedMeshInverseBindposes,
    };

    #[derive(Resource, Default)]
    struct Input(Vec<(usize, Quad)>);

    fn sync(
        mut pool: ResMut<SpritePool>,
        input: Res<Input>,
        mut commands: Commands,
        mut meshes: ResMut<Assets<Mesh>>,
        stream: Res<MeshStream>,
    ) {
        pool.update(&input.0, &mut commands, &mut meshes, &stream);
    }

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            TransformPlugin,
            VisibilityPlugin,
        ))
        .init_asset::<Mesh>()
        .init_asset::<SmokeMaterial>()
        .init_asset::<SkinnedMeshInverseBindposes>()
        .init_resource::<MeshStream>()
        .init_resource::<Input>();
        let handles = (0..5)
            .map(|_| {
                app.world_mut()
                    .resource_mut::<Assets<SmokeMaterial>>()
                    .add(SmokeMaterial::default())
            })
            .collect();
        app.insert_resource(SpritePool {
            materials: handles,
            ..default()
        });
        app.world_mut().spawn((
            Camera3d::default(),
            Transform::IDENTITY,
            crate::ViewerCamera,
        ));
        app
    }

    fn quad() -> Quad {
        Quad {
            positions: [[0., 0., -5.], [1., 0., -5.], [1., 1., -5.], [0., 1., -5.]],
            normals: [[0., 0., 1.]; 4],
            uvs: [[0., 1.], [1., 1.], [1., 0.], [0., 0.]],
            colour: [1.; 4],
        }
    }

    fn assert_visibility(app: &App, entity: Entity, visible: bool) {
        assert_eq!(
            app.world()
                .get::<InheritedVisibility>(entity)
                .unwrap()
                .get(),
            visible
        );
        assert_eq!(
            app.world().get::<ViewVisibility>(entity).unwrap().get(),
            visible
        );
        let camera = app.world().iter_entities().find(|e| e.contains::<crate::ViewerCamera>()).unwrap();
        let visible_entities = camera.get::<bevy::camera::visibility::VisibleEntities>().unwrap();
        assert_eq!(visible_entities.get(std::any::TypeId::of::<Mesh3d>()).contains(&entity), visible);
    }

    #[test]
    fn stock_visibility_schedule_observes_spawn_shrink_and_regrow_same_update() {
        let mut app = app();
        app.add_systems(
            PostUpdate,
            sync.after(TransformSystems::Propagate)
                .before(VisibilitySystems::VisibilityPropagate),
        );
        app.world_mut().resource_mut::<Input>().0 = vec![(0, quad()), (1, quad())];
        app.update();
        let ids: Vec<_> = app
            .world()
            .resource::<SpritePool>()
            .slots
            .iter()
            .map(|s| s.entity)
            .collect();
        let handles = app.world().resource::<SpritePool>().meshes.clone();
        assert_visibility(&app, ids[0], true);
        assert_visibility(&app, ids[1], true);
        app.world_mut().resource_mut::<Input>().0.truncate(1);
        app.update();
        assert_visibility(&app, ids[0], true);
        assert_visibility(&app, ids[1], false);
        app.world_mut().resource_mut::<Input>().0.clear();
        app.update();
        assert_visibility(&app, ids[0], false);
        assert_visibility(&app, ids[1], false);
        app.world_mut().resource_mut::<Input>().0 = vec![(4, quad()), (3, quad())];
        app.update();
        assert_visibility(&app, ids[0], true);
        assert_visibility(&app, ids[1], true);
        assert_eq!(app.world().resource::<SpritePool>().meshes, handles);
        assert_eq!(app.world().resource::<SpritePool>().slots[1].entity, ids[1]);
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources; real BMW runtime + keyboard R"]
    fn bmw_istanbul_real_keyboard_reset_clears_smoke_then_reuses_pool() {
        std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024)
            .spawn(|| {
                use grid_tools::source_vehicle::{Controls, DELTA, Runtime};
                let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
                let root = std::path::Path::new(&root);
                let route =
                    grid_tools::route::Route::new("tracks/europe/istanbul_park/route_0").unwrap();
                let car = crate::CarModel::new(grid_tools::car::BMW).unwrap();
                let mut runtime = Runtime::load(root, &route, &car, "slot_00", 200679).unwrap();
                let mut smoke = Smoke::new(Arc::new(Library::load(root).unwrap()));
                for t in 0..302 {
                    runtime
                        .tick(Controls {
                            throttle: 1.,
                            steering: 1.,
                            handbrake: t >= 190,
                            ..default()
                        })
                        .unwrap();
                    let car = smoke::CarInput::read(&runtime, false);
                    let eye = car.pose[12..15].try_into().unwrap();
                    smoke.tick(&[car], eye, DELTA);
                }
                assert!(smoke.particle_count() > 0);
                assert!(
                    !smoke.sprites().is_empty(),
                    "fixture must exercise an active pool"
                );
                let mut app = app();
                // Use the actual presenter draw and tick, not Input/synthetic reset.
                let pool = app.world_mut().remove_resource::<SpritePool>().unwrap();
                app.insert_resource(TyreSmoke {
                    smoke,
                    pool: Some(pool),
                    last: Some((0, runtime.ticks)),
                    meshes: vec![],
                    entities: vec![],
                    quads: vec![],
                    hidden: vec![],
                    eye: [0.; 3],
                    frames: 0,
                });
                app.insert_resource(crate::Driving {
                    runtime,
                    geometry: None,
                    paused: false,
                    failure: None,
                    notices: vec![],
                    scripted_ticks: None,
                    scripted: default(),
                    handbrake_from: None,
                    last_controls: None,
                    segment: 0,
                    started: std::time::Instant::now(),
                    opponents: None,
                    auto_reset: default(),
                    reset_line: None,
                })
                .insert_resource(ButtonInput::<KeyCode>::default())
                .add_message::<AppExit>();
                app.add_systems(Update, (crate::drive_actions, tick).chain());
                app.add_systems(
                    PostUpdate,
                    draw_sprites
                        .after(TransformSystems::Propagate)
                        .before(VisibilitySystems::VisibilityPropagate),
                );
                app.update();
                let handles = app
                    .world()
                    .resource::<TyreSmoke>()
                    .pool
                    .as_ref()
                    .unwrap()
                    .meshes
                    .clone();
                let entities: Vec<_> = app
                    .world()
                    .resource::<TyreSmoke>()
                    .pool
                    .as_ref()
                    .unwrap()
                    .slots
                    .iter()
                    .map(|s| s.entity)
                    .collect();
                assert!(!handles.is_empty());
                app.world_mut()
                    .resource_mut::<ButtonInput<KeyCode>>()
                    .press(KeyCode::KeyR);
                app.update();
                assert_eq!(app.world().resource::<crate::Driving>().segment, 1);
                assert_eq!(app.world().resource::<crate::Driving>().runtime.ticks, 0);
                let state = app.world().resource::<TyreSmoke>();
                assert_eq!(state.last, Some((1, 0)));
                assert_eq!(state.smoke.particle_count(), 0);
                assert!(state.smoke.sprites().is_empty());
                assert!(state.pool.as_ref().unwrap().slots.iter().all(|s| s.hidden));
                for &entity in &entities {
                    assert_visibility(&app, entity, false);
                }
                app.world_mut()
                    .resource_mut::<ButtonInput<KeyCode>>()
                    .reset_all();
                // Actual runtime reset re-run generates smoke again; this fixture
                // intentionally bypasses drive_tick's contacts/countdown scheduling.
                app.world_mut()
                    .resource_scope(|world, mut state: Mut<TyreSmoke>| {
                        let mut driving = world.resource_mut::<crate::Driving>();
                        for t in 0..302 {
                            driving
                                .runtime
                                .tick(Controls {
                                    throttle: 1.,
                                    steering: 1.,
                                    handbrake: t >= 190,
                                    ..default()
                                })
                                .unwrap();
                            let car = smoke::CarInput::read(&driving.runtime, false);
                            let eye = car.pose[12..15].try_into().unwrap();
                            state.smoke.tick(&[car], eye, DELTA);
                        }
                        state.last = Some((1, driving.runtime.ticks));
                    });
                app.update();
                let pool = app.world().resource::<TyreSmoke>().pool.as_ref().unwrap();
                assert!(
                    !app.world()
                        .resource::<TyreSmoke>()
                        .smoke
                        .sprites()
                        .is_empty()
                );
                assert!(pool.meshes.len() >= handles.len());
                assert_eq!(&pool.meshes[..handles.len()], handles.as_slice());
                assert_eq!(&pool.slots[..entities.len()].iter().map(|s| s.entity).collect::<Vec<_>>(), &entities);
                assert_eq!(pool.slots[0].entity, entities[0]);
                assert_visibility(&app, entities[0], true);
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
