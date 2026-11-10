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

/// Needle blur (0x69CFC0 [D]): a smoothed needle value follows the needle
/// by 0.4 of the difference each update; the up blur is 30 times the needle's
/// lead over it, the down blur 30 times its lag, each 0 below 0.01 and 1
/// above 0.99. The original updates once a frame; the port applies the 0.4
/// per 1/30 s (AUTHORED rate), so the blur does not depend on the frame rate.
/// Returns the (up, down) blur fractions.
pub fn needle_blur(smoothed: &mut f32, needle: f32, dt: f32) -> (f32, f32) {
    let follow = 1. - 0.6_f32.powf(dt.max(0.) * 30.);
    *smoothed += follow * (needle - *smoothed);
    let lead = 30. * (needle - *smoothed);
    let clamp = |v: f32| if v < 0.01 { 0. } else if v > 0.99 { 1. } else { v };
    (clamp(lead), clamp(-lead))
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
    /// The split pills last reported (`osd_splits`).
    splits: String,
    /// The needle blur's smoothed needle (`needle_blur`).
    blur: f32,
    /// Split pills flipped in (`playerN`), kept up while they flip out.
    pills: [bool; 4],
    /// The penalty pill last shown (`PenaltyPill::serial`).
    penalty: Option<u32>,
    /// The brake light's flash timer (s).
    brake_flash: f32,
    /// The brake light is flashing (severity above `startflashSeverity`).
    brake_flashing: bool,
}

pub fn drive(
    view: Res<HudView>,
    osd: Option<ResMut<Osd>>,
    mobile: Option<Res<crate::touch_controls::Mobile>>,
    time: Option<Res<Time<Real>>>,
    advice: Option<Res<crate::speed_advice::SpeedAdvice>>,
    mut driver: Local<Driver>,
) {
    let dt = time.map_or(0., |t| t.delta_secs());
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
        "object/pc_chat_whole=pc_chat",
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
    penalty_pill(&view, &mut osd, &mut driver);
    position_group(&view, &mut osd, &mut driver);
    dial(&view, &mut osd, &mut driver, dt);
    brake_light(advice.map_or(0., |a| a.severity), &mut osd, &mut driver, dt);
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
    // The minimap flag: in while the player is inside an active yellow
    // flag's zone, out otherwise (0x6BA460; green and chequered have none).
    osd.play_once("object/flag=flag", if view.yellow_flag { "in" } else { "out" });
}

const BRAKE_LIGHT: &str = "brake_light";

/// The minimap's excessive-speed brake light (0x6BA460, `speed_advice`):
/// the severity seeks its animation, and the light flashes above
/// `startflashSeverity`.
fn brake_light(severity: f32, osd: &mut Osd, driver: &mut Driver, dt: f32) {
    let duration = osd.duration(BRAKE_LIGHT);
    osd.seek(BRAKE_LIGHT, severity * duration);
    let on = crate::speed_advice::flash(severity, &mut driver.brake_flash, dt);
    osd.show(BRAKE_LIGHT, on);
    let flashing = severity > crate::speed_advice::START_FLASH_SEVERITY;
    if flashing && !driver.brake_flashing {
        println!("osd_brake flashing=true severity={severity:.2}");
    }
    driver.brake_flashing = flashing;
}

const PENALTY: &str = "object/penalty_pill=penalty";

/// The touge penalty pill (0x6B3380 binds it; 0x64E1E0 shows it for the
/// player when a penalty is counted [D]). 0x6B3500 / 0x6B3630 set the header
/// to `osd_minor` / `osd_major` and the text to "+%d" of the seconds, show
/// the pill and play the `small` / `big` range from its start; the range's
/// end hides it again (anim flags 3) [D].
fn penalty_pill(view: &HudView, osd: &mut Osd, driver: &mut Driver) {
    if let Some(p) = view.penalty
        && driver.penalty != Some(p.serial)
    {
        driver.penalty = Some(p.serial);
        if let Some(id) = osd.find(PENALTY, "header_text") {
            osd.set_text(&id, &view.label(if p.major { "osd_major" } else { "osd_minor" }));
        }
        if let Some(id) = osd.find(PENALTY, "penalty_text") {
            osd.set_text(&id, &penalty_text(p.seconds));
        }
        osd.play(PENALTY, if p.major { "big" } else { "small" });
        // Reported for the interactive window check (hud_window_check.py).
        println!("osd_penalty serial={} major={} text={}", p.serial, p.major, penalty_text(p.seconds));
    }
    if view.penalty.is_none() {
        driver.penalty = None;
    }
    let up = driver.penalty.is_some() && !osd.finished(PENALTY);
    osd.show(PENALTY, up);
}

