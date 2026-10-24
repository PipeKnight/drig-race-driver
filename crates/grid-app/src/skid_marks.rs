//! Tyre skid marks for every car, following the original skid-mark plugin
//! (docs/GROUND_EFFECTS_RESEARCH.md, emission 73F860, vertices 739570).
//! The surface profile `skidmarks/road_tm.xml` (Lacetti `vehicle_skid_type`
//! 4) and the atlas `skidmarks/road_tm.pssg` are read from the purchased
//! installation. Presentation only; never feeds physics.
use super::wheel_ground::{Wheel, WheelGround};
use bevy::{
    mesh::MeshVertexBufferLayoutRef,
    pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline},
    prelude::*,
    render::render_resource::{AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError},
};
use grid_formats::skidmarks::{Layer, Profile};
use std::path::Path;


/// Default-off diagnostic; no smoke/skid simulation or atlas changes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum OrderMode {
    #[default]
    Off,
    Control,
    Isolated,
}
impl OrderMode {
    pub(crate) fn parse(value: &str) -> Result<Self, &'static str> {
        match value {
            "off" => Ok(Self::Off),
            "control" => Ok(Self::Control),
            "isolated" => Ok(Self::Isolated),
            _ => Err("--skid-order needs off, control or isolated"),
        }
    }
}

/// Deliberately only two scalar values; absent CLI preserves the stock path.
pub(crate) fn parse_reflectance(value: &str) -> Result<f32, &'static str> {
    match value {
        "0.5" => Ok(0.5),
        "0" => Ok(0.),
        _ => Err("--skid-reflectance needs 0.5 (neutral) or 0 (candidate)"),
    }
}

// StandardMaterial.depth_bias sets BOTH a transparent sort offset and raster
// depth bias. This extension keeps raster constant50 while permitting sort0.
// No vertex/fragment override: use the stock StandardMaterial PBR shaders.
type SkidOrderMaterial = ExtendedMaterial<StandardMaterial, SkidRasterBias>;
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
#[bind_group_data(SkidRasterKey)]
struct SkidRasterBias {
    // Nonempty extension bind group; intentionally unused by stock shaders.
    #[uniform(100)]
    marker: Vec4,
    raster_constant: i32,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
struct SkidRasterKey(i32);
impl From<&SkidRasterBias> for SkidRasterKey {
    fn from(value: &SkidRasterBias) -> Self { Self(value.raster_constant) }
}
impl MaterialExtension for SkidRasterBias {
    fn specialize(
        _: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _: &MeshVertexBufferLayoutRef,
        key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // ExtendedMaterial calls this AFTER StandardMaterial specialization.
        // Preserve slope/clamp, compare, write-enable, blend and every shader.
        restore_raster_constant(descriptor, key.bind_group_data.0);
        Ok(())
    }
}
fn restore_raster_constant(descriptor: &mut RenderPipelineDescriptor, constant: i32) {
    if let Some(depth) = descriptor.depth_stencil.as_mut() {
        depth.bias.constant = constant;
    }
}

fn extended_order_material(mut base: StandardMaterial, mode: OrderMode) -> SkidOrderMaterial {
    debug_assert_ne!(mode, OrderMode::Off);
    base.depth_bias = if mode == OrderMode::Control { 50. } else { 0. };
    SkidOrderMaterial {
        base,
        extension: SkidRasterBias { marker: Vec4::ZERO, raster_constant: 50 },
    }
}

/// Global strip pool and strip length [D 73F860/739570].
const STRIPS: usize = 240;
const QUADS: usize = 32;
/// Retire the farthest finished strip while free + retiring strips < 40 [D].
const SPARE: usize = 40;
/// Alpha lost by the oldest quad of a retiring strip per update (25/255) [D].
const RETIRE_STEP: f32 = 25. / 255.;
/// A point is laid after the wheel moved more than 0.1 m [D].
const STEP: f32 = 0.1;
/// A strip ends when its direction turns by more than 10 degrees [D 73F0A0].
const TURN_COS: f32 = 0.984_807_7;
/// Skid layers need excess wheel force / mass above this [D].
const SKID_EXCESS: f32 = 2.0;
/// Side-skid layer once 1 - |cos(slip angle)| reaches 0.5 [D].
const SIDE_FACTOR: f32 = 0.5;
/// Atlas aspect (1024 x 256) used to keep the texel aspect along the mark [D].
const ATLAS_ASPECT: f32 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Tread,
    Skid,
    SideSkid,
}

