//! Race replays ([research](../../../docs/REPLAY.md)): the original records
//! state snapshots, not inputs, and plays them back with interpolation.
//!
//! - Recording: a snapshot of every car (pose, velocity, wheels, wheel
//!   geometry, body deformation, lost wheels) and of the dynamic props
//!   (broken flags, live pieces) at 30 Hz of simulation time [D: the replay
//!   statistics are "@ 30Hz"], encoded into [`stream::Stream`]. Recording
//!   only reads the simulation.
//! - Instant Replay (pause menu, or the Instant Replay key in the race): the
//!   last 10 s [D: 0xDB4FEC, read by StateInstantReplayRewind and 0x63EBC0],
//!   clamped to the in-memory buffer, playing in a loop until Replay Exit.
//! - View Replay (results): the whole recorded race from its start, looping.
//! - Controls (preset 0 / pad preset 4 replay actions): pause, the
//!   rewind/fast-forward shuttle ([`player`]), next/previous camera,
//!   next/previous lap, exit.
//! - Cameras: the route's replay director ([`director`]), then the car's
//!   replay views of `cameras.xml`, then its race views.
//!
//! While a replay plays the simulation is held (`Driving::paused`), the live
//! presentation keeps computing and the replay overwrites it last, so
//! leaving the replay shows the live race again unchanged.
pub mod director;
pub mod frame;
pub mod player;
pub mod stream;
#[cfg(test)]
mod race_tests;

use crate::input::{Action, Actions};
use bevy::{
    camera::primitives::{Aabb, MeshAabb},
    prelude::*,
};
use frame::{Car, Deformation, Encoder, Pose, Props, Shared, Snapshot, Wheel, WheelGeometry};
use grid_tools::source_vehicle::{DELTA, Runtime};

/// [D] sample rate of the original's replay stream.
pub const SAMPLE_HZ: u64 = 30;
/// [D] Instant Replay rewind, seconds (0xDB4FEC).
pub const INSTANT_REWIND: f64 = 10.;
/// [D] the replay interface hides its controls after `timeOut="6.0f"`
/// without input (`ScreenReplayInterface`).
pub const INTERFACE_TIMEOUT: f32 = 6.;

/// A tick is sampled when it starts a new 1/30 s of simulation time; a tick
/// is 16 ms, so `floor(tick * 30 * 0.016) = floor(tick * 12 / 25)`.
pub fn sample_due(previous: Option<u64>, tick: u64) -> bool {
    previous.is_none_or(|p| tick > p && tick * 12 / 25 > p * 12 / 25)
}

pub fn seconds(tick: u64) -> f64 {
    tick as f64 * f64::from(DELTA)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// The last [`INSTANT_REWIND`] seconds, in memory.
    Instant,
    /// The whole recorded race.
    Full,
}

/// Where the replay was started from; Replay Exit returns there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    Race,
    PauseMenu,
    Results,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    Director,
    /// A view of the player's `cameras.xml`.
    View(usize),
}

pub struct Session {
    pub kind: Kind,
    pub origin: Origin,
    pub player: player::Player,
    resume_paused: bool,
    pub cameras: Vec<Choice>,
    pub camera: usize,
    /// The state shown this frame.
    pub shown: Option<Snapshot>,
    /// Frame numbers holding each car's deformation at the replay time, and
    /// those projected into the meshes.
    deformation: Vec<Option<u32>>,
    projected: Option<Vec<Option<u32>>>,
    activation: Option<director::Activation>,
    fov: Option<f32>,
    history: crate::camera::History,
    last_time: Option<f64>,
    hidden: Vec<(Entity, Visibility)>,
    idle: f32,
    laps: Vec<f64>,
}

#[derive(Resource)]
pub struct Replay {
    pub stream: stream::Stream,
    encoder: Encoder,
    recorded: Option<(u64, u64)>,
    /// The last (segment, tick) whose lap events were read.
    seen: Option<(u64, u64)>,
    /// Player lap starts, ticks.
    laps: Vec<u64>,
    pub director: Option<director::Director>,
    /// Director zone entries of the player's car: (tick, zone).
    entries: Vec<(u64, usize)>,
    last_target: Option<Vec3>,
    pub session: Option<Session>,
    request: Option<(Kind, Origin)>,
    /// The replay ended this frame: its exit key belongs to it.
    pub exited: bool,
    /// What the ended replay hid, for `present` to show again.
    ending: Option<Vec<(Entity, Visibility)>>,
}

