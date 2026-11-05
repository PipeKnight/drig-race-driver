//! Track ornament levels of detail as in the original (docs/TRACK_LOD.md).
//! Every placed object (barriers, props, Washington's buildings) has up to
//! three levels in `objects.pssg`; the route's `ornament_attributes.xml`
//! gives each prototype the distances d0, d1 and D. Each frame every
//! instance gets `v` from the distance to the near side of its box along the
//! line to its centre, warped by the camera's FOV (0x965790), and draws the
//! levels whose range holds `v` (0x95D2A0, 0x96EC80): level 0 inside d0,
//! level 1 to d1, level 2 to D, nothing beyond D. "scenery" prototypes (the
//! buildings) stretch each finer level 20 m into the next and cross-fade
//! there; the port switches in the middle of that overlap. Shadows use level
//! 1 at every distance inside D (0x96EC80 pass 0x401): it casts from the
//! shadow layer, which no camera draws, while another level is shown.
//!
//! The original switches levels and culls at D at once. The port fades each
//! change over [`FADE_SECONDS`] instead (owner, 2026-11-02: nothing should
//! appear in view): the old and the new level are drawn with Bevy's
//! complementary dither (`VisibilityRange`), so every pixel shows one of
//! them, and an instance entering or leaving D dithers in or out. A camera
//! cut switches at once. `GRID_ORNAMENT_FADE=0` keeps the hard switch.
//!
//! Level entities are not merged by `static_batch`: a merged mesh would be
//! rebuilt whenever one of its instances changes level. Common build;
//! `GRID_ORNAMENT_LOD=0` keeps every ornament at level 0 (comparisons).
use bevy::{
    asset::uuid_handle,
    camera::{
        primitives::Aabb,
        visibility::{RenderLayers, VisibilityRange, VisibilitySystems},
    },
    light::NotShadowCaster,
    mesh::MeshTag,
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
    transform::TransformSystems,
};
use std::collections::HashMap;

const SHADOW_SHADER: Handle<Shader> = uuid_handle!("3e9a5c71-0b8d-4f26-9a47-c5d1e2f80b64");
/// A shadow-level part's shadow while its instance crosses D (`shadow_proxies`).
type ShadowMaterial = ExtendedMaterial<StandardMaterial, ShadowFade>;

/// Bevy 0.19's shadow pass takes no visibility-range dither
/// (`specialize_shadows` never sets `VISIBILITY_RANGE_DITHER`), so a fading
/// instance casts through a proxy whose prepass dithers by its mesh tag
/// (`ornament_shadow_fade.wgsl`).
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
struct ShadowFade {
    #[uniform(100)]
    marker: Vec4,
}

impl MaterialExtension for ShadowFade {
    // The prepass pipeline defines VERTEX_OUTPUT_INSTANCE_INDEX itself.
    fn prepass_fragment_shader() -> ShaderRef {
        SHADOW_SHADER.into()
    }
}

/// Per shadow-level part fading across D: its proxy and whether the part
/// cast no shadow before (then it keeps none).
#[derive(Resource, Default)]
struct ShadowProxies {
    proxies: HashMap<Entity, (Entity, bool)>,
    materials: HashMap<AssetId<StandardMaterial>, Option<Handle<ShadowMaterial>>>,
}

/// Reference FOV of the distances: 55 degrees (0x979430, tan(0.47996554)).
const REFERENCE_HALF_FOV: f32 = 0.479_965_54;
/// Overlap of a scenery level into the next, metres (0x95D2A0).
const SCENERY_OVERLAP: f32 = 20.;
/// `lod_distance_00..02` of a prototype without a record (0x9522F0).
pub const DEFAULT_DISTANCES: [f32; 3] = [30., 60., 100.];

/// Length of a level change or of an appearance at D, seconds [authored].
pub const FADE_SECONDS: f32 = 0.4;
/// A camera move this long in one frame is a cut: levels switch at once.
pub const CUT_METRES: f32 = 20.;
/// Width of the dither ramps in [`fade_range`], metres: so wide that the
/// camera distance (under 30 km) moves the dither level by under 0.5.
const RAMP: f32 = 1.0e6;
/// Dither steps of Bevy's crossfade (`visibility_range_dither`).
const STEPS: f32 = 16.;

pub fn enabled() -> bool {
    std::env::var("GRID_ORNAMENT_LOD").map_or(true, |v| v != "0")
}

pub fn fade_enabled() -> bool {
    std::env::var("GRID_ORNAMENT_FADE").map_or(true, |v| v != "0")
}

