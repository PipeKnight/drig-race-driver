//! Time-of-day routes (Le Mans' daynight.xml): the baseline sun, back light,
//! ambient and road specular follow the timeline's `backlight_*` colours and
//! scales at the event's clock, or at `sky::static_start` in a static scene,
//! instead of keeping the static lighting.xml's day values. The light unit
//! stays the scene's (the static preset's [`graphics::sun_reference`]), so
//! the emissive and night-light units, set once at startup, stay valid and
//! the night keeps its authored level (docs/TRACK_ENVIRONMENT.md "Night
//! level"). Colours resolve through the source packing
//! (`effective_lighting::PackedLighting`). The sun keeps the baseline's
//! colour and direction; only its lux follows the timeline's sun peak.
use crate::{
    effective_lighting::{EffectiveLighting, PackedLighting, SpecularColour},
    graphics, source_light_trial, track_surface,
};
use bevy::prelude::*;
use grid_tools::time_of_day::{Mode, Timeline};

#[derive(Resource)]
pub(crate) struct TimelineLighting {
    packed: PackedLighting,
    specular: SpecularColour,
    timeline: Timeline,
    /// Lux per original light unit: the baseline sun's lux over the static
    /// preset's sun reference.
    unit: f32,
    last: Option<EffectiveLighting>,
}

/// The Bevy lights for one resolved sample.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Lights {
    pub sun_lux: f32,
    pub fill: graphics::Fill,
    pub phong_colour: [f32; 3],
}

impl TimelineLighting {
    pub(crate) fn new(
        lighting: &grid_formats::track_environment::Lighting,
        timeline: Timeline,
        sun_lux: f32,
    ) -> Result<Self, &'static str> {
        let packed = PackedLighting::from_static(lighting)?;
        let specular = SpecularColour::from_static(lighting)?;
        let sun = lighting.rgb("backlight_suncolour").ok_or("source sun absent")?;
        let unit = sun_lux / graphics::sun_reference(sun, false);
        Ok(Self { packed, specular, timeline, unit, last: None })
    }

    /// The timeline's lighting at day fraction `t`, in the scene's unit
    /// (Fill's and Phong's linear arithmetic).
    pub(crate) fn sample(&self, t: f64) -> Result<(EffectiveLighting, Lights), &'static str> {
        let (packed, source) = self.packed.resolve(Mode::Timeline, Some(&self.timeline), t)?;
        let peak = |c: [f32; 3], floor: f32| c.into_iter().fold(floor, f32::max);
        let fill: [f32; 3] = std::array::from_fn(|i| source.ambient[i] + source.back[i] / 2.);
        let lights = Lights {
            sun_lux: peak(source.sun, 0.) * self.unit,
            fill: graphics::Fill {
                ambient: fill.map(|c| c / peak(fill, 1e-6)),
                ambient_brightness: peak(fill, 1e-6) * self.unit
                    / std::f32::consts::PI
                    / graphics::BEVY_DIFFUSE_AMBIENT,
                back: source.back.map(|c| c / peak(source.back, 1e-6)),
                back_lux: peak(source.back, 1e-6) / 2. * self.unit,
            },
            phong_colour: self
                .specular
                .rgb(packed.families[0])
                .map(|c| c.max(0.) * self.unit / std::f32::consts::PI),
        };
        Ok((source, lights))
    }
}

/// Installs the follower on a baseline time-of-day scene with the default
/// linear fill (not with the source-lighting trial, which owns these lights).
pub(crate) fn install(
    app: &mut App,
    lighting: &grid_formats::track_environment::Lighting,
    timeline: Option<Timeline>,
    settings: &graphics::Settings,
) {
    let linear = std::env::var("GRID_BASELINE_FILL").map_or(true, |m| m == "linear");
    let Some(timeline) = timeline else { return };
    if settings.enhanced() || settings.baseline_fill.is_none() || !linear || app.world().contains_resource::<source_light_trial::Trial>() {
        println!("timeline_lighting skipped enhanced={} fill={} linear={linear}", settings.enhanced(), settings.baseline_fill.is_some());
        return;
    }
    match TimelineLighting::new(lighting, timeline, settings.sun_illuminance()) {
        Ok(follower) => {
            println!("timeline_lighting version=1 unit={:.1}", follower.unit);
            app.insert_resource(follower).add_systems(Update, follow.after(crate::le_mans_event::tick));
        }
        Err(error) => eprintln!("timeline_lighting skipped: {error}"),
    }
}

