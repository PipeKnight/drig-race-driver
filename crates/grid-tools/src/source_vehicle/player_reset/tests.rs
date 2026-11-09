use super::*;
use crate::route::Route;
use grid_formats::progress::{Point, Route as ProgressRoute};
use grid_world::reset_lines::{FADE, MIDPOINT, TAIL, length};
use std::path::PathBuf;

fn progress(kind: TrackKind, reverse: bool, points: &[[f32; 3]], total: f32) -> Progress {
    let mut distance = 0.;
    let mut last = points[0];
    Progress {
        kind,
        total_distance: total,
        gates: Vec::new(),
        points: points
            .iter()
            .map(|&p| {
                distance += length([p[0] - last[0], p[1] - last[1], p[2] - last[2]]);
                last = p;
                Point {
                    distance,
                    position: p,
                }
            })
            .collect(),
        routes: vec![ProgressRoute {
            splits: Vec::new(),
            reverse,
        }],
    }
}

#[test]
fn reset_line_is_closed_on_a_circuit_and_ends_at_a_run_finish() {
    let square = [[0., 0., 0.], [100., 0., 0.], [100., 0., 100.], [0., 0., 100.]];
    let lap = ResetLine::from_progress(&progress(TrackKind::Circuit, false, &square, 400.)).unwrap();
    assert!(lap.line.closed());
    assert_eq!(lap.finish, None);
    assert_eq!(lap.line.length(), 400.);
    let run = ResetLine::from_progress(&progress(TrackKind::PointToPoint, false, &square, 290.)).unwrap();
    assert!(!run.line.closed());
    assert_eq!(run.finish, Some(290.));
    // Against the file order the direction at a point comes from the
    // previous one: at a corner it is the arriving side's, in file order
    // the leaving side's.
    let back = ResetLine::from_progress(&progress(TrackKind::PointToPoint, true, &square, 290.)).unwrap();
    for (at, arriving, leaving) in [(100., [1., 0., 0.], [0., 0., 1.]), (200., [0., 0., 1.], [-1., 0., 0.])] {
        let (_, d) = back.line.point_at(at);
        let (_, f) = run.line.point_at(at);
        let close = |a: [f32; 3], b: [f32; 3]| (0..3).all(|k| (a[k] - b[k]).abs() < 1e-3);
        assert!(close(d, arriving) && close(f, leaving), "{at}: {d:?} {f:?}");
    }
    assert!(ResetLine::from_progress(&progress(TrackKind::Circuit, false, &square[..1], 0.)).is_none());
}

/// The grid pose turned onto its roof, 1.5 m up.
fn flipped(mut pose: [f32; 16]) -> [f32; 16] {
    for k in 0..3 {
        pose[k] = -pose[k];
        pose[4 + k] = -pose[4 + k];
    }
    pose[13] += 1.5;
    pose
}

const HOLD: Controls = Controls {
    throttle: 0.,
    brake: 0.,
    steering: 0.,
    handbrake: true,
};

const GO: Controls = Controls {
    throttle: 1.,
    brake: 0.,
    steering: 0.,
    handbrake: false,
};

