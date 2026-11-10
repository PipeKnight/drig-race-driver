//! Whole-race replay checks (docs/REPLAY.md) with the shared world, the AI
//! field and the race manager, on the pause menu's race harness.
use super::*;
use crate::pause_menu::race_tests::{
    big_stack, digest, drive_to, keys, race_app_with, tap, ticks,
};
use crate::{Driving, pause_menu::PauseMenu, post_race::PostRace};
use grid_tools::route::{DONINGTON, ISTANBUL_PARK, LONG_BEACH, NURBURGRING, Route};
use std::time::Duration;

/// The wreck screen's replay asks for the terminal-damage session; its
/// director takes each zone's `TERMINAL` camera and keeps the ordinary one
/// where a zone has none. Any other request is an ordinary replay.
#[test]
fn terminal_replay_uses_the_zones_terminal_cameras() {
    use director::{Activation, Director, Zone};
    let zone = |activation, terminal| Zone {
        centre: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        half: Vec3::ONE,
        activation,
        terminal,
    };
    let director = Director {
        cameras: Vec::new(),
        zones: vec![
            zone(Some(Activation::Track(2)), Some(Activation::Track(1))),
            zone(Some(Activation::Track(3)), None),
        ],
        initial: Some(Activation::Track(0)),
    };
    let mut r = Replay::new(stream::Stream::new(stream::Limits::default(), None), Some(director));
    r.entries = vec![(10, 0), (20, 1)];
    let at = |r: &Replay, tick, terminal| r.activation_for(tick, terminal).map(|(a, _)| a);
    assert_eq!(at(&r, 5, true), Some(Activation::Track(0)));
    assert_eq!(at(&r, 15, false), Some(Activation::Track(2)));
    assert_eq!(at(&r, 15, true), Some(Activation::Track(1)));
    assert_eq!(at(&r, 25, true), Some(Activation::Track(3)));
    r.request_terminal(Some(30.));
    assert_eq!(r.request, Some((Kind::Instant, Origin::Results)));
    assert_eq!(r.terminal_request, Some(Some(30.)));
    // Nothing recorded: no session, and the flag does not outlive the request.
    let (kind, origin) = r.request.take().unwrap();
    assert!(!r.start(kind, origin, false, &crate::camera::Views::fallback()));
    assert!(r.terminal_request.is_none());
    r.request_terminal(None);
    r.request(Kind::Full, Origin::Results);
    assert!(r.terminal_request.is_none());
    // The window starts 10 s before the crash, within the recording.
    assert_eq!(super::terminal_start(5., 60., 30.), 20.);
    assert_eq!(super::terminal_start(25., 60., 30.), 25.);
    assert_eq!(super::terminal_start(5., 60., 75.), 60.);
}

/// The race harness plus the recorder after each tick and the replay
/// controls before the menus; real time advances 1/60 s per frame.
pub(crate) fn with_replay(mut app: App, route: &str) -> App {
    let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
    let director =
        director::Director::load(std::path::Path::new(&root), &Route::new(route).unwrap()).unwrap();
    app.insert_resource(Replay::new(
        stream::Stream::new(stream::Limits::default(), None),
        Some(director),
    ))
    .insert_resource(Time::<Real>::default())
    .add_systems(
        Update,
        (
            control
                .after(crate::input::sample)
                .before(crate::post_race::update),
            record.after(crate::drive_tick),
        ),
    );
    app
}

pub(crate) fn frame(app: &mut App) {
    app.world_mut()
        .resource_mut::<Time<Real>>()
        .update_with_duration(Duration::from_secs_f64(1. / 60.));
    app.update();
}

pub(crate) fn replay(app: &App) -> &Replay {
    app.world().resource::<Replay>()
}

/// Every car's live state at a tick: pose words, velocity and the live
/// presentation's body and wheel matrices (drive_present's formula).
struct Live {
    tick: u64,
    poses: Vec<[f32; 16]>,
    velocities: Vec<[f32; 3]>,
    wheels: Vec<[Mat4; 4]>,
    pieces: Option<usize>,
    /// What the ground effects and the tyre smoke read of each car.
    ground: Vec<[crate::wheel_ground::Wheel; 4]>,
    smoke: Vec<grid_tools::tyre_smoke::CarInput>,
}

