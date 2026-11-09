//! Source static glass coverage and an owner-approved solid grandstand backing.
//! Other object families retain existing cutouts.
//! glass_simple colour uses SRC_ALPHA/ONE_MINUS_SRC_ALPHA with alpha test off.
//! Bevy PBR depth/separate-alpha remain adaptations, not original pass parity.
use bevy::prelude::*;
use grid_formats::pssg;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn glass_materials(tree: &pssg::Tree<'_>) -> grid_formats::Result<BTreeSet<String>> {
    let mut result = BTreeSet::new();
    for i in 0..tree.nodes.len() {
        if tree.name(i) == b"SHADERINSTANCE"
            && tree.string(i, b"shaderGroup")? == b"#glass_simple.fx"
        {
            let id = String::from_utf8_lossy(tree.string(i, b"id")?);
            result.insert(format!("objects/{id}"));
        }
    }
    Ok(result)
}

/// `god_rays*.fx` light-shaft cards (Washington's `godray_tree`): the
/// shaderpack draws them alpha-blended with `fp_light_shafts`, a colour from
/// the shader constants, not the texture. Drawn as plain textured objects
/// they are black spindles in the sky (owner, 2026-11-02); light_shafts.rs
/// gives them the light-shaft material.
pub(crate) fn light_shaft_materials(tree: &pssg::Tree<'_>) -> grid_formats::Result<BTreeSet<String>> {
    let mut result = BTreeSet::new();
    for i in 0..tree.nodes.len() {
        if tree.name(i) == b"SHADERINSTANCE"
            && tree.string(i, b"shaderGroup")?.starts_with(b"#god_rays")
        {
            let id = String::from_utf8_lossy(tree.string(i, b"id")?);
            result.insert(format!("objects/{id}"));
        }
    }
    Ok(result)
}

/// Saved object_simple_mesh normal, velocity/depth, shadow and cheap passes
/// all specify cullFaceType=NONE. Match the material's source family, not names.
/// Other families retain their existing culling policy.
pub(crate) fn two_sided_materials(tree: &pssg::Tree<'_>) -> grid_formats::Result<BTreeSet<String>> {
    let mut result = BTreeSet::new();
    for i in 0..tree.nodes.len() {
        if tree.name(i) == b"SHADERINSTANCE"
            && tree.string(i, b"shaderGroup")? == b"#object_simple_mesh.fx"
        {
            let id = String::from_utf8_lossy(tree.string(i, b"id")?);
            result.insert(format!("objects/{id}"));
        }
    }
    Ok(result)
}

pub(crate) fn configure_sidedness(name: &str, two_sided: &BTreeSet<String>, material: &mut StandardMaterial) {
    if two_sided.contains(name) {
        material.cull_mode = None;
        // Preserve current front lighting and face the newly visible back normal
        // toward the camera. Source vFace also changes normal sign, but exact
        // D3D/front-winding and source lighting parity remain unverified.
        material.double_sided = true;
    }
}

pub(crate) fn configure(name: &str, glass: &BTreeSet<String>, material: &mut StandardMaterial) {
    // Mesh-family main colour/depth uses permanent RawMaterial at initial spawn.
    // Glass keeps current Blend adaptation: auxiliary/depth parity stays open.
    if glass.contains(name) { material.alpha_mode = AlphaMode::Blend; }
}

/// An object material's emissive map and the colour it is added with, in the
/// original's shading units (light 1 = `backlight_suncolour`'s peak).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Emissive {
    /// Texture key `container#name` as the diffuse bindings (`objectstextures.pssg#...`).
    pub texture: String,
    pub colour: [f32; 3],
    /// `object_simple_night_emissive*`: `colour` is further scaled by the
    /// lighting manager's `EmissiveTime` (emissive_time).
    pub night: bool,
}

