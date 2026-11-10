//! Route reset lines: the `resetlines.cqtc` mesh and the per-car segment test
//! of the race's ResetLines object (race+0x22AC; ctor 0x7E6160, update
//! 0x7E4DF0). A car whose body origin crosses a triangle between two ticks
//! gets the triangle material's RESET type; a final boundary disqualifies it.
//! Only Haruna ships the file (every triangle FBND). The manager arithmetic
//! of `grid_world::reset_lines` is a different object (player resets).
//! docs/RESET_AND_RESPAWN_RESEARCH.md, "Reset lines".
use grid_formats::{binxml, cqtc};
use std::collections::BTreeMap;
use std::path::Path;

/// RESET type of a material (parser 0x7DBD30 [D]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// No RESET block, or an unknown type (0): the update's last branch,
    /// 0x7DD480.
    Other,
    /// "warning", "reset" and "unrecoverable" all give type 3: 0x7DD430,
    /// handler 0x612BD0.
    Unrecoverable,
    /// "finalboundary", type 4: 0x7E1110, handler 0x5EBC90 disqualifies the
    /// car (0x9E64E0(6, 0)) whatever the direction.
    FinalBoundary,
}

/// A material's RESET record: type and "delay" (default 2.0).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reset {
    pub kind: Kind,
    pub delay: f32,
}

/// RESET records by material code from `surface_materials.xml`.
pub fn reset_records(doc: &binxml::Document<'_>) -> BTreeMap<[u8; 4], Reset> {
    fn attr<'a>(e: &binxml::Element<'a>, key: &str) -> Option<&'a str> {
        e.attributes.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
    }
    let mut out = BTreeMap::new();
    for e in doc.elements.iter().filter(|e| e.name == "MATERIAL") {
        let Some(code) = attr(e, "name").and_then(|n| <[u8; 4]>::try_from(n.as_bytes()).ok())
        else {
            continue;
        };
        let Some(block) = e
            .children
            .clone()
            .filter_map(|c| doc.elements.get(c))
            .find(|c| c.name == "RESET")
        else {
            continue;
        };
        let kind = match attr(block, "type") {
            Some("warning" | "reset" | "unrecoverable") => Kind::Unrecoverable,
            Some("finalboundary") => Kind::FinalBoundary,
            _ => Kind::Other,
        };
        let delay = attr(block, "delay")
            .and_then(|d| d.trim().parse().ok())
            .unwrap_or(2.0);
        out.insert(code, Reset { kind, delay });
    }
    out
}

/// One crossing of the last tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Crossing {
    pub car: usize,
    pub reset: Reset,
    /// dot(triangle normal, displacement) < 0 (the handlers' direction
    /// argument). The normal's winding is the CQTC triangle order [I].
    pub against: bool,
    pub triangle: usize,
}

/// Smallest ground cell; a larger mesh gets larger cells (at most 512 a side).
const CELL: f32 = 16.;

/// A route's reset-line triangles, bucketed on a ground grid.
pub struct ResetLines {
    positions: Vec<[f32; 3]>,
    triangles: Vec<[u16; 3]>,
    resets: Vec<Option<Reset>>,
    origin: [f32; 2],
    cell: f32,
    columns: usize,
    rows: usize,
    cells: Vec<Vec<u32>>,
}

impl ResetLines {
    /// The route's `resetlines.cqtc`, or `None` when the route has none.
    pub fn read_route(
        root: &Path,
        route: &crate::route::Route,
    ) -> Result<Option<Self>, Box<dyn std::error::Error>> {
        let path = root.join(route.file("resetlines.cqtc"));
        if !path.is_file() {
            return Ok(None);
        }
        let mesh = std::fs::read(&path)?;
        let surfaces = std::fs::read(root.join("surface_materials.xml"))?;
        if mesh.len() > 4 << 20 || surfaces.len() > 16 << 20 {
            return Err("reset-line source exceeds bound".into());
        }
        let records = reset_records(&binxml::read(&surfaces)?);
        Self::new(&cqtc::read(&mesh)?, &records).map(Some)
    }

