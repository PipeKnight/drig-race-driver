//! Whole-race checks with the shared world, the AI field and the race
//! manager: the menu's pause is bit-identical and Restart Race resets all of it.
use super::*;
use crate::{Driving, Opponents, input, time_trial};
use std::path::Path;

pub(crate) const OPPONENTS: usize = 3;

/// A 2-lap race at Donington like `settings/race.ini`, player at the back.
fn race_app() -> App {
    race_app_with(grid_tools::route::DONINGTON, &["lac"], 2, false)
}

/// A race on `route` with `OPPONENTS` AI cars; car models cycle through
/// `models` (car 0, the player, takes the first), player at the back.
/// `autopilot`: the ported AI drives the player's car too.
pub(crate) fn race_app_with(route: &str, models: &[&str], laps: u32, autopilot: bool) -> App {
    use grid_tools::ai_driver::{AiData, AiDriver, Line, Parameters};
    use grid_tools::{car::Car, route::Route};
    let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
    let root = Path::new(&root);
    let read = |name| std::fs::read(std::env::var_os(name).unwrap()).unwrap();
    let table = read("GRID_CONTACT_DIRECTIONS");
    let events = Some(
        grid_tools::body_impact_parameters::read(&read("GRID_BODY_EVENT_PARAMETERS")).unwrap(),
    );
    let deformation = Some(
        grid_tools::deformation_parameters::read(&read("GRID_DEFORMATION_PARAMETERS")).unwrap(),
    );
    let route = Route::new(route).unwrap();
    let field: Vec<Car> = (0..=OPPONENTS)
        .map(|i| Car::new(models[i % models.len()]).unwrap())
        .collect();
    let car = field[0].clone();
    let player = OPPONENTS;
    let slots: Vec<String> = std::iter::once(player)
        .chain((0..=OPPONENTS).filter(|&i| i != player))
        .map(|i| format!("slot_{i:02}"))
        .collect();
    let entries: Vec<(&str, &Car)> = slots.iter().map(String::as_str).zip(&field).collect();
    let mut runtime =
        grid_tools::source_vehicle::Runtime::load(root, &route, &car, &slots[0], 200679).unwrap();
    runtime.body_events = events;
    runtime.enable_lap_timing(root).unwrap();
    // As the app (main.rs): the player's own track gets the boundary sheet,
    // deformation and dynamic props. The shared world ticks this runtime as
    // car 0, so without them the player alone could pass the boundary walls.
    let mut geometry = runtime.prepare_body_geometry(root, &table).unwrap();
    if let Some((limit, bulk)) = &deformation {
        runtime.enable_deformation(root, &mut geometry).unwrap();
        runtime
            .register_deformation_callbacks(*limit, bulk.clone(), 1)
            .unwrap();
    }
    runtime.enable_dynamic_props(root).unwrap();
    let mut multi = grid_tools::source_vehicle::multi::MultiRuntime::load_deforming_on(
        root,
        &route,
        &entries,
        &table,
        events,
        deformation,
    )
    .unwrap();
    multi.enable_dynamic_props(root).unwrap();
    let ai = AiData::load(root, route.path(), "touring", car.stem()).unwrap();
    let car_ai: Vec<AiData> = field[1..]
        .iter()
        .map(|m| AiData::load(root, route.path(), "touring", m.stem()).unwrap())
        .collect();
    let settings = grid_tools::ai_pacing::Settings::default();
    let drivers = (0..OPPONENTS)
        .map(|i| {
            AiDriver::new(
                Parameters::new(&car_ai[i], settings.difficulty, |_| 1.).unwrap(),
                0x5eed + i as u32 + 1,
            )
        })
        .collect();
    let pilot = autopilot.then(|| {
        AiDriver::new(
            Parameters::new(&ai, settings.difficulty, |_| 1.).unwrap(),
            0x5eed,
        )
    });
    let start = {
        let gate = &grid_tools::lap_progress::load_route(root, &route)
            .unwrap()
            .progress
            .gates[0];
        std::array::from_fn(|k| (gate.left[k] + gate.right[k]) * 0.5)
    };
    let positions: Vec<[f32; 3]> = std::iter::once(&runtime)
        .chain(multi.cars.iter().skip(1))
        .map(|c| {
            let p = c.pose();
            [p[12], p[13], p[14]]
        })
        .collect();
    let standings = grid_tools::race::Standings::new(&ai.line, laps, start, &positions);
    let pacing = grid_tools::ai_pacing::Field::new(
        grid_tools::ai_pacing::Data::load(root, &settings).unwrap(),
        settings.clone(),
        &ai.line,
        laps,
        start,
        &positions,
        0,
        vec![1.; OPPONENTS + 1],
        0x5eed,
    );
    let opponents = Opponents {
        multi,
        drivers,
        car_ai,
        ai,
        standings,
        names: (0..=OPPONENTS).map(|i| format!("car {i}")).collect(),
        autopilot: pilot,
        after_finish: None,
        throttles: vec![0.; OPPONENTS],
        pacing,
        ai_messages: Vec::new(),
    };
    let mut trial =
        time_trial::Trial::new(laps, grid_tools::time_trial::Mode::Events, false).unwrap();
    trial.race = true;
    trial.state = grid_tools::time_trial::State::new(
        laps,
        f64::from(grid_tools::source_vehicle::DELTA),
        crate::START_SEQUENCE_SECONDS,
    )
    .unwrap();
    let mut app = App::new();
    app.add_message::<AppExit>()
        .insert_resource(ButtonInput::<KeyCode>::default())
        .insert_resource(trial)
        .insert_resource(Driving {
            runtime,
            geometry: Some(geometry),
            paused: false,
            failure: None,
            notices: Vec::new(),
            scripted_ticks: None,
            scripted: Default::default(),
            handbrake_from: None,
            last_controls: None,
            segment: 0,
            started: std::time::Instant::now(),
            opponents: Some(opponents),
            auto_reset: Default::default(),
            reset_line: Some(Line::load(root, route.path()).unwrap()),
        })
        .insert_resource(input::Actions::new(
            input::ActionMap::keyboard(),
            input::Pads::Off,
        ))
        .init_resource::<ShiftQueue>()
        .insert_resource(PauseMenu::new(
            Variant::FreeRace,
            false,
            true,
            Options::default(),
        ))
        .insert_resource(crate::post_race::PostRace::new(Variant::FreeRace))
        .add_systems(
            Update,
            (
                input::sample,
                crate::post_race::update,
                update,
                crate::drive_actions,
                crate::drive_tick,
            )
                .chain(),
        );
    app
}

