//! Direct purchased-resource deformation construction; explicit player policy.
//! Graphical selection is an explicit independent prototype policy.
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
    pub consumers: deformation::ConsumerSettings,
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
    // Any purchased car folder (F401); the files below must parse.
    crate::car::Car::new(stem)?;
    let ned_bytes = read(
        &root.join(format!("cars/{stem}/{stem}.ned")),
        16 * 1024 * 1024,
    )?;
    let document = binxml::read(&ned_bytes)?;
    let settings = deformation::settings(&document)?;
    let consumers = deformation::consumer_settings(&document)?;
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
        consumers,
    })
}

/// Initialize over an existing Standard owner, retaining history and descriptor
/// words which the original constructor preserves (including A480=-1).
pub fn initialize_existing(
    prepared: &Prepared,
    owner: &[u32],
) -> Result<Vec<u32>, grid_world::Error> {
    use grid_world::{Error, vehicle_publication::VEHICLE_WORDS};
    if owner.len() != VEHICLE_WORDS
        || owner[0xc7f8 / 4] != 0xd240
        || owner[0xc8a4 / 4] != 0
        || owner[0xd004 / 4] != 0
    {
        return Err(Error("unsupported source deformation owner"));
    }
    let built = deformation_initialization::calculate(
        &owner[0x2300 / 4..0xc800 / 4],
        &[Bounds {
            minimum: prepared.bounds.minimum,
            maximum: prepared.bounds.maximum,
        }],
        Parameters {
            margin: 0.01,
            spacing_factor: 0.025,
            spacing_growth: 1.02,
            fallback_xy: -1.,
            fallback_z: -3.,
            flag_a4b0: 1,
        },
        &prepared.settings,
        [0.75; 2],
        0x9a0c90,
        0xd240,
    )?;
    let mut next = owner.to_vec();
    next[0x2300 / 4..0xc800 / 4].copy_from_slice(&built.initialized);
    next[0xd240 / 4..0xd240 / 4 + prepared.settings.len()].copy_from_slice(&prepared.settings);
    for (at, value) in [
        (0xee40, prepared.consumers.light_cluster_threshold),
        (0xeef8, prepared.consumers.wheel_deformation_amount),
        (0xeefc, prepared.consumers.wheel_movement_level),
    ] {
        next[at / 4] = value.to_bits();
    }
    // Verified outer defaults. No original graphical part/anchor factory claim:
    // this owner has no graphical parts/lights; wheel anchors derive from the
    // already source-initialized wheel positions relative to the grid center.
    for at in [0xcd90, 0xcd94, 0xcd98, 0xcd9c] {
        next[at / 4] = 0.1f32.to_bits();
    }
    for i in 0..4 {
        for a in 0..4 {
            let value = if a == 3 {
                0.
            } else {
                f32::from_bits(next[(0x1990 + i * 0x150) / 4 + a])
                    - f32::from_bits(next[0x1630 / 4 + a])
            };
            next[(0xcda0 + i * 16) / 4 + a] = value.to_bits();
        }
    }
    Ok(next)
}
