//! Dirt, grass, gravel and sand thrown up by the wheels of every car
//! (docs/GROUND_EFFECTS_RESEARCH.md, original kick-up chain 6758B0 -> 6587A0
//! -> 6358B0). Surface -> PARTICLE material (`surface_materials.xml`) ->
//! `<kickup><dry>` system (`effects/particleMaterials.xml`) -> emitters
//! (`effects/particle_systems/*.xml`) and textures (`pfx_textures.pssg`) are
//! read from the purchased installation. Smoke-textured emitters belong to
//! the tyre-smoke presentation (`SMOKE_TEXTURES`) and are skipped here.
//! Presentation only.
use super::{
    particles::{Look, Particle, Store},
    wheel_ground::{Wheel, WheelGround},
};
use bevy::prelude::*;
use grid_formats::{binxml, kickup};
// Emitters using these textures are drawn by the tyre-smoke presentation.
use grid_tools::tyre_smoke::SMOKE_TEXTURES;
use std::{collections::BTreeMap, path::Path};

/// effects/config.xml `totalParticles` [V].
const PARTICLE_LIMIT: usize = 12_000;
/// tunePfx.xml: ignoreDist 400, reductionDist 100, AI factor 0.7 -> 0.2,
/// AI front-wheel factor 0.75 [V data, D use].
const IGNORE_DISTANCE: f32 = 400.;
const REDUCTION_DISTANCE: f32 = 100.;
const AI_NEAR: f32 = 0.7;
const AI_FAR: f32 = 0.2;
const AI_FRONT: f32 = 0.75;
/// vehiclepfxconfig.xml kick-up speed bands (m/s) [V]: static < 4, low
/// 4..10, medium 10..15; the high band keeps the medium multiplier [D 67C590].
const LOW_SPEED: f32 = 4.;
const MEDIUM_SPEED: f32 = 10.;
const HIGH_SPEED: f32 = 15.;
/// Per-axle `lo` of the tyre-slip term, manager constants rear 1.7 / front
/// 3.5 (docs/TYRE_SMOKE_RESEARCH.md 3.2) [V].
const REAR_SLIP: f32 = 1.7;
const FRONT_SLIP: f32 = 3.5;
/// Original particle update 8A9CC0: gravity 0.01*influence*9.81 m/s^2, drag
/// v -= v*dt*0.003*influence*rangeX*0.01 (docs/TYRE_SMOKE_RESEARCH.md 4.2) [V].
const GRAVITY: f32 = 9.81;
const DRAG: f32 = 0.003 * 0.01;

#[derive(Clone, Debug)]
struct EmitterSpec {
    look: Look,
    rate: f32,
    life: f32,
    life_variance: f32,
    speed: f32,
    speed_variation: f32,
    divergence: f32,
    inherit: f32,
    position: Vec3,
    pitch: f32,
    scale_variance: f32,
    initial_rotation: f32,
    spin_variance: f32,
    cumulative: Vec<f32>,
}

#[derive(Clone, Copy, Debug)]
struct Kick {
    player: Option<usize>,
    ai: Option<usize>,
    multipliers: [f32; 4],
    rate: f32,
    speed: f32,
    tyreslip: f32,
}

/// Original speed-band factor B (6358B0/67C590).
fn band(speed: f32, m: [f32; 4]) -> f32 {
    let [_, medium, low, still] = m;
    if speed < LOW_SPEED {
        still * speed / LOW_SPEED
    } else if speed < MEDIUM_SPEED {
        still + (low - still) * (speed - LOW_SPEED) / (MEDIUM_SPEED - LOW_SPEED)
    } else if speed < HIGH_SPEED {
        low + (medium - low) * (speed - MEDIUM_SPEED) / (HIGH_SPEED - MEDIUM_SPEED)
    } else {
        medium
    }
}

/// Original wheel-load factor L (6358B0).
fn load_factor(load: f32) -> f32 {
    if load < 1000. {
        (load / 1000.).max(0.)
    } else if load <= 2500. {
        1.
    } else {
        (1. + (load - 2500.) * 0.002).min(2.)
    }
}

