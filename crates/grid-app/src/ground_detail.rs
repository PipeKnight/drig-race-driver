//! Ground-cover grass on the painted terrain (docs/GROUND_EFFECTS_RESEARCH.md,
//! original CMR6GroundCover: scatter 96CFA0 -> 95BC90 -> 95AF90, billboard
//! vertices 94FAF0, update 97EDE0). `grass.grs`, `ground_cover.xml` and
//! `ground_cover.pssg` (sprites and `gc_billboard.tga`) are read from the
//! purchased installation for the selected route's track. Presentation only.
use super::mesh_stream::{MeshStream, VertexLayout, put};
use bevy::camera::primitives::Aabb;
use bevy::{
    asset::{RenderAssetUsages, uuid_handle},
    mesh::{Indices, PrimitiveTopology},
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderType},
    shader::ShaderRef,
    tasks::{AsyncComputeTaskPool, Task, block_on, poll_once},
};
use std::sync::Arc;
use grid_formats::{
    binxml,
    ground_cover::{self, CoverType, Distances, Patch},
    pssg,
};
use grid_tools::route::Route;
use std::{collections::HashMap, path::Path};

/// One candidate point per 0.5 m grid cell, jittered by +-0.1 m [D 95BC90].
const CELL: f32 = 0.5;
const JITTER: f32 = 0.1;
/// LOD subset of a cell, table 0xD11698 indexed [(u & 3) + (v & 3) * 4] [V].
const SUBSETS: [u8; 16] = [0, 1, 2, 3, 3, 2, 1, 0, 1, 0, 3, 2, 2, 3, 0, 1];
/// Spatial buckets for gathering instances near the camera (AUTHORED).
const BUCKET: f32 = 16.;
/// Update when the camera moved or turned this much (AUTHORED; the original
/// regenerates instances after 10 m and orients billboards every frame).
const MOVE: f32 = 0.05;
const TURN_COS: f32 = 0.9998;
/// The vertex shader faces and fades the billboards (F580). The CPU gathers
/// every instance within its draw distance plus this margin; a gather stays
/// valid until the camera moved this far. The next one starts on the async
/// compute pool after half of it. Blended cover also gathers again, at once,
/// in a new bucket (the bucket order sets its draw order, as before).
const MARGIN: f32 = 6.;

const SHADER: Handle<Shader> = uuid_handle!("a9b2b0e1-4de3-4237-a7bb-c3488244f121");
type CoverMaterial = ExtendedMaterial<StandardMaterial, CoverFacing>;

/// The main camera every pass faces, as the CPU path used it.
#[derive(Clone, Copy, Debug, Default, ShaderType)]
struct CoverView {
    right: Vec4,
    up: Vec4,
    eye: Vec4,
    /// x: fade start as a share of the draw distance.
    fade: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
struct CoverFacing {
    #[uniform(100)]
    view: CoverView,
}

impl MaterialExtension for CoverFacing {
    fn vertex_shader() -> ShaderRef {
        SHADER.into()
    }
    fn prepass_vertex_shader() -> ShaderRef {
        SHADER.into()
    }
    fn deferred_vertex_shader() -> ShaderRef {
        SHADER.into()
    }
}

/// Coverage requires multisampling. Native DLAA shares the existing single-sample
/// soft/mask choices without changing the ordinary baseline coverage path.
fn cover_material(
    image: Handle<Image>,
    fade_start: f32,
    graphics: &super::graphics::Settings,
) -> CoverMaterial {
    let no_msaa = graphics.enhanced() || graphics.anti_aliasing == 3;
    let alpha_mode = if no_msaa && graphics.grass_soft_alpha {
        AlphaMode::Blend
    } else if no_msaa {
        AlphaMode::Mask(0.5)
    } else {
        AlphaMode::AlphaToCoverage
    };
    ExtendedMaterial {
        base: StandardMaterial {
            base_color_texture: Some(image),
            alpha_mode,
            perceptual_roughness: 1.,
            reflectance: 0.,
            double_sided: false,
            cull_mode: None,
            ..default()
        },
        extension: CoverFacing {
            view: CoverView {
                fade: Vec4::new(fade_start, 0., 0., 0.),
                ..default()
            },
        },
    }
}

#[derive(Clone, Copy, Debug)]
struct Instance {
    position: Vec3,
    normal: Vec3,
    /// Index into the flattened sprite list.
    sprite: u16,
    subset: u8,
    mirror: bool,
    width: f32,
    height: f32,
}

#[derive(Clone, Debug)]
struct SpriteQuad {
    corners: [[f32; 2]; 4],
    uvs: [[f32; 2]; 4],
}

/// Original scatter LCG: x = x * 0x41C64E6D + 0x40584C0F, (x & 0xFFFF) / 65536 [D].
struct Lcg(u32);
impl Lcg {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(0x41c6_4e6d).wrapping_add(0x4058_4c0f);
        (self.0 & 0xffff) as f32 / 65_536.
    }
}

