//! The original's front-end 3D materials `fe3d_generic`, `fe3d_genericGD`,
//! `fe3d_genericGPD`, `fe3d_genericGMPD` and `fe3d_genericI`
//! (docs/RACE_HUD.md §6.7), shared by every screen
//! that draws front-end package nodes: the post-race and pause screens, the
//! loading world.
//!
//! [D] Lighting parameters: `frontend/fe_paramcontrol.xml` groups and zones,
//! read as `FE::MaterialLibrary` does (0x4D04B0 zones, 0x4D6EE0 values,
//! defaults of 0x4CFA70) and turned into the shader's global constants as
//! 0x4DFC00 does (light directions 0x4CFE40, the glow pulse). Shader
//! arithmetic: the `feig3d_*` (in-game) programs of `shaderpack.pssg`, the
//! instance streams of `frontend/fe_shadersetup.xml` (`DiffuseColour` and
//! `Alpha`, `GlowAmount`).
//! Textures hold raw (gamma-space) values (`Rgba8Unorm`): the original's
//! arithmetic runs on them and only the result is converted for the target.
//! [I] A zero-extent bounds axis adds nothing to the glow falloff (the
//! original divides by zero there); no screen-space shadow mask (the
//! zones the port draws have a black third light).
use bevy::{
    asset::uuid_handle,
    mesh::{MeshVertexAttribute, MeshVertexBufferLayoutRef},
    pbr::{MaterialPipeline, MaterialPipelineKey},
    prelude::*,
    render::render_resource::{
        AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, CompareFunction,
        RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError, VertexFormat,
    },
    shader::ShaderRef,
};
use grid_formats::osd;
use std::collections::HashMap;

const SHADER: Handle<Shader> = uuid_handle!("3f1c9a7e-52b4-4d0e-9a61-8e2f4b7c1d95");

/// Per vertex: the position relative to its node's saved bounds,
/// `2 (p − min) / (max − min) − 1` (0 on an axis without extent), for the
/// GD/GPD glow falloff; for GMPD the mesh's `TEXCOORD1` vector instead.
pub const ATTRIBUTE_BOUNDS: MeshVertexAttribute =
    MeshVertexAttribute::new("Fe3dBounds", 0x6fe3_d001, VertexFormat::Float32x3);

// ---------------------------------------------------------------- parameters

/// One parameter group (optionally one of its zones) of
/// `fe_paramcontrol.xml`: each `Param` as its floats (`value`, or
/// `valueR/G/B`, or `valueX/Y/Z`; `true`/`false` as 1/0).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ParamSet(pub HashMap<String, Vec<f32>>);

fn attr<'a>(element: &'a str, key: &str) -> Option<&'a str> {
    let pat = format!(" {key}=\"");
    let at = element.find(&pat)? + pat.len();
    let rest = &element[at..];
    Some(&rest[..rest.find('"')?])
}

fn number(v: &str) -> Option<f32> {
    match v.trim() {
        "true" => Some(1.),
        "false" => Some(0.),
        v => v.parse().ok(),
    }
}

fn strip_comments(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len());
    let mut rest = xml;
    while let Some(at) = rest.find("<!--") {
        out.push_str(&rest[..at]);
        rest = rest[at..].find("-->").map_or("", |e| &rest[at + e + 3..]);
    }
    out.push_str(rest);
    out
}

fn read_params(body: &str, into: &mut ParamSet) {
    let mut rest = body;
    while let Some(at) = rest.find("<Param ") {
        let end = rest[at..].find("/>").map_or(rest.len(), |e| at + e);
        let element = &rest[at..end];
        rest = &rest[end..];
        let Some(name) = attr(element, "name") else { continue };
        let values: Option<Vec<f32>> = match attr(element, "value") {
            Some(v) => number(v).map(|v| vec![v]),
            None => ["R", "G", "B"]
                .iter()
                .map(|c| attr(element, &format!("value{c}")).and_then(number))
                .collect::<Option<Vec<f32>>>()
                .or_else(|| {
                    ["X", "Y", "Z"].iter().map(|c| attr(element, &format!("value{c}")).and_then(number)).collect()
                }),
        };
        if let Some(v) = values {
            into.0.insert(name.to_owned(), v);
        }
    }
}

