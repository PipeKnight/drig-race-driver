//! Independently authored rendering presets. Neither is an original GRID renderer.
use bevy::{
    anti_alias::{
        smaa::{Smaa, SmaaPreset},
        taa::TemporalAntiAliasing,
    },
    camera::{Exposure, Hdr},
    core_pipeline::tonemapping::Tonemapping,
    image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    light::{
        Atmosphere, AtmosphereEnvironmentMapLight, CascadeShadowConfig, CascadeShadowConfigBuilder,
        DirectionalLightShadowMap, ShadowFilteringMethod, atmosphere::ScatteringMedium,
        light_consts::lux,
    },
    pbr::{AtmosphereSettings, DistanceFog, FogFalloff, ScreenSpaceAmbientOcclusion, ScreenSpaceReflections},
    post_process::{bloom::Bloom, motion_blur::MotionBlur},
    prelude::*,
    render::view::{ColorGrading, ColorGradingGlobal},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Preset {
    #[default]
    Baseline,
    Enhanced,
}

impl Preset {
    pub fn parse(value: &str) -> Result<Self, &'static str> {
        match value {
            "baseline" => Ok(Self::Baseline),
            "enhanced" => Ok(Self::Enhanced),
            _ => Err("--graphics needs baseline or enhanced"),
        }
    }
}

#[derive(Clone, Copy, Resource)]
pub struct Settings {
    /// Optional authored Gaussian filtering floor from actual frame duration.
    pub adaptive_wheel_exposure: bool,
    /// Authored adaptive Gaussian sigma as a fraction of frame duration.
    pub wheel_filter_floor: f32,
    /// Optional authored angular support cap, radians; preserves visible detail.
    pub wheel_max_arc: Option<f32>,
    /// Experimental swept-hull radial scene filter, player only; default off.
    pub wheel_geometry_shutter_seconds: f32,
    /// Fixed exposure duration, opt-in until wheel filtering is accepted.
    pub wheel_shutter_seconds: f32,
    /// Bounded read-only spin prediction between physical ticks.
    pub smooth_wheel_motion: bool,
    /// 0: TAA diagnostic, 1: SMAA Ultra (default), 2: unfiltered, 3: native DLAA.
    pub anti_aliasing: u8,
    /// 0: sky IBL, 1: deferred control, 2: SSR, 3: local HDR scene probe.
    pub reflection_mode: u8,
    /// The cars' dynamic environment cube (docs/CAR_REFLECTIONS.md).
    pub car_reflections: crate::car_reflections::Level,
    pub probe_size: u32,
    pub moving_probe: bool,
    pub ssr_thickness: f32,
    pub layered_track: bool,
    /// Source blended-alpha base-F0 adaptation; CLI resolves eligible track defaults.
    pub surface_specular: super::track_surface::Specular,
    /// Default-off diagnostic: match stock PBR view mip bias on source surface samples.
    pub surface_view_mip_bias: bool,
    /// Default-off RGB-only low-pass pilot for audited Donington asphalt layers.
    pub asphalt_soften: f32,
    /// Blend original and filtered road RGB;1 preserves the existing mip pilot.
    pub asphalt_filter_strength: f32,
    /// Optional authored roughness for audited standalone Donington asphalt only.
    pub asphalt_roughness: Option<f32>,
    /// Default-off authored view-dependent F0 gain; not independent physical F90.
    pub asphalt_grazing_gain: Option<f32>,
    /// Optional Bevy directional blur; fraction of the rendered frame, not milliseconds.
    pub motion_shutter: f32,
    /// Bounded scripted DLAA diagnostic only; never a playable default.
    pub dlaa_reset_every_frame: bool,
    pub thin_motion_antenna: bool,
    pub ground_edge_pilot: bool,
    pub source_land: bool,
    /// The route's source forest (`trees.pssg`, `trees.bin`), on by default;
    /// `--trees off` leaves it out.
    pub source_trees: bool,
    pub player_race_number: Option<grid_formats::car_decals::Number>,
    pub source_race_numbers: bool,
    /// Source green mask adapted only to ambient PBR occlusion, default off.
    pub source_body_occlusion: bool,
    /// Authored base-lobe alpha-mask pilot; requires the paint-specular build feature.
    pub source_body_specular: bool,
    /// Diagnostic white map for rendered compensation parity; never a launcher default.
    pub white_body_specular: bool,
    /// Fixed photographic exposure; +1EV halves scene light before tonemapping.
    pub exposure_ev100: f32,
    /// Optional single-variable authored-grade diagnostic; default preserves current look.
    pub post_saturation_override: Option<f32>,
    /// Optional rendered authored-grade diagnostic; absent preserves AgX.
    pub tone_map_override: Option<Tonemapping>,
    /// Window-only material control; light lenses are a separate material.
    pub window_roughness: f32,
    pub window_reflectance: f32,
    /// Bounded diagnostic: no change to accepted interactive driving materials.
    pub window_pilot: super::window_response::Mode,
    pub object_coverage: super::object_materials::CoverageMode,
    /// Opt-in approximately doubled wire alpha in the exact Detroit fence atlas.
    pub double_fence_wire: bool,
    /// Authored player-paint coat controls, independent of base mask and glass.
    pub paint_clearcoat: f32,
    /// Default-off authored environment-only paint layer; not original shader parity.
    pub paint_environment: f32,
    /// Default-off authored Lacetti reflector pilot; covers and matt remain separate.
    pub lamp_reflectors: super::lamp_materials::Mode,
    /// Independent default-off clear-cover normal/PBR experiment.
    pub lamp_lenses: super::lamp_materials::Mode,
    /// Independent default-off section emission adaptation.
    pub lamp_signals: super::lamp_signals::Mode,
    pub paint_coat_roughness: f32,
    /// Optional saved RGB gradient on distant treesheets; alpha stays unchanged.
    pub tree_sheet_colours: bool,
    /// Diagnostic isolation of ambient occlusion and grass shadow reception.
    pub ambient_occlusion: bool,
    pub grass_receive_shadows: bool,
    /// Authored enhanced soft coverage; mask remains available for comparison.
    pub grass_soft_alpha: bool,
    /// Authored enhanced directional-shadow reach; defaults to the verified 250 m.
    pub shadow_distance_metres: f32,
    /// `[video] shadows` with the original's layout (shadow_layout.rs).
    pub shadow_quality: super::shadow_layout::Quality,
    /// Authored coverage-preserving leaf alpha mips (`--trees coverage`, the
    /// default); `--trees source` keeps the source mips.
    pub foliage_coverage: bool,
    pub clearcoat_prepass_fix: bool,
    pub tread_normals: bool,
    pub layered_car: bool,
    pub source_ibl: bool,
    pub smooth_tyres: bool,
    pub track_grade: bool,
    pub saved_grade: Option<grid_formats::graphics_grade::Grade>,
    /// Opt-in distance-only adaptation of saved source fog; height formula unknown.
    pub saved_fog: Option<grid_formats::track_lighting::Fog>,
    pub benchmark_frames: Option<u32>,
    /// Optional bounded extension for offscreen diagnostics on busy machines.
    pub capture_timeout_seconds: Option<f64>,
    pub source_sky: bool,
    /// Optional recovered sky scale/exponent applied to background and source IBL.
    pub source_sky_tone: bool,
    pub source_sun: bool,
    pub source_sun_colour: bool,
    pub saved_sun_colour: Option<[f32; 3]>,
    pub saved_sun: Option<grid_formats::track_lighting::Sun>,
    /// Baseline track scenes: the original's back/ambient fill (row 109).
    pub baseline_fill: Option<Fill>,
    /// `ambientOcclusionScales.x`: lighting.xml `sao_scale` (0x72BB40 reads it,
    /// 0x730F90 packs it into lane x), the strength of the track surfaces'
    /// macro AO, `ao = 1 + scale (macro.a - 1)` (`track_infield.cg.fp`).
    pub surface_ao_scale: f32,
    /// Baseline track scenes: the original road/infield specular (track_surface.rs).
    pub baseline_specular: Option<Phong>,
    /// Bevy radiance per original light unit for object emissive maps
    /// (object_materials::configure_emissive): the baseline sun's lux over the
    /// route's `backlight_suncolour` peak, over pi. None: sun lux over pi.
    pub emissive_unit: Option<f32>,
    pub preset: Preset,
    pub width: u32,
    pub height: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            adaptive_wheel_exposure: false,
            wheel_filter_floor: 0.5,
            wheel_max_arc: None,
            wheel_geometry_shutter_seconds: 0.,
            wheel_shutter_seconds: 0.,
            smooth_wheel_motion: true,
            anti_aliasing: 1,
            reflection_mode: 0,
            car_reflections: crate::car_reflections::Level::from_env().unwrap_or_default(),
            probe_size: 1024,
            moving_probe: false,
            ssr_thickness: 0.05,
            layered_track: true,
            surface_specular: super::track_surface::Specular::Off,
            surface_view_mip_bias: false,
            asphalt_soften: 0.,
            asphalt_filter_strength: 1.,
            asphalt_roughness: None,
            asphalt_grazing_gain: None,
            motion_shutter: 0.,
            dlaa_reset_every_frame: false,
            thin_motion_antenna: false,
            ground_edge_pilot: false,
            source_land: true,
            source_trees: true,
            player_race_number: None,
            source_race_numbers: false,
            source_body_occlusion: false,
            source_body_specular: false,
            white_body_specular: false,
            exposure_ev100: 13.,
            post_saturation_override: None,
            tone_map_override: None,
            window_roughness: 0.08,
            window_reflectance: 0.5,
            window_pilot: super::window_response::Mode::Off,
            object_coverage: super::object_materials::CoverageMode::Off,
            double_fence_wire: false,
            paint_clearcoat: 0.35,
            paint_environment: 0.,
            lamp_reflectors: super::lamp_materials::Mode::Off,
            lamp_lenses: super::lamp_materials::Mode::Off,
            lamp_signals: super::lamp_signals::Mode::Off,
            paint_coat_roughness: 0.18,
            tree_sheet_colours: false,
            ambient_occlusion: true,
            grass_receive_shadows: true,
            grass_soft_alpha: true,
            shadow_distance_metres: 250.,
            shadow_quality: super::shadow_layout::Quality::High,
            foliage_coverage: true,
            clearcoat_prepass_fix: true,
            tread_normals: true,
            layered_car: true,
            source_ibl: false,
            smooth_tyres: true,
            track_grade: false,
            saved_grade: None,
            saved_fog: None,
            benchmark_frames: None,
            capture_timeout_seconds: None,
            source_sky: true,
            source_sky_tone: true,
            source_sun: false,
            source_sun_colour: false,
            saved_sun_colour: None,
            saved_sun: None,
            baseline_fill: None,
            surface_ao_scale: 1.,
            baseline_specular: None,
            emissive_unit: None,
            preset: Preset::Baseline,
            width: 1280,
            height: 800,
        }
    }
}

