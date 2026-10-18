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
    if progress.kind == progress::TrackKind::PointToPoint {
        return run(progress, route.subroute());
    }
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

/// A point-to-point route as one run: the subroute's start gate becomes gate
/// 0 and the first finish gate in its direction the last gate; gates and
/// points are re-ordered in travel order and measured from the start, and
/// the time splits between them are the sectors.
///
/// [V] Haruna ships per folder a `forwards` route (start, two time splits,
/// one or two finishes) and a `reverse` route with start and finish
/// swapped; the Yokohama arenas one route with a start and a finish.
/// AUTHORED: of two finishes (Haruna route 0) the first reached ends the run.
fn run(
    mut progress: progress::Progress,
    subroute: u8,
) -> Result<Prepared, Box<dyn std::error::Error>> {
    use progress::SplitKind::{Finish, Start, Time};
    let route = progress
        .routes
        .get(subroute as usize)
        .ok_or("point-to-point subroute absent")?;
    let gates_of = |kind| -> Vec<usize> {
        route
            .splits
            .iter()
            .filter(|s| s.kind == kind)
            .map(|s| s.gate)
            .collect()
    };
    let [start] = gates_of(Start)[..] else {
        return Err("point-to-point run needs one start gate".into());
    };
    let reverse = route.reverse;
    let ahead = |g: usize| if reverse { g < start } else { g > start };
    let finish = gates_of(Finish)
        .into_iter()
        .filter(|&g| ahead(g))
        .min_by_key(|&g| g.abs_diff(start))
        .ok_or("point-to-point run has no finish ahead of its start")?;
    let order: Vec<usize> = if reverse {
        (finish..=start).rev().collect()
    } else {
        (start..=finish).collect()
    };
    let origin = progress.gates[start].distance;
    let end = progress.gates[finish].distance;
    let measure = |d: f32| if reverse { origin - d } else { d - origin };
    let sectors: Vec<usize> = gates_of(Time)
        .into_iter()
        .filter_map(|g| order.iter().position(|&o| o == g))
        .filter(|&i| i > 0 && i + 1 < order.len())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut gates: Vec<_> = std::mem::take(&mut progress.gates)
        .into_iter()
        .enumerate()
        .filter(|(i, _)| order.contains(i))
        .map(|(_, g)| g)
        .collect();
    if reverse {
        gates.reverse();
    }
    for g in &mut gates {
        g.distance = measure(g.distance);
    }
    let (lo, hi) = (origin.min(end), origin.max(end));
    let mut points: Vec<_> = std::mem::take(&mut progress.points)
        .into_iter()
        .filter(|p| (lo..=hi).contains(&p.distance))
        .collect();
    if reverse {
        points.reverse();
    }
    for p in &mut points {
        p.distance = measure(p.distance);
    }
    let last = gates.len() - 1;
    progress.gates = gates;
    progress.points = points;
    progress.total_distance = (end - origin).abs();
    progress.routes = vec![progress::Route {
        splits: std::iter::once(progress::Split {
            kind: Start,
            gate: 0,
        })
        .chain(sectors.iter().map(|&gate| progress::Split { kind: Time, gate }))
        .chain(std::iter::once(progress::Split {
            kind: Finish,
            gate: last,
        }))
        .collect(),
        reverse: false,
    }];
    let endpoints: Vec<_> = progress.gates.iter().map(|g| [g.left, g.right]).collect();
    let required: Vec<_> = std::iter::once(0)
        .chain(sectors.iter().copied())
        .chain(std::iter::once(last))
        .collect();
    let course = Course::open(&endpoints, &sectors, Policy::default())?.with_required(&required)?;
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
            kind: progress::TrackKind::Circuit,
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
                reverse: false,
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
