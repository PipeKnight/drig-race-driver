//! Private diagnostic: replay a DR01 control log with exact sampled-pose checks
//! (deformation enabled) and, inside a tick window, print per-wheel damage
//! inputs (level, anchor displacement +1650, angular deformation +1790, grid
//! correction +C740) and the car's contact points in car coordinates
//! (+X left, +Z forward). Explains which impact drives which wheel's damage.
use grid_tools::source_vehicle::{Controls, Runtime};
use std::{fs, path::Path};
const RECORD: usize = 100;

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.len() != 7 {
        return Err("usage: grid-wheel-damage-trace ROOT CD01 BP01 DF01 DR01 FROM TO".into());
    }
    let raw = fs::read(&a[4])?;
    if raw.len() < 8 || &raw[..4] != b"DR01" {
        return Err("invalid replay".into());
    }
    let n = u32::from_le_bytes(raw[4..8].try_into()?) as usize;
    if raw.len() != 8 + n * RECORD {
        return Err("invalid replay extent".into());
    }
    let (from, to): (u64, u64) = (a[5].parse()?, a[6].parse()?);
    let root = Path::new(&a[0]);
    let mut runtime = Runtime::lacetti(root, "slot_00", 200679)?;
    let mut geometry = runtime.prepare_body_geometry(root, &fs::read(&a[1])?)?;
    runtime.body_events = Some(grid_tools::body_impact_parameters::read(&fs::read(&a[2])?)?);
    runtime.enable_deformation(root, &mut geometry)?;
    let (limit, bulk) = grid_tools::deformation_parameters::read(&fs::read(&a[3])?)?;
    runtime.register_deformation_callbacks(limit, bulk, 1)?;
    let m = grid_world::vehicle_handling::MUTABLE;
    let mut checked = 0;
    let mut previous_nodes: Option<Vec<[f32; 3]>> = None;
    for (index, r) in raw[8..].as_chunks::<RECORD>().0.iter().enumerate() {
        let u = |o: usize| u32::from_le_bytes(r[o..o + 4].try_into().unwrap());
        let controls = Controls {
            throttle: f32::from_bits(u(0)),
            brake: f32::from_bits(u(4)),
            steering: f32::from_bits(u(8)),
            handbrake: u(12) != 0,
        };
        runtime.tick_with_deforming_body_geometry(controls, &mut geometry)?;
        let pose = runtime.pose();
        if u(16) != 0 {
            let actual: Vec<u32> = pose
                .into_iter()
                .chain(runtime.velocity())
                .map(f32::to_bits)
                .collect();
            if actual.iter().enumerate().any(|(i, w)| *w != u(20 + 4 * i)) {
                return Err(format!("sample mismatch tick={}", index + 1).into());
            }
            checked += 1;
        }
        let tick = runtime.ticks;
        if tick < from || tick > to {
            continue;
        }
        let b = &runtime.state.car.owner.vehicle;
        let f = |o: usize| f32::from_bits(b[o / 4]);
        let levels: Vec<f32> = (0..4).map(|i| f(m + 0x18 + 4 * i)).collect();
        let displacement: Vec<[f32; 3]> = (0..4)
            .map(|i| std::array::from_fn(|j| f(0x1650 + 16 * i + 4 * j)))
            .collect();
        let angular: Vec<f32> = (0..4).map(|i| f(0x1790 + 4 * i)).collect();
        let correction: [f32; 3] = std::array::from_fn(|j| f(0xc740 + 4 * j));
        println!(
            "tick={tick} status={} terminal={} levels={levels:.3?} displacement={displacement:.3?} angular={angular:.3?} correction={correction:.3?}",
            b[(m + 0x444) / 4],
            (b[(m + 0x448) / 4] >> (8 * ((m + 0x448) % 4))) & 255
        );
        // Grid nodes that moved this tick: rest position relative to the
        // wheel-sampling center (+1630) versus the wheel anchors.
        let count = (0..3)
            .map(|i| b[0xc714 / 4 + i] as usize)
            .product::<usize>();
        let center: [f32; 3] = std::array::from_fn(|j| f(0x1630 + 4 * j));
        let nodes: Vec<[f32; 3]> = (0..count)
            .map(|i| std::array::from_fn(|j| f(0x4880 + i * 80 + 4 * j)))
            .collect();
        if tick == from {
            let minimum: [f32; 3] = std::array::from_fn(|j| f(0xc730 + 4 * j));
            println!(
                "  grid dims={:?} spacing={:.3} minimum={minimum:.3?} center_1630={center:.3?} anchors={:.3?}",
                (0..3).map(|i| b[0xc714 / 4 + i]).collect::<Vec<_>>(),
                f(0xc720),
                (0..4)
                    .map(|i| [
                        f(0x1990 + i * 0x150),
                        f(0x1994 + i * 0x150),
                        f(0x1998 + i * 0x150)
                    ])
                    .collect::<Vec<_>>()
            );
        }
        if let Some(previous) = &previous_nodes {
            let mut moved: Vec<(usize, f32)> = nodes
                .iter()
                .zip(previous)
                .enumerate()
                .map(|(i, (n, p))| {
                    (
                        i,
                        ((n[0] - p[0]).powi(2) + (n[1] - p[1]).powi(2) + (n[2] - p[2]).powi(2))
                            .sqrt(),
                    )
                })
                .filter(|&(_, d)| d > 0.001)
                .collect();
            moved.sort_by(|a, b| b.1.total_cmp(&a.1));
            for &(i, d) in moved.iter().take(8) {
                let rest: [f32; 3] = std::array::from_fn(|j| f(0x4880 + i * 80 + 16 + 4 * j));
                println!(
                    "  node {i} moved={d:.3} rest={rest:.2?} now={:.2?}",
                    nodes[i]
                );
            }
        }
        previous_nodes = Some(nodes);
        // Contacts of the last solve, in car coordinates.
        let origin = [pose[12], pose[13], pose[14]];
        let axis = |row: usize| [pose[row * 4], pose[row * 4 + 1], pose[row * 4 + 2]];
        let (x, y, z) = (axis(0), axis(1), axis(2));
        let world = &runtime.state.world.groups.contacts;
        for (k, c) in world.iter().enumerate().take(64) {
            let depth = f32::from_bits(c[0x38 / 4]);
            let impulse = f32::from_bits(c[0x3c / 4]);
            if c[1] == 0 || (depth == 0. && impulse == 0.) {
                continue;
            }
            let p: [f32; 3] = std::array::from_fn(|j| f32::from_bits(c[0x50 / 4 + j]) - origin[j]);
            let dot = |v: [f32; 3]| v[0] * p[0] + v[1] * p[1] + v[2] * p[2];
            // Retained slots from earlier ticks lie far from the car.
            if p.iter().map(|v| v * v).sum::<f32>() > 100. {
                continue;
            }
            println!(
                "  contact {k} shape={} local=({:.2},{:.2},{:.2}) depth={depth:.3} impulse={impulse:.1}",
                c[0x2c / 4],
                dot(x),
                dot(y),
                dot(z)
            );
        }
    }
    println!("replay ticks={n} exact_samples={checked}");
    Ok(())
}

fn main() {
    let result = std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| run().map_err(|e| e.to_string()))
        .expect("trace thread")
        .join()
        .expect("trace panic");
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1)
    }
}
