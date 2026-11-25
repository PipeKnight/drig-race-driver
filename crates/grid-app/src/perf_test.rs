//! Performance test (AUTHORED diagnostics, P12, docs/PERFORMANCE.md "Phone
//! bisection"): presentation switches applied live from the front end's
//! Performance test screen, so one phone run can show which part of the frame
//! costs what. Every `frame_stats` line carries the active switches
//! (`perf_test=...`); with Auto cycle the switches step through [`CYCLE`]
//! every [`CYCLE_SECONDS`], so a single run gives a whole table.
//!
//! Only visibility, shadow casting (`NotShadowCaster`), the 3D camera's
//! settings (MSAA, indirect drawing, environment maps), log lines and which
//! executor runs the schedules change, never the simulation, and nothing is
//! saved: every start is the normal game.
//!
//! The mobile preset's render profile is built from the same switches
//! ([`PerfTest::base`], [`base_for`]): with `mobile::enabled()` every start
//! begins at [`Switches::LEAN`] (F928; owner 2026-11-25: the phone may show
//! less detail and a worse picture), and the screen's or the setting's words
//! add to it; the word `full` starts from [`Switches::NORMAL`] instead, for
//! A/B runs. The desktop base is NORMAL, so the desktop picture is unchanged.
use bevy::camera::primitives::Aabb;
use bevy::camera::visibility::{RenderLayers, VisibilityRange};
use bevy::light::{
    CascadeShadowConfig, CascadeShadowConfigBuilder, DirectionalLightShadowMap, EnvironmentMapLight,
    GeneratedEnvironmentMapLight, NotShadowCaster, ShadowFilteringMethod,
};
use bevy::render::view::NoIndirectDrawing;
use bevy::ecs::schedule::{
    InternedScheduleLabel, MultiThreadedExecutor, ScheduleLabel, SingleThreadedExecutor,
};
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

/// The switches; `true` turns that part off.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Switches {
    pub shadows: bool,
    pub foliage: bool,
    pub ground: bool,
    pub effects: bool,
    pub hud: bool,
    pub samples: bool,
    pub empty: bool,
    /// 3D view rows (`render_scale`), 0 = the setting in use.
    pub render_rows: u16,
    /// Sun shadows at mobile cost: 1024 map, cascades to 60 m.
    pub light_shadows: bool,
    /// Small meshes (props, signs, foliage) stop casting shadows.
    pub small_casters: bool,
    /// Small meshes beyond `mobile::FAR_PROPS_METRES` are not drawn.
    pub far_props: bool,
    /// The 3D camera renders without MSAA.
    pub msaa_off: bool,
    /// The cockpit's rear-view mirror stops updating (head-cam only).
    pub mirror: bool,
    /// The 3D camera renders with 2x MSAA (`msaa_off` wins).
    pub msaa2: bool,
    /// The 3D camera draws without GPU-built indirect draws
    /// (`NoIndirectDrawing`, docs/ANDROID_GPU.md).
    pub no_indirect: bool,
    /// The cars' reflection overlays are hidden and the dynamic cube draws
    /// no faces.
    pub reflections: bool,
    /// Direct and environment light off: every light is hidden and the 3D
    /// camera's environment maps are removed, so the PBR fragment skips the
    /// light loops, shadow lookups and cube samples. The opaque pass then
    /// shows what the rest of the fragment work, the geometry and the
    /// render targets cost (docs/ANDROID_GPU.md).
    pub lighting: bool,
    /// The main world's update schedules ([`main_schedules`]) run on one
    /// thread; parallel queries inside systems still use the compute pool (F915).
    pub main_single: bool,
    /// The render world's schedules ([`render_schedules`]) run on one thread.
    pub render_single: bool,
    /// The 3D cameras filter shadows with one hardware 2x2 compare
    /// (`ShadowFilteringMethod::Hardware2x2`) instead of the Gaussian kernel.
    pub hard_shadows: bool,
    /// Point and spot lights are hidden (the sun and the sky light stay), so
    /// the clustered light loop finds no lights.
    pub local_lights: bool,
    /// The 3D cameras' environment maps (the sky or scene cube every PBR
    /// fragment samples) are removed; the lights stay. With `reflections`
    /// this is the `no_reflections` word: no cube is drawn or sampled.
    pub env_maps: bool,
    /// The hard shadow cut: like `light_shadows` with a 256 map and the
    /// cascades within 25 m (wins over `light_shadows`; set it before the
    /// cut starts, the two do not swap while one is active).
    pub tiny_shadows: bool,
}