/// FNV-1a over every car's vehicle and physical words (as grid-opponent-race
/// prints), plus the lap, event clock, race, props and race manager records.
pub(crate) fn digest(app: &App) -> (Vec<u64>, String) {
    let d = app.world().resource::<Driving>();
    let o = d.opponents.as_ref().unwrap();
    let cars = std::iter::once(&d.runtime)
        .chain(o.multi.cars.iter().skip(1))
        .map(|c| {
            let owner = &c.state.car.owner;
            let mut hash = 0xcbf2_9ce4_8422_2325u64;
            for &word in owner.vehicle.iter().chain(&owner.physical) {
                hash = (hash ^ u64::from(word)).wrapping_mul(0x100_0000_01b3);
            }
            hash
        })
        .collect();
    let props = o.multi.dynamic_props().unwrap();
    let rest = format!(
        "ticks={} {} lap={:?} trial={:?} standings={:?} props={} contacts={} retired={:?} pacing={:?} throttles={:?}",
        d.runtime.ticks,
        o.multi.ticks,
        d.runtime.lap,
        app.world().resource::<time_trial::Trial>().state,
        o.standings.classification(&o.names),
        props.events.len(),
        o.multi.last_car_contacts,
        (0..=OPPONENTS)
            .map(|i| o.multi.retirement(i).is_some())
            .collect::<Vec<_>>(),
        o.pacing.log,
        o.throttles,
    );
    (cars, rest)
}

pub(crate) fn ticks(app: &App) -> u64 {
    app.world().resource::<Driving>().runtime.ticks
}

pub(crate) fn keys(app: &mut App) -> Mut<'_, ButtonInput<KeyCode>> {
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>()
}

/// One menu key press and release (two frames).
pub(crate) fn tap(app: &mut App, key: KeyCode) {
    keys(app).press(key);
    app.update();
    keys(app).release(key);
    app.update();
}

