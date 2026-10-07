//! Private Standard static-wheel-frame comparison with supplied gravity and geometry.
use grid_world::wheel_frame;
use std::{fs::OpenOptions, io::Write, path::Path};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let tyre = args.get(2).is_some_and(|v| v == "tyre");
    if args.len() != 2 && !(args.len() == 3 && tyre) {
        return Err("usage: grid-wheel-frame INPUT.tsv local/OUTPUT.tsv [tyre]".into());
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
            || cells.len() != if tyre { 61 } else { 53 }
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
        let matrix =
            |offset| std::array::from_fn(|i| std::array::from_fn(|j| words[offset + i * 4 + j]));
        let result =
            wheel_frame::prepare(matrix(0), matrix(16), matrix(32), words[48..52].try_into()?)?;
        let tyre_axes = if tyre {
            Some(wheel_frame::tyre_axes(
                matrix(0),
                std::array::from_fn(|w| [words[52 + w * 2], words[53 + w * 2]]),
            )?)
        } else {
            None
        };
        report += &format!("P\t{}", cells[0]);
        for (w, frame) in result.into_iter().enumerate() {
            for value in frame
                .point
                .into_iter()
                .chain(frame.anchor)
                .chain(frame.axis)
            {
                report += &format!("\t{:08x}", value.to_bits());
            }
            if let Some(axes) = tyre_axes {
                for value in axes[w].into_iter().flatten() {
                    report += &format!("\t{:08x}", value.to_bits());
                }
            }
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
    println!("wheel-frame_cases={count}");
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("grid-wheel-frame: {error}");
        std::process::exit(1);
    }
}
