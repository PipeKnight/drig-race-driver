//! Private Standard static-contact-ray comparison with supplied gravity and geometry.
use grid_world::contact_ray;
use std::{fs::OpenOptions, io::Write, path::Path};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-contact-ray INPUT.tsv local/OUTPUT.tsv".into());
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
            || !(cells.len() == 61 || cells.len() == 77)
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
        let result = contact_ray::prepare(
            matrix(0),
            matrix(16),
            matrix(32),
            words[48..52].try_into()?,
            words[52..56].try_into()?,
            words[56..60].try_into()?,
        )?;
        report += &format!("P\t{}", cells[0]);
        for (index, ray) in result.into_iter().enumerate() {
            let axis = if words.len() == 76 {
                matrix(60)[index]
            } else {
                matrix(0)[1]
            };
            let normal = contact_ray::regularize(matrix(0)[1], axis)?;
            for value in ray
                .start
                .into_iter()
                .chain(ray.end)
                .chain([-1000.])
                .chain(normal)
            {
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
    println!("contact-ray_cases={count}");
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("grid-contact-ray: {error}");
        std::process::exit(1);
    }
}
