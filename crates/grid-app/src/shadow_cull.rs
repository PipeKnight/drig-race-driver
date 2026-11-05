//! Sun shadow casters culled per cascade by the receivers they can shade
//! (docs/PERFORMANCE.md "Sun shadow cost").
//!
//! Bevy fits cascade `i` to the camera slice from `(1 - overlap) * far(i-1)`
//! (the first from `minimum_distance`) to `far(i)` and samples it only for
//! fragments in that slice. It keeps a caster when its box meets the cascade's
//! light-space square, which is much larger than the slice: it also takes
//! casters beside and behind the camera. A caster whose shadow, swept along
//! the light direction, cannot reach the slice writes no texel the slice
//! samples, so it is dropped from that cascade's list. A margin of
//! [`MARGIN_TEXELS`] cascade texels covers the shadow filter and the normal
//! bias. Casters without bounds stay, and so do casters with
//! `NoFrustumCulling`, as in Bevy's own lists: the pipeline warm-up unculls
//! every mesh so each shadow variant is built behind its cover. Dropping them
//! here left those variants to be built the first time a tree or its fade
//! proxy entered a cascade mid-race, and that shadow was not drawn until the
//! pipeline compiled (row 115, F740). The picture is unchanged.
//!
//! `GRID_SHADOW_CULL=0` keeps Bevy's lists (comparisons).
//!
//! `GRID_SHADOW_TRACE=1` (research, row 115) prints `shadow_dropout` for each
//! caster that is in a cascade's final list in frames N-2 and N but not in
//! N-1 (a shadow missing for one frame), with Bevy's `FrameCount` of N-1,
//! why it was left out in N-1 (`cause`, see [`cause`]), its distance from
//! the camera and its box centre.
use bevy::camera::primitives::Aabb;
use bevy::camera::visibility::CascadesVisibleEntities;
use bevy::light::{CascadeShadowConfig, Cascades, SimulationLightSystems};
use bevy::math::Vec3A;
use bevy::prelude::*;

/// Texels of the cascade added around each slice plane.
const MARGIN_TEXELS: f32 = 10.;

pub fn enabled() -> bool {
    std::env::var("GRID_SHADOW_CULL").map_or(true, |v| v != "0")
}

pub fn install(app: &mut App) {
    if enabled() {
        app.add_systems(
            PostUpdate,
            cull.after(SimulationLightSystems::CheckLightVisibility),
        );
    }
    if std::env::var("GRID_SHADOW_TRACE").is_ok_and(|v| v == "1") {
        app.add_systems(
            PostUpdate,
            trace_dropouts
                .after(SimulationLightSystems::CheckLightVisibility)
                .after(cull),
        );
    }
}

/// Per light, view and cascade: the last frame's final list and why each
/// caster that left it in that frame was left out.
type Lists = std::collections::HashMap<(Entity, Entity, usize), (std::collections::HashSet<Entity>, std::collections::HashMap<Entity, String>)>;

#[allow(clippy::type_complexity)]
type Caster<'a> = (
    Option<&'a Name>,
    Has<Mesh3d>,
    Has<bevy::light::NotShadowCaster>,
    Option<&'a InheritedVisibility>,
    Option<&'a bevy::camera::visibility::RenderLayers>,
    Has<bevy::camera::visibility::VisibilityRange>,
    Option<Ref<'a, Aabb>>,
    Option<Ref<'a, GlobalTransform>>,
);

