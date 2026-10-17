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
    post_process::bloom::Bloom,
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
    pub source_land: bool,
    /// Source forest pilot; off until same-view GPU validation.
    pub source_trees: bool,
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
    pub source_sky: bool,
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
            source_land: true,
            source_trees: false,
            clearcoat_prepass_fix: true,
            tread_normals: true,
            layered_car: true,
            source_ibl: false,
            smooth_tyres: true,
            track_grade: false,
            saved_grade: None,
            saved_fog: None,
            benchmark_frames: None,
            source_sky: true,
            preset: Preset::Baseline,
            width: 1280,
            height: 800,
        }
    }
}

impl Settings {
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
                maximum_distance: 250.,
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
        settings.enhanced(),
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
    commands.insert_resource(DirectionalLightShadowMap { size: 4096 });
    commands.insert_resource(GlobalAmbientLight::NONE);
    commands.spawn(Atmosphere::earth(
        media.add(ScatteringMedium::earth(256, 256)),
    ));
    commands.entity(camera).insert((
        Hdr,
        Exposure { ev100: 13.0 },
        Tonemapping::AgX,
        // Authored restrained warm grade; original GRID's LUT is not recovered.
        ColorGrading {
            global: ColorGradingGlobal {
                temperature: if settings.track_grade { 0.0 } else { 0.012 },
                tint: if settings.track_grade { 0.0 } else { -0.003 },
                post_saturation: if settings.track_grade { 1.0 } else { 0.9 },
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
        ScreenSpaceAmbientOcclusion::default(),
        ShadowFilteringMethod::Gaussian,
    ));
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