impl Replay {
    pub fn new(stream: stream::Stream, director: Option<director::Director>) -> Self {
        Self {
            stream,
            encoder: Encoder::default(),
            recorded: None,
            seen: None,
            laps: Vec::new(),
            director,
            entries: Vec::new(),
            last_target: None,
            session: None,
            request: None,
            exited: false,
            ending: None,
        }
    }

    /// Diagnostics `replay_capture`: the offscreen capture waits for the
    /// replay to have played that long.
    pub fn capture_pending(&self) -> bool {
        let Some(seconds) = crate::settings::diagnostics().replay_capture else {
            return false;
        };
        self.session
            .as_ref()
            .is_none_or(|s| s.player.time - s.player.loop_from < seconds)
    }

    /// The menus leave the input alone while this is true.
    pub fn owns_input(&self) -> bool {
        self.session.is_some() || self.exited || self.request.is_some()
    }

    pub fn request(&mut self, kind: Kind, origin: Origin) {
        self.request = Some((kind, origin));
    }

    fn clear(&mut self) {
        self.stream.clear();
        self.encoder = Encoder::default();
        self.recorded = None;
        self.seen = None;
        self.laps.clear();
        self.entries.clear();
        self.last_target = None;
    }

    /// Records the cars and props if `tick` is due (see [`sample_due`]).
    pub fn record(
        &mut self,
        segment: u64,
        runtimes: &[&Runtime],
        props: Option<Props>,
        lost: impl Fn(usize, usize) -> Option<Mat4>,
    ) {
        let Some(player) = runtimes.first() else {
            return;
        };
        let tick = player.ticks;
        if self
            .seen
            .is_some_and(|(s, t)| s != segment || tick < t)
        {
            self.clear();
        }
        // Lap starts of the player (Started, then every Finished).
        if self.seen != Some((segment, tick)) {
            self.seen = Some((segment, tick));
            for event in &player.lap_events {
                use grid_world::lap_timing::Event;
                if matches!(event, Event::Started { .. } | Event::Finished(_)) {
                    self.laps.push(tick);
                }
            }
        }
        if !sample_due(self.recorded.map(|(_, t)| t), tick) || self.stream.full() {
            return;
        }
        let snapshot = Snapshot {
            tick,
            cars: runtimes
                .iter()
                .enumerate()
                .map(|(i, r)| car(r, |w| lost(i, w)))
                .collect(),
            props,
        };
        let target = snapshot.cars[0].pose.translation();
        if let (Some(d), Some(from)) = (&self.director, self.last_target) {
            for zone in d.crossed(from, target) {
                self.entries.push((tick, zone));
            }
        }
        self.last_target = Some(target);
        let number = self.stream.next();
        let bytes = self.encoder.encode(&snapshot, number);
        self.stream.push(tick, &bytes);
        self.encoder.forget_before(self.stream.frames().start as u32);
        self.recorded = Some((segment, tick));
    }

    /// Frame `frame` with its shared items resolved.
    pub fn snapshot(&mut self, frame: usize) -> Option<Snapshot> {
        let decoded = frame::decode(&self.stream.read(frame)?)?;
        let mut cars = Vec::with_capacity(decoded.cars.len());
        for (i, c) in decoded.cars.into_iter().enumerate() {
            let geometry = match c.geometry {
                Shared::Inline(g) => g,
                Shared::At(at) => match frame::decode(&self.stream.read(at as usize)?)?
                    .cars
                    .get(i)?
                    .geometry
                {
                    Shared::Inline(g) => g,
                    Shared::At(_) => return None,
                },
            };
            let deformation = match c.deformation {
                None => None,
                Some(Shared::Inline(d)) => Some(d),
                Some(Shared::At(at)) => {
                    match frame::decode(&self.stream.read(at as usize)?)?
                        .cars
                        .get(i)?
                        .deformation
                        .clone()
                    {
                        Some(Shared::Inline(d)) => Some(d),
                        _ => return None,
                    }
                }
            };
            cars.push(Car {
                pose: c.pose,
                velocity: c.velocity,
                wheels: c.wheels,
                geometry,
                deformation,
                lost: c.lost,
            });
        }
        let props = match decoded.props {
            None => None,
            Some(p) => Some(Props {
                broken: match p.broken {
                    Shared::Inline(b) => b,
                    Shared::At(at) => match frame::decode(&self.stream.read(at as usize)?)?
                        .props?
                        .broken
                    {
                        Shared::Inline(b) => b,
                        Shared::At(_) => return None,
                    },
                },
                pieces: p.pieces,
            }),
        };
        Some(Snapshot {
            tick: decoded.tick,
            cars,
            props,
        })
    }