/// The original track shader's light besides the sun [V static,
/// `track_surface_simple.cg.fp`]: `back * (1 - N.L) / 2 + ambient`, from
/// `sao_scale` of a lighting.xml without one (0x726A50 stores 0.9 at +0x298).
pub const SAO_SCALE_DEFAULT: f32 = 0.9;

/// What Bevy's ambient light gives a diffuse surface per unit of albedo,
/// colour and brightness. pbr_ambient.wgsl lights it with
/// `EnvBRDFApprox(albedo, F_AB(1, N.V))`; without Bevy's `dfg_lut` feature
/// (this build enables none), `F_AB` at roughness 1 is (0.4524, 0.00005) for
/// every N.V (`bevy_f_ab`). The fills divide by it, so that a shadowed
/// surface gets the `albedo * (ambient + back / 2)` they mean (row 125).
pub const BEVY_DIFFUSE_AMBIENT: f32 = 0.4524;

/// lighting.xml's `backlight_backcolour` / `backlight_ambientcolour`, scaled
/// against `backlight_suncolour`'s peak = the baseline sun's 18,000 lux.
/// AUTHORED: as Bevy lights, `ambient + back / 2` is a uniform ambient and
/// `back / 2` a shadowless light from opposite the sun (the original's term
/// also dims the lit side by up to back / 2; not reproduced). `gamma`
/// decodes the saved colours with 2.2 first (the original's shading space
/// is unverified).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fill {
    pub ambient: [f32; 3],
    pub ambient_brightness: f32,
    pub back: [f32; 3],
    pub back_lux: f32,
}

impl Fill {
    pub fn from_lighting(
        lighting: &grid_formats::track_environment::Lighting,
        sun_lux: f32,
        gamma: bool,
    ) -> Option<Self> {
        let decode = |rgb: [f32; 3]| if gamma { rgb.map(|c| c.max(0.).powf(2.2)) } else { rgb };
        let sun = decode(lighting.rgb("backlight_suncolour")?);
        let back = decode(lighting.rgb("backlight_backcolour")?);
        let ambient = decode(lighting.rgb("backlight_ambientcolour")?);
        let unit = sun_lux / sun.into_iter().fold(1e-3_f32, f32::max);
        let fill: [f32; 3] = std::array::from_fn(|i| ambient[i] + back[i] / 2.);
        let peak = |c: [f32; 3]| c.into_iter().fold(1e-6_f32, f32::max);
        // Bevy: a directional light gives albedo/pi * lux * N.L; the ambient
        // gives BEVY_DIFFUSE_AMBIENT * albedo * colour * brightness.
        Some(Self {
            ambient: fill.map(|c| c / peak(fill)),
            ambient_brightness: peak(fill) * unit / std::f32::consts::PI / BEVY_DIFFUSE_AMBIENT,
            back: back.map(|c| c / peak(back)),
            back_lux: peak(back) / 2. * unit,
        })
    }
}

/// The road and infield programs' direct specular light
/// (track_surface_simple.cg.fp and siblings): `specularColour` from
/// selected linked packed SUN or unlinked packed `specular_colour`, in Fill units (against
/// `backlight_suncolour`'s peak = the baseline sun's lux), and the
/// `specularDirection` it reflects toward: the sun's unless `spec_linked`
/// is 0, then `spec_heightangle`/`spec_rotationangle` (`spec_linked` absent:
/// source726A50 initializes linked=1). Colour linking also copies packedSUN.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Phong {
    /// Radiance per unit of the source lobe, before exposure.
    pub colour: [f32; 3],
    pub direction_to_light: [f32; 3],
}

/// Bevy radiance per original light unit, before exposure: `sun_lux` over
/// `backlight_suncolour`'s peak, over pi (as Fill and Phong scale).
pub fn light_unit(lighting: &grid_formats::track_environment::Lighting, sun_lux: f32, gamma: bool) -> Option<f32> {
    let sun = lighting.rgb("backlight_suncolour")?;
    let sun = if gamma { sun.map(|c| c.max(0.).powf(2.2)) } else { sun };
    Some(sun_lux / sun.into_iter().fold(1e-3_f32, f32::max) / std::f32::consts::PI)
}

