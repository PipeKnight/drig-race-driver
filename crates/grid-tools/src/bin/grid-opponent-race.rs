//! Private headless race: N source cars in one shared world. Cars are driven by
//! the ported original AI driver (`ai`, default) or the authored pursuit
//! placeholder (`pursuit`). Reports laps, car-to-car contacts, failures and an
//! optional per-tick TSV of car 0's AI telemetry.
use grid_tools::{
    ai_driver::{AiData, AiDriver, NORMAL_DIFFICULTY, Other, Parameters, View},
    pursuit_driver::PursuitDriver,
    source_vehicle::{Controls, Runtime, multi::MultiRuntime},
};
use std::{io::Write, path::Path};

enum Driver {
    Ai(Box<AiDriver>),
    Pursuit(PursuitDriver),
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !(5..=7).contains(&args.len()) {
        return Err(
            "usage: grid-opponent-race ROOT CD01 BP01 CARS TICKS [ai|pursuit] [local/TRACE.tsv]"
                .into(),
        );
    }
    let root = Path::new(&args[0]);
    let table = std::fs::read(&args[1])?;
    let events = Some(grid_tools::body_impact_parameters::read(&std::fs::read(
        &args[2],
    )?)?);
    let cars: usize = args[3].parse()?;
    let ticks: u64 = args[4].parse()?;
    if !(1..=8).contains(&cars) || ticks > 200_000 {
        return Err("1..8 cars and at most 200000 ticks".into());
    }
    let kind = args.get(5).map_or("ai", String::as_str);
    let mut trace = match args.get(6) {
        Some(path) => {
            if !Path::new(path).starts_with("local") {
                return Err("trace must stay under local/".into());
            }
            Some(std::io::BufWriter::new(std::fs::File::create(path)?))
        }
        None => None,
    };
    let slots: Vec<String> = (0..cars).map(|i| format!("slot_{i:02}")).collect();
    let slot_refs: Vec<&str> = slots.iter().map(String::as_str).collect();
    let mut multi = MultiRuntime::lacetti(root, &slot_refs, &table, events)?;
    let data = AiData::load(root, "tracks/europe/donington/route_0", "touring", "lac")?;
    let progress = grid_tools::lap_progress::load(root)?.progress;
    let mut drivers: Vec<Driver> = (0..cars)
        .map(|i| -> Result<Driver, Box<dyn std::error::Error>> {
            Ok(match kind {
                "ai" => Driver::Ai(Box::new(AiDriver::new(
                    Parameters::new(&data, NORMAL_DIFFICULTY, |_| 1.)?,
                    0x5eed + i as u32,
                ))),
                "pursuit" => Driver::Pursuit(PursuitDriver::new(&progress, 18. + i as f64 * 0.5)),
                _ => return Err("driver must be ai or pursuit".into()),
            })
        })
        .collect::<Result<_, _>>()?;
    println!(
        "driver={kind} maximum_steering_angle={} line_length={}",
        multi.cars[0].maximum_steering_angle(),
        data.line.length
    );
    if let Some(t) = &mut trace {
        writeln!(
            t,
            "tick\tspeed\tthrottle\tbrake\tsteering\tarc\tlook_ahead\tcurvature\theading_error\tline_distance\tline_target\toffset\tline_error\tslip"
        )?;
    }
    let positions_of = |multi: &MultiRuntime| -> Vec<[f32; 3]> {
        multi
            .cars
            .iter()
            .map(|c| {
                let p = c.pose();
                [p[12], p[13], p[14]]
            })
            .collect()
    };
    let start = {
        let gate = &progress.gates[0];
        std::array::from_fn(|k| (gate.left[k] + gate.right[k]) * 0.5)
    };
    let mut standings =
        grid_tools::race::Standings::new(&data.line, 2, start, &positions_of(&multi));
    let names: Vec<String> = (0..cars).map(|i| format!("car{i}")).collect();
    let mut car_contacts = 0;
    let mut laps: Vec<Vec<f64>> = vec![Vec::new(); cars];
    let mut maximum_speed = 0f32;
    for _ in 0..ticks {
        let positions: Vec<Other> = multi
            .cars
            .iter()
            .map(|c| {
                let p = c.pose();
                Other {
                    position: [p[12], p[13], p[14]],
                    velocity: c.velocity(),
                }
            })
            .collect();
        let mut controls: Vec<Controls> = Vec::with_capacity(cars);
        for (i, (car, driver)) in multi.cars.iter().zip(&mut drivers).enumerate() {
            controls.push(match driver {
                Driver::Ai(ai) => {
                    let others: Vec<Other> = positions
                        .iter()
                        .enumerate()
                        .filter(|&(j, _)| j != i)
                        .map(|(_, p)| *p)
                        .collect();
                    let view = View {
                        pose: car.pose(),
                        velocity: car.velocity(),
                        angular: car.angular_velocity(),
                        maximum_steering_angle: car.maximum_steering_angle(),
                    };
                    ai.controls(&data, &view, &others, grid_tools::source_vehicle::DELTA)
                }
                Driver::Pursuit(p) => p.controls(car.pose(), car.velocity()),
            });
        }
        if let Err(error) = multi.tick(&controls) {
            println!("failure tick={} error={error}", multi.ticks + 1);
            break;
        }
        car_contacts += multi.last_car_contacts;
        standings.update(
            &data.line,
            &positions_of(&multi),
            multi.ticks as f64 * f64::from(grid_tools::source_vehicle::DELTA),
            &[],
        );
        if multi.ticks.is_multiple_of(1500) {
            let order: Vec<usize> = standings.order();
            println!("standings tick={} order={order:?}", multi.ticks);
        }
        let v = multi.cars[0].velocity();
        let speed = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        maximum_speed = maximum_speed.max(speed);
        if let (Some(t), Driver::Ai(ai)) = (&mut trace, &drivers[0]) {
            let c = controls[0];
            let m = ai.telemetry;
            writeln!(
                t,
                "{}\t{speed:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.2}\t{:.2}\t{:.5}\t{:.4}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.3}",
                multi.ticks,
                c.throttle,
                c.brake,
                c.steering,
                m.arc,
                m.look_ahead,
                m.curvature,
                m.heading_error,
                m.line_distance,
                m.line_target,
                m.offset,
                m.line_error,
                m.slip
            )?;
        }
        for (i, car) in multi.cars.iter().enumerate() {
            use grid_tools::lap_progress::Event;
            for event in &car.lap_events {
                if let Event::Finished(completed) = event {
                    laps[i].push(completed.seconds);
                }
            }
        }
        if (0..cars).all(|i| standings.finished(i).is_some()) {
            break;
        }
    }
    for (i, car) in multi.cars.iter().enumerate() {
        let lap = car.lap.as_ref().unwrap();
        let p = Runtime::pose(car);
        println!(
            "car{i} valid_laps={} attempts={} lap_times={:?} pos=({:.1},{:.1}) terminal={:?}",
            lap.valid_laps,
            lap.attempts,
            laps[i],
            p[12],
            p[14],
            car.terminal_wheel()
        );
    }
    for line in standings.classification(&names) {
        println!("classification {line}");
    }
    println!(
        "race ticks={} car_contacts={car_contacts} car0_max_speed={maximum_speed:.2}",
        multi.ticks
    );
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