/// Layer of the original decision: skidding wheels (excess > 2) use the skid
/// or side-skid layer when the surface has `skid`, rolling wheels the tread
/// layer when it has `tread`; otherwise no mark (the strip ends).
fn choose(wheel: &Wheel, profile: &Profile) -> Option<(Kind, Layer)> {
    if !wheel.contact {
        return None;
    }
    let surface = profile.surfaces.iter().find(|s| s.code == wheel.code)?;
    if wheel.excess > SKID_EXCESS {
        if !surface.skid.enabled {
            return None;
        }
        Some(if wheel.side_factor >= SIDE_FACTOR {
            (Kind::SideSkid, surface.side_skid)
        } else {
            (Kind::Skid, surface.skid)
        })
    } else if surface.tread.enabled {
        Some((Kind::Tread, surface.tread))
    } else {
        None
    }
}

#[derive(Clone, Copy, Debug)]
struct Point {
    center: Vec3,
    side: Vec3,
    normal: Vec3,
    half_width: f32,
    u: [f32; 2],
    v: f32,
    alpha: f32,
}

#[derive(Clone, Debug, Default)]
struct Strip {
    points: Vec<Point>,
    /// (car, wheel) while the strip is being extended.
    owner: Option<(usize, usize)>,
    retiring: bool,
    /// Index of the oldest point still fading during retirement.
    fade: usize,
    used: bool,
}

#[derive(Clone, Copy, Debug)]
struct Head {
    strip: usize,
    kind: Kind,
    direction: Option<Vec3>,
}

#[derive(Resource)]
pub(crate) struct SkidMarks {
    profile: Profile,
    strips: Vec<Strip>,
    heads: Vec<[Option<Head>; 4]>,
    camera: Vec3,
    generation: u64,
    mesh: Handle<Mesh>,
    dirty: bool,
    pub laid: u64,
}

impl SkidMarks {
    fn new(profile: Profile) -> Self {
        Self {
            profile,
            strips: vec![Strip::default(); STRIPS],
            heads: Vec::new(),
            camera: Vec3::ZERO,
            generation: 0,
            mesh: Handle::default(),
            dirty: true,
            laid: 0,
        }
    }

    fn clear(&mut self) {
        self.strips = vec![Strip::default(); STRIPS];
        self.heads.clear();
        self.dirty = true;
    }

    fn distance(&self, strip: &Strip) -> f32 {
        strip
            .points
            .last()
            .map_or(f32::INFINITY, |p| p.center.distance_squared(self.camera))
    }

    /// Free strip, else a retiring one, else steal the active strip farthest
    /// from the camera, even one a wheel is still laying [V 73F330]. Releasing
    /// it detaches that wheel, which starts a fresh strip next sample [V 7289C0].
    fn allocate(&mut self) -> Option<usize> {
        let index = self
            .strips
            .iter()
            .position(|s| !s.used)
            .or_else(|| self.strips.iter().position(|s| s.retiring))
            .or_else(|| {
                (0..STRIPS).max_by(|&a, &b| {
                    self.distance(&self.strips[a])
                        .total_cmp(&self.distance(&self.strips[b]))
                })
            })?;
        if let Some((car, wheel)) = self.strips[index].owner
            && let Some(head) = self.heads.get_mut(car).and_then(|h| h.get_mut(wheel))
        {
            *head = None;
        }
        self.strips[index] = Strip {
            used: true,
            ..Strip::default()
        };
        Some(index)
    }