impl Phong {
    pub fn from_lighting(
        lighting: &grid_formats::track_environment::Lighting,
        sun_lux: f32,
        gamma: bool,
    ) -> Option<Self> {
        let decode = |rgb: [f32; 3]| if gamma { rgb.map(|c| c.max(0.).powf(2.2)) } else { rgb };
        let sun = decode(lighting.rgb("backlight_suncolour")?);
        let source=crate::effective_lighting::SpecularColour::from_static(lighting).ok()?;
        let packed_sun=crate::effective_lighting::PackedFamily::from_static(lighting.rgb("backlight_suncolour")?).ok()?;
        // Source packs/selects first. Gamma is an EXPLICIT authored adaptation
        // AFTER source byte reconstruction; calibration's historical rawSun
        // decode/order stays unchanged. No source GPU colour-domain claim.
        let specular=decode(source.rgb(packed_sun));
        let unit = sun_lux / sun.into_iter().fold(1e-3_f32, f32::max);
        let [height_degrees, rotation_degrees] = if source.linked() {
            lighting.sun()?
        } else {
            [lighting.scalar("spec_heightangle")?, lighting.scalar("spec_rotationangle")?]
        };
        let sun = grid_formats::track_lighting::Sun { height_degrees, rotation_degrees };
        // The source adds the lobe to albedo * light; Bevy's lambert divides by pi.
        Some(Self {
            colour: specular.map(|c| c.max(0.) * unit / std::f32::consts::PI),
            direction_to_light: sun.direction_to_light(),
        })
    }
}

impl Settings {
    pub fn tone_map(&mut self, value: &str) -> Result<(), &'static str> {
        let mode = match value {
            "agx" => Tonemapping::AgX,
            "reinhard" => Tonemapping::Reinhard,
            _ => return Err("--tone-map needs agx or reinhard"),
        };
        self.tone_map_override = Some(mode);
        Ok(())
    }

    pub fn post_saturation(&mut self, value: &str) -> Result<(), &'static str> {
        let value = value.parse::<f32>().map_err(|_| "post saturation must be finite 0..2")?;
        if !value.is_finite() || !(0.0..=2.0).contains(&value) {
            return Err("post saturation must be finite 0..2");
        }
        self.post_saturation_override = Some(value);
        Ok(())
    }

    /// Actual scene context: a bounded supervisor still owns the ordinary GPU window.
    pub fn validate_display_context(&self, driving: bool, capture: bool, bounded: bool, reference: bool, collision: bool, headless: bool) -> Result<(), &'static str> {
        let rendered = capture && if driving { bounded } else { reference };
        let windowed_driving = driving && !capture && !headless;
        self.validate_display_scene(rendered, windowed_driving, collision, headless)
    }

    pub fn validate_display_scene(&self, rendered: bool, manual: bool, collision: bool, headless: bool) -> Result<(), &'static str> {
        let explicit = self.tone_map_override.is_some() || self.post_saturation_override.is_some();
        if explicit && (self.track_grade || collision || headless || !(rendered || manual)) {
            return Err("display overrides require rendered reference/bounded driving or ordinary interactive driving; no track grade, collision or headless context");
        }
        Ok(())
    }

    /// Resolve persisted preferences only after parsing CLI/context. Existing fields are CLI authority.
    /// Controlled captures/tools retain preset defaults unless CLI explicitly requested display changes.
    pub fn display_preferences(&mut self, profile: &crate::settings::Settings, ordinary: bool) -> Result<(), &'static str> {
        let tone_from_preferences = ordinary && self.tone_map_override.is_none() && profile.tone_map.is_some();
        let saturation_from_preferences = ordinary && self.post_saturation_override.is_none() && profile.post_saturation.is_some();
        if tone_from_preferences && let Some(value) = &profile.tone_map { self.tone_map(value)?; }
        if saturation_from_preferences && let Some(value) = profile.post_saturation { self.post_saturation(&value.to_string())?; }
        println!("graphics_display_preferences ordinary={} tone_from_preferences={} saturation_from_preferences={} controlled_context_preferences_ignored={}", ordinary, tone_from_preferences, saturation_from_preferences,
            !ordinary && (profile.tone_map.is_some() || profile.post_saturation.is_some()));
        Ok(())
    }

    pub fn race_number(&mut self, value: &str) -> Result<(), &'static str> {
        let number = grid_formats::car_decals::Number::parse(value).map_err(|_| "race number needs 1..3 decimal digits")?;
        self.player_race_number = Some(number);
        Ok(())
    }
    pub fn glass_reflectance(&mut self, value: &str) -> Result<(), &'static str> {
        let reflectance = value.parse::<f32>().map_err(|_| "invalid glass reflectance")?;
        if !reflectance.is_finite() || !(0.0..=1.0).contains(&reflectance) {
            return Err("glass reflectance must be finite and 0..1");
        }
        self.window_reflectance = reflectance;
        Ok(())
    }

    pub fn glass_roughness(&mut self, value: &str) -> Result<(), &'static str> {
        let roughness = value.parse::<f32>().map_err(|_| "invalid glass roughness")?;
        if !roughness.is_finite() || !(0.04..=1.0).contains(&roughness) {
            return Err("glass roughness must be finite and 0.04..1");
        }
        self.window_roughness = roughness;
        Ok(())
    }

    pub fn paint_clearcoat(&mut self, value: &str) -> Result<(), &'static str> {
        let strength = value.parse::<f32>().map_err(|_| "invalid paint clearcoat")?;
        if !strength.is_finite() || !(0.0..=1.0).contains(&strength) {
            return Err("paint clearcoat must be finite and 0..1");
        }
        self.paint_clearcoat = strength;
        Ok(())
    }

    pub fn paint_coat_roughness(&mut self, value: &str) -> Result<(), &'static str> {
        let roughness = value.parse::<f32>().map_err(|_| "invalid paint coat roughness")?;
        if !roughness.is_finite() || !(0.04..=1.0).contains(&roughness) {
            return Err("paint coat roughness must be finite and 0.04..1");
        }
        self.paint_coat_roughness = roughness;
        Ok(())
    }

    pub fn exposure(&mut self, value: &str) -> Result<(), &'static str> {
        let ev = value.parse::<f32>().map_err(|_| "invalid exposure EV100")?;
        if !ev.is_finite() || !(6.0..=20.0).contains(&ev) {
            return Err("exposure must be finite and 6..20 EV100");
        }
        self.exposure_ev100 = ev;
        Ok(())
    }

    pub fn resolution(&mut self, value: &str) -> Result<(), &'static str> {
        let (width, height) = value
            .split_once('x')
            .ok_or("--resolution needs WIDTHxHEIGHT")?;
        let width = width
            .parse::<u32>()
            .map_err(|_| "invalid resolution width")?;
        let height = height
            .parse::<u32>()
            .map_err(|_| "invalid resolution height")?;
        // Bound GPU allocation while allowing native UHD and ultrawide comparisons.
        if !(320..=7680).contains(&width)
            || !(200..=4320).contains(&height)
            || u64::from(width) * u64::from(height) > 33_177_600
        {
            return Err("resolution must be 320..7680 x 200..4320, at most 33,177,600 pixels");
        }
        self.width = width;
        self.height = height;
        Ok(())
    }

    pub fn shadow_distance(&mut self, value: &str) -> Result<(), &'static str> {
        let distance = value.parse::<f32>().map_err(|_| "invalid shadow distance")?;
        if !distance.is_finite() || !(50.0..=2000.0).contains(&distance) {
            return Err("shadow distance must be finite and 50..2000 metres");
        }
        self.shadow_distance_metres = distance;
        Ok(())
    }

    pub fn capture_timeout(&mut self, value: &str) -> Result<(), &'static str> {
        let seconds = value.parse::<f64>().map_err(|_| "invalid capture timeout")?;
        if !seconds.is_finite() || !(30.0..=3600.0).contains(&seconds) {
            return Err("capture timeout must be finite and 30..3600 seconds");
        }
        self.capture_timeout_seconds = Some(seconds);
        Ok(())
    }

    pub fn enhanced(&self) -> bool {
        self.preset == Preset::Enhanced
    }

    pub fn sampler(&self, repeat: bool) -> ImageSampler {
        if !repeat && !self.enhanced() {
            return ImageSampler::Default;
        }
        let mut sampler = ImageSamplerDescriptor::linear();
        if repeat {
            sampler.address_mode_u = ImageAddressMode::Repeat;
            sampler.address_mode_v = ImageAddressMode::Repeat;
        }
        sampler.anisotropy_clamp = if self.enhanced() { 16 } else { 1 };
        ImageSampler::Descriptor(sampler)
    }

    pub fn sun_transform(&self) -> Transform {
        // Saved only for track scenes: --sun source (enhanced) or the
        // baseline's lighting.xml direction.
        if let Some(sun) = self.saved_sun {
            let direction = Vec3::from_array(sun.direction_to_light()).normalize();
            let up = if direction.dot(Vec3::Y).abs() > 0.9999 {
                Vec3::Z
            } else {
                Vec3::Y
            };
            return Transform::from_translation(direction).looking_at(Vec3::ZERO, up);
        }
        Transform::from_xyz(-4., 8., 5.).looking_at(Vec3::ZERO, Vec3::Y)
    }

    pub fn sun_colour(&self) -> Color {
        if self.enhanced() && let Some(rgb) = self.saved_sun_colour {
            return Color::linear_rgb(rgb[0], rgb[1], rgb[2]);
        }
        Color::WHITE
    }

    pub fn sun_illuminance(&self) -> f32 {
        if self.enhanced() {
            lux::RAW_SUNLIGHT
        } else {
            18_000.
        }
    }

    /// The original layout's world reach: 252 m (cascade 2's 250 m plus its
    /// 2 m extension); the enhanced profile's shadow distance plus 2 m.
    pub fn shadow_layout_far(&self) -> f32 {
        if self.enhanced() {
            self.shadow_distance_metres + 2.
        } else {
            super::shadow_layout::WORLD_FAR[1]
        }
    }

    pub fn cascades(&self) -> CascadeShadowConfig {
        if self.enhanced() {
            CascadeShadowConfigBuilder {
                num_cascades: 4,
                first_cascade_far_bound: 18.,
                maximum_distance: self.shadow_distance_metres,
                overlap_proportion: 0.2,
                ..default()
            }
            .build()
        } else {
            CascadeShadowConfig::default()
        }
    }
}

