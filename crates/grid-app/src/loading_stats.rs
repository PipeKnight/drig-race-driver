//! The loading screen's stat panel (`ScreenLoadingStats`, docs/FRONT_END_FLOW.md
//! "Loading stat panel", F821): three lines of one profile fact fly in, stay,
//! fly out and come back with the next fact while the next screen loads.
//!
//! - Which facts [D 0x4EA770, 0x4EF1A0, 0x4E4ED0, 0x4D6D40]: the 38 slots of
//!   [`SLOTS`], each with a mode mask; a slot is offered when its mask meets
//!   the current mode's and its statistic gives a value. The valid ones are
//!   shuffled once with MSVC `random_shuffle` ([`Rand`], 0x4F4510) and shown
//!   in that order, wrapping (0x4DFAD0). The port keeps no profile statistics
//!   object; it reads the three facts the career save counts ([`Stats`]) and
//!   leaves the other slots invalid [I]. Mode bit 2 (career) is taken as "a
//!   career save exists" [I]; the drift, touge and Le Mans bits stay clear.
//! - When [D 0x52B970, 0x5048D0, 0x504A30, 0x504AC0]: the cycle of [`Cycle`],
//!   with the `<loading_stats>` times of `persistentdatacommon.xml`; the port
//!   has no load estimate, so it takes the desired times [I].
//! - How [D fePersistentDatabase.pssg]: the glyph `loading_stats_root` under
//!   the world's `screen/loading_stats=loading_stats` instance; its `Anim!1`
//!   moves the three helpers, and each `loading_N_in`/`_out` event plays one
//!   segment of it ([`IN`], [`OUT`]) over the event's duration override, at
//!   a linear rate [I]. The text nodes are `fe3d_genericGPD` (`crate::fe3d`);
//!   their `TR1` (`reflection_sharp.dds`) is not in the bundle, whose only
//!   cube is `reflection_load.dds` [D], so they reflect that [I].
//!   `load_top#DiffuseColour`'s small key over 0–1 s is not applied [I].
//!   Lines `text=1/2/3` are taken as header, body and details [I].
use crate::fe3d::{self, Fe3dMaterial};
use crate::osd_scene;
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use grid_formats::{osd, osd_glyph, pssg, texture};
use std::collections::HashMap;
use std::path::Path;

/// The world's instance of the panel [D].
pub const INSTANCE: &str = "screen/loading_stats=loading_stats";
/// The animation of the glyph's helpers [D].
pub const ANIMATION: &str = "Anim!1";

/// One fact slot of 0x4EA770: its name (`fact_<name>_header` …), mode mask,
/// whether it has a body, and its details id's suffix.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slot {
    pub name: &'static str,
    pub mask: u32,
    pub body: bool,
    pub details: Option<&'static str>,
}

const fn slot(name: &'static str, mask: u32, body: bool, details: Option<&'static str>) -> Slot {
    Slot { name, mask, body, details }
}
const D: Option<&str> = Some("details");

/// The fact table [D 0x4EA770].
pub const SLOTS: [Slot; 38] = [
    slot("biggest_rep", 1, true, None),
    slot("top_speed", 1, false, None),
    slot("total_distance", 1, false, D),
    slot("dirtiest_car", 2, false, None),
    slot("ebay_earnings", 2, false, D),
    slot("career_results", 1, true, D),
    slot("owned_cars", 2, true, D),
    slot("event_results", 2, true, D),
    slot("sponsor_earnings", 2, false, D),
    slot("instant_replay", 1, true, D),
    slot("fave_discipline", 1, false, D),
    slot("trophies", 2, true, D),
    slot("car_spending", 2, false, D),
    slot("teammate_spending", 2, false, D),
    slot("best_teammate", 2, false, D),
    slot("worst_teammate", 2, false, D),
    slot("max_drift_score", 4, true, D),
    slot("longest_drift", 4, false, D),
    slot("drift_level", 4, true, D),
    slot("drift_level", 4, true, D),
    slot("drift_level", 4, true, D),
    slot("driver_offers", 1, true, D),
    slot("win_streak", 1, true, D),
    slot("touge_speed", 8, false, None),
    slot("biggest_rival", 2, false, None),
    slot("top_rookie", 2, false, None),
    slot("top_friend", 0x20, false, D),
    slot("online_results", 0x20, true, D),
    slot("teammates", 2, true, D),
    slot("license", 2, false, D),
    slot("lemans_lap", 0x10, false, None),
    slot("lemans_results", 0x10, true, D),
    slot("biggest_drift", 1, true, Some("detail_no_drift")),
    slot("hot_driver", 2, false, D),
    slot("drift_master", 0x40, false, D),
    slot("over_achiever", 0x40, false, D),
    slot("crasher", 0x40, false, D),
    slot("speed_demon", 0x40, false, D),
];

