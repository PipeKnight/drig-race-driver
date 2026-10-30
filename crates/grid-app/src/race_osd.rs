//! Drives the original OSD scene (`osd_scene`) from the HUD values
//! (`race_hud::HudView`): texts of the text nodes and the times of the
//! scene's animations (docs/RACE_HUD.md "Original look").
//!
//! [D] node and range names: `big_lap_number`, `pos_total`, `gear` ranges
//! `R N 0..9`, `light_revs` `on/off`, `mph_kmh` `mph/kmh`, `outer_face`
//! `8/10/12`, the position pills `normal/player/first`, the groups' `on`
//! slide-in, the layout's aspect ranges (`%1.2f` cut to three characters,
//! grid.exe 0x8D5CB0; no range for other aspects keeps the default layout).
//! [I] value to time: speed and odometer digits at digit/6 s (the atlas
//! steps of `speedo_N` and `odo_NN`), pedals and damage over `start` 0..1,
//! the needle over its 3.333 s sweep, the red zone over `redzone` `start`.
use crate::osd_scene::Osd;
use crate::race_hud::HudView;
use bevy::prelude::*;

const LAYOUT: &str = "implementation_osd/osd_layout=layout";
const GROUPS: [&str; 4] = [
    "implementation_osd/osd_left=left",
    "implementation_osd/osd_right=right",
    "implementation_osd/osd_top=top",
    "implementation_osd/osd_bottom=bottom",
];

/// Layout units per aspect unit: `anim_right` X is 0 at the 16:9 range and
/// -52.82 at the 16:10 range [D], 52.82 / (16/9 - 16/10).
pub const LAYOUT_SLOPE: f32 = 52.82 / (16. / 9. - 1.6);
/// Lowest saved scene y of the dial (damage panel) and the minimap [D bounds].
const DIAL_BOTTOM: f32 = -316.5;
const MINIMAP_BOTTOM: f32 = -307.1;
/// Scene y of 48 % screen height from the top (camera half-height 357.9):
/// above the touch zones, which start at half height (AUTHORED).
const TOUCH_BOTTOM: f32 = 357.9 * (1. - 2. * 0.48);

/// The layout range the original plays for a screen aspect (`%1.2f`, first
/// three characters): 1.78 -> "1.7".
pub fn aspect_range(aspect: f32) -> String {
    format!("{aspect:1.2}").chars().take(3).collect()
}

/// Digits of `value` right-aligned in `places` (leading zeros blank).
pub fn digits(value: u32, places: usize) -> Vec<Option<u32>> {
    let s = value.to_string();
    let s = if s.len() > places { "9".repeat(places) } else { s };
    let pad = places - s.len();
    (0..places)
        .map(|i| (i >= pad).then(|| s.as_bytes()[i - pad] as u32 - u32::from(b'0')))
        .collect()
}

/// Fraction of the `redzone` animation for a display range. `dial_redzone`
/// [D]: red marks the arc from 12 o'clock (half the range) to the end of the
/// 240 degree sweep, green the angle from 12 o'clock over 135 degrees; the
/// red zone shows from the red line on [I: the ramp grows linearly with the
/// animation, Ramp 0..1 over `start`].
pub fn red_zone_time(scale: f32) -> f32 {
    let from_top = (scale - crate::race_hud::RED_LINE) / scale * 240. - 120.;
    (from_top / 135.).clamp(0., 1.)
}

#[derive(Default)]
pub struct Driver {
    aspect: Option<String>,
    faces: Option<f32>,
    lights: [Option<bool>; 2],
    mph: Option<bool>,
    countdown: Option<String>,
    wrong_way: bool,
}

