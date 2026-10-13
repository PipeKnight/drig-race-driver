//! Shared private supplied-contact producer/callback diagnostics (ICW1/ICC1).
use grid_world::{
    deformation_impact::Node, deformation_settle::Edge, impact_generation::Contact,
    impact_producer as kernel,
};
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
fn put(data: &mut [u8], offset: usize, value: [f32; 4]) {
    for (i, v) in value.into_iter().enumerate() {
        data[offset + i * 4..offset + i * 4 + 4].copy_from_slice(&v.to_le_bytes());
    }
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
pub fn run(callback: bool) -> Result<(), Box<dyn std::error::Error>> {
    run_mode(callback, false, false)
}
pub fn run_serial() -> Result<(), Box<dyn std::error::Error>> {
    run_mode(true, true, false)
}
pub fn run_owned_static() -> Result<(), Box<dyn std::error::Error>> {
    run_mode(true, true, true)
}
fn write_consumption(
    output: &mut Vec<u8>,
    contacts: &[grid_world::response_consumption::Contact],
    pending: &[grid_world::response_consumption::Pending],
) {
    for contact in contacts {
        output.extend(contact.accumulated.to_le_bytes());
    }
    output.extend((pending.len() as u32).to_le_bytes());
    for pair in pending {
        output.extend((pair.contact - 1).to_le_bytes());
        output.extend(pair.value.to_le_bytes());
    }
}
fn run_mode(callback: bool, serial: bool, owned: bool) -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-impact-producer INPUT.bin local/OUTPUT.bin".into());
    }
    if fs::metadata(&args[0])?.len() > 64 * 1024 * 1024 {
        return Err("input exceeds limit".into());
    }
    let data = fs::read(&args[0])?;
    let mut cursor = 0;
    let header = take(&data, &mut cursor, 8)?;
    let cases = word(header, 4) as usize;
    let magic = if serial {
        0x31435349
    } else if callback {
        0x31434349
    } else {
        0x31574349
    };
    if word(header, 0) != magic || !(1..=1024).contains(&cases) {
        return Err("invalid input header".into());
    }
    let mut output = header.to_vec();
    for _ in 0..cases {
        let header = take(&data, &mut cursor, 12)?;
        let count = word(header, 0) as usize;
        if !(if callback { 0 } else { 1 }..=80).contains(&count) {
            return Err("contact count outside bound".into());
        }
        let old_linked = word(header, 8);
        let mut d = take(&data, &mut cursor, 0xa500)?.to_vec();
        let s = take(&data, &mut cursor, 0x1050)?;
        let mut linked = take(&data, &mut cursor, 0x424)?.to_vec();
        let body_offset = vector(take(&data, &mut cursor, 16)?, 0);
        let body_b20 = vector(take(&data, &mut cursor, 16)?, 0);
        let globals = take(&data, &mut cursor, 32)?;
        let corner_scale = number(take(&data, &mut cursor, 4)?, 0);
        let static_mass = number(take(&data, &mut cursor, 4)?, 0);
        let mut contacts = Vec::with_capacity(count);
        let mut roles = Vec::with_capacity(count);
        let mut contact_wire = Vec::with_capacity(count);
        for _ in 0..count {
            let c = take(&data, &mut cursor, 0x80)?;
            contact_wire.push(c.to_vec());
            let has_other = word(take(&data, &mut cursor, 4)?, 0);
            if has_other > 1 {
                return Err("invalid counterpart flag".into());
            }
            let other = vector(take(&data, &mut cursor, 16)?, 0);
            roles.push((c[0xc] == 0, has_other != 0));
            contacts.push(Contact {
                enabled: c[0x73] != 0,
                reverse: c[0xc] == 0 && word(c, 8) == old_linked,
                point: vector(c, 0x50),
                direction: vector(c, 0x60),
                value_38: number(c, 0x38),
                value_3c: number(c, 0x3c),
                value_40: number(c, 0x40),
                value_44: number(c, 0x44),
                counterpart_velocity: (has_other == 1).then_some(other),
            });
        }
        let center = vector(take(&data, &mut cursor, 16)?, 0);
        let history = take(&data, &mut cursor, 40)?;
        let history_extra = number(take(&data, &mut cursor, 4)?, 0);
        let mass = number(take(&data, &mut cursor, 4)?, 0);
        let body_globals = take(&data, &mut cursor, 20)?;
        let seed = word(take(&data, &mut cursor, 4)?, 0);
        let point = vector(take(&data, &mut cursor, 16)?, 0);
        let direction = vector(take(&data, &mut cursor, 16)?, 0);
        let scale = number(take(&data, &mut cursor, 4)?, 0);
        let solver_globals = take(&data, &mut cursor, 24)?;
        let clip = word(take(&data, &mut cursor, 4)?, 0);
        let present = word(take(&data, &mut cursor, 4)?, 0);
        if clip > 1 || present > 1 || linked[0x390] > 1 {
            return Err("invalid policy boolean".into());
        }
        let values: Vec<_> = (0..count)
            .map(|_| take(&data, &mut cursor, 4).map(|b| number(b, 0)))
            .collect::<Result<_, _>>()?;
        let preparation = if callback {
            let policy = take(&data, &mut cursor, 80)?;
            let disabled = word(take(&data, &mut cursor, 4)?, 0);
            if disabled > 1 || d[0xa4af] > 1 {
                return Err("invalid callback disable flag".into());
            }
            Some((
                disabled != 0,
                grid_world::contact_preparation::Policy {
                    body_factor_a: number(policy, 0),
                    body_factor_b: number(policy, 4),
                    global_factor: number(policy, 8),
                    linked_mass: number(&linked, 0x3f0),
                    scale: number(policy, 12),
                    table: std::array::from_fn(|i| number(policy, 16 + i * 4)),
                },
            ))
        } else {
            None
        };
        let mut consumed_contacts = Vec::new();
        let mut pending = Vec::new();
        let selected: Vec<_> = (0..count).collect();
        if serial {
            use grid_world::response_consumption::{Contact as ConsumedContact, Owner, Pending};
            if present != 1 || values.iter().any(|&v| v != 0.) {
                return Err("serial wrapper requires zero initialized response operands".into());
            }
            let pending_count = word(take(&data, &mut cursor, 4)?, 0) as usize;
            if pending_count > 240 {
                return Err("pending pairs exceed bound".into());
            }
            for (i, g) in contacts.iter().enumerate() {
                let registered = word(take(&data, &mut cursor, 4)?, 0);
                if registered > 1 || (registered != 0 && !roles[i].1) {
                    return Err("invalid other-owner registration".into());
                }
                let current = Some(Owner {
                    identity: 1,
                    registered: true,
                });
                let other = roles[i].1.then_some(Owner {
                    identity: 2 + i as u32,
                    registered: registered != 0,
                });
                let (first, second) = if g.reverse {
                    (other, current)
                } else {
                    (current, other)
                };
                consumed_contacts.push(ConsumedContact {
                    identity: 1 + i as u32,
                    first,
                    second,
                    second_is_body: roles[i].0,
                    accumulated: g.value_38,
                });
            }
            for _ in 0..pending_count {
                let pair = take(&data, &mut cursor, 8)?;
                let index = word(pair, 0) as usize;
                let value = number(pair, 4);
                if index >= count || !value.is_finite() {
                    return Err("invalid pending pair".into());
                }
                pending.push(Pending {
                    contact: 1 + index as u32,
                    value,
                });
            }
        }
        if owned {
            use grid_world::deformation_callback_state::{self, Parameters};
            if roles.iter().any(|&(two, other)| two || other) || !pending.is_empty() {
                return Err(
                    "owned diagnostic requires static-first contacts and empty pending".into(),
                );
            }
            let (global_disabled, policy) = preparation.ok_or("missing owned callback policy")?;
            let mut owner = vec![0u32; 0xd230 / 4];
            for (at, raw) in [(0x2300, d.as_slice()), (0x290, history)] {
                for (i, chunk) in raw.as_chunks::<4>().0.iter().enumerate() {
                    owner[at / 4 + i] = u32::from_le_bytes(*chunk);
                }
            }
            owner[0x1630 / 4..0x1640 / 4].copy_from_slice(&body_offset.map(f32::to_bits));
            owner[0xb20 / 4..0xb30 / 4].copy_from_slice(&body_b20.map(f32::to_bits));
            owner[0xc7f8 / 4] = 0;
            owner[0x2e0 / 4] = history_extra.to_bits();
            owner[0x15a8 / 4] = mass.to_bits();
            owner[0xd16c / 4] = policy.body_factor_a.to_bits();
            owner[0xd22c / 4] = policy.body_factor_b.to_bits();
            let settings: Vec<_> = s
                .as_chunks::<4>()
                .0
                .iter()
                .map(|c| u32::from_le_bytes(*c))
                .collect();
            let mut physical: [u32; 0x424 / 4] = std::array::from_fn(|i| word(&linked, i * 4));
            let original_center: [u32; 4] = physical[0x1c0 / 4..0x1d0 / 4].try_into().unwrap();
            let original_pointer = physical[0x334 / 4];
            physical[0x334 / 4] = 0x1c0;
            physical[0x1c0 / 4..0x1d0 / 4].copy_from_slice(&center.map(f32::to_bits));
            for at in [0x418, 0x41c, 0x420] {
                physical[at / 4] = 0;
            }
            let wire: Vec<[u32; 32]> = contact_wire
                .iter()
                .map(|c| {
                    let mut r = std::array::from_fn(|i| word(c, i * 4));
                    r[1] = 0;
                    r[2] = 0;
                    r
                })
                .collect();
            let result = deformation_callback_state::calculate(
                &owner,
                &physical,
                &wire,
                &settings,
                number(header, 4),
                &Parameters {
                    generation: [
                        corner_scale,
                        static_mass,
                        number(globals, 8),
                        number(globals, 16),
                        number(globals, 20),
                        number(globals, 12),
                    ],
                    body: std::array::from_fn(|i| number(body_globals, i * 4)),
                    grid: [
                        number(solver_globals, 8),
                        number(solver_globals, 16),
                        number(solver_globals, 20),
                    ],
                    grid_iterations: word(solver_globals, 12),
                    clip_each_iteration: clip != 0,
                    response_scale: scale,
                    correction_gain: number(globals, 0),
                    subtract_baseline: globals[4] != 0,
                    policy_global_factor: policy.global_factor,
                    policy_scale: policy.scale,
                    policy_table: policy.table,
                    global_disabled,
                },
                seed,
            )?;
            let mut physical = result.physical;
            physical[0x334 / 4] = original_pointer;
            physical[0x1c0 / 4..0x1d0 / 4].copy_from_slice(&original_center);
            for at in [0x418, 0x41c, 0x420] {
                physical[at / 4] = word(&linked, at);
            }
            for value in result.owner[0x2300 / 4..0xc800 / 4]
                .iter()
                .chain(&physical)
                .chain(&result.owner[0x290 / 4..0x2b8 / 4])
            {
                output.extend(value.to_le_bytes());
            }
            output.extend(result.owner[0x2e0 / 4].to_le_bytes());
            output.extend(result.random_seed.to_le_bytes());
            for c in result.contacts {
                output.extend(c[0x38 / 4].to_le_bytes());
            }
            output.extend(0u32.to_le_bytes());
            continue;
        }
        let edge_count = word(&d, 0xa410) as usize;
        if edge_count > 1200 {
            return Err("edge count exceeds bound".into());
        }
        let edges: Vec<_> = (0..edge_count)
            .map(|i| Edge {
                endpoints: [
                    u16::from_le_bytes(d[i * 8..i * 8 + 2].try_into().unwrap()) as usize,
                    u16::from_le_bytes(d[i * 8 + 2..i * 8 + 4].try_into().unwrap()) as usize,
                ],
                length: number(&d, i * 8 + 4),
            })
            .collect();
        let nodes: Vec<_> = (0..400)
            .map(|i| {
                let offset = 0x2580 + i * 80;
                Node {
                    position: vector(&d, offset),
                    rest: vector(&d, offset + 16),
                    bounds: vector(&d, offset + 32),
                    motion: vector(&d, offset + 48),
                    marker: number(&d, offset + 64),
                }
            })
            .collect();
        let input = kernel::Input {
            grid: kernel::Grid {
                dimensions: std::array::from_fn(|i| word(&d, 0xa414 + i * 4) as usize),
                spacing: number(&d, 0xa420),
                minimum: vector(&d, 0xa430),
                state: number(&d, 0xa480),
                corners: vector(&d, 0xa4b4),
                window: std::array::from_fn(|i| number(&d, 0xa4c4 + i * 4)),
                flags: &d[0xa280..0xa410],
                dirty: d[0xa4f0..0xa4f5].try_into().unwrap(),
                nodes: &nodes,
                edges: &edges,
                mode: word(&d, 0xa4a0),
                strain_threshold: number(s, 0x1024),
                strain_gain: number(s, 0x1028),
                displacement_threshold: number(s, 0x1048),
                integration_step: number(solver_globals, 8),
                damping: number(solver_globals, 16),
                motion_scale: number(solver_globals, 20),
                iterations: word(solver_globals, 12),
                counter: word(&d, 0xa450),
                clip_each_iteration: clip != 0,
                offset: vector(&d, 0xa440),
            },
            generation: kernel::Generation {
                pose: std::array::from_fn(|i| vector(&linked, 0x1d0 + i * 16)),
                body_offset,
                body_b20,
                velocity: vector(&linked, 0x360),
                supplied_scalar: number(header, 4),
                corner_scale,
                static_mass,
                limit_scale: number(globals, 8),
                flag_radius_scale: number(globals, 16),
                record_radius_scale: number(globals, 20),
                radial_scale: number(globals, 12),
                damage_threshold: number(s, 0x1030),
                damage_gain: number(s, 0x1034),
                projection: number(s, 0x1038),
                contacts: &contacts,
            },
            body: kernel::Body {
                angular: vector(&linked, 0x210),
                linear: vector(&linked, 0x220),
                point,
                direction,
                center,
                history: std::array::from_fn(|i| number(history, i * 4)),
                history_extra,
                mass,
                gain: number(body_globals, 0),
                random_gain: number(body_globals, 4),
                history_range: number(body_globals, 8),
                history_threshold: number(body_globals, 12),
                speed_threshold: number(body_globals, 16),
                random_seed: seed,
            },
            response: kernel::Response {
                values: (present != 0).then_some(values.as_slice()),
                accumulator: vector(&linked, 0x380),
                dirty: linked[0x390] != 0,
                scale,
                correction_gain: number(globals, 0),
                subtract_baseline: globals[4] != 0,
            },
        };
        let mut consumption = None;
        let result = if let Some((disabled, policy)) = preparation {
            let callback_input = grid_world::collision_callback::Input {
                producer: input,
                intensity: number(&d, 0xa494),
                descriptor_disabled: d[0xa4af] != 0,
                global_disabled: disabled,
                policy,
            };
            let result = if serial {
                let result =
                    grid_world::serial_callback::calculate(grid_world::serial_callback::Input {
                        callback: callback_input,
                        owner: 1,
                        contacts: &consumed_contacts,
                        selected: &selected,
                        pending: &pending,
                    })?;
                consumption = result.consumption;
                result.callback
            } else {
                grid_world::collision_callback::calculate(callback_input)?
            };
            if let Some(active) = result.activation {
                d[0xa4a0..0xa4a4].copy_from_slice(&1u32.to_le_bytes());
                d[0xa454..0xa458].copy_from_slice(&active.strength.to_le_bytes());
                put(&mut d, 0xa460, active.point);
                put(&mut d, 0xa470, active.direction);
            }
            if let Some(producer) = result.producer {
                producer
            } else {
                d[0xa4f8..0xa4fc].fill(0);
                output.extend(d);
                output.extend(linked);
                output.extend(history);
                output.extend(history_extra.to_le_bytes());
                output.extend(seed.to_le_bytes());
                if serial {
                    write_consumption(&mut output, &consumed_contacts, &pending);
                } else {
                    for v in values {
                        output.extend(v.to_le_bytes());
                    }
                    output.extend(0u32.to_le_bytes());
                }
                continue;
            }
        } else {
            kernel::calculate(input)?
        };
        let generated = result.generation;
        d[0xa280..0xa410].copy_from_slice(&generated.flags);
        d[0xa4f0..0xa4f5].copy_from_slice(&generated.dirty);
        d[0xa480..0xa484].copy_from_slice(&generated.state.to_le_bytes());
        for (i, node) in result.grid.nodes.iter().enumerate() {
            put(&mut d, 0x2580 + i * 80, node.position);
            put(&mut d, 0x25b0 + i * 80, node.motion);
        }
        for (i, length) in result.grid.lengths.iter().enumerate() {
            d[i * 8 + 4..i * 8 + 8].copy_from_slice(&length.to_le_bytes());
        }
        put(&mut d, 0xa440, result.grid.offset);
        d[0xa450..0xa454].copy_from_slice(&result.grid.counter.to_le_bytes());
        d[0xa4f0] = u8::from(result.grid.dirty);
        d[0xa4f8..0xa4fc].fill(0);
        put(&mut linked, 0x210, result.body.angular);
        put(&mut linked, 0x220, result.body.linear);
        put(&mut linked, 0x380, result.response.accumulator);
        linked[0x390] = u8::from(result.response.dirty);
        output.extend(d);
        output.extend(linked);
        for v in result.body.history {
            output.extend(v.to_le_bytes());
        }
        output.extend(result.body.history_extra.to_le_bytes());
        output.extend(result.body.random_seed.to_le_bytes());
        if serial {
            let consumed = consumption.ok_or("serial callback missing consumption")?;
            write_consumption(&mut output, &consumed.contacts, &consumed.pending);
        } else {
            for v in result.response.response.unwrap_or(values) {
                output.extend(v.to_le_bytes());
            }
            if callback {
                output.extend(1u32.to_le_bytes());
            }
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
    println!(
        "{}_cases={cases}",
        if serial {
            "serial_callback"
        } else if callback {
            "collision_callback"
        } else {
            "impact_producer"
        }
    );
    Ok(())
}
