//! Private authored-grid comparison diagnostic.
use grid_world::candidate_grid::{Grid, Tile};
use std::{fs::OpenOptions, io::Write, path::Path};
fn captured_grid(path: &Path) -> Result<Grid, Box<dyn std::error::Error>> {
    if path.metadata()?.len() > 5 * 1024 * 1024 {
        return Err("captured grid too large".into());
    }
    let data = std::fs::read(path)?;
    if data.get(..4) != Some(b"GRC1") {
        return Err("invalid captured fixture tag".into());
    }
    let mut cursor = 4;
    let mut next = || -> Result<u32, Box<dyn std::error::Error>> {
        let bytes = data
            .get(cursor..cursor + 4)
            .ok_or("truncated captured grid")?;
        cursor += 4;
        Ok(u32::from_le_bytes(bytes.try_into()?))
    };
    let mut bounds = [0.; 4];
    for b in &mut bounds {
        *b = f32::from_bits(next()?);
    }
    let mut tiles = Vec::new();
    let mut total = 0usize;
    for _ in 0..1024 {
        let exponent = next()?;
        let width = next()?;
        if exponent > 6 || width != 1u32 << exponent {
            return Err("invalid captured tile dimensions".into());
        }
        let count = (width * width) as usize;
        total += count;
        if total > 1024 * 1024 {
            return Err("captured cell bound exceeded".into());
        }
        let cells = (0..count).map(|_| next()).collect::<Result<Vec<_>, _>>()?;
        tiles.push(Tile {
            exponent: exponent as u8,
            cells,
        });
    }
    if cursor != data.len() {
        return Err("trailing captured grid bytes".into());
    }
    Ok(Grid {
        bounds,
        active: true,
        tiles,
    })
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 && args.len() != 3 {
        return Err(
            "usage: grid-candidate-grid INPUT.tsv local/OUTPUT.tsv [PRIVATE_GRID.bin]".into(),
        );
    }
    let input = Path::new(&args[0]);
    if input.metadata()?.len() > 1024 * 1024 {
        return Err("input exceeds bound".into());
    }
    let out = Path::new(&args[1]);
    let parent = out.parent().ok_or("missing parent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("private output only".into());
    }
    let captured = if args.len() == 3 {
        Some(captured_grid(Path::new(&args[2]))?)
    } else {
        None
    };
    let mut report = String::new();
    let mut count = 0;
    for line in std::fs::read_to_string(input)?.lines() {
        count += 1;
        let fields: Vec<_> = line.split_whitespace().collect();
        if count > 4096
            || fields.len() != 13
            || fields[0].len() > 63
            || !fields[0]
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err("invalid case framing".into());
        }
        let words: Vec<u32> = fields[1..]
            .iter()
            .map(|s| {
                if s.len() != 8 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err("invalid word");
                }
                u32::from_str_radix(s, 16).map_err(|_| "invalid word")
            })
            .collect::<Result<_, _>>()?;
        let f: Vec<_> = words[..8].iter().map(|w| f32::from_bits(*w)).collect();
        let capacity = words[8] as usize;
        let seed = words[9];
        let exponent = words[10];
        if seed > 1000 || exponent > 3 || words[11] > 1 {
            return Err("invalid fixture configuration".into());
        }
        let width = 1u32 << exponent;
        let tiles = (0..1024)
            .map(|tile| Tile {
                exponent: exponent as u8,
                cells: (0..width * width)
                    .map(|c| {
                        let key = (tile % 32) * 13
                            + (tile / 32) * 7
                            + (c % width) * 5
                            + (c / width) * 11
                            + seed;
                        if key.is_multiple_of(5) {
                            0
                        } else {
                            key % (seed % 31 + 1) + 1
                        }
                    })
                    .collect(),
            })
            .collect();
        let mut grid = Grid {
            bounds: f[..4].try_into()?,
            active: words[11] != 0,
            tiles,
        };
        if let Some(supplied) = &captured {
            if grid.bounds != supplied.bounds {
                return Err("fixture bounds differ from requests".into());
            }
            grid = supplied.clone();
            grid.active = words[11] != 0;
        }
        let low = [f[4], f[5]];
        let high = [f[6], f[7]];
        let a = grid.coordinates(low)?;
        let b = grid.coordinates(high)?;
        let ids = grid.candidates(low, high, capacity)?;
        report += &format!(
            "P\t{}\t{:08x}\t{:08x}\t{:08x}\t{:08x}\t{:08x}",
            fields[0],
            a[0],
            a[1],
            b[0],
            b[1],
            ids.len()
        );
        for id in ids {
            report += &format!("\t{id:08x}");
        }
        report.push('\n');
    }
    if count == 0 {
        return Err("no cases".into());
    }
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(parent.join(out.file_name().ok_or("missing name")?))?
        .write_all(report.as_bytes())?;
    println!("candidate_grid_cases={count}");
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-candidate-grid: {e}");
        std::process::exit(1);
    }
}
