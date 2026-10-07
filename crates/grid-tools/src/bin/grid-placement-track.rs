//! Private replay of bounded captured placement inputs against Donington geometry.
use grid_formats::{collision, jpk};
use grid_world::{Mesh, Segment, Triangle, spawn, triangle};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
};

fn read(path: &Path, limit: u64) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    File::open(path)?.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err("input exceeds bound".into());
    }
    Ok(bytes)
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() < 3 || args.len() > 4 {
        return Err(
            "usage: grid-placement-track GAME_ROOT INPUT.tsv local/OUTPUT.tsv [f64|kernel]".into(),
        );
    }
    let kernel = match args.get(3).and_then(|v| v.to_str()) {
        None | Some("f64") => false,
        Some("kernel") => true,
        _ => return Err("query mode must be f64 or kernel".into()),
    };
    let output = Path::new(&args[2]);
    let parent = output
        .parent()
        .ok_or("missing output parent")?
        .canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("private output required".into());
    }
    let track = read(
        &Path::new(&args[0]).join("tracks/europe/donington/route_0/track.jpk"),
        256 * 1024 * 1024,
    )?;
    let mut triangles = Vec::new();
    for entry in jpk::inspect(&track)?.entries {
        if !entry.name.ends_with(b".vcqtc")
            || entry.uncompressed_size as usize != entry.stored_data.len()
        {
            return Err("unsupported collision entry".into());
        }
        let chunk = collision::read_vcqtc(&track[entry.stored_data])?;
        if triangles.len() + chunk.triangles.len() > 1_000_000 {
            return Err("triangle bound".into());
        }
        triangles.extend(chunk.triangles.iter().map(|t| Triangle {
            vertices: t.indices.map(|i| chunk.positions[i as usize]),
            material: chunk.materials[t.material as usize],
            flags: t.flags,
        }));
    }
    let mesh = Mesh::new(triangles.clone())?;
    let input = String::from_utf8(read(Path::new(&args[1]), 1024 * 1024)?)?;
    let mut reports = Vec::new();
    let mut cases = 0;
    for line in input.lines() {
        cases += 1;
        if cases > 32 {
            return Err("case bound".into());
        }
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() != 27
            || fields[0].len() > 63
            || !fields[0]
                .bytes()
                .all(|v| v.is_ascii_alphanumeric() || v == b'_')
        {
            return Err("expected case name and 26 hex words".into());
        }
        let values: Vec<f32> = fields[1..]
            .iter()
            .map(|v| {
                if v.len() != 8 {
                    return Err("expected eight-digit hex word".into());
                }
                Ok(f32::from_bits(u32::from_str_radix(v, 16)?))
            })
            .collect::<Result<_, Box<dyn std::error::Error>>>()?;
        let wheels = std::array::from_fn(|i| spawn::Wheel {
            position: values[6 + i * 5..9 + i * 5].try_into().unwrap(),
            radius: values[9 + i * 5],
            vertical_displacement: values[10 + i * 5],
        });
        let mut ordinal = 0;
        let pose = spawn::align_with_normalization(
            values[..3].try_into()?,
            values[3..6].try_into()?,
            wheels,
            spawn::Normalization::ReferenceSse,
            |x, z, upper, lower| {
                let result = mesh.query(Segment {
                    start: [x as f64, upper as f64, z as f64],
                    end: [x as f64, lower as f64, z as f64],
                })?;
                let height = if let Some(hit) = result.hit.as_ref() {
                    let fraction = if kernel {
                        triangle::query(
                            [x, upper, z],
                            [0., lower - upper, 0.],
                            triangles[hit.triangle].vertices,
                        )
                        .ok_or(grid_world::Error(
                            "selected triangle missed under f32 kernel",
                        ))?
                        .fraction
                    } else {
                        hit.fraction as f32
                    };
                    Some(upper - (upper - lower) * fraction)
                } else {
                    None
                };
                reports.push(format!(
                    "Q\t{}\t{ordinal}\t{:08x}\t{:08x}\t{:08x}\t{:08x}\t{}\t{}",
                    fields[0],
                    x.to_bits(),
                    z.to_bits(),
                    upper.to_bits(),
                    lower.to_bits(),
                    height.map_or_else(|| "none".into(), |v| format!("{:08x}", v.to_bits())),
                    result
                        .hit
                        .as_ref()
                        .map_or_else(|| "none".into(), |h| h.triangle.to_string())
                ));
                ordinal += 1;
                Ok(height)
            },
        )?;
        let words: String = pose
            .matrix()
            .into_iter()
            .map(|v| format!("\t{:08x}", v.to_bits()))
            .collect();
        reports.push(format!("P\t{}{words}\t{ordinal}", fields[0]));
    }
    if cases == 0 {
        return Err("empty input".into());
    }
    let mut writer = std::io::BufWriter::new(
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(parent.join(output.file_name().ok_or("output filename absent")?))?,
    );
    writeln!(
        writer,
        "C\tf64_candidate_query\treference_sse\t{}\t{}",
        mesh.triangle_count(),
        if kernel { "kernel" } else { "f64" }
    )?;
    for row in reports {
        writeln!(writer, "{row}")?;
    }
    writer.flush()?;
    println!("captured_input_cases={cases}");
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("grid-placement-track: {error}");
        std::process::exit(1);
    }
}
