//! Private whole-force comparison; never dereferences captured addresses.
use grid_world::vehicle_force::{self, Frame};
use std::{fs::OpenOptions, io::Write, path::Path};
const RECORD: usize = 15384;
fn word(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap())
}
fn words<const N: usize>(data: &[u8], offset: usize) -> [u32; N] {
    std::array::from_fn(|i| word(data, offset + 4 * i))
}
fn put(data: &mut [u8], offset: usize, words: &[u32]) {
    for (i, value) in words.iter().enumerate() {
        data[offset + 4 * i..offset + 4 * i + 4].copy_from_slice(&value.to_le_bytes());
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-vehicle-force INPUT.bin local/OUTPUT.bin".into());
    }
    let input = Path::new(&args[0]);
    let output = Path::new(&args[1]);
    if input.metadata()?.len() > 2 * 1024 * 1024
        || !output
            .parent()
            .ok_or("missing parent")?
            .canonicalize()?
            .starts_with(Path::new("local").canonicalize()?)
    {
        return Err("bounded input and private output required".into());
    }
    let mut data = std::fs::read(input)?;
    if data.len() < 8 || ![0x31304657, 0x31304650].contains(&word(&data, 0)) {
        return Err("invalid header".into());
    }
    let propulsion = word(&data, 0) == 0x31304650;
    let count = word(&data, 4) as usize;
    if !(1..=64).contains(&count) || data.len() != 8 + count * RECORD {
        return Err("invalid size/count".into());
    }
    for (index, record) in data[8..].as_chunks_mut::<RECORD>().0.iter_mut().enumerate() {
        if word(record, 0) as usize != index {
            return Err("invalid case identity".into());
        }
        let address = word(record, 8);
        let view = word(record, 12);
        let solver = word(record, 16);
        let body = words(record, 36);
        let view_words = words(record, 36 + 0x2600);
        let solver_words = words(record, 36 + 0x2600 + 0x300);
        if body[0] != view
            || view_words[0] != solver
            || solver.checked_sub(view) != Some(0x2b0)
            || record[36 + 0x2600 + 0x2b0..36 + 0x2600 + 0x300]
                != record[36 + 0x2600 + 0x300..36 + 0x2600 + 0x350]
        {
            return Err("invalid aliases".into());
        }
        let n = body[0x18f0 / 4] as usize;
        if !(1..=4).contains(&n) {
            return Err("invalid driven count".into());
        }
        let mut driven = Vec::new();
        for i in 0..n {
            let offset = body[0x18d8 / 4 + i]
                .checked_sub(address)
                .ok_or("foreign wheel")?;
            let wheel = offset.checked_sub(0x1990).ok_or("foreign wheel")?;
            if wheel % 0x150 != 0 || wheel / 0x150 >= 4 {
                return Err("foreign wheel".into());
            }
            driven.push((wheel / 0x150) as usize);
        }
        let mut materials = [None; 4];
        for (i, material) in materials.iter_mut().enumerate() {
            if body[(0x1990 + i * 0x150 + 0x34) / 4]
                != address.checked_add(0x15a0).ok_or("address overflow")?
            {
                return Err("invalid wheel owner".into());
            }
            let present = word(record, 20 + 4 * i);
            if present != u32::from(body[(0x1990 + i * 0x150 + 0xc0) / 4] != 0) {
                return Err("invalid material presence".into());
            }
            if present != 0 {
                if propulsion {
                    let pointer = body[(0x1990 + i * 0x150 + 0xc0) / 4];
                    let base = 36 + 0x2600 + 0x300 + 0xdd0;
                    for j in 0..i {
                        if body[(0x1990 + j * 0x150 + 0xc0) / 4] == pointer
                            && record[base + i * 0x140..base + (i + 1) * 0x140]
                                != record[base + j * 0x140..base + (j + 1) * 0x140]
                        {
                            return Err("inconsistent material alias".into());
                        }
                    }
                }
                *material = Some(f32::from_bits(word(
                    record,
                    36 + 0x2600 + 0x300 + 0xdd0 + i * 0x140 + 0x64,
                )));
            }
        }
        let diagnostics =
            std::array::from_fn(|i| f32::from_bits(word(record, RECORD - 32 + 4 * i)));
        let mut frame = Frame {
            body,
            view: view_words,
            solver: solver_words,
            driven,
            material_multipliers: materials,
            diagnostics,
        };
        let delta = f32::from_bits(word(record, 4));
        if propulsion {
            let roll = std::array::from_fn(|i| {
                materials[i].map(|_| {
                    f32::from_bits(word(record, 36 + 0x2600 + 0x300 + 0xdd0 + i * 0x140 + 0x20))
                })
            });
            grid_world::vehicle_propulsion::apply(&mut frame, roll, delta)?;
        } else {
            vehicle_force::apply(&mut frame, delta)?;
        }
        put(record, 36, &frame.body);
        put(record, 36 + 0x2600, &frame.view);
        put(record, 36 + 0x2600 + 0x2b0, &frame.solver[..0x50 / 4]);
        put(record, 36 + 0x2600 + 0x300, &frame.solver);
        put(record, RECORD - 32, &frame.diagnostics.map(f32::to_bits));
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    file.write_all(&data)?;
    println!("vehicle_force_cases={count}");
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