pub fn drive(
    view: Res<HudView>,
    osd: Option<ResMut<Osd>>,
    mobile: Option<Res<crate::touch_controls::Mobile>>,
    mut driver: Local<Driver>,
) {
    let Some(mut osd) = osd else {
        return;
    };
    let touch = mobile.is_some();
    // The aspect ratio layout (held at the range start, as the original).
    if let Some(aspect) = osd.aspect() {
        let range = aspect_range(aspect);
        if driver.aspect.as_ref() != Some(&range) {
            if osd.has_range(LAYOUT, &range) {
                osd.hold(LAYOUT, &range);
            } else {
                osd.seek(LAYOUT, 0.);
            }
            driver.aspect = Some(range);
        }
    }
    // Wider than 16:9 (a 20:9 phone): the original has no range and keeps
    // the 16:9 layout; the port continues the layout's own slope so the
    // groups stay near the edges (AUTHORED).
    let wide = osd.aspect().map_or(0., |a| (a - 16. / 9.).max(0.)) * LAYOUT_SLOPE;
    osd.offset("anim_left", Vec3::new(-wide, 0., 0.));
    osd.offset("anim_right", Vec3::new(wide, 0., 0.));
    // Touch zones (P11) fill the lower half's sides: the minimap and dial
    // move up above them (AUTHORED).
    let lift = |bottom: f32| if touch { TOUCH_BOTTOM - bottom } else { 0. };
    osd.offset("object/minimap_main=minimap", Vec3::new(0., lift(MINIMAP_BOTTOM), 0.));
    osd.offset("object/dial_main=dial", Vec3::new(0., lift(DIAL_BOTTOM), 0.));
    osd.show(LAYOUT, !view.hidden);
    for g in GROUPS {
        osd.play_once(g, "on");
    }
    osd.show("object/lap_time=lap_time", view.show_lap);
    osd.show("object/position_main=position", view.show_position);
    osd.show("object/dial_main=dial", view.show_dials);
    osd.show("object/minimap_main=minimap", view.show_minimap);
    // Unported groups stay off.
    for node in [
        "object/drift_main=drift",
        "object/points_main=points",
        "object/driftcombo_main=drift_combo",
        "object/pc_chat_whole=pc_chat",
        "object/penalty_pill=penalty",
        "object/drift_message=drift_message",
        "offers_group",
        "object/boost=boost",
    ] {
        osd.show(node, false);
    }
    for i in 1..=7 {
        for kind in ["driver_id", "driver_id_team", "driver_id_wrongway"] {
            osd.show(&format!("object/{kind}=driver_id_{i:02}"), false);
        }
    }
    lap_group(&view, &mut osd);
    position_group(&view, &mut osd);
    dial(&view, &mut osd, &mut driver);
    // Central message box: the countdown number, then GO.
    if driver.countdown != view.countdown {
        if view.countdown.is_some() {
            osd.play("messagebox_anim", "expand");
        }
        driver.countdown = view.countdown.clone();
    }
    osd.show("object/messagebox=counter", view.countdown.is_some());
    osd.set_text("number_text", view.countdown.as_deref().unwrap_or(""));
    osd.set_text("message_text", "");
    // Wrong way: the pill's helper slides in and out.
    if driver.wrong_way != view.wrong_way {
        osd.play("anim_helper", if view.wrong_way { "in" } else { "out" });
        driver.wrong_way = view.wrong_way;
    }
    osd.show("object/wrong_way_pill=wrong_way", view.wrong_way);
    // The minimap flag: in when the leader has the chequered flag.
    osd.play_once("object/flag=flag", if view.chequered { "in" } else { "out" });
}

fn lap_group(view: &HudView, osd: &mut Osd) {
    let up = |id: &'static str| view.label(id).to_uppercase();
    let info = &view.lap;
    osd.set_text("big_lap_title", &up("osd_lap"));
    osd.set_text("big_lap_number", &info.lap.to_string());
    osd.set_text("big_lap_slash", if info.total.is_some() { "/" } else { "" });
    osd.set_text("big_lap_total", &info.total.map_or(String::new(), |t| t.to_string()));
    let time = |t: Option<f64>| t.map_or_else(|| "-:--.---".into(), crate::race_hud::lap_time);
    osd.set_text("race_title", &up("osd_race"));
    osd.set_text("race_time", &time(info.race));
    osd.show("race_group", info.race.is_some());
    osd.set_text("lap_title", &up("osd_laptime"));
    osd.set_text("lap_time", &time(info.current));
    osd.set_text("best_title", &up("osd_best"));
    osd.set_text("best_time", &time(info.best));
    // PB marker: the best group's record pill.
    let pb = "object/pb_wr=best__record";
    osd.set_text("multiplier_text!1", &up("osd_pb"));
    osd.play_once(pb, if view.new_best { "on" } else { "off" });
    for record in ["object/pb_wr=lap_record", "object/pb_wr=race_record"] {
        osd.play_once(record, "off");
    }
}