/// Scatter every painted triangle on its dominant-axis 0.5 m grid.
fn scatter(patches: &[Patch], types: &[CoverType]) -> (Vec<Instance>, Vec<SpriteQuad>) {
    let mut quads = Vec::new();
    // (type nibble, [(sprite index, cumulative weight, height range, width range)])
    let mut kinds = Vec::new();
    for t in types
        .iter()
        .filter(|t| t.mode == 0 && !t.sprites.is_empty())
    {
        let total: f32 = t.sprites.iter().map(|s| s.weight.max(0.)).sum();
        if total <= 0. {
            continue;
        }
        let mut cumulative = 0.;
        let sprites: Vec<_> = t
            .sprites
            .iter()
            .map(|s| {
                cumulative += s.weight.max(0.) / total;
                quads.push(SpriteQuad {
                    corners: s.corners,
                    uvs: s.uvs,
                });
                (quads.len() - 1, cumulative, s.height_range, s.width_range)
            })
            .collect();
        kinds.push((t.index - 1, sprites));
    }
    let mut instances = Vec::new();
    for patch in patches {
        for tri in &patch.triangles {
            let p = tri.map(|i| Vec3::from_array(patch.positions[i as usize]));
            let n = (p[1] - p[0]).cross(p[2] - p[0]);
            if n.length_squared() < 1e-12 {
                continue;
            }
            let mut normal = n.normalize();
            // Rasterise on the axis the normal mostly faces [D].
            let axis = if normal.y.abs() >= std::f32::consts::FRAC_1_SQRT_2 {
                1
            } else if normal.x.abs() >= normal.z.abs() {
                0
            } else {
                2
            };
            if normal[axis] < 0. && axis == 1 {
                normal = -normal;
            }
            let (ua, va) = match axis {
                0 => (1, 2),
                1 => (0, 2),
                _ => (0, 1),
            };
            let weights: Vec<f32> = kinds
                .iter()
                .map(|(nibble, _)| {
                    tri.iter()
                        .map(|&i| patch.densities[i as usize][*nibble])
                        .sum::<f32>()
                        / 3.
                })
                .collect();
            let total: f32 = weights.iter().sum();
            if total <= 0. {
                continue;
            }
            let uv = p.map(|q| [q[ua], q[va]]);
            let lo = [
                uv.iter().map(|c| c[0]).fold(f32::INFINITY, f32::min),
                uv.iter().map(|c| c[1]).fold(f32::INFINITY, f32::min),
            ];
            let hi = [
                uv.iter().map(|c| c[0]).fold(f32::NEG_INFINITY, f32::max),
                uv.iter().map(|c| c[1]).fold(f32::NEG_INFINITY, f32::max),
            ];
            let area = (uv[1][0] - uv[0][0]) * (uv[2][1] - uv[0][1])
                - (uv[2][0] - uv[0][0]) * (uv[1][1] - uv[0][1]);
            if area.abs() < 1e-9 {
                continue;
            }
            let ia = (lo[0] / CELL).floor() as i32..=(hi[0] / CELL).floor() as i32;
            let ib = (lo[1] / CELL).floor() as i32..=(hi[1] / CELL).floor() as i32;
            for a in ia {
                for b in ib.clone() {
                    // Deterministic per-cell seed (AUTHORED; the original seed
                    // sequence depends on its regeneration order).
                    let mut r = Lcg((a as u32).wrapping_mul(0x9e37_79b9)
                        ^ (b as u32).wrapping_mul(0x85eb_ca6b)
                        ^ axis as u32);
                    let x = (a as f32 + 0.5) * CELL + (r.next() * 2. - 1.) * JITTER;
                    let y = (b as f32 + 0.5) * CELL + (r.next() * 2. - 1.) * JITTER;
                    // Barycentric inside test in the projection plane.
                    let w1 = ((x - uv[0][0]) * (uv[2][1] - uv[0][1])
                        - (uv[2][0] - uv[0][0]) * (y - uv[0][1]))
                        / area;
                    let w2 = ((uv[1][0] - uv[0][0]) * (y - uv[0][1])
                        - (x - uv[0][0]) * (uv[1][1] - uv[0][1]))
                        / area;
                    if w1 < 0. || w2 < 0. || w1 + w2 > 1. {
                        continue;
                    }
                    // Type from the cumulative weights, accepted with its weight [D].
                    let roll = r.next() * total;
                    let mut acc = 0.;
                    let Some(k) = weights.iter().position(|w| {
                        acc += w;
                        roll < acc
                    }) else {
                        continue;
                    };
                    if r.next() >= weights[k] {
                        continue;
                    }
                    let position = p[0] + (p[1] - p[0]) * w1 + (p[2] - p[0]) * w2;
                    let pick = r.next();
                    let sprites = &kinds[k].1;
                    let &(sprite, _, h, w) = sprites
                        .iter()
                        .find(|s| pick < s.1)
                        .unwrap_or(sprites.last().unwrap());
                    let height = h[0] + (h[1] - h[0]) * r.next();
                    let width = w[0] + (w[1] - w[0]) * r.next();
                    instances.push(Instance {
                        position,
                        normal,
                        sprite: sprite as u16,
                        subset: SUBSETS[((b & 3) + (a & 3) * 4) as usize],
                        mirror: r.next() < 0.5,
                        width,
                        height,
                    });
                }
            }
        }
    }
    (instances, quads)
}

