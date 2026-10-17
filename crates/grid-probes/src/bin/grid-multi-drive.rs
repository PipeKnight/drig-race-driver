//! Private shared-world checks. `independence`: car 0 drives while car 1 idles on
//! its grid slot; car 0 must match the single-car runtime exactly until contact.
//! `push`: car 0 idles while car 1 drives at it with constant steering; reports
//! car-to-car contacts and car 0's displacement.
use grid_tools::source_vehicle::{Controls, Runtime, multi::MultiRuntime};
use std::path::Path;

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 5 {
        return Err("usage: grid-multi-drive ROOT CD01 BP01 MODE TICKS [STEER] [SLOT1]".into());
    }
    let root = Path::new(&args[0]);
    let table = std::fs::read(&args[1])?;
    let events = Some(grid_tools::body_impact_parameters::read(&std::fs::read(
        &args[2],
    )?)?);
    let ticks: u64 = args[4].parse()?;
    let steer: f32 = args.get(5).map_or(Ok(0.), |s| s.parse())?;
    let slot1 = args.get(6).map_or("slot_01", String::as_str);
    let mut multi = MultiRuntime::lacetti(root, &["slot_00", slot1], &table, events)?;
    let start: Vec<[f32; 16]> = multi.cars.iter().map(Runtime::pose).collect();
    println!(
        "start car0=({:.2},{:.2}) car1=({:.2},{:.2})",
        start[0][12], start[0][14], start[1][12], start[1][14]
    );
    match args[3].as_str() {
        "independence" => {
            let mut single = Runtime::lacetti(root, "slot_00", 200679)?;
            let geometry = single.prepare_body_geometry(root, &table)?;
            single.body_events = events;
            single.enable_lap_timing(root)?;
            let drive = Controls {
                throttle: 1.,
                steering: steer,
                ..Default::default()
            };
            let mut first_difference = None;
            for t in 1..=ticks {
                single.tick_with_body_geometry(drive, &geometry)?;
                multi.tick(&[drive, Controls::default()])?;
                // Pose, angular and linear velocity (physical 1D0..230) plus the
                // complete vehicle; world-link words legitimately differ.
                let motion =
                    |r: &Runtime| r.state.car.owner.physical[0x1d0 / 4..0x230 / 4].to_vec();
                let pose_differs = motion(&single) != motion(&multi.cars[0]);
                let vehicle_differs =
                    single.state.car.owner.vehicle != multi.cars[0].state.car.owner.vehicle;
                if first_difference.is_none() && (pose_differs || vehicle_differs) {
                    let word = single
                        .state
                        .car
                        .owner
                        .vehicle
                        .iter()
                        .zip(&multi.cars[0].state.car.owner.vehicle)
                        .position(|(a, b)| a != b)
                        .map(|w| format!("{:x}", w * 4));
                    first_difference = Some((t, pose_differs, word, multi.last_car_contacts));
                }
            }
            let p = multi.cars[0].pose();
            let q = multi.cars[1].pose();
            println!(
                "independence ticks={ticks} first_difference={first_difference:?} car0=({:.2},{:.2}) car1_moved={:.4}",
                p[12],
                p[14],
                ((q[12] - start[1][12]).powi(2) + (q[14] - start[1][14]).powi(2)).sqrt()
            );
        }
        "push" => {
            let mut total = 0;
            let mut first = None;
            for t in 1..=ticks {
                multi.tick(&[
                    Controls::default(),
                    Controls {
                        throttle: 1.,
                        steering: steer,
                        ..Default::default()
                    },
                ])?;
                total += multi.last_car_contacts;
                if first.is_none() && multi.last_car_contacts > 0 {
                    first = Some(t);
                }
            }
            let p = multi.cars[0].pose();
            let q = multi.cars[1].pose();
            let v = multi.cars[0].velocity();
            println!(
                "push ticks={ticks} car_contacts={total} first_contact_tick={first:?} car0_moved={:.3} car0_speed={:.3} car1=({:.2},{:.2})",
                ((p[12] - start[0][12]).powi(2) + (p[14] - start[0][14]).powi(2)).sqrt(),
                (v[0] * v[0] + v[2] * v[2]).sqrt(),
                q[12],
                q[14]
            );
        }
        _ => return Err("mode must be independence or push".into()),
    }
    Ok(())
}

fn main() {
    let result = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| run().map_err(|e| e.to_string()))
        .expect("multi worker")
        .join()
        .expect("multi worker panic");
    if let Err(error) = result {
        eprintln!("grid-multi-drive: {error}");
        std::process::exit(1);
    }
}
