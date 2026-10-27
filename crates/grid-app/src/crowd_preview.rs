//! Static USA grandstand billboard diagnostic, NOT recovered crowd animation/LOD.
//! Bounded diagnostic. Source positions/sex/group are retained.
//! Source palette variant0/colourSet0 and fixed CRT-draw fixture5000 chosen explicitly.
//! Actual global RNG, timing and pose are not recovered.
use bevy::{
    asset::{RenderAssetUsages, uuid_handle},
    light::NotShadowCaster,
    mesh::{Indices, PrimitiveTopology},
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};
use grid_formats::{binxml, crowd_placements, pssg, texture};
use grid_tools::route::Route;
use std::{collections::BTreeMap, error::Error, path::Path};
type AnyResult<T> = Result<T, Box<dyn Error>>;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Off,
    Control,
    StaticBillboards,
    OneNeutralActor,
    OneArmPose,
    OneKeyHypothesis,
}
impl Mode {
    pub fn parse(s: &str) -> Result<Self, &'static str> {
        match s {
            "off" => Ok(Self::Off),
            "control" => Ok(Self::Control),
            "static-billboards" => Ok(Self::StaticBillboards),
            "one-neutral-actor" => Ok(Self::OneNeutralActor),
            "one-arm-pose" => Ok(Self::OneArmPose),
            "one-key-hypothesis" => Ok(Self::OneKeyHypothesis),
            _ => Err(
                "--crowd-preview needs off, control, static-billboards, one-neutral-actor, one-arm-pose or one-key-hypothesis",
            ),
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn validate(
        self,
        baseline: bool,
        static_reference: bool,
        detroit8: bool,
        lod0: bool,
        render: bool,
        frames: Option<u32>,
        no_perf: bool,
    ) -> Result<(), &'static str> {
        if self != Self::Off
            && !(baseline
                && static_reference
                && detroit8
                && lod0
                && render
                && frames.is_some_and(|n| (1..=600).contains(&n))
                && no_perf)
        {
            return Err(
                "--crowd-preview requires bounded baseline Detroit route8 static LOD0 reference; no drive/perf/headless/collision",
            );
        }
        Ok(())
    }
}
#[derive(Clone, Debug)]
struct Spec {
    uv: [f32; 4],
    dimensions: [f32; 2],
    colours: [Vec4; 3],
    instance_scale: [f32; 2],
}
#[derive(Clone, Debug)]
struct Row {
    position: Vec3,
    orientation_pair: [f32; 2],
    opaque_scalar: f32,
    cell: u32,
    sex: usize,
}
#[derive(Resource)]
pub struct Prepared {
    mode: Mode,
    actor: Option<super::crowd_rig::Actor>,
    rows: Vec<Row>,
    specs: [Spec; 2],
    diffuse: Vec<u8>,
    tint: Vec<u8>,
    total: u32,
}
fn attr<'a>(e: &binxml::Element<'a>, name: &str) -> AnyResult<&'a str> {
    e.attributes
        .iter()
        .find(|v| v.0 == name)
        .map(|v| v.1)
        .ok_or_else(|| format!("crowd missing {name}").into())
}
fn float(e: &binxml::Element<'_>, name: &str) -> AnyResult<f32> {
    let v: f32 = attr(e, name)?.parse()?;
    if !v.is_finite() {
        return Err("crowd nonfinite definition".into());
    }
    Ok(v)
}
fn one<'d, 'a>(
    d: &'d binxml::Document<'a>,
    parent: usize,
    tag: &str,
    name: Option<&str>,
) -> AnyResult<(usize, &'d binxml::Element<'a>)> {
    let mut it = d.elements[parent].children.clone().filter_map(|i| {
        let e = &d.elements[i];
        (e.name == tag && name.is_none_or(|n| attr(e, "name").ok() == Some(n))).then_some((i, e))
    });
    let e = it
        .next()
        .ok_or_else(|| format!("crowd {tag}/{name:?} missing"))?;
    if it.next().is_some() {
        return Err("duplicate crowd definition".into());
    }
    Ok(e)
}
// Explicit source-draw fixture, NOT replay of the original global CRT RNG.
// 0x986990 packs width in low half and height in high half; finite positive
// normal scales only. Its first branch uses x87, its second branch uses SSE.
fn source_scale_fixture(height_range: f32, width_range: f32) -> [f32; 2] {
    const DRAW: f32 = 5000.;
    const RATIO: f32 = f32::from_bits(0x38d1b717);
    fn truncate_half_normal(v: f32) -> f32 {
        // For the validated range (0.5,1.5), source half conversion discards
        // the low 13 mantissa bits. Return the exactly decoded scale as f32.
        f32::from_bits(v.to_bits() & !0x1fff)
    }
    let height =
        (1.0f64 - 2. * f64::from(DRAW) * f64::from(RATIO) * f64::from(height_range)) as f32;
    let r = DRAW * RATIO;
    let width = 1. + ((r * 2.) - 1.) * width_range;
    [truncate_half_normal(width), truncate_half_normal(height)]
}
fn spec(d: &binxml::Document<'_>, definition: &str) -> AnyResult<Spec> {
    let (country, _) = one(d, 0, "crowdDefSet", Some("usa"))?;
    let (def, _) = one(d, country, "crowdDef", Some(definition))?;
    let (_, scale_stance) = one(d, def, "stance", Some("sitting"))?;
    let height_range = float(scale_stance, "randomHeightScale")?;
    let width_range = float(scale_stance, "randomWidthScale")?;
    if !(0. ..0.25).contains(&height_range) || !(0. ..0.25).contains(&width_range) {
        return Err("crowd diagnostic source scale range unsupported".into());
    }
    let instance_scale = source_scale_fixture(height_range, width_range);
    let (variant, _) = one(d, def, "textureVariant", Some("texVariant0"))?;
    let (bd, _) = one(d, variant, "billboardData", None)?;
    let (_, s) = one(d, bd, "stance", Some("sitting"))?;
    let uv = [
        float(s, "u0")?,
        float(s, "v0")?,
        float(s, "u1")?,
        float(s, "v1")?,
    ];
    let dimensions = [float(s, "width")?, float(s, "height")?];
    if !(uv.iter().all(|v| (0. ..=1.).contains(v))
        && uv[0] < uv[2]
        && uv[1] < uv[3]
        && dimensions.iter().all(|v| *v > 0. && *v < 5.))
    {
        return Err("crowd UV/dimensions invalid".into());
    }
    let (set, _) = one(d, variant, "colourSet", Some("colourSet0"))?;
    let cs: Vec<_> = d.elements[set]
        .children
        .clone()
        .map(|i| &d.elements[i])
        .collect();
    if cs.len() != 3 || cs.iter().any(|c| c.name != "colour") {
        return Err("crowd requires three source tint colours".into());
    }
    let mut colours = [Vec4::ZERO; 3];
    for (dst, c) in colours.iter_mut().zip(cs) {
        *dst = Vec4::new(float(c, "red")?, float(c, "green")?, float(c, "blue")?, 1.);
    }
    Ok(Spec {
        uv,
        dimensions,
        colours,
        instance_scale,
    })
}
fn verify_set(d: &binxml::Document<'_>, id: &str, map: &str, def: &str) -> AnyResult<()> {
    // Resolve source instanceSet -> generic PC instanceMap -> USA crowdDef.
    let sets = 0; // instanceSet elements are direct crowdDefs root children.
    let (set, _) = one(d, sets, "instanceSet", Some(id))?;
    let (_, m) = one(d, set, "map", Some(map))?;
    let _ = m;
    let (maps, _) = d
        .elements
        .iter()
        .enumerate()
        .find(|(_, e)| e.name == "instanceMapGroup" && attr(e, "name").ok() == Some("generic"))
        .ok_or("PC crowd maps absent")?;
    let (_, m) = one(d, maps, "instanceMap", Some(map))?;
    if attr(m, "crowdDef")? != def || attr(m, "stance")? != "sitting" {
        return Err("grandstand instance map changed".into());
    }
    Ok(())
}
fn preflight_selected_rows(current: usize, additional: usize) -> Result<(), &'static str> {
    if current.checked_add(additional).is_none_or(|n| n > 100_000) {
        return Err("crowd preview source rows exceed bound");
    }
    Ok(())
}
pub fn prepare(root: &Path, route: &Route, mode: Mode) -> AnyResult<Prepared> {
    if route.path() != "tracks/usa/detroit/route_8" {
        return Err("crowd preview only Detroit route8".into());
    }
    // This narrow adapter requires the pinned text XML dataset references
    // before using the selected USA source resources.
    let data = std::fs::read_to_string(root.join(route.track_file("organism_track_dataset.xml")))?;
    for (file, user) in [
        ("cr_us_billboard.pssg", "billboardusa"),
        ("cr_male_usa.pssg", "maleVariant0"),
        ("cr_female_usa.pssg", "femaleVariant0"),
    ] {
        if !data.lines().any(|l| {
            l.contains(&format!("/data/anims/{file}"))
                && l.contains(&format!("userdata=\"{user}\""))
        }) {
            return Err("Detroit USA dataset changed".into());
        }
    }
    let defs = std::fs::read(root.join("anims/crowdDefs.xml"))?;
    let doc = binxml::read(&defs)?;
    verify_set(&doc, "m_gr", "maleSit", "crowdMale")?;
    verify_set(&doc, "f_gr", "femaleSit", "crowdFemale")?;
    let specs = [spec(&doc, "crowdMale")?, spec(&doc, "crowdFemale")?];
    let placements = std::fs::read(root.join(route.file("crowd_standing.bin")))?;
    let catalogue = crowd_placements::read(&placements)?;
    let mut rows = Vec::new();
    for g in &catalogue.groups {
        let sex = match g.instance_set {
            b"m_gr" => 0,
            b"f_gr" => 1,
            _ => continue,
        };
        let crowd_placements::Placements::Standing(points) = &g.placements else {
            return Err("grandstand moving paths unsupported".into());
        };
        preflight_selected_rows(rows.len(), points.len())?;
        for p in points {
            rows.push(Row {
                position: Vec3::from_array(p.position),
                orientation_pair: p.orientation_pair,
                opaque_scalar: p.opaque_scalar,
                cell: p.cell,
                sex,
            });
        }
    }
    if rows.is_empty() || rows.len() > 100_000 {
        return Err("crowd preview count absent/exceeds bound".into());
    }
    if matches!(
        mode,
        Mode::OneNeutralActor | Mode::OneArmPose | Mode::OneKeyHypothesis
    ) {
        let row = rows.get(4275).ok_or("neutral actor row4275 absent")?;
        if row.sex != 1 || row.cell != 89 || !row.position.is_finite() {
            return Err("neutral female row4275/cell89 differs".into());
        }
        let actor = if mode == Mode::OneKeyHypothesis {
            super::crowd_rig::prepare_key_hypothesis(root)?
        } else if mode == Mode::OneArmPose {
            super::crowd_rig::prepare_arm_pose(root)?
        } else {
            super::crowd_rig::prepare(root)?
        };
        println!(
            "crowd_neutral_selected ordinal=4275 cell=89 sex=female source_position={:?} model_scale=1 yaw=authored-camera-facing-XZ foot_alignment=authored-neutral-bounds clocks=off",
            row.position
        );
        return Ok(Prepared {
            mode,
            actor: Some(actor),
            rows,
            specs,
            diffuse: Vec::new(),
            tint: Vec::new(),
            total: catalogue.total_instances,
        });
    }
    let atlas = std::fs::read(root.join("anims/cr_us_billboard.pssg"))?;
    let tree = pssg::read_tree(&atlas)?;
    let named = texture::read_named(&tree, &[b"cr_US_billboard_d.tga", b"cr_US_billboard_m.tga"])?;
    let d = named
        .iter()
        .find(|t| t.id == b"cr_US_billboard_d.tga")
        .ok_or("crowd diffuse absent")?;
    let m = named
        .iter()
        .find(|t| t.id == b"cr_US_billboard_m.tga")
        .ok_or("crowd tint absent")?;
    if d.width != 512
        || d.height != 256
        || d.format != texture::Format::Bc3
        || m.width != 512
        || m.height != 256
        || m.format != texture::Format::Bc1
    {
        return Err("crowd atlas format changed".into());
    }
    Ok(Prepared {
        mode,
        actor: None,
        rows,
        specs,
        diffuse: d.validated_dds().0,
        tint: m.validated_dds().0,
        total: catalogue.total_instances,
    })
}