    pub fn new(
        chunk: &cqtc::Chunk<'_>,
        records: &BTreeMap<[u8; 4], Reset>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let triangles: Vec<[u16; 3]> = chunk.triangles.iter().map(|t| t.indices).collect();
        if triangles.len() > 1 << 20
            || triangles
                .iter()
                .flatten()
                .any(|&v| v as usize >= chunk.positions.len())
            || chunk.positions.iter().flatten().any(|v| !v.is_finite())
        {
            return Err("invalid reset-line mesh".into());
        }
        let resets = chunk
            .triangles
            .iter()
            .map(|t| {
                chunk
                    .materials
                    .get(t.material as usize)
                    .and_then(|code| records.get(code).copied())
            })
            .collect();
        let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
        for p in &chunk.positions {
            for (k, a) in [0, 2].into_iter().enumerate() {
                lo[k] = lo[k].min(p[a]);
                hi[k] = hi[k].max(p[a]);
            }
        }
        if triangles.is_empty() {
            (lo, hi) = ([0.; 2], [0.; 2]);
        }
        let cell = CELL.max((hi[0] - lo[0]).max(hi[1] - lo[1]) / 512.);
        let columns = (((hi[0] - lo[0]) / cell) as usize + 1).min(513);
        let rows = (((hi[1] - lo[1]) / cell) as usize + 1).min(513);
        let mut lines = Self {
            positions: chunk.positions.clone(),
            triangles,
            resets,
            origin: lo,
            cell,
            columns,
            rows,
            cells: vec![Vec::new(); columns * rows],
        };
        for (i, t) in lines.triangles.iter().enumerate() {
            let p = t.map(|v| lines.positions[v as usize]);
            let (c0, r0) = lines.cell([
                p[0][0].min(p[1][0]).min(p[2][0]),
                p[0][2].min(p[1][2]).min(p[2][2]),
            ]);
            let (c1, r1) = lines.cell([
                p[0][0].max(p[1][0]).max(p[2][0]),
                p[0][2].max(p[1][2]).max(p[2][2]),
            ]);
            for r in r0..=r1 {
                for c in c0..=c1 {
                    lines.cells[r * lines.columns + c].push(i as u32);
                }
            }
        }
        Ok(lines)
    }

    pub fn triangle_count(&self) -> usize {
        self.triangles.len()
    }

    /// The RESET record of a triangle's material.
    pub fn reset(&self, triangle: usize) -> Option<Reset> {
        self.resets.get(triangle).copied().flatten()
    }

    fn cell(&self, p: [f32; 2]) -> (usize, usize) {
        let f = |v: f32, o: f32, n: usize| (((v - o) / self.cell).max(0.) as usize).min(n - 1);
        (
            f(p[0], self.origin[0], self.columns),
            f(p[1], self.origin[1], self.rows),
        )
    }

    /// The nearest triangle the segment `from`→`to` crosses (either side),
    /// with its unit normal: the segment query 0xA71F40 against the reset
    /// mesh's layer.
    pub fn first_hit(&self, from: [f32; 3], to: [f32; 3]) -> Option<(usize, f32, [f32; 3])> {
        if self.triangles.is_empty() {
            return None;
        }
        let (c0, r0) = self.cell([from[0].min(to[0]), from[2].min(to[2])]);
        let (c1, r1) = self.cell([from[0].max(to[0]), from[2].max(to[2])]);
        let d = sub(to, from);
        let mut best: Option<(usize, f32, [f32; 3])> = None;
        for r in r0..=r1 {
            for c in c0..=c1 {
                for &i in &self.cells[r * self.columns + c] {
                    let p = self.triangles[i as usize].map(|v| self.positions[v as usize]);
                    if let Some(t) = segment_triangle(from, d, p)
                        && best.is_none_or(|b| t < b.1)
                    {
                        let n = cross(sub(p[1], p[0]), sub(p[2], p[0]));
                        let l = dot(n, n).sqrt().max(f32::MIN_POSITIVE);
                        best = Some((i as usize, t, n.map(|v| v / l)));
                    }
                }
            }
        }
        best
    }
}

