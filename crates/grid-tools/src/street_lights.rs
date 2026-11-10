//! The route's track street lights (`street_lights.xml`, docs/NIGHT_LIGHTS.md).
//! Only Haruna and Le Mans ship the file; a track without it has none.
use crate::route::Route;
pub use grid_formats::street_lights::Light;
use std::{io::Read, path::Path};

/// Unnamed lights of the route's track (the ones the night pass draws, as
/// 0x732970 stores them). A missing file yields an empty list.
pub fn load(root: &Path, route: &Route) -> Result<Vec<Light>, Box<dyn std::error::Error>> {
    let path = root.join(route.track_file("street_lights.xml"));
    let file = match std::fs::File::open(&path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    let mut bytes = Vec::new();
    file.take(1024 * 1024 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > 1024 * 1024 {
        return Err("street-light source exceeds bound".into());
    }
    let document = grid_formats::binxml::read(&bytes)?;
    let mut lights = grid_formats::street_lights::read(&document)?;
    lights.retain(|l| l.name.is_empty());
    Ok(lights)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route::{HARUNA, LE_MANS, SPA};

    #[test]
    #[ignore = "requires GRID_GAME_ROOT"]
    fn night_tracks_ship_street_lights() {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let root = Path::new(&root);
        let count = |id| load(root, &Route::new(id).unwrap()).unwrap().len();
        assert_eq!(count(HARUNA), 4);
        assert_eq!(count(LE_MANS), 162);
        assert_eq!(count(SPA), 0);
    }
}