// Only the authored frozen-pulse diagnostic uses these selected-grandstand
// ordinals. Coordinates always come from current validated Prepared rows.
#[derive(Clone, Copy)]
struct FlashRequest {
    ordinal: usize,
    sex: usize,
    cell: u32,
}
const FLASH_REQUESTS: [FlashRequest; 3] = [
    FlashRequest {
        ordinal: 1907,
        sex: 0,
        cell: 88,
    },
    FlashRequest {
        ordinal: 1902,
        sex: 0,
        cell: 89,
    },
    FlashRequest {
        ordinal: 4275,
        sex: 1,
        cell: 89,
    },
];
fn select_flash_rows(
    rows: &[Row],
    has_camera: [bool; 2],
    requests: &[FlashRequest],
) -> AnyResult<Vec<super::flash_fixture::Anchor>> {
    if requests.len() != 3 {
        return Err("flash fixture requires exactly three requested source rows".into());
    }
    let mut anchors = Vec::with_capacity(3);
    for (i, request) in requests.iter().enumerate() {
        if requests[..i].iter().any(|r| r.ordinal == request.ordinal) {
            return Err("duplicate flash fixture source ordinal".into());
        }
        let row = rows
            .get(request.ordinal)
            .ok_or("flash fixture source ordinal missing")?;
        if request.sex > 1 || row.sex != request.sex || row.cell != request.cell {
            return Err("flash fixture source group/cell changed".into());
        }
        if !has_camera.get(request.sex).copied().unwrap_or(false) {
            return Err("flash fixture source definition hasCamera is false".into());
        }
        if !row.position.is_finite() {
            return Err("nonfinite flash fixture source origin".into());
        }
        anchors.push(super::flash_fixture::Anchor {
            source_index: u32::try_from(request.ordinal)?,
            source_cell: row.cell,
            position: row.position.to_array(),
        });
    }
    Ok(anchors)
}
pub(crate) fn flash_fixture_anchors(
    root: &Path,
    prepared: &Prepared,
) -> AnyResult<Vec<super::flash_fixture::Anchor>> {
    if prepared.mode != Mode::StaticBillboards
        || prepared.total != 6854
        || prepared.rows.len() != 4667
    {
        return Err("flash fixture needs exact admitted Detroit static crowd population".into());
    }
    let raw = std::fs::read(root.join("anims/crowdDefs.xml"))?;
    let doc = binxml::read(&raw)?;
    let (country, _) = one(&doc, 0, "crowdDefSet", Some("usa"))?;
    let mut has_camera = [false; 2];
    for (slot, name) in ["crowdMale", "crowdFemale"].into_iter().enumerate() {
        let (_, definition) = one(&doc, country, "crowdDef", Some(name))?;
        has_camera[slot] = match attr(definition, "hasCamera")? {
            "0" => false,
            "1" => true,
            _ => return Err("unsupported crowd hasCamera flag".into()),
        };
    }
    select_flash_rows(&prepared.rows, has_camera, &FLASH_REQUESTS)
}