    /// Where each car's deformation shown at `frame` was written.
    fn deformation_frames(&mut self, frame: usize) -> Vec<Option<u32>> {
        let Some(decoded) = self.stream.read(frame).and_then(|b| frame::decode(&b)) else {
            return Vec::new();
        };
        decoded
            .cars
            .iter()
            .map(|c| match &c.deformation {
                None => None,
                Some(Shared::Inline(_)) => Some(frame as u32),
                Some(Shared::At(at)) => Some(*at),
            })
            .collect()
    }

    /// The interpolated state at `time` (simulation seconds).
    pub fn state_at(&mut self, time: f64) -> Option<Snapshot> {
        let tick = time / f64::from(DELTA);
        let i = self.stream.at_or_before(tick.max(0.).floor() as u64)?;
        let a = self.snapshot(i)?;
        let next = i + 1;
        if next >= self.stream.frames().end {
            return Some(a);
        }
        let Some(b) = self.snapshot(next) else {
            return Some(a);
        };
        let span = (b.tick - a.tick) as f64;
        let t = ((tick - a.tick as f64) / span).clamp(0., 1.) as f32;
        let since = ((tick - a.tick as f64).max(0.) * f64::from(DELTA)) as f32;
        Some(frame::interpolate(&a, &b, t, since))
    }

    /// The director's activation at `tick`.
    pub fn activation(&self, tick: u64) -> Option<director::Activation> {
        let d = self.director.as_ref()?;
        let n = self.entries.partition_point(|&(t, _)| t <= tick);
        self.entries[..n]
            .iter()
            .rev()
            .find_map(|&(_, z)| d.zones[z].activation.clone())
            .or_else(|| d.initial.clone())
    }

    /// The readable window for `kind`, simulation seconds.
    pub fn window(&self, kind: Kind) -> Option<(f64, f64)> {
        let frames = self.stream.frames();
        if frames.is_empty() {
            return None;
        }
        let first = match kind {
            Kind::Instant => self.stream.memory_start().min(frames.end - 1),
            Kind::Full => frames.start,
        };
        Some((
            seconds(self.stream.tick(first)),
            seconds(self.stream.tick(frames.end - 1)),
        ))
    }

    /// A new session, or `None` with nothing recorded.
    pub fn start(&mut self, kind: Kind, origin: Origin, paused: bool, views: &crate::camera::Views) -> bool {
        let Some((start, end)) = self.window(kind) else {
            return false;
        };
        let from = match kind {
            Kind::Instant => end - INSTANT_REWIND,
            Kind::Full => start,
        };
        let mut cameras = Vec::new();
        if self.director.is_some() {
            cameras.push(Choice::Director);
        }
        // The car's replay views, then its race views (AUTHORED order).
        cameras.extend(
            views
                .0
                .iter()
                .enumerate()
                .filter(|(_, v)| v.hidden && v.ident.starts_with("replay_"))
                .map(|(i, _)| Choice::View(i)),
        );
        cameras.extend(
            views
                .0
                .iter()
                .enumerate()
                .filter(|(_, v)| !v.hidden)
                .map(|(i, _)| Choice::View(i)),
        );
        self.session = Some(Session {
            kind,
            origin,
            player: player::Player::new(start, end, from),
            resume_paused: paused,
            cameras,
            camera: 0,
            shown: None,
            deformation: Vec::new(),
            projected: None,
            activation: None,
            fov: None,
            history: Default::default(),
            last_time: None,
            hidden: Vec::new(),
            idle: 0.,
            laps: self.laps.iter().map(|&t| seconds(t)).collect(),
        });
        true
    }
}

