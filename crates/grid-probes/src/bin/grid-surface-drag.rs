//! Private whole four-wheel surface-drag diagnostic.
use grid_world::{
    dynamics::ImpulseBody,
    surface_drag::{self, Input, Material, Wheel},
};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 && args.len() != 3 {
        return Err("usage: grid-surface-drag INPUT.tsv local/OUTPUT.tsv [SURFACE.xml]".into());
    }
    let surfaces = if args.len() == 3 {
        Some(grid_tools::contact_surfaces::Surfaces::read(&fs::read(
            &args[2],
        )?)?)
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
        if cells.len() != (if surfaces.is_some() { 83 } else { 79 }) || results.len() >= 4096 {
            return Err("invalid input row".into());
        }
        let v: Vec<_> = cells[1..]
            .iter()
            .map(|v| u32::from_str_radix(v, 16).map(f32::from_bits))
            .collect::<Result<_, _>>()?;
        if v[..78].iter().any(|v| !v.is_finite()) {
            return Err("invalid finite inputs".into());
        }
        let mut wheels = Vec::new();
        let mut selected = [u32::MAX; 4];
        for i in 0..4 {
            let q = &v[30 + i * 12..42 + i * 12];
            if [7, 8].iter().any(|i| q[*i] != 0. && q[*i] != 1.) {
                return Err("invalid flags".into());
            }
            let material = if q[7] == 0. {
                None
            } else if let Some(surfaces) = &surfaces {
                let (index, material) = surfaces.drag(v[78 + i].to_bits().to_le_bytes())?;
                selected[i] = u32::try_from(index)?;
                Some(material)
            } else {
                Some(Material {
                    flag_73: q[8] != 0.,
                    minimum_speed: q[9],
                    maximum_speed: q[10],
                    slowdown: q[11],
                })
            };
            wheels.push(Wheel {
                lever: q[..4].try_into()?,
                spin: q[4],
                radius: q[5],
                additive_22b4: q[6],
                material,
            });
        }
        let input = Input {
            delta: v[0],
            speed: v[1],
            body: ImpulseBody {
                mass: v[2],
                inertia: v[3..6].try_into()?,
                basis: std::array::from_fn(|i| v[6 + i * 4..10 + i * 4].try_into().unwrap()),
            },
            linear: v[22..26].try_into()?,
            angular: v[26..30].try_into()?,
            wheels: wheels.try_into().map_err(|_| "four wheels required")?,
        };
        let r = surface_drag::apply(input)?;
        let mut words: Vec<_> = r
            .linear
            .into_iter()
            .chain(r.angular)
            .chain(r.spins)
            .map(f32::to_bits)
            .collect();
        if surfaces.is_some() {
            words.extend(selected);
        }
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
    println!("surface_drag_cases={}", results.len());
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-surface-drag: {e}");
        std::process::exit(1);
    }
}