impl ParamSet {
    /// A group's own values, then the zone's over them (0x4D04B0 reads the
    /// group's values into each zone before the zone's own).
    pub fn read(xml: &str, group: &str, zone: Option<&str>) -> Option<ParamSet> {
        let xml = strip_comments(xml);
        let start = xml.find(&format!("<ParameterGroup name=\"{group}\""))?;
        let body = &xml[start..];
        let body = &body[..body.find("</ParameterGroup>")?];
        let mut set = ParamSet::default();
        read_params(&body[..body.find("<Zone").unwrap_or(body.len())], &mut set);
        if let Some(zone) = zone {
            let at = body.find(&format!("<Zone id=\"{zone}\""))?;
            let z = &body[at..];
            read_params(&z[..z.find("</Zone>")?], &mut set);
        }
        Some(set)
    }
    fn get(&self, name: &str) -> Option<&[f32]> {
        self.0.get(name).map(Vec::as_slice)
    }
    fn scalar(&self, name: &str, default: f32) -> f32 {
        self.get(name).and_then(|v| v.first().copied()).unwrap_or(default)
    }
    fn vec3(&self, name: &str) -> Vec3 {
        self.get(name).filter(|v| v.len() == 3).map_or(Vec3::ZERO, |v| Vec3::new(v[0], v[1], v[2]))
    }
}

/// One directional light of a zone.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Directional {
    /// 0–255 per channel.
    pub colour: Vec3,
    pub elevation: f32,
    pub bearing: f32,
    pub diff_scale: f32,
    pub spec_scale: f32,
    /// Camera-relative direction (0x4CFE40 with the camera node).
    pub locked: bool,
}

/// The lighting of one zone (FE::MaterialLibrary parameter block, +0x154 …
/// +0x238).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lights {
    pub ambient: Vec3,
    pub d1: Directional,
    pub d2: Directional,
    pub d3_colour: Vec3,
    pub d3_locked: bool,
    pub r_min: Vec3,
    pub r_max: Vec3,
    pub r_gamma: f32,
    pub r_scale: f32,
    pub dirt: f32,
    pub pattern: f32,
    pub g_swing: f32,
    pub g_rate: f32,
    pub g_level: f32,
    pub g_variance: f32,
}

impl Default for Lights {
    /// The constructor's values (0x4CFA70); colours and directions zero.
    fn default() -> Self {
        Lights {
            ambient: Vec3::ZERO,
            d1: Directional::default(),
            d2: Directional::default(),
            d3_colour: Vec3::ZERO,
            d3_locked: false,
            r_min: Vec3::ZERO,
            r_max: Vec3::ZERO,
            r_gamma: 0.,
            r_scale: 0.,
            dirt: 1.,
            pattern: 1.,
            g_swing: 0.25,
            g_rate: 1.,
            g_level: 1.,
            g_variance: 0.,
        }
    }
}

impl Lights {
    pub fn from_params(p: &ParamSet) -> Lights {
        let d = Lights::default();
        let dir = |n: &str| Directional {
            colour: p.vec3(&format!("{n}_colour")),
            elevation: p.scalar(&format!("{n}_elevation"), 0.),
            bearing: p.scalar(&format!("{n}_bearing"), 0.),
            diff_scale: p.scalar(&format!("{n}_diff_scale"), 0.),
            spec_scale: p.scalar(&format!("{n}_spec_scale"), 0.),
            locked: p.scalar(&format!("{n}_locked"), 0.) != 0.,
        };
        Lights {
            ambient: p.vec3("ambient"),
            d1: dir("d1"),
            d2: dir("d2"),
            d3_colour: p.vec3("d3_colour"),
            d3_locked: p.scalar("d3_locked", 0.) != 0.,
            r_min: p.vec3("r_min"),
            r_max: p.vec3("r_max"),
            r_gamma: p.scalar("r_gamma", d.r_gamma),
            r_scale: p.scalar("r_scale", d.r_scale),
            dirt: p.scalar("dirt_Level", d.dirt),
            pattern: p.scalar("pattern_Level", d.pattern),
            g_swing: p.scalar("g_swing", d.g_swing),
            g_rate: p.scalar("g_rate", d.g_rate),
            g_level: p.scalar("g_level", d.g_level),
            g_variance: p.scalar("g_variance", d.g_variance),
        }
    }

