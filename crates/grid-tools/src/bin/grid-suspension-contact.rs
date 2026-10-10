//! Whole suspension preparation against locally purchased packed track geometry.
use grid_world::{Error, rectangle_cull, suspension_prepare};
use std::{fs::OpenOptions, io::Write, path::Path};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 4 {
        return Err(
            "usage: grid-suspension-contact TRACK.jpk SURFACE.xml INPUT.bin local/OUTPUT.bin"
                .into(),
        );
    }
    let input = Path::new(&args[2]);
    if input.metadata()?.len() > 2 * 1024 * 1024 {
        return Err("input exceeds bound".into());
    }
    let mut bytes = std::fs::read(input)?;
    let word = |b: &[u8]| u32::from_le_bytes(b.try_into().unwrap());
    if bytes.len() < 8 || &bytes[..4] != b"SP01" {
        return Err("invalid preparation header".into());
    }
    let count = word(&bytes[4..8]) as usize;
    const RECORD: usize = 0x2608;
    if !(1..=128).contains(&count) || bytes.len() != 8 + count * RECORD {
        return Err("invalid preparation framing".into());
    }
    let output = Path::new(&args[3]);
    let parent = output
        .parent()
        .ok_or("missing output parent")?
        .canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("private output only".into());
    }
    let track_path = Path::new(&args[0]);
    let surface_path = Path::new(&args[1]);
    if track_path.metadata()?.len() > 256 * 1024 * 1024
        || surface_path.metadata()?.len() > 16 * 1024 * 1024
    {
        return Err("source exceeds bound".into());
    }
    let track = grid_tools::track::Track::read(&std::fs::read(track_path)?)?;
    let surfaces = grid_tools::contact_surfaces::Surfaces::read(&std::fs::read(surface_path)?)?;
    for (index, record) in bytes[8..]
        .as_chunks_mut::<RECORD>()
        .0
        .iter_mut()
        .enumerate()
    {
        if word(&record[..4]) as usize != index {
            return Err("case index mismatch".into());
        }
        let delta = f32::from_bits(word(&record[4..8]));
        let words = std::array::from_fn(|i| word(&record[8 + i * 4..12 + i * 4]));
        let mut state = suspension_prepare::State { words };
        for offset in [0x5d4, 0x608, 0x60c]
            .into_iter()
            .map(|o| o + 0x3d0)
            .chain((0..4).flat_map(|i| [0x3d0 + i * 0x170 + 0x178, 0x3d0 + i * 0x170 + 0x17c]))
        {
            if state.words[offset / 4] != 0 {
                return Err("unowned input pointer/table index".into());
            }
        }
        let mut cache = rectangle_cull::Cache::default();
        suspension_prepare::apply(&mut state, delta, |_, ray| {
            let hit = track
                .query(ray.start, ray.end, &mut cache)
                .map_err(|_| Error("track query failed"))?
                .hit;
            hit.map(|h| {
                let (index, material) = surfaces
                    .contact(h.contact.material)
                    .map_err(|_| Error("missing source mechanics"))?;
                Ok(suspension_prepare::Contact {
                    normal: h.contact.normal,
                    fraction: h.contact.fraction,
                    material,
                    tokens: [index as u32 + 1, 1, 1],
                })
            })
            .transpose()
        })?;
        for (slot, word) in record[8..]
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(state.words)
        {
            slot.copy_from_slice(&word.to_le_bytes());
        }
    }
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(parent.join(output.file_name().ok_or("missing output name")?))?
        .write_all(&bytes)?;
    println!("suspension_preparation_records={count}");
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("grid-suspension-contact: {error}");
        std::process::exit(1);
    }
}
