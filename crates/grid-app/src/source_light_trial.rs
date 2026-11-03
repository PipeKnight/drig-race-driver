//! Bounded source-lighting trial. Bevy units are AUTHORED; default off.
use crate::{
    effective_lighting::{AuthoredFill, EffectiveLighting, PackedLighting},
    graphics, track_surface,
};
use bevy::prelude::*;
use grid_tools::time_of_day::{Mode, Timeline};

#[derive(Component)]
pub(crate) struct SourceSun;
#[derive(Component)]
pub(crate) struct SourceBack;
#[derive(Clone, Copy, Debug, PartialEq)]
struct Sample {
    source: EffectiveLighting,
    fill: AuthoredFill,
    sun_colour: [f32; 3],
    sun_lux: f32,
    phong_colour: [f32; 3],
}
#[derive(Resource)]
pub(crate) struct Trial {
    packed: PackedLighting,
    mode: Mode,
    units: f32,
    // Existing sourcePhong colour already accounts for constructor fallback
    // and spec_linked. Recalibrate its unit, preserving shader and direction.
    phong_colour: [f32; 3],
    old_unit: f32,
    last: Option<Sample>,
}

pub(crate) fn requested() -> Result<Option<f32>, String> {
    match std::env::var("GRID_SOURCE_LIGHTING_UNITS") {
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(_) => Err("source lighting units must be Unicode".into()),
        Ok(value) => parse_units(&value).map(Some),
    }
}
fn parse_units(value: &str) -> Result<f32, String> {
    let units = value
        .parse::<f32>()
        .map_err(|_| "invalid authored source lighting units")?;
    if !units.is_finite() || units <= 0. {
        return Err("authored source lighting units must be finite positive".into());
    }
    Ok(units)
}
pub(crate) fn validate_context(
    drive: bool,
    reference: bool,
    headless: bool,
    collision: bool,
    capture: (bool, Option<u64>),
    settings: &graphics::Settings,
    linear_fill: bool,
) -> Result<(), &'static str> {
    if !drive
        || !reference
        || headless
        || collision
        || !capture.0
        || !capture.1.is_some_and(|n| (1..=1800).contains(&n))
        || settings.enhanced()
        || !settings.layered_track
        || settings.baseline_fill.is_none()
        || settings.baseline_specular.is_none()
        || !linear_fill
    {
        return Err(
            "source lighting trial requires bounded baseline source-material reference driving render with source fill and Phong (1..1800 ticks)",
        );
    }
    Ok(())
}
impl Trial {
    fn prepare(
        raw: &grid_formats::track_environment::Lighting,
        mode: Mode,
        settings: &graphics::Settings,
        units: f32,
    ) -> Result<Self, String> {
        if !units.is_finite() || units <= 0. {
            return Err("source lighting units invalid".into());
        }
        let packed = PackedLighting::from_static(raw)?;
        let raw_sun = raw.rgb("backlight_suncolour").ok_or("source sun absent")?;
        let old_unit = settings.sun_illuminance() / raw_sun.into_iter().fold(1e-3_f32, f32::max);
        let phong = settings.baseline_specular.ok_or("sourcePhong required")?;
        if !old_unit.is_finite() || old_unit <= 0. {
            return Err("baseline sourcePhong calibration invalid".into());
        }
        let value = Self {
            packed,
            mode,
            units,
            phong_colour: phong.colour,
            old_unit,
            last: None,
        };
        // Validate static source/calibration before system installation. Timeline
        // admission below validates its ACTUAL current clock, without a dummy state.
        let (_, initial) = packed.resolve(Mode::None, None, 0.)?;
        initial.authored_fill(units)?;
        Ok(value)
    }
    fn sample(&self, timeline: Option<&Timeline>, t: f64) -> Result<Sample, String> {
        let (_, source) = self.packed.resolve(self.mode, timeline, t)?;
        let fill = source.authored_fill(self.units)?;
        let peak = source.sun.into_iter().fold(0., f32::max);
        let sun_colour = if peak > 0. {
            source.sun.map(|v| v / peak)
        } else {
            [0.; 3]
        };
        let sun_lux = peak * self.units;
        let phong_colour = self.phong_colour.map(|v| v * (self.units / self.old_unit));
        if !sun_lux.is_finite() || !phong_colour.into_iter().all(|v| v.is_finite() && v >= 0.) {
            return Err("source lighting authored calibration overflow".into());
        }
        Ok(Sample {
            source,
            fill,
            sun_colour,
            sun_lux,
            phong_colour,
        })
    }
}
pub(crate) fn install(
    app: &mut App,
    settings: &graphics::Settings,
    units: f32,
) -> Result<(), String> {
    let env = &app
        .world()
        .get_resource::<crate::track_environment::TrackEnvironment>()
        .ok_or("source lighting trial requires actual TrackEnvironment")?
        .0;
    let trial = Trial::prepare(&env.lighting, env.mode(), settings, units)?;
    let event = app
        .world()
        .get_resource::<crate::le_mans_event::LeMansEvent>();
    let (timeline, t) = event.map_or((None, 0.), |e| (e.timeline.as_ref(), e.time_of_day));
    trial.sample(timeline, t)?;
    println!(
        "source_lighting_trial version=1 event=install mode={:?} units={units} unit_policy=AUTHORED_SHARED colour_space=linear-adaptation original_units=UNPROVEN static_x87_boundaries=UNPROVEN clock=existing-event transform_shadow_reflection_overrides=false",
        trial.mode
    );
    app.insert_resource(trial)
        .add_systems(Update, update.after(crate::drive_present));
    Ok(())
}
fn update(
    mut trial: ResMut<Trial>,
    event: Option<Res<crate::le_mans_event::LeMansEvent>>,
    replay: Option<Res<crate::replay::Replay>>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut lights: Query<(&mut DirectionalLight, Has<SourceSun>, Has<SourceBack>)>,
    mut materials: ResMut<Assets<track_surface::SurfaceMaterial>>,
) {
    assert!(
        !replay.as_ref().is_some_and(|r| r.session.is_some()),
        "bounded source lighting trial does not support replay clock"
    );
    let (timeline, t) = event
        .as_ref()
        .map_or((None, 0.), |e| (e.timeline.as_ref(), e.time_of_day));
    let sample = trial
        .sample(timeline, t)
        .expect("bounded source lighting trial failed source resolution");
    if trial.last == Some(sample) {
        return;
    }
    let suns = lights
        .iter()
        .filter(|(_, sun, back)| *sun && !*back)
        .count();
    let backs = lights
        .iter()
        .filter(|(_, sun, back)| *back && !*sun)
        .count();
    let marked = lights.iter().filter(|(_, sun, back)| *sun || *back).count();
    assert_eq!(marked, 2, "ambiguous source-light marker targets");
    assert_eq!(
        (suns, backs),
        (1, 1),
        "source lighting trial requires exact existing sun/back entity markers"
    );
    let changed = track_surface::set_trial_phong_colour(&mut materials, sample.phong_colour);
    assert!(
        changed > 0,
        "source lighting trial requires an attached existing sourcePhong material"
    );
    // All source/marker/material admission precedes writes to the lights.
    for (mut light, sun, back) in &mut lights {
        if sun {
            light.color = Color::linear_rgb(
                sample.sun_colour[0],
                sample.sun_colour[1],
                sample.sun_colour[2],
            );
            light.illuminance = sample.sun_lux;
        } else if back {
            light.color = Color::linear_rgb(
                sample.fill.back_rgb[0],
                sample.fill.back_rgb[1],
                sample.fill.back_rgb[2],
            );
            light.illuminance = sample.fill.back_lux;
        }
    }
    ambient.color = Color::linear_rgb(
        sample.fill.ambient_rgb[0],
        sample.fill.ambient_rgb[1],
        sample.fill.ambient_rgb[2],
    );
    ambient.brightness = sample.fill.ambient_brightness;
    println!(
        "source_lighting_trial version=1 event=apply t={t} units={} source={:?} sun_lux={} fill={:?} phong_colour={:?} source_materials={changed} clock=existing-event",
        trial.units, sample.source, sample.sun_lux, sample.fill, sample.phong_colour
    );
    trial.last = Some(sample);
}
#[cfg(test)]
mod tests {
    use super::*;
    use grid_tools::time_of_day::Curve;
    fn raw() -> grid_formats::track_environment::Lighting {
        let mut l = grid_formats::track_environment::Lighting::default();
        for k in [
            "backlight_suncolour",
            "backlight_backcolour",
            "backlight_ambientcolour",
        ] {
            l.params.insert(k.into(), "2 1 0.5".into());
        }
        l
    }
    fn settings() -> graphics::Settings {
        graphics::Settings {
            baseline_fill: Some(graphics::Fill {
                ambient: [1.; 3],
                ambient_brightness: 1.,
                back: [1.; 3],
                back_lux: 1.,
            }),
            baseline_specular: Some(graphics::Phong {
                colour: [3., 2., 1.],
                direction_to_light: [0., 1., 0.],
            }),
            ..default()
        }
    }
    fn app(
        mode: Mode,
    ) -> (
        App,
        Entity,
        Entity,
        Entity,
        Handle<track_surface::SurfaceMaterial>,
    ) {
        let mut a = App::new();
        a.insert_resource(Trial::prepare(&raw(), mode, &settings(), 10.).unwrap());
        a.init_resource::<GlobalAmbientLight>();
        a.init_resource::<Assets<track_surface::SurfaceMaterial>>();
        let h = a
            .world_mut()
            .resource_mut::<Assets<track_surface::SurfaceMaterial>>()
            .add(track_surface::trial_fixture());
        let sun = a
            .world_mut()
            .spawn((
                DirectionalLight {
                    illuminance: 123.,
                    shadow_maps_enabled: true,
                    ..default()
                },
                SourceSun,
                Transform::from_xyz(1., 2., 3.),
            ))
            .id();
        let back = a
            .world_mut()
            .spawn((
                DirectionalLight {
                    illuminance: 456.,
                    shadow_maps_enabled: false,
                    ..default()
                },
                SourceBack,
                Transform::from_xyz(4., 5., 6.),
            ))
            .id();
        let other = a
            .world_mut()
            .spawn(DirectionalLight {
                illuminance: 73.,
                ..default()
            })
            .id();
        a.add_systems(Update, update);
        (a, sun, back, other, h)
    }
    #[test]
    fn real_ecs_consumer_updates_only_marked_lights_and_existing_phong() {
        let (mut a, sun, back, other, h) = app(Mode::ForcedNight);
        let st = *a.world().get::<Transform>(sun).unwrap();
        let bt = *a.world().get::<Transform>(back).unwrap();
        let before = track_surface::trial_snapshot(
            a.world()
                .resource::<Assets<track_surface::SurfaceMaterial>>()
                .get(&h)
                .unwrap(),
        );
        a.update();
        let s = a.world().get::<DirectionalLight>(sun).unwrap();
        assert_eq!(s.illuminance, 20.);
        assert!(s.shadow_maps_enabled);
        let b = a.world().get::<DirectionalLight>(back).unwrap();
        assert!(!b.shadow_maps_enabled);
        assert_eq!(b.illuminance, 10.);
        assert_eq!(*a.world().get::<Transform>(sun).unwrap(), st);
        assert_eq!(*a.world().get::<Transform>(back).unwrap(), bt);
        assert_eq!(
            a.world()
                .get::<DirectionalLight>(other)
                .unwrap()
                .illuminance,
            73.
        );
        assert!(a.world().resource::<GlobalAmbientLight>().brightness > 0.);
        let after = track_surface::trial_snapshot(
            a.world()
                .resource::<Assets<track_surface::SurfaceMaterial>>()
                .get(&h)
                .unwrap(),
        );
        assert_eq!(before.1, after.1);
        assert_eq!(before.0.w, after.0.w);
        assert_ne!(before.0.truncate(), after.0.truncate());
        let applied = a.world().resource::<Trial>().last;
        a.update();
        assert_eq!(a.world().resource::<Trial>().last, applied);
    }
    #[test]
    fn real_ecs_timeline_uses_existing_clock_and_restores_initial_bytes_on_reset() {
        let (mut a, sun, _, _, _) = app(Mode::Timeline);
        let timeline = Timeline {
            curves: [
                (
                    "backlight_suncolour_r".into(),
                    Curve::new(&[(0, 0.), (720, 1.)], None),
                ),
                (
                    "backlight_sunscale".into(),
                    Curve::new(&[(0, 1.), (720, 2.)], None),
                ),
            ]
            .into_iter()
            .collect(),
        };
        let mut event =
            crate::le_mans_event::LeMansEvent::new(None, Mode::Timeline, vec![], Some(timeline));
        event.time_of_day = 0.;
        a.insert_resource(event);
        a.update();
        let start = a.world().resource::<Trial>().last.unwrap();
        assert_eq!(start.source.sun[0], 0.);
        a.world_mut()
            .resource_mut::<crate::le_mans_event::LeMansEvent>()
            .time_of_day = 0.5;
        a.update();
        let noon = a.world().resource::<Trial>().last.unwrap();
        assert_eq!(noon.source.sun[0], 2.);
        assert_ne!(start, noon);
        let light = a.world().get::<DirectionalLight>(sun).unwrap().illuminance;
        a.update();
        assert_eq!(
            a.world().get::<DirectionalLight>(sun).unwrap().illuminance,
            light
        );
        a.world_mut()
            .resource_mut::<crate::le_mans_event::LeMansEvent>()
            .time_of_day = 0.;
        a.update();
        assert_eq!(a.world().resource::<Trial>().last, Some(start));
        // Fields without timeline keys retain initialized source bytes throughout.
        assert_eq!(start.source.ambient, noon.source.ambient);
    }
    #[test]
    fn explicit_units_and_actual_context_are_required_without_changing_off() {
        for v in ["0", "-1", "NaN", "inf", "garbage"] {
            assert!(parse_units(v).is_err());
        }
        assert_eq!(parse_units("1").unwrap(), 1.);
        assert!(
            validate_context(
                true,
                true,
                false,
                false,
                (true, Some(300)),
                &graphics::Settings::default(),
                true
            )
            .is_err()
        );
        assert!(
            validate_context(
                true,
                true,
                false,
                false,
                (true, Some(300)),
                &settings(),
                true
            )
            .is_ok()
        );
        assert!(
            validate_context(
                true,
                true,
                false,
                false,
                (false, Some(300)),
                &settings(),
                true
            )
            .is_err()
        );
        assert!(
            validate_context(
                true,
                true,
                false,
                false,
                (true, Some(1801)),
                &settings(),
                true
            )
            .is_err()
        );
        // Missing actual environment is an admission error, never guessed route data.
        assert!(install(&mut App::new(), &settings(), 1.).is_err());
    }
}