/// The range a level entity carries: fully drawn (`step` 0 of a fade-out),
/// or `step` of 16 into a fade-in (`appearing`) or a fade-out. The ramps
/// start at negative distances [`RAMP`] wide, so the dither step does not
/// depend on the camera distance, and only 33 distinct ranges exist (Bevy
/// never reuses a range's GPU slot). An entity carries one only while it
/// fades: Bevy checks every ranged entity each frame and re-extracts all of
/// them when one changes, and the dither variant discards, which costs the
/// main pass its early depth test (F703); [`respecialize_ranged`] keeps the
/// pipeline in step. Car levels fade with it too (`car_batch`).
pub fn fade_range(appearing: bool, step: u8) -> VisibilityRange {
    let shift = f32::from(step.min(16)) / STEPS * RAMP;
    if appearing {
        VisibilityRange {
            start_margin: -shift..RAMP - shift,
            end_margin: 10. * RAMP..11. * RAMP,
            use_aabb: false,
        }
    } else {
        VisibilityRange {
            start_margin: -3. * RAMP..-2. * RAMP,
            end_margin: -shift..RAMP - shift,
            use_aabb: false,
        }
    }
}

/// A level change in progress: `from` dithers out while `to` dithers in,
/// `t` from 0 to 1 (1: done, only `to` drawn).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Fade {
    from: u8,
    to: u8,
    t: f32,
}

impl Fade {
    pub(crate) const NONE: Self = Self { from: NONE, to: NONE, t: 1. };

    /// Step to `target` for a frame of `step` (fraction of the fade).
    pub(crate) fn advance(&mut self, target: u8, step: f32, cut: bool) {
        if cut {
            *self = Self { from: target, to: target, t: 1. };
        } else if target == self.to {
            self.t = (self.t + step).min(1.);
        } else if target == self.from && self.t < 1. {
            // Back to the level that was fading out: reverse.
            *self = Self { from: self.to, to: target, t: (1. - self.t + step).min(1.) };
        } else {
            // A new change: from the level that shows most now.
            let from = if self.t >= 0.5 { self.to } else { self.from };
            *self = Self { from, to: target, t: step.min(1.) };
        }
    }

    /// How `level` is drawn: `None` hidden, else (appearing, dither step).
    pub(crate) fn draw(&self, level: u8) -> Option<(bool, u8)> {
        let step = (self.t * STEPS).round() as u8;
        if self.t >= 1. || self.from == self.to {
            (level == self.to).then_some((false, 0))
        } else if level == self.to {
            Some((true, step))
        } else if level == self.from {
            Some((false, step))
        } else {
            None
        }
    }

    /// The instance's presence fading in or out across D, as [`Self::draw`].
    fn presence(&self) -> Option<(bool, u8)> {
        let step = (self.t * STEPS).round() as u8;
        if self.t < 1. && self.from == NONE {
            Some((true, step))
        } else if self.t < 1. && self.to == NONE {
            Some((false, step))
        } else if self.to == NONE {
            None
        } else {
            Some((false, 0))
        }
    }
}

/// One prototype: `lod_distance_00..02`, its level count, whether it is
/// in the `scenery` category and whether its last level is an impostor
/// card turned to the eye (trees), which cannot cast the sun's shadow.
#[derive(Clone, Debug)]
pub struct Type {
    pub distances: [f32; 3],
    pub levels: u8,
    pub scenery: bool,
    pub impostor: bool,
}

/// One placed instance: its type and world box.
#[derive(Clone, Debug)]
pub struct Instance {
    pub kind: u32,
    pub min: Vec3,
    pub max: Vec3,
}

/// What the loader found; the spawn turns it into [`Lods`] and
/// [`OrnamentLevel`] components.
#[derive(Default)]
pub struct Levels {
    pub types: Vec<Type>,
    pub instances: Vec<Instance>,
    /// Part name -> its level and the instance of each matrix.
    pub parts: HashMap<String, (u8, Vec<u32>)>,
}

#[derive(Resource)]
pub struct Lods {
    types: Vec<Type>,
    instances: Vec<Instance>,
    /// Per instance this frame: the level shown (`NONE` when culled) and
    /// whether it is inside D.
    shown: Vec<u8>,
    /// Per instance: the fade between the drawn levels.
    fades: Vec<Fade>,
    /// The camera position of the last frame (cut detection).
    eye: Option<Vec3>,
}

pub(crate) const NONE: u8 = u8::MAX;

#[derive(Component)]
pub struct OrnamentLevel {
    pub instance: u32,
    pub level: u8,
}

impl Levels {
    /// The level that casts the type's shadow: level 1 when it has one,
    /// unless that is an impostor.
    pub fn shadow_level(&self, instance: u32) -> u8 {
        shadow_level(&self.types[self.instances[instance as usize].kind as usize])
    }
}

fn shadow_level(kind: &Type) -> u8 {
    u8::from(kind.levels >= 2 && !kind.impostor)
}

