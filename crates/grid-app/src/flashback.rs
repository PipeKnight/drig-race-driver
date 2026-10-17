//! Flashback ([research](../../../docs/FLASHBACK.md)): from an Instant
//! Replay the player jumps back into the race at the replay time.
//!
//! The original restores every recordable from the replay frame at or before
//! the replay time and cuts the replay stream there [D 0x93DDA0]. The port's
//! replay frames hold presentation only, so the same state is reached
//! exactly another way (AUTHORED): full simulation checkpoints every
//! [`CHECKPOINT_TICKS`] (and after a car reset from the pause menu), the
//! player's inputs per tick, and on a jump-in the checkpoint at or before the
//! frame's tick restored and the ticks up to it run again with the logged
//! inputs. The simulation is deterministic, so the race then goes on from
//! exactly the state it had at that frame.
//!
//! Rules [D] (`grid_tools::flashback`): the count of the difficulty row (4 at
//! Normal), none in Pro Mode, one used by each jump-in, no jump-in within
//! 0.25 s of the moment the replay was entered, only from the in-memory
//! Instant Replay (in the race, from the pause menu and the wreck screen's
//! replay; not from View Replay). Then `flashback_flash1..3` (1 s each) and
//! `replay_jump_in` (2 frames) before the race resumes.
use crate::input::{Action, Actions};
use crate::replay::{FlashbackStatus, Kind, Replay};
use bevy::prelude::*;
use grid_tools::flashback::{self as rules, Manager};
use grid_tools::source_vehicle::{Controls, DELTA, Runtime, assists::Shift};
use std::collections::VecDeque;

/// AUTHORED checkpoint interval: 4 s of simulation; a jump-in runs at most
/// this many ticks again, spread over frames ([`RUN_BUDGET`]).
pub const CHECKPOINT_TICKS: u64 = 250;

/// AUTHORED real time per frame for running ticks again; the transition's
/// first flash holds until they are done.
pub const RUN_BUDGET: std::time::Duration = std::time::Duration::from_millis(8);

/// The player's inputs of each tick: logged in the race, given back while
/// ticks run again ([`player`]).
#[derive(Resource, Default)]
pub struct Inputs {
    log: VecDeque<(u64, Controls, Option<Shift>)>,
    again: Option<VecDeque<(u64, Controls, Option<Shift>)>>,
}

/// `drive_tick`'s player input for `tick`: the live controls and gear
/// request, logged; or while ticks run again, the logged ones.
pub fn player(
    inputs: Option<&mut Inputs>,
    tick: u64,
    live: Controls,
    shift: Option<Shift>,
) -> (Controls, Option<Shift>) {
    let Some(inputs) = inputs else {
        return (live, shift);
    };
    if let Some(again) = &mut inputs.again {
        while again.front().is_some_and(|e| e.0 < tick) {
            again.pop_front();
        }
        return match again.front() {
            Some(&(t, controls, shift)) if t == tick => {
                again.pop_front();
                (controls, shift)
            }
            _ => (Controls::default(), None),
        };
    }
    inputs.log.push_back((tick, live, shift));
    (live, shift)
}

/// The race's whole simulation state after a tick.
#[derive(Clone)]
struct Checkpoint {
    tick: u64,
    /// Taken after a change outside the tick (a pause-menu car reset) at
    /// this tick: the frame of this tick shows the state before it.
    after_change: bool,
    runtime: Runtime,
    geometry: Option<grid_tools::body_geometry::BodyGeometry>,
    auto_reset: grid_tools::source_vehicle::player_reset::PlayerReset,
    last_controls: Option<Controls>,
    failure: Option<String>,
    notices: Vec<String>,
    opponents: Option<Field>,
    trial: Option<(grid_tools::time_trial::State, Option<u64>)>,
    damage: Option<crate::damage_parts::Saved>,
    drift: Option<(Vec<grid_tools::drift::Entry>, u64, u64)>,
}

#[derive(Clone)]
struct Field {
    multi: grid_tools::source_vehicle::multi::MultiRuntime,
    drivers: Vec<grid_tools::ai_driver::AiDriver>,
    standings: grid_tools::race::Standings,
    autopilot: Option<grid_tools::ai_driver::AiDriver>,
    after_finish: Option<grid_tools::ai_driver::AiDriver>,
    throttles: Vec<f32>,
    pacing: grid_tools::ai_pacing::Field,
}