fn car(runtime: &Runtime, lost: impl Fn(usize) -> Option<Mat4>) -> Car {
    let v = runtime.velocity();
    let wheels = runtime.wheels();
    let deformation = runtime
        .deformation_enabled
        .then(|| grid_tools::body_deformation::Field::read(&runtime.state.car.owner.vehicle).ok())
        .flatten()
        .map(|field| {
            let (nodes, active) = field.current();
            Deformation {
                active,
                nodes: nodes.to_vec(),
            }
        });
    Car {
        pose: Pose::from_mat4(Mat4::from_cols_array(&runtime.pose())),
        velocity: Vec3::new(v[0], v[1], v[2]),
        wheels: std::array::from_fn(|k| Wheel {
            displacement: wheels[k].displacement,
            steering: wheels[k].steering,
            phase: wheels[k].phase,
            spin: crate::wheel_presentation::angular_speed(runtime, k),
        }),
        geometry: std::array::from_fn(|k| WheelGeometry {
            anchor: wheels[k].anchor,
            axis: wheels[k].axis,
            deformation_translation: wheels[k].deformation_translation,
            deformation_orientation: wheels[k].deformation_orientation,
        }),
        deformation,
        lost: std::array::from_fn(|k| lost(k).map(Pose::from_mat4)),
    }
}

/// The live props as recorded.
pub fn props(world: &grid_tools::dynamic_props::PropWorld, bodies: &[grid_world::contact_groups::Body]) -> Props {
    Props {
        broken: world.broken.clone(),
        pieces: world
            .poses(bodies)
            .into_iter()
            .map(|(b, s, m)| frame::Piece {
                breakable: b as u16,
                spec: s as u16,
                pose: Pose::from_mat4(crate::dynamic_props::pose(&m)),
            })
            .collect(),
    }
}

/// The live presentation's wheel transform from a replayed wheel.
pub fn wheel_motion(wheel: &Wheel, geometry: &WheelGeometry) -> Mat4 {
    crate::wheel_motion(grid_tools::source_vehicle::WheelPose {
        anchor: geometry.anchor,
        axis: geometry.axis,
        displacement: wheel.displacement,
        steering: wheel.steering,
        phase: wheel.phase,
        deformation_translation: geometry.deformation_translation,
        deformation_orientation: geometry.deformation_orientation,
    })
}

pub fn install(app: &mut App, root: &std::path::Path, route: &grid_tools::route::Route) {
    let director = match director::Director::load(root, route) {
        Ok(d) => Some(d),
        Err(error) => {
            warn!("Replay director unavailable; car views only: {error}");
            None
        }
    };
    let spool = crate::settings::data_dir().join("replay-cache").join("replay.spool");
    let stream = stream::Stream::new(stream::Limits::default(), Some(spool));
    log_line!(
        "drive_race_replay version=1 event=install sample_hz={SAMPLE_HZ} memory_bytes={} spool={} director_cameras={} zones={}",
        stream::MEMORY_BYTES,
        stream.spooling(),
        director.as_ref().map_or(0, |d| d.cameras.len()),
        director.as_ref().map_or(0, |d| d.zones.len()),
    );
    app.insert_resource(Replay::new(stream, director))
        .add_systems(
            FixedUpdate,
            record
                .after(crate::drive_tick)
                .after(crate::damage_parts::tick),
        )
        .add_systems(
            Update,
            control
                .before(crate::pause_menu::update)
                .before(crate::post_race::update),
        )
        .add_systems(Update, frame_camera.after(crate::drive_present))
        .add_systems(
            PostUpdate,
            present.before(bevy::transform::TransformSystems::Propagate),
        );
}

/// Records after the fixed tick (and the damage parts' tick).
pub fn record(
    mut replay: ResMut<Replay>,
    driving: Res<crate::Driving>,
    damage: Option<Res<crate::damage_parts::DamageParts>>,
) {
    let runtimes: Vec<&Runtime> = std::iter::once(&driving.runtime)
        .chain(
            driving
                .opponents
                .iter()
                .flat_map(|o| o.multi.cars.iter().skip(1)),
        )
        .collect();
    let props = match &driving.opponents {
        Some(o) => o.multi.dynamic_props().map(|p| props(p, o.multi.bodies())),
        None => driving
            .runtime
            .dynamic_props()
            .map(|p| props(p, &driving.runtime.state.world.groups.bodies)),
    };
    replay.record(driving.segment, &runtimes, props, |car, wheel| {
        damage.as_ref().and_then(|d| d.lost_wheel(car, wheel))
    });
}