    /// Reads a group and zone of `fe_paramcontrol.xml`.
    pub fn read(xml: &str, group: &str, zone: Option<&str>) -> Option<Lights> {
        ParamSet::read(xml, group, zone).map(|p| Lights::from_params(&p))
    }

    /// The shader's global constants (0x4DFC00). `camera` is the viewing
    /// camera's world matrix (for locked lights), `sun` the race's sun
    /// (bearing, elevation) in turns, for an unlocked third light, `pulse`
    /// the glow phase's triangle value ([`Pulse::value`]).
    pub fn constants(&self, camera: Option<&Mat4>, sun: Option<[f32; 2]>, pulse: f32) -> Constants {
        let rgb = |c: Vec3| c / 255.;
        let light = |d: &Directional| {
            (
                direction(d.bearing, d.elevation, d.locked.then_some(camera).flatten()).extend(d.diff_scale),
                rgb(d.colour).extend(d.spec_scale),
            )
        };
        let (d1dirds, d1colss) = light(&self.d1);
        let (d2dirds, d2colss) = light(&self.d2);
        let d3dir = if self.d3_locked {
            direction(0.25, 0.422_222_23, None)
        } else {
            match sun {
                Some([bearing, elevation]) => direction(bearing, elevation, camera),
                None => direction(0.25, 0.422_222_23, None),
            }
        };
        Constants {
            ref_min_gamma: self.r_min.extend(self.r_gamma),
            ref_max_scale: self.r_max.extend(self.r_scale),
            ambient: rgb(self.ambient).extend(0.),
            d1dirds,
            d1colss,
            d2dirds,
            d2colss,
            d3dir: d3dir.extend(0.),
            d3col: rgb(self.d3_colour).extend(0.),
            glowdirtpattern: Vec4::new(self.g_variance * pulse + self.g_level, self.dirt, self.pattern, 0.),
        }
    }
}

/// A light direction from its bearing and elevation in turns (0x4CFE40):
/// `(sin 2πa cos πb, sin πb, cos 2πa cos πb)`, through the camera's world
/// matrix when the light is camera-locked.
pub fn direction(bearing: f32, elevation: f32, camera: Option<&Mat4>) -> Vec3 {
    let (a, b) = (std::f32::consts::TAU * bearing, std::f32::consts::PI * elevation);
    let d = Vec3::new(a.sin() * b.cos(), b.sin(), a.cos() * b.cos());
    match camera {
        Some(m) => m.transform_vector3(d).normalize_or_zero(),
        None => d,
    }
}

/// The glow pulse of 0x4DFC00: a phase advancing at `g_rate` per second,
/// wrapping past 1, read as a triangle rising until `g_swing`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pulse {
    pub phase: f32,
}

impl Pulse {
    pub fn advance(&mut self, dt: f32, rate: f32) {
        self.phase += rate * dt;
        if self.phase > 1. {
            self.phase -= self.phase.floor();
        }
    }
    pub fn value(&self, swing: f32) -> f32 {
        if self.phase < swing {
            self.phase / swing
        } else {
            1. - (self.phase - swing) / (1. - swing)
        }
    }
}

/// A zone change's blend weight at `t` (0–1 of its duration), as 0x4D0100
/// eases it. For the front-end world's zone blends; the results screen
/// holds one zone.
#[cfg_attr(not(test), expect(dead_code, reason = "used by the front-end world next"))]
pub fn zone_weight(t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    (2. * t * t - t * t * t).clamp(0., 1.)
}

