//! Private supplied-grid whole wheel geometry diagnostic.
use grid_world::{
    deformation_grid::Grid,
    wheel_deformation::{self, Input, Output, Wheel},
};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
fn flag(v: f32) -> Result<bool, Box<dyn std::error::Error>> {
    match v {
        0. => Ok(false),
        1. => Ok(true),
        _ => Err("invalid flag".into()),
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-wheel-deformation INPUT.tsv local/OUTPUT.tsv".into());
    }
    let text = fs::read_to_string(&args[0])?;
    if text.len() > 16 * 1024 * 1024 {
        return Err("input exceeds limit".into());
    }
    let mut results = Vec::new();
    for line in text.lines() {
        let cells: Vec<_> = line.split('\t').collect();
        if cells.len() < 174 || results.len() >= 1024 || cells[0].is_empty() || cells[0].len() > 63
        {
            return Err("invalid row".into());
        }
        let v: Vec<_> = cells[1..]
            .iter()
            .map(|s| u32::from_str_radix(s, 16).map(f32::from_bits))
            .collect::<Result<_, _>>()?;
        if v.iter().any(|x| !x.is_finite())
            || v[..3]
                .iter()
                .any(|d| !(2. ..=16.).contains(d) || d.fract() != 0.)
        {
            return Err("invalid grid input".into());
        }
        let dimensions = std::array::from_fn(|i| v[i] as usize);
        let count = dimensions.iter().product::<usize>();
        if count > 400 || v.len() != 173 + count * 4 {
            return Err("invalid node count".into());
        }
        let mut wheels = [Wheel {
            position: [0.; 4],
            current_radius: 1.,
            normal_radius: 1.,
            deflated_radius: 1.,
            deflated: false,
        }; 4];
        for (i, w) in wheels.iter_mut().enumerate() {
            let start = 17 + i * 8;
            *w = Wheel {
                position: v[start..start + 4].try_into()?,
                current_radius: v[start + 4],
                normal_radius: v[start + 5],
                deflated_radius: v[start + 6],
                deflated: flag(v[start + 7])?,
            };
        }
        let prior = Output {
            displacement: std::array::from_fn(|i| v[49 + i * 4..53 + i * 4].try_into().unwrap()),
            angles: v[65..69].try_into()?,
            orientation: std::array::from_fn(|i| {
                std::array::from_fn(|j| {
                    v[69 + i * 16 + j * 4..73 + i * 16 + j * 4]
                        .try_into()
                        .unwrap()
                })
            }),
            radius_samples: std::array::from_fn(|i| {
                std::array::from_fn(|j| {
                    v[133 + i * 10 + j * 2..135 + i * 10 + j * 2]
                        .try_into()
                        .unwrap()
                })
            }),
            deflated: wheels.map(|w| w.deflated),
        };
        let out = wheel_deformation::calculate(Input {
            grid: Grid {
                dimensions,
                spacing: v[3],
                minimum: v[4..8].try_into()?,
                positions: v[173..].as_chunks::<4>().0,
            },
            center: v[8..12].try_into()?,
            scale: v[12],
            angular_limit: v[13],
            radius_thresholds: v[14..16].try_into()?,
            active: flag(v[16])?,
            wheels,
            previous: prior,
        })?;
        let words = out
            .displacement
            .into_iter()
            .flatten()
            .chain(out.angles)
            .chain(out.orientation.into_iter().flatten().flatten())
            .chain(out.radius_samples.into_iter().flatten().flatten())
            .map(|v| format!("\t{:08x}", v.to_bits()))
            .collect::<String>();
        let flags = out
            .deflated
            .iter()
            .map(|v| format!("\t{}", u8::from(*v)))
            .collect::<String>();
        results.push(format!("P\t{}{words}{flags}", cells[0]));
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
    println!("wheel_deformation_cases={}", results.len());
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-wheel-deformation: {e}");
        std::process::exit(1);
    }
}
