//! `--float-audit local/OUT.tsv` (backlog row 102): every placed object,
//! entity and route-split draw against the track collision below it, and
//! every part whose material has no loaded texture. Diagnostic only; it reads
//! the parts exactly as they are drawn and exits.
use super::Part;
use bevy::{
    math::Vec3,
    mesh::{Indices, Mesh, VertexAttributeValues},
};
use grid_tools::{route::Route, track::Track};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write,
    path::Path,
};

/// A base vertex this close to the collision below counts as resting on it.
const TOUCH: f32 = 0.05;
/// Vertices this close to the lowest one form the base.
const BASE_BAND: f32 = 0.02;
const BASE_SAMPLES: usize = 16;
/// The ray starts this far above a vertex and ends this far below it.
const ABOVE: f32 = 1.;
const BELOW: f32 = 30.;

/// The vertices a mesh's indices use, and its triangles over them.
fn referenced(mesh: &Mesh) -> (Vec<Vec3>, Vec<[u32; 3]>) {
    let Some(VertexAttributeValues::Float32x3(positions)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        return (Vec::new(), Vec::new());
    };
    let indices: Vec<usize> = match mesh.indices() {
        Some(Indices::U16(i)) => i.iter().map(|&v| v as usize).collect(),
        Some(Indices::U32(i)) => i.iter().map(|&v| v as usize).collect(),
        None => (0..positions.len()).collect(),
    };
    let used: BTreeSet<usize> = indices.iter().copied().filter(|&i| i < positions.len()).collect();
    let slot: BTreeMap<usize, u32> = used.iter().enumerate().map(|(k, &i)| (i, k as u32)).collect();
    let triangles = indices
        .as_chunks::<3>()
        .0
        .iter()
        .filter_map(|t| Some([*slot.get(&t[0])?, *slot.get(&t[1])?, *slot.get(&t[2])?]))
        .collect();
    (used.into_iter().map(|i| Vec3::from_array(positions[i])).collect(), triangles)
}

/// Height of the collision below `p` (from `ABOVE` over it), if any.
fn ground(track: &Track, p: Vec3) -> Result<Option<f32>, Box<dyn std::error::Error>> {
    let mut cache = grid_world::rectangle_cull::Cache::default();
    let start = [p.x, p.y + ABOVE, p.z, 1.];
    let end = [p.x, p.y - BELOW, p.z, 1.];
    let hit = track.query(start, end, &mut cache).map_err(|e| e.0)?.hit;
    Ok(hit.map(|h| start[1] + (end[1] - start[1]) * h.contact.fraction))
}

/// The drawn track (track scene and route split) as vertical-ray triangles.
#[derive(Default)]
struct Drawn {
    triangles: Vec<[Vec3; 3]>,
    grid: BTreeMap<(i32, i32), Vec<u32>>,
}

impl Drawn {
    const CELL: f32 = 4.;

    fn cell(v: f32) -> i32 {
        (v / Self::CELL).floor() as i32
    }

    fn add(&mut self, mesh: &Mesh, matrix: &bevy::math::Mat4) {
        let Some(VertexAttributeValues::Float32x3(positions)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            return;
        };
        let indices: Vec<usize> = match mesh.indices() {
            Some(Indices::U16(i)) => i.iter().map(|&v| v as usize).collect(),
            Some(Indices::U32(i)) => i.iter().map(|&v| v as usize).collect(),
            None => return,
        };
        for tri in indices.as_chunks::<3>().0 {
            let Some(p) = tri
                .iter()
                .map(|&i| positions.get(i).map(|&p| matrix.transform_point3(Vec3::from_array(p))))
                .collect::<Option<Vec<_>>>()
            else {
                continue;
            };
            let index = self.triangles.len() as u32;
            let (lo, hi) = (p[0].min(p[1]).min(p[2]), p[0].max(p[1]).max(p[2]));
            for cx in Self::cell(lo.x)..=Self::cell(hi.x) {
                for cz in Self::cell(lo.z)..=Self::cell(hi.z) {
                    self.grid.entry((cx, cz)).or_default().push(index);
                }
            }
            self.triangles.push([p[0], p[1], p[2]]);
        }
    }

    /// Height of the highest drawn triangle under `p` within `ABOVE` over it
    /// and `BELOW` under it.
    fn below(&self, p: Vec3) -> Option<f32> {
        let mut best: Option<f32> = None;
        for &i in self.grid.get(&(Self::cell(p.x), Self::cell(p.z)))? {
            let [a, b, c] = self.triangles[i as usize];
            let d = (b.x - a.x) * (c.z - a.z) - (c.x - a.x) * (b.z - a.z);
            if d.abs() < 1e-9 {
                continue;
            }
            let u = ((p.x - a.x) * (c.z - a.z) - (c.x - a.x) * (p.z - a.z)) / d;
            let v = ((b.x - a.x) * (p.z - a.z) - (p.x - a.x) * (b.z - a.z)) / d;
            if u < 0. || v < 0. || u + v > 1. {
                continue;
            }
            let y = a.y + u * (b.y - a.y) + v * (c.y - a.y);
            if y <= p.y + ABOVE && y >= p.y - BELOW && best.is_none_or(|h| y > h) {
                best = Some(y);
            }
        }
        best
    }
}

