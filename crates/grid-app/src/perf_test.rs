//! Performance test (AUTHORED diagnostics, P12, docs/PERFORMANCE.md "Phone
//! bisection"): presentation switches applied live from the front end's
//! Performance test screen, so one phone run can show which part of the frame
//! costs what. Every `frame_stats` line carries the active switches
//! (`perf_test=...`); with Auto cycle the switches step through [`CYCLE`]
//! every [`CYCLE_SECONDS`], so a single run gives a whole table.
//!
//! Only visibility, shadow casting (`NotShadowCaster`), a camera and log lines change, never the
//! simulation, and nothing is saved: every start is the normal game.
use bevy::prelude::*;
use std::collections::HashMap;
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
}

impl Switches {
    pub const ALL_OFF: Self = Self {
        shadows: true,
        foliage: true,
        ground: true,
        effects: true,
        hud: true,
        samples: true,
        empty: false,
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
        ];
        let off: Vec<&str> = names.iter().filter(|(on, _)| *on).map(|(_, n)| *n).collect();
        if off.is_empty() {
            "normal".into()
        } else {
            format!("off:{}", off.join("+"))
        }
    }
}

/// Auto cycle order: the normal frame, each part alone, everything, the car alone.
pub const CYCLE: [Switches; 10] = {
    const N: Switches = Switches {
        shadows: false,
        foliage: false,
        ground: false,
        effects: false,
        hud: false,
        samples: false,
        empty: false,
    };
    [
        N,
        Switches { shadows: true, ..N },
        Switches { foliage: true, ..N },
        Switches { ground: true, ..N },
        Switches { effects: true, ..N },
        Switches { hud: true, ..N },
        Switches { samples: true, ..N },
        Switches::ALL_OFF,
        Switches { empty: true, ..Switches::ALL_OFF },
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
                _ => warn!("GRID_PERF_TEST: unknown part {part:?}"),
            }
        }
    }
    app.insert_resource(test)
        .add_systems(Last, (cycle, apply).chain());
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

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn apply(
    mut test: ResMut<PerfTest>,
    mut hidden: Local<Hidden>,
    mut commands: Commands,
    mut no_casters: Local<Vec<Entity>>,
    casters: Query<Entity, (With<Mesh3d>, Without<bevy::light::NotShadowCaster>)>,
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
    if first {
        *LABEL.lock().unwrap_or_else(|e| e.into_inner()) = Some(s.label());
        QUIET_SAMPLES.store(s.samples, Ordering::Relaxed);
        log_line!("perf_test version=1 event=apply switches={}", s.label());
    }
    // Shadows: every mesh stops casting (the lights keep their shadow maps:
    // toggling `shadow_maps_enabled` at run time panics in Bevy 0.19.1's
    // cascade lookup). Only the markers added here are removed again.
    if s.shadows {
        for entity in &casters {
            commands.entity(entity).insert(bevy::light::NotShadowCaster);
            no_casters.push(entity);
        }
    } else {
        for entity in no_casters.drain(..) {
            if let Ok(mut e) = commands.get_entity(entity) {
                e.remove::<bevy::light::NotShadowCaster>();
            }
        }
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