pub fn configure(
    commands: &mut Commands,
    camera: Entity,
    settings: &Settings,
    media: &mut Assets<ScatteringMedium>,
) {
    let tone_map = settings.tone_map_override.unwrap_or(Tonemapping::AgX);
    println!(
        "graphics preset={:?} resolution={}x{} shadow_size={} anisotropy={} sky_ibl={} ssao={} taa={} tonemapping={}",
        settings.preset,
        settings.width,
        settings.height,
        if settings.enhanced() { 4096 } else { 2048 },
        if settings.enhanced() { 16 } else { 1 },
        settings.enhanced(),
        settings.enhanced() && settings.ambient_occlusion,
        settings.enhanced() && settings.anti_aliasing == 0,
        if settings.enhanced() {
            format!("{tone_map:?}")
        } else {
            "retained-or-explicit-see-graphics_display".to_owned()
        }
    );
    if !settings.enhanced() {
        if settings.anti_aliasing == 3 {
            // DLAA needs single-sample HDR colour and its own depth/motion inputs;
            // baseline lighting, exposure and display grading stay on their path.
            commands.entity(camera).insert((
                Hdr,
                Msaa::Off,
                bevy::core_pipeline::prepass::DepthPrepass,
                bevy::core_pipeline::prepass::MotionVectorPrepass,
            ));
            println!("graphics_aa mode=3 hdr=true msaa=off depth_prepass=true motion_prepass=true scope=main-view baseline_display=preserved");
        }
        configure_display(commands, camera, settings);
        return;
    }
    if let Some(fog) = settings.saved_fog {
        commands.entity(camera).insert(DistanceFog {
            // Bevy applies this after lighting/exposure and before HDR tonemapping.
            // Treating saved RGB as linear is an explicit pilot assumption.
            color: Color::linear_rgba(fog.colour[0] * fog.brightness,
                fog.colour[1] * fog.brightness, fog.colour[2] * fog.brightness, fog.density_cap),
            directional_light_color: Color::NONE,
            falloff: FogFalloff::Linear { start: 30., end: 30. + fog.distance },
            ..default()
        });
        println!("source_haze enabled=true distance={} density_cap={} colour={:?} brightness={} height_bounds=[{},{}] mapping=source-distance-shape-authored-uniforms-linear30 height_applied=false colour_space=assumed-linear source_sky=unchanged",
            fog.distance, fog.density_cap, fog.colour, fog.brightness, fog.base_height, fog.top_height);
    }
    if !super::shadow_layout::enabled() {
        println!("graphics_shadow_range metres={} cascades=4 first=18 map_size=4096 mapping=authored", settings.shadow_distance_metres);
    }
    println!("graphics_exposure ev100={} mode=fixed tonemapping={tone_map:?}", settings.exposure_ev100);
    println!("graphics_tone_map value={tone_map:?} override={} scope=main-view-authored-display-diagnostic exposure_grade_materials_probe_hdr=unchanged", settings.tone_map_override.is_some());
    println!("graphics_post_saturation value={} override={} scope=authored-camera-grade temperature_tint=unchanged", settings.post_saturation_override.unwrap_or(if settings.track_grade { 1.0 } else { 0.9 }), settings.post_saturation_override.is_some());
    if !super::shadow_layout::enabled() {
        commands.insert_resource(DirectionalLightShadowMap { size: 4096 });
    }
    commands.insert_resource(GlobalAmbientLight::NONE);
    commands.spawn(Atmosphere::earth(
        media.add(ScatteringMedium::earth(256, 256)),
    ));
    commands.entity(camera).insert((
        Hdr,
        Exposure { ev100: settings.exposure_ev100 },
        tone_map,
        // Authored restrained warm grade; original GRID's LUT is not recovered.
        ColorGrading {
            global: ColorGradingGlobal {
                temperature: if settings.track_grade { 0.0 } else { 0.012 },
                tint: if settings.track_grade { 0.0 } else { -0.003 },
                post_saturation: settings.post_saturation_override.unwrap_or(if settings.track_grade { 1.0 } else { 0.9 }),
                ..default()
            },
            ..default()
        },
        Bloom {
            intensity: 0.04,
            ..Bloom::NATURAL
        },
        AtmosphereSettings::default(),
        AtmosphereEnvironmentMapLight::default(),
        Msaa::Off,
        bevy::core_pipeline::prepass::MotionVectorPrepass,
        ShadowFilteringMethod::Gaussian,
    ));
    if settings.motion_shutter > 0. {
        commands.entity(camera).insert(MotionBlur {
            shutter_angle: settings.motion_shutter,
            samples: 8,
        });
    }
    println!("graphics_motion_blur enabled={} shutter_angle={} samples=8 units=fraction-of-rendered-frame scope=main-view default=off", settings.motion_shutter > 0., settings.motion_shutter);
    if settings.ambient_occlusion {
        commands.entity(camera).insert(ScreenSpaceAmbientOcclusion::default());
    }
    match settings.anti_aliasing {
        0 => {
            commands
                .entity(camera)
                .insert(TemporalAntiAliasing::default());
        }
        1 => {
            commands.entity(camera).insert(Smaa {
                preset: SmaaPreset::Ultra,
            });
        }
        _ => {}
    }
    println!(
        "graphics_aa mode={} motion_prepass=true scope=main-view",
        settings.anti_aliasing
    );
    println!(
        "graphics_wheel_motion smooth={} prediction_bound_seconds=0.016 physics_feedback=false",
        settings.smooth_wheel_motion
    );
    if matches!(settings.reflection_mode, 1 | 2) {
        commands
            .entity(camera)
            .insert(bevy::core_pipeline::prepass::DeferredPrepass);
    }
    if settings.reflection_mode == 2 {
        commands.entity(camera).insert(ScreenSpaceReflections {
            linear_steps: 32,
            bisection_steps: 8,
            edge_fadeout: 0.02..0.12,
            thickness: settings.ssr_thickness,
            ..default()
        });
    }
    println!(
        "reflections mode={} scope={} glass=forward",
        settings.reflection_mode,
        if settings.reflection_mode == 2 {
            "screen-visible-opaque-only"
        } else if settings.reflection_mode == 3 {
            if settings.moving_probe {
                "moving-local-HDR-probe"
            } else {
                "static-local-HDR-probe"
            }
        } else {
            "sky-IBL"
        }
    );
    if settings.reflection_mode == 2 {
        println!(
            "ssr thickness={} linear_steps=32 bisection_steps=8",
            settings.ssr_thickness
        );
    }
    configure_display(commands, camera, settings);
    if let Some(grade) = settings.saved_grade {
        commands
            .entity(camera)
            .insert(super::track_grade::TrackGrade::from(grade));
    }
}


