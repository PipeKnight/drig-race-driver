//! Source-supplied cold reset, regular layout, mode-zero limits and edges.
//! Original resource selection and dirty vehicle rebuild remain separate.
use crate::{
    Error,
    deformation_layout::{self, Bounds, Parameters, WORDS},
    deformation_volume,
    spawn::reference_seed,
};
pub const SETTINGS_WORDS: usize = 0x1054 / 4;
pub struct Output {
    pub reset: Vec<u32>,
    pub layout: Vec<u32>,
    pub initialized: Vec<u32>,
}
fn get(b: &[u32], at: usize) -> f32 {
    f32::from_bits(b[at / 4])
}
fn put(b: &mut [u32], at: usize, x: f32) {
    b[at / 4] = x.to_bits();
}
fn byte(b: &mut [u32], at: usize, x: u8) {
    let shift = (at % 4) * 8;
    b[at / 4] = (b[at / 4] & !(255 << shift)) | (u32::from(x) << shift);
}
fn node(index: usize) -> usize {
    0x2580 + index * 80
}
/// The callback and settings values are opaque tokens; neither is followed.
#[allow(clippy::assign_op_pattern)] // Keep observed SSE operand order explicit.
pub fn calculate(
    descriptor: &[u32],
    bounds: &[Bounds],
    parameters: Parameters,
    settings: &[u32],
    reset_coefficients: [f32; 2],
    callback_token: u32,
    settings_token: u32,
) -> Result<Output, Error> {
    if descriptor.len() != WORDS
        || settings.len() != SETTINGS_WORDS
        || descriptor[0xa410 / 4] != 0
        || settings[0x1000 / 4] > 32
        || reset_coefficients
            .iter()
            .any(|v| !v.is_finite() || !(0. ..=100.).contains(v))
        || [0x1014, 0x1018].iter().any(|o| {
            !get(settings, *o).is_finite() || !(0.0001..=100.).contains(&get(settings, *o))
        })
    {
        return Err(Error("invalid deformation initialization input"));
    }
    let boxes = settings[0x1000 / 4] as usize;
    for i in 0..boxes {
        let b = i * 0x80;
        if (0..16)
            .any(|a| !get(settings, b + a * 4).is_finite() || get(settings, b + a * 4).abs() > 100.)
            || [12, 28, 44].iter().any(|a| get(settings, b + a) != 0.)
            || get(settings, b + 60) != 1.
            || (0..3).any(|a| !(0.0001..=100.).contains(&get(settings, b + 0x40 + a * 4)))
            || get(settings, b + 0x4c) != 0.
            || (0..3).any(|a| !(0. ..=100.).contains(&get(settings, b + 0x50 + a * 4)))
            // Damage is a plain weight; prototypes use -1 on the cabin (F409).
            || !(-100. ..=100.).contains(&get(settings, b + 0x5c))
        {
            return Err(Error("invalid deformation strength box"));
        }
    }
    let mut reset = descriptor.to_vec();
    for at in (0xa4c4..=0xa4ec).step_by(4).chain([
        0xa490, 0xa498, 0xa4b4, 0xa4b8, 0xa4bc, 0xa4c0, 0xa4f8, 0xa440, 0xa444, 0xa448, 0xa44c,
    ]) {
        reset[at / 4] = 0;
    }
    put(&mut reset, 0xa488, reset_coefficients[0]);
    put(&mut reset, 0xa48c, reset_coefficients[1]);
    for at in [0xa4ad, 0xa4ae, 0xa4af, 0xa4ac, 0xa4b1] {
        byte(&mut reset, at, 0);
    }
    reset[0xa4a0 / 4] = u32::MAX;
    reset[0xa484 / 4] = callback_token;
    for at in [0xa420, 0xa4a8, 0xa49c] {
        put(&mut reset, at, 1.);
    }
    let layout =
        deformation_layout::calculate(&reset, bounds, settings_token, parameters)?.descriptor;
    let dims: [usize; 3] = std::array::from_fn(|i| layout[0xa414 / 4 + i] as usize);
    // Keep the already validated settling/sampling integration domain explicit.
    if dims.iter().any(|v| !(2..=16).contains(v)) {
        return Err(Error("unsupported initialization dimensions"));
    }
    let count = dims.iter().product::<usize>();
    let spacing = get(&layout, 0xa420);
    let cube = (spacing * spacing) * spacing;
    let mut initialized = layout.clone();
    initialized[0xa4a0 / 4] = 0;
    for i in 0..count {
        for a in 0..4 {
            put(&mut initialized, node(i) + a * 4, 0.);
            put(
                &mut initialized,
                node(i) + 0x20 + a * 4,
                if a == 3 { 1. } else { 0. },
            );
        }
    }
    let initial_vertices = vec![[0.; 4]; deformation_volume::VERTICES];
    let initial_triangles = vec![[0; 3]; deformation_volume::TRIANGLES];
    for z in 0..dims[2] - 1 {
        for y in 0..dims[1] - 1 {
            for x in 0..dims[0] - 1 {
                let mut planes = vec![[0.; 4]; 12];
                let mut offsets = vec![0.; 12];
                for (a, c) in [x, y, z].into_iter().enumerate() {
                    planes[a * 2][a] = -1.;
                    planes[a * 2 + 1][a] = 1.;
                    offsets[a * 2] = -((c as f32) * spacing + get(&layout, 0xa430 + a * 4));
                    offsets[a * 2 + 1] = ((c + 1) as f32) * spacing + get(&layout, 0xa430 + a * 4);
                }
                let mut volume = 0f32;
                let mut damage = 0f32;
                let mut crush = [0f32; 3];
                for b in 0..boxes {
                    let at = b * 0x80;
                    for a in 0..3 {
                        for side in 0..2 {
                            let sign = if side == 0 { -1. } else { 1. };
                            let p = a * 2 + side + 6;
                            for (lane, value) in planes[p].iter_mut().enumerate() {
                                *value = 0. + sign * get(settings, at + a * 16 + lane * 4);
                            }
                            let n = planes[p];
                            let dot = (n[1] * get(settings, at + 52)
                                + n[0] * get(settings, at + 48))
                                + (n[3] * get(settings, at + 60) + n[2] * get(settings, at + 56));
                            offsets[p] = dot + get(settings, at + 0x40 + a * 4);
                        }
                    }
                    let answer = deformation_volume::calculate(
                        &planes,
                        &offsets,
                        &initial_vertices,
                        &initial_triangles,
                    )?;
                    if answer.triangle_count > 0 {
                        damage = (f64::from(get(settings, at + 0x5c)) * answer.volume
                            + f64::from(damage)) as f32;
                        let v = answer.volume as f32;
                        let weighted = get(settings, at + 0x50) * v;
                        crush[0] += weighted;
                        volume = (answer.volume + f64::from(volume)) as f32;
                        crush[1] = get(settings, at + 0x54) * weighted + crush[1];
                        crush[2] = get(settings, at + 0x58) * weighted + crush[2];
                    }
                }
                let weighted_damage = get(settings, 0x1018) * damage;
                let compression = get(settings, 0x1014);
                let limits = [
                    crush[0] * compression,
                    crush[1] * compression,
                    crush[2] * compression,
                    compression * 0.,
                ];
                for dz in 0..2 {
                    for dy in 0..2 {
                        for dx in 0..2 {
                            let i = ((z + dz) * dims[1] + y + dy) * dims[0] + x + dx;
                            for (a, v) in
                                [volume, weighted_damage, 0., cube].into_iter().enumerate()
                            {
                                let at = node(i) + a * 4;
                                let old = get(&initialized, at);
                                put(&mut initialized, at, v + old);
                            }
                            for (a, v) in limits.into_iter().enumerate() {
                                let at = node(i) + 0x20 + a * 4;
                                let old = get(&initialized, at);
                                put(&mut initialized, at, v + old);
                            }
                        }
                    }
                }
            }
        }
    }
    for i in 0..count {
        let at = node(i);
        let volume = get(&initialized, at);
        if volume <= 0. {
            put(&mut initialized, at + 0x40, 0.);
        } else {
            let seed = reference_seed(volume, true)?;
            for a in 0..4 {
                let scaled = seed * get(&initialized, at + 0x20 + a * 4);
                let value = (1. - seed * volume) * scaled + scaled;
                let lower = if a == 3 { 1. } else { 0. };
                let upper = if a == 3 { 1. } else { 10. };
                let value = if value > lower { value } else { lower };
                let value = if value < upper { value } else { upper };
                put(&mut initialized, at + 0x20 + a * 4, value);
            }
            let marker = get(&initialized, at + 4) / volume;
            put(&mut initialized, at + 0x40, marker.clamp(0., 10.));
        }
        for a in 0..4 {
            initialized[at / 4 + a] = initialized[at / 4 + 4 + a];
        }
    }
    // With zero incoming edges, settling leaves these rest positions unchanged.
    for at in 0xa4f0..=0xa4f4 {
        byte(&mut initialized, at, 1);
    }
    for at in [0xa490, 0xa494, 0xa498, 0xa410] {
        initialized[at / 4] = 0;
    }
    let mut edges = 0usize;
    for z in 0..dims[2] {
        for y in 0..dims[1] {
            for x in 0..dims[0] {
                let i = (z * dims[1] + y) * dims[0] + x;
                for (within, j) in [
                    (x + 1 < dims[0], i + 1),
                    (y + 1 < dims[1], i + dims[0]),
                    (z + 1 < dims[2], i + dims[0] * dims[1]),
                ] {
                    if within {
                        initialized[edges * 2] = (i as u32) | ((j as u32) << 16);
                        if get(&initialized, node(i) + 0x40) > 0.
                            && get(&initialized, node(j) + 0x40) > 0.
                        {
                            edges += 1;
                        }
                    }
                }
            }
        }
    }
    for _ in 0..3 {
        initialized[edges * 2] = 399 | (399 << 16);
        edges += 1;
    }
    initialized[0xa410 / 4] = edges as u32;
    for e in 0..edges {
        let pair = initialized[e * 2];
        let a = (pair & 65535) as usize;
        let b = (pair >> 16) as usize;
        // Terminal self-edges read node399, whose opaque padding may be supplied.
        let d: [f32; 3] = std::array::from_fn(|axis| {
            get(&initialized, node(b) + axis * 4) - get(&initialized, node(a) + axis * 4)
        });
        let square = (d[0] * d[0] + d[1] * d[1]) + d[2] * d[2];
        let length = if square == 0. {
            0.
        } else {
            let seed = reference_seed(square, false)?;
            let root = seed * square;
            (1. - root * seed) * (root * 0.5) + root
        };
        if !length.is_finite() {
            return Err(Error("nonfinite initialized edge"));
        }
        put(
            &mut initialized,
            e * 8 + 4,
            length.clamp(spacing * f32::from_bits(0x3e4ccccd), spacing * 2.),
        );
    }
    byte(&mut initialized, 0xa4f0, 1);
    byte(&mut initialized, 0xa4ae, 1);
    Ok(Output {
        reset,
        layout,
        initialized,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_strength_keeps_nodes_inactive_and_appends_three_terminal_edges() {
        let descriptor = vec![0; WORDS];
        let mut settings = vec![0; SETTINGS_WORDS];
        put(&mut settings, 0x1014, 1.);
        put(&mut settings, 0x1018, 1.);
        let out = calculate(
            &descriptor,
            &[Bounds {
                minimum: [-1., -1., -1., 1.],
                maximum: [1.; 4],
            }],
            Parameters {
                margin: 0.,
                spacing_factor: 0.1,
                spacing_growth: 1.1,
                fallback_xy: -1.,
                fallback_z: -2.,
                flag_a4b0: 3,
            },
            &settings,
            [0.75; 2],
            17,
            0,
        )
        .unwrap();
        assert_eq!(out.initialized[0xa410 / 4], 3);
        assert_eq!(out.initialized[0], 399 | (399 << 16));
        assert_eq!(get(&out.initialized, 0x25c0), 0.);
        assert_eq!(
            (0..4)
                .map(|a| get(&out.initialized, 0x25a0 + a * 4))
                .collect::<Vec<_>>(),
            vec![0., 0., 0., 1.]
        );
        assert_eq!(out.reset[0xa484 / 4], 17);
        assert!(descriptor.iter().all(|v| *v == 0));
        let mut invalid = descriptor.clone();
        invalid[0xa410 / 4] = 1;
        assert!(
            calculate(
                &invalid,
                &[],
                Parameters {
                    margin: 0.,
                    spacing_factor: 0.1,
                    spacing_growth: 1.1,
                    fallback_xy: -1.,
                    fallback_z: -2.,
                    flag_a4b0: 0
                },
                &settings,
                [0.75; 2],
                17,
                0
            )
            .is_err()
        );
        assert_eq!(invalid[0xa410 / 4], 1);
        // The terminal self-edge consumes inactive node399 after all layout,
        // cell, marker and topology work; failure must keep the input intact.
        let mut late = descriptor.clone();
        late[node(399) / 4] = f32::NAN.to_bits();
        let before = late.clone();
        assert!(
            calculate(
                &late,
                &[Bounds {
                    minimum: [-1., -1., -1., 1.],
                    maximum: [1.; 4]
                }],
                Parameters {
                    margin: 0.,
                    spacing_factor: 0.1,
                    spacing_growth: 1.1,
                    fallback_xy: -1.,
                    fallback_z: -2.,
                    flag_a4b0: 3
                },
                &settings,
                [0.75; 2],
                17,
                0
            )
            .is_err()
        );
        assert_eq!(late, before);
    }
}
