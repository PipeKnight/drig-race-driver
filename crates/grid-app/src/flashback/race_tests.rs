//! Whole-race Flashback checks (docs/FLASHBACK.md): after a jump-in to tick T
//! the race with the same inputs equals, tick for tick, the race that never
//! flashed back; the count and the rules of the original.
use super::*;
use crate::pause_menu::race_tests::{big_stack, digest, keys, race_app_with, tap, ticks};
use crate::replay::race_tests::{frame, replay, with_replay};
use crate::replay::seconds;
use crate::{Driving, pause_menu::PauseMenu};
use grid_tools::route::{CIRCUITS, DONINGTON};

/// The race harness with replays and Flashback (recorder and checkpoints
/// after each tick, the jump-in after the replay controls).
fn with_flashback(app: App, route: &str) -> App {
    let mut app = with_replay(app, route);
    app.insert_resource(Flashback::new(Manager::new(rules::star_level(0.606), 2, false)))
        .init_resource::<Inputs>()
        .add_systems(
            Update,
            (
                capture
                    .after(crate::replay::record)
                    .after(crate::drift_event::tick),
                watch
                    .after(crate::pause_menu::update)
                    .before(crate::drive_tick),
                update
                    .after(crate::replay::control)
                    .before(crate::pause_menu::update),
            ),
        );
    app
}

/// The player's keys for tick `n`: throttle, weaving, a brake now and then.
fn keys_for(app: &mut App, n: u64) {
    let mut k = keys(app);
    k.press(KeyCode::KeyW);
    let set = |k: &mut ButtonInput<KeyCode>, key, on: bool| {
        if on {
            k.press(key);
        } else {
            k.release(key);
        }
    };
    set(&mut k, KeyCode::KeyD, (n / 50).is_multiple_of(4));
    set(&mut k, KeyCode::KeyA, (n / 50) % 4 == 2);
    set(&mut k, KeyCode::KeyS, n % 300 >= 250 && n % 300 < 270);
}

fn release(app: &mut App) {
    for key in [KeyCode::KeyW, KeyCode::KeyA, KeyCode::KeyD, KeyCode::KeyS] {
        keys(app).release(key);
    }
}

/// One frame with the keys of the next tick.
fn drive_frame(app: &mut App) {
    let n = ticks(app) + 1;
    keys_for(app, n);
    frame(app);
}

fn state(app: &App) -> (Vec<u64>, String) {
    let (cars, mut rest) = digest(app);
    if let Some(d) = app.world().get_resource::<crate::drift_event::DriftEvent>() {
        rest.push_str(&format!(" drift={:?}", d.checkpoint()));
    }
    (cars, rest)
}

/// Pause-menu Reset Vehicle (Escape, Button3, confirm).
fn menu_reset(app: &mut App) {
    release(app);
    tap(app, KeyCode::Escape);
    tap(app, KeyCode::ControlLeft);
    tap(app, KeyCode::Enter);
    assert!(!app.world().resource::<PauseMenu>().open);
}

fn flashback(app: &App) -> &Flashback {
    app.world().resource::<Flashback>()
}

/// Jump In (F12), then the frames that run the ticks again; returns the
/// jump-in tick.
fn jump_in(app: &mut App) -> u64 {
    keys(app).press(KeyCode::F12);
    frame(app);
    keys(app).release(KeyCode::F12);
    assert!(matches!(flashback(app).stage, Stage::Flash { index: 0, .. }));
    let mut frames = 0;
    while flashback(app).restoring() {
        frame(app);
        frames += 1;
        assert!(frames < 200);
    }
    let r = replay(app);
    let target = r.stream.tick(r.stream.frames().end - 1);
    assert_eq!(ticks(app), target);
    target
}

/// Waits through `flashback_flash1..3` and `replay_jump_in`; nothing ticks
/// until the race resumes (in the harness the resume frame also ticks, with
/// the keys of that tick).
fn transition(app: &mut App) -> usize {
    let tick = ticks(app);
    let mut frames = 0;
    while flashback(app).stage != Stage::Idle {
        keys_for(app, tick + 1);
        frame(app);
        frames += 1;
        assert!(frames < 400, "transition ends");
        if flashback(app).stage != Stage::Idle {
            assert_eq!(ticks(app), tick);
        }
    }
    frames
}

