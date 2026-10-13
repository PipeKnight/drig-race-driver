//! Complete owned car mapping for the verified whole9BB1E0 wheel update.
use crate::{
    Error,
    deformation_grid::Grid,
    vehicle_publication::VEHICLE_WORDS,
    wheel_deformation::{self, Input, Output, Wheel},
};
fn f(b: &[u32], at: usize) -> f32 {
    f32::from_bits(b[at / 4])
}
fn v(b: &[u32], at: usize) -> [f32; 4] {
    std::array::from_fn(|i| f(b, at + i * 4))
}
fn byte(b: &[u32], at: usize) -> u8 {
    (b[at / 4] >> (at % 4 * 8)) as u8
}
fn put_byte(b: &mut [u32], at: usize, v: u8) {
    let shift = at % 4 * 8;
    b[at / 4] = (b[at / 4] & !(255 << shift)) | (u32::from(v) << shift);
}
pub fn calculate(owner: &[u32]) -> Result<Vec<u32>, Error> {
    if owner.len() != VEHICLE_WORDS
        || owner[0x1620 / 4] != 4
        || byte(owner, 0xc7ad) > 1
        || (0..4).any(|i| byte(owner, 0x22e0 + i) > 1)
    {
        return Err(Error("invalid owned wheel deformation framing"));
    }
    let dimensions: [usize; 3] = std::array::from_fn(|i| owner[0xc714 / 4 + i] as usize);
    if dimensions.iter().any(|d| !(2..=16).contains(d))
        || dimensions.iter().product::<usize>() > 400
    {
        return Err(Error("invalid owned wheel deformation grid"));
    }
    let positions: Vec<_> = (0..dimensions.iter().product::<usize>())
        .map(|i| v(owner, 0x4880 + i * 80))
        .collect();
    let wheels: [Wheel; 4] = std::array::from_fn(|i| {
        let at = 0x1990 + i * 0x150;
        Wheel {
            position: v(owner, at),
            current_radius: f(owner, at + 0xe0),
            normal_radius: f(owner, at + 0xf4),
            deflated_radius: f(owner, at + 0xf8),
            deflated: byte(owner, 0x22e0 + i) != 0,
        }
    });
    let previous = Output {
        displacement: std::array::from_fn(|i| v(owner, 0x1650 + i * 16)),
        angles: std::array::from_fn(|i| f(owner, 0x1790 + i * 4)),
        orientation: std::array::from_fn(|i| {
            std::array::from_fn(|j| v(owner, 0x1690 + i * 64 + j * 16))
        }),
        radius_samples: std::array::from_fn(|i| {
            std::array::from_fn(|j| {
                [
                    f(owner, 0x30c + i * 40 + j * 8),
                    f(owner, 0x310 + i * 40 + j * 8),
                ]
            })
        }),
        deflated: wheels.map(|w| w.deflated),
    };
    let result = wheel_deformation::calculate(Input {
        grid: Grid {
            dimensions,
            spacing: f(owner, 0xc720),
            minimum: v(owner, 0xc730),
            positions: &positions,
        },
        center: v(owner, 0x1630),
        scale: f(owner, 0xeefc),
        angular_limit: f(owner, 0xd070),
        radius_thresholds: [f(owner, 0xe25c), f(owner, 0xe260)],
        active: byte(owner, 0xc7ad) != 0,
        wheels,
        previous,
    })?;
    let mut out = owner.to_vec();
    for (at, values) in [
        (
            0x1650,
            result
                .displacement
                .into_iter()
                .flatten()
                .collect::<Vec<_>>(),
        ),
        (0x1790, result.angles.to_vec()),
        (
            0x1690,
            result.orientation.into_iter().flatten().flatten().collect(),
        ),
        (
            0x30c,
            result
                .radius_samples
                .into_iter()
                .flatten()
                .flatten()
                .collect(),
        ),
    ] {
        for (i, value) in values.into_iter().enumerate() {
            out[at / 4 + i] = value.to_bits();
        }
    }
    for (i, value) in result.deflated.into_iter().enumerate() {
        put_byte(&mut out, 0x22e0 + i, u8::from(value));
    }
    Ok(out)
}
