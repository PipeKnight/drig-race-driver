//! Private Standard static-setup comparison with supplied gravity and geometry.
use grid_world::setup;
use std::{fs::OpenOptions, io::Write, path::Path};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-setup INPUT.tsv local/OUTPUT.tsv".into());
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
            || !(112..=113).contains(&cells.len())
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
        let mut result = setup::prepare(setup::Input {
            upper: words[0..30].try_into()?,
            lower: words[30..60].try_into()?,
            sliders: words[60..90].try_into()?,
            base_damper: words[90..92].try_into()?,
            base_threshold: words[92..94].try_into()?,
            base_high_ratio: words[94..96].try_into()?,
            height: words[96],
            denominator: words[97..101].try_into()?,
            local_y: words[101..105].try_into()?,
            local_z: words[105..109].try_into()?,
            mass: words[109],
            gravity: words[110],
        })?;
        if let Some(dimension) = words.get(111) {
            let capped = setup::cap_denominators(result.wheels.map(|w| w.denominator), *dimension)?;
            for (wheel, denominator) in result.wheels.iter_mut().zip(capped) {
                wheel.denominator = denominator;
            }
        }
        report += &format!("P\t{}\t{:08x}", cells[0], result.height.to_bits());
        for w in result.wheels {
            for value in [
                w.spring,
                w.damper,
                w.threshold,
                w.high_ratio,
                w.negative_scale,
                w.coupling,
                w.denominator,
                w.local_y,
                w.preload,
            ] {
                report += &format!("\t{:08x}", value.to_bits());
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
    println!("setup_cases={count}");
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("grid-setup: {error}");
        std::process::exit(1);
    }
}
