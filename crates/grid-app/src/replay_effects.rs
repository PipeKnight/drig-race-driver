//! Replayed ground effects (docs/REPLAY.md, "Replayed effects"). During a
//! replay the skid marks, wheel debris and tyre smoke are the race's own
//! effect systems stepped again, tick by tick, from the recorded cars (the
//! frame's [`crate::replay::frame::Effects`] words), starting at a checkpoint
//! of their state. So every replay time shows the marks and smoke it had in
//! the race, forwards, backwards and after a seek. On Replay Exit the race's
//! effects return as they were.
//!
//! The original records these as replay channels instead. Its replay setup
//! (0x64ECD0) registers recordables for "skidmarks" (SkidmarksPlugin+0x78),
//! "pfx_recordable", "broadcast: PFX scrapes", "broadcast: PFX collisions",
//! "mud" and "scratches", beside the vehicle, physics and loose parts
//! (names through 0x942460). The port re-runs the deterministic presentation
//! from the recorded inputs, which shows the same state at each replay time
//! without a stream per effect.
//!
//! A jump-in (crate::flashback) keeps the effects as they were at its frame
//! ([`adopt`]): the marks and smoke of the abandoned future go, and the
//! race lays on from the heads at the frame, with no mark across the rewind.
use crate::replay::{Replay, frame, seconds};
use crate::skid_marks::{self, SkidMarks};
use crate::tyre_smoke::{self, TyreSmoke};
use crate::wheel_debris::{self, WheelDebris};
use crate::wheel_ground::{self, Wheel};
use bevy::prelude::*;
use grid_tools::source_vehicle::DELTA;
use grid_tools::tyre_smoke as smoke;
use std::time::{Duration, Instant};

/// A live checkpoint every 2 s of race.
const LIVE_EVERY: u64 = 125;
/// Every live checkpoint of the last 20 s stays (the Instant Replay shows
/// the last 10 s); older ones only every 30 s.
const RECENT: u64 = 1250;
const SPARSE: u64 = 1875;
/// While a replay steps, a checkpoint every 0.4 s of replay, at most this
/// many, so rewinding re-runs at most 25 ticks per frame.
const LOCAL_EVERY: u64 = 25;
const LOCAL_MAX: usize = 48;
/// Re-run time per frame; a long seek catches up over several frames.
const BUDGET: Duration = Duration::from_millis(6);

#[derive(Clone)]
struct State {
    tick: u64,
    skids: Option<skid_marks::Saved>,
    debris: Option<wheel_debris::Saved>,
    smoke: Option<tyre_smoke::Saved>,
}

impl State {
    fn save(world: &World, tick: u64) -> Self {
        Self {
            tick,
            skids: world.get_resource::<SkidMarks>().map(SkidMarks::save),
            debris: world.get_resource::<WheelDebris>().map(WheelDebris::save),
            smoke: world.get_resource::<TyreSmoke>().map(TyreSmoke::save),
        }
    }

    fn apply(&self, world: &mut World) {
        if let (Some(s), Some(mut m)) = (&self.skids, world.get_resource_mut::<SkidMarks>()) {
            m.restore(s);
        }
        if let (Some(s), Some(mut d)) = (&self.debris, world.get_resource_mut::<WheelDebris>()) {
            d.restore(s);
        }
        if let (Some(s), Some(mut t)) = (&self.smoke, world.get_resource_mut::<TyreSmoke>()) {
            t.restore(s);
        }
    }

    fn bytes(&self) -> usize {
        self.skids.as_ref().map_or(0, skid_marks::Saved::bytes)
            + self.debris.as_ref().map_or(0, wheel_debris::Saved::bytes)
            + self.smoke.as_ref().map_or(0, tyre_smoke::Saved::bytes)
    }
}

/// The recorded frames around the tick being stepped.
struct Window {
    frame: usize,
    a: frame::Snapshot,
    b: Option<frame::Snapshot>,
}

#[derive(Resource, Default)]
pub struct ReplayEffects {
    segment: Option<u64>,
    /// Checkpoints of the race, by tick.
    live: Vec<State>,
    /// The race's effects when the replay started; Replay Exit restores them.
    saved: Option<State>,
    /// Checkpoints made while the replay stepped (replay data only).
    local: Vec<State>,
    /// The tick the effects show (None: not a replay tick yet).
    shown: Option<u64>,
    window: Option<Window>,
    /// This session: ticks stepped, checkpoint restores, longest frame.
    stepped: u64,
    restores: u64,
    longest: Duration,
}

pub fn install(app: &mut App) {
    app.init_resource::<ReplayEffects>()
        .add_systems(
            FixedUpdate,
            capture
                .after(crate::wheel_ground::update)
                .after(crate::skid_marks::lay)
                .after(crate::wheel_debris::simulate)
                .after(crate::tyre_smoke::tick),
        )
        .add_systems(
            Update,
            run.after(crate::flashback::update)
                .before(crate::replay::frame_camera),
        );
}