/// Shared admission for persisted playable preferences, deliberately separate from explicit CLI admission.
pub fn ordinary_display_context(driving: bool, capture: bool, scripted: bool, headless: bool, collision: bool, research: bool) -> bool {
    driving && !capture && !scripted && !headless && !collision && !research
}

/// Mutate only the requested baseline display fields, after pending camera setup.
/// No HDR/default change when neither option is present. Enhanced keeps its authored setup.
fn configure_display(commands: &mut Commands, camera: Entity, settings: &Settings) {
    let baseline = !settings.enhanced();
    let tone = settings.tone_map_override;
    let saturation = settings.post_saturation_override;
    commands.queue(move |world: &mut World| {
        let mut view = world.entity_mut(camera);
        if baseline && (tone.is_some() || saturation.is_some()) {
            view.insert(Hdr);
            if let Some(tone) = tone { view.insert(tone); }
            if let Some(value) = saturation {
                if let Some(mut grade) = view.get_mut::<ColorGrading>() {
                    grade.global.post_saturation = value;
                } else {
                    view.insert(ColorGrading {
                        global: ColorGradingGlobal { post_saturation: value, ..default() },
                        ..default()
                    });
                }
            }
        }
        println!("graphics_display scope=main-view baseline={} hdr={} tone={:?} post_saturation={:?} exposure_ev100={:?} msaa={:?} tone_override={} saturation_override={} lighting=unchanged authored=true",
            baseline, view.contains::<Hdr>(), view.get::<Tonemapping>(),
            view.get::<ColorGrading>().map(|g|g.global.post_saturation),
            view.get::<Exposure>().map(|e|e.ev100), view.get::<Msaa>(), tone.is_some(), saturation.is_some());
    });
}


#[cfg(test)]
mod graphics_settings_tests {
    use super::*;

    /// Bevy 0.19 pbr_lighting.wgsl `F_AB` without `dfg_lut`.
    fn bevy_f_ab(perceptual_roughness: f32, n_dot_v: f32) -> [f32; 2] {
        let c0 = [-1.0, -0.0275, -0.572, 0.022];
        let c1 = [1.0, 0.0425, 1.04, -0.04];
        let r: [f32; 4] = std::array::from_fn(|i| perceptual_roughness * c0[i] + c1[i]);
        let a004 = (r[0] * r[0]).min((-9.28 * n_dot_v).exp2()) * r[0] + r[1];
        [(-1.04 * a004 + r[2]).max(0.00005), (1.04 * a004 + r[3]).max(0.00005)]
    }

    #[test]
    fn fill_ambient_compensates_bevy_diffuse_ambient_response() {
        // A diffuse surface's ambient is albedo * F_AB.x + F_AB.y, the same at every N.V.
        for i in 0..=20 {
            let [scale, bias] = bevy_f_ab(1.0, i as f32 / 20.);
            assert!((scale - BEVY_DIFFUSE_AMBIENT).abs() < 1e-4, "{scale}");
            assert!(bias <= 1e-4, "{bias}");
        }
        let mut lighting = grid_formats::track_environment::Lighting::default();
        for (key, value) in [
            ("backlight_suncolour", "1 1 1"),
            ("backlight_backcolour", "0.2 0.2 0.2"),
            ("backlight_ambientcolour", "0.1 0.1 0.1"),
        ] {
            lighting.params.insert(key.into(), value.into());
        }
        let fill = Fill::from_lighting(&lighting, 18_000., false).unwrap();
        // Shadowed albedo-1 surface: Bevy's ambient response times the brightness
        // is the original's (ambient + back / 2) in the sun's albedo/pi * lux units.
        let shadowed = BEVY_DIFFUSE_AMBIENT * fill.ambient_brightness;
        let original = (0.1 + 0.2 / 2.) * 18_000. / std::f32::consts::PI;
        assert!((shadowed / original - 1.).abs() < 1e-5, "{shadowed} {original}");
    }

    #[test]
    fn baseline_dlaa_prepares_only_main_view_without_enhanced_display_or_lighting() {
        for aa in [1, 3] {
            let mut world = World::new();
            world.insert_resource(GlobalAmbientLight {
                brightness: 321.,
                ..default()
            });
            world.insert_resource(DirectionalLightShadowMap { size: 1024 });
            let camera = world.spawn((
                Camera3d::default(),
                Exposure { ev100: 9.5 },
                Tonemapping::Reinhard,
                ColorGrading {
                    global: ColorGradingGlobal { post_saturation: 0.73, ..default() },
                    ..default()
                },
                Msaa::Sample4,
            )).id();
            let other = world.spawn((Camera3d::default(), Msaa::Sample4)).id();
            let settings = Settings { anti_aliasing: aa, ..default() };
            let mut media = Assets::<ScatteringMedium>::default();
            let mut queue = bevy::ecs::world::CommandQueue::default();
            {
                let mut commands = Commands::new(&mut queue, &world);
                configure(&mut commands, camera, &settings, &mut media);
            }
            queue.apply(&mut world);
            let view = world.entity(camera);
            assert_eq!(view.get::<Exposure>().unwrap().ev100, 9.5);
            assert_eq!(*view.get::<Tonemapping>().unwrap(), Tonemapping::Reinhard);
            assert_eq!(view.contains::<Hdr>(), aa == 3);
            assert_eq!(view.contains::<bevy::core_pipeline::prepass::DepthPrepass>(), aa == 3);
            assert_eq!(view.contains::<bevy::core_pipeline::prepass::MotionVectorPrepass>(), aa == 3);
            assert_eq!(*view.get::<Msaa>().unwrap(), if aa == 3 { Msaa::Off } else { Msaa::Sample4 });
            assert!(!view.contains::<Bloom>());
            assert_eq!(view.get::<ColorGrading>().unwrap().global.post_saturation, 0.73);
            assert!(!view.contains::<ScreenSpaceAmbientOcclusion>());
            assert!(!view.contains::<AtmosphereSettings>());
            assert!(!world.entity(other).contains::<Hdr>());
            assert_eq!(*world.entity(other).get::<Msaa>().unwrap(), Msaa::Sample4);
            assert_eq!(world.resource::<GlobalAmbientLight>().brightness, 321.);
            assert_eq!(world.resource::<DirectionalLightShadowMap>().size, 1024);
            assert!(media.is_empty());
        }
    }

