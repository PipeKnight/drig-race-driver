//! Selected whole serial world step and callback-free wrapper consumption.
//! Cold or carried all-active owners; no registered pose cursor, body shapes, broadphase
//! pairs, joints, sleeping/wakeup, callbacks, allocation or asynchronous workers.
use crate::{Error, contact_groups, grouped_physical_frame};

pub fn calculate_cold(
    input: &contact_groups::Input,
    manager: &grouped_physical_frame::Manager,
    shapes: &[contact_groups::Shape],
) -> Result<grouped_physical_frame::Output, Error> {
    if input.bodies.iter().any(|b| (b[0x414 / 4] >> 8) & 255 != 0) {
        return Err(Error("unsupported deferred world pose publication"));
    }
    // In this owned scope the whole original scheduler's geometry queues are
    // empty. Its advance/group/solve stages are the verified integrated path.
    let mut out = grouped_physical_frame::calculate_tick_with_shapes(input, manager, shapes)?;
    // Original A727A0 consumes the transient pool after callback dispatch. Both
    // owned shape callback slots21C are absent; record bytes remain for reuse.
    out.pool_used = 0;
    Ok(out)
}

/// Advance a completed owned world, retaining consumed pool slots, dirty
/// membership, embedded group links and the shared manager's cache/workspace.
/// No fresh external contacts are produced by this entry.
pub fn advance(
    previous: &grouped_physical_frame::Output,
    shapes: &[contact_groups::Shape],
) -> Result<grouped_physical_frame::Output, Error> {
    let world = previous.groups.world;
    let selector = world[5];
    if selector > 1
        || previous.pool_used != 0
        || previous.groups.contacts.len() > 64
        || previous
            .groups
            .bodies
            .iter()
            .any(|b| (b[0x414 / 4] >> 8) & 255 != 0)
    {
        return Err(Error("invalid completed physical world"));
    }
    let mut order = Vec::new();
    let mut id = world[7 + selector as usize];
    while id != 0 {
        let i = id as usize - 1;
        if i >= previous.groups.bodies.len() || order.contains(&i) {
            return Err(Error("invalid carried world active chain"));
        }
        order.push(i);
        id = previous.groups.bodies[i][0x300 / 4];
    }
    if order.len() != previous.groups.bodies.len()
        || world[7 + (1 - selector) as usize] != 0
        || world[9 + (1 - selector) as usize] != 0
        || order.last().map(|&i| i as u32 + 1) != Some(world[9 + selector as usize])
    {
        return Err(Error("inconsistent carried world heads"));
    }
    let input = contact_groups::Input {
        bodies: previous.groups.bodies.clone(),
        contacts: vec![],
        active_order: order,
        selector,
    };
    let carry = contact_groups::Carry {
        dirty_head: world[4],
        contact_capacity: previous.groups.contacts.len(),
    };
    let mut out =
        grouped_physical_frame::calculate_tick_carried(&input, &previous.manager, shapes, &carry)?;
    out.groups.contacts = previous.groups.contacts.clone();
    out.pool_used = 0;
    Ok(out)
}

pub fn calculate_steps(
    input: &contact_groups::Input,
    manager: &grouped_physical_frame::Manager,
    shapes: &[contact_groups::Shape],
    steps: u32,
) -> Result<grouped_physical_frame::Output, Error> {
    if !(1..=64).contains(&steps) {
        return Err(Error("invalid physical world step count"));
    }
    let mut out = calculate_cold(input, manager, shapes)?;
    for _ in 1..steps {
        out = advance(&out, shapes)?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn completed() -> grouped_physical_frame::Output {
        let identity =
            std::array::from_fn(|r| std::array::from_fn(|c| if r == c { 1f32 } else { 0. }));
        let mut bodies = vec![[0; 0x424 / 4]; 2];
        for b in &mut bodies {
            b[0x334 / 4] = 0x1c0;
            for o in [0x90, 0x110, 0x150, 0x190, 0x1d0] {
                for (i, v) in identity.iter().flatten().enumerate() {
                    b[o / 4 + i] = v.to_bits();
                }
            }
            for o in [0x3b4, 0x3b8, 0x3bc, 0x3c0, 0x3c4] {
                b[o / 4] = f32::MAX.to_bits();
            }
            b[0x3f0 / 4] = 1f32.to_bits();
            b[0x3f4 / 4] = 1f32.to_bits();
        }
        let mut c = [0; 0x80 / 4];
        c[1] = 1;
        c[2] = 2;
        c[0xc / 4] = 0x01000001;
        c[0x3c / 4] = f32::MAX.to_bits();
        c[0x64 / 4] = 1f32.to_bits();
        let input = contact_groups::Input {
            bodies,
            contacts: vec![c],
            active_order: vec![0, 1],
            selector: 0,
        };
        let manager = grouped_physical_frame::Manager {
            parameters: [0.01, 1., 1., 1., 1., 0.2, 0.1, 0., 1., 1., 0.5],
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
        calculate_cold(&input, &manager, &[]).unwrap()
    }

    #[test]
    fn carried_dirty_cycle_and_foreign_active_tail_are_rejected() {
        let mut out = completed();
        let tail = out.groups.world[9 + out.groups.world[5] as usize];
        out.groups.bodies[tail as usize - 1][0x300 / 4] = 17;
        assert_eq!(
            advance(&out, &[]).err().unwrap().0,
            "invalid carried world active chain"
        );
        let mut out = completed();
        let head = out.groups.world[4];
        out.groups.bodies[head as usize - 1][0x2f0 / 4] = head;
        assert_eq!(
            advance(&out, &[]).err().unwrap().0,
            "invalid carried dirty chain"
        );
        let mut out = completed();
        out.pool_used = 1;
        assert_eq!(
            advance(&out, &[]).err().unwrap().0,
            "invalid completed physical world"
        );
    }

    #[test]
    fn failed_carried_publication_retains_full_world_pool_and_manager() {
        let mut out = advance(&completed(), &[]).unwrap();
        let b = &mut out.groups.bodies[1];
        b[0x1c0 / 4] = f32::MAX.to_bits();
        b[0x200 / 4] = f32::MAX.to_bits();
        b[0x380 / 4] = f32::MAX.to_bits();
        b[0x390 / 4] = 1;
        let bodies = out.groups.bodies.clone();
        let contacts = out.groups.contacts.clone();
        let world = out.groups.world;
        let manager = out.manager.clone();
        assert!(advance(&out, &[]).is_err());
        assert_eq!(out.groups.bodies, bodies);
        assert_eq!(out.groups.contacts, contacts);
        assert_eq!(out.groups.world, world);
        assert_eq!(out.pool_used, 0);
        assert_eq!(out.manager.rows, manager.rows);
        assert_eq!(out.manager.workspace, manager.workspace);
        assert_eq!(out.manager.properties, manager.properties);
        assert_eq!(out.manager.ends, manager.ends);
        assert_eq!(out.manager.cached_pair, manager.cached_pair);
        assert_eq!(out.manager.cached_material, manager.cached_material);
    }
}
