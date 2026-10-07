//! Private contact/material comparison with supplied query-result fixtures.
use grid_world::{contact_material, contact_ray};
use std::{fs::OpenOptions, io::Write, path::Path};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-contact-material INPUT.tsv local/OUTPUT.tsv".into());
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
            || cells.len() != 167
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
        let matrix = |offset: usize| {
            std::array::from_fn(|i| std::array::from_fn(|j| words[offset + i * 4 + j]))
        };
        let vec4 = |offset: usize| {
            [
                words[offset],
                words[offset + 1],
                words[offset + 2],
                words[offset + 3],
            ]
        };
        let basis = matrix(0);
        let rays =
            contact_ray::prepare(basis, matrix(16), matrix(32), vec4(48), vec4(52), vec4(56))?;
        let axes = matrix(60);
        let normals = matrix(137);
        let material = contact_material::Material {
            time_based: words[124] != 0.,
            wavelength: words[125],
            magnitude: words[126],
            real_proportion: words[127],
            car_proportion: words[128],
            height_offset: words[129],
        };
        report += &format!("P\t{}", cells[0]);
        for w in 0..4 {
            let result = contact_material::resolve(contact_material::Input {
                ray: rays[w],
                body_y: basis[1],
                suspension_axis: axes[w],
                query_normal: normals[w],
                fraction: words[133 + w],
                hit_present: words[153 + w] != 0.,
                material,
                wheel_ordinal: w,
                speed: words[130],
                clock: words[131],
                car_factor: words[132],
                calculate_bump: words[157] != 0.,
                previous_bump: words[158 + w],
                new_bump_before: words[162 + w],
                wheel_angle: words[76 + w],
                wheel_radius: words[48 + w],
                radius_samples: std::array::from_fn(|i| {
                    [words[80 + w * 10 + i * 2], words[81 + w * 10 + i * 2]]
                }),
                wheel_height_offset: words[120 + w],
            })?;
            for value in [result.floor, result.new_bump, words[158 + w]]
                .into_iter()
                .chain(result.wheel_normal)
                .chain(result.solver_normal)
            {
                report += &format!("\t{:08x}", value.to_bits());
            }
            for _ in 0..3 {
                report += &format!("\t{:08x}", u32::from(result.accepted));
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
    println!("contact_material_cases={count}");
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("grid-contact-material: {error}");
        std::process::exit(1);
    }
}
