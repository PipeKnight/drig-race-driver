//! Private contact-to-record prefix diagnostic (IPG1).
use grid_world::impact_generation as kernel;
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
fn take<'a>(
    data: &'a [u8],
    cursor: &mut usize,
    length: usize,
) -> Result<&'a [u8], Box<dyn std::error::Error>> {
    let end = cursor.checked_add(length).ok_or("length overflow")?;
    let result = data.get(*cursor..end).ok_or("truncated input")?;
    *cursor = end;
    Ok(result)
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-impact-generation INPUT.bin local/OUTPUT.bin".into());
    }
    if fs::metadata(&args[0])?.len() > 64 * 1024 * 1024 {
        return Err("input exceeds limit".into());
    }
    let data = fs::read(&args[0])?;
    let mut cursor = 0;
    let header = take(&data, &mut cursor, 8)?;
    let cases = word(header, 4) as usize;
    if word(header, 0) != 0x31475049 || !(1..=1024).contains(&cases) {
        return Err("invalid input header".into());
    }
    let mut output = header.to_vec();
    for _ in 0..cases {
        let header = take(&data, &mut cursor, 12)?;
        let count = word(header, 0) as usize;
        if !(1..=80).contains(&count) {
            return Err("contact count outside bound".into());
        }
        let old_linked = word(header, 8);
        let mut d = take(&data, &mut cursor, 0xa500)?.to_vec();
        let s = take(&data, &mut cursor, 0x1050)?;
        let linked = take(&data, &mut cursor, 0x424)?;
        let body_offset = vector(take(&data, &mut cursor, 16)?, 0);
        let body_b20 = vector(take(&data, &mut cursor, 16)?, 0);
        let globals = take(&data, &mut cursor, 32)?;
        let corner_scale = number(take(&data, &mut cursor, 4)?, 0);
        let static_mass = number(take(&data, &mut cursor, 4)?, 0);
        let mut contacts = Vec::with_capacity(count);
        for _ in 0..count {
            let c = take(&data, &mut cursor, 0x80)?;
            let has_other = word(take(&data, &mut cursor, 4)?, 0);
            if has_other > 1 {
                return Err("invalid counterpart flag".into());
            }
            let other = vector(take(&data, &mut cursor, 16)?, 0);
            contacts.push(kernel::Contact {
                enabled: c[0x73] != 0,
                reverse: c[0xc] == 0 && word(c, 8) == old_linked,
                point: vector(c, 0x50),
                direction: vector(c, 0x60),
                value_38: number(c, 0x38),
                value_3c: number(c, 0x3c),
                value_40: number(c, 0x40),
                value_44: number(c, 0x44),
                counterpart_velocity: (has_other == 1).then_some(other),
            });
        }
        let nodes: Vec<_> = (0..400)
            .map(|i| kernel::Node {
                position: vector(&d, 0x2580 + i * 80),
                marker: number(&d, 0x25c0 + i * 80),
            })
            .collect();
        let result = kernel::calculate(kernel::Input {
            dimensions: std::array::from_fn(|i| word(&d, 0xa414 + i * 4) as usize),
            spacing: number(&d, 0xa420),
            minimum: vector(&d, 0xa430),
            pose: std::array::from_fn(|i| vector(linked, 0x1d0 + i * 16)),
            body_offset,
            body_b20,
            velocity: vector(linked, 0x360),
            supplied_scalar: number(header, 4),
            state: number(&d, 0xa480),
            corners: vector(&d, 0xa4b4),
            window: std::array::from_fn(|i| number(&d, 0xa4c4 + i * 4)),
            corner_scale,
            static_mass,
            limit_scale: number(globals, 8),
            flag_radius_scale: number(globals, 16),
            record_radius_scale: number(globals, 20),
            radial_scale: number(globals, 12),
            damage_threshold: number(s, 0x1030),
            damage_gain: number(s, 0x1034),
            projection: number(s, 0x1038),
            flags: &d[0xa280..0xa410],
            dirty: d[0xa4f0..0xa4f5].try_into().unwrap(),
            nodes: &nodes,
            contacts: &contacts,
        })?;
        d[0xa280..0xa410].copy_from_slice(&result.flags);
        d[0xa4f0..0xa4f5].copy_from_slice(&result.dirty);
        d[0xa480..0xa484].copy_from_slice(&result.state.to_le_bytes());
        d[0xa4f8..0xa4fc].fill(0);
        output.extend((result.records.len() as u32).to_le_bytes());
        output.extend(d);
        for record in result.records {
            for v in record.direction.into_iter().chain(record.anchor) {
                output.extend(v.to_le_bytes());
            }
            output.extend(record.node.to_le_bytes());
            output.extend(record.contact.to_le_bytes());
            output.extend(record.scalar.to_le_bytes());
            output.extend(record.weight.to_le_bytes());
            output.extend([0; 4]);
        }
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
    println!("impact_generation_cases={cases}");
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("grid-impact-generation: {error}");
        std::process::exit(1);
    }
}