/// What the port counts of the player's profile: the career save's numbers.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stats {
    /// A career save exists (mode bit 2 [I]).
    pub career: bool,
    pub wins: i32,
    pub cars: usize,
    pub offers: usize,
}

impl Stats {
    /// The current mode's mask [D 0x4D6D40: 1 always, 2 in career].
    pub fn mask(&self) -> u32 {
        1 | if self.career { 2 } else { 0 }
    }

    /// Reads the career save in `dir`, if any.
    pub fn read(dir: &Path) -> Stats {
        match grid_tools::career::save::load(&dir.join(crate::career_screen::SAVE)) {
            Ok(c) => Stats { career: true, wins: c.races_won, cars: c.cars.len(), offers: c.completed_offers.len() },
            Err(_) => Stats::default(),
        }
    }

    /// The `[VALUE]`s of slot `i`'s body and details; `None` for a slot the
    /// port cannot fill. Career results is always valid [D 0x4E4ED0 case 5].
    pub fn values(&self, i: usize) -> Option<[Option<String>; 2]> {
        match SLOTS[i].name {
            // Podiums are not counted: no details line [I].
            "career_results" => Some([Some(self.wins.to_string()), None]),
            // The cars' value is not counted [I].
            "owned_cars" if self.career => Some([Some(self.cars.to_string()), None]),
            // The offers' earnings are not counted [I].
            "driver_offers" if self.career => Some([Some(self.offers.to_string()), None]),
            _ => None,
        }
    }
}

/// The three lines of slot `i`: header, body, details (empty when absent).
pub fn texts(i: usize, stats: &Stats, label: &dyn Fn(&str) -> String) -> Option<[String; 3]> {
    let s = SLOTS[i];
    let [body, details] = stats.values(i)?;
    let fill = |suffix: &str, value: Option<String>| match value {
        Some(v) => label(&format!("fact_{}_{suffix}", s.name)).replace("[VALUE]", &v),
        None => String::new(),
    };
    Some([
        label(&format!("fact_{}_header", s.name)),
        if s.body { fill("body", body) } else { String::new() },
        s.details.map_or(String::new(), |d| fill(d, details)),
    ])
}

/// The MSVC CRT `rand` with its `srand` state.
#[derive(Clone, Debug)]
pub struct Rand(pub u32);

impl Rand {
    pub fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(214_013).wrapping_add(2_531_011);
        (self.0 >> 16) & 0x7FFF
    }

    /// `std::random_shuffle` [D 0x4F4510]: element i swaps with element
    /// `rand() % (i + 1)` (one `rand` per step below 0x7FFF elements).
    pub fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in 1..v.len() {
            let j = self.next() as usize % (i + 1);
            v.swap(i, j);
        }
    }
}

/// The valid slots for `stats`, in shown order.
pub fn order(stats: &Stats, rand: &mut Rand) -> Vec<usize> {
    let mask = stats.mask();
    let mut v: Vec<usize> = (0..SLOTS.len()).filter(|&i| SLOTS[i].mask & mask != 0 && stats.values(i).is_some()).collect();
    rand.shuffle(&mut v);
    v
}

/// `loading_N_in` segments of `Anim!1`, N = 1…5: (from, to) key seconds [D].
pub const IN: [(f32, f32); 5] = seg([6., 3., 6., 9., 12.]);
/// `loading_N_out` segments [D].
pub const OUT: [(f32, f32); 5] = seg([13. / 3., 22. / 3., 31. / 3., 22. / 3., 40. / 3.]);
/// The events' duration overrides [D].
pub const IN_SECONDS: f32 = 1.3;
pub const OUT_SECONDS: f32 = 0.8;

const fn seg(from: [f32; 5]) -> [(f32, f32); 5] {
    let mut out = [(0., 0.); 5];
    let mut i = 0;
    while i < 5 {
        out[i] = (from[i], from[i] + 4. / 3.);
        i += 1;
    }
    out
}

