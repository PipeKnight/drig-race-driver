//! Private whole impact-only main manager diagnostic, PST1.
use grid_world::{
    contact_rows,
    impact_step::{self, Body, Input},
};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
type Failure = Box<dyn std::error::Error>;
fn word(d: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(d[o..o + 4].try_into().unwrap())
}
fn number(d: &[u8], o: usize) -> f32 {
    f32::from_bits(word(d, o))
}
fn vector(d: &[u8], o: usize) -> [f32; 4] {
    std::array::from_fn(|i| number(d, o + 4 * i))
}
fn matrix(d: &[u8], o: usize) -> [[f32; 4]; 4] {
    std::array::from_fn(|i| vector(d, o + 16 * i))
}
fn put(d: &mut [u8], o: usize, v: u32) {
    d[o..o + 4].copy_from_slice(&v.to_le_bytes());
}
fn store(d: &mut [u8], o: usize, v: [f32; 4]) {
    for (i, v) in v.into_iter().enumerate() {
        put(d, o + 4 * i, v.to_bits());
    }
}
fn take<'a>(d: &'a [u8], p: &mut usize, n: usize) -> Result<&'a [u8], Failure> {
    let end = p.checked_add(n).ok_or("length overflow")?;
    let result = d.get(*p..end).ok_or("truncated input")?;
    *p = end;
    Ok(result)
}
fn shape(i: u32) -> Result<Option<bool>, Failure> {
    match i {
        0 => Ok(None),
        1 => Ok(Some(false)),
        2 => Ok(Some(true)),
        _ => Err("invalid shape".into()),
    }
}
fn run() -> Result<(), Failure> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-impact-step INPUT.bin local/OUTPUT.bin".into());
    }
    if fs::metadata(&args[0])?.len() > 32 * 1024 * 1024 {
        return Err("input exceeds bound".into());
    }
    let data = fs::read(&args[0])?;
    let mut p = 0;
    let h = take(&data, &mut p, 8)?;
    let count = word(h, 4);
    if word(h, 0) != 0x31545350 || !(1..=1024).contains(&count) {
        return Err("invalid header".into());
    }
    let mut output = h.to_vec();
    for _ in 0..count {
        let h = take(&data, &mut p, 24)?;
        let nb = word(h, 0) as usize;
        let nm = word(h, 4) as usize;
        let nc = word(h, 8) as usize;
        let nr = word(h, 12) as usize;
        let capacity = word(h, 16) as usize;
        let iterations = word(h, 20) as usize;
        if !(1..=16).contains(&nb)
            || !(1..=8).contains(&nm)
            || nc > 64
            || !(1..=256).contains(&nr)
            || capacity > nr
            || iterations > 32
        {
            return Err("invalid counts".into());
        }
        let h = take(&data, &mut p, 44)?;
        let parameters = std::array::from_fn(|i| number(h, 4 * i));
        let h = take(&data, &mut p, 16)?;
        let initial_ends = std::array::from_fn(|i| word(h, 4 * i) as usize);
        let h = take(&data, &mut p, 60)?;
        let pair = [word(h, 0), word(h, 4)];
        let cached_pair = if pair == [u32::MAX; 2] {
            None
        } else {
            if pair.iter().any(|&i| i as usize >= nm) {
                return Err("invalid cached pair".into());
            }
            Some(pair.map(|i| i as usize))
        };
        let cached_material = std::array::from_fn(|i| number(h, 8 + 4 * i));
        let mut bodies = Vec::new();
        let mut body_wire = Vec::new();
        let mut workspace = Vec::new();
        for _ in 0..nb {
            let b = take(&data, &mut p, 0x424)?;
            if word(b, 0x334) != 0x1c0 || (0x40c..=0x410).any(|o| b[o] > 1) {
                return Err("invalid body framing".into());
            }
            let properties = take(&data, &mut p, 68)?;
            let w = take(&data, &mut p, 32)?;
            workspace.push([vector(w, 0), vector(w, 16)]);
            bodies.push(Body {
                contact: contact_rows::Body {
                    position: vector(b, 0x1c0),
                    angular: vector(b, 0x210),
                    linear: vector(b, 0x220),
                    scale_angular: b[0x410] != 0,
                    directional_friction: b[0x40f] != 0,
                },
                inverse_mass: number(properties, 0),
                inverse_inertia: matrix(properties, 4),
                force_inverse_mass: number(b, 0x3f4),
                torque_matrix: matrix(b, 0x110),
                impulse_accumulator: vector(b, 0x240),
                torque: vector(b, 0x250),
                force: vector(b, 0x260),
                impulse_active: b[0x40c] != 0,
                apply_torque: b[0x40d] != 0,
                apply_force: b[0x40e] != 0,
            });
            body_wire.push(b.to_vec());
        }
        let mut materials = Vec::new();
        for _ in 0..nm {
            let m = take(&data, &mut p, 52)?;
            materials.push(std::array::from_fn(|i| number(m, 4 * i)));
        }
        let mut contacts = Vec::new();
        let mut contact_wire = Vec::new();
        for _ in 0..nc {
            let mode = word(take(&data, &mut p, 4)?, 0);
            let c = take(&data, &mut p, 128)?;
            if mode > 1
                || word(c, 4) != word(c, 0x1c)
                || word(c, 8) != word(c, 0x20)
                || c[0xe] > 1
                || c[0x70] != 0
                || c[0x73] > 1
            {
                return Err("invalid contact framing".into());
            }
            contacts.push(contact_rows::Contact {
                two_bodies: mode != 0,
                bodies: [word(c, 0x1c) as usize, word(c, 0x20) as usize],
                materials: [word(c, 0x30) as usize, word(c, 0x34) as usize],
                shapes: [shape(word(c, 0x2c))?, shape(word(c, 0x28))?],
                special: c[0x73] != 0,
                group_end: false,
                disabled: c[0xe] != 0,
                accumulated: number(c, 0x38),
                previous_impulse: number(c, 0x3c),
                point: vector(c, 0x50),
                normal: vector(c, 0x60),
            });
            contact_wire.push(c.to_vec());
        }
        let raw = take(&data, &mut p, nr * 192)?;
        let rows = raw
            .as_chunks::<192>()
            .0
            .iter()
            .map(|r| std::array::from_fn(|i| word(r, 4 * i)))
            .collect();
        let o = impact_step::calculate(&Input {
            parameters,
            bodies,
            materials,
            contacts,
            rows,
            capacity,
            iterations,
            initial_ends,
            workspace,
            cached_pair,
            cached_material,
        })?;
        output.extend(u32::from(o.success).to_le_bytes());
        for e in o.ends {
            output.extend((e as u32).to_le_bytes());
        }
        for i in o.cached_pair.map_or([u32::MAX; 2], |p| p.map(|i| i as u32)) {
            output.extend(i.to_le_bytes());
        }
        for f in o.cached_material {
            output.extend(f.to_le_bytes());
        }
        for f in o.workspace.iter().flatten().flatten() {
            output.extend(f.to_le_bytes());
        }
        for (mut raw, b) in body_wire.into_iter().zip(o.bodies) {
            store(&mut raw, 0x210, b.contact.angular);
            store(&mut raw, 0x220, b.contact.linear);
            store(&mut raw, 0x240, b.impulse_accumulator);
            raw[0x40c] = u8::from(b.impulse_active);
            output.extend(raw);
        }
        for ((mut raw, c), metadata) in contact_wire.into_iter().zip(o.contacts).zip(o.metadata) {
            put(&mut raw, 0x14, metadata[0]);
            put(&mut raw, 0x18, metadata[1]);
            raw[0x70] = u8::from(c.group_end);
            put(&mut raw, 0x3c, c.previous_impulse.to_bits());
            output.extend(raw);
        }
        for w in o.rows.into_iter().flatten() {
            output.extend(w.to_le_bytes());
        }
    }
    if p != data.len() {
        return Err("trailing input".into());
    }
    let path = Path::new(&args[1]);
    let parent = path.parent().ok_or("missing parent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("private output required".into());
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(parent.join(path.file_name().ok_or("missing name")?))?;
    file.write_all(&output)?;
    file.flush()?;
    println!("impact_step_cases={count}");
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("grid-impact-step: {error}");
        std::process::exit(1);
    }
}
