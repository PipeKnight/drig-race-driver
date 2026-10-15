//! Tyre smoke ("kickup") data from the purchased installation. Semantics and
//! labels are in docs/TYRE_SMOKE_RESEARCH.md.
use grid_formats::binxml;
use std::{collections::HashMap, path::Path};

type Error = Box<dyn std::error::Error>;

/// Generic element tree shared by binary and plain XML sources.
#[derive(Debug, Default, Clone)]
pub struct Node {
    pub name: String,
    pub attributes: Vec<(String, String)>,
    pub children: Vec<Node>,
}
impl Node {
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
    fn float(&self, name: &str) -> Result<Option<f32>, Error> {
        self.attr(name)
            .map(|v| {
                let v: f32 = v.trim().parse()?;
                if v.is_finite() {
                    Ok(v)
                } else {
                    Err("non-finite particle value".into())
                }
            })
            .transpose()
    }
    fn floats<const N: usize>(&self, name: &str) -> Result<Option<[f32; N]>, Error> {
        let Some(v) = self.attr(name) else {
            return Ok(None);
        };
        let values = v
            .split_whitespace()
            .map(str::parse::<f32>)
            .collect::<Result<Vec<_>, _>>()?;
        if values.len() != N || values.iter().any(|v| !v.is_finite()) {
            return Err(format!("{name} needs {N} finite values").into());
        }
        Ok(Some(values.try_into().unwrap()))
    }
    fn float_list(&self, name: &str) -> Result<Option<Vec<f32>>, Error> {
        let Some(v) = self.attr(name) else {
            return Ok(None);
        };
        let values = v
            .split_whitespace()
            .map(str::parse::<f32>)
            .collect::<Result<Vec<_>, _>>()?;
        if values.is_empty() || values.len() > 64 || values.iter().any(|v| !v.is_finite()) {
            return Err(format!("{name} needs 1..64 finite values").into());
        }
        Ok(Some(values))
    }
    pub fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.name == name)
    }
    pub fn all<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> {
        self.children.iter().filter(move |c| c.name == name)
    }
}

fn from_binxml(document: &binxml::Document<'_>) -> Result<Node, Error> {
    fn build(d: &binxml::Document<'_>, i: usize, depth: usize) -> Result<Node, Error> {
        if depth > 32 {
            return Err("particle XML too deep".into());
        }
        let e = &d.elements[i];
        Ok(Node {
            name: e.name.to_owned(),
            attributes: e
                .attributes
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
            children: e
                .children
                .clone()
                .map(|c| build(d, c, depth + 1))
                .collect::<Result<_, _>>()?,
        })
    }
    if document.elements.is_empty() {
        return Err("empty binary XML".into());
    }
    build(document, 0, 0)
}

/// Minimal plain-XML reader for the shipped effect configuration files:
/// elements and quoted attributes only; comments, declarations and text are
/// skipped. Not a general XML parser.
pub fn parse_plain(text: &str) -> Result<Node, Error> {
    let text = text.trim_start_matches('\u{feff}');
    let mut stack = vec![Node::default()];
    let mut rest = text;
    while let Some(start) = rest.find('<') {
        rest = &rest[start..];
        if let Some(r) = rest.strip_prefix("<!--") {
            let end = r.find("-->").ok_or("unterminated XML comment")?;
            rest = &r[end + 3..];
            continue;
        }
        let end = rest.find('>').ok_or("unterminated XML tag")?;
        let tag = &rest[1..end];
        rest = &rest[end + 1..];
        if tag.starts_with('?') || tag.starts_with('!') {
            continue;
        }
        if let Some(name) = tag.strip_prefix('/') {
            let node = stack.pop().ok_or("unbalanced XML")?;
            if node.name != name.trim() || stack.is_empty() {
                return Err(format!("mismatched closing tag {name}").into());
            }
            stack.last_mut().unwrap().children.push(node);
            continue;
        }
        let closed = tag.ends_with('/');
        let tag = tag.trim_end_matches('/');
        let name_end = tag.find(|c: char| c.is_whitespace()).unwrap_or(tag.len());
        let mut node = Node {
            name: tag[..name_end].to_owned(),
            ..Default::default()
        };
        let mut attrs = &tag[name_end..];
        loop {
            attrs = attrs.trim_start();
            if attrs.is_empty() {
                break;
            }
            let eq = attrs.find('=').ok_or("XML attribute without value")?;
            let key = attrs[..eq].trim().to_owned();
            let after = attrs[eq + 1..].trim_start();
            let quote = after.chars().next().ok_or("XML attribute without value")?;
            if quote != '"' && quote != '\'' {
                return Err("unquoted XML attribute".into());
            }
            let close = after[1..].find(quote).ok_or("unterminated XML attribute")?;
            node.attributes.push((key, after[1..1 + close].to_owned()));
            attrs = &after[close + 2..];
        }
        if closed {
            stack.last_mut().unwrap().children.push(node);
        } else {
            if stack.len() > 32 {
                return Err("plain XML too deep".into());
            }
            stack.push(node);
        }
    }
    if stack.len() != 1 {
        return Err("unclosed XML element".into());
    }
    let mut root = stack.pop().unwrap();
    if root.children.len() != 1 {
        return Err("expected one XML root element".into());
    }
    Ok(root.children.pop().unwrap())
}

