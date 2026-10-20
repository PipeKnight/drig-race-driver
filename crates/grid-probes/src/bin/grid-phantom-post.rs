//! Private phantom-post diagnostic, FPP1 (tools/validate_phantom_post.py):
//! the port's post geometry on the cases tools/probe_phantom_post.c runs
//! through the original (0x7BBB70, 0x7BC370, 0x7BCDA0).
use grid_world::phantom_post::{self, Post, V4};
use std::{
    fs::{self, OpenOptions},
    io::Write,
};
const CASE: usize = 4 + 0x80 + 0x40 + 0x30;
fn word(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap())
}
fn v4(data: &[u8], offset: usize) -> V4 {
    std::array::from_fn(|i| f32::from_bits(word(data, offset + 4 * i)))
}
fn post(p: &[u8]) -> Post {
    Post {
        low: v4(p, 0x10),
        high: v4(p, 0x20),
        pivot: v4(p, 0x30),
        tip: v4(p, 0x40),
        centre: v4(p, 0x50),
        direction: v4(p, 0x60),
        length: f32::from_bits(word(p, 0x70)),
        hit: p[0x74] != 0,
    }
}
fn put(out: &mut [u8], offset: usize, v: V4) {
    for (i, x) in v.iter().enumerate() {
        out[offset + 4 * i..offset + 4 * i + 4].copy_from_slice(&x.to_bits().to_le_bytes());
    }
}
fn store(p: &mut [u8], post: &Post) {
    put(p, 0x10, post.low);
    put(p, 0x20, post.high);
    put(p, 0x30, post.pivot);
    put(p, 0x40, post.tip);
    put(p, 0x50, post.centre);
    put(p, 0x60, post.direction);
    p[0x70..0x74].copy_from_slice(&post.length.to_bits().to_le_bytes());
    p[0x74] = u8::from(post.hit);
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-phantom-post INPUT.bin local/OUTPUT.bin".into());
    }
    if fs::metadata(&args[0])?.len() > 64 * 1024 * 1024 {
        return Err("input exceeds bound".into());
    }
    let data = fs::read(&args[0])?;
    if data.len() < 8 || word(&data, 0) != 0x3150_5046 {
        return Err("invalid header".into());
    }
    let count = word(&data, 4) as usize;
    if count == 0 || count > 65536 || data.len() != 8 + count * CASE {
        return Err("invalid case count".into());
    }
    let mut out = Vec::with_capacity(8 + count * (4 + 0x20 + 0x80));
    out.extend_from_slice(&data[..8]);
    for k in 0..count {
        let c = &data[8 + k * CASE..8 + (k + 1) * CASE];
        let mode = word(c, 0);
        let mut p = c[4..0x84].to_vec();
        let b = &c[0x84..];
        let shape = phantom_post::Box {
            axes: [v4(b, 0), v4(b, 0x10), v4(b, 0x20)],
            centre: v4(b, 0x30),
            half: v4(b, 0x40),
        };
        let (a, z) = (v4(b, 0x50), v4(b, 0x60));
        let mut record = [0u8; 4 + 0x20];
        match mode {
            0 => {
                if let Some(v) = phantom_post::penetration(&post(&p), &shape) {
                    record[0] = 1;
                    put(&mut record, 4, v);
                }
            }
            1 => {
                let (on_segment, on_box) = phantom_post::closest(a, z, &shape);
                put(&mut record, 4, on_segment);
                put(&mut record, 0x14, on_box);
            }
            2 => {
                let mut moved = post(&p);
                phantom_post::push(&mut moved, &shape);
                store(&mut p, &moved);
            }
            _ => return Err("invalid mode".into()),
        }
        out.extend_from_slice(&record);
        out.extend_from_slice(&p);
    }
    let mut file = OpenOptions::new().write(true).create_new(true).open(&args[1])?;
    file.write_all(&out)?;
    println!("post_cases={count} port=1");
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-phantom-post: {e}");
        std::process::exit(1);
    }
}
