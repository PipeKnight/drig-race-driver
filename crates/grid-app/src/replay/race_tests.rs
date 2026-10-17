//! Whole-race replay checks (docs/REPLAY.md) with the shared world, the AI
//! field and the race manager, on the pause menu's race harness.
use super::*;
use crate::pause_menu::race_tests::{
    big_stack, digest, drive_to, keys, race_app_with, tap, ticks,
};
use crate::{Driving, pause_menu::PauseMenu, post_race::PostRace};
use grid_tools::route::{DONINGTON, ISTANBUL_PARK, LONG_BEACH, NURBURGRING, Route};
use std::time::Duration;

/// The race harness plus the recorder after each tick and the replay
/// controls before the menus; real time advances 1/60 s per frame.
fn with_replay(mut app: App, route: &str) -> App {
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

fn frame(app: &mut App) {
    app.world_mut()
        .resource_mut::<Time<Real>>()
        .update_with_duration(Duration::from_secs_f64(1. / 60.));
    app.update();
}

fn replay(app: &App) -> &Replay {
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
    }
}

/// The recorded frames decode to the live race bit for bit, and the
/// replayed presentation equals the live one at every sample.
fn check(app: &mut App, lives: &[Live]) {
    let mut replay = app.world_mut().resource_mut::<Replay>();
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

#[test]
#[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS / GRID_BODY_EVENT_PARAMETERS / GRID_DEFORMATION_PARAMETERS"]
fn recorded_frames_equal_the_live_race_on_every_circuit() {
    big_stack(|| {
        for route in [DONINGTON, NURBURGRING, ISTANBUL_PARK, LONG_BEACH] {
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
        }
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