fn position_group(view: &HudView, osd: &mut Osd) {
    let Some((pos, total, rows)) = &view.position else {
        return;
    };
    osd.set_text("pos_number", &pos.to_string());
    osd.set_text("pos_slash", "/");
    osd.set_text("pos_total", &total.to_string());
    // Split pills: player1 is the lowest pill; rows run top down [I].
    let shown = if view.splits { rows.len() } else { 0 };
    for pill in 1..=4usize {
        let row = rows.len().checked_sub(pill).and_then(|i| rows.get(i)).filter(|_| pill <= shown);
        let player = format!("player{pill}");
        osd.show(&player, row.is_some());
        let Some(r) = row else {
            continue;
        };
        osd.play_once(&player, "in");
        osd.play_once(&format!("splittime{pill}"), "in");
        let anim = format!("object/position_pill={pill}");
        osd.hold(&anim, if r.player { "player" } else if r.position == 1 { "first" } else { "normal" });
        let owner = format!("object/position_pill={pill}");
        if let Some(id) = osd.find(&owner, "name") {
            osd.set_text(&id, &r.name);
        }
        if let Some(id) = osd.find(&owner, "position") {
            osd.set_text(&id, &r.position.to_string());
        }
        if let Some(id) = osd.find(&format!("object/position_split={pill}"), "split") {
            osd.set_text(&id, &r.gap.map_or(String::new(), |g| format!("{g:+.1}")));
        }
    }
}

fn dial(view: &HudView, osd: &mut Osd, driver: &mut Driver) {
    let Some(g) = view.gauge else {
        return;
    };
    // Face for the display range, needle over the 240 degree sweep.
    if driver.faces != Some(g.scale) {
        let face = format!("{}", (g.scale / 1000.).round() as u32);
        osd.hold("outer_face", &face);
        driver.faces = Some(g.scale);
    }
    let sweep = osd.duration("needle");
    osd.seek("needle", (g.rpm / g.scale).clamp(0., 1.) * sweep);
    osd.seek("redzone", red_zone_time(g.scale) * osd.duration("redzone"));
    for (i, (anim, on)) in [("light_gear", g.gear_light), ("light_revs", g.rev_light)].into_iter().enumerate() {
        if driver.lights[i] != Some(on) {
            osd.play(anim, if on { "on" } else { "off" });
            driver.lights[i] = Some(on);
        }
    }
    // Gear digit, speed digits (leading zeros hidden), unit and odometer.
    osd.hold("gear", if view.gear.len() == 1 { &view.gear } else { "9" });
    for (i, d) in digits(view.speed.round().max(0.) as u32, 3).into_iter().enumerate() {
        let anim = format!("speedo_{}", i + 1);
        osd.show(&anim, d.is_some());
        osd.seek(&anim, d.unwrap_or(0) as f32 / 6.);
    }
    if driver.mph != Some(view.mph) {
        osd.hold("mph_kmh", if view.mph { "mph" } else { "kmh" });
        driver.mph = Some(view.mph);
    }
    let distance = if view.mph { view.odometer / 1609.344 } else { view.odometer / 1000. };
    // Seven odometer drums: six whole units and the red tenths [I].
    let tenths = (distance * 10.).floor().max(0.) as u64 % 10_000_000;
    for i in 0..7u32 {
        let digit = (tenths / 10u64.pow(6 - i)) % 10;
        osd.seek(&format!("odo_{:02}", i + 1), digit as f32 / 6.);
    }
    let start = |osd: &Osd, anim: &str| osd.range_start(anim, "start").zip(osd.range_end(anim, "start"));
    for (anim, value) in [("object/dial_pressure=accelerate", view.throttle), ("object/dial_pressure=brake", view.brake)] {
        if let Some((a, b)) = start(osd, anim) {
            osd.seek(anim, a + (b - a) * value.clamp(0., 1.));
        }
    }
    for anim in ["needle_blur_up", "needle_blur_down"] {
        osd.hold(anim, "on");
    }
}

/// The minimap's authored parts: the route line and one marker per car.
#[derive(Default)]
pub struct Minimap {
    done: bool,
    markers: Vec<Entity>,
    /// (min x, min z, size) of the route and the mask's centre and radius.
    map: Option<((f32, f32, f32), Vec2, f32)>,
    /// The markers' saved depth in the mask's space.
    depth: f32,
}

