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
//! Level entities are not merged by `static_batch`: a merged mesh would be
//! rebuilt whenever one of its instances changes level. Common build;
//! `GRID_ORNAMENT_LOD=0` keeps every ornament at level 0 (comparisons).
use bevy::{
    camera::visibility::{RenderLayers, VisibilitySystems},
    prelude::*,
    transform::TransformSystems,
};
use std::collections::HashMap;

/// Reference FOV of the distances: 55 degrees (0x979430, tan(0.47996554)).
const REFERENCE_HALF_FOV: f32 = 0.479_965_54;
/// Overlap of a scenery level into the next, metres (0x95D2A0).
const SCENERY_OVERLAP: f32 = 20.;
/// `lod_distance_00..02` of a prototype without a record (0x9522F0).
pub const DEFAULT_DISTANCES: [f32; 3] = [30., 60., 100.];

pub fn enabled() -> bool {
    std::env::var("GRID_ORNAMENT_LOD").map_or(true, |v| v != "0")
}

/// One prototype: `lod_distance_00..02`, its level count and whether it is
/// in the `scenery` category.
#[derive(Clone, Debug)]
pub struct Type {
    pub distances: [f32; 3],
    pub levels: u8,
    pub scenery: bool,
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
}

const NONE: u8 = u8::MAX;

#[derive(Component)]
pub struct OrnamentLevel {
    pub instance: u32,
    pub level: u8,
}

impl Levels {
    /// The level that casts the type's shadow: level 1 when it has one.
    pub fn shadow_level(&self, instance: u32) -> u8 {
        shadow_level(&self.types[self.instances[instance as usize].kind as usize])
    }
}

fn shadow_level(kind: &Type) -> u8 {
    u8::from(kind.levels >= 2)
}

pub fn install(app: &mut App) {
    app.add_systems(
        PostUpdate,
        select
            .run_if(resource_exists::<Lods>)
            .after(TransformSystems::Propagate)
            .before(VisibilitySystems::VisibilityPropagate),
    );
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

/// Level `i`'s range in `v` (0x95D2A0): drawn while `r0 < v <= r1`; `r2`
/// is the cross-fade width above `r0`.
pub fn range(kind: &Type, i: u8) -> [f32; 3] {
    let n = kind.levels.max(1);
    let [d0, d1, far] = kind.distances;
    let d = [d0, d1];
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
fn select(
    mut lods: ResMut<Lods>,
    camera: Query<(&GlobalTransform, &Projection), With<crate::ViewerCamera>>,
    mut parts: Query<(&OrnamentLevel, &mut Visibility, Option<&mut RenderLayers>)>,
) {
    let Some((eye, Projection::Perspective(lens))) = camera.iter().next() else {
        return;
    };
    let eye = eye.translation();
    let k = REFERENCE_HALF_FOV.tan() / (lens.fov * 0.5).tan();
    let lods = &mut *lods;
    for (instance, shown) in lods.instances.iter().zip(&mut lods.shown) {
        let kind = &lods.types[instance.kind as usize];
        let v = metric(instance.min, instance.max, eye, k, kind.distances[2]);
        *shown = level(kind, v);
    }
    let caster = RenderLayers::layer(0);
    let shadow_only = crate::car_batch::shadow_layer();
    for (part, mut visibility, layers) in &mut parts {
        let shown = lods.shown[part.instance as usize];
        let colour = shown == part.level;
        match layers {
            // The shadow level: drawn inside D, by the camera only while it
            // is the colour level.
            Some(mut layers) => {
                visibility.set_if_neq(if shown == NONE {
                    Visibility::Hidden
                } else {
                    Visibility::Inherited
                });
                let want = if colour { &caster } else { &shadow_only };
                if layers.clone().without(crate::car_reflections::LAYER) != *want {
                    *layers = want.clone();
                }
            }
            None => {
                visibility.set_if_neq(if colour {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn building() -> Type {
        Type {
            distances: [100., 300., 700.],
            levels: 3,
            scenery: true,
        }
    }

    fn at(kind: &Type, metres: f32) -> u8 {
        let eye = Vec3::new(-metres, 0., 0.);
        level(kind, metric(Vec3::ZERO, Vec3::ONE, eye, 1., kind.distances[2]))
    }

    #[test]
    fn levels_follow_the_attribute_distances() {
        let plain = Type {
            scenery: false,
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
        };
        assert_eq!(range(&kind, 1), [0., 0.75, 0.]);
        assert_eq!(shadow_level(&kind), 1);
        let single = Type { levels: 1, ..kind };
        assert_eq!(range(&single, 0), [0., 1., 0.]);
        assert_eq!(shadow_level(&single), 0);
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
}
