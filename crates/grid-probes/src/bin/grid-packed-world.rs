//! Private supplied-world comparison fixture; no proprietary outputs outside local/.
use grid_world::{
    candidate_grid::{Grid, Tile},
    packed_query::Mesh,
    packed_world::Shape,
    rectangle_cull::Cache,
};
use std::{fs::OpenOptions, io::Write, path::Path};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: grid-packed-world PRIVATE_FIXTURE INPUT.tsv local/OUTPUT.tsv".into());
    }
    let path = Path::new(&args[0]);
    let input = Path::new(&args[1]);
    if path.metadata()?.len() > 16 * 1024 * 1024 || input.metadata()?.len() > 1024 * 1024 {
        return Err("input bound exceeded".into());
    }
    let data = std::fs::read(path)?;
    if data.get(..4) != Some(b"WQG1") {
        return Err("invalid world fixture tag".into());
    }
    let mut cursor = 4;
    fn word(data: &[u8], cursor: &mut usize) -> Result<u32, Box<dyn std::error::Error>> {
        let value = u32::from_le_bytes(
            data.get(*cursor..*cursor + 4)
                .ok_or("truncated fixture")?
                .try_into()?,
        );
        *cursor += 4;
        Ok(value)
    }
    let mut bounds = [0.; 4];
    for v in &mut bounds {
        *v = f32::from_bits(word(&data, &mut cursor)?);
    }
    let count = word(&data, &mut cursor)? as usize;
    if count == 0 || count > 512 {
        return Err("shape count exceeds bound".into());
    }
    let mut tiles = Vec::new();
    let mut total = 0;
    for _ in 0..1024 {
        let exponent = word(&data, &mut cursor)?;
        let width = word(&data, &mut cursor)?;
        if exponent > 6 || width != 1 << exponent {
            return Err("invalid tile dimensions".into());
        }
        let cells_count = (width * width) as usize;
        total += cells_count;
        if total > 1024 * 1024 {
            return Err("too many cells".into());
        }
        let mut cells = Vec::new();
        for _ in 0..cells_count {
            let id = word(&data, &mut cursor)?;
            if id as usize > count {
                return Err("cell outside shape table".into());
            }
            cells.push(id);
        }
        tiles.push(Tile {
            exponent: exponent as u8,
            cells,
        });
    }
    let mut chunks = Vec::new();
    for _ in 0..count {
        let length = word(&data, &mut cursor)? as usize;
        if length > 2 * 1024 * 1024 {
            return Err("chunk bound exceeded".into());
        }
        chunks.push(grid_formats::collision::read_vcqtc(
            data.get(cursor..cursor + length).ok_or("truncated chunk")?,
        )?);
        cursor += length;
    }
    if cursor != data.len() {
        return Err("trailing fixture data".into());
    }
    let triangles: Vec<Vec<_>> = chunks
        .iter()
        .map(|c| c.triangles.iter().map(|t| t.indices).collect())
        .collect();
    let materials: Vec<Vec<_>> = chunks
        .iter()
        .map(|c| {
            c.triangles
                .iter()
                .map(|t| c.materials[t.material as usize])
                .collect()
        })
        .collect();
    let shapes: Vec<_> = chunks
        .iter()
        .enumerate()
        .map(|(i, c)| Shape {
            id: i as u32 + 1,
            mesh: Mesh {
                bounds: c.bounds,
                vertices: &c.positions,
                triangles: &triangles[i],
                nodes: &c.nodes,
                leaves: &c.references,
            },
            materials: &materials[i],
        })
        .collect();
    let mut grid = Grid {
        bounds,
        active: true,
        tiles,
    };
    let out = Path::new(&args[2]);
    let parent = out.parent().ok_or("missing parent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("private output only".into());
    }
    let mut report = String::new();
    let mut cases = 0;
    for line in std::fs::read_to_string(input)?.lines() {
        cases += 1;
        let fields: Vec<_> = line.split_whitespace().collect();
        if cases > 4096
            || fields.len() != 11
            || fields[0].len() > 63
            || !fields[0]
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err("invalid case framing".into());
        }
        let words: Vec<_> = fields[1..]
            .iter()
            .map(|s| {
                if s.len() != 8 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err("invalid word");
                }
                u32::from_str_radix(s, 16).map_err(|_| "invalid word")
            })
            .collect::<Result<_, _>>()?;
        if words[8] > 3 || words[9] > 1 {
            return Err("invalid cache or active flag".into());
        }
        grid.active = words[9] != 0;
        let mut cache = Cache {
            plane: words[8] as u8,
        };
        let result = grid_world::packed_world::query(
            &grid,
            &shapes,
            std::array::from_fn(|i| f32::from_bits(words[i])),
            std::array::from_fn(|i| f32::from_bits(words[i + 4])),
            &mut cache,
        )?;
        if let Some(hit) = result.hit {
            report += &format!(
                "H\t{}\t{:08x}\t{:08x}",
                fields[0],
                hit.shape,
                hit.contact.fraction.to_bits()
            );
            for v in hit.contact.normal {
                report += &format!("\t{:08x}", v.to_bits());
            }
            report += &format!("\t{:08x}", u32::from_le_bytes(hit.contact.material));
        } else {
            report += &format!("N\t{}", fields[0]);
        }
        report += &format!("\t{:08x}\t{:08x}", cache.plane, result.candidates.len());
        for id in result.candidates {
            report += &format!("\t{id:08x}");
        }
        report += "\n";
    }
    if cases == 0 {
        return Err("no cases".into());
    }
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(parent.join(out.file_name().ok_or("missing name")?))?
        .write_all(report.as_bytes())?;
    println!("packed_world_cases={cases}");
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-packed-world: {e}");
        std::process::exit(1);
    }
}
