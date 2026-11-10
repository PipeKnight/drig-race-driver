//! The team-mate's name tag over the car (docs/RACE_HUD.md "Driver name tags"): the OSD's
//! `driver_id_team` group, placed over the team-mate's car, with the race position, the name
//! and the nationality flag, fading out towards 60 m [D: update 0x6BF070, team path 0x6BD0B0,
//! limits 0x6A80A0, anchor 0x6B0820].
//!
//! The rival's tag (`driver_id` tags 0..6, career lookup 0x6BCE60) is not ported: a race does
//! not know the career rival yet.

use crate::osd_scene::{Osd, OsdCamera};
use bevy::prelude::*;

/// The team-mate's tag group in `Osd/dial2.pssg`.
pub const TEAM_TAG: &str = "object/driver_id_team=driver_id_04";
/// The tag's parts whose alpha the distance fades (0x6A80A0 sets alpha on seven parameter
/// tables at +0x120).
const PARTS: [&str; 7] = ["name_pill_cap", "flag_pill", "number_pill", "name_pill", "flag", "number", "name"];
/// Beyond this distance the tag is hidden, and its alpha reaches the minimum (0xDB4F60, 0xDB4F5C).
pub const RANGE: f32 = 60.;
/// The fade's exponent (0xDB4F68).
const FADE_POWER: f32 = 3.;
/// The anchor's lift over the car: 1.5 m plus 0.04 m per metre of distance (0x6B0820).
const LIFT: f32 = 1.5;
const LIFT_PER_METRE: f32 = 0.04;

/// The team-mate of a career race: car `car` of the race (1.. opponents), with the nationality
/// flag's atlas index (`nationality.flag_id`).
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct TeamMate {
    pub car: usize,
    pub flag: Option<i32>,
}

impl TeamMate {
    /// The flag of driver `driver` (`driver.id`): `driver.nationality_id` → `nationality.flag_id`
    /// (0x69C860).
    pub fn flag(db: &grid_formats::database::Database, driver: i32) -> Option<i32> {
        let nationality = db.table("driver")?.rows().find(|r| r.int("id") == Some(driver))?.int("nationality_id")?;
        db.table("nationality")?.rows().find(|r| r.int("id") == Some(nationality))?.int("flag_id")
    }

    /// The team-mate `car` driven by `driver`, its flag from the installation at `root`.
    pub fn load(root: &std::path::Path, car: usize, driver: Option<i32>) -> Self {
        let flag = driver.and_then(|d| {
            let db = grid_tools::read_database(root).ok()?;
            Self::flag(&db, d)
        });
        Self { car, flag }
    }
}

/// The tag's alpha factor at distance `d` (0x6A80A0): base − min(|d|/60, 1)³ × (base − min),
/// with the minimum 0 ([0xE6DFF4] is never written [I]).
pub fn fade(d: f32) -> f32 {
    1. - (d.abs() / RANGE).min(1.).powf(FADE_POWER)
}

/// The anchor's height over the car at distance `d` (0x6B0820).
pub fn lift(d: f32) -> f32 {
    LIFT + LIFT_PER_METRE * d.abs()
}

/// The flag atlas cell of `flag_id` as (scrollX, scrollY) (0x6A0820): nine rows of seven
/// columns; −1 or a cell outside the atlas shows no flag.
pub fn flag_scroll(flag_id: i32) -> Option<(f32, f32)> {
    if flag_id < 0 {
        return None;
    }
    let (x, y) = ((flag_id / 9) as f32 / 7., (flag_id % 9) as f32 / 9.);
    ((0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y)).then_some((x, y))
}

/// Signed distance along the racing line from arc `from` to arc `to` (a closed line of
/// `length` wraps to the shorter way round), as the team path's 0x90BDB0.
pub fn along(from: f32, to: f32, length: f32, open: bool) -> f32 {
    let d = to - from;
    if open || length <= 0. { d } else { (d + length / 2.).rem_euclid(length) - length / 2. }
}

/// The local (x, y) in `parent`'s space, at local depth `z`, of the point where the ray
/// `origin + t·dir` (t > 0) meets the plane z = `z`.
pub fn on_ray(parent: bevy::math::Affine3A, z: f32, origin: Vec3, dir: Vec3) -> Option<Vec2> {
    let m = parent.matrix3;
    let p0 = Vec3::from(parent.translation) + Vec3::from(m.z_axis) * z;
    let a = Mat3::from_cols(m.x_axis.into(), m.y_axis.into(), -dir);
    if a.determinant().abs() < 1e-9 {
        return None;
    }
    let s = a.inverse() * (origin - p0);
    (s.z > 0.).then(|| Vec2::new(s.x, s.y))
}

