//! Private control replay, with exact sampled-pose checks before a reported stop.
use grid_tools::source_vehicle::{Controls, Runtime};
use std::{fs, io::Write, path::Path};
const RECORD: usize = 100;
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = std::env::args_os().skip(1).collect();
    if a.len() != 6 {
        return Err("usage: grid-drive-replay ROOT CD01 BP01 DF01 DR01_INPUT local/OUTPUT".into());
    }
    if fs::metadata(&a[4])?.len() > 8 + 36_000 * RECORD as u64 {
        return Err("oversized replay".into());
    }
    let raw = fs::read(&a[4])?;
    if raw.len() < 8 || &raw[..4] != b"DR01" {
        return Err("invalid replay tag".into());
    }
    let n = u32::from_le_bytes(raw[4..8].try_into()?) as usize;
    if !(1..=36_000).contains(&n) || raw.len() != 8 + n * RECORD {
        return Err("invalid replay extent".into());
    }
    let root = Path::new(&a[0]);
    let mut runtime = Runtime::lacetti(root, "slot_00", 200679)?;
    let mut geometry = runtime.prepare_body_geometry(root, &fs::read(&a[1])?)?;
    runtime.body_events = Some(grid_tools::body_impact_parameters::read(&fs::read(&a[2])?)?);
    runtime.enable_deformation(root, &mut geometry)?;
    let (limit, bulk) = grid_tools::deformation_parameters::read(&fs::read(&a[3])?)?;
    runtime.register_deformation_callbacks(limit, bulk, 1)?;
    let mut output = String::new();
    let mut checked = 0;
    for (index, r) in raw[8..].as_chunks::<RECORD>().0.iter().enumerate() {
        let u = |o| u32::from_le_bytes(r[o..o + 4].try_into().unwrap());
        if u(12) > 1 || u(16) > 1 {
            return Err("invalid replay boolean".into());
        }
        let controls = Controls {
            throttle: f32::from_bits(u(0)),
            brake: f32::from_bits(u(4)),
            steering: f32::from_bits(u(8)),
            handbrake: u(12) != 0,
        };
        runtime.tick_with_deforming_body_geometry(controls, &mut geometry)?;
        let pose = runtime.pose();
        let velocity = runtime.velocity();
        let actual: Vec<_> = pose.into_iter().chain(velocity).map(f32::to_bits).collect();
        if u(16) != 0 {
            for (i, word) in actual.iter().enumerate() {
                if *word != u(20 + 4 * i) {
                    return Err(format!(
                        "sample mismatch tick={} lane={i} actual={word:08x} expected={:08x}",
                        index + 1,
                        u(20 + 4 * i)
                    )
                    .into());
                }
            }
            checked += 1;
        }
        let b = &runtime.state.car.owner.vehicle;
        let m = grid_world::vehicle_handling::MUTABLE;
        output.push_str(&format!(
            "T\t{}\t{}\t{}",
            runtime.ticks,
            b[(m + 0x444) / 4],
            (b[(m + 0x448) / 4] >> (8 * ((m + 0x448) % 4))) & 255
        ));
        for word in actual
            .into_iter()
            .chain((0..8).map(|i| b[(m + 0x3a0) / 4 + i]))
            .chain((0..4).map(|i| b[(m + 0x18) / 4 + i]))
            .chain([b[0x226c / 4], b[0xe34 / 4], b[0xe20 / 4], b[0xe1c / 4]])
        {
            output.push_str(&format!("\t{word:08x}"));
        }
        output.push('\n');
    }
    let target = Path::new(&a[5]);
    if !target
        .parent()
        .ok_or("missing output parent")?
        .canonicalize()?
        .starts_with(Path::new("local").canonicalize()?)
    {
        return Err("private output required".into());
    }
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)?
        .write_all(output.as_bytes())?;
    println!("drive_replay ticks={n} sampled_pose_velocity_exact={checked} completed=1");
    Ok(())
}
fn main() {
    // The unoptimized owned frame uses large staged stack values. Match the
    // existing source-driving tests' explicit stack rather than Windows' 1 MiB.
    let result = std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| run().map_err(|e| e.to_string()))
        .expect("replay thread")
        .join()
        .expect("replay panic");
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1)
    }
}
