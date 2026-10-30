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
//! bias. Casters without bounds stay. The picture is unchanged.
//!
//! `GRID_SHADOW_CULL=0` keeps Bevy's lists (comparisons).
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
fn cull(
    cameras: Query<(&GlobalTransform, &Projection)>,
    mut lights: Query<(
        &GlobalTransform,
        &CascadeShadowConfig,
        &Cascades,
        &mut CascadesVisibleEntities,
    )>,
    casters: Query<(&Aabb, &GlobalTransform)>,
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
            for (i, list) in lists.iter_mut().enumerate() {
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
                    let Ok((aabb, transform)) = casters.get(*entity) else {
                        return true;
                    };
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
}
