//! Time-of-day routes (Le Mans' daynight.xml): the sun, baseline back light,
//! ambient and road specular follow the timeline's `backlight_*` colours and
//! scales at the event's clock, or at `sky::static_start` in a static scene,
//! instead of keeping the static lighting.xml's day values. The light unit
//! stays the scene's (the static preset's [`graphics::sun_reference`]), so
//! the emissive and night-light units, set once at startup, stay valid and
//! the night keeps its authored level (docs/TRACK_ENVIRONMENT.md "Night
//! level"). Colours resolve through the source packing
//! (`effective_lighting::PackedLighting`). Both profiles follow the authored
//! sun direction. Enhanced scenes also use the sampled sun hue.
use crate::{
    effective_lighting::{EffectiveLighting, PackedLighting, SpecularColour},
    graphics, source_light_trial, track_surface,
};
use bevy::prelude::*;
use grid_formats::track_lighting::Sun;
use grid_tools::time_of_day::{Mode, Timeline};

#[derive(Resource)]
pub(crate) struct TimelineLighting {
    packed: PackedLighting,
    specular: SpecularColour,
    timeline: Timeline,
    /// Lux per original light unit: the baseline sun's lux over the static
    /// preset's sun reference.
    unit: f32,
    last: Option<Lights>,
    enhanced: bool,
    source_direction: bool,
    static_sun: [Option<f32>; 2],
    static_specular_sun: [Option<f32>; 2],
}

