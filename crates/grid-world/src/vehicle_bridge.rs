//! Object-pose/center-pose bridge with advance, supplied correction and readback.
//! The schedule is caller-selected; no world dispatch or reset-group ownership.
use crate::{Error, dynamics::BodyState, physical_pose, pose, suspension_prepare};

#[derive(Clone)]
pub struct State {
    /// Pointer-free owned body. Pointer slots must be zero on this interface.
    pub physical: [u32; 0x424 / 4],
    pub vehicle: suspension_prepare::State,
    /// Shape +170 is an opaque selector: 0 absent, 1 own +130, 2 own +F0.
    pub shapes: Vec<[u32; 0x220 / 4]>,
}

pub struct Settings {
    /// Setter notification is supported only when body +400 is not -1.
    pub notify: bool,
    pub advance: Option<f32>,
    pub correction: Option<([f32; 4], [f32; 4])>,
    pub group_head: bool,
}

pub struct Output {
    pub written: State,
    pub advanced: State,
    pub corrected: State,
}

fn vector(d: &[u32], o: usize) -> [f32; 4] {
    std::array::from_fn(|i| f32::from_bits(d[o / 4 + i]))
}
fn matrix(d: &[u32], o: usize) -> physical_pose::Matrix {
    std::array::from_fn(|i| vector(d, o + i * 16))
}
fn store(d: &mut [u32], o: usize, m: physical_pose::Matrix) {
    for (slot, v) in d[o / 4..o / 4 + 16].iter_mut().zip(m.into_iter().flatten()) {
        *slot = v.to_bits();
    }
}
fn shape_matrices(s: &State) -> Vec<Option<physical_pose::Matrix>> {
    s.shapes
        .iter()
        .map(|s| (s[0x214 / 4] & 255 != 0).then(|| matrix(s, 0xf0)))
        .collect()
}
fn commit(s: &mut State, out: physical_pose::Output) {
    store(&mut s.physical, 0x190, out.corrected);
    store(&mut s.physical, 0x1d0, out.published);
    for (o, m) in [0xd0, 0x110].into_iter().zip(out.derived) {
        store(&mut s.physical, o, m);
    }
    for (shape, m) in s.shapes.iter_mut().zip(out.shapes) {
        store(shape, 0x130, m);
    }
}
fn readback(s: &mut State) {
    s.vehicle.words[0x15c0 / 4..0x15d0 / 4].copy_from_slice(&s.physical[0x220 / 4..0x230 / 4]);
    s.vehicle.words[0x15d0 / 4..0x15e0 / 4].copy_from_slice(&s.physical[0x210 / 4..0x220 / 4]);
    s.vehicle.words[0x15e0 / 4..0x1620 / 4].copy_from_slice(&s.physical[0x1d0 / 4..0x210 / 4]);
}

