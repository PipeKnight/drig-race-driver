//! Independently authored rendering presets. Neither is an original GRID renderer.
use bevy::{
    anti_alias::taa::TemporalAntiAliasing,
    camera::{Exposure, Hdr},
    core_pipeline::tonemapping::Tonemapping,
    image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    light::{
        Atmosphere, AtmosphereEnvironmentMapLight, CascadeShadowConfig, CascadeShadowConfigBuilder,
        DirectionalLightShadowMap, ShadowFilteringMethod, atmosphere::ScatteringMedium,
        light_consts::lux,
    },
    pbr::{AtmosphereSettings, ScreenSpaceAmbientOcclusion},
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
    pub tread_normals: bool,
    pub layered_car: bool,
    pub source_ibl: bool,
    pub smooth_tyres: bool,
    pub track_grade: bool,
    pub saved_grade: Option<grid_formats::graphics_grade::Grade>,
    pub benchmark_frames: Option<u32>,
    pub source_sky: bool,
    pub preset: Preset,
    pub width: u32,
    pub height: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            tread_normals: true,
            layered_car: true,
            source_ibl: false,
            smooth_tyres: true,
            track_grade: false,
            saved_grade: None,
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
        settings.enhanced(),
        if settings.enhanced() {
            "AgX"
        } else {
            "Bevy-default"
        }
    );
    if !settings.enhanced() {
        return;
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
        TemporalAntiAliasing::default(),
        ScreenSpaceAmbientOcclusion::default(),
        ShadowFilteringMethod::Gaussian,
    ));
    if let Some(grade) = settings.saved_grade {
        commands
            .entity(camera)
            .insert(super::track_grade::TrackGrade::from(grade));
    }
}