pub fn install(app: &mut App) {
    app.add_systems(
        PostUpdate,
        select
            .run_if(resource_exists::<Lods>)
            .after(TransformSystems::Propagate)
            .before(VisibilitySystems::VisibilityPropagate),
    );
    if app.get_sub_app(bevy::render::RenderApp).is_some() {
        app.world_mut()
            .resource_mut::<Assets<Shader>>()
            .insert(
                SHADOW_SHADER.id(),
                Shader::from_wgsl(include_str!("ornament_shadow_fade.wgsl"), "grid_ornament_shadow_fade.wgsl"),
            )
            .expect("ornament shadow fade shader");
        app.add_plugins(MaterialPlugin::<ShadowMaterial>::default())
            .init_resource::<ShadowProxies>()
            .add_systems(
                PostUpdate,
                shadow_proxies
                    .run_if(resource_exists::<Lods>)
                    .after(select)
                    .before(VisibilitySystems::VisibilityPropagate),
            );
    }
    // After every range change of the frame (ornaments here, car levels before
    // transform propagation, drivers in Update) and before Bevy decides which
    // meshes to re-specialize this frame (`check_entities_needing_specialization`
    // follows this system); one frame late, the mesh discards for a frame.
    app.add_systems(
        PostUpdate,
        respecialize_ranged
            .after(select)
            .after(TransformSystems::Propagate)
            .before(bevy::mesh::mark_3d_meshes_as_changed_if_their_assets_changed),
    );
}

/// A shadow-level part whose instance crosses D casts through a proxy that
/// dithers in the shadow map like the colour fade (owner 2026-11-03: nothing
/// should appear in view; F713); the part itself casts none meanwhile.
///
/// A level change keeps the shadow level, but while the shadow level fades
/// in or out as a *colour* level its part carries the fade's
/// `VisibilityRange`, and Bevy's light visibility check applies ranges too:
/// at the fade-out's last step the range excludes every distance, so the
/// part left every cascade until the fade ended and the range went, and its
/// shadow was missing for a frame (row 115, F743: buildings and trees
/// flashed brighter as the car approached them). So it casts through a
/// proxy at full presence (tag 0) for the length of that fade too. A proxy
/// starts with the part's world transform and bounds, so its first frame
/// casts where the part is and not at the origin.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn shadow_proxies(
    lods: Res<Lods>,
    mut state: ResMut<ShadowProxies>,
    parts: Query<
        (
            Entity,
            &OrnamentLevel,
            &Mesh3d,
            Option<&MeshMaterial3d<StandardMaterial>>,
            &GlobalTransform,
            Option<&Aabb>,
            Has<NotShadowCaster>,
        ),
        With<RenderLayers>,
    >,
    mut tags: Query<&mut MeshTag>,
    standard: Res<Assets<StandardMaterial>>,
    mut materials: ResMut<Assets<ShadowMaterial>>,
    mut commands: Commands,
    warm: Option<Res<crate::pipeline_warmup::Warmup>>,
) {
    let state = &mut *state;
    // Pipeline warm-up: every shadow-level part casts through a step-0 proxy.
    let warming = warm.is_some_and(|w| w.proxies());
    for (part, level, mesh, material, transform, aabb, shadowless) in &parts {
        let fade = lods.fades[level.instance as usize];
        let casts = !shadowless || state.proxies.contains_key(&part);
        let colour_fading = matches!(fade.draw(level.level), Some((appearing, step)) if appearing || step > 0);
        let step = match fade.presence() {
            Some((appearing, step)) if (appearing || step > 0) && casts => {
                Some(step.min(31) as u32 | if appearing { 32 } else { 0 })
            }
            Some(_) if colour_fading && casts => Some(0),
            _ if warming && casts => Some(0),
            _ => None,
        };
        match (step, state.proxies.get(&part).copied()) {
            (Some(tag), Some((proxy, _))) => {
                if let Ok(mut current) = tags.get_mut(proxy) {
                    current.set_if_neq(MeshTag(tag));
                }
            }
            (Some(tag), None) => {
                let Some(source) = material else { continue };
                let entry = state.materials.entry(source.0.id()).or_insert_with(|| {
                    let mut base = standard.get(&source.0)?.clone();
                    // The fragment must run to dither: an opaque caster becomes
                    // a mask that keeps every texel; a blended one casts none.
                    base.alpha_mode = match base.alpha_mode {
                        AlphaMode::Opaque => AlphaMode::Mask(0.),
                        AlphaMode::Mask(cutoff) => AlphaMode::Mask(cutoff),
                        AlphaMode::AlphaToCoverage => AlphaMode::Mask(0.5),
                        _ => return None,
                    };
                    Some(materials.add(ShadowMaterial { base, extension: ShadowFade { marker: Vec4::ONE } }))
                });
                let Some(handle) = entry.clone() else { continue };
                let mut proxy = commands.spawn((
                    Mesh3d(mesh.0.clone()),
                    MeshMaterial3d(handle),
                    transform.compute_transform(),
                    *transform,
                    MeshTag(tag),
                    crate::car_batch::shadow_layer(),
                ));
                if let Some(aabb) = aabb {
                    proxy.insert(*aabb);
                }
                let proxy = proxy.id();
                commands.entity(part).insert(NotShadowCaster);
                state.proxies.insert(part, (proxy, shadowless));
            }
            (None, Some((proxy, was_shadowless))) => {
                commands.entity(proxy).despawn();
                if !was_shadowless {
                    commands.entity(part).remove::<NotShadowCaster>();
                }
                state.proxies.remove(&part);
            }
            (None, None) => {}
        }
    }
}

