//! Private conditional substep report; prepared inputs only.
use grid_world::{
    suspension,
    wheel_step::{self, Wheel},
};
use std::{fs::OpenOptions, io::Write, path::Path};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-wheel-step INPUT.tsv local/OUTPUT.tsv".into());
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
    let mut count = 0;
    let mut report = String::new();
    for line in std::fs::read_to_string(input)?.lines() {
        count += 1;
        let cells: Vec<_> = line.split_whitespace().collect();
        if count > 256
            || cells.len() != 140
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
        let mut wheels = Vec::new();
        for i in 0..4 {
            let v = &words[35 + i * 26..61 + i * 26];
            if v[23] != 0. && v[23] != 1. {
                return Err("invalid contact flag".into());
            }
            wheels.push(Wheel {
                suspension: suspension::Wheel {
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
                },
                axis: v[11..15].try_into()?,
                body_speed: v[15],
                body_rate: v[16],
                floor: v[17],
                contact_speed: v[18],
                raw: v[19],
                weighted: v[20],
                contact_raw: v[21],
                contact_weighted: v[22],
                contact: v[23] == 1.,
                lower: v[24],
                upper: v[25],
            });
        }
        let response = std::array::from_fn(|i| {
            words[3 + i * 4..7 + i * 4]
                .try_into()
                .expect("four-word row")
        });
        let secondary = std::array::from_fn(|i| {
            words[19 + i * 4..23 + i * 4]
                .try_into()
                .expect("four-word row")
        });
        let result = wheel_step::step(
            wheels.try_into().expect("four wheels"),
            response,
            secondary,
            words[2],
            words[0],
            words[1],
        )?;
        report += &format!("P\t{}", cells[0]);
        for w in result {
            for v in [
                w.suspension.displacement,
                w.suspension.speed,
                w.body_speed,
                w.floor,
                w.contact_speed,
                w.raw,
                w.weighted,
                w.contact_raw,
                w.contact_weighted,
                if w.contact { 1. } else { 0. },
            ] {
                report += &format!("\t{:08x}", v.to_bits());
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
    println!("wheel_step_cases={count}");
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("grid-wheel-step: {error}");
        std::process::exit(1);
    }
}