/// The pill's text: "+%d" of the seconds, truncated (0x6B3500).
pub fn penalty_text(seconds: f32) -> String {
    format!("+{}", seconds as i32)
}

fn lap_group(view: &HudView, osd: &mut Osd) {
    let up = |id: &'static str| view.label(id).to_uppercase();
    let info = &view.lap;
    osd.set_text("big_lap_title", &up("osd_lap"));
    osd.set_text("big_lap_number", &info.lap.to_string());
    osd.set_text("big_lap_slash", if info.total.is_some() { "/" } else { "" });
    osd.set_text("big_lap_total", &info.total.map_or(String::new(), |t| t.to_string()));
    // The original's OSD timers read mm:ss.hh (race_hud::osd_clock); the
    // empty placeholder is AUTHORED.
    let time = |t: Option<f64>| t.map_or_else(|| "--:--.--".into(), crate::race_hud::osd_clock);
    osd.set_text("race_title", &up("osd_race"));
    osd.set_text("race_time", &time(info.race));
    osd.show("race_group", info.race.is_some());
    osd.set_text("lap_title", &up("osd_laptime"));
    osd.set_text("lap_time", &time(info.current));
    osd.set_text("best_title", &up("osd_best"));
    osd.set_text("best_time", &time(info.best));
    // Record pill: the lap time's pill (0x6B3AF0 binds `lap_record` and
    // `pb_wr_lap_text`; 0x6B7A90 writes `osd_wr` for a world record, else
    // `osd_pb`, and plays "on", 0x6B2D00 "off" [D]). The other two record
    // pills are not bound by the original's code. In dial2.pssg the pill's
    // text node is `multiplier_text!0`, found under the pill.
    if let Some(id) = osd.find("object/pb_wr=lap_record", "multiplier_text") {
        osd.set_text(&id, &up(if view.world_record { "osd_wr" } else { "osd_pb" }));
    }
    osd.play_once("object/pb_wr=lap_record", if view.new_best { "on" } else { "off" });
    for record in ["object/pb_wr=best__record", "object/pb_wr=race_record"] {
        osd.play_once(record, "off");
    }
}

fn position_group(view: &HudView, osd: &mut Osd, driver: &mut Driver) {
    let Some((pos, total, rows)) = &view.position else {
        return;
    };
    // `osd_splits` reports each change of the shown pills (the interactive
    // window check, tools/hud_window_check.py).
    let report = if view.splits {
        rows.iter().map(|r| format!("{}:{}", r.position, r.time)).collect::<Vec<_>>().join(",")
    } else {
        String::new()
    };
    if driver.splits != report {
        println!("osd_splits shown={} rows={report}", if view.splits { rows.len() } else { 0 });
        driver.splits = report;
    }
    osd.set_text("pos_number", &pos.to_string());
    osd.set_text("pos_slash", "/");
    osd.set_text("pos_total", &total.to_string());
    // Split pills: player1 is the lowest pill; rows run top down [I].
    let shown = if view.splits { rows.len() } else { 0 };
    for pill in 1..=4usize {
        let row = rows.len().checked_sub(pill).and_then(|i| rows.get(i)).filter(|_| pill <= shown);
        let player = format!("player{pill}");
        // `playerN` `in` turns the pill from -90 to 0 to +90 degrees about X;
        // the original's rest pose is the range's middle (0x6A7180 selects
        // `in` and seeks to 0.5 [D]). The pill flips in over the first half
        // and out over the second [I].
        let Some(r) = row else {
            if driver.pills[pill - 1] {
                osd.play_part_once(&player, "in", 0.5, 1.);
                if osd.finished(&player) {
                    driver.pills[pill - 1] = false;
                }
            }
            osd.show(&player, driver.pills[pill - 1]);
            continue;
        };
        osd.show(&player, true);
        driver.pills[pill - 1] = true;
        osd.play_part_once(&player, "in", 0., 0.5);
        osd.play_once(&format!("splittime{pill}"), "in");
        // Both pills colour themselves through their attach animation's ranges (0x69F4E0,
        // 0x69F560). The split pill's ranges fade its alpha in from 0 (0.55 normal, 0.9 player,
        // 0.8 leader), so it plays its range and rests at the end [I].
        let state = if r.player { "player" } else if r.position == 1 { "first" } else { "normal" };
        osd.hold(&format!("object/position_pill={pill}"), state);
        osd.play_once(&format!("object/position_split={pill}"), state);
        let owner = format!("object/position_pill={pill}");
        if let Some(id) = osd.find(&owner, "name") {
            osd.set_text(&id, &r.name);
        }
        if let Some(id) = osd.find(&owner, "position") {
            osd.set_text(&id, &r.position.to_string());
        }
        if let Some(id) = osd.find(&format!("object/position_split={pill}"), "split") {
            osd.set_text(&id, &r.time);
        }
    }
}

