//! Private conditional response comparison, supplied points and axes only.
use grid_world::{
    dynamics::{BodyState, ImpulseBody},
    wheel_coupling::{self, Wheel},
};
use std::{fs::OpenOptions, io::Write, path::Path};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-wheel-coupling INPUT.tsv local/OUTPUT.tsv".into());
    }
    let input = Path::new(&args[0]);
    if input.metadata()?.len() > 1024 * 1024 {
        return Err("input exceeds limit".into());
    }
    let output = Path::new(&args[1]);
    let parent = output
        .parent()
        .ok_or("missing output parent")?
        .canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("private output only".into());
    }
    let mut report = String::new();
    let mut count = 0;
    for line in std::fs::read_to_string(input)?.lines() {
        count += 1;
        let cells: Vec<_> = line.split_whitespace().collect();
        if count > 256
            || cells.len() != 77
            || cells[0].len() > 63
            || !cells[0]
                .bytes()
                .all(|v| v.is_ascii_alphanumeric() || v == b'_')
        {
            return Err("invalid case framing".into());
        }
        let words: Vec<f32> = cells[1..]
            .iter()
            .map(|v| {
                if v.len() != 8 || !v.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err("invalid float word");
                }
                Ok(f32::from_bits(
                    u32::from_str_radix(v, 16).map_err(|_| "invalid float word")?,
                ))
            })
            .collect::<Result<_, _>>()?;
        let body = ImpulseBody {
            mass: words[0],
            inertia: words[1..4].try_into()?,
            basis: std::array::from_fn(|i| {
                words[4 + i * 4..8 + i * 4]
                    .try_into()
                    .expect("four-word row")
            }),
        };
        let state = BodyState {
            linear: words[20..24].try_into()?,
            angular: words[24..28].try_into()?,
            position: body.basis[3],
        };
        let wheels = std::array::from_fn(|i| {
            let offset = 28 + i * 12;
            Wheel {
                point: words[offset..offset + 4]
                    .try_into()
                    .expect("four-word point"),
                axis: words[offset + 4..offset + 8]
                    .try_into()
                    .expect("four-word axis"),
                projection_axis: words[offset + 8..offset + 12]
                    .try_into()
                    .expect("four-word projection"),
            }
        });
        let result = wheel_coupling::prepare(body, state, wheels)?;
        report += &format!("P\t{}", cells[0]);
        for value in result
            .speeds
            .into_iter()
            .chain(result.response.into_iter().flatten())
            .chain(result.secondary.into_iter().flatten())
        {
            report += &format!("\t{:08x}", value.to_bits());
        }
        report.push('\n');
    }
    if count == 0 {
        return Err("no cases".into());
    }
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(parent.join(output.file_name().ok_or("missing output name")?))?
        .write_all(report.as_bytes())?;
    println!("wheel_coupling_cases={count}");
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("grid-wheel-coupling: {error}");
        std::process::exit(1);
    }
}