    /// End a wheel's strip: last point transparent, the one before halved [D].
    fn end(&mut self, car: usize, wheel: usize) {
        if let Some(head) = self.heads[car][wheel].take() {
            let strip = &mut self.strips[head.strip];
            strip.owner = None;
            let n = strip.points.len();
            if n >= 1 {
                strip.points[n - 1].alpha = 0.;
            }
            if n >= 2 {
                strip.points[n - 2].alpha *= 0.5;
            }
            if n < 2 {
                *strip = Strip::default();
            }
            self.dirty = true;
        }
    }

    /// Advance one wheel by one physics sample.
    fn advance(&mut self, car: usize, index: usize, wheel: &Wheel) {
        let Some((kind, layer)) = choose(wheel, &self.profile) else {
            self.end(car, index);
            return;
        };
        let side = wheel.normal.cross(wheel.forward).normalize_or(Vec3::X);
        // Left-side wheels mirror the atlas U range [D].
        let u = if wheel.left {
            [layer.end_u, layer.start_u]
        } else {
            [layer.start_u, layer.end_u]
        };
        let width = wheel.width * layer.width_scale;
        let mut point = Point {
            center: wheel.point + wheel.normal * self.profile.offset,
            side,
            normal: wheel.normal,
            half_width: 0.5 * width,
            u,
            v: 0.,
            alpha: 1.,
        };
        if let Some(head) = self.heads[car][index] {
            let last = *self.strips[head.strip]
                .points
                .last()
                .expect("owned strip point");
            let delta = point.center - last.center;
            let step = delta.length();
            if step <= STEP {
                return;
            }
            let direction = delta / step;
            let turned = head.direction.is_some_and(|d| d.dot(direction) < TURN_COS);
            if step > 4.0 || turned || head.kind != kind {
                self.end(car, index);
            } else {
                point.v = last.v
                    + (layer.end_u - layer.start_u).abs() * ATLAS_ASPECT * step / width.max(1e-3);
                let full = self.strips[head.strip].points.len() > QUADS;
                let strip = if full {
                    // Chain into a new strip, keeping V continuity.
                    self.strips[head.strip].owner = None;
                    let Some(next) = self.allocate() else {
                        self.heads[car][index] = None;
                        return;
                    };
                    self.strips[next].points.push(last);
                    self.strips[next].owner = Some((car, index));
                    next
                } else {
                    head.strip
                };
                self.strips[strip].points.push(point);
                self.heads[car][index] = Some(Head {
                    strip,
                    kind,
                    direction: Some(direction),
                });
                self.laid += 1;
                self.dirty = true;
                return;
            }
        }
        // Fresh strip.
        let Some(strip) = self.allocate() else {
            return;
        };
        self.strips[strip].owner = Some((car, index));
        self.strips[strip].points.push(point);
        self.heads[car][index] = Some(Head {
            strip,
            kind,
            direction: None,
        });
    }

    /// Keep spare strips by retiring the farthest finished ones; fade them
    /// one quad at a time from the oldest end and free them when faded [D].
    fn maintain(&mut self) {
        let spare = self.strips.iter().filter(|s| !s.used || s.retiring).count();
        if spare < SPARE
            && let Some(far) = (0..STRIPS)
                .filter(|&i| {
                    let s = &self.strips[i];
                    s.used && !s.retiring && s.owner.is_none()
                })
                .max_by(|&a, &b| {
                    self.distance(&self.strips[a])
                        .total_cmp(&self.distance(&self.strips[b]))
                })
        {
            self.strips[far].retiring = true;
        }
        for strip in self.strips.iter_mut().filter(|s| s.retiring) {
            while strip.fade < strip.points.len() && strip.points[strip.fade].alpha <= 0. {
                strip.fade += 1;
            }
            if strip.fade + 1 >= strip.points.len() {
                *strip = Strip::default();
            } else {
                let p = &mut strip.points[strip.fade];
                p.alpha = (p.alpha - RETIRE_STEP).max(0.);
            }
            self.dirty = true;
        }
    }

