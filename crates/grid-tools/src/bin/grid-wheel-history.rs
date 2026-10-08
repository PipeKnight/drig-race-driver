//! Private supplied production wheel-history comparison, wheel angle zero.
use grid_world::wheel_history::{self, Input, Material, Vehicle, Wheel};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-wheel-history INPUT.tsv local/OUTPUT.tsv".into());
    }
    let text = fs::read_to_string(&args[0])?;
    if text.len() > 4 * 1024 * 1024 {
        return Err("input exceeds limit".into());
    }
    let mut results = Vec::new();
    for line in text.lines() {
        let cells: Vec<_> = line.split('\t').collect();
        if cells.len() != 57 || results.len() >= 4096 {
            return Err("invalid input row".into());
        }
        let v: Vec<_> = cells[1..]
            .iter()
            .map(|v| u32::from_str_radix(v, 16).map(f32::from_bits))
            .collect::<Result<_, _>>()?;
        if v.iter().any(|v| !v.is_finite())
            || [10, 11, 12, 13, 18, 30, 31]
                .iter()
                .any(|i| v[*i] != 0. && v[*i] != 1.)
            || ![0., 1., 2.].contains(&v[35])
        {
            return Err("invalid flags or values".into());
        }
        let input = Input {
            delta: v[0],
            scalar_argument_10: v[1],
            vehicle: Vehicle {
                speed: v[2],
                mass: v[3],
                scalar_1f3c: v[4],
                scalar_1ffc: v[5],
                scalar_1efc: v[6],
                scalar_e20: v[7],
                scalar_187c: v[8],
                minimum_alignment: v[9],
                flag_b11: v[10] != 0.,
                flag_e38: v[11] != 0.,
                direction: v[52..56].try_into()?,
            },
            material: if v[18] != 0. {
                Some(Material {
                    wear: v[14],
                    speed_grip: v[15..18].try_into()?,
                    flag_70: v[12] != 0.,
                    dirt_build_up: v[13] != 0.,
                    token: 1,
                })
            } else {
                None
            },
            wheel: Wheel {
                air_clock: v[19],
                return_clock: v[20],
                scalar_6c: v[21],
                grip_scale: v[22],
                spin: v[23],
                radius: v[24],
                scalar_138: v[25],
                scalar_e4: v[26],
                pair_b0_b4: [v[27], v[28]],
                scalar_10c: v[29],
                flag_fc: v[30] != 0.,
                flag_fd: v[31] != 0.,
                history: v[32..35].try_into()?,
                history_material: v[35] as u32,
            },
            velocity: v[36..40].try_into()?,
            lateral_axis: v[40..44].try_into()?,
            longitudinal_axis: v[44..48].try_into()?,
            normal: v[48..52].try_into()?,
        };
        let r = wheel_history::calculate(input)?;
        let w = r.wheel;
        let words = [
            w.air_clock.to_bits(),
            w.return_clock.to_bits(),
            w.scalar_6c.to_bits(),
            u32::from(w.flag_fd),
            w.history[0].to_bits(),
            w.history[1].to_bits(),
            w.history[2].to_bits(),
            w.history_material,
            r.grip.to_bits(),
        ];
        results.push(format!(
            "P\t{}{}",
            cells[0],
            words
                .iter()
                .map(|w| format!("\t{w:08x}"))
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
    println!("wheel_history_cases={}", results.len());
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-wheel-history: {e}");
        std::process::exit(1);
    }
}
