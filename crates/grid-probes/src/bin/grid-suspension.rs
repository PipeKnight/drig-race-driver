//! Supplied-coefficient four-wheel suspension diagnostic; not a whole solver.
use grid_world::suspension::{self, Wheel};
use std::{fs::OpenOptions, io::Write, path::Path};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-suspension INPUT.tsv local/OUTPUT.tsv".into());
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
        return Err("report must remain private".into());
    }
    let text = std::fs::read_to_string(input)?;
    let mut report = String::new();
    let mut count = 0;
    for line in text.lines() {
        count += 1;
        if count > 256 {
            return Err("case limit exceeded".into());
        }
        let cells: Vec<_> = line.split_whitespace().collect();
        if cells.len() != 48
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
        let wheels: [Wheel; 4] = std::array::from_fn(|i| {
            let v = &words[3 + i * 11..14 + i * 11];
            Wheel {
                displacement: v[0],
                speed: v[1],
                spring: v[2],
                preload: v[3],
                travel_denominator: v[4],
                damper: v[5],
                speed_threshold: v[6],
                high_speed_ratio: v[7],
                negative_speed_scale: v[8],
                bump: v[9],
                axle_coupling: v[10],
            }
        });
        if !words[1].is_finite() {
            return Err("nonfinite weight".into());
        }
        let response = suspension::impulses(wheels, words[2], words[0])?;
        report += &format!("P\t{}", cells[0]);
        for value in response {
            report += &format!(
                "\t{:08x}\t{:08x}",
                (value + 0.).to_bits(),
                (value * words[1] + 0.).to_bits()
            );
        }
        report.push('\n');
    }
    if count == 0 {
        return Err("no cases".into());
    }
    let mut writer = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(parent.join(output.file_name().ok_or("missing output name")?))?;
    writer.write_all(report.as_bytes())?;
    println!("controlled_suspension_cases={count}");
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("grid-suspension: {error}");
        std::process::exit(1);
    }
}
