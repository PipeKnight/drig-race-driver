//! The original's OSD scene (`Osd/dial2.pssg`) and its glyph fonts
//! (`frontend/fonts/*_osd*.pssg`, `micro_lap`, `swiss_time`, `micro_drift`):
//! nodes with their stored transforms, render quads with their shader
//! instances, the camera, and the animations with their named time ranges
//! (docs/RACE_HUD.md "Original look"). Values are read as saved; what each
//! shader does with its parameters is not stored here.
use crate::{Result, fail, mesh::half_to_f32, pssg::Tree, texture};
use std::collections::BTreeMap;

/// One shader-instance parameter: constant floats or a texture id.
#[derive(Clone, Debug, PartialEq)]
pub enum Param {
    Floats(Vec<f32>),
    Texture(String),
}

/// A `SHADERINSTANCE`: its shader group (effect file) and saved parameters.
#[derive(Clone, Debug)]
pub struct Material {
    pub id: String,
    pub effect: String,
    pub params: Vec<(String, Param)>,
}

impl Material {
    pub fn floats(&self, name: &str) -> Option<&[f32]> {
        self.params.iter().find_map(|(n, p)| match p {
            Param::Floats(v) if n == name => Some(v.as_slice()),
            _ => None,
        })
    }
    pub fn texture(&self, name: &str) -> Option<&str> {
        self.params.iter().find_map(|(n, p)| match p {
            Param::Texture(t) if n == name => Some(t.as_str()),
            _ => None,
        })
    }
}

/// Triangles of one render node in its local space.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u16>,
}

#[derive(Clone, Debug)]
pub struct Render {
    pub mesh: Mesh,
    /// Index into [`Scene::materials`].
    pub material: usize,
}

/// A scene node; `local` is the saved row-vector matrix (translation in
/// elements 12..15).
#[derive(Clone, Debug)]
pub struct Node {
    pub id: String,
    pub parent: Option<usize>,
    pub local: [f32; 16],
    /// Local bounds (min xyz, max xyz).
    pub bounds: [f32; 6],
    pub render: Option<Render>,
}

impl Node {
    /// The id without the `!n` suffix the exporter adds to repeated names.
    pub fn name(&self) -> &str {
        self.id.split('!').next().unwrap_or(&self.id)
    }
}

/// A Bezier key: value and the in/out control points (value, time).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Key {
    pub time: f32,
    pub value: f32,
    pub in_value: f32,
    pub in_time: f32,
    pub out_value: f32,
    pub out_time: f32,
}

/// One animated component.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Curve(pub Vec<Key>);

