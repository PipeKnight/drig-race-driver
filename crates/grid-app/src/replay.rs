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
//! - Replay Jump In (Flashback) from an Instant Replay: `crate::flashback`
//!   cuts the stream at the chosen frame ([`Replay::cut`]) and holds the
//!   session ([`Replay::freeze`]) through the transition.
//!
//! While a replay plays the simulation is held (`Driving::paused`), the live
//! presentation keeps computing and the replay overwrites it last, so
//! leaving the replay shows the live race again unchanged.
pub mod director;
pub mod frame;
pub mod player;
pub mod stream;
#[cfg(test)]
pub(crate) mod race_tests;

use crate::input::{Action, Actions};
use bevy::{
    camera::primitives::{Aabb, MeshAabb},
    prelude::*,
};
use frame::{Car, Decoder, Deformation, Encoder, Pose, Props, Shared, Snapshot, Wheel, WheelGeometry};
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
    /// The terminal-damage replay (flow `c_in_terminal_damage_loop`, state
    /// `instant_race_replay_silent` terminal="true", interface
    /// `terminal_replay_interface`): the director shows the zones'
    /// `TERMINAL` cameras; the image is the ordinary one (the preceding
    /// `terminal_fade` ends on Effect_0000).
    pub terminal: bool,
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
    /// The director's activation and the tick it happened at.
    activation: Option<(director::Activation, u64)>,
    shots: Option<Shots>,
    history: crate::camera::History,
    last_time: Option<f64>,
    hidden: Vec<(Entity, Visibility)>,
    idle: f32,
    laps: Vec<f64>,
}

/// A director shot's camera at the replay samples since its activation: the
/// original records its camera state at 30 Hz and interpolates it (0x7F7810,
/// 0x81A110); the port steps it at each sample from the activation, so
/// seeking and rewinding show the same shot.
struct Shots {
    camera: usize,
    /// Frame of the activation.
    first: usize,
    state: director::State,
    shots: Vec<director::Shot>,
}

#[derive(Resource)]
pub struct Replay {
    pub stream: stream::Stream,
    encoder: Encoder,
    decoder: Decoder,
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
    /// Flashback state for the panel (`crate::flashback`).
    pub flashback: Option<FlashbackStatus>,
    /// A jump-in holds the session: no replay input.
    frozen: bool,
    /// The Flashback transition's camera (`crate::flashback`).
    pub flash_camera: Option<FlashCamera>,
    /// The requested session is the terminal-damage replay, anchored at the
    /// crash time (simulation seconds) when known.
    terminal_request: Option<Option<f64>>,
}

/// The camera a Flashback transition state selects [D 0x579D70]; flash 1's
/// selectReplayCamera keeps the replay's own camera.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlashCamera {
    /// selectOnCarCamera: the player car's `replay_driver2` view.
    OnCar,
    /// restoreInGameCamera: the camera the player drove with.
    InGame,
}

/// What the replay panel shows of Flashback.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FlashbackStatus {
    pub remaining: u32,
    /// A jump-in is possible at the replay time (the original's icon test,
    /// 0x542A00: 0x937150 and 0x4CAD80).
    pub possible: bool,
}