struct Case {
    route: &'static str,
    models: &'static [&'static str],
    laps: u32,
    drift: bool,
    reset_at: Option<u64>,
}

fn app_for(case: &Case) -> App {
    let mut app = with_flashback(race_app_with(case.route, case.models, case.laps, false), case.route);
    if case.drift {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let root = std::path::Path::new(&root);
        let settings = grid_tools::drift::Settings::load(root).unwrap();
        let route = grid_tools::route::Route::new(case.route).unwrap();
        let flags = grid_tools::drift::close_drift_flags(root, &route).unwrap();
        app.insert_resource(crate::drift_event::DriftEvent::battle(
            settings,
            flags,
            crate::pause_menu::race_tests::OPPONENTS + 1,
            case.laps,
            1,
        ))
        .add_systems(Update, crate::drift_event::tick.after(crate::drive_tick));
    }
    app
}

/// Drives `app` to tick `end` with [`keys_for`], doing the case's menu reset.
fn run(app: &mut App, case: &Case, end: u64, mut each: impl FnMut(&App)) {
    while ticks(app) < end {
        if case.reset_at == Some(ticks(app)) {
            // The confirming frame resets, then ticks.
            menu_reset(app);
            each(app);
        }
        drive_frame(app);
        each(app);
    }
}

fn check(case: &Case) {
    const NOW: u64 = 900;
    const END: u64 = 1300;
    // Reference: the same inputs, never flashed back.
    let mut reference = app_for(case);
    let mut expected: Vec<Option<(Vec<u64>, String)>> = vec![None; END as usize + 1];
    run(&mut reference, case, END, |app| {
        expected[ticks(app) as usize] = Some(state(app));
    });
    drop(reference);

    let mut app = app_for(case);
    run(&mut app, case, NOW, |_| {});
    assert_eq!(Some(state(&app)), expected[NOW as usize]);
    let checkpoints = flashback(&app).checkpoint_ticks();
    // The Instant Replay key in the race, 1.4 s of rewind, then Jump In.
    release(&mut app);
    tap(&mut app, KeyCode::Enter);
    assert!(replay(&app).session.is_some());
    keys(&mut app).press(KeyCode::F2);
    for _ in 0..20 {
        frame(&mut app);
    }
    keys(&mut app).release(KeyCode::F2);
    frame(&mut app);
    let time = replay(&app).session.as_ref().unwrap().player.time;
    assert_eq!(replay(&app).flashback.map(|f| f.possible), Some(true));
    // The frame at or before the replay time [D 0x93DDA0]; the stream is cut
    // there.
    let target = jump_in(&mut app);
    assert!(target < NOW && seconds(target) <= time + 1e-9, "{target} {time}");
    assert!(seconds(target + 3) > time, "{target} {time}");
    assert!(app.world().resource::<Driving>().paused);
    assert_eq!(flashback(&app).manager.remaining(), 3);
    eprintln!(
        "flashback route={} models={:?} drift={} now={NOW} target={target} checkpoints={checkpoints:?}",
        case.route, case.models, case.drift
    );
    if let Some(reset) = case.reset_at {
        assert!(checkpoints.contains(&reset), "a checkpoint after the menu reset");
        assert!(target > reset);
    }
    // Exactly the state the race had at that tick.
    assert_eq!(Some(state(&app)), expected[target as usize], "restored at {target}");
    let frames = transition(&mut app);
    assert!((180..=190).contains(&frames), "3 x 1 s and 2 frames: {frames}");
    assert!(replay(&app).session.is_none());
    assert!(!app.world().resource::<Driving>().paused);
    assert!(ticks(&app) <= target + 1);
    assert_eq!(Some(state(&app)), expected[ticks(&app) as usize]);
    // The same inputs again: every tick equals the race that never flashed back.
    while ticks(&app) < END {
        drive_frame(&mut app);
        let t = ticks(&app);
        assert_eq!(Some(state(&app)), expected[t as usize], "tick {t} after the jump-in to {target}");
    }
    // Recording went on after the cut.
    let r = replay(&app);
    assert_eq!(r.stream.tick(r.stream.frames().end - 1), END);
}