fn dial(view: &HudView, osd: &mut Osd, driver: &mut Driver, dt: f32) {
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
    let needle = (g.rpm / g.scale).clamp(0., 1.);
    osd.seek("needle", needle * sweep);
    let (up, down) = needle_blur(&mut driver.blur, needle, dt);
    for (anim, blur) in [("needle_blur_up", up), ("needle_blur_down", down)] {
        osd.seek(anim, blur * osd.duration(anim));
    }
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
}

/// Per-track minimap camera values: database.bin `track` columns
/// `minimap_zoom`, `minimap_dist_behind` and `minimap_dist_above` [D], which
/// the HUD's 0x6BA460 reads and hands to the minimap (0x6C0F80 queues them,
/// 0x6A38E0 stores them on the minimap camera).
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct MapView {
    /// Scales the ribbon width and the icon sizes; the camera ignores it.
    pub zoom: f32,
    /// Eye distance behind the car along its flat heading.
    pub behind: f32,
    /// Eye height above the map plane.
    pub above: f32,
}

impl Default for MapView {
    /// The minimap's built-in values (0x6C0680 [D]), for a route the
    /// database has no track row for.
    fn default() -> Self {
        Self { zoom: 500., behind: 116., above: 110. }
    }
}

/// Field of view in degrees (0x6C0680 field +0x24 = 60, set on the minimap
/// camera before 0x6B28A0 draws [D value]; [I] vertical, square target).
const MAP_FOV: f32 = 60.;
/// Height of the camera's target above the map plane (field +0x2c [D]).
const MAP_TARGET_HEIGHT: f32 = 10.;
/// Ribbon half-width per zoom unit (field +0x30 [D]): 0x6B1610 places each
/// route node's two vertices at node ± side · 0.03 · zoom, at height 0.
const MAP_TRACK_WIDTH: f32 = 0.03;
/// Near and far planes of `minimap_root` (0x6AB8D0 [D]).
const MAP_NEAR: f32 = 0.1;
const MAP_FAR: f32 = 10_000.;
/// Track ribbon colour 0xEFD2D2D2 (ARGB, field +0x34 [D]).
const MAP_TRACK_RGB: f32 = 210. / 255.;
const MAP_TRACK_ALPHA: f32 = 239. / 255.;
/// Sides of the polygon that stands in for the ring when the ribbon is cut
/// to it (AUTHORED; the original masks a render target with `track_mask`).
const RING_SIDES: usize = 64;

