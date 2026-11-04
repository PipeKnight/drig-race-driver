//! Default-off live-only AUTHORED PBR headlight trial. Not source optics parity.
use bevy::{prelude::*, transform::TransformSystems};
use grid_tools::{car::Car as CarModel, time_of_day::Mode};
use std::path::Path;

pub fn requested() -> Result<Option<f32>, String> {
    match std::env::var("GRID_HEADLIGHT_PILOT_LUMENS") {
        Err(std::env::VarError::NotPresent) => Ok(None),
        Ok(s) => parse_intensity(&s).map(Some),
        Err(_) => Err("headlight pilot intensity must be Unicode".into()),
    }
}
fn parse_intensity(s: &str) -> Result<f32, String> {
    let value = s
        .parse::<f32>()
        .map_err(|_| "invalid authored headlight pilot lumens")?;
    if !value.is_finite() || value <= 0. {
        return Err("authored headlight pilot lumens must be finite positive".into());
    }
    Ok(value)
}
pub struct Preparation {
    geometry: crate::headlight_frame::Preparation,
    slots: usize,
    lumens: f32,
}
impl Preparation {
    pub fn load(
        root: &Path,
        models: &[CarModel],
        slots: &[usize],
        lumens: f32,
    ) -> Result<Self, String> {
        Ok(Self {
            geometry: crate::headlight_frame::Preparation::load(root, models, slots)?,
            slots: slots.len(),
            lumens,
        })
    }
    pub fn retain_anchor(&mut self, model: usize, anchor: Mat4) -> Result<(), String> {
        self.geometry.retain_anchor(model, anchor)
    }
}
#[derive(Component)]
pub struct Projector;
struct Slot {
    entity: Entity,
}
#[derive(Resource)]
pub struct Pilot {
    slots: Vec<Slot>,
    geometry: crate::headlight_frame::Prepared,
    mode: Mode,
    lumens: f32,
}
pub type Lights<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut SpotLight,
        &'static mut Transform,
        &'static mut Visibility,
    ),
    (
        With<Projector>,
        Without<crate::MovingPart>,
        Without<crate::OpponentPart>,
        Without<crate::ViewerCamera>,
    ),
