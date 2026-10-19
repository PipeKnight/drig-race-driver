//! Private joint diagnostic, JNT1 (tools/validate_joint.py): the port's joint
//! constructor on the cases tools/probe_joint.c runs through the original.
use grid_world::{
    joint::{self, Bodies, Joint},
    joint_rows::{self, Context, Pools},
};
use std::{
    fs::{self, OpenOptions},
    io::Write,
};
const BODY: usize = 0x424;
fn word(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap())
}
fn words<const N: usize>(data: &[u8]) -> [u32; N] {
    std::array::from_fn(|i| word(data, i * 4))
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-joint INPUT.bin local/OUTPUT.bin".into());
    }
    if fs::metadata(&args[0])?.len() > 64 * 1024 * 1024 {
        return Err("input exceeds bound".into());
    }
    let data = fs::read(&args[0])?;
    if data.len() < 8 || word(&data, 0) != 0x31544e4a {
        return Err("invalid header".into());
    }
    let count = word(&data, 4) as usize;
    let record = 16 + joint::DESCRIPTOR + 2 * BODY + 0x80 + 32;
    if !(1..=16384).contains(&count) || data.len() != 8 + count * record {
        return Err("invalid case framing".into());
    }
    let mut output = data[..8].to_vec();
    for c in data[8..].chunks_exact(record) {
        let (mode, flags, world) = (word(c, 0), word(c, 4), word(c, 8));
        let reseat = word(c, 12);
        let tail = 16 + joint::DESCRIPTOR + 2 * BODY + 0x80;
        let t: [u32; 8] = words(&c[tail..tail + 32]);
        if mode > 2 || flags > 3 || reseat > 1 || t[6] > 2 {
            return Err("unsupported case".into());
        }
        let mut descriptor: [u32; joint::DESCRIPTOR / 4] = words(&c[16..16 + joint::DESCRIPTOR]);
        let body = |k: usize| -> grid_world::contact_groups::Body {
            let o = 16 + joint::DESCRIPTOR + k * BODY;
            let mut b: grid_world::contact_groups::Body = words(&c[o..o + BODY]);
            for p in [0x320, 0x324, 0x54] {
                b[p / 4] = 0;
            }
            b[0x400 / 4] = u32::MAX;
            b
        };
        let (mut a, mut b) = (body(0), body(1));
        let fixed = flags & 2 != 0;
        descriptor[0xc / 4] = (descriptor[0xc / 4] & !0xff) | u32::from(fixed);
        let mut j = Joint::filled(0xcd);
        joint::construct(
            &mut j,
            &mut descriptor,
            &Bodies {
                a: &a,
                b: (flags & 1 != 0).then_some(&b),
                fixed,
            },
            world,
        );
        if mode >= 1 {
            let moved = 16 + joint::DESCRIPTOR + 2 * BODY;
            a[0x190 / 4..0x1d0 / 4].copy_from_slice(&words::<16>(&c[moved..moved + 0x40]));
            b[0x190 / 4..0x1d0 / 4].copy_from_slice(&words::<16>(&c[moved + 0x40..moved + 0x80]));
            let bytes = j.words[0x32c / 4].to_le_bytes();
            j.words[0x32c / 4] = u32::from_le_bytes([bytes[0], reseat as u8, bytes[2], bytes[3]]);
            joint::prepare(
                &mut j,
                &Bodies {
                    a: &a,
                    b: (flags & 1 != 0).then_some(&b),
                    fixed,
                },
            );
        }
        let mut extra = Vec::new();
        if mode == 2 {
            let mut pools = [[[0u32; 48]; 16], [[0u32; 48]; 16]];
            for (q, pool) in pools.iter_mut().enumerate() {
                for (r, row) in pool.iter_mut().enumerate() {
                    for (w, v) in row.iter_mut().enumerate() {
                        *v = t[7] ^ (q as u32 * 0x10000 + r as u32 * 48 + w as u32);
                    }
                    row[32] = 0;
                    row[33] = t[3];
                    row[34] = 0xff7f_ffff;
                    row[35] = 0x7f7f_ffff;
                    row[42] = 0;
                    row[40] = u32::MAX;
                    row[41] = u32::MAX;
                    row[45] = 0;
                }
            }
            j.words[0x1c / 4] = t[4];
            j.words[0x20 / 4] = t[5];
            let start = usize::from(t[6] != 0);
            let mut ctx = Context {
                inverse_step: f32::from_bits(t[0]),
                reduction: f32::from_bits(t[1]),
                step: f32::from_bits(t[2]),
                two_batch: 0,
                one_batch: 0,
                total: 0,
                one_cursor: start,
                two_cursor: start,
                one_head: None,
                two_head: None,
            };
            if t[6] != 0 {
                let other = if t[6] == 2 { t[4] + 1 } else { t[4] };
                for pool in pools.iter_mut() {
                    pool[0][40] = other;
                    pool[0][41] = t[5];
                }
                ctx.one_head = Some(0);
                ctx.two_head = Some(0);
                ctx.two_batch = 1;
                ctx.one_batch = 1;
            }
            let two = flags & 1 != 0 && !fixed;
            let [one_pool, two_pool] = &mut pools;
            joint_rows::write(
                &j,
                &Bodies {
                    a: &a,
                    b: (flags & 1 != 0).then_some(&b),
                    fixed,
                },
                two,
                &mut ctx,
                &mut Pools {
                    one: one_pool,
                    two: two_pool,
                },
            );
            let mut context = [0u32; 0x60 / 4];
            context[1] = ctx.inverse_step.to_bits();
            context[2] = ctx.reduction.to_bits();
            context[5] = ctx.step.to_bits();
            context[9] = ctx.two_batch;
            context[10] = ctx.one_batch;
            context[13] = ctx.total;
            context[14] = ctx.one_cursor as u32;
            context[16] = ctx.two_cursor as u32;
            context[18] = ctx.one_head.map_or(u32::MAX, |h| h as u32);
            context[20] = ctx.two_head.map_or(u32::MAX, |h| h as u32);
            for w in context {
                extra.extend(w.to_le_bytes());
            }
            for pool in &pools {
                for row in pool {
                    for w in row {
                        extra.extend(w.to_le_bytes());
                    }
                }
            }
        }
        for p in [
            0x0, 0x4, 0x8, 0x28, 0x2c, 0x30, 0x34, 0x38, 0x3c, 0x40, 0x44,
        ] {
            j.words[p / 4] = 0;
        }
        for w in j.words {
            output.extend(w.to_le_bytes());
        }
        for w in &descriptor[0x10 / 4..] {
            output.extend(w.to_le_bytes());
        }
        output.extend(extra);
    }
    let path = std::path::Path::new(&args[1]);
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(&output)?;
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-joint: {e}");
        std::process::exit(1);
    }
}
