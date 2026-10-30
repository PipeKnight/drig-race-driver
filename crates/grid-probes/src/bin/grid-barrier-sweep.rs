//! Private static barrier sweep (docs/BARRIER_COLLISION.md): for each route,
//! sample the purchased centreline every few metres and cast horizontal rays
//! to both sides against the port's body collision world (the boundary
//! sheet, packed road triangles, the route prop boxes), and list the boundary
//! sheet's free vertical edges near the track.
//!
//! Usage: grid-barrier-sweep ROOT local/OUT_DIR [tracks/REGION/TRACK/route_N ..]
//! With no routes, every route folder of the installation except
//! tracks/frontend. Writes OUT_DIR/<track>_<route>.tsv (one row per sample
//! and side), OUT_DIR/<track>_<route>-ends.tsv (free sheet ends) and prints
//! one summary line per route.
use grid_formats::{binxml, collision, cqtc, jpk, progress, surface};
use grid_tools::{entity_geometry::Props, route::Route};
use std::{collections::HashMap, fmt::Write as _, fs, path::Path};

/// Sample spacing along the centreline (m).
const STEP: f32 = 4.;
/// Ray reach to each side (m).
const REACH: f32 = 150.;
/// Ray heights above the centreline point (m): a car body's lower and upper half.
const HEIGHTS: [f32; 2] = [0.4, 1.0];
/// XZ cell of the triangle grid (m).
const CELL: f32 = 8.;

#[derive(Clone, Copy, PartialEq)]
enum Source {
    Boundary,
    Road,
    Prop,
    /// A rendered ornament placement's mesh box (no collision in the port
    /// unless its family has a stand-in prop box).
    Ornament,
}

struct Triangle {
    points: [[f32; 3]; 3],
    source: Source,
    /// Material key (boundary/road) or prop box index.
    key: [u8; 4],
    prop: usize,
    sheet: bool,
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn normal(t: &[[f32; 3]; 3]) -> [f32; 3] {
    // The sheet policy's face normal (packed_contacts): (t0-t1) x (t0-t2).
    let n = cross(sub(t[0], t[1]), sub(t[0], t[2]));
    let l = dot(n, n).sqrt().max(1e-12);
    n.map(|x| x / l)
}

/// Two-sided ray/triangle distance (Moller-Trumbore).
fn ray(o: [f32; 3], d: [f32; 3], t: &[[f32; 3]; 3]) -> Option<f32> {
    let e1 = sub(t[1], t[0]);
    let e2 = sub(t[2], t[0]);
    let p = cross(d, e2);
    let det = dot(e1, p);
    if det.abs() < 1e-9 {
        return None;
    }
    let inv = 1. / det;
    let s = sub(o, t[0]);
    let u = dot(s, p) * inv;
    if !(0. ..=1.).contains(&u) {
        return None;
    }
    let q = cross(s, e1);
    let v = dot(d, q) * inv;
    if v < 0. || u + v > 1. {
        return None;
    }
    let distance = dot(e2, q) * inv;
    (distance > 0.).then_some(distance)
}

struct World {
    triangles: Vec<Triangle>,
    cells: HashMap<(i32, i32), Vec<u32>>,
}

impl World {
    fn add(&mut self, t: Triangle) {
        let id = self.triangles.len() as u32;
        let lo = |j: usize| t.points.iter().map(|p| p[j]).fold(f32::MAX, f32::min);
        let hi = |j: usize| t.points.iter().map(|p| p[j]).fold(f32::MIN, f32::max);
        let cell = |x: f32| (x / CELL).floor() as i32;
        for i in cell(lo(0))..=cell(hi(0)) {
            for k in cell(lo(2))..=cell(hi(2)) {
                self.cells.entry((i, k)).or_default().push(id);
            }
        }
        self.triangles.push(t);
    }
    /// Whether a packed road triangle lies under `(x, z)` within 60 m below
    /// `y` (the cell holds every triangle whose xz bounds reach it).
    fn ground(&self, x: f32, y: f32, z: f32) -> bool {
        let c = ((x / CELL).floor() as i32, (z / CELL).floor() as i32);
        self.cells.get(&c).is_some_and(|ids| {
            ids.iter().any(|&id| {
                let t = &self.triangles[id as usize];
                t.source == Source::Road
                    && ray([x, y, z], [0., -1., 0.], &t.points).is_some_and(|d| d < 60.)
            })
        })
    }
    /// Nearest hit per source along `d` within `reach`.
    fn cast(&self, o: [f32; 3], d: [f32; 3], reach: f32, stamp: &mut [u32], mark: u32) -> Hits {
        let mut hits = Hits::default();
        let steps = (reach / (CELL * 0.5)).ceil() as i32;
        let mut last = None;
        for s in 0..=steps {
            let f = (s as f32 * CELL * 0.5).min(reach);
            let c = (
                ((o[0] + d[0] * f) / CELL).floor() as i32,
                ((o[2] + d[2] * f) / CELL).floor() as i32,
            );
            // The cell and its neighbours along the way.
            for (di, dk) in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)] {
                let c = (c.0 + di, c.1 + dk);
                if last == Some(c) {
                    continue;
                }
                let Some(ids) = self.cells.get(&c) else { continue };
                for &id in ids {
                    if stamp[id as usize] == mark {
                        continue;
                    }
                    stamp[id as usize] = mark;
                    let t = &self.triangles[id as usize];
                    let Some(distance) = ray(o, d, &t.points) else { continue };
                    if distance > reach {
                        continue;
                    }
                    let n = normal(&t.points);
                    let slot = match t.source {
                        Source::Boundary => &mut hits.boundary,
                        Source::Prop => &mut hits.prop,
                        Source::Ornament => &mut hits.ornament,
                        Source::Road if n[1].abs() < 0.6 => &mut hits.wall,
                        Source::Road => &mut hits.ground,
                    };
                    // The face normal points back at the car.
                    let facing = dot(n, d) < 0.;
                    match slot {
                        // A coincident face of the other orientation (a
                        // two-sided sheet) still faces the car.
                        Some(h) if (distance - h.distance).abs() < 0.05 => {
                            h.facing |= facing;
                            if distance < h.distance {
                                h.distance = distance;
                                h.id = id;
                            }
                        }
                        Some(h) if distance > h.distance => {}
                        _ => *slot = Some(Hit { distance, id, facing }),
                    }
                }
            }
            last = Some(c);
        }
        hits
    }
}

