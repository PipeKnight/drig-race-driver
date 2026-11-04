//! Live-only source-projector presentation using shared headlight geometry.
//! Geometry is owned by shared headlight_frame. No extra clock or parser.
use crate::{
    headlight_frame::{Frame, Prepared},
    source_projector::{self, Parameters},
};
use bevy::{prelude::*, transform::TransformSystems};
use grid_tools::time_of_day::Mode;

#[derive(Component)]
pub(crate) struct PresentedProjector;
#[derive(Resource)]
pub(crate) struct Presentation {
    geometry: Prepared,
    entities: Vec<Entity>,
    mode: Mode,
}
pub(crate) type Lights<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut SpotLight,
        &'static mut Transform,
        &'static mut Visibility,
    ),
    (
        With<PresentedProjector>,
        Without<crate::headlight_pilot::Projector>,
        Without<crate::MovingPart>,
        Without<crate::OpponentPart>,
        Without<crate::ViewerCamera>,
    ),
>;
pub(crate) type Context<'w, 's> = (
    Option<Res<'w, Presentation>>,
    Option<Res<'w, crate::vehicle_lights::State>>,
    Lights<'w, 's>,
);

/// The caller must choose explicit immutable RGB/U and declared range.
/// No normal default, whitelist, player-slot focus guess or physical lumens.
pub(crate) fn install(
    app: &mut App,
    geometry: Prepared,
    image: Handle<Image>,
    mode: Mode,
    range: f32,
    params: Parameters,
) -> Result<(), String> {
    let params = params.validate()?;
    if !range.is_finite() || range <= 0. {
        return Err("declared projector range must be finite positive".into());
    }
    let slots = geometry.len();
    let environment = app
        .world()
        .get_resource::<crate::track_environment::TrackEnvironment>()
        .ok_or("projector requires actual source environment")?;
    let state = app
        .world()
        .get_resource::<crate::vehicle_lights::State>()
        .ok_or("projector requires existing activation State")?;
    if slots == 0
        || slots != state.headlights.delays().len()
        || environment.0.mode() != mode
        || state.marker_lights != Some(environment.0.vehicle_lights())
    {
        return Err("aggregate projector context/slots mismatch".into());
    }
    if app.world().contains_resource::<Presentation>()
        || app.world().contains_resource::<Parameters>()
    {
        return Err("aggregate projector duplicate installation".into());
    }
    // CPU transaction preflight. Device/pipeline readiness is later admission,
    // never inferred from these checks or a successful Assets insertion.
    if app.get_sub_app(bevy::render::RenderApp).is_none()
        || app.is_plugin_added::<source_projector::SourceProjectorPlugin>()
    {
        return Err("aggregate projector requires unclaimed actual RenderApp".into());
    }
    if app
        .world()
        .get_resource::<Assets<Image>>()
        .and_then(|images| images.get(image.id()))
        .is_none()
    {
        return Err("aggregate projector actual cookie Image missing".into());
    }
    let mut entities = Vec::with_capacity(slots);
    for _ in 0..slots {
        let entity = app
            .world_mut()
            .spawn((
                PresentedProjector,
                SpotLight {
                    intensity: 0.,
                    range,
                    shadow_maps_enabled: false,
                    contact_shadows_enabled: false,
                    ..default()
                },
                Transform::IDENTITY,
                Visibility::Hidden,
            ))
            .id();
        entities.push(entity);
        if let Err(error) = source_projector::attach(app.world_mut(), entity, image.clone(), range)
        {
            for spawned in entities {
                app.world_mut().despawn(spawned);
            }
            return Err(error);
        }
    }
    app.insert_resource(params)
        .insert_resource(Presentation {
            geometry,
            entities,
            mode,
        })
        .add_plugins(source_projector::SourceProjectorPlugin)
        .add_systems(
            PostUpdate,
            replay_guard
                .after(crate::replay::present)
                .before(TransformSystems::Propagate),
        );
    println!(
        "source_projector_present slots={slots} range={range} range_policy=AUTHORED-explicit-all-slots focus_identity=false snapshot_colour=true same_activation_clock=true"
    );
    Ok(())
}
fn dark(light: &mut SpotLight, visibility: &mut Visibility) {
    light.intensity = 0.;
    *visibility = Visibility::Hidden;
}
pub(crate) fn begin(context: &mut Context<'_, '_>) {
    if context.0.is_some() {
        for (mut light, _, mut v) in &mut context.2 {
            dark(&mut light, &mut v);
        }
    }
}
fn apply(
    frame: Option<Frame>,
    weight: Option<f64>,
    light: &mut SpotLight,
    t: &mut Transform,
    v: &mut Visibility,
) -> Result<(), String> {
    dark(light, v);
    let (Some(frame), Some(weight)) = (frame, weight) else {
        return Ok(());
    };
    if !weight.is_finite() || !(0. ..=1.).contains(&weight) {
        return Err("projector activation outside normalized domain".into());
    }
    if weight < 0.001 {
        return Ok(());
    }
    // Source frame validation/math remains in headlight_frame; no second pose implementation.
    *t = Transform::from_translation(frame.origin).looking_to(frame.direction, frame.up);
    light.color = Color::WHITE;
    light.intensity = source_projector::transport_intensity(source_projector::source_mask_weight(
        weight as f32,
    )?)?;
    *v = Visibility::Inherited;
    Ok(())
}
pub(crate) fn present(context: &mut Context<'_, '_>, slot: usize, pose: Mat4) {
    let (presentation, state, lights) = context;
    let Some(presentation) = presentation else {
        return;
    };
    let state = state
        .as_ref()
        .expect("aggregate projector activation State retained");
    assert_eq!(
        presentation.entities.len(),
        state.headlights.delays().len(),
        "aggregate actual activation slots changed"
    );
    let entity = *presentation
        .entities
        .get(slot)
        .expect("aggregate actual presentation slot exists");
    let frame = presentation
        .geometry
        .frame(slot, pose, presentation.mode)
        .expect("shared source frame input valid");
    let (mut light, mut transform, mut visibility) = lights
        .get_mut(entity)
        .expect("aggregate projector entity retained");
    apply(
        frame,
        state.headlights.car(slot),
        &mut light,
        &mut transform,
        &mut visibility,
    )
    .expect("finite shared projector presentation");
}
fn replay_guard(replay: Option<Res<crate::replay::Replay>>, mut lights: Lights) {
    if replay.is_some_and(|r| r.session.is_some()) {
        for (mut light, _, mut v) in &mut lights {
            dark(&mut light, &mut v);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn frame() -> Frame {
        Frame {
            origin: Vec3::new(1., 2., 3.),
            direction: Vec3::NEG_Z,
            up: Vec3::Y,
        }
    }
    #[test]
    fn hidden_zero_on_missing_inactive_and_invalid_before_return() {
        for w in [None, Some(0.), Some(0.0005), Some(f64::NAN)] {
            let mut light = SpotLight {
                intensity: 777.,
                ..default()
            };
            let mut t = Transform::IDENTITY;
            let mut v = Visibility::Inherited;
            let _ = apply(Some(frame()), w, &mut light, &mut t, &mut v);
            assert_eq!(light.intensity, 0.);
            assert_eq!(v, Visibility::Hidden);
        }
    }
    #[test]
    fn real_transport_pose_then_reset_hide_without_second_timer() {
        let mut light = SpotLight::default();
        let mut t = Transform::IDENTITY;
        let mut v = Visibility::Hidden;
        apply(Some(frame()), Some(1.), &mut light, &mut t, &mut v).unwrap();
        assert_eq!(t.translation, Vec3::new(1., 2., 3.));
        assert_eq!(v, Visibility::Inherited);
        assert!((light.intensity / (4. * std::f32::consts::PI) - 2.5).abs() < 1e-6);
        apply(None, Some(1.), &mut light, &mut t, &mut v).unwrap();
        assert_eq!(light.intensity, 0.);
        assert_eq!(v, Visibility::Hidden);
    }
    #[test]
    fn lifecycle_dark_only_touches_owned_projector_query() {
        let mut world = World::new();
        let owned = world
            .spawn((
                PresentedProjector,
                SpotLight {
                    intensity: 10.,
                    ..default()
                },
                Transform::IDENTITY,
                Visibility::Inherited,
            ))
            .id();
        let other = world
            .spawn((
                SpotLight {
                    intensity: 20.,
                    ..default()
                },
                Transform::IDENTITY,
                Visibility::Inherited,
            ))
            .id();
        let mut q =
            world.query_filtered::<(&mut SpotLight, &mut Visibility), With<PresentedProjector>>();
        for (mut light, mut v) in q.iter_mut(&mut world) {
            dark(&mut light, &mut v);
        }
        assert_eq!(world.get::<SpotLight>(owned).unwrap().intensity, 0.);
        assert_eq!(world.get::<SpotLight>(other).unwrap().intensity, 20.);
    }
    #[test]
    fn actual_pilot_and_aggregate_contexts_register_together_without_b0001() {
        let mut world = World::new();
        world.spawn((
            crate::headlight_pilot::Projector,
            SpotLight::default(),
            Transform::IDENTITY,
            Visibility::Hidden,
        ));
        world.spawn((
            PresentedProjector,
            SpotLight::default(),
            Transform::IDENTITY,
            Visibility::Hidden,
        ));
        let mut state = bevy::ecs::system::SystemState::<(
            crate::headlight_pilot::Context<'_, '_>,
            Context<'_, '_>,
        )>::new(&mut world);
        let (mut pilot, mut aggregate) = state.get_mut(&mut world).unwrap();
        // No Pilot/Presentation resource needed: this verifies actual query registration.
        crate::headlight_pilot::begin(&mut pilot);
        begin(&mut aggregate);
    }
}

#[cfg(test)]
mod actual_preflight_transaction_tests {
    use super::*;
    fn prepared() -> Prepared {
        crate::headlight_frame::Preparation::fixture(
            vec![(Vec3::ZERO, "synthetic-geometry")],
            vec![Some(Mat4::IDENTITY)],
            vec![0],
        )
        .finish()
        .unwrap()
    }
    fn params() -> Parameters {
        Parameters {
            working_unit: 2.,
            global_colour: Vec3::ONE,
        }
    }
    fn app(render: bool, cookie: bool, slots: usize) -> (App, Handle<Image>, Entity) {
        let mut app = App::new();
        app.insert_resource(crate::track_environment::TrackEnvironment(
            grid_tools::environment::Environment {
                lighting_file: "synthetic-preflight".into(),
                lighting: Default::default(),
                weather_fx: Default::default(),
                flags: grid_tools::environment::TrackFlags {
                    headlights_on: true,
                    ..Default::default()
                },
            },
        ));
        app.insert_resource(crate::vehicle_lights::State::new(
            Mode::ForcedNight,
            slots,
            Some(false),
        ));
        app.insert_resource(Assets::<Image>::default());
        let image = if cookie {
            app.world_mut()
                .resource_mut::<Assets<Image>>()
                .add(Image::default())
        } else {
            Handle::default()
        };
        if render {
            app.insert_sub_app(bevy::render::RenderApp, bevy::app::SubApp::new());
        }
        let sentinel = app
            .world_mut()
            .spawn((
                Name::new("unrelated light"),
                SpotLight {
                    intensity: 17.,
                    ..default()
                },
                Transform::IDENTITY,
                Visibility::Inherited,
            ))
            .id();
        (app, image, sentinel)
    }
    fn unchanged(app: &App, sentinel: Entity, count: u32, had_params: bool) {
        assert_eq!(app.world().entities().len(), count);
        assert_eq!(
            app.world().get::<SpotLight>(sentinel).unwrap().intensity,
            17.
        );
        assert_eq!(
            *app.world().get::<Visibility>(sentinel).unwrap(),
            Visibility::Inherited
        );
        assert!(!app.world().contains_resource::<Presentation>());
        assert_eq!(app.world().contains_resource::<Parameters>(), had_params);
        assert!(!app.is_plugin_added::<source_projector::SourceProjectorPlugin>());
    }
    #[test]
    fn actual_install_preflight_failures_leave_entities_and_resources_unchanged() {
        // Each case reaches actual install with a real shared Prepared fixture.
        // It performs no GPU/device/plugin startup work, as preflight refuses first.
        for case in 0..6 {
            let (mut app, image, sentinel) =
                app(case != 2, case != 3, if case == 4 { 2 } else { 1 });
            if case == 5 {
                app.insert_resource(params());
            }
            let before = app.world().entities().len();
            let had = case == 5;
            let units = if case == 0 {
                Parameters {
                    working_unit: f32::NAN,
                    ..params()
                }
            } else {
                params()
            };
            let range = if case == 1 { 0. } else { 10. };
            let result = install(&mut app, prepared(), image, Mode::ForcedNight, range, units);
            assert!(result.is_err(), "preflight case={case}");
            unchanged(&app, sentinel, before, had);
            assert_eq!(
                app.world()
                    .resource::<crate::vehicle_lights::State>()
                    .headlights
                    .delays()
                    .len(),
                if case == 4 { 2 } else { 1 }
            );
        }
    }
    #[test]
    fn actual_install_missing_environment_refuses_without_entity_mutation() {
        let (mut app, image, sentinel) = app(true, true, 1);
        app.world_mut()
            .remove_resource::<crate::track_environment::TrackEnvironment>();
        let before = app.world().entities().len();
        assert!(
            install(
                &mut app,
                prepared(),
                image,
                Mode::ForcedNight,
                10.,
                params()
            )
            .is_err()
        );
        unchanged(&app, sentinel, before, false);
    }
}