/// Emission rate scale and speed scale of one wheel (6358B0), before the
/// emitter base values: rate = (B*L + S)*rateMult*F*D, speed = (B + S)*speedMult.
fn intensity(wheel: &Wheel, kick: &Kick, front_factor: f32, distance_factor: f32) -> (f32, f32) {
    let b = band(wheel.car_speed, kick.multipliers);
    let lo = if wheel.rear { REAR_SLIP } else { FRONT_SLIP };
    let s = kick.tyreslip * (wheel.excess - lo).max(0.);
    let rate = ((b * load_factor(wheel.load) + s) * kick.rate * front_factor * distance_factor)
        .clamp(0., 5.);
    let speed = ((b + s) * kick.speed).clamp(0., 2.);
    (rate, speed)
}

struct Random(u32);
impl Random {
    fn unit(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(0x41c6_4e6d).wrapping_add(12_345);
        ((self.0 >> 8) & 0xffff) as f32 / 65_536.
    }
    fn signed(&mut self) -> f32 {
        2. * self.unit() - 1.
    }
}

#[derive(Resource)]
pub(crate) struct WheelDebris {
    surfaces: BTreeMap<[u8; 4], Kick>,
    systems: Vec<Vec<EmitterSpec>>,
    store: Store,
    /// Fractional emission per car/wheel/emitter.
    pending: Vec<[Vec<f32>; 4]>,
    random: Random,
    camera: Vec3,
    generation: u64,
    /// Mesh per texture slot, and whether it currently holds particles.
    slots: Vec<(Handle<Mesh>, bool)>,
    pub emitted: u64,
}

struct Loaded {
    surfaces: BTreeMap<[u8; 4], Kick>,
    systems: Vec<Vec<EmitterSpec>>,
    /// Texture name, lit flag, DDS bytes per slot.
    slots: Vec<(String, bool, Vec<u8>)>,
}

