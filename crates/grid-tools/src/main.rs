use std::{
    env,
    fs::{File, OpenOptions},
    io::{BufWriter, Read, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};

fn private_output(path: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    // Purchased-resource exports are local evidence, never public source fixtures.
    let root = env::current_dir()?.join("local").canonicalize()?;
    let parent = path
        .parent()
        .ok_or("output needs a parent directory")?
        .canonicalize()?;
    if !parent.starts_with(&root) {
        return Err("exports must remain beneath the workspace local/ directory".into());
    }
    Ok(parent.join(path.file_name().ok_or("output filename absent")?))
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args_os().skip(1);
    let format = args
        .next()
        .ok_or("usage: grid-tools <pssg|pssg-tree|pssg-mesh|pssg-textures|pssg-obj|pssg-dds|jpk> <local-file> [private-output]")?;
    let path = args.next().ok_or("missing local file path")?;
    let output = args.next();
    if args.next().is_some() {
        return Err("unexpected extra arguments".into());
    }
    let mut bytes = Vec::new();
    // Limit an inspection to 256 MiB and detect growth while reading.
    File::open(&path)?
        .take(256 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 256 * 1024 * 1024 {
        return Err("file exceeds 256 MiB inspector limit".into());
    }
    let exports = format == "pssg-obj" || format == "pssg-dds";
    if exports != output.is_some() {
        return Err("provide output only for pssg-obj or pssg-dds".into());
    }
    if format == "pssg" {
        let p = grid_formats::pssg::inspect(&bytes)?;
        println!(
            "PSSG bytes={} elements={} attribute_definitions={} root_id={} root_offset=0x{:x} root_attributes={} opaque_payload={}..{}",
            bytes.len(),
            p.schema.len(),
            p.attribute_count,
            p.root_id,
            p.root_offset,
            p.root_attributes.len(),
            p.root_payload.start,
            p.root_payload.end
        );
        for e in &p.schema {
            println!(
                "element {} name={:?} attributes={}",
                e.id,
                String::from_utf8_lossy(e.name),
                e.attributes.len()
            );
        }
    } else if format == "pssg-tree" || format == "pssg-mesh" || format == "pssg-textures" || exports
    {
        let tree = grid_formats::pssg::read_tree(&bytes)?;
        println!("PSSG tree nodes={}", tree.nodes.len());
        if format == "pssg-tree" {
            let mut counts = std::collections::BTreeMap::new();
            for i in 0..tree.nodes.len() {
                *counts.entry(tree.name(i)).or_insert(0) += 1usize;
            }
            for (name, count) in counts {
                println!(
                    "node name={:?} count={count}",
                    String::from_utf8_lossy(name)
                );
            }
        } else if format == "pssg-mesh" || format == "pssg-obj" {
            let meshes = grid_formats::mesh::read_meshes(&tree)?;
            let vertices: usize = meshes.iter().map(|m| m.positions.len()).sum();
            let triangles: usize = meshes.iter().map(|m| m.indices.len() / 3).sum();
            println!(
                "meshes={} vertices={vertices} triangles={triangles}",
                meshes.len()
            );
            for m in &meshes {
                let mut min = [f32::INFINITY; 3];
                let mut max = [f32::NEG_INFINITY; 3];
                for position in &m.positions {
                    for axis in 0..3 {
                        min[axis] = min[axis].min(position[axis]);
                        max[axis] = max[axis].max(position[axis]);
                    }
                }
                println!(
                    "mesh id={:?} vertices={} triangles={} nonfinite_normals={} min={min:?} max={max:?}",
                    String::from_utf8_lossy(m.id),
                    m.positions.len(),
                    m.indices.len() / 3,
                    m.nonfinite_normals
                );
            }
            if format == "pssg-obj" {
                if meshes.is_empty() {
                    return Err("no supported meshes to export".into());
                }
                let out = private_output(Path::new(output.as_ref().unwrap()))?;
                let mut writer =
                    BufWriter::new(OpenOptions::new().write(true).create_new(true).open(&out)?);
                writeln!(
                    writer,
                    "# Diagnostic raw render-source geometry; no instance transforms, skinning or materials"
                )?;
                let mut base = 1usize;
                let mut normal_base = 1usize;
                let mut uv_base = 1usize;
                for (number, m) in meshes.iter().enumerate() {
                    writeln!(writer, "o source_{number}")?;
                    for v in &m.positions {
                        writeln!(writer, "v {} {} {}", v[0], v[1], v[2])?;
                    }
                    for v in &m.texcoords {
                        writeln!(writer, "vt {} {}", v[0], v[1])?;
                    }
                    if m.nonfinite_normals == 0 && !m.normals.is_empty() {
                        for v in &m.normals {
                            writeln!(writer, "vn {} {} {}", v[0], v[1], v[2])?;
                        }
                    } else {
                        writeln!(
                            writer,
                            "# {} nonfinite source normals; omit normals for this source",
                            m.nonfinite_normals
                        )?;
                    }
                    for t in m.indices.as_chunks::<3>().0 {
                        let a = base + t[0] as usize;
                        let b = base + t[1] as usize;
                        let c = base + t[2] as usize;
                        let ta = uv_base + t[0] as usize;
                        let tb = uv_base + t[1] as usize;
                        let tc = uv_base + t[2] as usize;
                        if m.nonfinite_normals == 0
                            && !m.normals.is_empty()
                            && !m.texcoords.is_empty()
                        {
                            let na = normal_base + t[0] as usize;
                            let nb = normal_base + t[1] as usize;
                            let nc = normal_base + t[2] as usize;
                            writeln!(writer, "f {a}/{ta}/{na} {b}/{tb}/{nb} {c}/{tc}/{nc}")?;
                        } else if !m.texcoords.is_empty() {
                            writeln!(writer, "f {a}/{ta} {b}/{tb} {c}/{tc}")?;
                        } else if m.nonfinite_normals == 0 && !m.normals.is_empty() {
                            let na = normal_base + t[0] as usize;
                            let nb = normal_base + t[1] as usize;
                            let nc = normal_base + t[2] as usize;
                            writeln!(writer, "f {a}//{na} {b}//{nb} {c}//{nc}")?;
                        } else {
                            writeln!(writer, "f {a} {b} {c}")?;
                        }
                    }
                    base += m.positions.len();
                    uv_base += m.texcoords.len();
                    if m.nonfinite_normals == 0 && !m.normals.is_empty() {
                        normal_base += m.normals.len();
                    }
                }
                writer.flush()?;
                println!("private OBJ written {:?}", out);
            }
        } else {
            let textures = grid_formats::texture::read_textures(&tree)?;
            println!("textures={}", textures.len());
            let mut export_paths = Vec::new();
            if format == "pssg-dds" {
                let dir = Path::new(output.as_ref().unwrap()).canonicalize()?;
                for i in 0..textures.len() {
                    let out = private_output(&dir.join(format!("texture_{i:03}.dds")))?;
                    if out.exists() {
                        return Err("refusing to overwrite texture export".into());
                    }
                    export_paths.push(out);
                }
            }
            for (i, texture) in textures.iter().enumerate() {
                println!(
                    "texture index={i} id={:?} width={} height={} format={:?} mip_attribute={} base_bytes={} opaque_tail_bytes={}",
                    String::from_utf8_lossy(texture.id),
                    texture.width,
                    texture.height,
                    texture.format,
                    texture.declared_mip_levels,
                    texture.base_blocks.len(),
                    texture.remaining_data.len()
                );
                if let Some(out) = export_paths.get(i) {
                    let mut file = OpenOptions::new().write(true).create_new(true).open(out)?;
                    file.write_all(&texture.base_dds())?;
                }
            }
        }
    } else if format == "jpk" {
        let p = grid_formats::jpk::inspect(&bytes)?;
        println!(
            "JPAK bytes={} entries={} alignment={} unknown_header={:?}",
            bytes.len(),
            p.entries.len(),
            p.alignment,
            p.unknown_header
        );
        for e in &p.entries {
            println!(
                "entry name={:?} stored={}..{} stored_size={} declared_uncompressed={} unknown={:?}",
                String::from_utf8_lossy(e.name),
                e.stored_data.start,
                e.stored_data.end,
                e.stored_data.len(),
                e.uncompressed_size,
                e.unknown
            );
        }
    } else {
        return Err("unsupported inspector; see usage".into());
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("grid-tools: {e}");
            ExitCode::FAILURE
        }
    }
}
