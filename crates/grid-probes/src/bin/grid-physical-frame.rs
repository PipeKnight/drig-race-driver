//! Private whole no-callback physical frame diagnostic, PFR1.
use grid_world::{
    contact_rows, impact_frame,
    impact_step::{Body, Input},
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
pub(crate) fn run(deformation: bool, active: bool) -> Result<(), Failure> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-physical-frame INPUT.bin local/OUTPUT.bin".into());
    }
    if fs::metadata(&args[0])?.len() > 32 * 1024 * 1024 {
        return Err("input exceeds bound".into());
    }
    let data = fs::read(&args[0])?;
    let mut p = 0;
    let h = take(&data, &mut p, 8)?;
    let count = word(h, 4);
    if word(h, 0)
        != if active {
            0x33444650
        } else if deformation {
            0x31444650
        } else {
            0x31524650
        }
        || !(1..=1024).contains(&count)
    {
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
        let parameters: [f32; 11] = std::array::from_fn(|i| number(h, 4 * i));
        let gravity = std::array::from_fn(|i| parameters[i + 1]);
        let step_parameters = [
            parameters[0],
            1.,
            1.,
            1.,
            1.,
            parameters[5],
            parameters[6],
            parameters[7],
            parameters[8],
            parameters[9],
            parameters[10],
        ];
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
        let mut settings = Vec::new();
        let mut body_wire = Vec::new();
        let mut workspace = Vec::new();
        for _ in 0..nb {
            let b = take(&data, &mut p, 0x424)?;
            if word(b, 0x334) != 0x1c0 || (0x40c..=0x410).any(|o| b[o] > 1) {
                return Err("invalid body framing".into());
            }
            let pointers = [
                0x2fc, 0x320, 0x324, 0x328, 0x32c, 0x338, 0x33c, 0x340, 0x418, 0x41c, 0x420,
            ];
            if pointers.iter().any(|&o| word(b, o) != 0) || word(b, 0x400) != 0 || b[0x390] > 1 {
                return Err("unsupported body links/wakeup/callbacks".into());
            }
            settings.push(impact_frame::Body {
                basis: matrix(b, 0x190),
                local_matrices: [matrix(b, 0x90), matrix(b, 0x150)],
                offset: vector(b, 0x230),
                sleep: std::array::from_fn(|i| number(b, 0x3a0 + 4 * i)),
                limits: vector(b, 0x3b8),
                damping: vector(b, 0x3c8),
                drag: std::array::from_fn(|i| number(b, 0x3d8 + 4 * i)),
                mass: number(b, 0x3f0),
                gravity_scale: number(b, 0x3f8),
                pending_shift: vector(b, 0x380),
                shift_active: b[0x390] != 0,
            });
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
            if word(c, 0x10) != 0
                || if active && mode == 0 {
                    c[0xc] > 1 || word(c, 8) != 0
                } else {
                    c[0xc] != if mode != 0 { 0 } else { 1 }
                }
                || c[0xd] != 0
                || c[0xf] != 1
                || mode > 1
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
        let mut input = impact_frame::Input {
            correction: None,
            park: None,
            gravity,
            bodies: settings,
            step: Input {
                parameters: step_parameters,
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
                joints: Default::default(),
            },
        };
        let mut descriptor = None;
        let mut deformation_seed = None;
        let frame = if deformation {
            use grid_world::{
                deformation_contact_limit::Parameters, deformation_layout::WORDS,
                deformation_physical_frame,
            };
            let raw = take(&data, &mut p, WORDS * 4)?;
            let d: Vec<_> = (0..WORDS).map(|i| word(raw, i * 4)).collect();
            let raw = take(&data, &mut p, 96)?;
            let f = |i| number(raw, i * 4);
            if nb != 1 || word(raw, 92) > 1 {
                return Err("invalid deformation frame roles".into());
            }
            let body = std::array::from_fn(|i| word(&body_wire[0], i * 4));
            let contacts: Vec<_> = contact_wire
                .iter()
                .map(|c| std::array::from_fn(|i| word(c, i * 4)))
                .collect();
            let limit = Parameters {
                low_mass_threshold: f(0),
                low_mass_speed_cap: f(1),
                corner_scale: f(2),
                velocity_projection: f(3),
                stiffness_scale: f(4),
                unlimited: f(5),
                damage_scale: f(6),
                table: std::array::from_fn(|i| f(7 + i)),
                global_disabled: word(raw, 92) != 0,
            };
            if active {
                use grid_world::{
                    deformation_callback_state::Parameters as Bulk, deforming_physical_frame,
                };
                let owner_raw = take(&data, &mut p, 0xd230)?;
                let owner: Vec<_> = owner_raw
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|c| u32::from_le_bytes(*c))
                    .collect();
                if owner[0x2300 / 4..0xc800 / 4] != d {
                    return Err("active owner descriptor mismatch".into());
                }
                let settings_raw = take(&data, &mut p, 0x1054)?;
                let settings: Vec<_> = settings_raw
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|c| u32::from_le_bytes(*c))
                    .collect();
                let raw = take(&data, &mut p, 156)?;
                let f = |i| number(raw, i * 4);
                if [60, 72, 148].iter().any(|at| word(raw, *at) > 1) {
                    return Err("invalid active deformation booleans".into());
                }
                let bulk = Bulk {
                    generation: std::array::from_fn(&f),
                    body: std::array::from_fn(|i| f(6 + i)),
                    grid: std::array::from_fn(|i| f(11 + i)),
                    grid_iterations: word(raw, 56),
                    clip_each_iteration: word(raw, 60) != 0,
                    response_scale: f(16),
                    correction_gain: f(17),
                    subtract_baseline: word(raw, 72) != 0,
                    policy_global_factor: f(19),
                    policy_scale: f(20),
                    policy_table: std::array::from_fn(|i| f(21 + i)),
                    global_disabled: word(raw, 148) != 0,
                };
                let random_seed = word(raw, 152);
                let correction = take(&data, &mut p, 20)?;
                input.correction = Some(impact_frame::Correction {
                    iterations: word(correction, 0) as usize,
                    softness: number(correction, 4),
                    scale: number(correction, 8),
                    threshold: number(correction, 12),
                    relaxation: number(correction, 16),
                });
                let result =
                    deforming_physical_frame::calculate(deforming_physical_frame::Input {
                        frame: &input,
                        owner: &owner,
                        physical: &body,
                        contacts: &contacts,
                        settings: &settings,
                        shapes: &[
                            grid_world::contact_groups::Shape::default(),
                            grid_world::contact_groups::Shape {
                                angular: true,
                                ..Default::default()
                            },
                        ],
                        limit,
                        bulk: &bulk,
                        random_seed,
                    })?;
                descriptor = Some(result.owner);
                deformation_seed = Some(result.random_seed);
                for (wire, r) in contact_wire.iter_mut().zip(result.contacts) {
                    for at in [0x38, 0x3c, 0x40, 0x44] {
                        put(wire, at, r[at / 4]);
                    }
                }
                result.frame
            } else {
                let result = deformation_physical_frame::calculate(
                    &input,
                    &d,
                    &body,
                    &contacts,
                    limit,
                    &[
                        grid_world::contact_groups::Shape {
                            flag230: true,
                            ..Default::default()
                        },
                        grid_world::contact_groups::Shape {
                            angular: true,
                            ..Default::default()
                        },
                    ],
                )?;
                descriptor = Some(result.descriptor);
                for (wire, raw) in contact_wire.iter_mut().zip(result.prepared_contacts) {
                    for at in [0x3c, 0x40, 0x44] {
                        put(wire, at, raw[at / 4]);
                    }
                }
                result.frame
            }
        } else {
            impact_frame::calculate(&input)?
        };
        let started = frame.started;
        output.extend(frame.status.to_le_bytes());
        output.extend(u32::from(started).to_le_bytes());
        let o = frame.step;
        for e in o.ends {
            output.extend((e as u32).to_le_bytes());
        }
        for i in o.cached_pair.map_or([u32::MAX; 2], |p| p.map(|i| i as u32)) {
            output.extend(i.to_le_bytes());
        }
        for f in o.cached_material {
            output.extend(f.to_le_bytes());
        }
        for b in &o.bodies {
            output.extend(b.inverse_mass.to_le_bytes());
            for f in b.inverse_inertia.iter().flatten() {
                output.extend(f.to_le_bytes());
            }
        }
        for f in o.workspace.iter().flatten().flatten() {
            output.extend(f.to_le_bytes());
        }
        for (i, (mut raw, b)) in body_wire.into_iter().zip(o.bodies).enumerate() {
            store(&mut raw, 0x210, b.contact.angular);
            store(&mut raw, 0x220, b.contact.linear);
            store(&mut raw, 0x240, b.impulse_accumulator);
            raw[0x40c] = u8::from(b.impulse_active);
            store(&mut raw, 0x250, b.torque);
            store(&mut raw, 0x260, b.force);
            raw[0x40d] = u8::from(b.apply_torque);
            raw[0x40e] = u8::from(b.apply_force);
            let settings = &frame.bodies[i];
            put(&mut raw, 0x3a0, settings.sleep[0].to_bits());
            store(&mut raw, 0x380, settings.pending_shift);
            raw[0x390] = u8::from(settings.shift_active);
            if let Some(publication) = &frame.publications[i] {
                for (offset, m) in [
                    (0x190, publication.corrected),
                    (0x1d0, publication.published),
                    (0xd0, publication.derived[0]),
                    (0x110, publication.derived[1]),
                ] {
                    for (r, v) in m.into_iter().enumerate() {
                        store(&mut raw, offset + 16 * r, v);
                    }
                }
            }
            output.extend(raw);
        }
        for ((mut raw, c), metadata) in contact_wire.into_iter().zip(o.contacts).zip(o.metadata) {
            if started {
                put(&mut raw, 0x14, metadata[0]);
                put(&mut raw, 0x18, metadata[1]);
                raw[0x70] = u8::from(c.group_end);
                put(&mut raw, 0x3c, c.previous_impulse.to_bits());
            }
            output.extend(raw);
        }
        for w in o.rows.into_iter().flatten() {
            output.extend(w.to_le_bytes());
        }
        if let Some(d) = descriptor {
            for v in d {
                output.extend(v.to_le_bytes());
            }
        }
        if let Some(seed) = deformation_seed {
            output.extend(seed.to_le_bytes());
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
    println!("physical_frame_cases={count}");
    Ok(())
}
fn main() {
    if let Err(error) = run(false, false) {
        eprintln!("grid-physical-frame: {error}");
        std::process::exit(1);
    }
}
