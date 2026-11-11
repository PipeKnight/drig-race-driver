//! The race results screens in the original's front-end art (docs/RACE_HUD.md
//! §6.7, docs/FIDELITY_BACKLOG.md row 69): a screen glyph of
//! fePersistentDatabase ([`Screen`]: `screen/pause_leaderboards`,
//! `screen/drift_scoreboard`) with its glyphs grafted in
//! (`grid_formats::osd_glyph`), seen through the `pause_root` camera
//! `camera1`, drawn with the shared front-end materials (`fe3d`) into an image
//! shown over the race, and filled from the post-race results view
//! (`post_race::View`). Each screen is a [`Def`] of names; the layout, pose,
//! row filler and text binding are shared.
//!
//! [D] The tree, materials and text layouts; the camera, its target and the
//! animation that moves both with the screen (`Anim!96` of the package); the
//! row filler 0x54D270 instancing one row glyph per entry (`list.%d`, with a
//! `player` / `friends` / `deactivate` state); the lighting zone
//! `leaderboards` of `fe_paramcontrol.xml`.
//! [I] The pose: the first key time of that animation at which the screen
//! lies at the target's distance along the view; the other animations' middle key as
//! the on-screen pose of the node they move; twelve visible rows with the
//! window around the player; which results column fills which text node; the
//! player's row marked by the row cursor pill.
use bevy::{
    asset::RenderAssetUsages,
    camera::{RenderTarget, visibility::RenderLayers},
    core_pipeline::tonemapping::Tonemapping,
    image::ImageSampler,
    light::{Cascades, SimulationLightSystems},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureViewDescriptor, TextureViewDimension},
    transform::TransformSystems,
    ui_render::prelude::MaterialNode,
};
use crate::fe3d::{self, Fe3dMaterial};
use crate::osd_scene::{self, OsdDisplay};
use grid_formats::{osd, osd_glyph, pssg, texture};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// The render layer of the results art (free of the OSD's 23).
pub const LAYER: usize = 24;
pub(crate) const PACKAGES: [&str; 3] = [
    "frontend/fePersistentDatabase.pssg",
    "frontend/feNonPersistentDatabase.pssg",
    "frontend/fe_DLC_0.pssg",
];
/// The front end's texture bundle (the packages hold only the names).
pub(crate) const BUNDLE: &str = "frontend/b_persistent.pssg";
pub(crate) const PARAMS: &str = "frontend/fe_paramcontrol.xml";
/// The reflection cubes: `TR1` names `reflection_soft.dds` /
/// `reflection_sharp.dds`, which the race OSD package holds as `env_soft.dds`
/// / `env_sharp.dds` [I: the same images under the OSD's names].
pub(crate) const CUBES: &str = "Osd/dial2.pssg";
const CAMERA: &str = "camera=camera1";
const TARGET: &str = "target=camera1";

/// A results screen the art draws.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Screen {
    /// `screen/pause_leaderboards`: the race and drift results.
    #[default]
    Results,
    /// `screen/drift_scoreboard`: the drift statistics and the drift types
    /// (states.xml `post_race_drift_scoreboard`, glyph `drift_scoreboard`).
    DriftScoreboard,
}

/// Every screen, in the order of [`DEFS`].
const SCREENS: [Screen; 2] = [Screen::Results, Screen::DriftScoreboard];

impl Screen {
    fn def(self) -> &'static Def {
        &DEFS[self as usize]
    }
}

/// The names the art reads of one screen glyph.
#[derive(Debug)]
struct Def {
    /// The screen node.
    screen: &'static str,
    /// The row glyphs' name up to the row number (`{rows}0` is the first).
    rows: &'static str,
    /// The column titles' glyph.
    header: &'static str,
    /// Rows drawn at once.
    visible: usize,
    /// The `library_simple=content` children drawn, with the event kind; any
    /// other child is never drawn.
    content: &'static [(&'static str, Variant)],
    /// Text nodes showing the view's title and body.
    title: &'static [&'static str],
    body: &'static [&'static str],
    /// Subtrees never drawn.
    hidden: &'static [&'static str],
}

const DEFS: [Def; 2] = [
    Def {
        screen: "screen/pause_leaderboards=leaderboards",
        rows: "gadget/list_items=",
        header: "gadget/list_titles=titles",
        // [I] The list's frame holds twelve.
        visible: 12,
        content: &[("null=race_results", Variant::Race), ("null=drift_results", Variant::Drift)],
        title: &["title_0", "title_1"],
        body: &["table_title"],
        // The race timer, the row arrows and flags, the frame without
        // scroller [I: the frame of a list that scrolls].
        hidden: &[
            "text=timer",
            "{30}component/leaderboard_arrow=arrow",
            "library/image_flag=flag",
            "component/leaderboard_noscroll=noscroll",
        ],
    },
    Def {
        screen: "screen/drift_scoreboard=drift_scoreboard",
        rows: "gadget/drift_list_items=",
        header: "gadget/drift_list_titles=drift_titles",
        // [I] Row 7 would cover the total (rows 8 apart from y 36, the
        // total at -20).
        visible: 7,
        // states.xml binds both lists' content to these objects [D].
        content: &[("null=online_leaderboard", Variant::Any), ("null=event_standings", Variant::Any)],
        // `title_01` = drift_scoreboard_screen_title; `title_00` and
        // `table_title` are not bound [D].
        title: &["title_01"],
        body: &[],
        hidden: &[],
    },
];

// ------------------------------------------------------------------ the board

/// What the fourth results column holds (post_race `view_with`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Fourth {
    #[default]
    Team,
    Class,
    Points,
    Round,
}

/// The results the art shows.
#[derive(Clone, Debug, PartialEq)]
pub struct Board {
    pub view: crate::post_race::View,
    pub drift: bool,
    pub fourth: Fourth,
    pub screen: Screen,
}

/// Written by the post-race screen: the results to draw, `None` to hide.
#[derive(Resource, Default)]
pub struct Shown {
    pub board: Option<Board>,
}

/// True once the art is loaded and spawned: the post-race panel then leaves
/// the results to it.
#[derive(Resource, Default)]
pub struct Ready(pub bool);

// ------------------------------------------------------------------ gates

/// Which content variant (`library_simple=content` child) a node belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Variant {
    Any,
    Race,
    Drift,
    Never,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Gate {
    hidden: bool,
    variant: Variant,
    row: bool,
    header: bool,
    player_only: bool,
    scrollbar: bool,
}

/// The gate of a node of screen `def` from its name and its ancestors'
/// (`names[0]` the node, then up to the screen).
fn gate(names: &[&str], def: &Def) -> Gate {
    let any = |n: &str| names.contains(&n);
    let hidden = names.iter().any(|n| def.hidden.contains(n) || n.strip_prefix(def.rows).is_some_and(|k| k != "0"))
        || names.first() == Some(&"scroller_v_manual=centre_face");
    let variant = names
        .windows(2)
        .find(|w| w[1].ends_with("library_simple=content"))
        .map_or(Variant::Any, |w| def.content.iter().find(|c| c.0 == w[0]).map_or(Variant::Never, |c| c.1));
    let row = names.iter().any(|n| n.strip_prefix(def.rows) == Some("0"));
    Gate {
        hidden,
        variant,
        row,
        header: any(def.header),
        // The row cursor pill; the drift statistics' pills are always lit.
        player_only: row && any("null=track_info_pill"),
        scrollbar: any("null=scrollbar"),
    }
}

/// A results column as the text nodes name it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Column {
    Position,
    Driver,
    Time,
    Car,
    Team,
    Points,
    Class,
    Difference,
    /// The drift scoreboard's drift type and count.
    DriftType,
    Amount,
}

fn column(text: &str) -> Option<Column> {
    Some(match text {
        "position" => Column::Position,
        "driver" => Column::Driver,
        "time" | "score" => Column::Time,
        "car" => Column::Car,
        "team" => Column::Team,
        "points" => Column::Points,
        "class" => Column::Class,
        "difference" => Column::Difference,
        "drift_type" => Column::DriftType,
        "amount" | "drift_amount" => Column::Amount,
        _ => return None,
    })
}

