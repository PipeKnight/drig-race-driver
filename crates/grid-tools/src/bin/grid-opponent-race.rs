//! Private headless race: N source cars in one shared world, each driven by the
//! authored placeholder pursuit driver with its own speed cap. Reports laps,
//! car-to-car contacts and failures. Not original AI.
use grid_tools::{
    pursuit_driver::PursuitDriver,
    source_vehicle::{Runtime, multi::MultiRuntime},
};
use std::path::Path;

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 5 {
        return Err("usage: grid-opponent-race ROOT CD01 BP01 CARS TICKS".into());
    }
    let root = Path::new(&args[0]);
    let table = std::fs::read(&args[1])?;
    let events = Some(grid_tools::body_impact_parameters::read(&std::fs::read(
        &args[2],
    )?)?);
    let cars: usize = args[3].parse()?;
    let ticks: u64 = args[4].parse()?;
    if !(2..=8).contains(&cars) || ticks > 100_000 {
        return Err("2..8 cars and at most 100000 ticks".into());
    }
    let slots: Vec<String> = (0..cars).map(|i| format!("slot_{i:02}")).collect();
    let slot_refs: Vec<&str> = slots.iter().map(String::as_str).collect();
    let mut multi = MultiRuntime::lacetti(root, &slot_refs, &table, events)?;
    let progress = grid_tools::lap_progress::load(root)?.progress;
    let mut drivers: Vec<PursuitDriver> = (0..cars)
        .map(|i| PursuitDriver::new(&progress, 18. + i as f64 * 0.5))
        .collect();
    let mut car_contacts = 0;
    for _ in 0..ticks {
        let controls: Vec<_> = multi
            .cars
            .iter()
            .zip(&mut drivers)
            .map(|(car, driver)| driver.controls(car.pose(), car.velocity()))
            .collect();
        if let Err(error) = multi.tick(&controls) {
            println!("failure tick={} error={error}", multi.ticks + 1);
            break;
        }
        car_contacts += multi.last_car_contacts;
        if multi
            .cars
            .iter()
            .all(|c| c.lap.as_ref().is_some_and(|l| l.valid_laps >= 1))
        {
            break;
        }
    }
    for (i, car) in multi.cars.iter().enumerate() {
        let lap = car.lap.as_ref().unwrap();
        let p = Runtime::pose(car);
        println!(
            "car{i} valid_laps={} attempts={} lap_elapsed={:?} pos=({:.1},{:.1}) terminal={:?}",
            lap.valid_laps,
            lap.attempts,
            lap.elapsed(),
            p[12],
            p[14],
            car.terminal_wheel()
        );
    }
    println!("race ticks={} car_contacts={car_contacts}", multi.ticks);
    Ok(())
}

fn main() {
    let result = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| run().map_err(|e| e.to_string()))
        .expect("race worker")
        .join()
        .expect("race worker panic");
    if let Err(error) = result {
        eprintln!("grid-opponent-race: {error}");
        std::process::exit(1);
    }
}
