//! Read-only wheel UV audit; all purchased vertex evidence stays ignored/local.
use grid_formats::{mesh, pssg, scene};
use std::{error::Error, fs, io::Write, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: audit_wheel_uv GAME_ROOT local/OUTPUT.tsv".into());
    }
    let private = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../local")
        .canonicalize()?;
    let output = PathBuf::from(&args[1]);
    if !output
        .parent()
        .ok_or("missing output parent")?
        .canonicalize()?
        .starts_with(private)
    {
        return Err("output must stay under this checkout's existing ignored local/".into());
    }
    let path = PathBuf::from(&args[0]).join("cars/lac/lac_highLOD.pssg");
    if path.metadata()?.len() > 256 * 1024 * 1024 {
        return Err("source exceeds audit bound".into());
    }
    let bytes = fs::read(path)?;
    let tree = pssg::read_tree(&bytes)?;
    let meshes = mesh::read_meshes(&tree)?;
    let mut report = String::from("joint\tmaterial\tvertex\tx\ty\tz\tu\tv\tnx\tny\tnz\n");
    let mut count = 0;
    for draw in scene::read_draws(&tree, &meshes, b"LOD0_")? {
        if !matches!(
            draw.material,
            b"Tyre Wall" | b"Tyre Tread" | b"Wheel" | b"Discs"
        ) {
            continue;
        }
        let source = &meshes[draw.source];
        for i in draw.vertices.clone() {
            let p = source.positions[i];
            let uv = source.texcoords[i];
            let n = source.normals[i];
            report.push_str(&format!(
                "{}\t{}\t{i}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                std::str::from_utf8(draw.joint)?,
                std::str::from_utf8(draw.material)?,
                p[0],
                p[1],
                p[2],
                uv[0],
                uv[1],
                n[0],
                n[1],
                n[2]
            ));
            count += 1;
        }
    }
    if count == 0 {
        return Err("no selected wheel vertices".into());
    }
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?
        .write_all(report.as_bytes())?;
    println!(
        "wheel UV audit vertices={count}; source coordinates, not a proven circular UV layout"
    );
    Ok(())
}