/// Hold W (throttle) and D (right) until the player's tick count.
pub(crate) fn drive_to(app: &mut App, target: u64) {
    keys(app).press(KeyCode::KeyW);
    keys(app).press(KeyCode::KeyD);
    while ticks(app) < target {
        assert!(advance(app), "wrecked at tick {}", ticks(app));
    }
    assert_eq!(ticks(app), target);
}

/// Update until the player's tick count moves. False when it stops for good:
/// a wrecked player (terminal damage) is no longer ticked, which used to hang
/// the loops above. The player has the boundary walls since F403.
pub(crate) fn advance(app: &mut App) -> bool {
    for _ in 0..1000 {
        let before = ticks(app);
        app.update();
        if ticks(app) != before {
            return true;
        }
    }
    let d = app.world().resource::<Driving>();
    assert!(d.failure.is_none(), "{:?}", d.failure);
    assert!(
        d.runtime.terminal_wheel().is_some(),
        "the player stopped ticking without terminal damage"
    );
    false
}

pub(crate) fn big_stack(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(256 * 1024 * 1024)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap();
}

#[test]
#[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS / GRID_BODY_EVENT_PARAMETERS / GRID_DEFORMATION_PARAMETERS"]
fn pause_menu_freezes_the_race_and_resumes_bit_identically() {
    big_stack(|| {
        const PAUSE_AT: u64 = 300;
        const END: u64 = 600;
        let mut straight = race_app();
        drive_to(&mut straight, END);
        let expected = digest(&straight);

        let mut paused = race_app();
        drive_to(&mut paused, PAUSE_AT);
        let before = digest(&paused);
        keys(&mut paused).release(KeyCode::KeyD);
        tap(&mut paused, KeyCode::Escape);
        assert!(paused.world().resource::<PauseMenu>().open);
        assert!(paused.world().resource::<Driving>().paused);
        // Browse the menu: Options > Sound Setup, change and undo the effects
        // volume, back out; throttle held, many frames.
        for key in [KeyCode::ArrowDown; 3] {
            tap(&mut paused, key);
        }
        tap(&mut paused, KeyCode::Enter);
        tap(&mut paused, KeyCode::Enter);
        tap(&mut paused, KeyCode::ArrowLeft);
        assert_eq!(paused.world().resource::<PauseMenu>().values.effects, 90);
        tap(&mut paused, KeyCode::ControlLeft);
        assert_eq!(paused.world().resource::<PauseMenu>().values.effects, 100);
        for _ in 0..50 {
            paused.update();
        }
        tap(&mut paused, KeyCode::Escape);
        tap(&mut paused, KeyCode::Escape);
        for key in [KeyCode::ArrowUp; 3] {
            tap(&mut paused, key);
        }
        assert_eq!(paused.world().resource::<PauseMenu>().screen, Screen::Main);
        assert!(paused.world().resource::<PauseMenu>().open);
        // Nothing moved while the menu was open.
        assert_eq!(digest(&paused), before);
        assert_eq!(ticks(&paused), PAUSE_AT);
        // Continue, with the steering held as before.
        keys(&mut paused).press(KeyCode::KeyD);
        keys(&mut paused).press(KeyCode::Enter);
        paused.update();
        keys(&mut paused).release(KeyCode::Enter);
        assert!(!paused.world().resource::<PauseMenu>().open);
        assert_eq!(ticks(&paused), PAUSE_AT + 1);
        drive_to(&mut paused, END);
        assert_eq!(digest(&paused), expected);
    });
}