/// The scattered cover, shared with the gathers on the async pool.
struct Cover {
    instances: Vec<Instance>,
    quads: Vec<SpriteQuad>,
    buckets: HashMap<(i32, i32), Vec<u32>>,
    distances: Distances,
}

/// A finished gather: the eye and the instances in draw order.
struct Gathered {
    eye: Vec3,
    indices: Vec<u32>,
}

#[derive(Resource)]
pub(crate) struct GroundDetail {
    cover: Arc<Cover>,
    mesh: Handle<Mesh>,
    material: Handle<CoverMaterial>,
    /// The cover entity (alpha-blended in Enhanced: its sort centre).
    entity: Entity,
    last: Option<(Vec3, Vec3)>,
    /// Alpha blended (Enhanced soft alpha): sorted by its drawn quads.
    blended: bool,
    /// Instances in the mesh, in draw order, and the eye they were gathered at.
    gathered: Vec<u32>,
    gathered_at: Option<Vec3>,
    capacity: usize,
    layout: Option<VertexLayout>,
    pending: Option<Task<Gathered>>,
    /// Billboard slots of order-free (masked or alpha-to-coverage) cover.
    slots: Slots,
}

const FREE: u32 = u32::MAX;

/// Stable billboard slots: an instance keeps its slot while gathers hold it,
/// so a gather writes only the slots that changed instead of the whole
/// capacity (F842). Free slots hold `PAD`, which is never drawn. Masked and
/// alpha-to-coverage cover (depth tested) take any free slot
/// ([`Slots::update`]); blended cover keeps the gather's draw order in the
/// buffer ([`Slots::update_ordered`]).
#[derive(Default)]
struct Slots {
    /// Slot -> instance, or `FREE`.
    held: Vec<u32>,
    /// Instance -> slot, or `FREE`.
    slot_of: Vec<u32>,
    /// Free slots; the lowest is taken first.
    free: Vec<u32>,
    /// Instances of the gather being applied.
    wanted: Vec<bool>,
}

impl Slots {
    /// Slots from `first` on hold `indices`, the rest of `capacity` is free.
    fn reset(&mut self, indices: &[u32], first: usize, capacity: usize, instances: usize) {
        self.slot_of.clear();
        self.slot_of.resize(instances, FREE);
        self.wanted.clear();
        self.wanted.resize(instances, false);
        self.held.clear();
        self.held.resize(capacity, FREE);
        self.held[first..first + indices.len()].copy_from_slice(indices);
        for (slot, &i) in indices.iter().enumerate() {
            self.slot_of[i as usize] = (first + slot) as u32;
        }
        self.free = (0..capacity as u32)
            .rev()
            .filter(|&s| self.held[s as usize] == FREE)
            .collect();
    }

    /// Moves to the gather `indices` (which must fit the capacity) and returns
    /// the changed slots as sorted runs; runs closer than `GAP` slots are joined.
    fn update(&mut self, indices: &[u32]) -> Vec<std::ops::Range<u32>> {
        for &i in indices {
            self.wanted[i as usize] = true;
        }
        let mut dirty = Vec::new();
        for (slot, held) in self.held.iter_mut().enumerate() {
            if *held != FREE && !self.wanted[*held as usize] {
                self.slot_of[*held as usize] = FREE;
                *held = FREE;
                self.free.push(slot as u32);
                dirty.push(slot as u32);
            }
        }
        // Lowest free slots first keeps the drawn range compact.
        self.free.sort_unstable_by(|a, b| b.cmp(a));
        for &i in indices {
            self.wanted[i as usize] = false;
            if self.slot_of[i as usize] != FREE {
                continue;
            }
            let slot = self.free.pop().expect("gather fits the capacity");
            self.held[slot as usize] = i;
            self.slot_of[i as usize] = slot;
            dirty.push(slot);
        }
        dirty.sort_unstable();
        dirty.dedup();
        runs(dirty)
    }