    #[test]
    fn timeout_extension_is_bounded_finite_and_failed_edits_preserve_the_value() {
        let mut settings = Settings::default();
        assert_eq!(settings.capture_timeout_seconds, None);
        for value in ["30", "900", "3600"] {
            settings.capture_timeout(value).unwrap();
        }
        for value in ["", "NaN", "inf", "0", "29.99", "-1", "3600.1"] {
            assert!(settings.capture_timeout(value).is_err());
            assert_eq!(settings.capture_timeout_seconds, Some(3600.));
        }
    }
    #[test]
    fn race_number_is_explicit_bounded_and_preserved_after_invalid_input() {
        let mut settings = Settings::default();
        assert_eq!(settings.player_race_number, None);
        settings.race_number("70").unwrap();
        for value in ["", "-1", "+1", "1000", "1.5", "a", " 1"] {
            assert!(settings.race_number(value).is_err());
            assert_eq!(settings.player_race_number, Some(grid_formats::car_decals::Number::parse("70").unwrap()));
        }
        settings.race_number("01").unwrap();
        assert_eq!(settings.player_race_number.unwrap().as_str(), "01");
        settings.race_number("0").unwrap();
        assert_eq!(settings.player_race_number, Some(grid_formats::car_decals::Number::parse("0").unwrap()));
    }
    #[test]
    fn glass_response_control_is_bounded_without_changing_default() {
        let mut settings = Settings::default();
        assert_eq!(settings.window_roughness, 0.08);
        assert_eq!(settings.window_reflectance, 0.5);
        settings.glass_reflectance("0.25").unwrap();
        for value in ["", "NaN", "inf", "-0.01", "1.001"] {
            assert!(settings.glass_reflectance(value).is_err());
            assert_eq!(settings.window_reflectance, 0.25);
        }
        settings.glass_roughness("0.2").unwrap();
        for value in ["", "NaN", "inf", "0", "0.039", "1.001"] {
            assert!(settings.glass_roughness(value).is_err());
            assert_eq!(settings.window_roughness, 0.2);
        }
    }
    #[test]
    fn paint_coat_controls_preserve_defaults_and_reject_nonfinite_values() {
        let mut settings = Settings::default();
        assert_eq!((settings.paint_clearcoat, settings.paint_coat_roughness), (0.35, 0.18));
        for value in ["0", "0.35", "1"] {
            settings.paint_clearcoat(value).unwrap();
        }
        for value in ["0.04", "0.12", "1"] {
            settings.paint_coat_roughness(value).unwrap();
        }
        for value in ["", "NaN", "inf", "-inf", "-0.01", "1.01"] {
            assert!(settings.paint_clearcoat(value).is_err());
            assert_eq!(settings.paint_clearcoat, 1.);
        }
        for value in ["", "NaN", "inf", "-inf", "0", "0.039", "1.01"] {
            assert!(settings.paint_coat_roughness(value).is_err());
            assert_eq!(settings.paint_coat_roughness, 1.);
        }
        assert_eq!((settings.window_reflectance, settings.window_roughness), (0.5, 0.08));
    }

    #[test]
    fn exposure_is_bounded_and_invalid_values_preserve_previous_setting() {
        let mut settings = Settings::default();
        assert_eq!(settings.exposure_ev100, 13.);
        for value in ["6", "13.5", "20"] {
            settings.exposure(value).unwrap();
        }
        for value in ["", "NaN", "inf", "-inf", "5.99", "20.01"] {
            assert!(settings.exposure(value).is_err());
            assert_eq!(settings.exposure_ev100, 20.);
        }
    }
    #[test]
    fn source_sun_emits_away_from_the_light_and_handles_vertical_poles() {
        let mut settings = Settings {
            preset: Preset::Enhanced,
            ..Default::default()
        };
        let authored = settings.sun_transform();
        for height in [-90., 0., 37., 90.] {
            let sun = grid_formats::track_lighting::Sun {
                height_degrees: height,
                rotation_degrees: 123.,
            };
            settings.saved_sun = Some(sun);
            let transform = settings.sun_transform();
            assert!(transform.rotation.is_finite());
            assert!(
                Vec3::from(transform.forward())
                    .distance(-Vec3::from_array(sun.direction_to_light()))
                    < 0.000001
            );
        }
        settings.saved_sun = None;
        settings.preset = Preset::Baseline;
        assert_eq!(settings.sun_transform(), authored);
    }
    #[test]
    fn source_sun_colour_keeps_baseline_white_and_lux_unchanged() {
        let mut settings = Settings { preset: Preset::Enhanced, ..Default::default() };
        let lux = settings.sun_illuminance();
        assert_eq!(settings.sun_colour(), Color::WHITE);
        settings.saved_sun_colour = Some([1., 0.5, 0.25]);
        assert_eq!(settings.sun_colour(), Color::linear_rgb(1., 0.5, 0.25));
        assert_eq!(settings.sun_illuminance(), lux);
        settings.preset = Preset::Baseline;
        assert_eq!(settings.sun_colour(), Color::WHITE);
    }
    #[test]
    fn shadow_distance_reaches_requested_bound_and_rejects_invalid_limits() {
        let mut settings = Settings { preset: Preset::Enhanced, ..Default::default() };
        let original = settings.cascades().bounds;
        assert!((*original.last().unwrap() - 250.).abs() < 0.001);
        settings.shadow_distance("1000").unwrap();
        let bounds = settings.cascades().bounds;
        assert_eq!(bounds.len(), 4);
        assert!((bounds[0] - 18.).abs() < 0.001);
        assert!((*bounds.last().unwrap() - 1000.).abs() < 0.001);
        assert!(bounds.windows(2).all(|w| w[0].is_finite() && w[0] < w[1]));
        for value in ["NaN", "inf", "0", "49.9", "2000.1", ""] {
            assert!(settings.shadow_distance(value).is_err());
            assert_eq!(settings.shadow_distance_metres, 1000.);
        }
        settings.preset = Preset::Baseline;
        assert_eq!(settings.cascades().bounds, CascadeShadowConfig::default().bounds);
        settings.preset = Preset::Enhanced;
        settings.shadow_distance("250").unwrap();
        assert_eq!(settings.cascades().bounds, original);
    }

    #[test]
    fn phong_takes_the_sun_unless_unlinked_in_fill_units() {
        let xml = |linked: &str| format!(r#"<lighting><param heightangle="90" /><param rotationangle="0" />
<param spec_linked="{linked}" /><param spec_heightangle="0" /><param spec_rotationangle="90" />
<param specular_colour="1 0.5 0" /><param backlight_suncolour="2 1 1" /></lighting>"#);
        let read = |linked| grid_formats::track_environment::read_lighting(xml(linked).as_bytes()).unwrap();
        let pi = std::f32::consts::PI;
        let linked = Phong::from_lighting(&read("1"), 18_000., false).unwrap();
        // One unit of the source lobe = the sun colour's peak = 18,000 lux.
        assert_eq!(linked.colour, [9_000. / pi, (127. * (1. / 255.)) * 9_000. / pi, (127. * (1. / 255.)) * 9_000. / pi]);
        assert!(linked.direction_to_light[1] > 0.9999);
        let unlinked = Phong::from_lighting(&read("0"), 18_000., false).unwrap();
        assert!(unlinked.direction_to_light[0] < -0.9999);
        assert_eq!(unlinked.colour,[9_000./pi,(127.*(1./255.))*9_000./pi,0.]);
        let gamma = Phong::from_lighting(&read("1"), 18_000., true).unwrap();
        assert!((gamma.colour[1] - (127.0_f32 * (1. / 255.)).powf(2.2) * 18_000. / 2_f32.powf(2.2) / pi).abs() < 1e-2);
    }

    #[test]
    #[ignore = "requires the purchased installation (GRID_GAME_ROOT)"]
    fn every_shipped_lighting_file_gives_a_phong_light() {
        let root = std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required"));
        let mut files = 0;
        let mut stack = vec![root.join("tracks")];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.file_name().is_some_and(|n| n.eq_ignore_ascii_case("lighting.xml"))
                    && !path.to_string_lossy().contains("frontend")
                {
                    let lighting = grid_formats::track_environment::read_lighting(&std::fs::read(&path).unwrap()).unwrap();
                    let phong = Phong::from_lighting(&lighting, 18_000., false);
                    let phong = phong.unwrap_or_else(|| panic!("{}", path.display()));
                    assert!(phong.colour.iter().all(|c| c.is_finite() && *c >= 0.), "{}", path.display());
                    files += 1;
                }
            }
        }
        assert_eq!(files, 20);
    }
}

