//! Private bounded whole-grid supplied-impact diagnostic (GIM1 binary protocol).
use grid_world::{deformation_impact as impact, deformation_settle::Edge};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

fn u32_at(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap())
}
fn f32_at(data: &[u8], offset: usize) -> f32 {
    f32::from_bits(u32_at(data, offset))
}
fn vector(data: &[u8], offset: usize) -> [f32; 4] {
    std::array::from_fn(|i| f32_at(data, offset + i * 4))
}
fn put_vector(data: &mut [u8], offset: usize, value: [f32; 4]) {
    for (i, v) in value.into_iter().enumerate() {
        data[offset + i * 4..offset + i * 4 + 4].copy_from_slice(&v.to_le_bytes());
    }
}
fn take<'a>(
    data: &'a [u8],
    cursor: &mut usize,
    length: usize,
) -> Result<&'a [u8], Box<dyn std::error::Error>> {
    let end = cursor.checked_add(length).ok_or("input length overflow")?;
    let result = data.get(*cursor..end).ok_or("truncated input")?;
    *cursor = end;
    Ok(result)
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-deformation-impact INPUT.bin local/OUTPUT.bin".into());
    }
    if fs::metadata(&args[0])?.len() > 64 * 1024 * 1024 {
        return Err("input exceeds limit".into());
    }
    let data = fs::read(&args[0])?;
    let mut cursor = 0;
    let header = take(&data, &mut cursor, 8)?;
    let cases = u32_at(header, 4) as usize;
    if u32_at(header, 0) != 0x314d4947 || !(1..=1024).contains(&cases) {
        return Err("invalid input header".into());
    }
    let mut output = header.to_vec();
    for _ in 0..cases {
        let header = take(&data, &mut cursor, 8)?;
        let records = u32_at(header, 0) as usize;
        if records > 512 {
            return Err("record count exceeds limit".into());
        }
        let mut d = take(&data, &mut cursor, 0xa500)?.to_vec();
        let s = take(&data, &mut cursor, 0x1050)?;
        let globals = take(&data, &mut cursor, 24)?;
        let clip = u32_at(take(&data, &mut cursor, 4)?, 0);
        if clip > 1
            || d[0xa4f0] > 1
            || (0..11).any(|i| !f32_at(s, 0x1024 + i * 4).is_finite())
            || (0..6)
                .filter(|i| *i != 3)
                .any(|i| !f32_at(globals, i * 4).is_finite())
        {
            return Err("invalid supplied policy".into());
        }
        let source = take(&data, &mut cursor, records * 48)?;
        let records: Vec<_> = source
            .as_chunks::<48>()
            .0
            .iter()
            .map(|r| impact::Record {
                direction: vector(r, 0),
                anchor: vector(r, 0x10),
                node: u16::from_le_bytes(r[0x20..0x22].try_into().unwrap()) as usize,
                scalar: f32_at(r, 0x24),
            })
            .collect();
        let edge_count = u32_at(&d, 0xa410) as usize;
        if edge_count > 1200 {
            return Err("edge count exceeds limit".into());
        }
        let edges: Vec<_> = (0..edge_count)
            .map(|i| Edge {
                endpoints: [
                    u16::from_le_bytes(d[i * 8..i * 8 + 2].try_into().unwrap()) as usize,
                    u16::from_le_bytes(d[i * 8 + 2..i * 8 + 4].try_into().unwrap()) as usize,
                ],
                length: f32_at(&d, i * 8 + 4),
            })
            .collect();
        let nodes: Vec<_> = (0..400)
            .map(|i| {
                let offset = 0x2580 + i * 80;
                impact::Node {
                    position: vector(&d, offset),
                    rest: vector(&d, offset + 0x10),
                    bounds: vector(&d, offset + 0x20),
                    motion: vector(&d, offset + 0x30),
                    marker: f32_at(&d, offset + 0x40),
                }
            })
            .collect();
        let result = impact::calculate(impact::Input {
            dimensions: std::array::from_fn(|i| u32_at(&d, 0xa414 + i * 4) as usize),
            spacing: f32_at(&d, 0xa420),
            mode: u32_at(&d, 0xa4a0),
            strain_threshold: f32_at(s, 0x1024),
            strain_gain: f32_at(s, 0x1028),
            displacement_threshold: f32_at(s, 0x1048),
            supplied_gain: f32_at(header, 4),
            integration_step: f32_at(globals, 8),
            damping: f32_at(globals, 16),
            motion_scale: f32_at(globals, 20),
            iterations: u32_at(globals, 12),
            counter: u32_at(&d, 0xa450),
            clip_each_iteration: clip == 1,
            offset: vector(&d, 0xa440),
            dirty: d[0xa4f0] != 0,
            nodes: &nodes,
            edges: &edges,
            records: &records,
        })?;
        for (i, node) in result.nodes.iter().enumerate() {
            put_vector(&mut d, 0x2580 + i * 80, node.position);
            put_vector(&mut d, 0x25b0 + i * 80, node.motion);
        }
        for (i, length) in result.lengths.iter().enumerate() {
            d[i * 8 + 4..i * 8 + 8].copy_from_slice(&length.to_le_bytes());
        }
        put_vector(&mut d, 0xa440, result.offset);
        d[0xa450..0xa454].copy_from_slice(&result.counter.to_le_bytes());
        d[0xa4f0] = u8::from(result.dirty);
        d[0xa4f8..0xa4fc].fill(0);
        output.extend(d);
    }
    if cursor != data.len() {
        return Err("trailing input".into());
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
    println!("deformation_impact_cases={cases}");
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("grid-deformation-impact: {error}");
        std::process::exit(1);
    }
}