    /// Moves to the gather `indices`, keeping their order in the buffer (free
    /// slots between them are not drawn), and returns the changed slots as
    /// [`Slots::update`]; `None` (state unchanged) when they do not fit in
    /// order. The gather's order is a fixed global order (buckets by x, then
    /// z, see [`Cover::gather`]), so as the eye moves, instances leave and
    /// arrive at the ends of each bucket column; a kept instance stays in its
    /// slot when the arrivals before it fit in the free slots before it, and
    /// arrivals are put just before the next kept instance, in the slots the
    /// leavers there freed.
    fn update_ordered(&mut self, indices: &[u32]) -> Option<Vec<std::ops::Range<u32>>> {
        let capacity = self.held.len();
        let mut next = vec![FREE; capacity];
        let mut pending: Vec<u32> = Vec::new();
        let mut at = 0;
        for &i in indices {
            let slot = self.slot_of[i as usize];
            if slot != FREE && slot as usize >= at + pending.len() {
                let first = slot as usize - pending.len();
                next[first..slot as usize].copy_from_slice(&pending);
                pending.clear();
                next[slot as usize] = i;
                at = slot as usize + 1;
            } else {
                pending.push(i);
            }
        }
        if at + pending.len() > capacity {
            return None;
        }
        next[at..at + pending.len()].copy_from_slice(&pending);
        let mut dirty = Vec::new();
        for (slot, (old, new)) in self.held.iter().zip(&next).enumerate() {
            if old != new {
                if *old != FREE {
                    self.slot_of[*old as usize] = FREE;
                }
                dirty.push(slot as u32);
            }
        }
        for &slot in &dirty {
            let new = next[slot as usize];
            if new != FREE {
                self.slot_of[new as usize] = slot;
            }
        }
        self.held = next;
        self.free = (0..capacity as u32)
            .rev()
            .filter(|&s| self.held[s as usize] == FREE)
            .collect();
        Some(runs(dirty))
    }
}

/// Sorted dirty slots as runs; runs closer than `GAP` slots are joined.
fn runs(dirty: Vec<u32>) -> Vec<std::ops::Range<u32>> {
    const GAP: u32 = 8;
    let mut runs: Vec<std::ops::Range<u32>> = Vec::new();
    for slot in dirty {
        match runs.last_mut() {
            Some(run) if slot <= run.end + GAP => run.end = slot + 1,
            _ => runs.push(slot..slot + 1),
        }
    }
    runs
}

struct Loaded {
    instances: Vec<Instance>,
    quads: Vec<SpriteQuad>,
    distances: Distances,
    texture: Vec<u8>,
}

fn load(root: &Path, route: &Route) -> Result<Loaded, Box<dyn std::error::Error>> {
    let started = std::time::Instant::now();
    let patches =
        ground_cover::read_patches(&super::read(&root.join(route.track_file("grass.grs")))?)?;
    let config = super::read(&root.join(route.track_file("ground_cover.xml")))?;
    let distances = ground_cover::read_distances(&binxml::read(&config)?)?;
    let package = route.track_file("ground_cover.pssg");
    let bytes = super::read(&root.join(&package))?;
    let types = ground_cover::read_types(&pssg::read_tree(&bytes)?)?;
    let textures = super::ground_effect_assets::package(root, &package, &["gc_billboard.tga"])?;
    let texture = textures
        .get("gc_billboard.tga")
        .ok_or("ground cover texture absent")?
        .clone();
    let read_ms = started.elapsed().as_millis();
    let (instances, quads) = scatter(&patches, &types);
    println!(
        "ground_detail enabled=true track={} patches={} types={} sprites={} instances={} lods={:?} fade_start={} billboards=camera-facing read_ms={read_ms} total_ms={} fidelity=experimental",
        route.track(),
        patches.len(),
        types.len(),
        quads.len(),
        instances.len(),
        distances.lods,
        distances.fade_start,
        started.elapsed().as_millis()
    );
    Ok(Loaded {
        instances,
        quads,
        distances,
        texture,
    })
}

pub(crate) fn install(
    app: &mut App,
    root: &Path,
    route: &Route,
) -> Result<(), Box<dyn std::error::Error>> {
    let loaded = load(root, route)?;
    super::ground_effect_assets::install_timing(app);
    app.world_mut().resource_mut::<Assets<Shader>>().insert(
        SHADER.id(),
        Shader::from_wgsl(include_str!("ground_cover.wgsl"), "grid_ground_cover.wgsl"),
    )?;
    app.add_plugins(MaterialPlugin::<CoverMaterial>::default())
        .init_resource::<MeshStream>()
        .insert_resource(PendingDetail(Some(loaded)))
        .add_systems(Startup, start)
        .add_systems(
            Update,
            present
                .after(super::drive_present)
                .after(super::update_orbit)
                .after(super::ground_effect_assets::validation_camera),
        );
    Ok(())
}

#[derive(Resource)]
struct PendingDetail(Option<Loaded>);

