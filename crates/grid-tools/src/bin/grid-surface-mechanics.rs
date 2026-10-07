//! Private surface mechanics descriptor report.
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
};
fn hex(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let force = args.get(2).is_some_and(|v| v == "force");
    if args.len() != 2 && !(args.len() == 3 && force) {
        return Err("usage: grid-surface-mechanics INPUT local/OUTPUT.tsv [force]".into());
    }
    let out = Path::new(&args[1]);
    let parent = out.parent().ok_or("output parent absent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("output must remain private".into());
    }
    let mut bytes = Vec::new();
    File::open(&args[0])?
        .take(256 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 256 * 1024 * 1024 {
        return Err("resource exceeds limit".into());
    }
    let doc = grid_formats::binxml::read(&bytes)?;
    let mut writer = std::io::BufWriter::new(
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(parent.join(out.file_name().ok_or("output name absent")?))?,
    );
    let materials = grid_formats::surface::read(&doc)?;
    for material in &materials {
        write!(
            writer,
            "M\t{:08x}\t{:08x}",
            u32::from_le_bytes(material.code),
            u32::from(material.mechanics.is_some())
        )?;
        let (words, name) = if let Some(m) = &material.mechanics {
            (
                [
                    u32::from(m.time_based),
                    m.wavelength.to_bits(),
                    m.magnitude.to_bits(),
                    m.real_proportion.to_bits(),
                    m.car_proportion.to_bits(),
                    m.depth.to_bits(),
                    m.grip.to_bits(),
                    m.hardness.to_bits(),
                    u32::from_le_bytes(m.tyre_table),
                ],
                m.name,
            )
        } else {
            ([0; 9], "")
        };
        for word in words {
            write!(writer, "\t{word:08x}")?;
        }
        if force {
            let extra = if let Some(m) = &material.mechanics {
                [
                    m.grip_per_ms.to_bits(),
                    m.ms_for_max_effect.to_bits(),
                    m.camera_factor.to_bits(),
                    m.roughness.to_bits(),
                    m.minimum_speed_for_slowdown.to_bits(),
                    m.speed_for_maximum_slowdown.to_bits(),
                    m.slowdown.to_bits(),
                    m.roll_induce.to_bits(),
                    m.heat.to_bits(),
                    m.wear.to_bits(),
                    u32::from(m.curved),
                    u32::from(m.dirt_build_up),
                    u32::from(m.high_downforce),
                ]
            } else {
                [0; 13]
            };
            for word in extra {
                write!(writer, "\t{word:08x}")?;
            }
        }
        writeln!(writer, "\t{}", hex(name))?;
    }
    writer.flush()?;
    println!(
        "surface_materials={} mechanics={}",
        materials.len(),
        materials.iter().filter(|m| m.mechanics.is_some()).count()
    );
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-surface-mechanics: {e}");
        std::process::exit(1);
    }
}
