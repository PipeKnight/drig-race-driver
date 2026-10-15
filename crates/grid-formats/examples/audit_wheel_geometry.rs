//! Read-only geometry audit; purchased-resource metadata stays private.
use grid_formats::{mesh, pssg, scene};
use std::{collections::BTreeSet, error::Error, fs, io::Write, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: audit_wheel_geometry GAME_ROOT local/OUTPUT.tsv".into());
    }
    let private = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../local")
        .canonicalize()?;
    let output = PathBuf::from(&args[1]);
    let parent = output
        .parent()
        .ok_or("output parent missing")?
        .canonicalize()?;
    if !parent.starts_with(&private) {
        return Err("output must stay under this checkout's existing ignored local/".into());
    }
    let source = PathBuf::from(&args[0]).join("cars/lac/lac_highLOD.pssg");
    if source.metadata()?.len() > 256 * 1024 * 1024 {
        return Err("source exceeds audit bound".into());
    }
    let bytes = fs::read(source)?;
    let tree = pssg::read_tree(&bytes)?;
    let sources = mesh::read_meshes(&tree)?;
    let mut report = String::from(
        "lod\tjoint\tmaterial\tvertices_used\ttriangles\taxis_width\tradius_y\tradius_z\touter_angular_samples\tlargest_outer_gap_deg\tideal_circle_sag_ratio\n",
    );
    let mut count = 0;
    for lod in [b"LOD0_".as_slice(), b"LOD1_".as_slice()] {
        for draw in scene::read_draws(&tree, &sources, lod)? {
            let joint = std::str::from_utf8(draw.joint)?;
            let Some((_, suffix)) = joint.split_once('_') else {
                continue;
            };
            let Some((kind, side)) = suffix.split_once('_') else {
                continue;
            };
            if !matches!(kind, "wheel" | "tyre" | "disc")
                || !matches!(side, "bl" | "br" | "fl" | "fr")
            {
                continue;
            }
            let positions = &sources[draw.source].positions[draw.vertices.clone()];
            let used: BTreeSet<_> = draw.indices.iter().map(|&i| usize::from(i)).collect();
            let mut lo = [f32::INFINITY; 3];
            let mut hi = [f32::NEG_INFINITY; 3];
            for &i in &used {
                let p = positions.get(i).ok_or("index outside draw")?;
                for axis in 0..3 {
                    lo[axis] = lo[axis].min(p[axis]);
                    hi[axis] = hi[axis].max(p[axis]);
                }
            }
            // Source rotation is about local X. Bbox center is an audit
            // approximation, not a recovered physical or artist pivot.
            let cy = (lo[1] + hi[1]) * 0.5;
            let cz = (lo[2] + hi[2]) * 0.5;
            let radius = used
                .iter()
                .map(|&i| (positions[i][1] - cy).hypot(positions[i][2] - cz))
                .fold(0.0_f32, f32::max);
            if radius <= 0.0 {
                return Err("degenerate wheel radius".into());
            }
            let mut angles: Vec<_> = used
                .iter()
                .filter_map(|&i| {
                    let y = positions[i][1] - cy;
                    let z = positions[i][2] - cz;
                    (y.hypot(z) >= radius * 0.98)
                        .then(|| y.atan2(z).rem_euclid(std::f32::consts::TAU))
                })
                .collect();
            angles.sort_by(f32::total_cmp);
            angles.dedup_by(|a, b| (*a - *b).abs() < 1e-5);
            if angles.len() < 3 {
                return Err("insufficient outer samples".into());
            }
            let gap = angles
                .windows(2)
                .map(|p| p[1] - p[0])
                .chain(std::iter::once(
                    angles[0] + std::f32::consts::TAU - angles[angles.len() - 1],
                ))
                .fold(0.0_f32, f32::max);
            report.push_str(&format!(
                "{}\t{:?}\t{:?}\t{}\t{}\t{:.6}\t{:.6}\t{:.6}\t{}\t{:.5}\t{:.8}\n",
                std::str::from_utf8(lod)?,
                joint,
                std::str::from_utf8(draw.material)?,
                used.len(),
                draw.indices.len() / 3,
                hi[0] - lo[0],
                (hi[1] - lo[1]) * 0.5,
                (hi[2] - lo[2]) * 0.5,
                angles.len(),
                gap.to_degrees(),
                1.0 - (gap * 0.5).cos()
            ));
            count += 1;
        }
    }
    if count == 0 {
        return Err("no selected wheel draws".into());
    }
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?
        .write_all(report.as_bytes())?;
    println!(
        "wheel audit rows={count}; local-X, 98% outer annulus; sag is an ideal-circle estimate, not a rendered-error measurement"
    );
    Ok(())
}