/// The `track_model` row of `route` (`tracks/<folder>/<file>/<route>`),
/// else the first row of its venue.
pub fn track_model<'a>(db: &'a grid_formats::database::Database, route: &str) -> Option<grid_formats::database::Row<'a>> {
    let models = db.table("track_model")?;
    let route = route.replace('\\', "/").to_ascii_lowercase();
    let venue = route.rsplit_once('/').map_or(route.as_str(), |(v, _)| v);
    let key = |m: &grid_formats::database::Row<'_>| -> Option<(String, String)> {
        let venue = format!("tracks/{}/{}", m.text("folder_string")?, m.text("file_string")?).to_ascii_lowercase();
        Some((format!("{venue}/{}", m.text("route_string")?.to_ascii_lowercase()), venue))
    };
    models
        .rows()
        .find(|m| key(m).is_some_and(|(r, _)| r == route))
        .or_else(|| models.rows().find(|m| key(m).is_some_and(|(_, v)| v == venue)))
}

impl MapView {
    /// The values of `route`'s track (`tracks/<folder>/<file>/<route>`):
    /// the `track_model` row of that route, else any row of its venue, then
    /// that row's `track`.
    pub fn from_database(db: &grid_formats::database::Database, route: &str) -> Option<Self> {
        let tracks = db.table("track")?;
        let model = track_model(db, route)?;
        let track = tracks.rows().find(|t| t.int("id") == model.int("track_id"))?;
        Some(Self {
            zoom: track.float("minimap_zoom")?,
            behind: track.float("minimap_dist_behind")?,
            above: track.float("minimap_dist_above")?,
        })
    }

    /// [`Self::from_database`] from the installation at `root`, else the
    /// built-in values.
    pub fn load(root: &std::path::Path, route: &str) -> Self {
        std::fs::read(root.join("database/schema.bin"))
            .and_then(|s| std::fs::read(root.join("database/database.bin")).map(|d| (s, d)))
            .ok()
            .and_then(|(s, d)| grid_formats::database::read(&s, &d).ok())
            .and_then(|db| Self::from_database(&db, route))
            .unwrap_or_default()
    }

    /// The minimap camera of a car at `pos` heading `forward` (0x6B28A0
    /// [D]): the eye `behind` back along the flat heading at height `above`,
    /// looking at the point `MAP_TARGET_HEIGHT` above the car, world up. The
    /// heights are absolute: the camera, like the ribbon, ignores the car's
    /// and the route's elevation. Returns (view, projection).
    pub fn camera(&self, pos: Vec3, forward: Vec3) -> Option<(Mat4, Mat4)> {
        let f = Vec3::new(forward.x, 0., forward.z).try_normalize()?;
        let ground = Vec3::new(pos.x, 0., pos.z);
        let eye = ground - f * self.behind + Vec3::Y * self.above;
        let view = Mat4::look_at_rh(eye, ground + Vec3::Y * MAP_TARGET_HEIGHT, Vec3::Y);
        Some((view, Mat4::perspective_rh(MAP_FOV.to_radians(), 1., MAP_NEAR, MAP_FAR)))
    }
}

/// A map-plane point (world x, z at height 0) seen by the minimap camera:
/// view-space position, or `None` behind the near plane.
fn map_view_point(view: Mat4, p: [f32; 3]) -> Option<Vec3> {
    let v = view.transform_point3(Vec3::new(p[0], 0., p[2]));
    (v.z <= -MAP_NEAR).then_some(v)
}

/// A view-space point on the minimap target, in the mask's local space: the
/// target's square spans the ring's square (NDC ±1 = ±radius).
fn map_target(projection: Mat4, v: Vec3, centre: Vec2, radius: f32) -> Vec2 {
    let clip = projection * v.extend(1.);
    centre + Vec2::new(clip.x, clip.y) / clip.w * radius
}

/// Sutherland-Hodgman: the part of `poly` on the side of `inside`, with
/// `cut` giving the crossing point of an edge.
fn clip_polygon<T: Copy>(poly: &[T], inside: impl Fn(T) -> bool, cut: impl Fn(T, T) -> T) -> Vec<T> {
    let mut out = Vec::with_capacity(poly.len() + 2);
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        match (inside(a), inside(b)) {
            (true, true) => out.push(b),
            (true, false) => out.push(cut(a, b)),
            (false, true) => out.extend([cut(a, b), b]),
            (false, false) => {}
        }
    }
    out
}