>;
pub type Context<'w, 's> = (
    Option<Res<'w, Pilot>>,
    Option<Res<'w, crate::vehicle_lights::State>>,
    Lights<'w, 's>,
);
fn require_context(world: &World, mode: Mode, slots: usize) -> Result<(), String> {
    let environment = world
        .get_resource::<crate::track_environment::TrackEnvironment>()
        .ok_or("requested headlight pilot: source TrackEnvironment unavailable (load failed)")?;
    if environment.0.mode() != mode {
        return Err("requested headlight pilot: source environment mode mismatch".into());
    }
    let state = world
        .get_resource::<crate::vehicle_lights::State>()
        .ok_or("requested headlight pilot: shared source activation State unavailable")?;
    let markers = state.marker_lights.ok_or(
        "requested headlight pilot: activation State was initialized without source environment",
    )?;
    if markers != environment.0.vehicle_lights()
        || slots == 0
        || state.headlights.delays().len() != slots
    {
        return Err(
            "requested headlight pilot: shared source activation context/actual slots mismatch"
                .into(),
        );
    }
    Ok(())
}
pub fn install(app: &mut App, preparation: Preparation, mode: Mode) -> Result<(), String> {
    require_context(app.world(), mode, preparation.slots)?;
    let geometry = preparation.geometry.finish()?;
    let mut slots = Vec::new();
    for slot in 0..geometry.len() {
        let entity = app
            .world_mut()
            .spawn((
                Projector,
                SpotLight {
                    color: Color::WHITE,
                    intensity: 0.,
                    range: if slot == 0 { 110. } else { 60. },
                    inner_angle: 2.0_f32.atan() * 0.8,
                    outer_angle: 2.0_f32.atan(),
                    shadow_maps_enabled: false,
                    contact_shadows_enabled: false,
                    ..default()
                },
                Transform::IDENTITY,
                Visibility::Hidden,
            ))
            .id();
        slots.push(Slot { entity });
    }
    for (model, value, provenance) in geometry.selections() {
        println!(
            "headlight_pilot model={model} point={:?} selection={} scope=canonical-LOD0 runtime8slot_parity=false",
            value, provenance
        );
    }
    println!(
        "headlight_pilot enabled=true lumens={} adapter=AUTHORED-neutral-PBR source_photometry=false live_only=true shadows=false cookie=false range=source-number-engine-units cone=authored-circular-atan2-inner0.8",
        preparation.lumens
    );
    app.insert_resource(Pilot {
        slots,
        geometry,
        mode,
        lumens: preparation.lumens,
    })
    .add_systems(
        PostUpdate,
        replay_guard
            .after(crate::replay::present)
            .before(TransformSystems::Propagate),
    );
    Ok(())
}
pub fn begin(context: &mut Context<'_, '_>) {
    if context.0.is_some() {
        for (mut light, _, mut visibility) in &mut context.2 {
            light.intensity = 0.;
            *visibility = Visibility::Hidden;
        }
    }
}
fn apply(
    pilot: &Pilot,
    slot: usize,
    pose: Mat4,
    weight: Option<f64>,
    light: &mut SpotLight,
    t: &mut Transform,
    v: &mut Visibility,
) -> Result<(), String> {
    light.intensity = 0.;
    *v = Visibility::Hidden;
    pilot.slots.get(slot).ok_or("headlight slot absent")?;
    let Some(weight) = weight else {
        return Ok(());
    };
    if pilot.mode == Mode::None {
        return Ok(());
    }
    if !weight.is_finite() || !(0.001..=1.).contains(&weight) {
        return Err("invalid headlight presentation domain".into());
    }
    let Some(frame) = pilot.geometry.frame(slot, pose, pilot.mode)? else {
        return Ok(());
    };
    *t = Transform::from_translation(frame.origin).looking_to(frame.direction, frame.up);
    light.intensity = pilot.lumens * (weight as f32);
    *v = Visibility::Inherited;
    Ok(())
}
pub fn present(context: &mut Context<'_, '_>, slot: usize, pose: Mat4) {
    let (pilot, state, lights) = context;
    let Some(pilot) = pilot else {
        return;
    };
    let state = state
        .as_ref()
        .expect("pilot requires shared activation State");
    assert_eq!(
        pilot.slots.len(),
        state.headlights.delays().len(),
        "pilot actual activation slots changed"
    );
    assert!(
        slot < state.headlights.delays().len(),
        "pilot cannot use missing-slot activation fallback"
    );
    let binding = &pilot.slots[slot];
    let (mut light, mut transform, mut visibility) = lights
        .get_mut(binding.entity)
        .expect("pilot light entity exists");
    apply(
        pilot,
        slot,
        pose,
        state.headlights.car(slot),
        &mut light,
        &mut transform,
        &mut visibility,
    )
    .expect("finite consumed headlight input");
}
fn hide(lights: &mut Lights<'_, '_>) {
    for (mut l, _, mut v) in lights {
        l.intensity = 0.;
        *v = Visibility::Hidden;
    }
}
fn replay_guard(replay: Option<Res<crate::replay::Replay>>, mut lights: Lights) {
    if replay.is_some_and(|r| r.session.is_some()) {
        hide(&mut lights);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture_preparation(
        models: Vec<(Vec3, &'static str)>,
        anchors: Vec<Option<Mat4>>,
        slots: Vec<usize>,
        lumens: f32,
    ) -> Preparation {
        Preparation {
            slots: slots.len(),
            geometry: crate::headlight_frame::Preparation::fixture(models, anchors, slots),
            lumens,
        }
    }

    fn environment(mode: Mode) -> crate::track_environment::TrackEnvironment {
        crate::track_environment::TrackEnvironment(grid_tools::environment::Environment {
            lighting_file: "authored-context-fixture".into(),
            lighting: default(),
            weather_fx: default(),
            flags: grid_tools::environment::TrackFlags {
                supports_time_of_day: mode == Mode::Timeline,
                headlights_on: mode == Mode::ForcedNight,
                vehicle_lights_on: false,
            },
        })
    }
    #[test]
    fn requested_installer_refuses_missing_source_context_before_spawning_and_accepts_valid_day() {
        for invalid in ["0", "-1", "NaN", "inf", "bad"] {
            assert!(parse_intensity(invalid).is_err());
        }
        let preparation = || {
            fixture_preparation(
                vec![(Vec3::Z, "authored-context-control")],
                vec![Some(Mat4::IDENTITY)],
                vec![0],
                123.,
            )
        };
        let mut app = App::new();
        assert!(
            install(&mut app, preparation(), Mode::None)
                .unwrap_err()
                .contains("TrackEnvironment unavailable")
        );
        assert!(!app.world().contains_resource::<Pilot>());
        app.insert_resource(environment(Mode::None));
        assert!(
            install(&mut app, preparation(), Mode::None)
                .unwrap_err()
                .contains("State unavailable")
        );
        app.insert_resource(crate::vehicle_lights::State::new(Mode::None, 1, None));
        assert!(
            install(&mut app, preparation(), Mode::None)
                .unwrap_err()
                .contains("initialized without source environment")
        );
        app.insert_resource(crate::vehicle_lights::State::new(
            Mode::None,
            2,
            Some(false),
        ));
        assert!(
            install(&mut app, preparation(), Mode::None)
                .unwrap_err()
                .contains("actual slots mismatch")
        );
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<Projector>>()
                .iter(app.world())
                .count(),
            0
        );
        app.insert_resource(crate::vehicle_lights::State::new(
            Mode::None,
            1,
            Some(false),
        ));
        install(&mut app, preparation(), Mode::None).unwrap();
        let entity = app.world().resource::<Pilot>().slots[0].entity;
        assert_eq!(app.world().get::<SpotLight>(entity).unwrap().intensity, 0.);
        assert_eq!(
            *app.world().get::<Visibility>(entity).unwrap(),
            Visibility::Hidden
        );
    }
    #[derive(Resource)]
    struct Poses(Vec<Mat4>);
    fn consume(poses: Res<Poses>, mut context: Context) {
        begin(&mut context);
        for (slot, &p) in poses.0.iter().enumerate() {
            present(&mut context, slot, p);
        }
    }
    #[test]
    fn actual_ecs_consumer_uses_model_anchor_slot_fade_and_independent_lights() {
        let mut app = App::new();
        app.insert_resource(environment(Mode::ForcedNight));
        let state = crate::vehicle_lights::State::new(Mode::ForcedNight, 3, Some(false));
        let old = state.headlights.clone();
        app.insert_resource(state);
        let models = vec![(Vec3::Z, "authored-control"), (Vec3::X, "authored-control")];
        let a = Mat4::from_translation(Vec3::Y);
        let b = Mat4::from_translation(Vec3::X * 5.);
        install(
            &mut app,
            fixture_preparation(models, vec![Some(a), Some(b)], vec![0, 1, 0], 123.),
            Mode::ForcedNight,
        )
        .unwrap();
        app.insert_resource(Poses(vec![
            Mat4::from_translation(Vec3::Z * 3.),
            Mat4::from_translation(Vec3::Y * 7.),
            Mat4::IDENTITY,
        ]))
        .add_systems(Update, consume);
        app.update();
        let pilot = app.world().resource::<Pilot>();
        let e0 = pilot.slots[0].entity;
        let e1 = pilot.slots[1].entity;
        let e2 = pilot.slots[2].entity;
        assert_eq!(
            app.world().get::<Transform>(e0).unwrap().translation,
            Vec3::new(0., 1., 4.)
        );
        assert_eq!(
            app.world().get::<Transform>(e1).unwrap().translation,
            Vec3::new(6., 7., 0.)
        );
        assert_eq!(
            app.world().get::<Transform>(e2).unwrap().translation,
            Vec3::new(0., 1., 1.)
        );
        assert_eq!(app.world().get::<SpotLight>(e1).unwrap().intensity, 123.);
        assert!(
            !app.world()
                .get::<SpotLight>(e1)
                .unwrap()
                .shadow_maps_enabled
        );
        assert!(app.world().get::<ChildOf>(e0).is_none());
        assert!(
            app.world()
                .get::<Transform>(e0)
                .unwrap()
                .forward()
                .dot(Vec3::new(0., -0.8, 0.9962).normalize())
                > 0.9999
        );
        app.update();
        assert_eq!(
            app.world()
                .resource::<crate::vehicle_lights::State>()
                .headlights,
            old,
            "presentation never advances clock"
        );
        app.world_mut()
            .insert_resource(crate::vehicle_lights::State::new(Mode::None, 3, Some(true)));
        app.update();
        assert_eq!(
            app.world().get::<SpotLight>(e0).unwrap().intensity,
            0.,
            "marker-only ordinary day contributes no projector"
        );
        app.world_mut().resource_mut::<Pilot>().mode = Mode::Timeline;
        let mut timeline = crate::vehicle_lights::State::new(Mode::Timeline, 3, Some(false));
        timeline.advance(0, 1000, true, 16.);
        app.world_mut().insert_resource(timeline);
        app.update();
        assert_eq!(app.world().get::<SpotLight>(e0).unwrap().intensity, 123.);
        assert!(
            app.world()
                .get::<Transform>(e0)
                .unwrap()
                .forward()
                .dot(Vec3::new(0., -0.45, 0.9962).normalize())
                > 0.9999
        );
        app.world_mut()
            .resource_mut::<crate::vehicle_lights::State>()
            .advance(0, 3000, false, 48.);
        app.update();
        assert_eq!(app.world().get::<SpotLight>(e0).unwrap().intensity, 0.);
        app.world_mut()
            .resource_mut::<crate::vehicle_lights::State>()
            .advance(1, 0, true, 0.);
        app.update();
        app.world_mut()
            .resource_mut::<crate::vehicle_lights::State>()
            .advance(2, 0, true, 0.);
        app.world_mut().resource_mut::<Poses>().0[0] = Mat4::from_translation(Vec3::X * 9.);
        app.update();
        assert_eq!(
            app.world().get::<Transform>(e0).unwrap().translation,
            Vec3::new(9., 1., 1.)
        );
        assert_eq!(app.world().get::<SpotLight>(e0).unwrap().intensity, 123.);
    }
    #[test]
    fn actual_ecs_consumer_rotates_noncommuting_model_anchor_and_source_frame() {
        let mut app = App::new();
        app.add_plugins(bevy::transform::TransformPlugin);
        app.insert_resource(environment(Mode::ForcedNight));
        let state = crate::vehicle_lights::State::new(Mode::ForcedNight, 1, Some(false));
        let original_activation = state.headlights.clone();
        app.insert_resource(state);
        let mut preparation = fixture_preparation(
            vec![(Vec3::new(1., 2., 3.), "authored-noncommuting-frame-control")],
            vec![None],
            vec![0],
            123.,
        );
        preparation
            .retain_anchor(0, Mat4::from_translation(Vec3::new(4., -1., 2.)))
            .unwrap();
        install(&mut app, preparation, Mode::ForcedNight).unwrap();
        let pose = Mat4::from_rotation_translation(
            Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
            Vec3::new(10., 20., 30.),
        );
        app.insert_resource(Poses(vec![pose]))
            .add_systems(Update, consume);
        app.update();
        let entity = app.world().resource::<Pilot>().slots[0].entity;
        let transform = app.world().get::<Transform>(entity).unwrap();
        // Hand-derived numeric oracle, not the implementation's matrix product:
        // C=(1,2,3), M translates (4,-1,2), so M*C=(5,1,5).
        // P rotates +90deg aboutY: (x,y,z)->(z,y,-x), then adds (10,20,30).
        // Thus P*M*C=(15,21,25); reversed M*P*C would be (17,21,31).
        let expected_position = Vec3::new(15., 21., 25.);
        assert!(transform.translation.distance(expected_position) < 0.00001);
        assert!(
            app.world()
                .get::<GlobalTransform>(entity)
                .unwrap()
                .translation()
                .distance(expected_position)
                < 0.00001
        );
        // ForcedNight D=(0,-.8,.9962)/sqrt(.8^2+.9962^2).
        // The same +90degY maps D to (+.779706803,-.626144793,0),
        // and U=(0,D.z,-D.y) to (+.626144793,+.779706803,0).
        // Bevy's spotlight points along Transform.forward() (-localZ).
        let expected_forward = Vec3::new(0.7797068, -0.6261448, 0.);
        let expected_up = Vec3::new(0.6261448, 0.7797068, 0.);
        assert!(transform.forward().as_vec3().distance(expected_forward) < 0.000002);
        assert!(transform.up().as_vec3().distance(expected_up) < 0.000002);
        assert_eq!(
            app.world().get::<SpotLight>(entity).unwrap().intensity,
            123.
        );
        assert_eq!(
            *app.world().get::<Visibility>(entity).unwrap(),
            Visibility::Inherited
        );
        assert_eq!(
            app.world()
                .resource::<crate::vehicle_lights::State>()
                .headlights,
            original_activation
        );
    }

    #[test]
    fn real_replay_lifecycle_late_update_suppresses_only_live_pilot_before_propagation() {
        use crate::replay::{self, frame, stream};
        #[derive(Clone, Copy)]
        enum Action {
            Idle,
            Start,
            Freeze,
            Finish,
        }
        #[derive(Resource)]
        struct Transition(Action);
        #[derive(Resource, Default)]
        struct BeforeGuard(Vec<(f32, Visibility)>);
        #[derive(Resource, Default)]
        struct AfterGuard(Vec<(f32, Visibility, Vec3)>);
        fn transition_late(
            mut replay: ResMut<replay::Replay>,
            mut transition: ResMut<Transition>,
            lights: Query<(&SpotLight, &Visibility), With<Projector>>,
            mut before: ResMut<BeforeGuard>,
        ) {
            let (light, visibility) = lights.single().unwrap();
            before.0.push((light.intensity, *visibility));
            match std::mem::replace(&mut transition.0, Action::Idle) {
                Action::Idle => {}
                Action::Start => assert!(replay.start(
                    replay::Kind::Instant,
                    replay::Origin::Race,
                    false,
                    &crate::camera::Views(vec![])
                )),
                Action::Freeze => {
                    let time = replay.session.as_ref().unwrap().player.time;
                    replay.freeze(time);
                }
                Action::Finish => assert_eq!(replay.finish(), Some(replay::Origin::Race)),
            }
        }
        fn observe_final(
            lights: Query<(&SpotLight, &Visibility, &GlobalTransform), With<Projector>>,
            mut after: ResMut<AfterGuard>,
        ) {
            let (light, visibility, global) = lights.single().unwrap();
            after
                .0
                .push((light.intensity, *visibility, global.translation()));
        }
        let mut stream = stream::Stream::new(stream::Limits::default(), None);
        let mut encoder = frame::Encoder::default();
        for (number, tick) in [0, 2].into_iter().enumerate() {
            let snapshot = frame::Snapshot {
                tick,
                cars: vec![],
                props: None,
            };
            assert!(stream.push(tick, &encoder.encode(&snapshot, number as u32)));
        }
        let mut replay = replay::Replay::new(stream, None);
        assert!(
            replay.snapshot(0).unwrap().cars.is_empty(),
            "real encoded frame, not placeholder bytes"
        );
        let mut app = App::new();
        app.add_plugins(bevy::transform::TransformPlugin);
        app.insert_resource(environment(Mode::ForcedNight));
        let state = crate::vehicle_lights::State::new(Mode::ForcedNight, 1, Some(false));
        let original_activation = state.headlights.clone();
        app.insert_resource(state);
        install(
            &mut app,
            fixture_preparation(
                vec![(Vec3::Z, "authored-lifecycle-control")],
                vec![Some(Mat4::from_translation(Vec3::Y * 2.))],
                vec![0],
                123.,
            ),
            Mode::ForcedNight,
        )
        .unwrap();
        app.insert_resource(replay)
            .insert_resource(Poses(vec![Mat4::IDENTITY]))
            .insert_resource(Transition(Action::Idle))
            .init_resource::<BeforeGuard>()
            .init_resource::<AfterGuard>()
            .add_systems(Update, (consume, transition_late).chain())
            .add_systems(
                PostUpdate,
                observe_final
                    .after(replay_guard)
                    .after(TransformSystems::Propagate),
            );
        app.update();
        assert_eq!(
            app.world().resource::<AfterGuard>().0[0],
            (123., Visibility::Inherited, Vec3::new(0., 2., 1.))
        );
        app.world_mut().resource_mut::<Transition>().0 = Action::Start;
        app.world_mut().resource_mut::<Poses>().0[0] = Mat4::from_translation(Vec3::X * 4.);
        app.update();
        let session = app
            .world()
            .resource::<replay::Replay>()
            .session
            .as_ref()
            .unwrap();
        assert!(
            session.shown.is_none(),
            "real newly started session has no shown pose; guard must still own it"
        );
        assert_eq!(
            app.world().resource::<AfterGuard>().0[1],
            (0., Visibility::Hidden, Vec3::new(4., 2., 1.))
        );
        app.world_mut().resource_mut::<Transition>().0 = Action::Freeze;
        app.world_mut().resource_mut::<Poses>().0[0] = Mat4::from_translation(Vec3::X * 8.);
        app.update();
        assert_eq!(
            app.world()
                .resource::<replay::Replay>()
                .session
                .as_ref()
                .unwrap()
                .player
                .mode,
            replay::player::Mode::Pause
        );
        assert_eq!(
            app.world().resource::<AfterGuard>().0[2],
            (0., Visibility::Hidden, Vec3::new(8., 2., 1.))
        );
        app.update();
        assert_eq!(app.world().resource::<AfterGuard>().0[3].0, 0.);
        app.world_mut().resource_mut::<Transition>().0 = Action::Finish;
        app.world_mut().resource_mut::<Poses>().0[0] = Mat4::from_translation(Vec3::X * 12.);
        app.update();
        assert!(app.world().resource::<replay::Replay>().session.is_none());
        assert_eq!(
            app.world().resource::<AfterGuard>().0[4],
            (123., Visibility::Inherited, Vec3::new(12., 2., 1.))
        );
        app.world_mut().resource_mut::<Poses>().0[0] = Mat4::from_translation(Vec3::X * 16.);
        app.update();
        assert_eq!(
            app.world().resource::<AfterGuard>().0[5],
            (123., Visibility::Inherited, Vec3::new(16., 2., 1.))
        );
        assert!(
            app.world()
                .resource::<BeforeGuard>()
                .0
                .iter()
                .all(|&(intensity, visibility)| intensity == 123.
                    && visibility == Visibility::Inherited),
            "positive live consumer ran before every late replay transition; guard was not vacuous"
        );
        assert_eq!(
            app.world()
                .resource::<crate::vehicle_lights::State>()
                .headlights,
            original_activation,
            "presentation/replay lifecycle must not advance the shared clock"
        );
    }
}