impl Checkpoint {
    fn take(
        driving: &crate::Driving,
        trial: Option<&crate::time_trial::Trial>,
        damage: Option<&crate::damage_parts::DamageParts>,
        drift: Option<&crate::drift_event::DriftEvent>,
        after_change: bool,
    ) -> Self {
        Self {
            tick: driving.runtime.ticks,
            after_change,
            runtime: driving.runtime.clone(),
            geometry: driving.geometry.clone(),
            auto_reset: driving.auto_reset,
            last_controls: driving.last_controls,
            failure: driving.failure.clone(),
            notices: driving.notices.clone(),
            opponents: driving.opponents.as_ref().map(|o| Field {
                multi: o.multi.clone(),
                drivers: o.drivers.clone(),
                standings: o.standings.clone(),
                autopilot: o.autopilot.clone(),
                after_finish: o.after_finish.clone(),
                throttles: o.throttles.clone(),
                pacing: o.pacing.clone(),
            }),
            trial: trial.map(|t| t.checkpoint()),
            damage: damage.map(|d| d.checkpoint()),
            drift: drift.map(|d| d.checkpoint()),
        }
    }

    fn apply(&self, world: &mut World) {
        let mut driving = world.resource_mut::<crate::Driving>();
        let driving = &mut *driving;
        driving.runtime = self.runtime.clone();
        driving.geometry = self.geometry.clone();
        driving.auto_reset = self.auto_reset;
        driving.last_controls = self.last_controls;
        driving.failure = self.failure.clone();
        driving.notices = self.notices.clone();
        if let (Some(o), Some(f)) = (&mut driving.opponents, &self.opponents) {
            o.multi = f.multi.clone();
            o.drivers = f.drivers.clone();
            o.standings = f.standings.clone();
            o.autopilot = f.autopilot.clone();
            o.after_finish = f.after_finish.clone();
            o.throttles = f.throttles.clone();
            o.pacing = f.pacing.clone();
        }
        if let (Some(mut t), Some(saved)) = (world.get_resource_mut::<crate::time_trial::Trial>(), &self.trial) {
            t.restore(saved);
        }
        if let (Some(mut d), Some(saved)) = (world.get_resource_mut::<crate::damage_parts::DamageParts>(), &self.damage) {
            d.restore(saved);
        }
        if let (Some(mut d), Some(saved)) = (world.get_resource_mut::<crate::drift_event::DriftEvent>(), &self.drift) {
            d.restore(saved);
        }
    }