fn live(app: &App) -> Live {
    let d = app.world().resource::<Driving>();
    let o = d.opponents.as_ref().unwrap();
    let cars: Vec<&Runtime> = std::iter::once(&d.runtime)
        .chain(o.multi.cars.iter().skip(1))
        .collect();
    Live {
        tick: d.runtime.ticks,
        poses: cars.iter().map(|c| c.pose()).collect(),
        velocities: cars
            .iter()
            .map(|c| {
                let v = c.velocity();
                [v[0], v[1], v[2]]
            })
            .collect(),
        wheels: cars
            .iter()
            .map(|c| {
                let pose = Mat4::from_cols_array(&c.pose());
                let motion =
                    crate::wheel_presentation::wheels(c, 0.).map(crate::wheel_motion);
                std::array::from_fn(|k| pose * motion[k])
            })
            .collect(),
        pieces: o
            .multi
            .dynamic_props()
            .map(|p| p.poses(o.multi.bodies()).len()),
        ground: cars.iter().map(|c| crate::wheel_ground::sample(c)).collect(),
        smoke: cars
            .iter()
            .enumerate()
            .map(|(i, c)| grid_tools::tyre_smoke::CarInput::read(c, i > 0))
            .collect(),
    }
}

/// The recorded frames decode to the live race bit for bit, and the
/// replayed presentation equals the live one at every sample.
fn check(app: &mut App, lives: &[Live]) {
    let world = app.world_mut();
    let surfaces = world.resource::<Driving>().runtime.surfaces().clone();
    let mut replay = world.resource_mut::<Replay>();
    let frames = replay.stream.frames();
    assert_eq!(frames.len(), lives.len());
    for (f, l) in frames.zip(lives) {
        let s = replay.snapshot(f).unwrap();
        assert_eq!(s.tick, l.tick);
        assert_eq!(s.cars.len(), l.poses.len());
        for (i, car) in s.cars.iter().enumerate() {
            let pose = l.poses[i];
            // A rigid pose: the last row is exact, so the 3x4 record is too.
            assert_eq!([pose[3], pose[7], pose[11], pose[15]], [0., 0., 0., 1.]);
            assert_eq!(car.pose.to_mat4().to_cols_array(), pose, "tick {} car {i}", l.tick);
            assert_eq!(car.velocity.to_array(), l.velocities[i]);
            for k in 0..4 {
                let replayed =
                    car.pose.to_mat4() * wheel_motion(&car.wheels[k], &car.geometry[k]);
                assert_eq!(replayed, l.wheels[i][k], "tick {} car {i} wheel {k}", l.tick);
            }
            // The replayed effects read the race's own inputs
            // (crate::replay_effects).
            assert_eq!(
                crate::wheel_ground::replayed(car, |t| surfaces.code(t)),
                l.ground[i],
                "ground tick {} car {i}",
                l.tick
            );
            assert_eq!(
                crate::tyre_smoke::replayed_input(car, i > 0),
                l.smoke[i],
                "smoke tick {} car {i}",
                l.tick
            );
        }
        assert_eq!(s.props.as_ref().map(|p| p.pieces.len()), l.pieces);
        // Exactly the sample, also through the interpolating reader.
        let at = replay.state_at(seconds(l.tick)).unwrap();
        assert_eq!(at.cars[0].pose, s.cars[0].pose);
    }
}

/// Runs the race to `end`, capturing the live state at every sample tick
/// (the first recorded tick is 1, after the first tick).
fn run(app: &mut App, end: u64) -> Vec<Live> {
    let mut lives = Vec::new();
    let mut last = None;
    while ticks(app) < end {
        frame(app);
        let t = ticks(app);
        if sample_due(last, t) {
            lives.push(live(app));
            last = Some(t);
        }
    }
    lives
}

