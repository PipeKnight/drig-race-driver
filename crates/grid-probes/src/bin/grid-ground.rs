//! Private finite-segment query report over original Donington collision geometry.
use grid_formats::{collision, jpk};
use grid_world::{Mesh, Segment, Triangle};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: grid-ground GAME_ROOT QUERIES.tsv local/OUTPUT.tsv".into());
    }
    let out = Path::new(&args[2]);
    let parent = out.parent().ok_or("output parent absent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("report must remain under local/".into());
    }
    let mut bytes = Vec::new();
    File::open(Path::new(&args[0]).join("tracks/europe/donington/route_0/track.jpk"))?
        .take(256 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 256 * 1024 * 1024 {
        return Err("collision resource exceeds limit".into());
    }
    let archive = jpk::inspect(&bytes)?;
    let mut triangles = Vec::new();
    let mut identities = Vec::new();
    for entry in &archive.entries {
        if !entry.name.ends_with(b".vcqtc")
            || entry.uncompressed_size as usize != entry.stored_data.len()
        {
            return Err("unsupported collision entry".into());
        }
        let name = std::str::from_utf8(entry.name)?;
        if name.contains(['\t', '\r', '\n']) {
            return Err("invalid chunk name".into());
        }
        let chunk = collision::read_vcqtc(&bytes[entry.stored_data.clone()])?;
        if triangles.len() + chunk.triangles.len() > 1_000_000 {
            return Err("collision triangle limit exceeded".into());
        }
        for (i, t) in chunk.triangles.iter().enumerate() {
            triangles.push(Triangle {
                vertices: t.indices.map(|v| chunk.positions[v as usize]),
                material: chunk.materials[t.material as usize],
                flags: t.flags,
            });
            identities.push((name, i));
        }
    }
    let mesh = Mesh::new(triangles)?;
    let mut input = String::new();
    File::open(&args[1])?
        .take(4 * 1024 * 1024 + 1)
        .read_to_string(&mut input)?;
    if input.len() > 4 * 1024 * 1024 {
        return Err("query input exceeds limit".into());
    }
    let mut queries = Vec::new();
    for line in input.lines() {
        let values: Vec<_> = line.split('\t').collect();
        if values.len() != 7 || values[0].is_empty() {
            return Err("query needs name and six endpoints".into());
        }
        let coordinates: Vec<f64> = values[1..]
            .iter()
            .map(|v| v.parse())
            .collect::<Result<_, _>>()?;
        queries.push((
            values[0],
            Segment {
                start: coordinates[..3].try_into()?,
                end: coordinates[3..].try_into()?,
            },
        ));
    }
    let mut writer = std::io::BufWriter::new(
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(parent.join(out.file_name().ok_or("output name absent")?))?,
    );
    writeln!(
        writer,
        "M\t{}\t{}\t{}",
        mesh.triangle_count(),
        mesh.node_count(),
        mesh.skipped_degenerate
    )?;
    let mut hits = 0;
    let mut tested = 0;
    for (name, segment) in &queries {
        let result = mesh.query(*segment)?;
        tested += result.tested_triangles;
        if let Some(hit) = result.hit {
            hits += 1;
            let (chunk, triangle) = identities[hit.triangle];
            write!(
                writer,
                "H\t{name}\t{chunk}\t{triangle}\t{:.17e}",
                hit.fraction
            )?;
            for v in hit
                .position
                .into_iter()
                .chain(hit.normal)
                .chain(hit.barycentric)
            {
                write!(writer, "\t{v:.17e}")?;
            }
            writeln!(
                writer,
                "\t{:02x}{:02x}{:02x}{:02x}\t{}\t{}",
                hit.material[0],
                hit.material[1],
                hit.material[2],
                hit.material[3],
                hit.flags,
                result.tested_triangles
            )?;
        } else {
            writeln!(writer, "N\t{name}\t{}", result.tested_triangles)?;
        }
    }
    writer.flush()?;
    println!(
        "triangles={} nodes={} skipped={} queries={} hits={hits} tested_triangles={tested}",
        mesh.triangle_count(),
        mesh.node_count(),
        mesh.skipped_degenerate,
        queries.len()
    );
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-ground: {e}");
        std::process::exit(1);
    }
}