fn input(actions: &Actions) -> player::Input {
    player::Input {
        pause: actions.pressed(Action::ReplayPause),
        rewind: actions.value(Action::ReplayRewind),
        fast_forward: actions.value(Action::ReplayFastForward),
        next_lap: actions.pressed(Action::ReplayNextLap),
        previous_lap: actions.pressed(Action::ReplayPreviousLap),
    }
}

/// Starts requested replays, runs the playing one and ends it on Replay Exit.
#[allow(clippy::too_many_arguments)]
pub fn control(
    mut replay: ResMut<Replay>,
    mut driving: ResMut<crate::Driving>,
    actions: Res<Actions>,
    mut shifts: ResMut<crate::input::ShiftQueue>,
    menu: Option<Res<crate::pause_menu::PauseMenu>>,
    post: Option<Res<crate::post_race::PostRace>>,
    cameras: Option<Res<crate::camera::Cameras>>,
    time: Option<Res<Time<Real>>>,
) {
    let replay = &mut *replay;
    replay.exited = false;
    let dt = time.map_or(0., |t| t.delta_secs()).min(0.1);
    // Diagnostics replay_capture: an Instant Replay once the script ended.
    if crate::settings::diagnostics().replay_capture.is_some()
        && replay.session.is_none()
        && replay.request.is_none()
        && !replay.exited
        && driving.scripted_ticks.is_some_and(|n| driving.runtime.ticks >= n)
        && !driving.paused
    {
        replay.request(Kind::Instant, Origin::Race);
    }
    // The Instant Replay action in the race (flow `c_in_instant_replay`).
    if replay.session.is_none()
        && replay.request.is_none()
        && actions.pressed(Action::InstantReplay)
        && !driving.paused
        && driving.failure.is_none()
        && !menu.as_ref().is_some_and(|m| m.open)
        && !post.as_ref().is_some_and(|p| p.active())
    {
        replay.request(Kind::Instant, Origin::Race);
    }
    if let Some((kind, origin)) = replay.request.take() {
        let fallback = crate::camera::Views::fallback();
        let views = cameras.as_ref().map_or(&fallback, |c| &c.views);
        if replay.start(kind, origin, driving.paused, views) {
            driving.paused = true;
            let s = replay.session.as_ref().unwrap();
            log_line!(
                "drive_race_replay version=1 event=start kind={kind:?} origin={origin:?} segment={} tick={} from={:.3} start={:.3} end={:.3} frames={} memory_bytes={} spooled_bytes={}",
                driving.segment,
                driving.runtime.ticks,
                s.player.time,
                s.player.start,
                s.player.end,
                replay.stream.frames().len(),
                replay.stream.memory_bytes(),
                replay.stream.spooled_bytes()
            );
        } else {
            log_line!("drive_race_replay version=1 event=empty kind={kind:?}");
            replay.exited = true;
        }
        *shifts = crate::input::ShiftQueue::default();
        return;
    }
    let Some(session) = replay.session.as_mut() else {
        return;
    };
    *shifts = crate::input::ShiftQueue::default();
    if actions.pressed(Action::ReplayExit) {
        let origin = session.origin;
        driving.paused = session.resume_paused;
        log_line!(
            "drive_race_replay version=1 event=exit origin={origin:?} time={:.3}",
            session.player.time
        );
        // `present` restores the live view and HUD this frame.
        replay.ending = replay.session.take().map(|s| s.hidden);
        replay.exited = true;
        return;
    }
    let step = input(&actions);
    let n = session.cameras.len().max(1);
    if actions.pressed(Action::ReplayNextCamera) {
        session.camera = (session.camera + 1) % n;
    }
    if actions.pressed(Action::ReplayPrevCamera) {
        session.camera = (session.camera + n - 1) % n;
    }
    let busy = step != player::Input::default()
        || actions.pressed(Action::ReplayNextCamera)
        || actions.pressed(Action::ReplayPrevCamera);
    session.idle = if busy { 0. } else { session.idle + dt };
    let laps = std::mem::take(&mut session.laps);
    let event = session.player.step(step, f64::from(dt), &laps);
    session.laps = laps;
    if event == Some(player::Event::Loop) {
        log_line!("drive_race_replay version=1 event=loop time={:.3}", session.player.time);
    }
    let time = session.player.time;
    let shown = replay.state_at(time);
    let frame = replay
        .stream
        .at_or_before((time / f64::from(DELTA)).max(0.).floor() as u64);
    let deformation = frame.map(|f| replay.deformation_frames(f)).unwrap_or_default();
    let activation = shown.as_ref().and_then(|s| replay.activation(s.tick));
    let session = replay.session.as_mut().unwrap();
    session.shown = shown;
    if session.activation != activation {
        session.activation = activation;
        session.fov = None;
    }
    if !deformation.is_empty() {
        session.deformation = deformation;
    }
}

