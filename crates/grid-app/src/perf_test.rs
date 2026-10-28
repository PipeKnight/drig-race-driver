//! Performance test (AUTHORED diagnostics, P12, docs/PERFORMANCE.md "Phone
//! bisection"): presentation switches applied live from the front end's
//! Performance test screen, so one phone run can show which part of the frame
//! costs what. Every `frame_stats` line carries the active switches
//! (`perf_test=...`); with Auto cycle the switches step through [`CYCLE`]
//! every [`CYCLE_SECONDS`], so a single run gives a whole table.
//!
//! Only visibility, shadow casting (`NotShadowCaster`), a camera and log lines change, never the
//! simulation, and nothing is saved: every start is the normal game.
use bevy::camera::primitives::Aabb;
use bevy::camera::visibility::{RenderLayers, VisibilityRange};
use bevy::light::{CascadeShadowConfig, CascadeShadowConfigBuilder, DirectionalLightShadowMap, NotShadowCaster};
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
}

impl Switches {
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

/// Auto cycle order: the normal frame, each part alone, everything, the car
/// alone, then the resolution and anti-aliasing variants (`mobile:`). With
/// the mobile preset the normal frame is the preset's, so `render1080` is
/// the phone's full resolution.
pub const CYCLE: [Switches; 15] = {
    const N: Switches = Switches::NORMAL;
    [
        N,
        Switches { shadows: true, ..N },
        Switches { foliage: true, ..N },
        Switches { ground: true, ..N },
        Switches { effects: true, ..N },
        Switches { hud: true, ..N },
        Switches { samples: true, ..N },
        Switches { mirror: true, ..N },
        Switches::ALL_OFF,
        Switches { empty: true, ..Switches::ALL_OFF },
        N,
        Switches { render_rows: 1080, ..N },
        Switches { render_rows: 540, ..N },
        Switches { msaa_off: true, ..N },
        N,
    ]
};
pub const CYCLE_SECONDS: f32 = 8.;

/// Live state, changed by the front end's Performance test screen.
#[derive(Resource, Debug, Default)]
pub struct PerfTest {
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

/// ` perf_test=...` for `frame_stats`, empty when the screen was never used.
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
    let mut test = PerfTest::default();
    // Desktop diagnostics: GRID_PERF_TEST=auto, or the parts to turn off,
    // e.g. GRID_PERF_TEST=shadows,foliage (names as in the label).
    if let Ok(value) = std::env::var("GRID_PERF_TEST") {
        for part in value.split(',').map(str::trim) {
            let s = &mut test.switches;
            match part {
                "auto" => test.auto = true,
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
                rows if rows.starts_with("render") => {
                    s.render_rows = rows[6..].parse().unwrap_or(0);
                }
                _ => warn!("GRID_PERF_TEST: unknown part {part:?}"),
            }
        }
    }
    app.insert_resource(test)
        .add_systems(Last, (cycle, apply).chain())
        .add_systems(Last, churn);
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

fn cycle(mut test: ResMut<PerfTest>, time: Res<Time<Real>>) {
    if !test.auto {
        test.running = false;
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
    /// The 3D camera's MSAA before `msaa_off`.
    msaa: Option<Msaa>,
    /// Shadow map size and cascades before `light_shadows`.
    shadows: Option<(usize, Vec<(Entity, CascadeShadowConfig)>)>,
}

/// The view settings the mobile-cost switches change.
#[derive(bevy::ecs::system::SystemParam)]
struct View<'w, 's> {
    scale: Option<ResMut<'w, crate::render_scale::RenderScale>>,
    shadow_map: Option<ResMut<'w, DirectionalLightShadowMap>>,
    cascades: Query<'w, 's, (Entity, &'static mut CascadeShadowConfig)>,
    msaa: Query<'w, 's, &'static mut Msaa, With<crate::ViewerCamera>>,
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
    mut osd: Query<&mut Camera, With<crate::osd_scene::OsdCamera>>,
    mut visible: Query<&mut Visibility>,
    foliage: Query<Entity, With<crate::source_trees::Facing>>,
    named: Query<(Entity, &Name), With<Mesh3d>>,
    meshes: Query<Entity, (With<Mesh3d>, Without<crate::MovingPart>)>,
    hud: Query<Entity, Or<(With<crate::race_hud::Part>, With<crate::damage_hud::DamagePanel>)>>,
    smoke: Option<Res<crate::tyre_smoke::TyreSmoke>>,
    handles: Query<(Entity, &Mesh3d)>,
) {
    let s = test.switches;
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
    for mut m in &mut view.msaa {
        if s.msaa_off {
            r.msaa.get_or_insert(*m);
            if *m != Msaa::Off {
                *m = Msaa::Off;
            }
        } else if let Some(before) = r.msaa
            && *m != before
        {
            *m = before;
        }
    }
    if !s.msaa_off {
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
    if s.light_shadows && r.shadows.is_none() {
        let size = view.shadow_map.as_ref().map_or(2048, |m| m.size);
        let saved = view.cascades.iter().map(|(e, c)| (e, c.clone())).collect();
        r.shadows = Some((size, saved));
        if let Some(map) = view.shadow_map.as_deref_mut() {
            map.size = 1024;
        } else {
            commands.insert_resource(DirectionalLightShadowMap { size: 1024 });
        }
        for (_, mut config) in &mut view.cascades {
            // Same cascade count (changing it at run time is not safe in
            // Bevy 0.19.1), shorter reach.
            *config = CascadeShadowConfigBuilder {
                num_cascades: config.bounds.len().max(1),
                first_cascade_far_bound: 6.,
                maximum_distance: 60.,
                ..default()
            }
            .build();
        }
    } else if !s.light_shadows
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
    for mut camera in &mut osd {
        camera.is_active = !s.hud;
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
    }
}
