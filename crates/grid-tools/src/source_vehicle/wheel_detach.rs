//! Asset tests of wheel detachment (docs/MECHANICAL_DAMAGE.md section 5):
//! the loose-part rule on a real car, then the car without the wheel.
use super::{Controls, Runtime, multi::MultiRuntime};
use crate::{body_impacts::Impact, car::Car, route::Route};
use std::path::Path;

const DRIVE: Controls = Controls {
    throttle: 1.,
    brake: 0.,
    steering: 0.,
    handbrake: false,
};

fn run(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(test)
        .unwrap()
        .join()
        .unwrap();
}

fn f(car: &Runtime, o: usize) -> f32 {
    f32::from_bits(car.state.car.owner.vehicle[o / 4])
}

fn speed(car: &Runtime) -> f32 {
    let v = car.velocity();
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

/// World position of a car-local point (pose rows are the basis).
fn world(car: &Runtime, local: [f32; 3]) -> [f32; 3] {
    let p = car.pose();
    std::array::from_fn(|j| p[12 + j] + local[0] * p[j] + local[1] * p[4 + j] + local[2] * p[8 + j])
}

/// A hit on the front-left corner, ahead of the front-left wheel and pushing
/// the car backwards (the shape of a corner into a wall).
fn front_left_hit(car: &Runtime, impact: f32) -> Impact {
    let anchor = car.wheels()[2].anchor;
    let p = car.pose();
    let back: [f32; 3] = std::array::from_fn(|j| -p[8 + j]);
    Impact {
        point: world(car, [anchor[0] + 0.1, anchor[1] + 0.3, anchor[2] + 0.9]),
        normal: back,
        impact,
    }
}

fn mass(car: &Runtime) -> f32 {
    f32::from_bits(car.state.car.owner.physical[0x3f0 / 4])
}

/// Drive 2 s, lose the front-left wheel to one corner hit, then 4 s more.
fn lose_front_left(stem: &str) {
    let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
    let root = Path::new(&root);
    let table = std::fs::read(std::env::var_os("GRID_CONTACT_DIRECTIONS").unwrap()).unwrap();
    let model = Car::new(stem).unwrap();
    let mut car = Runtime::load(root, &Route::default(), &model, "slot_00", 200679).unwrap();
    let geometry = car.prepare_body_geometry(root, &table).unwrap();
    for _ in 0..125 {
        car.tick_with_body_geometry(DRIVE, &geometry).unwrap();
    }
    let threshold = 6. * (mass(&car) * 4.);
    println!(
        "{stem}: mass {} threshold {threshold} speed {}",
        mass(&car),
        speed(&car)
    );
    // Trigger: at the threshold nothing happens, just above it the
    // front-left wheel goes (the front-right zone gets only 0.2 of it).
    assert_eq!(
        car.detach_wheels(&[front_left_hit(&car, threshold)]),
        [false; 4]
    );
    assert_eq!(car.detached_wheels(), [false; 4]);
    let lost = car.detach_wheels(&[front_left_hit(&car, threshold * 1.01)]);
    assert_eq!(lost, [false, false, true, false]);
    assert_eq!(car.detached_wheels(), lost);
    assert_eq!(car.terminal_wheel(), None, "terminal only on the next tick");
    let spin_before = f(&car, 0x1990 + 2 * 0x150 + 0x104);
    assert!(
        spin_before.abs() > 1.,
        "the wheel was turning: {spin_before}"
    );

    // First tick without it: 9E65B0 makes it terminal damage (status 4); the
    // wheel has no grip or contact. (A03670 zeroes its spin, but on a driven
    // wheel the drivetrain turns it again later in the same tick.)
    car.tick_with_body_geometry(DRIVE, &geometry).unwrap();
    assert_eq!(car.terminal_wheel(), Some(2));
    assert!(car.lost_wheel());
    let w = 0x1990 + 2 * 0x150;
    assert_eq!(f(&car, w + 0x78), 0., "grip");
    assert_eq!(
        car.state.car.owner.vehicle[0x2278 / 4] >> 16 & 255,
        1,
        "body copy +227A"
    );
    // The suspension still meets the floor at that corner: 9C1F60 restarts
    // the travel at zero each substep but has no detached check in its floor
    // contact loop, so the bare hub rides on the floor stop without grip.
    let contact =
        |car: &Runtime, i: usize| car.state.car.owner.vehicle[(0x3d0 + i * 0x170 + 0x84) / 4] & 255;
    // A wheel detaches once: the same hit again changes nothing.
    assert_eq!(
        car.detach_wheels(&[front_left_hit(&car, threshold * 2.)]),
        [false; 4]
    );

    // The car runs on, slides to a stop on three wheels and the bare front
    // left corner sits lower than the front right.
    for _ in 0..250 {
        car.tick_with_body_geometry(DRIVE, &geometry).unwrap();
        assert_eq!(f(&car, w + 0x78), 0., "no grip");
    }
    let anchors = car.wheels().map(|w| w.anchor);
    let fl = world(&car, anchors[2])[1];
    let fr = world(&car, anchors[3])[1];
    println!(
        "{stem}: after 4 s speed {} front-left hub {fl} front-right hub {fr} contacts {:?} loads {:?}",
        speed(&car),
        [0, 1, 2, 3].map(|i| contact(&car, i)),
        [0, 1, 2, 3].map(|i| f(&car, 0x1990 + i * 0x150 + 0x138))
    );
    // 9BD7F0 copies status != 7 to +226C at the start of each later tick.
    assert_eq!(f(&car, 0x226c), 1., "controls disabled");
    assert!(speed(&car) < 3., "comes to rest: {}", speed(&car));
    assert!(fl < fr - 0.03, "the bare corner is down: {fl} vs {fr}");
    assert_eq!(car.terminal_wheel(), Some(2));
}

#[test]
#[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS"]
fn a_lacetti_loses_its_front_left_wheel_to_a_corner_hit() {
    run(|| lose_front_left("lac"));
}

#[test]
#[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS"]
fn a_bmw_loses_its_front_left_wheel_to_a_corner_hit() {
    run(|| lose_front_left("bmw"));
}

#[test]
#[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS"]
fn damage_off_keeps_every_wheel() {
    run(|| {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let mut car = Runtime::lacetti(Path::new(&root), "slot_00", 200679).unwrap();
        let mut assists = car.assists;
        assists.damage = grid_world::mechanical_damage::Damage::Off;
        car.set_assists(assists).unwrap();
        let hit = front_left_hit(&car, 1e6);
        assert_eq!(car.detach_wheels(&[hit]), [false; 4]);
        assert_eq!(car.detached_wheels(), [false; 4]);
    });
}

/// The shared world: an opponent loses a wheel and the world races on.
#[test]
#[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS"]
fn a_shared_world_car_loses_a_wheel_without_retiring() {
    run(|| {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let root = Path::new(&root);
        let table = std::fs::read(std::env::var_os("GRID_CONTACT_DIRECTIONS").unwrap()).unwrap();
        let mut multi = MultiRuntime::lacetti(root, &["slot_00", "slot_01"], &table, None).unwrap();
        for _ in 0..60 {
            multi.tick(&[DRIVE, DRIVE]).unwrap();
        }
        let threshold = 6. * (mass(&multi.cars[1]) * 4.);
        let hit = front_left_hit(&multi.cars[1], threshold * 1.01);
        assert_eq!(
            multi.cars[1].detach_wheels(&[hit]),
            [false, false, true, false]
        );
        for _ in 0..250 {
            multi.tick(&[DRIVE, DRIVE]).unwrap();
        }
        assert!(multi.retirement(0).is_none() && multi.retirement(1).is_none());
        assert_eq!(multi.cars[1].terminal_wheel(), Some(2));
        assert_eq!(multi.cars[0].terminal_wheel(), None);
        assert_eq!(multi.cars[0].detached_wheels(), [false; 4]);
        println!(
            "shared: car 0 speed {} car 1 speed {}",
            speed(&multi.cars[0]),
            speed(&multi.cars[1])
        );
        assert!(speed(&multi.cars[0]) > 10.);
    });
}