fn read_bounded(path: &Path) -> Result<Vec<u8>, Error> {
    let bytes = std::fs::read(path)?;
    if bytes.len() > 4 << 20 {
        return Err("effect source exceeds bound".into());
    }
    Ok(bytes)
}
/// Binary or plain XML by magic.
pub fn read_xml(path: &Path) -> Result<Node, Error> {
    let bytes = read_bounded(path)?;
    if bytes.starts_with(&0x7252221au32.to_le_bytes()) {
        from_binxml(&binxml::read(&bytes)?)
    } else {
        parse_plain(std::str::from_utf8(&bytes)?)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScaleType {
    Linear,
    QuadraticAccelerate,
    QuadraticDecelerate,
    CubicAccelerate,
    CubicDecelerate,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeLocation {
    Volume,
    Surface,
    Pivot,
}
/// One `<emitter>` of a particle-system library; absent attributes stay `None`.
#[derive(Debug, Clone)]
pub struct Emitter {
    pub id: u32,
    pub name: String,
    pub horizon: bool,
    pub texture: String,
    /// Atlas layout (frames, columns, rows) for `textureType="frequency"`.
    pub atlas: Option<(u32, u32, u32)>,
    pub frequencies: Option<Vec<f32>>,
    pub gravity: Option<f32>,
    /// (influence, rangeX)
    pub drag: Option<(f32, f32)>,
    pub angular_drag: Option<(f32, f32)>,
    /// box/rectangle extents (length, width, height) and location.
    pub extent: [f32; 3],
    pub location: ShapeLocation,
    pub rate: f32,
    pub number: Option<f32>,
    pub parent_relative: bool,
    pub life: Option<f32>,
    pub life_variance: Option<f32>,
    pub speed: Option<f32>,
    pub speed_variation: Option<f32>,
    pub divergence: Option<f32>,
    pub inherit: Option<f32>,
    pub spin_rate: Option<f32>,
    pub spin_variance: Option<f32>,
    pub world_spin: bool,
    pub initial_rotation_variance: Option<f32>,
    pub growth: Option<f32>,
    pub initial_size: [Option<f32>; 2],
    pub final_size: [Option<f32>; 2],
    pub scale_variance: Option<f32>,
    pub scale_type: Option<ScaleType>,
    pub alpha: [Option<f32>; 3],
    pub alpha_in: Option<f32>,
    pub alpha_out: Option<f32>,
    pub colour: [Option<[f32; 3]>; 2],
}
#[derive(Debug, Clone)]
pub struct Birth {
    pub emitter: usize,
    pub start: Option<f32>,
    pub end: Option<f32>,
    pub position: [f32; 3],
    /// Degrees as stored.
    pub orientation: [f32; 3],
}
#[derive(Debug, Clone)]
pub struct System {
    pub name: String,
    pub emitters: Vec<Emitter>,
    pub births: Vec<Birth>,
}

fn parse_emitter(e: &Node) -> Result<Emitter, Error> {
    let no_model = Node::default();
    let model = e.child("model").unwrap_or(&no_model);
    // Mesh emitters (debris) carry no material; their texture stays empty.
    let no_material = Node::default();
    let material = e.child("material").unwrap_or(&no_material);
    let (texture, atlas, frequencies) = if let Some(t) = material.child("materialtexture") {
        let n = |k: &str| -> Result<u32, Error> {
            Ok(t.attr(k).ok_or("atlas attribute absent")?.trim().parse()?)
        };
        let atlas = (n("numFrames")?, n("widthInBlocks")?, n("heightInBlocks")?);
        if atlas.0 == 0 || atlas.1 == 0 || atlas.2 == 0 || atlas.0 > atlas.1 * atlas.2 {
            return Err("invalid particle atlas".into());
        }
        (
            t.attr("filename")
                .ok_or("atlas without filename")?
                .to_owned(),
            Some(atlas),
            material
                .child("properties")
                .map(|p| p.float_list("frequencies"))
                .transpose()?
                .flatten(),
        )
    } else {
        (
            material.attr("textureName").unwrap_or_default().to_owned(),
            None,
            None,
        )
    };
    let force = |kind: &str| e.all("force").find(|f| f.attr("type") == Some(kind));
    let pair = |kind: &str| -> Result<Option<(f32, f32)>, Error> {
        force(kind)
            .map(|f| {
                Ok((
                    f.float("influence")?.unwrap_or(0.),
                    f.float("rangeX")?.unwrap_or(0.),
                ))
            })
            .transpose()
    };
    let empty = Node::default();
    let shape = e.child("emitterShape").unwrap_or(&empty);
    let spawn = e.child("spawn").unwrap_or(&empty);
    let props = e.child("particleProperties").unwrap_or(&empty);
    let part = |k: &str| props.child(k).unwrap_or(&empty);
    let (life, speed, spin, agescale, alpha, colour) = (
        part("life"),
        part("speed"),
        part("spin"),
        part("agescale"),
        part("alpha"),
        part("colour"),
    );
    let inherit = if speed.attr("inheritemitter") == Some("true") {
        Some(speed.float("inheritamount")?.unwrap_or(1.))
    } else {
        None
    };
    Ok(Emitter {
        id: e.attr("id").ok_or("emitter without id")?.parse()?,
        name: e.attr("name").unwrap_or_default().to_owned(),
        horizon: model.attr("billboardingType") == Some("HorizonAligned"),
        texture,
        atlas,
        frequencies,
        gravity: force("gravity")
            .map(|f| f.float("influence"))
            .transpose()?
            .flatten(),
        drag: pair("drag")?,
        angular_drag: pair("angulardrag")?,
        extent: [
            shape.float("length")?.unwrap_or(0.),
            shape.float("width")?.unwrap_or(0.),
            shape.float("height")?.unwrap_or(0.),
        ],
        location: match shape.attr("location") {
            Some("surface") => ShapeLocation::Surface,
            Some("pivot") => ShapeLocation::Pivot,
            _ => ShapeLocation::Volume,
        },
        rate: spawn.float("rate")?.unwrap_or(0.),
        number: spawn.float("number")?,
        parent_relative: e
            .child("parentRelative")
            .and_then(|p| p.attr("emitterRelativeParticles"))
            == Some("true"),
        life: life.float("life")?,
        life_variance: life.float("variance")?,
        speed: speed.float("speed")?,
        speed_variation: speed.float("variation")?,
        divergence: speed.float("divergence")?,
        inherit,
        spin_rate: spin.float("rate")?,
        spin_variance: spin.float("variance")?,
        world_spin: spin.attr("axisType") == Some("WorldSpace"),
        initial_rotation_variance: spin.float("initialRotationVariance")?,
        growth: part("scale").float("growthPercentage")?,
        initial_size: [
            agescale.float("initialWidth")?,
            agescale.float("initialHeight")?,
        ],
        final_size: [
            agescale.float("finalWidth")?,
            agescale.float("finalHeight")?,
        ],
        scale_variance: agescale.float("scaleVariance")?,
        scale_type: match agescale.attr("scaleType") {
            None => None,
            Some("linear") => Some(ScaleType::Linear),
            Some("quadraticAccelerate") => Some(ScaleType::QuadraticAccelerate),
            Some("quadraticDecelerate") => Some(ScaleType::QuadraticDecelerate),
            Some("cubicAccelerate") => Some(ScaleType::CubicAccelerate),
            Some("cubicDecelerate") => Some(ScaleType::CubicDecelerate),
            Some(other) => return Err(format!("unknown particle scale type {other}").into()),
        },
        alpha: [
            alpha.float("start")?,
            alpha.float("middle")?,
            alpha.float("end")?,
        ],
        alpha_in: alpha.float("inTime")?,
        alpha_out: alpha.float("outTime")?,
        colour: [colour.floats::<3>("start")?, colour.floats::<3>("end")?],
    })
}

pub fn parse_system(root: &Node) -> Result<System, Error> {
    let library = root.child("EmitterLibrary").ok_or("no emitter library")?;
    let emitters = library
        .all("emitter")
        .map(parse_emitter)
        .collect::<Result<Vec<_>, _>>()?;
    let system = root
        .child("SystemLibrary")
        .and_then(|l| l.child("System"))
        .ok_or("no particle system")?;
    let births = system
        .child("Births")
        .ok_or("system without births")?
        .all("birth")
        .map(|b| {
            let id: u32 = b
                .attr("emitterID")
                .ok_or("birth without emitter")?
                .parse()?;
            Ok(Birth {
                emitter: emitters
                    .iter()
                    .position(|e| e.id == id)
                    .ok_or("birth names an unknown emitter")?,
                start: b.float("start")?,
                end: b.float("end")?,
                position: b.floats::<3>("relPosition")?.unwrap_or_default(),
                orientation: b.floats::<3>("relOrientation")?.unwrap_or_default(),
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    Ok(System {
        name: system.attr("name").unwrap_or_default().to_owned(),
        emitters,
        births,
    })
}

/// `<kickup><dry>` of one particle material.
#[derive(Debug, Clone, PartialEq)]
pub struct Kickup {
    pub effect: String,
    pub front: Option<String>,
    pub open: Option<String>,
    pub ai: Option<String>,
    /// high, medium, low, static, rate, speed
    pub multipliers: [f32; 6],
    pub tyreslip: f32,
    pub downforce: f32,
}

pub fn parse_kickups(root: &Node) -> Result<HashMap<String, Kickup>, Error> {
    let mut out = HashMap::new();
    for p in root.all("PARTICLE") {
        let Some(dry) = p.child("kickup").and_then(|k| k.child("dry")) else {
            continue;
        };
        let Some(effect) = dry.child("effect") else {
            continue;
        };
        let m = dry.child("multipliers");
        let mult = |k: &str| -> Result<f32, Error> {
            Ok(m.map(|m| m.float(k)).transpose()?.flatten().unwrap_or(0.))
        };
        let special = dry.child("special");
        let sp = |k: &str| -> Result<f32, Error> {
            Ok(special
                .map(|s| s.float(k))
                .transpose()?
                .flatten()
                .unwrap_or(0.))
        };
        out.insert(
            p.attr("name")
                .ok_or("particle material without name")?
                .to_owned(),
            Kickup {
                effect: effect.attr("name").unwrap_or("none").to_owned(),
                front: effect.attr("frontWheelName").map(str::to_owned),
                open: effect.attr("openWheelName").map(str::to_owned),
                ai: effect.attr("AIName").map(str::to_owned),
                multipliers: [
                    mult("high")?,
                    mult("medium")?,
                    mult("low")?,
                    mult("static")?,
                    mult("rate")?,
                    mult("speed")?,
                ],
                tyreslip: sp("tyreslip")?,
                downforce: sp("downforce")?,
            },
        );
    }
    Ok(out)
}

/// Surface token (1-based material order) -> (mechanics name, particle name).
pub fn parse_surface_particles(root: &Node) -> Vec<(Option<String>, Option<String>)> {
    root.all("MATERIAL")
        .map(|m| {
            (
                m.child("MECHANICS")
                    .and_then(|c| c.attr("name"))
                    .map(str::to_owned),
                m.child("PARTICLE")
                    .and_then(|c| c.attr("name"))
                    .map(str::to_owned),
            )
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Per-wheel inputs (0x6574F0 reads; field meanings in TYRE_SMOKE_RESEARCH.md)

/// One wheel as the kickup driver reads it.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct WheelInput {
    /// Contact surface token (wheel+C0, 0 = airborne).
    pub token: u32,
    /// Contact point in the car frame: wheel centre lowered by radius (+E0).
    pub contact: [f32; 3],
    /// Wheel angular speed (wheel+104).
    pub spin: f32,
    /// Wheel load (wheel+138).
    pub load: f32,
    /// Smoothed grip-overflow force (wheel+AC).
    pub excess: f32,
    /// Handbrake applies to this wheel (wheel+FC byte).
    pub handbraked: bool,
    /// wheel+148 (rear term; meaning not identified).
    pub rear_term: f32,
    /// Steering direction words (wheel+28, wheel+2C).
    pub steer: [f32; 2],
    pub front: bool,
    pub left: bool,
}
/// One car for wheel effects.
#[derive(Debug, Clone, PartialEq)]
pub struct CarInput {
    /// Column-major world pose (physical +1D0); car frame +Y up, +Z forward.
    pub pose: [f32; 16],
    /// Body linear velocity (inherited by emitters).
    pub velocity: [f32; 3],
    /// Car speed (body+1648).
    pub speed: f32,
    /// body+15A0.
    pub mass: f32,
    /// Handbrake byte body+1640.
    pub handbrake: bool,
    /// body+1008 (rear handling coefficient).
    pub rear_coefficient: f32,
    /// Open-wheel car (body+EDA8 != 0) [I].
    pub open_wheel: bool,
    pub ai: bool,
    pub wheels: [WheelInput; 4],
}
impl CarInput {
    pub fn read(runtime: &crate::source_vehicle::Runtime, ai: bool) -> Self {
        let b = &runtime.state.car.owner.vehicle;
        let f = |o: usize| f32::from_bits(b[o / 4]);
        let byte = |o: usize| (b[o / 4] >> (o % 4 * 8)) & 255;
        let poses = runtime.wheels();
        let v = runtime.velocity();
        let wheels = std::array::from_fn(|i| {
            let o = 0x1990 + i * 0x150;
            let p = &poses[i];
            let mut contact: [f32; 3] = std::array::from_fn(|a| {
                p.anchor[a] + p.deformation_translation[a] + p.axis[a] * p.displacement
            });
            contact[1] -= f(o + 0xe0);
            WheelInput {
                token: b[(o + 0xc0) / 4],
                contact,
                spin: f(o + 0x104),
                load: f(o + 0x138),
                excess: f(o + 0xac),
                handbraked: byte(o + 0xfc) != 0,
                rear_term: f(o + 0x148),
                steer: [f(o + 0x28), f(o + 0x2c)],
                front: p.anchor[2] > 0.,
                left: p.anchor[0] < 0.,
            }
        });
        Self {
            pose: runtime.pose(),
            velocity: [v[0], v[1], v[2]],
            speed: f(0x1648),
            mass: f(0x15a0),
            handbrake: byte(0x1640) != 0,
            rear_coefficient: f(0x1008),
            open_wheel: b[0xeda8 / 4] != 0,
            ai,
            wheels,
        }
    }
    fn rows(&self) -> Rows {
        std::array::from_fn(|a| [self.pose[a * 4], self.pose[a * 4 + 1], self.pose[a * 4 + 2]])
    }
    fn origin(&self) -> [f32; 3] {
        [self.pose[12], self.pose[13], self.pose[14]]
    }
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn length(a: [f32; 3]) -> f32 {
    dot(a, a).sqrt()
}

// ---------------------------------------------------------------------------
// Row-vector transforms (D3D convention: `world = local · rows`)

pub type Rows = [[f32; 3]; 3];
pub const IDENTITY: Rows = [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]];

/// `Rx(x)·Ry(y)·Rz(z)` in row-vector form from degrees (pfx parser order).
pub fn euler_rows(degrees: [f32; 3]) -> Rows {
    let [x, y, z] = degrees.map(f32::to_radians);
    let (sx, cx) = x.sin_cos();
    let (sy, cy) = y.sin_cos();
    let (sz, cz) = z.sin_cos();
    let rx = [[1., 0., 0.], [0., cx, sx], [0., -sx, cx]];
    let ry = [[cy, 0., -sy], [0., 1., 0.], [sy, 0., cy]];
    let rz = [[cz, sz, 0.], [-sz, cz, 0.], [0., 0., 1.]];
    mul(&mul(&rx, &ry), &rz)
}
pub fn mul(a: &Rows, b: &Rows) -> Rows {
    std::array::from_fn(|r| std::array::from_fn(|c| (0..3).map(|k| a[r][k] * b[k][c]).sum()))
}
/// `v · m` (row vector).
pub fn apply(v: [f32; 3], m: &Rows) -> [f32; 3] {
    std::array::from_fn(|c| v[0] * m[0][c] + v[1] * m[1][c] + v[2] * m[2][c])
}

// ---------------------------------------------------------------------------
// Library

/// Kickup configuration (`vehiclepfxconfig.xml`, `tunePfx.xml`) and the
/// driver's manager constants.
#[derive(Debug, Clone, PartialEq)]
pub struct Tuning {
    /// Speed band upper bounds: static, low, medium (vehicle pfx +C6C/+C68/+C64).
    pub speed_bands: [f32; 3],
    pub ignore_distance: f32,
    pub lod_max: f32,
    pub lod_min: f32,
    pub reduction_distance: f32,
    pub front_wheel: f32,
    pub front_wheel_ai: f32,
    pub max_particle_peak: f32,
    /// AUTHORED pool cap: config.xml totalParticles (the executable never
    /// reads the `<limits>` names).
    pub particle_limit: usize,
}

/// Emitter textures drawn as smoke; the remaining kickup emitters (mud,
/// grass, stones, sand bits) belong to the ground-effects module.
pub const SMOKE_TEXTURES: [&str; 5] = [
    "SM33.tga",
    "SM33_dark.tga",
    "SM38.tga",
    "Wisps08_32.tga",
    "Wheel_spin.tga",
];

/// Loaded kickup smoke data.
pub struct Library {
    /// Kickup systems with only their smoke emitters (births re-indexed).
    pub systems: Vec<System>,
    pub kickups: Vec<(String, Kickup)>,
    /// Per kickup: system index for default, front, open-wheel and AI names.
    choices: Vec<[Option<usize>; 4]>,
    /// Token - 1 -> kickup index.
    surfaces: Vec<Option<usize>>,
    pub tuning: Tuning,
    textures: Vec<String>,
    atlases: Vec<(u32, u32)>,
}

impl Library {
    pub fn load(root: &Path) -> Result<Self, Error> {
        let effects = root.join("effects");
        let mut kickups: Vec<(String, Kickup)> =
            parse_kickups(&read_xml(&effects.join("particleMaterials.xml"))?)?
                .into_iter()
                .collect();
        kickups.sort_by(|a, b| a.0.cmp(&b.0));
        let mut systems: Vec<System> = Vec::new();
        let mut choices = Vec::new();
        for (_, k) in &kickups {
            let mut choice = [None; 4];
            for (slot, name) in [
                Some(&k.effect),
                k.front.as_ref(),
                k.open.as_ref(),
                k.ai.as_ref(),
            ]
            .into_iter()
            .enumerate()
            {
                let Some(name) = name.filter(|n| n.starts_with("K_")) else {
                    continue;
                };
                let index = match systems.iter().position(|s| &s.name == name) {
                    Some(i) => i,
                    None => {
                        let path = effects.join(format!("particle_systems/{name}.xml"));
                        let mut system = parse_system(&read_xml(&path)?)?;
                        system.name = name.clone();
                        smoke_only(&mut system);
                        systems.push(system);
                        systems.len() - 1
                    }
                };
                if !systems[index].births.is_empty() {
                    choice[slot] = Some(index);
                }
            }
            choices.push(choice);
        }
        let surfaces = parse_surface_particles(&read_xml(&root.join("surface_materials.xml"))?)
            .iter()
            .map(|(_, particle)| {
                let i = kickups
                    .iter()
                    .position(|(n, _)| Some(n) == particle.as_ref())?;
                choices[i].iter().any(Option::is_some).then_some(i)
            })
            .collect();
        let vehicle = read_xml(&effects.join("vehiclepfxconfig.xml"))?;
        let ranges = vehicle
            .child("kickup")
            .and_then(|k| k.child("speedranges"))
            .ok_or("vehicle pfx without kickup speed ranges")?;
        let max = |n: &str| -> Result<f32, Error> {
            ranges
                .child(n)
                .ok_or("speed range absent")?
                .float("max")?
                .ok_or_else(|| "speed range max absent".into())
        };
        let tune = read_xml(&effects.join("tunePfx.xml"))?;
        let tweak = tune
            .child("effect_tweak")
            .ok_or("tunePfx without effect_tweak")?;
        let t = |k: &str| -> Result<f32, Error> {
            tweak
                .float(k)?
                .ok_or_else(|| format!("tunePfx {k} absent").into())
        };
        let limit = read_xml(&effects.join("config.xml"))?
            .child("settings")
            .and_then(|s| s.child("limits"))
            .ok_or("pfx config without limits")?
            .float("totalParticles")?
            .ok_or("totalParticles absent")?;
        let tuning = Tuning {
            speed_bands: [max("static")?, max("low")?, max("medium")?],
            ignore_distance: t("ignoreDist")?,
            lod_max: t("max")?,
            lod_min: t("min")?,
            reduction_distance: t("reductionDist")?,
            front_wheel: t("frontWheel")?,
            front_wheel_ai: t("frontWheelAi")?,
            max_particle_peak: t("maxParticlePeak")?,
            particle_limit: limit.clamp(0., 100_000.) as usize,
        };
        let mut textures: Vec<String> = Vec::new();
        let mut atlases = Vec::new();
        for e in systems.iter().flat_map(|s| &s.emitters) {
            if !textures.contains(&e.texture) {
                textures.push(e.texture.clone());
                atlases.push(e.atlas.map_or((1, 1), |a| (a.1, a.2)));
            }
        }
        Ok(Self {
            systems,
            kickups,
            choices,
            surfaces,
            tuning,
            textures,
            atlases,
        })
    }
    pub fn system_count(&self) -> usize {
        self.systems.len()
    }
    pub fn particle_limit(&self) -> usize {
        self.tuning.particle_limit
    }
    /// Texture file names in sprite `texture` index order.
    pub fn texture_names(&self) -> Vec<&str> {
        self.textures.iter().map(String::as_str).collect()
    }
    fn texture_index(&self, name: &str) -> usize {
        self.textures.iter().position(|t| t == name).unwrap_or(0)
    }
    /// UV rectangle (u0, v0, u1, v1) of an atlas frame: column = frame % W.
    pub fn frame_uv(&self, texture: usize, frame: u8) -> [f32; 4] {
        let (columns, rows) = self.atlases.get(texture).copied().unwrap_or((1, 1));
        let (c, r) = (
            u32::from(frame) % columns,
            u32::from(frame) / columns % rows,
        );
        let (w, h) = (1. / columns as f32, 1. / rows as f32);
        [
            c as f32 * w,
            r as f32 * h,
            (c + 1) as f32 * w,
            (r + 1) as f32 * h,
        ]
    }
    /// Kickup index of a surface token.
    pub fn kickup(&self, token: u32) -> Option<usize> {
        *self.surfaces.get((token as usize).checked_sub(1)?)?
    }
    /// System selection 0x658F45..0x659001 for one wheel.
    pub fn choose(&self, kickup: usize, front: bool, ai: bool, open_wheel: bool) -> Option<usize> {
        let k = &self.kickups[kickup].1;
        let [default, front_system, open, ai_system] = self.choices[kickup];
        let ordinary = if open_wheel && k.open.is_some() {
            open
        } else {
            default
        };
        if front {
            if k.front.is_some() {
                return if ai { None } else { front_system };
            }
            if ai && k.ai.is_some() {
                return None;
            }
            ordinary
        } else if ai && k.ai.is_some() {
            if open_wheel && k.open.is_some() {
                open
            } else {
                ai_system
            }
        } else {
            ordinary
        }
    }
}

fn smoke_only(system: &mut System) {
    let keep: Vec<bool> = system
        .emitters
        .iter()
        .map(|e| SMOKE_TEXTURES.contains(&e.texture.as_str()))
        .collect();
    let remap: Vec<Option<usize>> = keep
        .iter()
        .scan(0, |n, &k| {
            let v = k.then_some(*n);
            *n += usize::from(k);
            Some(v)
        })
        .collect();
    system.births.retain_mut(|b| match remap[b.emitter] {
        Some(i) => {
            b.emitter = i;
            true
        }
        None => false,
    });
    let mut i = 0;
    system.emitters.retain(|_| {
        i += 1;
        keep[i - 1]
    });
}

// ---------------------------------------------------------------------------
// Particle runtime (0x898DC0 system update, 0x898AD0 births, 0x89F1B0
// emitter update, 0x890A10 spawn, 0x8A9CC0 particle update)

/// One drawable particle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sprite {
    pub position: [f32; 3],
    pub size: [f32; 2],
    /// Radians about the view axis (or world up for horizon-aligned).
    pub rotation: f32,
    /// Colour as stored in the data (display-referred) and alpha.
    pub colour: [f32; 4],
    pub texture: usize,
    pub frame: u8,
    pub horizon: bool,
}

/// System placement: a frame in the parent (car) space plus the parent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    pub rows: Rows,
    pub origin: [f32; 3],
    pub parent_rows: Rows,
    pub parent_origin: [f32; 3],
}
impl Frame {
    pub const WORLD: Frame = Frame {
        rows: IDENTITY,
        origin: [0.; 3],
        parent_rows: IDENTITY,
        parent_origin: [0.; 3],
    };
}

/// Live emitter control written by the kickup driver every frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Drive {
    /// Multiplier of each emitter's base rate (capped at 5x base).
    pub rate: f32,
    /// Multiplier of each emitter's base speed (clamped to 0..2x base).
    pub speed: f32,
    /// Copied into new particles (emitter+224); used as an alpha scale [I].
    pub fraction: f32,
    /// Inherited emitter velocity (body linear velocity).
    pub velocity: [f32; 3],
    /// Drift mode active for this car (rear Drift_effect runs only then).
    pub drift: bool,
}
impl Drive {
    pub const IDLE: Drive = Drive {
        rate: 0.,
        speed: 0.,
        fraction: 0.,
        velocity: [0.; 3],
        drift: false,
    };
}

#[derive(Debug, Clone)]
struct EmitterInstance {
    birth: usize,
    age: f32,
    /// None = runs until the system is stopped.
    duration: Option<f32>,
    carry: f32,
    previous: Option<[f32; 3]>,
    burst_done: bool,
}

#[derive(Debug, Clone)]
struct SystemInstance {
    serial: u64,
    key: Option<u64>,
    system: usize,
    age: f32,
    hold: bool,
    created: Vec<bool>,
    emitters: Vec<EmitterInstance>,
    frame: Frame,
    drive: Drive,
    /// World emitter frames of the last update, per birth.
    emitter_frames: Vec<(Rows, [f32; 3])>,
}

#[derive(Debug, Clone, Copy)]
struct Particle {
    /// World position, or emitter-local for emitter-relative particles.
    position: [f32; 3],
    velocity: [f32; 3],
    /// (instance serial, birth) for emitter-relative particles.
    relative: Option<(u64, usize)>,
    age: f32,
    life: f32,
    angle: f32,
    spin: f32,
    colour: [f32; 3],
    colour_rate: [f32; 3],
    fraction: f32,
    system: usize,
    emitter: usize,
    frame: u8,
    size: [f32; 2],
}

/// Original particle LCG (0x869360).
#[derive(Debug, Clone)]
struct Random(u32);
impl Random {
    fn unit(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(0x41c6_4e6d).wrapping_add(0x4058_4c0f);
        (self.0 & 0xffff) as f32 / 65536.
    }
    /// U(-1, 1).
    fn signed(&mut self) -> f32 {
        2. * self.unit() - 1.
    }
}

/// System slots of the particle manager.
pub const SYSTEM_SLOTS: usize = 80;
const GRAVITY: f32 = -9.81;

/// Generic held-system particle runtime.
pub struct Pfx {
    library: std::sync::Arc<Library>,
    instances: Vec<SystemInstance>,
    particles: Vec<Particle>,
    random: Random,
    serial: u64,
    /// Particles spawned by the last update.
    pub spawned: usize,
    /// Spawns refused by the pool cap or full system slots.
    pub dropped: u64,
}

impl Pfx {
    pub fn new(library: std::sync::Arc<Library>) -> Self {
        Self {
            library,
            instances: Vec::new(),
            particles: Vec::new(),
            // AUTHORED seed.
            random: Random(0x1234_5678),
            serial: 0,
            spawned: 0,
            dropped: 0,
        }
    }
    pub fn clear(&mut self) {
        self.instances.clear();
        self.particles.clear();
        self.spawned = 0;
    }
    pub fn particle_count(&self) -> usize {
        self.particles.len()
    }
    /// Held (emitting) systems.
    pub fn held_count(&self) -> usize {
        self.instances.iter().filter(|s| s.hold).count()
    }
    /// Create (0x8A99C0) or re-request (0x8A9AF0) the held system for `key`
    /// and write its frame and live emitter control.
    pub fn hold(&mut self, key: u64, system: usize, frame: Frame, drive: Drive) -> bool {
        if let Some(s) = self.instances.iter_mut().find(|s| s.key == Some(key)) {
            if s.system == system {
                s.frame = frame;
                s.drive = drive;
                return true;
            }
            Self::stop_instance(s);
        }
        if self.held_count() >= SYSTEM_SLOTS {
            self.dropped += 1;
            return false;
        }
        self.serial += 1;
        let births = self.library.systems[system].births.len();
        self.instances.push(SystemInstance {
            serial: self.serial,
            key: Some(key),
            system,
            age: 0.,
            hold: true,
            created: vec![false; births],
            emitters: Vec::new(),
            frame,
            drive,
            emitter_frames: Vec::new(),
        });
        true
    }
    /// Stop (0x882EC0 / release 0x886840): emission ends, particles live on.
    pub fn stop(&mut self, key: u64) {
        if let Some(s) = self.instances.iter_mut().find(|s| s.key == Some(key)) {
            Self::stop_instance(s);
        }
    }
    pub fn stop_where(&mut self, mut predicate: impl FnMut(u64) -> bool) {
        for s in &mut self.instances {
            if s.key.is_some_and(&mut predicate) {
                Self::stop_instance(s);
            }
        }
    }
    fn stop_instance(s: &mut SystemInstance) {
        s.hold = false;
        s.key = None;
        for e in &mut s.emitters {
            e.duration = Some(e.age);
        }
        s.created.iter_mut().for_each(|c| *c = true);
    }
    /// Driver reset of the system time (0x65A3DA) while not emitting.
    pub fn reset_time(&mut self, key: u64) {
        if let Some(s) = self.instances.iter_mut().find(|s| s.key == Some(key)) {
            s.age = 0.;
        }
    }

    pub fn update(&mut self, dt: f32, limit: usize) {
        let library = self.library.clone();
        let mut spawns = Vec::new();
        for s in &mut self.instances {
            let system = &library.systems[s.system];
            s.age += dt;
            for (b, birth) in system.births.iter().enumerate() {
                let start = birth.start.unwrap_or(0.);
                if !s.created[b] && s.age >= start {
                    s.created[b] = true;
                    let end = birth.end.unwrap_or(0.);
                    s.emitters.push(EmitterInstance {
                        birth: b,
                        age: 0.,
                        duration: if end < 0. || s.hold {
                            None
                        } else {
                            Some(end - start)
                        },
                        carry: 0.,
                        previous: None,
                        burst_done: false,
                    });
                }
            }
            let f = s.frame;
            s.emitter_frames = system
                .births
                .iter()
                .map(|birth| {
                    let rows = mul(
                        &mul(&euler_rows(birth.orientation), &f.rows),
                        &f.parent_rows,
                    );
                    // relPosition is added to the system translation unrotated.
                    let local = std::array::from_fn(|i| birth.position[i] + f.origin[i]);
                    let world = apply(local, &f.parent_rows);
                    (rows, std::array::from_fn(|i| world[i] + f.parent_origin[i]))
                })
                .collect();
            for e in &mut s.emitters {
                e.age += dt;
                let birth = &system.births[e.birth];
                let def = &system.emitters[birth.emitter];
                let (rows, origin) = s.emitter_frames[e.birth];
                let previous = e.previous.unwrap_or(origin);
                e.previous = Some(origin);
                let base = def.rate.max(0.);
                let rate = if def.name == "Drift_effect" && !s.drive.drift {
                    0.
                } else {
                    (base * s.drive.rate).clamp(0., 5. * base)
                };
                let mut count = 0usize;
                if !e.burst_done {
                    e.burst_done = true;
                    count += def.number.unwrap_or(0.).max(0.) as usize;
                }
                if e.duration.is_none_or(|d| e.age < d) {
                    let n = rate * dt + e.carry;
                    let whole = n.floor();
                    e.carry = n - whole;
                    count += whole as usize;
                }
                for i in 0..count {
                    let frac = if count == 1 {
                        1.
                    } else {
                        i as f32 / (count - 1) as f32
                    };
                    let at =
                        std::array::from_fn(|a| previous[a] + (origin[a] - previous[a]) * frac);
                    spawns.push((
                        s.serial,
                        s.system,
                        birth.emitter,
                        e.birth,
                        rows,
                        at,
                        s.drive,
                    ));
                }
            }
        }
        self.spawned = 0;
        for (serial, system, emitter, birth, rows, at, drive) in spawns {
            if self.particles.len() >= limit {
                self.dropped += 1;
                continue;
            }
            let p = self.spawn(serial, system, emitter, birth, rows, at, &drive);
            self.particles.push(p);
            self.spawned += 1;
        }
        let systems = &library.systems;
        let instances = &self.instances;
        self.particles.retain_mut(|p| {
            let def = &systems[p.system].emitters[p.emitter];
            p.age += dt;
            if p.life >= 0. && p.age >= p.life {
                return false;
            }
            // Gravity: influence percent of 9.81 m/s^2, positive pulls down.
            let mut g = [0., 0.01 * def.gravity.unwrap_or(0.) * GRAVITY, 0.];
            if let Some((serial, b)) = p.relative {
                let Some(s) = instances.iter().find(|s| s.serial == serial) else {
                    return false;
                };
                let rows = s.emitter_frames[b].0;
                g = std::array::from_fn(|i| dot(rows[i], g));
            }
            for (v, g) in p.velocity.iter_mut().zip(g) {
                *v += g * dt;
            }
            if let Some((influence, range)) = def.drag
                && length(p.velocity) > 0.01
            {
                let k = dt.abs() * 0.003 * influence * range * 0.01;
                p.velocity = p.velocity.map(|v| v - v * k);
            }
            for a in 0..3 {
                p.position[a] += p.velocity[a] * dt;
            }
            if let Some((influence, range)) = def.angular_drag
                && p.spin.abs() > 0.01
            {
                p.spin -= p.spin * (dt.abs() * 0.003 * influence * range * 0.01).clamp(0., 1.);
            }
            let f = if p.life > 0. { p.age / p.life } else { 1. };
            p.angle += (1. - 0.25 * f) * p.spin * dt;
            for a in 0..3 {
                p.colour[a] += p.colour_rate[a] * dt;
            }
            true
        });
        // A stopped instance ends with its emitters; its emitter-relative
        // particles end with it.
        self.instances.retain(|s| {
            s.hold
                || s.emitters
                    .iter()
                    .any(|e| e.duration.is_none_or(|d| e.age < d))
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn spawn(
        &mut self,
        serial: u64,
        system: usize,
        emitter: usize,
        birth: usize,
        rows: Rows,
        at: [f32; 3],
        drive: &Drive,
    ) -> Particle {
        let def = &self.library.systems[system].emitters[emitter];
        let r = &mut self.random;
        let life =
            def.life.unwrap_or(1.) * (1. + r.signed() * def.life_variance.unwrap_or(0.) / 100.);
        let [l, w, h] = def.extent.map(|v| 0.5 * v);
        let local = match (def.location, h > 0.) {
            (ShapeLocation::Pivot, _) => [0.; 3],
            (_, true) => [r.signed() * w, r.signed() * l, r.signed() * h],
            (_, false) => [r.signed() * w, r.signed() * l, 0.],
        };
        let div = def.divergence.unwrap_or(0.).to_radians();
        let (a, b) = (r.signed() * div, r.signed() * div);
        let direction = [a.cos() * b.sin(), -a.sin(), a.cos() * b.cos()];
        let base = def.speed.unwrap_or(0.);
        let live = (base * drive.speed).clamp(0., 2. * base.max(0.));
        let speed = live + def.speed_variation.unwrap_or(0.) * r.signed();
        let mut velocity = apply(direction.map(|d| d * speed), &rows);
        if let Some(amount) = def.inherit {
            for (v, e) in velocity.iter_mut().zip(drive.velocity) {
                *v += amount * e;
            }
        }
        let spin = (def.spin_rate.unwrap_or(0.) + def.spin_variance.unwrap_or(0.) * r.signed())
            .to_radians();
        let angle = (def.initial_rotation_variance.unwrap_or(0.) * r.signed()).to_radians();
        let start = def.colour[0].unwrap_or([1.; 3]);
        let end = def.colour[1].unwrap_or(start);
        let colour_rate = std::array::from_fn(|i| {
            if life > 0. {
                (end[i] - start[i]) / life
            } else {
                0.
            }
        });
        let frame = match (&def.atlas, &def.frequencies) {
            (Some(_), Some(f)) if !f.is_empty() => {
                let total = f[f.len() - 1].max(f32::MIN_POSITIVE);
                let u = r.unit();
                f.iter().position(|c| u < c / total).unwrap_or(f.len() - 1) as u8
            }
            _ => 0,
        };
        let variance = if def.scale_type.is_none() {
            1. + r.signed() * def.scale_variance.unwrap_or(0.) / 100.
        } else {
            1.
        };
        let size = [
            def.initial_size[0].unwrap_or(1.) * variance,
            def.initial_size[1].unwrap_or(1.) * variance,
        ];
        let (position, velocity, relative) = if def.parent_relative {
            // Emitter-relative: local position and local velocity.
            let inverse = |v: [f32; 3]| std::array::from_fn(|i| dot(rows[i], v));
            (local, inverse(velocity), Some((serial, birth)))
        } else {
            let p = apply(local, &rows);
            (std::array::from_fn(|i| p[i] + at[i]), velocity, None)
        };
        Particle {
            position,
            velocity,
            relative,
            age: 0.,
            life,
            angle,
            spin,
            colour: start,
            colour_rate,
            fraction: drive.fraction,
            system,
            emitter,
            frame,
            size,
        }
    }

    /// Drawable sprites (alpha below 0.001 dropped, as the renderer does).
    pub fn sprites(&self, out: &mut Vec<Sprite>) {
        out.clear();
        for p in &self.particles {
            let def = &self.library.systems[p.system].emitters[p.emitter];
            let f = if p.life > 0. {
                (p.age / p.life).clamp(0., 1.)
            } else {
                1.
            };
            let size: [f32; 2] = match def.scale_type {
                Some(kind) => {
                    // The names are swapped against their exponents.
                    let t = match kind {
                        ScaleType::Linear => f,
                        ScaleType::CubicAccelerate => f * f,
                        ScaleType::CubicDecelerate => 1. - (1. - f) * (1. - f),
                        ScaleType::QuadraticAccelerate => f * f * f,
                        ScaleType::QuadraticDecelerate => 1. - (1. - f).powi(3),
                    };
                    std::array::from_fn(|i| {
                        let a = def.initial_size[i].unwrap_or(1.);
                        let b = def.final_size[i].unwrap_or(1.);
                        a + (b - a) * t
                    })
                }
                None => {
                    let g = def.growth.unwrap_or(1.).max(0.);
                    p.size.map(|s| s * g.powi((p.age * 25.).floor() as i32))
                }
            };
            let [start, middle, end] = def.alpha.map(|a| a.unwrap_or(1.));
            let (fade_in, fade_out) = (def.alpha_in.unwrap_or(0.), def.alpha_out.unwrap_or(0.));
            let age_alpha = if p.age < fade_in {
                start + (middle - start) * p.age / fade_in
            } else if p.age <= p.life - fade_out {
                middle
            } else {
                middle + (end - middle) * ((p.age - (p.life - fade_out)) / fade_out).min(1.)
            };
            let alpha = (age_alpha * p.fraction).clamp(0., 1.);
            if alpha < 0.001 {
                continue;
            }
            let position = match p.relative {
                Some((serial, b)) => match self.instances.iter().find(|s| s.serial == serial) {
                    Some(s) => {
                        let (rows, origin) = s.emitter_frames[b];
                        let w = apply(p.position, &rows);
                        std::array::from_fn(|i| w[i] + origin[i])
                    }
                    None => continue,
                },
                None => p.position,
            };
            out.push(Sprite {
                position,
                size,
                rotation: p.angle,
                colour: [p.colour[0], p.colour[1], p.colour[2], alpha],
                texture: self.library.texture_index(&def.texture),
                frame: p.frame,
                horizon: def.horizon,
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Kickup driver (0x6758B0 manager update -> 0x6587A0 per car -> 0x6574F0
// per wheel inputs, 0x6358B0 intensity)

/// Manager constants without XML source (constructor defaults) [V].
pub const REAR_THRESHOLDS: [f32; 2] = [1.7, 9.5];
pub const FRONT_THRESHOLDS: [f32; 2] = [3.5, 9.0];
pub const REAR_SPIN_GAIN: f32 = 2.2;
pub const REAR_SPIN_OFFSET: f32 = 0.17;

/// Intensity outputs of 0x6358B0 for one wheel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Intensity {
    pub rate: f32,
    pub speed: f32,
    pub fraction: f32,
}

/// 0x67C590: band (0 static .. 3 high) and position in the band.
pub fn speed_band(bands: [f32; 3], speed: f32) -> (usize, f32) {
    let [s, l, m] = bands;
    if speed < s {
        (0, speed / s)
    } else if speed < l {
        (1, (speed - s) / (l - s))
    } else if speed < m {
        (2, (speed - l) / (m - l))
    } else {
        (3, 0.)
    }
}

/// 0x6358B0 with per-emitter base values factored out: `rate` and `speed`
/// multiply each emitter's base rate and speed.
#[allow(clippy::too_many_arguments)]
pub fn intensity(
    kickup: &Kickup,
    tuning: &Tuning,
    slip: f32,
    load: f32,
    speed: f32,
    front: bool,
    ai: bool,
    lod: f32,
) -> Intensity {
    let [lo, hi] = if front {
        FRONT_THRESHOLDS
    } else {
        REAR_THRESHOLDS
    };
    let load_factor = if load < 1000. {
        load * 0.001
    } else if load > 2500. {
        1. + ((load - 2500.) * 0.002).clamp(0., 1.)
    } else {
        1.
    };
    let slip_term = kickup.tyreslip * (slip - lo).max(0.);
    let fraction = ((slip - lo) / (hi - lo)).clamp(0., 1.);
    let [high, medium, low, still, rate_k, speed_k] = kickup.multipliers;
    let _ = high; // The top band uses `medium`: its position is always 0.
    let (band, t) = speed_band(tuning.speed_bands, speed);
    let speed_term = match band {
        0 => still * t,
        1 => still + (low - still) * t,
        2 => low + (medium - low) * t,
        _ => medium,
    };
    let front_factor = match (front, ai) {
        (false, _) => 1.,
        (true, false) => tuning.front_wheel,
        (true, true) => tuning.front_wheel_ai,
    };
    Intensity {
        rate: ((speed_term * load_factor + slip_term) * rate_k * front_factor * lod).min(5.),
        speed: ((speed_term + slip_term) * speed_k).clamp(0., 2.),
        fraction: (fraction + speed_term).clamp(0., 1.),
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct WheelState {
    kickup: Option<usize>,
    slip: f32,
    intensity: f32,
}

/// Tyre smoke for every car: the driver plus the particle runtime.
pub struct Smoke {
    pfx: Pfx,
    wheels: Vec<[WheelState; 4]>,
    budget: f32,
    sprites: Vec<Sprite>,
}

impl Smoke {
    pub fn new(library: std::sync::Arc<Library>) -> Self {
        Self {
            pfx: Pfx::new(library),
            wheels: Vec::new(),
            budget: 1.,
            sprites: Vec::new(),
        }
    }
    pub fn library(&self) -> &std::sync::Arc<Library> {
        &self.pfx.library
    }
    pub fn clear(&mut self) {
        self.pfx.clear();
        self.wheels.clear();
        self.budget = 1.;
        self.sprites.clear();
    }
    pub fn sprites(&self) -> &[Sprite] {
        &self.sprites
    }
    pub fn particle_count(&self) -> usize {
        self.pfx.particle_count()
    }
    pub fn dropped(&self) -> u64 {
        self.pfx.dropped
    }
    /// Wheels whose live rate multiplier is positive.
    pub fn emitting_wheels(&self) -> usize {
        self.wheels
            .iter()
            .flatten()
            .filter(|w| w.intensity > 0.)
            .count()
    }
    /// Per car and wheel: (driver slip, live rate multiplier).
    pub fn wheel_report(&self) -> Vec<[(f32, f32); 4]> {
        self.wheels
            .iter()
            .map(|w| std::array::from_fn(|i| (w[i].slip, w[i].intensity)))
            .collect()
    }

    /// One update. `eye` is the camera position for distance level of detail.
    pub fn tick(&mut self, cars: &[CarInput], eye: [f32; 3], dt: f32) {
        let library = self.pfx.library.clone();
        let tuning = &library.tuning;
        // Particle budget from the particles generated by the last update.
        let target = 1. - (self.pfx.spawned as f32 / tuning.max_particle_peak).min(1.);
        self.budget = if target > self.budget {
            self.budget + 0.5 * (target - self.budget)
        } else {
            target
        };
        let ignore = tuning.ignore_distance * self.budget;
        let reduction = tuning.reduction_distance * self.budget;
        self.wheels.resize(cars.len(), Default::default());
        for (c, car) in cars.iter().enumerate() {
            let key =
                |w: usize, reverse: bool| (c as u64) << 8 | (w as u64) << 1 | u64::from(reverse);
            let origin = car.origin();
            let distance = length(std::array::from_fn(|i| origin[i] - eye[i]));
            if distance > ignore {
                self.pfx.stop_where(|k| k >> 8 == c as u64);
                for w in &mut self.wheels[c] {
                    w.intensity = 0.;
                }
                continue;
            }
            let lod = if !car.ai {
                (self.budget + 1.) * 0.5
            } else if reduction > 0. && distance <= reduction {
                tuning.lod_max - (tuning.lod_max - tuning.lod_min) * distance / reduction
            } else {
                tuning.lod_min
            };
            // Driver slip (0x6574F0).
            let mut slip: [f32; 4] = std::array::from_fn(|i| {
                let w = &car.wheels[i];
                let mut s = w.excess
                    * if car.mass > 0. {
                        1. / car.mass
                    } else {
                        1. / 1700.
                    };
                if car.handbrake && w.handbraked {
                    s *= 2.;
                }
                if !w.front {
                    s += w.rear_term.abs()
                        * 0.2
                        * REAR_SPIN_GAIN
                        * ((car.rear_coefficient - 0.12) * 2.0833 - REAR_SPIN_OFFSET).max(0.);
                }
                s
            });
            // 0x6587A0: lift the weaker rear wheel by half the rear spread.
            let rear: Vec<usize> = (0..4).filter(|&i| !car.wheels[i].front).collect();
            if !rear.is_empty() {
                let max = rear.iter().map(|&i| slip[i]).fold(f32::MIN, f32::max);
                let min = rear.iter().map(|&i| slip[i]).fold(f32::MAX, f32::min);
                let half = 0.5 * (max - min);
                for &i in &rear {
                    if slip[i] < max - half {
                        slip[i] += half;
                    }
                }
            }
            let car_rows = car.rows();
            for (i, w) in car.wheels.iter().enumerate() {
                let state = &mut self.wheels[c][i];
                state.slip = slip[i];
                if w.token != 0 {
                    let kickup = library.kickup(w.token);
                    if kickup != state.kickup {
                        self.pfx.stop(key(i, false));
                        self.pfx.stop(key(i, true));
                        state.kickup = kickup;
                    }
                }
                let reverse = w.spin <= -0.02;
                self.pfx.stop(key(i, !reverse));
                let Some(kickup) = state.kickup else {
                    state.intensity = 0.;
                    continue;
                };
                let Some(system) = library.choose(kickup, w.front, car.ai, car.open_wheel) else {
                    self.pfx.stop(key(i, reverse));
                    state.intensity = 0.;
                    continue;
                };
                let drive = if w.token == 0 {
                    Drive::IDLE
                } else {
                    let k = &library.kickups[kickup].1;
                    let n = intensity(k, tuning, slip[i], w.load, car.speed, w.front, car.ai, lod);
                    Drive {
                        rate: n.rate,
                        speed: n.speed,
                        fraction: n.fraction,
                        velocity: car.velocity,
                        drift: false,
                    }
                };
                state.intensity = drive.rate;
                let frame = wheel_frame(w, reverse, car_rows, car.origin());
                if self.pfx.hold(key(i, reverse), system, frame, drive) {
                    // 0x65A3DA: system time restarts while a non-drift
                    // emitter's live rate is below 0.001.
                    let base = library.systems[system]
                        .births
                        .iter()
                        .map(|b| &library.systems[system].emitters[b.emitter])
                        .filter(|e| e.name != "Drift_effect" && e.rate > 0.)
                        .map(|e| e.rate)
                        .fold(f32::MAX, f32::min);
                    if base == f32::MAX || base * drive.rate < 0.001 {
                        self.pfx.reset_time(key(i, reverse));
                    }
                }
            }
        }
        for c in cars.len()..self.wheels.len() {
            self.pfx.stop_where(|k| k >> 8 == c as u64);
        }
        self.wheels.truncate(cars.len());
        self.pfx.update(dt, tuning.particle_limit);
        let mut sprites = std::mem::take(&mut self.sprites);
        self.pfx.sprites(&mut sprites);
        self.sprites = sprites;
    }
}

/// Effect frame of one wheel in the car frame: contact point; yaw by the
/// steering direction; +Z mirrored to point rearward unless the wheel turns
/// backwards; X mirrored on left wheels.
pub fn wheel_frame(w: &WheelInput, reverse: bool, car_rows: Rows, car_origin: [f32; 3]) -> Frame {
    let yaw = if w.steer[0] != 0. || w.steer[1] != 0. {
        -(w.steer[1].atan2(w.steer[0]))
    } else {
        0.
    };
    let (s, c) = yaw.sin_cos();
    let ry = [[c, 0., -s], [0., 1., 0.], [s, 0., c]];
    let mirror = [
        [if w.left { -1. } else { 1. }, 0., 0.],
        [0., 1., 0.],
        [0., 0., if reverse { 1. } else { -1. }],
    ];
    Frame {
        rows: mul(&mirror, &ry),
        origin: w.contact,
        parent_rows: car_rows,
        parent_origin: car_origin,
    }
}

/// Particle textures by name from `effects/textures/pfx_textures.pssg`, as DDS
/// bytes with their validated mip count. Read at runtime, never copied.
pub fn load_textures(
    root: &Path,
    names: &[&str],
) -> Result<HashMap<String, (Vec<u8>, u32)>, Error> {
    use grid_formats::{pssg, texture};
    let bytes = std::fs::read(root.join("effects/textures/pfx_textures.pssg"))?;
    if bytes.len() > 64 << 20 {
        return Err("particle texture package exceeds bound".into());
    }
    let tree = pssg::read_tree(&bytes)?;
    let ids: Vec<&[u8]> = names.iter().map(|n| n.as_bytes()).collect();
    let mut out = HashMap::new();
    for t in texture::read_named(&tree, &ids)? {
        out.insert(String::from_utf8(t.id.to_vec())?, t.validated_dds());
    }
    for name in names {
        if !out.contains_key(*name) {
            return Err(format!("particle texture {name} absent").into());
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Particle count and per-wheel (slip, rate) after one tick.
    type Sample = (usize, [(f32, f32); 4]);

    #[test]
    fn euler_rows_match_the_parser_convention() {
        let (x, y, z) = (0.3f32, -0.7f32, 1.1f32);
        let m = euler_rows([x.to_degrees(), y.to_degrees(), z.to_degrees()]);
        let (sx, cx) = x.sin_cos();
        let (sy, cy) = y.sin_cos();
        let (sz, cz) = z.sin_cos();
        let row2 = [cx * sy * cz + sx * sz, cx * sy * sz - sx * cz, cx * cy];
        for (a, b) in m[2].iter().zip(row2) {
            assert!((a - b).abs() < 1e-6);
        }
        // Divergence direction (a, b) is row 2 of Rx(a)·Ry(b).
        let d = euler_rows([20., 30., 0.])[2];
        let (a, b) = (20f32.to_radians(), 30f32.to_radians());
        let expected = [a.cos() * b.sin(), -a.sin(), a.cos() * b.cos()];
        assert!(d.iter().zip(expected).all(|(u, v)| (u - v).abs() < 1e-6));
    }

    #[test]
    fn tarmac_intensity_comes_from_slip_only() {
        let tuning = Tuning {
            speed_bands: [4., 10., 15.],
            ignore_distance: 400.,
            lod_max: 0.7,
            lod_min: 0.2,
            reduction_distance: 100.,
            front_wheel: 1.,
            front_wheel_ai: 0.75,
            max_particle_peak: 100.,
            particle_limit: 12000,
        };
        let tarmac = Kickup {
            effect: "K_Tarmac".into(),
            front: None,
            open: None,
            ai: None,
            multipliers: [0., 0., 0., 0., 1., 1.],
            tyreslip: 1.,
            downforce: 2.,
        };
        let n = |slip, front| intensity(&tarmac, &tuning, slip, 1500., 20., front, false, 1.);
        assert_eq!(n(1.7, false).rate, 0.);
        assert!((n(3.7, false).rate - 2.).abs() < 1e-6);
        assert!((n(3.7, false).speed - 2.).abs() < 1e-6);
        assert!((n(5.6, false).fraction - 0.5).abs() < 1e-6);
        assert_eq!(n(3.5, true).rate, 0.);
        assert_eq!(n(100., false).rate, 5.);
        // Light load scales only the speed term; rumble uses the medium band.
        let rumble = Kickup {
            multipliers: [1., 0.5, 0., 0., 1., 1.],
            ..tarmac
        };
        let r = intensity(&rumble, &tuning, 0., 500., 30., false, false, 1.);
        assert!((r.rate - 0.25).abs() < 1e-6 && (r.speed - 0.5).abs() < 1e-6);
        assert_eq!(speed_band([4., 10., 15.], 2.), (0, 0.5));
        assert_eq!(speed_band([4., 10., 15.], 12.5), (2, 0.5));
        assert_eq!(speed_band([4., 10., 15.], 15.), (3, 0.));
    }

    #[test]
    fn plain_xml_reader_handles_comments_and_nesting() {
        let root = parse_plain(
            "\u{feff}<?xml version=\"1.0\"?>\n<a x='1'><!-- <b/> --><b y=\"2 3\" /><c><d/></c></a>",
        )
        .unwrap();
        assert_eq!(root.name, "a");
        assert_eq!(root.attr("x"), Some("1"));
        assert_eq!(root.children.len(), 2);
        assert_eq!(root.child("b").unwrap().attr("y"), Some("2 3"));
        assert_eq!(root.child("c").unwrap().children[0].name, "d");
        assert!(parse_plain("<a><b></a>").is_err());
        assert!(parse_plain("<a x=1/>").is_err());
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn shipped_particle_systems_and_kickups_parse() {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let root = Path::new(&root);
        let folder = root.join("effects/particle_systems");
        let mut count = 0;
        for entry in std::fs::read_dir(&folder).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "xml") {
                let system = parse_system(&read_xml(&path).unwrap()).unwrap();
                assert!(!system.births.is_empty(), "{path:?}");
                count += 1;
            }
        }
        assert_eq!(count, 46);
        let tarmac = parse_system(&read_xml(&folder.join("K_Tarmac.xml")).unwrap()).unwrap();
        assert_eq!(tarmac.emitters.len(), 8);
        assert_eq!(tarmac.births[0].start, Some(1.));
        assert_eq!(tarmac.emitters[2].atlas, Some((4, 2, 2)));
        assert!(tarmac.emitters[5].horizon && tarmac.emitters[5].parent_relative);
        let kickups =
            parse_kickups(&read_xml(&root.join("effects/particleMaterials.xml")).unwrap()).unwrap();
        let smooth = &kickups["SMOOTHDRYTAR"];
        assert_eq!(smooth.effect, "K_Tarmac");
        assert_eq!(smooth.ai.as_deref(), Some("K_AI_Tarmac"));
        assert_eq!((smooth.tyreslip, smooth.downforce), (1., 2.));
        assert_eq!(kickups["RUMBLE"].multipliers[..4], [1., 0.5, 0., 0.]);
        let surfaces =
            parse_surface_particles(&read_xml(&root.join("surface_materials.xml")).unwrap());
        assert_eq!(surfaces[76].1.as_deref(), Some("SMOOTHDRYTAR"));
    }

    /// Scripted source-vehicle run feeding the smoke driver; returns per-tick
    /// (particles, per-wheel (slip, rate)) and the final sprites.
    fn scripted(
        root: &Path,
        ticks: u32,
        controls: impl Fn(u32) -> crate::source_vehicle::Controls,
    ) -> (Vec<Sample>, Vec<Sprite>) {
        use crate::source_vehicle::{DELTA, Runtime};
        let library = std::sync::Arc::new(Library::load(root).unwrap());
        let mut runtime = Runtime::lacetti(root, "slot_00", 200679).unwrap();
        let mut smoke = Smoke::new(library);
        let mut trace = Vec::new();
        for tick in 0..ticks {
            runtime.tick(controls(tick)).unwrap();
            let car = CarInput::read(&runtime, false);
            let eye = car.pose[12..15].try_into().unwrap();
            smoke.tick(&[car], eye, DELTA);
            trace.push((smoke.particle_count(), smoke.wheel_report()[0]));
        }
        (trace, smoke.sprites().to_vec())
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn launch_and_lock_emit_smoke_reproducibly() {
        std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024)
            .spawn(|| {
                use crate::source_vehicle::Controls;
                let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
                let root = Path::new(&root);
                let launch = |t: u32| Controls {
                    throttle: 1.,
                    handbrake: t < 100,
                    ..Default::default()
                };
                let (trace, sprites) = scripted(root, 260, launch);
                for (t, (n, w)) in trace.iter().enumerate().step_by(10) {
                    println!("launch tick={t} particles={n} wheels={w:?}");
                }
                assert_eq!(scripted(root, 260, launch).0, trace);
                let lock = |t: u32| Controls {
                    throttle: 1.,
                    steering: if t > 250 { 1. } else { 0. },
                    handbrake: (400..460).contains(&t),
                    ..Default::default()
                };
                let (trace2, sprites2) = scripted(root, 600, lock);
                for (t, (n, w)) in trace2.iter().enumerate().step_by(20) {
                    println!("lock tick={t} particles={n} wheels={w:?}");
                }
                println!("sprites launch={} lock={}", sprites.len(), sprites2.len());
                // Front-drive launch: only the front wheels smoke.
                assert!(trace.iter().map(|t| t.0).max().unwrap() > 50);
                assert!(trace.iter().all(|(_, w)| w[0].1 == 0. && w[1].1 == 0.));
                assert!(trace.iter().any(|(_, w)| w[2].1 > 1. && w[3].1 > 1.));
                // Full lock and handbrake at speed: the rear wheels smoke, capped at 5x.
                assert!(trace2.iter().any(|(_, w)| w[0].1 > 1. || w[1].1 > 1.));
                assert!(trace2.iter().all(|(_, w)| w.iter().all(|(_, r)| *r <= 5.)));
                assert!(
                    trace
                        .iter()
                        .all(|(_, w)| w.iter().all(|(s, r)| s.is_finite() && r.is_finite()))
                );
                assert!(sprites.iter().chain(&sprites2).all(|s| {
                    s.position
                        .iter()
                        .chain(&s.size)
                        .chain(&s.colour)
                        .all(|v| v.is_finite())
                }));
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn library_selects_tarmac_systems_and_loads_textures() {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let root = Path::new(&root);
        let library = Library::load(root).unwrap();
        let name = |s: Option<usize>| s.map(|i| library.systems[i].name.as_str());
        // STDR (token 77) -> SMOOTHDRYTAR -> tarmac systems.
        let k = library.kickup(77).unwrap();
        assert_eq!(library.kickups[k].0, "SMOOTHDRYTAR");
        assert_eq!(
            name(library.choose(k, false, false, false)),
            Some("K_Tarmac")
        );
        assert_eq!(
            name(library.choose(k, true, false, false)),
            Some("K_Tarmac_Front")
        );
        assert_eq!(
            name(library.choose(k, false, false, true)),
            Some("K_Tarmac_open")
        );
        assert_eq!(
            name(library.choose(k, false, true, false)),
            Some("K_AI_Tarmac")
        );
        assert_eq!(library.choose(k, true, true, false), None);
        // Debris emitters are left out; K_Tarmac keeps all eight.
        let tarmac = library
            .systems
            .iter()
            .find(|s| s.name == "K_Tarmac")
            .unwrap();
        assert_eq!(tarmac.emitters.len(), 8);
        let grass = library
            .systems
            .iter()
            .find(|s| s.name == "K_Grass")
            .unwrap();
        assert!(
            grass
                .emitters
                .iter()
                .all(|e| SMOKE_TEXTURES.contains(&e.texture.as_str()))
        );
        assert_eq!(library.tuning.speed_bands, [4., 10., 15.]);
        let textures = load_textures(root, &library.texture_names()).unwrap();
        assert_eq!(textures.len(), library.texture_names().len());
        assert!(
            textures
                .values()
                .all(|(dds, levels)| dds.len() > 128 && *levels == 9)
        );
    }
}
