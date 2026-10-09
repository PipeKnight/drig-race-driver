//! Private supplied-record response diagnostic (IPR1).
use grid_world::impact_response as kernel;
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
        return Err("usage: grid-impact-response INPUT.bin local/OUTPUT.bin".into());
    }
    if fs::metadata(&args[0])?.len() > 64 * 1024 * 1024 {
        return Err("input exceeds limit".into());
    }
    let data = fs::read(&args[0])?;
    let mut cursor = 0;
    let header = take(&data, &mut cursor, 8)?;
    let cases = word(header, 4) as usize;
    if word(header, 0) != 0x31525049 || !(1..=1024).contains(&cases) {
        return Err("invalid input header".into());
    }
    let mut output = header.to_vec();
    for _ in 0..cases {
        let h = take(&data, &mut cursor, 64)?;
        let nodes_count = word(h, 0) as usize;
        let contacts_count = word(h, 4) as usize;
        let records_count = word(h, 8) as usize;
        if !(1..=400).contains(&nodes_count)
            || !(1..=80).contains(&contacts_count)
            || records_count > 400
            || [12, 16, 20].iter().any(|i| word(h, *i) > 1)
        {
            return Err("invalid response row bounds/flags".into());
        }
        let nodes: Vec<_> = (0..nodes_count)
            .map(|_| take(&data, &mut cursor, 16).map(|b| vector(b, 0)))
            .collect::<Result<_, _>>()?;
        let mut contacts = Vec::with_capacity(contacts_count);
        for _ in 0..contacts_count {
            let c = take(&data, &mut cursor, 32)?;
            if word(c, 0) > 1 || word(c, 4) > 1 {
                return Err("invalid contact flags".into());
            }
            contacts.push(kernel::Contact {
                enabled: word(c, 0) != 0,
                reverse: word(c, 4) != 0,
                baseline: number(c, 8),
                positive_gate: number(c, 12),
                normal: vector(c, 16),
            });
        }
        let mut records = Vec::with_capacity(records_count);
        for _ in 0..records_count {
            let r = take(&data, &mut cursor, 40)?;
            records.push(kernel::Record {
                direction: vector(r, 0),
                anchor: vector(r, 16),
                node: u16::from_le_bytes(r[32..34].try_into().unwrap()),
                contact: u16::from_le_bytes(r[34..36].try_into().unwrap()),
                weight: number(r, 36),
            });
        }
        let response: Vec<_> = (0..contacts_count)
            .map(|_| take(&data, &mut cursor, 4).map(|b| number(b, 0)))
            .collect::<Result<_, _>>()?;
        let result = kernel::calculate(kernel::Input {
            nodes: &nodes,
            records: &records,
            contacts: &contacts,
            response: (word(h, 12) != 0).then_some(response.as_slice()),
            subtract_baseline: word(h, 16) != 0,
            dirty: word(h, 20) != 0,
            scale: number(h, 24),
            correction_gain: number(h, 28),
            normalization_vector: vector(h, 32),
            accumulator: vector(h, 48),
        })?;
        for v in result.accumulator {
            output.extend(v.to_le_bytes());
        }
        output.extend(u32::from(result.dirty).to_le_bytes());
        for v in result.response.unwrap_or(response) {
            output.extend(v.to_le_bytes());
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
    println!("impact_response_cases={cases}");
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("grid-impact-response: {error}");
        std::process::exit(1);
    }
}
