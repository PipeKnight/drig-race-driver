//! Bounded private complete intersection/volume diagnostic.
use grid_world::deformation_volume::{self, TRIANGLES, VERTICES};
use std::{fs::OpenOptions, io::Write, path::Path};
fn word(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-deformation-volume INPUT.bin local/OUTPUT.bin".into());
    }
    let output = Path::new(&args[1]);
    if !output
        .parent()
        .ok_or("missing parent")?
        .canonicalize()?
        .starts_with(Path::new("local").canonicalize()?)
        || std::fs::metadata(&args[0])?.len() > 8 * 1024 * 1024
    {
        return Err("bounded private output required".into());
    }
    let input = std::fs::read(&args[0])?;
    const ROW: usize = 244 + VERTICES * 16 + TRIANGLES * 12;
    if input.len() < 8
        || &input[..4] != b"DV01"
        || !(1..=128).contains(&word(&input, 4))
        || input.len() != 8 + word(&input, 4) as usize * ROW
    {
        return Err("invalid volume framing".into());
    }
    let mut results = input[..8].to_vec();
    for r in input[8..].as_chunks::<ROW>().0 {
        let n = word(r, 0) as usize;
        if !(3..=12).contains(&n) {
            return Err("invalid plane count".into());
        }
        let planes: Vec<[f32; 4]> = (0..n)
            .map(|i| std::array::from_fn(|a| f32::from_bits(word(r, 4 + i * 16 + a * 4))))
            .collect();
        let offsets: Vec<_> = (0..n)
            .map(|i| f32::from_bits(word(r, 196 + i * 4)))
            .collect();
        let vertices: Vec<[f32; 4]> = (0..VERTICES)
            .map(|i| std::array::from_fn(|a| f32::from_bits(word(r, 244 + i * 16 + a * 4))))
            .collect();
        let triangles: Vec<[u32; 3]> = (0..TRIANGLES)
            .map(|i| std::array::from_fn(|a| word(r, 244 + VERTICES * 16 + i * 12 + a * 4)))
            .collect();
        let answer = deformation_volume::calculate(&planes, &offsets, &vertices, &triangles)?;
        results.extend((answer.vertex_count as u32).to_le_bytes());
        results.extend((answer.triangle_count as u32).to_le_bytes());
        results.extend(answer.volume.to_le_bytes());
        for v in answer.vertices {
            for x in v {
                results.extend(x.to_bits().to_le_bytes());
            }
        }
        for t in answer.triangles {
            for x in t {
                results.extend(x.to_le_bytes());
            }
        }
    }
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?
        .write_all(&results)?;
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
