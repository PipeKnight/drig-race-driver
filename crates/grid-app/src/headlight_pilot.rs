//! Default-off live-only AUTHORED PBR headlight trial. Not source optics parity.
use bevy::{prelude::*, transform::TransformSystems};
use grid_formats::{binxml, pssg, scene};
use grid_tools::{car::Car as CarModel, time_of_day::Mode};
use std::{collections::BTreeMap, path::Path};

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
#[derive(Clone, Debug)]
struct Point {
    value: Vec3,
    provenance: &'static str,
}
#[derive(Clone, Default)]
struct Candidate {
    bounds: Vec<Vec<u8>>,
    transforms: Vec<Vec<u8>>,
}
fn selected_point(n: &Candidate) -> Result<Vec3, String> {
    let ([bounds], [matrix]) = (&n.bounds[..], &n.transforms[..]) else {
        return Err("selected lamp requires one bounds and transform".into());
    };
    if bounds.len() != 24 {
        return Err("selected lamp bounds must be six floats".into());
    }
    let b: [f32; 6] =
        std::array::from_fn(|i| f32::from_be_bytes(bounds[i * 4..i * 4 + 4].try_into().unwrap()));
    if b.iter().any(|x| !x.is_finite()) || (0..3).any(|i| b[i] > b[i + 3]) {
        return Err("invalid selected lamp bounds".into());
    }
    let m = scene::decode_matrix(matrix, 0).map_err(|e| e.to_string())?;
    let value = Vec3::new(
        (b[0] + b[3]) * 0.5 + m[12],
        (b[1] + b[4]) * 0.5 + m[13],
        (b[2] + b[5]) * 0.5 + m[14],
    );
    if !value.is_finite() {
        return Err("selected lamp point overflow".into());
    }
    Ok(value)
}
fn point(nodes: &BTreeMap<String, Vec<Candidate>>, offset: Vec3) -> Result<Point, String> {
    for (kind, left, right) in [
        (
            "canonical-LOD0-glass-pair",
            "x0_light_glass_fl",
            "x0_light_glass_fr",
        ),
        (
            "canonical-LOD0-pod-pair",
            "x0_light_pod_fl",
            "x0_light_pod_fr",
        ),
    ] {
        if let (Some(l), Some(r)) = (nodes.get(left), nodes.get(right)) {
            let ([l], [r]) = (&l[..], &r[..]) else {
                return Err("ambiguous selected lamp pair".into());
            };
            let value = (selected_point(l)? + selected_point(r)?) * 0.5 + offset;
            if !value.is_finite() {
                return Err("lamp midpoint overflow".into());
            }
            return Ok(Point {
                value,
                provenance: kind,
            });
        }
    }
    Ok(Point {
        value: Vec3::new(0., 0.6, 2.2) + offset,
        provenance: "source-original-default-pair",
    })
}
fn offsets(ned: &binxml::Document<'_>) -> Result<Vec3, String> {
    let parts: Vec<_> = ned.elements.iter().filter(|e| e.name == "PARTS").collect();
    if parts.is_empty() {
        return Ok(Vec3::ZERO);
    }
    let [parts] = &parts[..] else {
        return Err("ambiguous PARTS".into());
    };
    let mut result = Vec3::new(0., 0., 0.2);
    for (key, axis) in [("m_light_ray_y_offset", 1), ("m_light_ray_z_offset", 2)] {
        let values: Vec<_> = parts.attributes.iter().filter(|(k, _)| *k == key).collect();
        match &values[..] {
            [] => {}
            [(_, text)] => {
                let v = text
                    .parse::<f32>()
                    .map_err(|_| "invalid headlight offset")?;
                if !v.is_finite() {
                    return Err("nonfinite headlight offset".into());
                }
                result[axis] = v;
            }
            _ => return Err("duplicate consumed headlight offset".into()),
        }
    }
    Ok(result)
}
fn descriptor(tree: &pssg::Tree<'_>, ned: &binxml::Document<'_>) -> Result<Point, String> {
    let mut bundles = Vec::new();
    for i in 0..tree.nodes.len() {
        if tree.name(i) == b"MATRIXPALETTEBUNDLENODE"
            && tree.string(i, b"id").map_err(|e| e.to_string())? == b"LOD0_"
        {
            bundles.push(i);
        }
    }
    let [bundle] = &bundles[..] else {
        return Err("expected one canonical LOD0 bundle".into());
    };
    let mut nodes: BTreeMap<String, Vec<Candidate>> = BTreeMap::new();
    for &i in &tree.nodes[*bundle].children {
        if tree.name(i) != b"MATRIXPALETTEJOINTNODE" {
            continue;
        }
        let id = tree.string(i, b"id").map_err(|e| e.to_string())?;
        let Ok(id) = std::str::from_utf8(id) else {
            continue;
        };
        if !matches!(
            id,
            "x0_light_glass_fl" | "x0_light_glass_fr" | "x0_light_pod_fl" | "x0_light_pod_fr"
        ) {
            continue;
        }
        let mut n = Candidate::default();
        for &c in &tree.nodes[i].children {
            match tree.name(c) {
                b"BOUNDINGBOX" => n
                    .bounds
                    .push(tree.nodes[c].data.unwrap_or_default().to_vec()),
                b"TRANSFORM" => n
                    .transforms
                    .push(tree.nodes[c].data.unwrap_or_default().to_vec()),
                _ => {}
            }
        }
        nodes.entry(id.to_owned()).or_default().push(n);
    }
    point(&nodes, offsets(ned)?)
}
pub struct Preparation {
    models: Vec<Point>,
    anchors: Vec<Option<Mat4>>,
    slots: Vec<usize>,
    lumens: f32,
}
impl Preparation {
    pub fn load(
        root: &Path,
        models: &[CarModel],
        slots: &[usize],
        lumens: f32,
    ) -> Result<Self, String> {
        let models = models
            .iter()
            .map(|model| {
                let high = crate::read(&root.join(model.file("_highLOD.pssg")))
                    .map_err(|e| e.to_string())?;
                let ned = crate::read(&root.join(model.file(".ned"))).map_err(|e| e.to_string())?;
                descriptor(
                    &pssg::read_tree(&high).map_err(|e| e.to_string())?,
                    &binxml::read(&ned).map_err(|e| e.to_string())?,
                )
            })
            .collect::<Result<Vec<_>, String>>()?;
        if slots.is_empty() || slots.iter().any(|&m| m >= models.len()) {
            return Err("invalid actual headlight Field mapping".into());
        }
        Ok(Self {
            anchors: vec![None; models.len()],
            models,
            slots: slots.to_vec(),
            lumens,
        })
    }
    pub fn retain_anchor(&mut self, model: usize, anchor: Mat4) -> Result<(), String> {
        let a = self
            .anchors
            .get_mut(model)
            .ok_or("headlight model missing")?;
        if a.is_some() || !anchor.is_finite() {
            return Err("ambiguous/invalid append anchor".into());
        }
        *a = Some(anchor);
        Ok(())
    }
}
#[derive(Component)]
pub struct Projector;
struct Slot {
    entity: Entity,
    model: usize,
    model_from_source: Mat4,
}
#[derive(Resource)]
pub struct Pilot {
    slots: Vec<Slot>,
    models: Vec<Point>,
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
    require_context(app.world(), mode, preparation.slots.len())?;
    let anchors = preparation
        .anchors
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or("missing actual append anchor")?;
    let mut slots = Vec::new();
    for (slot, &model) in preparation.slots.iter().enumerate() {
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
        slots.push(Slot {
            entity,
            model,
            model_from_source: anchors[model],
        });
    }
    for (model, p) in preparation.models.iter().enumerate() {
        println!(
            "headlight_pilot model={model} point={:?} selection={} scope=canonical-LOD0 runtime8slot_parity=false",
            p.value, p.provenance
        );
    }
    println!(
        "headlight_pilot enabled=true lumens={} adapter=AUTHORED-neutral-PBR source_photometry=false live_only=true shadows=false cookie=false range=source-number-engine-units cone=authored-circular-atan2-inner0.8",
        preparation.lumens
    );
    app.insert_resource(Pilot {
        slots,
        models: preparation.models,
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
    let binding = pilot.slots.get(slot).ok_or("headlight slot absent")?;
    let Some(weight) = weight else {
        return Ok(());
    };
    let y = match pilot.mode {
        Mode::Timeline => -0.45,
        Mode::ForcedNight => -0.8,
        Mode::None => return Ok(()),
    };
    if !pose.is_finite() || !weight.is_finite() || !(0.001..=1.).contains(&weight) {
        return Err("invalid headlight presentation domain".into());
    }
    let local = Vec3::new(0., y, 0.9962).normalize();
    let direction = pose
        .transform_vector3(local)
        .try_normalize()
        .ok_or("invalid headlight direction")?;
    let up = pose
        .transform_vector3(Vec3::new(0., local.z, -local.y))
        .try_normalize()
        .ok_or("invalid headlight up")?;
    let position =
        (pose * binding.model_from_source).transform_point3(pilot.models[binding.model].value);
    if !position.is_finite() {
        return Err("invalid world headlight origin".into());
    }
    *t = Transform::from_translation(position).looking_to(direction, up);
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
    fn candidate(x: f32) -> Candidate {
        Candidate {
            bounds: vec![
                [x, 0., 0., x + 2., 2., 2.]
                    .into_iter()
                    .flat_map(f32::to_be_bytes)
                    .collect(),
            ],
            transforms: vec![
                Mat4::from_rotation_y(1.)
                    .to_cols_array()
                    .into_iter()
                    .flat_map(f32::to_be_bytes)
                    .collect(),
            ],
        }
    }
    fn ned(
        attributes: Vec<(&'static str, &'static str)>,
        present: bool,
    ) -> binxml::Document<'static> {
        binxml::Document {
            strings: vec![],
            string_padding: &[],
            elements: if present {
                vec![binxml::Element {
                    name: "PARTS",
                    value: None,
                    attributes,
                    children: 0..0,
                    parent: None,
                }]
            } else {
                vec![]
            },
        }
    }
    #[test]
    fn real_point_preparation_preserves_atomic_selection_and_offset_domains() {
        let mut n = BTreeMap::from([
            ("x0_light_glass_fl".into(), vec![candidate(0.)]),
            ("x0_light_glass_fr".into(), vec![candidate(2.)]),
            ("x0_light_pod_fl".into(), vec![candidate(8.)]),
            ("x0_light_pod_fr".into(), vec![candidate(10.)]),
        ]);
        let offset = offsets(&ned(vec![], true)).unwrap();
        assert_eq!(point(&n, offset).unwrap().value, Vec3::new(2., 1., 1.2));
        assert_eq!(offsets(&ned(vec![], false)).unwrap(), Vec3::ZERO);
        n.get_mut("x0_light_pod_fl").unwrap()[0].bounds.clear();
        assert!(
            point(&n, offset).is_ok(),
            "unselected malformed pod ignored"
        );
        n.remove("x0_light_glass_fr");
        assert!(
            point(&n, offset).is_err(),
            "selected malformed pod rejected"
        );
        n.insert("x0_light_pod_fl".into(), vec![candidate(8.)]);
        assert_eq!(
            point(&n, offset).unwrap().provenance,
            "canonical-LOD0-pod-pair"
        );
        n.remove("x0_light_pod_fr");
        assert_eq!(
            point(&n, offset).unwrap().provenance,
            "source-original-default-pair"
        );
        assert!(
            offsets(&ned(
                vec![("m_light_ray_z_offset", "0"), ("m_light_ray_z_offset", "1")],
                true
            ))
            .is_err()
        );
        assert!(offsets(&ned(vec![("m_light_ray_y_offset", "NaN")], true)).is_err());
        for invalid in ["0", "-1", "NaN", "inf", "bad"] {
            assert!(parse_intensity(invalid).is_err());
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
        let preparation = || Preparation {
            models: vec![Point {
                value: Vec3::Z,
                provenance: "authored-context-control",
            }],
            anchors: vec![Some(Mat4::IDENTITY)],
            slots: vec![0],
            lumens: 123.,
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
        let models = vec![
            Point {
                value: Vec3::Z,
                provenance: "authored-control",
            },
            Point {
                value: Vec3::X,
                provenance: "authored-control",
            },
        ];
        let a = Mat4::from_translation(Vec3::Y);
        let b = Mat4::from_translation(Vec3::X * 5.);
        install(
            &mut app,
            Preparation {
                models,
                anchors: vec![Some(a), Some(b)],
                slots: vec![0, 1, 0],
                lumens: 123.,
            },
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
        assert!(app.world().get::<Transform>(e0).unwrap().forward().dot(Vec3::new(0.,-0.8,0.9962).normalize())>0.9999);
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
        let mut preparation = Preparation {
            models: vec![Point {
                value: Vec3::new(1., 2., 3.),
                provenance: "authored-noncommuting-frame-control",
            }],
            anchors: vec![None],
            slots: vec![0],
            lumens: 123.,
        };
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
            Preparation {
                models: vec![Point {
                    value: Vec3::Z,
                    provenance: "authored-lifecycle-control",
                }],
                anchors: vec![Some(Mat4::from_translation(Vec3::Y * 2.))],
                slots: vec![0],
                lumens: 123.,
            },
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
    #[test]
    #[ignore = "requires absolute GRID_GAME_ROOT; purchased model readers, CPU only"]
    fn purchased_all_six_canonical_source_descriptors() {
        let root = std::path::PathBuf::from(
            std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required; no skip"),
        );
        assert!(root.is_absolute());
        for (name, expected) in [
            ("lac", [0.0000333, 0.6453644, 2.1526121]),
            ("bmw", [-0.0006400, 0.7822034, 2.0537605]),
            ("stk", [0.0000000, 0.6, 2.4]),
            ("sal", [-0.0000003, 0.524_505, 2.1360629]),
            ("r10", [-0.0000003, 0.9857384, 2.019_38]),
            ("787", [0.0000003, 0.954_859, 1.9609809]),
        ] {
            let model = CarModel::new(name).unwrap();
            let high = crate::read(&root.join(model.file("_highLOD.pssg"))).unwrap();
            let ned = crate::read(&root.join(model.file(".ned"))).unwrap();
            let p = descriptor(
                &pssg::read_tree(&high).unwrap(),
                &binxml::read(&ned).unwrap(),
            )
            .unwrap();
            assert!(p.value.is_finite());
            assert!(
                p.value.distance(Vec3::from_array(expected)) < 0.000002,
                "independent diagnostic59 point: {name}"
            );
            eprintln!(
                "headlight pilot source {name} {:?} {}",
                p.value, p.provenance
            );
        }
    }
}