impl Curve {
    /// The value at `t`: held before the first and after the last key, a
    /// cubic Bezier through the control points in between (`interpMode` 3).
    pub fn sample(&self, t: f32) -> f32 {
        let keys = &self.0;
        let Some(first) = keys.first() else {
            return 0.;
        };
        if t <= first.time {
            return first.value;
        }
        let last = keys.last().unwrap();
        if t >= last.time {
            return last.value;
        }
        let i = keys.partition_point(|k| k.time <= t) - 1;
        let (a, b) = (keys[i], keys[i + 1]);
        let x = [a.time, a.out_time, b.in_time, b.time];
        let y = [a.value, a.out_value, b.in_value, b.value];
        let bez = |p: [f32; 4], u: f32| {
            let v = 1. - u;
            v * v * v * p[0] + 3. * v * v * u * p[1] + 3. * v * u * u * p[2] + u * u * u * p[3]
        };
        // x(u) rises monotonically for the exporter's control times; bisect.
        let (mut lo, mut hi) = (0f32, 1f32);
        for _ in 0..32 {
            let mid = (lo + hi) / 2.;
            if bez(x, mid) < t {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        bez(y, (lo + hi) / 2.)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Semantic {
    Translation,
    Rotation,
    Scale,
    Parameter,
}

/// A channel: its target (node id, or `material#parameter`) and components
/// (1, 3 for XYZ groups, 4 for colours).
#[derive(Clone, Debug)]
pub struct Channel {
    pub target: String,
    pub semantic: Semantic,
    pub components: Vec<Curve>,
}

/// A named time range of an animation (`NETIMERANGE`); `end` 0 means the
/// range runs to the end of the animation.
#[derive(Clone, Debug, PartialEq)]
pub struct Range {
    pub name: String,
    pub start: f32,
    pub end: f32,
}

#[derive(Clone, Debug)]
pub struct Animation {
    pub id: String,
    pub ranges: Vec<Range>,
    pub channels: Vec<Channel>,
}

impl Animation {
    /// The node or object the animation belongs to (`Anim_<name>`).
    pub fn owner(&self) -> &str {
        self.id.strip_prefix("Anim_").unwrap_or(&self.id)
    }
    pub fn range(&self, name: &str) -> Option<&Range> {
        self.ranges.iter().find(|r| r.name == name)
    }
    /// The last key time of any channel.
    pub fn duration(&self) -> f32 {
        self.channels
            .iter()
            .flat_map(|c| &c.components)
            .filter_map(|c| c.0.last().map(|k| k.time))
            .fold(0., f32::max)
    }
}

/// `CAMERANODE` values.
#[derive(Clone, Debug, PartialEq)]
pub struct Camera {
    pub fov_degrees: f32,
    pub near: f32,
    pub far: f32,
    /// World matrix (row-vector convention).
    pub world: [f32; 16],
}

/// A `CUBEMAPTEXTURE` (DXT1 faces, no mips) in the order +X, -X, +Y, -Y,
/// +Z, -Z (`Raw` is the +X face).
pub struct Cubemap<'a> {
    pub id: String,
    pub size: u32,
    pub faces: [&'a [u8]; 6],
}

pub struct Scene<'a> {
    pub textures: Vec<texture::Texture<'a>>,
    pub cubemaps: Vec<Cubemap<'a>>,
    pub materials: Vec<Material>,
    pub nodes: Vec<Node>,
    pub camera: Option<Camera>,
    pub animations: Vec<Animation>,
}

impl Scene<'_> {
    pub fn node(&self, id: &str) -> Option<usize> {
        self.nodes.iter().position(|n| n.id == id)
    }
    pub fn material(&self, id: &str) -> Option<usize> {
        self.materials.iter().position(|m| m.id == id)
    }
    /// The world matrix of a node from the saved transforms.
    pub fn world(&self, node: usize) -> [f32; 16] {
        let n = &self.nodes[node];
        match n.parent {
            Some(p) => crate::scene::compose(&n.local, &self.world(p)),
            None => n.local,
        }
    }
}

fn text(tree: &Tree<'_>, node: usize, name: &[u8]) -> Result<String> {
    Ok(String::from_utf8_lossy(tree.string(node, name)?).into_owned())
}

fn reference(tree: &Tree<'_>, node: usize, name: &[u8]) -> Result<String> {
    let s = text(tree, node, name)?;
    match s.strip_prefix('#') {
        Some(r) => Ok(r.to_owned()),
        None => fail(tree.nodes[node].offset, "expected local reference"),
    }
}

fn float(tree: &Tree<'_>, node: usize, name: &[u8]) -> Result<f32> {
    Ok(f32::from_bits(tree.integer(node, name)?))
}

/// A four-byte metric saved either as a signed integer or as a float: small
/// integers are read as integers, anything else as a float bit pattern.
fn number(tree: &Tree<'_>, node: usize, name: &[u8]) -> Result<f32> {
    let bits = tree.integer(node, name)?;
    let int = bits as i32;
    Ok(if (-1_000_000..1_000_000).contains(&int) { int as f32 } else { f32::from_bits(bits) })
}

fn children<'t>(tree: &'t Tree<'_>, node: usize, name: &'t [u8]) -> impl Iterator<Item = usize> + 't {
    tree.nodes[node]
        .children
        .iter()
        .copied()
        .filter(move |&c| tree.name(c) == name)
}

fn floats_be(data: &[u8]) -> Vec<f32> {
    data.chunks_exact(4)
        .map(|c| f32::from_be_bytes(c.try_into().unwrap()))
        .collect()
}

