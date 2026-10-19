//! Looks for road collision under points (soak piece losses, SOAK_TESTING.md).
//!
//! Usage: grid-road-hole GAME_ROOT ROUTE X,Z [X,Z..]
//! For each point: the packed world's vertical query from 50 m above to 50 m
//! below the highest triangle there, and a brute-force test of every packed
//! triangle of every chunk whose xz projection covers the point.
use grid_formats::{collision, jpk};
use grid_tools::{route::Route, track::Track};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        return Err("usage: grid-road-hole GAME_ROOT ROUTE X,Z [X,Z..]".into());
    }
    let root = std::path::Path::new(&args[1]);
    let route = Route::new(&args[2])?;
    let bytes = std::fs::read(root.join(route.file("track.jpk")))?;
    let track = Track::read(&bytes)?;
    let mut chunks = Vec::new();
    for entry in jpk::inspect(&bytes)?.entries {
        if entry.uncompressed_size as usize != entry.stored_data.len() {
            return Err("compressed collision entry".into());
        }
        let name = String::from_utf8_lossy(entry.name).into_owned();
        chunks.push((name, collision::read_vcqtc(&bytes[entry.stored_data])?));
    }
    let boundary = grid_tools::cqtc_geometry::Geometry::read(&std::fs::read(
        root.join(route.file("boundarylines.cqtc")),
    )?)?;
    for point in &args[3..] {
        let (x, z) = point.split_once(',').ok_or("point needs X,Z")?;
        let (x, z): (f32, f32) = (x.parse()?, z.parse()?);
        let mut found = Vec::new();
        for (name, chunk) in &chunks {
            for t in &chunk.triangles {
                let [a, b, c] = t.indices.map(|i| chunk.positions[i as usize]);
                let side = |p: [f32; 3], q: [f32; 3]| (q[0] - p[0]) * (z - p[2]) - (q[2] - p[2]) * (x - p[0]);
                let s = [side(a, b), side(b, c), side(c, a)];
                if !(s.iter().all(|v| *v >= 0.) || s.iter().all(|v| *v <= 0.)) {
                    continue;
                }
                // Height of the triangle's plane at (x, z).
                let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
                let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
                if n[1].abs() < 1e-6 {
                    continue;
                }
                let y = a[1] - (n[0] * (x - a[0]) + n[2] * (z - a[2])) / n[1];
                found.push((y, name.clone()));
            }
        }
        // Boundary-line sheet triangles within 3 m (xz) of the point.
        let near = boundary
            .triangles
            .iter()
            .filter(|t| {
                t.iter().any(|&i| {
                    let p = boundary.positions[i as usize];
                    (p[0] - x).hypot(p[2] - z) < 3.
                })
            })
            .map(|t| format!("{:.2?}", t.map(|i| boundary.positions[i as usize])))
            .collect::<Vec<_>>();
        println!("point ({x}, {z}): boundary sheet triangles within 3 m: {} {:?}", near.len(), near.iter().take(8).collect::<Vec<_>>());
        found.sort_by(|p, q| q.0.total_cmp(&p.0));
        let top = found.first().map_or(200., |f| f.0);
        let mut cache = grid_world::rectangle_cull::Cache::default();
        let q = track.query([x, top + 50., z, 1.], [x, top - 50., z, 1.], &mut cache)?;
        let hit = q.hit.map(|h| format!("shape {} {:?}", h.shape, h.contact));
        println!(
            "point ({x}, {z}): brute force {} triangles {:?}; query candidates {} hit {:?}",
            found.len(),
            found.iter().take(4).map(|(y, n)| format!("{y:.3} {n}")).collect::<Vec<_>>(),
            q.candidates.len(),
            hit
        );
    }
    Ok(())
}