/// The global float4 constants the front-end shaders read (registered by
/// FE::MaterialLibrary, 0x4D0960).
#[derive(Clone, Copy, Debug, Default, PartialEq, ShaderType)]
pub struct Constants {
    pub ref_min_gamma: Vec4,
    pub ref_max_scale: Vec4,
    pub ambient: Vec4,
    pub d1dirds: Vec4,
    pub d1colss: Vec4,
    pub d2dirds: Vec4,
    pub d2colss: Vec4,
    pub d3dir: Vec4,
    pub d3col: Vec4,
    pub glowdirtpattern: Vec4,
}

// ------------------------------------------------------------------ material

/// Family numbers of [`family`].
pub const GENERIC: u32 = 0;
pub const GD: u32 = 1;
pub const GPD: u32 = 2;
pub const I: u32 = 3;
pub const GMPD: u32 = 4;

/// The shader family of a front-end effect, `None` for effects this module
/// does not draw.
pub fn family(effect: &str) -> Option<u32> {
    match effect.trim_end_matches(".fx") {
        "fe3d_generic" => Some(GENERIC),
        "fe3d_genericGD" => Some(GD),
        "fe3d_genericGPD" => Some(GPD),
        "fe3d_genericI" => Some(I),
        "fe3d_genericGMPD" => Some(GMPD),
        _ => None,
    }
}

/// A material's own values (`fe3d.wgsl` `Material`).
#[derive(Clone, Copy, Debug, Default, PartialEq, ShaderType)]
pub struct Values {
    /// DiffuseColour, Alpha (the instance colour stream).
    pub colour: Vec4,
    /// GlowAmount, family, Emmisive, SpecularLevel.
    pub a: Vec4,
    /// FresnelMin, FresnelMax, FresnelPower, SpecularPow.
    pub b: Vec4,
    /// DirtScale, DirtBlend, DirtGlowBlend, PatternScale.
    pub c: Vec4,
    /// PatternBlend.
    pub d: Vec4,
}

/// The values of a scene material of a known family.
pub fn values(m: &osd::Material) -> Option<Values> {
    let family = family(&m.effect)? as f32;
    let f = |n: &str, d: f32| m.floats(n).and_then(|v| v.first().copied()).unwrap_or(d);
    let colour = m.floats("DiffuseColour").filter(|v| v.len() >= 3).map_or(Vec3::ONE, |v| Vec3::new(v[0], v[1], v[2]));
    Some(Values {
        colour: colour.extend(f("Alpha", 1.)),
        a: Vec4::new(f("GlowAmount", 0.), family, f("Emmisive", 0.), f("SpecularLevel", 0.)),
        b: Vec4::new(f("FresnelMin", 0.), f("FresnelMax", 0.), f("FresnelPower", 1.), f("SpecularPow", 1.)),
        c: Vec4::new(f("DirtScale", 1.), f("DirtBlend", 0.), f("DirtGlowBlend", 0.), f("PatternScale", 1.)),
        d: Vec4::new(f("PatternBlend", 0.), 0., 0., 0.),
    })
}

/// One front-end material instance.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct Fe3dMaterial {
    /// TR1, the reflection cube.
    #[texture(0, dimension = "cube")]
    #[sampler(1)]
    pub reflection: Handle<Image>,
    /// TDirt1.
    #[texture(2)]
    #[sampler(3)]
    pub dirt: Handle<Image>,
    /// TPattern.
    #[texture(4)]
    #[sampler(5)]
    pub pattern: Handle<Image>,
    #[uniform(6)]
    pub values: Values,
    #[uniform(7)]
    pub constants: Constants,
    /// TDiffuseMap (family I), sampled at the mesh's first ST stream.
    #[texture(8)]
    #[sampler(9)]
    pub diffuse: Handle<Image>,
}

