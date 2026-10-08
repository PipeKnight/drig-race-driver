//! Private whole assisted four-wheel coefficient diagnostic.
use grid_world::{
    assisted_steering::{self, Motion, Parameters},
    steering_coefficients::{Input, Wheel},
    steering_input,
};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
fn flag(v: f32) -> Result<bool, &'static str> {
    match v {
        0. => Ok(false),
        1. => Ok(true),
        _ => Err("invalid flag"),
    }
}
fn ordinal(v: f32) -> Result<usize, &'static str> {
    match v {
        0. => Ok(0),
        1. => Ok(1),
        2. => Ok(2),
        3. => Ok(3),
        _ => Err("invalid ordinal"),
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let wheel_source_mode = args.len() == 5 && args[2] == "--source-wheel-geometry";
    let geometry_mode = (args.len() == 5 && args[2] == "--source-geometry") || wheel_source_mode;
    if args.len() != 2
        && !(args.len() == 4 && (args[2] == "--source" || args[2] == "--source-parameters"))
        && !geometry_mode
    {
        return Err(
            "usage: grid-assisted-steering INPUT.tsv local/OUTPUT.tsv [--source|--source-parameters CAR.csv | --source-geometry|--source-wheel-geometry CAR.csv CAR.ned]".into(),
        );
    }
    let source = if args.len() == 4 || geometry_mode {
        let bytes = fs::read(&args[3])?;
        let table = grid_formats::tuning::read(&bytes)?;
        if table.rows.len() != 1 {
            return Err("one source row required".into());
        }
        Some((
            grid_formats::vehicle::read_standard_steering_angle(&table.rows[0])?,
            grid_formats::vehicle::read_standard_steering_parameters(&table.rows[0])?,
            if geometry_mode {
                let bytes = fs::read(&args[4])?;
                let document = grid_formats::binxml::read(&bytes)?;
                let geometry = grid_formats::vehicle::read_wheel_geometry(&document)?;
                Some(
                    grid_formats::vehicle::standard_wheel_layout(&table.rows[0], geometry)?
                        .positions,
                )
            } else {
                None
            },
            if wheel_source_mode {
                Some(grid_formats::vehicle::read_standard_wheel_steering(
                    &table.rows[0],
                )?)
            } else {
                None
            },
        ))
    } else {
        None
    };
    let text = fs::read_to_string(&args[0])?;
    if text.len() > 4 * 1024 * 1024 {
        return Err("input exceeds limit".into());
    }
    let mut rows = Vec::new();
    for line in text.lines() {
        let cells: Vec<_> = line.split('\t').collect();
        if cells.len() != 110 || rows.len() >= 4096 {
            return Err("invalid input row".into());
        }
        let mut v: Vec<_> = cells[1..]
            .iter()
            .map(|v| u32::from_str_radix(v, 16).map(f32::from_bits))
            .collect::<Result<_, _>>()?;
        let mut geometry_words = String::new();
        if let Some((angle, parameters, points, wheel_source)) = source {
            v[4] = angle;
            if args[2] == "--source-parameters" || geometry_mode {
                v[85..105].copy_from_slice(&parameters);
            } else {
                for i in [5, 6, 7, 19] {
                    v[85 + i] = parameters[i];
                }
            }
            if let Some(points) = points {
                let g = grid_world::steering_geometry::initialize(points, angle)?;
                v[5] = g.span_21d4;
                v[6] = g.span_21d8;
                v[11] = g.pair[0] as f32;
                v[12] = g.pair[1] as f32;
                for (i, point) in points.iter().enumerate() {
                    v[13 + i * 10] = point[0];
                }
                v[77..81].copy_from_slice(&points[2]);
                v[81..85].copy_from_slice(&points[3]);
                geometry_words = format!(
                    "\t{:08x}\t{}\t{}\t{:08x}\t{:08x}",
                    g.scale_e54.to_bits(),
                    g.pair[0],
                    g.pair[1],
                    g.span_21d4.to_bits(),
                    g.span_21d8.to_bits()
                );
            }
            if let Some(wheel_source) = wheel_source {
                v[9] = f32::from(wheel_source.mode_1f74_is_one);
                for i in 0..4 {
                    let toe = wheel_source.toe_e8[usize::from(i >= 2)];
                    v[14 + i * 10] = toe;
                    v[21 + i * 10] = wheel_source.limit_f0;
                    geometry_words.push_str(&format!(
                        "\t{:08x}\t{:08x}",
                        toe.to_bits(),
                        wheel_source.limit_f0.to_bits()
                    ));
                }
            }
        }
        let mut wheels = Vec::new();
        for q in v[13..53].as_chunks::<10>().0 {
            wheels.push(Wheel {
                x: q[0],
                toe_e8: q[1],
                rotation_offset: q[2],
                frozen: flag(q[3])?,
                blend_20: q[4],
                blend_24: q[5],
                smooth_28: q[6],
                smooth_2c: q[7],
                limit_f0: q[8],
                rotation_144: q[9],
            });
        }
        let input = Input {
            command: steering_input::Input {
                command: v[0],
                perturbation: v[1],
                speed_scalar: v[2],
                phase_scalar: v[3],
            },
            scale_e54: v[4],
            span_21d4: v[5],
            span_21d8: v[6],
            cap_2298: v[7],
            cap_229c: v[8],
            mode_1f74_is_one: flag(v[9])?,
            delta: v[10],
            pair: [ordinal(v[11])?, ordinal(v[12])?],
            wheels: wheels.try_into().map_err(|_| "four wheels required")?,
        };
        let motion = Motion {
            matrix: std::array::from_fn(|i| v[53 + i * 4..57 + i * 4].try_into().unwrap()),
            linear: v[69..73].try_into()?,
            angular: v[73..77].try_into()?,
            wheel_points_2_3: std::array::from_fn(|i| {
                v[77 + i * 4..81 + i * 4].try_into().unwrap()
            }),
        };
        let p = &v[85..105];
        let parameters = Parameters {
            command_deadzone_fdc: p[0],
            shaping_138c: p[1],
            high_scale_123c: p[2],
            scale_start_1240: p[3],
            scale_end_1244: p[4],
            speed_cap_e70: p[5],
            minimum_scale_e5c: p[6],
            feedback_gain_e58: p[7],
            low_gain_101c: p[8],
            gain_speed_1020: p[9],
            maximum_angle_1024: p[10],
            minimum_angle_1028: p[11],
            feedback_deadzone_fec: p[12],
            linear_response_fe8: p[13],
            slew_low_1248: p[14],
            slew_high_1250: p[15],
            slew_start_124c: p[16],
            slew_end_1254: p[17],
            input_factor_1388: p[18],
            smoothing_factor_1f2c: p[19],
        };
        let output =
            assisted_steering::calculate(input, parameters, motion, v[105..109].try_into()?)?;
        let words = output
            .iter()
            .flat_map(|s| {
                [
                    s.blend_20,
                    s.blend_24,
                    s.smooth_28,
                    s.smooth_2c,
                    s.history_38,
                ]
            })
            .map(|v| format!("\t{:08x}", v.to_bits()))
            .collect::<String>();
        rows.push(format!("P\t{}{geometry_words}{words}", cells[0]));
    }
    let out = Path::new(&args[1]);
    let parent = out.parent().ok_or("missing parent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("private output required".into());
    }
    let mut writer = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(parent.join(out.file_name().ok_or("missing name")?))?;
    for row in &rows {
        writeln!(writer, "{row}")?;
    }
    writer.flush()?;
    println!("assisted_steering_cases={}", rows.len());
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-assisted-steering: {e}");
        std::process::exit(1);
    }
}