fn load(root: &Path) -> Result<Loaded, Box<dyn std::error::Error>> {
    let surface_bytes = super::read(&root.join("surface_materials.xml"))?;
    let surface_doc = binxml::read(&surface_bytes)?;
    let surface_particles = kickup::surface_particles(&surface_doc);
    let materials = kickup::read_materials(&std::fs::read_to_string(
        root.join("effects/particleMaterials.xml"),
    )?)?;
    let mut system_index: BTreeMap<String, usize> = BTreeMap::new();
    let mut systems = Vec::new();
    let mut slot_index: BTreeMap<(String, bool), usize> = BTreeMap::new();
    let mut slots: Vec<(String, bool)> = Vec::new();
    let mut system = |name: &str| -> Result<Option<usize>, Box<dyn std::error::Error>> {
        if name.is_empty() || name.eq_ignore_ascii_case("none") {
            return Ok(None);
        }
        if let Some(&i) = system_index.get(name) {
            return Ok(Some(i));
        }
        if !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
            return Err("invalid particle system name".into());
        }
        let bytes = super::read(&root.join(format!("effects/particle_systems/{name}.xml")))?;
        let doc = binxml::read(&bytes)?;
        let (emitters, found) = kickup::read_systems(&doc)?;
        let spec = found
            .iter()
            .find(|s| s.name == name)
            .ok_or("particle system absent")?;
        let mut specs = Vec::new();
        for birth in &spec.births {
            let e = emitters
                .iter()
                .find(|e| e.id == birth.emitter)
                .ok_or("birth names an absent emitter")?;
            if e.texture.is_empty() || SMOKE_TEXTURES.contains(&e.texture.as_str()) {
                continue;
            }
            let lit = e.particle_type.starts_with("lit");
            let key = (e.texture.clone(), lit);
            let slot = *slot_index.entry(key.clone()).or_insert_with(|| {
                slots.push(key);
                slots.len() - 1
            });
            let mut total = 0.;
            let cumulative = e
                .weights
                .iter()
                .map(|w| {
                    total += w;
                    total
                })
                .collect();
            specs.push(EmitterSpec {
                look: Look {
                    slot,
                    frames: e.frames.max(1),
                    columns: e.columns.max(1),
                    rows: e.rows.max(1),
                    width: e.width,
                    scale_type: e.scale_type,
                    growth: e.growth,
                    alpha: e.alpha,
                    in_time: e.in_time,
                    out_time: e.out_time,
                    colour: e.colour,
                    gravity: GRAVITY * e.gravity / 100.,
                    drag: DRAG * e.drag * e.drag_range,
                    bounce: e.collision.then_some(e.bounce),
                },
                rate: e.rate,
                life: e.life,
                life_variance: e.life_variance / 100.,
                speed: e.speed,
                speed_variation: e.speed_variation,
                divergence: e.divergence.to_radians(),
                inherit: e.inherit,
                position: Vec3::from_array(birth.position),
                pitch: birth.orientation[0].to_radians(),
                // Only emitters without a scale type use the variance [V 8AB3ED].
                scale_variance: if e.scale_type == 0 {
                    e.scale_variance
                } else {
                    0.
                },
                initial_rotation: e.initial_rotation.to_radians(),
                spin_variance: e.spin_variance.to_radians(),
                cumulative,
            });
        }
        systems.push(specs);
        system_index.insert(name.to_owned(), systems.len() - 1);
        Ok(Some(systems.len() - 1))
    };
    let mut surfaces = BTreeMap::new();
    for (code, particle) in surface_particles {
        let Some(k) = materials.iter().find(|m| m.material == particle) else {
            continue;
        };
        let player = system(&k.effect)?;
        let ai = match &k.ai_effect {
            Some(name) => system(name)?,
            None => None,
        };
        if player.is_some() || ai.is_some() {
            surfaces.insert(
                code,
                Kick {
                    player,
                    ai,
                    multipliers: k.multipliers,
                    rate: k.rate,
                    speed: k.speed,
                    tyreslip: k.tyreslip,
                },
            );
        }
    }
    let names: Vec<&str> = slots.iter().map(|(n, _)| n.as_str()).collect();
    let textures =
        super::ground_effect_assets::package(root, "effects/textures/pfx_textures.pssg", &names)?;
    let slots = slots
        .into_iter()
        .map(|(name, lit)| {
            let dds = textures
                .get(&name)
                .ok_or_else(|| format!("kick-up texture {name} absent"))?
                .clone();
            Ok((name, lit, dds))
        })
        .collect::<Result<_, Box<dyn std::error::Error>>>()?;
    Ok(Loaded {
        surfaces,
        systems,
        slots,
    })
}

