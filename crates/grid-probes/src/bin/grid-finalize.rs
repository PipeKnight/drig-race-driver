//! Private four-wheel two-pass finalization comparison, supplied impulses.
use grid_world::{
    dynamics::{BodyState, ImpulseBody},
    finalize::{self, Wheel},
};
use std::{fs::OpenOptions, io::Write, path::Path};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-finalize INPUT.tsv local/OUTPUT.tsv".into());
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
            || cells.len() != 121
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
            let o = 32 + i * 22;
            Wheel {
                point: words[o..o + 4].try_into().expect("point"),
                axis: words[o + 4..o + 8].try_into().expect("axis"),
                projection_axis: words[o + 8..o + 12].try_into().expect("projection"),
                raw: words[o + 12],
                weighted: words[o + 13],
                contact_raw: words[o + 14],
                contact_weighted: words[o + 15],
                displacement: words[o + 16],
                speed: words[o + 17],
                floor: words[o + 18],
                lower: words[o + 19],
                clip_upper: words[o + 20],
                force: words[o + 21],
            }
        });
        let result = finalize::finish(body, state, wheels, words[29..32].try_into()?, words[28])?;
        report += &format!("P\t{}", cells[0]);
        for value in [result.body.mass]
            .into_iter()
            .chain(result.body.inertia)
            .chain(result.body.basis.into_iter().flatten())
            .chain(result.state.linear)
            .chain(result.state.angular)
            .chain(result.wheels.into_iter().flat_map(|w| {
                [w.displacement, w.speed, w.floor_difference, w.force]
                    .into_iter()
                    .chain(w.remaining)
            }))
            .chain([result.clock])
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
    println!("finalizer_cases={count}");
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("grid-finalize: {error}");
        std::process::exit(1);
    }
}