#[derive(Clone, Copy)]
struct Hit {
    distance: f32,
    id: u32,
    facing: bool,
}

#[derive(Default)]
struct Hits {
    boundary: Option<Hit>,
    wall: Option<Hit>,
    ground: Option<Hit>,
    prop: Option<Hit>,
    ornament: Option<Hit>,
}

fn key(k: [u8; 4]) -> String {
    k.iter()
        .map(|&c| if c.is_ascii_graphic() { c as char } else { '_' })
        .collect()
}

fn routes(root: &Path) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut out = Vec::new();
    for region in fs::read_dir(root.join("tracks"))? {
        let region = region?;
        let r = region.file_name().to_string_lossy().into_owned();
        if r == "frontend" || !region.file_type()?.is_dir() {
            continue;
        }
        for track in fs::read_dir(region.path())? {
            let track = track?;
            if !track.file_type()?.is_dir() {
                continue;
            }
            for route in fs::read_dir(track.path())? {
                let route = route?;
                let name = route.file_name().to_string_lossy().into_owned();
                if name.starts_with("route_") && route.path().join("boundarylines.cqtc").is_file() {
                    out.push(format!(
                        "tracks/{r}/{}/{name}",
                        track.file_name().to_string_lossy()
                    ));
                }
            }
        }
    }
    out.sort();
    Ok(out)
}