/// The track ribbon the minimap camera sees, in the mask's local space and
/// cut to the ring: the route at height 0, `half_width` either side of each
/// point (0x6B1610 [D]; the original's nodes carry their own side vectors,
/// here the route's flat normal), closed unless `open`.
fn map_ribbon(
    points: &[[f32; 3]],
    open: bool,
    half_width: f32,
    (view, projection): (Mat4, Mat4),
    centre: Vec2,
    radius: f32,
) -> grid_formats::osd::Mesh {
    let mut m = grid_formats::osd::Mesh::default();
    let n = points.len();
    if n < 2 {
        return m;
    }
    let flat = |p: [f32; 3]| Vec2::new(p[0], p[2]);
    let sides: Vec<[Vec3; 2]> = (0..n)
        .map(|i| {
            let prev = if i > 0 { i - 1 } else if open { 0 } else { n - 1 };
            let next = if i + 1 < n { i + 1 } else if open { n - 1 } else { 0 };
            let d = (flat(points[next]) - flat(points[prev])).normalize_or_zero();
            let s = Vec2::new(-d.y, d.x) * half_width;
            let p = flat(points[i]);
            [Vec3::new(p.x + s.x, 0., p.y + s.y), Vec3::new(p.x - s.x, 0., p.y - s.y)]
        })
        .map(|[l, r]| [view.transform_point3(l), view.transform_point3(r)])
        .collect();
    let ring: Vec<Vec2> = (0..RING_SIDES)
        .map(|k| centre + Vec2::from_angle(k as f32 * std::f32::consts::TAU / RING_SIDES as f32) * radius)
        .collect();
    // Inside every side of the ring polygon: no cut needed.
    let safe = radius * (std::f32::consts::PI / RING_SIDES as f32).cos();
    let segments = if open { n - 1 } else { n };
    for i in 0..segments {
        let j = (i + 1) % n;
        let [l0, r0] = sides[i];
        let [l1, r1] = sides[j];
        for tri in [[l0, r0, r1], [l0, r1, l1]] {
            if tri.iter().all(|v| v.z > -MAP_NEAR) {
                continue;
            }
            let near = clip_polygon(&tri, |v| v.z <= -MAP_NEAR, |a, b| a.lerp(b, (-MAP_NEAR - a.z) / (b.z - a.z)));
            let mut poly: Vec<Vec2> = near.iter().map(|v| map_target(projection, *v, centre, radius)).collect();
            let lo = poly.iter().fold(Vec2::INFINITY, |a, p| a.min(*p));
            let hi = poly.iter().fold(Vec2::NEG_INFINITY, |a, p| a.max(*p));
            if hi.x < centre.x - radius || lo.x > centre.x + radius || hi.y < centre.y - radius || lo.y > centre.y + radius {
                continue;
            }
            if poly.iter().any(|p| p.distance(centre) > safe) {
                for k in 0..RING_SIDES {
                    let (a, b) = (ring[k], ring[(k + 1) % RING_SIDES]);
                    let edge = b - a;
                    let side = |p: Vec2| edge.perp_dot(p - a);
                    poly = clip_polygon(&poly, |p| side(p) >= 0., |p, q| p.lerp(q, side(p) / (side(p) - side(q))));
                    if poly.len() < 3 {
                        break;
                    }
                }
            }
            // The u16 index range bounds one ribbon (AUTHORED guard).
            if poly.len() < 3 || m.positions.len() + poly.len() > u16::MAX as usize {
                continue;
            }
            let base = m.positions.len() as u16;
            for p in &poly {
                m.positions.push([p.x, p.y, 0.]);
                m.normals.push([0., 0., 1.]);
                m.uvs.push([0., 0.]);
            }
            for k in 1..poly.len() as u16 - 1 {
                m.indices.extend([base, base + k, base + k + 1]);
            }
        }
    }
    m
}

/// The minimap's parts: the ribbon and one marker per car.
#[derive(Default)]
pub struct Minimap {
    done: bool,
    markers: Vec<Entity>,
    /// The mask's centre and ring radius in its local space.
    ring: Option<(Vec2, f32)>,
    ribbon: Option<Handle<Mesh>>,
    /// The markers' saved depth in the mask's space.
    depth: f32,
    /// The opponent marker's saved scale.
    scale: Vec3,
}

