//! Read-only typed progress report; private coordinates remain under local/.
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
};
fn bits(v: f32) -> String {
    format!("{:08x}", v.to_bits())
}
fn vector(v: [f32; 3]) -> String {
    v.map(bits).join("\t")
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-progress INPUT local/OUTPUT.tsv".into());
    }
    let path = Path::new(&args[1]);
    let parent = path.parent().ok_or("missing parent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("report must be private".into());
    }
    let mut bytes = Vec::new();
    File::open(&args[0])?
        .take(256 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 256 * 1024 * 1024 {
        return Err("input exceeds size limit".into());
    }
    let doc = grid_formats::binxml::read(&bytes)?;
    let data = grid_formats::progress::read(&doc)?;
    let mut out = std::io::BufWriter::new(
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(parent.join(path.file_name().ok_or("missing filename")?))?,
    );
    writeln!(out, "D\t{}", bits(data.total_distance))?;
    for (i, g) in data.gates.iter().enumerate() {
        writeln!(
            out,
            "G\t{i}\t{}\t{}\t{}",
            bits(g.distance),
            vector(g.left),
            vector(g.right)
        )?;
    }
    for (i, p) in data.points.iter().enumerate() {
        writeln!(out, "P\t{i}\t{}\t{}", bits(p.distance), vector(p.position))?;
    }
    for (i, r) in data.routes.iter().enumerate() {
        for (j, s) in r.splits.iter().enumerate() {
            writeln!(out, "S\t{i}\t{j}\t{:?}\t{}", s.kind, s.gate)?;
        }
    }
    out.flush()?;
    println!(
        "distance={} gates={} points={} routes={}",
        data.total_distance,
        data.gates.len(),
        data.points.len(),
        data.routes.len()
    );
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-progress: {e}");
        std::process::exit(1);
    }
}
