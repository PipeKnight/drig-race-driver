//! Private active-friction comparison with neutral cross-wheel responses.
use grid_world::{suspension, tyre_friction, wheel_step};
use std::{fs::OpenOptions, io::Write, path::Path};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-tyre-friction INPUT.tsv local/OUTPUT.tsv".into());
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
            || fields.len() != 44
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
                spring: 0.,
                preload: 0.,
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
        let wheels =
            wheel_step::step(wheels, [[0.; 4]; 4], [[0.; 4]; 4], 1., values[0], values[1])?;
        report += &format!("P\t{}", fields[0]);
        for (w, wheel) in wheels.iter().enumerate() {
            let v = &values[3 + w * 10..3 + (w + 1) * 10];
            let result = tyre_friction::resolve(tyre_friction::Input {
                normal_impulse: wheel.contact_raw,
                radius: v[1],
                spin: v[2],
                inertia: v[3],
                normal_scale: v[4],
                lateral_denominator: v[5],
                lateral: v[6],
                displacement: v[7],
                rate: v[8],
                strength: v[9],
                slide: values[2],
                delta: values[0],
            })?;
            for value in [
                wheel.contact_raw,
                wheel.floor,
                wheel.suspension.speed,
                wheel.suspension.displacement,
                result.displacement,
                0. + result.first_impulse,
                result.first_impulse * values[1] + 0.,
                0. + result.second_impulse,
                result.second_impulse * values[1] + 0.,
                result.angular_delta + v[2],
                v[8] - result.angular_delta,
            ] {
                report += &format!("\t{:08x}", value.to_bits());
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