/// Per-car last positions of the ResetLines elements (element words 4..7,
/// zero at registration 0x7E6220).
#[derive(Clone, Debug, Default)]
pub struct Tracker {
    last: Vec<[f32; 3]>,
}

impl Tracker {
    pub fn new(cars: usize) -> Self {
        Self {
            last: vec![[0.; 3]; cars],
        }
    }

    /// One update 0x7E4DF0 for car `car` at body origin `now`. A move of
    /// squared length in (0.001, 25) runs the segment test; a move above
    /// 0.001 replaces the stored position (a smaller one keeps it, so slow
    /// motion accumulates) [D].
    pub fn update(&mut self, lines: &ResetLines, car: usize, now: [f32; 3]) -> Option<Crossing> {
        let last = self.last.get_mut(car)?;
        let d = sub(now, *last);
        let d2 = dot(d, d);
        if d2.is_nan() || d2 <= 0.001 {
            return None;
        }
        let from = std::mem::replace(last, now);
        if d2 >= 25. {
            return None;
        }
        let (triangle, _, normal) = lines.first_hit(from, now)?;
        Some(Crossing {
            car,
            reset: lines.reset(triangle)?,
            against: dot(normal, d) < 0.,
            triangle,
        })
    }
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Two-sided segment/triangle fraction in [0, 1] (Möller–Trumbore).
fn segment_triangle(o: [f32; 3], d: [f32; 3], p: [[f32; 3]; 3]) -> Option<f32> {
    let e1 = sub(p[1], p[0]);
    let e2 = sub(p[2], p[0]);
    let h = cross(d, e2);
    let a = dot(e1, h);
    if a.abs() < 1e-12 {
        return None;
    }
    let f = 1. / a;
    let s = sub(o, p[0]);
    let u = f * dot(s, h);
    if !(0. ..=1.).contains(&u) {
        return None;
    }
    let q = cross(s, e1);
    let v = f * dot(d, q);
    if v < 0. || u + v > 1. {
        return None;
    }
    let t = f * dot(e2, q);
    (0. ..=1.).contains(&t).then_some(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wall(material: [u8; 4], reset: Option<Reset>) -> ResetLines {
        // A vertical wall in the plane x = 10, z in [-50, 50], y in [-5, 5].
        let positions = vec![
            [10., -5., -50.],
            [10., -5., 50.],
            [10., 5., 50.],
            [10., 5., -50.],
        ];
        let mut records = BTreeMap::new();
        if let Some(r) = reset {
            records.insert(material, r);
        }
        let chunk = cqtc::Chunk {
            bounds: [10., -5., -50., 10., 5., 50.],
            materials: vec![material],
            packed_vertices: &[],
            positions,
            triangles: vec![
                grid_formats::collision::Triangle {
                    indices: [0, 1, 2],
                    material: 0,
                    flags: 0,
                },
                grid_formats::collision::Triangle {
                    indices: [0, 2, 3],
                    material: 0,
                    flags: 0,
                },
            ],
            nodes: Vec::new(),
            references: BTreeMap::new(),
        };
        ResetLines::new(&chunk, &records).unwrap()
    }

    const FBND: Reset = Reset {
        kind: Kind::FinalBoundary,
        delay: 2.,
    };

    #[test]
    fn crossing_a_final_boundary_reports_once_per_crossing() {
        let lines = wall(*b"FBND", Some(FBND));
        let mut t = Tracker::new(1);
        // Registration position is zero: the first update from far away
        // (d² >= 25) only records the position.
        assert_eq!(t.update(&lines, 0, [8., 0., 30.]), None);
        assert_eq!(t.update(&lines, 0, [9.5, 0., 30.]), None);
        let c = t.update(&lines, 0, [10.5, 0., 30.]).unwrap();
        assert_eq!((c.car, c.reset), (0, FBND));
        assert_eq!(t.update(&lines, 0, [11.5, 0., 30.]), None);
        // Back across: the other direction.
        let back = t.update(&lines, 0, [9., 0., 30.]).unwrap();
        assert_ne!(back.against, c.against);
    }

    #[test]
    fn a_teleport_or_a_creep_does_not_test_the_segment() {
        let lines = wall(*b"FBND", Some(FBND));
        let mut t = Tracker::new(1);
        t.update(&lines, 0, [5., 0., 0.]);
        t.update(&lines, 0, [9.99, 0., 0.]);
        // Across the wall by 5.1 m in one tick (d² > 25): no test.
        assert_eq!(t.update(&lines, 0, [15.09, 0., 0.]), None);
        // Creeping: steps of 0.02 m (d² = 0.0004) keep the old position until
        // the accumulated move passes 0.001, then that segment is tested.
        let mut t = Tracker::new(1);
        t.update(&lines, 0, [5., 0., 0.]);
        t.update(&lines, 0, [9.99, 0., 0.]);
        assert_eq!(t.update(&lines, 0, [10.01, 0., 0.]), None);
        assert!(t.update(&lines, 0, [10.03, 0., 0.]).is_some());
    }

    #[test]
    fn materials_without_a_reset_block_or_off_the_mesh_report_nothing() {
        let lines = wall(*b"SHTC", None);
        let mut t = Tracker::new(1);
        t.update(&lines, 0, [8., 0., 0.]);
        assert_eq!(t.update(&lines, 0, [11., 0., 0.]), None);
        let lines = wall(*b"FBND", Some(FBND));
        let mut t = Tracker::new(1);
        t.update(&lines, 0, [8., 0., 60.]);
        assert_eq!(t.update(&lines, 0, [11., 0., 60.]), None, "beyond the wall's end");
        t.update(&lines, 0, [8., 6., 0.]);
        assert_eq!(t.update(&lines, 0, [11., 6., 0.]), None, "above the wall");
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT"]
    fn purchased_reset_lines_are_haruna_final_boundaries() {
        let root = std::path::PathBuf::from(
            std::env::var("GRID_GAME_ROOT").expect("GRID_GAME_ROOT"),
        );
        let doc_bytes = std::fs::read(root.join("surface_materials.xml")).unwrap();
        let records = reset_records(&binxml::read(&doc_bytes).unwrap());
        assert_eq!(records[b"FBND"].kind, Kind::FinalBoundary);
        let mut with = Vec::new();
        for region in ["europe", "japan", "usa"] {
            for track in std::fs::read_dir(root.join("tracks").join(region)).unwrap() {
                let track = track.unwrap().path();
                for r in 0..16 {
                    let Ok(route) = crate::route::Route::new(&format!(
                        "tracks/{region}/{}/route_{r}",
                        track.file_name().unwrap().to_str().unwrap()
                    )) else {
                        continue;
                    };
                    if !root.join(route.path()).is_dir() {
                        continue;
                    }
                    if let Some(lines) = ResetLines::read_route(&root, &route).unwrap() {
                        assert!(lines.triangle_count() > 0);
                        assert!((0..lines.triangle_count())
                            .all(|i| lines.reset(i).map(|r| r.kind) == Some(Kind::FinalBoundary)));
                        // A body origin moving 1 m through a triangle's centroid
                        // along its normal crosses that sheet (or a coincident one).
                        for i in (0..lines.triangle_count()).step_by(97) {
                            let p = lines.triangles[i].map(|v| lines.positions[v as usize]);
                            let n = cross(sub(p[1], p[0]), sub(p[2], p[0]));
                            let l = dot(n, n).sqrt();
                            if l < 1e-3 {
                                continue;
                            }
                            let c: [f32; 3] =
                                std::array::from_fn(|k| (p[0][k] + p[1][k] + p[2][k]) / 3.);
                            let from = std::array::from_fn(|k| c[k] - n[k] / l * 0.5);
                            let to = std::array::from_fn(|k| c[k] + n[k] / l * 0.5);
                            let mut t = Tracker::new(1);
                            t.last[0] = from;
                            let hit = t.update(&lines, 0, to).expect("centroid crossing");
                            assert_eq!(hit.reset.kind, Kind::FinalBoundary);
                        }
                        with.push(route.path().to_owned());
                    }
                }
            }
        }
        with.sort();
        assert_eq!(with.len(), 10, "{with:?}");
        assert!(with.iter().all(|p| p.contains("haruna")), "{with:?}");
    }
}
