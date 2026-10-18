//! Private, read-only route reader survey. For one route, runs each reader the
//! headless race and the app use on its own file and prints `ok` or the error,
//! so a failing load names its file. `nodes` prints every PSSG node of one
//! element with its attributes, to inspect an unsupported variant.
//!
//! grid-route-readers stages GAME_ROOT ROUTE
//! grid-route-readers nodes FILE ELEMENT [LIMIT]
//! grid-route-readers streams FILE
use grid_formats::{collision, entities, jpk, mesh, ornaments, pssg, scene, start_lights};
use grid_tools::route::Route;
use std::{error::Error, path::Path};

type Res<T> = Result<T, Box<dyn Error>>;

fn read(path: &Path) -> Res<Vec<u8>> {
    let bytes = std::fs::read(path)?;
    if bytes.len() > 256 << 20 {
        return Err("resource exceeds 256 MiB".into());
    }
    Ok(bytes)
}

fn report(stage: &str, file: &str, result: Res<String>) {
    match result {
        Ok(detail) => println!("{stage}\t{file}\tok\t{detail}"),
        Err(e) => println!("{stage}\t{file}\tERROR\t{e}"),
    }
}

fn collision_archive(bytes: &[u8]) -> Res<String> {
    let archive = jpk::inspect(bytes)?;
    let mut most = [0usize; 3];
    for (i, entry) in archive.entries.iter().enumerate() {
        let name = String::from_utf8_lossy(entry.name);
        let chunk = collision::read_vcqtc(&bytes[entry.stored_data.clone()])
            .map_err(|e| format!("entry {i} {name}: {e}"))?;
        most[0] = most[0].max(chunk.triangles.len());
        most[1] = most[1].max(chunk.positions.len());
        most[2] = most[2].max(chunk.materials.len());
    }
    grid_tools::track::Track::read(bytes)?;
    Ok(format!(
        "entries={} max_triangles={} max_vertices={} max_materials={}",
        archive.entries.len(),
        most[0],
        most[1],
        most[2]
    ))
}

fn meshes(bytes: &[u8], static_only: bool, draws: &str) -> Res<String> {
    let tree = pssg::read_tree(bytes)?;
    let sources = if static_only {
        mesh::read_static_meshes(&tree)?
    } else {
        mesh::read_meshes(&tree)?
    };
    let count = match draws {
        "object" => scene::read_object_draws(&tree, &sources)?.len(),
        _ => scene::read_static_draws(&tree, &sources)?.len(),
    };
    Ok(format!("sources={} draws={count}", sources.len()))
}

fn stages(root: &Path, route: &Route) -> Res<()> {
    let file = |name: &str| route.file(name);
    let track = |name: &str| route.track_file(name);
    let run = |stage: &str, path: String, f: &dyn Fn(&[u8]) -> Res<String>| {
        let result = read(&root.join(&path)).and_then(|b| f(&b));
        report(stage, &path, result);
    };
    run("collision", file("track.jpk"), &collision_archive);
    run("boundary", file("boundarylines.cqtc"), &|b| {
        grid_tools::cqtc_geometry::Geometry::read(b)?;
        Ok(String::new())
    });
    run("grid", file("grids.pssg"), &|b| {
        let tree = pssg::read_tree(b)?;
        grid_formats::grid::read(&tree, b"slot_00")?;
        Ok("slot_00".into())
    });
    run("progress", file("progress_track.xml"), &|_| {
        let p = grid_tools::lap_progress::load_route(root, route)?;
        Ok(format!("gates={}", p.progress.gates.len()))
    });
    run("ai", file("ai_track.xml"), &|_| {
        grid_tools::ai_driver::AiData::load(root, route.path(), "touring", "lac")?;
        Ok("touring/lac".into())
    });
    run("start_lights", track("start_lights.xml"), &|b| {
        let s = start_lights::read(&grid_formats::binxml::read(b)?)?;
        Ok(format!("{s:?}").chars().take(80).collect())
    });
    run("entities", file("objects.ens"), &|b| {
        let placements = pssg::read_tree(b)?;
        let types_bytes = read(&root.join(track("objecttypes.pssg")))?;
        let types = pssg::read_tree(&types_bytes)?;
        Ok(format!(
            "placements={}",
            entities::read(&placements, &types)?.len()
        ))
    });
    run("ornaments", file("ornaments.bin"), &|b| {
        Ok(format!(
            "prototypes={}",
            ornaments::read(b)?.prototypes.len()
        ))
    });
    run("objects_static", track("objects.pssg"), &|b| {
        meshes(b, true, "object")
    });
    run("props", file("objects.ens"), &|_| {
        let p = grid_tools::entity_geometry::Props::read_route(root, route)?;
        Ok(format!("boxes={}", p.boxes.len()))
    });
    run("tracksplit", track("tracksplit.pssg"), &|b| {
        meshes(b, false, "static")
    });
    run("land", track("land.pssg"), &|b| meshes(b, false, "static"));
    run("trees", track("trees.pssg"), &|b| meshes(b, true, "static"));
    run("sky", track("sky.pssg"), &|b| meshes(b, false, "static"));
    Ok(())
}

