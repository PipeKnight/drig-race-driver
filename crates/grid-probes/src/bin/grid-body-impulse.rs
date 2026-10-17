//! Supplied-impulse diagnostic. Generates no tyre/contact/suspension forces.
use grid_world::dynamics::{self, BodyState, ImpulseBody};
use std::{fs::OpenOptions, io::Write, path::Path};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let point_response = args.len() == 3 && args[2] == "point";
    if args.len() != 2 && !point_response {
        return Err("usage: grid-body-impulse INPUT.tsv local/OUTPUT.tsv [point]".into());
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
        return Err("report must remain private".into());
    }
    let text = std::fs::read_to_string(input)?;
    let mut report = String::new();
    let mut count = 0;
    for line in text.lines() {
        count += 1;
        if count > 256 {
            return Err("case limit exceeded".into());
        }
        let cells: Vec<_> = line.split_whitespace().collect();
        if cells.len() != 37
            || cells[0].len() > 63
            || !cells[0]
                .bytes()
                .all(|v| v.is_ascii_alphanumeric() || v == b'_')
        {
            return Err("invalid case framing".into());
        }
        let words: Vec<f32> = cells[1..]
            .iter()
            .map(|v| {
                if v.len() != 8 || !v.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err("invalid float word");
                }
                Ok(f32::from_bits(
                    u32::from_str_radix(v, 16).map_err(|_| "invalid float word")?,
                ))
            })
            .collect::<Result<_, _>>()?;
        let body = ImpulseBody {
            mass: words[0],
            inertia: words[1..4].try_into()?,
            basis: std::array::from_fn(|i| {
                words[4 + i * 4..8 + i * 4]
                    .try_into()
                    .expect("four-word row")
            }),
        };
        let impulse = words[28..32].try_into()?;
        let lever = words[32..36].try_into()?;
        let state = BodyState {
            linear: words[20..24].try_into()?,
            angular: words[24..28].try_into()?,
            position: body.basis[3],
        };
        let (response, linear, angular) = if point_response {
            let response = dynamics::point_velocity(state, lever)?;
            let result = dynamics::apply_impulse_at_point(body, state, impulse, lever)?;
            (response, result.linear, result.angular)
        } else {
            let response = dynamics::angular_impulse(body, impulse, lever)?;
            let (linear, angular) =
                dynamics::apply_impulse(body, state.linear, state.angular, impulse, lever)?;
            (response, linear, angular)
        };
        report += &format!("P\t{}", cells[0]);
        for value in response.into_iter().chain(linear).chain(angular) {
            report += &format!("\t{:08x}", value.to_bits());
        }
        report.push('\n');
    }
    if count == 0 {
        return Err("no cases".into());
    }
    let mut writer = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(parent.join(output.file_name().ok_or("missing output name")?))?;
    writer.write_all(report.as_bytes())?;
    println!("supplied_impulse_cases={count}");
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("grid-body-impulse: {error}");
        std::process::exit(1);
    }
}