    fn build(&self) -> Mesh {
        let mut positions = Vec::new();
        let mut normals = Vec::new();
        let mut uvs = Vec::new();
        let mut colors = Vec::new();
        let mut indices = Vec::new();
        for strip in self.strips.iter().filter(|s| s.points.len() >= 2) {
            // Fresh strips ramp in over their first two quads [D byte0 ramp].
            let first = positions.len() as u32;
            for (i, p) in strip.points.iter().enumerate() {
                let ramp = match i {
                    0 => 0.,
                    1 => 0.5,
                    _ => 1.,
                };
                let alpha = p.alpha
                    * if strip.owner.is_some() || i < 2 {
                        ramp
                    } else {
                        1.
                    };
                for (sign, u) in [(-1., p.u[0]), (1., p.u[1])] {
                    positions.push((p.center + p.side * p.half_width * sign).to_array());
                    normals.push(p.normal.to_array());
                    uvs.push([u, p.v]);
                    colors.push([1., 1., 1., alpha]);
                }
            }
            for i in 0..strip.points.len() as u32 - 1 {
                let a = first + i * 2;
                indices.extend([a, a + 2, a + 1, a + 1, a + 2, a + 3]);
            }
        }
        super::ground_effect_assets::effect_mesh(positions, normals, uvs, colors, indices)
    }
}

pub(crate) fn install(app: &mut App, root: &Path, mode: OrderMode, reflectance: Option<f32>) -> Result<(), Box<dyn std::error::Error>> {
    let text = std::fs::read_to_string(root.join("skidmarks/road_tm.xml"))?;
    let profile = grid_formats::skidmarks::read(&text)?;
    let textures =
        super::ground_effect_assets::package(root, "skidmarks/road_tm.pssg", &["diffuse.tga"])?;
    let diffuse = textures
        .get("diffuse.tga")
        .ok_or("skidmark diffuse texture absent")?;
    let diffuse = diffuse.clone();
    println!(
        "skid_marks enabled=true profile=road_tm surfaces={} offset={} strips={STRIPS}x{QUADS} cars=all fidelity=experimental",
        profile.surfaces.len(),
        profile.offset
    );
    if mode != OrderMode::Off {
        app.add_plugins(MaterialPlugin::<SkidOrderMaterial>::default());
    }
    println!("skid_order mode={mode:?} raster_constant=50 sort_bias={} source_parity=false",
        if mode == OrderMode::Isolated { 0 } else { 50 });
    app.insert_resource(Pending { profile, diffuse, mode, reflectance })
        .add_systems(Startup, start)
        .add_systems(FixedUpdate, lay.after(super::wheel_ground::update))
        .add_systems(Update, present.after(super::drive_present));
    Ok(())
}

#[derive(Resource)]
struct Pending {
    profile: Profile,
    diffuse: Vec<u8>,
    mode: OrderMode,
    reflectance: Option<f32>,
}