/// Complete selected setters → optional advance → optional correction → getters.
/// Inputs are unchanged on failure. Notification's group-reset branch is rejected.
pub fn calculate(input: &State, settings: &Settings) -> Result<Output, Error> {
    if input.shapes.len() > 64
        || [0x328, 0x32c, 0x334, 0x338, 0x33c, 0x340]
            .iter()
            .any(|o| input.physical[o / 4] != 0)
        || (settings.notify && input.physical[0x400 / 4] == u32::MAX)
        || settings
            .advance
            .is_some_and(|d| !d.is_finite() || d <= 0. || d > 0.25)
        || input
            .shapes
            .iter()
            .any(|s| s[0xe0 / 4] != 0 || s[0x170 / 4] > 2 || s[0x214 / 4] & 255 > 1)
    {
        return Err(Error("unsupported vehicle/physical bridge state"));
    }
    let p = &input.physical;
    let local = [matrix(p, 0x90), matrix(p, 0x150)];
    let offset = vector(p, 0x230);
    let mut published = input.vehicle.basis(0x15e0);
    // B12710 clears orientation W and forces position W before any arithmetic.
    for row in &mut published[..3] {
        row[3] = 0.;
    }
    published[3][3] = 1.;
    if published
        .iter()
        .flatten()
        .chain(local.iter().flatten().flatten())
        .chain(offset.iter())
        .chain(input.vehicle.vector(0x15c0).iter())
        .chain(input.vehicle.vector(0x15d0).iter())
        .any(|v| !v.is_finite())
        || input
            .shapes
            .iter()
            .any(|s| matrix(s, 0xf0).iter().flatten().any(|v| !v.is_finite()))
    {
        return Err(Error("nonfinite vehicle/physical bridge input"));
    }
    let mut next = input.clone();
    commit(
        &mut next,
        physical_pose::from_publication(published, offset, local, &shape_matrices(input))?,
    );
    store(&mut next.physical, 0x40, published);
    for shape in &mut next.shapes {
        let source = match shape[0x170 / 4] {
            1 => Some(matrix(shape, 0x130)),
            2 => Some(matrix(shape, 0xf0)),
            _ => None,
        };
        if let Some(m) = source {
            store(shape, 0x70, m);
        }
    }
    next.physical[0x220 / 4..0x230 / 4]
        .copy_from_slice(&input.vehicle.words[0x15c0 / 4..0x15d0 / 4]);
    next.physical[0x210 / 4..0x220 / 4]
        .copy_from_slice(&input.vehicle.words[0x15d0 / 4..0x15e0 / 4]);
    readback(&mut next);
    let written = next.clone();
    if let Some(delta) = settings.advance {
        let center = matrix(&next.physical, 0x190);
        let old_publication = matrix(&next.physical, 0x1d0);
        store(&mut next.physical, 0x270, center);
        store(&mut next.physical, 0x2b0, old_publication);
        next.physical[0x360 / 4..0x370 / 4].fill(0);
        let center = pose::predict(
            center,
            BodyState {
                linear: vector(&next.physical, 0x220),
                angular: vector(&next.physical, 0x210),
                position: center[3],
            },
            delta,
        )?;
        let out = physical_pose::publish(center, offset, local, &shape_matrices(&next))?;
        commit(&mut next, out);
        let movement: [f32; 4] =
            std::array::from_fn(|i| f32::from_bits(next.physical[0x2a0 / 4 + i]) - center[3][i]);
        for shape in &mut next.shapes {
            for (word, value) in shape[0x40 / 4..0x50 / 4].iter_mut().zip(movement) {
                *word = value.to_bits();
            }
        }
        readback(&mut next);
    }
    let advanced = next.clone();
    if let Some((linear, angular)) = settings.correction {
        let out = physical_pose::calculate(&physical_pose::Input {
            basis: matrix(&next.physical, 0x190),
            offset,
            local_matrices: local,
            group_head: settings.group_head,
            shapes: shape_matrices(&next),
            linear,
            angular,
        })?;
        commit(&mut next, out);
        readback(&mut next);
    }
    Ok(Output {
        written,
        advanced,
        corrected: next,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn center_offset_stays_out_of_vehicle_pose_and_history() {
        let identity = [
            [1., 0., 0., 0.],
            [0., 1., 0., 0.],
            [0., 0., 1., 0.],
            [10., 20., 30., 9.],
        ];
        let mut input = State {
            physical: [0; 0x424 / 4],
            vehicle: suspension_prepare::State {
                words: [0; 0x2600 / 4],
            },
            shapes: vec![[0; 0x220 / 4]],
        };
        store(&mut input.physical, 0x90, identity);
        store(&mut input.physical, 0x150, identity);
        for (slot, value) in input.physical[0x230 / 4..0x240 / 4]
            .iter_mut()
            .zip([0.25_f32, 0.5, 0.75, 0.])
        {
            *slot = value.to_bits();
        }
        for (i, row) in identity.into_iter().enumerate() {
            input.vehicle.put(0x15e0 + i * 16, row);
        }
        input.vehicle.put(0x15c0, [2., 4., 6., 0.]);
        input.shapes[0][0x170 / 4] = 1;
        let out = calculate(
            &input,
            &Settings {
                notify: true,
                advance: Some(0.25),
                correction: Some(([1., 2., 3., 0.], [0.; 4])),
                group_head: false,
            },
        )
        .unwrap();
        assert_eq!(out.written.vehicle.vector(0x1610), [10., 20., 30., 1.]);
        assert_eq!(
            vector(&out.written.physical, 0x1c0),
            [10.25, 20.5, 30.75, 1.]
        );
        assert_eq!(out.advanced.vehicle.vector(0x1610), [10.5, 21., 31.5, 1.]);
        assert_eq!(out.corrected.vehicle.vector(0x1610), [11.5, 23., 34.5, 1.]);
        assert_eq!(vector(&out.corrected.shapes[0], 0xa0), [10., 20., 30., 1.]);
        assert_eq!(
            vector(&out.corrected.shapes[0], 0x40),
            [-0.5, -1., -1.5, 0.]
        );
    }
}
