//! Tyre skid marks for every car, following the original skid-mark plugin
//! (docs/GROUND_EFFECTS_RESEARCH.md, emission 73F860, vertices 739570).
//! The surface profile `skidmarks/road_tm.xml` (Lacetti `vehicle_skid_type`
//! 4) and the atlas `skidmarks/road_tm.pssg` are read from the purchased
//! installation. Presentation only; never feeds physics.
use super::mesh_stream::{MeshStream, VertexLayout, put};
use super::wheel_ground::{Wheel, WheelGround};
use bevy::{
    asset::{RenderAssetUsages, uuid_handle},
    camera::primitives::Aabb,
    mesh::{Indices, MeshVertexBufferLayoutRef, PrimitiveTopology},
    pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline},
    prelude::*,
    render::render_resource::{AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError},
};
use grid_formats::skidmarks::{Layer, Profile};
use std::path::Path;
use bevy::shader::ShaderRef;


/// Default-off diagnostic; no smoke/skid simulation or atlas changes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum OrderMode {
    #[default]
    Off,
    Control,
    Isolated,
    GroundFirst,
}
impl OrderMode {
    pub(crate) fn parse(value: &str) -> Result<Self, &'static str> {
        match value {
            "off" => Ok(Self::Off),
            "control" => Ok(Self::Control),
            "isolated" => Ok(Self::Isolated),
            "ground-first" => Ok(Self::GroundFirst),
            _ => Err("--skid-order needs off, control, isolated or ground-first"),
        }
    }
    pub(crate) fn sort_bias(self) -> f32 {
        match self { Self::Off | Self::Control => 50., Self::Isolated => 0., Self::GroundFirst => -1024. }
    }
}

/// Deliberately only two scalar values; absent CLI preserves the stock path.
pub(crate) fn parse_reflectance(value: &str) -> Result<f32, &'static str> {
    match value {
        "0.5" => Ok(0.5),
        "0" => Ok(0.),
        _ => Err("--skid-reflectance needs 0.5 (historical PBR control) or 0"),
    }
}

// StandardMaterial.depth_bias sets BOTH a transparent sort offset and raster
// depth bias. This extension keeps raster constant50 while permitting bounded diagnostic sort offsets.
// One diffuse crossfade fragment for ordinary rendering and smoke replay;
// base PBR lighting and the existing raster/sort policy remain authored.
pub(crate) type SkidMaterial = ExtendedMaterial<StandardMaterial, SkidRasterBias>;
const LAYER_SHADER: Handle<Shader> = uuid_handle!("28837716-d9b9-4d87-b230-4055be1080b8");
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
#[bind_group_data(SkidRasterKey)]
pub(crate) struct SkidRasterBias {
    #[texture(100)]
    #[sampler(101)]
    atlas: Handle<Image>,
    raster_constant: i32,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(crate) struct SkidRasterKey(i32);
impl From<&SkidRasterBias> for SkidRasterKey {
    fn from(value: &SkidRasterBias) -> Self { Self(value.raster_constant) }
}
impl MaterialExtension for SkidRasterBias {
    // Fixed AlphaMode::Blend uses the native forward queue only. No custom
    // deferred/prepass/meshlet shader support is advertised by this material.
    fn fragment_shader() -> ShaderRef { LAYER_SHADER.into() }
    fn specialize(
        _: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _: &MeshVertexBufferLayoutRef,
        key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // ExtendedMaterial calls this AFTER StandardMaterial specialization.
        // Preserve slope/clamp, compare, write-enable, blend and every shader.
        restore_raster_constant(descriptor, key.bind_group_data.0);
        Ok(())
    }
}
fn restore_raster_constant(descriptor: &mut RenderPipelineDescriptor, constant: i32) {
    if let Some(depth) = descriptor.depth_stencil.as_mut() {
        depth.bias.constant = constant;
    }
}

/// Closed factory: callers cannot pass unlit/transmission/UV-transform flags
/// that the shared forward-Blend fragment does not implement.
fn skid_material(atlas: Handle<Image>, mode: OrderMode, reflectance: Option<f32>) -> SkidMaterial {
    // This existing diagnostic only admits0/0.5; enforce it in release too.
    assert!(reflectance.is_none_or(|v| v == 0. || v == 0.5), "unsupported skid reflectance");
    SkidMaterial {
        base: StandardMaterial {
            alpha_mode: AlphaMode::Blend,
            unlit: false,
            perceptual_roughness: 0.9,
            diffuse_transmission: 0.,
            specular_transmission: 0.,
            reflectance: reflectance.unwrap_or(0.),
            depth_bias: mode.sort_bias(),
            // White/UV0/identity, no auxiliary maps, zero transmission and
            // existing back-cull/fog defaults. No arbitrary base input.
            ..default()
        },
        extension: SkidRasterBias { atlas, raster_constant: 50 },
    }
}

/// Global strip pool and strip length [D 73F860/739570].
const STRIPS: usize = 240;
const QUADS: usize = 32;
/// Segment-local UV payloads need four vertices per quad; seams coincide.
const SLOT: usize = QUADS * 4;
/// Retire the farthest finished strip while free + retiring strips < 40 [D].
const SPARE: usize = 40;
/// Alpha lost by the oldest quad of a retiring strip per update (25/255) [D].
const RETIRE_STEP: f32 = 25. / 255.;
/// A point is laid after the wheel moved more than 0.1 m [D].
const STEP: f32 = 0.1;
/// A strip ends when its direction turns by more than 10 degrees [D 73F0A0].
const TURN_COS: f32 = 0.984_807_7;
/// Skid layers need excess wheel force / mass above this [D].
const SKID_EXCESS: f32 = 2.0;
/// Side-skid layer once 1 - |cos(slip angle)| reaches 0.5 [D].
const SIDE_FACTOR: f32 = 0.5;
/// Atlas aspect (1024 x 256) used to keep the texel aspect along the mark [D].
const ATLAS_ASPECT: f32 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Tread,
    Skid,
    SideSkid,
}

/// Layer of the original decision: skidding wheels (excess > 2) use the skid
/// or side-skid layer when the surface has `skid`, rolling wheels the tread
/// layer when it has `tread`; otherwise no mark (the strip ends).
fn choose(wheel: &Wheel, profile: &Profile) -> Option<(Kind, Layer)> {
    if !wheel.contact {
        return None;
    }
    let surface = profile.surfaces.iter().find(|s| s.code == wheel.code)?;
    if wheel.excess > SKID_EXCESS {
        if !surface.skid.enabled {
            return None;
        }
        Some(if wheel.side_factor >= SIDE_FACTOR {
            (Kind::SideSkid, surface.side_skid)
        } else {
            (Kind::Skid, surface.skid)
        })
    } else if surface.tread.enabled {
        Some((Kind::Tread, surface.tread))
    } else {
        None
    }
}

fn finite_phase(pair: [f32; 2]) -> bool {
    pair.into_iter().all(|value| value.is_finite() && value >= 0.)
}

