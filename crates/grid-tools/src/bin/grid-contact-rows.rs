//! Private complete initial constraint builder diagnostic, CRW1.
use grid_world::contact_rows::{self, Body, Contact, Input};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

struct Reader {
    data: Vec<u8>,
    at: usize,
}
impl Reader {
    fn word(&mut self) -> Result<u32, Box<dyn std::error::Error>> {
        let bytes = self
            .data
            .get(self.at..self.at + 4)
            .ok_or("truncated input")?;
        self.at += 4;
        Ok(u32::from_le_bytes(bytes.try_into()?))
    }
    fn numbers<const N: usize>(&mut self) -> Result<[f32; N], Box<dyn std::error::Error>> {
        let mut result = [0.; N];
        for value in &mut result {
            *value = f32::from_bits(self.word()?);
        }
        Ok(result)
    }
    fn boolean(&mut self) -> Result<bool, Box<dyn std::error::Error>> {
        match self.word()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err("invalid boolean".into()),
        }
    }
    fn shape(&mut self) -> Result<Option<bool>, Box<dyn std::error::Error>> {
        match self.word()? {
            0 => Ok(None),
            1 => Ok(Some(false)),
            2 => Ok(Some(true)),
            _ => Err("invalid shape".into()),
        }
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-contact-rows INPUT.bin local/OUTPUT.bin".into());
    }
    if fs::metadata(&args[0])?.len() > 32 * 1024 * 1024 {
        return Err("input exceeds bound".into());
    }
    let mut r = Reader {
        data: fs::read(&args[0])?,
        at: 0,
    };
    let magic = r.word()?;
    let count = r.word()?;
    if magic != 0x31575243 || !(1..=1024).contains(&count) {
        return Err("invalid header".into());
    }
    let mut output = Vec::<u32>::new();
    output.extend([magic, count]);
    for _ in 0..count {
        let nb = r.word()? as usize;
        let nm = r.word()? as usize;
        let nc = r.word()? as usize;
        if !(1..=16).contains(&nb) || !(1..=8).contains(&nm) || nc > 64 {
            return Err("invalid counts".into());
        }
        let parameters = r.numbers()?;
        let pair = [r.word()?, r.word()?];
        let cached_pair = if pair == [u32::MAX; 2] {
            None
        } else {
            if pair.iter().any(|&i| i as usize >= nm) {
                return Err("invalid cached pair".into());
            }
            Some(pair.map(|i| i as usize))
        };
        let cached_material = r.numbers()?;
        let mut bodies = Vec::with_capacity(nb);
        for _ in 0..nb {
            bodies.push(Body {
                position: r.numbers()?,
                angular: r.numbers()?,
                linear: r.numbers()?,
                scale_angular: r.boolean()?,
                directional_friction: r.boolean()?,
            });
        }
        let mut materials = Vec::with_capacity(nm);
        for _ in 0..nm {
            materials.push(r.numbers()?);
        }
        let mut contacts = Vec::with_capacity(nc);
        for _ in 0..nc {
            let two_bodies = r.boolean()?;
            let bodies = [r.word()? as usize, r.word()? as usize];
            let materials = [r.word()? as usize, r.word()? as usize];
            let shapes = [r.shape()?, r.shape()?];
            let special = r.boolean()?;
            let group_end = r.boolean()?;
            let disabled = r.boolean()?;
            let [accumulated, previous_impulse] = r.numbers()?;
            let point = r.numbers()?;
            let normal = r.numbers()?;
            contacts.push(Contact {
                two_bodies,
                bodies,
                materials,
                shapes,
                special,
                group_end,
                disabled,
                accumulated,
                previous_impulse,
                point,
                normal,
            });
        }
        let mut rows = std::array::from_fn(|_| Vec::new());
        for (p, pool) in rows.iter_mut().enumerate() {
            for _ in 0..nc.max(1) * if p % 2 == 0 { 1 } else { 3 } {
                let mut row = [0; 48];
                for word in &mut row {
                    *word = r.word()?;
                }
                pool.push(row);
            }
        }
        let o = contact_rows::calculate(&Input {
            parameters,
            bodies,
            materials,
            contacts,
            rows,
            cached_pair,
            cached_material,
        })?;
        output.extend(o.batch_counts);
        output.push(o.normal_count);
        output.extend(o.cursors.map(|i| i as u32 * 192));
        output.extend(o.heads.map(|i| i.map_or(u32::MAX, |i| i as u32 * 192)));
        output.extend(o.point_sum.map(f32::to_bits));
        output.extend(o.normal_sum.map(f32::to_bits));
        output.extend([o.friction_sum.to_bits(), o.group_count]);
        output.extend(o.cached_pair.map_or([u32::MAX; 2], |p| p.map(|i| i as u32)));
        output.extend(o.cached_material.map(f32::to_bits));
        output.extend(o.rows.into_iter().flatten().flatten());
    }
    if r.at != r.data.len() {
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
    for word in output {
        file.write_all(&word.to_le_bytes())?;
    }
    file.flush()?;
    println!("contact_rows_cases={count}");
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("grid-contact-rows: {error}");
        std::process::exit(1);
    }
}