impl Switches {
    /// Turns on every switch `other` has on (a word such as `lean` adds to the others).
    pub fn add(&mut self, other: Self) {
        self.shadows |= other.shadows;
        self.foliage |= other.foliage;
        self.ground |= other.ground;
        self.effects |= other.effects;
        self.hud |= other.hud;
        self.samples |= other.samples;
        self.empty |= other.empty;
        self.light_shadows |= other.light_shadows;
        self.small_casters |= other.small_casters;
        self.far_props |= other.far_props;
        self.msaa_off |= other.msaa_off;
        self.mirror |= other.mirror;
        self.msaa2 |= other.msaa2;
        self.no_indirect |= other.no_indirect;
        self.reflections |= other.reflections;
        self.lighting |= other.lighting;
        self.main_single |= other.main_single;
        self.render_single |= other.render_single;
        self.hard_shadows |= other.hard_shadows;
        self.local_lights |= other.local_lights;
        self.env_maps |= other.env_maps;
        self.tiny_shadows |= other.tiny_shadows;
        if other.render_rows != 0 {
            self.render_rows = other.render_rows;
        }
    }
    pub const NORMAL: Self = Self {
        shadows: false,
        foliage: false,
        ground: false,
        effects: false,
        hud: false,
        samples: false,
        empty: false,
        render_rows: 0,
        light_shadows: false,
        small_casters: false,
        far_props: false,
        msaa_off: false,
        mirror: false,
        msaa2: false,
        no_indirect: false,
        reflections: false,
        lighting: false,
        main_single: false,
        render_single: false,
        hard_shadows: false,
        local_lights: false,
        env_maps: false,
        tiny_shadows: false,
    };
    /// `no_reflections`: what every reflection costs, apart from the other
    /// cuts: no car overlay, no dynamic cube face, no camera environment map.
    pub const NO_REFLECTIONS: Self = Self { reflections: true, env_maps: true, ..Self::NORMAL };
    /// `lean`: every cheaper path at once, the ceiling of the mobile cuts
    /// (docs/ANDROID_GPU.md section 7): no MSAA, a 256 sun shadow map within
    /// 25 m with a 2x2 filter, small props cast no shadows and end at the far-prop range,
    /// no car reflection overlays, no point or spot lights.
    pub const LEAN: Self = Self {
        msaa_off: true,
        light_shadows: true,
        small_casters: true,
        far_props: true,
        reflections: true,
        hard_shadows: true,
        local_lights: true,
        tiny_shadows: true,
        ..Self::NORMAL
    };
    pub const ALL_OFF: Self = Self {
        shadows: true,
        foliage: true,
        ground: true,
        effects: true,
        hud: true,
        samples: true,
        ..Self::NORMAL
    };

    /// `frame_stats` label: the parts turned off, or `normal`.
    pub fn label(&self) -> String {
        let names = [
            (self.shadows, "shadows"),
            (self.foliage, "foliage"),
            (self.ground, "ground"),
            (self.effects, "effects"),
            (self.hud, "hud"),
            (self.samples, "samples"),
            (self.empty, "empty_scene"),
            (self.mirror, "mirror"),
            (self.reflections, "reflections"),
            (self.lighting, "lighting"),
            (self.local_lights, "local_lights"),
            (self.env_maps, "env_maps"),
        ];
        let off: Vec<&str> = names.iter().filter(|(on, _)| *on).map(|(_, n)| *n).collect();
        let mut cheaper: Vec<String> = Vec::new();
        if self.render_rows > 0 {
            cheaper.push(format!("render{}", self.render_rows));
        }
        for (on, name) in [
            (self.light_shadows, "light_shadows"),
            (self.small_casters, "no_small_casters"),
            (self.far_props, "far_props"),
            (self.msaa_off, "msaa_off"),
            (self.msaa2, "msaa2"),
            (self.no_indirect, "no_indirect"),
            (self.hard_shadows, "hard_shadows"),
            (self.tiny_shadows, "tiny_shadows"),
            (self.main_single, "main_single"),
            (self.render_single, "render_single"),
        ] {
            if on {
                cheaper.push(name.into());
            }
        }
        let mut parts = Vec::new();
        if !off.is_empty() {
            parts.push(format!("off:{}", off.join("+")));
        }
        if !cheaper.is_empty() {
            parts.push(format!("mobile:{}", cheaper.join("+")));
        }
        if parts.is_empty() { "normal".into() } else { parts.join(",") }
    }
}

/// Auto cycle order (docs/ANDROID_GPU.md section 7): the normal frame, each
/// drawn part alone, all of them, the car alone, then the GPU variants: 720
/// rows (F555's phone height), no MSAA, 2x MSAA, no indirect draws, no car
/// reflections, no lighting, the cars alone without lighting, and the normal
/// frame again, so a drift between the first and last steps shows heat. Only
/// switches that can change GPU time are here; Sample logging, the mirror (no
/// mirror in the chase view) and the executors stay screen-only (F915/F925).
/// 16 steps of [`CYCLE_SECONDS`] take 5 min 20 s.
pub const CYCLE: [Switches; 16] = {
    const N: Switches = Switches::NORMAL;
    [
        N,
        Switches { shadows: true, ..N },
        Switches { foliage: true, ..N },
        Switches { ground: true, ..N },
        Switches { effects: true, ..N },
        Switches { hud: true, ..N },
        Switches::ALL_OFF,
        Switches { empty: true, ..Switches::ALL_OFF },
        Switches { render_rows: 720, ..N },
        Switches { msaa_off: true, ..N },
        Switches { msaa2: true, ..N },
        Switches { no_indirect: true, ..N },
        Switches { reflections: true, ..N },
        Switches { lighting: true, ..N },
        Switches { lighting: true, empty: true, ..Switches::ALL_OFF },
        N,
    ]
};
/// Seconds per Auto cycle step: long enough for a phone's GPU clock and the
/// dynamic resolution to settle (owner, 2026-11-25: 20-30 s).
pub const CYCLE_SECONDS: f32 = 20.;