/// A live checkpoint every [`LIVE_EVERY`] ticks, after the effects ticked.
fn capture(world: &mut World) {
    if world
        .get_resource::<Replay>()
        .is_some_and(|r| r.session.is_some())
    {
        return;
    }
    let driving = world.resource::<crate::Driving>();
    let (segment, tick) = (driving.segment, driving.runtime.ticks);
    let live = !driving.paused && driving.failure.is_none();
    let mut fx = world.resource_mut::<ReplayEffects>();
    if fx.segment != Some(segment) {
        fx.segment = Some(segment);
        fx.live.clear();
    }
    if !live || tick == 0 || !tick.is_multiple_of(LIVE_EVERY) || fx.live.last().is_some_and(|c| c.tick >= tick) {
        return;
    }
    let state = State::save(world, tick);
    let mut fx = world.resource_mut::<ReplayEffects>();
    fx.live.push(state);
    fx.live
        .retain(|c| c.tick + RECENT >= tick || c.tick.is_multiple_of(SPARSE));
}

/// The replay's effects at its time, within [`BUDGET`] per frame; the race's
/// effects back on Replay Exit.
pub fn run(world: &mut World) {
    let Some(mut fx) = world.remove_resource::<ReplayEffects>() else {
        return;
    };
    let time = world
        .get_resource::<Replay>()
        .and_then(|r| r.session.as_ref())
        .map(|s| s.player.time);
    match time {
        None => {
            if let Some(saved) = fx.saved.take() {
                saved.apply(world);
                log_line!(
                    "replay_effects version=1 event=exit tick={} stepped={} restores={} longest_ms={:.2}",
                    saved.tick,
                    fx.stepped,
                    fx.restores,
                    fx.longest.as_secs_f64() * 1000.
                );
                fx.end_session();
            }
        }
        Some(time) => {
            if fx.saved.is_none() {
                fx.begin_session(world);
            }
            let started = Instant::now();
            if let Some(target) = target_tick(world, time) {
                fx.seek(world, target, Some(started + BUDGET));
            }
            fx.longest = fx.longest.max(started.elapsed());
        }
    }
    world.insert_resource(fx);
}

/// A jump-in resumes the race at its frame: the effects stay as the replay
/// shows them there, the abandoned future's marks and smoke go, and the race
/// lays on from that state (called by crate::flashback before the replay
/// ends).
pub fn adopt(world: &mut World) {
    let Some(mut fx) = world.remove_resource::<ReplayEffects>() else {
        return;
    };
    let driving = world.resource::<crate::Driving>();
    let (segment, reached) = (driving.segment, driving.runtime.ticks);
    if fx.saved.is_some() {
        fx.seek(world, reached, None);
    }
    fx.live.retain(|c| c.tick <= reached);
    if let Some(mut s) = world.get_resource_mut::<TyreSmoke>() {
        s.continue_from((segment, reached));
    }
    log_line!(
        "replay_effects version=1 event=adopt segment={segment} tick={reached} shown={:?} skid_strips={} debris={} smoke={} stepped={} restores={}",
        fx.shown,
        world.get_resource::<SkidMarks>().map_or(0, SkidMarks::used),
        world.get_resource::<WheelDebris>().map_or(0, WheelDebris::live),
        world.get_resource::<TyreSmoke>().map_or(0, TyreSmoke::particles),
        fx.stepped,
        fx.restores
    );
    fx.end_session();
    world.insert_resource(fx);
}

/// The stream tick the replay shows at `time`, inside the recorded frames.
fn target_tick(world: &World, time: f64) -> Option<u64> {
    let stream = &world.get_resource::<Replay>()?.stream;
    let frames = stream.frames();
    if frames.is_empty() {
        return None;
    }
    let first = stream.tick(frames.start);
    let last = stream.tick(frames.end - 1);
    let tick = (time / f64::from(DELTA)).max(0.).floor() as u64;
    Some(tick.clamp(first, last))
}

impl ReplayEffects {
    fn begin_session(&mut self, world: &World) {
        let driving = world.resource::<crate::Driving>();
        let tick = driving.runtime.ticks;
        if self.segment != Some(driving.segment) {
            self.segment = Some(driving.segment);
            self.live.clear();
        }
        let saved = State::save(world, tick);
        log_line!(
            "replay_effects version=1 event=start tick={tick} checkpoints={} checkpoint_bytes={} skid_strips={} debris={} smoke={}",
            self.live.len(),
            self.live.iter().map(State::bytes).sum::<usize>() + saved.bytes(),
            world.get_resource::<SkidMarks>().map_or(0, SkidMarks::used),
            world.get_resource::<WheelDebris>().map_or(0, WheelDebris::live),
            world.get_resource::<TyreSmoke>().map_or(0, TyreSmoke::particles),
        );
        self.saved = Some(saved);
        self.shown = None;
        self.stepped = 0;
        self.restores = 0;
        self.longest = Duration::ZERO;
    }