/// The Bevy lights for one resolved sample.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Lights {
    pub sun_lux: f32,
    pub sun_direction: Option<Sun>,
    pub phong_direction: Option<Sun>,
    pub sun_colour: [f32; 3],
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
        Ok(Self {
            packed, specular, timeline, unit, last: None, enhanced: false, source_direction: true,
            static_sun: [lighting.scalar("heightangle"), lighting.scalar("rotationangle")],
            static_specular_sun: [lighting.scalar("spec_heightangle"), lighting.scalar("spec_rotationangle")],
        })
    }

    /// The timeline's lighting at day fraction `t`, in the scene's unit
    /// (Fill's and Phong's linear arithmetic).
    pub(crate) fn sample(&self, t: f64) -> Result<(EffectiveLighting, Lights), &'static str> {
        let (packed, source) = self.packed.resolve(Mode::Timeline, Some(&self.timeline), t)?;
        let peak = |c: [f32; 3], floor: f32| c.into_iter().fold(floor, f32::max);
        let fill: [f32; 3] = std::array::from_fn(|i| source.ambient[i] + source.back[i] / 2.);
        let sun_peak = peak(source.sun, 0.);
        let sun_direction = self.source_direction
            .then(|| sample_angles(&self.timeline, t, ["heightangle", "rotationangle"], self.static_sun))
            .flatten();
        let phong_direction = if self.specular.linked() {
            sun_direction
        } else {
            sample_angles(
                &self.timeline, t, ["spec_heightangle", "spec_rotationangle"], self.static_specular_sun,
            )
        };
        let lights = Lights {
            sun_lux: sun_peak * self.unit,
            sun_direction,
            phong_direction,
            sun_colour: source.sun.map(|channel| channel / sun_peak.max(1e-6)),
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

/// Timeline curves override individual initialized lighting.xml fields.
fn sample_angles(timeline: &Timeline, t: f64, keys: [&str; 2], fallback: [Option<f32>; 2]) -> Option<Sun> {
    let [height, rotation] = std::array::from_fn(|i| timeline.value(keys[i], t).or(fallback[i]));
    let (height_degrees, rotation_degrees) = (height?, rotation?);
    (height_degrees.is_finite() && rotation_degrees.is_finite())
        .then_some(Sun { height_degrees, rotation_degrees })
}

/// Follows the source timeline on scenes using the standard lighting path.
pub(crate) fn install(
    app: &mut App,
    lighting: &grid_formats::track_environment::Lighting,
    timeline: Option<Timeline>,
    settings: &graphics::Settings,
    source_direction: bool,
) {
    let linear = std::env::var("GRID_BASELINE_FILL").map_or(true, |m| m == "linear");
    let Some(timeline) = timeline else { return };
    if (!settings.enhanced() && (settings.baseline_fill.is_none() || !linear))
        || app.world().contains_resource::<source_light_trial::Trial>()
    {
        println!("timeline_lighting skipped enhanced={} fill={} linear={linear}", settings.enhanced(), settings.baseline_fill.is_some());
        return;
    }
    match TimelineLighting::new(lighting, timeline, settings.sun_illuminance()) {
        Ok(mut follower) => {
            follower.enhanced = settings.enhanced();
            follower.source_direction = source_direction;
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
    mut lights: Query<(
        &mut DirectionalLight, &mut Transform,
        Has<source_light_trial::SourceSun>, Has<source_light_trial::SourceBack>,
    )>,
    mut materials: ResMut<Assets<track_surface::SurfaceMaterial>>,
) {
    let t = event.as_ref().map_or_else(crate::sky::static_start, |e| e.time_of_day);
    let (_, sample) = match follower.sample(t) {
        Ok(sample) => sample,
        Err(error) => {
            eprintln!("timeline_lighting t={t}: {error}");
            return;
        }
    };
    let previous = follower.last;
    if previous == Some(sample) {
        return;
    }
    let sun_transform = if previous.is_none_or(|old| old.sun_direction != sample.sun_direction) {
        sample.sun_direction.map(graphics::source_sun_transform)
    } else {
        None
    };
    for (mut light, mut transform, sun, back) in &mut lights {
        if sun {
            if light.illuminance != sample.sun_lux {
                light.illuminance = sample.sun_lux;
            }
            if let Some(sun_transform) = sun_transform {
                transform.set_if_neq(sun_transform);
            }
            if follower.enhanced {
                let [r, g, b] = sample.sun_colour;
                let colour = Color::linear_rgb(r, g, b);
                if light.color != colour {
                    light.color = colour;
                }
            }
        } else if back && !follower.enhanced {
            let [r, g, b] = sample.fill.back;
            let colour = Color::linear_rgb(r, g, b);
            if light.color != colour {
                light.color = colour;
            }
            if light.illuminance != sample.fill.back_lux {
                light.illuminance = sample.fill.back_lux;
            }
            if let Some(sun_transform) = sun_transform {
                let rotation = sun_transform.rotation * Quat::from_rotation_x(std::f32::consts::PI);
                if transform.rotation != rotation {
                    transform.rotation = rotation;
                }
            }
        }
    }
    let phong = if follower.enhanced {
        // The enhanced renderer gets its fill from its environment.
        0
    } else {
        let [r, g, b] = sample.fill.ambient;
        let colour = Color::linear_rgb(r, g, b);
        if ambient.color != colour {
            ambient.color = colour;
        }
        if ambient.brightness != sample.fill.ambient_brightness {
            ambient.brightness = sample.fill.ambient_brightness;
        }
        if previous.is_none_or(|old| {
            old.phong_colour != sample.phong_colour || old.phong_direction != sample.phong_direction
        }) {
            track_surface::set_phong_lighting(
                &mut materials, sample.phong_colour, sample.phong_direction.map(Sun::direction_to_light),
            ).0
        } else {
            0
        }
    };
    if follower.last.is_none() {
        println!(
            "timeline_lighting apply t={t:.4} sun_lux={:.0} ambient={:.1} back_lux={:.0} phong_materials={phong}",
            sample.sun_lux, sample.fill.ambient_brightness, sample.fill.back_lux
        );
    }
    follower.last = Some(sample);
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
    fn source_direction_uses_timeline_and_falls_back_per_angle() {
        let timeline = Timeline {
            curves: [(
                "heightangle".to_string(),
                Curve::new(&[(0, 93.5), (720, 139.)], None),
            )].into_iter().collect(),
        };
        let mut static_lighting = lighting();
        static_lighting.params.insert("rotationangle".into(), "62.5".into());
        let mut follower = TimelineLighting::new(&static_lighting, timeline, 18_000.).unwrap();
        let (_, night) = follower.sample(0.).unwrap();
        let (_, day) = follower.sample(0.5).unwrap();
        let sun = night.sun_direction.unwrap();
        assert_eq!(sun.height_degrees, 93.5);
        assert_eq!(sun.rotation_degrees, 62.5);
        assert_eq!(night.phong_direction, night.sun_direction);
        assert_ne!(day.sun_direction, night.sun_direction);
        // The original expects a source vector towards the light. Bevy's
        // directional light emits from its local -Z in the opposite direction.
        let toward = Vec3::from_array(sun.direction_to_light());
        assert!(toward.x > 0. && toward.y > 0.99 && toward.z < 0.);
        let emit = graphics::source_sun_transform(sun).forward().as_vec3();
        assert!(emit.distance(-toward) < 1e-6);

        follower.source_direction = false;
        assert_eq!(follower.sample(0.).unwrap().1.sun_direction, None);
    }

    #[test]
    fn unchanged_phong_uniforms_and_non_phong_assets_are_skipped() {
        let mut assets = Assets::default();
        let active = assets.add(track_surface::trial_fixture());
        let inactive = assets.add(track_surface::trial_fixture_with_strength(0.));
        let inactive_before = track_surface::trial_snapshot(assets.get(&inactive).unwrap());
        assert_eq!(track_surface::set_phong_lighting(&mut assets, [1., 2., 3.], Some([0., 1., 0.])), (1, 1));
        assert_eq!(track_surface::set_phong_lighting(&mut assets, [1., 2., 3.], Some([0., 1., 0.])), (1, 0));
        assert_eq!(track_surface::set_phong_lighting(&mut assets, [1., 2., 3.], Some([1., 0., 0.])), (1, 1));
        assert_eq!(track_surface::trial_snapshot(assets.get(&active).unwrap()).1, Vec4::X);
        assert_eq!(track_surface::trial_snapshot(assets.get(&inactive).unwrap()), inactive_before);
    }

    #[test]
    fn unlinked_phong_direction_retains_its_separate_static_angles() {
        let mut static_lighting = lighting();
        for (k, v) in [("spec_linked", "0"), ("spec_heightangle", "22"), ("spec_rotationangle", "70")] {
            static_lighting.params.insert(k.into(), v.into());
        }
        let timeline = Timeline { curves: Default::default() };
        let follower = TimelineLighting::new(&static_lighting, timeline, 18_000.).unwrap();
        let (_, sample) = follower.sample(0.).unwrap();
        assert_eq!(sample.sun_direction.unwrap().rotation_degrees, 0.);
        assert_eq!(sample.phong_direction, Some(Sun { height_degrees: 22., rotation_degrees: 70. }));
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
