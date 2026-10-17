//! Private one-wheel visual damage comparison diagnostic.
use grid_world::wheel_damage::{self, Input};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 && !(args.len() == 4 && args[2] == "--source") {
        return Err("usage: grid-wheel-damage INPUT.tsv local/OUTPUT.tsv [--source HDM]".into());
    }
    let source = if args.len() == 4 {
        Some(grid_formats::handling_damage::read(&fs::read(&args[3])?)?.visual_damage)
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
        if cells.len() != 20 || results.len() >= 4096 || cells[0].is_empty() || cells[0].len() > 63
        {
            return Err("invalid input row".into());
        }
        let mut v: Vec<_> = cells[1..]
            .iter()
            .map(|v| u32::from_str_radix(v, 16).map(f32::from_bits))
            .collect::<Result<_, _>>()?;
        if v.iter().any(|v| !v.is_finite()) || !(0. ..=3.).contains(&v[4]) || v[4].fract() != 0. {
            return Err("invalid wheel input".into());
        }
        let correction = match v[15] {
            0. => false,
            1. => true,
            _ => return Err("invalid correction flag".into()),
        };
        if let Some(p) = source {
            v[16] = p.angular_limit_280;
            v[17] = p.displacement_limit_284;
        }
        let mut levels: [f32; 4] = v[..4].try_into()?;
        let index = v[4] as usize;
        levels[index] = wheel_damage::calculate(Input {
            previous_level: levels[index],
            angular_deformation_1790: v[5],
            displacement_1650: v[6..10].try_into()?,
            correction_vector_c740: v[10..14].try_into()?,
            correction_scale_eefc: v[14],
            correction_enabled_c7ae: correction,
            angular_limit_280: v[16],
            displacement_limit_284: v[17],
        })?;
        // v[18] is the finite supplied delta, intentionally unconsumed by this helper.
        let parameters = source
            .map(|p| {
                p.scalars()
                    .iter()
                    .map(|v| format!("\t{:08x}", v.to_bits()))
                    .collect::<String>()
            })
            .unwrap_or_default();
        results.push(format!(
            "P\t{}{parameters}{}",
            cells[0],
            levels
                .iter()
                .map(|v| format!("\t{:08x}", v.to_bits()))
                .collect::<String>()
        ));
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
    println!("wheel_damage_cases={}", results.len());
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-wheel-damage: {e}");
        std::process::exit(1);
    }
}