/// The object materials whose original pixel program adds `TEmissiveMap`
/// (docs/GRAPHICS_AUDIT.md, "Emissive objects"; shaderpack disassembly):
/// `object_simple_emissive*`, `object_lod_emissive*`: `out += E.rgb`, unscaled
/// (`EmissiveMapScale` is not read); `object_animated_emissive*`:
/// `E.rgb * EmissiveColour * EmissivePower` (the UV animation is not
/// reproduced); `object_simple_night_emissive*`: `E.rgb * EmissiveTime *
/// EmissivePower / 3` (EmissiveTime: 0x9790E0; the program also adds the map
/// to the light through the street-light mask, unported). All add before fog.
/// Without the map these screens and signs draw their near-black diffuse
/// (Shibuya `big_screen_*`). `object_emissive_ripple*` (Shibuya water) is left
/// out: its emissive only lights through the street-light mask.
pub(crate) fn emissive_materials(tree: &pssg::Tree<'_>) -> grid_formats::Result<BTreeMap<String, Emissive>> {
    use grid_formats::material;
    let bindings = material::read_texture_bindings(tree)?;
    let mut result = BTreeMap::new();
    for s in material::read_constants(tree)? {
        let group = s.group.strip_suffix(b".fx").unwrap_or(s.group);
        let animated = group.starts_with(b"object_animated_emissive");
        let night = group.starts_with(b"object_simple_night_emissive");
        if !(animated || night || group.starts_with(b"object_simple_emissive") || group.starts_with(b"object_lod_emissive")) {
            continue;
        }
        let Some(binding) = bindings
            .iter()
            .find(|b| b.shader == s.shader && b.sampler == b"TEmissiveMap")
        else {
            continue;
        };
        let power = s.values.get(&b"EmissivePower"[..]).and_then(|v| v.first().copied()).unwrap_or(1.);
        let colour = if animated {
            let c = s.values.get(&b"EmissiveColour"[..]).map(|v| v.as_slice()).unwrap_or(&[1., 1., 1.]);
            std::array::from_fn(|i| c.get(i).copied().unwrap_or(1.) * power)
        } else if night {
            [power / 3.; 3]
        } else {
            [1.; 3]
        };
        result.insert(
            format!("objects/{}", String::from_utf8_lossy(s.shader)),
            Emissive {
                texture: format!(
                    "{}#{}",
                    String::from_utf8_lossy(binding.container.unwrap_or(b"objectstextures.pssg")),
                    String::from_utf8_lossy(binding.texture)
                ),
                colour,
                night,
            },
        );
    }
    Ok(result)
}

/// Adds an object's emissive map. `unit` is Bevy radiance per original light
/// unit before exposure (the baseline sun's lux over `backlight_suncolour`'s
/// peak, over pi, as graphics::Phong), so the map adds what the original adds
/// next to `albedo * light`; exposure applies to it like to lit surfaces.
/// `time` is the current `EmissiveTime` (night materials only).
pub(crate) fn configure_emissive(emissive: &Emissive, texture: Handle<Image>, unit: f32, time: f32, material: &mut StandardMaterial) {
    material.emissive = emissive_colour(emissive, unit, time);
    material.emissive_texture = Some(texture);
    material.emissive_exposure_weight = 1.;
}

fn emissive_colour(emissive: &Emissive, unit: f32, time: f32) -> LinearRgba {
    let scale = unit * if emissive.night { time.max(0.) } else { 1. };
    let [r, g, b] = emissive.colour.map(|c| c.max(0.) * scale);
    LinearRgba::rgb(r, g, b)
}

/// 0x9790E0 stores 3.0 (0x40400000) in forced night and as the start value.
pub(crate) const FORCED_NIGHT_EMISSIVE_TIME: f32 = 3.;

