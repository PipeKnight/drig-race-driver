//! Private whole unassisted coefficient update diagnostic.
use grid_world::{
    steering_coefficients::{self, Input, Wheel},
    steering_geometry, steering_input,
};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
fn ordinal(v: f32) -> Result<usize, &'static str> {
    match v {
        0. => Ok(0),
        1. => Ok(1),
        2. => Ok(2),
        3. => Ok(3),
        _ => Err("invalid wheel ordinal"),
    }
}
fn flag(v: f32) -> Result<bool, &'static str> {
    match v {
        0. => Ok(false),
        1. => Ok(true),
        _ => Err("invalid flag"),
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let source_mode = args.len() == 4 && args[2] == "--geometry";
    let geometry_mode = (args.len() == 3 && args[2] == "--geometry") || source_mode;
    if args.len() != 2 && !geometry_mode {
        return Err(
            "usage: grid-steering-coefficients INPUT.tsv local/OUTPUT.tsv [--geometry [CAR.csv]]"
                .into(),
        );
    }
    let source_angle = if source_mode {
        let bytes = fs::read(&args[3])?;
        let table = grid_formats::tuning::read(&bytes)?;
        if table.rows.len() != 1 {
            return Err("source mode requires one Standard row".into());
        }
        Some(grid_formats::vehicle::read_standard_steering_angle(
            &table.rows[0],
        )?)
    } else {
        None
    };
    let text = fs::read_to_string(&args[0])?;
    if text.len() > 4 * 1024 * 1024 {
        return Err("input exceeds limit".into());
    }
    let mut results = Vec::new();
    for line in text.lines() {
        let cells: Vec<_> = line.split('\t').collect();
        if cells.len() != if geometry_mode { 70 } else { 54 } || results.len() >= 4096 {
            return Err("invalid input row".into());
        }
        let v: Vec<_> = cells[1..]
            .iter()
            .map(|v| u32::from_str_radix(v, 16).map(f32::from_bits))
            .collect::<Result<_, _>>()?;
        let mut wheels = Vec::new();
        for q in v[13..53].as_chunks::<10>().0 {
            wheels.push(Wheel {
                x: q[0],
                toe_e8: q[1],
                rotation_offset: q[2],
                frozen: flag(q[3])?,
                blend_20: q[4],
                blend_24: q[5],
                smooth_28: q[6],
                smooth_2c: q[7],
                limit_f0: q[8],
                rotation_144: q[9],
            });
        }
        let mut input = Input {
            command: steering_input::Input {
                command: v[0],
                perturbation: v[1],
                speed_scalar: v[2],
                phase_scalar: v[3],
            },
            scale_e54: v[4],
            span_21d4: v[5],
            span_21d8: v[6],
            cap_2298: v[7],
            cap_229c: v[8],
            mode_1f74_is_one: flag(v[9])?,
            delta: v[10],
            pair: [ordinal(v[11])?, ordinal(v[12])?],
            wheels: wheels.try_into().map_err(|_| "four wheels required")?,
        };
        let mut geometry_words = String::new();
        if geometry_mode {
            let points = std::array::from_fn(|i| v[53 + i * 4..57 + i * 4].try_into().unwrap());
            let g = steering_geometry::initialize(points, source_angle.unwrap_or(v[4]))?;
            input.scale_e54 = g.scale_e54;
            input.span_21d4 = g.span_21d4;
            input.span_21d8 = g.span_21d8;
            input.pair = g.pair;
            for (w, p) in input.wheels.iter_mut().zip(points) {
                w.x = p[0];
            }
            geometry_words = format!(
                "\t{:08x}\t{}\t{}\t{:08x}\t{:08x}",
                g.scale_e54.to_bits(),
                g.pair[0],
                g.pair[1],
                g.span_21d4.to_bits(),
                g.span_21d8.to_bits()
            );
        }
        let output = steering_coefficients::unassisted(input)?;
        let words = output
            .iter()
            .flat_map(|s| {
                [
                    s.blend_20,
                    s.blend_24,
                    s.smooth_28,
                    s.smooth_2c,
                    s.history_38,
                ]
            })
            .map(|v| format!("\t{:08x}", v.to_bits()))
            .collect::<String>();
        results.push(format!("P\t{}{geometry_words}{words}", cells[0]));
    }
    let out = Path::new(&args[1]);
    let parent = out.parent().ok_or("missing parent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("private output required".into());
    }
    let mut writer = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(parent.join(out.file_name().ok_or("missing name")?))?;
    for row in &results {
        writeln!(writer, "{row}")?;
    }
    writer.flush()?;
    println!("steering_coefficient_cases={}", results.len());
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-steering-coefficients: {e}");
        std::process::exit(1);
    }
}