// One test per circuit, so the circuits run in parallel (as one loop: 42 s).
macro_rules! recorded_frames_on {
    ($($name:ident = $route:ident),* $(,)?) => {$(
        #[test]
        #[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS / GRID_BODY_EVENT_PARAMETERS / GRID_DEFORMATION_PARAMETERS"]
        fn $name() {
            recorded_frames_equal_the_live_race($route);
        }
    )*};
}
recorded_frames_on!(
    recorded_frames_equal_the_live_race_on_donington = DONINGTON,
    recorded_frames_equal_the_live_race_on_nurburgring = NURBURGRING,
    recorded_frames_equal_the_live_race_on_istanbul_park = ISTANBUL_PARK,
    recorded_frames_equal_the_live_race_on_long_beach = LONG_BEACH,
);

fn recorded_frames_equal_the_live_race(route: &'static str) {
    big_stack(move || {
        // Both cars in the field; the AI drives the player's car too.
        let mut app = with_replay(race_app_with(route, &["lac", "bmw"], 2, true), route);
        let lives = run(&mut app, 1600);
        check(&mut app, &lives);
        let r = replay(&app);
        // 30 Hz of simulation time: 12 samples per 25 ticks.
        let frames = r.stream.frames().len() as u64;
        assert_eq!(frames, 1600 * 12 / 25 + 1, "{route}");
        assert!(r.stream.memory_bytes() <= stream::MEMORY_BYTES, "{route}");
        // The director's zones are passed by the car on the racing line.
        let entries = r.entries.len();
        // Nearest approach of the player's path to each zone centre.
        let path: Vec<Vec3> = lives
            .iter()
            .map(|l| Vec3::new(l.poses[0][12], l.poses[0][13], l.poses[0][14]))
            .collect();
        for (z, zone) in r.director.as_ref().unwrap().zones.iter().enumerate().take(6) {
            let near = path
                .iter()
                .min_by(|a, b| {
                    a.xz().distance(zone.centre.xz()).total_cmp(&b.xz().distance(zone.centre.xz()))
                })
                .unwrap();
            eprintln!(
                "replay_zone route={route} zone={z} centre={} nearest={near} local={}",
                zone.centre,
                zone.rotation.inverse() * (*near - zone.centre)
            );
        }
        eprintln!(
            "replay_route route={route} frames={frames} bytes={} zone_entries={entries} first={:?}",
            r.stream.memory_bytes(),
            r.entries.first()
        );
        assert!(entries >= 3, "{route}: {entries} zone entries");
        assert!(matches!(
            r.activation(1600),
            Some(director::Activation::Track(_) | director::Activation::Car(_))
        ));
    });
}