/// Bevy 0.19 re-specializes a mesh's pipeline only when its `Mesh3d` or
/// material changes (`check_entities_needing_specialization`), not when a
/// `VisibilityRange` is added or removed. A dither variant left without a
/// range reads no range and discards the whole mesh (F703), so every insert
/// and removal marks the entity's `Mesh3d` changed.
fn respecialize_ranged(
    mut meshes: Query<&mut Mesh3d>,
    added: Query<Entity, Added<VisibilityRange>>,
    mut removed: RemovedComponents<VisibilityRange>,
) {
    for entity in added.iter().chain(removed.read()) {
        if let Ok(mut mesh) = meshes.get_mut(entity) {
            mesh.set_changed();
        }
    }
}

impl Lods {
    pub fn new(levels: Levels) -> Self {
        println!(
            "ornament_lod version=1 types={} instances={} level_parts={} scenery_types={}",
            levels.types.len(),
            levels.instances.len(),
            levels.parts.len(),
            levels.types.iter().filter(|t| t.scenery).count()
        );
        Self {
            shown: vec![NONE; levels.instances.len()],
            fades: vec![Fade::NONE; levels.instances.len()],
            eye: None,
            types: levels.types,
            instances: levels.instances,
        }
    }
}

/// `v` of an instance (0x965790): 1 inside the box, 0 at or beyond `far`.
/// `k = tan(27.5 deg) / tan(fov/2)`.
pub fn metric(min: Vec3, max: Vec3, eye: Vec3, k: f32, far: f32) -> f32 {
    let dir = ((min + max) * 0.5 - eye).normalize_or_zero();
    let near = Vec3::select(dir.cmpgt(Vec3::ZERO), min, max);
    let d = (near - eye).dot(dir);
    if d <= 0. {
        1.
    } else if d >= far {
        0.
    } else {
        1. - d / ((1. - d / far) * (k - 1.) + 1.) / far
    }
}

/// Where a tree's card replaces its full tree, metres. The tree pass
/// (0x972500, thresholds from 0x986550 with `D = d1 - d0`) fades the full
/// tree in from d1 to `d0 + 0.75 D` and the card out from `d0 + 0.95 D` to
/// `d0 + 0.6 D`, both drawn in between; their weights are equal at
/// `d0 + 5/6 D` (Washington `large_tree` 30 / 150 m: 130 m). The port's
/// levels are exclusive, so it switches there with the ornament fade.
/// `GRID_TREE_SWITCH=d0` keeps the earlier switch at d0, for review.
pub fn impostor_switch(d0: f32, d1: f32) -> f32 {
    static AT_D0: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if d1 <= d0 || *AT_D0.get_or_init(|| std::env::var("GRID_TREE_SWITCH").is_ok_and(|v| v == "d0")) {
        d0
    } else {
        d0 + (d1 - d0) * 5. / 6.
    }
}

/// Level `i`'s range in `v` (0x95D2A0): drawn while `r0 < v <= r1`; `r2`
/// is the cross-fade width above `r0`. A tree (`impostor`) shows its full
/// tree out to [`impostor_switch`] and its card beyond.
pub fn range(kind: &Type, i: u8) -> [f32; 3] {
    let n = kind.levels.max(1);
    let [d0, d1, far] = kind.distances;
    let d = if kind.impostor { [impostor_switch(d0, d1), d1] } else { [d0, d1] };
    let v = |m: f32| (1. - m / far).clamp(0., 1.);
    if n == 1 {
        return [0., 1., 0.];
    }
    let ext = if kind.scenery { SCENERY_OVERLAP } else { 0. };
    let upper = if i == 0 { 1. } else { v(d[i as usize - 1]) };
    if i + 1 >= n {
        [0., upper, 0.]
    } else {
        let r0 = v(d[i as usize] + ext);
        [r0, upper, (v(d[i as usize]) - r0).clamp(0., 1.)]
    }
}