/// Why Bevy's `check_dir_light_mesh_visibility` (bevy_light 0.19.1) or this
/// module's `cull` left a caster out of a cascade's list, in the order Bevy
/// tests: `gone`, `not_caster`, `hidden`, `layers`, `range`, `frustum`, else
/// `port_cull`; with `+moved` / `+bounds` when its transform or box changed
/// in that frame.
#[allow(clippy::too_many_arguments)]
fn cause(
    entity: Entity,
    casters: &Query<Caster>,
    light_layers: &bevy::camera::visibility::RenderLayers,
    frustum: Option<&bevy::camera::primitives::Frustum>,
    ranges: Option<&bevy::camera::visibility::VisibleEntityRanges>,
    view: Entity,
) -> String {
    let Ok((_, mesh, not_caster, inherited, layers, ranged, aabb, transform)) = casters.get(entity) else {
        return "gone".into();
    };
    let mut why = if !mesh {
        "gone"
    } else if not_caster {
        "not_caster"
    } else if !inherited.is_some_and(|v| v.get()) {
        "hidden"
    } else if !light_layers.intersects(layers.unwrap_or_default()) {
        "layers"
    } else if ranged && ranges.is_some_and(|r| !r.entity_is_in_range_of_view(entity, view)) {
        "range"
    } else if let (Some(f), Some(a), Some(t)) = (frustum, aabb.as_deref(), transform.as_deref())
        && !f.intersects_obb(a, &t.affine(), false, true)
    {
        "frustum"
    } else {
        "port_cull"
    }
    .to_string();
    if transform.is_some_and(|t| t.is_changed()) {
        why += "+moved";
    }
    if aabb.is_some_and(|a| a.is_changed()) {
        why += "+bounds";
    }
    why
}

#[allow(clippy::type_complexity)]
fn trace_dropouts(
    lights: Query<(
        Entity,
        &CascadesVisibleEntities,
        &bevy::camera::primitives::CascadesFrusta,
        Option<&bevy::camera::visibility::RenderLayers>,
    )>,
    casters: Query<Caster>,
    cameras: Query<&GlobalTransform>,
    ranges: Option<Res<bevy::camera::visibility::VisibleEntityRanges>>,
    frame: Res<bevy::diagnostic::FrameCount>,
    mut last: Local<Lists>,
) {
    for (light, visible, frusta, light_layers) in &lights {
        let light_layers = light_layers.cloned().unwrap_or_default();
        for (view, lists) in &visible.entities {
            let eye = cameras.get(*view).map_or(Vec3::ZERO, |c| c.translation());
            for (i, list) in lists.iter().enumerate() {
                let now: std::collections::HashSet<Entity> = list.entities.iter().copied().collect();
                let (previous, left) = last.entry((light, *view, i)).or_default();
                let mut back: Vec<(&Entity, &String)> = left.iter().filter(|(e, _)| now.contains(e)).collect();
                back.sort();
                for (e, why) in back {
                    let (name, centre) = casters.get(*e).map_or((String::new(), Vec3::ZERO), |c| {
                        let centre = match (c.6.as_deref(), c.7.as_deref()) {
                            (Some(a), Some(t)) => t.transform_point(a.center.into()),
                            _ => Vec3::ZERO,
                        };
                        (c.0.map_or_else(String::new, |n| n.to_string()), centre)
                    });
                    println!(
                        "shadow_dropout frame={} cascade={i} cause={why} entity={e} name={name:?} distance={:.1} centre={:.1},{:.1},{:.1}",
                        frame.0.wrapping_sub(1),
                        centre.distance(eye),
                        centre.x,
                        centre.y,
                        centre.z,
                    );
                }
                let frustum = frusta.frusta.get(view).and_then(|f| f.get(i));
                *left = previous
                    .iter()
                    .filter(|e| !now.contains(e))
                    .map(|&e| (e, cause(e, &casters, &light_layers, frustum, ranges.as_deref(), *view)))
                    .collect();
                *previous = now;
            }
        }
    }
}

/// Inward half-space `n . p + w >= 0` of one slice face.
#[derive(Clone, Copy)]
struct Plane {
    normal: Vec3A,
    w: f32,
}

