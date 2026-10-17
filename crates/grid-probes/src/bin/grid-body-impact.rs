use grid_world::{
    body_impact::{self, Event, Parameters},
    vehicle_publication::{PHYSICAL_WORDS, State, VEHICLE_WORDS},
};
use std::{fs, io::Write, path::Path};
const PREFIX: usize = 52;
const SIZE: usize = PREFIX + VEHICLE_WORDS * 4 + PHYSICAL_WORDS * 4;
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3
        || fs::metadata(&args[0])?.len() > (8 + 64 * SIZE) as u64
        || fs::metadata(&args[1])?.len() != 24
    {
        return Err("usage: grid-body-impact BH01_INPUT PRIVATE_PARAMETERS local/OUTPUT".into());
    }
    let raw = fs::read(&args[0])?;
    if raw.len() < 8 || &raw[..4] != b"BH01" {
        return Err("invalid body event framing".into());
    }
    let n = u32::from_le_bytes(raw[4..8].try_into().unwrap()) as usize;
    if !(1..=64).contains(&n) || raw.len() != 8 + n * SIZE {
        return Err("invalid body event extent".into());
    }
    let parameters = fs::read(&args[1])?;
    let pf = |i: usize| f32::from_le_bytes(parameters[i * 4..i * 4 + 4].try_into().unwrap());
    let p = Parameters {
        impact_scale: pf(0),
        timer_cap: pf(1),
        linear_squared_threshold: pf(2),
        suspension_scale: pf(3),
        suspension_gain: pf(4),
        suspension_offset: pf(5),
    };
    let mut out = raw[..8].to_vec();
    for r in raw[8..].as_chunks::<SIZE>().0 {
        let u = |o| u32::from_le_bytes(r[o..o + 4].try_into().unwrap());
        let f = |o| f32::from_bits(u(o));
        if u(4) > 7 || u(48) > 0x7ff {
            return Err("invalid body event selection".into());
        }
        let state = State {
            damage_random_state: None,
            vehicle: (0..VEHICLE_WORDS).map(|i| u(PREFIX + i * 4)).collect(),
            physical: std::array::from_fn(|i| u(PREFIX + VEHICLE_WORDS * 4 + i * 4)),
        };
        let side = match u(4) {
            0 | 2 | 4 => Some(0),
            1 | 3 | 6 => Some(1),
            _ => None,
        };
        // Preflight even unmatched registrations, without publishing their result.
        let selected = side.unwrap_or(0);
        let other_registration = match u(4) {
            2 | 3 => 0,
            _ => {
                if selected == 0 {
                    2
                } else {
                    1
                }
            }
        };
        let event = Event {
            point: std::array::from_fn(|j| f(16 + j * 4)),
            normal: std::array::from_fn(|j| f(32 + j * 4)),
            impact: f(8),
            parameter: f(12),
            other_registration,
            other_word: if selected == 0 {
                0x9abcdef0
            } else {
                0x12345678
            },
        };
        let users = std::array::from_fn(|j| {
            if u(48) & (1 << j) != 0 {
                Some(0x24680000 + j as u32)
            } else {
                None
            }
        });
        let result = body_impact::calculate(&state, event, selected, p, &users, u(0))?;
        let (answer, random_state, notifications) = if side.is_some() {
            (result.state, result.random_state, result.notifications)
        } else {
            (state, u(0), Vec::new())
        };
        out.extend(((answer.vehicle[0xba5 / 4] >> 8) & 255).to_le_bytes());
        out.extend(random_state.to_le_bytes());
        out.extend((notifications.len() as u32).to_le_bytes());
        for packet in &notifications {
            for word in packet {
                out.extend(word.to_le_bytes());
            }
        }
        out.extend(vec![0; (11 - notifications.len()) * 56]);
        for word in answer.vehicle.iter().chain(answer.physical.iter()) {
            out.extend(word.to_le_bytes());
        }
    }
    let path = Path::new(&args[2]);
    if !path
        .parent()
        .ok_or("missing parent")?
        .canonicalize()?
        .starts_with(Path::new("local").canonicalize()?)
    {
        return Err("private output required".into());
    }
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?
        .write_all(&out)?;
    println!("body_impact_cases={n}");
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