impl Replay {
    pub fn new(stream: stream::Stream, director: Option<director::Director>) -> Self {
        Self {
            stream,
            encoder: Encoder::default(),
            decoder: Decoder::default(),
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
            flashback: None,
            frozen: false,
            flash_camera: None,
            terminal_request: None,
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

    /// Flashback jump-in at `tick`, a recorded sample in memory: the later
    /// frames, laps and director entries are forgotten and recording goes on
    /// from it [D 0x93DDA0 cuts the stream at the frame at or before the
    /// replay time].
    pub fn cut(&mut self, segment: u64, tick: u64) -> bool {
        let Some(frame) = self.stream.at_or_before(tick) else {
            return false;
        };
        if self.stream.tick(frame) != tick || !self.stream.truncate(frame) {
            return false;
        }
        // Shared items and the per-frame words are written whole again; the
        // frame numbers after the cut are recorded anew.
        self.encoder = Encoder::default();
        self.decoder = Decoder::default();
        // The session's camera shots may reach past the cut (they run one
        // frame ahead of the replay time); they are made again from the cut
        // stream (the frame camera would index a frame no longer recorded).
        if let Some(s) = self.session.as_mut() {
            s.shots = None;
        }
        self.recorded = Some((segment, tick));
        self.seen = Some((segment, tick));
        self.laps.retain(|&t| t <= tick);
        self.entries.retain(|&(t, _)| t <= tick);
        self.last_target = self
            .snapshot(frame)
            .and_then(|s| s.cars.first().map(|c| c.pose.translation()));
        true
    }

    /// Holds the session paused at `time` (the jump-in frame) with no input.
    pub fn freeze(&mut self, time: f64) {
        if let Some(session) = &mut self.session {
            session.player.time = time.clamp(session.player.start, session.player.end);
            session.player.mode = player::Mode::Pause;
            session.player.shuttle = 1.;
            session.idle = INTERFACE_TIMEOUT;
            self.frozen = true;
        }
    }

    /// Ends the session as Replay Exit does, without its key; the live view
    /// and HUD come back in `present`.
    pub fn finish(&mut self) -> Option<Origin> {
        self.frozen = false;
        self.flash_camera = None;
        let session = self.session.take()?;
        self.ending = Some(session.hidden);
        self.exited = true;
        Some(session.origin)
    }

    /// The menus leave the input alone while this is true.
    pub fn owns_input(&self) -> bool {
        self.session.is_some() || self.exited || self.request.is_some()
    }

    pub fn request(&mut self, kind: Kind, origin: Origin) {
        self.request = Some((kind, origin));
        self.terminal_request = None;
    }

    /// The wreck screen's Replay: the terminal-damage replay of the
    /// [`INSTANT_REWIND`] seconds before the crash at `wreck` (simulation
    /// seconds) up to now; without a crash time, the last
    /// [`INSTANT_REWIND`] seconds. The original's replay-state handler
    /// 0x63EBC0 sets the window END to the replay's start and START to
    /// max(crash − 10 s [0xDB4FE4], oldest frame) (0x91CC00); rewind stops
    /// at START. Flashback stays possible, as the original's
    /// `terminal_replay_interface` has a `jump_in` link.
    pub fn request_terminal(&mut self, wreck: Option<f64>) {
        self.request = Some((Kind::Instant, Origin::Results));
        self.terminal_request = Some(wreck);
    }

    fn clear(&mut self) {
        self.stream.clear();
        self.encoder = Encoder::default();
        self.decoder = Decoder::default();
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
        parts: impl Fn(usize) -> Vec<frame::Part>,
    ) {
        let Some(player) = runtimes.first() else {
            return;
        };
        // The race holds while a replay plays. A Flashback re-runs its ticks
        // from a checkpoint before the cut frame under the session (over
        // several frames); recording those as a rewound race would clear the
        // stream, so recording resumes when the session ends.
        if self.session.is_some() {
            return;
        }
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
                .map(|(i, r)| car(r, |w| lost(i, w), parts(i)))
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
        let mut bytes = self.encoder.encode(&snapshot, number);
        if self.stream.opens_chunk(bytes.len()) {
            // A chunk starts with a frame that refers to no earlier one, so a
            // memory-only stream stays readable when it drops older chunks.
            self.encoder = Encoder::default();
            bytes = self.encoder.encode(&snapshot, number);
        }
        self.stream.push(tick, &bytes);
        self.encoder.forget_before(self.stream.frames().start as u32);
        self.recorded = Some((segment, tick));
    }

    /// Frame `frame` with its shared items resolved.
    pub fn snapshot(&mut self, frame: usize) -> Option<Snapshot> {
        let Replay {
            decoder, stream, ..
        } = self;
        decoder.snapshot(frame, |n| stream.read(n))
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
    #[cfg(test)]
    pub fn activation(&self, tick: u64) -> Option<director::Activation> {
        self.activation_at(tick).map(|(a, _)| a)
    }

    /// The director's activation at `tick` and the tick of that activation
    /// (0 for the initial camera).
    pub fn activation_at(&self, tick: u64) -> Option<(director::Activation, u64)> {
        let terminal = self.session.as_ref().is_some_and(|s| s.terminal);
        self.activation_for(tick, terminal)
    }

    /// [`Self::activation_at`] for an ordinary (`terminal` false) or a
    /// terminal-damage replay.
    fn activation_for(&self, tick: u64, terminal: bool) -> Option<(director::Activation, u64)> {
        let d = self.director.as_ref()?;
        let n = self.entries.partition_point(|&(t, _)| t <= tick);
        self.entries[..n]
            .iter()
            .rev()
            .find_map(|&(t, z)| {
                let zone = &d.zones[z];
                // A zone without a terminal camera keeps its ordinary one [I].
                let a = if terminal {
                    zone.terminal.clone().or_else(|| zone.activation.clone())
                } else {
                    zone.activation.clone()
                };
                a.map(|a| (a, t))
            })
            .or_else(|| d.initial.clone().map(|a| (a, 0)))
    }

    /// The player's car in frame `frame`: tick, pose and velocity.
    fn player_in(
        decoder: &mut Decoder,
        stream: &mut stream::Stream,
        frame: usize,
    ) -> Option<(u64, Pose, Vec3)> {
        let f = decoder.snapshot(frame, |n| stream.read(n))?;
        let car = f.cars.first()?;
        Some((f.tick, car.pose, car.velocity))
    }

    /// The trackside camera `camera` activated at tick `from`, at `time`;
    /// `true` when the shot is new.
    fn director_shot(&mut self, camera: usize, from: u64, time: f64) -> Option<(director::Shot, bool)> {
        let Replay {
            director,
            stream,
            decoder,
            session,
            ..
        } = self;
        let camera = director.as_ref()?.cameras.get(camera).map(|c| (camera, c))?;
        let session = session.as_mut()?;
        let tick = time / f64::from(DELTA);
        let frames = stream.frames();
        let i = stream.at_or_before(tick.max(0.).floor() as u64)?;
        let first = stream.at_or_before(from).unwrap_or(frames.start).max(frames.start);
        let mut shots = session
            .shots
            .take()
            .filter(|s| s.camera == camera.0 && s.first == first && i >= first);
        let fresh = shots.is_none();
        if fresh {
            let (_, pose, velocity) = Self::player_in(decoder, stream, first)?;
            let (state, shot) = camera.1.start(&pose, velocity);
            shots = Some(Shots {
                camera: camera.0,
                first,
                state,
                shots: vec![shot],
            });
        }
        let mut s = shots?;
        let last = (i + 1).min(frames.end - 1).max(first);
        let mut previous = stream.tick(first + s.shots.len() - 1);
        while first + s.shots.len() <= last {
            let Some((tick, pose, velocity)) = Self::player_in(decoder, stream, first + s.shots.len()) else {
                break;
            };
            let dt = (tick.saturating_sub(previous)) as f32 * DELTA;
            s.shots.push(camera.1.step(&mut s.state, &pose, velocity, dt));
            previous = tick;
        }
        let k = (i.max(first) - first).min(s.shots.len() - 1);
        let mut shot = s.shots[k];
        if let Some(next) = s.shots.get(k + 1) {
            let (a, b) = (stream.tick(first + k) as f64, stream.tick(first + k + 1) as f64);
            if b > a {
                shot = shot.lerp(*next, ((tick - a) / (b - a)).clamp(0., 1.) as f32);
            }
        }
        session.shots = Some(s);
        Some((shot, fresh))
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
        let terminal_request = self.terminal_request.take();
        let terminal = terminal_request.is_some();
        let Some((mut start, end)) = self.window(kind) else {
            return false;
        };
        let from = match (kind, terminal_request) {
            (Kind::Instant, Some(Some(wreck))) => {
                // The terminal window: rewind stops at its START.
                start = terminal_start(start, end, wreck);
                start
            }
            (Kind::Instant, _) => end - INSTANT_REWIND,
            (Kind::Full, _) => start,
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
            terminal,
            player: player::Player::new(start, end, from),
            resume_paused: paused,
            cameras,
            camera: 0,
            shown: None,
            deformation: Vec::new(),
            projected: None,
            activation: None,
            shots: None,
            history: Default::default(),
            last_time: None,
            hidden: Vec::new(),
            idle: 0.,
            laps: self.laps.iter().map(|&t| seconds(t)).collect(),
        });
        true
    }
}

/// The terminal window's START: [`INSTANT_REWIND`] seconds before the crash
/// at `wreck`, not before the `oldest` frame (0x91CC00) nor after `end`.
fn terminal_start(oldest: f64, end: f64, wreck: f64) -> f64 {
    (wreck - INSTANT_REWIND).max(oldest).min(end)
}

fn car(runtime: &Runtime, lost: impl Fn(usize) -> Option<Mat4>, parts: Vec<frame::Part>) -> Car {
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
        parts,
        effects: effects(runtime),
    }
}

/// The ground effects' source words of `runtime` (see [`frame::Effects`]).
pub fn effects(runtime: &Runtime) -> frame::Effects {
    let b = &runtime.state.car.owner.vehicle;
    let f = |o: usize| f32::from_bits(b[o / 4]);
    let byte = |o: usize| (b[o / 4] >> (o % 4 * 8)) & 255;
    let w = runtime.angular_velocity();
    // The engine sound's control as engine_sound builds it for a live car.
    let throttle = if runtime.terminal_wheel().is_some() {
        0.
    } else {
        grid_tools::engine_audio::Control::applied_throttle(b)
    };
    let engine = grid_tools::engine_audio::Control::vehicle(b, throttle, true, 0);
    frame::Effects {
        angular_velocity: Vec3::new(w[0], w[1], w[2]),
        speed: f(0x1648),
        mass: f(0x15a0),
        rear_coefficient: f(0x1008),
        handbrake: byte(0x1640) != 0,
        open_wheel: b[0xeda8 / 4] != 0,
        rpm: engine.normalized_rpm,
        throttle: engine.throttle,
        gear: u8::try_from(engine.gear.min(15)).unwrap_or(0),
        wheels: std::array::from_fn(|i| {
            let o = 0x1990 + i * 0x150;
            frame::WheelEffects {
                token: b[(o + 0xc0) / 4],
                radius: f(o + 0xe0),
                width: f(o + 0xf8),
                load: f(o + 0x138),
                excess: f(o + 0xac),
                rear_term: f(o + 0x148),
                omega: f(o + 0x104),
                steer: [f(o + 0x28), f(o + 0x2c)],
                handbraked: byte(o + 0xfc) != 0,
            }
        }),
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
    replay.record(
        driving.segment,
        &runtimes,
        props,
        |car, wheel| damage.as_ref().and_then(|d| d.lost_wheel(car, wheel)),
        |car| damage.as_ref().map_or_else(Vec::new, |d| d.shown_parts(car)),
    );
}

fn input(actions: &Actions) -> player::Input {
    player::Input {
        pause: actions.pressed(Action::ReplayPause),
        rewind: actions.value(Action::ReplayRewind),
        fast_forward: actions.value(Action::ReplayFastForward),
        next_lap: actions.pressed(Action::ReplayNextLap),
        previous_lap: actions.pressed(Action::ReplayPreviousLap),
        slow: actions.pressed(Action::ReplaySlowMotion),
        step_back: actions.pressed(Action::ReplayStepBack),
        step_forward: actions.pressed(Action::ReplayStepForward),
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
        if crate::settings::diagnostics().replay_terminal {
            replay.request_terminal(None);
        } else {
            replay.request(Kind::Instant, Origin::Race);
        }
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
            let s = replay.session.as_mut().unwrap();
            s.player.slow = crate::settings::diagnostics().replay_slow;
            let s = &*s;
            log_line!(
                "drive_race_replay version=1 event=start kind={kind:?} origin={origin:?} segment={} tick={} from={:.3} start={:.3} end={:.3} slow={} terminal={} frames={} memory_bytes={} spooled_bytes={}",
                driving.segment,
                driving.runtime.ticks,
                s.player.time,
                s.player.start,
                s.player.end,
                s.player.slow,
                s.terminal,
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
    if replay.frozen {
        return;
    }
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
    let activation = shown.as_ref().and_then(|s| replay.activation_at(s.tick));
    let session = replay.session.as_mut().unwrap();
    session.shown = shown;
    session.activation = activation;
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
    // Director: its trackside camera at the replay time.
    let track = replay.session.as_ref().and_then(|s| {
        match (s.cameras.get(s.camera), &s.activation) {
            (Some(Choice::Director), Some((director::Activation::Track(c), from))) => {
                Some((*c, *from, s.player.time))
            }
            _ => None,
        }
    });
    // A Flashback transition's camera replaces the director's.
    let forced = replay.flash_camera;
    let shot = track
        .filter(|_| forced.is_none())
        .and_then(|(c, from, time)| replay.director_shot(c, from, time).map(|s| (c, s)));
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
    let activation = session.activation.as_ref().map(|(a, _)| a);
    let (eye, rotation, fov, near) = match (shot, director) {
        (Some((c, (shot, fresh))), Some(d)) => {
            if fresh {
                let camera = &d.cameras[c];
                log_line!(
                    "drive_race_replay version=1 event=camera ident={} kind={:?} source_path={} target_path={} tick={} distance={:.2} fov={:.2}",
                    camera.ident,
                    camera.kind,
                    camera.path.as_ref().map_or("-".to_owned(), |p| format!("{:?}", p.mode)),
                    camera.target_path.as_ref().map_or("-".to_owned(), |p| format!("{:?}", p.mode)),
                    shown.tick,
                    shot.eye.distance(car.pose.translation()),
                    shot.fov.to_degrees()
                );
            }
            (shot.eye, shot.rotation, shot.fov, None)
        }
        _ => {
            let current = cameras.as_ref().map(|c| c.current);
            let index = match forced {
                Some(FlashCamera::OnCar) => view("replay_driver2"),
                Some(FlashCamera::InGame) => current,
                None => None,
            }
            .or_else(|| match (choice, activation) {
                (Some(Choice::View(i)), _) => Some(i),
                (_, Some(director::Activation::Car(name))) => view(name),
                _ => None,
            })
            .or(current);
            let Some((index, views)) = index.zip(views) else {
                return;
            };
            let sample = crate::camera::Sample::moving(car.pose.to_mat4(), car.velocity, shown.tick, 0);
            let shot = crate::camera::shot(&views.0[index].rig, &mut session.history, &sample, dt, crate::camera::Look::default(), |o| o);
            let fov = cameras.as_ref().map_or(shot.fov, |c| c.fov.apply(views.fov_group(index), shot.fov));
            (shot.eye, shot.rotation, fov, shot.near)
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
        Entity,
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
        Entity,
    ),
    (Without<crate::MovingPart>, Without<crate::ViewerCamera>),
>;

/// Where a car part is drawn in the replay. A wheel follows the replayed
/// wheel (or its lost-wheel frame); other parts keep their place relative to
/// the live car (damage_parts draws them with the replayed swing angle) and
/// follow the replayed body, except the detached parts damage_parts placed
/// in the world (`world`), which stay where it put them.
#[allow(clippy::too_many_arguments)]
fn part_transform(
    replay_pose: Mat4,
    live_pose: Mat4,
    live: &Transform,
    rest: Mat4,
    wheel: Option<usize>,
    lost_with_wheel: bool,
    car: &Car,
    world: bool,
) -> Option<Transform> {
    if let Some(w) = wheel {
        if lost_with_wheel && let Some(lost) = car.lost[w] {
            return Some(Transform::from_matrix(lost.to_mat4() * rest));
        }
        return Some(Transform::from_matrix(
            replay_pose * wheel_motion(&car.wheels[w], &car.geometry[w]) * rest,
        ));
    }
    if world {
        return None;
    }
    let local = live_pose.inverse() * live.to_matrix();
    Some(Transform::from_matrix(replay_pose * local))
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
    damage: Option<Res<crate::damage_parts::DamageParts>>,
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
    // The Flashback icon and count show while a jump-in is possible.
    if let Some(f) = replay.flashback
        && f.possible
        && session.kind == Kind::Instant
    {
        let label = labels
            .as_ref()
            .map_or("FLASHBACKS AVAILABLE:", |l| l.get("flashbacks_available"));
        text.push_str(&format!("\nF12 flashback   {label} {}", f.remaining));
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
    // replayed body, from where the live car is drawn.
    let live_pose = |car: usize| driving.shown_pose(car);
    if let (Some(car), Some(live)) = (shown.cars.first(), live_pose(0)) {
        let pose = car.pose.to_mat4();
        for (part, mut transform, .., entity) in &mut parts {
            let world = damage.as_ref().is_some_and(|d| d.placed_in_world(entity));
            if let Some(t) = part_transform(pose, live, &transform, part.rest, part.wheel, part.lost_with_wheel, car, world) {
                *transform = t;
            }
        }
    }
    for (part, mut transform, .., entity) in &mut opponents {
        let (Some(car), Some(live)) = (shown.cars.get(part.car), live_pose(part.car)) else {
            continue;
        };
        let world = damage.as_ref().is_some_and(|d| d.placed_in_world(entity));
        if let Some(t) = part_transform(car.pose.to_mat4(), live, &transform, part.rest, part.wheel, part.lost_with_wheel, car, world) {
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
            for (_, _, source, handle, mut bounds, _) in &mut parts {
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
        for (part, _, deforming, handle, mut bounds, _) in &mut opponents {
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