/// The six faces of a frustum slice from its corners in Bevy's order (near
/// plane bottom right, top right, top left, bottom left, then the far plane),
/// oriented so the slice's centre is inside.
fn slice_planes(corners: &[Vec3A; 8]) -> [Plane; 6] {
    let centre = corners.iter().copied().sum::<Vec3A>() / 8.;
    let face = |a: usize, b: usize, c: usize| {
        let normal = (corners[b] - corners[a])
            .cross(corners[c] - corners[a])
            .normalize_or_zero();
        let w = -normal.dot(corners[a]);
        if normal.dot(centre) + w < 0. {
            Plane { normal: -normal, w: -w }
        } else {
            Plane { normal, w }
        }
    };
    [
        face(0, 1, 2),
        face(4, 5, 6),
        face(0, 1, 5),
        face(1, 2, 6),
        face(2, 3, 7),
        face(3, 0, 4),
    ]
}

/// Whether a box (centre, half axes in world space) swept from where it is
/// along `light` can come within `margin` of the slice. A face whose normal
/// points along the light cannot separate: the sweep crosses it.
fn reaches(planes: &[Plane; 6], light: Vec3A, centre: Vec3A, axes: &[Vec3A; 3], margin: f32) -> bool {
    planes.iter().all(|p| {
        if p.normal.dot(light) > 0. {
            return true;
        }
        let radius: f32 = axes.iter().map(|a| p.normal.dot(*a).abs()).sum();
        p.normal.dot(centre) + p.w + radius >= -margin
    })
}

