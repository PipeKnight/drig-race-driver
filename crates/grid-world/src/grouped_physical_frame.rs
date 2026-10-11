//! Supplied collision pool -> original-style groups -> complete serial solving.
//! Cold all-active bodies, impact contacts and enough row capacity only; geometric
//! detection, callbacks and parallel dispatch remain external; calculate_tick advances.
use crate::{
    Error, contact_groups, contact_rows,
    dynamics::BodyState,
    impact_frame, impact_step,
    physical_pose::{self, Matrix},
    pose,
};

#[derive(Clone)]
pub struct Manager {
    pub parameters: [f32; 11],
    pub gravity: [f32; 4],
    pub correction: Option<impact_frame::Correction>,
    pub materials: Vec<contact_rows::Material>,
    pub rows: Vec<contact_rows::Row>,
    pub capacity: usize,
    pub iterations: usize,
    pub ends: [usize; 4],
    pub properties: Vec<(f32, Matrix)>,
    pub workspace: Vec<[[f32; 4]; 2]>,
    pub cached_pair: Option<[usize; 2]>,
    pub cached_material: contact_rows::Material,
}
pub struct Output {
    pub groups: contact_groups::Output,
    pub manager: Manager,
    /// Root identity, number of bodies, number of contacts, started, corrected.
    pub visits: Vec<(usize, usize, usize, bool, bool)>,
}
fn f(row: &[u32], offset: usize) -> f32 {
    f32::from_bits(row[offset / 4])
}
fn v(row: &[u32], offset: usize) -> [f32; 4] {
    std::array::from_fn(|i| f(row, offset + i * 4))
}
fn m(row: &[u32], offset: usize) -> Matrix {
    std::array::from_fn(|i| v(row, offset + i * 16))
}
fn flag(row: &[u32], offset: usize) -> bool {
    (row[offset / 4] >> (offset % 4 * 8)) & 255 != 0
}
fn set_flag(row: &mut [u32], offset: usize, value: bool) {
    let shift = offset % 4 * 8;
    row[offset / 4] = (row[offset / 4] & !(255 << shift)) | (u32::from(value) << shift);
}
fn store(row: &mut [u32], offset: usize, value: [f32; 4]) {
    row[offset / 4..offset / 4 + 4].copy_from_slice(&value.map(f32::to_bits));
}
fn store_matrix(row: &mut [u32], offset: usize, value: Matrix) {
    for (i, r) in value.into_iter().enumerate() {
        store(row, offset + 16 * i, r);
    }
}
fn chain(head: u32, rows: &[[u32; 0x424 / 4]], offset: usize) -> Vec<usize> {
    let mut result = Vec::new();
    let mut id = head;
    while id != 0 {
        let i = id as usize - 1;
        result.push(i);
        id = rows[i][offset / 4];
    }
    result
}

/// Serial world advance followed by grouping and complete serial solving.
/// Contact offsets are supplied, then translated using the advanced centers;
/// they are not geometrically detected by this function.
pub fn calculate_tick(input: &contact_groups::Input, manager: &Manager) -> Result<Output, Error> {
    contact_groups::validate(input)?;
    let delta = manager.parameters[0];
    if !delta.is_finite() || delta <= 0. || delta > 0.25 {
        return Err(Error("invalid grouped tick delta"));
    }
    let mut next = input.clone();
    let mut update_next = vec![0; input.bodies.len()];
    let mut previous = 0;
    for &i in &input.active_order {
        update_next[i] = previous;
        previous = i as u32 + 1;
    }
    for &i in input.active_order.iter().rev() {
        let raw = &mut next.bodies[i];
        let basis = m(raw, 0x190);
        let published = m(raw, 0x1d0);
        let corrected = pose::predict(
            basis,
            BodyState {
                linear: v(raw, 0x220),
                angular: v(raw, 0x210),
                position: basis[3],
            },
            delta,
        )?;
        let out =
            physical_pose::publish(corrected, v(raw, 0x230), [m(raw, 0x90), m(raw, 0x150)], &[])?;
        store_matrix(raw, 0x270, basis);
        store_matrix(raw, 0x2b0, published);
        store(raw, 0x360, [0.; 4]);
        for (offset, value) in [
            (0x190, out.corrected),
            (0x1d0, out.published),
            (0xd0, out.derived[0]),
            (0x110, out.derived[1]),
        ] {
            store_matrix(raw, offset, value);
        }
    }
    let mut out = calculate(&next, manager)?;
    for (body, next) in out.groups.bodies.iter_mut().zip(update_next) {
        body[0x318 / 4] = next;
    }
    Ok(out)
}