#[cfg(test)]
mod shared_display_tests {
    use super::*;
    fn configured(aa: u8, tone: Option<&str>, saturation: Option<&str>, enhanced: bool) -> World {
        let mut world=World::new();
        world.insert_resource(GlobalAmbientLight { brightness: 321., ..default() });
        world.insert_resource(DirectionalLightShadowMap { size: 1024 });
        let camera=world.spawn((Camera3d::default(), Tonemapping::default(), Exposure {ev100:9.5}, ColorGrading {
            global:ColorGradingGlobal {temperature:0.17,tint:0.23,post_saturation:0.73,..default()},..default()
        },Msaa::Sample4)).id();
        let other=world.spawn((Camera2d, Tonemapping::Reinhard)).id();
        let mut settings=Settings {anti_aliasing:aa,preset:if enhanced {Preset::Enhanced}else{Preset::Baseline},ambient_occlusion:!enhanced,..default()};
        if let Some(t)=tone {settings.tone_map(t).unwrap();}
        if let Some(s)=saturation {settings.post_saturation(s).unwrap();}
        let mut media=Assets::<ScatteringMedium>::default();
        let mut queue=bevy::ecs::world::CommandQueue::default();
        configure(&mut Commands::new(&mut queue,&world),camera,&settings,&mut media);
        queue.apply(&mut world);
        if !enhanced { assert!(media.is_empty()); }
        assert_eq!(*world.entity(other).get::<Tonemapping>().unwrap(),Tonemapping::Reinhard);
        assert!(!world.entity(other).contains::<Hdr>());
        world.insert_resource(TestCamera(camera));
        world
    }
    #[derive(Resource)] struct TestCamera(Entity);
    #[test]
    fn baseline_actual_configure_preserves_none_and_changes_only_explicit_display_fields() {
        for aa in [1,3] { for (tone,sat) in [(None,None),(Some("reinhard"),None),(None,Some("0.9")),(Some("reinhard"),Some("0.9"))] {
            let world=configured(aa,tone,sat,false);
            let view=world.entity(world.resource::<TestCamera>().0);
            assert_eq!(*view.get::<Tonemapping>().unwrap(),if tone.is_some(){Tonemapping::Reinhard}else{Tonemapping::TonyMcMapface});
            assert_eq!(view.get::<ColorGrading>().unwrap().global.post_saturation,if sat.is_some(){0.9}else{0.73});
            assert_eq!(view.get::<ColorGrading>().unwrap().global.temperature,0.17);
            assert_eq!(view.get::<ColorGrading>().unwrap().global.tint,0.23);
            assert_eq!(view.get::<Exposure>().unwrap().ev100,9.5);
            assert_eq!(view.contains::<Hdr>(),aa==3 || tone.is_some() || sat.is_some());
            assert_eq!(*view.get::<Msaa>().unwrap(),if aa==3{Msaa::Off}else{Msaa::Sample4});
            assert_eq!(view.contains::<bevy::core_pipeline::prepass::DepthPrepass>(),aa==3);
            assert_eq!(view.contains::<bevy::core_pipeline::prepass::MotionVectorPrepass>(),aa==3);
            assert!(!view.contains::<Bloom>()); assert!(!view.contains::<AtmosphereSettings>());
            assert!(!view.contains::<ScreenSpaceAmbientOcclusion>());
            assert_eq!(world.resource::<GlobalAmbientLight>().brightness,321.);
            assert_eq!(world.resource::<DirectionalLightShadowMap>().size,1024);
        }}
    }
    #[test]
    fn windowed_scripted_display_uses_actual_camera_and_keeps_unsupported_guards() {
        for enhanced in [false, true] {
            let mut settings = Settings { preset: if enhanced { Preset::Enhanced } else { Preset::Baseline }, ..default() };
            settings.tone_map("reinhard").unwrap();
            settings.post_saturation("0.9").unwrap();
            // Both menu/career supervisors append a bound without a capture target.
            // A bound changes termination, not the actual Window rendering context.
            for bounded in [false, true] {
                settings.validate_display_context(true, false, bounded, false, false, false).unwrap();
                for aa in [1, 3] {
                    let world = configured(aa, Some("reinhard"), Some("0.9"), enhanced);
                    let view = world.entity(world.resource::<TestCamera>().0);
                    assert!(view.contains::<Hdr>());
                    assert_eq!(*view.get::<Tonemapping>().unwrap(), Tonemapping::Reinhard);
                    assert_eq!(view.get::<ColorGrading>().unwrap().global.post_saturation, 0.9);
                    assert_eq!(*view.get::<Msaa>().unwrap(), if enhanced || aa == 3 { Msaa::Off } else { Msaa::Sample4 });
                }
            }
            // Existing capture-reference/bounded-drive support stays admitted.
            settings.validate_display_context(false, true, false, true, false, false).unwrap();
            settings.validate_display_context(true, true, true, true, false, false).unwrap();
            for context in [
                (false, false, false, false, false, false), // static non-driving window
                (false, true, false, false, false, false), // non-reference capture
                (true, true, false, true, false, false),   // unbounded capture
                (true, false, true, false, true, false),   // collision remains refused
                (true, false, true, false, false, true),   // headless remains refused
            ] {
                let (drive, capture, bounded, reference, collision, headless) = context;
                assert!(settings.validate_display_context(drive, capture, bounded, reference, collision, headless).is_err());
            }
            settings.track_grade = true;
            assert!(settings.validate_display_context(true, false, true, false, false, false).is_err());
        }
    }

    #[test]
    fn enhanced_actual_configure_keeps_authored_defaults_and_explicit_overrides() {
        for explicit in [false,true] {
            let world=configured(1,explicit.then_some("reinhard"),explicit.then_some("0.81"),true);
            let view=world.entity(world.resource::<TestCamera>().0);
            assert_eq!(*view.get::<Tonemapping>().unwrap(),if explicit{Tonemapping::Reinhard}else{Tonemapping::AgX});
            assert_eq!(view.get::<ColorGrading>().unwrap().global.post_saturation,if explicit{0.81}else{0.9});
            assert_eq!(view.get::<ColorGrading>().unwrap().global.temperature,0.012);
            assert_eq!(view.get::<ColorGrading>().unwrap().global.tint,-0.003);
            assert_eq!(view.get::<Exposure>().unwrap().ev100,Settings::default().exposure_ev100);
            assert!(view.contains::<Hdr>() && view.contains::<Bloom>());
            assert_eq!(*view.get::<Msaa>().unwrap(),Msaa::Off);
        }
    }
}

#[cfg(test)]
mod shared_display_admission_tests {
    use super::*;
    #[test]
    fn consumed_scene_guard_admits_both_presets_and_keeps_diagnostic_isolation() {
        for preset in [Preset::Baseline,Preset::Enhanced] {
            let mut settings=Settings {preset,..default()};
            assert!(settings.validate_display_scene(false,false,true,true).is_ok());
            settings.tone_map("reinhard").unwrap(); settings.post_saturation("0.9").unwrap();
            assert!(settings.validate_display_scene(true,false,false,false).is_ok());
            assert!(settings.validate_display_scene(false,true,false,false).is_ok());
            for (rendered,manual,collision,headless) in [(false,false,false,false),(true,false,true,false),(true,false,false,true)] {
                assert!(settings.validate_display_scene(rendered,manual,collision,headless).is_err());
            }
            settings.track_grade=true;
            assert!(settings.validate_display_scene(true,false,false,false).is_err());
        }
    }
}

#[cfg(test)]
mod shared_display_smoke_admission_tests {
    #[test]
    fn normal_display_preferences_preserve_actual_automatic_smoke_admission() {
        let mut profile=crate::settings::Settings::default();
        profile.apply("[video]\ntone_map=reinhard\npost_saturation=0.9\n",std::path::Path::new(".")).unwrap();
        let mut settings=super::Settings::default();
        settings.display_preferences(&profile, true).unwrap();
        settings.validate_display_scene(false,true,false,false).unwrap();
        for args in [vec!["--drive","--tone-map","reinhard","--post-saturation","0.9"], vec!["--drive","--aa","dlaa","--tone-map","reinhard","--post-saturation","0.9"]] {
            let explicit=args.iter().any(|arg|crate::smoke_boundary::explicit_policy_arg(arg));
            assert!(!explicit);
            assert!(crate::smoke_boundary::automatic_selected(true,true,true,true,explicit));
            let mut research=args.clone(); research.extend(["--smoke-boundary","off"]);
            assert!(!crate::smoke_boundary::automatic_selected(true,true,true,true,research.iter().any(|arg|crate::smoke_boundary::explicit_policy_arg(arg))));
            assert!(crate::smoke_boundary::explicit_policy_arg("--temporal-audit"));
            assert!(crate::smoke_boundary::explicit_policy_arg("--temporal-colour-audit"));
        }
    }
}

