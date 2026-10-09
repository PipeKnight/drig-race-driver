//! Private supplied collision preparation diagnostic (ICP1).
use grid_world::{
    contact_preparation as kernel,
    deformation_settle::{self, Edge, Node},
};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
fn word(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap())
}
fn number(data: &[u8], offset: usize) -> f32 {
    f32::from_bits(word(data, offset))
}
fn vector(data: &[u8], offset: usize) -> [f32; 4] {
    std::array::from_fn(|i| number(data, offset + i * 4))
}
fn put(data: &mut [u8], offset: usize, values: impl IntoIterator<Item = f32>) {
    for (i, v) in values.into_iter().enumerate() {
        data[offset + i * 4..offset + i * 4 + 4].copy_from_slice(&v.to_le_bytes());
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-contact-preparation INPUT.bin local/OUTPUT.bin".into());
    }
    if fs::metadata(&args[0])?.len() > 64 * 1024 * 1024 {
        return Err("input exceeds bound".into());
    }
    let data = fs::read(&args[0])?;
    if data.len() < 8 {
        return Err("truncated header".into());
    }
    let count = word(&data, 4) as usize;
    const SIZE: usize = 0xa500 + 0x1050 + 0x424 + 16 + 64 + 16 + 4 * 4 + 64;
    if word(&data, 0) != 0x31504349
        || !(1..=1024).contains(&count)
        || data.len() != 8 + count * SIZE
    {
        return Err("invalid framing".into());
    }
    let mut output = data[..8].to_vec();
    for row in data[8..].as_chunks::<SIZE>().0 {
        let mut d = row[..0xa500].to_vec();
        let s = &row[0xa500..0xb550];
        let linked = &row[0xb550..0xb974];
        let parameters = &row[0xb974..];
        let nodes: Vec<_> = (0..400)
            .map(|i| Node {
                position: vector(&d, 0x2580 + i * 80),
                rest: vector(&d, 0x2590 + i * 80),
                bounds: vector(&d, 0x25a0 + i * 80),
            })
            .collect();
        let edge_count = word(&d, 0xa410) as usize;
        if edge_count > 1200 || d[0xa4af] > 1 {
            return Err("invalid edge count/disable flag".into());
        }
        let edges: Vec<_> = (0..edge_count)
            .map(|i| Edge {
                endpoints: [
                    u16::from_le_bytes(d[i * 8..i * 8 + 2].try_into().unwrap()) as usize,
                    u16::from_le_bytes(d[i * 8 + 2..i * 8 + 4].try_into().unwrap()) as usize,
                ],
                length: number(&d, i * 8 + 4),
            })
            .collect();
        let result = kernel::calculate(kernel::Input {
            grid: deformation_settle::Input {
                dimensions: std::array::from_fn(|i| word(&d, 0xa414 + i * 4) as usize),
                spacing: number(&d, 0xa420),
                mode: word(&d, 0xa4a0),
                threshold: number(s, 0x1024),
                gain: number(s, 0x1028),
                nodes: &nodes,
                edges: &edges,
            },
            intensity: number(&d, 0xa494),
            disabled: d[0xa4af] != 0,
            up: vector(linked, 0x1e0),
            direction: vector(parameters, 0),
            inverse: std::array::from_fn(|i| vector(parameters, 16 + i * 16)),
            point: vector(parameters, 80),
            policy: kernel::Policy {
                body_factor_a: number(parameters, 96),
                body_factor_b: number(parameters, 100),
                global_factor: number(parameters, 104),
                linked_mass: number(linked, 0x3f0),
                scale: number(parameters, 108),
                table: std::array::from_fn(|i| number(parameters, 112 + i * 4)),
            },
        })?;
        if let Some(active) = result.activated {
            d[0xa4a0..0xa4a4].copy_from_slice(&1u32.to_le_bytes());
            put(&mut d, 0xa454, [active.strength]);
            put(&mut d, 0xa460, active.point);
            put(&mut d, 0xa470, active.direction);
            for (i, p) in active.settled.positions.into_iter().enumerate() {
                put(&mut d, 0x2580 + i * 80, p);
            }
            for (i, v) in active.settled.lengths.into_iter().enumerate() {
                put(&mut d, i * 8 + 4, [v]);
            }
            d[0xa4f0..0xa4f5].fill(1);
        }
        d[0xa4f8..0xa4fc].fill(0);
        output.extend(d);
        output.extend(linked);
        output.extend(&parameters[..96]);
        output.extend(result.scalar.to_le_bytes());
    }
    let out = Path::new(&args[1]);
    let parent = out.parent().ok_or("missing parent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("private output required".into());
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(parent.join(out.file_name().ok_or("missing name")?))?;
    file.write_all(&output)?;
    file.flush()?;
    println!("contact_preparation_cases={count}");
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-contact-preparation: {e}");
        std::process::exit(1);
    }
}