fn start(
    mut commands: Commands,
    pending: Option<Res<Pending>>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    experimental: Option<ResMut<Assets<SkidOrderMaterial>>>,
) {
    let Some(pending) = pending else { return };
    // Device-dependent upload must follow texture_upload::configure (PreStartup).
    // install() runs before the render device has selected BC/ETC2/RGBA support.
    let image = match super::ground_effect_assets::image(&pending.diffuse, true) {
        Ok(image) => image,
        Err(error) => {
            warn!("skid mark texture: {error}");
            return;
        }
    };
    let mut marks = SkidMarks::new(pending.profile.clone());
    marks.mesh = meshes.add(marks.build());
    // Alpha blended, no depth write, lit by the ground normal. The original
    // also blends a normal map by the Fade bound; not reproduced (AUTHORED).
    let mut base = StandardMaterial {
        base_color_texture: Some(images.add(image)),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.9,
        depth_bias: 50.,
        ..default()
    };
    if let Some(value) = pending.reflectance {
        base.reflectance = value;
        println!("skid_reflectance active=true value={value} mode=Isolated shader=stock-lit-PBR source_parity=false");
    }
    if pending.mode == OrderMode::Off {
        let material = materials.add(base);
        commands.spawn((
            Name::new("skid marks"),
            Mesh3d(marks.mesh.clone()),
            MeshMaterial3d(material),
            Transform::IDENTITY,
            bevy::camera::visibility::NoFrustumCulling,
        ));
    } else {
        let mut experimental = experimental.expect("skid order MaterialPlugin installed");
        let material = experimental.add(extended_order_material(base, pending.mode));
        commands.spawn((
            Name::new("skid marks"),
            Mesh3d(marks.mesh.clone()),
            MeshMaterial3d(material),
            Transform::IDENTITY,
            bevy::camera::visibility::NoFrustumCulling,
        ));
    }
    commands.remove_resource::<Pending>();
    commands.insert_resource(marks);
}

fn lay(
    marks: Option<ResMut<SkidMarks>>,
    ground: Res<WheelGround>,
    camera: Query<&GlobalTransform, With<super::ViewerCamera>>,
) {
    let Some(mut marks) = marks else { return };
    if marks.generation == ground.generation {
        return;
    }
    marks.generation = ground.generation;
    let start = std::time::Instant::now();
    if ground.reset {
        marks.clear();
    }
    if let Ok(camera) = camera.single() {
        marks.camera = camera.translation();
    }
    marks.heads.resize(ground.cars.len(), [None; 4]);
    for (car, wheels) in ground.cars.iter().enumerate() {
        for (index, wheel) in wheels.iter().enumerate() {
            marks.advance(car, index, wheel);
        }
    }
    marks.maintain();
    super::ground_effect_assets::charge(super::ground_effect_assets::TICK, start);
}