/// The `View` table column a text node shows [I].
fn view_column(c: Column, fourth: Fourth) -> Option<usize> {
    match c {
        Column::Position => Some(0),
        Column::Driver => Some(1),
        Column::Time => Some(2),
        Column::Car => Some(3),
        Column::Team => (fourth != Fourth::Points).then_some(4),
        Column::Points => (fourth == Fourth::Points).then_some(4),
        Column::Class => (fourth == Fourth::Class).then_some(4),
        // The touge's second run [I: the gap column's place].
        Column::Difference => (fourth == Fourth::Round).then_some(5),
        Column::DriftType => Some(0),
        Column::Amount => Some(1),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Field {
    Title,
    Body,
    Header(Column),
    Cell(Column),
    /// A drift statistic (`View::stats` index): its label, or its value.
    Stat(usize, bool),
    /// The label of a pill of the strip ([`Strip`]).
    Pill,
}

/// The drift statistic a node belongs to: its `drift_cursor_half` or
/// `drift_cursor_total` glyph, by the states.xml item id [D].
fn stat(names: &[&str]) -> Option<usize> {
    names
        .iter()
        .find_map(|n| n.strip_prefix("component/drift_cursor_half=").or_else(|| n.strip_prefix("component/drift_cursor_total=")))
        .and_then(|k| crate::post_race::DRIFT_STATS.iter().position(|s| *s == k))
}

fn field(names: &[&str], g: &Gate, def: &Def) -> Option<Field> {
    let text = names.first()?.strip_prefix("text=")?;
    if def.title.contains(&text) {
        return Some(Field::Title);
    }
    if def.body.contains(&text) {
        return Some(Field::Body);
    }
    if let Some(i) = stat(names) {
        // states.xml binds `drift_type` (`total` in the total) to the label;
        // `score` shows the value [D].
        return match text {
            "drift_type" | "total" => Some(Field::Stat(i, false)),
            "score" => Some(Field::Stat(i, true)),
            _ => None,
        };
    }
    let c = column(text)?;
    if g.row {
        Some(Field::Cell(c))
    } else if g.header {
        Some(Field::Header(c))
    } else {
        None
    }
}

/// The first data row drawn of `visible`: the player's row in the middle of
/// the window.
fn window(rows: usize, player: Option<usize>, visible: usize) -> usize {
    if rows <= visible {
        return 0;
    }
    player.unwrap_or(0).saturating_sub(visible / 2).min(rows - visible)
}

/// The string a text field shows for `board` (row `slot` of the window).
fn text_of(field: Field, slot: Option<usize>, board: &Board) -> String {
    let v = &board.view;
    let cell = |row: usize, c: Column| {
        view_column(c, board.fourth).and_then(|i| v.table.get(row).map(|r| r[i].clone())).unwrap_or_default()
    };
    match field {
        Field::Title => v.title.clone(),
        Field::Body => v.body.clone(),
        Field::Header(c) => cell(0, c),
        Field::Cell(c) => {
            let rows = v.table.len().saturating_sub(1);
            let start = window(rows, v.player_row.map(|p| p.saturating_sub(1)), board.screen.def().visible);
            cell(1 + start + slot.unwrap_or(0), c)
        }
        Field::Stat(i, value) => v.stats.get(i).map(|s| s[usize::from(value)].clone()).unwrap_or_default(),
        Field::Pill => slot.and_then(|k| v.buttons.get(k)).cloned().unwrap_or_default(),
    }
}

/// Whether row `slot` of the window is the player's row.
fn on_player_row(slot: Option<usize>, board: &Board) -> bool {
    let rows = board.view.table.len().saturating_sub(1);
    let player = board.view.player_row.map(|p| p.saturating_sub(1));
    slot.is_some_and(|k| Some(window(rows, player, board.screen.def().visible) + k) == player)
}

/// Whether an item of `gate` in row `slot` is drawn for `board`.
fn visible(gate: &Gate, slot: Option<usize>, board: &Board) -> bool {
    let variant = match gate.variant {
        Variant::Any => true,
        Variant::Race => !board.drift,
        Variant::Drift => board.drift,
        Variant::Never => false,
    };
    let rows = board.view.table.len().saturating_sub(1);
    let player = board.view.player_row.map(|p| p.saturating_sub(1));
    let shown = board.screen.def().visible;
    let start = window(rows, player, shown);
    let row = slot.map(|k| start + k);
    variant
        && !gate.hidden
        && row.is_none_or(|r| r < rows)
        && (!gate.player_only || (row.is_some() && row == player))
        && (!gate.scrollbar || rows > shown)
}

// ------------------------------------------------------------------ pose

pub(crate) const IDENTITY: [f32; 16] = [1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.];

/// `saved` with the translation, rotation and scale the channels give at `t`.
pub(crate) fn animated(saved: &[f32; 16], channels: &[&osd::Channel], t: f32) -> [f32; 16] {
    let (mut s, mut r, mut p) = Mat4::from_cols_array(saved).to_scale_rotation_translation();
    for c in channels {
        if c.components.len() != 3 {
            continue;
        }
        let v = Vec3::new(c.components[0].sample(t), c.components[1].sample(t), c.components[2].sample(t));
        match c.semantic {
            osd::Semantic::Translation => p = v,
            osd::Semantic::Scale => s = v,
            osd::Semantic::Rotation => r = Quat::from_euler(EulerRot::ZYX, v.z, v.y, v.x),
            _ => {}
        }
    }
    Mat4::from_scale_rotation_translation(s, r, p).to_cols_array()
}

pub(crate) fn targets(c: &osd::Channel, n: &osd::Node) -> bool {
    c.target == n.id || c.target == n.name()
}

/// The camera pose of the results screen.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Pose {
    time: f32,
    eye: Vec3,
    target: Vec3,
    fov_degrees: f32,
}

/// Picks the first key time of `anim` (the screen's translation keys) from
/// 0 at which the screen lies at the target's distance along the camera's
/// view, within [`ARRIVED`] of it [I]: the animation flies the screen in and
/// holds it in front of the camera. `pause_root`'s `Anim!96` arrives at
/// 1.667 s; its second hold (3.333 to 4.667 s) brings the screen 30 % closer,
/// past the view's edges. Without such a key, the least error.
fn pose(s: &osd::Scene, anim: &osd::Animation, screen: usize, camera: usize, target: usize) -> Pose {
    let keys = key_errors(s, anim, screen, camera, target);
    let time = keys
        .iter()
        .find(|&&(t, e)| t >= 0. && e < ARRIVED)
        .or_else(|| keys.iter().min_by(|a, b| a.1.total_cmp(&b.1)))
        .map_or(0., |&(t, _)| t);
    pose_at(s, anim, camera, target, time)
}

/// The camera pose at time `time` of `anim`.
fn pose_at(s: &osd::Scene, anim: &osd::Animation, camera: usize, target: usize, time: f32) -> Pose {
    let world = |node: usize, t: f32| world_at(s, anim, node, t);
    let fov_degrees = anim
        .channels
        .iter()
        .find(|c| c.semantic == osd::Semantic::Fov && targets(c, &s.nodes[camera]))
        .and_then(|c| c.components.first())
        .map(|k| k.sample(time))
        .or_else(|| s.cameras.iter().find(|c| c.node == camera).map(|c| c.fov_degrees))
        .unwrap_or(45.);
    Pose { time, eye: world(camera, time), target: world(target, time), fov_degrees }
}

/// A node's world position at time `t` of `anim`.
fn world_at(s: &osd::Scene, anim: &osd::Animation, node: usize, t: f32) -> Vec3 {
    let mut chain = vec![node];
    while let Some(p) = s.nodes[*chain.last().unwrap()].parent {
        chain.push(p);
    }
    let mut w = IDENTITY;
    for &n in chain.iter().rev() {
        let channels: Vec<&osd::Channel> = anim.channels.iter().filter(|c| targets(c, &s.nodes[n])).collect();
        w = grid_formats::scene::compose(&animated(&s.nodes[n].local, &channels, t), &w);
    }
    Vec3::new(w[12], w[13], w[14])
}

/// Relative distance error under which the screen has arrived (`pose`).
const ARRIVED: f32 = 0.01;

/// Each key time of the screen's translation with how far the screen then
/// lies from the target's distance along the camera's view, relative to that
/// distance.
fn key_errors(s: &osd::Scene, anim: &osd::Animation, screen: usize, camera: usize, target: usize) -> Vec<(f32, f32)> {
    let world = |node: usize, t: f32| world_at(s, anim, node, t);
    let mut times: Vec<f32> = anim
        .channels
        .iter()
        .filter(|c| c.semantic == osd::Semantic::Translation && targets(c, &s.nodes[screen]))
        .flat_map(|c| c.components.iter().flat_map(|k| k.0.iter().map(|k| k.time)))
        .collect();
    times.sort_by(f32::total_cmp);
    times.dedup();
    let error = |t: f32| {
        let (e, g, p) = (world(camera, t), world(target, t), world(screen, t));
        let reach = (g - e).length();
        if reach < 1e-4 {
            return f32::MAX;
        }
        ((p - e).dot((g - e) / reach) - reach).abs() / reach
    };
    times.into_iter().map(|t| (t, error(t))).collect()
}

/// The on-screen translation an animation of three-key slides gives a node:
/// its middle key, when the first key is the saved translation [I].
fn slide(channel: &osd::Channel, saved: Vec3) -> Option<Vec3> {
    if channel.semantic != osd::Semantic::Translation
        || channel.components.len() != 3
        || channel.components.iter().any(|c| c.0.len() != 3)
    {
        return None;
    }
    let at = |i: usize| Vec3::new(channel.components[0].0[i].value, channel.components[1].0[i].value, channel.components[2].0[i].value);
    let start = Vec3::new(
        channel.components[0].sample(0.),
        channel.components[1].sample(0.),
        channel.components[2].sample(0.),
    );
    ((start - saved).abs().max_element() < 0.01).then(|| at(1))
}

// ------------------------------------------------------------------ layout

#[derive(Clone, Debug)]
enum Shape {
    Static(osd::Mesh),
    Text { layout: osd::TextLayout, field: Field },
}

#[derive(Clone, Debug)]
struct Item {
    world: Mat4,
    material: usize,
    family: u32,
    bounds: [f32; 6],
    shape: Shape,
    gate: Gate,
    /// The [`Layout::motions`] entries of the node and its ancestors.
    motion: Vec<usize>,
    /// In the pill strip: its [`Strip::nodes`] entry and the pill template
    /// (the root's child) it belongs to.
    strip: Option<(usize, usize)>,
}

/// Which event of a screen's own animations plays: the flow 0x4A6670 sends
/// the leaving screen `exit` and waits for it before the next screen's
/// `enter` [D].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Play {
    Enter,
    Exit,
}

/// A node of the screen that its own animation slides on `enter` and `exit`
/// (`helper=anim` of `leaderboards_root`: X -180 at frame 0, 0 at 1, 150 at
/// 2, each event 0.4 s) [D].
#[derive(Clone, Debug)]
struct Motion {
    channel: osd::Channel,
    /// The world matrix of the node's parent.
    parent: Mat4,
    /// The translation the layout placed it at.
    rest: Vec3,
    enter: Option<osd::Event>,
    exit: Option<osd::Event>,
    /// The frame `enter` ends at (where `exit` starts).
    held: f32,
}

impl Motion {
    fn event(&self, play: Play) -> Option<&osd::Event> {
        match play {
            Play::Enter => self.enter.as_ref(),
            Play::Exit => self.exit.as_ref(),
        }
    }

    /// The animation frame `t` seconds into `play`.
    fn frame(&self, play: Play, t: f32) -> f32 {
        self.event(play).map_or(self.held, |e| e.at(self.held, t).0)
    }

    /// The channel's translation at `frame`.
    fn at(&self, frame: f32) -> Vec3 {
        let c = &self.channel.components;
        Vec3::new(
            c.first().map_or(self.rest.x, |k| k.sample(frame)),
            c.get(1).map_or(self.rest.y, |k| k.sample(frame)),
            c.get(2).map_or(self.rest.z, |k| k.sample(frame)),
        )
    }

    /// The world offset of the node at `frame` from where the layout put it,
    /// which is where `enter` leaves it: the authored move from the held
    /// frame. Some nodes rest a little away from their channel's held key
    /// (the results title: x 0.76 against 0.6), so the delta, not the
    /// absolute key, is applied [I].
    fn offset(&self, frame: f32) -> Vec3 {
        self.parent.transform_vector3(self.at(frame) - self.at(self.held))
    }
}

/// How long `play` runs on a screen: its longest event.
fn play_seconds(motions: &[Motion], play: Play) -> f32 {
    motions.iter().filter_map(|m| m.event(play).map(|e| e.seconds(m.held))).fold(0., f32::max)
}

#[derive(Clone, Debug)]
struct MaterialDef {
    values: fe3d::Values,
    /// 1 for the sharp reflection cube, 0 for the soft one.
    sharp: bool,
    dirt: Option<String>,
    pattern: Option<String>,
    diffuse: Option<String>,
    /// The DiffuseColour of the row states `player` and `deactivate` when the
    /// packages animate it ([`state_colours`]).
    states: [Option<Vec3>; 2],
    /// The values after the events `activate` and `deactivate` (the pills,
    /// [`event_values`]).
    events: [Option<fe3d::Values>; 2],
}

#[derive(Debug)]
struct Layout {
    pose: Pose,
    near: f32,
    far: f32,
    items: Vec<Item>,
    materials: Vec<MaterialDef>,
    /// World offset from one row to the next.
    pitch: Vec3,
    /// The nodes the screen's `enter` and `exit` slide.
    motions: Vec<Motion>,
    /// The pill strip.
    strip: Option<Strip>,
    /// What was left out, for the log.
    skipped: HashMap<&'static str, usize>,
}

/// The end times of the held windows of `a`: the key intervals over which a
/// DiffuseColour component that changes somewhere keeps its value, merged
/// where they touch.
///
/// Every animation of the row text colours (`text_pill_*#DiffuseColour` in
/// all three packages, without time ranges) has two: the row text holds 0.83
/// from 0.33 to 0.67 s and 0.10 from 1.00 to 1.33 s, while the cursor pill
/// (`pill_glow_txtrd_3`, same 1.67 s animations) is 0.10 at 0.67 s and eases
/// from orange at 1.00 s to 0.60 at 1.33 s. 0x54D270 sends each row
/// (`list.%d`, `list.%d.cursor`) the state `player`, `friends` or
/// `deactivate`. [I] The first window ends in `deactivate` (light text on the
/// grey panel) and the second in `player` (dark text on the lit pill); which
/// key range each state plays was not traced.
fn hold_ends(a: &osd::Animation) -> Vec<f32> {
    let mut spans: Vec<(f32, f32)> = a
        .channels
        .iter()
        .filter(|c| c.semantic == osd::Semantic::Parameter && c.target.ends_with("#DiffuseColour"))
        .flat_map(|c| &c.components)
        .filter(|curve| curve.0.iter().any(|k| (k.value - curve.0[0].value).abs() > 1e-4))
        .flat_map(|curve| {
            curve.0.windows(2).filter(|w| w[1].time > w[0].time && (w[1].value - w[0].value).abs() < 1e-4).map(|w| (w[0].time, w[1].time))
        })
        .collect();
    spans.sort_by(|x, y| x.0.total_cmp(&y.0));
    let mut out: Vec<(f32, f32)> = Vec::new();
    for (start, end) in spans {
        match out.last_mut() {
            Some(last) if start <= last.1 + 1e-3 => last.1 = last.1.max(end),
            _ => out.push((start, end)),
        }
    }
    out.into_iter().map(|w| w.1).collect()
}

/// The DiffuseColour of each material `a` colours at the end of its second
/// and first held window ([`hold_ends`]): the `player` and `deactivate` row
/// states, by material id.
fn animation_states(a: &osd::Animation) -> Vec<(&str, [Option<Vec3>; 2])> {
    let ends = hold_ends(a);
    if ends.is_empty() {
        return Vec::new();
    }
    let at = |c: &osd::Channel, t: Option<&f32>| t.map(|&t| Vec3::new(c.components[0].sample(t), c.components[1].sample(t), c.components[2].sample(t)));
    a.channels
        .iter()
        .filter(|c| c.semantic == osd::Semantic::Parameter && c.components.len() >= 3)
        .filter_map(|c| c.target.split_once('#').filter(|(_, p)| *p == "DiffuseColour").map(|(id, _)| (id, c)))
        .map(|(id, c)| (id, [at(c, ends.get(1)), at(c, ends.first())]))
        .collect()
}

/// [`animation_states`] of `s` by material: from the shortest animation that
/// gives both states, else the shortest that gives one. [I] The 8.63 s
/// animation of fePersistentDatabase keys dozens of components together, and
/// their merged windows end at other times (the cursor pill would read 0.2 in
/// both states); the 1.67 s state animations of each component do not.
fn state_colours<'s>(s: &'s osd::Scene) -> HashMap<&'s str, [Option<Vec3>; 2]> {
    type Ranked = ((bool, f32), [Option<Vec3>; 2]);
    let mut best: HashMap<&str, Ranked> = HashMap::new();
    for a in &s.animations {
        let length = a.duration();
        for (id, states) in animation_states(a) {
            let rank = (states[0].is_none(), length);
            let e = best.entry(id).or_insert((rank, states));
            if rank < e.0 {
                *e = (rank, states);
            }
        }
    }
    best.into_iter().map(|(id, (_, states))| (id, states)).collect()
}