/// Shader instances with their parameters named from the shader group.
fn materials(tree: &Tree<'_>) -> Result<Vec<Material>> {
    let mut groups = BTreeMap::new();
    for i in 0..tree.nodes.len() {
        if tree.name(i) == b"SHADERGROUP" {
            let names = children(tree, i, b"SHADERINPUTDEFINITION")
                .map(|d| text(tree, d, b"name"))
                .collect::<Result<Vec<_>>>()?;
            groups.insert(text(tree, i, b"id")?, names);
        }
    }
    let mut out = Vec::new();
    for i in 0..tree.nodes.len() {
        if tree.name(i) != b"SHADERINSTANCE" {
            continue;
        }
        let effect = reference(tree, i, b"shaderGroup")?;
        let names = groups.get(&effect);
        let mut params = Vec::new();
        for input in children(tree, i, b"SHADERINPUT") {
            let id = tree.integer(input, b"parameterID")? as usize;
            let name = names
                .and_then(|n| n.get(id))
                .cloned()
                .unwrap_or_else(|| format!("#{id}"));
            let param = match tree.string(input, b"type")? {
                b"texture" => Param::Texture(reference(tree, input, b"texture")?),
                _ => Param::Floats(floats_be(tree.nodes[input].data.unwrap_or_default())),
            };
            params.push((name, param));
        }
        out.push(Material {
            id: text(tree, i, b"id")?,
            effect,
            params,
        });
    }
    Ok(out)
}

/// Vertex, normal and ST streams of every `RENDERDATASOURCE`, by id.
fn sources(tree: &Tree<'_>) -> Result<BTreeMap<String, Mesh>> {
    let mut blocks = BTreeMap::new();
    for i in 0..tree.nodes.len() {
        if tree.name(i) == b"DATABLOCK" {
            blocks.insert(text(tree, i, b"id")?, i);
        }
    }
    let mut out = BTreeMap::new();
    for source in 0..tree.nodes.len() {
        if tree.name(source) != b"RENDERDATASOURCE" {
            continue;
        }
        let mut mesh = Mesh::default();
        for stream in children(tree, source, b"RENDERSTREAM") {
            let Some(&block) = blocks.get(&reference(tree, stream, b"dataBlock")?) else {
                return fail(tree.nodes[stream].offset, "unresolved data block");
            };
            let sub = tree.integer(stream, b"subStream")? as usize;
            let Some(desc) = children(tree, block, b"DATABLOCKSTREAM").nth(sub) else {
                return fail(tree.nodes[stream].offset, "substream out of range");
            };
            let Some(data) = children(tree, block, b"DATABLOCKDATA")
                .next()
                .and_then(|d| tree.nodes[d].data)
            else {
                return fail(tree.nodes[block].offset, "data block without data");
            };
            let count = tree.integer(block, b"elementCount")? as usize;
            let offset = tree.integer(desc, b"offset")? as usize;
            let stride = tree.integer(desc, b"stride")? as usize;
            let format = tree.string(desc, b"dataType")?;
            let width = match format {
                b"float2" => 8,
                b"float3" => 12,
                b"float4" => 16,
                b"half2" => 4,
                b"half4" => 8,
                _ => return fail(tree.nodes[desc].offset, "unsupported OSD stream format"),
            };
            if count == 0 || stride < offset + width || (count - 1) * stride + offset + width > data.len() {
                return fail(tree.nodes[desc].offset, "OSD stream exceeds its data block");
            }
            let element = |i: usize| -> Vec<f32> {
                let at = i * stride + offset;
                let raw = &data[at..at + width];
                match format {
                    b"half2" | b"half4" => raw
                        .chunks_exact(2)
                        .map(|c| half_to_f32(u16::from_be_bytes([c[0], c[1]])))
                        .collect(),
                    _ => floats_be(raw),
                }
            };
            match tree.string(desc, b"renderType")? {
                b"Vertex" => mesh.positions = (0..count).map(|i| { let v = element(i); [v[0], v[1], v[2]] }).collect(),
                b"Normal" => mesh.normals = (0..count).map(|i| { let v = element(i); [v[0], v[1], v[2]] }).collect(),
                b"ST" if mesh.uvs.is_empty() => mesh.uvs = (0..count).map(|i| { let v = element(i); [v[0], v[1]] }).collect(),
                _ => {}
            }
        }
        let Some(index) = children(tree, source, b"RENDERINDEXSOURCE").next() else {
            return fail(tree.nodes[source].offset, "render source without indices");
        };
        if tree.string(index, b"primitive")? != b"triangles" || tree.string(index, b"format")? != b"ushort" {
            return fail(tree.nodes[index].offset, "unsupported OSD index source");
        }
        let data = children(tree, index, b"INDEXSOURCEDATA")
            .next()
            .and_then(|d| tree.nodes[d].data)
            .unwrap_or_default();
        mesh.indices = data
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        let n = mesh.positions.len();
        if mesh.indices.iter().any(|&i| usize::from(i) >= n)
            || (!mesh.uvs.is_empty() && mesh.uvs.len() != n)
            || (!mesh.normals.is_empty() && mesh.normals.len() != n)
        {
            return fail(tree.nodes[source].offset, "OSD mesh streams disagree");
        }
        out.insert(text(tree, source, b"id")?, mesh);
    }
    Ok(out)
}