fn start(
    mut commands: Commands,
    mut pending: ResMut<PendingDetail>,
    graphics: Res<super::graphics::Settings>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<CoverMaterial>>,
) {
    let Some(loaded) = pending.0.take() else {
        return;
    };
    let image = match super::ground_effect_assets::image(&loaded.texture, true) {
        Ok(image) => images.add(image),
        Err(error) => {
            warn!("ground cover texture: {error}");
            return;
        }
    };
    let mut buckets: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
    for (i, instance) in loaded.instances.iter().enumerate() {
        let key = (
            (instance.position.x / BUCKET).floor() as i32,
            (instance.position.z / BUCKET).floor() as i32,
        );
        buckets.entry(key).or_default().push(i as u32);
    }
    let mesh = meshes.add(cover_mesh(&[], 1));
    let material = cover_material(image, loaded.distances.fade_start, &graphics);
    let alpha_mode = material.base.alpha_mode;
    let material = materials.add(material);
    let entity = commands
        .spawn((
            Name::new("ground cover"),
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::IDENTITY,
            bevy::camera::visibility::NoFrustumCulling,
            bevy::light::NotShadowCaster,
        ))
        .id();
    if !graphics.grass_receive_shadows {
        commands
            .entity(entity)
            .insert(bevy::light::NotShadowReceiver);
    }
    println!(
        "ground_detail_shading receive_shadows={} ssao={} alpha={alpha_mode:?} normals=terrain",
        graphics.grass_receive_shadows, graphics.ambient_occlusion
    );
    commands.insert_resource(GroundDetail {
        cover: Arc::new(Cover {
            instances: loaded.instances,
            quads: loaded.quads,
            buckets,
            distances: loaded.distances,
        }),
        mesh,
        material,
        entity,
        last: None,
        blended: alpha_mode == AlphaMode::Blend,
        gathered: Vec::new(),
        gathered_at: None,
        capacity: 0,
        layout: None,
        pending: None,
        slots: Slots::default(),
    });
}

/// One billboard's vertices: the centre four times, the corner offsets in
/// UV1 and the draw distance in the colour's alpha (the shader faces, fades
/// and drops it). Padding has distance 0 and is never drawn.
#[derive(Clone, Copy)]
struct Billboard {
    centre: [f32; 3],
    normal: [f32; 3],
    uvs: [[f32; 2]; 4],
    offsets: [[f32; 2]; 4],
    limit: f32,
}

const PAD: Billboard = Billboard {
    centre: [0.; 3],
    normal: [0., 1., 0.],
    uvs: [[0.; 2]; 4],
    offsets: [[0.; 2]; 4],
    limit: 0.,
};

fn padded(billboards: &[Billboard], capacity: usize) -> impl Iterator<Item = &Billboard> {
    billboards
        .iter()
        .chain(std::iter::repeat_n(&PAD, capacity.saturating_sub(billboards.len())))
}

fn cover_mesh(billboards: &[Billboard], capacity: usize) -> Mesh {
    let (mut positions, mut normals, mut uvs, mut offsets, mut colours, mut indices) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for b in padded(billboards, capacity) {
        let base = positions.len() as u32;
        positions.extend([b.centre; 4]);
        normals.extend([b.normal; 4]);
        uvs.extend(b.uvs);
        offsets.extend(b.offsets);
        colours.extend([[1., 1., 1., b.limit]; 4]);
        indices.extend(TRIANGLES.map(|k| base + k));
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, offsets);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colours);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// The packed vertex bytes of `cover_mesh(billboards, capacity)`.
fn pack(billboards: &[Billboard], capacity: usize, layout: &VertexLayout, bytes: &mut Vec<u8>) {
    let at = |attribute| layout.offset(attribute).expect("cover attribute");
    let (p, n, uv, o, c) = (
        at(Mesh::ATTRIBUTE_POSITION),
        at(Mesh::ATTRIBUTE_NORMAL),
        at(Mesh::ATTRIBUTE_UV_0),
        at(Mesh::ATTRIBUTE_UV_1),
        at(Mesh::ATTRIBUTE_COLOR),
    );
    let start = bytes.len();
    bytes.resize(start + capacity * 4 * layout.stride, 0);
    let mut vertex = start;
    for b in padded(billboards, capacity) {
        for k in 0..4 {
            put(bytes, vertex + p, b.centre);
            put(bytes, vertex + n, b.normal);
            put(bytes, vertex + uv, b.uvs[k]);
            put(bytes, vertex + o, b.offsets[k]);
            put(bytes, vertex + c, [1., 1., 1., b.limit]);
            vertex += layout.stride;
        }
    }
}

impl Cover {
    /// Gather at `eye`.
    fn gathered(&self, eye: Vec3) -> Gathered {
        let mut indices = Vec::new();
        self.gather(eye, MARGIN, &mut indices);
        Gathered { eye, indices }
    }

    fn billboard(&self, i: u32) -> Billboard {
        let g = &self.instances[i as usize];
        let quad = &self.quads[g.sprite as usize];
        Billboard {
            centre: g.position.to_array(),
            normal: g.normal.to_array(),
            // Mirroring swaps left and right UVs [D 94FAF0].
            uvs: std::array::from_fn(|k| quad.uvs[if g.mirror { k ^ 2 } else { k }]),
            offsets: std::array::from_fn(|k| {
                let [x, y] = quad.corners[k];
                [x * g.width, y * g.height]
            }),
            limit: self.distances.lods[g.subset as usize],
        }
    }