/// One drawn instance of an object or entity part.
struct Instance<'a> {
    part: &'a str,
    material: &'a str,
    min: Vec3,
    max: Vec3,
    centre: Vec3,
    /// Signed gap of the base vertex nearest the collision; `None`: no
    /// collision below any base vertex.
    gap: Option<f32>,
    /// The same against the drawn track.
    drawn: Option<f32>,
    /// World vertices and triangles (indices into `vertices`).
    vertices: Vec<Vec3>,
    triangles: std::sync::Arc<Vec<[u32; 3]>>,
}

/// Squared distance from `p` to the triangle `abc` (Ericson, Real-Time
/// Collision Detection 5.1.5).
fn distance_squared(p: Vec3, [a, b, c]: [Vec3; 3]) -> f32 {
    let (ab, ac, ap) = (b - a, c - a, p - a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0. && d2 <= 0. {
        return ap.length_squared();
    }
    let bp = p - b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0. && d4 <= d3 {
        return bp.length_squared();
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0. && d1 >= 0. && d3 <= 0. {
        return (ap - ab * (d1 / (d1 - d3))).length_squared();
    }
    let cp = p - c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0. && d5 <= d6 {
        return cp.length_squared();
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0. && d2 >= 0. && d6 <= 0. {
        return (ap - ac * (d2 / (d2 - d6))).length_squared();
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0. && d4 - d3 >= 0. && d5 - d6 >= 0. {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return (bp - (c - b) * w).length_squared();
    }
    let denom = 1. / (va + vb + vc);
    (ap - ab * (vb * denom) - ac * (vc * denom)).length_squared()
}

/// Whether a vertex of `x` lies within `TOUCH` of a triangle of `y`.
fn touches(x: &Instance<'_>, y: &Instance<'_>) -> bool {
    let (lo, hi) = (y.min - TOUCH, y.max + TOUCH);
    let near: Vec<Vec3> = x
        .vertices
        .iter()
        .copied()
        .filter(|v| v.cmpge(lo).all() && v.cmple(hi).all())
        .collect();
    if near.is_empty() {
        return false;
    }
    let (lo, hi) = (x.min - TOUCH, x.max + TOUCH);
    y.triangles.iter().any(|t| {
        let tri = t.map(|i| y.vertices[i as usize]);
        let (a, b) = (tri[0].min(tri[1]).min(tri[2]), tri[0].max(tri[1]).max(tri[2]));
        a.cmple(hi).all()
            && b.cmpge(lo).all()
            && near.iter().any(|&v| distance_squared(v, tri) <= TOUCH * TOUCH)
    })
}

/// Instances held up by something drawn: an instance rests on the collision
/// or the drawn track, or touches a held instance (a tyre on a tyre, a board
/// on its frame, a rail on its posts). Held instances spread from the ones on
/// the ground.
fn held(instances: &[Instance<'_>]) -> Vec<bool> {
    const CELL: f32 = 4.;
    let cell = |v: f32| (v / CELL).floor() as i32;
    let mut grid = BTreeMap::<(i32, i32), Vec<usize>>::new();
    for (i, x) in instances.iter().enumerate() {
        for cx in cell(x.min.x - TOUCH)..=cell(x.max.x + TOUCH) {
            for cz in cell(x.min.z - TOUCH)..=cell(x.max.z + TOUCH) {
                grid.entry((cx, cz)).or_default().push(i);
            }
        }
    }
    let mut held: Vec<bool> = instances
        .iter()
        // Resting on a surface, or set into it and reaching above it (a post,
        // a fence's lower half): either holds what touches it.
        .map(|x| {
            [x.gap, x.drawn].iter().flatten().any(|&g| g <= TOUCH && x.max.y - x.min.y > -g)
        })
        .collect();
    let mut queue: Vec<usize> = (0..instances.len()).filter(|&i| held[i]).collect();
    while let Some(b) = queue.pop() {
        let y = &instances[b];
        let mut seen = BTreeSet::new();
        for cx in cell(y.min.x - TOUCH)..=cell(y.max.x + TOUCH) {
            for cz in cell(y.min.z - TOUCH)..=cell(y.max.z + TOUCH) {
                for &a in grid.get(&(cx, cz)).into_iter().flatten() {
                    if held[a] || !seen.insert(a) {
                        continue;
                    }
                    let x = &instances[a];
                    let overlap = (x.min - TOUCH).cmple(y.max).all() && (y.min - TOUCH).cmple(x.max).all();
                    if overlap && (touches(x, y) || touches(y, x)) {
                        held[a] = true;
                        queue.push(a);
                    }
                }
            }
        }
    }
    held
}

pub fn run(
    root: &Path,
    route: &Route,
    parts: &[Part],
    bindings: &BTreeMap<String, String>,
    textures: &BTreeMap<String, Vec<u8>>,
    out: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let track = Track::read(&std::fs::read(root.join(route.file("track.jpk")))?)?;
    let mut file = std::io::BufWriter::new(
        std::fs::OpenOptions::new().write(true).create_new(true).open(out)?,
    );
    writeln!(file, "kind\tpart\tmaterial\tdrawn_gap\tx\ty\tz\tgap\tsize")?;
    let mut instances = Vec::new();
    // Vertices above the collision by more than TOUCH (and under ABOVE): the
    // route split, and the track scene itself as the baseline.
    let mut vertices = BTreeMap::<&str, (usize, usize)>::new();
    let mut untextured = BTreeMap::<&str, usize>::new();
    let mut drawn = Drawn::default();
    for part in parts.iter().filter(|p| !p.material.starts_with("objects/")) {
        for matrix in &part.matrices {
            drawn.add(&part.mesh, matrix);
        }
    }
    for part in parts {
        match bindings.get(&part.material) {
            Some(texture) if textures.contains_key(texture) => {}
            _ => *untextured.entry(part.material.as_str()).or_default() += 1,
        }
        let kind = if part.material.starts_with("route/") {
            "route"
        } else if part.material.starts_with("objects/") {
            "object"
        } else {
            "track"
        };
        // Damage pieces stay hidden until their prop breaks; then they follow
        // their physics bodies, so their rest pose is not a placement.
        if part.name.starts_with("dynamic/piece/") {
            continue;
        }
        let (local, triangles) = referenced(&part.mesh);
        if local.is_empty() {
            continue;
        }
        let triangles = std::sync::Arc::new(triangles);
        for matrix in &part.matrices {
            let world: Vec<Vec3> = local.iter().map(|&p| matrix.transform_point3(p)).collect();
            if kind != "object" {
                let counts = vertices.entry(kind).or_default();
                for &v in &world {
                    let Some(g) = ground(&track, v)? else { continue };
                    counts.0 += 1;
                    let gap = v.y - g;
                    if gap > TOUCH && gap < ABOVE {
                        counts.1 += 1;
                        if kind == "route" {
                            writeln!(
                                file,
                                "route_vertex\t{}\t{}\t1\t{:.2}\t{:.3}\t{:.2}\t{gap:.3}\t0",
                                part.name, part.material, v.x, v.y, v.z
                            )?;
                        }
                    }
                }
                continue;
            }
            let (mut min, mut max) = (Vec3::INFINITY, Vec3::NEG_INFINITY);
            for &v in &world {
                min = min.min(v);
                max = max.max(v);
            }
            let base: Vec<Vec3> = world.iter().copied().filter(|v| v.y <= min.y + BASE_BAND).collect();
            let step = base.len().div_ceil(BASE_SAMPLES).max(1);
            let nearest = |best: Option<f32>, d: f32| Some(best.map_or(d, |b| if d.abs() < b.abs() { d } else { b }));
            let (mut gap, mut on_drawn) = (None, None);
            for &v in base.iter().step_by(step) {
                if let Some(g) = ground(&track, v)? {
                    gap = nearest(gap, v.y - g);
                }
                if let Some(g) = drawn.below(v) {
                    on_drawn = nearest(on_drawn, v.y - g);
                }
            }
            instances.push(Instance {
                part: &part.name,
                material: &part.material,
                min,
                max,
                centre: base.iter().copied().sum::<Vec3>() / base.len() as f32,
                gap,
                drawn: on_drawn,
                vertices: world,
                triangles: triangles.clone(),
            });
        }
    }
    let (mut floating, mut sunk, mut unknown) = (0usize, 0usize, 0usize);
    let held = held(&instances);
    for (x, _) in instances.iter().zip(&held).filter(|(_, h)| !**h) {
        let (label, gap) = match x.gap {
            None => {
                unknown += 1;
                ("no_ground", f32::NAN)
            }
            Some(g) if g > TOUCH => {
                floating += 1;
                ("floating", g)
            }
            Some(g) => {
                sunk += 1;
                ("sunk", g)
            }
        };
        writeln!(
            file,
            "{label}\t{}\t{}\t{:.3}\t{:.2}\t{:.3}\t{:.2}\t{gap:.3}\t{:.1}",
            x.part,
            x.material,
            x.drawn.unwrap_or(f32::NAN),
            x.centre.x,
            x.centre.y,
            x.centre.z,
            (x.max - x.min).max_element()
        )?;
    }
    for (material, count) in &untextured {
        writeln!(file, "untextured\t-\t{material}\t{count}\t\t\t\t\t")?;
    }
    let (route_all, route_raised) = vertices.get("route").copied().unwrap_or_default();
    let (track_all, track_raised) = vertices.get("track").copied().unwrap_or_default();
    println!(
        "float_audit route={} instances={} held={} floating={floating} sunk={sunk} no_ground={unknown} route_vertices={route_all} route_raised={route_raised} track_vertices={track_all} track_raised={track_raised} untextured_materials={} touch={TOUCH}",
        route.path(),
        instances.len(),
        held.iter().filter(|&&h| h).count(),
        untextured.len()
    );
    Ok(())
}
