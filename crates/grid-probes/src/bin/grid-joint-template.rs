//! Private constraint-template diagnostic, JTP1 (tools/validate_joint_template.py):
//! the port's template frames and joint request on the cases
//! tools/probe_joint_template.c runs through the original.
use grid_world::joint_template::{self, Props};
use std::{
    fs::{self, OpenOptions},
    io::Write,
};
const PROPS: usize = 0x110;
const TEMPLATE: usize = 0x150;
fn word(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap())
}
fn float(data: &[u8], offset: usize) -> f32 {
    f32::from_bits(word(data, offset))
}
fn floats<const N: usize>(data: &[u8], offset: usize) -> [f32; N] {
    std::array::from_fn(|i| float(data, offset + 4 * i))
}
/// The props object's fields at the original's offsets.
fn props(p: &[u8]) -> Props {
    Props {
        linear_limit: [floats(p, 0x30), floats(p, 0x40)],
        linear_spring: [floats(p, 0x50), floats(p, 0x60)],
        spring_k: floats(p, 0x70),
        spring_s: floats(p, 0x7c),
        rotation_type: word(p, 0x88),
        rotation: floats(p, 0x8c),
        twist_type: word(p, 0xa0),
        twist: floats(p, 0xa4),
        break_linear: float(p, 0xb4),
        break_angular: float(p, 0xb8),
        hard_friction: float(p, 0xc0),
        break_at_angular_extent: word(p, 0xc8) != 0,
        rotations: [floats(p, 0xcc), floats(p, 0xdc), floats(p, 0xec)],
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-joint-template INPUT.bin local/OUTPUT.bin".into());
    }
    if fs::metadata(&args[0])?.len() > 64 * 1024 * 1024 {
        return Err("input exceeds bound".into());
    }
    let data = fs::read(&args[0])?;
    if data.len() < 8 || word(&data, 0) != 0x3150544a {
        return Err("invalid header".into());
    }
    let count = word(&data, 4) as usize;
    let record = PROPS + TEMPLATE + 8;
    if count == 0 || count > 65536 || data.len() != 8 + count * record {
        return Err("invalid case count".into());
    }
    let mut out = Vec::with_capacity(8 + count * (0xc0 + joint_template::REQUEST));
    out.extend_from_slice(&data[..8]);
    for k in 0..count {
        let c = &data[8 + k * record..8 + (k + 1) * record];
        let (p, t) = (&c[..PROPS], &c[PROPS..PROPS + TEMPLATE]);
        let scale = float(c, PROPS + TEMPLATE);
        let props = props(p);
        let transform = std::array::from_fn(|r| floats(t, 0x100 + 0x10 * r));
        let frames = joint_template::template_frames(&transform, &props);
        for v in frames.iter().flatten().flatten() {
            out.extend_from_slice(&v.to_le_bytes());
        }
        let request = joint_template::request(&frames, &props, scale, t[0x140] != 0);
        for w in request.words {
            out.extend_from_slice(&w.to_le_bytes());
        }
    }
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[1])?
        .write_all(&out)?;
    println!("template_cases={count}");
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
