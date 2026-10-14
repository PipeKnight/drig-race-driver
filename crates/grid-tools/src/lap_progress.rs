//! Purchased ordered circuit gates for the independent practice-lap policy.
use grid_formats::{binxml, progress};
pub use grid_world::lap_timing::Event;
use grid_world::lap_timing::{Course, Policy};
use std::{io::Read, path::Path};
pub struct Prepared {
    pub course: Course,
    pub progress: progress::Progress,
}
pub fn load(root: &Path) -> Result<Prepared, Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    std::fs::File::open(root.join("tracks/europe/donington/route_0/progress_track.xml"))?
        .take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 16 * 1024 * 1024 {
        return Err("source lap progress exceeds bound".into());
    }
    let document = binxml::read(&bytes)?;
    let progress = progress::read(&document)?;
    if progress.routes.len() != 1
        || progress.routes[0]
            .splits
            .iter()
            .filter(|s| s.kind == progress::SplitKind::Finish)
            .map(|s| s.gate)
            .collect::<Vec<_>>()
            != [0]
    {
        return Err("prototype lap timing requires one forward route with finish gate zero".into());
    }
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
