//! Private supplied400-node settling diagnostic.
use grid_world::deformation_settle::{self, Edge, Input, Node};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

fn integer(value: f32, maximum: usize) -> Result<usize, Box<dyn std::error::Error>> {
    if !value.is_finite() || value < 0. || value > maximum as f32 || value.fract() != 0. {
        return Err("invalid integer field".into());
    }
    Ok(value as usize)
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-deformation-settle INPUT.tsv local/OUTPUT.tsv".into());
    }
    let text = fs::read_to_string(&args[0])?;
    if text.len() > 16 * 1024 * 1024 {
        return Err("input exceeds limit".into());
    }
    let mut results = Vec::new();
    for line in text.lines() {
        let cells: Vec<_> = line.split('\t').collect();
        if cells.len() < 4809 || results.len() >= 1024 || cells[0].is_empty() || cells[0].len() > 63
        {
            return Err("invalid row".into());
        }
        let v: Vec<_> = cells[1..]
            .iter()
            .map(|s| u32::from_str_radix(s, 16).map(f32::from_bits))
            .collect::<Result<_, _>>()?;
        let dimensions = [integer(v[0], 16)?, integer(v[1], 16)?, integer(v[2], 16)?];
        let edge_count = integer(v[7], 1200)?;
        if v.len() != 4808 + edge_count * 3 {
            return Err("invalid edge count".into());
        }
        let nodes: Vec<_> = v[8..4808]
            .as_chunks::<12>()
            .0
            .iter()
            .map(|n| Node {
                position: n[..4].try_into().unwrap(),
                rest: n[4..8].try_into().unwrap(),
                bounds: n[8..].try_into().unwrap(),
            })
            .collect();
        let edges = v[4808..]
            .as_chunks::<3>()
            .0
            .iter()
            .map(|e| {
                Ok(Edge {
                    endpoints: [integer(e[0], 399)?, integer(e[1], 399)?],
                    length: e[2],
                })
            })
            .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;
        let out = deformation_settle::calculate(Input {
            dimensions,
            spacing: v[3],
            mode: integer(v[4], 2)? as u32,
            threshold: v[5],
            gain: v[6],
            nodes: &nodes,
            edges: &edges,
        })?;
        let words = out
            .positions
            .iter()
            .flatten()
            .chain(&out.lengths)
            .map(|v| format!("\t{:08x}", v.to_bits()))
            .collect::<String>();
        results.push(format!("P\t{}{words}", cells[0]));
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
    println!("deformation_settle_cases={}", results.len());
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-deformation-settle: {e}");
        std::process::exit(1);
    }
}