/// The original's `EmissiveTime` (0x9790E0, lighting manager +0x250): the
/// daynight `emissive` curve in the timeline mode, 3.0 in forced night, else
/// 0 (3.0 only after a forced-night route ran in the same session, a sticky
/// carry-over not reproduced).
pub(crate) fn emissive_time(mode: grid_tools::time_of_day::Mode, timeline: Option<&grid_tools::time_of_day::Timeline>, t: f64) -> f32 {
    use grid_tools::time_of_day::Mode;
    match mode {
        Mode::Timeline => timeline.and_then(|tl| tl.value("emissive", t)).unwrap_or(FORCED_NIGHT_EMISSIVE_TIME),
        Mode::ForcedNight => FORCED_NIGHT_EMISSIVE_TIME,
        Mode::None => 0.,
    }
}

/// The night-emissive materials and the `EmissiveTime` they show.
#[derive(Resource)]
pub(crate) struct NightEmissive {
    pub materials: Vec<(Handle<StandardMaterial>, Emissive)>,
    pub unit: f32,
    pub time: f32,
}

/// Follows the Le Mans timeline's `emissive` curve as the clock runs.
pub(crate) fn update_night_emissive(
    event: Option<Res<crate::le_mans_event::LeMansEvent>>,
    night: Option<ResMut<NightEmissive>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let (Some(event), Some(mut night)) = (event, night) else { return };
    let time = emissive_time(event.mode, event.timeline.as_ref(), event.time_of_day);
    if night.materials.is_empty() || (time - night.time).abs() < 1e-3 {
        return;
    }
    night.time = time;
    for (handle, emissive) in &night.materials {
        if let Some(mut material) = materials.get_mut(handle) {
            material.emissive = emissive_colour(emissive, night.unit, time);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn shibuya_big_screens_add_their_emissive_map() {
        let root = std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required");
        let bytes = std::fs::read(std::path::Path::new(&root).join("tracks/japan/shibuya/objects.pssg")).unwrap();
        let tree = pssg::read_tree(&bytes).unwrap();
        let emissive = emissive_materials(&tree).unwrap();
        let screens: Vec<_> = emissive.keys().filter(|k| k.starts_with("objects/big_screen")).collect();
        assert!(!screens.is_empty(), "no big_screen emissive among {:?}", emissive.keys().take(8).collect::<Vec<_>>());
        let mut material = StandardMaterial::default();
        configure_emissive(&emissive[screens[0]], Handle::default(), 2., 0., &mut material);
        assert!(material.emissive_texture.is_some());
        assert_eq!(material.emissive_exposure_weight, 1.);
        assert!(material.emissive.red > 0.);
    }

    #[test]
    fn emissive_scales_the_colour_by_the_light_unit() {
        let mut material = StandardMaterial::default();
        let e = Emissive { texture: "t".into(), colour: [1., 0.5, -1.], night: false };
        configure_emissive(&e, Handle::default(), 4., 0., &mut material);
        assert_eq!(material.emissive, LinearRgba::rgb(4., 2., 0.));
        // Night materials follow EmissiveTime: off by day, on at night.
        let n = Emissive { night: true, ..e };
        configure_emissive(&n, Handle::default(), 4., 0., &mut material);
        assert_eq!(material.emissive, LinearRgba::rgb(0., 0., 0.));
        configure_emissive(&n, Handle::default(), 4., 3., &mut material);
        assert_eq!(material.emissive, LinearRgba::rgb(12., 6., 0.));
    }

    #[test]
    fn emissive_time_follows_the_time_of_day_mode() {
        use grid_tools::time_of_day::Mode;
        assert_eq!(emissive_time(Mode::None, None, 0.5), 0.);
        assert_eq!(emissive_time(Mode::ForcedNight, None, 0.5), 3.);
        assert_eq!(emissive_time(Mode::Timeline, None, 0.5), 3.);
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn detroit_stand_glass_does_not_use_the_concrete_fence_cutout() {
        let root = std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required");
        let bytes = std::fs::read(std::path::Path::new(&root).join("tracks/usa/detroit/objects.pssg")).unwrap();
        let tree = pssg::read_tree(&bytes).unwrap();
        let glass = glass_materials(&tree).unwrap();
        let stand = "objects/gg_building_grandstand_01_j_04";
        let fence = "objects/concrete_fence_white_end_01";
        let backing = "objects/building_grandstand_01_j_01";
        assert!((0..tree.nodes.len()).any(|i| tree.name(i) == b"SHADERINSTANCE"
            && tree.string(i, b"id").is_ok_and(|id| id == b"building_grandstand_01_j_01")
            && tree.string(i, b"shaderGroup").is_ok_and(|group| group == b"#object_simple_mesh.fx")));
        assert!(glass.contains(stand));
        assert!(!glass.contains(fence));
        for (name, expected) in [(stand, AlphaMode::Blend), (fence, AlphaMode::Mask(0.5)), (backing, AlphaMode::Mask(0.5)), ("trees/leaf", AlphaMode::Mask(0.5))] {
            let mut material = StandardMaterial { alpha_mode: AlphaMode::Mask(0.5), ..default() };
            configure(name, &glass, &mut material);
            assert_eq!(material.alpha_mode, expected);
        }
    }
}


/// Authored Bevy coverage diagnostic, NOT raw source GT0/A2C parity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum CoverageMode { #[default] Off, Control, A2c, Mask35 }
impl CoverageMode {
    pub(crate) fn parse(value: &str) -> Result<Self, &'static str> {
        match value { "off" => Ok(Self::Off), "control" => Ok(Self::Control),
            "a2c" => Ok(Self::A2c), "mask35" => Ok(Self::Mask35), _ => Err("--object-coverage needs off, control, a2c or mask35") }
    }
    pub(crate) fn supports_dlaa(self, aa: u8) -> bool {
        cfg!(feature = "dlaa") && aa == 3 && matches!(self, Self::Control | Self::Mask35)
    }
    pub(crate) fn validate(self, c: CoverageContext) -> Result<(), &'static str> {
        if self == Self::Off { return Ok(()); }
        if (c.enhanced && !self.supports_dlaa(c.aa)) || c.drive || !c.reference || c.collision || c.headless
            || !c.render || !c.detroit || !c.lod0 || c.perf || c.compositor
            || c.frames.is_none_or(|n| !(1..=600).contains(&n)) {
            return Err("--object-coverage requires static rendered Detroit LOD0 reference, baseline MSAA4 or Control/Mask35 enhanced native DLAA, --benchmark-frames1..600, compositor off and no GRID_PERF_TEST");
        }
        Ok(())
    }
}
#[derive(Clone, Copy)]
pub(crate) struct CoverageContext {
    pub enhanced: bool, pub aa: u8, pub drive: bool, pub reference: bool,
    pub collision: bool, pub headless: bool, pub render: bool,
    pub detroit: bool, pub lod0: bool, pub perf: bool, pub compositor: bool,
    pub frames: Option<u32>,
}
pub(crate) const COVERAGE_TARGET: &str = "objects/concrete_fence_white_end_01";
pub(crate) fn configure_coverage(mode: CoverageMode, name: &str, m: &mut StandardMaterial) {
    // Off/Control perform ZERO material mutations, including alpha/cull/depth.
    if name == COVERAGE_TARGET {
        match mode {
            CoverageMode::A2c => m.alpha_mode = AlphaMode::AlphaToCoverage,
            CoverageMode::Mask35 => m.alpha_mode = AlphaMode::Mask(0.35),
            CoverageMode::Off | CoverageMode::Control => {},
        }
    }
}
pub(crate) fn assert_coverage_msaa(
    settings: Res<super::graphics::Settings>,
    cameras: Query<&Msaa, With<super::ViewerCamera>>,
) {
    let samples = cameras.single().expect("object coverage requires one main camera");
    let expected = if settings.enhanced() {
        assert!(settings.object_coverage.supports_dlaa(settings.anti_aliasing),
            "only Control/Mask35 support enhanced native DLAA object coverage");
        Msaa::Off
    } else { Msaa::Sample4 };
    assert_eq!(*samples, expected, "object coverage actual MSAA differs from validated preset");
}
#[cfg(test)]
mod coverage_tests {
    use super::*;
    fn valid() -> CoverageContext { CoverageContext { enhanced:false, aa:1, drive:false,
        reference:true, collision:false, headless:false, render:true, detroit:true,
        lod0:true, perf:false, compositor:false, frames:Some(20) } }
    #[test]
    fn coverage_gate_rejects_playable_perf_and_unbounded_contexts() {
        for mode in [CoverageMode::Control, CoverageMode::A2c, CoverageMode::Mask35] {
            assert!(mode.validate(valid()).is_ok());
            let invalid = [CoverageContext { drive:true, ..valid() },
                CoverageContext { enhanced:true, ..valid() }, CoverageContext { perf:true, ..valid() },
                CoverageContext { frames:None, ..valid() }, CoverageContext { frames:Some(0), ..valid() },
                CoverageContext { frames:Some(601), ..valid() }, CoverageContext { render:false, ..valid() },
                CoverageContext { reference:false, ..valid() }, CoverageContext { collision:true, ..valid() },
                CoverageContext { headless:true, ..valid() }, CoverageContext { detroit:false, ..valid() },
                CoverageContext { lod0:false, ..valid() }, CoverageContext { compositor:true, ..valid() }];
            for c in invalid { assert!(mode.validate(c).is_err()); assert!(CoverageMode::Off.validate(c).is_ok()); }
        }
    }
    #[test]
    fn enhanced_coverage_only_allows_native_dlaa_control_and_mask35() {
        for mode in [CoverageMode::Control, CoverageMode::Mask35, CoverageMode::A2c] {
            for aa in [0, 1, 2, 3, 4] {
                let c = CoverageContext { enhanced:true, aa, ..valid() };
                let allowed = cfg!(feature = "dlaa") && aa == 3
                    && matches!(mode, CoverageMode::Control | CoverageMode::Mask35);
                assert_eq!(mode.validate(c).is_ok(), allowed);
                assert!(CoverageMode::Off.validate(c).is_ok());
            }
        }
        let dlaa = CoverageContext { enhanced:true, aa:3, ..valid() };
        for mode in [CoverageMode::Control, CoverageMode::Mask35] {
            for c in [CoverageContext { drive:true, ..dlaa },
                CoverageContext { perf:true, ..dlaa }, CoverageContext { compositor:true, ..dlaa },
                CoverageContext { frames:None, ..dlaa }, CoverageContext { render:false, ..dlaa },
                CoverageContext { detroit:false, ..dlaa }, CoverageContext { reference:false, ..dlaa },
                CoverageContext { headless:true, ..dlaa }, CoverageContext { collision:true, ..dlaa },
                CoverageContext { lod0:false, ..dlaa }, CoverageContext { frames:Some(601), ..dlaa }] {
                assert!(mode.validate(c).is_err());
            }
        }
    }
    #[test]
    fn coverage_control_is_full_material_neutral_and_a2c_is_exact_target_only() {
        let original = StandardMaterial { alpha_mode: AlphaMode::Mask(0.5),
            perceptual_roughness:0.73, metallic:0.17, depth_bias:3.,
            double_sided:true, cull_mode:None, ..default() };
        for mode in [CoverageMode::Off, CoverageMode::Control] {
            let mut m=original.clone(); configure_coverage(mode,COVERAGE_TARGET,&mut m);
            assert_eq!(format!("{m:?}"), format!("{original:?}"));
        }
        for (mode, expected) in [(CoverageMode::A2c, AlphaMode::AlphaToCoverage),
            (CoverageMode::Mask35, AlphaMode::Mask(0.35))] {
            for name in ["objects/building_grandstand_01_j_01", "objects/gg_building_grandstand_01_j_04",
                "objects/concrete_fence_white_end_01!0", "trees/leaf"] {
                let mut m=original.clone(); configure_coverage(mode,name,&mut m);
                assert_eq!(format!("{m:?}"),format!("{original:?}"));
            }
            let mut m=original.clone(); configure_coverage(mode,COVERAGE_TARGET,&mut m);
            assert_eq!(m.alpha_mode,expected);
            m.alpha_mode=original.alpha_mode;
            assert_eq!(format!("{m:?}"),format!("{original:?}"));
        }
        assert!(CoverageMode::parse("blend").is_err());
    }
}

/// Separate bounded native-DLAA drive contract. Static validate is unchanged.
#[derive(Clone, Copy)]
pub(crate) struct MovingCoverageContext {
    pub base: CoverageContext,
    pub exact_route_and_car: bool,
    pub clean_settings: bool,
    pub solo: bool,
    pub no_menu: bool,
    pub no_extra_diagnostics: bool,
    pub native_resolution_land_off: bool,
    pub capture_fits: bool,
}
impl CoverageMode {
    pub(crate) fn validate_moving(self, c: MovingCoverageContext) -> Result<(), &'static str> {
        let b=c.base;
        if !b.drive || !b.enhanced || !self.supports_dlaa(b.aa) || !b.reference || b.collision || b.headless || !b.render || !b.detroit || !b.lod0 || b.perf || b.compositor || b.frames.is_some()
            || !c.exact_route_and_car || !c.clean_settings || !c.solo || !c.no_menu || !c.no_extra_diagnostics || !c.native_resolution_land_off || !c.capture_fits {
            return Err("moving object coverage requires isolated solo Detroit route_8 Lacetti LOD0 reference, enhanced native DLAA Control/Mask35, land off/native1080, clean settings, drive1..600ticks and30..60frame bounded capture, no menu/perf/other audits");
        }
        Ok(())
    }
}
#[cfg(test)]
mod moving_coverage_tests {
    use super::*;
    fn valid() -> MovingCoverageContext { MovingCoverageContext {
        base:CoverageContext { enhanced:true,aa:3,drive:true,reference:true,collision:false,headless:false,render:true,detroit:true,lod0:true,perf:false,compositor:false,frames:None },
        exact_route_and_car:true,clean_settings:true,solo:true,no_menu:true,no_extra_diagnostics:true,native_resolution_land_off:true,capture_fits:true } }
    #[test]
    fn moving_exception_is_separate_and_all_context_guards_are_required() {
        let c=valid();
        for mode in [CoverageMode::Control,CoverageMode::Mask35] {
            assert_eq!(mode.validate_moving(c).is_ok(),cfg!(feature="dlaa"));
            // Original static validator still rejects this drive.
            assert!(mode.validate(c.base).is_err());
            let invalid=[MovingCoverageContext { exact_route_and_car:false,..c },MovingCoverageContext { clean_settings:false,..c },MovingCoverageContext { solo:false,..c },MovingCoverageContext { no_menu:false,..c },MovingCoverageContext { no_extra_diagnostics:false,..c },MovingCoverageContext { native_resolution_land_off:false,..c },MovingCoverageContext { capture_fits:false,..c }];
            for bad in invalid {assert!(mode.validate_moving(bad).is_err());}
            for b in [CoverageContext { drive:false,..c.base },CoverageContext { enhanced:false,..c.base },CoverageContext { aa:0,..c.base },CoverageContext { aa:1,..c.base },CoverageContext { aa:2,..c.base },CoverageContext { reference:false,..c.base },CoverageContext { collision:true,..c.base },CoverageContext { headless:true,..c.base },CoverageContext { render:false,..c.base },CoverageContext { detroit:false,..c.base },CoverageContext { lod0:false,..c.base },CoverageContext { perf:true,..c.base },CoverageContext { compositor:true,..c.base },CoverageContext { frames:Some(120),..c.base }] {assert!(mode.validate_moving(MovingCoverageContext {base:b,..c}).is_err());}
        }
        for mode in [CoverageMode::Off,CoverageMode::A2c] {assert!(mode.validate_moving(c).is_err());}
    }
}