/// The switches every start begins from: [`Switches::LEAN`] with the
/// mobile preset (the phone's render profile), else [`Switches::NORMAL`].
pub fn base_for(mobile: bool) -> Switches {
    if mobile { Switches::LEAN } else { Switches::NORMAL }
}

/// Live state, changed by the front end's Performance test screen.
#[derive(Resource, Debug, Default)]
pub struct PerfTest {
    /// The platform's render profile ([`base_for`]); `full` resets it.
    pub base: Switches,
    /// The screen's, the setting's or the Auto cycle's switches, added to `base`.
    pub switches: Switches,
    pub auto: bool,
    running: bool,
    step: usize,
    timer: f32,
    applied: Option<Switches>,
    frames: u32,
}

static LABEL: Mutex<Option<String>> = Mutex::new(None);
static QUIET_SAMPLES: AtomicBool = AtomicBool::new(false);

impl PerfTest {
    /// What is in effect: [`PerfTest::base`] plus [`PerfTest::switches`].
    pub fn active(&self) -> Switches {
        let mut s = self.base;
        s.add(self.switches);
        s
    }
}

/// ` perf_test=...` for `frame_stats`, empty while everything is normal.
pub fn stats_suffix() -> String {
    LABEL
        .lock()
        .ok()
        .and_then(|l| l.clone())
        .map_or_else(String::new, |l| format!(" perf_test={l}"))
}

/// Periodic `drive_*_sample` lines are skipped (Sample logging off).
pub fn quiet_samples() -> bool {
    QUIET_SAMPLES.load(Ordering::Relaxed)
}

pub fn install(app: &mut App) {
    let mut test = PerfTest { base: base_for(crate::mobile::enabled()), ..default() };
    // Diagnostics `perf_test` (settings) or GRID_PERF_TEST: auto, or the
    // parts to turn off, e.g. shadows,foliage (names as in the label). The
    // phone has no environment, so tools/android_bench.ps1 uses the setting.
    if let Some(value) = crate::settings::diagnostics().perf_test.clone() {
        for part in value.split(',').map(str::trim) {
            let s = &mut test.switches;
            match part {
                "auto" => test.auto = true,
                "full" => test.base = Switches::NORMAL,
                "shadows" => s.shadows = true,
                "foliage" => s.foliage = true,
                "ground" => s.ground = true,
                "effects" => s.effects = true,
                "hud" => s.hud = true,
                "samples" => s.samples = true,
                "empty_scene" | "empty" => s.empty = true,
                "light_shadows" => s.light_shadows = true,
                "no_small_casters" => s.small_casters = true,
                "far_props" => s.far_props = true,
                "msaa_off" => s.msaa_off = true,
                "mirror" => s.mirror = true,
                "msaa2" => s.msaa2 = true,
                "no_indirect" => s.no_indirect = true,
                "reflections" => s.reflections = true,
                "lighting" => s.lighting = true,
                "main_single" => s.main_single = true,
                "render_single" => s.render_single = true,
                "hard_shadows" => s.hard_shadows = true,
                "local_lights" => s.local_lights = true,
                "env_maps" => s.env_maps = true,
                "tiny_shadows" => s.tiny_shadows = true,
                "lean" => s.add(Switches::LEAN),
                "no_reflections" => s.add(Switches::NO_REFLECTIONS),
                rows if rows.starts_with("render") => {
                    s.render_rows = rows[6..].parse().unwrap_or(0);
                }
                _ => warn!("GRID_PERF_TEST: unknown part {part:?}"),
            }
        }
    }
    app.insert_resource(test)
        .add_systems(Last, (cycle, apply, main_executors).chain())
        .add_systems(Last, churn);
    if let Some(render) = app.get_sub_app_mut(bevy::render::RenderApp) {
        render.add_systems(bevy::render::ExtractSchedule, render_executors);
    }
}

/// Main-world schedules `main_single` switches. Bevy gives each the
/// multi-threaded executor; `Main`, `FixedMain` and `RunFixedMainLoop` are
/// single-threaded already, and `Last`, where the switch runs, keeps its own.
fn main_schedules() -> [InternedScheduleLabel; 9] {
    [
        First.intern(),
        PreUpdate.intern(),
        FixedFirst.intern(),
        FixedPreUpdate.intern(),
        FixedUpdate.intern(),
        FixedPostUpdate.intern(),
        FixedLast.intern(),
        Update.intern(),
        PostUpdate.intern(),
    ]
}

