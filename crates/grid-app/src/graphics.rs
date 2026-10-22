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
    /// 0: TAA diagnostic, 1: SMAA Ultra (default), 2: unfiltered diagnostic.
    pub anti_aliasing: u8,
    /// 0: sky IBL, 1: deferred control, 2: SSR, 3: local HDR scene probe.
    pub reflection_mode: u8,
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
    /// Optional Bevy directional blur; fraction of the rendered frame, not milliseconds.
    pub motion_shutter: f32,
    /// Bounded scripted DLAA diagnostic only; never a playable default.
    pub dlaa_reset_every_frame: bool,
    pub thin_motion_antenna: bool,
    pub ground_edge_pilot: bool,
    pub source_land: bool,
    /// Source forest pilot; off until same-view GPU validation.
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
    /// Window-only material control; light lenses are a separate material.
    pub window_roughness: f32,
    pub window_reflectance: f32,
    /// Bounded diagnostic: no change to accepted interactive driving materials.
    pub window_pilot: super::window_response::Mode,
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
    /// Optional authored coverage-preserving leaf alpha mips; source remains default.
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
            probe_size: 1024,
            moving_probe: false,
            ssr_thickness: 0.05,
            layered_track: true,
            surface_specular: super::track_surface::Specular::Off,
            surface_view_mip_bias: false,
            asphalt_soften: 0.,
            asphalt_filter_strength: 1.,
            motion_shutter: 0.,
            dlaa_reset_every_frame: false,
            thin_motion_antenna: false,
            ground_edge_pilot: false,
            source_land: true,
            source_trees: false,
            player_race_number: None,
            source_race_numbers: false,
            source_body_occlusion: false,
            source_body_specular: false,
            white_body_specular: false,
            exposure_ev100: 13.,
            post_saturation_override: None,
            window_roughness: 0.08,
            window_reflectance: 0.5,
            window_pilot: super::window_response::Mode::Off,
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
            foliage_coverage: false,
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
            preset: Preset::Baseline,
            width: 1280,
            height: 800,
        }
    }
}

impl Settings {
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
        if self.enhanced()
            && let Some(sun) = self.saved_sun
        {
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
            "AgX"
        } else {
            "Bevy-default"
        }
    );
    if !settings.enhanced() {
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
    println!("graphics_shadow_range metres={} cascades=4 first=18 map_size=4096 mapping=authored", settings.shadow_distance_metres);
    println!("graphics_exposure ev100={} mode=fixed tonemapping=AgX", settings.exposure_ev100);
    println!("graphics_post_saturation value={} override={} scope=authored-camera-grade temperature_tint=unchanged", settings.post_saturation_override.unwrap_or(if settings.track_grade { 1.0 } else { 0.9 }), settings.post_saturation_override.is_some());
    commands.insert_resource(DirectionalLightShadowMap { size: 4096 });
    commands.insert_resource(GlobalAmbientLight::NONE);
    commands.spawn(Atmosphere::earth(
        media.add(ScatteringMedium::earth(256, 256)),
    ));
    commands.entity(camera).insert((
        Hdr,
        Exposure { ev100: settings.exposure_ev100 },
        Tonemapping::AgX,
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
    if let Some(grade) = settings.saved_grade {
        commands
            .entity(camera)
            .insert(super::track_grade::TrackGrade::from(grade));
    }
}


#[cfg(test)]
mod graphics_settings_tests {
    use super::*;
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

}