/// Fraction of the mask radius the route's square fills (AUTHORED).
const MAP_FILL: f32 = 0.68;
/// Depth step between minimap markers, toward the OSD camera (AUTHORED, scene
/// units): a few float steps at the camera's distance of about 3,300 units,
/// and 12 cars span 0.024, a tenth of the route ribbon's 0.2 offset.
const MARKER_STEP: f32 = 0.002;

/// A point of the map plane (world x, z) in the mask's local space: x to
/// the right, world +Z up [I, as the plain HUD].
fn map_local(p: [f32; 3], (x0, z0, size): (f32, f32, f32), centre: Vec2, radius: f32) -> Vec2 {
    let u = (p[0] - x0) / size * 2. - 1.;
    let v = (p[2] - z0) / size * 2. - 1.;
    centre + Vec2::new(u, v) * radius * MAP_FILL
}

/// A closed ribbon along the route (local x, y; z 0).
fn ribbon(points: &[Vec2], width: f32) -> grid_formats::osd::Mesh {
    let mut m = grid_formats::osd::Mesh::default();
    let n = points.len();
    if n < 2 {
        return m;
    }
    for i in 0..n {
        let (a, b) = (points[(i + n - 1) % n], points[(i + 1) % n]);
        let d = (b - a).normalize_or_zero();
        let side = Vec2::new(-d.y, d.x) * width / 2.;
        let p = points[i];
        for q in [p + side, p - side] {
            m.positions.push([q.x, q.y, 0.]);
            m.normals.push([0., 0., 1.]);
            m.uvs.push([0., 0.]);
        }
    }
    for i in 0..n as u16 {
        let j = (i + 1) % n as u16;
        m.indices.extend([2 * i, 2 * i + 1, 2 * j, 2 * j, 2 * i + 1, 2 * j + 1]);
    }
    m
}

/// Draws the route inside the original minimap frame (the original's
/// `rd4minimap` draws its own track shape into `track_mask`, AUTHORED here)
/// and moves the original player and opponent markers; the player arrow
/// turns with the car's heading [I: north-up map].
#[allow(clippy::too_many_arguments)]
pub fn minimap(
    mut commands: Commands,
    driving: Res<crate::Driving>,
    osd: Option<Res<Osd>>,
    mut state: Local<Minimap>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<crate::osd_scene::OsdMaterial>>,
    looks: Query<(&Mesh3d, &MeshMaterial3d<crate::osd_scene::OsdMaterial>)>,
    mut transforms: Query<&mut Transform>,
    camera: Query<&GlobalTransform, With<crate::osd_scene::OsdCamera>>,
    globals: Query<&GlobalTransform>,
) {
    let (Some(osd), Some(line)) = (osd, &driving.reset_line) else {
        return;
    };
    let (Some(mask), Some(bounds)) = (osd.entity("track_mask"), osd.bounds("track_mask")) else {
        return;
    };
    let cars = driving.opponents.as_ref().map_or(1, |o| o.multi.cars.len());
    if !state.done {
        state.done = true;
        let centre = Vec2::new(bounds[0] + bounds[3], bounds[1] + bounds[4]) / 2.;
        let radius = (bounds[3] - bounds[0]).min(bounds[4] - bounds[1]) / 2.;
        let fit = crate::race_hud::map_bounds(line.points());
        state.map = Some((fit, centre, radius));
        let step = line.points().len().div_ceil(240).max(1);
        let points: Vec<Vec2> = line.points().iter().step_by(step).map(|p| map_local(*p, fit, centre, radius)).collect();
        let mesh = crate::osd_scene::mesh(&ribbon(&points, radius * 0.045));
        commands.entity(mask).remove::<Mesh3d>();
        commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(materials.add(osd.colour_material(Vec3::splat(0.85), 0.85))),
            Transform::from_xyz(0., 0., 0.2),
            bevy::camera::visibility::RenderLayers::layer(crate::osd_scene::LAYER),
            bevy::camera::visibility::NoFrustumCulling,
            ChildOf(mask),
        ));
        let player = osd.entity("player");
        let opponent = osd.entity("opponent");
        if let Some(Ok(t)) = opponent.or(player).map(|e| transforms.get(e)) {
            state.depth = t.translation.z;
        }
        state.markers = vec![player.unwrap_or(Entity::PLACEHOLDER)];
        if let Some(o) = opponent {
            state.markers.push(o);
            if let (Ok((mesh, material)), Ok(t)) = (looks.get(o), transforms.get(o)) {
                for _ in 2..cars {
                    let e = commands
                        .spawn((
                            mesh.clone(),
                            material.clone(),
                            *t,
                            bevy::camera::visibility::RenderLayers::layer(crate::osd_scene::LAYER),
                            bevy::camera::visibility::NoFrustumCulling,
                            ChildOf(mask),
                        ))
                        .id();
                    state.markers.push(e);
                }
            }
        }
        return;
    }
    let Some((fit, centre, radius)) = state.map else {
        return;
    };
    // The markers blend at one saved depth, so where they overlap their
    // order was a sort tie that unrelated changes flipped. Each car now has
    // its own depth: the player's arrow in front, then the opponents in car
    // order (AUTHORED order).
    let toward = match (camera.single(), globals.get(mask)) {
        (Ok(camera), Ok(mask)) => {
            let local = mask.affine().inverse().transform_point3(camera.translation());
            if local.z >= state.depth { 1. } else { -1. }
        }
        _ => 1.,
    };
    let count = state.markers.len();
    for (car, &e) in state.markers.iter().enumerate() {
        let Ok(mut t) = transforms.get_mut(e) else {
            continue;
        };
        t.translation.z = state.depth + toward * MARKER_STEP * (count - car) as f32;
        match crate::race_hud::car_pose(&driving, car).filter(|_| car < cars) {
            Some(p) => {
                let at = map_local([p[12], p[13], p[14]], fit, centre, radius);
                t.translation.x = at.x;
                t.translation.y = at.y;
                if car == 0 {
                    // Heading from the car's forward (local +Z) axis.
                    t.rotation = Quat::from_rotation_z(-p[8].atan2(p[10]));
                }
            }
            None => t.translation.x = 1.0e6,
        }
    }
}