/// Render-world schedules `render_single` switches (multi-threaded in Bevy;
/// `ExtractSchedule`, where the switch runs, keeps its own executor).
fn render_schedules() -> [InternedScheduleLabel; 4] {
    [
        bevy::render::Render.intern(),
        bevy::render::renderer::RenderGraph.intern(),
        bevy::core_pipeline::Core3d.intern(),
        bevy::core_pipeline::Core2d.intern(),
    ]
}

/// Gives each schedule in `labels` that exists the single- or the
/// multi-threaded executor; returns how many it changed.
fn set_executors(schedules: &mut Schedules, labels: &[InternedScheduleLabel], single: bool) -> usize {
    let mut changed = 0;
    for &label in labels {
        if let Some(schedule) = schedules.get_mut(label) {
            if single {
                schedule.set_executor(SingleThreadedExecutor::new());
            } else {
                schedule.set_executor(MultiThreadedExecutor::new());
            }
            changed += 1;
        }
    }
    changed
}

/// `main_single`: a running schedule is out of `Schedules`, so this runs in
/// `Last` and changes the others for the next frame, only when the switch changes.
fn main_executors(world: &mut World, mut applied: Local<bool>) {
    let single = world.get_resource::<PerfTest>().is_some_and(|t| t.active().main_single);
    if single == *applied {
        return;
    }
    *applied = single;
    let changed = set_executors(&mut world.resource_mut::<Schedules>(), &main_schedules(), single);
    log_line!("perf_test version=1 event=executor world=main single={single} schedules={changed}");
}

/// `render_single`: runs in the render world's `ExtractSchedule`, while its
/// render schedules are idle, and reads the switch from the main world.
fn render_executors(world: &mut World, mut applied: Local<bool>) {
    let single = world
        .get_resource::<bevy::render::MainWorld>()
        .and_then(|main| main.get_resource::<PerfTest>())
        .is_some_and(|t| t.active().render_single);
    if single == *applied {
        return;
    }
    *applied = single;
    let changed = set_executors(&mut world.resource_mut::<Schedules>(), &render_schedules(), single);
    log_line!("perf_test version=1 event=executor world=render single={single} schedules={changed}");
}

/// Seconds per `asset_churn` line.
const CHURN_SECONDS: f32 = 10.;

#[derive(Default)]
struct Churn {
    elapsed: f32,
    frames: u32,
    /// Added, modified, removed.
    images: [u32; 3],
    meshes: [u32; 3],
    modified_images: HashMap<AssetId<Image>, u32>,
    modified_meshes: HashMap<AssetId<Mesh>, u32>,
}

fn count<A: Asset>(
    events: &mut MessageReader<AssetEvent<A>>,
    totals: &mut [u32; 3],
    modified: &mut HashMap<AssetId<A>, u32>,
) {
    for event in events.read() {
        match event {
            AssetEvent::Added { .. } => totals[0] += 1,
            AssetEvent::Modified { id } => {
                totals[1] += 1;
                *modified.entry(*id).or_default() += 1;
            }
            AssetEvent::Removed { .. } => totals[2] += 1,
            _ => {}
        }
    }
}

/// `asset_churn` every [`CHURN_SECONDS`]: Image and Mesh assets added,
/// modified and removed per frame, and the most modified ones (meshes named
/// by an entity that draws them). Every change is prepared again on the
/// render thread, so steady churn is per-frame upload cost.
fn churn(
    mut state: Local<Churn>,
    time: Res<Time<Real>>,
    mut image_events: MessageReader<AssetEvent<Image>>,
    mut mesh_events: MessageReader<AssetEvent<Mesh>>,
    images: Res<Assets<Image>>,
    drawn: Query<(&Mesh3d, Option<&Name>, Option<&ChildOf>)>,
    names: Query<&Name>,
) {
    let s = &mut *state;
    count(&mut image_events, &mut s.images, &mut s.modified_images);
    count(&mut mesh_events, &mut s.meshes, &mut s.modified_meshes);
    s.frames += 1;
    s.elapsed += time.delta_secs();
    if s.elapsed < CHURN_SECONDS {
        return;
    }
    let frames = s.frames.max(1) as f32;
    let per = |n: u32| n as f32 / frames;
    let mut top_meshes: Vec<_> = s.modified_meshes.drain().collect();
    top_meshes.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    let meshes: Vec<String> = top_meshes
        .iter()
        .take(6)
        .map(|(id, n)| {
            let name = drawn
                .iter()
                .find(|(m, ..)| m.0.id() == *id)
                .map(|(_, name, parent)| {
                    name.map(|n| n.as_str().to_owned())
                        .or_else(|| {
                            parent
                                .and_then(|p| names.get(p.parent()).ok())
                                .map(|n| format!("child_of_{}", n.as_str()))
                        })
                        .unwrap_or_else(|| "unnamed".into())
                })
                .unwrap_or_else(|| "not_drawn".into());
            format!("{:.2}:{}", per(*n), name.replace(' ', "_"))
        })
        .collect();
    let mut top_images: Vec<_> = s.modified_images.drain().collect();
    top_images.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    let image_list: Vec<String> = top_images
        .iter()
        .take(6)
        .map(|(id, n)| {
            let size = images.get(*id).map_or("gone".into(), |i| {
                format!("{}x{}:{:?}", i.width(), i.height(), i.texture_descriptor.format)
            });
            format!("{:.2}:{size}", per(*n))
        })
        .collect();
    let list = |v: Vec<String>| if v.is_empty() { "-".into() } else { v.join(",") };
    log_line!(
        "asset_churn version=1 seconds={:.1} frames={} image_added={:.2} image_modified={:.2} image_removed={:.2} mesh_added={:.2} mesh_modified={:.2} mesh_removed={:.2} top_meshes={} top_images={}{}",
        s.elapsed,
        s.frames,
        per(s.images[0]),
        per(s.images[1]),
        per(s.images[2]),
        per(s.meshes[0]),
        per(s.meshes[1]),
        per(s.meshes[2]),
        list(meshes),
        list(image_list),
        stats_suffix()
    );
    *s = Churn::default();
}