#[test]
#[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS / GRID_BODY_EVENT_PARAMETERS / GRID_DEFORMATION_PARAMETERS"]
fn restart_race_resets_cars_damage_ai_timing_and_props() {
    big_stack(|| {
        const RUN: u64 = 900;
        let start = digest(&race_app());

        let mut app = race_app();
        assert_eq!(digest(&app), start);
        // A first attempt flat out and straight on from the back of the grid:
        // into the field and off at the first corner.
        keys(&mut app).press(KeyCode::KeyW);
        let mut car_contacts = 0;
        // Wrecked at the first corner is the expected end of this attempt.
        while ticks(&app) < 2 * RUN && advance(&mut app) {
            car_contacts += app
                .world()
                .resource::<Driving>()
                .opponents
                .as_ref()
                .unwrap()
                .multi
                .last_car_contacts;
        }
        {
            let d = app.world().resource::<Driving>();
            let o = d.opponents.as_ref().unwrap();
            let damage = |v: &[u32]| {
                [0, 4, 8, 0x10, 0x14, 0x28, 0x2c, 0x30, 0x34].map(|k| v[(0xcdf0 + k) / 4])
            };
            let pristine = damage(&start_vehicle());
            let damaged = damage(&d.runtime.state.car.owner.vehicle) != pristine;
            let props = o.multi.dynamic_props().unwrap().events.len();
            eprintln!(
                "restart_before damage_changed={damaged} props={props} car_contacts={car_contacts} lap={:?}",
                d.runtime.lap.as_ref().map(|l| l.lap_number())
            );
            // The attempt really left damage, contacts or broken props behind.
            assert!(damaged || props > 0 || car_contacts > 0);
            assert_ne!(digest(&app), start);
        }
        keys(&mut app).release(KeyCode::KeyW);
        keys(&mut app).release(KeyCode::KeyD);
        tap(&mut app, KeyCode::Escape);
        tap(&mut app, KeyCode::ArrowDown);
        tap(&mut app, KeyCode::ArrowDown);
        tap(&mut app, KeyCode::Enter);
        assert_eq!(
            app.world().resource::<PauseMenu>().screen,
            Screen::Confirm(Confirm::Restart)
        );
        // Yes is the default answer.
        keys(&mut app).press(KeyCode::Enter);
        app.update();
        keys(&mut app).release(KeyCode::Enter);
        {
            let d = app.world().resource::<Driving>();
            assert!(!app.world().resource::<PauseMenu>().open);
            assert!(!d.paused);
            assert_eq!(d.segment, 1);
            assert!(d.notices.is_empty() && d.failure.is_none());
            // The restart frame's own tick already ran (one countdown tick).
            assert_eq!(d.runtime.ticks, 1);
        }
        // A fresh race given the same input: one frame without keys, then
        // throttle and steering held.
        let mut fresh = race_app();
        fresh.update();
        assert_eq!(digest(&app), digest(&fresh));
        // And the restarted race replays the fresh one exactly.
        drive_to(&mut app, RUN);
        drive_to(&mut fresh, RUN);
        assert_eq!(digest(&app), digest(&fresh));
    });
}

/// The player's vehicle words at the grid, for the damage report.
fn start_vehicle() -> Vec<u32> {
    let app = race_app();
    app.world()
        .resource::<Driving>()
        .runtime
        .state
        .car
        .owner
        .vehicle
        .clone()
}

fn post(app: &App) -> &crate::post_race::PostRace {
    app.world().resource::<crate::post_race::PostRace>()
}

fn results(app: &App) -> Vec<crate::post_race::Row> {
    crate::post_race::table(
        app.world().resource::<Driving>(),
        Some(app.world().resource::<time_trial::Trial>()),
    )
}

// One test per circuit, so the circuits run in parallel (as one loop: 43 s).
macro_rules! retire_on {
    ($($name:ident: $route:ident, $models:expr);* $(;)?) => {$(
        #[test]
        #[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS / GRID_BODY_EVENT_PARAMETERS / GRID_DEFORMATION_PARAMETERS"]
        fn $name() {
            retire_from_race_shows_the_results(grid_tools::route::$route, $models);
        }
    )*};
}
retire_on!(
    retire_from_race_shows_the_results_on_donington: DONINGTON, &["lac"];
    retire_from_race_shows_the_results_on_nurburgring: NURBURGRING, &["bmw"];
    retire_from_race_shows_the_results_on_istanbul_park: ISTANBUL_PARK, &["lac", "bmw"];
    // Jarama route 1 finishes at a later gate (docs/JARAMA.md).
    retire_from_race_shows_the_results_on_jarama: JARAMA, &["lac"];
    // Yokohama: the first Japanese track, a 2.0 s start (docs/YOKOHAMA.md).
    retire_from_race_shows_the_results_on_yokohama: YOKOHAMA, &["bmw", "lac"];
    // Spa: the 7.0 km Grand Prix circuit (docs/SPA.md).
    retire_from_race_shows_the_results_on_spa: SPA, &["lac"];
    // The street circuits (docs/STREET_CIRCUITS.md).
    retire_from_race_shows_the_results_on_milan: MILAN, &["lac"];
    retire_from_race_shows_the_results_on_shibuya: SHIBUYA, &["bmw"];
    retire_from_race_shows_the_results_on_san_francisco: SAN_FRANCISCO, &["lac"];
    // Circuit de la Sarthe: 13.6 km, 20 grid slots (docs/LE_MANS.md).
    retire_from_race_shows_the_results_on_le_mans: LE_MANS, &["lac"];
);