/// Mesh extent of every objects.pssg prototype placed by ornaments.bin, as
/// an oriented box per placement (prop index = name index). Placements of
/// prototypes laid out over more than 30 m (group layouts) are skipped.
fn add_ornaments(
    root: &Path,
    route: &Route,
    world: &mut World,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    use grid_formats::{mesh, ornaments, pssg, scene};
    let objects = fs::read(root.join(route.track_file("objects.pssg")))?;
    let tree = pssg::read_tree(&objects)?;
    let sources = mesh::read_static_meshes(&tree)?;
    let mut extents: HashMap<Vec<u8>, ([f32; 3], [f32; 3])> = HashMap::new();
    for d in scene::read_object_draws(&tree, &sources)? {
        let Some(prototype) = d.prototype else { continue };
        let source = &sources[d.source];
        let e = extents
            .entry(prototype.to_vec())
            .or_insert(([f32::MAX; 3], [f32::MIN; 3]));
        for &i in &d.indices {
            let q = scene::transform_point(&d.transform, source.positions[i as usize]);
            for (j, v) in q.iter().enumerate() {
                e.0[j] = e.0[j].min(*v);
                e.1[j] = e.1[j].max(*v);
            }
        }
    }
    let bytes = fs::read(root.join(route.file("ornaments.bin")))?;
    let placements = ornaments::read(&bytes)?;
    let mut names = Vec::new();
    for p in &placements.prototypes {
        let name = String::from_utf8_lossy(p.name).into_owned();
        let Some(&(lo, hi)) = extents.get(format!("{name} Root").as_bytes()) else { continue };
        // Inventory on stderr: route, prototype, words +44/+48, placements, extent.
        eprintln!(
            "prototype\t{}\t{name}\t{:#x}\t{}\t{}\t{:.2}x{:.2}x{:.2}",
            route.path(),
            p.opaque_words[0],
            p.opaque_words[1],
            p.transforms.len(),
            hi[0] - lo[0],
            hi[1] - lo[1],
            hi[2] - lo[2]
        );
        if (0..3).any(|j| hi[j].partial_cmp(&lo[j]).is_none_or(|o| o.is_lt())) || (hi[0] - lo[0]).max(hi[2] - lo[2]) > 30. {
            continue;
        }
        let index = names.len();
        // Record word +48 (the type builder's list flag, docs/TRACK_LOD.md 1.3).
        names.push(format!("{name}[{}]", p.opaque_words[1]));
        for m in &p.transforms {
            let c: Vec<[f32; 3]> = (0..8)
                .map(|i| {
                    let local = std::array::from_fn(|j| if i >> j & 1 == 0 { lo[j] } else { hi[j] });
                    scene::transform_point(m, local)
                })
                .collect();
            box_triangles(&c, Source::Ornament, index, world);
        }
    }
    Ok(names)
}

/// The twelve triangles of a box given by its corners indexed by bits (x, y, z).
fn box_triangles(c: &[[f32; 3]], source: Source, index: usize, world: &mut World) {
    for axis in 0..3 {
        let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
        for side in 0..2 {
            let corner = |a: usize, b: usize| c[(side << axis) | (a << u) | (b << v)];
            for points in [
                [corner(0, 0), corner(1, 0), corner(1, 1)],
                [corner(0, 0), corner(1, 1), corner(0, 1)],
            ] {
                world.add(Triangle {
                    points,
                    source,
                    key: [0; 4],
                    prop: index,
                    sheet: false,
                });
            }
        }
    }
}

