//! Private whole-force comparison; never dereferences captured addresses.
use grid_world::vehicle_force::{self, Frame};
use std::{fs::OpenOptions, io::Write, path::Path};
const RECORD: usize = 15384;
const REFERENCE_GRAVITY: f32 = f32::from_bits(0x411cf5c3);
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
    if args.len() != 2 && args.len() != 4 {
        return Err(
            "usage: grid-vehicle-force [TRACK.jpk SURFACE.xml] INPUT.bin local/OUTPUT.bin".into(),
        );
    }
    let offset = args.len() - 2;
    let input = Path::new(&args[offset]);
    let output = Path::new(&args[offset + 1]);
    if input.metadata()?.len() > 16 * 1024 * 1024
        || !output
            .parent()
            .ok_or("missing parent")?
            .canonicalize()?
            .starts_with(Path::new("local").canonicalize()?)
    {
        return Err("bounded input and private output required".into());
    }
    let mut data = std::fs::read(input)?;
    if data.len() < 8
        || ![
            0x31304657, 0x31304650, 0x31304643, 0x31304647, 0x3130464a, 0x31304655, 0x31304648,
            0x3130464d, 0x31304644, 0x31304645, 0x31304653, 0x31304651, 0x3130464f, 0x3130464b,
            0x3230464b, 0x31304654,
        ]
        .contains(&word(&data, 0))
    {
        return Err("invalid header".into());
    }
    let car_manager = word(&data, 0) == 0x31304654;
    let handling_queries = word(&data, 0) == 0x3230464b || car_manager;
    let car_job = word(&data, 0) == 0x3130464b || handling_queries;
    let complete = word(&data, 0) == 0x3130464f || car_job;
    let outer = word(&data, 0) == 0x31304651 || complete;
    let driver = word(&data, 0) == 0x31304645 || outer;
    let initialize_gearing = word(&data, 0) == 0x31304653;
    let chassis = word(&data, 0) == 0x3130464d || driver;
    let driver_prepare = word(&data, 0) == 0x31304644;
    let post_force = word(&data, 0) == 0x31304648 || chassis;
    let vectors = word(&data, 0) == 0x31304655 || post_force;
    let joined = word(&data, 0) == 0x3130464a || vectors;
    if joined != (args.len() == 4) {
        return Err("source arguments required only for JF01/UF01/HF01/MF01/EF01/QF01/OF01".into());
    }
    let sources = if joined {
        let track = Path::new(&args[0]);
        let surface = Path::new(&args[1]);
        if track.metadata()?.len() > 256 * 1024 * 1024
            || surface.metadata()?.len() > 16 * 1024 * 1024
        {
            return Err("source exceeds bound".into());
        }
        Some((
            grid_tools::track::Track::read(&std::fs::read(track)?)?,
            grid_tools::contact_surfaces::Surfaces::read(&std::fs::read(surface)?)?,
        ))
    } else {
        None
    };
    let prepared = word(&data, 0) == 0x31304647 || joined;
    let controls = word(&data, 0) == 0x31304643 || prepared;
    let propulsion = word(&data, 0) == 0x31304650 || controls;
    let count = word(&data, 4) as usize;
    let record_size = RECORD
        + if complete {
            0x21000 - 0x2600 + 0x424
        } else {
            0
        }
        + if handling_queries { 8 + 64 * 8 } else { 0 };
    if !(1..=64).contains(&count) || data.len() != 8 + count * record_size {
        return Err("invalid size/count".into());
    }
    for (index, record) in data[8..].chunks_exact_mut(record_size).enumerate() {
        let mut query_rows = Vec::new();
        let mut query_present = false;
        if handling_queries {
            let tail = RECORD + 0x21000 - 0x2600 + 0x424;
            let present = word(record, tail);
            let n = word(record, tail + 4) as usize;
            if present > 1
                || n > 64
                || (present == 0 && n != 0)
                || record[tail + 8 + n.min(64) * 8..].iter().any(|&x| x != 0)
            {
                return Err("invalid handling query records".into());
            }
            query_present = present != 0;
            query_rows.extend((0..n).map(|i| grid_world::vehicle_handling::QueryRecord {
                id: word(record, tail + 8 + i * 8),
                status: word(record, tail + 12 + i * 8),
            }));
        }
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
        if initialize_gearing {
            grid_world::vehicle_gearing::initialize(&mut frame)?;
        } else if driver_prepare {
            let flags = std::array::from_fn(|i| {
                record[36 + 0x2600 + 0x300 + 0xdd0 + i * 0x140 + 0x75] != 0
            });
            grid_world::vehicle_driver_prepare::apply(&mut frame, delta, REFERENCE_GRAVITY, flags)?;
        } else if joined {
            let (track, surfaces) = sources.as_ref().unwrap();
            let mut pair = [0; 2];
            for (i, slot) in pair.iter_mut().enumerate() {
                let off = body[(0x21cc + i * 4) / 4]
                    .checked_sub(address)
                    .and_then(|x| x.checked_sub(0x1990))
                    .ok_or("foreign steering wheel")?;
                if off % 0x150 != 0 || off / 0x150 >= 4 {
                    return Err("foreign steering wheel".into());
                }
                *slot = (off / 0x150) as usize;
            }
            let mut cache = grid_world::rectangle_cull::Cache::default();
            let apply = if outer {
                |frame: &mut Frame, pair, delta, query, material| {
                    grid_world::vehicle_step::apply(
                        frame,
                        pair,
                        delta,
                        REFERENCE_GRAVITY,
                        query,
                        material,
                    )
                }
            } else if driver {
                |frame: &mut Frame, pair, delta, query, material| {
                    grid_world::vehicle_contact_update::apply_driver(
                        frame,
                        pair,
                        delta,
                        REFERENCE_GRAVITY,
                        query,
                        material,
                    )
                }
            } else if chassis {
                grid_world::vehicle_contact_update::apply_with_chassis
            } else if post_force {
                grid_world::vehicle_contact_update::apply_with_post_force
            } else if vectors {
                grid_world::vehicle_contact_update::apply
            } else {
                grid_world::vehicle_contact_force::apply
            };
            let query = |_, ray: grid_world::contact_ray::Ray| {
                track
                    .query(ray.start, ray.end, &mut cache)
                    .map_err(|_| grid_world::Error("track query failed"))?
                    .hit
                    .map(|h| {
                        let (index, material) = surfaces
                            .contact(h.contact.material)
                            .map_err(|_| grid_world::Error("missing source mechanics"))?;
                        Ok(grid_world::suspension_prepare::Contact {
                            normal: h.contact.normal,
                            fraction: h.contact.fraction,
                            material,
                            tokens: [index as u32 + 1, 1, 1],
                        })
                    })
                    .transpose()
            };
            let resolved = if complete {
                let mut vehicle = frame.body.to_vec();
                vehicle.extend(
                    (RECORD..RECORD + 0x21000 - 0x2600)
                        .step_by(4)
                        .map(|o| word(record, o)),
                );
                let state = grid_world::vehicle_outer::State {
                    owner: grid_world::vehicle_publication::State {
                        vehicle,
                        physical: words(record, RECORD + 0x21000 - 0x2600),
                    },
                    frame,
                };
                let result = if handling_queries {
                    let calculate = if car_manager {
                        grid_world::vehicle_manager::calculate
                    } else {
                        grid_world::vehicle_job::calculate_with_queries
                    };
                    calculate(
                        &state,
                        pair,
                        delta,
                        REFERENCE_GRAVITY,
                        query_present.then_some(query_rows.as_slice()),
                        query,
                        |token| surfaces.joined(token),
                    )?
                } else {
                    let calculate = if car_job {
                        grid_world::vehicle_job::calculate
                    } else {
                        grid_world::vehicle_outer::calculate
                    };
                    calculate(&state, pair, delta, REFERENCE_GRAVITY, query, |token| {
                        surfaces.joined(token)
                    })?
                };
                frame = result.state.frame;
                put(record, RECORD, &result.state.owner.vehicle[0x2600 / 4..]);
                put(
                    record,
                    RECORD + 0x21000 - 0x2600,
                    &result.state.owner.physical,
                );
                result.materials
            } else {
                apply(&mut frame, pair, delta, query, |token| {
                    surfaces.joined(token)
                })?
            };
            for (i, m) in resolved.into_iter().enumerate() {
                record[20 + 4 * i..24 + 4 * i]
                    .copy_from_slice(&u32::from(m.is_some()).to_le_bytes());
                let base = 36 + 0x2600 + 0x300 + 0xdd0 + i * 0x140;
                record[base..base + 0x140].fill(0);
                if let Some(m) = m {
                    record[base + 0x75] = u8::from(m.high_downforce);
                    for (o, x) in [
                        (0x10, m.prepared.roughness),
                        (0x14, m.prepared.drag.minimum_speed),
                        (0x18, m.prepared.drag.maximum_speed),
                        (0x1c, m.prepared.drag.slowdown),
                        (0x20, m.prepared.roll),
                        (0x64, m.hardness),
                    ] {
                        put(record, base + o, &[x.to_bits()]);
                    }
                }
            }
        } else if propulsion {
            let roll = std::array::from_fn(|i| {
                materials[i].map(|_| {
                    f32::from_bits(word(record, 36 + 0x2600 + 0x300 + 0xdd0 + i * 0x140 + 0x20))
                })
            });
            if controls {
                let mut pair = [0; 2];
                for (i, wheel) in pair.iter_mut().enumerate() {
                    let offset = body[(0x21cc + i * 4) / 4]
                        .checked_sub(address)
                        .and_then(|v| v.checked_sub(0x1990))
                        .ok_or("foreign steering wheel")?;
                    if offset % 0x150 != 0 || offset / 0x150 >= 4 {
                        return Err("foreign steering wheel".into());
                    }
                    *wheel = (offset / 0x150) as usize;
                }
                if prepared {
                    let mechanics = std::array::from_fn(|i| {
                        materials[i].map(|_| {
                            let base = 36 + 0x2600 + 0x300 + 0xdd0 + i * 0x140;
                            let value = |o| f32::from_bits(word(record, base + o));
                            grid_world::vehicle_prepared_frame::Material {
                                drag: grid_world::surface_drag::Material {
                                    minimum_speed: value(0x14),
                                    maximum_speed: value(0x18),
                                    slowdown: value(0x1c),
                                    flag_73: record[base + 0x73] != 0,
                                },
                                roughness: value(0x10),
                                roll: value(0x20),
                            }
                        })
                    });
                    grid_world::vehicle_prepared_frame::apply(&mut frame, pair, mechanics, delta)?;
                } else {
                    grid_world::vehicle_controls::apply(&mut frame, pair, roll, delta)?;
                }
            } else {
                grid_world::vehicle_propulsion::apply(&mut frame, roll, delta)?;
            }
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