/// The view ray of normalised device point `ndc` for a perspective camera at `camera` with a
/// vertical field of view `fov` and `aspect`.
pub fn ray(camera: &GlobalTransform, fov: f32, aspect: f32, ndc: Vec2) -> (Vec3, Vec3) {
    let h = (fov / 2.).tan();
    let local = Vec3::new(ndc.x * h * aspect, ndc.y * h, -1.);
    (camera.translation(), camera.rotation() * local)
}

/// The normalised device point of world point `p` seen by a camera at `view` (its transform)
/// with `clip_from_view`; none behind the camera (the original moves the tag off screen).
pub fn ndc(view: &Transform, clip_from_view: Mat4, p: Vec3) -> Option<Vec2> {
    let v = view.compute_affine().inverse().transform_point3(p);
    if v.z >= 0. {
        return None;
    }
    let c = clip_from_view * v.extend(1.);
    (c.w > 0.).then(|| Vec2::new(c.x, c.y) / c.w)
}

#[derive(Default)]
pub(crate) struct TagState {
    shown: bool,
    started: bool,
}

/// Places, fills and fades the team-mate's tag each frame.
#[allow(clippy::too_many_arguments)]
pub(crate) fn team_tag(
    driving: Res<crate::Driving>,
    mate: Option<Res<TeamMate>>,
    osd: Option<ResMut<Osd>>,
    viewer: Query<(&Transform, &Camera), With<crate::ViewerCamera>>,
    osd_camera: Query<(&GlobalTransform, &Projection), With<OsdCamera>>,
    globals: Query<&GlobalTransform>,
    mut transforms: Query<&mut Transform, Without<crate::ViewerCamera>>,
    mut state: Local<TagState>,
) {
    let Some(mut osd) = osd else {
        return;
    };
    let Some(root) = osd.entity(TEAM_TAG) else {
        return;
    };
    // The update's first frame hides every tag (0x6BF070).
    if !state.started {
        state.started = true;
        osd.show(TEAM_TAG, false);
    }
    let placed = mate.as_deref().and_then(|mate| {
        let race = driving.opponents.as_ref()?;
        let me = crate::race_hud::car_pose(&driving, 0)?;
        let it = crate::race_hud::car_pose(&driving, mate.car)?;
        let (me, it) = (Vec3::new(me[12], me[13], me[14]), Vec3::new(it[12], it[13], it[14]));
        let d = match &driving.reset_line {
            Some(line) => along(line.arc(me.to_array()), line.arc(it.to_array()), line.length, line.is_open()),
            None => me.distance(it),
        };
        if d.abs() > RANGE {
            return None;
        }
        let (view, camera) = viewer.single().ok()?;
        let at = ndc(view, camera.clip_from_view(), it + Vec3::Y * lift(d))?;
        let (osd_view, projection) = osd_camera.single().ok()?;
        let Projection::Perspective(p) = projection else {
            return None;
        };
        let (origin, dir) = ray(osd_view, p.fov, p.aspect_ratio, at);
        // The tag's parent space (the OSD layout), from the root's global and local transforms.
        let local = transforms.get(root).ok()?;
        let parent = globals.get(root).ok()?.affine() * local.compute_affine().inverse();
        let xy = on_ray(parent, local.translation.z, origin, dir)?;
        let position = race.standings.order().iter().position(|&c| c == mate.car).map_or(0, |i| i + 1);
        Some((xy, d, position, race.names.get(mate.car).cloned().unwrap_or_default()))
    });
    match placed {
        Some((xy, d, position, name)) => {
            if let Ok(mut t) = transforms.get_mut(root) {
                t.translation.x = xy.x;
                t.translation.y = xy.y;
            }
            osd.show(TEAM_TAG, true);
            let alpha = fade(d);
            for part in PARTS {
                if let Some(node) = osd.find(TEAM_TAG, part) {
                    osd.fade(&node, alpha);
                }
            }
            if let Some(node) = osd.find(TEAM_TAG, "number") {
                osd.set_text(&node, &position.to_string());
            }
            if let Some(node) = osd.find(TEAM_TAG, "name") {
                osd.set_text(&node, &name.chars().take(32).collect::<String>());
            }
            let flag = mate.as_deref().and_then(|m| m.flag).and_then(flag_scroll);
            if let Some(node) = osd.find(TEAM_TAG, "flag") {
                match flag {
                    Some((x, y)) => {
                        osd.show(&node, true);
                        osd.scroll_x(&node, x);
                        osd.scroll_y(&node, y);
                    }
                    None => osd.show(&node, false),
                }
            }
            osd.play_once(TEAM_TAG, "expand");
            if !state.shown {
                state.shown = true;
                println!(
                    "osd_tag version=1 event=shown car={} distance={d:.1} position={position} alpha={alpha:.3} at={:.2},{:.2}",
                    mate.as_deref().map_or(0, |m| m.car),
                    xy.x,
                    xy.y
                );
            }
        }
        None => {
            osd.show(TEAM_TAG, false);
            if state.shown {
                state.shown = false;
                println!("osd_tag version=1 event=hidden car={}", mate.as_deref().map_or(0, |m| m.car));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fade_is_full_at_the_car_and_gone_at_sixty_metres() {
        assert_eq!(fade(0.), 1.);
        assert!((fade(30.) - 0.875).abs() < 1e-6);
        assert!((fade(-30.) - 0.875).abs() < 1e-6);
        assert_eq!(fade(60.), 0.);
        assert_eq!(fade(90.), 0.);
    }

    #[test]
    fn anchor_rises_with_distance() {
        assert_eq!(lift(0.), 1.5);
        assert!((lift(-50.) - 3.5).abs() < 1e-6);
    }

    #[test]
    fn flag_cells_follow_the_atlas() {
        assert_eq!(flag_scroll(-1), None);
        assert_eq!(flag_scroll(0), Some((0., 0.)));
        let (x, y) = flag_scroll(20).unwrap();
        assert!((x - 2. / 7.).abs() < 1e-6 && (y - 2. / 9.).abs() < 1e-6);
        assert_eq!(flag_scroll(9 * 8), None);
    }

    #[test]
    fn along_track_distance_wraps_the_short_way() {
        assert_eq!(along(100., 130., 1000., false), 30.);
        assert_eq!(along(990., 10., 1000., false), 20.);
        assert_eq!(along(10., 990., 1000., false), -20.);
        assert_eq!(along(990., 10., 1000., true), -980.);
    }

    #[test]
    fn ray_meets_the_tag_plane() {
        // A parent moved and scaled: the solved point lies on the ray.
        let parent = bevy::math::Affine3A::from_scale_rotation_translation(Vec3::splat(2.), Quat::from_rotation_y(0.1), Vec3::new(1., 2., 3.));
        let (origin, dir) = (Vec3::new(0., 0.73, 3319.7), Vec3::new(0.1, -0.05, -1.));
        let xy = on_ray(parent, -20., origin, dir).unwrap();
        let p = parent.transform_point3(Vec3::new(xy.x, xy.y, -20.));
        let t = (p - origin).dot(dir) / dir.length_squared();
        assert!((origin + dir * t - p).length() < 1e-2, "{p} {t}");
        // Behind the ray's origin: none.
        assert!(on_ray(bevy::math::Affine3A::IDENTITY, 4000., origin, dir).is_none());
    }

    #[test]
    fn projection_round_trip_hits_the_centre_and_hides_behind() {
        let view = Transform::from_xyz(0., 2., 0.).looking_at(Vec3::new(0., 2., -10.), Vec3::Y);
        let clip = Mat4::perspective_infinite_reverse_rh(1., 16. / 9., 0.1);
        let c = ndc(&view, clip, Vec3::new(0., 2., -10.)).unwrap();
        assert!(c.length() < 1e-5);
        assert!(ndc(&view, clip, Vec3::new(0., 2., 10.)).is_none());
        // A ray through the NDC of a point goes through the point.
        let p = Vec3::new(3., 4., -20.);
        let n = ndc(&view, clip, p).unwrap();
        let (o, d) = ray(&GlobalTransform::from(view), 1., 16. / 9., n);
        let t = (p - o).dot(d) / d.length_squared();
        assert!((o + d * t - p).length() < 1e-3);
    }

    #[test]
    #[ignore = "needs GRID_GAME_ROOT"]
    fn every_driver_has_a_flag_cell() {
        let root = std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required"));
        let db = grid_tools::read_database(&root).unwrap();
        let ids: Vec<i32> = db.table("driver").unwrap().rows().filter_map(|r| r.int("id")).collect();
        let cells = ids.iter().filter(|&&d| TeamMate::flag(&db, d).and_then(flag_scroll).is_some()).count();
        assert!(cells * 10 >= ids.len() * 9, "{cells} of {} drivers have a flag cell", ids.len());
    }
}