fn layout(scenes: &[&osd::Scene], def: &Def) -> Result<Layout, String> {
    let s0 = scenes[0];
    let find = |name: &str| s0.nodes.iter().position(|n| n.name() == name).ok_or(format!("no {name}"));
    let (screen, camera, target) = (find(def.screen)?, find(CAMERA)?, find(TARGET)?);
    let anim = s0
        .animations
        .iter()
        .position(|a| {
            a.channels.iter().any(|c| targets(c, &s0.nodes[camera])) && a.channels.iter().any(|c| targets(c, &s0.nodes[screen]))
        })
        .ok_or("no animation moves the camera with the screen")?;
    let a = &s0.animations[anim];
    let pose = pose(s0, a, screen, camera, target);
    let cam = s0.cameras.iter().find(|c| c.node == camera);
    // Scene-0 nodes the pose animation moves: the screen and its ancestors.
    let mut chain = HashSet::new();
    let mut i = Some(screen);
    while let Some(n) = i {
        chain.insert(n);
        i = s0.nodes[n].parent;
    }
    let slides: Vec<HashMap<&str, &osd::Channel>> = scenes
        .iter()
        .enumerate()
        .map(|(si, s)| {
            let mut m = HashMap::new();
            for (ai, an) in s.animations.iter().enumerate() {
                if si == 0 && ai == anim {
                    continue;
                }
                for c in an.channels.iter().filter(|c| c.semantic == osd::Semantic::Translation) {
                    m.entry(c.target.as_str()).or_insert(c);
                }
            }
            m
        })
        .collect();
    // Translation channels of animations with their own `enter` or `exit`.
    let moving: Vec<HashMap<&str, (&osd::Animation, &osd::Channel)>> = scenes
        .iter()
        .enumerate()
        .map(|(si, s)| {
            let mut m = HashMap::new();
            for (ai, an) in s.animations.iter().enumerate() {
                if (si == 0 && ai == anim) || (an.event("enter").is_none() && an.event("exit").is_none()) {
                    continue;
                }
                for c in an.channels.iter().filter(|c| c.semantic == osd::Semantic::Translation) {
                    m.entry(c.target.as_str()).or_insert((an, c));
                }
            }
            m
        })
        .collect();
    let local = |si: usize, n: usize| -> [f32; 16] {
        let node = &scenes[si].nodes[n];
        if si == 0 && chain.contains(&n) {
            let channels: Vec<&osd::Channel> = a.channels.iter().filter(|c| targets(c, node)).collect();
            return animated(&node.local, &channels, pose.time);
        }
        let channel = slides[si].get(node.id.as_str()).or_else(|| slides[si].get(node.name()));
        let saved = Vec3::new(node.local[12], node.local[13], node.local[14]);
        match channel.and_then(|c| slide(c, saved)) {
            Some(p) => {
                let mut m = node.local;
                m[12..15].copy_from_slice(&p.to_array());
                m
            }
            None => node.local,
        }
    };
    let parent_world = {
        let mut w = IDENTITY;
        let mut up = Vec::new();
        let mut i = s0.nodes[screen].parent;
        while let Some(n) = i {
            up.push(n);
            i = s0.nodes[n].parent;
        }
        for &n in up.iter().rev() {
            w = grid_formats::scene::compose(&local(0, n), &w);
        }
        w
    };
    let placed = osd_glyph::expand_with(scenes, 0, screen, &parent_world, &local);
    let (strip, strip_index) = match Strip::find(scenes, &placed, &parent_world) {
        Some((s, index)) => (Some(s), index),
        None => (None, HashMap::new()),
    };
    let name = |p: &osd_glyph::Placed| scenes[p.scene].nodes[p.node].name();
    let at = |n: &str| placed.iter().find(|p| name(p) == n).map(|p| Vec3::new(p.world[12], p.world[13], p.world[14]));
    let pitch = match (at(&format!("{}0", def.rows)), at(&format!("{}1", def.rows))) {
        (Some(a), Some(b)) => b - a,
        _ => return Err(format!("{}: no row pitch", def.screen)),
    };
    let mut skipped: HashMap<&'static str, usize> = HashMap::new();
    let mut materials = Vec::new();
    let mut material_index: HashMap<(usize, usize), usize> = HashMap::new();
    let states: Vec<HashMap<&str, [Option<Vec3>; 2]>> = scenes.iter().map(|s| state_colours(s)).collect();
    let mut items = Vec::new();
    let mut motions = Vec::new();
    // Every animated ancestor moves the subtree; translations alone leave the
    // linear part of the world matrices as they are, so the moves add up.
    let mut motion_of: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut motion = |q: usize| -> Vec<usize> {
        let mut path = Vec::new();
        let mut at = Some(q);
        let mut known = Vec::new();
        while let Some(i) = at {
            if let Some(m) = motion_of.get(&i) {
                known = m.clone();
                break;
            }
            path.push(i);
            at = placed[i].parent;
        }
        for &i in path.iter().rev() {
            let pl = &placed[i];
            let node = &scenes[pl.scene].nodes[pl.node];
            if let Some(&(an, c)) = moving[pl.scene].get(node.id.as_str()).or_else(|| moving[pl.scene].get(node.name())) {
                let saved = Vec3::new(node.local[12], node.local[13], node.local[14]);
                let enter = an.event("enter").cloned();
                // The frame the screen rests at: the slide key the layout put
                // the node at; else where an `enter` that sets the frame
                // leaves it; else the key nearest the saved pose (`text=team`
                // of Anim!59: an instant `enter`, then `exit` from frame 1) [I].
                let moves = |e: &osd::Event| e.steps.iter().any(|s| !matches!(s, osd::Step::Trigger(_)));
                let at = |t: f32| Vec3::from_array(std::array::from_fn(|k| c.components.get(k).map_or(saved[k], |k| k.sample(t))));
                let held = match (slide(c, saved), enter.as_ref().filter(|e| moves(e))) {
                    (Some(_), _) => c.components[0].0[1].time,
                    (None, Some(e)) => e.at(an.default_frame, f32::MAX).0,
                    (None, None) => c
                        .components
                        .first()
                        .and_then(|k| k.0.iter().map(|k| k.time).min_by(|a, b| at(*a).distance(saved).total_cmp(&at(*b).distance(saved))))
                        .unwrap_or(an.default_frame),
                };
                motions.push(Motion {
                    channel: c.clone(),
                    parent: pl.parent.map_or(Mat4::from_cols_array(&parent_world), |r| Mat4::from_cols_array(&placed[r].world)),
                    rest: slide(c, saved).unwrap_or(saved),
                    enter,
                    exit: an.event("exit").cloned(),
                    held,
                });
                known.push(motions.len() - 1);
            }
            motion_of.insert(i, known.clone());
        }
        known
    };
    for (pi, p) in placed.iter().enumerate() {
        let node = &scenes[p.scene].nodes[p.node];
        let Some(render) = &node.render else {
            continue;
        };
        let mut names = vec![node.name()];
        let mut up = p.parent;
        while let Some(q) = up {
            names.push(name(&placed[q]));
            up = placed[q].parent;
        }
        let g = gate(&names, def);
        if g.hidden || g.variant == Variant::Never {
            *skipped.entry("gated").or_default() += 1;
            continue;
        }
        let m = &scenes[p.scene].materials[render.material];
        let (Some(family), Some(values)) = (fe3d::family(&m.effect), fe3d::values(m)) else {
            *skipped.entry("effect").or_default() += 1;
            continue;
        };
        let shape = match &node.text {
            Some(t) => match strip_index.contains_key(&pi).then_some(Field::Pill).or_else(|| field(&names, &g, def)) {
                Some(field) => Shape::Text { layout: t.clone(), field },
                None => {
                    *skipped.entry("text").or_default() += 1;
                    continue;
                }
            },
            None => Shape::Static(render.mesh.clone()),
        };
        let material = *material_index.entry((p.scene, render.material)).or_insert_with(|| {
            let name = |n: &str| m.texture(n).map(str::to_owned);
            materials.push(MaterialDef {
                values,
                sharp: m.texture("TR1").is_some_and(|t| t.contains("sharp")),
                dirt: name("TDirt1"),
                pattern: name("TPattern"),
                diffuse: name("TDiffuseMap"),
                states: states[p.scene].get(m.id.as_str()).copied().unwrap_or_default(),
                events: event_values(scenes[p.scene], m.id.as_str(), values),
            });
            materials.len() - 1
        });
        items.push(Item {
            world: Mat4::from_cols_array(&p.world),
            material,
            family,
            bounds: node.bounds,
            shape,
            gate: g,
            motion: motion(pi),
            strip: strip_index.get(&pi).map(|&f| (f, strip.as_ref().and_then(|s| s.template(f)).unwrap_or(0))),
        });
    }
    Ok(Layout {
        pose,
        near: cam.map_or(0.1, |c| c.near).max(0.1),
        far: cam.map_or(10000., |c| c.far),
        items,
        materials,
        pitch,
        motions,
        strip,
        skipped,
    })
}

/// The values `id`'s parameters take once the events `activate` and
/// `deactivate` of an animation of `s` have played: the pills' states
/// (`matinst=pill_glow_txtrd_2`: orange (1, 0.79, 0) with glow 0.2 after
/// `activate` PlayTo 1.0, 0.1 without glow after `deactivate` PlayTo 0.667;
/// the label `text_pill_optiontitles` 0.1 and 0.83) [D keys and events; I:
/// the strip's items take these states, the lit pill `activate`]. From the
/// shortest such animation (the first of equal ones); `None` where none
/// keys the material.
fn event_values(s: &osd::Scene, id: &str, base: fe3d::Values) -> [Option<fe3d::Values>; 2] {
    let mut best: Option<(f32, [Option<fe3d::Values>; 2])> = None;
    for a in &s.animations {
        let channels: Vec<(&str, &osd::Channel)> = a
            .channels
            .iter()
            .filter(|c| c.semantic == osd::Semantic::Parameter)
            .filter_map(|c| c.target.split_once('#').filter(|(m, _)| *m == id).map(|(_, p)| (p, c)))
            .collect();
        let (Some(on), Some(off)) = (a.event("activate"), a.event("deactivate")) else {
            continue;
        };
        if channels.is_empty() {
            continue;
        }
        let at = |e: &osd::Event| {
            let frame = e.at(a.default_frame, f32::MAX).0;
            let mut v = base;
            for (p, c) in &channels {
                let x: Vec<f32> = c.components.iter().map(|k| k.sample(frame)).collect();
                set_parameter(&mut v, p, &x);
            }
            v
        };
        let length = a.duration();
        if best.as_ref().is_none_or(|b| length < b.0) {
            best = Some((length, [Some(at(on)), Some(at(off))]));
        }
    }
    best.map_or([None; 2], |b| b.1)
}

/// Sets the parameter `name` of `v` (the names [`fe3d::values`] reads).
fn set_parameter(v: &mut fe3d::Values, name: &str, x: &[f32]) {
    let Some(&first) = x.first() else {
        return;
    };
    match name {
        "DiffuseColour" if x.len() >= 3 => v.colour = Vec3::new(x[0], x[1], x[2]).extend(v.colour.w),
        "Alpha" => v.colour.w = first,
        "GlowAmount" => v.a.x = first,
        "Emmisive" => v.a.z = first,
        "SpecularLevel" => v.a.w = first,
        "FresnelMin" => v.b.x = first,
        "FresnelMax" => v.b.y = first,
        "FresnelPower" => v.b.z = first,
        "SpecularPow" => v.b.w = first,
        "DirtScale" => v.c.x = first,
        "DirtBlend" => v.c.y = first,
        "DirtGlowBlend" => v.c.z = first,
        "PatternScale" => v.c.w = first,
        "PatternBlend" => v.d.x = first,
        _ => {}
    }
}

// ------------------------------------------------------------------ pills

/// Pills a strip shows at most: states.xml's itemflow of the results
/// screens names `pills.0` to `pills.3` [D].
const MAX_PILLS: usize = 4;

/// How a node of the pill strip lays out its children.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Flow {
    Plain,
    /// `stacker_left` (GlyphStackerLeft, vtable 0xCBE810): the children
    /// along +x in the order of their load-time positions, each gap as at
    /// load (load 0x4AA580, re-flow 0x498DF0) [D].
    Stacker,
    /// `scaler_x` (GlyphScalerX, vtable 0xCBD2A0): stretches its `scalee`
    /// child along x over its other children (0x48C090) [D].
    Scaler,
    Scalee,
}