/// Authored atlas-space thickness trial; only the material-specific image clone changes.
/// Radius one takes the audited median base wire cross-section about 5 -> 9 texels.
pub(crate) fn double_wire(image: &mut Image) -> Result<(), &'static str> {
    use bevy::render::render_resource::TextureFormat;
    let size = image.texture_descriptor.size;
    if size.width != 512 || size.height != 512 || size.depth_or_array_layers != 1
        || image.texture_descriptor.mip_level_count != 10 {
        return Err("fence trial requires the audited 512x512 complete mip atlas");
    }
    super::texture_upload::decode_in_place(image).map_err(|_| "fence BC decode failed")?;
    if image.texture_descriptor.format != TextureFormat::Rgba8UnormSrgb {
        return Err("fence trial requires RGBA8 sRGB");
    }
    let data = image.data.as_mut().ok_or("missing fence pixels")?;
    let expected: usize = (0..10).map(|m| (512usize >> m).max(1).pow(2)*4).sum();
    if data.len() != expected { return Err("fence mip payload differs"); }
    let mut alpha: Vec<u8> = data[..512*512*4].as_chunks::<4>().0.iter().map(|p| p[3]).collect();
    let source = alpha.clone();
    for y in 0..384usize {
        for x in 0..512usize {
            let mut a = source[y*512+x];
            for dy in -1..=1isize {
                let sy = (y as isize+dy).clamp(0,383) as usize;
                for dx in -1..=1isize {
                    let sx = (x as isize+dx).rem_euclid(512) as usize;
                    a = a.max(source[sy*512+sx]);
                }
            }
            alpha[y*512+x] = a;
        }
    }
    // Rebuild only alpha inside the mapped wire ROI at each level. All RGB and
    // every alpha sample outside that ROI retain the decoded source bytes.
    let mut width = 512usize;
    let mut wire_height = 384usize;
    let mut offset = 0;
    for level in 0..10 {
        for y in 0..wire_height {
            for x in 0..width { data[offset+(y*width+x)*4+3] = alpha[y*width+x]; }
        }
        offset += width*width*4;
        if level == 9 { break; }
        let next_width = width/2;
        let mut next = vec![0u8;next_width*next_width];
        for y in 0..next_width {
            for x in 0..next_width {
                let sum = u16::from(alpha[(y*2)*width+x*2])
                    + u16::from(alpha[(y*2)*width+x*2+1])
                    + u16::from(alpha[(y*2+1)*width+x*2])
                    + u16::from(alpha[(y*2+1)*width+x*2+1]);
                next[y*next_width+x] = ((sum+2)/4) as u8;
            }
        }
        alpha = next; width = next_width; wire_height /= 2;
    }
    println!("fence_wire scale=2 authored=true atlas=512x512 wire_roi=0,0,512,384 radius=1 nominal_median_texels=5_to_9 rgb=source other_materials=source rails_concrete=source");
    Ok(())
}