/// `<loading_stats>` of `persistentdatacommon.xml`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timing {
    pub animations: u32,
    pub on: f32,
    pub off: f32,
}

impl Timing {
    pub fn read(xml: &str) -> Option<Timing> {
        let block = &xml[xml.find("<loading_stats ")?..];
        let block = &block[..block.find("</loading_stats>")?];
        let attr = |tag: &str, key: &str| -> Option<f32> {
            let e = &block[block.find(tag)?..];
            let e = &e[..e.find('>')?];
            let pat = format!(" {key}=\"");
            let at = e.find(&pat)? + pat.len();
            e[at..].split('"').next()?.trim().parse().ok()
        };
        Some(Timing {
            animations: attr("<loading_stats", "definedanimations")? as u32,
            on: attr("<on_time", "desired")?,
            off: attr("<off_time", "desired")?,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Playing {
    from: f32,
    to: f32,
    seconds: f32,
    elapsed: f32,
}

/// What one step did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Step {
    /// The next fact was taken.
    pub refreshed: bool,
    /// `loading_N_in` started.
    pub played_in: bool,
    /// `loading_N_out` started.
    pub played_out: bool,
}

/// The panel's state machine [D 0x52B970]: the busy (a segment plays),
/// refresh (take the next fact) and shown flags, and the on and off timers.
#[derive(Clone, Debug)]
pub struct Cycle {
    timing: Timing,
    busy: Option<Playing>,
    refresh: bool,
    shown: bool,
    on: f32,
    off: f32,
    /// The animation number N of the current fact (1-based).
    pub n: usize,
    /// The key time of `Anim!1` the helpers hold (0: every line off screen).
    pub time: f32,
}

impl Cycle {
    /// Hidden, with refresh set [I].
    pub fn new(timing: Timing) -> Cycle {
        Cycle { timing, busy: None, refresh: true, shown: false, on: 0., off: 0., n: 1, time: 0. }
    }

    /// Advances by `dt`; `pick(animations)` draws N (0x5048D0).
    pub fn step(&mut self, dt: f32, pick: &mut dyn FnMut(u32) -> usize) -> Step {
        let mut step = Step::default();
        if let Some(p) = &mut self.busy {
            p.elapsed += dt;
            let u = (p.elapsed / p.seconds).min(1.);
            self.time = p.from + (p.to - p.from) * u;
            if u >= 1. {
                self.busy = None;
            }
            return step;
        }
        if self.refresh {
            self.refresh = false;
            self.n = pick(self.timing.animations).clamp(1, IN.len());
            step.refreshed = true;
        }
        if self.shown {
            self.on += dt;
            if self.on >= self.timing.on {
                self.shown = false;
                self.refresh = true;
                self.off = 0.;
                self.play(OUT[self.n - 1], OUT_SECONDS);
                step.played_out = true;
            }
        } else {
            self.off += dt;
            if self.off >= self.timing.off {
                self.shown = true;
                self.on = 0.;
                self.play(IN[self.n - 1], IN_SECONDS);
                step.played_in = true;
            }
        }
        step
    }

    fn play(&mut self, (from, to): (f32, f32), seconds: f32) {
        self.time = from;
        self.busy = Some(Playing { from, to, seconds, elapsed: 0. });
    }
}

// ------------------------------------------------------------------ layout

/// `saved` with the translation, rotation and scale the channels give at `t`
/// (as results_art's).
fn animated(saved: &[f32; 16], channels: &[osd::Channel], t: f32) -> [f32; 16] {
    let (mut s, mut r, mut p) = Mat4::from_cols_array(saved).to_scale_rotation_translation();
    for c in channels.iter().filter(|c| c.components.len() == 3) {
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

/// One node between the instance and a text line, with its channels.
#[derive(Clone, Debug)]
struct Link {
    local: [f32; 16],
    channels: Vec<osd::Channel>,
}

/// One text line of the panel.
#[derive(Clone, Debug)]
pub struct Line {
    /// 0 header, 1 body, 2 details.
    pub line: usize,
    pub node: String,
    /// The instance's world.
    base: [f32; 16],
    /// The nodes below the instance down to the text node.
    chain: Vec<Link>,
    pub layout: osd::TextLayout,
    pub bounds: [f32; 6],
    pub family: u32,
    pub values: fe3d::Values,
    pub dirt: Option<String>,
    pub pattern: Option<String>,
}

impl Line {
    /// The line's world at key time `t` of the animation.
    pub fn world(&self, t: f32) -> Mat4 {
        let mut w = self.base;
        for l in &self.chain {
            let local = if l.channels.is_empty() { l.local } else { animated(&l.local, &l.channels, t) };
            w = grid_formats::scene::compose(&local, &w);
        }
        Mat4::from_cols_array(&w)
    }

    /// The number of animated nodes above the line.
    pub fn animated(&self) -> usize {
        self.chain.iter().filter(|l| !l.channels.is_empty()).count()
    }
}

/// The panel's text lines in `scene`: the glyph grafted under the instance,
/// each node between them moved by the animation's channels.
pub fn lines(scene: &osd::Scene<'_>) -> Result<Vec<Line>, String> {
    let instance = scene.nodes.iter().position(|n| n.name() == INSTANCE).ok_or(format!("no {INSTANCE}"))?;
    let anim = scene.animations.iter().find(|a| a.id == ANIMATION).ok_or(format!("no {ANIMATION}"))?;
    let parent = scene.nodes[instance].parent.map_or(Mat4::IDENTITY.to_cols_array(), |p| scene.world(p));
    let placed = osd_glyph::expand_with(&[scene], 0, instance, &parent, &|_, n| scene.nodes[n].local);
    let mut out = Vec::new();
    for (i, p) in placed.iter().enumerate() {
        let node = &scene.nodes[p.node];
        let (Some(layout), Some(render)) = (&node.text, &node.render) else {
            continue;
        };
        let line = match node.name() {
            "text=1" => 0,
            "text=2" => 1,
            "text=3" => 2,
            _ => continue,
        };
        let m = &scene.materials[render.material];
        let (Some(family), Some(values)) = (fe3d::family(&m.effect), fe3d::values(m)) else {
            return Err(format!("{}: effect {}", node.id, m.effect));
        };
        let mut chain = Vec::new();
        let mut at = Some(i);
        while let Some(k) = at.filter(|&k| k != 0) {
            let n = &scene.nodes[placed[k].node];
            let channels = anim.channels.iter().filter(|c| c.target == n.id || c.target == n.name()).cloned().collect();
            chain.push(Link { local: n.local, channels });
            at = placed[k].parent;
        }
        chain.reverse();
        out.push(Line {
            line,
            node: node.id.clone(),
            base: placed[0].world,
            chain,
            layout: layout.clone(),
            bounds: node.bounds,
            family,
            values,
            dirt: m.texture("TDirt1").map(str::to_owned),
            pattern: m.texture("TPattern").map(str::to_owned),
        });
    }
    out.sort_by_key(|l| l.line);
    if out.iter().map(|l| l.line).ne(0..3) {
        return Err(format!("{INSTANCE}: lines {:?}", out.iter().map(|l| &l.node).collect::<Vec<_>>()));
    }
    Ok(out)
}

// ------------------------------------------------------------------ app

#[derive(Resource)]
struct Live {
    lines: Vec<Line>,
    entities: Vec<Entity>,
    fonts: Vec<osd_scene::Font>,
    materials: Vec<Handle<Fe3dMaterial>>,
    lights: fe3d::Lights,
    pulse: fe3d::Pulse,
    camera: Option<Entity>,
    cycle: Cycle,
    rand: Rand,
    /// The valid facts' lines, in shown order.
    facts: Vec<[String; 3]>,
    /// The fact shown; starts at the count so the first refresh wraps to 0 [D 0x4DFAD0].
    cursor: usize,
}

/// Adds the panel to the loading app, lit for the world's `camera`.
pub fn install(app: &mut App, root: &Path, camera: Entity) -> Result<(), String> {
    let read = |p: &str| std::fs::read(root.join(p)).map_err(|e| format!("{p}: {e}"));
    let spec = &crate::fe_world::LOADING;
    let bytes = read(spec.package)?;
    let tree = pssg::read_tree(&bytes).map_err(|e| format!("{}: {e}", spec.package))?;
    let scene = osd::read_scene(&tree).map_err(|e| format!("{}: {e}", spec.package))?;
    let lines = lines(&scene)?;
    let common = String::from_utf8_lossy(&read("frontend/persistentdatacommon.xml")?).into_owned();
    let timing = Timing::read(&common).ok_or("persistentdatacommon.xml: no <loading_stats> times")?;
    let params = String::from_utf8_lossy(&read("frontend/fe_paramcontrol.xml")?).into_owned();
    let lights = fe3d::Lights::read(&params, spec.lights, spec.zone).unwrap_or_default();
    let bundle_bytes = read(spec.bundle)?;
    let bundle = pssg::read_tree(&bundle_bytes).map_err(|e| format!("{}: {e}", spec.bundle))?;
    let cubes = osd::read_cubemaps(&bundle).map_err(|e| format!("{}: {e}", spec.bundle))?;
    let cube = cubes.iter().find(|c| c.id == spec.reflection).ok_or(format!("{}: no {}", spec.bundle, spec.reflection))?;
    // fe3d reads raw (gamma-space) texels.
    let mut cube = osd_scene::cube(cube);
    cube.texture_descriptor.format = TextureFormat::Rgba8Unorm;
    let mut images: HashMap<String, Image> = HashMap::new();
    for name in lines.iter().flat_map(|l| [&l.dirt, &l.pattern]).flatten() {
        if images.contains_key(name) {
            continue;
        }
        for t in texture::read_named(&bundle, &[name.as_bytes()]).map_err(|e| format!("{}: {e}", spec.bundle))? {
            let wrap = texture::wrap(&bundle, t.id).unwrap_or([texture::Wrap::Repeat; 2]);
            let (dds, _) = t.validated_dds();
            let image = crate::texture_upload::image_linear(&dds, osd_scene::sampler(wrap), RenderAssetUsages::RENDER_WORLD)
                .map_err(|e| format!("{name}: {e}"))?;
            images.insert(name.clone(), image);
        }
    }
    let mut by_name: HashMap<String, osd_scene::Font> = HashMap::new();
    for l in &lines {
        if by_name.contains_key(&l.layout.font) {
            continue;
        }
        let path = ["PSSG", "pssg"]
            .iter()
            .map(|ext| format!("frontend/fonts/{}.{ext}", l.layout.font))
            .find(|p| root.join(p).is_file())
            .ok_or(format!("font {} missing", l.layout.font))?;
        let bytes = read(&path)?;
        let tree = pssg::read_tree(&bytes).map_err(|e| format!("{path}: {e}"))?;
        by_name.insert(l.layout.font.clone(), osd_scene::font(osd::read_font(&tree).map_err(|e| format!("{path}: {e}"))?));
    }
    let fonts = lines.iter().map(|l| by_name[&l.layout.font].clone()).collect();
    let stats = Stats::read(crate::settings::data_dir());
    let labels = crate::pause_menu::Labels::load(root);
    let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_secs() as u32);
    let mut rand = Rand(seed);
    let facts: Vec<[String; 3]> =
        order(&stats, &mut rand).into_iter().filter_map(|i| texts(i, &stats, &|id| labels.get(id).to_owned())).collect();
    println!(
        "loading_stats lines={} animated={} facts={} mask={} on={} off={} animations={} fonts={:?} textures={}",
        lines.len(),
        lines.iter().filter(|l| l.animated() > 0).count(),
        facts.len(),
        stats.mask(),
        timing.on,
        timing.off,
        timing.animations,
        lines.iter().map(|l| l.layout.font.as_str()).collect::<Vec<_>>(),
        images.len()
    );
    fe3d::plugin(app);
    let world = app.world_mut();
    let reflection = world.resource_mut::<Assets<Image>>().add(cube);
    let handles: HashMap<String, Handle<Image>> =
        images.into_iter().map(|(name, image)| (name, world.resource_mut::<Assets<Image>>().add(image))).collect();
    let image = |name: &Option<String>| name.as_ref().and_then(|n| handles.get(n)).cloned().unwrap_or_default();
    let constants = lights.constants(None, None, 0.);
    let defs: Vec<Fe3dMaterial> = lines
        .iter()
        .map(|l| Fe3dMaterial {
            reflection: reflection.clone(),
            dirt: image(&l.dirt),
            pattern: image(&l.pattern),
            values: l.values,
            constants,
            diffuse: Handle::default(),
        })
        .collect();
    let materials = defs.into_iter().map(|m| world.resource_mut::<Assets<Fe3dMaterial>>().add(m)).collect();
    spawn(app, Panel { lines, fonts, materials, lights, camera: Some(camera), cycle: Cycle::new(timing), rand, facts });
    Ok(())
}

/// What [`spawn`] puts up.
struct Panel {
    lines: Vec<Line>,
    fonts: Vec<osd_scene::Font>,
    materials: Vec<Handle<Fe3dMaterial>>,
    lights: fe3d::Lights,
    camera: Option<Entity>,
    cycle: Cycle,
    rand: Rand,
    facts: Vec<[String; 3]>,
}

fn spawn(app: &mut App, p: Panel) {
    let world = app.world_mut();
    let entities = p
        .lines
        .iter()
        .zip(&p.materials)
        .map(|(l, m)| {
            world
                .spawn((
                    Transform::from_matrix(l.world(0.)),
                    Visibility::Hidden,
                    Mesh3d::default(),
                    MeshMaterial3d(m.clone()),
                    bevy::camera::visibility::NoFrustumCulling,
                ))
                .id()
        })
        .collect();
    let cursor = p.facts.len();
    world.insert_resource(Live {
        lines: p.lines,
        entities,
        fonts: p.fonts,
        materials: p.materials,
        lights: p.lights,
        pulse: fe3d::Pulse::default(),
        camera: p.camera,
        cycle: p.cycle,
        rand: p.rand,
        facts: p.facts,
        cursor,
    });
    app.add_systems(Update, update);
}

/// Runs the cycle by the app's clock, places the lines at its key time and
/// lights them for the world's camera.
fn update(
    live: Option<ResMut<Live>>,
    time: Res<Time>,
    cameras: Query<&GlobalTransform>,
    mut parts: Query<(&mut Transform, &mut Visibility, &mut Mesh3d)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<Fe3dMaterial>>,
) {
    let Some(mut live) = live else {
        return;
    };
    let live = &mut *live;
    if live.facts.is_empty() {
        return;
    }
    let dt = time.delta_secs();
    let rand = &mut live.rand;
    let step = live.cycle.step(dt, &mut |n| 1 + rand.next() as usize % n.max(1) as usize);
    if step.refreshed {
        live.cursor = if live.cursor + 1 >= live.facts.len() { 0 } else { live.cursor + 1 };
        let fact = &live.facts[live.cursor];
        for (k, l) in live.lines.iter().enumerate() {
            let Ok((_, mut visibility, mut mesh)) = parts.get_mut(live.entities[k]) else {
                continue;
            };
            let text = &fact[l.line];
            if text.trim().is_empty() {
                visibility.set_if_neq(Visibility::Hidden);
                continue;
            }
            let m = osd_scene::text_mesh(&live.fonts[k], text, &l.layout, &l.bounds);
            mesh.0 = meshes.add(fe3d::mesh(&m, &l.bounds, l.family));
            visibility.set_if_neq(Visibility::Inherited);
        }
        println!("loading_stats fact={} n={} lines={fact:?}", live.cursor, live.cycle.n);
    }
    for (k, l) in live.lines.iter().enumerate() {
        if let Ok((mut transform, _, _)) = parts.get_mut(live.entities[k]) {
            let placed = Transform::from_matrix(l.world(live.cycle.time));
            if *transform != placed {
                *transform = placed;
            }
        }
    }
    live.pulse.advance(dt, live.lights.g_rate);
    let camera = live.camera.and_then(|c| cameras.get(c).ok()).map(GlobalTransform::to_matrix);
    let constants = live.lights.constants(camera.as_ref(), None, live.pulse.value(live.lights.g_swing));
    for h in &live.materials {
        if let Some(mut m) = materials.get_mut(h)
            && m.constants != constants
        {
            m.constants = constants;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TIMING: Timing = Timing { animations: 5, on: 1.5, off: 0. };

    #[test]
    fn msvc_rand_and_random_shuffle() {
        let mut r = Rand(1);
        assert_eq!([r.next(), r.next(), r.next(), r.next()], [41, 18467, 6334, 26500]);
        // Seed 1: i=3 swaps with 6334 % 4 = 2, i=4 with 26500 % 5 = 0.
        let mut v = [0, 1, 2, 3, 4];
        Rand(1).shuffle(&mut v);
        assert_eq!(v, [4, 1, 3, 2, 0]);
    }

    #[test]
    fn facts_follow_the_mode_and_the_save() {
        assert_eq!(SLOTS.len(), 38);
        assert_eq!(SLOTS.iter().filter(|s| s.mask & 0x40 != 0).count(), 4);
        // No save: career results only, with 0 wins.
        let none = Stats::default();
        assert_eq!(none.mask(), 1);
        assert_eq!(order(&none, &mut Rand(7)), [5]);
        let label = |id: &str| match id {
            "fact_career_results_header" => "Career History".to_owned(),
            "fact_career_results_body" => "[VALUE] wins".to_owned(),
            "fact_owned_cars_body" => "[VALUE] owned".to_owned(),
            other => other.to_owned(),
        };
        assert_eq!(texts(5, &none, &label).unwrap(), ["Career History".to_owned(), "0 wins".into(), String::new()]);
        assert!(texts(6, &none, &label).is_none());
        // A save: the three counted facts, shuffled.
        let save = Stats { career: true, wins: 4, cars: 2, offers: 7 };
        assert_eq!(save.mask(), 3);
        let mut o = order(&save, &mut Rand(1));
        o.sort();
        assert_eq!(o, [5, 6, 21]);
        assert_eq!(texts(6, &save, &label).unwrap()[1], "2 owned");
        // Mask 1 slots without a statistic stay out (top speed, biggest drift).
        assert!(save.values(1).is_none() && save.values(32).is_none());
    }

    #[test]
    fn timing_reads_the_common_block() {
        let xml = r#"<x><loading_stats definedanimations="5">
            <on_time minimum="0.8" desired="1.5" /><off_time minimum="0" desired="0" maximum="0.95" />
        </loading_stats></x>"#;
        assert_eq!(Timing::read(xml), Some(TIMING));
    }

    #[test]
    fn the_cycle_flies_in_holds_and_flies_out() {
        let mut c = Cycle::new(TIMING);
        assert_eq!(c.time, 0.);
        let mut pick = |n: u32| {
            assert_eq!(n, 5);
            3
        };
        let dt = 0.1;
        // The first step takes a fact and, with no off time, starts in 3.
        let s = c.step(dt, &mut pick);
        assert!(s.refreshed && s.played_in && !s.played_out);
        assert_eq!((c.n, c.time), (3, IN[2].0));
        // In plays over 1.3 s to its end key.
        for _ in 0..13 {
            assert_eq!(c.step(dt, &mut pick), Step::default());
        }
        assert!((c.time - IN[2].1).abs() < 1e-4, "{}", c.time);
        // Shown for 1.5 s, then out 3.
        let mut steps = 0;
        let out = loop {
            steps += 1;
            let s = c.step(dt, &mut pick);
            if s.played_out {
                break s;
            }
        };
        assert!(!out.refreshed && (14..=17).contains(&steps), "{steps}");
        assert_eq!(c.time, OUT[2].0);
        for _ in 0..8 {
            c.step(dt, &mut pick);
        }
        assert!((c.time - OUT[2].1).abs() < 1e-4);
        // Then the next fact flies in.
        let s = c.step(dt, &mut pick);
        assert!(s.refreshed && s.played_in);
    }

    fn line(line: usize, x: f32) -> Line {
        let curve = |a: f32, b: f32| {
            let key = |time: f32, value: f32| osd::Key { time, value, in_value: value, in_time: time, out_value: value, out_time: time };
            osd::Curve(vec![key(0., a), key(14., b)])
        };
        Line {
            line,
            node: format!("text={}", line + 1),
            base: Mat4::IDENTITY.to_cols_array(),
            chain: vec![
                Link {
                    local: Mat4::IDENTITY.to_cols_array(),
                    channels: vec![osd::Channel {
                        target: "anim_top".into(),
                        semantic: osd::Semantic::Translation,
                        components: vec![curve(x, x), curve(0., 0.), curve(-250., 0.)],
                    }],
                },
                Link { local: Mat4::from_translation(Vec3::Y).to_cols_array(), channels: vec![] },
            ],
            layout: osd::TextLayout {
                font: "f".into(),
                height: 1.,
                depth: 0.,
                h_align: 0,
                v_align: 0,
                wrapping: 0,
                line_spacing: 1.,
                tracking: 0.,
                case: 0,
            },
            bounds: [0., 0., 0., 10., 1., 0.],
            family: fe3d::GPD,
            values: fe3d::Values::default(),
            dirt: None,
            pattern: None,
        }
    }

    #[test]
    fn a_line_follows_its_helper() {
        let l = line(0, 2.);
        assert_eq!(l.animated(), 1);
        assert!(l.world(0.).w_axis.truncate().distance(Vec3::new(2., 1., -250.)) < 1e-3);
        let mid = l.world(7.).w_axis.truncate();
        assert!((mid.z - -125.).abs() < 1., "{mid:?}");
    }

    /// The interactive path: the window app's clock runs the system, which
    /// shows the first fact's non-empty lines and moves them in.
    #[test]
    fn the_app_clock_runs_the_panel() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<Image>()
            .init_asset::<Fe3dMaterial>()
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_millis(100)));
        let materials: Vec<_> = (0..3)
            .map(|_| {
                app.world_mut().resource_mut::<Assets<Fe3dMaterial>>().add(Fe3dMaterial {
                    reflection: Handle::default(),
                    dirt: Handle::default(),
                    pattern: Handle::default(),
                    values: fe3d::Values::default(),
                    constants: fe3d::Constants::default(),
                    diffuse: Handle::default(),
                })
            })
            .collect();
        spawn(
            &mut app,
            Panel {
                lines: (0..3).map(|i| line(i, i as f32)).collect(),
                fonts: vec![osd_scene::Font::default(); 3],
                materials,
                lights: fe3d::Lights::default(),
                camera: None,
                cycle: Cycle::new(TIMING),
                rand: Rand(1),
                facts: vec![["Career History".to_owned(), "3 wins".into(), String::new()]],
            },
        );
        for _ in 0..4 {
            app.update();
        }
        let mut q = app.world_mut().query::<(&Visibility, &Transform)>();
        let parts: Vec<_> = q.iter(app.world()).map(|(v, t)| (*v != Visibility::Hidden, t.translation)).collect();
        assert_eq!(parts.iter().filter(|(v, _)| *v).count(), 2, "header and body, no details: {parts:?}");
        let live = app.world().resource::<Live>();
        assert!(live.cycle.time > IN[live.cycle.n - 1].0, "playing in: {}", live.cycle.time);
        assert_eq!(live.cursor, 0);
        // The lines left their t = 0 place (Z -250) for the segment's.
        assert!(parts.iter().all(|(_, t)| t.z > -250.), "{parts:?}");
    }

    /// The panel from the install: three GPD lines under the instance, moved
    /// by `Anim!1`, each segment's end in front of `load_cam`'s saved place.
    #[test]
    #[ignore = "needs the GRID install (GRID_GAME_ROOT)"]
    fn the_panel_reads_from_the_install() {
        let root = std::env::var("GRID_GAME_ROOT").expect("GRID_GAME_ROOT");
        let root = Path::new(&root);
        let spec = &crate::fe_world::LOADING;
        let bytes = std::fs::read(root.join(spec.package)).unwrap();
        let tree = pssg::read_tree(&bytes).unwrap();
        let scene = osd::read_scene(&tree).unwrap();
        let lines = lines(&scene).unwrap();
        let cam = scene.cameras.iter().find(|c| scene.nodes[c.node].id == "camera=load_cam").unwrap();
        let view = Mat4::from_cols_array(&scene.world(cam.node)).inverse();
        let proj = Mat4::perspective_rh(cam.fov_degrees.to_radians(), 16. / 9., 0.1, 10_000.);
        for l in &lines {
            println!(
                "line {} {} font={} height={} h_align={} bounds={:?} family={} animated={} dirt={:?} pattern={:?}",
                l.line, l.node, l.layout.font, l.layout.height, l.layout.h_align, l.bounds, l.family, l.animated(), l.dirt, l.pattern
            );
            assert_eq!(l.family, fe3d::GPD);
            assert!(l.animated() > 0, "{} has no animated helper", l.node);
            for (n, (from, to)) in IN.iter().enumerate() {
                let p = l.world(*to).w_axis.truncate();
                let v = view.transform_point3(p);
                let ndc = proj.project_point3(v);
                println!("  in {} t={from}..{to} end={p:?} ndc=({:.2},{:.2}) start={:?}", n + 1, ndc.x, ndc.y, l.world(*from).w_axis.truncate());
                assert!(v.z < 0. && ndc.x.abs() < 1.2 && ndc.y.abs() < 1.2, "line {} in {} ends off screen: {ndc:?}", l.line, n + 1);
            }
        }
        let common = std::fs::read_to_string(root.join("frontend/persistentdatacommon.xml")).unwrap();
        assert_eq!(Timing::read(&common), Some(TIMING));
        let fonts: Vec<_> = lines.iter().map(|l| root.join(format!("frontend/fonts/{}.PSSG", l.layout.font))).collect();
        assert!(fonts.iter().all(|f| f.is_file() || f.with_extension("pssg").is_file()), "{fonts:?}");
    }
}
