//! CPU source-shape admission then bounded projected-triangle metrics; no GPU primitive proof.
//! grid-render-triangle-candidate PRIVATE_REQUEST.txt (stdout contains no source vertices).
use grid_formats::{mesh, pssg, scene};
use grid_tools::private_parameters as p;
use std::{collections::BTreeMap, error::Error, io::Read};
type E = Box<dyn Error>;
type M = [[f64; 4]; 4];
fn need(ok: bool, why: &str) -> Result<(), E> {
    if ok { Ok(()) } else { Err(why.into()) }
}
// Bound bytes on the opened handle, not a separate metadata snapshot (TOCTOU).
fn read_limited(path: &str, limit: u64) -> Result<Vec<u8>, E> {
    let take = limit.checked_add(1).ok_or("read limit overflow")?;
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(take)
        .read_to_end(&mut bytes)?;
    need(
        bytes.len() as u64 <= limit,
        "bounded input exceeded while reading",
    )?;
    Ok(bytes)
}
fn sha(b: &[u8]) -> String {
    p::hex(&p::sha256(b))
}
fn words(s: &str) -> Result<[u32; 16], E> {
    let v = s
        .split(',')
        .map(str::parse)
        .collect::<Result<Vec<u32>, _>>()?;
    v.try_into()
        .map_err(|_| "matrix must have sixteen actual u32 words".into())
}
fn matrix(w: [u32; 16]) -> Result<M, E> {
    let mut a = [[0.; 4]; 4];
    for r in 0..4 {
        for c in 0..4 {
            a[r][c] = f32::from_bits(w[c * 4 + r]) as f64;
            need(a[r][c].is_finite(), "nonfinite captured matrix")?;
        }
    }
    Ok(a)
}
fn multiply(a: M, p: [f64; 4]) -> [f64; 4] {
    std::array::from_fn(|r| (0..4).map(|c| a[r][c] * p[c]).sum())
}
fn inverse(a: M) -> Result<M, E> {
    let mut t = [[0.; 8]; 4];
    for r in 0..4 {
        for c in 0..4 {
            t[r][c] = a[r][c];
            t[r][c + 4] = if r == c { 1. } else { 0. };
        }
    }
    for c in 0..4 {
        let pivot = (c..4)
            .max_by(|&i, &j| t[i][c].abs().total_cmp(&t[j][c].abs()))
            .unwrap();
        need(
            t[pivot][c] != 0. && t[pivot][c].is_finite(),
            "singular actual inverse_jittered",
        )?;
        t.swap(c, pivot);
        let divisor = t[c][c];
        for value in &mut t[c] {
            *value /= divisor;
        }
        let pivot_row = t[c];
        for (r, row) in t.iter_mut().enumerate() {
            if r != c {
                let scale = row[c];
                for (value, pivot_value) in row.iter_mut().zip(pivot_row) {
                    *value -= scale * pivot_value;
                }
            }
        }
    }
    let result = std::array::from_fn(|r| std::array::from_fn(|c| t[r][c + 4]));
    need(
        result.iter().flatten().all(|x| x.is_finite()),
        "inverse result nonfinite",
    )?;
    Ok(result)
}
fn shape(positions: &[[f32; 3]], indices: &[u16]) -> String {
    let mut bytes = Vec::with_capacity(16 + positions.len() * 12 + indices.len() * 4);
    bytes.extend((positions.len() as u64).to_le_bytes());
    bytes.extend((indices.len() as u64).to_le_bytes());
    bytes.extend(
        positions
            .iter()
            .flatten()
            .flat_map(|v| v.to_bits().to_le_bytes()),
    );
    bytes.extend(indices.iter().flat_map(|&i| u32::from(i).to_le_bytes()));
    sha(&bytes)
}
fn project(p: [f32; 3], world: M, clip: M, size: [u32; 2]) -> Result<[f64; 4], E> {
    let w = multiply(world, [p[0] as f64, p[1] as f64, p[2] as f64, 1.]);
    let c = multiply(clip, w);
    need(
        c.iter().all(|x| x.is_finite()) && c[3] > 0.,
        "candidate triangle crosses/behind eye; no guessed clipping",
    )?;
    let projected = [
        (c[0] / c[3] + 1.) * 0.5 * size[0] as f64,
        (1. - c[1] / c[3]) * 0.5 * size[1] as f64,
        c[2] / c[3],
        c[3],
    ];
    need(
        projected.iter().all(|x| x.is_finite()),
        "projected coordinates/depth nonfinite",
    )?;
    Ok(projected)
}
fn area(a: [f64; 4], b: [f64; 4], c: [f64; 4]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}
fn self_check() -> Result<(), E> {
    let a = [
        [2., 0., 0., 3.],
        [0., 3., 0., 4.],
        [0., 0., 4., 5.],
        [0., 0., 0., 1.],
    ];
    let original = [1., 2., 3., 1.];
    let back = multiply(inverse(a)?, multiply(a, original));
    need(
        back.iter()
            .zip(original)
            .all(|(a, b)| (a - b).abs() < 1e-12),
        "analytic matrix roundtrip",
    )?;
    let p = [[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]];
    let mut q = p;
    q[0][0] = f32::from_bits(1);
    need(
        shape(&p, &[0, 1, 2]) != shape(&q, &[0, 1, 2]),
        "one-bit source-shape control",
    )?;
    need(
        shape(&p, &[0, 1, 2]) != shape(&p, &[0, 2, 1]),
        "index order control",
    )?;
    let id = [
        [1., 0., 0., 0.],
        [0., 1., 0., 0.],
        [0., 0., 1., 0.],
        [0., 0., 0., 1.],
    ];
    let p0 = project([-1., -1., 0.75], id, id, [100, 100])?;
    let p1 = project([1., -1., 0.75], id, id, [100, 100])?;
    let p2 = project([0., 1., 0.75], id, id, [100, 100])?;
    need(
        area(p0, p1, p2) == -10000.,
        "analytic projected triangle area",
    )?;
    need(
        inverse([[0.; 4]; 4]).is_err(),
        "singular inverse must refuse",
    )?;
    // Finite homogeneous intermediates can still overflow after division.
    let tiny_w = [
        [1e308, 0., 0., 0.],
        [0., 1., 0., 0.],
        [0., 0., 1., 0.],
        [0., 0., 0., 1e-308],
    ];
    need(
        project([1., 0., 0.75], id, tiny_w, [100, 100]).is_err(),
        "nonfinite projected coordinates must refuse",
    )?;
    println!("self_check=PASS CPU_only=true");
    Ok(())
}
fn main() -> Result<(), E> {
    let path = std::env::args()
        .nth(1)
        .ok_or("PRIVATE_REQUEST.txt or --self-check required")?;
    if path == "--self-check" {
        return self_check();
    }
    let text = String::from_utf8(read_limited(&path, 16384)?)?;
    let mut q = BTreeMap::new();
    for line in text.lines() {
        let (key, value) = line.split_once('=').ok_or("request needs key=value")?;
        need(q.insert(key, value).is_none(), "duplicate request key")?;
    }
    let get = |key| {
        q.get(key)
            .copied()
            .ok_or_else(|| -> E { format!("missing request {key}").into() })
    };
    let bytes = read_limited(get("mesh")?, 64 * 1024 * 1024)?;
    need(
        sha(&bytes) == get("mesh_sha256")?,
        "actual source file changed",
    )?;
    let tree = pssg::read_tree(&bytes)?;
    let sources = mesh::read_meshes(&tree)?;
    let draws = scene::read_draws(&tree, &sources, get("bundle")?.as_bytes())?;
    let matches: Vec<_> = draws
        .iter()
        .filter(|d| d.id == get("draw").unwrap_or("").as_bytes())
        .collect();
    need(
        matches.len() == 1,
        "source draw absent/ambiguous; no first match",
    )?;
    let draw = matches[0];
    let source = sources.get(draw.source).ok_or("source index")?;
    let positions = source
        .positions
        .get(draw.vertices.clone())
        .ok_or("source vertex slice")?;
    need(
        positions.len() <= 200000 && draw.indices.len() / 3 <= 400000,
        "source candidate geometry bound",
    )?;
    let actual = shape(positions, &draw.indices);
    // REQUIRED GATE: no world/view projection is performed before exact current-shape admission.
    need(
        actual == get("current_shape_sha256")?,
        "source rest shape differs from frozen CURRENT CPU shape; REFUSE projection",
    )?;
    let world = matrix(words(get("world_bits")?)?)?;
    let clip = inverse(matrix(words(get("inverse_jittered_bits")?)?)?)?;
    let sizes = get("size")?
        .split(',')
        .map(str::parse)
        .collect::<Result<Vec<u32>, _>>()?;
    let size: [u32; 2] = sizes.try_into().map_err(|_| "two dimensions")?;
    need(size.iter().all(|&n| n > 0 && n <= 16384), "dimensions")?;
    let mut triangles = get("triangles")?
        .split(',')
        .map(str::parse)
        .collect::<Result<Vec<usize>, _>>()?;
    need(
        !triangles.is_empty() && triangles.len() <= 32,
        "1..32 candidate triangles",
    )?;
    let before = triangles.len();
    triangles.sort_unstable();
    triangles.dedup();
    need(before == triangles.len(), "duplicate triangle request")?;
    let pixels = get("pixels")?
        .split(';')
        .map(|p| {
            let xy = p
                .split(',')
                .map(str::parse)
                .collect::<Result<Vec<u32>, _>>()?;
            need(
                xy.len() == 2 && xy[0] < size[0] && xy[1] < size[1],
                "pixel bounds",
            )?;
            Ok([xy[0], xy[1]])
        })
        .collect::<Result<Vec<[u32; 2]>, E>>()?;
    need(
        !pixels.is_empty() && pixels.len() <= 32,
        "1..32 requested pixels",
    )?;
    let mut unique = pixels.clone();
    unique.sort_unstable();
    unique.dedup();
    need(unique.len() == pixels.len(), "duplicate pixels")?;
    println!(
        "matched_source_shape=true cpu_candidate_only=true gpu_primitive_verified=false source_file_sha256={} source_draw={:?} source_mesh={:?} frozen_entity={} source_frame={} source_tick={} shape_sha256={} precision=f64_from_actual_f32_matrix_words_no_GPU_bit_oracle",
        sha(&bytes),
        get("draw")?,
        String::from_utf8_lossy(source.id),
        get("entity")?,
        get("source_frame")?,
        get("source_tick")?,
        actual
    );
    println!(
        "input_provenance raw_metadata_sha256={} cpu_summary_sha256={}",
        get("raw_metadata_sha256")?,
        get("cpu_summary_sha256")?
    );
    for triangle in triangles {
        let start = triangle.checked_mul(3).ok_or("triangle overflow")?;
        let end = start.checked_add(3).ok_or("triangle overflow")?;
        let ix = draw
            .indices
            .get(start..end)
            .ok_or("candidate triangle outside source draw")?;
        let points = ix
            .iter()
            .map(|&i| {
                project(
                    *positions
                        .get(i as usize)
                        .ok_or("triangle vertex outside slice")?,
                    world,
                    clip,
                    size,
                )
            })
            .collect::<Result<Vec<_>, E>>()?;
        let a2 = area(points[0], points[1], points[2]);
        let lo: [f64; 2] =
            std::array::from_fn(|k| points.iter().map(|p| p[k]).fold(f64::INFINITY, f64::min));
        let hi: [f64; 2] = std::array::from_fn(|k| {
            points
                .iter()
                .map(|p| p[k])
                .fold(f64::NEG_INFINITY, f64::max)
        });
        let min_w = points.iter().map(|p| p[3]).fold(f64::INFINITY, f64::min);
        let extent = [hi[0] - lo[0], hi[1] - lo[1]];
        need(
            a2.is_finite() && min_w.is_finite() && extent.iter().all(|x| x.is_finite()),
            "projected area/bounds nonfinite",
        )?;
        println!(
            "triangle={triangle} signed_double_area_px={a2} bbox_width_px={} bbox_height_px={} min_clip_w={min_w} source_vertices_omitted=true",
            extent[0], extent[1]
        );
        for xy in &pixels {
            let p = [xy[0] as f64 + 0.5, xy[1] as f64 + 0.5, 0., 0.];
            let edges = [
                area(points[0], points[1], p),
                area(points[1], points[2], p),
                area(points[2], points[0], p),
            ];
            need(
                edges.iter().all(|x| x.is_finite()),
                "projected edge equations nonfinite",
            )?;
            let inside =
                a2 != 0. && (edges.iter().all(|&v| v >= 0.) || edges.iter().all(|&v| v <= 0.));
            println!(
                "triangle={triangle} pixel={xy:?} CPU_projected_centre_inside={inside} edge_equations={edges:?} hardware_fill_rule_verified=false"
            );
        }
    }
    Ok(())
}