#[test]
#[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS / GRID_BODY_EVENT_PARAMETERS / GRID_DEFORMATION_PARAMETERS"]
fn instant_replay_leaves_the_race_bit_identical() {
    big_stack(|| {
        const PAUSE_AT: u64 = 900;
        const END: u64 = 1200;
        // Reference: no recorder at all.
        let mut straight = race_app_with(DONINGTON, &["lac", "bmw"], 2, false);
        drive_to(&mut straight, END);
        let expected = digest(&straight);

        let mut app = with_replay(race_app_with(DONINGTON, &["lac", "bmw"], 2, false), DONINGTON);
        drive_to(&mut app, PAUSE_AT);
        let before = digest(&app);
        keys(&mut app).release(KeyCode::KeyD);
        // Pause, Down to Instant Replay, select.
        tap(&mut app, KeyCode::Escape);
        tap(&mut app, KeyCode::ArrowDown);
        tap(&mut app, KeyCode::Enter);
        {
            let r = replay(&app);
            let s = r.session.as_ref().expect("instant replay");
            assert_eq!((s.kind, s.origin), (Kind::Instant, Origin::PauseMenu));
            let end = seconds(r.stream.tick(r.stream.frames().end - 1));
            assert!((s.player.end - end).abs() < 1e-9);
            assert!((s.player.loop_from - (end - INSTANT_REWIND)).abs() < 1e-9);
            assert!(app.world().resource::<Driving>().paused);
            assert!(app.world().resource::<PauseMenu>().open);
        }
        // 12 s of replay: it loops back after 10 s; fast forward and rewind
        // in between, then change the camera.
        let mut looped = 0;
        let mut previous = replay(&app).session.as_ref().unwrap().player.time;
        for i in 0..720 {
            let key = match i {
                100..=130 => Some(KeyCode::F3),
                200..=230 => Some(KeyCode::F2),
                _ => None,
            };
            if let Some(key) = key {
                keys(&mut app).press(key);
            }
            frame(&mut app);
            if let Some(key) = key {
                keys(&mut app).release(key);
            }
            let now = replay(&app).session.as_ref().unwrap().player.time;
            if now < previous - 1. && !(200..=231).contains(&i) {
                looped += 1;
            }
            previous = now;
        }
        assert!(looped >= 1, "the replay loops at its end");
        tap(&mut app, KeyCode::F8);
        assert_eq!(replay(&app).session.as_ref().unwrap().camera, 1);
        // Nothing in the race moved.
        assert_eq!(digest(&app), before);
        assert_eq!(ticks(&app), PAUSE_AT);
        // Replay Exit: back to the open pause menu, still paused.
        tap(&mut app, KeyCode::Escape);
        assert!(replay(&app).session.is_none());
        assert!(app.world().resource::<PauseMenu>().open);
        assert!(app.world().resource::<Driving>().paused);
        // Continue with the steering held as before; the race goes on as if
        // it had never been replayed.
        keys(&mut app).press(KeyCode::KeyD);
        tap(&mut app, KeyCode::Escape);
        assert!(!app.world().resource::<Driving>().paused);
        drive_to(&mut app, END);
        assert_eq!(digest(&app), expected);

        // The Instant Replay key in the race: back to the race on exit.
        keys(&mut app).release(KeyCode::KeyW);
        keys(&mut app).release(KeyCode::KeyD);
        tap(&mut app, KeyCode::Enter);
        let s = replay(&app).session.as_ref().unwrap();
        assert_eq!(s.origin, Origin::Race);
        assert!(app.world().resource::<Driving>().paused);
        assert!(!app.world().resource::<PauseMenu>().open);
        tap(&mut app, KeyCode::Escape);
        assert!(replay(&app).session.is_none());
        assert!(!app.world().resource::<Driving>().paused);
        assert!(!app.world().resource::<PauseMenu>().open, "Replay Exit is not Pause");
    });
}

#[test]
#[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS / GRID_BODY_EVENT_PARAMETERS / GRID_DEFORMATION_PARAMETERS"]
fn view_replay_plays_the_whole_race_from_the_results() {
    use crate::post_race::Stage;
    big_stack(|| {
        let mut app = with_replay(race_app_with(NURBURGRING, &["bmw", "lac"], 2, false), NURBURGRING);
        keys(&mut app).press(KeyCode::KeyW);
        while ticks(&app) < 700 {
            frame(&mut app);
        }
        keys(&mut app).release(KeyCode::KeyW);
        // Retire from Race, then the results' View Replay pill.
        tap(&mut app, KeyCode::Escape);
        tap(&mut app, KeyCode::ArrowUp);
        tap(&mut app, KeyCode::Enter);
        tap(&mut app, KeyCode::Enter);
        assert_eq!(app.world().resource::<PostRace>().stage, Stage::Results);
        let ticks_before = ticks(&app);
        let before = digest(&app);
        tap(&mut app, KeyCode::ArrowRight);
        tap(&mut app, KeyCode::Enter);
        {
            let s = replay(&app).session.as_ref().expect("full replay");
            assert_eq!((s.kind, s.origin), (Kind::Full, Origin::Results));
            assert_eq!(s.player.start, seconds(1));
            assert_eq!(s.player.time, s.player.start, "from the race start");
            assert!(s.player.end >= seconds(690));
        }
        // The shown state is the recorded start: every car on its slot.
        frame(&mut app);
        let first = app
            .world_mut()
            .resource_mut::<Replay>()
            .snapshot(0)
            .unwrap();
        let shown = replay(&app).session.as_ref().unwrap().shown.clone().unwrap();
        assert_eq!(shown.cars.len(), first.cars.len());
        assert!(shown.cars[0].pose.translation().distance(first.cars[0].pose.translation()) < 0.5);
        // Next Lap / Previous Lap and the shuttle stay inside the window.
        tap(&mut app, KeyCode::F5);
        tap(&mut app, KeyCode::F4);
        let s = replay(&app).session.as_ref().unwrap();
        assert!(s.player.time >= s.player.start && s.player.time <= s.player.end);
        tap(&mut app, KeyCode::Escape);
        assert!(replay(&app).session.is_none());
        assert_eq!(app.world().resource::<PostRace>().stage, Stage::Results);
        assert_eq!(ticks(&app), ticks_before);
        assert_eq!(digest(&app), before);
    });
}