fn cycle(mut test: ResMut<PerfTest>, time: Res<Time<Real>>, race: Option<Res<crate::Driving>>) {
    if !test.auto {
        test.running = false;
        return;
    }
    // In a race the cycle starts once the simulation runs, so loading and the
    // Race Options screen do not fill the first step, and it holds while paused.
    if race.is_some_and(|d| d.paused || d.runtime.ticks == 0) {
        return;
    }
    if test.running {
        test.timer += time.delta_secs();
        if test.timer < CYCLE_SECONDS {
            return;
        }
        test.step = (test.step + 1) % CYCLE.len();
    } else {
        test.running = true;
        test.step = 0;
    }
    test.timer = 0.;
    test.switches = CYCLE[test.step];
    log_line!(
        "perf_test version=1 event=step step={} of={} switches={}",
        test.step + 1,
        CYCLE.len(),
        test.switches.label()
    );
}

type Hidden = HashMap<Entity, Visibility>;

/// What the mobile-cost switches changed, to restore it exactly.
#[derive(Default)]
struct Restore {
    /// Entities given `NotShadowCaster` here.
    no_casters: HashSet<Entity>,
    /// Entities given a `VisibilityRange` here.
    ranged: HashSet<Entity>,
    /// The render height before `render_rows`.
    height: Option<crate::render_scale::RenderHeight>,
    /// The 3D camera's MSAA before `msaa_off` or `msaa2`.
    msaa: Option<Msaa>,
    /// Cameras given `NoIndirectDrawing` here.
    no_indirect: HashSet<Entity>,
    /// The environment maps `lighting` or `env_maps` removed from the 3D cameras.
    environment: HashMap<Entity, (Option<EnvironmentMapLight>, Option<GeneratedEnvironmentMapLight>)>,
    /// Shadow map size and cascades before `light_shadows`.
    shadows: Option<(usize, Vec<(Entity, CascadeShadowConfig)>)>,
    /// Shadow filters `hard_shadows` replaced, per camera.
    filters: HashMap<Entity, Option<ShadowFilteringMethod>>,
}

