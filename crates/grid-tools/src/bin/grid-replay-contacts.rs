//! Private diagnostic: replay DR01 controls (no pose checks) and, inside a tick
//! window, report the cold geometric contacts by shape together with the true
//! deepest hull-point overlap with route prop boxes.
use grid_tools::source_vehicle::{Controls, Runtime};
use std::{fs, path::Path};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.len() != 7 {
        return Err("usage: grid-replay-contacts ROOT CD01 BP01 DF01 DR01 FROM TO".into());
    }
    let raw = fs::read(&a[4])?;
    if raw.len() < 8 || &raw[..4] != b"DR01" {
        return Err("invalid replay".into());
    }
    let n = u32::from_le_bytes(raw[4..8].try_into()?) as usize;
    let (from, to): (u64, u64) = (a[5].parse()?, a[6].parse()?);
    let root = Path::new(&a[0]);
    let mut runtime = Runtime::lacetti(root, "slot_00", 200679)?;
    let mut geometry = runtime.prepare_body_geometry(root, &fs::read(&a[1])?)?;
    runtime.body_events = Some(grid_tools::body_impact_parameters::read(&fs::read(&a[2])?)?);
    runtime.enable_deformation(root, &mut geometry)?;
    let (limit, bulk) = grid_tools::deformation_parameters::read(&fs::read(&a[3])?)?;
    runtime.register_deformation_callbacks(limit, bulk, 1)?;
    let props_shape = runtime.track_shape_count() as u32 + 1;
    for r in raw[8..8 + n * 100].as_chunks::<100>().0 {
        let u = |o: usize| u32::from_le_bytes(r[o..o + 4].try_into().unwrap());
        let controls = Controls {
            throttle: f32::from_bits(u(0)),
            brake: f32::from_bits(u(4)),
            steering: f32::from_bits(u(8)),
            handbrake: u(12) != 0,
        };
        runtime.tick_with_deforming_body_geometry(controls, &mut geometry)?;
        if runtime.ticks < from {
            continue;
        }
        if runtime.ticks > to {
            break;
        }
        let body = runtime.state.world.groups.bodies[0];
        let contacts = geometry.contacts(runtime.track(), runtime.surfaces(), &[body], 32)?;
        let (mut props, mut boundary, mut other, mut deepest) = (0, 0, 0, 0f32);
        for c in &contacts {
            let pen = f32::from_bits(c[0x38 / 4]).abs();
            match c[0x2c / 4] {
                s if s == props_shape => {
                    props += 1;
                    deepest = deepest.max(pen);
                }
                s if s == props_shape - 1 => boundary += 1,
                _ => other += 1,
            }
        }
        let overlap = runtime
            .track()
            .props()
            .and_then(|p| p.deepest_overlap(&geometry.hull_world_points(&body).ok()?));
        let v = runtime.velocity();
        println!(
            "tick={} speed={:.2} applied_body_contacts={} fresh: props={props} (max pen {deepest:.3}) boundary={boundary} other={other} hull_overlap={}",
            runtime.ticks,
            (v[0] * v[0] + v[2] * v[2]).sqrt(),
            runtime.last_body_contacts,
            overlap.map_or("none".into(), |(i, d)| format!(
                "{:.3} m in {}",
                d,
                runtime.track().props().unwrap().boxes[i].template
            ))
        );
    }
    Ok(())
}

fn main() {
    let result = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| run().map_err(|e| e.to_string()))
        .expect("replay worker")
        .join()
        .expect("replay worker panic");
    if let Err(error) = result {
        eprintln!("grid-replay-contacts: {error}");
        std::process::exit(1);
    }
}