fn present(marks: Option<ResMut<SkidMarks>>, mut meshes: ResMut<Assets<Mesh>>) {
    let Some(mut marks) = marks else { return };
    if !marks.dirty {
        return;
    }
    let start = std::time::Instant::now();
    marks.dirty = false;
    let mesh = marks.build();
    if let Some(mut target) = meshes.get_mut(&marks.mesh) {
        *target = mesh;
    }
    super::ground_effect_assets::charge(super::ground_effect_assets::SKID, start);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_mode_is_default_off_and_rejects_unknown_values() {
        assert_eq!(OrderMode::default(), OrderMode::Off);
        assert_eq!(OrderMode::parse("control"), Ok(OrderMode::Control));
        assert_eq!(OrderMode::parse("isolated"), Ok(OrderMode::Isolated));
        assert!(OrderMode::parse("zero").is_err());
    }
    #[test]
    fn isolated_sort_preserves_raster_key_atlas_alpha_and_lighting() {
        let original = StandardMaterial {
            base_color_texture: Some(Handle::default()),
            base_color: Color::srgba(0.8, 0.7, 0.6, 0.4),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.9,
            depth_bias: 50.,
            ..default()
        };
        let control = extended_order_material(original.clone(), OrderMode::Control);
        let isolated = extended_order_material(original.clone(), OrderMode::Isolated);
        assert_eq!(control.base.depth_bias, 50.);
        assert_eq!(isolated.base.depth_bias, 0.);
        assert_eq!(<SkidOrderMaterial as bevy::pbr::Material>::depth_bias(&control), 50.);
        assert_eq!(<SkidOrderMaterial as bevy::pbr::Material>::depth_bias(&isolated), 0.);
        assert_eq!(SkidRasterKey::from(&control.extension), SkidRasterKey(50));
        assert_eq!(SkidRasterKey::from(&isolated.extension), SkidRasterKey(50));
        for material in [control, isolated] {
            assert_eq!(material.base.base_color_texture, original.base_color_texture);
            assert_eq!(material.base.base_color, original.base_color);
            assert_eq!(material.base.alpha_mode, original.alpha_mode);
            assert_eq!(material.base.perceptual_roughness, original.perceptual_roughness);
            assert_eq!(material.base.cull_mode, original.cull_mode);
            assert_eq!(material.base.unlit, original.unlit);
            assert_eq!(material.base.fog_enabled, original.fog_enabled);
        }
        let changed = SkidRasterBias { marker: Vec4::ZERO, raster_constant: 51 };
        assert_ne!(SkidRasterKey::from(&changed), SkidRasterKey(50));
    }


    #[test]
    fn raster_restore_changes_only_constant_and_keeps_default_stock_shaders() {
        use bevy::render::render_resource::{BlendState, ColorTargetState, ColorWrites,
            CompareFunction, DepthBiasState, DepthStencilState, FragmentState, TextureFormat};
        let mut descriptor = RenderPipelineDescriptor {
            label: Some("skid-order-fixture".into()),
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth32Float,
                depth_write_enabled: Some(false),
                depth_compare: Some(CompareFunction::GreaterEqual),
                stencil: default(),
                bias: DepthBiasState { constant: 0, slope_scale: 0.125, clamp: 0.25 },
            }),
            fragment: Some(FragmentState {
                targets: vec![Some(ColorTargetState {
                    format: TextureFormat::Rgba16Float,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
                ..default()
            }),
            ..default()
        };
        let mut expected = descriptor.clone();
        expected.depth_stencil.as_mut().unwrap().bias.constant = 50;
        restore_raster_constant(&mut descriptor, 50);
        assert_eq!(descriptor, expected); // Entire descriptor; only constant differs.
        let mut no_depth = RenderPipelineDescriptor::default();
        let untouched = no_depth.clone();
        restore_raster_constant(&mut no_depth, 50);
        assert_eq!(no_depth, untouched);
        assert!(matches!(<SkidRasterBias as MaterialExtension>::vertex_shader(), bevy::shader::ShaderRef::Default));
        assert!(matches!(<SkidRasterBias as MaterialExtension>::fragment_shader(), bevy::shader::ShaderRef::Default));
        assert!(matches!(<SkidRasterBias as MaterialExtension>::prepass_fragment_shader(), bevy::shader::ShaderRef::Default));
        assert!(matches!(<SkidRasterBias as MaterialExtension>::deferred_fragment_shader(), bevy::shader::ShaderRef::Default));
    }

    fn profile() -> Profile {
        grid_formats::skidmarks::read(
            r#"<SKIDMARKS offset="0.04">
<SURFACE name="STDR" tread="0" skid="1" skidStartU="0.02" skidEndU="0.0625"
  sideSkidStartU="0.001" sideSkidEndU="0.02" skidWidthScale="1.1" sideSkidWidthScale="0.6"/>
<SURFACE name="GRAS" tread="1" treadStartU="0.5" treadEndU="0.56" skid="1"
  skidStartU="0.56" skidEndU="0.62" sideSkidStartU="0.56" sideSkidEndU="0.62"/>
</SKIDMARKS>"#,
        )
        .unwrap()
    }

    fn wheel(code: &[u8; 4], excess: f32, side: f32, x: f32) -> Wheel {
        Wheel {
            contact: true,
            code: *code,
            point: Vec3::new(x, 0., 0.),
            normal: Vec3::Y,
            forward: Vec3::X,
            velocity: Vec3::X * 10.,
            width: 0.27,
            excess,
            side_factor: side,
            ..Default::default()
        }
    }

    #[test]
    fn original_layer_choice() {
        let p = profile();
        assert!(choose(&wheel(b"STDR", 1.0, 0., 0.), &p).is_none());
        assert_eq!(
            choose(&wheel(b"STDR", 2.5, 0.2, 0.), &p).unwrap().0,
            Kind::Skid
        );
        assert_eq!(
            choose(&wheel(b"STDR", 2.5, 0.6, 0.), &p).unwrap().0,
            Kind::SideSkid
        );
        assert_eq!(
            choose(&wheel(b"GRAS", 0.0, 0., 0.), &p).unwrap().0,
            Kind::Tread
        );
        assert!(choose(&wheel(b"TARM", 3.0, 0., 0.), &p).is_none());
        let mut airborne = wheel(b"STDR", 3.0, 0., 0.);
        airborne.contact = false;
        assert!(choose(&airborne, &p).is_none());
    }

    #[test]
    fn strips_step_chain_end_and_recycle() {
        let mut marks = SkidMarks::new(profile());
        marks.heads.resize(1, [None; 4]);
        // 0.06 m steps: only every other sample lays a point.
        for i in 0..=8 {
            marks.advance(0, 0, &wheel(b"STDR", 3.0, 0., i as f32 * 0.06));
        }
        let head = marks.heads[0][0].unwrap();
        assert_eq!(marks.strips[head.strip].points.len(), 5);
        // Long straight run chains into further strips.
        for i in 0..200 {
            marks.advance(0, 0, &wheel(b"STDR", 3.0, 0., 0.5 + i as f32 * 0.2));
        }
        let used = marks.strips.iter().filter(|s| s.used).count();
        assert!(used >= 6, "{used}");
        assert!(marks.strips.iter().all(|s| s.points.len() <= QUADS + 1));
        // Grip regained on tarmac: strip ends with a transparent last point.
        let strip = marks.heads[0][0].unwrap().strip;
        marks.advance(0, 0, &wheel(b"STDR", 0.5, 0., 41.));
        assert!(marks.heads[0][0].is_none());
        assert_eq!(marks.strips[strip].points.last().unwrap().alpha, 0.);
        // The pool never exceeds its fixed size and retires far strips.
        for k in 0..2000 {
            let x = 100. + k as f32 * 0.5;
            marks.advance(0, 0, &wheel(b"STDR", 3.0, 0., x));
            if k % 3 == 0 {
                marks.advance(0, 0, &wheel(b"STDR", 0.0, 0., x));
            }
            marks.maintain();
        }
        assert_eq!(marks.strips.len(), STRIPS);
        assert!(marks.strips.iter().any(|s| s.retiring) || marks.strips.iter().any(|s| !s.used));
        assert!(marks.build().count_vertices() > 0);
    }

    #[test]
    fn full_pool_steals_the_farthest_active_strip_like_the_original() {
        let mut marks = SkidMarks::new(profile());
        marks.heads.resize(2, [None; 4]);
        // Car 1 lays a strip far from the camera and keeps it open.
        marks.advance(1, 0, &wheel(b"STDR", 3.0, 0., 500.));
        marks.advance(1, 0, &wheel(b"STDR", 3.0, 0., 500.2));
        let far = marks.heads[1][0].unwrap().strip;
        // Every other strip is a finished one near the camera.
        for (i, strip) in marks.strips.iter_mut().enumerate() {
            if i != far {
                strip.used = true;
                strip.points = vec![
                    Point {
                        center: Vec3::new(i as f32 * 0.01, 0., 0.),
                        side: Vec3::Z,
                        normal: Vec3::Y,
                        half_width: 0.1,
                        u: [0., 1.],
                        v: 0.,
                        alpha: 1.,
                    };
                    2
                ];
            }
        }
        // 73F330: no free or retiring strip, so the open far strip is stolen
        // and its wheel detached (7289C0).
        assert_eq!(marks.allocate(), Some(far));
        assert!(marks.heads[1][0].is_none());
        assert!(marks.strips[far].owner.is_none() && marks.strips[far].points.is_empty());
    }
}