#[cfg(test)]
mod scoped_display_preferences_tests {
    use super::*;
    #[test]
    fn consumed_resolver_preserves_per_field_cli_authority_and_controlled_defaults() {
        let mut profile=crate::settings::Settings::default();
        profile.apply("[video]\ntone_map=reinhard\npost_saturation=0.9\n",std::path::Path::new(".")).unwrap();
        for ordinary in [false,true] { for cli_tone in [false,true] { for cli_saturation in [false,true] {
            let mut settings=Settings::default();
            if cli_tone {settings.tone_map("agx").unwrap();}
            if cli_saturation {settings.post_saturation("0.7").unwrap();}
            settings.display_preferences(&profile,ordinary).unwrap();
            assert_eq!(settings.tone_map_override,if cli_tone {Some(Tonemapping::AgX)} else if ordinary {Some(Tonemapping::Reinhard)} else {None});
            assert_eq!(settings.post_saturation_override,if cli_saturation {Some(0.7)} else if ordinary {Some(0.9)} else {None});
            if !ordinary && !cli_tone && !cli_saturation {
                // Same consumed guard as main: unrelated headless/tool/collision no longer poisoned.
                assert!(settings.validate_display_scene(false,false,true,true).is_ok());
                assert!(settings.validate_display_scene(true,false,false,false).is_ok());
            }
            if !ordinary && (cli_tone || cli_saturation) {
                assert!(settings.validate_display_scene(false,false,true,true).is_err());
                // Explicit CLI captures remain supported; preferences do not override them.
                assert!(settings.validate_display_scene(true,false,false,false).is_ok());
            }
        }}}
        for context in [(true,false,false,false,false,false), (true,true,false,false,false,false),
            (true,false,true,false,false,false), (true,false,false,true,false,false),
            (true,false,false,false,true,false), (true,false,false,false,false,true),
            (false,false,false,false,false,false)] {
            let (drive,capture,scripted,headless,collision,research)=context;
            let ordinary=ordinary_display_context(drive,capture,scripted,headless,collision,research);
            let mut resolved=Settings::default(); resolved.display_preferences(&profile,ordinary).unwrap();
            assert_eq!(resolved.tone_map_override.is_some(),context==(true,false,false,false,false,false));
            assert_eq!(resolved.post_saturation_override.is_some(),context==(true,false,false,false,false,false));
            if !ordinary { assert!(resolved.validate_display_scene(false,false,collision,headless).is_ok()); }
        }
        let mut probe=Settings::default(); probe.display_preferences(&profile,false).unwrap();
        assert!(probe.tone_map_override.is_none() && probe.post_saturation_override.is_none());
        // Native probe receives no inherited display fields; its existing exclusion checks remain intact.
        assert_eq!(probe.tone_map_override,Settings::default().tone_map_override);
        assert_eq!(probe.post_saturation_override,Settings::default().post_saturation_override);
    }
}

#[cfg(test)]
mod night_fill_consumed_fixture_tests {
    use super::*;
    #[test]
    #[ignore = "requires absolute GRID_GAME_ROOT; reports actual purchased Environment/Timeline/Fill, CPU only"]
    fn purchased_night_day_and_timeline_report_actual_consumed_fill_without_new_units() {
        use grid_tools::{environment::Environment, route::Route, time_of_day::{Mode, Timeline}};
        let root = std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").expect("absolute GRID_GAME_ROOT required"));
        assert!(root.is_absolute());
        let baseline = Settings::default();
        for (route_path, mode, lighting_file) in [
            ("tracks/japan/haruna/route_0", Mode::None, "tracks/japan/haruna/lighting.xml"),
            ("tracks/japan/haruna/route_1", Mode::ForcedNight, "tracks/japan/haruna/route_1/lighting.xml"),
            ("tracks/europe/le_mans/route_0", Mode::Timeline, "tracks/europe/le_mans/lighting.xml"),
        ] {
            let route = Route::new(route_path).unwrap();
            let environment = Environment::for_route(&root, &route).unwrap();
            assert_eq!(environment.mode(), mode);
            assert_eq!(environment.lighting_file.replace('\\', "/"), lighting_file);
            let input = std::fs::read(root.join(&environment.lighting_file)).unwrap();
            let sun_adaptation = grid_formats::track_lighting::read_sun_colour(&input).unwrap();
            let source = &environment.lighting;
            let raw_sun = source.rgb("backlight_suncolour").unwrap();
            let raw_back = source.rgb("backlight_backcolour").unwrap();
            let raw_ambient = source.rgb("backlight_ambientcolour").unwrap();
            let scales = ["backlight_sunscale", "backlight_scale", "backlight_ambientscale"].map(|key| source.scalar(key));
            let fill = Fill::from_lighting(source, baseline.sun_illuminance(), false).unwrap();
            assert!(raw_sun.into_iter().chain(raw_back).chain(raw_ambient).all(|v| v.is_finite() && v >= 0.));
            assert!(fill.ambient_brightness.is_finite() && fill.back_lux.is_finite());
            // A counterfactual scale edit checks the ACTUAL existing consumer, not a copied formula.
            // Static XML packing overwrites scales in the selected original path; dynamic binding is separate.
            let mut changed_scales = source.clone();
            for key in ["backlight_sunscale", "backlight_scale", "backlight_ambientscale"] {
                changed_scales.params.insert(key.into(), "16".into());
            }
            let same = Fill::from_lighting(&changed_scales, baseline.sun_illuminance(), false).unwrap();
            assert_eq!(fill.ambient, same.ambient);
            assert_eq!(fill.back, same.back);
            assert_eq!(fill.ambient_brightness.to_bits(), same.ambient_brightness.to_bits());
            assert_eq!(fill.back_lux.to_bits(), same.back_lux.to_bits());
            println!("night_fill_fixture static_route={route_path} mode={mode:?} lighting={} source_sha256={} raw_sun={raw_sun:?} raw_back={raw_back:?} raw_ambient={raw_ambient:?} raw_saved_scales={scales:?} parser_sun_adaptation={sun_adaptation:?} baseline_sun_colour={:?} baseline_sun_lux={} actual_fill={fill:?} effective_original_uniform=UNRESOLVED source_units=dimensionless bevy_units=authored_existing",
                environment.lighting_file, grid_tools::private_parameters::hex(&grid_tools::private_parameters::sha256(&input)), baseline.sun_colour(), baseline.sun_illuminance());
            if mode == Mode::Timeline {
                let timeline = Timeline::load(&root, &route).unwrap().expect("Le Mans timeline required");
                for (clock, t, wanted) in [("1500", 15.0 / 24.0, false), ("0000", 0.0, true)] {
                    let values = timeline.values(t);
                    assert_eq!(timeline.flag("enableHeadlights", t), Some(wanted));
                    let fields: Vec<_> = values.iter().filter(|(key, _)| key.starts_with("backlight_") || matches!(**key, "enableheadlights" | "luminancefactor" | "luminancefactorheadcam" | "shadow_strength")).collect();
                    assert!(fields.iter().all(|(_, value)| value.is_finite()));
                    println!("night_fill_fixture timeline_clock={clock} t={t} raw_dynamic_fields={fields:?} wanted={wanted} dynamic_to_effective_sun_binding=UNRESOLVED no_dynamic_Fill_constructed=true");
                }
            }
        }
    }
}