impl Material for Fe3dMaterial {
    fn vertex_shader() -> ShaderRef {
        SHADER.into()
    }
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn specialize(
        _: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let vertex = layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_NORMAL.at_shader_location(1),
            ATTRIBUTE_BOUNDS.at_shader_location(2),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(3),
        ])?;
        descriptor.vertex.buffers = vec![vertex];
        descriptor.primitive.cull_mode = None;
        if let Some(depth) = descriptor.depth_stencil.as_mut() {
            depth.depth_write_enabled = Some(false);
            depth.depth_compare = Some(CompareFunction::Always);
        }
        if let Some(target) = descriptor.fragment.as_mut().and_then(|f| f.targets.first_mut()).and_then(|t| t.as_mut()) {
            target.blend = Some(BlendState {
                color: BlendComponent {
                    src_factor: BlendFactor::SrcAlpha,
                    dst_factor: BlendFactor::OneMinusSrcAlpha,
                    operation: BlendOperation::Add,
                },
                alpha: BlendComponent {
                    src_factor: BlendFactor::One,
                    dst_factor: BlendFactor::OneMinusSrcAlpha,
                    operation: BlendOperation::Add,
                },
            });
        }
        Ok(())
    }
}

/// The mesh of a front-end node of `family` for [`Fe3dMaterial`]:
/// positions, normals, UVs and the [`ATTRIBUTE_BOUNDS`] glow vector.
pub fn mesh(m: &osd::Mesh, bounds: &[f32; 6], family: u32) -> Mesh {
    let mut mesh = crate::osd_scene::mesh(m);
    let q: Vec<[f32; 3]> = if family == GMPD {
        // [D] o8 = TEXCOORD1.xyz; [I] zero without the stream.
        if m.texcoord1.len() == m.positions.len() { m.texcoord1.clone() } else { vec![[0.; 3]; m.positions.len()] }
    } else {
        glow_vector(m, bounds)
    };
    mesh.insert_attribute(ATTRIBUTE_BOUNDS, q);
    mesh
}

fn glow_vector(m: &osd::Mesh, bounds: &[f32; 6]) -> Vec<[f32; 3]> {
    m
        .positions
        .iter()
        .map(|p| {
            std::array::from_fn(|k| {
                let extent = bounds[3 + k] - bounds[k];
                if extent.abs() > 1e-6 { 2. * (p[k] - bounds[k]) / extent - 1. } else { 0. }
            })
        })
        .collect()
}