#[test]
#[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS / GRID_BODY_EVENT_PARAMETERS / GRID_DEFORMATION_PARAMETERS"]
fn a_flashback_resumes_the_race_tick_for_tick_on_every_circuit() {
    big_stack(|| {
        for (i, route) in CIRCUITS.into_iter().enumerate() {
            // Both cars in every field; the player's alternates.
            let models: &[&str] = if i % 2 == 0 { &["lac", "bmw"] } else { &["bmw", "lac"] };
            check(&Case {
                route,
                models,
                laps: 2,
                drift: false,
                // A pause-menu car reset between the checkpoint and the frame.
                reset_at: (route == DONINGTON).then_some(160),
            });
        }
    });
}

#[test]
#[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS / GRID_BODY_EVENT_PARAMETERS / GRID_DEFORMATION_PARAMETERS"]
fn a_flashback_in_a_drift_battle_restores_the_scores() {
    big_stack(|| {
        check(&Case {
            route: "tracks/usa/long_beach/route_1",
            models: &["bmw"],
            laps: 1,
            drift: true,
            reset_at: None,
        });
    });
}

#[test]
#[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS / GRID_BODY_EVENT_PARAMETERS / GRID_DEFORMATION_PARAMETERS"]
fn four_flashbacks_at_normal_then_none() {
    big_stack(|| {
        let mut app = with_flashback(race_app_with(DONINGTON, &["lac", "bmw"], 2, false), DONINGTON);
        assert_eq!(flashback(&app).manager.maximum, 4);
        let to = |app: &mut App, end: u64| {
            while ticks(app) < end {
                drive_frame(app);
            }
            release(app);
        };
        to(&mut app, 800);
        // At the event time: pause, scrub forward to the live end (a scrub
        // stops there; playing would loop), Jump In refused.
        tap(&mut app, KeyCode::Enter);
        tap(&mut app, KeyCode::F1);
        keys(&mut app).press(KeyCode::F3);
        for _ in 0..700 {
            frame(&mut app);
        }
        keys(&mut app).release(KeyCode::F3);
        frame(&mut app);
        let s = replay(&app).session.as_ref().unwrap();
        assert!(s.player.end - s.player.time < rules::EVENT_GUARD);
        assert_eq!(replay(&app).flashback.map(|f| f.possible), Some(false));
        tap(&mut app, KeyCode::F12);
        assert_eq!(flashback(&app).stage, Stage::Idle);
        assert_eq!(flashback(&app).manager.used, 0);
        tap(&mut app, KeyCode::Escape);
        assert!(replay(&app).session.is_none());
        for used in 1..=4 {
            to(&mut app, 800);
            if used == 2 {
                // From the pause menu's Instant Replay: the menu closes.
                tap(&mut app, KeyCode::Escape);
                tap(&mut app, KeyCode::ArrowDown);
                tap(&mut app, KeyCode::Enter);
                assert!(app.world().resource::<PauseMenu>().open);
            } else {
                tap(&mut app, KeyCode::Enter);
            }
            assert!(replay(&app).session.is_some());
            let target = jump_in(&mut app);
            assert_eq!(flashback(&app).manager.used, used);
            assert!(target < 800 - 600);
            transition(&mut app);
            assert!(!app.world().resource::<PauseMenu>().open);
            assert!(!app.world().resource::<Driving>().paused);
        }
        // None left: the replay goes on, Jump In does nothing.
        to(&mut app, 800);
        tap(&mut app, KeyCode::Enter);
        assert_eq!(replay(&app).flashback.map(|f| (f.remaining, f.possible)), Some((0, false)));
        tap(&mut app, KeyCode::F12);
        assert!(replay(&app).session.is_some());
        assert_eq!(flashback(&app).stage, Stage::Idle);
        assert_eq!(ticks(&app), 800);
        tap(&mut app, KeyCode::Escape);
        // Restart Race gives them back [I].
        tap(&mut app, KeyCode::KeyR);
        drive_frame(&mut app);
        assert_eq!(flashback(&app).manager.remaining(), 4);
    });
}