/// The replay camera, after the live camera was framed.
pub fn frame_camera(
    mut replay: ResMut<Replay>,
    cameras: Option<Res<crate::camera::Cameras>>,
    mut camera: Single<(&mut Transform, Option<&mut Projection>), With<crate::ViewerCamera>>,
) {
    let replay = &mut *replay;
    let director = replay.director.as_ref();
    let Some(session) = replay.session.as_mut() else {
        return;
    };
    let Some(shown) = &session.shown else {
        return;
    };
    let Some(car) = shown.cars.first() else {
        return;
    };
    let time = session.player.time;
    let dt = session.last_time.map_or(0., |t| (time - t).abs()) as f32;
    session.last_time = Some(time);
    let choice = session.cameras.get(session.camera).copied();
    let views = cameras.as_ref().map(|c| &c.views);
    let view = |ident: &str| views.and_then(|v| v.0.iter().position(|x| x.ident == ident));
    // Director: its trackside camera or a view of the car.
    let (eye, rotation, fov, near) = match (choice, director, &session.activation) {
        (Some(Choice::Director), Some(d), Some(director::Activation::Track(c))) => {
            let fresh = session.fov.is_none();
            let camera = &d.cameras[*c];
            let shot = camera.shot(&car.pose, car.velocity, &mut session.fov, dt);
            if fresh {
                log_line!(
                    "drive_race_replay version=1 event=camera ident={} kind={:?} path_points={} tick={} distance={:.2} fov={:.2}",
                    camera.ident,
                    camera.kind,
                    camera.path.len(),
                    shown.tick,
                    shot.eye.distance(car.pose.translation()),
                    shot.fov.to_degrees()
                );
            }
            (shot.eye, shot.rotation, shot.fov, None)
        }
        (choice, _, activation) => {
            let index = match (choice, activation) {
                (Some(Choice::View(i)), _) => Some(i),
                (_, Some(director::Activation::Car(name))) => view(name),
                _ => None,
            }
            .or_else(|| cameras.as_ref().map(|c| c.current));
            let Some((index, views)) = index.zip(views) else {
                return;
            };
            let sample = crate::camera::Sample {
                pose: car.pose.to_mat4(),
                velocity: car.velocity,
                ticks: shown.tick,
                segment: 0,
            };
            let shot = crate::camera::shot(&views.0[index].rig, &mut session.history, &sample, dt, |o| o);
            (shot.eye, shot.rotation, shot.fov, shot.near)
        }
    };
    let (transform, projection) = &mut *camera;
    **transform = Transform::from_translation(eye).with_rotation(rotation);
    if let Some(Projection::Perspective(p)) = projection.as_deref_mut() {
        p.fov = fov;
        p.near = near.unwrap_or(PerspectiveProjection::default().near);
    }
}

/// The replay's text panel.
#[derive(Component)]
pub struct Panel;

type PartQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static crate::MovingPart,
        &'static mut Transform,
        Option<&'static crate::DeformingPart>,
        &'static Mesh3d,
        &'static mut Aabb,
    ),
    (Without<crate::OpponentPart>, Without<crate::ViewerCamera>),
>;
type OpponentQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static crate::OpponentPart,
        &'static mut Transform,
        Option<&'static crate::OpponentDeforming>,
        &'static Mesh3d,
        &'static mut Aabb,
    ),
    (Without<crate::MovingPart>, Without<crate::ViewerCamera>),
>;

/// Parts that moved this far from their rest place on the live car are
/// lying on the road (detached bodywork): they stay where they are
/// (AUTHORED).
const DETACHED: f32 = 2.;

