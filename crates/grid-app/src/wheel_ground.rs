//! Per-wheel ground state shared by the ground effects (skid marks, wheel
//! debris). Sampled once per successful physics tick for every car from the
//! source wheel records; presentation only, never feeds physics.
//!
//! Wheel record `body+1990+i*150` (TYRE_AUDIO_RESEARCH.md): `+C0` contact
//! material token, `+E0` radius, `+104` angular speed, `+138` load, `+AC`
//! smoothed excess force, `+F8` tyre width.
use bevy::prelude::*;
use grid_tools::source_vehicle::Runtime;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Wheel {
    /// Wheel touches the ground (non-zero source material token).
    pub contact: bool,
    /// Four-character code of the wheel's source contact material.
    pub code: [u8; 4],
    /// Ground point under the wheel centre and its upward unit normal.
    pub point: Vec3,
    pub normal: Vec3,
    /// Rolling direction of the steered wheel, tangent to the ground.
    pub forward: Vec3,
    /// Contact-patch velocity (car velocity plus rotation), world m/s.
    pub velocity: Vec3,
    /// Wheel surface speed minus ground speed along `forward` (m/s; positive
    /// is wheelspin, negative is a locked or braking wheel).
    pub longitudinal: f32,
    /// Ground speed across the wheel (m/s, unsigned).
    pub lateral: f32,
    pub radius: f32,
    /// Tyre width (wheel `+F8`, from the car's `.ned` wheel widths).
    pub width: f32,
    /// Smoothed excess wheel force (wheel `+AC`) divided by body mass (`+15A0`);
    /// the original lays skid layers above 2.0 (73F860).
    pub excess: f32,
    /// 1 - |cos| of the angle between wheel heading and contact velocity.
    pub side_factor: f32,
    /// Source wheel order rear L/R, front L/R; left wheels have anchor x < 0.
    pub rear: bool,
    pub left: bool,
    /// Wheel load (wheel `+138`, N).
    pub load: f32,
    /// Car speed (m/s), the kick-up speed-band input.
    pub car_speed: f32,
}

impl Wheel {
    pub fn speed(&self) -> f32 {
        self.velocity.length()
    }
}

/// One entry per car; car 0 is the player.
#[derive(Resource, Default)]
pub(crate) struct WheelGround {
    pub cars: Vec<[Wheel; 4]>,
    /// (segment, tick) of the sample; a segment change means a reset.
    pub key: Option<(u64, u64)>,
    /// Incremented for every new sample, so consumers run once per tick.
    pub generation: u64,
    pub reset: bool,
}

/// The recorded state of one car that [`compose`] reads: what the race
/// samples live, and what a replay frame holds (crate::replay::frame).
pub(crate) struct Source<'a> {
    pub pose: Mat4,
    pub velocity: Vec3,
    pub angular_velocity: Vec3,
    /// Per wheel: anchor, axis, displacement and steering (WheelPose words).
    pub anchor: [[f32; 3]; 4],
    pub axis: [[f32; 3]; 4],
    pub displacement: [f32; 4],
    pub steering: [[f32; 2]; 4],
    pub effects: &'a crate::replay::frame::Effects,
}

pub(crate) fn sample(runtime: &Runtime) -> [Wheel; 4] {
    let effects = crate::replay::effects(runtime);
    let v = runtime.velocity();
    let poses = runtime.wheels();
    compose(
        &Source {
            pose: Mat4::from_cols_array(&runtime.pose()),
            velocity: Vec3::new(v[0], v[1], v[2]),
            angular_velocity: effects.angular_velocity,
            anchor: poses.map(|w| w.anchor),
            axis: poses.map(|w| w.axis),
            displacement: poses.map(|w| w.displacement),
            steering: poses.map(|w| w.steering),
            effects: &effects,
        },
        |token| runtime.surfaces().code(token),
    )
}

/// The wheel ground state of a replayed car (crate::replay_effects).
pub(crate) fn replayed(
    car: &crate::replay::frame::Car,
    code: impl Fn(u32) -> Option<[u8; 4]>,
) -> [Wheel; 4] {
    compose(
        &Source {
            pose: car.pose.to_mat4(),
            velocity: car.velocity,
            angular_velocity: car.effects.angular_velocity,
            anchor: car.geometry.map(|g| g.anchor),
            axis: car.geometry.map(|g| g.axis),
            displacement: car.wheels.map(|w| w.displacement),
            steering: car.wheels.map(|w| w.steering),
            effects: &car.effects,
        },
        code,
    )
}