fn flow_kind(name: &str) -> Flow {
    if name.starts_with("stacker_left=") {
        Flow::Stacker
    } else if name.starts_with("scaler_x=") {
        Flow::Scaler
    } else if name.starts_with("scalee=") {
        Flow::Scalee
    } else {
        Flow::Plain
    }
}

#[derive(Clone, Debug)]
struct FlowNode {
    flow: Flow,
    parent: Option<usize>,
    /// By load-time position (a stacker's order).
    children: Vec<usize>,
    /// In the parent's frame.
    local: Mat4,
    /// The mesh's x span in the node's frame (render nodes only).
    own: Option<[f32; 2]>,
    /// A text node's layout and bounds: its label's span replaces `own`.
    text: Option<(osd::TextLayout, [f32; 6])>,
    /// A stacker's gap after each child (the last 0) and its start.
    gaps: Vec<f32>,
    start: f32,
}

/// A pill label's x span from its text layout, bounds and text.
type LabelSpan<'a> = dyn Fn(&osd::TextLayout, &[f32; 6], &str) -> Option<[f32; 2]> + 'a;

/// The pill strip of a screen: a `stacker_left` whose children are numbered
/// pill templates (`stacker_left=pill_stack` with `component/h_pill=0` and
/// `=1` on both results screens), with its subtree. The stacker message
/// SetSize (0x4AA940) shows the first `count` children and clones the last
/// (0x4AA7F0, every gap between pills then the template's) [D]; states.xml's
/// itemflow sizes it to the screen's pills [I: the post-race sender was not
/// found]. Each pill is `stacker_left=pill_build` of a left cap, a
/// `scaler_x` middle and a right cap; the middle's `scalee=center` stretches
/// over the label `text=pill_header`.
#[derive(Clone, Debug)]
struct Strip {
    /// The root stacker first, then its subtree in placed order.
    nodes: Vec<FlowNode>,
    /// The world matrix of the root's parent.
    base: Mat4,
}

/// Orders a stacker's children by their load-time positions and keeps the
/// gap after each and the first one's position (0x4AA580: pos = bound min,
/// size = max - min, gap = next pos - pos - size, the last 0) [D]. `spans`
/// are the children's x spans in the stacker's frame.
fn sort_stacker(n: &mut FlowNode, spans: &[Option<[f32; 2]>]) {
    let pos = |c: usize| spans[c].map_or(f32::MAX, |s| s[0]);
    n.children.sort_by(|&a, &b| pos(a).total_cmp(&pos(b)));
    let shown: Vec<[f32; 2]> = n.children.iter().filter_map(|&c| spans[c]).collect();
    n.start = shown.first().map_or(0., |s| s[0]);
    n.gaps = vec![0.; n.children.len()];
    let mut k = 0;
    for (i, &c) in n.children.iter().enumerate() {
        if spans[c].is_some() {
            n.gaps[i] = shown.get(k + 1).map_or(0., |next| next[0] - shown[k][1]);
            k += 1;
        }
    }
}

/// The x span `e` of a child in its parent's frame through `local`.
fn through(local: &Mat4, e: [f32; 2]) -> [f32; 2] {
    let (a, b) = (local.w_axis.x + local.x_axis.x * e[0], local.w_axis.x + local.x_axis.x * e[1]);
    [a.min(b), a.max(b)]
}

fn union(a: Option<[f32; 2]>, b: Option<[f32; 2]>) -> Option<[f32; 2]> {
    match (a, b) {
        (Some(a), Some(b)) => Some([a[0].min(b[0]), a[1].max(b[1])]),
        (a, b) => a.or(b),
    }
}

/// A pill template's number (`component/h_pill=1` is 1).
fn numbered(name: &str) -> Option<usize> {
    name.rsplit_once('=').and_then(|(_, k)| k.parse().ok())
}

impl Strip {
    /// The strip among `placed` ([`Strip`]) with the [`Strip::nodes`] entry
    /// of each placed node in it.
    fn find(scenes: &[&osd::Scene], placed: &[osd_glyph::Placed], parent_world: &[f32; 16]) -> Option<(Strip, HashMap<usize, usize>)> {
        let node = |i: usize| &scenes[placed[i].scene].nodes[placed[i].node];
        let children = |i: usize| placed.iter().enumerate().filter(move |(_, q)| q.parent == Some(i)).map(|(c, _)| c);
        let root = (0..placed.len()).find(|&i| {
            flow_kind(node(i).name()) == Flow::Stacker
                && children(i).next().is_some()
                && children(i).all(|c| numbered(node(c).name()).is_some())
        })?;
        // `expand` places a node before its children.
        let mut index = HashMap::from([(root, 0)]);
        let mut order = vec![root];
        for (i, p) in placed.iter().enumerate().skip(root + 1) {
            if p.parent.is_some_and(|q| index.contains_key(&q)) {
                index.insert(i, order.len());
                order.push(i);
            }
        }
        let world = |i: usize| Mat4::from_cols_array(&placed[i].world);
        let base = placed[root].parent.map_or(Mat4::from_cols_array(parent_world), world);
        let mut nodes: Vec<FlowNode> = order
            .iter()
            .map(|&i| {
                let n = node(i);
                let up = if i == root { base } else { placed[i].parent.map_or(base, world) };
                FlowNode {
                    flow: flow_kind(n.name()),
                    parent: if i == root { None } else { placed[i].parent.and_then(|q| index.get(&q).copied()) },
                    children: Vec::new(),
                    local: up.inverse() * world(i),
                    own: n.render.is_some().then_some([n.bounds[0], n.bounds[3]]),
                    text: n.text.clone().map(|t| (t, n.bounds)),
                    gaps: Vec::new(),
                    start: 0.,
                }
            })
            .collect();
        for f in 1..nodes.len() {
            if let Some(p) = nodes[f].parent {
                nodes[p].children.push(f);
            }
        }
        // The load-time extents (children after parents), then each
        // stacker's order, gaps and start (0x4AA580; pos = bound min, size =
        // max - min, gap = next pos - pos - size).
        let mut extent: Vec<Option<[f32; 2]>> = vec![None; nodes.len()];
        for f in (0..nodes.len()).rev() {
            let e = nodes[f].children.iter().fold(nodes[f].own, |e, &c| union(e, extent[c].map(|x| through(&nodes[c].local, x))));
            extent[f] = e;
        }
        let spans: Vec<Option<[f32; 2]>> = (0..nodes.len()).map(|c| extent[c].map(|e| through(&nodes[c].local, e))).collect();
        for n in nodes.iter_mut().filter(|n| n.flow == Flow::Stacker) {
            sort_stacker(n, &spans);
        }
        let map = order.iter().enumerate().map(|(f, &i)| (i, f)).collect();
        Some((Strip { nodes, base }, map))
    }

    /// The pill template (the root's child, in clone order) node `f` is in.
    fn template(&self, mut f: usize) -> Option<usize> {
        while let Some(p) = self.nodes[f].parent {
            if p == 0 {
                return self.nodes[0].children.iter().position(|&c| c == f);
            }
            f = p;
        }
        None
    }

    /// Lays out the subtree of `f` for `label`: the new local of each node
    /// under it in `locals`, and its x span in its own frame. `span` gives a
    /// text node's span for the label.
    fn flow(&self, f: usize, label: &str, span: &LabelSpan, locals: &mut [Option<Mat4>]) -> Option<[f32; 2]> {
        let n = &self.nodes[f];
        let own = match &n.text {
            Some((t, b)) => span(t, b, label).or(n.own),
            None => n.own,
        };
        match n.flow {
            Flow::Stacker => {
                // Re-flow 0x498DF0: each shown child's position moves to the
                // cursor, which advances by its size and gap.
                let mut cursor = n.start;
                let mut extent = own;
                for (i, &c) in n.children.iter().enumerate() {
                    let mut l = self.nodes[c].local;
                    if let Some(e) = self.flow(c, label, span, locals) {
                        l.w_axis.x += cursor - through(&l, e)[0];
                        let s = through(&l, e);
                        cursor = s[1] + n.gaps[i];
                        extent = union(extent, Some(s));
                    }
                    locals[c] = Some(l);
                }
                extent
            }
            Flow::Scaler => {
                // 0x48C090: B, the other children's span; S, the scalee's own;
                // scale = |B| / |S|; the scalee's local = T(B.min - S.min *
                // scale, saved y, saved z) * diag(scale, 1, 1); no margins in
                // the packages [D].
                let scalee = n.children.iter().copied().find(|&c| self.nodes[c].flow == Flow::Scalee);
                let mut b = None;
                for &c in n.children.iter().filter(|&&c| Some(c) != scalee) {
                    let l = self.nodes[c].local;
                    b = union(b, self.flow(c, label, span, locals).map(|e| through(&l, e)));
                    locals[c] = Some(l);
                }
                let mut extent = union(own, b);
                if let Some(s) = scalee {
                    let saved = self.nodes[s].local;
                    let e = self.flow(s, label, span, locals);
                    locals[s] = Some(match (b, e) {
                        (Some(b), Some(e)) if e[1] - e[0] > 1e-6 => {
                            let scale = (b[1] - b[0]) / (e[1] - e[0]);
                            let t = saved.w_axis.truncate();
                            Mat4::from_translation(Vec3::new(b[0] - e[0] * scale, t.y, t.z)) * Mat4::from_scale(Vec3::new(scale, 1., 1.))
                        }
                        _ => {
                            extent = union(extent, e.map(|e| through(&saved, e)));
                            saved
                        }
                    });
                }
                extent
            }
            Flow::Plain | Flow::Scalee => {
                let mut extent = own;
                for &c in &n.children {
                    let l = self.nodes[c].local;
                    extent = union(extent, self.flow(c, label, span, locals).map(|e| through(&l, e)));
                    locals[c] = Some(l);
                }
                extent
            }
        }
    }

    /// The world matrix of each node of each pill for `labels` (at most
    /// [`MAX_PILLS`]; `None` outside the pill's template), pill `k` a copy of
    /// template `min(k, last)`; a pill without label is hidden.
    fn place(&self, labels: &[String], span: &LabelSpan) -> Vec<Vec<Option<Mat4>>> {
        let root = &self.nodes[0];
        let m = root.children.len();
        let root_world = self.base * root.local;
        let mut cursor = root.start;
        let mut out = Vec::new();
        for (k, label) in labels.iter().enumerate().take(MAX_PILLS) {
            let mut worlds = vec![None; self.nodes.len()];
            if m == 0 || label.trim().is_empty() {
                out.push(worlds);
                continue;
            }
            let t = root.children[k.min(m - 1)];
            let mut locals = vec![None; self.nodes.len()];
            let mut l = self.nodes[t].local;
            if let Some(e) = self.flow(t, label, span, &mut locals) {
                l.w_axis.x += cursor - through(&l, e)[0];
                // A clone's gap is the template's between its last two.
                cursor = through(&l, e)[1] + root.gaps.get(k.min(m.saturating_sub(2))).copied().unwrap_or(0.);
            }
            locals[t] = Some(l);
            for f in 1..self.nodes.len() {
                if let (Some(l), Some(p)) = (locals[f], self.nodes[f].parent) {
                    let up = if p == 0 { Some(root_world) } else { worlds[p] };
                    worlds[f] = up.map(|w| w * l);
                }
            }
            out.push(worlds);
        }
        out
    }
}

// ------------------------------------------------------------------ loading

struct Prepared {
    /// Per screen, in the order of [`SCREENS`].
    layouts: Vec<Layout>,
    /// Bundle textures by name.
    images: HashMap<String, Image>,
    cubes: [Image; 2],
    fonts: HashMap<String, osd_scene::Font>,
    /// Per screen, the Bevy mesh of each static item.
    meshes: Vec<Vec<Option<Mesh>>>,
    lights: fe3d::Lights,
}

/// A cube of the OSD package with raw (gamma-space) texels, as fe3d reads them.
pub(crate) fn cube(c: &osd::Cubemap<'_>) -> Image {
    let mut data = Vec::new();
    for face in c.faces {
        data.extend(crate::texture_upload::decode_levels(face, c.size, c.size, 1, false).unwrap_or_default());
    }
    let mut image = Image::new(
        Extent3d { width: c.size, height: c.size, depth_or_array_layers: 6 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });
    image.sampler = ImageSampler::linear();
    image
}