pub fn calculate(input: &contact_groups::Input, manager: &Manager) -> Result<Output, Error> {
    contact_groups::validate(input)?;
    if manager.properties.len() != input.bodies.len()
        || manager.workspace.len() != input.bodies.len()
        || manager.rows.is_empty()
        || manager.rows.len() > 256
        || manager.capacity > manager.rows.len()
        || manager.capacity < input.contacts.len() * 4
        || manager.iterations == 0
        || manager.iterations > 32
        || manager.materials.is_empty()
        || manager.materials.len() > 8
        || manager.properties.iter().any(|(mass, inertia)| {
            !mass.is_finite() || inertia.iter().flatten().any(|v| !v.is_finite())
        })
        || manager
            .workspace
            .iter()
            .flatten()
            .flatten()
            .any(|v| !v.is_finite())
        || input.bodies.iter().any(|b| {
            b[0x28 / 4] != 0
                || f(b, 0x3b4) != f32::MAX
                || [0x338, 0x33c, 0x340].iter().any(|&o| b[o / 4] != 0)
                || (0x40c..=0x410).any(|o| (b[o / 4] >> (o % 4 * 8)) & 255 > 1)
                || (b[0x390 / 4] & 255) > 1
        })
        || input
            .contacts
            .iter()
            .any(|c| flag(c, 0xd) || flag(c, 0x70) || ((c[0xe / 4] >> (0xe % 4 * 8)) & 255) > 1)
    {
        return Err(Error("unsupported grouped physical manager"));
    }
    let mut groups = contact_groups::calculate(input)?;
    let mut manager = manager.clone();
    let mut visits = Vec::new();
    let roots = chain(groups.world[1], &groups.bodies, 0x1c);
    for root in roots {
        let ids = chain(groups.bodies[root][8 / 4], &groups.bodies, 0x2fc);
        let mut contact_ids = Vec::new();
        let mut id = groups.bodies[root][0];
        while id != 0 {
            let i = id as usize - 1;
            contact_ids.push(i);
            id = groups.contacts[i][0x10 / 4];
        }
        let settings = ids
            .iter()
            .map(|&i| {
                let b = &groups.bodies[i];
                impact_frame::Body {
                    basis: m(b, 0x190),
                    local_matrices: [m(b, 0x90), m(b, 0x150)],
                    offset: v(b, 0x230),
                    sleep: std::array::from_fn(|k| f(b, 0x3a0 + 4 * k)),
                    limits: v(b, 0x3b8),
                    damping: v(b, 0x3c8),
                    drag: std::array::from_fn(|k| f(b, 0x3d8 + 4 * k)),
                    mass: f(b, 0x3f0),
                    gravity_scale: f(b, 0x3f8),
                    pending_shift: v(b, 0x380),
                    shift_active: flag(b, 0x390),
                }
            })
            .collect();
        let bodies = ids
            .iter()
            .enumerate()
            .map(|(local, &i)| {
                let b = &groups.bodies[i];
                impact_step::Body {
                    contact: contact_rows::Body {
                        position: v(b, 0x1c0),
                        angular: v(b, 0x210),
                        linear: v(b, 0x220),
                        scale_angular: flag(b, 0x410),
                        directional_friction: flag(b, 0x40f),
                    },
                    inverse_mass: manager.properties[local].0,
                    inverse_inertia: manager.properties[local].1,
                    force_inverse_mass: f(b, 0x3f4),
                    torque_matrix: m(b, 0x110),
                    impulse_accumulator: v(b, 0x240),
                    torque: v(b, 0x250),
                    force: v(b, 0x260),
                    impulse_active: flag(b, 0x40c),
                    apply_torque: flag(b, 0x40d),
                    apply_force: flag(b, 0x40e),
                }
            })
            .collect();
        let contacts = contact_ids
            .iter()
            .map(|&i| {
                let c = &groups.contacts[i];
                contact_rows::Contact {
                    two_bodies: !flag(c, 0xc),
                    bodies: [
                        c[0x1c / 4] as usize,
                        if flag(c, 0xc) {
                            0
                        } else {
                            c[0x20 / 4] as usize
                        },
                    ],
                    materials: [c[0x30 / 4] as usize, c[0x34 / 4] as usize],
                    shapes: [None, None],
                    special: false,
                    group_end: false,
                    disabled: flag(c, 0xe),
                    accumulated: f(c, 0x38),
                    previous_impulse: f(c, 0x3c),
                    point: v(c, 0x50),
                    normal: v(c, 0x60),
                }
            })
            .collect();
        let frame = impact_frame::calculate(&impact_frame::Input {
            bodies: settings,
            gravity: manager.gravity,
            correction: manager.correction.clone(),
            step: impact_step::Input {
                parameters: manager.parameters,
                bodies,
                contacts,
                materials: manager.materials.clone(),
                rows: manager.rows.clone(),
                capacity: manager.capacity,
                iterations: manager.iterations,
                initial_ends: manager.ends,
                workspace: manager.workspace[..ids.len()].to_vec(),
                cached_pair: manager.cached_pair,
                cached_material: manager.cached_material,
            },
        })?;
        if frame.status != 0 {
            return Err(Error(
                "unsupported grouped physical sleep or capacity status",
            ));
        }
        visits.push((
            root,
            ids.len(),
            contact_ids.len(),
            frame.started,
            frame.correction_active,
        ));
        for (local, &i) in ids.iter().enumerate() {
            let raw = &mut groups.bodies[i];
            let b = &frame.step.bodies[local];
            let settings = &frame.bodies[local];
            store(raw, 0x210, b.contact.angular);
            store(raw, 0x220, b.contact.linear);
            store(raw, 0x240, b.impulse_accumulator);
            store(raw, 0x250, b.torque);
            store(raw, 0x260, b.force);
            set_flag(raw, 0x40c, b.impulse_active);
            set_flag(raw, 0x40d, b.apply_torque);
            set_flag(raw, 0x40e, b.apply_force);
            raw[0x3a0 / 4] = settings.sleep[0].to_bits();
            store(raw, 0x380, settings.pending_shift);
            set_flag(raw, 0x390, settings.shift_active);
            if let Some(p) = &frame.publications[local] {
                for (o, value) in [
                    (0x190, p.corrected),
                    (0x1d0, p.published),
                    (0xd0, p.derived[0]),
                    (0x110, p.derived[1]),
                ] {
                    store_matrix(raw, o, value);
                }
            }
            manager.properties[local] = (b.inverse_mass, b.inverse_inertia);
        }
        if frame.started {
            for ((&i, c), metadata) in contact_ids
                .iter()
                .zip(&frame.step.contacts)
                .zip(&frame.step.metadata)
            {
                let raw = &mut groups.contacts[i];
                raw[0x14 / 4] = metadata[0];
                raw[0x18 / 4] = metadata[1];
                set_flag(raw, 0x70, c.group_end);
                raw[0x3c / 4] = c.previous_impulse.to_bits();
            }
        }
        groups.bodies[root][0x30 / 4] = frame.status;
        manager.workspace[..ids.len()].copy_from_slice(&frame.step.workspace);
        manager.ends = frame.step.ends;
        manager.rows = frame.step.rows;
        manager.cached_pair = frame.step.cached_pair;
        manager.cached_material = frame.step.cached_material;
    }
    Ok(Output {
        groups,
        manager,
        visits,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn later_group_pose_overflow_preserves_world_and_shared_manager() {
        let identity =
            std::array::from_fn(|r| std::array::from_fn(|c| if r == c { 1. } else { 0. }));
        let mut bodies = vec![[0; 0x424 / 4]; 2];
        for body in &mut bodies {
            body[0x334 / 4] = 0x1c0;
            for o in [0x90, 0x110, 0x150, 0x190] {
                store_matrix(body, o, identity);
            }
            body[0x3b4 / 4] = f32::MAX.to_bits();
            for o in [0x3b8, 0x3bc, 0x3c0, 0x3c4] {
                body[o / 4] = f32::MAX.to_bits();
            }
            body[0x3f0 / 4] = 1f32.to_bits();
            body[0x3f4 / 4] = 1f32.to_bits();
        }
        bodies[0][0x260 / 4] = 1f32.to_bits();
        set_flag(&mut bodies[0], 0x40e, true);
        bodies[1][0x1c0 / 4] = f32::MAX.to_bits();
        bodies[1][0x380 / 4] = f32::MAX.to_bits();
        set_flag(&mut bodies[1], 0x390, true);
        let input = contact_groups::Input {
            bodies,
            contacts: vec![],
            active_order: vec![0, 1],
            selector: 0,
        };
        let manager = Manager {
            parameters: [0.1, 1., 1., 1., 1., 0.2, 0.1, 0., 1., 1., 0.5],
            gravity: [0.; 4],
            correction: None,
            materials: vec![[0.; 13]],
            rows: vec![[0; 48]; 8],
            capacity: 8,
            iterations: 2,
            ends: [0; 4],
            properties: vec![(1., identity); 2],
            workspace: vec![[[0.; 4]; 2]; 2],
            cached_pair: None,
            cached_material: [0.; 13],
        };
        let before = input.clone();
        let scratch = manager.clone();
        assert!(calculate(&input, &manager).is_err());
        assert!(calculate_tick(&input, &manager).is_err());
        assert_eq!(input.bodies, before.bodies);
        assert_eq!(input.contacts, before.contacts);
        assert_eq!(manager.rows, scratch.rows);
        assert_eq!(manager.workspace, scratch.workspace);
        assert_eq!(manager.properties, scratch.properties);
        assert_eq!(manager.ends, scratch.ends);
        assert_eq!(manager.cached_pair, scratch.cached_pair);
        assert_eq!(manager.cached_material, scratch.cached_material);
        let mut first = input.clone();
        first.bodies.truncate(1);
        first.active_order = vec![0];
        let mut smaller = manager.clone();
        smaller.properties.truncate(1);
        smaller.workspace.truncate(1);
        let success = calculate(&first, &smaller).unwrap();
        assert_eq!(success.visits.len(), 1);
        assert!(success.visits[0].3);
        assert_ne!(
            success.groups.bodies[0][0x220 / 4],
            before.bodies[0][0x220 / 4]
        );
        assert!(!flag(&success.groups.bodies[0], 0x40e));
    }
}