/// The colour level of an instance at `v`, `NONE` beyond D. In a scenery
/// overlap the finer level stays while its fade weight is at least one half.
pub fn level(kind: &Type, v: f32) -> u8 {
    if v <= 0. {
        return NONE;
    }
    for i in 0..kind.levels.max(1) {
        let [r0, r1, r2] = range(kind, i);
        if r0 < v && v <= r1 {
            let fading = r2 > 0. && v < r0 + r2;
            if !fading || (v - r0) / r2 >= 0.5 {
                return i;
            }
        }
    }
    // Only a fading finer level held `v`: the next one is drawn.
    (0..kind.levels.max(1))
        .rev()
        .find(|&i| {
            let [r0, r1, _] = range(kind, i);
            r0 < v && v <= r1
        })
        .unwrap_or(NONE)
}

#[allow(clippy::type_complexity)]
pub(crate) fn select(
    mut lods: ResMut<Lods>,
    camera: Query<(&GlobalTransform, &Projection), With<crate::ViewerCamera>>,
    mut parts: Query<(
        Entity,
        &OrnamentLevel,
        &mut Visibility,
        Option<&mut RenderLayers>,
        Option<&mut VisibilityRange>,
    )>,
    mut commands: Commands,
    time: Option<Res<Time>>,
    mut frame: Local<u64>,
    warm: Option<Res<crate::pipeline_warmup::Warmup>>,
) {
    let Some((view, projection @ Projection::Perspective(lens))) = camera.iter().next() else {
        return;
    };
    let eye = view.translation();
    let k = REFERENCE_HALF_FOV.tan() / (lens.fov * 0.5).tan();
    let lods = &mut *lods;
    let cut = lods.eye.is_none_or(|last| last.distance(eye) > CUT_METRES) || !fade_enabled();
    lods.eye = Some(eye);
    let step = time.map_or(0., |t| t.delta_secs()) / FADE_SECONDS;
    *frame += 1;
    let trace = trace_enabled().then(|| projection.get_clip_from_view() * view.to_matrix().inverse());
    for ((instance, shown), fade) in lods.instances.iter().zip(&mut lods.shown).zip(&mut lods.fades) {
        let kind = &lods.types[instance.kind as usize];
        let v = metric(instance.min, instance.max, eye, k, kind.distances[2]);
        let new = level(kind, v);
        if let Some(clip_from_world) = trace
            && new != *shown
            && *frame > 1
        {
            trace_change(*frame, instance, kind, *shown, new, eye, lens.fov, clip_from_world);
        }
        *shown = new;
        fade.advance(new, step, cut);
    }
    let caster = RenderLayers::layer(0);
    let shadow_only = crate::car_batch::shadow_layer();
    // Pipeline warm-up (behind a cover): every level is drawn and casts; the
    // warm-up owns the ranges meanwhile.
    let warming = warm.is_some_and(|w| w.showing_all());
    for (entity, part, mut visibility, layers, range) in &mut parts {
        if warming {
            visibility.set_if_neq(Visibility::Inherited);
            if let Some(mut layers) = layers
                && layers.clone().without(crate::car_reflections::LAYER) != caster
            {
                *layers = caster.clone();
            }
            continue;
        }
        let fade = lods.fades[part.instance as usize];
        let colour = fade.draw(part.level);
        // The shadow level is drawn inside D, by the camera only while it is
        // a colour level; otherwise it fades with the instance's presence.
        let drawn = if layers.is_some() { colour.or(fade.presence()) } else { colour };
        visibility.set_if_neq(if drawn.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
        if let Some(mut layers) = layers {
            let want = if colour.is_some() { &caster } else { &shadow_only };
            if layers.clone().without(crate::car_reflections::LAYER) != *want {
                *layers = want.clone();
            }
        }
        match (range, drawn) {
            (Some(mut range), Some((appearing, step))) if appearing || step > 0 => {
                range.set_if_neq(fade_range(appearing, step));
            }
            (None, Some((appearing, step))) if appearing || step > 0 => {
                commands.entity(entity).insert(fade_range(appearing, step));
            }
            (Some(_), _) => {
                commands.entity(entity).remove::<VisibilityRange>();
            }
            (None, _) => {}
        }
    }
}

/// `GRID_ORNAMENT_LOD_TRACE=1`: print each on-screen level change (research).
pub fn trace_enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("GRID_ORNAMENT_LOD_TRACE").is_ok_and(|v| v == "1"))
}

