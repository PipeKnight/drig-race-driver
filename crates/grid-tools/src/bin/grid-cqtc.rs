//! Read-only source decoder projection; never consumes original probe answers.
use std::{fs, io::Write, path::Path};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 || fs::metadata(&args[0])?.len() > 2 * 1024 * 1024 {
        return Err("usage: grid-cqtc SOURCE_CQTC local/OUTPUT".into());
    }
    let bytes = fs::read(&args[0])?;
    let c = grid_formats::cqtc::read(&bytes)?;
    let output = Path::new(&args[1]);
    if !output.starts_with("local") {
        return Err("output must be private local/".into());
    }
    let mut data = Vec::new();
    for p in &c.positions {
        for x in [p[0], p[1], p[2], 1.] {
            data.extend(x.to_le_bytes());
        }
    }
    for t in &c.triangles {
        for i in t.indices {
            data.extend(u32::from(i).to_le_bytes());
        }
        data.extend(c.materials[t.material as usize]);
    }
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?
        .write_all(&data)?;
    println!(
        "cqtc vertices={} triangles={} nodes={} lists={} references={}",
        c.positions.len(),
        c.triangles.len(),
        c.nodes.len(),
        c.references.len(),
        c.references.values().map(Vec::len).sum::<usize>()
    );
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
