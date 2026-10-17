//! Purchased ordered circuit gates for the independent practice-lap policy.
use crate::route::Route;
use grid_formats::{binxml, progress};
pub use grid_world::lap_timing::Event;
use grid_world::lap_timing::{Course, Policy};
use std::{io::Read, path::Path};
pub struct Prepared {
    pub course: Course,
    pub progress: progress::Progress,
}
/// The default (Donington) route's gates.
pub fn load(root: &Path) -> Result<Prepared, Box<dyn std::error::Error>> {
    load_route(root, &Route::default())
}
pub fn load_route(root: &Path, route: &Route) -> Result<Prepared, Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    std::fs::File::open(root.join(route.file("progress_track.xml")))?
        .take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 16 * 1024 * 1024 {
        return Err("source lap progress exceeds bound".into());
    }
    let document = binxml::read(&bytes)?;
    let mut progress = progress::read(&document)?;
    let finish: Vec<_> = progress
        .routes
        .first()
        .map(|r| {
            r.splits
                .iter()
                .filter(|s| s.kind == progress::SplitKind::Finish)
                .map(|s| s.gate)
                .collect()
        })
        .unwrap_or_default();
    if progress.routes.len() != 1 || finish.len() != 1 {
        return Err("prototype lap timing requires one forward route with one finish gate".into());
    }
    start_at_finish(&mut progress, finish[0]);
    let endpoints: Vec<_> = progress.gates.iter().map(|g| [g.left, g.right]).collect();
    let sectors: Vec<_> = progress.routes[0]
        .splits
        .iter()
        .filter_map(|s| (s.kind == progress::SplitKind::Time).then_some(s.gate))
        .collect();
    let required: Vec<_> = std::iter::once(0).chain(sectors.iter().copied()).collect();
    let course = Course::new(&endpoints, &sectors, Policy::default())?.with_required(&required)?;
    Ok(Prepared { course, progress })
}

/// Re-index a circuit so its finish gate is gate 0: gates, points and their
/// distances are rotated and measured from the finish, and the splits follow.
/// Routes whose finish is already gate 0 (Donington, Nürburgring, Istanbul
/// Park, Long Beach route 0) are unchanged; Donington routes 1-3, Nürburgring
/// route 3, Jarama route 1 and Le Mans finish at a later gate
/// (docs/LONG_BEACH.md "Finish gates").
fn start_at_finish(progress: &mut progress::Progress, finish: usize) {
    if finish == 0 {
        return;
    }
    let n = progress.gates.len();
    let total = progress.total_distance;
    let origin = progress.gates[finish].distance;
    let shift = |d: f32| {
        let d = d - origin;
        if d < 0. { d + total } else { d }
    };
    progress.gates.rotate_left(finish);
    for g in &mut progress.gates {
        g.distance = shift(g.distance);
    }
    let first = progress
        .points
        .iter()
        .position(|p| p.distance >= origin)
        .unwrap_or(0);
    progress.points.rotate_left(first);
    for p in &mut progress.points {
        p.distance = shift(p.distance);
    }
    for split in &mut progress.routes[0].splits {
        split.gate = (split.gate + n - finish) % n;
    }
    progress.routes[0].splits.sort_by_key(|s| s.gate);
}

#[cfg(test)]
mod tests {
    use super::*;
    use progress::{Gate, Point, Progress, Split, SplitKind};

    fn circuit(finish: usize) -> Progress {
        let gate = |d: f32| Gate {
            distance: d,
            left: [d, 0., -5.],
            right: [d, 0., 5.],
        };
        Progress {
            total_distance: 100.,
            gates: [0., 20., 40., 60., 80.].map(gate).into(),
            points: [0., 10., 30., 50., 70., 90.]
                .map(|d| Point {
                    distance: d,
                    position: [d, 0., 0.],
                })
                .into(),
            routes: vec![progress::Route {
                splits: vec![
                    Split {
                        kind: SplitKind::Time,
                        gate: 1,
                    },
                    Split {
                        kind: SplitKind::Finish,
                        gate: finish,
                    },
                    Split {
                        kind: SplitKind::Time,
                        gate: 4,
                    },
                ],
            }],
        }
    }

    #[test]
    fn a_later_finish_gate_becomes_gate_zero() {
        let mut p = circuit(3);
        start_at_finish(&mut p, 3);
        assert_eq!(p.gates[0].left[0], 60.);
        assert_eq!(
            p.gates.iter().map(|g| g.distance).collect::<Vec<_>>(),
            [0., 20., 40., 60., 80.]
        );
        assert_eq!(
            p.gates.iter().map(|g| g.left[0]).collect::<Vec<_>>(),
            [60., 80., 0., 20., 40.]
        );
        assert_eq!(
            p.points
                .iter()
                .map(|q| (q.distance, q.position[0]))
                .collect::<Vec<_>>(),
            [
                (10., 70.),
                (30., 90.),
                (40., 0.),
                (50., 10.),
                (70., 30.),
                (90., 50.)
            ]
        );
        let splits: Vec<_> = p.routes[0]
            .splits
            .iter()
            .map(|s| (s.kind, s.gate))
            .collect();
        assert_eq!(
            splits,
            [
                (SplitKind::Finish, 0),
                (SplitKind::Time, 1),
                (SplitKind::Time, 3)
            ]
        );
        // A finish at gate 0 leaves everything as read.
        let mut q = circuit(0);
        start_at_finish(&mut q, 0);
        assert_eq!(q.gates[0].left[0], 0.);
        assert_eq!(q.routes[0].splits[0].gate, 1);
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT"]
    fn routes_finishing_at_a_later_gate_load() {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let root = Path::new(&root);
        for (path, finish) in [
            ("tracks/europe/donington/route_1", 54),
            ("tracks/europe/nurburgring/route_3", 72),
            ("tracks/europe/jarama/route_1", 67),
            ("tracks/europe/le_mans/route_0", 157),
        ] {
            let route = Route::new(path).unwrap();
            let prepared = load_route(root, &route).unwrap();
            let raw = {
                let bytes = std::fs::read(root.join(route.file("progress_track.xml"))).unwrap();
                let doc = binxml::read(&bytes).unwrap();
                progress::read(&doc).unwrap()
            };
            assert_eq!(prepared.progress.gates.len(), raw.gates.len(), "{path}");
            assert_eq!(
                prepared.progress.gates[0].left, raw.gates[finish].left,
                "{path}"
            );
            assert_eq!(prepared.progress.gates[0].distance, 0., "{path}");
            assert_eq!(prepared.course.gate_count(), raw.gates.len(), "{path}");
            assert_eq!(prepared.course.required_gates().len(), 3, "{path}");
        }
    }
}