    /// Every instance within its subset's draw distance plus `margin` of
    /// `eye`, in the bucket order the draw used before F580.
    fn gather(&self, eye: Vec3, margin: f32, out: &mut Vec<u32>) {
        out.clear();
        let far = self.distances.lods.iter().copied().fold(0., f32::max);
        let reach = (far / BUCKET).ceil() as i32 + 1;
        let center = (
            (eye.x / BUCKET).floor() as i32,
            (eye.z / BUCKET).floor() as i32,
        );
        for dx in -reach..=reach {
            for dz in -reach..=reach {
                let Some(list) = self.buckets.get(&(center.0 + dx, center.1 + dz)) else {
                    continue;
                };
                for &i in list {
                    let g = &self.instances[i as usize];
                    let limit = self.distances.lods[g.subset as usize];
                    if g.position.distance(eye) >= limit + margin {
                        continue;
                    }
                    out.push(i);
                }
            }
        }
    }
}

/// Corners are (left, top), (left, bottom), (right, top), (right, bottom).
const TRIANGLES: [u32; 6] = [0, 1, 2, 2, 1, 3];

impl GroundDetail {
    /// Puts a gather in the mesh: only the billboard slots that changed are
    /// written over the GPU allocation (F842), or a new asset when the cover
    /// outgrew its capacity (AUTHORED headroom, so the asset is rarely replaced
    /// again) or blended cover found no room in its order.
    fn apply(&mut self, done: Gathered, meshes: &mut Assets<Mesh>, stream: &MeshStream) {
        let instances = self.cover.instances.len();
        let runs = match self.layout {
            Some(_) if done.indices.len() <= self.capacity => Some(if !self.blended {
                self.slots.update(&done.indices)
            } else if let Some(runs) = self.slots.update_ordered(&done.indices) {
                runs
            } else {
                // No room in order: lay the gather out again over the whole
                // allocation, centred so it can grow at both ends.
                let first = (self.capacity - done.indices.len()) / 2;
                self.slots
                    .reset(&done.indices, first, self.capacity, instances);
                std::iter::once(0..self.capacity as u32).collect()
            }),
            _ => None,
        };
        match (runs, self.layout.as_ref()) {
            (Some(runs), Some(layout)) => {
                for run in runs {
                    let billboards: Vec<_> = self.slots.held[run.start as usize..run.end as usize]
                        .iter()
                        .map(|&i| {
                            if i == FREE {
                                PAD
                            } else {
                                self.cover.billboard(i)
                            }
                        })
                        .collect();
                    stream.write_at(self.mesh.id(), run.start * 4, layout.stride, |out| {
                        pack(&billboards, billboards.len(), layout, out)
                    });
                }
            }
            _ => {
                let billboards: Vec<_> = done
                    .indices
                    .iter()
                    .map(|&i| self.cover.billboard(i))
                    .collect();
                self.capacity = (billboards.len() + billboards.len() / 4).max(1);
                let asset = cover_mesh(&billboards, self.capacity);
                self.layout = Some(VertexLayout::of(&asset));
                if let Some(mut target) = meshes.get_mut(&self.mesh) {
                    *target = asset;
                }
                self.slots.reset(&done.indices, 0, self.capacity, instances);
            }
        }
        self.gathered = done.indices;
        self.gathered_at = Some(done.eye);
    }
}

