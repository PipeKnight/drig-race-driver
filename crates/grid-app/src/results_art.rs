//! The race results screen in the original's front-end art (docs/RACE_HUD.md
//! §6.7, docs/FIDELITY_BACKLOG.md row 69): the `screen/pause_leaderboards`
//! glyph of fePersistentDatabase with its glyphs grafted in
//! (`grid_formats::osd_glyph`), seen through the `pause_root` camera
//! `camera1`, drawn with the shared front-end materials (`fe3d`) into an image
//! shown over the race, and filled from the post-race results view
//! (`post_race::View`).
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
const SCREEN: &str = "screen/pause_leaderboards=leaderboards";
const CAMERA: &str = "camera=camera1";
const TARGET: &str = "target=camera1";
/// Rows drawn at once [I: the list's frame holds twelve].
pub const VISIBLE_ROWS: usize = 12;
/// Subtrees never drawn: the race timer, the row arrows and flags, the pill
/// helper, the frame without scroller [I: the frame of a list that scrolls].
const HIDDEN: [&str; 5] = [
    "text=timer",
    "{30}component/leaderboard_arrow=arrow",
    "null=pill_helper",
    "library/image_flag=flag",
    "component/leaderboard_noscroll=noscroll",
];
const ROW: &str = "gadget/list_items=0";
const HEADER: &str = "gadget/list_titles=titles";

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

/// The gate of a node from its name and its ancestors' (`names[0]` the node,
/// then up to the screen).
fn gate(names: &[&str]) -> Gate {
    let any = |n: &str| names.contains(&n);
    let hidden = names.iter().any(|n| {
        HIDDEN.contains(n) || n.strip_prefix("gadget/list_items=").is_some_and(|k| k != "0")
    }) || names.first() == Some(&"scroller_v_manual=centre_face");
    let variant = names
        .windows(2)
        .find(|w| w[1].ends_with("library_simple=content"))
        .map_or(Variant::Any, |w| match w[0] {
            "null=race_results" => Variant::Race,
            "null=drift_results" => Variant::Drift,
            _ => Variant::Never,
        });
    Gate {
        hidden,
        variant,
        row: any(ROW),
        header: any(HEADER),
        player_only: any("null=track_info_pill"),
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
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Field {
    Title,
    Body,
    Header(Column),
    Cell(Column),
}

fn field(names: &[&str], g: &Gate) -> Option<Field> {
    let text = names.first()?.strip_prefix("text=")?;
    match text {
        "title_0" | "title_1" => Some(Field::Title),
        "table_title" => Some(Field::Body),
        _ => {
            let c = column(text)?;
            if g.row {
                Some(Field::Cell(c))
            } else if g.header {
                Some(Field::Header(c))
            } else {
                None
            }
        }
    }
}

/// The first data row drawn: the player's row in the middle of the window.
fn window(rows: usize, player: Option<usize>) -> usize {
    if rows <= VISIBLE_ROWS {
        return 0;
    }
    player.unwrap_or(0).saturating_sub(VISIBLE_ROWS / 2).min(rows - VISIBLE_ROWS)
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
            let start = window(rows, v.player_row.map(|p| p.saturating_sub(1)));
            cell(1 + start + slot.unwrap_or(0), c)
        }
    }
}

/// Whether row `slot` of the window is the player's row.
fn on_player_row(slot: Option<usize>, board: &Board) -> bool {
    let rows = board.view.table.len().saturating_sub(1);
    let player = board.view.player_row.map(|p| p.saturating_sub(1));
    slot.is_some_and(|k| Some(window(rows, player) + k) == player)
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
    let start = window(rows, player);
    let row = slot.map(|k| start + k);
    variant
        && !gate.hidden
        && row.is_none_or(|r| r < rows)
        && (!gate.player_only || (row.is_some() && row == player))
        && (!gate.scrollbar || rows > VISIBLE_ROWS)
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

fn layout(scenes: &[&osd::Scene]) -> Result<Layout, String> {
    let s0 = scenes[0];
    let find = |name: &str| s0.nodes.iter().position(|n| n.name() == name).ok_or(format!("no {name}"));
    let (screen, camera, target) = (find(SCREEN)?, find(CAMERA)?, find(TARGET)?);
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
    let name = |p: &osd_glyph::Placed| scenes[p.scene].nodes[p.node].name();
    let at = |n: &str| placed.iter().find(|p| name(p) == n).map(|p| Vec3::new(p.world[12], p.world[13], p.world[14]));
    let pitch = match (at("gadget/list_items=0"), at("gadget/list_items=1")) {
        (Some(a), Some(b)) => b - a,
        _ => return Err("no row pitch".into()),
    };
    let mut skipped: HashMap<&'static str, usize> = HashMap::new();
    let mut materials = Vec::new();
    let mut material_index: HashMap<(usize, usize), usize> = HashMap::new();
    let states: Vec<HashMap<&str, [Option<Vec3>; 2]>> = scenes.iter().map(|s| state_colours(s)).collect();
    let mut items = Vec::new();
    for p in &placed {
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
        let g = gate(&names);
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
            Some(t) => match field(&names, &g) {
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
            });
            materials.len() - 1
        });
        items.push(Item { world: Mat4::from_cols_array(&p.world), material, family, bounds: node.bounds, shape, gate: g });
    }
    Ok(Layout {
        pose,
        near: cam.map_or(0.1, |c| c.near).max(0.1),
        far: cam.map_or(10000., |c| c.far),
        items,
        materials,
        pitch,
        skipped,
    })
}