fn matrix_and_bounds(tree: &Tree<'_>, node: usize) -> ([f32; 16], [f32; 6]) {
    let mut local = [1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.];
    let mut bounds = [0.; 6];
    if let Some(d) = children(tree, node, b"TRANSFORM").next().and_then(|t| tree.nodes[t].data)
        && d.len() >= 64
    {
        local.copy_from_slice(&floats_be(&d[..64]));
    }
    if let Some(d) = children(tree, node, b"BOUNDINGBOX").next().and_then(|t| tree.nodes[t].data)
        && d.len() >= 24
    {
        bounds.copy_from_slice(&floats_be(&d[..24]));
    }
    (local, bounds)
}

fn keys(tree: &Tree<'_>, blocks: &BTreeMap<String, usize>, times: &str, values: &str, width: usize) -> Result<Vec<Curve>> {
    let data = |id: &str| -> Result<(Vec<f32>, usize)> {
        let Some(&b) = blocks.get(id) else {
            return fail(0, "unresolved animation key block");
        };
        let count = tree.integer(b, b"keyCount")? as usize;
        let d = children(tree, b, b"NEANIMKEYS")
            .next()
            .and_then(|k| tree.nodes[k].data)
            .unwrap_or_default();
        Ok((floats_be(d), count))
    };
    let (t, n) = data(times)?;
    let (v, m) = data(values)?;
    if t.len() != n || m != n || v.len() != n * 5 * width {
        return fail(0, "animation key block sizes disagree");
    }
    Ok((0..width)
        .map(|c| {
            Curve(
                (0..n)
                    .map(|k| {
                        let e = &v[(k * width + c) * 5..][..5];
                        Key {
                            time: t[k],
                            value: e[0],
                            in_value: e[1],
                            in_time: e[2],
                            out_value: e[3],
                            out_time: e[4],
                        }
                    })
                    .collect(),
            )
        })
        .collect())
}