/// The automatic crash replay with a real wreck: the player loses a wheel
/// through the field's shared loose-wheel path (`MultiRuntime::
/// force_wheel_loss`, as `grid-opponent-race`'s scripted crash does), and
/// the car's own handling update then finds the damage terminal (status 4,
/// 9E65B0). The wreck screen then plays the terminal-damage replay by itself
/// (flow `terminal_damage_delay` → `instant_race_replay_silent`), from 10 s
/// before the wreck, and its exit shows the wreck menu with the race still
/// frozen.
#[test]
#[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS / GRID_BODY_EVENT_PARAMETERS / GRID_DEFORMATION_PARAMETERS"]
fn a_real_wreck_plays_the_crash_replay_by_itself() {
    use crate::post_race::Stage;
    big_stack(|| {
        let mut app = with_replay(race_app_with(DONINGTON, &["lac", "bmw"], 2, true), DONINGTON);
        drive_to(&mut app, 900);
        {
            // The player is car 0 of the field only inside the tick (main.rs
            // swaps it in around `MultiRuntime::tick`).
            let mut d = app.world_mut().resource_mut::<Driving>();
            let d = &mut *d;
            let o = d.opponents.as_mut().expect("field race");
            std::mem::swap(&mut d.runtime, &mut o.multi.cars[0]);
            let lost = o.multi.force_wheel_loss(0, 2);
            std::mem::swap(&mut d.runtime, &mut o.multi.cars[0]);
            assert!(lost.unwrap(), "the rear-left wheel comes off");
        }
        let mut hits = 0;
        while app.world().resource::<Driving>().runtime.terminal_wheel().is_none() {
            assert!(hits < 120, "no terminal damage {hits} frames after the wheel loss");
            hits += 1;
            frame(&mut app);
        }
        let wreck = ticks(&app);
        // The wreck transitions, then the crash replay starts by itself.
        let mut frames = 0;
        while replay(&app).session.is_none() {
            assert!(frames < 1200, "no crash replay {frames} frames after the wreck");
            frame(&mut app);
            frames += 1;
        }
        assert_eq!(ticks(&app), wreck, "the wrecked car is frozen");
        let before = digest(&app);
        {
            let r = replay(&app);
            let s = r.session.as_ref().unwrap();
            assert_eq!((s.kind, s.origin), (Kind::Instant, Origin::Results));
            assert!(s.terminal, "the terminal-damage replay");
            assert!(s.player.end >= seconds(wreck) - seconds(3));
            let oldest = seconds(r.stream.tick(r.stream.frames().start));
            assert!(
                (s.player.start - (seconds(wreck) - INSTANT_REWIND).max(oldest)).abs() <= seconds(3),
                "starts 10 s before the wreck: {} {}",
                s.player.start,
                seconds(wreck)
            );
        }
        // The replay shows the recorded crash: the replayed player reaches
        // the wrecked pose.
        for _ in 0..900 {
            frame(&mut app);
            let s = replay(&app).session.as_ref().expect("the crash replay plays on");
            if s.player.time >= s.player.end - seconds(2) {
                break;
            }
        }
        let shown = replay(&app).session.as_ref().unwrap().shown.clone().unwrap();
        let live = Mat4::from_cols_array(&app.world().resource::<Driving>().runtime.pose());
        assert!(
            shown.cars[0].pose.translation().distance(live.w_axis.truncate()) < 5.,
            "the replay ends at the wreck"
        );
        // Replay Exit: the wreck menu, nothing moved.
        tap(&mut app, KeyCode::Escape);
        assert!(replay(&app).session.is_none());
        assert_eq!(app.world().resource::<PostRace>().stage, Stage::Wreck);
        assert_eq!(digest(&app), before);
        eprintln!("crash replay: hits={hits} wreck_tick={wreck} frames_to_replay={frames}");
    });
}
