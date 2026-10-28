//! Opt-in player lamp signals: recovered fades, authored Bevy emission adaptation.
//! Source optics, beam projectors and live sampler gamma are not reconstructed.
use bevy::{asset::RenderAssetUsages, image::ImageSampler, prelude::*};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Off,
    Live,
    Brake,
    Reverse,
    Headlights,
}
impl Mode {
    pub fn parse(s: &str) -> Result<Self, &'static str> {
        match s {
            "off" => Ok(Self::Off),
            "live" => Ok(Self::Live),
            "brake" => Ok(Self::Brake),
            "reverse" => Ok(Self::Reverse),
            "headlights" => Ok(Self::Headlights),
            _ => Err("--lamp-signals needs off, live, brake, reverse or headlights"),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Main,
    Brake,
    Reverse,
}
#[derive(Component)]
pub(crate) struct Section {
    kind: Kind,
    joint: String,
}
pub fn section(material: &str, joint: &str) -> Option<Section> {
    if !matches!(
        material,
        "car0/Head Lights Pod"
            | "car0/Rear Lights Pod"
            | "car0/Head Lights Glass"
            | "car0/Rear Lights Glass"
    ) {
        return None;
    }
    let (_, name) = joint.split_once('_')?;
    let kind = match name {
        "light_pod_fl" | "light_pod_fr" | "light_glass_fl" | "light_glass_fr" | "light_pod_bl"
        | "light_pod_br" => Kind::Main,
        "light_brake_pod_bl"
        | "light_brake_pod_br"
        | "light_brake_glass_bl"
        | "light_brake_glass_br" => Kind::Brake,
        "light_rev_pod_bl" | "light_rev_pod_br" => Kind::Reverse,
        _ => return None,
    };
    Some(Section {
        kind,
        joint: name.to_owned(),
    })
}
#[derive(Resource)]
pub struct Atlas(pub Handle<Image>);
#[derive(Resource, Default)]
struct State {
    fades: Fades,
    last_tick: Option<u64>,
}
impl State {
    fn step_tick(&mut self, ticks: u64, brake: bool, reverse: bool) {
        if self.last_tick.is_some_and(|old| ticks < old) {
            self.fades = Fades::default();
            self.last_tick = None;
        }
        if self.last_tick == Some(ticks) {
            return;
        }
        let dt = ticks.saturating_sub(self.last_tick.unwrap_or(ticks.saturating_sub(1))) as f32
            * grid_tools::source_vehicle::DELTA;
        self.fades.step(dt, [false, brake, reverse], Fades::CAPS);
        self.last_tick = Some(ticks);
    }
}
#[derive(Clone, Copy, Default, Debug)]
struct Fades([f32; 3]);
impl Fades {
    // Source constructor defaults; actual source setters can still change these.
    const CAPS: [f32; 3] = [0.7, 0.7, 1.];
    fn step(&mut self, dt: f32, on: [bool; 3], caps: [f32; 3]) -> [f32; 4] {
        let amount = dt.abs() * 20.;
        for i in 0..3 {
            self.0[i] = (self.0[i] + if on[i] { amount } else { -amount }).clamp(0., caps[i]);
        }
        [
            self.0[0],
            self.0[1],
            self.0[2],
            (self.0[0] + self.0[1]).min(2.),
        ]
    }
    fn value(self, kind: Kind) -> f32 {
        self.0[match kind {
            Kind::Main => 0,
            Kind::Brake => 1,
            Kind::Reverse => 2,
        }]
    }
}
pub fn validate(car: &super::Car) -> Result<(), &'static str> {
    if !car.textures.contains_key("car0/lac_lights_on.tga")
        || car
            .parts
            .iter()
            .filter(|p| section(&p.material, &p.joint).is_some())
            .count()
            != 12
    {
        return Err(
            "--lamp-signals requires all12 purchased Lacetti LOD0 lamp sections and lights_on texture",
        );
    }
    Ok(())
}
pub fn upload(
    car: &super::Car,
    images: &mut Assets<Image>,
    sampler: ImageSampler,
) -> Handle<Image> {
    let image = super::texture_upload::image_linear(
        &car.textures["car0/lac_lights_on.tga"],
        sampler,
        RenderAssetUsages::default(),
    )
    .expect("validated lamp emissive DDS");
    println!(
        "lamp_signal_atlas format={:?} raw_linear=true upload=shared-helper source_sampler_gamma_unverified=true",
        image.texture_descriptor.format
    );
    images.add(image)
}
pub fn install(app: &mut App) {
    app.init_resource::<State>()
        .add_systems(PostStartup, attach.after(super::lamp_materials::attach))
        .add_systems(FixedUpdate, tick.after(super::drive_tick))
        .add_systems(Update, present.after(super::damage_parts::Presented));
}
type PlayerSections = (With<Section>, Without<super::OpponentPart>);
fn attach(
    mut commands: Commands,
    atlas: Res<Atlas>,
    parts: Query<(Entity, &MeshMaterial3d<StandardMaterial>), PlayerSections>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    settings: Res<super::graphics::Settings>,
    mut state: ResMut<State>,
) {
    state.fades = match settings.lamp_signals {
        Mode::Headlights => Fades([0.7, 0., 0.]),
        Mode::Brake => Fades([0., 0.7, 0.]),
        Mode::Reverse => Fades([0., 0., 1.]),
        _ => Fades::default(),
    };
    let mut count = 0;
    for (entity, original) in &parts {
        let mut material = materials
            .get(&original.0)
            .expect("source lamp material")
            .clone();
        material.emissive_texture = Some(atlas.0.clone());
        material.emissive = LinearRgba::BLACK;
        // Authored exposure-independent adaptation, not recovered source radiometry.
        material.emissive_exposure_weight = 0.;
        let handle = materials.add(material);
        commands.entity(entity).insert(MeshMaterial3d(handle));
        count += 1;
    }
    println!(
        "lamp_signals mode={:?} attached_parts={count} scope=player-only caps=[0.7,0.7,1.0] radiometry=authored-exposure-independent source_optics=false live_headlights=daytime-off",
        settings.lamp_signals
    );
}
fn tick(
    driving: Option<Res<super::Driving>>,
    settings: Res<super::graphics::Settings>,
    mut state: ResMut<State>,
) {
    if settings.lamp_signals != Mode::Live {
        return;
    }
    let Some(driving) = driving else {
        return;
    };
    let ticks = driving.runtime.ticks;
    if driving.paused || driving.failure.is_some() {
        return;
    }
    let brake = f32::from_bits(driving.runtime.state.car.owner.vehicle[0xe20 / 4]) > 0.;
    let reverse = driving.runtime.reverse_gear();
    let before = state.fades.0;
    state.step_tick(ticks, brake, reverse);
    if state.fades.0 != before {
        println!(
            "lamp_signal_state tick={ticks} brake={brake} reverse={reverse} levels={:?} source_input=true",
            state.fades.0
        );
    }
}
fn present(
    state: Res<State>,
    damage: Option<Res<super::damage_parts::DamageParts>>,
    parts: Query<(&Section, &MeshMaterial3d<StandardMaterial>), Without<super::OpponentPart>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (section, handle) in &parts {
        let level = if damage
            .as_ref()
            .is_some_and(|d| d.light_smashed(0, &section.joint))
        {
            0.
        } else {
            state.fades.value(section.kind)
        };
        if let Some(mut material) = materials.get_mut(&handle.0) {
            material.emissive = LinearRgba::rgb(level, level, level);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_fades_match_576_original_controls() {
        // Fingerprint of2304float words executed by unchanged source9024C0.
        // Authored controls, empty draw graph; tools/validate_lamp_fades.py.
        let mut hash = 0xcbf29ce484222325u64;
        let mut count = 0;
        for dt in [0., 1. / 300., 1. / 60., 1. / 30., 0.1, -0.016] {
            for start in [
                [0., 0., 0.],
                [0.4, 0.6, 0.8],
                [1., 1., 1.],
                [0.99, 0.01, 0.25],
            ] {
                for caps in [[1., 1., 1.], [0.6, 1.5, 0.8], [0.7, 0.7, 1.]] {
                    for head in [false, true] {
                        for brake in [false, true] {
                            for reverse in [false, true] {
                                let words = Fades(start).step(dt, [head, brake, reverse], caps);
                                for value in words {
                                    for byte in value.to_le_bytes() {
                                        hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
                                    }
                                }
                                count += 1;
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(count, 576);
        assert_eq!(hash, 0xfc3eafff8611bdb9);
    }
    #[test]
    fn section_scope_rejects_other_cars_and_unknown_source_joints() {
        assert!(section("car0/Rear Lights Pod", "x0_light_brake_pod_bl").is_some());
        assert!(section("car1/Rear Lights Pod", "x0_light_brake_pod_bl").is_none());
        assert!(section("car0/Car Body Paint", "x0_light_brake_pod_bl").is_none());
        assert!(section("car0/Rear Lights Pod", "x0_other_light").is_none());
        assert_eq!(Mode::default(), Mode::Off);
        assert!(Mode::parse("on").is_err());
    }
    #[test]
    fn per_section_clones_preserve_optics_and_isolate_shared_opponent() {
        let mut app = App::new();
        app.insert_resource(super::super::graphics::Settings {
            lamp_signals: Mode::Brake,
            ..default()
        })
        .insert_resource(Atlas(Handle::default()))
        .init_resource::<State>()
        .init_resource::<Assets<StandardMaterial>>()
        .add_systems(Update, (attach, present).chain());
        let original = StandardMaterial {
            alpha_mode: AlphaMode::Blend,
            base_color: Color::srgba(0.8, 0.1, 0.1, 0.4),
            normal_map_texture: Some(Handle::default()),
            ..default()
        };
        let base = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(original.clone());
        let mut entities = Vec::new();
        for name in [
            "x0_light_pod_bl",
            "x0_light_brake_pod_bl",
            "x0_light_brake_pod_bl",
        ] {
            entities.push(
                app.world_mut()
                    .spawn((
                        section("car0/Rear Lights Pod", name).unwrap(),
                        MeshMaterial3d(base.clone()),
                    ))
                    .id(),
            );
        }
        app.world_mut()
            .entity_mut(entities[2])
            .insert(super::super::OpponentPart {
                car: 1,
                rest: Mat4::IDENTITY,
                wheel: None,
                lost_with_wheel: false,
            });
        app.update();
        for (i, entity) in entities.into_iter().enumerate() {
            let handle = &app
                .world()
                .get::<MeshMaterial3d<StandardMaterial>>(entity)
                .unwrap()
                .0;
            if i == 2 {
                assert_eq!(handle, &base);
                continue;
            }
            assert_ne!(handle, &base);
            let m = app
                .world()
                .resource::<Assets<StandardMaterial>>()
                .get(handle)
                .unwrap();
            assert_eq!(m.base_color, original.base_color);
            assert_eq!(m.alpha_mode, original.alpha_mode);
            assert_eq!(m.normal_map_texture, original.normal_map_texture);
            assert!(m.emissive_texture.is_some());
            assert_eq!(
                m.emissive,
                if i == 0 {
                    LinearRgba::BLACK
                } else {
                    LinearRgba::rgb(0.7, 0.7, 0.7)
                }
            );
        }
    }
    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn purchased_lacetti_sections_and_emissive_resource() {
        let root = std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required");
        let car = super::super::load(
            std::path::Path::new(&root),
            &grid_tools::route::Route::default(),
            &grid_tools::car::Car::default(),
            "LOD0_",
            false,
            false,
        )
        .unwrap();
        assert!(car.textures.contains_key("lac_lights_on.tga"));
        let count = car
            .parts
            .iter()
            .filter(|p| section(&format!("car0/{}", p.material), &p.joint).is_some())
            .count();
        assert_eq!(count, 12);
    }
    #[test]
    fn simulation_ticks_do_not_double_advance_and_reset_clears_history() {
        let mut state = State::default();
        state.step_tick(0, false, false);
        state.step_tick(1, true, false);
        assert!((state.fades.0[1] - 0.32).abs() < 1e-6);
        let before = state.fades.0;
        state.step_tick(1, false, true);
        assert_eq!(state.fades.0, before);
        state.step_tick(2, true, false);
        state.step_tick(3, true, false);
        assert_eq!(state.fades.0, [0., 0.7, 0.]);
        state.step_tick(0, false, false);
        assert_eq!(state.fades.0, [0.; 3]);
        state.step_tick(1, false, true);
        assert!((state.fades.0[2] - 0.32).abs() < 1e-6);
        assert_eq!(state.fades.0[1], 0.);
    }
}
