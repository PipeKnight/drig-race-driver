//! The route's track start-light sequence (docs/ISTANBUL_PARK.md "Start lights").
//! Races hold the grid for the shipped sequence time (docs/RACE_MODE.md [I]).
use crate::route::Route;
pub use grid_formats::start_lights::Sequence;
use std::{io::Read, path::Path};

pub fn load(root: &Path, route: &Route) -> Result<Sequence, Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    std::fs::File::open(root.join(route.track_file("start_lights.xml")))?
        .take(64 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 64 * 1024 {
        return Err("start-light source exceeds bound".into());
    }
    let document = grid_formats::binxml::read(&bytes)?;
    Ok(grid_formats::start_lights::read(&document)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route::{DONINGTON, ISTANBUL_PARK, JARAMA, LONG_BEACH, NURBURGRING, YOKOHAMA};

    #[test]
    #[ignore = "requires GRID_GAME_ROOT"]
    fn circuits_ship_their_own_start_sequences() {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let root = Path::new(&root);
        let names = |n: &[u8]| -> Vec<String> {
            n.iter()
                .map(|i| format!("start_lights_{i}_e.tga"))
                .collect()
        };
        let donington = load(root, &Route::new(DONINGTON).unwrap()).unwrap();
        assert_eq!(donington.object, "startlights_lights_horizontal");
        assert_eq!(donington.seconds, 4.0);
        assert_eq!(donington.stages, names(&[1, 2, 3, 4, 5, 7]));
        for path in [NURBURGRING, ISTANBUL_PARK, JARAMA] {
            let s = load(root, &Route::new(path).unwrap()).unwrap();
            assert_eq!(s.object, "gg_startlights_lights_horiz", "{path}");
            assert_eq!(s.seconds, 5.0);
            assert_eq!(s.stages, names(&[1, 2, 3, 4, 5, 6, 7]));
        }
        // Long Beach: Donington's board object with the 7-stage, 5.0 s sequence.
        let long_beach = load(root, &Route::new(LONG_BEACH).unwrap()).unwrap();
        assert_eq!(long_beach.object, "startlights_lights_horizontal");
        assert_eq!(long_beach.seconds, 5.0);
        assert_eq!(long_beach.stages, names(&[1, 2, 3, 4, 5, 6, 7]));
        // Yokohama: its own board object, four stages over 2.0 s.
        let yokohama = load(root, &Route::new(YOKOHAMA).unwrap()).unwrap();
        assert_eq!(yokohama.object, "startlights_lights");
        assert_eq!(yokohama.seconds, 2.0);
        assert_eq!(yokohama.stages, names(&[1, 2, 3, 7]));
    }
}