/// One level change of an instance whose box centre is in the view: its
/// distance and its box diagonal in pixels of a 1080-row view.
#[allow(clippy::too_many_arguments)]
fn trace_change(frame: u64, instance: &Instance, kind: &Type, from: u8, to: u8, eye: Vec3, fov: f32, clip_from_world: Mat4) {
    let centre = (instance.min + instance.max) * 0.5;
    let clip = clip_from_world * centre.extend(1.);
    if clip.w <= 0. {
        return;
    }
    let ndc = clip.truncate() / clip.w;
    if ndc.x.abs() > 1.1 || ndc.y.abs() > 1.1 {
        return;
    }
    let distance = (centre - eye).length().max(0.1);
    let pixels = (instance.max - instance.min).length() / distance / (2. * (fov * 0.5).tan()) * 1080.;
    let name = |l: u8| if l == NONE { "none".to_string() } else { l.to_string() };
    println!(
        "ornament_change frame={frame} kind={} from={} to={} distance={distance:.1} far={:.0} levels={} scenery={} pixels={pixels:.0} ndc={:.2},{:.2}",
        instance.kind,
        name(from),
        name(to),
        kind.distances[2],
        kind.levels,
        u8::from(kind.scenery),
        ndc.x,
        ndc.y
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn building() -> Type {
        Type {
            distances: [100., 300., 700.],
            levels: 3,
            scenery: true,
            impostor: false,
        }
    }

    fn at(kind: &Type, metres: f32) -> u8 {
        let eye = Vec3::new(-metres, 0., 0.);
        level(kind, metric(Vec3::ZERO, Vec3::ONE, eye, 1., kind.distances[2]))
    }

    #[test]
    fn a_level_change_fades_over_the_fade_time() {
        let mut fade = Fade::NONE;
        fade.advance(2, 0.25, true);
        assert_eq!(fade.draw(2), Some((false, 0)));
        // 2 -> 1 over four frames: 1 dithers in while 2 dithers out.
        fade.advance(1, 0.25, false);
        assert_eq!((fade.draw(1), fade.draw(2), fade.draw(0)), (Some((true, 4)), Some((false, 4)), None));
        fade.advance(1, 0.25, false);
        fade.advance(1, 0.25, false);
        fade.advance(1, 0.25, false);
        assert_eq!((fade.draw(1), fade.draw(2)), (Some((false, 0)), None));
        // Back halfway: reversed, not restarted.
        fade.advance(2, 0.25, false);
        fade.advance(2, 0.25, false);
        fade.advance(1, 0.25, false);
        assert_eq!(fade, Fade { from: 2, to: 1, t: 0.75 });
        // Leaving D fades the presence out, a cut switches at once.
        fade.advance(NONE, 0.5, false);
        assert_eq!(fade.presence(), Some((false, 8)));
        fade.advance(1, 0., true);
        assert_eq!(fade.draw(1), Some((false, 0)));
    }

    #[test]
    fn fade_ranges_give_the_dither_step_at_any_distance() {
        // Bevy's get_visibility_range_dither_level, on the CPU.
        let dither = |r: &VisibilityRange, d: f32| -> i32 {
            let (offset, a, b) = if d >= r.end_margin.start {
                (0, r.end_margin.start, r.end_margin.end)
            } else {
                (-16, r.start_margin.start, r.start_margin.end)
            };
            offset + (((d - a) / (b - a) * 16.).round() as i32).clamp(0, 16)
        };
        for d in [0., 1., 250., 7000., 25000.] {
            for step in 0..=16u8 {
                let s = i32::from(step);
                assert_eq!(dither(&fade_range(true, step), d), s - 16, "in {step} at {d}");
                assert_eq!(dither(&fade_range(false, step), d), s, "out {step} at {d}");
                assert!(fade_range(true, step).is_visible_at_all(d));
                // The last fade-out step is culled outright.
                assert_eq!(fade_range(false, step).is_visible_at_all(d), step < 16);
            }
        }
        assert!(!fade_range(false, 0).is_abrupt());
    }

    #[test]
    fn levels_follow_the_attribute_distances() {

        let plain = Type {
            scenery: false,
            impostor: false,
            ..building()
        };
        assert_eq!(at(&plain, 50.), 0);
        assert_eq!(at(&plain, 150.), 1);
        assert_eq!(at(&plain, 400.), 2);
        assert_eq!(at(&plain, 800.), NONE);
        // Scenery keeps the finer level to the middle of its 20 m overlap.
        let scenery = building();
        assert_eq!(at(&scenery, 105.), 0);
        assert_eq!(at(&scenery, 115.), 1);
        assert_eq!(at(&scenery, 305.), 1);
        assert_eq!(at(&scenery, 315.), 2);
    }

    #[test]
    fn metric_measures_to_the_near_side_and_warps_by_fov() {
        // Inside the box: full detail.
        assert_eq!(metric(Vec3::ZERO, Vec3::ONE, Vec3::splat(0.5), 1., 100.), 1.);
        // 10 m to the near face at the reference FOV.
        let v = metric(Vec3::ZERO, Vec3::ONE, Vec3::new(-10., 0.5, 0.5), 1., 100.);
        assert!((v - 0.9).abs() < 1e-5, "{v}");
        // A narrower FOV (k > 1) brings it nearer, the cut-off stays at D.
        assert!(metric(Vec3::ZERO, Vec3::ONE, Vec3::new(-10., 0.5, 0.5), 2., 100.) > v);
        assert_eq!(metric(Vec3::ZERO, Vec3::ONE, Vec3::new(-101., 0.5, 0.5), 2., 100.), 0.);
    }

    #[test]
    fn two_levels_ignore_the_middle_distance() {
        let kind = Type {
            distances: [100., 9999., 400.],
            levels: 2,
            scenery: false,
            impostor: false,
        };
        assert_eq!(range(&kind, 1), [0., 0.75, 0.]);
        assert_eq!(shadow_level(&kind), 1);
        let single = Type { levels: 1, ..kind.clone() };
        assert_eq!(range(&single, 0), [0., 1., 0.]);
        assert_eq!(shadow_level(&single), 0);
        // A tree's card faces the eye: the full tree casts.
        assert_eq!(shadow_level(&Type { impostor: true, ..kind }), 0);
    }

    #[test]
    fn a_tree_switches_to_its_card_where_the_original_cross_fades() {
        let tree = Type {
            distances: [30., 150., 940.],
            levels: 2,
            scenery: false,
            impostor: true,
        };
        assert_eq!(impostor_switch(30., 150.), 130.);
        assert_eq!(at(&tree, 100.), 0);
        assert_eq!(at(&tree, 125.), 0);
        assert_eq!(at(&tree, 135.), 1);
        assert_eq!(at(&tree, 900.), 1);
        assert_eq!(at(&tree, 950.), NONE);
        // Without a larger d1 it stays at d0.
        assert_eq!(impostor_switch(30., 30.), 30.);
    }

    #[test]
    fn installed_selector_handles_static_scene_lifecycle_without_driving() {
        // Exercise actual registration and PostUpdate selection, with no Driving
        // resource or renderer. GlobalTransform is supplied as it is after the
        // production transform-propagation boundary.
        let mut app = App::new();
        install(&mut app);
        let camera = app.world_mut().spawn((
            crate::ViewerCamera,
            GlobalTransform::from_translation(Vec3::new(-10., 0.5, 0.5)),
            Projection::Perspective(PerspectiveProjection {
                fov: REFERENCE_HALF_FOV * 2., ..default()
            }),
        )).id();
        let part = app.world_mut().spawn((
            OrnamentLevel { instance: 0, level: 0 },
            Visibility::Hidden,
        )).id();

        // Shared registration is safe before setup creates Lods (and in scenes
        // which never create one): resource_exists must prevent ResMut failure.
        app.world_mut().run_schedule(PostUpdate);
        assert_eq!(*app.world().get::<Visibility>(part).unwrap(), Visibility::Hidden);

        app.insert_resource(Lods::new(Levels {
            types: vec![building()],
            instances: vec![Instance { kind: 0, min: Vec3::ZERO, max: Vec3::ONE }],
            parts: HashMap::new(),
        }));
        app.world_mut().run_schedule(PostUpdate);
        assert_eq!(*app.world().get::<Visibility>(part).unwrap(), Visibility::Inherited);

        // A real camera change must cull the same entity outside its stored D.
        *app.world_mut().get_mut::<GlobalTransform>(camera).unwrap() =
            GlobalTransform::from_translation(Vec3::new(-701., 0.5, 0.5));
        app.world_mut().run_schedule(PostUpdate);
        assert_eq!(*app.world().get::<Visibility>(part).unwrap(), Visibility::Hidden);

        // Removing a scene's Lods also leaves the shared selector safe.
        let _ = app.world_mut().remove_resource::<Lods>();
        app.world_mut().run_schedule(PostUpdate);
        assert_eq!(*app.world().get::<Visibility>(part).unwrap(), Visibility::Hidden);
    }

    #[test]
    fn a_shadow_crossing_d_casts_through_a_dithered_proxy() {
        // The renderer half (MaterialPlugin, shader) is left out; the system
        // runs as `install` orders it when a RenderApp exists.
        let mut app = App::new();
        install(&mut app);
        app.init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<ShadowMaterial>>()
            .init_resource::<ShadowProxies>()
            .insert_resource(Time::<()>::default())
            .add_systems(PostUpdate, shadow_proxies.run_if(resource_exists::<Lods>).after(select));
        let material = app.world_mut().resource_mut::<Assets<StandardMaterial>>().add(StandardMaterial::default());
        let camera = app.world_mut().spawn((
            crate::ViewerCamera,
            GlobalTransform::from_translation(Vec3::new(-705., 0.5, 0.5)),
            Projection::Perspective(PerspectiveProjection {
                fov: REFERENCE_HALF_FOV * 2., ..default()
            }),
        )).id();
        // The shadow level of a three-level building is level 1.
        let part = app.world_mut().spawn((
            OrnamentLevel { instance: 0, level: 1 },
            Mesh3d(Handle::default()),
            MeshMaterial3d(material),
            GlobalTransform::IDENTITY,
            RenderLayers::layer(0),
        )).id();
        app.insert_resource(Lods::new(Levels {
            types: vec![building()],
            instances: vec![Instance { kind: 0, min: Vec3::ZERO, max: Vec3::ONE }],
            parts: HashMap::new(),
        }));
        let frame = |app: &mut App| {
            app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_millis(100));
            app.world_mut().run_schedule(PostUpdate);
        };
        let proxies = |app: &mut App| {
            app.world_mut().query::<(&MeshTag, &MeshMaterial3d<ShadowMaterial>)>().iter(app.world()).map(|(t, _)| t.0).collect::<Vec<_>>()
        };
        // Outside D: nothing casts.
        frame(&mut app);
        assert!(proxies(&mut app).is_empty());

        // Crossing D (not a cut): the part stops casting and the proxy dithers in.
        *app.world_mut().get_mut::<GlobalTransform>(camera).unwrap() =
            GlobalTransform::from_translation(Vec3::new(-695., 0.5, 0.5));
        frame(&mut app);
        let tags = proxies(&mut app);
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0] & 32, 32, "appearing");
        assert!(tags[0] & 31 > 0 && tags[0] & 31 < 16);
        assert!(app.world().get::<NotShadowCaster>(part).is_some());
        frame(&mut app);
        assert!(proxies(&mut app)[0] & 31 > tags[0] & 31);

        // At the end of the fade the part casts again and the proxy is gone.
        for _ in 0..4 {
            frame(&mut app);
        }
        assert!(proxies(&mut app).is_empty());
        assert!(app.world().get::<NotShadowCaster>(part).is_none());
    }

    #[test]
    fn the_shadow_level_casts_in_full_while_it_fades_as_a_colour_level() {
        let mut app = App::new();
        install(&mut app);
        app.init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<ShadowMaterial>>()
            .init_resource::<ShadowProxies>()
            .insert_resource(Time::<()>::default())
            .add_systems(PostUpdate, shadow_proxies.run_if(resource_exists::<Lods>).after(select));
        let material = app.world_mut().resource_mut::<Assets<StandardMaterial>>().add(StandardMaterial::default());
        // Level 1 (the shadow level) is the colour level at 200 m.
        let camera = app.world_mut().spawn((
            crate::ViewerCamera,
            GlobalTransform::from_translation(Vec3::new(-200., 0.5, 0.5)),
            Projection::Perspective(PerspectiveProjection {
                fov: REFERENCE_HALF_FOV * 2., ..default()
            }),
        )).id();
        let placed = GlobalTransform::from_translation(Vec3::new(3., 4., 5.));
        let part = app.world_mut().spawn((
            OrnamentLevel { instance: 0, level: 1 },
            Mesh3d(Handle::default()),
            MeshMaterial3d(material),
            placed,
            Visibility::Inherited,
            RenderLayers::layer(0),
        )).id();
        app.insert_resource(Lods::new(Levels {
            types: vec![building()],
            instances: vec![Instance { kind: 0, min: Vec3::ZERO, max: Vec3::ONE }],
            parts: HashMap::new(),
        }));
        let frame = |app: &mut App| {
            app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_millis(100));
            app.world_mut().run_schedule(PostUpdate);
        };
        let proxies = |app: &mut App| {
            app.world_mut()
                .query::<(&MeshTag, &GlobalTransform, &MeshMaterial3d<ShadowMaterial>)>()
                .iter(app.world())
                .map(|(t, g, _)| (t.0, *g))
                .collect::<Vec<_>>()
        };
        frame(&mut app);
        frame(&mut app);
        assert!(proxies(&mut app).is_empty());
        assert!(app.world().get::<NotShadowCaster>(part).is_none());

        // Approaching to 50 m in steps under CUT_METRES: level 1 fades out as
        // a colour level over FADE_SECONDS and carries the fade's range
        // meanwhile. Its shadow must not fade with it: a full proxy (tag 0) at
        // the part's place casts.
        let mut fading = 0;
        for x in (5..=20).rev() {
            *app.world_mut().get_mut::<GlobalTransform>(camera).unwrap() =
                GlobalTransform::from_translation(Vec3::new(-10. * x as f32, 0.5, 0.5));
            frame(&mut app);
            if app.world().get::<VisibilityRange>(part).is_some() {
                fading += 1;
                assert_eq!(proxies(&mut app), vec![(0, placed)], "full shadow where the part is");
                assert!(app.world().get::<NotShadowCaster>(part).is_some());
            }
        }
        assert!(fading > 0, "the colour fade ran");
        // After the fade the part casts again, without a range.
        for _ in 0..4 {
            frame(&mut app);
        }
        assert!(app.world().get::<VisibilityRange>(part).is_none());
        assert!(proxies(&mut app).is_empty());
        assert!(app.world().get::<NotShadowCaster>(part).is_none());
    }
}