const SHADER: Handle<Shader> = uuid_handle!("3f1983b1-35cd-4ea7-986e-22b81999f3a2");
type CrowdMaterial = ExtendedMaterial<StandardMaterial, Tint>;
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct Tint {
    #[texture(100)]
    #[sampler(101)]
    map: Handle<Image>,
    #[uniform(102)]
    colour0: Vec4,
    #[uniform(103)]
    colour1: Vec4,
    #[uniform(104)]
    colour2: Vec4,
}
impl MaterialExtension for Tint {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }
}
pub fn install(app: &mut App, prepared: Prepared) -> AnyResult<()> {
    if !matches!(
        prepared.mode,
        Mode::OneNeutralActor | Mode::OneArmPose | Mode::OneKeyHypothesis
    ) {
        println!(
            "crowd_preview mode={:?} total_source={} selected_grandstand={} variant=0 colour_set=0 scale_fixture_draws=5000,5000 poses=unswayed-static-billboards alpha=Bevy-A2C lighting=PBR source_parity=false shadows=disabled unknown_scalar=opaque rotation_pair=retained-billboard-source-faces-camera",
            prepared.mode,
            prepared.total,
            prepared.rows.len()
        );
    }
    if !matches!(
        prepared.mode,
        Mode::StaticBillboards | Mode::OneNeutralActor | Mode::OneArmPose | Mode::OneKeyHypothesis
    ) {
        return Ok(());
    }
    app.world_mut().resource_mut::<Assets<Shader>>().insert(
        SHADER.id(),
        Shader::from_wgsl(
            include_str!("crowd_preview.wgsl"),
            "grid_crowd_preview.wgsl",
        ),
    )?;
    app.insert_resource(prepared)
        .add_plugins(MaterialPlugin::<CrowdMaterial>::default())
        .add_systems(PostStartup, spawn);
    Ok(())
}
#[derive(Default)]
struct Batch {
    p: Vec<[f32; 3]>,
    n: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
    indices: Vec<u32>,
}
fn card(batch: &mut Batch, row: &Row, s: &Spec, eye: Vec3) -> Result<(), &'static str> {
    let d = eye - row.position;
    let len = d.x.hypot(d.z);
    if !row.position.is_finite() || !eye.is_finite() || !d.is_finite() || !len.is_finite() {
        return Err("nonfinite static crowd card basis");
    }
    if len < 0.0001 {
        return Err("camera coincides with crowd XZ");
    }
    if !s
        .dimensions
        .into_iter()
        .chain(s.instance_scale)
        .all(|v| v.is_finite() && v > 0.)
        || !s.uv.into_iter().all(f32::is_finite)
    {
        return Err("nonfinite/invalid static crowd card specification");
    }
    let right = Vec3::new(-d.z / len, 0., d.x / len);
    let forward = Vec3::new(d.x / len, 0., d.z / len);
    if !right.is_finite() || !forward.is_finite() {
        return Err("nonfinite static crowd card axes");
    }
    // Source table: dimension-bearing unswayed corners, u1 at negative local X.
    // This fixture uses explicit draw5000 scales. f32 corners and PBR normals
    // remain diagnostic adaptations; actual actor RNG, sway/near LOD are open.
    let first = u32::try_from(batch.p.len()).map_err(|_| "crowd vertex index overflow")?;
    first.checked_add(3).ok_or("crowd vertex index overflow")?;
    let half_width = s.dimensions[0] * 0.5;
    let mut positions = [[0.; 3]; 4];
    let mut normals = [[0.; 3]; 4];
    let mut uvs = [[0.; 2]; 4];
    for (i, (x, y, u, v)) in [
        (-half_width, 0., s.uv[2], s.uv[3]),
        (-half_width, s.dimensions[1], s.uv[2], s.uv[1]),
        (half_width, 0., s.uv[0], s.uv[3]),
        (half_width, s.dimensions[1], s.uv[0], s.uv[1]),
    ]
    .into_iter()
    .enumerate()
    {
        let position =
            row.position + right * x * s.instance_scale[0] + Vec3::Y * y * s.instance_scale[1];
        // Authored curved-normal approximation, not source backlight/fog.
        let normal = (forward + right * (4. * x.signum())).normalize();
        // Finite basis alone does not prevent scaled corner/addition overflow.
        // Validate all outputs before appending any partial card to the batch.
        if !position.is_finite() || !normal.is_finite() {
            return Err("nonfinite static crowd card vertex");
        }
        positions[i] = position.to_array();
        normals[i] = normal.to_array();
        uvs[i] = [u, v];
    }
    batch.p.extend(positions);
    batch.n.extend(normals);
    batch.uv.extend(uvs);
    batch
        .indices
        .extend([first, first + 1, first + 2, first + 3, first + 2, first + 1]);
    Ok(())
}
fn spawn(
    mut commands: Commands,
    prepared: Res<Prepared>,
    view: Query<(&Transform, &Msaa), With<super::ViewerCamera>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<CrowdMaterial>>,
    settings: Res<super::graphics::Settings>,
) {
    let (camera, msaa) = view.single().expect("one static preview camera");
    assert_eq!(
        *msaa,
        Msaa::Sample4,
        "bounded crowd preview requires actual MSAA4"
    );
    let (diffuse_bytes, tint_bytes) = if let Some(actor) = &prepared.actor {
        (&actor.diffuse, &actor.tint)
    } else {
        (&prepared.diffuse, &prepared.tint)
    };
    let diffuse = images.add(
        super::texture_upload::image(
            diffuse_bytes,
            settings.sampler(false),
            RenderAssetUsages::default(),
        )
        .expect("validated crowd DDS"),
    );
    let tint = images.add(
        super::texture_upload::image_linear(
            tint_bytes,
            settings.sampler(false),
            RenderAssetUsages::default(),
        )
        .expect("validated crowd tint DDS"),
    );
    let handles: Vec<_> = prepared
        .specs
        .iter()
        .map(|s| {
            materials.add(ExtendedMaterial {
                base: StandardMaterial {
                    base_color_texture: Some(diffuse.clone()),
                    alpha_mode: if prepared.actor.is_some() {
                        AlphaMode::Opaque
                    } else {
                        AlphaMode::AlphaToCoverage
                    },
                    perceptual_roughness: 1.,
                    reflectance: 0.,
                    cull_mode: None,
                    double_sided: true,
                    ..default()
                },
                extension: Tint {
                    map: tint.clone(),
                    colour0: s.colours[0],
                    colour1: s.colours[1],
                    colour2: s.colours[2],
                },
            })
        })
        .collect();
    if let Some(actor) = &prepared.actor {
        let row = &prepared.rows[4275];
        let d = camera.translation - row.position;
        let len = d.x.hypot(d.z);
        assert!(
            d.is_finite() && len.is_finite() && len > 0.0001,
            "invalid one-actor authored yaw"
        );
        // Camera-facing yaw is authored. Source orientation_pair remains opaque.
        let forward = Vec3::new(d.x / len, 0., d.z / len);
        let yaw = Quat::from_rotation_y(forward.x.atan2(forward.z));
        let translation = row.position + yaw * actor.foot_offset;
        assert!(translation.is_finite());
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, actor.positions.clone());
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, actor.normals.clone());
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, actor.uv.clone());
        mesh.insert_indices(Indices::U16(actor.indices.clone()));
        commands.spawn((
            Name::new(
                if actor.pose.as_ref().is_some_and(|p| p.stored_key_hypothesis) {
                    "crowd-preview/one-key-HYPOTHESIS-female4275"
                } else if actor.pose.is_some() {
                    "crowd-preview/one-authored-arm-female4275"
                } else {
                    "crowd-preview/one-neutral-female4275"
                },
            ),
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(handles[1].clone()),
            Transform {
                translation,
                rotation: yaw,
                ..default()
            },
            NotShadowCaster,
        ));
        let label = if actor.pose.as_ref().is_some_and(|p| p.stored_key_hypothesis) {
            "crowd_key_hypothesis_spawn"
        } else if actor.pose.is_some() {
            "crowd_authored_arm_spawn"
        } else {
            "crowd_neutral_spawn"
        };
        println!(
            "{label} actors=1 ordinal=4275 row_position={:?} neutral_bounds={:?}/{:?} foot_offset={:?} raw_position_count={} raw_normal_count={} position_error={} palette_error={} inverse_affine_error={} yaw={:?} material=authored-near-PBR-tint-opaque-two-sided texture=near1024x512 textureShift=0 UV=source normals=weighted-normalized raw_attributes=retained shadows=off original_pose_parity=false",
            row.position,
            actor.source_min,
            actor.source_max,
            actor.foot_offset,
            actor.source_positions.len(),
            actor.source_normals.len(),
            actor.max_position_error,
            actor.max_palette_error,
            actor.max_inverse_affine_error,
            yaw
        );
        return;
    }
    let mut batches: BTreeMap<(usize, u32), Batch> = BTreeMap::new();
    for row in &prepared.rows {
        card(
            batches.entry((row.sex, row.cell)).or_default(),
            row,
            &prepared.specs[row.sex],
            camera.translation,
        )
        .expect("validated static billboard expansion");
    }
    for ((sex, cell), b) in batches {
        let mut m = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
        m.insert_attribute(Mesh::ATTRIBUTE_POSITION, b.p);
        m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, b.n);
        m.insert_attribute(Mesh::ATTRIBUTE_UV_0, b.uv);
        m.insert_indices(Indices::U32(b.indices));
        commands.spawn((
            Name::new(format!("crowd-preview/sex{sex}/cell{cell}")),
            Mesh3d(meshes.add(m)),
            MeshMaterial3d(handles[sex].clone()),
            Transform::IDENTITY,
            NotShadowCaster,
        ));
    }
    // Retain raw source metadata: neither field has been reinterpreted or used
    // to fabricate time/RNG. Reader tests check their preserved float bits.
    let _raw = prepared.rows.iter().fold(0u32, |v, r| {
        v ^ r.orientation_pair[0].to_bits()
            ^ r.orientation_pair[1].to_bits()
            ^ r.opaque_scalar.to_bits()
    });
}
#[cfg(test)]
mod tests {
    use super::*;

