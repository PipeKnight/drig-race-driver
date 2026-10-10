//! Source contact/suspension/grip followed by the actual post-grip force interval.
use crate::{
    Error, contact_ray,
    suspension_prepare::{Contact, State},
    vehicle_force::Frame,
    vehicle_grip, vehicle_prepared_frame, wheel_history,
};

#[derive(Clone, Copy, Debug)]
pub struct Material {
    pub history: wheel_history::Material,
    pub prepared: vehicle_prepared_frame::Material,
    pub hardness: f32,
}

/// Source identities remain opaque tokens. Earlier force vectors are supplied.
/// Suspension performs its selected pose advance; this call adds no second one.
/// All body/view/solver/diagnostic state commits only after both stages succeed.
pub fn apply(
    frame: &mut Frame,
    pair: [usize; 2],
    delta: f32,
    query: impl FnMut(usize, contact_ray::Ray) -> Result<Option<Contact>, Error>,
    mut material: impl FnMut(u32) -> Result<Option<Material>, Error>,
) -> Result<[Option<Material>; 4], Error> {
    if [0x3d0 + 0x5d4, 0x3d0 + 0x608, 0x3d0 + 0x60c]
        .into_iter()
        .chain((0..4).flat_map(|i| [0x3d0 + i * 0x170 + 0x178, 0x3d0 + i * 0x170 + 0x17c]))
        .any(|o| frame.body[o / 4] != 0)
    {
        return Err(Error("unowned source-contact pointer/table index"));
    }
    let mut next = frame.clone();
    let owner = next.body[0];
    let mut state = State { words: next.body };
    state.words[0] = 0;
    let mut view = [0; vehicle_grip::VIEW_WORDS];
    view[..next.view.len()].copy_from_slice(&next.view);
    view[next.view.len()..].copy_from_slice(&next.solver[..0x50 / 4]);
    vehicle_grip::apply(&mut state, &mut view, delta, query, |token| {
        Ok(material(token)?.map(|m| m.history))
    })?;
    next.body = state.words;
    next.body[0] = owner;
    let n = next.view.len();
    next.view.copy_from_slice(&view[..n]);
    next.solver[..0x50 / 4].copy_from_slice(&view[n..]);
    let mut materials = [None; 4];
    for (i, slot) in materials.iter_mut().enumerate() {
        let token = next.body[(0x1990 + i * 0x150 + 0xc0) / 4];
        *slot = if token == 0 {
            None
        } else {
            Some(material(token)?.ok_or(Error("missing computed wheel mechanics"))?)
        };
    }
    next.material_multipliers = materials.map(|m| m.map(|m| m.hardness));
    vehicle_prepared_frame::apply(
        &mut next,
        pair,
        materials.map(|m| m.map(|m| m.prepared)),
        delta,
    )?;
    *frame = next;
    Ok(materials)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn later_control_failure_rolls_back_completed_source_suspension_and_grip() {
        let mut state = State {
            words: [0; crate::suspension_prepare::WORDS],
        };
        let c = 0x3d0;
        state.words[(c + 0x5d8) / 4] = 4;
        state.words[0x1620 / 4] = 4;
        for (o, x) in [
            (c, 1.),
            (0x15a0, 1000.),
            (0x1160, 5.),
            (0xfcc, 0.53),
            (0x1f3c, 0.01),
            (0x21fc, 6.),
            (0x21b4, 2.),
            (0x21b8, 5.),
            (0x21bc, 10.),
            (0x21c0, 1.),
        ] {
            state.set(o, x);
        }
        for i in 0..3 {
            state.set(0x15a8 + i * 4, 600.);
        }
        for i in 0..7 {
            state.set(0x2198 + i * 4, 1.);
        }
        for (i, row) in [
            [1., 0., 0., 0.],
            [0., 1., 0., 0.],
            [0., 0., 1., 0.],
            [0., 1., 0., 1.],
        ]
        .into_iter()
        .enumerate()
        {
            state.put(0x15e0 + i * 16, row);
            state.put(0xb40 + i * 16, row);
        }
        for i in 0..4 {
            let w = 0x1990 + i * 0x150;
            state.put(c + i * 0x170 + 0x50, [0., 1., 0., 0.]);
            state.set(c + i * 0x170 + 0x6c, -1000.);
            for (o, x) in [
                (0x48, -0.1),
                (0x4c, 0.2),
                (0x50, 0.25),
                (0xe0, 0.3),
                (0x54, -0.04),
                (0x5c, 20000.),
            ] {
                state.set(w + o, x);
            }
        }
        let mut frame = Frame {
            body: state.words,
            view: [0; crate::vehicle_force::VIEW_WORDS],
            solver: [0xdeadbeef; 884],
            driven: vec![2, 3],
            material_multipliers: [None; 4],
            diagnostics: [1.; 8],
        };
        for i in 0..4 {
            frame.view[(0x298 + i * 4) / 4] = 1f32.to_bits();
        }
        let before = frame.clone();
        let mut queries = 0;
        let error = apply(
            &mut frame,
            [2, 3],
            0.01,
            |_, _| {
                queries += 1;
                Ok(None)
            },
            |_| Ok(None),
        )
        .unwrap_err();
        assert_eq!(queries, 4);
        assert_eq!(error.0, "invalid selected control input");
        assert_eq!(frame.body, before.body);
        assert_eq!(frame.view, before.view);
        assert_eq!(frame.solver, before.solver);
        assert_eq!(frame.diagnostics, before.diagnostics);
        assert_eq!(frame.material_multipliers, before.material_multipliers);
    }
}