impl WheelDebris {
    fn emit(&mut self, car: usize, index: usize, wheel: &Wheel, dt: f32) {
        if !wheel.contact {
            return;
        }
        let Some(kick) = self.surfaces.get(&wheel.code).copied() else {
            return;
        };
        let ai = car > 0;
        // AI cars use the AI variant where one exists; AI front wheels never
        // get one, but the player system (no AI variant) plays on all wheels [D].
        let system = match (ai, kick.ai) {
            (false, _) => kick.player,
            (true, Some(_)) if !wheel.rear => None,
            (true, Some(s)) => Some(s),
            (true, None) => kick.player,
        };
        let Some(system) = system else { return };
        let distance = wheel.point.distance(self.camera);
        if ai && distance > IGNORE_DISTANCE {
            return;
        }
        let distance_factor = if ai {
            AI_NEAR + (AI_FAR - AI_NEAR) * (distance / REDUCTION_DISTANCE).min(1.)
        } else {
            1.
        };
        let front_factor = if ai && !wheel.rear { AI_FRONT } else { 1. };
        let (rate, speed_scale) = intensity(wheel, &kick, front_factor, distance_factor);
        let up = wheel.normal;
        let forward = wheel.forward;
        let right = forward.cross(up);
        let count = self.systems[system].len();
        let pending = &mut self.pending[car][index];
        pending.resize(count, 0.);
        for e in 0..count {
            let spec = &self.systems[system][e];
            self.pending[car][index][e] += rate * spec.rate * dt;
            while self.pending[car][index][e] >= 1. {
                self.pending[car][index][e] -= 1.;
                let spec = &self.systems[system][e];
                let r = &mut self.random;
                // Emitter axis: up, tilted toward the rear by the birth pitch;
                // random direction inside the divergence cone [I].
                let axis = (up * spec.pitch.cos() + forward * spec.pitch.sin()).normalize();
                let tangent = axis.cross(right).normalize_or(forward);
                let bitangent = axis.cross(tangent);
                let cone = spec.divergence * r.unit().sqrt();
                let around = std::f32::consts::TAU * r.unit();
                let direction = axis * cone.cos()
                    + (tangent * around.cos() + bitangent * around.sin()) * cone.sin();
                let speed = (spec.speed + spec.speed_variation * r.signed()).max(0.) * speed_scale;
                let pick = r.unit();
                let frame = spec.cumulative.iter().position(|&c| pick < c).unwrap_or(0) as u32;
                let side = if wheel.left { -1. } else { 1. };
                let particle = Particle {
                    look: spec.look,
                    position: wheel.point
                        + right * spec.position.x * side
                        + up * spec.position.y
                        + forward * spec.position.z,
                    velocity: direction * speed + wheel.velocity * spec.inherit,
                    ground: wheel.point.y,
                    age: 0.,
                    life: (spec.life * (1. + spec.life_variance * r.signed())).max(0.02),
                    // Spawn 890A10 draws a whole percent v in +-scaleVariance.
                    scale: 1. + (spec.scale_variance * r.signed()).trunc() / 100.,
                    rotation: spec.initial_rotation * r.unit(),
                    spin: spec.spin_variance * r.signed(),
                    frame: frame.min(spec.look.frames - 1),
                };
                if self.store.push(particle) {
                    self.emitted += 1;
                }
            }
        }
    }
}

pub(crate) fn install(app: &mut App, root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let loaded = load(root)?;
    println!(
        "wheel_debris enabled=true surfaces={} systems={} textures={} limit={PARTICLE_LIMIT} smoke=excluded fidelity=experimental",
        loaded.surfaces.len(),
        loaded.systems.len(),
        loaded.slots.len()
    );
    app.insert_resource(PendingDebris(Some(loaded)))
        .add_systems(Startup, start)
        .add_systems(FixedUpdate, simulate.after(super::wheel_ground::update))
        .add_systems(
            Update,
            present
                .after(super::drive_present)
                .after(super::ground_effect_assets::validation_camera),
        );
    Ok(())
}

#[derive(Resource)]
struct PendingDebris(Option<Loaded>);

fn start(
    mut commands: Commands,
    mut pending: ResMut<PendingDebris>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(loaded) = pending.0.take() else {
        return;
    };
    let empty = Store::new(0);
    let mut slots = Vec::new();
    for (i, (name, lit, dds)) in loaded.slots.iter().enumerate() {
        let image = match super::ground_effect_assets::image(dds, false) {
            Ok(image) => images.add(image),
            Err(error) => {
                warn!("kick-up texture {name}: {error}");
                Handle::default()
            }
        };
        let mesh = meshes.add(empty.build(i, Vec3::X, Vec3::Y));
        let material = materials.add(StandardMaterial {
            base_color_texture: Some(image),
            alpha_mode: AlphaMode::Blend,
            unlit: !lit,
            perceptual_roughness: 1.,
            double_sided: true,
            cull_mode: None,
            ..default()
        });
        commands.spawn((
            Name::new(format!("wheel debris {name}")),
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material),
            Transform::IDENTITY,
            bevy::camera::visibility::NoFrustumCulling,
        ));
        slots.push((mesh, false));
    }
    commands.insert_resource(WheelDebris {
        surfaces: loaded.surfaces,
        systems: loaded.systems,
        store: Store::new(PARTICLE_LIMIT),
        pending: Vec::new(),
        random: Random(0x5eed_d1e5),
        camera: Vec3::ZERO,
        generation: 0,
        slots,
        emitted: 0,
    });
}