    fn flash_rows_fixture() -> (Vec<Row>, [FlashRequest; 3]) {
        let rows = (0..3)
            .map(|i| Row {
                position: Vec3::new(i as f32, 2., 3.),
                orientation_pair: [0., 1.],
                opaque_scalar: 0.,
                cell: i as u32 + 7,
                sex: usize::from(i == 2),
            })
            .collect();
        let requests = [
            FlashRequest {
                ordinal: 0,
                sex: 0,
                cell: 7,
            },
            FlashRequest {
                ordinal: 1,
                sex: 0,
                cell: 8,
            },
            FlashRequest {
                ordinal: 2,
                sex: 1,
                cell: 9,
            },
        ];
        (rows, requests)
    }
    #[test]
    fn flash_row_bridge_retains_exact_source_origin_and_order() {
        let (rows, requests) = flash_rows_fixture();
        let anchors = select_flash_rows(&rows, [true, true], &requests).unwrap();
        assert_eq!(anchors.len(), 3);
        for (i, a) in anchors.iter().enumerate() {
            assert_eq!(a.source_index, i as u32);
            assert_eq!(a.source_cell, rows[i].cell);
            assert_eq!(
                a.position.map(f32::to_bits),
                rows[i].position.to_array().map(f32::to_bits)
            );
        }
    }
    #[test]
    fn flash_row_bridge_rejects_duplicate_missing_group_cell_camera_and_nonfinite() {
        let (mut rows, requests) = flash_rows_fixture();
        let mut bad = requests;
        bad[1] = bad[0];
        assert!(select_flash_rows(&rows, [true, true], &bad).is_err());
        let mut bad = requests;
        bad[2].ordinal = 3;
        assert!(select_flash_rows(&rows, [true, true], &bad).is_err());
        let mut bad = requests;
        bad[0].sex = 1;
        assert!(select_flash_rows(&rows, [true, true], &bad).is_err());
        let mut bad = requests;
        bad[0].cell = 99;
        assert!(select_flash_rows(&rows, [true, true], &bad).is_err());
        assert!(select_flash_rows(&rows, [true, false], &requests).is_err());
        assert!(select_flash_rows(&rows, [true, true], &requests[..2]).is_err());
        rows[0].position.x = f32::NAN;
        assert!(select_flash_rows(&rows, [true, true], &requests).is_err());
    }
    #[test]
    #[ignore = "requires purchased GRID_GAME_ROOT; CPU only, no GPU"]
    fn purchased_flash_bridge_resolves_exact_three_usa_camera_rows() {
        let root = std::path::PathBuf::from(
            std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required"),
        );
        let route = Route::new("tracks/usa/detroit/route_8").unwrap();
        let prepared = prepare(&root, &route, Mode::StaticBillboards).unwrap();
        let anchors = flash_fixture_anchors(&root, &prepared).unwrap();
        assert_eq!(anchors.len(), 3);
        for (request, anchor) in FLASH_REQUESTS.iter().zip(anchors) {
            let row = &prepared.rows[request.ordinal];
            assert_eq!(anchor.source_index, request.ordinal as u32);
            assert_eq!(anchor.source_cell, request.cell);
            assert_eq!(row.sex, request.sex);
            assert_eq!(
                anchor.position.map(f32::to_bits),
                row.position.to_array().map(f32::to_bits)
            );
        }
    }