#[cfg(test)]
mod sidedness_tests {
    use super::*;
    use bevy::render::render_resource::Face;

    #[test]
    fn source_family_classification_is_exact_and_material_names_are_irrelevant() {
        let inputs: &[(&str, &str)] = &[
            ("arbitrary_panel", "#object_simple_mesh.fx"),
            ("closed_building", "#object_simple_mesh.fx"),
            ("fence_named_but_other_family", "#object_simple.fx"),
            ("glass", "#glass_simple.fx"),
            ("skin_variant", "#object_simple_mesh_skin.fx"),
            ("suffix", "#object_simple_mesh.fx_extra"),
            ("missing_hash", "object_simple_mesh.fx"),
        ];
        let encoded: Vec<_> = inputs.iter().flat_map(|(id, group)| [*id, *group])
            .map(|s| { let mut b = (s.len() as u32).to_be_bytes().to_vec(); b.extend_from_slice(s.as_bytes()); b }).collect();
        let tree = pssg::Tree {
            framing: pssg::Pssg {
                declared_size: 0, attribute_count: 2, root_id: 1,
                root_offset: 0, root_attributes: vec![], root_payload: 0..0,
                schema: vec![pssg::ElementDefinition { id: 1, name: b"SHADERINSTANCE",
                    attributes: vec![pssg::AttributeDefinition { id: 1, name: b"id" },
                        pssg::AttributeDefinition { id: 2, name: b"shaderGroup" }] }],
            },
            nodes: encoded.as_chunks::<2>().0.iter().map(|pair| pssg::Node {
                id: 1, offset: 0, parent: None, children: vec![], data: None,
                attributes: vec![pssg::Attribute { id: 1, value: &pair[0] },
                    pssg::Attribute { id: 2, value: &pair[1] }],
            }).collect(),
        };
        let materials = two_sided_materials(&tree).unwrap();
        assert_eq!(materials, BTreeSet::from(["objects/arbitrary_panel".into(), "objects/closed_building".into()]));
        assert_eq!(glass_materials(&tree).unwrap(), BTreeSet::from(["objects/glass".into()]));
    }