fn retire_from_race_shows_the_results(route: &'static str, models: &'static [&'static str]) {
    use crate::post_race::{Cell, Stage};
    use grid_tools::race::{Out, Status};
    big_stack(move || {
        let mut app = race_app_with(route, models, 2, false);
        keys(&mut app).press(KeyCode::KeyW);
        while ticks(&app) < 500 {
            app.update();
        }
        keys(&mut app).release(KeyCode::KeyW);
        tap(&mut app, KeyCode::Escape);
        // Up wraps to the last button, Retire from Race; Yes by default.
        tap(&mut app, KeyCode::ArrowUp);
        tap(&mut app, KeyCode::Enter);
        tap(&mut app, KeyCode::Enter);
        assert_eq!(post(&app).stage, Stage::Results, "{route}");
        assert!(!app.world().resource::<PauseMenu>().open);
        {
            let d = app.world().resource::<Driving>();
            let o = d.opponents.as_ref().unwrap();
            assert!(o.standings.over(), "{route}");
            assert_eq!(o.standings.status(0), Status::Out(Out::Retired));
        }
        let rows = results(&app);
        eprintln!("retire_results route={route} rows={rows:?}");
        assert_eq!(rows.len(), OPPONENTS + 1);
        assert_eq!(
            rows.iter().map(|r| r.position).collect::<Vec<_>>(),
            (1..=OPPONENTS + 1).collect::<Vec<_>>()
        );
        let player = rows.iter().find(|r| r.car == 0).unwrap();
        assert!(matches!(player.time, Cell::Out(Out::Retired, _)));
        // Every opponent racing on was estimated (the human did not finish).
        for r in rows.iter().filter(|r| r.car != 0) {
            assert!(r.estimated || matches!(r.time, Cell::Out(..)), "{route} {r:?}");
        }
        let vehicles: Vec<&str> = rows.iter().map(|r| r.vehicle.as_str()).collect();
        if models.contains(&"bmw") {
            assert!(vehicles.iter().any(|v| v.contains("BMW")), "{vehicles:?}");
        }
        // The race is frozen behind the results.
        let frozen = digest(&app);
        for _ in 0..20 {
            app.update();
        }
        assert_eq!(digest(&app), frozen);
        // Restart Race (the fourth pill) after its confirmation.
        for _ in 0..3 {
            tap(&mut app, KeyCode::ArrowRight);
        }
        tap(&mut app, KeyCode::Enter);
        keys(&mut app).press(KeyCode::Enter);
        app.update();
        keys(&mut app).release(KeyCode::Enter);
        assert_eq!(post(&app).stage, Stage::Hidden);
        let d = app.world().resource::<Driving>();
        assert_eq!((d.segment, d.runtime.ticks), (1, 1), "{route}");
        assert!(!d.opponents.as_ref().unwrap().standings.over());
    });
}

#[test]
#[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS / GRID_BODY_EVENT_PARAMETERS / GRID_DEFORMATION_PARAMETERS"]
fn a_finished_race_goes_through_stats_to_classified_results() {
    big_stack(|| finish_one_lap(grid_tools::route::DONINGTON));
}

/// Jarama route 1 finishes at gate 67 of its progress file; lap timing re-indexes
/// it so the finish is gate 0 (docs/JARAMA.md).
#[test]
#[ignore = "row 89: autopilot wrecked at Jarama turn 1, see F497; GRID_RUN_KNOWN_FAILING=1 runs it"]
fn a_race_finishing_at_a_later_gate_is_classified() {
    // FAILS (F497): on the track view's Catmull-Rom line the autopilot takes
    // turn 1 at brake line 0's 55.4 m/s, runs 18 m wide at full lock and is
    // wrecked at tick 1013 (FIDELITY_BACKLOG row 89).
    if std::env::var_os("GRID_RUN_KNOWN_FAILING").is_none() {
        eprintln!("skipped: known failing, FIDELITY_BACKLOG row 89");
        return;
    }
    big_stack(|| finish_one_lap(grid_tools::route::JARAMA));
}

/// One lap with the ported AI driving the player's car, then the stats and the
/// classified results.
fn finish_one_lap(route: &str) {
    use crate::post_race::{Cell, Stage};
    let mut app = race_app_with(route, &["lac"], 1, true);
    while post(&app).stage == Stage::Hidden {
        app.update();
        assert!(ticks(&app) < 12_000, "no finish");
        // A wrecked player ends the race; the wreck screen waits for real
        // time, so fail here instead of updating forever.
        let d = app.world().resource::<Driving>();
        assert!(
            d.runtime.terminal_wheel().is_none(),
            "player wrecked at tick {}",
            d.runtime.ticks
        );
    }
    assert_eq!(post(&app).stage, Stage::Stats);
    assert!(app.world().resource::<time_trial::Trial>().finished());
    let rows = results(&app);
    let player = rows.iter().find(|r| r.car == 0).unwrap().clone();
    let view = post(&app).view(&rows, &Labels::default());
    eprintln!(
        "finish_stats tick={} position={} best={:?} title={}",
        ticks(&app),
        player.position,
        player.best,
        view.title
    );
    assert!(player.best.is_some());
    assert!(view.title.starts_with("post_race_stats_position"));
    // Continue: the race is ended and classified.
    tap(&mut app, KeyCode::Enter);
    assert_eq!(post(&app).stage, Stage::Results);
    let d = app.world().resource::<Driving>();
    assert!(d.opponents.as_ref().unwrap().standings.over());
    let rows = results(&app);
    eprintln!("finish_results rows={rows:?}");
    assert_eq!(rows.len(), OPPONENTS + 1);
    // Times rise down the table: the winner's total, then growing gaps.
    let mut last = 0.;
    for (i, r) in rows.iter().enumerate() {
        match r.time {
            Cell::Total(t) => assert!(i == 0 && t > 60.),
            Cell::Gap(g) => {
                assert!(i > 0 && g >= last, "{r:?}");
                last = g;
            }
            Cell::Out(..) | Cell::Running => {}
        }
    }
    assert!(matches!(rows[0].time, Cell::Total(_)));
}

#[test]
#[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS / GRID_BODY_EVENT_PARAMETERS / GRID_DEFORMATION_PARAMETERS"]
fn a_drift_battle_is_scored_banked_and_classified_by_drift_points() {
    use crate::drift_event::DriftEvent;
    use crate::post_race::{Cell, Stage};
    big_stack(|| {
        // One lap of the Long Beach drift circuit (route_1), BMW field, the
        // ported AI driving the player's car, as settings/drift-battle.ini.
        let route = "tracks/usa/long_beach/route_1";
        let mut app = race_app_with(route, &["bmw"], 1, true);
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let root = Path::new(&root);
        let settings = grid_tools::drift::Settings::load(root).unwrap();
        let flags = grid_tools::drift::close_drift_flags(
            root,
            &grid_tools::route::Route::new(route).unwrap(),
        )
        .unwrap();
        assert!(!flags.is_empty());
        app.insert_resource(DriftEvent::battle(settings, flags, OPPONENTS + 1, 1, 1))
            .add_systems(Update, crate::drift_event::tick.after(crate::drive_tick));
        app.world_mut()
            .resource_mut::<crate::post_race::PostRace>()
            .drift = true;
        let mut fastest = 0f32;
        let mut largest_angle = 0f32;
        while post(&app).stage == Stage::Hidden {
            app.update();
            let e = &app.world().resource::<DriftEvent>().entries[0];
            fastest = fastest.max(e.detector.speed_mph);
            largest_angle = largest_angle.max(e.detector.angle);
            assert!(ticks(&app) < 12_000, "no finish");
        }
        assert_eq!(post(&app).stage, Stage::Stats);
        // The player's entry was banked at its finish (0x65F250).
        let event = app.world().resource::<DriftEvent>();
        let player = &event.entries[0];
        let board = event.board().unwrap();
        eprintln!(
            "drift_finish tick={} fastest_mph={fastest} largest_angle={largest_angle} score={} drifts={} board={board:?}",
            ticks(&app),
            player.score(),
            player.scorer.drifts
        );
        assert!(player.done && !player.estimated);
        assert!(fastest > 50., "the detector sees the car's speed");
        assert_eq!(board.total, player.score());
        let view = post(&app).view_with(&[], &Labels::default(), Some(&board));
        assert_eq!(view.title, "post_race_stats_drift_total_title");
        // Continue: the race is ended; every entry is banked.
        tap(&mut app, KeyCode::Enter);
        assert_eq!(post(&app).stage, Stage::Scoreboard);
        tap(&mut app, KeyCode::Enter);
        assert_eq!(post(&app).stage, Stage::Results);
        let d = app.world().resource::<Driving>();
        let o = d.opponents.as_ref().unwrap();
        assert!(o.standings.over());
        let event = app.world().resource::<DriftEvent>();
        assert!(event.entries.iter().all(|e| e.done));
        let rows = event.results(Some(&o.standings), results(&app));
        eprintln!("drift_results rows={rows:?}");
        assert_eq!(rows.len(), OPPONENTS + 1);
        assert_eq!(
            rows.iter().map(|r| r.position).collect::<Vec<_>>(),
            (1..=OPPONENTS + 1).collect::<Vec<_>>()
        );
        let scores: Vec<i64> = rows.iter().map(|r| r.score.unwrap()).collect();
        assert!(scores.windows(2).all(|w| w[0] >= w[1]), "{scores:?}");
        for r in &rows {
            assert_eq!(r.score.unwrap(), event.entries[r.car].score());
            assert!(!matches!(r.time, Cell::Running));
        }
        // Equal scores keep the race order.
        let order = o.standings.order();
        for w in rows.windows(2) {
            if w[0].score.unwrap() == w[1].score.unwrap() {
                let at = |car| order.iter().position(|&c| c == car).unwrap();
                assert!(at(w[0].car) < at(w[1].car));
            }
        }
    });
}

#[test]
#[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS / GRID_BODY_EVENT_PARAMETERS / GRID_DEFORMATION_PARAMETERS"]
fn a_handbrake_turn_in_the_bmw_scores_drift_points() {
    use crate::drift_event::DriftEvent;
    big_stack(|| {
        let route = "tracks/usa/long_beach/route_1";
        let mut app = race_app_with(route, &["bmw"], 2, false);
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let root = Path::new(&root);
        let settings = grid_tools::drift::Settings::load(root).unwrap();
        app.insert_resource(DriftEvent::battle(settings, Vec::new(), OPPONENTS + 1, 2, 1))
            .add_systems(Update, crate::drift_event::tick.after(crate::drive_tick));
        let entry = |app: &App| app.world().resource::<DriftEvent>().entries[0].clone();
        // A wrecked car stops ticking: fail instead of waiting forever.
        let step = |app: &mut App| assert!(advance(app), "wrecked at tick {}", ticks(app));
        // Flat out after the countdown, then a handbrake turn to the right
        // held on the throttle, before the first wall (about tick 800 flat
        // out, now that the player has the boundary sheet as in the app).
        keys(&mut app).press(KeyCode::KeyW);
        while ticks(&app) < 600 {
            step(&mut app);
        }
        let mut most = (0f32, 0f32, false);
        for (hold, keys_down) in [
            (40, &[KeyCode::KeyW, KeyCode::KeyD, KeyCode::Space][..]),
            (150, &[KeyCode::KeyW, KeyCode::KeyD][..]),
        ] {
            for k in [KeyCode::KeyD, KeyCode::Space] {
                keys(&mut app).release(k);
            }
            for &k in keys_down {
                keys(&mut app).press(k);
            }
            let end = ticks(&app) + hold;
            while ticks(&app) < end {
                step(&mut app);
                let e = entry(&app);
                most = (
                    most.0.max(e.detector.angle),
                    most.1.max(e.detector.speed_mph),
                    most.2 || e.detector.drifting,
                );
            }
        }
        let e = entry(&app);
        eprintln!(
            "handbrake_turn largest_angle={} fastest_mph={} drifted={} display={} total={} combo={} rating={} drifts={}",
            most.0, most.1, most.2, e.scorer.display, e.scorer.total, e.scorer.combo, e.scorer.rating, e.scorer.drifts
        );
        assert!(most.2, "the BMW slides past 15 degrees above 50 mph");
        // The scorer counts the slide. From 108 mph this open-loop turn ends
        // in a spin, which banks nothing; banked drift scores are covered by
        // the autopiloted Drift Battle tests. (Before F403 the player passed
        // through the boundary wall here and slid out at 27 degrees.)
        assert_eq!(e.scorer.drifts, 1);
    });
}

/// A time-limited Le Mans race (race type 3, docs/LE_MANS.md): nobody finishes
/// at the line, the race ends for every car at the limit, the stats follow and
/// the results rank by progress with the race clock (24 h of day in the limit). The limit is 40 s here, the career's 720 s.
#[test]
#[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS / GRID_BODY_EVENT_PARAMETERS / GRID_DEFORMATION_PARAMETERS"]
fn a_time_limited_le_mans_race_ends_at_the_limit_and_ranks_by_progress() {
    use crate::le_mans_event::LeMansEvent;
    use crate::post_race::{Cell, Stage};
    use grid_tools::time_of_day::{Mode, Timeline};
    const LIMIT: f64 = 40.;
    big_stack(|| {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let root = Path::new(&root);
        let route = grid_tools::route::Route::new(grid_tools::route::LE_MANS).unwrap();
        let mut app = race_app_with(route.path(), &["lac"], 8, true);
        {
            let mut d = app.world_mut().resource_mut::<Driving>();
            let o = d.opponents.as_mut().unwrap();
            o.standings = o.standings.clone().with_time_limit(LIMIT);
        }
        app.world_mut().resource_mut::<time_trial::Trial>().state =
            grid_tools::time_trial::State::new(
                99,
                f64::from(grid_tools::source_vehicle::DELTA),
                crate::START_SEQUENCE_SECONDS,
            )
            .unwrap()
            .with_time_limit(LIMIT)
            .unwrap();
        let schema = std::fs::read(root.join("database/schema.bin")).unwrap();
        let data = std::fs::read(root.join("database/database.bin")).unwrap();
        let db = grid_formats::database::read(&schema, &data).unwrap();
        let lac = grid_tools::series::lemans_class(&db, &grid_tools::car::Car::new("lac").unwrap())
            .unwrap();
        app.insert_resource(LeMansEvent::new(
            Some(LIMIT),
            Mode::Timeline,
            vec![lac; OPPONENTS + 1],
            Timeline::load(root, &route).unwrap(),
        ))
        .add_systems(Update, crate::le_mans_event::tick.after(crate::drive_tick));
        while post(&app).stage == Stage::Hidden {
            app.update();
            assert!(ticks(&app) < 4_000, "no end at the time limit");
        }
        assert_eq!(post(&app).stage, Stage::Stats);
        let d = app.world().resource::<Driving>();
        let s = &d.opponents.as_ref().unwrap().standings;
        assert!(s.over());
        // Every car still racing is classified with the limit as its time; a
        // car may be out (an AI recovery can wreck it within the limit).
        let out = |car| matches!(s.status(car), grid_tools::race::Status::Out(_));
        for car in 0..=OPPONENTS {
            if !out(car) {
                assert_eq!(s.finished(car), Some(LIMIT), "car {car} {:?}", s.status(car));
            }
        }
        assert!((0..=OPPONENTS).any(|car| !out(car)));
        // 40 s of a 40 s limit is the whole day: back to 15:00.
        let event = app.world().resource::<LeMansEvent>();
        assert_eq!(grid_tools::time_of_day::clock_text(event.time_of_day), "15:00:00");
        let rows = event.results(Some(s), results(&app));
        eprintln!("le_mans_results rows={rows:?}");
        assert_eq!(rows.len(), OPPONENTS + 1);
        let mut last = f64::INFINITY;
        for (i, r) in rows.iter().enumerate() {
            assert_eq!(r.position, i + 1);
            if out(r.car) {
                assert!(matches!(r.time, Cell::Out(..)), "{r:?}");
            } else {
                assert_eq!(r.time, Cell::Total(grid_tools::time_of_day::DAY_SECONDS));
            }
            let progress = s.progress(r.car);
            assert!(progress <= last);
            last = progress;
        }
        let view = post(&app).view(&rows, &Labels::default());
        eprintln!("le_mans_stats title={}", view.title);
        tap(&mut app, KeyCode::Enter);
        assert_eq!(post(&app).stage, Stage::Results);
        let view = post(&app).view(&rows, &Labels::default());
        assert_eq!(view.table[1][2], "1440:00.000");
    });
}