/// Depth step between minimap markers, toward the OSD camera (AUTHORED, scene
/// units): a few float steps at the camera's distance of about 3,300 units,
/// and 12 cars span 0.024, a tenth of the route ribbon's 0.2 offset.
const MARKER_STEP: f32 = 0.002;

/// Draws the original minimap (0x6B28A0 [D]): the route ribbon and the cars
/// as the minimap camera sees them from behind and above the player, heading
/// up, inside the `track_mask` ring. The player arrow points up; opponent
/// discs scale with their distance like the original's world-sized icons
/// (0x6AB520), and leave the map outside the ring.
#[allow(clippy::too_many_arguments)]
pub fn minimap(
    mut commands: Commands,
    driving: Res<crate::Driving>,
    osd: Option<Res<Osd>>,
    view: Option<Res<MapView>>,
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
    let view = view.map(|v| *v).unwrap_or_default();
    let cars = driving.opponents.as_ref().map_or(1, |o| o.multi.cars.len());
    if !state.done {
        state.done = true;
        let centre = Vec2::new(bounds[0] + bounds[3], bounds[1] + bounds[4]) / 2.;
        let radius = (bounds[3] - bounds[0]).min(bounds[4] - bounds[1]) / 2.;
        state.ring = Some((centre, radius));
        let ribbon = meshes.add(crate::osd_scene::mesh(&grid_formats::osd::Mesh::default()));
        state.ribbon = Some(ribbon.clone());
        commands.entity(mask).remove::<Mesh3d>();
        commands.spawn((
            Mesh3d(ribbon),
            MeshMaterial3d(materials.add(osd.colour_material(Vec3::splat(MAP_TRACK_RGB), MAP_TRACK_ALPHA))),
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
        state.scale = opponent.and_then(|o| transforms.get(o).ok()).map_or(Vec3::ONE, |t| t.scale);
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
    let (Some((centre, radius)), Some(player)) = (state.ring, crate::race_hud::car_pose(&driving, 0)) else {
        return;
    };
    let Some(cam) = view.camera(Vec3::new(player[12], player[13], player[14]), Vec3::new(player[8], 0., player[10])) else {
        return;
    };
    if let Some(mut mesh) = state.ribbon.as_ref().and_then(|h| meshes.get_mut(h)) {
        let ribbon = map_ribbon(line.points(), line.is_open(), MAP_TRACK_WIDTH * view.zoom, cam, centre, radius);
        *mesh = crate::osd_scene::mesh(&ribbon);
    }
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
    let player_depth = map_view_point(cam.0, [player[12], 0., player[14]]).map_or(1., |v| -v.z);
    let count = state.markers.len();
    for (car, &e) in state.markers.iter().enumerate() {
        let Ok(mut t) = transforms.get_mut(e) else {
            continue;
        };
        t.translation.z = state.depth + toward * MARKER_STEP * (count - car) as f32;
        let seen = crate::race_hud::car_pose(&driving, car)
            .filter(|_| car < cars)
            .and_then(|p| map_view_point(cam.0, [p[12], 0., p[14]]))
            .map(|v| (map_target(cam.1, v, centre, radius), -v.z))
            .filter(|(at, _)| at.distance(centre) <= radius);
        match seen {
            Some((at, depth)) => {
                t.translation.x = at.x;
                t.translation.y = at.y;
                if car == 0 {
                    // The camera follows the heading: the arrow points up.
                    t.rotation = Quat::IDENTITY;
                } else {
                    t.scale = state.scale * (player_depth / depth).clamp(0.25, 4.);
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
    fn penalty_text_truncates_the_seconds() {
        assert_eq!(penalty_text(2.), "+2");
        assert_eq!(penalty_text(10.9), "+10");
    }

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

    #[test]
    fn needle_blur_follows_the_needle_speed() {
        let mut s = 0.;
        // A steady needle has no blur.
        for _ in 0..200 {
            needle_blur(&mut s, 0.5, 1. / 60.);
        }
        assert_eq!(needle_blur(&mut s, 0.5, 1. / 60.), (0., 0.));
        // One 1/30 s step up by 0.02: the smoothed value moves 0.4 of it,
        // the lead 0.012 blurs up 0.36.
        let (up, down) = needle_blur(&mut s, 0.52, 1. / 30.);
        assert!((up - 0.36).abs() < 1e-4 && down == 0., "{up} {down}");
        // A fast drop saturates the down blur.
        assert_eq!(needle_blur(&mut s, 0.3, 1. / 30.), (0., 1.));
    }

    /// The minimap camera of a car at the origin heading +Z with the
    /// Donington values, and its mapping onto a unit ring.
    fn donington_camera() -> (Mat4, Mat4) {
        MapView { zoom: 200., behind: 116., above: 110. }.camera(Vec3::ZERO, Vec3::Z).unwrap()
    }

    #[test]
    fn minimap_camera_shows_the_car_below_centre_heading_up() {
        let cam = donington_camera();
        let at = |p: [f32; 3]| map_view_point(cam.0, p).map(|v| map_target(cam.1, v, Vec2::ZERO, 1.));
        // The target is 10 above the car: the car sits 2.7 degrees below the
        // view's centre, -tan(2.73)/tan(30) of the half-height.
        let car = at([0., 0., 0.]).unwrap();
        assert!(car.x.abs() < 1e-5 && (car.y + 0.083).abs() < 0.005, "{car}");
        // The route ahead goes up the map, the car's right (Bevy: -X for a
        // +Z heading) to the right; the eye's elevation is ignored.
        assert!(at([0., 0., 150.]).unwrap().y > 0.3);
        assert!(at([-20., 0., 0.]).unwrap().x > 0.1);
        assert_eq!(at([0., 35., 0.]), Some(car));
        // Far behind the eye is behind the near plane.
        assert!(at([0., 0., -400.]).is_none());
    }

    #[test]
    fn minimap_ribbon_stays_inside_the_ring() {
        // A straight route through the car, then a turn behind it.
        let points: Vec<[f32; 3]> = (-100..300).map(|i| [0., 0., i as f32 * 5.]).chain((0..200).map(|i| [i as f32 * 5., 0., -500.])).collect();
        let m = map_ribbon(&points, false, MAP_TRACK_WIDTH * 200., donington_camera(), Vec2::new(3., 4.), 2.);
        assert!(!m.indices.is_empty() && m.indices.len().is_multiple_of(3));
        assert!(m.positions.iter().all(|p| Vec2::new(p[0], p[1]).distance(Vec2::new(3., 4.)) <= 2. + 1e-4));
        // The ribbon reaches the ring at the top (ahead) and the bottom.
        let ys = m.positions.iter().map(|p| p[1]);
        let (lo, hi) = ys.fold((f32::MAX, f32::MIN), |(a, b), y| (a.min(y), b.max(y)));
        assert!(lo < 4. - 1.9 && hi > 4. + 1.9, "{lo} {hi}");
        // At the car it is 12 m wide, 0.065 of the 184 m the view spans at the
        // car's depth: 0.13 of the NDC range.
        let near_car: Vec<f32> = m.positions.iter().filter(|p| (p[1] - (4. - 0.166)).abs() < 0.02).map(|p| p[0]).collect();
        let width = near_car.iter().cloned().fold(f32::MIN, f32::max) - near_car.iter().cloned().fold(f32::MAX, f32::min);
        assert!((width / 2. - 0.135).abs() < 0.02, "{width}");
    }

    #[test]
    #[ignore = "needs GRID_GAME_ROOT"]
    fn every_route_has_minimap_values() {
        let root = std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required"));
        let schema = std::fs::read(root.join("database/schema.bin")).unwrap();
        let data = std::fs::read(root.join("database/database.bin")).unwrap();
        let db = grid_formats::database::read(&schema, &data).unwrap();
        for route in grid_tools::route::CIRCUITS {
            let v = MapView::from_database(&db, route).unwrap_or_else(|| panic!("{route}"));
            println!("minimap route={route} {v:?}");
            assert!(v.zoom > 0. && v.behind > 0. && v.above > 0., "{route} {v:?}");
        }
        let d = MapView::from_database(&db, grid_tools::route::DONINGTON).unwrap();
        assert_eq!(d, MapView { zoom: 200., behind: 116., above: 110. });
    }
}
