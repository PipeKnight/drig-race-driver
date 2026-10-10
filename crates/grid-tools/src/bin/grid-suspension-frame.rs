//! Private comparison of joined suspension frames with supplied geometry.
use grid_world::{
    dynamics::{BodyState, ImpulseBody},
    suspension,
    suspension_frame::{self, Input, Wheel},
    wheel_step,
};
use std::{fs::OpenOptions, io::Write, path::Path};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-suspension-frame INPUT.tsv local/OUTPUT.tsv".into());
    }
    let input = Path::new(&args[0]);
    if input.metadata()?.len() > 1024 * 1024 {
        return Err("input exceeds limit".into());
    }
    let output = Path::new(&args[1]);
    let parent = output
        .parent()
        .ok_or("missing output parent")?
        .canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("private output only".into());
    }
    let mut report = String::new();
    let mut count = 0;
    let mut total_frames = 0;
    for line in std::fs::read_to_string(input)?.lines() {
        count += 1;
        let cells: Vec<_> = line.split_whitespace().collect();
        if count > 256
            || cells.len() != 183
            || cells[0].len() > 63
            || !cells[0]
                .bytes()
                .all(|v| v.is_ascii_alphanumeric() || v == b'_')
        {
            return Err("invalid case framing".into());
        }
        let raw: Vec<u32> = cells[1..]
            .iter()
            .map(|v| {
                if v.len() != 8 || !v.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err("invalid word");
                }
                u32::from_str_radix(v, 16).map_err(|_| "invalid word")
            })
            .collect::<Result<_, _>>()?;
        let frames = raw[0];
        if !(1..=120).contains(&frames) {
            return Err("invalid frame count".into());
        }
        let v: Vec<f32> = raw[1..].iter().map(|v| f32::from_bits(*v)).collect();
        if v.iter().any(|v| !v.is_finite()) {
            return Err("nonfinite input".into());
        }
        let body = ImpulseBody {
            mass: v[0],
            inertia: v[1..4].try_into()?,
            basis: std::array::from_fn(|i| v[4 + i * 4..8 + i * 4].try_into().expect("basis row")),
        };
        let mut wheels = Vec::new();
        for i in 0..4 {
            let w = &v[41 + i * 35..76 + i * 35];
            if w[34] != 0. && w[34] != 1. {
                return Err("invalid contact flag".into());
            }
            wheels.push(Wheel {
                point: w[0..4].try_into()?,
                projection_axis: w[8..12].try_into()?,
                clip_upper: w[32],
                force: w[33],
                step: wheel_step::Wheel {
                    suspension: suspension::Wheel {
                        displacement: w[12],
                        speed: w[13],
                        spring: w[14],
                        preload: w[15],
                        travel_denominator: w[16],
                        damper: w[17],
                        speed_threshold: w[18],
                        high_speed_ratio: w[19],
                        negative_speed_scale: w[20],
                        bump: w[21],
                        axle_coupling: w[22],
                    },
                    axis: w[4..8].try_into()?,
                    body_speed: 0.,
                    body_rate: w[23],
                    floor: w[24],
                    contact_speed: w[25],
                    raw: w[26],
                    weighted: w[27],
                    contact_raw: w[28],
                    contact_weighted: w[29],
                    lower: w[30],
                    upper: w[31],
                    contact: w[34] == 1.,
                },
            });
        }
        let mut state = Input {
            body,
            state: BodyState {
                linear: v[20..24].try_into()?,
                angular: v[24..28].try_into()?,
                position: body.basis[3],
            },
            wheels: wheels.try_into().expect("four wheels"),
            delta: v[28],
            scale: v[29],
            alternate_inertia: v[30..33].try_into()?,
            linear_rate: v[33..37].try_into()?,
            angular_rate: v[37..41].try_into()?,
        };
        for frame in 0..frames {
            let out = suspension_frame::calculate(&state)?;
            report += &format!(
                "P\t{}\t{frame}\t{}\t{:08x}\t{:08x}",
                cells[0],
                out.schedule.count,
                out.schedule.delta.to_bits(),
                out.last_weight.to_bits()
            );
            let values = out
                .coupling
                .speeds
                .into_iter()
                .chain(out.coupling.response.into_iter().flatten())
                .chain(out.coupling.secondary.into_iter().flatten())
                .chain(out.accumulated.into_iter().flat_map(|w| {
                    [
                        w.suspension.displacement,
                        w.suspension.speed,
                        w.body_speed,
                        w.floor,
                        w.contact_speed,
                        w.raw,
                        w.weighted,
                        w.contact_raw,
                        w.contact_weighted,
                        if w.contact { 1. } else { 0. },
                    ]
                }))
                .chain([out.finalized.body.mass])
                .chain(out.finalized.body.inertia)
                .chain(out.finalized.body.basis.into_iter().flatten())
                .chain(out.finalized.state.linear)
                .chain(out.finalized.state.angular)
                .chain(out.finalized.wheels.into_iter().flat_map(|w| {
                    [w.displacement, w.speed, w.floor_difference, w.force]
                        .into_iter()
                        .chain(w.remaining)
                }))
                .chain([out.finalized.clock]);
            for value in values {
                report += &format!("\t{:08x}", value.to_bits());
            }
            report.push('\n');
            total_frames += 1;
            state = suspension_frame::carry(&state, &out);
        }
    }
    if count == 0 {
        return Err("no cases".into());
    }
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(parent.join(output.file_name().ok_or("missing output name")?))?
        .write_all(report.as_bytes())?;
    println!("suspension_frame_cases={count} frames={total_frames}");
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("grid-suspension-frame: {error}");
        std::process::exit(1);
    }
}