pub(crate) fn compose(source: &Source, code: impl Fn(u32) -> Option<[u8; 4]>) -> [Wheel; 4] {
    let pose = source.pose;
    let origin = pose.w_axis.truncate();
    let velocity = source.velocity;
    let spin = source.angular_velocity;
    let mass = source.effects.mass;
    std::array::from_fn(|i| {
        let e = &source.effects.wheels[i];
        let token = e.token;
        let radius = e.radius;
        let omega = e.omega;
        let local = Vec3::from_array(source.anchor[i])
            + Vec3::from_array(source.axis[i]) * source.displacement[i];
        // Mark/emitter point: the wheel centre lowered by its radius in the
        // car frame, then to world, like 73F860 (no track probe) [D].
        let point = pose.transform_point3(local - Vec3::Y * radius);
        let normal = pose.transform_vector3(Vec3::Y).normalize_or(Vec3::Y);
        let yaw = source.steering[i][1].atan2(source.steering[i][0]);
        let heading = pose.transform_vector3(Quat::from_rotation_y(yaw) * Vec3::Z);
        let forward = (heading - normal * heading.dot(normal)).normalize_or(heading);
        let side = normal.cross(forward);
        let contact_velocity = velocity + spin.cross(point - origin);
        let code = code(token).unwrap_or(*b"\0\0\0\0");
        let planar = contact_velocity - normal * contact_velocity.dot(normal);
        let side_factor = if planar.length() > 1e-3 {
            1. - planar.normalize().dot(forward).abs().min(1.)
        } else {
            0.
        };
        Wheel {
            contact: token != 0,
            code,
            point,
            normal,
            forward,
            velocity: contact_velocity,
            longitudinal: omega * radius - contact_velocity.dot(forward),
            lateral: contact_velocity.dot(side).abs(),
            radius,
            width: e.width,
            excess: if mass > 0. { e.excess / mass } else { 0. },
            side_factor,
            rear: i < 2,
            left: source.anchor[i][0] < 0.,
            load: e.load,
            car_speed: velocity.length(),
        }
    })
}

pub(crate) fn install(app: &mut App) {
    super::ground_effect_assets::install_timing(app);
    app.init_resource::<WheelGround>()
        .add_systems(FixedUpdate, update.after(super::drive_tick))
        .add_systems(
            Update,
            super::ground_effect_assets::validation_camera.after(super::drive_present),
        );
}

pub(crate) fn update(
    mut ground: ResMut<WheelGround>,
    driving: Res<super::Driving>,
    replay: Option<Res<crate::replay::Replay>>,
) {
    // A replay (and a jump-in's re-run under its frozen session) steps the
    // ground effects itself (crate::replay_effects): no marks bridge the
    // rewound tick.
    if replay.is_some_and(|r| r.session.is_some()) {
        return;
    }
    let key = (driving.segment, driving.runtime.ticks);
    if ground.key == Some(key) {
        return;
    }
    ground.reset = ground.key.is_none_or(|(segment, _)| segment != key.0) || key.1 == 0;
    ground.key = Some(key);
    ground.generation += 1;
    let start = std::time::Instant::now();
    let mut cars = vec![sample(&driving.runtime)];
    if let Some(opponents) = &driving.opponents {
        cars.extend(opponents.multi.cars.iter().skip(1).map(sample));
    }
    // Diagnostics ground_trace: player wheel samples every 16 ticks (validation).
    if key.1.is_multiple_of(16) && crate::settings::diagnostics().ground_trace {
        for (i, w) in cars[0].iter().enumerate() {
            println!(
                "ground_trace tick={} wheel={i} contact={} surface={} speed={:.2} longitudinal={:.2} lateral={:.2} excess={:.2} side={:.2} width={:.3} point={:.2?}",
                key.1,
                w.contact,
                String::from_utf8_lossy(&w.code),
                w.speed(),
                w.longitudinal,
                w.lateral,
                w.excess,
                w.side_factor,
                w.width,
                w.point
            );
        }
    }
    ground.cars = cars;
    super::ground_effect_assets::charge(super::ground_effect_assets::TICK, start);
}