fn speed(car: &Runtime) -> f32 {
    let v = car.velocity();
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

fn assets() -> (PathBuf, Vec<u8>) {
    let root = PathBuf::from(std::env::var_os("GRID_GAME_ROOT").unwrap());
    let table = std::fs::read(std::env::var_os("GRID_CONTACT_DIRECTIONS").unwrap()).unwrap();
    (root, table)
}

/// Upright, on the road and heading along the line: the state right after
/// the placement (still, first gear, at the point) and two seconds later.
fn check_placed(car: &Runtime, reset: &Reset, rest: bool) {
    let p = car.pose();
    let drift = (p[12] - reset.point[0]).hypot(p[14] - reset.point[2]);
    let along = p[8] * reset.heading[0] + p[10] * reset.heading[2];
    let mut cache = grid_world::rectangle_cull::Cache::default();
    let below = car
        .track()
        .query(
            [p[12], p[13], p[14], 1.],
            [p[12], p[13] - 3., p[14], 1.],
            &mut cache,
        )
        .unwrap()
        .hit
        .map(|h| 3. * h.contact.fraction);
    // Anything solid within 20 m ahead at bonnet height.
    let (h, q) = (reset.heading, [p[12], p[13] + 0.5, p[14]]);
    let ahead = car
        .track()
        .query(
            [q[0], q[1], q[2], 1.],
            [q[0] + 20. * h[0], q[1], q[2] + 20. * h[2], 1.],
            &mut cache,
        )
        .unwrap()
        .hit
        .map(|h| 20. * h.contact.fraction);
    println!("placed at {:?} heading {:?} solid_ahead={ahead:?}", reset.point, reset.heading);
    println!(
        "placed rest={rest} up_y={} drift={drift} along={along} speed={} ground_below={below:?} gear={} retries={}",
        p[5],
        speed(car),
        car.state.car.owner.vehicle[0x196c / 4],
        reset.retries
    );
    assert!(p[5] > 0.95, "upright: {}", p[5]);
    assert!(along > 0.95, "heading along the line: {along}");
    assert!(below.is_some_and(|d| d < 1.), "on the road: {below:?}");
    if rest {
        assert!(speed(car) < 0.3, "at rest: {}", speed(car));
        assert!(drift < 0.5, "stays put: {drift}");
    } else {
        assert_eq!(speed(car), 0.);
        assert!(drift < 1e-3, "at the line point: {drift}");
        assert_eq!(car.state.car.owner.vehicle[0x196c / 4], 1);
        for i in 0..4 {
            let spin = car.state.car.owner.vehicle[(0x1990 + i * 0x150 + 0x104) / 4];
            assert_eq!(f32::from_bits(spin), 0.);
        }
    }
}

fn run(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(test)
        .unwrap()
        .join()
        .unwrap();
}

/// Ticks of the fade before the placement and of the whole job.
const PLACE_TICKS: u64 = ((FADE - MIDPOINT) / crate::source_vehicle::DELTA) as u64;
const JOB_TICKS: u64 = ((FADE + TAIL) / crate::source_vehicle::DELTA) as u64;

/// Flip the player's car on its roof on `route`'s grid and run the whole
/// job: the auto trigger after 2 s still and tilted, neutral input and a
/// darkening screen until the placement halfway through the fade, the car
/// upright at rest on the reset line with its damage kept, the job over
/// after the tail, and a car that drives off.
fn flipped_on(route: Route) {
    let (root, table) = assets();
    let mut car = Runtime::lacetti_on(&root, &route, "slot_00", 200679).unwrap();
    let geometry = car.prepare_body_geometry(&root, &table).unwrap();
    let line = ResetLine::load(&root, &route).unwrap().expect("a reset line");
    let damage = car.state.car.owner.vehicle[0xcdf0 / 4..0xce30 / 4].to_vec();
    let grid = car.pose();
    println!("{}: grid at {:?}", route.path(), [grid[12], grid[13], grid[14]]);
    car.set_pose_at_rest(flipped(car.pose()));
    let mut reset = PlayerReset::default();
    let mut still_since = None;
    let mut tilted_from = None;
    let mut started = None;
    let mut placed = None;
    let mut ended = None;
    for tick in 1..=1200u64 {
        // Full throttle is offered during the fade: it must not reach the car.
        let fading = started.is_some() && placed.is_none();
        let controls = reset.controls(if fading { GO } else { HOLD });
        if fading {
            assert_eq!(controls, Controls::default(), "input ignored until the placement");
        }
        car.tick_with_body_geometry(controls, &geometry).unwrap();
        let (v, w) = (car.velocity(), car.angular_velocity());
        let tilted = car.pose()[5] <= 0.17
            && length([v[0], v[1], v[2]]) + length([w[0], w[1], w[2]]) <= 0.8;
        if !tilted {
            still_since = None;
        } else if still_since.is_none() {
            still_since = Some(tick);
        }
        let clock = tick as f32 * crate::source_vehicle::DELTA;
        match reset.update(&mut car, None, Some(&line), clock).unwrap() {
            Some(Event::Started { auto, target }) => {
                assert!(auto);
                println!("{}: started at tick {tick} target {target}", route.path());
                started = Some(tick);
                tilted_from = still_since;
            }
            Some(Event::Placed(r)) => {
                assert!(reset.darkness() > 0.95, "black at the placement: {}", reset.darkness());
                check_placed(&car, &r, false);
                assert_eq!(car.state.car.owner.vehicle[0xcdf0 / 4..0xce30 / 4], damage[..]);
                placed = Some((tick, r));
            }
            Some(Event::Ended) => {
                ended = Some(tick);
                break;
            }
            Some(other) => panic!("unexpected {other:?}"),
            None => {}
        }
        // Two seconds after the placement, still inside the job's tail.
        if let Some((t, r)) = &placed
            && tick == t + 125
        {
            check_placed(&car, r, true);
        }
    }
    let started = started.expect("the auto-reset fires");
    let (placed, _) = placed.expect("the car is placed");
    let ended = ended.expect("the job ends");
    let tilted_from = tilted_from.expect("still and tilted before the trigger");
    println!("{}: tilted {tilted_from} started {started} placed {placed} ended {ended}", route.path());
    // 2 s of still-and-tilted (125 accumulating ticks), then the firing one.
    assert!((125..=127).contains(&(started - tilted_from)), "{started} {tilted_from}");
    assert!((PLACE_TICKS..=PLACE_TICKS + 2).contains(&(placed - started)), "{placed} {started}");
    assert!((JOB_TICKS - 1..=JOB_TICKS + 3).contains(&(ended - started)), "{ended} {started}");
    assert_eq!(reset.job, None);
    assert_eq!(reset.darkness(), 0.);
    // Drives off: an arena's line may point at a wall a few tens of metres
    // away, so the distance driven counts, not the final speed.
    let from = car.pose();
    let mut top = 0f32;
    for _ in 0..250 {
        car.tick_with_body_geometry(reset.controls(GO), &geometry).unwrap();
        top = top.max(speed(&car));
    }
    let p = car.pose();
    let driven = (p[12] - from[12]).hypot(p[14] - from[14]);
    println!("drive-off in 4 s: {driven} m, top speed {top}, final {}", speed(&car));
    assert!(driven > 10., "drives off: {driven} m");
}

#[test]
#[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS"]
fn a_flipped_player_car_is_reset_on_every_circuit() {
    for route in crate::route::CIRCUITS {
        run(move || flipped_on(Route::new(route).unwrap()));
    }
}

/// Haruna's touge roads both ways: the line is open and runs in travel order.
#[test]
#[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS"]
fn a_flipped_player_car_is_reset_on_the_touge_roads_both_ways() {
    for route in crate::route::HARUNA_RACED {
        for subroute in [0, 1] {
            run(move || flipped_on(Route::new(route).unwrap().with_subroute(subroute).unwrap()));
        }
    }
}

/// The Yokohama freestyle arenas: their placeholder progress is the line.
#[test]
#[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS"]
fn a_flipped_player_car_is_reset_in_the_drift_arenas() {
    let (root, _) = assets();
    for n in 3..=6 {
        let path = format!("tracks/japan/yokohama_docks/route_{n}");
        let route = Route::new(&path).unwrap().ignoring_progress();
        let line = ResetLine::load(&root, &route).unwrap();
        println!(
            "{path}: reset line {:?}",
            line.as_ref().map(|l| (l.line.closed(), l.line.length(), l.finish))
        );
    }
    let failed: Vec<_> = [4, 5, 6]
        .into_iter()
        .filter(|n| {
            let path = format!("tracks/japan/yokohama_docks/route_{n}");
            std::thread::Builder::new()
                .stack_size(64 * 1024 * 1024)
                .spawn(move || flipped_on(Route::new(&path).unwrap().ignoring_progress()))
                .unwrap()
                .join()
                .is_err()
        })
        .collect();
    assert!(failed.is_empty(), "arenas failed: {failed:?}");
}

/// The pause menu's Reset Vehicle runs the same job from a car on its
/// wheels; a second request during the job is refused.
#[test]
#[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS"]
fn the_menu_reset_runs_the_same_job() {
    run(|| {
        let (root, table) = assets();
        let route = Route::new(crate::route::DONINGTON).unwrap();
        let mut car = Runtime::lacetti_on(&root, &route, "slot_00", 200679).unwrap();
        let geometry = car.prepare_body_geometry(&root, &table).unwrap();
        let line = ResetLine::load(&root, &route).unwrap().unwrap();
        // Off the road: two seconds of full lock and throttle.
        let off = Controls {
            steering: 1.,
            ..GO
        };
        for _ in 0..125 {
            car.tick_with_body_geometry(off, &geometry).unwrap();
        }
        assert!(reset_request(&mut car, &geometry, &line));
    });
}

fn reset_request(
    car: &mut Runtime,
    geometry: &crate::body_geometry::BodyGeometry,
    line: &ResetLine,
) -> bool {
    let mut reset = PlayerReset::default();
    let clock = 100.;
    // Track the car before the request, as the drive tick does every tick.
    car.tick_with_body_geometry(HOLD, geometry).unwrap();
    assert_eq!(reset.update(car, None, Some(line), clock).unwrap(), None);
    let before = car.pose();
    assert!(reset.request());
    car.tick_with_body_geometry(HOLD, geometry).unwrap();
    let Some(Event::Started { auto: false, target }) =
        reset.update(car, None, Some(line), clock).unwrap()
    else {
        panic!("the menu request starts a job");
    };
    // After the early window the target is the tracker's own trackpos.
    assert_eq!(target, reset.tracker.distance);
    assert!(!reset.request(), "refused while the job runs");
    for _ in 0..PLACE_TICKS + 3 {
        car.tick_with_body_geometry(reset.controls(GO), geometry).unwrap();
        if let Some(Event::Placed(r)) = reset.update(car, None, Some(line), clock).unwrap() {
            println!(
                "menu reset from ({}, {}) to {:?} target {target}",
                before[12], before[14], r.point
            );
            check_placed(car, &r, false);
            return true;
        }
    }
    false
}

/// Demolition Derby: the same trigger disqualifies the stock car on the
/// Detroit stadium instead of resetting it; its controls are then
/// disabled (status 6, vehicle_sync) and the car stays where it is.
#[test]
#[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS"]
fn a_flipped_derby_car_is_disqualified_not_reset() {
    run(|| {
        let (root, table) = assets();
        let route = Route::new(crate::route::DETROIT_STADIUM).unwrap();
        let stock = crate::car::Car::new("stk").unwrap();
        let mut car = Runtime::load(&root, &route, &stock, "slot_00", 200679).unwrap();
        let geometry = car.prepare_body_geometry(&root, &table).unwrap();
        let line = ResetLine::load(&root, &route).unwrap();
        car.set_pose_at_rest(flipped(car.pose()));
        let mut reset = PlayerReset {
            demolition: true,
            ..Default::default()
        };
        let mut at = None;
        for tick in 1..=1200u64 {
            car.tick_with_body_geometry(HOLD, &geometry).unwrap();
            match reset.update(&mut car, None, line.as_ref(), 100.).unwrap() {
                None => {}
                Some(Event::Disqualified) => {
                    at = Some(tick);
                    break;
                }
                Some(other) => panic!("unexpected {other:?}"),
            }
        }
        let tick = at.expect("the flipped derby car is disqualified");
        assert!(car.disqualified());
        assert_eq!(reset.job, None);
        let pose = car.pose();
        assert!(pose[5] < 0.17, "still on its roof: {}", pose[5]);
        // The next tick synchronises the disabled controls.
        car.tick_with_body_geometry(HOLD, &geometry).unwrap();
        let disabled = f32::from_bits(car.state.car.owner.vehicle[0x226c / 4]);
        assert_eq!(disabled, 1., "controls disabled after tick {tick}");
        assert!(!car.disqualify(), "only once");
    });
}

fn shared_tick(player: &mut Runtime, multi: &mut MultiRuntime, controls: [Controls; 2]) {
    std::mem::swap(player, &mut multi.cars[0]);
    multi.tick(&controls).unwrap();
    std::mem::swap(player, &mut multi.cars[0]);
}

/// The shared world: the player runtime is held outside slot 0 between
/// ticks, as in grid-app and the soak Session. The placed car is a ghost to
/// the other car until the job ends.
#[test]
#[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS"]
fn a_flipped_player_car_is_reset_in_the_shared_world() {
    run(|| {
        let (root, table) = assets();
        let mut multi = MultiRuntime::lacetti(&root, &["slot_00", "slot_01"], &table, None).unwrap();
        let line = ResetLine::load(&root, multi.cars[0].route()).unwrap().unwrap();
        let pose = flipped(multi.cars[0].pose());
        multi.set_pose_at_rest(0, pose);
        let mut player = Runtime::lacetti(&root, "slot_00", 200679).unwrap();
        std::mem::swap(&mut player, &mut multi.cars[0]);
        let mut reset = PlayerReset::default();
        let mut placed = None;
        let mut ended = None;
        for tick in 1..=1200u64 {
            let hold = reset.controls(HOLD);
            shared_tick(&mut player, &mut multi, [hold, HOLD]);
            let clock = tick as f32 * crate::source_vehicle::DELTA;
            match reset.update(&mut player, Some(&mut multi), Some(&line), clock).unwrap() {
                Some(Event::Placed(r)) => {
                    check_placed(&player, &r, false);
                    assert!(multi.ghost(0), "a ghost after the placement");
                    placed = Some((tick, r));
                }
                Some(Event::Ended) => {
                    ended = Some(tick);
                    break;
                }
                _ => {}
            }
        }
        let (tick, r) = placed.expect("the car is placed");
        let ended = ended.expect("the job ends");
        println!("shared world: placed at tick {tick}, ended at {ended}");
        assert!(!multi.ghost(0), "collides again after the job");
        check_placed(&player, &r, true);
        assert!(multi.retirement(0).is_none() && multi.retirement(1).is_none());
    });
}

/// A placement onto another car steps back along the line until it is
/// clear, and the job ends only once the cars do not overlap.
#[test]
#[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS"]
fn a_reset_onto_another_car_steps_back_along_the_line() {
    run(|| {
        let (root, table) = assets();
        let mut multi = MultiRuntime::lacetti(&root, &["slot_00", "slot_01"], &table, None).unwrap();
        let line = ResetLine::load(&root, multi.cars[0].route()).unwrap().unwrap();
        // Car 1 parked on the line 300 m into the lap.
        let (point, heading) = line.line.point_at(300.);
        multi.place_at_rest(1, point, heading).unwrap();
        let mut player = Runtime::lacetti(&root, "slot_00", 200679).unwrap();
        std::mem::swap(&mut player, &mut multi.cars[0]);
        let r = place(&mut player, Some(&mut multi), &line, 300.).unwrap();
        println!("wedge: {} retries, placed at {} m", r.retries, r.distance);
        assert!((3..=8).contains(&r.retries), "{}", r.retries);
        assert!((r.distance - (300. - r.retries as f32)).abs() < 1e-3);
        std::mem::swap(&mut player, &mut multi.cars[0]);
        assert!(!multi.overlaps_other_car(0).unwrap());
        std::mem::swap(&mut player, &mut multi.cars[0]);
        check_placed(&player, &r, false);
    });
}
