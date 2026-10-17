//! Private controlled numeric diagnostic, not a driving simulation.
use grid_world::dynamics::{self, BodyState};
use std::{fs::OpenOptions, io::Write, path::Path};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-dynamics-contribution INPUT.tsv local/OUTPUT.tsv".into());
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
        if cells.len() != 23
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
        let delta = words[0];
        let schedule = dynamics::substeps(delta)?;
        let result = dynamics::apply_solver_contribution(
            BodyState {
                linear: words[1..5].try_into()?,
                angular: words[5..9].try_into()?,
                position: words[9..13].try_into()?,
            },
            delta,
            words[13..17].try_into()?,
            words[17..21].try_into()?,
        )?;
        if !words[21].is_finite() || !(words[21] + delta).is_finite() {
            return Err("invalid clock".into());
        }
        let mut weight = 1_f32;
        let mut last_weight = weight;
        for _ in 0..schedule.count {
            last_weight = weight;
            weight -= schedule.weight_decrement;
        }
        report += &format!(
            "S\t{}\t{}\t{:08x}\t{:08x}\t{:08x}\t{:08x}\n",
            cells[0],
            schedule.count,
            schedule.delta.to_bits(),
            1_f32.to_bits(),
            last_weight.to_bits(),
            (words[21] + delta).to_bits()
        );
        report += &format!("P\t{}", cells[0]);
        for value in result
            .linear
            .into_iter()
            .chain(result.angular)
            .chain(result.position)
        {
            report += &format!("\t{:08x}", value.to_bits());
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
    println!("conditional_dynamics_cases={count}");
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("grid-dynamics-contribution: {error}");
        std::process::exit(1);
    }
}