    fn end_session(&mut self) {
        self.saved = None;
        self.local.clear();
        self.shown = None;
        self.window = None;
    }

    /// Steps (or restores and steps) the effects to `target`; stops at
    /// `deadline`, keeping the tick reached.
    fn seek(&mut self, world: &mut World, target: u64, deadline: Option<Instant>) {
        if self.shown == Some(target) {
            return;
        }
        let Some(first) = world
            .get_resource::<Replay>()
            .filter(|r| !r.stream.frames().is_empty())
            .map(|r| r.stream.tick(r.stream.frames().start))
        else {
            return;
        };
        let best = self
            .live
            .iter()
            .chain(&self.local)
            .filter(|c| c.tick <= target && c.tick >= first)
            .max_by_key(|c| c.tick)
            .map(|c| c.tick);
        let from = match self.shown {
            Some(s) if s <= target && best.is_none_or(|b| s >= b) => s,
            _ => {
                self.restores += 1;
                match best {
                    Some(b) => {
                        let state = self
                            .live
                            .iter()
                            .chain(&self.local)
                            .find(|c| c.tick == b)
                            .cloned()
                            .unwrap();
                        state.apply(world);
                        b
                    }
                    None => {
                        // Nothing laid before the first recorded frame.
                        reset(world);
                        self.step(world, first);
                        first
                    }
                }
            }
        };
        self.shown = Some(from);
        let mut tick = from;
        while tick < target {
            if deadline.is_some_and(|d| Instant::now() >= d) {
                break;
            }
            tick += 1;
            self.step(world, tick);
            self.shown = Some(tick);
            if tick.is_multiple_of(LOCAL_EVERY)
                && !self.live.iter().chain(&self.local).any(|c| c.tick == tick)
            {
                self.local.push(State::save(world, tick));
                if self.local.len() > LOCAL_MAX {
                    // Drop the checkpoint farthest from where the replay is.
                    let far = (0..self.local.len())
                        .max_by_key(|&i| self.local[i].tick.abs_diff(tick))
                        .unwrap();
                    self.local.swap_remove(far);
                }
            }
        }
    }

    /// One tick of the effects with the recorded cars at `tick`.
    fn step(&mut self, world: &mut World, tick: u64) {
        let Some(cars) = self.cars_at(world, tick) else {
            return;
        };
        let (wheels, inputs) = {
            let surfaces = world.resource::<crate::Driving>().runtime.surfaces();
            let wheels: Vec<[Wheel; 4]> = cars
                .iter()
                .map(|c| wheel_ground::replayed(c, |token| surfaces.code(token)))
                .collect();
            let inputs: Vec<smoke::CarInput> = cars
                .iter()
                .enumerate()
                .map(|(i, c)| tyre_smoke::replayed_input(c, i > 0))
                .collect();
            (wheels, inputs)
        };
        let camera = world
            .query_filtered::<&GlobalTransform, With<crate::ViewerCamera>>()
            .iter(world)
            .next()
            .map(GlobalTransform::translation);
        if let Some(mut m) = world.get_resource_mut::<SkidMarks>() {
            m.step(&wheels, camera);
        }
        if let Some(mut d) = world.get_resource_mut::<WheelDebris>() {
            d.step(&wheels, camera, DELTA);
        }
        if let Some(mut s) = world.get_resource_mut::<TyreSmoke>() {
            s.step(&inputs);
        }
        self.stepped += 1;
    }

    /// The recorded cars interpolated at `tick` (as `Replay::state_at`, with
    /// the two frames kept between ticks).
    fn cars_at(&mut self, world: &mut World, tick: u64) -> Option<Vec<frame::Car>> {
        let mut replay = world.get_resource_mut::<Replay>()?;
        let i = replay.stream.at_or_before(tick)?;
        if self.window.as_ref().is_none_or(|w| w.frame != i) {
            let a = match self.window.take() {
                Some(w) if w.frame + 1 == i && w.b.is_some() => w.b.unwrap(),
                _ => replay.snapshot(i)?,
            };
            let b = (i + 1 < replay.stream.frames().end)
                .then(|| replay.snapshot(i + 1))
                .flatten();
            self.window = Some(Window { frame: i, a, b });
        }
        let w = self.window.as_ref()?;
        let Some(b) = &w.b else {
            return Some(w.a.cars.clone());
        };
        let span = (b.tick - w.a.tick).max(1) as f32;
        let t = ((tick - w.a.tick) as f32 / span).clamp(0., 1.);
        let since = (seconds(tick) - seconds(w.a.tick)) as f32;
        Some(frame::interpolate(&w.a, b, t, since).cars)
    }
}

fn reset(world: &mut World) {
    if let Some(mut m) = world.get_resource_mut::<SkidMarks>() {
        m.reset();
    }
    if let Some(mut d) = world.get_resource_mut::<WheelDebris>() {
        d.reset();
    }
    if let Some(mut s) = world.get_resource_mut::<TyreSmoke>() {
        s.reset();
    }
}