/// Faces the cover to the camera when it moved or turned past the threshold:
/// the vertex shader expands and fades the billboards from the camera given
/// here (F580). The instances are gathered again as `MARGIN` says, off the
/// main thread when there is time (`GroundDetail::apply` puts a gather in
/// the mesh).
fn present(
    detail: Option<ResMut<GroundDetail>>,
    camera: Query<&Transform, With<super::ViewerCamera>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<CoverMaterial>>,
    stream: Res<MeshStream>,
) {
    let Some(mut detail) = detail else { return };
    let Ok(camera) = camera.single() else { return };
    let eye = camera.translation;
    let forward = camera.forward().as_vec3();
    if let Some((last_eye, last_forward)) = detail.last
        && last_eye.distance(eye) < MOVE
        && last_forward.dot(forward) > TURN_COS
    {
        return;
    }
    let detail = &mut *detail;
    detail.last = Some((eye, forward));
    let start = std::time::Instant::now();
    let (right, up) = (camera.right().as_vec3(), camera.up().as_vec3());
    let bucket = |p: Vec3| ((p.x / BUCKET).floor() as i32, (p.z / BUCKET).floor() as i32);
    if let Some(done) = detail.pending.as_mut().and_then(|task| block_on(poll_once(task))) {
        detail.pending = None;
        detail.apply(done, &mut meshes, &stream);
    }
    let moved = detail.gathered_at.map(|at| (at.distance(eye), bucket(at) != bucket(eye)));
    match moved {
        Some((distance, new_bucket)) if distance <= MARGIN && !(detail.blended && new_bucket) => {
            if detail.pending.is_none() && distance > MARGIN / 2. {
                let cover = detail.cover.clone();
                detail.pending =
                    Some(AsyncComputeTaskPool::get().spawn(async move { cover.gathered(eye) }));
            }
        }
        // First frame, out of the gather's reach, or blended cover in a new
        // bucket: gather now.
        _ => {
            detail.pending = None;
            let done = detail.cover.gathered(eye);
            detail.apply(done, &mut meshes, &stream);
        }
    }
    if let Some(mut material) = materials.get_mut(&detail.material) {
        let view = &mut material.extension.view;
        view.right = right.extend(0.);
        view.up = up.extend(0.);
        view.eye = eye.extend(1.);
    }
    // Blended (Enhanced soft alpha) cover sorts by the centre of the drawn
    // quads, as the unpadded mesh replaced on every rebuild before F392; an
    // empty cover was three degenerate vertices at the origin (F398). The
    // corners are the shader's. Masked cover is not sorted.
    if detail.blended {
        let cover = &detail.cover;
        let corners = detail.gathered.iter().flat_map(|&i| {
            let g = &cover.instances[i as usize];
            let shown = g.position.distance(eye) < cover.distances.lods[g.subset as usize];
            let quad = &cover.quads[g.sprite as usize];
            let corners: [Vec3; 4] = std::array::from_fn(|k| {
                let [x, y] = quad.corners[k];
                g.position + right * (x * g.width) + up * (y * g.height)
            });
            corners.into_iter().take(if shown { 4 } else { 0 })
        });
        let centre = Aabb::enclosing(corners).map_or(Vec3::ZERO, |aabb| aabb.center.into());
        stream.sort_centre(detail.entity, centre);
    }
    super::ground_effect_assets::charge(super::ground_effect_assets::GRASS, start);
}

#[cfg(test)]
mod tests {
    use super::*;
    use grid_formats::ground_cover::Sprite;

    #[test]
    fn slots_keep_held_instances_and_write_only_changes() {
        let mut slots = Slots::default();
        slots.reset(&[10, 11, 12, 13], 0, 8, 40);
        // 11 leaves, 20 and 21 arrive: 11's slot is reused first, then slot 4.
        let runs = slots.update(&[13, 12, 10, 20, 21]);
        assert_eq!(slots.held, [10, 20, 12, 13, 21, FREE, FREE, FREE]);
        assert_eq!(runs, std::iter::once(1..5).collect::<Vec<_>>());
        // The same gather again writes nothing.
        assert!(slots.update(&[10, 12, 13, 20, 21]).is_empty());
        // Far changes stay separate runs.
        slots.reset(&(0..30).collect::<Vec<_>>(), 0, 40, 40);
        let mut next: Vec<u32> = (1..29).collect();
        next.push(35);
        let runs = slots.update(&next);
        assert_eq!(runs, [0..1, 29..30]);
        assert_eq!(slots.held[0], 35);
        assert_eq!(slots.held[29], FREE);
        for (slot, &i) in slots.held.iter().enumerate() {
            if i != FREE {
                assert_eq!(slots.slot_of[i as usize], slot as u32);
            }
        }
    }

    #[test]
    fn ordered_slots_keep_the_gather_order_and_write_only_changes() {
        // Instances in the global order of `Cover::gather`: bucket x, bucket z,
        // then 3 per bucket; a gather takes the 7x7 buckets around the eye.
        const SIDE: u32 = 40;
        let gather = |cx: u32, cz: u32| -> Vec<u32> {
            let mut out = Vec::new();
            for x in cx - 3..=cx + 3 {
                for z in cz - 3..=cz + 3 {
                    out.extend((0..3).map(|k| (x * SIDE + z) * 3 + k));
                }
            }
            out
        };
        let first = gather(10, 10);
        let capacity = first.len() + first.len() / 4;
        let mut slots = Slots::default();
        slots.reset(&first, 0, capacity, (SIDE * SIDE * 3) as usize);
        let (mut cx, mut cz, mut written, mut rebuilds) = (10, 10, 0, 0);
        for step in 0..24 {
            // Diagonal drive: north for two buckets, then east for one.
            if step % 3 == 2 {
                cx += 1
            } else {
                cz += 1
            }
            let indices = gather(cx, cz);
            match slots.update_ordered(&indices) {
                Some(runs) => {
                    written += runs.iter().map(|run| run.len()).sum::<usize>();
                }
                None => {
                    // As `GroundDetail::apply`: the whole allocation, centred.
                    rebuilds += 1;
                    written += capacity;
                    let first = (capacity - indices.len()) / 2;
                    slots.reset(&indices, first, capacity, (SIDE * SIDE * 3) as usize);
                }
            }
            let drawn: Vec<u32> = slots.held.iter().copied().filter(|&i| i != FREE).collect();
            assert_eq!(drawn, indices, "step {step}");
            for (slot, &i) in slots.held.iter().enumerate() {
                if i != FREE {
                    assert_eq!(slots.slot_of[i as usize], slot as u32);
                }
            }
            // The same gather again writes nothing.
            assert_eq!(slots.update_ordered(&indices), Some(Vec::new()));
        }
        // A whole-buffer write per gather would be 24 * capacity slots. Each
        // step east appends a column at the end, which the slack after a
        // centred layout holds about once: east steps rewrite the buffer,
        // north steps write a bucket per column (1707 of 4392 slots here).
        assert!(rebuilds <= 8, "{rebuilds} rebuilds");
        assert!(
            written < 24 * capacity / 2,
            "{written} of {} slots written",
            24 * capacity
        );
    }