    #[test]
    fn static_billboard_has_source_anchor_dimensions_and_distinct_stance_uv() {
        let row = Row {
            position: Vec3::new(3., 4., 5.),
            orientation_pair: [0., 1.],
            opaque_scalar: 7.,
            cell: 0,
            sex: 0,
        };
        let s = Spec {
            uv: [0.1, 0.2, 0.3, 0.8],
            dimensions: [0.6, 1.5],
            colours: [Vec4::ONE; 3],
            instance_scale: [1., 1.],
        };
        let mut b = Batch::default();
        card(&mut b, &row, &s, Vec3::new(3., 4., 10.)).unwrap();
        assert_eq!(b.p.len(), 4);
        assert_eq!(b.indices.len(), 6);
        assert!((b.p[1][1] - b.p[0][1] - 1.5).abs() < 1e-6);
        assert_eq!(b.uv[0], [0.3, 0.8]);
        assert_eq!(b.uv[1], [0.3, 0.2]);
        assert_eq!(b.uv[2], [0.1, 0.8]);
        assert_eq!(b.uv[3], [0.1, 0.2]);
        assert_eq!(row.position, Vec3::new(3., 4., 5.));
        assert!(card(&mut Batch::default(), &row, &s, row.position).is_err());
    }
    #[test]
    fn invalid_preview_context_rejects_and_off_is_inert() {
        assert!(
            Mode::Off
                .validate(false, false, false, false, false, None, false)
                .is_ok()
        );
        assert!(
            Mode::Control
                .validate(true, true, true, true, true, Some(120), true)
                .is_ok()
        );
        for (baseline, reference, route, lod, render, frames, perf) in [
            (false, true, true, true, true, Some(120), true),
            (true, false, true, true, true, Some(120), true),
            (true, true, false, true, true, Some(120), true),
            (true, true, true, false, true, Some(120), true),
            (true, true, true, true, false, Some(120), true),
            (true, true, true, true, true, None, true),
            (true, true, true, true, true, Some(601), true),
            (true, true, true, true, true, Some(120), false),
        ] {
            assert!(
                Mode::StaticBillboards
                    .validate(baseline, reference, route, lod, render, frames, perf)
                    .is_err()
            );
        }
    }
    fn ordinary_card_fixture() -> (Row, Spec) {
        (
            Row {
                position: Vec3::ZERO,
                orientation_pair: [0., 1.],
                opaque_scalar: 7.5,
                cell: 0,
                sex: 0,
            },
            Spec {
                uv: [0.1, 0.2, 0.3, 0.8],
                dimensions: [0.6, 1.5],
                colours: [Vec4::ONE; 3],
                instance_scale: [1., 1.],
            },
        )
    }
    #[test]
    fn card_nonfinite_extreme_and_scaled_output_reject_without_partial_append() {
        let (ordinary, spec) = ordinary_card_fixture();
        for (position, eye) in [
            (Vec3::ZERO, Vec3::new(f32::NAN, 0., 1.)),
            (Vec3::new(f32::NAN, 0., 0.), Vec3::Z),
            (Vec3::new(-f32::MAX, 0., 0.), Vec3::new(f32::MAX, 0., 1.)),
        ] {
            let mut batch = Batch::default();
            let row = Row {
                position,
                ..ordinary.clone()
            };
            assert!(card(&mut batch, &row, &spec, eye).is_err());
            assert!(
                batch.p.is_empty()
                    && batch.n.is_empty()
                    && batch.uv.is_empty()
                    && batch.indices.is_empty()
            );
        }
        let mut batch = Batch::default();
        let huge = Spec {
            dimensions: [f32::MAX, f32::MAX],
            instance_scale: [4., 4.],
            ..spec
        };
        assert_eq!(
            card(&mut batch, &ordinary, &huge, Vec3::Z),
            Err("nonfinite static crowd card vertex")
        );
        assert!(batch.p.is_empty() && batch.indices.is_empty());
        // A finite basis and finite unscaled corner can also overflow only
        // when added to the origin, independently of multiplication overflow.
        let (mut origin_row, mut wide) = ordinary_card_fixture();
        origin_row.position = Vec3::new(f32::MAX, 0., 0.);
        wide.dimensions = [f32::MAX, 1.];
        let mut batch = Batch::default();
        assert_eq!(
            card(&mut batch, &origin_row, &wide, Vec3::new(f32::MAX, 0., 1.)),
            Err("nonfinite static crowd card vertex")
        );
        assert!(
            batch.p.is_empty()
                && batch.n.is_empty()
                && batch.uv.is_empty()
                && batch.indices.is_empty()
        );
    }
    #[test]
    fn source_scale_is_applied_once_and_uv_order_preserved() {
        let (row, spec) = ordinary_card_fixture();
        let s = Spec {
            instance_scale: [2., 0.5],
            ..spec
        };
        let mut b = Batch::default();
        card(&mut b, &row, &s, Vec3::new(0., 0., 10.)).unwrap();
        assert!((b.p[0][0] - b.p[2][0] - 1.2).abs() < 1e-6);
        assert!((b.p[1][1] - b.p[0][1] - 0.75).abs() < 1e-6);
        assert_eq!(b.uv, vec![[0.3, 0.8], [0.3, 0.2], [0.1, 0.8], [0.1, 0.2]]);
        assert!(
            b.p.iter()
                .flatten()
                .chain(b.n.iter().flatten())
                .all(|v| v.is_finite())
        );
    }
    #[test]
    fn selected_count_preflight_is_bounded_without_allocating_rows() {
        assert!(preflight_selected_rows(99_999, 1).is_ok());
        assert!(preflight_selected_rows(100_000, 0).is_ok());
        assert!(preflight_selected_rows(100_000, 1).is_err());
        assert!(preflight_selected_rows(usize::MAX, 1).is_err());
    }
    #[test]
    fn source_draw_5000_fixture_is_explicit() {
        assert_eq!(source_scale_fixture(0.05, 0.05), [1., f32::from_bits(0x3f732000)]);
        assert_eq!(source_scale_fixture(0., 0.), [1., 1.]);
    }
    #[test]
    #[ignore = "requires purchased GRID_GAME_ROOT; CPU only, no GPU"]
    fn purchased_detroit_usa_crowd_control() {
        let root = std::path::PathBuf::from(
            std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required"),
        );
        let route = Route::new("tracks/usa/detroit/route_8").unwrap();
        let raw = std::fs::read(root.join(route.file("crowd_standing.bin"))).unwrap();
        let catalogue = crowd_placements::read(&raw).unwrap();
        assert_eq!(catalogue.cells.len(), 106);
        assert_eq!(catalogue.total_instances, 6854);
        assert!(
            catalogue
                .groups
                .iter()
                .all(|g| matches!(&g.placements, crowd_placements::Placements::Standing(_)))
        );
        let p = prepare(&root, &route, Mode::Control).unwrap();
        assert_eq!(p.total, 6854);
        assert_eq!(p.rows.len(), 4667);
        assert_eq!(p.rows.iter().filter(|r| r.sex == 0).count(), 2439);
        assert_eq!(p.rows.iter().filter(|r| r.sex == 1).count(), 2228);
        let mut selected = 0;
        for group in &catalogue.groups {
            let sex = match group.instance_set {
                b"m_gr" => 0,
                b"f_gr" => 1,
                _ => continue,
            };
            let crowd_placements::Placements::Standing(rows) = &group.placements else {
                panic!("unexpected moving grandstand")
            };
            for r in rows {
                let out = &p.rows[selected];
                assert_eq!(
                    out.position.to_array().map(f32::to_bits),
                    r.position.map(f32::to_bits)
                );
                assert_eq!(
                    out.orientation_pair.map(f32::to_bits),
                    r.orientation_pair.map(f32::to_bits)
                );
                assert_eq!(out.opaque_scalar.to_bits(), r.opaque_scalar.to_bits());
                assert_eq!(out.cell, r.cell);
                assert_eq!(out.sex, sex);
                selected += 1;
            }
        }
        assert_eq!(selected, 4667);
        for spec in &p.specs {
            assert_eq!(spec.instance_scale, [1., f32::from_bits(0x3f732000)]);
        }
        // prepare also resolves exact source USA dataset/atlas IDs and formats.
        assert!(!p.diffuse.is_empty() && !p.tint.is_empty());
    }
}