// ------------------------------------------------------------------ loading

struct Prepared {
    layout: Layout,
    /// Bundle textures by name.
    images: HashMap<String, Image>,
    cubes: [Image; 2],
    fonts: HashMap<String, osd_scene::Font>,
    /// The Bevy mesh of each static item.
    meshes: Vec<Option<Mesh>>,
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
    let layout = layout(&refs)?;
    // Bundle textures.
    let wanted: HashSet<&str> = layout
        .materials
        .iter()
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
    for item in &layout.items {
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
    let meshes = layout
        .items
        .iter()
        .map(|i| match &i.shape {
            Shape::Static(m) => Some(fe3d::mesh(m, &i.bounds, i.family)),
            Shape::Text { .. } => None,
        })
        .collect();
    let xml = String::from_utf8_lossy(&read(PARAMS)?).into_owned();
    let lights = fe3d::Lights::read(&xml, "ingame", Some("leaderboards")).unwrap_or_default();
    Ok(Prepared { layout, images, cubes, fonts, meshes, lights })
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
            "results_shot path={} art={} rows={}",
            path.display(),
            if art.is_some() { "on" } else { "off" },
            shown.board.as_ref().map_or(0, |b| b.view.table.len().saturating_sub(1))
        );
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
    }
}

// ------------------------------------------------------------------ live

#[derive(Component)]
struct ResultsCamera;

/// One drawn copy of a layout item (`slot`: its row in the window).
#[derive(Component)]
struct Art {
    item: usize,
    slot: Option<usize>,
}

#[derive(Resource)]
struct Live {
    items: Vec<Item>,
    fonts: HashMap<String, osd_scene::Font>,
    /// Per material: authored, `deactivate` state (the same when unanimated).
    materials: Vec<[Handle<Fe3dMaterial>; 2]>,
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
    let l = &p.layout;
    let mut skipped: Vec<String> = l.skipped.iter().map(|(k, v)| format!("{k}:{v}")).collect();
    skipped.sort();
    log_line!(
        "results_art version=1 loaded=true items={} materials={} textures={} pose_time={:.3} eye={:.1},{:.1},{:.1} fov={:.2} skipped={}",
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
    let white = images.add(white());
    let [soft, sharp] = p.cubes.map(|c| images.add(c));
    let named: HashMap<String, Handle<Image>> = p.images.into_iter().map(|(k, v)| (k, images.add(v))).collect();
    let texture = |n: &Option<String>| n.as_ref().and_then(|n| named.get(n)).cloned().unwrap_or_else(|| white.clone());
    let constants = p.lights.constants(None, None, 0.);
    // Per material: the authored colour (the player's row) and the
    // `deactivate` state colour (the other rows and the fixed items).
    let handles: Vec<[Handle<Fe3dMaterial>; 2]> = l
        .materials
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
            let authored = materials.add(authored);
            [player.unwrap_or_else(|| authored.clone()), rest.unwrap_or(authored)]
        })
        .collect();
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
    for (i, item) in l.items.iter().enumerate() {
        let mesh = p.meshes[i].clone().map(|m| meshes.add(m)).unwrap_or_default();
        let slots: Vec<Option<usize>> = if item.gate.row { (0..VISIBLE_ROWS).map(Some).collect() } else { vec![None] };
        for slot in slots {
            let world = Mat4::from_translation(l.pitch * slot.unwrap_or(0) as f32) * item.world;
            commands.spawn((
                Art { item: i, slot },
                Transform::from_matrix(world),
                Visibility::Hidden,
                Mesh3d(mesh.clone()),
                MeshMaterial3d(handles[item.material][1].clone()),
                RenderLayers::layer(LAYER),
                bevy::camera::visibility::NoFrustumCulling,
            ));
        }
    }
    commands.insert_resource(Live {
        items: l.items.clone(),
        fonts: p.fonts,
        materials: handles,
        lights: p.lights,
        pulse: fe3d::Pulse::default(),
        camera_world: transform.to_matrix(),
        camera,
        nodes: [dim, shown],
        image,
        display,
        size: UVec2::ZERO,
        board: None,
    });
    ready.0 = true;
}