    #[test]
    fn actual_cover_material_uses_soft_alpha_for_native_dlaa_only() {
        let texture: Handle<Image> = uuid_handle!("8529d0c6-6f05-492a-97af-3c1659e8fd53");
        for enhanced in [false, true] {
            for aa in 0..=3 {
                for soft in [false, true] {
                    let graphics = super::super::graphics::Settings {
                        preset: if enhanced {
                            super::super::graphics::Preset::Enhanced
                        } else {
                            super::super::graphics::Preset::Baseline
                        },
                        anti_aliasing: aa,
                        grass_soft_alpha: soft,
                        ..default()
                    };
                    let material = cover_material(texture.clone(), 0.7, &graphics);
                    let expected = match (enhanced, aa, soft) {
                        (false, 0..=2, _) => AlphaMode::AlphaToCoverage,
                        (_, _, true) => AlphaMode::Blend,
                        _ => AlphaMode::Mask(0.5),
                    };
                    assert_eq!(material.base.alpha_mode, expected);
                    assert_eq!(material.base.base_color_texture, Some(texture.clone()));
                    assert_eq!(material.base.base_color, Color::WHITE);
                    assert_eq!(material.base.perceptual_roughness, 1.);
                    assert_eq!(material.base.reflectance, 0.);
                    assert!(!material.base.double_sided);
                    assert_eq!(material.base.cull_mode, None);
                    assert_eq!(material.extension.view.fade, Vec4::new(0.7, 0., 0., 0.));
                    assert_eq!(material.extension.view.eye, Vec4::ZERO);
                    assert_eq!(material.extension.view.right, Vec4::ZERO);
                    assert_eq!(material.extension.view.up, Vec4::ZERO);
                    // The actual Bevy specialization follows the same Blend branch
                    // used by GroundDetail.blended for sorted billboard gathering.
                    if !enhanced && aa == 3 && soft {
                        let key = bevy::pbr::alpha_mode_pipeline_key(
                            material.base.alpha_mode,
                            &Msaa::Off,
                        );
                        assert_eq!(key, bevy::pbr::MeshPipelineKey::BLEND_ALPHA);
                        assert!(!key.contains(bevy::pbr::MeshPipelineKey::MAY_DISCARD));
                    }
                }
            }
        }
    }

    #[test]
    fn scatter_density_subsets_and_inside() {
        let patch = Patch {
            min: [0.; 3],
            max: [10., 0., 10.],
            positions: vec![[0., 0., 0.], [10., 0., 0.], [0., 0., 10.], [10., 0., 10.]],
            densities: vec![
                std::array::from_fn(|i| if i == 0 { 1. } else { 0. }),
                std::array::from_fn(|i| if i == 0 { 1. } else { 0. }),
                std::array::from_fn(|i| if i == 0 { 1. } else { 0. }),
                [0.; 12],
            ],
            triangles: vec![[0, 1, 2], [1, 3, 2]],
        };
        let sprite = Sprite {
            name: "a".into(),
            weight: 1.,
            height_range: [1., 1.5],
            width_range: [1., 1.5],
            corners: [[-0.5, 0.2], [-0.5, 0.], [0.5, 0.2], [0.5, 0.]],
            uvs: [[0., 0.], [0., 1.], [1., 0.], [1., 1.]],
        };
        let types = vec![CoverType {
            index: 1,
            mode: 0,
            sprites: vec![sprite],
        }];
        let (instances, quads) = scatter(&[patch], &types);
        assert_eq!(quads.len(), 1);
        // Full-density triangle (50 m^2 at 4 cells/m^2) plus a 2/3-density one.
        let first = instances
            .iter()
            .filter(|g| g.position.x + g.position.z <= 10.)
            .count();
        assert!((190..=210).contains(&first), "{first}");
        let second = instances.len() - first;
        assert!((110..=155).contains(&second), "{second}");
        assert!(instances.iter().all(|g| (0. ..=10.).contains(&g.position.x)
            && (0. ..=10.).contains(&g.position.z)
            && g.position.y == 0.
            && g.normal.y > 0.99
            && (1. ..=1.5).contains(&g.width)));
        for s in 0..4 {
            let n = instances.iter().filter(|g| g.subset == s).count();
            assert!(n * 6 > instances.len(), "subset {s} {n}");
        }
    }
}