fn nodes(path: &Path, element: &str, limit: usize) -> Res<()> {
    let bytes = read(path)?;
    let tree = pssg::read_tree(&bytes)?;
    let schema = &tree.framing.schema;
    let attribute_name = |id: u32| {
        schema
            .iter()
            .flat_map(|e| &e.attributes)
            .find(|a| a.id == id)
            .map(|a| String::from_utf8_lossy(a.name).into_owned())
            .unwrap_or_else(|| format!("#{id}"))
    };
    let mut shown = 0;
    for (i, node) in tree.nodes.iter().enumerate() {
        if tree.name(i) != element.as_bytes() {
            continue;
        }
        if shown == limit {
            println!("...");
            break;
        }
        shown += 1;
        let parent = node
            .parent
            .map(|p| String::from_utf8_lossy(tree.name(p)).into_owned())
            .unwrap_or_default();
        let attributes: Vec<String> = node
            .attributes
            .iter()
            .map(|a| {
                let v = a.value;
                let text =
                    if v.len() > 4 && v[4..].iter().all(|c| c.is_ascii_graphic() || *c == b' ') {
                        String::from_utf8_lossy(&v[4..]).into_owned()
                    } else if v.len() == 4 {
                        let w = u32::from_be_bytes(v.try_into().unwrap());
                        format!("0x{w:08x}/{}", f32::from_bits(w))
                    } else {
                        v.iter().map(|b| format!("{b:02x}")).collect()
                    };
                format!("{}={text}", attribute_name(a.id))
            })
            .collect();
        let children: Vec<String> = node
            .children
            .iter()
            .map(|&c| String::from_utf8_lossy(tree.name(c)).into_owned())
            .collect();
        println!(
            "0x{:x}\tparent={parent}\t{}\tchildren=[{}]\tdata={}",
            node.offset,
            attributes.join(" "),
            children.join(","),
            node.data.map_or(0, <[u8]>::len)
        );
    }
    println!("shown={shown}");
    Ok(())
}

fn run() -> Res<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["stages", root, route] => stages(Path::new(root), &Route::new(route)?),
        ["streams", file] => streams(Path::new(file)),
        ["nodes", file, element] => nodes(Path::new(file), element, 20),
        ["nodes", file, element, limit] => nodes(Path::new(file), element, limit.parse()?),
        _ => Err(
            "usage: grid-route-readers stages GAME_ROOT ROUTE | nodes FILE ELEMENT [LIMIT]".into(),
        ),
    }
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("grid-route-readers: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

/// Every render stream's kind and format by data block, with the count of
/// nonfinite half2 pairs (first few raw words) and the source that uses it.
fn streams(path: &Path) -> Res<()> {
    let bytes = read(path)?;
    let tree = pssg::read_tree(&bytes)?;
    let mut blocks = std::collections::BTreeMap::new();
    for i in 0..tree.nodes.len() {
        if tree.name(i) == b"DATABLOCK" {
            blocks.insert(tree.string(i, b"id")?, i);
        }
    }
    let mut kinds = std::collections::BTreeMap::<String, usize>::new();
    for source in 0..tree.nodes.len() {
        if tree.name(source) != b"RENDERDATASOURCE" {
            continue;
        }
        let id = String::from_utf8_lossy(tree.string(source, b"id")?).into_owned();
        for &stream in &tree.nodes[source].children {
            if tree.name(stream) != b"RENDERSTREAM" {
                continue;
            }
            let reference = tree.string(stream, b"dataBlock")?;
            let block = blocks[&reference[1..]];
            let descriptors: Vec<_> = tree.nodes[block]
                .children
                .iter()
                .copied()
                .filter(|&i| tree.name(i) == b"DATABLOCKSTREAM")
                .collect();
            let descriptor = descriptors[tree.integer(stream, b"subStream")? as usize];
            let kind =
                String::from_utf8_lossy(tree.string(descriptor, b"renderType")?).into_owned();
            let format =
                String::from_utf8_lossy(tree.string(descriptor, b"dataType")?).into_owned();
            *kinds.entry(format!("{kind}/{format}")).or_default() += 1;
            let data_node = tree.nodes[block]
                .children
                .iter()
                .copied()
                .find(|&i| tree.name(i) == b"DATABLOCKDATA")
                .ok_or("no data")?;
            let data = tree.nodes[data_node].data.ok_or("no raw data")?;
            let count = tree.integer(block, b"elementCount")? as usize;
            let offset = tree.integer(descriptor, b"offset")? as usize;
            let stride = tree.integer(descriptor, b"stride")? as usize;
            if format == "half2" {
                let mut bad = Vec::new();
                for i in 0..count {
                    let p = i * stride + offset;
                    let w = [
                        u16::from_be_bytes([data[p], data[p + 1]]),
                        u16::from_be_bytes([data[p + 2], data[p + 3]]),
                    ];
                    if w.iter().any(|h| h & 0x7c00 == 0x7c00) {
                        bad.push((i, w));
                    }
                }
                if !bad.is_empty() {
                    let sample: Vec<String> = bad
                        .iter()
                        .take(4)
                        .map(|(i, w)| format!("{i}:{:04x},{:04x}", w[0], w[1]))
                        .collect();
                    println!(
                        "nonfinite_half2\tsource={id}\tstream={kind}\tblock=0x{:x}\tdata=0x{:x}\tbad={}/{count}\t{}",
                        tree.nodes[block].offset,
                        tree.nodes[data_node].offset,
                        bad.len(),
                        sample.join(" ")
                    );
                }
            }
            if !matches!(
                (kind.as_str(), format.as_str()),
                (
                    "Vertex" | "Normal" | "Tangent" | "Binormal" | "SkinnableVertex",
                    "float3"
                ) | ("ST", "float2" | "half2")
                    | ("SkinIndices", "float")
                    | ("Color", _)
                    | ("Tangent" | "Binormal", _)
            ) {
                println!(
                    "unsupported\tsource={id}\tstream={kind}/{format}\tdescriptor=0x{:x}\toffset={offset}\tstride={stride}\tcount={count}",
                    tree.nodes[descriptor].offset
                );
            }
        }
    }
    for (k, n) in kinds {
        println!("kind\t{k}\t{n}");
    }
    Ok(())
}