fn prepare(root: &Path) -> Result<Prepared, String> {
    let read = |p: &str| std::fs::read(root.join(p)).map_err(|e| format!("{p}: {e}"));
    let bytes: Vec<Vec<u8>> = PACKAGES.iter().map(|p| read(p)).collect::<Result<_, _>>()?;
    let trees: Vec<pssg::Tree> =
        bytes.iter().zip(PACKAGES).map(|(b, p)| pssg::read_tree(b).map_err(|e| format!("{p}: {e}"))).collect::<Result<_, _>>()?;
    let scenes: Vec<osd::Scene> =
        trees.iter().zip(PACKAGES).map(|(t, p)| osd::read_scene(t).map_err(|e| format!("{p}: {e}"))).collect::<Result<_, _>>()?;
    let refs: Vec<&osd::Scene> = scenes.iter().collect();
    let layouts: Vec<Layout> = SCREENS.iter().map(|s| layout(&refs, s.def())).collect::<Result<_, _>>()?;
    // Bundle textures.
    let wanted: HashSet<&str> = layouts
        .iter()
        .flat_map(|l| &l.materials)
        .flat_map(|m| [&m.dirt, &m.pattern, &m.diffuse])
        .filter_map(|t| t.as_deref())
        .collect();
    let bundle_bytes = read(BUNDLE)?;
    let bundle = pssg::read_tree(&bundle_bytes).map_err(|e| format!("{BUNDLE}: {e}"))?;
    let ids: Vec<&[u8]> = wanted.iter().map(|w| w.as_bytes()).collect();
    let mut images = HashMap::new();
    for t in texture::read_named(&bundle, &ids).map_err(|e| format!("{BUNDLE}: {e}"))? {
        let wrap = texture::wrap(&bundle, t.id).unwrap_or([texture::Wrap::Repeat; 2]);
        let (dds, _) = t.validated_dds();
        let image = crate::texture_upload::image_linear(&dds, osd_scene::sampler(wrap), RenderAssetUsages::RENDER_WORLD)
            .map_err(|e| format!("{}: {e}", String::from_utf8_lossy(t.id)))?;
        images.insert(String::from_utf8_lossy(t.id).into_owned(), image);
    }
    // Reflection cubes.
    let osd_bytes = read(CUBES)?;
    let osd_tree = pssg::read_tree(&osd_bytes).map_err(|e| format!("{CUBES}: {e}"))?;
    let osd_scene = osd::read_scene(&osd_tree).map_err(|e| format!("{CUBES}: {e}"))?;
    let cube_of = |id: &str| osd_scene.cubemaps.iter().find(|c| c.id == id).map(cube).ok_or(format!("{CUBES}: no {id}"));
    let cubes = [cube_of("env_soft.dds")?, cube_of("env_sharp.dds")?];
    // Fonts.
    let mut fonts = HashMap::new();
    for item in layouts.iter().flat_map(|l| &l.items) {
        if let Shape::Text { layout: t, .. } = &item.shape
            && !fonts.contains_key(&t.font)
        {
            let path = ["PSSG", "pssg"]
                .iter()
                .map(|ext| PathBuf::from(format!("frontend/fonts/{}.{ext}", t.font)))
                .find(|p| root.join(p).is_file())
                .ok_or(format!("font {} missing", t.font))?;
            let bytes = std::fs::read(root.join(&path)).map_err(|e| format!("{}: {e}", path.display()))?;
            let tree = pssg::read_tree(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
            fonts.insert(t.font.clone(), osd_scene::font(osd::read_font(&tree).map_err(|e| format!("{}: {e}", path.display()))?));
        }
    }
    let meshes = layouts
        .iter()
        .map(|l| {
            l.items
                .iter()
                .map(|i| match &i.shape {
                    Shape::Static(m) => Some(fe3d::mesh(m, &i.bounds, i.family)),
                    Shape::Text { .. } => None,
                })
                .collect()
        })
        .collect();
    let xml = String::from_utf8_lossy(&read(PARAMS)?).into_owned();
    let lights = fe3d::Lights::read(&xml, "ingame", Some("leaderboards")).unwrap_or_default();
    Ok(Prepared { layouts, images, cubes, fonts, meshes, lights })
}

#[derive(Resource)]
struct Loading(Arc<Mutex<Option<Result<Prepared, String>>>>);

/// Starts loading the art in the background; the post-race panel draws the
/// results itself until (and unless) it is ready.
pub fn install(app: &mut App, root: &Path) {
    app.init_resource::<Shown>();
    if crate::settings::diagnostics().results_shot.is_some() {
        app.add_systems(Last, shot);
    }
    if crate::settings::diagnostics().results_art_off {
        return;
    }
    let slot = Arc::new(Mutex::new(None));
    let fill = slot.clone();
    let root = root.to_path_buf();
    std::thread::spawn(move || {
        let result = prepare(&root);
        if let Ok(mut s) = fill.lock() {
            *s = Some(result);
        }
    });
    fe3d::plugin(app);
    osd_scene::display_plugin(app);
    app.insert_resource(Loading(slot))
        .init_resource::<Ready>()
        .add_systems(Update, spawn)
        .add_systems(PostUpdate, (resize, update).chain().before(TransformSystems::Propagate))
        .add_systems(
            PostUpdate,
            drop_sun_cascades
                .after(SimulationLightSystems::UpdateDirectionalLightCascades)
                .before(SimulationLightSystems::UpdateLightFrusta),
        );
}

/// Diagnostics `results_shot`: one window shot of the results screen, that
/// long after its board first shows with the art ready (or with the art off).
fn shot(
    mut commands: Commands,
    shown: Res<Shown>,
    ready: Option<Res<Ready>>,
    time: Res<Time<Real>>,
    mut since: Local<Option<f64>>,
    mut done: Local<bool>,
) {
    use bevy::render::view::screenshot::{Screenshot, save_to_disk};
    let Some(after) = crate::settings::diagnostics().results_shot else {
        return;
    };
    let art = ready.as_ref().map(|r| r.0);
    if *done || shown.board.is_none() || art == Some(false) {
        return;
    }
    let now = time.elapsed_secs_f64();
    let start = *since.get_or_insert(now);
    if now - start >= after {
        *done = true;
        let path = crate::settings::data_dir().join("window-shot-results.png");
        println!(
            "results_shot path={} art={} rows={} screen={:?}",
            path.display(),
            if art.is_some() { "on" } else { "off" },
            shown.board.as_ref().map_or(0, |b| b.view.table.len().saturating_sub(1)),
            shown.board.as_ref().map(|b| b.screen).unwrap_or_default()
        );
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
    }
}

// ------------------------------------------------------------------ live

#[derive(Component)]
struct ResultsCamera;

/// One drawn copy of a layout item of `screen` (`slot`: its row in the
/// window; `text`: what its mesh spells).
#[derive(Component)]
struct Art {
    screen: Screen,
    item: usize,
    slot: Option<usize>,
    text: Option<String>,
}

/// A screen's items, materials and camera.
struct LiveScreen {
    items: Vec<Item>,
    /// Per material: the row states `player` and `deactivate`, then the
    /// states after the events `activate` and `deactivate` (each the authored
    /// values when unanimated).
    materials: Vec<[Handle<Fe3dMaterial>; 4]>,
    camera: Transform,
    fov_degrees: f32,
    near: f32,
    far: f32,
    motions: Vec<Motion>,
    pitch: Vec3,
    strip: Option<Strip>,
    /// The strip's node worlds per pill for the drawn board ([`Strip::place`]).
    pills: Vec<Vec<Option<Mat4>>>,
}

impl LiveScreen {
    /// Where the layout puts `item` in row (or pill) `slot`; `None` for a
    /// pill node the board's pills do not place.
    fn rest(&self, item: &Item, slot: Option<usize>) -> Option<Mat4> {
        match item.strip {
            Some((f, _)) => slot.and_then(|k| self.pills.get(k)).and_then(|p| p[f]),
            None => Some(Mat4::from_translation(self.pitch * slot.unwrap_or(0) as f32) * item.world),
        }
    }
}

#[derive(Resource)]
struct Live {
    /// Per screen, in the order of [`SCREENS`].
    screens: Vec<LiveScreen>,
    fonts: HashMap<String, osd_scene::Font>,
    lights: fe3d::Lights,
    pulse: fe3d::Pulse,
    camera_world: Mat4,
    camera: Entity,
    nodes: [Entity; 2],
    image: Handle<Image>,
    display: Handle<OsdDisplay>,
    size: UVec2,
    /// The board the entities were last set up for.
    board: Option<Board>,
    /// The event the drawn screen plays and how long it has run.
    play: Option<(Play, f32)>,
}

/// A white texel for a texture slot whose image the bundle lacks.
pub(crate) fn white() -> Image {
    Image::new(
        Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        TextureDimension::D2,
        vec![255; 4],
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    )
}

#[allow(clippy::too_many_arguments)]
fn spawn(
    mut commands: Commands,
    loading: Option<Res<Loading>>,
    viewer: Query<Entity, With<crate::ViewerCamera>>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<Fe3dMaterial>>,
    mut displays: ResMut<Assets<OsdDisplay>>,
    mut ready: ResMut<Ready>,
) {
    let (Some(loading), Some(viewer)) = (loading, viewer.iter().next()) else {
        return;
    };
    let Some(result) = loading.0.lock().ok().and_then(|mut s| s.take()) else {
        return;
    };
    commands.remove_resource::<Loading>();
    let p = match result {
        Ok(p) => p,
        Err(e) => {
            log_line!("results_art version=1 loaded=false error={e:?}");
            return;
        }
    };
    for (screen, l) in SCREENS.iter().zip(&p.layouts) {
        let mut skipped: Vec<String> = l.skipped.iter().map(|(k, v)| format!("{k}:{v}")).collect();
        skipped.sort();
        log_line!(
            "results_art version=1 loaded=true items={} materials={} textures={} pose_time={:.3} eye={:.1},{:.1},{:.1} fov={:.2} skipped={} screen={screen:?}",
            l.items.len(),
            l.materials.len(),
            p.images.len(),
            l.pose.time,
            l.pose.eye.x,
            l.pose.eye.y,
            l.pose.eye.z,
            l.pose.fov_degrees,
            skipped.join(",")
        );
    }
    let white = images.add(white());
    let [soft, sharp] = p.cubes.map(|c| images.add(c));
    let named: HashMap<String, Handle<Image>> = p.images.into_iter().map(|(k, v)| (k, images.add(v))).collect();
    let texture = |n: &Option<String>| n.as_ref().and_then(|n| named.get(n)).cloned().unwrap_or_else(|| white.clone());
    let constants = p.lights.constants(None, None, 0.);
    // Per material: the authored colour (the player's row) and the
    // `deactivate` state colour (the other rows and the fixed items).
    let mut handles_of = |l: &Layout| -> Vec<[Handle<Fe3dMaterial>; 4]> {
        l.materials
        .iter()
        .map(|m| {
            let authored = Fe3dMaterial {
                reflection: if m.sharp { sharp.clone() } else { soft.clone() },
                dirt: texture(&m.dirt),
                pattern: texture(&m.pattern),
                values: m.values,
                constants,
                diffuse: texture(&m.diffuse),
            };
            let [player, rest] = m.states.map(|state| {
                state.map(|c| {
                    let mut r = authored.clone();
                    r.values.colour = c.extend(m.values.colour.w);
                    materials.add(r)
                })
            });
            let [activate, deactivate] = m.events.map(|v| {
                v.map(|values| {
                    let mut r = authored.clone();
                    r.values = values;
                    materials.add(r)
                })
            });
            let authored = materials.add(authored);
            let or = |h: Option<Handle<Fe3dMaterial>>| h.unwrap_or_else(|| authored.clone());
            [or(player), or(rest), or(activate), or(deactivate)]
        })
        .collect()
    };
    let l = &p.layouts[0];
    let image = images.add(Image::new_target_texture(16, 16, TextureFormat::Rgba8UnormSrgb, None));
    let display = displays.add(OsdDisplay::new(image.clone()));
    let transform = Transform::from_translation(l.pose.eye).looking_at(l.pose.target, Vec3::Y);
    let camera = commands
        .spawn((
            ResultsCamera,
            Camera3d::default(),
            Camera {
                order: -19,
                is_active: false,
                clear_color: ClearColorConfig::Custom(Color::NONE),
                ..default()
            },
            RenderTarget::Image(image.clone().into()),
            Projection::Perspective(PerspectiveProjection {
                fov: l.pose.fov_degrees.to_radians(),
                near: l.near,
                far: l.far,
                aspect_ratio: 16. / 9.,
                ..default()
            }),
            transform,
            Tonemapping::None,
            if cfg!(target_os = "android") { Msaa::Off } else { Msaa::Sample4 },
            RenderLayers::layer(LAYER),
        ))
        .id();
    let full = Node {
        position_type: PositionType::Absolute,
        width: percent(100.),
        height: percent(100.),
        ..default()
    };
    // The dim behind the art (the post-race panel's own, which it drops).
    let dim = commands
        .spawn((full.clone(), BackgroundColor(Color::srgba(0., 0., 0., 0.5)), GlobalZIndex(7), Visibility::Hidden, Pickable::IGNORE, UiTargetCamera(viewer)))
        .id();
    let shown = commands
        .spawn((full, MaterialNode(display.clone()), GlobalZIndex(8), Visibility::Hidden, Pickable::IGNORE, UiTargetCamera(viewer)))
        .id();
    let mut screens = Vec::new();
    for ((&screen, l), mesh_of) in SCREENS.iter().zip(&p.layouts).zip(&p.meshes) {
        let handles = handles_of(l);
        for (i, item) in l.items.iter().enumerate() {
            let mesh = mesh_of[i].clone().map(|m| meshes.add(m)).unwrap_or_default();
            // A pill node: in its template's pill, the last template's also in
            // every clone after it (SetSize 0x4AA940) [D].
            let templates = l.strip.as_ref().map_or(0, |s| s.nodes[0].children.len());
            let slots: Vec<Option<usize>> = match item.strip {
                Some((_, j)) if j + 1 < templates => vec![Some(j)],
                Some((_, j)) => (j..MAX_PILLS).map(Some).collect(),
                None if item.gate.row => (0..screen.def().visible).map(Some).collect(),
                None => vec![None],
            };
            for slot in slots {
                let world = if item.strip.is_some() { item.world } else { Mat4::from_translation(l.pitch * slot.unwrap_or(0) as f32) * item.world };
                commands.spawn((
                    Art { screen, item: i, slot, text: None },
                    Transform::from_matrix(world),
                    Visibility::Hidden,
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(handles[item.material][if item.strip.is_some() { 3 } else { 1 }].clone()),
                    RenderLayers::layer(LAYER),
                    bevy::camera::visibility::NoFrustumCulling,
                ));
            }
        }
        screens.push(LiveScreen {
            items: l.items.clone(),
            materials: handles,
            camera: Transform::from_translation(l.pose.eye).looking_at(l.pose.target, Vec3::Y),
            fov_degrees: l.pose.fov_degrees,
            near: l.near,
            far: l.far,
            motions: l.motions.clone(),
            pitch: l.pitch,
            strip: l.strip.clone(),
            pills: Vec::new(),
        });
    }
    commands.insert_resource(Live {
        screens,
        fonts: p.fonts,
        lights: p.lights,
        pulse: fe3d::Pulse::default(),
        camera_world: transform.to_matrix(),
        camera,
        nodes: [dim, shown],
        image,
        display,
        size: UVec2::ZERO,
        board: None,
        play: None,
    });
    ready.0 = true;
}

/// The art camera, apart from the art meshes.
type ArtCamera<'w, 's> =
    Query<'w, 's, (&'static mut Camera, &'static mut Transform, &'static mut Projection), (With<ResultsCamera>, Without<Art>)>;

#[allow(clippy::too_many_arguments)]
fn update(
    live: Option<ResMut<Live>>,
    shown: Res<Shown>,
    time: Option<Res<Time<Real>>>,
    mut cameras: ArtCamera,
    mut nodes: Query<&mut Visibility, Without<Art>>,
    mut art: Query<(&mut Art, &mut Visibility, &mut Mesh3d, &mut MeshMaterial3d<Fe3dMaterial>, &mut Transform)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<Fe3dMaterial>>,
) {
    let Some(mut live) = live else {
        return;
    };
    let on = shown.board.is_some();
    if let Ok((mut camera, ..)) = cameras.get_mut(live.camera)
        && camera.is_active != on
    {
        camera.is_active = on;
    }
    for node in live.nodes {
        if let Ok(mut v) = nodes.get_mut(node) {
            v.set_if_neq(if on { Visibility::Inherited } else { Visibility::Hidden });
        }
    }
    let Some(board) = &shown.board else {
        live.board = None;
        live.play = None;
        return;
    };
    let live = &mut *live;
    let dt = time.as_ref().map_or(0., |t| t.delta_secs());
    if let Some((_, t)) = &mut live.play {
        *t += dt;
    }
    // A new screen waits for the drawn one's `exit` (flow 0x4A6670) [D].
    let mut board = board;
    if let Some(drawn) = &live.board
        && drawn.screen != board.screen
    {
        let seconds = play_seconds(&live.screens[drawn.screen as usize].motions, Play::Exit);
        match live.play {
            Some((Play::Exit, t)) if t >= seconds => {
                println!("results_art exit screen={:?} seconds={seconds:.2} done", drawn.screen);
            }
            Some((Play::Exit, _)) => board = drawn,
            _ if seconds > 0. => {
                live.play = Some((Play::Exit, 0.));
                board = drawn;
            }
            _ => {}
        }
    }
    let board = &board.clone();
    if live.board.as_ref() != Some(board) {
        if live.board.as_ref().is_none_or(|d| d.screen != board.screen) {
            live.play = Some((Play::Enter, 0.));
        }
        // The pills for the board's labels, each label's span from its font.
        let fonts = &live.fonts;
        let screen = &mut live.screens[board.screen as usize];
        let span = |t: &osd::TextLayout, b: &[f32; 6], label: &str| fonts.get(&t.font).map(|f| osd_scene::text_span(f, label, t, b));
        screen.pills = screen.strip.as_ref().map(|s| s.place(&board.view.buttons, &span)).unwrap_or_default();
        let current = &live.screens[board.screen as usize];
        // The camera of the board's screen.
        if let Ok((_, mut transform, mut projection)) = cameras.get_mut(live.camera) {
            transform.set_if_neq(current.camera);
            if let Projection::Perspective(p) = projection.as_mut() {
                p.fov = current.fov_degrees.to_radians();
                p.near = current.near;
                p.far = current.far;
            }
        }
        live.camera_world = current.camera.to_matrix();
        let mut drawn = 0;
        for (mut a, mut visibility, mut mesh, mut material, mut transform) in &mut art {
            if a.screen != board.screen {
                visibility.set_if_neq(Visibility::Hidden);
                continue;
            }
            let item = &current.items[a.item];
            // The lit pill is the cursor's (`activate`), the others
            // `deactivate` [I: the strip's sender was not found].
            let state = match item.strip {
                Some(_) => 2 + usize::from(a.slot != Some(board.view.button)),
                None => usize::from(!on_player_row(a.slot, board)),
            };
            let state = &current.materials[item.material][state];
            if material.0 != *state {
                material.0 = state.clone();
            }
            let rest = current.rest(item, a.slot);
            if let Some(world) = rest {
                transform.set_if_neq(Transform::from_matrix(world));
            }
            let mut on = rest.is_some() && visible(&item.gate, if item.strip.is_some() { None } else { a.slot }, board);
            if let Shape::Text { layout, field } = &item.shape {
                let text = text_of(*field, a.slot, board);
                on &= !text.trim().is_empty();
                if on
                    && a.text.as_deref() != Some(text.as_str())
                    && let Some(font) = live.fonts.get(&layout.font)
                {
                    let m = osd_scene::text_mesh(font, &text, layout, &item.bounds);
                    mesh.0 = meshes.add(fe3d::mesh(&m, &item.bounds, item.family));
                    a.text = Some(text);
                }
            }
            visibility.set_if_neq(if on { Visibility::Inherited } else { Visibility::Hidden });
            drawn += usize::from(on);
        }
        println!(
            "results_art board rows={} drift={} drawn={drawn} screen={:?} motions={}",
            board.view.table.len().saturating_sub(1),
            board.drift,
            board.screen,
            current.motions.len()
        );
        live.board = Some(board.clone());
    }
    // The screen's own slides.
    if let Some((play, t)) = live.play {
        let current = &live.screens[board.screen as usize];
        let seconds = play_seconds(&current.motions, play);
        let frames: Vec<Vec3> = current.motions.iter().map(|m| m.offset(m.frame(play, t))).collect();
        for (a, _, _, _, mut transform) in &mut art {
            if a.screen != board.screen {
                continue;
            }
            let item = &current.items[a.item];
            if !item.motion.is_empty()
                && let Some(rest) = current.rest(item, a.slot)
            {
                let m: Vec3 = item.motion.iter().map(|&m| frames[m]).sum();
                transform.set_if_neq(Transform::from_matrix(Mat4::from_translation(m) * rest));
            }
        }
        if t >= seconds && play == Play::Enter {
            println!("results_art enter screen={:?} seconds={seconds:.2} done", board.screen);
            live.play = None;
        }
    }
    live.pulse.advance(dt, live.lights.g_rate);
    let constants = live.lights.constants(Some(&live.camera_world), None, live.pulse.value(live.lights.g_swing));
    for h in live.screens.iter().flat_map(|s| &s.materials).flatten() {
        if let Some(mut m) = materials.get_mut(h)
            && m.constants != constants
        {
            m.constants = constants;
        }
    }
}

/// Keeps the art image at the UI camera's size (as the OSD's).
fn resize(
    live: Option<ResMut<Live>>,
    viewer: Query<&Camera, (With<crate::ViewerCamera>, Without<ResultsCamera>)>,
    ui: Query<&Camera, (With<IsDefaultUiCamera>, Without<ResultsCamera>)>,
    mut images: ResMut<Assets<Image>>,
    mut displays: ResMut<Assets<OsdDisplay>>,
    mut projection: Query<&mut Projection, With<ResultsCamera>>,
) {
    let (Some(mut live), Some(camera)) = (live, ui.iter().next().or(viewer.iter().next())) else {
        return;
    };
    let Some(size) = camera.physical_target_size().filter(|s| s.x > 0 && s.y > 0) else {
        return;
    };
    if size == live.size {
        return;
    }
    live.size = size;
    if let Some(mut image) = images.get_mut(&live.image) {
        image.resize(Extent3d { width: size.x, height: size.y, depth_or_array_layers: 1 });
    }
    // A UI material rebuilds its bind group only when it changes (F737).
    if let Some(mut display) = displays.get_mut(&live.display) {
        display.touch();
    }
    if let Ok(mut p) = projection.get_mut(live.camera)
        && let Projection::Perspective(p) = p.as_mut()
    {
        p.aspect_ratio = size.x as f32 / size.y as f32;
    }
}

/// No sun shadow cascades for the art camera (it draws unlit materials).
fn drop_sun_cascades(art: Query<Entity, With<ResultsCamera>>, mut lights: Query<&mut Cascades>) {
    for camera in &art {
        for mut cascades in &mut lights {
            if cascades.cascades.contains_key(&camera) {
                cascades.cascades.remove(&camera);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::post_race::View;

    const ROW: &str = "gadget/list_items=0";
    const HEADER: &str = "gadget/list_titles=titles";
    const R: &Def = &DEFS[0];
    const D: &Def = &DEFS[1];

    fn board(rows: usize, player: usize, drift: bool) -> Board {
        let mut table = vec![std::array::from_fn(|i| format!("h{i}"))];
        for r in 0..rows {
            table.push(std::array::from_fn(|i| format!("r{r}c{i}")));
        }
        Board {
            view: View { title: "T".into(), body: "B".into(), table, player_row: Some(player + 1), ..View::default() },
            drift,
            fourth: Fourth::Team,
            screen: Screen::Results,
        }
    }

    #[test]
    fn window_centres_the_player() {
        assert_eq!(window(8, Some(7), 12), 0);
        assert_eq!(window(20, Some(0), 12), 0);
        assert_eq!(window(20, Some(10), 12), 4);
        assert_eq!(window(20, Some(19), 12), 8);
        assert_eq!(window(20, None, 12), 0);
        assert_eq!(window(10, None, 7), 0);
    }

    #[test]
    fn gates_follow_the_tree() {
        let g = gate(&["text=driver", "null=race_results", "library_simple=content", "null=anim", ROW, "stacker_top=list"], R);
        assert_eq!((g.variant, g.row, g.header, g.hidden), (Variant::Race, true, false, false));
        assert_eq!(gate(&["text=x", "null=online_leaderboard", "library_simple=content"], R).variant, Variant::Never);
        assert_eq!(gate(&["null=frame", "null=table"], R).variant, Variant::Any);
        assert!(gate(&["null=x", "gadget/list_items=1"], R).hidden);
        assert!(!gate(&["null=x", ROW], R).hidden);
        assert!(gate(&["unique=default", "library/image_flag=flag"], R).hidden);
        // The scroller face hides its own mesh only.
        assert!(gate(&["scroller_v_manual=centre_face"], R).hidden);
        assert!(!gate(&["null=scrollbar", "scroller_v_manual=centre_face"], R).hidden);
        let g = gate(&["null=track_info_pill", "component/leaderboards_cursor_alt=cursor", ROW], R);
        assert!(g.player_only);
        // The drift scoreboard: its rows, content and always-lit statistic pills.
        let row = ["text=amount", "null=online_leaderboard", "library_simple=content", "gadget/drift_list_items=0", "stacker_top=list"];
        let g = gate(&row, D);
        assert_eq!((g.variant, g.row, g.hidden), (Variant::Any, true, false));
        assert_eq!(field(&row, &g, D), Some(Field::Cell(Column::Amount)));
        assert!(gate(&["null=x", "gadget/drift_list_items=3"], D).hidden);
        assert!(!gate(&["null=anim_dummy", "object/h_pill_group=undefined"], D).hidden);
        assert!(!gate(&["null=track_info_pill", "component/drift_cursor_half=avg_speed"], D).player_only);
        let half = ["text=drift_type", "component/drift_cursor_half=avg_speed", "screen/drift_scoreboard=drift_scoreboard"];
        assert_eq!(field(&half, &gate(&half, D), D), Some(Field::Stat(3, false)));
        let total = ["text=score", "component/drift_cursor_total=total"];
        assert_eq!(field(&total, &gate(&total, D), D), Some(Field::Stat(6, true)));
        assert_eq!(field(&["text=title_01"], &gate(&["text=title_01"], D), D), Some(Field::Title));
        assert_eq!(field(&["text=table_title"], &gate(&["text=table_title"], D), D), None, "not bound");
        let header = ["text=drift_amount", "null=event_standings", "library_simple=content", "gadget/drift_list_titles=drift_titles"];
        assert_eq!(field(&header, &gate(&header, D), D), Some(Field::Header(Column::Amount)));
    }

    #[test]
    fn fields_and_columns() {
        let row = gate(&["text=time", "null=race_results", "library_simple=content", ROW], R);
        assert_eq!(field(&["text=time"], &row, R), Some(Field::Cell(Column::Time)));
        let head = gate(&["text=points", "null=race_results", "library_simple=content", HEADER], R);
        assert_eq!(field(&["text=points"], &head, R), Some(Field::Header(Column::Points)));
        assert_eq!(field(&["text=title_1"], &gate(&["text=title_1"], R), R), Some(Field::Title));
        assert_eq!(field(&["text=table_title"], &gate(&["text=table_title"], R), R), Some(Field::Body));
        assert_eq!(field(&["text=07"], &row, R), None);
        assert_eq!(view_column(Column::Team, Fourth::Team), Some(4));
        assert_eq!(view_column(Column::Team, Fourth::Points), None);
        assert_eq!(view_column(Column::Points, Fourth::Points), Some(4));
        assert_eq!(view_column(Column::Difference, Fourth::Team), None);
        assert_eq!(view_column(Column::Difference, Fourth::Round), Some(5));
        let b = board(20, 15, false);
        // Window starts at 8 (rows 8..20); slot 0 is data row 8.
        assert_eq!(text_of(Field::Cell(Column::Driver), Some(0), &b), "r8c1");
        assert_eq!(text_of(Field::Header(Column::Car), None, &b), "h3");
        assert_eq!(text_of(Field::Cell(Column::Points), Some(0), &b), "");
        assert_eq!(text_of(Field::Title, None, &b), "T");
    }

    #[test]
    fn visibility_by_variant_row_and_player() {
        let race = Gate { hidden: false, variant: Variant::Race, row: true, header: false, player_only: false, scrollbar: false };
        let b = board(5, 2, false);
        assert!(visible(&race, Some(4), &b));
        assert!(!visible(&race, Some(5), &b), "past the last car");
        assert!(!visible(&race, Some(0), &board(5, 2, true)), "race cells in a drift event");
        let pill = Gate { player_only: true, variant: Variant::Any, ..race };
        assert!(visible(&pill, Some(2), &b));
        assert!(!visible(&pill, Some(1), &b));
        let bar = Gate { scrollbar: true, row: false, variant: Variant::Any, ..race };
        assert!(!visible(&bar, None, &b));
        assert!(visible(&bar, None, &board(20, 2, false)));
    }

    fn key(time: f32, value: f32) -> osd::Key {
        osd::Key { time, value, in_value: value, in_time: time, out_value: value, out_time: time }
    }

    fn channel(target: &str, semantic: osd::Semantic, keys: [&[(f32, f32)]; 3]) -> osd::Channel {
        osd::Channel {
            target: target.into(),
            semantic,
            components: keys.iter().map(|k| osd::Curve(k.iter().map(|&(t, v)| key(t, v)).collect())).collect(),
        }
    }

    #[test]
    fn animated_and_slide() {
        let mut saved = IDENTITY;
        saved[0] = 2.;
        saved[5] = 2.;
        saved[10] = 2.;
        let c = channel("n", osd::Semantic::Translation, [&[(0., 1.), (1., 3.)], &[(0., 0.)], &[(0., 5.)]]);
        let m = animated(&saved, &[&c], 1.);
        assert_eq!((m[0], m[12], m[13], m[14]), (2., 3., 0., 5.), "the scale stays, the translation moves");
        let s = channel("n", osd::Semantic::Translation, [&[(0., 1.), (1., 9.), (2., 1.)], &[(0., 0.), (1., 2.), (2., 0.)], &[(0., 0.), (1., 0.), (2., 0.)]]);
        assert_eq!(slide(&s, Vec3::new(1., 0., 0.)), Some(Vec3::new(9., 2., 0.)));
        assert_eq!(slide(&s, Vec3::new(4., 0., 0.)), None, "not from the saved pose");
        assert_eq!(slide(&c, Vec3::new(1., 0., 5.)), None, "not three keys");
    }

    #[test]
    fn row_state_colour() {
        // The authored row text and cursor pill colour keys (all three packages).
        let k: &[(f32, f32)] = &[(0., 0.1), (0.333, 0.83), (0.667, 0.83), (1., 0.1), (1.333, 0.1), (1.667, 0.83)];
        let pill: &[(f32, f32)] = &[(0., 0.15), (0.333, 0.1), (0.667, 0.1), (1., 1.), (1.333, 0.6), (1.667, 0.2)];
        let still: &[(f32, f32)] = &[(0., 1.), (1.667, 1.)];
        let mut text = channel("text#DiffuseColour", osd::Semantic::Parameter, [k, k, k]);
        text.components.push(osd::Curve(still.iter().map(|&(t, v)| key(t, v)).collect()));
        let a = osd::Animation {
            id: "a".into(),
            ranges: Vec::new(),
            channels: vec![text, channel("pill#DiffuseColour", osd::Semantic::Parameter, [pill, pill, pill]), channel("pill#Alpha", osd::Semantic::Parameter, [k, k, k])],
            external: Vec::new(),
            events: Vec::new(),
            default_frame: 0.,
        };
        assert_eq!(hold_ends(&a), vec![0.667, 1.333], "the still alpha component and the Alpha channel add no window");
        let s = animation_states(&a);
        assert_eq!(s[0], ("text", [Some(Vec3::splat(0.1)), Some(Vec3::splat(0.83))]));
        assert_eq!(s[1], ("pill", [Some(Vec3::splat(0.6)), Some(Vec3::splat(0.1))]));
        assert_eq!(s.len(), 2);
        let b = board(20, 12, false);
        // The window starts at row 6, so the player's row is slot 6.
        assert!(on_player_row(Some(6), &b));
        assert!(!on_player_row(Some(5), &b));
        assert!(!on_player_row(None, &b), "fixed items keep the state colour");
    }

    #[test]
    fn drift_statistics_fill_their_glyphs() {
        let mut b = board(3, 0, true);
        b.screen = Screen::DriftScoreboard;
        b.view.player_row = None;
        b.view.stats = crate::post_race::DRIFT_STATS.iter().map(|k| [format!("{k} label"), format!("{k} value")]).collect();
        assert_eq!(text_of(Field::Stat(1, false), None, &b), "drift_time label");
        assert_eq!(text_of(Field::Stat(6, true), None, &b), "total value");
        assert_eq!(text_of(Field::Cell(Column::DriftType), Some(2), &b), "r2c0");
        let row = Gate { hidden: false, variant: Variant::Any, row: true, header: false, player_only: false, scrollbar: false };
        assert!(visible(&row, Some(2), &b) && !visible(&row, Some(3), &b));
        let cursor = Gate { player_only: true, ..row };
        assert!(!visible(&cursor, Some(0), &b), "no player row on the scoreboard");
    }

    /// Needs GRID_GAME_ROOT: the drift scoreboard lays out with every
    /// statistic, the drift type columns and its title, inside the view.
    #[test]
    #[ignore]
    fn drift_scoreboard_lays_out() {
        let root = std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").unwrap());
        let p = prepare(&root).unwrap();
        let l = &p.layouts[Screen::DriftScoreboard as usize];
        println!("pose {:?} pitch {} items {} skipped {:?}", l.pose, l.pitch, l.items.len(), l.skipped);
        assert!((l.pitch.y + 8.).abs() < 0.5, "{}", l.pitch);
        for i in 0..crate::post_race::DRIFT_STATS.len() {
            for value in [false, true] {
                let f = Field::Stat(i, value);
                assert!(l.items.iter().any(|it| matches!(&it.shape, Shape::Text { field, .. } if *field == f)), "{f:?}");
            }
        }
        for f in [Field::Title, Field::Header(Column::DriftType), Field::Header(Column::Amount), Field::Cell(Column::DriftType), Field::Cell(Column::Time)] {
            assert!(l.items.iter().any(|i| matches!(&i.shape, Shape::Text { field, .. } if *field == f)), "{f:?}");
        }
        let view = Mat4::look_at_rh(l.pose.eye, l.pose.target, Vec3::Y);
        let proj = Mat4::perspective_rh(l.pose.fov_degrees.to_radians(), 1.6, l.near, l.far);
        let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
        for it in &l.items {
            let b = it.bounds;
            for k in 0..8 {
                let c = Vec3::new(b[if k & 1 == 0 { 0 } else { 3 }], b[if k & 2 == 0 { 1 } else { 4 }], b[if k & 4 == 0 { 2 } else { 5 }]);
                let q = proj * view * it.world.transform_point3(c).extend(1.);
                assert!(q.w > 0., "art behind the camera");
                let n = q.truncate().truncate() / q.w;
                lo = lo.min(n);
                hi = hi.max(n);
            }
        }
        println!("ndc lo {lo:.2} hi {hi:.2}");
        assert!(lo.cmpge(Vec2::splat(-1.)).all() && hi.cmple(Vec2::splat(1.)).all(), "art leaves the view: {lo} {hi}");
    }

    /// Needs GRID_GAME_ROOT: the results screen lays out and loads.
    #[test]
    #[ignore]
    fn results_screen_lays_out() {
        let root = std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").unwrap());
        let p = prepare(&root).unwrap();
        let l = &p.layouts[0];
        let texts = l.items.iter().filter(|i| matches!(i.shape, Shape::Text { .. })).count();
        let rows = l.items.iter().filter(|i| i.gate.row).count();
        println!(
            "pose {:?} near {} far {} pitch {} items {} texts {texts} row items {rows} materials {} images {} fonts {:?} skipped {:?}",
            l.pose,
            l.near,
            l.far,
            l.pitch,
            l.items.len(),
            l.materials.len(),
            p.images.len(),
            p.fonts.keys().collect::<Vec<_>>(),
            l.skipped
        );
        let states: Vec<_> = l.materials.iter().map(|m| m.states).filter(|s| s.iter().any(Option::is_some)).collect();
        println!("state colours [player, deactivate] {states:?}");
        let text = l
            .items
            .iter()
            .filter(|i| matches!(i.shape, Shape::Text { .. }) && i.gate.row)
            .map(|i| l.materials[i.material].states)
            .filter(|s| s[1].is_some())
            .collect::<Vec<_>>();
        let near = |c: Option<Vec3>, v: f32| c.is_some_and(|c| (c.x - v).abs() < 0.01);
        assert!(!text.is_empty() && text.iter().all(|s| near(s[0], 0.1) && near(s[1], 0.83)), "row text dark on the player's row, light elsewhere: {text:?}");
        assert!(states.iter().any(|s| near(s[0], 0.6) && near(s[1], 0.1)), "the cursor pill lights on the player's row");
        // Where the art lands on a 16:10 screen at the pose.
        let view = Mat4::look_at_rh(l.pose.eye, l.pose.target, Vec3::Y);
        let proj = Mat4::perspective_rh(l.pose.fov_degrees.to_radians(), 1.6, l.near, l.far);
        let ndc = |w: Vec3| {
            let q = proj * view * w.extend(1.);
            assert!(q.w > 0., "art behind the camera");
            q.truncate().truncate() / q.w
        };
        let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
        for it in &l.items {
            let b = it.bounds;
            for k in 0..8 {
                let c = Vec3::new(b[if k & 1 == 0 { 0 } else { 3 }], b[if k & 2 == 0 { 1 } else { 4 }], b[if k & 4 == 0 { 2 } else { 5 }]);
                let n = ndc(it.world.transform_point3(c));
                lo = lo.min(n);
                hi = hi.max(n);
            }
        }
        println!("ndc lo {lo:.2} hi {hi:.2}");
        assert!(lo.cmpge(Vec2::splat(-1.)).all() && hi.cmple(Vec2::splat(1.)).all(), "art leaves the view: {lo} {hi}");
        assert!((l.pose.time - 1.667).abs() < 0.01, "{:?}", l.pose);
        assert!((l.pitch.y + 8.).abs() < 0.5, "{}", l.pitch);
        for f in [Field::Title, Field::Body, Field::Header(Column::Driver), Field::Cell(Column::Driver), Field::Cell(Column::Time)] {
            assert!(l.items.iter().any(|i| matches!(&i.shape, Shape::Text { field, .. } if *field == f)), "{f:?}");
        }
        assert!(rows > 0 && texts > 0);
        // The slide in and out: off to the left at the start of `enter`,
        // at rest when it ends, off to the right at the end of `exit`.
        let moved = l.items.iter().filter(|i| !i.motion.is_empty()).count();
        println!("motions {} items moved {moved} enter {:.2}s exit {:.2}s", l.motions.len(), play_seconds(&l.motions, Play::Enter), play_seconds(&l.motions, Play::Exit));
        assert!(!l.motions.is_empty() && moved * 2 > l.items.len(), "{moved} of {}", l.items.len());
        assert!((play_seconds(&l.motions, Play::Enter) - 0.4).abs() < 1e-3);
        for m in &l.motions {
            println!(
                "motion {} held {} enter {:?} exit {:?}",
                m.channel.target,
                m.held,
                m.enter.as_ref().map(|e| (e.duration, &e.steps)),
                m.exit.as_ref().map(|e| (e.duration, &e.steps))
            );
            // At rest when `enter` ends and when `exit` starts.
            assert!(m.offset(m.frame(Play::Enter, 0.4)).length() < 1e-3);
            assert!(m.offset(m.frame(Play::Exit, 0.)).length() < 1e-3);
        }
        // Per drawn item, all its animated ancestors together.
        let (mut slides_in, mut slides_out, mut from_off) = (0, 0, 0);
        for i in l.items.iter().filter(|i| !i.motion.is_empty()) {
            let centre = i.world.transform_point3(Vec3::new((i.bounds[0] + i.bounds[3]) / 2., (i.bounds[1] + i.bounds[4]) / 2., (i.bounds[2] + i.bounds[5]) / 2.));
            let x = |play, t| ndc(centre + i.motion.iter().map(|&m| l.motions[m].offset(l.motions[m].frame(play, t))).sum::<Vec3>()).x;
            let rest = x(Play::Enter, 0.4);
            slides_in += usize::from(x(Play::Enter, 0.) < rest - 0.1);
            slides_out += usize::from(x(Play::Exit, 0.4) > rest + 0.1);
            from_off += usize::from(x(Play::Enter, 0.) < -1.);
        }
        println!("items slide in {slides_in} out {slides_out} from off screen {from_off} of {moved}");
        assert!(slides_out * 2 > moved && slides_in * 2 > moved && from_off > 0);
    }

    /// A strip like the results screens': two pill templates (x 0 and 2.0),
    /// each a stacker of a left cap, a scaler middle (scalee and label at
    /// -0.5) and a right cap at 1.08.
    fn synthetic_strip() -> Strip {
        let text = osd::TextLayout {
            font: "swiss_black0".into(),
            height: 0.5,
            depth: 0.,
            h_align: 0,
            v_align: 0,
            wrapping: 0,
            line_spacing: 1.,
            tracking: 0.,
            case: 0,
        };
        let node = |flow, parent, x: f32, own: Option<[f32; 2]>| FlowNode {
            flow,
            parent,
            children: Vec::new(),
            local: Mat4::from_translation(Vec3::new(x, 0.25, 1.)),
            own,
            text: None,
            gaps: Vec::new(),
            start: 0.,
        };
        let mut nodes = vec![node(Flow::Stacker, None, 0., None)];
        for x in [2.0, 0.] {
            let pill = nodes.len();
            nodes.push(node(Flow::Plain, Some(0), x, None));
            nodes.push(node(Flow::Stacker, Some(pill), 0., None));
            let build = pill + 1;
            nodes.push(node(Flow::Plain, Some(build), 1.08, Some([0., 0.5])));
            nodes.push(node(Flow::Plain, Some(build), 0., Some([0., 0.5])));
            nodes.push(node(Flow::Scaler, Some(build), 1.0, None));
            let middle = nodes.len() - 1;
            nodes.push(node(Flow::Scalee, Some(middle), -0.5, Some([0., 0.58])));
            let mut label = node(Flow::Plain, Some(middle), -0.5, Some([0., 0.6]));
            label.text = Some((text.clone(), [0., 0., 0., 0.6, 0.5, 0.]));
            nodes.push(label);
        }
        for f in 1..nodes.len() {
            let p = nodes[f].parent.unwrap();
            nodes[p].children.push(f);
        }
        // The load-time order, gaps and starts as `Strip::find` derives them.
        let mut extent: Vec<Option<[f32; 2]>> = vec![None; nodes.len()];
        for f in (0..nodes.len()).rev() {
            let e = nodes[f].children.iter().fold(nodes[f].own, |e, &c| union(e, extent[c].map(|x| through(&nodes[c].local, x))));
            extent[f] = e;
        }
        let spans: Vec<Option<[f32; 2]>> = (0..nodes.len()).map(|c| extent[c].map(|e| through(&nodes[c].local, e))).collect();
        for n in nodes.iter_mut().filter(|n| n.flow == Flow::Stacker) {
            sort_stacker(n, &spans);
        }
        Strip { nodes, base: Mat4::IDENTITY }
    }

    #[test]
    fn stackers_keep_their_load_gaps() {
        let s = synthetic_strip();
        let root = &s.nodes[0];
        // Template 0 is the one at x 0 (authored second).
        assert_eq!(root.children, [8, 1]);
        assert!((root.gaps[0] - 0.42).abs() < 1e-5 && root.gaps[1] == 0. && root.start == 0., "{:?} {}", root.gaps, root.start);
        let build = &s.nodes[9];
        assert_eq!(build.children, [11, 12, 10], "left cap, middle, right cap");
        let want = [0., -0.02, 0.];
        assert!(build.gaps.iter().zip(want).all(|(g, w)| (g - w).abs() < 1e-5), "{:?}", build.gaps);
        assert_eq!((s.template(14), s.template(7), s.template(0)), (Some(0), Some(1), None));
    }

    #[test]
    fn pills_reflow_around_their_labels() {
        let s = synthetic_strip();
        let labels: Vec<String> = ["AB", "ABCD", "X", ""].map(String::from).into();
        // A label 0.1 wide per character.
        let span = |_: &osd::TextLayout, _: &[f32; 6], l: &str| Some([0., 0.1 * l.len() as f32]);
        let pills = s.place(&labels, &span);
        assert_eq!(pills.len(), 4);
        let x = |k: usize, f: usize| pills[k][f].map(|w| w.w_axis.x);
        let near = |a: Option<f32>, b: f32| a.is_some_and(|a| (a - b).abs() < 1e-4);
        // Pill 0 (template 0): right cap after the 0.2 label, gap -0.02.
        assert!(near(x(0, 8), 0.) && near(x(0, 10), 0.68), "{:?} {:?}", x(0, 8), x(0, 10));
        // The scalee spans the label: scale 0.2 / 0.58 from x 0.5.
        let center = pills[0][13].unwrap();
        assert!((center.x_axis.x - 0.2 / 0.58).abs() < 1e-4 && near(Some(center.w_axis.x), 0.5) && near(Some(center.w_axis.y), 1.25));
        assert!(pills[0][1].is_none(), "template 1's nodes are not in pill 0");
        // Pill 1 (template 1) starts after pill 0 and the 0.42 gap; pill 2
        // clones it after the same gap.
        assert!(near(x(1, 1), 1.6) && near(x(1, 3), 1.6 + 0.88), "{:?} {:?}", x(1, 1), x(1, 3));
        assert!(near(x(2, 1), 3.4) && near(x(2, 3), 3.4 + 0.58), "{:?} {:?}", x(2, 1), x(2, 3));
        assert!(pills[2][8].is_none());
        // An empty label shows no pill.
        assert!(pills[3].iter().all(Option::is_none));
        // The stacker keeps y and z.
        assert!(near(pills[1][3].map(|w| w.w_axis.y), 1.) && near(pills[1][3].map(|w| w.w_axis.z), 4.));
    }

    #[test]
    fn post_race_pills_fit_the_strip() {
        use crate::post_race::{RESULTS_PILLS, TEST_DRIVE_WRECK_PILLS, WRECK_PILLS};
        assert!(RESULTS_PILLS.len() <= MAX_PILLS && WRECK_PILLS.len() <= MAX_PILLS && TEST_DRIVE_WRECK_PILLS.len() <= MAX_PILLS);
    }

    /// The pill strips of the packages (`GRID_GAME_ROOT`).
    #[test]
    #[ignore]
    fn pill_strips_lay_out() {
        let root = std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").unwrap());
        let p = prepare(&root).unwrap();
        for (screen, l) in SCREENS.iter().zip(&p.layouts) {
            let s = l.strip.as_ref().unwrap_or_else(|| panic!("{screen:?}: no strip"));
            let root = &s.nodes[0];
            println!("{screen:?} templates {} gaps {:?} start {}", root.children.len(), root.gaps, root.start);
            assert_eq!(root.children.len(), 2);
            assert!((root.gaps[0] - 0.42).abs() < 0.01);
            let builds: Vec<&FlowNode> = s.nodes.iter().skip(1).filter(|n| n.flow == Flow::Stacker).collect();
            assert_eq!(builds.len(), 2);
            for b in builds {
                assert!(b.gaps.len() == 3 && b.gaps[0].abs() < 0.01 && (b.gaps[1] + 0.02).abs() < 0.01, "{:?}", b.gaps);
            }
            let labels = l.items.iter().filter(|i| matches!(i.shape, Shape::Text { field: Field::Pill, .. })).count();
            assert_eq!(labels, 2, "{screen:?}: a label per template");
            // The labels' text boxes lie in their node's x-z plane
            // (`osd_scene::text_mesh` lays them out there).
            for i in l.items.iter().filter(|i| matches!(i.shape, Shape::Text { field: Field::Pill, .. })) {
                let b = i.bounds;
                assert!(b[4] - b[1] < 1e-4 && b[5] - b[2] > 1., "{b:?}");
            }
            // The pill (`pill_glow_txtrd_2`) lights orange with glow on
            // `activate` and goes dark on `deactivate`; its label
            // (`text_pill_optiontitles`) dark on the lit pill, light on the
            // others.
            let events: Vec<_> = l.items.iter().filter(|i| i.strip.is_some()).map(|i| l.materials[i.material].events).filter(|e| e[0].is_some()).collect();
            let near = |v: f32, w: f32| (v - w).abs() < 0.02;
            assert!(
                events.iter().any(|e| e[0].is_some_and(|v| near(v.colour.x, 1.) && near(v.colour.y, 0.79) && v.a.x > 0.1)
                    && e[1].is_some_and(|v| near(v.colour.x, 0.1) && v.a.x < 0.01)),
                "{events:?}"
            );
            assert!(events.iter().any(|e| e[0].is_some_and(|v| near(v.colour.x, 0.1)) && e[1].is_some_and(|v| near(v.colour.x, 0.83))), "{events:?}");
            let labels: Vec<String> = crate::post_race::RESULTS_PILLS.iter().map(|(_, id)| id.to_string()).collect();
            let fonts = &p.fonts;
            let span = |t: &osd::TextLayout, b: &[f32; 6], label: &str| fonts.get(&t.font).map(|f| osd_scene::text_span(f, label, t, b));
            let pills = s.place(&labels, &span);
            let xs: Vec<f32> = pills.iter().filter_map(|p| p.iter().flatten().next().map(|w| w.w_axis.x)).collect();
            println!("{screen:?} pill x {xs:?}");
            assert!(xs.len() == 4 && xs.windows(2).all(|w| w[1] > w[0]));
        }
    }
}