fn part_transform(
    replay_pose: Mat4,
    live_pose: Mat4,
    live: &Transform,
    rest: Mat4,
    wheel: Option<usize>,
    lost_with_wheel: bool,
    car: &Car,
) -> Option<Transform> {
    if let Some(w) = wheel {
        if lost_with_wheel && let Some(lost) = car.lost[w] {
            return Some(Transform::from_matrix(lost.to_mat4() * rest));
        }
        return Some(Transform::from_matrix(
            replay_pose * wheel_motion(&car.wheels[w], &car.geometry[w]) * rest,
        ));
    }
    let local = live_pose.inverse() * live.to_matrix();
    let moved = (local * rest.inverse()).w_axis.truncate().length();
    (moved <= DETACHED).then(|| Transform::from_matrix(replay_pose * local))
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn present(
    mut commands: Commands,
    mut replay: ResMut<Replay>,
    driving: Res<crate::Driving>,
    mut parts: PartQuery,
    mut opponents: OpponentQuery,
    mut intact: Query<(&crate::dynamic_props::IntactProp, &mut Visibility), Without<crate::dynamic_props::PropPiece>>,
    mut pieces: Query<
        (&crate::dynamic_props::PropPiece, &mut Transform, &mut Visibility),
        (Without<crate::dynamic_props::IntactProp>, Without<crate::MovingPart>, Without<crate::OpponentPart>, Without<crate::ViewerCamera>),
    >,
    mut ui: Query<
        (Entity, &mut Visibility),
        (With<Node>, Without<ChildOf>, Without<Panel>, Without<crate::dynamic_props::IntactProp>, Without<crate::dynamic_props::PropPiece>),
    >,
    mut panel: Query<(&mut Text, &mut Visibility), (With<Panel>, Without<crate::dynamic_props::IntactProp>, Without<crate::dynamic_props::PropPiece>)>,
    viewer: Query<Entity, With<crate::ViewerCamera>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut presentation: ResMut<crate::DeformationPresentation>,
    labels: Option<Res<crate::pause_menu::Labels>>,
) {
    let replay = &mut *replay;
    // A replay that ended: the live view and HUD come back.
    if let Some(hidden) = replay.ending.take() {
        for (entity, visibility) in hidden {
            if let Ok((_, mut v)) = ui.get_mut(entity) {
                *v = visibility;
            }
        }
        for (_, mut v) in &mut panel {
            *v = Visibility::Hidden;
        }
        // Live dents are projected again by drive_present.
        presentation.0 = None;
        presentation.1.clear();
        return;
    }
    let Some(session) = replay.session.as_mut() else {
        return;
    };
    // The HUD and menus are hidden (`osd_play_off`, `hide_pause_show_overlay`).
    for (entity, mut visibility) in &mut ui {
        if !session.hidden.iter().any(|(e, _)| *e == entity) {
            session.hidden.push((entity, *visibility));
        }
        visibility.set_if_neq(Visibility::Hidden);
    }
    if panel.is_empty()
        && let Some(camera) = viewer.iter().next()
    {
        commands.spawn((
            Panel,
            Node {
                position_type: PositionType::Absolute,
                left: px(24.),
                bottom: px(24.),
                padding: UiRect::all(px(10.)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.025, 0.04, 0.065, 0.78)),
            Text::new(""),
            TextFont {
                font_size: FontSize::Px(20.),
                ..default()
            },
            TextColor(Color::WHITE),
            UiTargetCamera(camera),
        ));
    }
    let title = labels.as_ref().map_or("INSTANT REPLAY", |l| match session.kind {
        Kind::Instant => l.get("pause_mainmenu_button_instant_replay"),
        Kind::Full => l.get("post_race_menu_replay_button_header"),
    });
    let p = &session.player;
    let state = match p.mode {
        player::Mode::Pause => "PAUSED".to_owned(),
        player::Mode::Play if p.shuttle == 1. => "PLAY".to_owned(),
        _ => format!("{:+.2}x", p.shuttle),
    };
    let clock = |s: f64| crate::race_hud::lap_time(s.max(0.));
    let camera = match session.cameras.get(session.camera) {
        Some(Choice::Director) => "Replay Director".to_owned(),
        Some(Choice::View(i)) => format!("view {i}"),
        None => String::new(),
    };
    let mut text = format!(
        "{title}\n{} / {}   {state}   {camera}",
        clock(p.time - p.start),
        clock(p.end - p.start)
    );
    if session.idle < INTERFACE_TIMEOUT {
        text.push_str("\nF1 pause   F2/F3 hold: rewind/forward   F7/F8 camera   F4/F5 lap   Esc exit");
    }
    for (mut t, mut v) in &mut panel {
        if t.0 != text {
            t.0.clone_from(&text);
        }
        v.set_if_neq(Visibility::Inherited);
    }
    let Some(shown) = session.shown.clone() else {
        return;
    };
    // Cars: the replayed body and wheels; the live damage parts follow the
    // replayed body.
    let live_pose = |car: usize| -> Option<Mat4> {
        if car == 0 {
            return Some(Mat4::from_cols_array(&driving.runtime.pose()));
        }
        let o = driving.opponents.as_ref()?;
        Some(Mat4::from_cols_array(&o.multi.cars.get(car)?.pose()))
    };
    if let (Some(car), Some(live)) = (shown.cars.first(), live_pose(0)) {
        let pose = car.pose.to_mat4();
        for (part, mut transform, ..) in &mut parts {
            if let Some(t) = part_transform(pose, live, &transform, part.rest, part.wheel, part.lost_with_wheel, car) {
                *transform = t;
            }
        }
    }
    for (part, mut transform, ..) in &mut opponents {
        let (Some(car), Some(live)) = (shown.cars.get(part.car), live_pose(part.car)) else {
            continue;
        };
        if let Some(t) = part_transform(car.pose.to_mat4(), live, &transform, part.rest, part.wheel, part.lost_with_wheel, car) {
            *transform = t;
        }
    }
    // Body dents of the replayed time; drive_present keeps the live keys and
    // so does not project over them until the replay ends.
    if session.projected.as_ref() != Some(&session.deformation) {
        session.projected = Some(session.deformation.clone());
        let runtime = |car: usize| -> Option<&Runtime> {
            if car == 0 {
                return Some(&driving.runtime);
            }
            driving.opponents.as_ref()?.multi.cars.get(car)
        };
        let field = |car: usize| -> Option<grid_tools::body_deformation::Field> {
            let recorded = shown.cars.get(car)?.deformation.as_ref()?;
            let live = grid_tools::body_deformation::Field::read(&runtime(car)?.state.car.owner.vehicle).ok()?;
            live.with_current(&recorded.nodes, recorded.active).ok()
        };
        if let Some(f) = field(0) {
            for (_, _, source, handle, mut bounds) in &mut parts {
                if let Some(source) = source
                    && let Ok(mesh) = crate::body_damage::project(&source.source, source.raw_bind, &f)
                    && let Some(next) = mesh.compute_aabb()
                    && let Some(mut target) = meshes.get_mut(&handle.0)
                {
                    *target = mesh;
                    *bounds = next;
                }
            }
        }
        let fields: Vec<Option<grid_tools::body_deformation::Field>> = (0..shown.cars.len()).map(|c| if c == 0 { None } else { field(c) }).collect();
        for (part, _, deforming, handle, mut bounds) in &mut opponents {
            if let (Some(crate::OpponentDeforming(source)), Some(Some(f))) = (deforming, fields.get(part.car))
                && let Ok(mesh) = crate::body_damage::project(&source.source, source.raw_bind, f)
                && let Some(next) = mesh.compute_aabb()
                && let Some(mut target) = meshes.get_mut(&handle.0)
            {
                *target = mesh;
                *bounds = next;
            }
        }
    }
    // Props: broken flags and live pieces of the replayed time.
    let catalog = match &driving.opponents {
        Some(o) => o.multi.dynamic_props(),
        None => driving.runtime.dynamic_props(),
    };
    if let (Some(props), Some(world)) = (&shown.props, catalog) {
        for (prop, mut visibility) in &mut intact {
            let broken = props.broken.get(prop.0).copied().unwrap_or(false);
            visibility.set_if_neq(if broken { Visibility::Hidden } else { Visibility::Inherited });
        }
        for (piece, mut transform, mut visibility) in &mut pieces {
            match props
                .pieces
                .iter()
                .find(|p| (usize::from(p.breakable), usize::from(p.spec)) == (piece.breakable, piece.spec))
            {
                Some(p) => {
                    let start = crate::dynamic_props::pose(
                        &world.catalog.breakables[piece.breakable].pieces[piece.spec].pose,
                    );
                    *transform = Transform::from_matrix(p.pose.to_mat4() * start.inverse() * piece.rest);
                    visibility.set_if_neq(Visibility::Inherited);
                }
                None => {
                    visibility.set_if_neq(Visibility::Hidden);
                }
            }
        }
    }
}