/// Registers the material and its shader (once; every screen that draws
/// front-end nodes calls it).
pub fn plugin(app: &mut App) {
    if app.is_plugin_added::<MaterialPlugin<Fe3dMaterial>>() {
        return;
    }
    app.add_plugins(MaterialPlugin::<Fe3dMaterial>::default());
    let shader = Shader::from_wgsl(include_str!("fe3d.wgsl"), "grid_fe3d.wgsl");
    let _ = app.world_mut().resource_mut::<Assets<Shader>>().insert(SHADER.id(), shader);
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<xml>
  <ParameterGroup name="ingame" >
    <Param name="ambient" valueR="10" valueG="10" valueB="10" />
    <Param name="d1_colour" valueR="212" valueG="225" valueB="225 " />
    <Param name="d1_locked" value="true" />
    <!-- <Param name="dirt_Level" value="9" /> -->
    <Param name="dirt_Level" value="0.7" />
    <Param name="r_min" valueX="-5.1" valueY="0.1" valueZ="-7.8" />
    <Zone id="default">
      <Param name="dirt_Level" value="0.2" />
    </Zone>
    <Zone id="leaderboards">
      <Param name="d1_colour" valueR="255" valueG="252" valueB="234" />
      <Param name="g_level" value="1.8" />
    </Zone>
  </ParameterGroup>
  <ParameterGroup name="osd" >
    <Param name="dirt_Level" value="0.1" />
  </ParameterGroup>
</xml>"#;

    #[test]
    fn zone_over_group() {
        let l = Lights::read(XML, "ingame", Some("leaderboards")).unwrap();
        assert_eq!(l.ambient, Vec3::splat(10.));
        assert_eq!(l.d1.colour, Vec3::new(255., 252., 234.));
        assert!(l.d1.locked);
        assert_eq!(l.dirt, 0.7, "the zone keeps the group's value, the comment is ignored");
        assert_eq!(l.g_level, 1.8);
        assert_eq!(l.r_min, Vec3::new(-5.1, 0.1, -7.8));
        // Unset values keep the constructor's.
        assert_eq!((l.g_swing, l.pattern, l.r_gamma), (0.25, 1., 0.));
        let g = Lights::read(XML, "ingame", None).unwrap();
        assert_eq!(g.d1.colour, Vec3::new(212., 225., 225.), "a trailing space in a value parses");
        assert_eq!(Lights::read(XML, "ingame", Some("default")).unwrap().dirt, 0.2);
        assert_eq!(Lights::read(XML, "osd", None).unwrap().dirt, 0.1);
        assert!(Lights::read(XML, "ingame", Some("missing")).is_none());
    }

    #[test]
    fn direction_formula() {
        let d = direction(0., 0., None);
        assert!((d - Vec3::Z).length() < 1e-6);
        let d = direction(0.25, 0., None);
        assert!((d - Vec3::X).length() < 1e-6);
        let d = direction(0., 0.5, None);
        assert!((d - Vec3::Y).length() < 1e-6);
        // Locked: through the camera's rotation.
        let cam = Mat4::from_rotation_y(std::f32::consts::FRAC_PI_2) * Mat4::from_translation(Vec3::new(5., 0., 0.));
        let d = direction(0., 0., Some(&cam));
        assert!((d - Vec3::X).length() < 1e-5, "{d}");
    }

    #[test]
    fn constants_and_pulse() {
        let l = Lights::read(XML, "ingame", Some("leaderboards")).unwrap();
        let c = l.constants(None, None, 0.5);
        assert!((c.d1colss.truncate() - Vec3::new(1., 252. / 255., 234. / 255.)).length() < 1e-6);
        assert_eq!(c.ambient.w, 0.);
        assert_eq!(c.glowdirtpattern, Vec4::new(1.8, 0.7, 1., 0.));
        let mut p = Pulse::default();
        p.advance(0.1, 1.);
        assert!((p.value(0.25) - 0.4).abs() < 1e-6);
        p.advance(0.4, 1.);
        assert!((p.value(0.25) - (1. - 0.25 / 0.75)).abs() < 1e-5);
        p.advance(0.6, 1.);
        assert!((p.phase - 0.1).abs() < 1e-5, "wraps past 1");
        assert_eq!(zone_weight(0.), 0.);
        assert_eq!(zone_weight(1.), 1.);
        assert_eq!(zone_weight(2.), 1.);
    }

    #[test]
    fn bounds_attribute() {
        let m = osd::Mesh {
            positions: vec![[0., 0., 0.], [2., 4., 0.]],
            normals: vec![],
            uvs: vec![],
            colors_argb: vec![],
            texcoord1: vec![],
            indices: vec![0, 1, 0],
        };
        let glow = |m: &osd::Mesh, family| {
            let mesh = mesh(m, &[0., 0., 0., 2., 4., 0.], family);
            let Some(bevy::mesh::VertexAttributeValues::Float32x3(q)) = mesh.attribute(ATTRIBUTE_BOUNDS) else {
                panic!()
            };
            q.clone()
        };
        assert_eq!(glow(&m, GD), vec![[-1., -1., 0.], [1., 1., 0.]]);
        // GMPD reads its TEXCOORD1 vector, zero without one.
        assert_eq!(glow(&m, GMPD), vec![[0.; 3]; 2]);
        let m = osd::Mesh { texcoord1: vec![[0.5, 0., 0.], [0., -1., 2.]], ..m };
        assert_eq!(glow(&m, GMPD), vec![[0.5, 0., 0.], [0., -1., 2.]]);
    }

    #[test]
    fn families() {
        let names = ["fe3d_generic.fx", "fe3d_genericGD.fx", "fe3d_genericGPD.fx", "fe3d_genericI.fx", "fe3d_genericGMPD.fx"];
        let got: Vec<_> = names.iter().map(|n| family(n)).collect();
        assert_eq!(got, [Some(GENERIC), Some(GD), Some(GPD), Some(I), Some(GMPD)]);
        assert_eq!(family("feov_colour.fx"), None);
    }
}
