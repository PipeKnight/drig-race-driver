//! Private active-friction comparison with neutral cross-wheel responses.
use grid_world::{suspension, tyre_friction, tyre_response, wheel_rotation, wheel_step};
use std::{fs::OpenOptions, io::Write, path::Path};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let drive = args.get(2).is_some_and(|v| v == "drive");
    let coupled = drive || args.get(2).is_some_and(|v| v == "coupled");
    if args.len() != 2 && !(args.len() == 3 && coupled) {
        return Err("usage: grid-tyre-friction INPUT.tsv local/OUTPUT.tsv [coupled|drive]".into());
    }
    let input = Path::new(&args[0]);
    if input.metadata()?.len() > 1024 * 1024 {
        return Err("input bound".into());
    }
    let mut report = String::new();
    let mut count = 0;
    for line in std::fs::read_to_string(input)?.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        count += 1;
        if count > 4096
            || fields.len()
                != if drive {
                    226
                } else if coupled {
                    208
                } else {
                    44
                }
            || fields[0].len() > 63
            || !fields[0]
                .bytes()
                .all(|v| v.is_ascii_alphanumeric() || v == b'_')
        {
            return Err("invalid framing".into());
        }
        let values: Vec<f32> = fields[1..]
            .iter()
            .map(|v| {
                if v.len() != 8 || !v.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err("invalid word");
                }
                Ok(f32::from_bits(
                    u32::from_str_radix(v, 16).map_err(|_| "invalid word")?,
                ))
            })
            .collect::<Result<_, _>>()?;
        let wheels: [wheel_step::Wheel; 4] = std::array::from_fn(|w| wheel_step::Wheel {
            suspension: suspension::Wheel {
                displacement: 0.,
                speed: 0.,
                spring: if coupled { 1. } else { 0. },
                preload: if coupled { values[43 + w] } else { 0. },
                travel_denominator: 1.,
                damper: 0.,
                speed_threshold: 1.,
                high_speed_ratio: 1.,
                negative_speed_scale: 1.,
                bump: 0.,
                axle_coupling: 0.,
            },
            axis: [0., 1., 0., 0.],
            body_speed: 0.,
            body_rate: 0.,
            floor: 0.,
            contact_speed: values[3 + w * 10],
            raw: 0.,
            weighted: 0.,
            contact_raw: 0.,
            contact_weighted: 0.,
            contact: false,
            lower: -98.,
            upper: 98.,
        });
        let response = if coupled {
            std::array::from_fn(|s| {
                std::array::from_fn(|t| {
                    std::array::from_fn(|w| {
                        std::array::from_fn(|j| values[47 + (s * 3 + t) * 16 + w * 4 + j])
                    })
                })
            })
        } else {
            [[[[0.; 4]; 4]; 3]; 3]
        };
        let secondary = if coupled {
            std::array::from_fn(|w| std::array::from_fn(|j| values[191 + w * 4 + j]))
        } else {
            [[0.; 4]; 4]
        };
        let suspension_impulses =
            suspension::impulses(wheels.map(|w| w.suspension), 1., values[0])?;
        let wheels = wheel_step::step(wheels, response[0][0], secondary, 1., values[0], values[1])?;
        let rotation = std::array::from_fn(|w| wheel_rotation::Wheel {
            spin: values[5 + w * 10],
            rate: values[11 + w * 10],
            inertia: values[6 + w * 10],
            drive_delta: if drive { values[207 + w] } else { 0. },
            brake_delta: if drive { values[211 + w] } else { 0. },
        });
        let rotation = if drive {
            wheel_rotation::prepare(
                rotation,
                std::array::from_fn(|e| wheel_rotation::Exchange {
                    lower: values[215 + e * 3],
                    upper: values[216 + e * 3],
                    strength: values[217 + e * 3],
                }),
                values[224],
                values[0],
            )?
        } else {
            rotation
        };
        let prepared: [tyre_response::Wheel; 4] = std::array::from_fn(|w| {
            let v = &values[3 + w * 10..3 + (w + 1) * 10];
            tyre_response::Wheel {
                normal: wheels[w],
                raw: [0.; 2],
                weighted: [0.; 2],
                tyre: tyre_friction::Input {
                    normal_impulse: wheels[w].contact_raw,
                    radius: v[1],
                    spin: rotation[w].spin,
                    inertia: v[3],
                    normal_scale: v[4],
                    lateral_denominator: v[5],
                    lateral: v[6],
                    displacement: v[7],
                    rate: rotation[w].rate,
                    strength: v[9],
                    slide: values[2],
                    delta: values[0],
                },
            }
        });
        let resolved = tyre_response::apply(prepared, response, suspension_impulses, values[1])?;
        report += &format!("P\t{}", fields[0]);
        for result in resolved {
            let wheel = result.normal;
            for value in [
                wheel.contact_raw,
                wheel.floor,
                wheel.suspension.speed,
                wheel.suspension.displacement,
                result.tyre.displacement,
                result.raw[0],
                result.weighted[0],
                result.raw[1],
                result.weighted[1],
                result.tyre.spin,
                result.tyre.rate,
            ] {
                report += &format!("\t{:08x}", value.to_bits());
            }
            if coupled {
                for value in [wheel.body_speed, result.tyre.lateral] {
                    report += &format!("\t{:08x}", value.to_bits());
                }
            }
        }
        report.push('\n');
    }
    if count == 0 {
        return Err("no cases".into());
    }
    let output = Path::new(&args[1]);
    let parent = output.parent().ok_or("missing parent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("private output only".into());
    }
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(parent.join(output.file_name().ok_or("missing name")?))?
        .write_all(report.as_bytes())?;
    println!("tyre_friction_cases={count}");
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-tyre-friction: {e}");
        std::process::exit(1);
    }
}