fn sweep(
    root: &Path,
    out: &Path,
    path: &str,
    sheets: &HashMap<[u8; 4], bool>,
) -> Result<String, Box<dyn std::error::Error>> {
    let route = Route::new(path)?;
    let mut world = World {
        triangles: Vec::new(),
        cells: HashMap::new(),
    };
    let sheet = |k: [u8; 4]| sheets.get(&k).copied().unwrap_or(false);
    let boundary_bytes = fs::read(root.join(route.file("boundarylines.cqtc")))?;
    let boundary = cqtc::read(&boundary_bytes)?;
    for t in &boundary.triangles {
        let k = boundary.materials[t.material as usize];
        world.add(Triangle {
            points: t.indices.map(|i| boundary.positions[i as usize]),
            source: Source::Boundary,
            key: k,
            prop: 0,
            sheet: sheet(k),
        });
    }
    let boundary_count = world.triangles.len();
    let road_bytes = fs::read(root.join(route.file("track.jpk")))?;
    for entry in jpk::inspect(&road_bytes)?.entries {
        let chunk = collision::read_vcqtc(&road_bytes[entry.stored_data])?;
        for t in &chunk.triangles {
            let k = chunk.materials[t.material as usize];
            world.add(Triangle {
                points: t.indices.map(|i| chunk.positions[i as usize]),
                source: Source::Road,
                key: k,
                prop: 0,
                sheet: sheet(k),
            });
        }
    }
    let props = Props::read_route(root, &route)?;
    for (index, b) in props.boxes.iter().enumerate() {
        if !b.enabled || b.positions.len() != 8 {
            continue;
        }
        box_triangles(&b.positions, Source::Prop, index, &mut world);
    }
    let ornament_names = add_ornaments(root, &route, &mut world)?;
    let progress_bytes = fs::read(root.join(route.file("progress_track.xml")))?;
    let document = binxml::read(&progress_bytes)?;
    let progress = progress::read(&document)?;
    let points = &progress.points;
    if points.len() < 2 {
        return Err("centreline too short".into());
    }
    let mut stamp = vec![0u32; world.triangles.len()];
    let mut mark = 0;
    let mut rows = String::from(
        "distance\tx\ty\tz\tside\theight\tedge\tboundary\tb_key\tb_sheet\tb_facing\twall\tw_key\tground\tprop\tp_template\tnearest\n",
    );
    let total = points.last().unwrap().distance;
    let mut open = 0usize;
    let mut away = 0usize;
    let mut voids = 0usize;
    // Per ornament prototype: rays, nearest distance, first places, gaps.
    let mut uncovered: HashMap<usize, (usize, f32, Vec<String>, Vec<f32>)> = HashMap::new();
    let mut void_runs: Vec<(f32, f32, f32, &str)> = Vec::new();
    let mut samples = 0usize;
    let mut runs: Vec<(f32, f32, &str)> = Vec::new();
    let mut d = 0f32;
    let mut segment = 0usize;
    while d <= total {
        while segment + 2 < points.len() && points[segment + 1].distance < d {
            segment += 1;
        }
        let (a, b) = (&points[segment], &points[segment + 1]);
        let span = (b.distance - a.distance).max(1e-3);
        let f = ((d - a.distance) / span).clamp(0., 1.);
        let p: [f32; 3] = std::array::from_fn(|j| a.position[j] + (b.position[j] - a.position[j]) * f);
        let t = sub(b.position, a.position);
        let l = t[0].hypot(t[2]).max(1e-6);
        let forward = [t[0] / l, 0., t[2] / l];
        // Left of travel with Y up.
        let left = [forward[2], 0., -forward[0]];
        // Track edges from the nearest gate.
        let gate = progress
            .gates
            .iter()
            .min_by(|g, h| (g.distance - d).abs().total_cmp(&(h.distance - d).abs()));
        for (side_name, dir) in [("left", left), ("right", left.map(|x| -x))] {
            let edge = gate.map_or(f32::NAN, |g| {
                [g.left, g.right]
                    .iter()
                    .map(|e| dot(sub(*e, p), dir))
                    .fold(f32::MIN, f32::max)
            });
            let mut any_solid = false;
            let mut faced_away = false;
            let mut void = 0f32;
            for h in HEIGHTS {
                mark += 1;
                let o = [p[0], p[1] + h, p[2]];
                let hits = world.cast(o, dir, REACH, &mut stamp, mark);
                let nearest = [
                    ("boundary", hits.boundary),
                    ("wall", hits.wall),
                    ("ground", hits.ground),
                    ("prop", hits.prop),
                ]
                .into_iter()
                .filter_map(|(n, h)| h.map(|h| (n, h)))
                .min_by(|a, b| a.1.distance.total_cmp(&b.1.distance));
                any_solid |= nearest.is_some();
                // A rendered ornament standing at least 1 m in front of the
                // first collision surface on this ray.
                if let Some(o) = hits.ornament {
                    let solid = [hits.boundary, hits.wall, hits.prop]
                        .into_iter()
                        .flatten()
                        .map(|h| h.distance)
                        .fold(f32::MAX, f32::min);
                    if o.distance + 1. < solid {
                        let name = world.triangles[o.id as usize].prop;
                        let e = uncovered.entry(name).or_insert((0, f32::MAX, Vec::new(), Vec::new()));
                        e.0 += 1;
                        e.1 = e.1.min(o.distance);
                        // The gap to the collision behind it (150 m: none).
                        e.3.push(solid.min(REACH) - o.distance);
                        if e.2.len() < 6 {
                            e.2.push(format!("{side_name}@{d:.0}"));
                        }
                    }
                }
                // Metres of the longest stretch without road collision under
                // it before the first barrier (a car falls out of the world
                // there without touching any wall).
                if h == HEIGHTS[0] {
                    let barrier = [hits.boundary, hits.wall, hits.prop]
                        .into_iter()
                        .flatten()
                        .map(|h| h.distance)
                        .fold(REACH, f32::min);
                    let (mut run, mut s) = (0f32, 1f32);
                    while s < barrier - 1. {
                        let q = [p[0] + dir[0] * s, p[2] + dir[2] * s];
                        if world.ground(q[0], p[1] + 30., q[1]) {
                            run = 0.;
                        } else {
                            run += 1.;
                            void = void.max(run);
                        }
                        s += 1.;
                    }
                }
                if let Some(("boundary", h)) = nearest {
                    let t = &world.triangles[h.id as usize];
                    faced_away |= t.sheet && !h.facing;
                }
                let fmt = |h: Option<Hit>| h.map_or("-".to_owned(), |h| format!("{:.2}", h.distance));
                let b = hits.boundary.map(|h| &world.triangles[h.id as usize]);
                let w = hits.wall.map(|h| &world.triangles[h.id as usize]);
                let pt = hits.prop.map(|h| &world.triangles[h.id as usize]);
                writeln!(
                    rows,
                    "{d:.1}\t{:.2}\t{:.2}\t{:.2}\t{side_name}\t{h}\t{edge:.2}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                    p[0],
                    p[1],
                    p[2],
                    fmt(hits.boundary),
                    b.map_or("-".into(), |t| key(t.key)),
                    b.map_or("-", |t| if t.sheet { "1" } else { "0" }),
                    hits.boundary.map_or("-", |h| if h.facing { "1" } else { "0" }),
                    fmt(hits.wall),
                    w.map_or("-".into(), |t| key(t.key)),
                    fmt(hits.ground),
                    fmt(hits.prop),
                    pt.map_or("-", |t| props.boxes[t.prop].template.as_str()),
                    nearest.map_or("-", |n| n.0),
                )?;
            }
            samples += 1;
            if void >= 3. {
                voids += 1;
                match void_runs.iter_mut().rev().find(|r| r.3 == side_name) {
                    Some(r) if d - r.1 <= STEP * 1.5 => {
                        r.1 = d;
                        r.2 = r.2.max(void);
                    }
                    _ => void_runs.push((d, d, void, side_name)),
                }
            }
            let class = if !any_solid {
                open += 1;
                Some("open")
            } else if faced_away {
                away += 1;
                Some("away")
            } else {
                None
            };
            if let Some(class) = class {
                match runs.last_mut() {
                    Some(r) if r.2 == class && d - r.1 <= STEP * 1.5 => r.1 = d,
                    _ => runs.push((d, d, class)),
                }
            }
        }
        d += STEP;
    }
    let name = path.trim_start_matches("tracks/").replace('/', "_");
    fs::write(out.join(format!("{name}.tsv")), rows)?;

    let mut listed: Vec<_> = uncovered.iter().collect();
    listed.sort_by_key(|e| std::cmp::Reverse(e.1.0));
    let mut text = String::from("prototype\trays\tnearest_m\tmedian_gap_m\tmax_gap_m\twhere\n");
    for (name, (rays, nearest, at, gaps)) in &listed {
        let mut g = gaps.clone();
        g.sort_by(f32::total_cmp);
        writeln!(
            text,
            "{}\t{rays}\t{nearest:.1}\t{:.2}\t{:.2}\t{}",
            ornament_names[**name],
            g[g.len() / 2],
            g[g.len() - 1],
            at.join(",")
        )?;
    }
    fs::write(out.join(format!("{name}-ornaments.tsv")), text)?;
    let top: Vec<String> = listed
        .iter()
        .take(6)
        .map(|(n, (rays, _, _, _))| format!("{}:{rays}", ornament_names[**n]))
        .collect();

    // Free vertical edges of the boundary sheet: welded at 1 cm.
    let weld = |p: [f32; 3]| p.map(|x| (x * 100.).round() as i64);
    type Edge = (usize, [f32; 3], [f32; 3]);
    let mut edges: HashMap<([i64; 3], [i64; 3]), Edge> = HashMap::new();
    for t in &world.triangles[..boundary_count] {
        for k in 0..3 {
            let (a, b) = (t.points[k], t.points[(k + 1) % 3]);
            let (wa, wb) = (weld(a), weld(b));
            let key = if wa <= wb { (wa, wb) } else { (wb, wa) };
            edges.entry(key).or_insert((0, a, b)).0 += 1;
        }
    }
    let mut ends = String::from("x\tz\tcentre_distance\tcentre_at\tnearest_other_end\n");
    let free: Vec<[f32; 3]> = edges
        .values()
        .filter(|(n, a, b)| *n == 1 && (a[1] - b[1]).abs() > 1. && (a[0] - b[0]).hypot(a[2] - b[2]) < 0.05)
        .map(|(_, a, _)| *a)
        .collect();
    let mut near_ends = 0;
    for (i, e) in free.iter().enumerate() {
        let (dist, at) = points
            .iter()
            .map(|p| ((p.position[0] - e[0]).hypot(p.position[2] - e[2]), p.distance))
            .fold((f32::MAX, 0.), |a, b| if b.0 < a.0 { b } else { a });
        let other = free
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, o)| (o[0] - e[0]).hypot(o[2] - e[2]))
            .fold(f32::MAX, f32::min);
        if dist < 40. {
            near_ends += 1;
        }
        writeln!(ends, "{:.2}\t{:.2}\t{dist:.2}\t{at:.1}\t{other:.2}", e[0], e[2])?;
    }
    fs::write(out.join(format!("{name}-ends.tsv")), ends)?;
    let runs_text: Vec<String> = runs
        .iter()
        .map(|(a, b, c)| format!("{c}:{a:.0}-{b:.0}"))
        .collect();
    Ok(format!(
        "{path}\tlength={total:.0}\tsamples={samples}\topen={open}\taway={away}\tvoid={voids}\tuncovered={}\tvoid_runs={}\tboundary_tris={boundary_count}\tprops={}\tfree_ends={}\tfree_ends_near={near_ends}\truns={}",
        top.join(","),
        void_runs
            .iter()
            .map(|(a, b, v, s)| format!("{s}:{a:.0}-{b:.0}:{v:.0}m"))
            .collect::<Vec<_>>()
            .join(","),
        props.boxes.len(),
        free.len(),
        runs_text.join(",")
    ))
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        return Err("usage: grid-barrier-sweep ROOT local/OUT_DIR [tracks/REGION/TRACK/route_N ..]".into());
    }
    let root = Path::new(&args[0]);
    let out = Path::new(&args[1]);
    fs::create_dir_all(out)?;
    if !out.canonicalize()?.starts_with(Path::new("local").canonicalize()?) {
        return Err("output must be under local/".into());
    }
    let surface_bytes = fs::read(root.join("surface_materials.xml"))?;
    let surfaces = binxml::read(&surface_bytes)?;
    let sheets: HashMap<[u8; 4], bool> = surface::read(&surfaces)?
        .iter()
        .map(|m| (m.code, m.physical.sheet))
        .collect();
    let sheet_keys: Vec<String> = sheets.iter().filter(|s| *s.1).map(|s| key(*s.0)).collect();
    println!("sheet materials: {}", sheet_keys.join(","));
    let list = if args.len() > 2 { args[2..].to_vec() } else { routes(root)? };
    for path in &list {
        match sweep(root, out, path, &sheets) {
            Ok(line) => println!("{line}"),
            Err(e) => println!("{path}\terror={e}"),
        }
    }
    Ok(())
}

fn main() {
    let result = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| run().map_err(|e| e.to_string()))
        .expect("sweep worker")
        .join()
        .expect("sweep worker panic");
    if let Err(error) = result {
        eprintln!("grid-barrier-sweep: {error}");
        std::process::exit(1);
    }
}