    #[test]
    fn sidedness_only_changes_selected_culling_and_normal_policy() {
        let selected = BTreeSet::from(["objects/arbitrary_panel".into()]);
        let original = StandardMaterial { alpha_mode: AlphaMode::Mask(0.5),
            cull_mode: Some(Face::Back), double_sided: false,
            perceptual_roughness: 0.71, metallic: 0.23, ..default() };
        let mut m = original.clone();
        configure_sidedness("objects/arbitrary_panel", &selected, &mut m);
        assert_eq!(m.cull_mode, None);
        assert!(m.double_sided);
        m.cull_mode = original.cull_mode;
        m.double_sided = original.double_sided;
        assert_eq!(format!("{m:?}"), format!("{original:?}"));
        for name in ["objects/closed_body_other_family", "Car Body Paint", "trees/leaf", "objects/arbitrary_panel!0"] {
            let mut m = original.clone();
            configure_sidedness(name, &selected, &mut m);
            assert_eq!(format!("{m:?}"), format!("{original:?}"));
        }
        let tree_material = StandardMaterial { cull_mode: None, double_sided: false, ..original.clone() };
        let mut m = tree_material.clone();
        configure_sidedness("trees/leaf", &selected, &mut m);
        assert_eq!(format!("{m:?}"), format!("{tree_material:?}"));
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn purchased_detroit_and_donington_use_family_culling_without_global_disable() {
        let root = std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required"));
        for (track, selected, other) in [
            ("usa/detroit", "concrete_fence_white_end_01", ""),
            ("europe/donington", "building_grandstand_01_g_01", "building_m_01"),
        ] {
            let bytes = std::fs::read(root.join("tracks").join(track).join("objects.pssg")).unwrap();
            let tree = pssg::read_tree(&bytes).unwrap();
            let materials = two_sided_materials(&tree).unwrap();
            assert!(materials.contains(&format!("objects/{selected}")), "{track}");
            if track == "usa/detroit" {
                assert_eq!(materials.len(), 22);
                assert!(materials.contains("objects/building_grandstand_01_j_01"));
            }
            let mut m = StandardMaterial { cull_mode: Some(Face::Back), double_sided: false, ..default() };
            configure_sidedness(&format!("objects/{selected}"), &materials, &mut m);
            assert_eq!(m.cull_mode, None);
            assert!(m.double_sided);
            if !other.is_empty() {
                assert!(!materials.contains(&format!("objects/{other}")));
                let mut m = StandardMaterial { cull_mode: Some(Face::Back), double_sided: false, ..default() };
                configure_sidedness(&format!("objects/{other}"), &materials, &mut m);
                assert_eq!(m.cull_mode, Some(Face::Back));
                assert!(!m.double_sided);
            }
        }
    }
}