/// The view settings the mobile-cost switches change.
#[derive(bevy::ecs::system::SystemParam)]
#[allow(clippy::type_complexity)]
struct View<'w, 's> {
    scale: Option<ResMut<'w, crate::render_scale::RenderScale>>,
    shadow_map: Option<ResMut<'w, DirectionalLightShadowMap>>,
    cascades: Query<'w, 's, (Entity, &'static mut CascadeShadowConfig)>,
    cameras: Query<
        'w,
        's,
        (
            Entity,
            &'static mut Msaa,
            Has<NoIndirectDrawing>,
            Option<&'static EnvironmentMapLight>,
            Option<&'static GeneratedEnvironmentMapLight>,
            Option<&'static ShadowFilteringMethod>,
        ),
        With<crate::ViewerCamera>,
    >,
    /// Point and spot lights, for `local_lights`.
    local: Query<'w, 's, Entity, Or<(With<PointLight>, With<SpotLight>)>>,
    /// Every light, for `lighting`.
    lights: Query<'w, 's, Entity, Or<(With<DirectionalLight>, With<PointLight>, With<SpotLight>)>>,
    /// The cars' reflection overlays, for `reflections`.
    overlays: Query<'w, 's, Entity, With<crate::car_reflection_overlay::OverlayOf>>,
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn apply(
    mut test: ResMut<PerfTest>,
    mut hidden: Local<Hidden>,
    mut commands: Commands,
    mut restore: Local<Restore>,
    all_meshes: Query<
        (
            Entity,
            Option<&Aabb>,
            &GlobalTransform,
            Has<crate::MovingPart>,
            Has<NotShadowCaster>,
            Has<VisibilityRange>,
        ),
        (With<Mesh3d>, Without<RenderLayers>),
    >,
    mut view: View,
    osd: Option<ResMut<crate::osd_scene::Osd>>,
    mut visible: Query<&mut Visibility>,
    foliage: Query<Entity, With<crate::source_trees::Facing>>,
    named: Query<(Entity, &Name), With<Mesh3d>>,
    meshes: Query<Entity, (With<Mesh3d>, Without<crate::MovingPart>)>,
    hud: Query<Entity, Or<(With<crate::race_hud::Part>, With<crate::damage_hud::DamagePanel>)>>,
    smoke: Option<Res<crate::tyre_smoke::TyreSmoke>>,
    handles: Query<(Entity, &Mesh3d)>,
) {
    let s = test.active();
    test.frames += 1;
    // New entities (a lost wheel, late assets) follow the switches too.
    if test.applied == Some(s) && !test.frames.is_multiple_of(60) {
        return;
    }
    let first = test.applied != Some(s);
    test.applied = Some(s);
    // Shadows: every mesh (or every small static one) stops casting; the
    // lights keep their shadow maps (toggling `shadow_maps_enabled` at run
    // time panics in Bevy 0.19.1's cascade lookup). Far props get a
    // `VisibilityRange`. Only what was added here is removed again.
    let r = &mut *restore;
    let mut small_count = 0;
    for (entity, aabb, transform, car, no_cast, ranged) in &all_meshes {
        let is_small = !car && aabb.is_some_and(|a| crate::mobile::small(a, transform));
        small_count += usize::from(is_small);
        let want_no_cast = s.shadows || (s.small_casters && is_small);
        if want_no_cast && !no_cast {
            commands.entity(entity).insert(NotShadowCaster);
            r.no_casters.insert(entity);
        } else if !want_no_cast && r.no_casters.remove(&entity) {
            commands.entity(entity).remove::<NotShadowCaster>();
        }
        let want_range = s.far_props && is_small;
        if want_range && !ranged {
            commands
                .entity(entity)
                .insert(crate::mobile::far_range(crate::mobile::FAR_PROPS_METRES));
            r.ranged.insert(entity);
        } else if !want_range && r.ranged.remove(&entity) {
            commands.entity(entity).remove::<VisibilityRange>();
        }
    }
    let want_msaa = if s.msaa_off {
        Some(Msaa::Off)
    } else if s.msaa2 {
        Some(Msaa::Sample2)
    } else {
        None
    };
    for (entity, mut m, indirect_off, environment, generated, filter) in &mut view.cameras {
        if s.hard_shadows && filter != Some(&ShadowFilteringMethod::Hardware2x2) {
            r.filters.entry(entity).or_insert(filter.copied());
            commands.entity(entity).insert(ShadowFilteringMethod::Hardware2x2);
        } else if !s.hard_shadows
            && let Some(before) = r.filters.remove(&entity)
        {
            match before {
                Some(f) => commands.entity(entity).insert(f),
                None => commands.entity(entity).remove::<ShadowFilteringMethod>(),
            };
        }
        match want_msaa {
            Some(want) => {
                r.msaa.get_or_insert(*m);
                if *m != want {
                    *m = want;
                }
            }
            None => {
                if let Some(before) = r.msaa
                    && *m != before
                {
                    *m = before;
                }
            }
        }
        if s.no_indirect && !indirect_off {
            commands.entity(entity).insert(NoIndirectDrawing);
            r.no_indirect.insert(entity);
        } else if !s.no_indirect && r.no_indirect.remove(&entity) {
            commands.entity(entity).remove::<NoIndirectDrawing>();
        }
        let no_maps = s.lighting || s.env_maps;
        if no_maps && (environment.is_some() || generated.is_some()) {
            // A map added again later (a sky change) is removed again on the
            // next pass; the first one taken is what comes back.
            r.environment.entry(entity).or_insert((environment.cloned(), generated.cloned()));
            commands
                .entity(entity)
                .remove::<(EnvironmentMapLight, GeneratedEnvironmentMapLight)>();
        } else if !no_maps
            && let Some((environment, generated)) = r.environment.remove(&entity)
        {
            let mut e = commands.entity(entity);
            if let Some(map) = environment {
                e.insert(map);
            }
            if let Some(map) = generated {
                e.insert(map);
            }
        }
    }
    if want_msaa.is_none() {
        r.msaa = None;
    }
    if let Some(scale) = view.scale.as_deref_mut() {
        // The steps compare fixed heights: the dynamic height waits.
        scale.hold = s.render_rows > 0;
        if s.render_rows > 0 {
            r.height.get_or_insert(scale.height);
            scale.set_height(crate::render_scale::RenderHeight::Rows(u32::from(s.render_rows)));
        } else if let Some(height) = r.height.take() {
            scale.set_height(height);
        }
    }
    let cut = s.light_shadows || s.tiny_shadows;
    let (cut_size, cut_first, cut_far) = if s.tiny_shadows { (256, 4., 25.) } else { (1024, 6., 60.) };
    if cut && r.shadows.is_none() {
        let size = view.shadow_map.as_ref().map_or(2048, |m| m.size);
        let saved = view.cascades.iter().map(|(e, c)| (e, c.clone())).collect();
        r.shadows = Some((size, saved));
        if let Some(map) = view.shadow_map.as_deref_mut() {
            map.size = cut_size;
        } else {
            commands.insert_resource(DirectionalLightShadowMap { size: cut_size });
        }
        for (_, mut config) in &mut view.cascades {
            // Same cascade count (changing it at run time is not safe in
            // Bevy 0.19.1), shorter reach.
            *config = CascadeShadowConfigBuilder {
                num_cascades: config.bounds.len().max(1),
                first_cascade_far_bound: cut_first,
                maximum_distance: cut_far,
                ..default()
            }
            .build();
        }
    } else if !cut
        && let Some((size, saved)) = r.shadows.take()
    {
        if let Some(map) = view.shadow_map.as_deref_mut() {
            map.size = size;
        }
        for (entity, config) in saved {
            if let Ok((_, mut c)) = view.cascades.get_mut(entity) {
                *c = config;
            }
        }
    }
    if first {
        *LABEL.lock().unwrap_or_else(|e| e.into_inner()) = Some(s.label());
        QUIET_SAMPLES.store(s.samples, Ordering::Relaxed);
        log_line!(
            "perf_test version=1 event=apply switches={} meshes={} small_static={small_count}",
            s.label(),
            all_meshes.iter().len()
        );
    }
    if let Some(mut o) = osd {
        o.set_active(!s.hud);
    }
    // The entities each switch hides.
    let mut want: Vec<Entity> = Vec::new();
    if s.empty {
        want.extend(meshes.iter());
    }
    if s.foliage {
        want.extend(foliage.iter());
    }
    if s.ground || s.effects {
        for (entity, name) in &named {
            let n = name.as_str();
            if (s.ground && n == "ground cover")
                || (s.effects && (n.starts_with("wheel debris") || n == "skid marks"))
            {
                want.push(entity);
            }
        }
    }
    if s.effects
        && let Some(smoke) = &smoke
    {
        want.extend(
            handles
                .iter()
                .filter(|(_, m)| smoke.mesh_handles().contains(&m.0))
                .map(|(e, _)| e),
        );
    }
    if s.hud {
        want.extend(hud.iter());
    }
    if s.lighting {
        want.extend(view.lights.iter());
    }
    if s.local_lights {
        want.extend(view.local.iter());
    }
    if s.reflections {
        want.extend(view.overlays.iter());
    }
    // Restore what no switch hides any more, then hide the rest.
    let keep: std::collections::HashSet<Entity> = want.iter().copied().collect();
    hidden.retain(|entity, original| {
        if keep.contains(entity) {
            return true;
        }
        if let Ok(mut v) = visible.get_mut(*entity) {
            *v = *original;
        }
        false
    });
    for entity in want {
        if let Ok(mut v) = visible.get_mut(entity) {
            hidden.entry(entity).or_insert(*v);
            *v = Visibility::Hidden;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_name_the_parts_turned_off() {
        assert_eq!(Switches::default().label(), "normal");
        assert_eq!(
            Switches { shadows: true, hud: true, ..Default::default() }.label(),
            "off:shadows+hud"
        );
        assert_eq!(CYCLE[0], Switches::default());
        assert_eq!(CYCLE[CYCLE.len() - 1], Switches::default());
        assert_eq!(
            Switches { main_single: true, render_single: true, ..Default::default() }.label(),
            "mobile:main_single+render_single"
        );
    }

    #[derive(Resource, Default)]
    struct Count(u32);

    fn counter(mut n: ResMut<Count>) {
        n.0 += 1;
    }

    /// The switch swaps the executors of the main-world schedules and back,
    /// and the systems keep running under either.
    #[test]
    fn main_single_swaps_executors_and_systems_keep_running() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Count>()
            .add_systems(Update, counter)
            .add_systems(PostUpdate, counter)
            .init_resource::<PerfTest>()
            .add_systems(Last, main_executors);
        app.update();
        assert_eq!(app.world().resource::<Count>().0, 2);
        app.world_mut().resource_mut::<PerfTest>().switches.main_single = true;
        app.update();
        app.update();
        assert_eq!(app.world().resource::<Count>().0, 6);
        app.world_mut().resource_mut::<PerfTest>().switches.main_single = false;
        app.update();
        app.update();
        assert_eq!(app.world().resource::<Count>().0, 10);
        let mut schedules = Schedules::default();
        schedules.insert(Schedule::new(Update));
        assert_eq!(set_executors(&mut schedules, &main_schedules(), true), 1);
        assert_eq!(set_executors(&mut schedules, &render_schedules(), true), 0);
    }

    /// The GPU switches change the 3D camera, the lights and the reflection
    /// overlays, and everything comes back exactly when they are off.
    #[test]
    fn gpu_switches_change_the_view_and_restore_it() {
        use crate::car_reflection_overlay::OverlayOf;
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<PerfTest>()
            .add_systems(Update, apply);
        let camera = app
            .world_mut()
            .spawn((
                crate::ViewerCamera,
                Msaa::Sample4,
                EnvironmentMapLight { intensity: 7., ..default() },
            ))
            .id();
        let sun = app.world_mut().spawn((DirectionalLight::default(), Visibility::Visible)).id();
        let lamp = app.world_mut().spawn((PointLight::default(), Visibility::Inherited)).id();
        let overlay = app.world_mut().spawn((OverlayOf, Visibility::Inherited)).id();
        app.update();
        let switches = |app: &mut App, s: Switches| {
            app.world_mut().resource_mut::<PerfTest>().switches = s;
            app.update();
        };
        let all = Switches {
            msaa2: true,
            no_indirect: true,
            reflections: true,
            lighting: true,
            ..Switches::NORMAL
        };
        assert_eq!(all.label(), "off:reflections+lighting,mobile:msaa2+no_indirect");
        switches(&mut app, all);
        let w = app.world();
        assert_eq!(*w.get::<Msaa>(camera).unwrap(), Msaa::Sample2);
        assert!(w.get::<NoIndirectDrawing>(camera).is_some());
        assert!(w.get::<EnvironmentMapLight>(camera).is_none());
        for e in [sun, lamp, overlay] {
            assert_eq!(*w.get::<Visibility>(e).unwrap(), Visibility::Hidden);
        }
        // `msaa_off` wins over `msaa2`.
        switches(&mut app, Switches { msaa_off: true, ..all });
        assert_eq!(*app.world().get::<Msaa>(camera).unwrap(), Msaa::Off);
        switches(&mut app, Switches::NORMAL);
        let w = app.world();
        assert_eq!(*w.get::<Msaa>(camera).unwrap(), Msaa::Sample4);
        assert!(w.get::<NoIndirectDrawing>(camera).is_none());
        assert_eq!(w.get::<EnvironmentMapLight>(camera).unwrap().intensity, 7.);
        assert_eq!(*w.get::<Visibility>(sun).unwrap(), Visibility::Visible);
        assert_eq!(*w.get::<Visibility>(lamp).unwrap(), Visibility::Inherited);
        assert_eq!(*w.get::<Visibility>(overlay).unwrap(), Visibility::Inherited);
        // `hard_shadows` and `local_lights`: the 2x2 filter and the point light go, the sun stays.
        switches(&mut app, Switches { hard_shadows: true, local_lights: true, ..Switches::NO_REFLECTIONS });
        let w = app.world();
        assert!(w.get::<EnvironmentMapLight>(camera).is_none());
        assert_eq!(*w.get::<Visibility>(overlay).unwrap(), Visibility::Hidden);
        assert_eq!(*w.get::<ShadowFilteringMethod>(camera).unwrap(), ShadowFilteringMethod::Hardware2x2);
        assert_eq!(*w.get::<Visibility>(lamp).unwrap(), Visibility::Hidden);
        assert_eq!(*w.get::<Visibility>(sun).unwrap(), Visibility::Visible);
        switches(&mut app, Switches::NORMAL);
        let w = app.world();
        assert!(w.get::<ShadowFilteringMethod>(camera).is_none());
        assert_eq!(w.get::<EnvironmentMapLight>(camera).unwrap().intensity, 7.);
        assert_eq!(*w.get::<Visibility>(lamp).unwrap(), Visibility::Inherited);
    }

    /// `no_reflections` turns off every cube draw and sample, and nothing else.
    #[test]
    fn no_reflections_takes_the_maps_and_keeps_the_lights() {
        let mut s = Switches::NORMAL;
        s.add(Switches::NO_REFLECTIONS);
        assert!(s.reflections && s.env_maps && !s.lighting && !s.local_lights);
        assert_eq!(s.label(), "off:reflections+env_maps");
    }

    /// The mobile preset starts from `lean`, the desktop from normal; the
    /// words add to the base, and `full` drops it.
    #[test]
    fn mobile_base_is_lean_and_words_add_to_it() {
        assert_eq!(base_for(false), Switches::NORMAL);
        assert_eq!(base_for(true), Switches::LEAN);
        let mut test = PerfTest { base: base_for(true), ..default() };
        test.switches.hud = true;
        test.switches.render_rows = 360;
        let s = test.active();
        assert!(s.hud && s.msaa_off && s.tiny_shadows && s.reflections);
        assert_eq!(s.render_rows, 360);
        test.base = Switches::NORMAL;
        assert_eq!(test.active(), Switches { hud: true, render_rows: 360, ..Switches::NORMAL });
        assert_eq!(PerfTest::default().active(), Switches::NORMAL);
    }

    /// `lean` adds every cheaper path to the words before and after it.
    #[test]
    fn lean_adds_to_the_other_switches() {
        let mut s = Switches { hud: true, render_rows: 720, ..Switches::NORMAL };
        s.add(Switches::LEAN);
        assert!(s.hud && s.msaa_off && s.tiny_shadows && s.hard_shadows && s.local_lights);
        assert!(s.small_casters && s.far_props && s.reflections && !s.lighting);
        assert_eq!(s.render_rows, 720);
        assert_eq!(
            Switches::LEAN.label(),
            "off:reflections+local_lights,mobile:light_shadows+no_small_casters+far_props+msaa_off+hard_shadows+tiny_shadows"
        );
    }
}
