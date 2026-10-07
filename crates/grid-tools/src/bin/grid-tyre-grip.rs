//! Private supplied-row tyre lookup and speed grip comparison.
use grid_world::tyre_grip::{self, Key};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let csv = args.get(3).is_some_and(|v| v == "csv");
    if args.len() != 3 && !(args.len() == 4 && csv) {
        return Err("usage: grid-tyre-grip TABLE INPUT.tsv local/OUTPUT.tsv [csv]".into());
    }
    let raw = fs::read(&args[0])?;
    let source = if csv {
        Some(grid_tools::tyre_tables::Tables::read(&raw)?)
    } else {
        None
    };
    if !csv && (raw.len() < 8 || raw.len() > 8 + 64 * 56 || &raw[..4] != b"TYG1") {
        return Err("invalid supplied table".into());
    }
    let count = if csv {
        0
    } else {
        u32::from_le_bytes(raw[4..8].try_into()?) as usize
    };
    if !csv && (count > 64 || raw.len() != 8 + count * 56) {
        return Err("invalid supplied table length".into());
    }
    let fixture = if csv { &[][..] } else { &raw[8..] };
    let keys = fixture[..count * 4].as_chunks::<4>().0.to_vec();
    let values: Vec<f32> = fixture[count * 4..]
        .as_chunks::<4>()
        .0
        .iter()
        .map(|v| f32::from_le_bytes(*v))
        .collect();
    let input = fs::read_to_string(&args[1])?;
    if input.len() > 1024 * 1024 {
        return Err("input exceeds limit".into());
    }
    let mut results = Vec::new();
    for line in input.lines() {
        let cells: Vec<_> = line.split('\t').collect();
        if cells.len() != 9 || results.len() >= 4096 {
            return Err("invalid request".into());
        }
        let words = cells[1..]
            .iter()
            .map(|v| u32::from_str_radix(v, 16))
            .collect::<Result<Vec<_>, _>>()?;
        if words[0] > 1 || words[2] > 1 || words[3] >= 13 {
            return Err("invalid table flags or row".into());
        }
        let row = words[3] as usize;
        let mut key = if words[2] != 0 {
            Key::Code(words[1].to_le_bytes())
        } else {
            Key::Index(words[1] as usize)
        };
        let v: Vec<_> = words[4..].iter().map(|v| f32::from_bits(*v)).collect();
        let grip = if let Some(source) = &source {
            if words[0] == 0 {
                tyre_grip::grip(v[0], v[1], v[2], v[3], 1.)?
            } else {
                source.grip(row, &mut key, [v[0], v[1], v[2]], v[3])?
            }
        } else {
            let factor = tyre_grip::factor(
                &keys,
                &values[row * count..(row + 1) * count],
                words[0] != 0,
                &mut key,
            )?;
            tyre_grip::grip(v[0], v[1], v[2], v[3], factor)?
        };
        let (cached, unresolved) = match key {
            Key::Code(v) => (u32::from_le_bytes(v), 1),
            Key::Index(v) => (v as u32, 0),
        };
        results.push(format!(
            "P\t{}\t{:08x}\t{cached:08x}\t{unresolved:08x}",
            cells[0],
            grip.to_bits()
        ));
    }
    let out = Path::new(&args[2]);
    let parent = out.parent().ok_or("missing parent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("private output required".into());
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(parent.join(out.file_name().ok_or("missing name")?))?;
    for line in &results {
        writeln!(file, "{line}")?;
    }
    file.flush()?;
    println!("tyre_grip_cases={}", results.len());
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("grid-tyre-grip: {error}");
        std::process::exit(1);
    }
}