fn finite_seed(pair: [f32; 2]) -> [f32; 2] {
    if finite_phase(pair) { pair } else { [0.; 2] }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Point {
    center: Vec3,
    side: Vec3,
    normal: Vec3,
    half_width: f32,
    u: [f32; 2],
    // Independent UV0/UV1 endpoint phases, seeded from persistent pool-slot state.
    v: [f32; 2],
    alpha: f32,
}

impl Point {
    fn finite_payload(&self) -> bool {
        self.center.is_finite() && self.side.is_finite() && self.normal.is_finite()
            && self.normal.length_squared().is_finite() && self.normal.length_squared() > 0.
            && self.half_width.is_finite() && self.half_width > 0.
            && self.u.into_iter().all(f32::is_finite) && finite_phase(self.v)
            && self.alpha.is_finite() && (0. ..=1.).contains(&self.alpha)
            && (self.center - self.side * self.half_width).is_finite()
            && (self.center + self.side * self.half_width).is_finite()
    }
}

/// Each segment keeps both atlas bands immutable when the next layer changes.
#[derive(Clone, Copy, Debug, PartialEq)]
struct QuadUv {
    old: [[f32; 2]; 4],
    new: [[f32; 2]; 4],
}
impl QuadUv {
    fn between(a: &Point, b: &Point, step: f32) -> Option<Self> {
        if !a.finite_payload() || !b.finite_payload() || !step.is_finite() || step < 0. {
            return None;
        }
        // Cached739570 advances both independent phases using the CURRENT
        // endpoint width as common denominator. Constructor/recycle/capacity
        // phase policy is handled separately from this segment payload.
        let scale = ATLAS_ASPECT * step / (2. * b.half_width).max(1e-3);
        let old_end_v = a.v[0] + (a.u[1] - a.u[0]).abs() * scale;
        let new_end_v = a.v[1] + (b.u[1] - b.u[0]).abs() * scale;
        let result = Self {
            old: [[a.u[0], a.v[0]], [a.u[1], a.v[0]],
                [a.u[0], old_end_v], [a.u[1], old_end_v]],
            new: [[b.u[0], a.v[1]], [b.u[1], a.v[1]],
                [b.u[0], new_end_v], [b.u[1], new_end_v]],
        };
        result.finite_payload().then_some(result)
    }
    fn finite_payload(&self) -> bool {
        self.old.iter().chain(&self.new)
            .all(|uv| uv[0].is_finite() && uv[1].is_finite() && uv[1] >= 0.)
    }
}

/// Recovered finite-domain capacity math. Domain failure is NOT uploaded:
/// callers apply the explicit authored terminate/reseed policy below.
fn capacity_phase(previous: [f32; 2]) -> Option<[f32; 2]> {
    if !finite_phase(previous) { return None; }
    let divisor = f64::from(previous[0]).floor();
    if !divisor.is_finite() || divisor < 1. { return None; }
    let result = previous.map(|phase| (f64::from(phase) % divisor) as f32);
    finite_phase(result).then_some(result)
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Strip {
    points: Vec<Point>,
    segments: Vec<QuadUv>,
    /// Contact-start ramp position at point zero; capacity chains retain it.
    ramp_start: usize,
    /// (car, wheel) while the strip is being extended.
    owner: Option<(usize, usize)>,
    retiring: bool,
    /// Index of the oldest point still fading during retirement.
    fade: usize,
    used: bool,
}

#[derive(Clone, Copy, Debug)]
struct Head {
    strip: usize,
    direction: Option<Vec3>,
}

#[derive(Resource)]
pub(crate) struct SkidMarks {
    profile: Profile,
    /// Initial construction zeros phases; geometry recycle/reset does not.
    phases: [[f32; 2]; STRIPS],
    strips: Vec<Strip>,
    heads: Vec<[Option<Head>; 4]>,
    camera: Vec3,
    generation: u64,
    mesh: Handle<Mesh>,
    owned_ground: Option<super::ground_skid_pass::OwnedGroundSkid>,
    /// The marks entity (alpha blended: its sort centre) and the streamed
    /// mesh's vertex layout.
    entity: Entity,
    layout: Option<VertexLayout>,
    dirty: bool,
    /// The strips as last written to the mesh (none yet: write it whole).
    written: Option<Vec<Strip>>,
    pub laid: u64,
    phase_safety_events: u64,
}

/// Packed position, normal, previous/current atlas UV and blend/opacity.
type SkidVertex = ([f32; 3], [f32; 3], [f32; 2], [f32; 2], [f32; 4]);

impl SkidMarks {
    pub(crate) fn owned_ground_draw(&self) -> Option<super::ground_skid_pass::OwnedGroundSkid> {
        self.owned_ground.filter(|draw| draw.mesh == self.mesh.id())
    }
    fn new(profile: Profile) -> Self {
        Self {
            profile,
            phases: [[0.; 2]; STRIPS],
            strips: vec![Strip::default(); STRIPS],
            heads: Vec::new(),
            camera: Vec3::ZERO,
            generation: 0,
            mesh: Handle::default(),
            owned_ground: None,
            entity: Entity::PLACEHOLDER,
            layout: None,
            dirty: true,
            written: None,
            laid: 0,
            phase_safety_events: 0,
        }
    }

    fn clear(&mut self) {
        // Keep slot phase history. Clearing all geometry on reset is still the
        // port policy; the source reset ends wheels rather than rebuilding pool.
        self.strips = vec![Strip::default(); STRIPS];
        self.heads.clear();
        self.dirty = true;
    }

    fn distance(&self, strip: &Strip) -> f32 {
        strip
            .points
            .last()
            .map_or(f32::INFINITY, |p| p.center.distance_squared(self.camera))
    }

    /// Free strip, else a retiring one, else steal the active strip farthest
    /// from the camera, even one a wheel is still laying [V 73F330]. Releasing
    /// it detaches that wheel, which starts a fresh strip next sample [V 7289C0].
    fn allocate(&mut self) -> Option<usize> {
        let index = self
            .strips
            .iter()
            .position(|s| !s.used)
            .or_else(|| self.strips.iter().position(|s| s.retiring))
            .or_else(|| {
                (0..STRIPS).max_by(|&a, &b| {
                    self.distance(&self.strips[a])
                        .total_cmp(&self.distance(&self.strips[b]))
                })
            })?;
        if let Some((car, wheel)) = self.strips[index].owner
            && let Some(head) = self.heads.get_mut(car).and_then(|h| h.get_mut(wheel))
        {
            *head = None;
        }
        self.strips[index] = Strip {
            used: true,
            ..Strip::default()
        };
        // The source slot constructor is not rerun for recycled geometry.
        // Also invalidate any previous visible contents when stealing a slot.
        self.dirty = true;
        Some(index)
    }

    /// End a wheel's strip: last point transparent, the one before halved [D].
    fn end(&mut self, car: usize, wheel: usize) {
        if let Some(head) = self.heads[car][wheel].take() {
            let strip = &mut self.strips[head.strip];
            strip.owner = None;
            let n = strip.points.len();
            if n >= 1 {
                strip.points[n - 1].alpha = 0.;
            }
            if n >= 2 {
                strip.points[n - 2].alpha *= 0.5;
            }
            if n < 2 {
                *strip = Strip::default();
            }
            self.dirty = true;
        }
    }

    fn safety_event(&mut self, car: usize, wheel: usize, reason: &'static str) {
        self.phase_safety_events = self.phase_safety_events.saturating_add(1);
        let count = self.phase_safety_events;
        // Bounded logarithmic reporting, not one message every bad frame.
        if count <= 4 || count.is_power_of_two() {
            println!("skid_phase_safety authored=true event=finite-domain-recovery count={count} reason={reason} car={car} wheel={wheel} source_CRT_domain_parity=false");
        }
    }

    fn begin(&mut self, car: usize, wheel: usize, strip: usize, mut point: Point) {
        if !finite_phase(self.phases[strip]) {
            self.safety_event(car, wheel, "seed-domain");
        }
        self.phases[strip] = finite_seed(self.phases[strip]);
        point.v = self.phases[strip];
        if !point.finite_payload() {
            self.safety_event(car, wheel, "point-suppressed");
            return;
        }
        self.strips[strip].owner = Some((car, wheel));
        self.strips[strip].points.push(point);
        self.heads[car][wheel] = Some(Head { strip, direction: None });
        self.dirty = true;
    }

    /// AUTHORED safety deviation, not an original CRT exception fallback.
    /// End first; a fresh current point cannot bridge the invalid continuation.
    fn recover(&mut self, car: usize, wheel: usize, point: Option<Point>, reason: &'static str) {
        self.safety_event(car, wheel, reason);
        if let Some(head) = self.heads[car][wheel] {
            self.phases[head.strip] = finite_seed(self.phases[head.strip]);
        }
        self.end(car, wheel);
        if let Some(point) = point.filter(Point::finite_payload)
            && let Some(strip) = self.allocate()
        {
            self.begin(car, wheel, strip, point);
        }
    }

    /// Advance one wheel by one physics sample.
    fn advance(&mut self, car: usize, index: usize, wheel: &Wheel) {
        let Some((_kind, layer)) = choose(wheel, &self.profile) else {
            self.end(car, index);
            return;
        };
        let side = wheel.normal.cross(wheel.forward).normalize_or(Vec3::X);
        // Left-side wheels mirror the atlas U range [D].
        let u = if wheel.left {
            [layer.end_u, layer.start_u]
        } else {
            [layer.start_u, layer.end_u]
        };
        let width = wheel.width * layer.width_scale;
        let mut point = Point {
            center: wheel.point + wheel.normal * self.profile.offset,
            side,
            normal: wheel.normal,
            half_width: 0.5 * width,
            u,
            v: [0.; 2],
            alpha: 1.,
        };
        if !point.finite_payload() {
            self.recover(car, index, None, "point-domain");
            return;
        }
        if let Some(head) = self.heads[car][index] {
            if !finite_phase(self.phases[head.strip]) {
                self.recover(car, index, Some(point), "incoming-phase");
                return;
            }
            let mut last = *self.strips[head.strip]
                .points
                .last()
                .expect("owned strip point");
            let delta = point.center - last.center;
            let step = delta.length();
            if !step.is_finite() {
                self.recover(car, index, None, "step-domain");
                return;
            }
            if step <= STEP {
                return;
            }
            let direction = delta / step;
            let turned = head.direction.is_some_and(|d| d.dot(direction) < TURN_COS);
            if step > 4.0 || turned {
                self.end(car, index);
            } else {
                let full = self.strips[head.strip].points.len() > QUADS;
                let ramp_start = (self.strips[head.strip].ramp_start
                    + self.strips[head.strip].points.len() - 1).min(2);
                let previous_phases = self.phases[head.strip];
                let strip = if full {
                    // Source capacity is32 emitted quads; this port reaches the
                    // same limit via33 points, at its next accepted-point tick.
                    self.strips[head.strip].owner = None;
                    let Some(next) = self.allocate() else {
                        self.heads[car][index] = None;
                        return;
                    };
                    if next == head.strip {
                        //73F860 drops the predecessor when allocation steals it.
                        // No linked capacity wrap or old-to-new bridge remains.
                        self.begin(car, index, next, point);
                        return;
                    }
                    let Some(wrapped) = capacity_phase(previous_phases) else {
                        // Allocation already cleared next: end the old head and
                        // reuse that exact slot for a fresh seed, not another allocation.
                        self.safety_event(car, index, "capacity-domain");
                        self.end(car, index);
                        self.begin(car, index, next, point);
                        return;
                    };
                    last.v = wrapped;
                    self.phases[next] = last.v;
                    self.strips[next].points.push(last);
                    self.strips[next].ramp_start = ramp_start;
                    self.strips[next].owner = Some((car, index));
                    next
                } else {
                    last.v = previous_phases;
                    head.strip
                };
                let Some(segment) = QuadUv::between(&last, &point, step) else {
                    if strip == head.strip {
                        self.recover(car, index, Some(point), "advance-domain");
                        return;
                    }
                    self.safety_event(car, index, "advance-domain");
                    self.end(car, index);
                    // If a distinct chain slot was prepared, clear its copied
                    // endpoint before seeding the current point. No bridge survives.
                    self.strips[strip].owner = None;
                    self.strips[strip].points.clear();
                    self.strips[strip].segments.clear();
                    self.strips[strip].ramp_start = 0;
                    self.strips[strip].used = true;
                    self.begin(car, index, strip, point);
                    return;
                };
                point.v = [segment.old[2][1], segment.new[2][1]];
                self.phases[strip] = point.v;
                self.strips[strip].segments.push(segment);
                self.strips[strip].points.push(point);
                self.heads[car][index] = Some(Head {
                    strip,
                    direction: Some(direction),
                });
                self.laid += 1;
                self.dirty = true;
                return;
            }
        }
        // Fresh strip.
        let Some(strip) = self.allocate() else {
            return;
        };
        self.begin(car, index, strip, point);
    }

    /// Keep spare strips by retiring the farthest finished ones; fade them
    /// one quad at a time from the oldest end and free them when faded [D].
    fn maintain(&mut self) {
        let spare = self.strips.iter().filter(|s| !s.used || s.retiring).count();
        if spare < SPARE
            && let Some(far) = (0..STRIPS)
                .filter(|&i| {
                    let s = &self.strips[i];
                    s.used && !s.retiring && s.owner.is_none()
                })
                .max_by(|&a, &b| {
                    self.distance(&self.strips[a])
                        .total_cmp(&self.distance(&self.strips[b]))
                })
        {
            self.strips[far].retiring = true;
        }
        for strip in self.strips.iter_mut().filter(|s| s.retiring) {
            while strip.fade < strip.points.len() && strip.points[strip.fade].alpha <= 0. {
                strip.fade += 1;
            }
            if strip.fade + 1 >= strip.points.len() {
                *strip = Strip::default();
            } else {
                let p = &mut strip.points[strip.fade];
                p.alpha = (p.alpha - RETIRE_STEP).max(0.);
            }
            self.dirty = true;
        }
    }

    /// Segment-local attributes, with coincident geometry/opacity at shared seams.
    /// COLOR.r is blend metadata; COLOR.a remains visible opacity.
    fn slot_vertex(strip: &Strip, k: usize)
        -> SkidVertex
    {
        let q = k / 4;
        let edge = k % 4;
        let Some(uv) = strip.segments.get(q) else {
            // All four unused vertices coincide: no trailing positive-area quad.
            let p = strip.points.last().map(|p| p.center)
                .filter(|p| p.is_finite()).unwrap_or(Vec3::ZERO);
            return (p.to_array(), [0., 1., 0.], [0.; 2], [0.; 2], [0.; 4]);
        };
        if !uv.finite_payload()
            || !strip.points.get(q).is_some_and(Point::finite_payload)
            || !strip.points.get(q + 1).is_some_and(Point::finite_payload)
        {
            return ([0.; 3], [0., 1., 0.], [0.; 2], [0.; 2], [0.; 4]);
        }
        let i = q + edge / 2;
        let p = &strip.points[i];
        let ordinal = strip.ramp_start + i;
        let ramp = match ordinal { 0 => 0., 1 => 0.5, _ => 1. };
        let alpha = p.alpha * if strip.owner.is_some() || ordinal < 2 { ramp } else { 1. };
        let sign = if edge.is_multiple_of(2) { -1. } else { 1. };
        ((p.center + p.side * p.half_width * sign).to_array(), p.normal.to_array(),
            uv.old[edge], uv.new[edge], [if edge < 2 { 0. } else { 1. }, 1., 1., alpha])
    }

    /// Every strip slot at its full length, so later changes are written over
    /// the mesh's GPU allocation (mesh_stream) instead of replacing the asset.
    /// The triangles with area and their order are the ones `build` makes.
    fn streamed(&self) -> Mesh {
        let count = STRIPS * SLOT;
        let (mut positions, mut normals, mut uvs, mut uvs_new, mut colors) = (
            Vec::with_capacity(count),
            Vec::with_capacity(count),
            Vec::with_capacity(count),
            Vec::with_capacity(count),
            Vec::with_capacity(count),
        );
        let mut indices = Vec::with_capacity(STRIPS * QUADS * 6);
        for (s, strip) in self.strips.iter().enumerate() {
            for k in 0..SLOT {
                let (p, n, uv, uv_new, c) = Self::slot_vertex(strip, k);
                positions.push(p);
                normals.push(n);
                uvs.push(uv);
                uvs_new.push(uv_new);
                colors.push(c);
            }
            for i in 0..QUADS as u32 {
                let a = (s * SLOT) as u32 + i * 4;
                indices.extend([a, a + 2, a + 1, a + 1, a + 2, a + 3]);
            }
        }
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, uvs_new);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
        mesh.insert_indices(Indices::U32(indices));
        mesh
    }

    /// The packed vertex bytes of `streamed()`.
    #[cfg(test)]
    fn pack(&self, layout: &VertexLayout, bytes: &mut Vec<u8>) {
        self.pack_strips(layout, 0..STRIPS, bytes);
    }

    /// Runs of strips that differ from the ones last written, which become
    /// the written ones (F592). All of them the first time.
    fn changes(&mut self) -> Vec<std::ops::Range<usize>> {
        let Some(written) = &mut self.written else {
            self.written = Some(self.strips.clone());
            return std::iter::once(0..STRIPS).collect();
        };
        let mut runs: Vec<std::ops::Range<usize>> = Vec::new();
        for (i, (now, old)) in self.strips.iter().zip(written.iter_mut()).enumerate() {
            if now == old {
                continue;
            }
            old.clone_from(now);
            match runs.last_mut() {
                Some(run) if run.end == i => run.end = i + 1,
                _ => runs.push(i..i + 1),
            }
        }
        runs
    }

    /// The packed vertex bytes of the strip slots in `strips`.
    fn pack_strips(
        &self,
        layout: &VertexLayout,
        strips: std::ops::Range<usize>,
        bytes: &mut Vec<u8>,
    ) {
        let at = |attribute| layout.offset(attribute).expect("skid mark attribute");
        let (p, n, uv, uv_new, c) = (
            at(Mesh::ATTRIBUTE_POSITION),
            at(Mesh::ATTRIBUTE_NORMAL),
            at(Mesh::ATTRIBUTE_UV_0),
            at(Mesh::ATTRIBUTE_UV_1),
            at(Mesh::ATTRIBUTE_COLOR),
        );
        let start = bytes.len();
        bytes.resize(start + strips.len() * SLOT * layout.stride, 0);
        let mut vertex = start;
        for strip in &self.strips[strips] {
            for k in 0..SLOT {
                let (pv, nv, uvv, uvn, cv) = Self::slot_vertex(strip, k);
                put(bytes, vertex + p, pv);
                put(bytes, vertex + n, nv);
                put(bytes, vertex + uv, uvv);
                put(bytes, vertex + uv_new, uvn);
                put(bytes, vertex + c, cv);
                vertex += layout.stride;
            }
        }
    }

    /// Centre of the bounding box `build`'s mesh would have (its transparent
    /// sort key): the strips' vertices, or the origin when there are none.
    fn sort_centre(&self) -> Vec3 {
        let corners = self
            .strips
            .iter()
            .filter(|s| s.points.len() >= 2)
            .flat_map(|s| (0..(s.points.len() - 1) * 4).map(move |k| Vec3::from(Self::slot_vertex(s, k).0)));
        Aabb::enclosing(corners).map_or(Vec3::ZERO, |aabb| aabb.center.into())
    }

    #[cfg(test)]
    fn build(&self) -> Mesh {
        let mut positions = Vec::new();
        let mut normals = Vec::new();
        let mut uvs = Vec::new();
        let mut colors = Vec::new();
        let mut indices = Vec::new();
        for strip in self.strips.iter().filter(|s| s.points.len() >= 2) {
            // Fresh strips ramp in over their first two quads [D byte0 ramp].
            let first = positions.len() as u32;
            for (i, p) in strip.points.iter().enumerate() {
                let ramp = match i {
                    0 => 0.,
                    1 => 0.5,
                    _ => 1.,
                };
                let alpha = p.alpha
                    * if strip.owner.is_some() || i < 2 {
                        ramp
                    } else {
                        1.
                    };
                for (sign, u) in [(-1., p.u[0]), (1., p.u[1])] {
                    positions.push((p.center + p.side * p.half_width * sign).to_array());
                    normals.push(p.normal.to_array());
                    uvs.push([u, p.v[0]]);
                    colors.push([1., 1., 1., alpha]);
                }
            }
            for i in 0..strip.points.len() as u32 - 1 {
                let a = first + i * 2;
                indices.extend([a, a + 2, a + 1, a + 1, a + 2, a + 3]);
            }
        }
        super::ground_effect_assets::effect_mesh(positions, normals, uvs, colors, indices)
    }
}

pub(crate) fn install(app: &mut App, root: &Path, mode: OrderMode, reflectance: Option<f32>) -> Result<(), Box<dyn std::error::Error>> {
    let text = std::fs::read_to_string(root.join("skidmarks/road_tm.xml"))?;
    let profile = grid_formats::skidmarks::read(&text)?;
    let textures =
        super::ground_effect_assets::package(root, "skidmarks/road_tm.pssg", &["diffuse.tga"])?;
    let diffuse = textures
        .get("diffuse.tga")
        .ok_or("skidmark diffuse texture absent")?;
    let diffuse = diffuse.clone();
    println!(
        "skid_marks enabled=true profile=road_tm surfaces={} offset={} strips={STRIPS}x{QUADS} cars=all fidelity=experimental",
        profile.surfaces.len(),
        profile.offset
    );
    app.world_mut().resource_mut::<Assets<Shader>>().insert(
        LAYER_SHADER.id(), Shader::from_wgsl(include_str!("skid_layers.wgsl"), "grid_skid_layers.wgsl"),
    ).expect("UUID skid layer shader");
    app.add_plugins(MaterialPlugin::<SkidMaterial>::default());
    println!("skid_layers geometry=segment-local phase_policy=dual-current-width pool_zero_recycle_preserve_common_floor_fmod source_precision_unverified=true source_shader_parity=false vertices={}", STRIPS * SLOT);
    println!("skid_order mode={mode:?} raster_constant=50 sort_bias={} source_parity=false",
        mode.sort_bias());
    app.init_resource::<MeshStream>()
        .insert_resource(Pending { profile, diffuse, mode, reflectance })
        .add_systems(Startup, start)
        .add_systems(FixedUpdate, lay.after(super::wheel_ground::update))
        .add_systems(Update, present.after(super::drive_present));
    Ok(())
}

#[derive(Resource)]
struct Pending {
    profile: Profile,
    diffuse: Vec<u8>,
    mode: OrderMode,
    reflectance: Option<f32>,
}

fn start(
    mut commands: Commands,
    pending: Option<Res<Pending>>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<SkidMaterial>>,
    ground_pass: Option<Res<super::ground_skid_pass::ProducerPolicy>>,
) {
    let Some(pending) = pending else { return };
    // Device-dependent upload must follow texture_upload::configure (PreStartup).
    // install() runs before the render device has selected BC/ETC2/RGBA support.
    let image = match super::ground_effect_assets::image(&pending.diffuse, true) {
        Ok(image) => image,
        Err(error) => {
            warn!("skid mark texture: {error}");
            return;
        }
    };
    let mut marks = SkidMarks::new(pending.profile.clone());
    println!("skid_phase_safety authored=true policy=terminate-reseed finite_only=true initial_events=0 source_CRT_domain_parity=false");
    let mesh = marks.streamed();
    marks.layout = Some(VertexLayout::of(&mesh));
    marks.mesh = meshes.add(mesh);
    let atlas = images.add(image);
    if let Some(value) = pending.reflectance {
        println!("skid_reflectance active=true value={value} mode=Isolated shader=shared-layer-lit-PBR source_parity=false");
    }
    let material = materials.add(skid_material(atlas, pending.mode, pending.reflectance));
    let material_id = material.id();
    let entity = commands.spawn((
        Name::new("skid marks"), Mesh3d(marks.mesh.clone()), MeshMaterial3d(material),
        Transform::IDENTITY, bevy::camera::visibility::NoFrustumCulling,
    )).id();
    marks.entity = entity;
    if pending.mode == OrderMode::Off && ground_pass.is_some() {
        let owner = super::ground_skid_pass::OwnedGroundSkid::new(entity, marks.mesh.id(), material_id);
        commands.entity(entity).insert((owner, bevy::render::batching::NoAutomaticBatching));
        marks.owned_ground = Some(owner);
    }
    commands.remove_resource::<Pending>();
    commands.insert_resource(marks);
}

fn lay(
    marks: Option<ResMut<SkidMarks>>,
    ground: Res<WheelGround>,
    camera: Query<&GlobalTransform, With<super::ViewerCamera>>,
) {
    let Some(mut marks) = marks else { return };
    if marks.generation == ground.generation {
        return;
    }
    marks.generation = ground.generation;
    let start = std::time::Instant::now();
    if ground.reset {
        marks.clear();
    }
    if let Ok(camera) = camera.single() {
        marks.camera = camera.translation();
    }
    marks.heads.resize(ground.cars.len(), [None; 4]);
    for (car, wheels) in ground.cars.iter().enumerate() {
        for (index, wheel) in wheels.iter().enumerate() {
            marks.advance(car, index, wheel);
        }
    }
    marks.maintain();
    super::ground_effect_assets::charge(super::ground_effect_assets::TICK, start);
}

/// Writes the changed marks over the mesh's GPU allocation (F580; before,
/// every change replaced the asset, which Bevy reallocated and repacked).
/// Only the strips that changed since the last write are packed and sent
/// (F592; before, all 240 slots, 760 KB, nearly every frame).
fn present(marks: Option<ResMut<SkidMarks>>, stream: Res<MeshStream>) {
    let Some(mut marks) = marks else { return };
    if !marks.dirty {
        return;
    }
    let start = std::time::Instant::now();
    marks.dirty = false;
    if let Some(layout) = &marks.layout {
        let layout = layout.clone();
        for run in marks.changes() {
            let first = (run.start * SLOT) as u32;
            stream.write_at(marks.mesh.id(), first, layout.stride, |bytes| {
                marks.pack_strips(&layout, run, bytes)
            });
        }
        stream.sort_centre(marks.entity, marks.sort_centre());
    }
    super::ground_effect_assets::charge(super::ground_effect_assets::SKID, start);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_mode_is_default_off_and_rejects_unknown_values() {
        assert_eq!(OrderMode::default(), OrderMode::Off);
        assert_eq!(OrderMode::parse("control"), Ok(OrderMode::Control));
        assert_eq!(OrderMode::parse("isolated"), Ok(OrderMode::Isolated));
        assert!(OrderMode::parse("zero").is_err());
    }
    #[test]
    fn isolated_sort_preserves_raster_key_atlas_alpha_and_lighting() {
        let atlas = Handle::default();
        let control = skid_material(atlas.clone(), OrderMode::Control, None);
        let isolated = skid_material(atlas.clone(), OrderMode::Isolated, None);
        let original = control.base.clone();
        assert_eq!(control.base.depth_bias, 50.);
        assert_eq!(isolated.base.depth_bias, 0.);
        assert_eq!(<SkidMaterial as bevy::pbr::Material>::depth_bias(&control), 50.);
        assert_eq!(<SkidMaterial as bevy::pbr::Material>::depth_bias(&isolated), 0.);
        assert_eq!(SkidRasterKey::from(&control.extension), SkidRasterKey(50));
        assert_eq!(SkidRasterKey::from(&isolated.extension), SkidRasterKey(50));
        for material in [control, isolated] {
            assert_eq!(material.extension.atlas, atlas);
            assert!(material.base.base_color_texture.is_none());
            assert_eq!(material.base.base_color, original.base_color);
            assert_eq!(material.base.alpha_mode, original.alpha_mode);
            assert_eq!(material.base.perceptual_roughness, original.perceptual_roughness);
            assert_eq!(material.base.cull_mode, original.cull_mode);
            assert_eq!(material.base.unlit, original.unlit);
            assert_eq!(material.base.fog_enabled, original.fog_enabled);
        }
        let changed = SkidRasterBias { atlas: Handle::default(), raster_constant: 51 };
        assert_ne!(SkidRasterKey::from(&changed), SkidRasterKey(50));
    }


    #[test]
    fn raster_restore_changes_only_constant_and_keeps_shared_layer_fragment() {
        use bevy::render::render_resource::{BlendState, ColorTargetState, ColorWrites,
            CompareFunction, DepthBiasState, DepthStencilState, FragmentState, TextureFormat};
        let mut descriptor = RenderPipelineDescriptor {
            label: Some("skid-order-fixture".into()),
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth32Float,
                depth_write_enabled: Some(false),
                depth_compare: Some(CompareFunction::GreaterEqual),
                stencil: default(),
                bias: DepthBiasState { constant: 0, slope_scale: 0.125, clamp: 0.25 },
            }),
            fragment: Some(FragmentState {
                targets: vec![Some(ColorTargetState {
                    format: TextureFormat::Rgba16Float,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
                ..default()
            }),
            ..default()
        };
        let mut expected = descriptor.clone();
        expected.depth_stencil.as_mut().unwrap().bias.constant = 50;
        restore_raster_constant(&mut descriptor, 50);
        assert_eq!(descriptor, expected); // Entire descriptor; only constant differs.
        let mut no_depth = RenderPipelineDescriptor::default();
        let untouched = no_depth.clone();
        restore_raster_constant(&mut no_depth, 50);
        assert_eq!(no_depth, untouched);
        assert!(matches!(<SkidRasterBias as MaterialExtension>::vertex_shader(), bevy::shader::ShaderRef::Default));
        assert!(matches!(<SkidRasterBias as MaterialExtension>::fragment_shader(), ShaderRef::Handle(_)));
        assert!(matches!(<SkidRasterBias as MaterialExtension>::prepass_fragment_shader(), bevy::shader::ShaderRef::Default));
        assert!(matches!(<SkidRasterBias as MaterialExtension>::deferred_fragment_shader(), ShaderRef::Default));
    }

    fn profile() -> Profile {
        grid_formats::skidmarks::read(
            r#"<SKIDMARKS offset="0.04">
<SURFACE name="STDR" tread="0" skid="1" skidStartU="0.02" skidEndU="0.0625"
  sideSkidStartU="0.001" sideSkidEndU="0.02" skidWidthScale="1.1" sideSkidWidthScale="0.6"/>
<SURFACE name="GRAS" tread="1" treadStartU="0.5" treadEndU="0.56" skid="1"
  skidStartU="0.56" skidEndU="0.62" sideSkidStartU="0.56" sideSkidEndU="0.62"/>
</SKIDMARKS>"#,
        )
        .unwrap()
    }

    fn wheel(code: &[u8; 4], excess: f32, side: f32, x: f32) -> Wheel {
        Wheel {
            contact: true,
            code: *code,
            point: Vec3::new(x, 0., 0.),
            normal: Vec3::Y,
            forward: Vec3::X,
            velocity: Vec3::X * 10.,
            width: 0.27,
            excess,
            side_factor: side,
            ..Default::default()
        }
    }

    #[test]
    fn original_layer_choice() {
        let p = profile();
        assert!(choose(&wheel(b"STDR", 1.0, 0., 0.), &p).is_none());
        assert_eq!(
            choose(&wheel(b"STDR", 2.5, 0.2, 0.), &p).unwrap().0,
            Kind::Skid
        );
        assert_eq!(
            choose(&wheel(b"STDR", 2.5, 0.6, 0.), &p).unwrap().0,
            Kind::SideSkid
        );
        assert_eq!(
            choose(&wheel(b"GRAS", 0.0, 0., 0.), &p).unwrap().0,
            Kind::Tread
        );
        assert!(choose(&wheel(b"TARM", 3.0, 0., 0.), &p).is_none());
        let mut airborne = wheel(b"STDR", 3.0, 0., 0.);
        airborne.contact = false;
        assert!(choose(&airborne, &p).is_none());
    }

    #[test]
    fn strips_step_chain_end_and_recycle() {
        let mut marks = SkidMarks::new(profile());
        marks.heads.resize(1, [None; 4]);
        // 0.06 m steps: only every other sample lays a point.
        for i in 0..=8 {
            marks.advance(0, 0, &wheel(b"STDR", 3.0, 0., i as f32 * 0.06));
        }
        let head = marks.heads[0][0].unwrap();
        assert_eq!(marks.strips[head.strip].points.len(), 5);
        // Long straight run chains into further strips.
        for i in 0..200 {
            marks.advance(0, 0, &wheel(b"STDR", 3.0, 0., 0.5 + i as f32 * 0.2));
        }
        let used = marks.strips.iter().filter(|s| s.used).count();
        assert!(used >= 6, "{used}");
        assert!(marks.strips.iter().all(|s| s.points.len() <= QUADS + 1));
        // Grip regained on tarmac: strip ends with a transparent last point.
        let strip = marks.heads[0][0].unwrap().strip;
        marks.advance(0, 0, &wheel(b"STDR", 0.5, 0., 41.));
        assert!(marks.heads[0][0].is_none());
        assert_eq!(marks.strips[strip].points.last().unwrap().alpha, 0.);
        // The pool never exceeds its fixed size and retires far strips.
        for k in 0..2000 {
            let x = 100. + k as f32 * 0.5;
            marks.advance(0, 0, &wheel(b"STDR", 3.0, 0., x));
            if k % 3 == 0 {
                marks.advance(0, 0, &wheel(b"STDR", 0.0, 0., x));
            }
            marks.maintain();
        }
        assert_eq!(marks.strips.len(), STRIPS);
        assert!(marks.strips.iter().any(|s| s.retiring) || marks.strips.iter().any(|s| !s.used));
        assert!(marks.build().count_vertices() > 0);
        assert_same_triangles(&marks);
    }

    /// Behavioral replay: material changes are continuous; true loss/reset is not.
    /// Atlas ranges are deliberately disjoint, including same-Kind surfaces.
    #[test]
    fn contact_layer_transitions_chain_without_bridging_airborne_or_reset() {
        let profile = grid_formats::skidmarks::read(r#"<SKIDMARKS offset="0.04">
<SURFACE name="GRAS" tread="1" treadStartU="0.1" treadEndU="0.2" treadWidthScale="1.1"
 skid="1" skidStartU="0.4" skidEndU="0.55" skidWidthScale="2.0"
 sideSkidStartU="0.8" sideSkidEndU="0.9" sideSkidWidthScale="2.25"/>
<SURFACE name="STDR" tread="0" skid="1" skidStartU="0.65" skidEndU="0.7" skidWidthScale="1.1"/>
</SKIDMARKS>"#).unwrap();
        let mut marks = SkidMarks::new(profile);
        assert!(marks.phases.iter().all(|phase| *phase == [0.; 2]));
        // Common divisor distinguishes this from independentfract or using
        // floor(newP0) after the first result: lane1 must remain1.75, not0.75.
        assert_eq!(capacity_phase([3.25, 4.75]), Some([0.25, 1.75]));
        marks.heads.resize(1, [None; 4]);
        let layout = VertexLayout::of(&marks.streamed());
        let mut gpu = Vec::new();
        // Independent Bevy packing is the byte oracle, not a second serializer.
        let flush = |marks: &mut SkidMarks, gpu: &mut Vec<u8>| {
            for run in marks.changes() {
                let mut part = Vec::new();
                marks.pack_strips(&layout, run.clone(), &mut part);
                let at = run.start * SLOT * layout.stride;
                gpu.resize((at + part.len()).max(gpu.len()), 0);
                gpu[at..at + part.len()].copy_from_slice(&part);
            }
            assert_eq!(*gpu, marks.streamed().create_packed_vertex_buffer_data());
        };
        flush(&mut marks, &mut gpu);
        for i in 0..=5 {
            marks.advance(0, 0, &wheel(b"GRAS", 0., 0., i as f32 * 0.2));
        }
        let first = marks.heads[0][0].unwrap().strip;
        // An unchanged layer remains neutral: both atlas sample paths agree.
        assert!(marks.strips[first].segments.iter().all(|uv| uv.old == uv.new));
        // All directed Tread/Skid/SideSkid transitions, then same-Kind new surface.
        let sequence = [(*b"GRAS", 3., 0.), (*b"GRAS", 3., 0.6),
            (*b"GRAS", 0., 0.), (*b"GRAS", 3., 0.6),
            (*b"GRAS", 3., 0.), (*b"GRAS", 0., 0.),
            (*b"GRAS", 3., 0.), (*b"STDR", 3., 0.)];
        for (j, (code, excess, side)) in sequence.into_iter().enumerate() {
            let frozen = marks.strips[first].segments.clone();
            marks.advance(0, 0, &wheel(&code, excess, side, (j + 6) as f32 * 0.2));
            assert_eq!(marks.heads[0][0].unwrap().strip, first);
            assert_eq!(&marks.strips[first].segments[..frozen.len()], frozen.as_slice());
            let strip = &marks.strips[first];
            let k = (strip.segments.len() - 1) * 4;
            // Warmed contact never fades merely because the selected layer changes.
            assert_eq!(SkidMarks::slot_vertex(strip, k).4[3], 1.);
            assert_eq!(SkidMarks::slot_vertex(strip, k + 2).4[3], 1.);
            let uv = strip.segments.last().unwrap();
            // Each sample travels along one band, never through intermediate bands.
            assert_eq!(uv.old[0][0], uv.old[2][0]);
            assert_eq!(uv.old[1][0], uv.old[3][0]);
            assert_eq!(uv.new[0][0], uv.new[2][0]);
            assert_eq!(uv.new[1][0], uv.new[3][0]);
            flush(&mut marks, &mut gpu);
        }
        let surface_change = marks.strips[first].segments.last().unwrap();
        assert_ne!(surface_change.old[0][1], surface_change.new[0][1]);
        assert_ne!(surface_change.old[0][0], surface_change.new[0][0]);
        assert_eq!(marks.strips.iter().filter(|s| s.used).count(), 1);
        for i in 14..=35 {
            marks.advance(0, 0, &wheel(b"GRAS", 3., 0., i as f32 * 0.2));
            flush(&mut marks, &mut gpu);
        }
        let chain = marks.heads[0][0].unwrap().strip;
        assert_ne!(first, chain);
        assert_eq!(marks.strips.iter().filter(|s| s.used).count(), 2);
        let a = SkidMarks::slot_vertex(&marks.strips[first], (QUADS - 1) * 4 + 2);
        let b = SkidMarks::slot_vertex(&marks.strips[chain], 0);
        assert_eq!(a.0, b.0); // physical capacity seam
        assert_eq!(a.1, b.1);
        assert_eq!(a.4[3], 1.);
        assert_eq!(a.4[3], b.4[3]); // no contact-start ramp on a capacity chain
        // Wrapping subtracts whole texture periods from each lane using one
        // common integer divisor; geometry/opacity stays continuous.
        assert!((a.2[1].fract() - b.2[1].fract()).abs() < 2e-6);
        assert!((a.3[1].fract() - b.3[1].fract()).abs() < 2e-6);
        for strip in marks.strips.iter().filter(|s| s.segments.len() > 1) {
            for q in 1..strip.segments.len() {
                for edge in 0..2 {
                    let end = SkidMarks::slot_vertex(strip, (q - 1) * 4 + 2 + edge);
                    let start = SkidMarks::slot_vertex(strip, q * 4 + edge);
                    assert_eq!(end.0, start.0);
                    assert_eq!(end.4[3], start.4[3]);
                    assert_eq!(end.2[1], start.2[1]);
                    assert_eq!(end.3[1], start.3[1]);
                }
            }
        }
        assert_same_triangles(&marks);
        let mut airborne = wheel(b"GRAS", 3., 0., 7.2);
        airborne.contact = false;
        marks.advance(0, 0, &airborne);
        assert!(marks.heads[0][0].is_none());
        assert_eq!(marks.strips[chain].points.last().unwrap().alpha, 0.);
        for x in [20., 20.2, 20.4] {
            marks.advance(0, 0, &wheel(b"GRAS", 3., 0., x));
        }
        // No positive-area triangle may cross the actual airborne interval.
        for tri in drawn(&marks.streamed()) {
            let min = tri.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
            let max = tri.iter().map(|p| p[0]).fold(f32::NEG_INFINITY, f32::max);
            assert!(!(min < 20. && max >= 20.));
        }
        flush(&mut marks, &mut gpu);
        let recycled_phases = marks.phases;
        marks.clear();
        assert_eq!(marks.phases, recycled_phases);
        assert!(marks.heads.is_empty());
        assert!(drawn(&marks.streamed()).is_empty());
        flush(&mut marks, &mut gpu);
        marks.heads.resize(1, [None; 4]);
        for x in [100., 100.2, 100.4] {
            let mut w = wheel(b"GRAS", 0., 0., x);
            w.left = true;
            marks.advance(0, 1, &w);
        }
        let left_slot = marks.heads[0][1].unwrap().strip;
        let left = &marks.strips[left_slot];
        assert_eq!(left.segments[0].old[0][1], recycled_phases[left_slot][0]);
        assert_eq!(left.segments[0].new[0][1], recycled_phases[left_slot][1]);
        assert!(left.segments[0].old[0][0] > left.segments[0].old[1][0]);
        assert!(drawn(&marks.streamed()).iter().flatten().all(|p| p[0] >= 100.));
        flush(&mut marks, &mut gpu);
        assert_same_triangles(&marks);
    }
    /// Inspect the actual mesh/Bevy packer output, not a duplicate phase serializer.
    fn assert_finite_stream(marks: &SkidMarks) {
        use bevy::mesh::VertexAttributeValues;
        let mesh = marks.streamed();
        for attribute in [Mesh::ATTRIBUTE_POSITION, Mesh::ATTRIBUTE_NORMAL,
            Mesh::ATTRIBUTE_UV_0, Mesh::ATTRIBUTE_UV_1, Mesh::ATTRIBUTE_COLOR] {
            let finite = match mesh.attribute(attribute).expect("stream attribute") {
                VertexAttributeValues::Float32x2(v) => v.iter().flatten().all(|v| v.is_finite()),
                VertexAttributeValues::Float32x3(v) => v.iter().flatten().all(|v| v.is_finite()),
                VertexAttributeValues::Float32x4(v) => v.iter().flatten().all(|v| v.is_finite()),
                _ => panic!("unexpected skid vertex format"),
            };
            assert!(finite, "nonfinite vertex attribute");
        }
        let layout = VertexLayout::of(&mesh);
        let mut packed = Vec::new();
        marks.pack(&layout, &mut packed);
        assert_eq!(packed, mesh.create_packed_vertex_buffer_data());
    }

    #[test]
    fn invalid_phase_domains_restart_without_crossing_or_nonfinite_uploads() {
        // Explicit expected seeds, rather than computing an oracle with the
        // implementation's finite_seed/capacity_phase helpers.
        let cases = [
            ([0., 0.], [0., 0.]), ([-0., 0.], [-0., 0.]),
            ([0.75, 4.75], [0.75, 4.75]),
            ([-1., 3.], [0., 0.]), ([3., -1.], [0., 0.]),
            ([f32::NAN, 3.], [0., 0.]), ([3., f32::NAN], [0., 0.]),
            ([f32::INFINITY, 3.], [0., 0.]), ([3., f32::INFINITY], [0., 0.]),
            ([f32::NEG_INFINITY, 3.], [0., 0.]), ([3., f32::NEG_INFINITY], [0., 0.]),
        ];
        for (invalid, expected_seed) in cases {
            let mut marks = SkidMarks::new(profile());
            marks.heads.resize(1, [None; 4]);
            for i in 0..=QUADS {
                marks.advance(0, 0, &wheel(b"STDR", 3., 0., i as f32 * 0.2));
            }
            let old = marks.heads[0][0].unwrap().strip;
            marks.phases[old] = invalid;
            // Exercise bad/recycled next-slot history as well as the old input.
            marks.phases[1] = invalid;
            let at = (QUADS + 1) as f32 * 0.2;
            marks.advance(0, 0, &wheel(b"STDR", 3., 0., at));
            assert!(marks.phase_safety_events > 0);
            let fresh = marks.heads[0][0].expect("valid current point reseeded").strip;
            assert_ne!(fresh, old);
            assert_eq!(marks.strips[fresh].points.len(), 1);
            assert_eq!(marks.strips[fresh].points[0].v, expected_seed);
            assert_eq!(marks.phases[fresh], expected_seed);
            // Actual contact stays true: this gap must be the explicit safety
            // deviation, not a fabricated airborne interval or old/new bridge.
            marks.advance(0, 0, &wheel(b"STDR", 3., 0., at + 0.2));
            for tri in drawn(&marks.streamed()) {
                let lo = tri.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
                let hi = tri.iter().map(|p| p[0]).fold(f32::NEG_INFINITY, f32::max);
                assert!(!(lo < at && hi >= at), "unsafe continuation bridged");
            }
            assert_finite_stream(&marks);
        }
    }

    #[test]
    fn ordinary_advance_overflow_and_bad_points_never_enter_the_stream() {
        // Finite source-style data whose ordinary non-capacity V increment
        // overflows f32; rejecting only capacity transforms would miss this.
        let huge = grid_formats::skidmarks::read(r#"<SKIDMARKS offset="0.04">
<SURFACE name="STDR" tread="0" skid="1" skidStartU="0" skidEndU="1e38" skidWidthScale="1"/>
</SKIDMARKS>"#).unwrap();
        let mut marks = SkidMarks::new(huge);
        marks.heads.resize(1, [None; 4]);
        marks.advance(0, 0, &wheel(b"STDR", 3., 0., 0.));
        marks.advance(0, 0, &wheel(b"STDR", 3., 0., 2.));
        assert!(marks.phase_safety_events > 0);
        assert!(drawn(&marks.streamed()).is_empty());
        let seed = marks.heads[0][0].unwrap().strip;
        assert_eq!(marks.strips[seed].points.len(), 1);
        assert_eq!(marks.strips[seed].points[0].center.x, 2.);
        assert_finite_stream(&marks);
        let mut bad = wheel(b"STDR", 3., 0., 3.);
        bad.normal = Vec3::new(f32::NAN, 1., 0.);
        marks.advance(0, 0, &bad);
        assert!(marks.heads[0][0].is_none());
        assert_finite_stream(&marks);

        // Independent final containment: corrupt metadata cannot poison an
        // upload even if a future producer bypasses the checked advance path.
        let mut marks = SkidMarks::new(profile());
        marks.heads.resize(1, [None; 4]);
        for i in 0..4 { marks.advance(0, 0, &wheel(b"STDR", 3., 0., i as f32 * 0.2)); }
        let strip = marks.heads[0][0].unwrap().strip;
        let before = drawn(&marks.streamed()).len();
        marks.strips[strip].segments[0].new[2][1] = f32::NAN;
        assert_eq!(drawn(&marks.streamed()).len(), before - 2);
        assert_finite_stream(&marks);
    }
    /// Triangles with area, as position triples in draw order.
    fn drawn(mesh: &Mesh) -> Vec<[[f32; 3]; 3]> {
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(p)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("positions");
        };
        let indices: Vec<usize> = mesh.indices().unwrap().iter().collect();
        indices
            .chunks(3)
            .map(|t| [p[t[0]], p[t[1]], p[t[2]]])
            .filter(|t| {
                let [a, b, c] = t.map(Vec3::from);
                (b - a).cross(c - a).length_squared() > 0.
            })
            .collect()
    }

    fn assert_same_triangles(marks: &SkidMarks) {
        use bevy::camera::primitives::MeshAabb;
        let streamed = marks.streamed();
        assert_eq!(drawn(&marks.build()), drawn(&streamed));
        assert_eq!(streamed.count_vertices(), STRIPS * SLOT);
        let layout = VertexLayout::of(&streamed);
        let mut bytes = Vec::new();
        marks.pack(&layout, &mut bytes);
        assert_eq!(bytes, streamed.create_packed_vertex_buffer_data());
        let built = marks.build().compute_aabb().map(|a| Vec3::from(a.center));
        assert_eq!(Some(marks.sort_centre()), built);
    }

    /// Writing only the changed strip runs over the previous bytes gives the
    /// whole packed buffer (F592).
    #[test]
    fn changed_strip_runs_rebuild_the_whole_buffer() {
        let mut marks = SkidMarks::new(profile());
        marks.heads.resize(2, [None; 4]);
        let layout = VertexLayout::of(&marks.streamed());
        let mut gpu = Vec::new();
        let apply = |marks: &mut SkidMarks, gpu: &mut Vec<u8>| {
            let runs = marks.changes();
            for run in runs {
                let mut part = Vec::new();
                marks.pack_strips(&layout, run.clone(), &mut part);
                let at = run.start * SLOT * layout.stride;
                if gpu.len() < at + part.len() {
                    gpu.resize(at + part.len(), 0);
                }
                gpu[at..at + part.len()].copy_from_slice(&part);
            }
            let mut whole = Vec::new();
            marks.pack(&layout, &mut whole);
            assert_eq!(*gpu, whole);
        };
        apply(&mut marks, &mut gpu);
        for step in 0..120 {
            let z = step as f32 * 0.15;
            marks.advance(0, 0, &wheel(b"STDR", 3.0, 0., z));
            marks.advance(1, 2, &wheel(b"STDR", 3.0, 0., 40. + z * 1.3));
            if step % 37 == 36 {
                marks.end(0, 0);
            }
            marks.maintain();
            apply(&mut marks, &mut gpu);
        }
        marks.clear();
        apply(&mut marks, &mut gpu);
    }

    #[test]
    fn full_pool_steals_the_farthest_active_strip_like_the_original() {
        let mut marks = SkidMarks::new(profile());
        marks.heads.resize(2, [None; 4]);
        // Car 1 lays a strip far from the camera and keeps it open.
        marks.advance(1, 0, &wheel(b"STDR", 3.0, 0., 500.));
        marks.advance(1, 0, &wheel(b"STDR", 3.0, 0., 500.2));
        let far = marks.heads[1][0].unwrap().strip;
        // Every other strip is a finished one near the camera.
        for (i, strip) in marks.strips.iter_mut().enumerate() {
            if i != far {
                strip.used = true;
                strip.points = vec![
                    Point {
                        center: Vec3::new(i as f32 * 0.01, 0., 0.),
                        side: Vec3::Z,
                        normal: Vec3::Y,
                        half_width: 0.1,
                        u: [0., 1.],
                        v: [0.; 2],
                        alpha: 1.,
                    };
                    2
                ];
                strip.segments = vec![QuadUv::between(&strip.points[0], &strip.points[1], 0.).unwrap()];
            }
        }
        // 73F330: no free or retiring strip, so the open far strip is stolen
        // and its wheel detached (7289C0).
        assert_eq!(marks.allocate(), Some(far));
        assert!(marks.heads[1][0].is_none());
        assert!(marks.strips[far].owner.is_none() && marks.strips[far].points.is_empty());
        assert_same_triangles(&marks);
        assert_same_triangles(&SkidMarks::new(profile()));
    }
}