fn follow(
    mut follower: ResMut<TimelineLighting>,
    event: Option<Res<crate::le_mans_event::LeMansEvent>>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut lights: Query<(&mut DirectionalLight, Has<source_light_trial::SourceSun>, Has<source_light_trial::SourceBack>)>,
    mut materials: ResMut<Assets<track_surface::SurfaceMaterial>>,
) {
    let t = event.as_ref().map_or_else(crate::sky::static_start, |e| e.time_of_day);
    let (source, sample) = match follower.sample(t) {
        Ok(sample) => sample,
        Err(error) => {
            eprintln!("timeline_lighting t={t}: {error}");
            return;
        }
    };
    if follower.last == Some(source) {
        return;
    }
    for (mut light, sun, back) in &mut lights {
        if sun {
            light.illuminance = sample.sun_lux;
        } else if back {
            let [r, g, b] = sample.fill.back;
            light.color = Color::linear_rgb(r, g, b);
            light.illuminance = sample.fill.back_lux;
        }
    }
    let [r, g, b] = sample.fill.ambient;
    ambient.color = Color::linear_rgb(r, g, b);
    ambient.brightness = sample.fill.ambient_brightness;
    let phong = track_surface::set_trial_phong_colour(&mut materials, sample.phong_colour);
    if follower.last.is_none() {
        println!(
            "timeline_lighting apply t={t:.4} sun_lux={:.0} ambient={:.1} back_lux={:.0} phong_materials={phong}",
            sample.sun_lux, sample.fill.ambient_brightness, sample.fill.back_lux
        );
    }
    follower.last = Some(source);
}

#[cfg(test)]
mod tests {
    use super::*;
    use grid_tools::time_of_day::Curve;

    fn lighting() -> grid_formats::track_environment::Lighting {
        let mut l = grid_formats::track_environment::Lighting::default();
        for (k, v) in [
            ("backlight_suncolour", "3 2 1"),
            ("backlight_backcolour", "0.2 0.2 0.2"),
            ("backlight_ambientcolour", "0.1 0.2 0.4"),
            ("heightangle", "45"),
            ("rotationangle", "0"),
        ] {
            l.params.insert(k.into(), v.into());
        }
        l
    }

    #[test]
    fn night_keyframe_keeps_the_scene_unit() {
        // Noon: the static colours; midnight: a dim blue sun.
        let timeline = Timeline {
            curves: [
                ("backlight_suncolour_r".to_string(), Curve::new(&[(0, 0.1), (720, 1.)], None)),
                ("backlight_suncolour_g".to_string(), Curve::new(&[(0, 0.1), (720, 1.)], None)),
                ("backlight_suncolour_b".to_string(), Curve::new(&[(0, 0.1), (720, 1.)], None)),
                ("backlight_sunscale".to_string(), Curve::new(&[(0, 1.), (720, 3.)], None)),
            ]
            .into_iter()
            .collect(),
        };
        let follower = TimelineLighting::new(&lighting(), timeline, 18_000.).unwrap();
        let (_, noon) = follower.sample(0.5).unwrap();
        let (_, night) = follower.sample(0.).unwrap();
        // The static preset's peak 3 is the reference: a full noon sun is 18,000 lux.
        assert!((noon.sun_lux - 18_000.).abs() < 1., "{}", noon.sun_lux);
        // Midnight's sun peak ~0.1 stays at ~0.1 / 3 of it; the fill does not change unit.
        assert!((night.sun_lux / 18_000. - 26. / 255. / 3.).abs() < 1e-3, "{}", night.sun_lux);
        assert_eq!(night.fill.ambient_brightness, noon.fill.ambient_brightness);
    }
}