#[allow(clippy::type_complexity)]
pub(crate) fn cull(
    cameras: Query<(&GlobalTransform, &Projection)>,
    mut lights: Query<(
        &GlobalTransform,
        &CascadeShadowConfig,
        &Cascades,
        &mut CascadesVisibleEntities,
    )>,
    casters: Query<(&Aabb, &GlobalTransform, Has<bevy::camera::visibility::NoFrustumCulling>)>,
) {
    for (light_transform, config, cascades, mut visible) in &mut lights {
        let light = Vec3A::from(light_transform.forward().as_vec3());
        let overlap = 1. - config.overlap_proportion;
        for (view, lists) in visible.entities.iter_mut() {
            let (Ok((camera, projection)), Some(view_cascades)) =
                (cameras.get(*view), cascades.cascades.get(view))
            else {
                continue;
            };
            let world_from_view = camera.affine();
            // With the original's layout cascade 0 is the player car's map
            // (shadow_layout.rs), not a slice of the view.
            let first = usize::from(crate::shadow_layout::enabled());
            for (i, list) in lists.iter_mut().enumerate().skip(first) {
                let (Some(&far), Some(cascade)) = (config.bounds.get(i), view_cascades.get(i)) else {
                    continue;
                };
                let near = if i == 0 {
                    config.minimum_distance
                } else {
                    overlap * config.bounds[i - 1]
                };
                let corners = projection
                    .get_frustum_corners(-near, -far)
                    .map(|c| world_from_view.transform_point3a(c));
                let planes = slice_planes(&corners);
                let margin = MARGIN_TEXELS * cascade.texel_size;
                list.entities.retain(|entity| {
                    let Ok((aabb, transform, unculled)) = casters.get(*entity) else {
                        return true;
                    };
                    if unculled {
                        return true;
                    }
                    let affine = transform.affine();
                    let centre = affine.transform_point3a(aabb.center);
                    let m = affine.matrix3;
                    let axes = [
                        m.x_axis * aabb.half_extents.x,
                        m.y_axis * aabb.half_extents.y,
                        m.z_axis * aabb.half_extents.z,
                    ];
                    reaches(&planes, light, centre, &axes, margin)
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A unit cube slice from z=-1 to z=-3 (camera looking down -Z).
    fn slice() -> [Plane; 6] {
        let c = |x: f32, y: f32, z: f32| Vec3A::new(x, y, z);
        slice_planes(&[
            c(1., -1., -1.),
            c(1., 1., -1.),
            c(-1., 1., -1.),
            c(-1., -1., -1.),
            c(1., -1., -3.),
            c(1., 1., -3.),
            c(-1., 1., -3.),
            c(-1., -1., -3.),
        ])
    }

    const UNIT: [Vec3A; 3] = [
        Vec3A::new(0.1, 0., 0.),
        Vec3A::new(0., 0.1, 0.),
        Vec3A::new(0., 0., 0.1),
    ];

    #[test]
    fn inside_box_reaches() {
        let down = Vec3A::NEG_Y;
        assert!(reaches(&slice(), down, Vec3A::new(0., 0., -2.), &UNIT, 0.));
    }

    #[test]
    fn caster_above_the_slice_shades_it_below() {
        // Light straight down: a box 10 m above the slice's middle shades it.
        let down = Vec3A::NEG_Y;
        assert!(reaches(&slice(), down, Vec3A::new(0., 10., -2.), &UNIT, 0.));
        // Below the slice it cannot.
        assert!(!reaches(&slice(), down, Vec3A::new(0., -10., -2.), &UNIT, 0.));
    }

    #[test]
    fn caster_beside_or_behind_is_dropped_unless_within_the_margin() {
        let down = Vec3A::NEG_Y;
        // Beside the slice (x = 3) with the light straight down.
        assert!(!reaches(&slice(), down, Vec3A::new(3., 5., -2.), &UNIT, 0.));
        // Behind the camera (z = +2).
        assert!(!reaches(&slice(), down, Vec3A::new(0., 5., 2.), &UNIT, 0.));
        // Just outside x = 1 but inside the margin.
        assert!(reaches(&slice(), down, Vec3A::new(1.2, 5., -2.), &UNIT, 0.2));
    }

    #[test]
    fn slanted_light_reaches_from_the_side() {
        // Light travelling towards -x and down: a box at x = 3 high up shades
        // the slice; with the light the other way it does not.
        let slant = Vec3A::new(-1., -1., 0.).normalize();
        assert!(reaches(&slice(), slant, Vec3A::new(3., 3., -2.), &UNIT, 0.));
        assert!(!reaches(&slice(), -slant * Vec3A::new(1., -1., 1.), Vec3A::new(3., 3., -2.), &UNIT, 0.));
    }

    /// Row 115: a caster far outside every slice is dropped, the same caster
    /// with `NoFrustumCulling` (the pipeline warm-up) stays in every cascade.
    #[test]
    fn unculled_casters_stay_in_every_cascade() {
        use bevy::camera::visibility::{NoFrustumCulling, VisibleMeshEntities};
        use bevy::light::cascade::Cascade;
        let mut app = App::new();
        app.add_systems(Update, cull);
        let world = app.world_mut();
        let view = world
            .spawn((GlobalTransform::IDENTITY, Projection::Perspective(PerspectiveProjection::default())))
            .id();
        let far = GlobalTransform::from_translation(Vec3::new(0., 0., 5000.));
        let aabb = Aabb { center: Vec3A::ZERO, half_extents: Vec3A::ONE };
        let culled = world.spawn((aabb, far)).id();
        let unculled = world.spawn((aabb, far, NoFrustumCulling)).id();
        let config = CascadeShadowConfig::default();
        let n = config.bounds.len();
        let mut cascades = Cascades::default();
        cascades.cascades.insert(view, vec![Cascade { texel_size: 0.1, ..default() }; n]);
        let mut visible = CascadesVisibleEntities::default();
        visible.entities.insert(view, vec![VisibleMeshEntities { entities: vec![culled, unculled] }; n]);
        let light = world.spawn((GlobalTransform::from_rotation(Quat::from_rotation_x(-1.)), config, cascades, visible)).id();
        app.update();
        let lists = &app.world().get::<CascadesVisibleEntities>(light).unwrap().entities[&view];
        let first = usize::from(crate::shadow_layout::enabled());
        for list in &lists[first..] {
            assert_eq!(list.entities, vec![unculled]);
        }
    }
}
