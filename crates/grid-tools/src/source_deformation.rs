//! Direct purchased-resource deformation construction; explicit player policy.
//! Not yet connected to source_vehicle's dirty wheel/setup publication path.
use grid_formats::{binxml, deformation, pssg};
use grid_world::{
    deformation_initialization::{self, Output},
    deformation_layout::{Bounds, Parameters},
};
use std::{io::Read, path::Path};
pub struct Prepared {
    pub construction: Output,
    pub settings: Vec<u32>,
    pub bounds: deformation::Bounds,
}
fn read(path: &Path, bound: u64) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(bound + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > bound {
        return Err("deformation source exceeds bound".into());
    }
    Ok(bytes)
}
pub fn load(root: &Path, stem: &str) -> Result<Prepared, Box<dyn std::error::Error>> {
    if !["lac", "bmw"].contains(&stem) {
        return Err("unsupported deformation source model".into());
    }
    let ned_bytes = read(
        &root.join(format!("cars/{stem}/{stem}.ned")),
        16 * 1024 * 1024,
    )?;
    let document = binxml::read(&ned_bytes)?;
    let settings = deformation::settings(&document)?;
    let pssg_bytes = read(
        &root.join(format!("cars/{stem}/{stem}_highLOD.pssg")),
        256 * 1024 * 1024,
    )?;
    let tree = pssg::read_tree(&pssg_bytes)?;
    let bounds = deformation::player_lod1_bounds(&tree)?;
    // File-backed scalar defaults verified at the whole constructor boundary.
    // The graphical selection is an explicit prototype policy, not a factory claim.
    let construction = deformation_initialization::calculate(
        &vec![0; grid_world::deformation_layout::WORDS],
        &[Bounds {
            minimum: bounds.minimum,
            maximum: bounds.maximum,
        }],
        Parameters {
            margin: 0.01,
            spacing_factor: 0.025,
            spacing_growth: 1.02,
            fallback_xy: -1.,
            fallback_z: -3.,
            flag_a4b0: 1,
        },
        &settings,
        [0.75; 2],
        0x9a0c90,
        0,
    )?;
    Ok(Prepared {
        construction,
        settings,
        bounds,
    })
}
