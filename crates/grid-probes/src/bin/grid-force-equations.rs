//! Private joined equation lifecycle comparison; no proprietary constants or assets.
use grid_world::force_equations::System;
use std::{fs::OpenOptions, io::Write, path::Path};

fn next(words: &[u32], at: &mut usize) -> Result<u32, Box<dyn std::error::Error>> {
    let value = *words.get(*at).ok_or("truncated operation")?;
    *at += 1;
    Ok(value)
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-force-equations INPUT.tsv local/OUTPUT.tsv".into());
    }
    let input = Path::new(&args[0]);
    if input.metadata()?.len() > 8 * 1024 * 1024 {
        return Err("input exceeds limit".into());
    }
    let output = Path::new(&args[1]);
    if !output
        .parent()
        .ok_or("missing parent")?
        .canonicalize()?
        .starts_with(Path::new("local").canonicalize()?)
    {
        return Err("private output only".into());
    }
    let mut report = String::new();
    let mut cases = 0;
    let mut operations = 0;
    for line in std::fs::read_to_string(input)?.lines() {
        cases += 1;
        let cells: Vec<_> = line.split_whitespace().collect();
        if cases > 1024
            || cells.len() < 3
            || cells.len() > 8192
            || cells[0].is_empty()
            || cells[0].len() > 63
            || !cells[0]
                .bytes()
                .all(|v| v.is_ascii_alphanumeric() || v == b'_')
        {
            return Err("invalid framing".into());
        }
        let words: Vec<_> = cells[1..]
            .iter()
            .map(|v| {
                if v.len() != 8 || !v.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err("invalid word");
                }
                u32::from_str_radix(v, 16).map_err(|_| "invalid word")
            })
            .collect::<Result<_, _>>()?;
        let mut at = 0;
        let mut system = System::new(next(&words, &mut at)? as usize)?;
        let count = next(&words, &mut at)?;
        if !(1..=128).contains(&count) {
            return Err("invalid operation count".into());
        }
        for operation in 0..count {
            if operations == 20_000 {
                return Err("total operation limit exceeded".into());
            }
            let success = match next(&words, &mut at)? {
                0 => {
                    let count = next(&words, &mut at)?;
                    if count > 8 {
                        return Err("too many terms".into());
                    }
                    let mut terms = Vec::new();
                    for _ in 0..count {
                        terms.push((
                            next(&words, &mut at)? as usize,
                            f32::from_bits(next(&words, &mut at)?),
                        ));
                    }
                    system.add(&terms, f32::from_bits(next(&words, &mut at)?))?;
                    true
                }
                1 => {
                    system.fix(
                        next(&words, &mut at)? as usize,
                        f32::from_bits(next(&words, &mut at)?),
                    )?;
                    true
                }
                2 => {
                    let count = next(&words, &mut at)?;
                    if !(1..=40).contains(&count) {
                        return Err("invalid selection size".into());
                    }
                    let mut rows = Vec::new();
                    let mut variables = Vec::new();
                    for _ in 0..count {
                        rows.push(next(&words, &mut at)? as usize);
                    }
                    for _ in 0..count {
                        variables.push(next(&words, &mut at)? as usize);
                    }
                    system.select(&rows, &variables)?
                }
                3 => system.remaining()?,
                _ => return Err("unknown operation".into()),
            };
            report += &format!("P\t{}\t{operation}\t{:08x}", cells[0], u32::from(success));
            for word in system.snapshot() {
                report += &format!("\t{word:08x}");
            }
            report.push('\n');
            operations += 1;
        }
        if at != words.len() {
            return Err("trailing words".into());
        }
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    file.write_all(report.as_bytes())?;
    println!("force_equation_cases={cases} operations={operations}");
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
