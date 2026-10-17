//! Private Donington placement prototype; original whole-query/tick fidelity is unverified.
use grid_formats::{binxml, car_collision, collision, grid, jpk, pssg, tuning, vehicle};
use grid_world::{Mesh, Segment, Triangle, spawn, triangle};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
};

fn read(path: &Path, limit: u64) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    File::open(path)?.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err("resource exceeds limit".into());
    }
    Ok(bytes)
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|v| format!("{v:02x}")).collect()
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() < 3 || args.len() > 4 {
        return Err(
            "usage: grid-spawn GAME_ROOT CAR_ID local/OUTPUT.tsv [geometric|reference|kernel]"
                .into(),
        );
    }
    let mode = args.get(3).and_then(|v| v.to_str()).unwrap_or("geometric");
    let kernel = mode == "kernel";
    let reference = match mode {
        "geometric" => false,
        "reference" | "kernel" => true,
        _ => return Err("placement arithmetic must be geometric, reference or kernel".into()),
    };
    let normalization = if reference {
        spawn::Normalization::ReferenceSse
    } else {
        spawn::Normalization::Standard
    };
    let root = Path::new(&args[0]);
    let car = args[1].to_str().ok_or("car id must be ASCII")?;
    if car.len() != 3 || !car.bytes().all(|v| v.is_ascii_alphanumeric()) {
        return Err("car id must have three ASCII letters/digits".into());
    }
    let output = Path::new(&args[2]);
    let parent = output
        .parent()
        .ok_or("missing output parent")?
        .canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("report must remain private".into());
    }
    let ned = read(
        &root.join(format!("cars/{car}/{car}.ned")),
        16 * 1024 * 1024,
    )?;
    let csv = read(&root.join(format!("cars/{car}/{car}.csv")), 4 * 1024 * 1024)?;
    let doc = binxml::read(&ned)?;
    let table = tuning::read(&csv)?;
    let row = table.rows.first().ok_or("car tuning row absent")?;
    let layout = vehicle::standard_wheel_layout(row, vehicle::read_wheel_geometry(&doc)?)?;
    let shape = car_collision::read(&doc)?;
    let front_extent = shape.bounds.max[2] + layout.offset[2];
    let wheels = std::array::from_fn(|i| spawn::Wheel {
        position: layout.positions[i][..3].try_into().unwrap(),
        radius: layout.radii[i],
        vertical_displacement: 0.,
    });
    let track = read(
        &root.join("tracks/europe/donington/route_0/track.jpk"),
        256 * 1024 * 1024,
    )?;
    let archive = jpk::inspect(&track)?;
    let mut triangles = Vec::new();
    for entry in archive.entries {
        if !entry.name.ends_with(b".vcqtc")
            || entry.uncompressed_size as usize != entry.stored_data.len()
        {
            return Err("unsupported collision entry".into());
        }
        let chunk = collision::read_vcqtc(&track[entry.stored_data])?;
        if triangles.len() + chunk.triangles.len() > 1_000_000 {
            return Err("triangle limit exceeded".into());
        }
        triangles.extend(chunk.triangles.iter().map(|t| Triangle {
            vertices: t.indices.map(|i| chunk.positions[i as usize]),
            material: chunk.materials[t.material as usize],
            flags: t.flags,
        }));
    }
    let mesh = Mesh::new(triangles.clone())?;
    let marker_bytes = read(
        &root.join("tracks/europe/donington/route_0/grids.pssg"),
        256 * 1024 * 1024,
    )?;
    let markers = pssg::read_tree(&marker_bytes)?;
    let mut reports = Vec::new();
    let mut count = 0;
    for i in 0..markers.nodes.len() {
        if markers.name(i) != b"NODE" {
            continue;
        }
        let Ok(id) = markers.string(i, b"id") else {
            continue;
        };
        if !id.starts_with(b"slot_") {
            continue;
        }
        let marker = grid::read(&markers, id)?;
        let width = marker.corners[1]
            .iter()
            .zip(marker.corners[0])
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f32>()
            .sqrt();
        if width * 0.5 > shape.bounds.max[0] {
            return Err("slot requires unimplemented random lateral selection".into());
        }
        let center = if kernel {
            spawn::reference_front_aligned_center(marker.corners, front_extent, 0.)?
        } else {
            marker.front_aligned_center(front_extent, 0.)?
        };
        let heading = if kernel {
            std::array::from_fn(|i| marker.corners[0][i] - marker.corners[2][i])
        } else {
            marker.forward
        };
        let mut ordinal = 0;
        let pose = spawn::align_with_normalization(
            center,
            heading,
            wheels,
            normalization,
            |x, z, upper, lower| {
                let query = mesh.query(Segment {
                    start: [x as f64, upper as f64, z as f64],
                    end: [x as f64, lower as f64, z as f64],
                })?;
                let height = query
                    .hit
                    .map(|hit| -> Result<f32, grid_world::Error> {
                        if reference {
                            let fraction = if kernel {
                                triangle::query(
                                    [x, upper, z],
                                    [0., lower - upper, 0.],
                                    triangles[hit.triangle].vertices,
                                )
                                .ok_or(grid_world::Error(
                                    "selected triangle missed under f32 kernel",
                                ))?
                                .fraction
                            } else {
                                hit.fraction as f32
                            };
                            Ok(upper - (upper - lower) * fraction)
                        } else {
                            Ok(hit.position[1] as f32)
                        }
                    })
                    .transpose()?;
                reports.push(format!(
                    "Q\t{}\t{ordinal}\t{:08x}\t{:08x}\t{:08x}\t{:08x}\t{}",
                    hex(id),
                    x.to_bits(),
                    z.to_bits(),
                    upper.to_bits(),
                    lower.to_bits(),
                    height.map_or_else(|| "none".into(), |v| format!("{:08x}", v.to_bits()))
                ));
                ordinal += 1;
                Ok(height)
            },
        )?;
        let words: String = center
            .into_iter()
            .chain(pose.matrix())
            .map(|v| format!("\t{:08x}", v.to_bits()))
            .collect();
        reports.push(format!(
            "P\t{}{words}\t{}\t{}",
            hex(id),
            pose.corner_misses,
            pose.clearance_hits
        ));
        count += 1;
    }
    if count == 0 {
        return Err("no supported grid markers".into());
    }
    let mut writer = std::io::BufWriter::new(
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(parent.join(output.file_name().ok_or("missing output name")?))?,
    );
    writeln!(
        writer,
        "C\t{car}\t{:08x}\tmode_one\tf64_world_query\tzero_wheel_displacement\t{}",
        front_extent.to_bits(),
        mode
    )?;
    for report in reports {
        writeln!(writer, "{report}")?;
    }
    writer.flush()?;
    println!(
        "prototype_placements={count} triangles={}",
        mesh.triangle_count()
    );
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("grid-spawn: {error}");
        std::process::exit(1);
    }
}