    /// Approximate size (bytes) of the copied state: [vehicle words,
    /// deformation owners, frames, bodies and contacts, solver rows, other]
    /// (diagnostics).
    fn bytes(&self) -> [usize; 6] {
        fn world(w: &grid_world::grouped_physical_frame::Output, out: &mut [usize; 6]) {
            out[3] += std::mem::size_of_val(&w.groups.bodies[..]) + std::mem::size_of_val(&w.groups.contacts[..]);
            let m = &w.manager;
            out[4] += std::mem::size_of_val(&m.rows[..])
                + std::mem::size_of_val(&m.materials[..])
                + std::mem::size_of_val(&m.properties[..])
                + std::mem::size_of_val(&m.workspace[..]);
        }
        fn car(r: &Runtime, out: &mut [usize; 6]) {
            out[0] += r.state.car.owner.vehicle.len() * 4;
            out[1] += r.deformation_registration.as_ref().map_or(0, |d| d.owner.len() * 4);
            out[2] += std::mem::size_of_val(&r.state.car.frame);
            world(&r.state.world, out);
            out[5] += std::mem::size_of::<Runtime>() + std::mem::size_of_val(&r.state.shapes[..]);
        }
        let mut out = [0; 6];
        car(&self.runtime, &mut out);
        if let Some(f) = &self.opponents {
            for c in &f.multi.cars {
                car(c, &mut out);
            }
            out[3] += std::mem::size_of_val(f.multi.bodies());
        }
        out
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Stage {
    Idle,
    /// `flashback_flash1..3`: which and for how long (real seconds).
    Flash { index: usize, elapsed: f32 },
    /// `replay_jump_in frameDelay="2"`: frames waited.
    JumpIn { frames: u32 },
}

#[derive(Resource)]
pub struct Flashback {
    pub manager: Manager,
    pub stage: Stage,
    checkpoints: VecDeque<Checkpoint>,
    segment: Option<u64>,
    /// The player's tick, pose and velocity after the last tick (a change
    /// outside the tick shows as a difference at the same tick).
    last: Option<(u64, [f32; 16], [f32; 4])>,
    /// The last refusal logged, to log each once.
    refused: Option<&'static str>,
    /// A jump-in still running ticks again.
    pending: Option<Pending>,
    /// Diagnostics `flashback_capture` jumped in.
    diagnostic: bool,
}

impl Flashback {
    pub fn new(manager: Manager) -> Self {
        Self {
            manager,
            stage: Stage::Idle,
            checkpoints: VecDeque::new(),
            segment: None,
            last: None,
            refused: None,
            pending: None,
            diagnostic: false,
        }
    }

    /// The checkpoint to run again from for a jump-in at `tick`.
    fn base(&self, tick: u64) -> Option<&Checkpoint> {
        self.checkpoints
            .iter()
            .rev()
            .find(|c| c.tick < tick || (c.tick == tick && !c.after_change))
    }

    /// Ticks of a jump-in are still being run again.
    #[cfg(test)]
    pub fn restoring(&self) -> bool {
        self.pending.is_some()
    }

    #[cfg(test)]
    pub fn checkpoint_ticks(&self) -> Vec<u64> {
        self.checkpoints.iter().map(|c| c.tick).collect()
    }
}

/// Overlay of the flashback transition.
#[derive(Component)]
struct Whiteout;

pub fn install(app: &mut App, ai_difficulty: f32) {
    let manager = Manager::new(rules::star_level(ai_difficulty), 2, false);
    log_line!(
        "drive_flashback version=1 event=install maximum={} max_award={} checkpoint_ticks={CHECKPOINT_TICKS}",
        manager.maximum,
        manager.max_award
    );
    app.insert_resource(Flashback::new(manager))
        .init_resource::<Inputs>()
        .add_systems(
            FixedUpdate,
            capture
                .after(crate::replay::record)
                .after(crate::damage_parts::tick)
                .after(crate::drift_event::tick),
        )
        .add_systems(
            Update,
            (
                watch
                    .after(crate::pause_menu::update)
                    .after(crate::post_race::update),
                update
                    .after(crate::replay::control)
                    .before(crate::replay::frame_camera),
            ),
        );
}

fn pose(r: &Runtime) -> ([f32; 16], [f32; 4]) {
    (r.pose(), r.velocity())
}

/// After each tick: a checkpoint every [`CHECKPOINT_TICKS`], kept while the
/// Instant Replay window reaches back to it.
pub fn capture(
    mut flashback: ResMut<Flashback>,
    mut inputs: ResMut<Inputs>,
    driving: Res<crate::Driving>,
    trial: Option<Res<crate::time_trial::Trial>>,
    damage: Option<Res<crate::damage_parts::DamageParts>>,
    drift: Option<Res<crate::drift_event::DriftEvent>>,
    replay: Option<Res<Replay>>,
) {
    let flashback = &mut *flashback;
    // A jump-in running its ticks again owns the history.
    if flashback.pending.is_some() {
        return;
    }
    let tick = driving.runtime.ticks;
    // Restart Race: a new race with its flashbacks [I].
    if flashback.segment != Some(driving.segment)
        || flashback.last.is_some_and(|(t, ..)| tick < t)
    {
        if flashback.segment.is_some() {
            flashback.manager.used = 0;
        }
        flashback.segment = Some(driving.segment);
        flashback.checkpoints.clear();
        flashback.last = None;
        inputs.log.clear();
    }
    if tick == 0 || flashback.last.is_some_and(|(t, ..)| t == tick) {
        return;
    }
    let (p, v) = pose(&driving.runtime);
    flashback.last = Some((tick, p, v));
    if flashback.checkpoints.is_empty() || tick.is_multiple_of(CHECKPOINT_TICKS) {
        let c = Checkpoint::take(
            &driving,
            trial.as_deref(),
            damage.as_deref(),
            drift.as_deref(),
            false,
        );
        if flashback.checkpoints.is_empty() {
            log_line!(
                "drive_flashback version=1 event=checkpoint segment={} tick={tick} approx_bytes={:?}",
                driving.segment,
                c.bytes()
            );
        }
        flashback.checkpoints.push_back(c);
    }
    // Forget checkpoints the in-memory replay window no longer reaches.
    if let Some(replay) = replay {
        let frames = replay.stream.frames();
        let start = replay.stream.memory_start();
        if start < frames.end {
            let oldest = replay.stream.tick(start);
            while flashback.checkpoints.len() > 1 && flashback.checkpoints[1].tick <= oldest {
                flashback.checkpoints.pop_front();
            }
        }
    }
    let first = flashback.checkpoints.front().map_or(0, |c| c.tick);
    while inputs.log.front().is_some_and(|e| e.0 <= first) {
        inputs.log.pop_front();
    }
}

/// A car reset from the pause menu changes the player's car outside the
/// tick: a checkpoint of the changed state, so ticks run again never cross
/// the change.
pub fn watch(
    mut flashback: ResMut<Flashback>,
    driving: Res<crate::Driving>,
    trial: Option<Res<crate::time_trial::Trial>>,
    damage: Option<Res<crate::damage_parts::DamageParts>>,
    drift: Option<Res<crate::drift_event::DriftEvent>>,
) {
    let Some((tick, p, v)) = flashback.last else {
        return;
    };
    if flashback.pending.is_some()
        || flashback.segment != Some(driving.segment)
        || driving.runtime.ticks != tick
    {
        return;
    }
    let now = pose(&driving.runtime);
    if now == (p, v) {
        return;
    }
    flashback.last = Some((tick, now.0, now.1));
    log_line!(
        "drive_flashback version=1 event=checkpoint_after_change segment={} tick={tick}",
        driving.segment
    );
    let c = Checkpoint::take(
        &driving,
        trial.as_deref(),
        damage.as_deref(),
        drift.as_deref(),
        true,
    );
    flashback.checkpoints.push_back(c);
}

/// Whether a jump-in is possible now, and to which tick.
fn target(world: &World) -> Result<u64, &'static str> {
    let flashback = world.resource::<Flashback>();
    let replay = world.get_resource::<Replay>().ok_or("no replay")?;
    let session = replay.session.as_ref().ok_or("no replay")?;
    if session.kind != Kind::Instant {
        return Err("not an instant replay");
    }
    if !flashback.manager.available() {
        return Err("no flashbacks left");
    }
    if world
        .get_resource::<crate::time_trial::Trial>()
        .is_some_and(|t| t.finished())
    {
        return Err("race finished");
    }
    let driving = world.resource::<crate::Driving>();
    let time = session.player.time;
    if !rules::time_allowed(time, crate::replay::seconds(driving.runtime.ticks)) {
        return Err("at the event time");
    }
    let frame = replay
        .stream
        .at_or_before((time / f64::from(DELTA)).max(0.).floor() as u64)
        .ok_or("no frame")?;
    if frame < replay.stream.memory_start() {
        return Err("before the memory window");
    }
    let tick = replay.stream.tick(frame);
    if tick > driving.runtime.ticks {
        return Err("after the live race");
    }
    flashback.base(tick).ok_or("no checkpoint")?;
    Ok(tick)
}

/// The jump-in, its transition and the resume.
pub fn update(world: &mut World) {
    let dt = world
        .get_resource::<Time<Real>>()
        .map_or(0., |t| t.delta_secs())
        .min(0.1);
    match world.resource::<Flashback>().stage {
        Stage::Idle => {
            let target = target(world);
            let remaining = world.resource::<Flashback>().manager.remaining();
            if let Some(mut replay) = world.get_resource_mut::<Replay>() {
                replay.flashback = Some(FlashbackStatus {
                    remaining,
                    possible: target.is_ok(),
                });
            }
            let mut pressed = world
                .get_resource::<Actions>()
                .is_some_and(|a| a.pressed(Action::ReplayJumpIn));
            // Diagnostics flashback_capture: one jump-in that far into the replay.
            if let Some(after) = crate::settings::diagnostics().flashback_capture
                && !world.resource::<Flashback>().diagnostic
                && world.get_resource::<Replay>().and_then(|r| r.session.as_ref()).is_some_and(|s| s.player.time - s.player.loop_from >= after)
            {
                world.resource_mut::<Flashback>().diagnostic = true;
                pressed = true;
            }
            let in_replay = world
                .get_resource::<Replay>()
                .is_some_and(|r| r.session.is_some());
            if !pressed || !in_replay {
                return;
            }
            match target {
                Ok(tick) => jump_in(world, tick),
                Err(reason) => {
                    let mut f = world.resource_mut::<Flashback>();
                    if f.refused != Some(reason) {
                        f.refused = Some(reason);
                        log_line!("drive_flashback version=1 event=refused reason={reason:?}");
                    }
                }
            }
        }
        Stage::Flash { index, elapsed } => {
            // The flash holds at full white while ticks run again.
            if world.resource::<Flashback>().pending.is_some() {
                advance(world);
                return;
            }
            let elapsed = elapsed + dt;
            let stage = if elapsed >= rules::FLASH_SECONDS {
                if index + 1 >= rules::FLASHES {
                    Stage::JumpIn { frames: 0 }
                } else {
                    Stage::Flash {
                        index: index + 1,
                        elapsed: 0.,
                    }
                }
            } else {
                Stage::Flash { index, elapsed }
            };
            world.resource_mut::<Flashback>().stage = stage;
            // IG_whiteout then FlashbackFX_01 over 1 s (AUTHORED: white fading out).
            let alpha = match stage {
                Stage::Flash { elapsed, .. } => (1. - elapsed / rules::FLASH_SECONDS).clamp(0., 1.) * 0.85,
                _ => 0.,
            };
            let mut q = world.query_filtered::<&mut BackgroundColor, With<Whiteout>>();
            for mut c in q.iter_mut(world) {
                c.0 = Color::srgba(1., 1., 1., alpha);
            }
        }
        Stage::JumpIn { frames } => {
            if frames + 1 < rules::JUMP_IN_FRAMES {
                world.resource_mut::<Flashback>().stage = Stage::JumpIn { frames: frames + 1 };
                return;
            }
            resume(world);
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Pending {
    base: u64,
    target: u64,
    from: u64,
    started: std::time::Instant,
}

fn jump_in(world: &mut World, tick: u64) {
    let now = world.resource::<crate::Driving>().runtime.ticks;
    let segment = world.resource::<crate::Driving>().segment;
    // StateReplayJumpInPrepare uses one [D 0x59AFA0].
    world.resource_mut::<Flashback>().manager.use_one();
    // The checkpoint at or before the frame, then its ticks run again.
    let base = world.resource::<Flashback>().base(tick).cloned().unwrap();
    base.apply(world);
    let again: VecDeque<_> = world
        .resource::<Inputs>()
        .log
        .iter()
        .filter(|e| e.0 > base.tick && e.0 <= tick)
        .copied()
        .collect();
    world.resource_mut::<Inputs>().again = Some(again);
    if let Some(mut shifts) = world.get_resource_mut::<crate::input::ShiftQueue>() {
        *shifts = Default::default();
    }
    // As the original, the replay is cut at the frame [D 0x93DDA0].
    let mut replay = world.resource_mut::<Replay>();
    let cut = replay.cut(segment, tick);
    replay.freeze(crate::replay::seconds(tick));
    let remaining = {
        let mut f = world.resource_mut::<Flashback>();
        f.stage = Stage::Flash {
            index: 0,
            elapsed: 0.,
        };
        f.refused = None;
        f.pending = Some(Pending {
            base: base.tick,
            target: tick,
            from: now,
            started: std::time::Instant::now(),
        });
        f.manager.remaining()
    };
    log_line!(
        "drive_flashback version=1 event=jump_in segment={segment} from={now} target={tick} checkpoint={} cut={cut} remaining={remaining}",
        base.tick
    );
    let camera = world
        .query_filtered::<Entity, With<crate::ViewerCamera>>()
        .iter(world)
        .next();
    if let Some(camera) = camera {
        world.spawn((
            Whiteout,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100.),
                height: percent(100.),
                ..default()
            },
            BackgroundColor(Color::srgba(1., 1., 1., 0.85)),
            GlobalZIndex(10),
            UiTargetCamera(camera),
        ));
    }
    advance(world);
}

/// Runs ticks of the pending jump-in again with the logged player inputs
/// (`drive_tick`, damage parts, drift scorers), for up to [`RUN_BUDGET`].
fn advance(world: &mut World) {
    let Some(pending) = world.resource::<Flashback>().pending else {
        return;
    };
    let started = std::time::Instant::now();
    let damage = world.contains_resource::<crate::damage_parts::DamageParts>();
    let drift = world.contains_resource::<crate::drift_event::DriftEvent>();
    world.resource_mut::<crate::Driving>().paused = false;
    let mut result = Ok(());
    while world.resource::<crate::Driving>().runtime.ticks < pending.target {
        let before = world.resource::<crate::Driving>().runtime.ticks;
        let mut step = || -> Result<(), String> {
            world.run_system_cached(crate::drive_tick).map_err(|e| e.to_string())?;
            if damage {
                world
                    .run_system_cached(crate::damage_parts::tick)
                    .map_err(|e| e.to_string())?;
            }
            if drift {
                world
                    .run_system_cached(crate::drift_event::tick)
                    .map_err(|e| e.to_string())?;
            }
            Ok(())
        };
        if let Err(e) = step() {
            result = Err(e);
            break;
        }
        if world.resource::<crate::Driving>().runtime.ticks == before {
            result = Err(format!("the race did not tick at {before}"));
            break;
        }
        if started.elapsed() >= RUN_BUDGET {
            break;
        }
    }
    world.resource_mut::<crate::Driving>().paused = true;
    let reached = world.resource::<crate::Driving>().runtime.ticks;
    if result.is_ok() && reached < pending.target {
        return;
    }
    // Done (or, not expected, stopped): the race holds at the reached tick.
    let mut inputs = world.resource_mut::<Inputs>();
    inputs.again = None;
    inputs.log.retain(|e| e.0 <= reached);
    let driving = world.resource::<crate::Driving>();
    let segment = driving.segment;
    let last = pose(&driving.runtime);
    let mut f = world.resource_mut::<Flashback>();
    f.pending = None;
    f.checkpoints
        .retain(|c| c.tick < reached || (c.tick == reached && !c.after_change));
    f.last = Some((reached, last.0, last.1));
    match result {
        Ok(()) => log_line!(
            "drive_flashback version=1 event=restored segment={segment} from={} target={} checkpoint={} ran={} ms={:.1}",
            pending.from,
            pending.target,
            pending.base,
            reached - pending.base,
            pending.started.elapsed().as_secs_f64() * 1000.
        ),
        Err(error) => log_line!(
            "drive_flashback version=1 event=failed segment={segment} target={} reached={reached} error={error:?}",
            pending.target
        ),
    }
}

/// After the transition: the replay ends and the race goes on.
fn resume(world: &mut World) {
    world.resource_mut::<Flashback>().stage = Stage::Idle;
    let origin = world.resource_mut::<Replay>().finish();
    world.resource_mut::<crate::Driving>().paused = false;
    if let Some(mut menu) = world.get_resource_mut::<crate::pause_menu::PauseMenu>()
        && menu.open
    {
        menu.close();
    }
    if let Some(mut post) = world.get_resource_mut::<crate::post_race::PostRace>() {
        post.resume_race();
    }
    let overlays: Vec<Entity> = world
        .query_filtered::<Entity, With<Whiteout>>()
        .iter(world)
        .collect();
    for e in overlays {
        world.despawn(e);
    }
    let driving = world.resource::<crate::Driving>();
    log_line!(
        "drive_flashback version=1 event=resume segment={} tick={} origin={origin:?}",
        driving.segment,
        driving.runtime.ticks
    );
}

#[cfg(test)]
mod race_tests;