/// The app's own help and status text (AUTHORED, not in the original)
/// moves from the top-left corner, where the lap group is, to a small strip
/// at the bottom centre.
pub fn place_help(
    mut help: Query<(&mut Node, &mut TextFont, &mut BackgroundColor), Added<crate::DrivingHud>>,
    mobile: Option<Res<crate::touch_controls::Mobile>>,
) {
    for (mut node, mut font, mut background) in &mut help {
        // With touch zones the strip goes under the top buttons instead.
        if mobile.is_some() {
            node.top = percent(15.);
            node.bottom = Val::Auto;
        } else {
            node.top = Val::Auto;
            node.bottom = px(8.);
        }
        node.left = percent(50.);
        node.width = px(560.);
        node.margin = UiRect::left(px(-280.));
        node.padding = UiRect::all(px(6.));
        font.font_size = FontSize::Px(13.);
        background.0 = Color::srgba(0.025, 0.04, 0.065, 0.55);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_range_is_the_cut_aspect_string() {
        assert_eq!(aspect_range(16. / 9.), "1.7");
        assert_eq!(aspect_range(16. / 10.), "1.6");
        assert_eq!(aspect_range(4. / 3.), "1.3");
        assert_eq!(aspect_range(5. / 4.), "1.2");
        // A 20:9 phone has no range: the default (16:9) layout stays.
        assert_eq!(aspect_range(20. / 9.), "2.2");
    }

    #[test]
    fn red_zone_starts_at_the_red_line_angle() {
        // 12000 rpm face: 10000 rpm is 80 degrees past 12 o'clock.
        assert!((red_zone_time(12000.) - 80. / 135.).abs() < 1e-6);
        assert!((red_zone_time(8000.) - 60. / 135.).abs() < 1e-6);
    }

    #[test]
    fn speed_digits_blank_leading_zeros() {
        assert_eq!(digits(7, 3), [None, None, Some(7)]);
        assert_eq!(digits(145, 3), [Some(1), Some(4), Some(5)]);
        assert_eq!(digits(0, 3), [None, None, Some(0)]);
        assert_eq!(digits(1234, 3), [Some(9), Some(9), Some(9)]);
    }
}