#[allow(clippy::too_many_arguments)]
fn update(
    live: Option<ResMut<Live>>,
    shown: Res<Shown>,
    time: Option<Res<Time<Real>>>,
    mut cameras: Query<&mut Camera, With<ResultsCamera>>,
    mut nodes: Query<&mut Visibility, Without<Art>>,
    mut art: Query<(&Art, &mut Visibility, &mut Mesh3d, &mut MeshMaterial3d<Fe3dMaterial>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<Fe3dMaterial>>,
) {
    let Some(mut live) = live else {
        return;
    };
    let on = shown.board.is_some();
    if let Ok(mut camera) = cameras.get_mut(live.camera)
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
        return;
    };
    let live = &mut *live;
    if live.board.as_ref() != Some(board) {
        let mut drawn = 0;
        for (a, mut visibility, mut mesh, mut material) in &mut art {
            let item = &live.items[a.item];
            let state = &live.materials[item.material][usize::from(!on_player_row(a.slot, board))];
            if material.0 != *state {
                material.0 = state.clone();
            }
            let mut on = visible(&item.gate, a.slot, board);
            if let Shape::Text { layout, field } = &item.shape {
                let text = text_of(*field, a.slot, board);
                on &= !text.trim().is_empty();
                let was = live.board.as_ref().map(|b| text_of(*field, a.slot, b));
                if on
                    && was.as_deref() != Some(text.as_str())
                    && let Some(font) = live.fonts.get(&layout.font)
                {
                    let m = osd_scene::text_mesh(font, &text, layout, &item.bounds);
                    mesh.0 = meshes.add(fe3d::mesh(&m, &item.bounds, item.family));
                }
            }
            visibility.set_if_neq(if on { Visibility::Inherited } else { Visibility::Hidden });
            drawn += usize::from(on);
        }
        println!(
            "results_art board rows={} drift={} drawn={drawn}",
            board.view.table.len().saturating_sub(1),
            board.drift
        );
        live.board = Some(board.clone());
    }
    let dt = time.map_or(0., |t| t.delta_secs());
    live.pulse.advance(dt, live.lights.g_rate);
    let constants = live.lights.constants(Some(&live.camera_world), None, live.pulse.value(live.lights.g_swing));
    for h in live.materials.iter().flatten() {
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

    fn board(rows: usize, player: usize, drift: bool) -> Board {
        let mut table = vec![std::array::from_fn(|i| format!("h{i}"))];
        for r in 0..rows {
            table.push(std::array::from_fn(|i| format!("r{r}c{i}")));
        }
        Board {
            view: View { title: "T".into(), body: "B".into(), table, player_row: Some(player + 1), pills: String::new() },
            drift,
            fourth: Fourth::Team,
        }
    }

    #[test]
    fn window_centres_the_player() {
        assert_eq!(window(8, Some(7)), 0);
        assert_eq!(window(20, Some(0)), 0);
        assert_eq!(window(20, Some(10)), 4);
        assert_eq!(window(20, Some(19)), 8);
        assert_eq!(window(20, None), 0);
    }

    #[test]
    fn gates_follow_the_tree() {
        let g = gate(&["text=driver", "null=race_results", "library_simple=content", "null=anim", ROW, "stacker_top=list"]);
        assert_eq!((g.variant, g.row, g.header, g.hidden), (Variant::Race, true, false, false));
        assert_eq!(gate(&["text=x", "null=online_leaderboard", "library_simple=content"]).variant, Variant::Never);
        assert_eq!(gate(&["null=frame", "null=table"]).variant, Variant::Any);
        assert!(gate(&["null=x", "gadget/list_items=1"]).hidden);
        assert!(!gate(&["null=x", ROW]).hidden);
        assert!(gate(&["unique=default", "library/image_flag=flag"]).hidden);
        // The scroller face hides its own mesh only.
        assert!(gate(&["scroller_v_manual=centre_face"]).hidden);
        assert!(!gate(&["null=scrollbar", "scroller_v_manual=centre_face"]).hidden);
        let g = gate(&["null=track_info_pill", "component/leaderboards_cursor_alt=cursor", ROW]);
        assert!(g.player_only);
    }

    #[test]
    fn fields_and_columns() {
        let row = gate(&["text=time", "null=race_results", "library_simple=content", ROW]);
        assert_eq!(field(&["text=time"], &row), Some(Field::Cell(Column::Time)));
        let head = gate(&["text=points", "null=race_results", "library_simple=content", HEADER]);
        assert_eq!(field(&["text=points"], &head), Some(Field::Header(Column::Points)));
        assert_eq!(field(&["text=title_1"], &gate(&["text=title_1"])), Some(Field::Title));
        assert_eq!(field(&["text=table_title"], &gate(&["text=table_title"])), Some(Field::Body));
        assert_eq!(field(&["text=07"], &row), None);
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

    /// Needs GRID_GAME_ROOT: the results screen lays out and loads.
    #[test]
    #[ignore]
    fn results_screen_lays_out() {
        let root = std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").unwrap());
        let p = prepare(&root).unwrap();
        let l = &p.layout;
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
    }
}