fn simulate(
    debris: Option<ResMut<WheelDebris>>,
    ground: Res<WheelGround>,
    camera: Query<&GlobalTransform, With<super::ViewerCamera>>,
) {
    let Some(mut debris) = debris else { return };
    if debris.generation == ground.generation {
        return;
    }
    debris.generation = ground.generation;
    let start = std::time::Instant::now();
    let dt = grid_tools::source_vehicle::DELTA;
    if ground.reset {
        debris.store.particles.clear();
        debris.pending.clear();
    }
    if let Ok(camera) = camera.single() {
        debris.camera = camera.translation();
    }
    debris.store.step(dt);
    debris
        .pending
        .resize_with(ground.cars.len(), Default::default);
    for (car, wheels) in ground.cars.iter().enumerate() {
        for (index, wheel) in wheels.iter().enumerate() {
            debris.emit(car, index, wheel, dt);
        }
    }
    super::ground_effect_assets::charge(super::ground_effect_assets::TICK, start);
    if ground.key.is_some_and(|(_, t)| t.is_multiple_of(16))
        && crate::settings::diagnostics().ground_trace
    {
        println!(
            "debris_trace tick={} emitted={} live={}",
            ground.key.unwrap().1,
            debris.emitted,
            debris.store.particles.len()
        );
    }
}

fn present(
    debris: Option<ResMut<WheelDebris>>,
    camera: Query<&GlobalTransform, With<super::ViewerCamera>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Some(mut debris) = debris else { return };
    let Ok(camera) = camera.single() else { return };
    let start = std::time::Instant::now();
    let (right, up) = (camera.right().as_vec3(), camera.up().as_vec3());
    let debris = &mut *debris;
    for (i, (handle, drawn)) in debris.slots.iter_mut().enumerate() {
        // Rebuild every frame while particles exist (the camera moves), and
        // once more to empty the mesh; an empty slot stays untouched (F392).
        let live = debris.store.particles.iter().any(|p| p.look.slot == i);
        if !live && !*drawn {
            continue;
        }
        *drawn = live;
        let mesh = debris.store.build(i, right, up);
        if let Some(mut target) = meshes.get_mut(handle) {
            *target = mesh;
        }
    }
    super::ground_effect_assets::charge(super::ground_effect_assets::DEBRIS, start);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_bands_load_and_slip() {
        let m = [0.5, 1.0, 0.5, 0.0];
        assert_eq!(band(2., m), 0.);
        assert!((band(7., m) - 0.25).abs() < 1e-6);
        assert!((band(12.5, m) - 0.75).abs() < 1e-6);
        // The high band keeps the medium multiplier.
        assert_eq!(band(30., m), 1.0);
        assert_eq!(load_factor(500.), 0.5);
        assert_eq!(load_factor(2000.), 1.);
        assert_eq!(load_factor(3000.), 2.);
        assert_eq!(load_factor(9000.), 2.);
        let kick = Kick {
            player: Some(0),
            ai: None,
            multipliers: m,
            rate: 1.,
            speed: 1.,
            tyreslip: 1.,
        };
        let wheel = Wheel {
            car_speed: 20.,
            load: 3000.,
            excess: 3.7,
            rear: true,
            ..Default::default()
        };
        // Rear lo 1.7: S = 2.0, rate = B*L + S = 1*2 + 2, speed capped at 2.
        let (rate, speed) = intensity(&wheel, &kick, 1., 1.);
        assert!((rate - 4.0).abs() < 1e-5 && (speed - 2.0).abs() < 1e-6);
        let (rate, _) = intensity(&wheel, &kick, 0.75, 0.2);
        assert!((rate - 0.6).abs() < 1e-5);
        // Front lo 3.5: S = 0.2.
        let front = Wheel {
            rear: false,
            ..wheel
        };
        let (rate, speed) = intensity(&front, &kick, 1., 1.);
        assert!((rate - 2.2).abs() < 1e-5 && (speed - 1.2).abs() < 1e-5);
    }
}