fn animations(tree: &Tree<'_>) -> Result<Vec<Animation>> {
    let mut blocks = BTreeMap::new();
    let mut channels = BTreeMap::new();
    let mut vcrs = BTreeMap::new();
    for i in 0..tree.nodes.len() {
        match tree.name(i) {
            b"NEANIMKEYDATABLOCK" => {
                blocks.insert(text(tree, i, b"id")?, i);
            }
            b"NEANIMCHANNEL_BEZIERFLOAT" | b"NEANIMCHANNEL_BEZIERFLOAT4" | b"NEANIMCHANNEL_GROUPXYZ" => {
                channels.insert(text(tree, i, b"id")?, i);
            }
            b"NETIMERANGELIST" => {
                let ranges = children(tree, i, b"NETIMERANGE")
                    .map(|r| {
                        Ok(Range {
                            name: text(tree, r, b"name")?,
                            start: float(tree, r, b"start")?,
                            end: float(tree, r, b"end")?,
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                vcrs.insert(text(tree, i, b"id")?, ranges);
            }
            _ => {}
        }
    }
    let curves = |id: &str| -> Result<Vec<Curve>> {
        let Some(&c) = channels.get(id) else {
            return fail(0, "unresolved animation channel");
        };
        match tree.name(c) {
            b"NEANIMCHANNEL_GROUPXYZ" => {
                let mut out = Vec::new();
                for child in children(tree, c, b"NEANIMCHILDCHANNELREF") {
                    let sub = reference(tree, child, b"channel")?;
                    let Some(&s) = channels.get(&sub) else {
                        return fail(tree.nodes[child].offset, "unresolved child channel");
                    };
                    out.extend(keys(tree, &blocks, &reference(tree, s, b"timeBlock")?, &reference(tree, s, b"valueBlock")?, 1)?);
                }
                Ok(out)
            }
            name => {
                let width = if name == b"NEANIMCHANNEL_BEZIERFLOAT4" { 4 } else { 1 };
                keys(tree, &blocks, &reference(tree, c, b"timeBlock")?, &reference(tree, c, b"valueBlock")?, width)
            }
        }
    };
    let mut out = Vec::new();
    for i in 0..tree.nodes.len() {
        if tree.name(i) != b"NEANIMATION" {
            continue;
        }
        let ranges = match children(tree, i, b"USERDATA").next() {
            Some(u) => vcrs.get(&reference(tree, u, b"object")?).cloned().unwrap_or_default(),
            None => Vec::new(),
        };
        let mut list = Vec::new();
        for r in children(tree, i, b"NECHANNELREF") {
            let semantic = match tree.string(r, b"semantic")? {
                b"translation" => Semantic::Translation,
                b"rotation_euler" => Semantic::Rotation,
                b"scale" => Semantic::Scale,
                b"parameter" => Semantic::Parameter,
                _ => return fail(tree.nodes[r].offset, "unknown animation semantic"),
            };
            list.push(Channel {
                target: text(tree, r, b"targetName")?,
                semantic,
                components: curves(&reference(tree, r, b"channel")?)?,
            });
        }
        out.push(Animation {
            id: text(tree, i, b"id")?,
            ranges,
            channels: list,
        });
    }
    Ok(out)
}

/// The whole OSD scene package.
pub fn read_scene<'a>(tree: &Tree<'a>) -> Result<Scene<'a>> {
    let materials = materials(tree)?;
    let sources = sources(tree)?;
    let material_index: BTreeMap<&str, usize> = materials
        .iter()
        .enumerate()
        .map(|(i, m)| (m.id.as_str(), i))
        .collect();
    let mut nodes: Vec<Node> = Vec::new();
    let mut camera = None;
    let mut index_of = BTreeMap::new();
    for i in 0..tree.nodes.len() {
        let kind = tree.name(i);
        if !matches!(kind, b"ROOTNODE" | b"NODE" | b"VISIBLERENDERNODE" | b"RENDERNODE" | b"CAMERANODE") {
            continue;
        }
        let parent = tree.nodes[i].parent.and_then(|p| index_of.get(&p).copied());
        let (local, bounds) = matrix_and_bounds(tree, i);
        let mut render = None;
        if let Some(instance) = children(tree, i, b"RENDERSTREAMINSTANCE").next() {
            let source = reference(tree, instance, b"indices")?;
            let shader = reference(tree, instance, b"shader")?;
            if let (Some(mesh), Some(&material)) = (sources.get(&source), material_index.get(shader.as_str())) {
                render = Some(Render {
                    mesh: mesh.clone(),
                    material,
                });
            }
        }
        index_of.insert(i, nodes.len());
        nodes.push(Node {
            id: text(tree, i, b"id")?,
            parent,
            local,
            bounds,
            render,
        });
        if kind == b"CAMERANODE" {
            camera = Some((
                nodes.len() - 1,
                float(tree, i, b"FOV")?,
                float(tree, i, b"nearPlane")?,
                float(tree, i, b"farPlane")?,
            ));
        }
    }
    let ids: Vec<&[u8]> = (0..tree.nodes.len())
        .filter(|&i| tree.name(i) == b"TEXTURE")
        .map(|i| tree.string(i, b"id"))
        .collect::<Result<_>>()?;
    let mut cubemaps = Vec::new();
    for i in 0..tree.nodes.len() {
        if tree.name(i) != b"CUBEMAPTEXTURE" {
            continue;
        }
        let size = tree.integer(i, b"width")?;
        if tree.string(i, b"texelFormat")? != b"dxt1" || tree.integer(i, b"height")? != size || size % 4 != 0 {
            return fail(tree.nodes[i].offset, "unsupported OSD cubemap");
        }
        let mut faces: [&[u8]; 6] = [&[]; 6];
        for block in children(tree, i, b"TEXTUREIMAGEBLOCK") {
            let face = match tree.string(block, b"typename")? {
                b"Raw" => 0,
                b"RawNegativeX" => 1,
                b"RawPositiveY" => 2,
                b"RawNegativeY" => 3,
                b"RawPositiveZ" => 4,
                b"RawNegativeZ" => 5,
                _ => return fail(tree.nodes[block].offset, "unknown cubemap face"),
            };
            let data = children(tree, block, b"TEXTUREIMAGEBLOCKDATA")
                .next()
                .and_then(|d| tree.nodes[d].data)
                .unwrap_or_default();
            if data.len() != (size as usize / 4).pow(2) * 8 {
                return fail(tree.nodes[block].offset, "cubemap face size mismatch");
            }
            faces[face] = data;
        }
        if faces.iter().any(|f| f.is_empty()) {
            return fail(tree.nodes[i].offset, "cubemap face missing");
        }
        cubemaps.push(Cubemap {
            id: text(tree, i, b"id")?,
            size,
            faces,
        });
    }
    let mut scene = Scene {
        textures: texture::read_named(tree, &ids)?,
        cubemaps,
        materials,
        nodes,
        camera: None,
        animations: animations(tree)?,
    };
    scene.camera = camera.map(|(node, fov_degrees, near, far)| Camera {
        fov_degrees,
        near,
        far,
        world: scene.world(node),
    });
    Ok(scene)
}

/// One glyph of an OSD font. The mesh's origin is the top-left corner of the
/// glyph's physical box (+y up); metrics are in em (`NEGLYPHMETRICS` divided
/// by the font's `scale`).
#[derive(Clone, Debug, PartialEq)]
pub struct Glyph {
    pub code: u32,
    pub mesh: Mesh,
    /// Local bounds (min xyz, max xyz).
    pub bounds: [f32; 6],
    pub advance: f32,
    /// Left edge of the physical box right of the pen.
    pub bearing_x: f32,
    /// Top of the physical box above the baseline.
    pub bearing_y: f32,
    /// Advance change before the listed following character.
    pub kerning: Vec<(u32, f32)>,
}

/// A `modelMetrics` font (`NEFONTMETRICS`, em units).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Font {
    pub ascender: f32,
    pub descender: f32,
    pub glyphs: Vec<Glyph>,
}

/// A font package: one render node per character code with its metrics.
pub fn read_font(tree: &Tree<'_>) -> Result<Font> {
    let sources = sources(tree)?;
    let mut metrics = BTreeMap::new();
    let mut kerning = BTreeMap::new();
    let mut font = Font::default();
    let mut scale = 1000f32;
    for i in 0..tree.nodes.len() {
        match tree.name(i) {
            b"NEFONTMETRICS" => {
                scale = number(tree, i, b"scale")?.max(1.);
                font.ascender = number(tree, i, b"ascender")?;
                font.descender = number(tree, i, b"descender")?;
            }
            b"NEGLYPHMETRICS" => {
                let code = number(tree, i, b"codePoint")? as u32;
                let kern = reference(tree, i, b"kerningData").ok();
                metrics.insert(
                    code,
                    (
                        number(tree, i, b"advanceWidth")?,
                        number(tree, i, b"horizontalBearing")?,
                        number(tree, i, b"verticalBearing")?,
                        kern,
                    ),
                );
            }
            b"NEGLYPHKERN" => {
                let pairs = children(tree, i, b"NEKERNINGOFFSET")
                    .map(|k| Ok((number(tree, k, b"codePoint")? as u32, number(tree, k, b"kerningOffset")?)))
                    .collect::<Result<Vec<_>>>()?;
                kerning.insert(text(tree, i, b"id")?, pairs);
            }
            _ => {}
        }
    }
    font.ascender /= scale;
    font.descender /= scale;
    for i in 0..tree.nodes.len() {
        if tree.name(i) != b"RENDERNODE" {
            continue;
        }
        let Ok(code) = text(tree, i, b"id")?.parse::<u32>() else {
            continue;
        };
        let (_, bounds) = matrix_and_bounds(tree, i);
        let Some(instance) = children(tree, i, b"RENDERSTREAMINSTANCE").next() else {
            continue;
        };
        let Some(mesh) = sources.get(&reference(tree, instance, b"indices")?) else {
            continue;
        };
        let (advance, bx, by, kern) = metrics.get(&code).cloned().unwrap_or((bounds[3] * scale, 0., 0., None));
        font.glyphs.push(Glyph {
            code,
            mesh: mesh.clone(),
            bounds,
            advance: advance / scale,
            bearing_x: bx / scale,
            bearing_y: by / scale,
            kerning: kern
                .and_then(|k| kerning.get(&k))
                .map(|p| p.iter().map(|&(c, o)| (c, o / scale)).collect())
                .unwrap_or_default(),
        });
    }
    Ok(font)
}

/// `rd4text` nodes of `Osd/sceneconfig.xml`: node name -> font `parameter`
/// (an index into `Osd/fontDescriptions.xml`). A name used twice keeps its
/// first parameter; the file never gives one name two fonts.
pub fn text_fonts(xml: &[u8]) -> Result<BTreeMap<String, u32>> {
    use quick_xml::{Reader, events::Event};
    let mut reader = Reader::from_reader(xml);
    let mut stack: Vec<String> = Vec::new();
    let mut out = BTreeMap::new();
    let bad = |reader: &Reader<&[u8]>| crate::Error {
        offset: reader.buffer_position() as usize,
        message: "invalid OSD scene configuration",
    };
    loop {
        let event = reader.read_event().map_err(|_| bad(&reader))?;
        let attrs = |e: &quick_xml::events::BytesStart<'_>| -> Vec<(Vec<u8>, String)> {
            e.attributes()
                .flatten()
                .map(|a| (a.key.as_ref().to_vec(), String::from_utf8_lossy(&a.value).into_owned()))
                .collect()
        };
        match event {
            Event::Start(e) if e.name().as_ref() == b"node" => {
                let name = attrs(&e).into_iter().find(|(k, _)| k == b"name").map(|(_, v)| v).unwrap_or_default();
                stack.push(name);
            }
            Event::End(e) if e.name().as_ref() == b"node" => {
                stack.pop();
            }
            Event::Empty(e) if e.name().as_ref() == b"attach" => {
                let a = attrs(&e);
                let get = |key: &[u8]| a.iter().find(|(k, _)| k == key).map(|(_, v)| v.trim().to_owned());
                if get(b"type").as_deref() == Some("rd4text")
                    && let (Some(node), Some(p)) = (stack.last(), get(b"parameter"))
                {
                    let p = p.parse().map_err(|_| bad(&reader))?;
                    out.entry(node.clone()).or_insert(p);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(time: f32, value: f32) -> Key {
        Key {
            time,
            value,
            in_value: value,
            in_time: time,
            out_value: value,
            out_time: time,
        }
    }

    #[test]
    fn curves_hold_outside_and_interpolate_inside() {
        // anim_right X of dial2: four keys of the aspect-ratio layout.
        let c = Curve(vec![
            Key { time: 0.6667, value: 0., in_value: 0., in_time: 0.6667, out_value: -17.61, out_time: 0.7778 },
            Key { time: 1., value: -52.82, in_value: -35.22, in_time: 0.8889, out_value: -80.09, out_time: 1.111 },
        ]);
        assert_eq!(c.sample(0.), 0.);
        assert_eq!(c.sample(2.), -52.82);
        let mid = c.sample(0.8333);
        assert!((mid - -26.4).abs() < 0.5, "{mid}");
        // Flat control points give a step held between keys.
        let s = Curve(vec![key(0., 0.), key(0.5, 0.), key(0.5001, 0.25), key(1., 0.25)]);
        assert_eq!(s.sample(0.25), 0.);
        assert!((s.sample(0.75) - 0.25).abs() < 1e-6);
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT purchased source"]
    fn dial2_scene_and_osd_fonts_read() {
        let root = std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").unwrap());
        let bytes = std::fs::read(root.join("Osd/dial2.pssg")).unwrap();
        let tree = crate::pssg::read_tree(&bytes).unwrap();
        let scene = read_scene(&tree).unwrap();
        assert_eq!(scene.textures.len(), 53);
        assert_eq!(scene.cubemaps.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(), ["env_soft.dds", "env_sharp.dds"]);
        assert_eq!(scene.materials.len(), 206);
        assert_eq!(scene.animations.len(), 101);
        let camera = scene.camera.as_ref().unwrap();
        assert!((camera.fov_degrees - 12.3068).abs() < 1e-3);
        assert!((camera.world[14] - 3320.).abs() < 1.);
        // The layout's aspect ranges move the right group in for narrower screens.
        let layout = scene.animations.iter().find(|a| a.owner() == "implementation_osd/osd_layout=layout").unwrap();
        let right = layout.channels.iter().find(|c| c.target == "anim_right").unwrap();
        let at = |name: &str| right.components[0].sample(layout.range(name).unwrap().start);
        assert_eq!(at("1.7"), 0.);
        assert!((at("1.3") - -134.6).abs() < 0.1);
        let rendered = scene.nodes.iter().filter(|n| n.render.is_some()).count();
        assert_eq!(rendered, 244);
        if std::env::var_os("GRID_OSD_DUMP").is_some() {
            for (i, n) in scene.nodes.iter().enumerate() {
                let w = scene.world(i);
                let r = n.render.as_ref().map(|r| {
                    let m = &scene.materials[r.material];
                    format!("{} v{} i{} uv{:?}", m.id, r.mesh.positions.len(), r.mesh.indices.len(), r.mesh.uvs.first())
                });
                println!("node {} p{:?} world({:.1},{:.1},{:.1}) b{:?} {:?}", n.id, n.parent, w[12], w[13], w[14], n.bounds, r);
            }
            for m in &scene.materials {
                println!("material {} {} {:?}", m.id, m.effect, m.params);
            }
            for a in &scene.animations {
                let ranges: Vec<String> = a.ranges.iter().map(|r| format!("{}:{:.3}-{:.3}", r.name, r.start, r.end)).collect();
                let channels: Vec<String> = a.channels.iter().map(|c| format!("{}[{:?}x{}]", c.target, c.semantic, c.components.len())).collect();
                println!("anim {} dur={:.3} ranges={:?} channels={:?}", a.owner(), a.duration(), ranges, channels);
            }
            for t in &scene.textures {
                println!("texture {} {}x{} {:?}", String::from_utf8_lossy(t.id), t.width, t.height, t.format);
            }
        }
        let fonts = text_fonts(&std::fs::read(root.join("Osd/sceneconfig.xml")).unwrap()).unwrap();
        assert_eq!(fonts.get("big_lap_number"), Some(&3));
        assert_eq!(fonts.get("race_time"), Some(&4));
        assert_eq!(fonts.get("name"), Some(&1));
        assert_eq!(fonts.get("drift_text"), Some(&2));
        for font in ["microgramma_osd.PSSG", "swiss_bold_osd_2d.PSSG", "micro_drift.pssg", "micro_lap.pssg", "swiss_time.pssg"] {
            let bytes = std::fs::read(root.join("frontend/fonts").join(font)).unwrap();
            let tree = crate::pssg::read_tree(&bytes).unwrap();
            let parsed = read_font(&tree).unwrap();
            let glyphs = &parsed.glyphs;
            assert!(glyphs.len() >= 30, "{font}");
            assert!(parsed.ascender > 0.9 && parsed.descender < -0.2, "{font}");
            if font == "swiss_time.pssg" {
                let dot = glyphs.iter().find(|g| g.code == u32::from('.')).unwrap();
                assert_eq!((dot.advance, dot.bearing_x, dot.bearing_y), (0.264, 0.047, 0.172));
                let p = glyphs.iter().find(|g| g.code == u32::from('P')).unwrap();
                assert!(p.kerning.contains(&(u32::from('.'), -0.184)));
            }
            if std::env::var_os("GRID_OSD_DUMP").is_some() {
                for g in glyphs {
                    println!("glyph {font} {} {:?} v{} {:?} {:?}", g.code, char::from_u32(g.code), g.mesh.positions.len(), g.bounds, g.mesh.positions.iter().zip(&g.mesh.uvs).collect::<Vec<_>>());
                }
            }
        }
    }
}
