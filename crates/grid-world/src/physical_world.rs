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
    advance_with_pool(previous, &previous.groups.contacts, 0, shapes)
}

pub fn advance_with_pool(
    previous: &grouped_physical_frame::Output,
    slots: &[contact_groups::Contact],
    used: usize,
    shapes: &[contact_groups::Shape],
) -> Result<grouped_physical_frame::Output, Error> {
    advance_with_geometry(previous, slots, used, shapes, used)
}

/// Advance retained bodies and solver state with supplied fresh contacts.
/// The external prefix stores relative points; the geometric suffix stores
/// world points. Unused slots retain their bytes and owned link capacity.
/// Detection, scheduling and callback consumption remain caller responsibilities.
pub fn advance_with_geometry(
    previous: &grouped_physical_frame::Output,
    slots: &[contact_groups::Contact],
    used: usize,
    shapes: &[contact_groups::Shape],
    external_count: usize,
) -> Result<grouped_physical_frame::Output, Error> {
    advance_owned(previous, slots, used, shapes, external_count, None, &mut [])
}

/// Advance poses, then obtain fresh world-space geometry before grouping/solving.
/// The supplied occupied slots are the relative wheel/external prefix. The
/// provider sees advanced bodies and remaining stable capacity, returns only the
/// geometric suffix, and cannot mutate authoritative input or the prefix.
pub fn advance_with_geometry_provider(
    previous: &grouped_physical_frame::Output,
    slots: &[contact_groups::Contact],
    external_count: usize,
    shapes: &[contact_groups::Shape],
    mut provider: impl FnMut(
        &[contact_groups::Body],
        usize,
    ) -> Result<Vec<contact_groups::Contact>, Error>,
) -> Result<grouped_physical_frame::Output, Error> {
    advance_owned(
        previous,
        slots,
        external_count,
        shapes,
        external_count,
        Some(&mut provider),
        &mut [],
    )
}

/// Geometry-provider advance with one optional deformation registration per
/// body (shared-world cars). A registered car alone in its contact group runs
/// both deformation phases; the registrations share one CRT stream.
pub fn advance_with_deformation(
    previous: &grouped_physical_frame::Output,
    slots: &[contact_groups::Contact],
    external_count: usize,
    shapes: &[contact_groups::Shape],
    provider: &mut grouped_physical_frame::GeometryProvider<'_>,
    registrations: &mut [Option<&mut crate::deformation_world::Registration>],
) -> Result<grouped_physical_frame::Output, Error> {
    advance_owned(
        previous,
        slots,
        external_count,
        shapes,
        external_count,
        Some(provider),
        registrations,
    )
}

#[allow(clippy::too_many_arguments)]
fn advance_owned(
    previous: &grouped_physical_frame::Output,
    slots: &[contact_groups::Contact],
    used: usize,
    shapes: &[contact_groups::Shape],
    external_count: usize,
    provider: Option<&mut grouped_physical_frame::GeometryProvider<'_>>,
    registrations: &mut [Option<&mut crate::deformation_world::Registration>],
) -> Result<grouped_physical_frame::Output, Error> {
    let world = previous.groups.world;
    let selector = world[5];
    if selector > 1
        || previous.pool_used != 0
        || previous.groups.contacts.len() > crate::MAX_WORLD_CONTACTS
        || slots.len() != previous.groups.contacts.len()
        || used > slots.len()
        || external_count > used
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
        contacts: slots[..used].to_vec(),
        active_order: order,
        selector,
    };
    let carry = contact_groups::Carry {
        dirty_head: world[4],
        contact_capacity: previous.groups.contacts.len(),
    };
    let mut out = if let Some(provider) = provider {
        if !registrations.is_empty() {
            grouped_physical_frame::calculate_tick_carried_deforming(
                &input,
                &previous.manager,
                shapes,
                &carry,
                provider,
                registrations,
            )?
        } else {
            grouped_physical_frame::calculate_tick_carried_with_provider(
                &input,
                &previous.manager,
                shapes,
                &carry,
                provider,
            )?
        }
    } else {
        grouped_physical_frame::calculate_tick_carried(
            &input,
            &previous.manager,
            shapes,
            &carry,
            external_count,
        )?
    };
    let active = out.groups.contacts;
    out.groups.contacts = slots.to_vec();
    out.groups.contacts[..active.len()].copy_from_slice(&active);
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

/// Repeated selected external contact construction and whole world solving.
/// Counts are authored decisions; this does not perform geometry queries.
pub fn calculate_produced(
    input: &contact_groups::Input,
    manager: &grouped_physical_frame::Manager,
    shapes: &[contact_groups::Shape],
    requests: &[crate::wheel_contacts::Request],
    counts: &[u32],
) -> Result<grouped_physical_frame::Output, Error> {
    if input.bodies.len() != 1
        || input.contacts.len() != 4
        || requests.len() != 4
        || !(1..=64).contains(&counts.len())
        || counts.iter().any(|&n| n > 4)
        || shapes.len() < 2
        || shapes[0]
            != (contact_groups::Shape {
                angular: false,
                flag217: false,
                flag229: true,
                flag230: false,
            })
        || shapes[1..].iter().any(|s| {
            *s != (contact_groups::Shape {
                angular: true,
                ..Default::default()
            })
        })
    {
        return Err(Error("unsupported selected reproduced world"));
    }
    let mut pool = crate::wheel_contacts::Pool {
        slots: input.contacts.clone(),
        used: 0,
        shapes: shapes.len() - 1,
    };
    crate::wheel_contacts::validate_requests(requests, pool.shapes)?;
    pool =
        crate::wheel_contacts::produce(&input.bodies[0], &pool, &requests[..counts[0] as usize])?;
    let mut first = input.clone();
    first.contacts = pool.slots[..pool.used].to_vec();
    let mut out = calculate_cold(&first, manager, shapes)?;
    pool.slots[..pool.used].copy_from_slice(&out.groups.contacts);
    out.groups.contacts = pool.slots;
    for &count in &counts[1..] {
        let pool = crate::wheel_contacts::Pool {
            slots: out.groups.contacts.clone(),
            used: 0,
            shapes: shapes.len() - 1,
        };
        let produced = crate::wheel_contacts::produce(
            &out.groups.bodies[0],
            &pool,
            &requests[..count as usize],
        )?;
        out = advance_with_pool(&out, &produced.slots, produced.used, shapes)?;
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
        assert!(advance_with_geometry(&out, &contacts, 1, &[], 0).is_err());
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

    #[test]
    fn retained_geometry_point_is_not_translated_by_body_center() {
        let mut previous = completed();
        for offset in [0x1c0, 0x200] {
            for (j, value) in [2f32, 3., 4., 1.].into_iter().enumerate() {
                previous.groups.bodies[0][offset / 4 + j] = value.to_bits();
            }
        }
        let mut c = [0; 0x80 / 4];
        c[1] = 1;
        c[2] = 2;
        c[0xc / 4] = 0x01000001;
        c[0x3c / 4] = f32::MAX.to_bits();
        c[0x64 / 4] = 1f32.to_bits();
        let point = [100f32, 200., 300., 1.].map(f32::to_bits);
        c[0x50 / 4..0x60 / 4].copy_from_slice(&point);
        let geometric = advance_with_geometry(&previous, &[c], 1, &[], 0).unwrap();
        assert_eq!(geometric.groups.contacts[0][0x50 / 4..0x60 / 4], point);
        c[0x5c / 4] = 0;
        let external = advance_with_geometry(&previous, &[c], 1, &[], 1).unwrap();
        assert_eq!(
            external.groups.contacts[0][0x50 / 4..0x60 / 4],
            [102f32, 203., 304., 1.].map(f32::to_bits)
        );
        assert_eq!(geometric.groups.world, external.groups.world);
        assert!(advance_with_geometry(&previous, &[c], 1, &[], 2).is_err());
    }

    #[test]
    fn provider_observes_advanced_pose_and_preserves_prefix_suffix_and_unused_slots() {
        let mut previous = completed();
        previous.groups.contacts.resize(3, [0; 32]);
        previous.groups.contacts[2][0x40 / 4] = 0x12345678;
        for offset in [0x1c0, 0x200] {
            previous.groups.bodies[0][offset / 4] = 2f32.to_bits();
        }
        previous.groups.bodies[0][0x220 / 4] = 5f32.to_bits();
        let mut c = [0; 32];
        c[1] = 1;
        c[2] = 2;
        c[0xc / 4] = 0x01000001;
        c[0x3c / 4] = f32::MAX.to_bits();
        c[0x64 / 4] = 1f32.to_bits();
        let mut slots = previous.groups.contacts.clone();
        slots[0] = c;
        slots[0][0x50 / 4..0x60 / 4].copy_from_slice(&[100f32, 200., 300., 0.].map(f32::to_bits));
        c[0x50 / 4..0x60 / 4].copy_from_slice(&[200f32, 300., 400., 1.].map(f32::to_bits));
        let next_x = 2f32 + 5. * previous.manager.parameters[0];
        let mut calls = 0;
        let out = advance_with_geometry_provider(&previous, &slots, 1, &[], |bodies, remaining| {
            calls += 1;
            assert_eq!(remaining, 2);
            assert_eq!(bodies[0][0x1c0 / 4], next_x.to_bits());
            assert_eq!(bodies[0][0x200 / 4], next_x.to_bits());
            assert_eq!(bodies[0][0x2a0 / 4], 2f32.to_bits());
            Ok(vec![c])
        })
        .unwrap();
        assert_eq!(calls, 1);
        assert_eq!(
            out.groups.contacts[0][0x50 / 4..0x60 / 4],
            [100f32 + next_x, 200., 300., 1.].map(f32::to_bits)
        );
        assert_eq!(
            out.groups.contacts[1][0x50 / 4..0x60 / 4],
            c[0x50 / 4..0x60 / 4]
        );
        assert_eq!(out.groups.contacts[2], slots[2]);
        assert_eq!(out.pool_used, 0);
        let bodies = previous.groups.bodies.clone();
        let world = previous.groups.world;
        let manager = previous.manager.clone();
        let old_slots = slots.clone();
        assert!(
            advance_with_geometry_provider(&previous, &slots, 1, &[], |_, _| Err(Error(
                "provider failure"
            )))
            .is_err()
        );
        assert!(
            advance_with_geometry_provider(&previous, &slots, 1, &[], |_, _| Ok(vec![c; 3]))
                .is_err()
        );
        let mut invalid = c;
        invalid[0x50 / 4] = f32::NAN.to_bits();
        assert!(
            advance_with_geometry_provider(&previous, &slots, 1, &[], |_, _| Ok(vec![invalid]))
                .is_err()
        );
        assert_eq!(previous.groups.bodies, bodies);
        assert_eq!(previous.groups.world, world);
        assert_eq!(slots, old_slots);
        assert_eq!(previous.groups.contacts[2], old_slots[2]);
        assert_eq!(previous.manager.rows, manager.rows);
        assert_eq!(previous.manager.workspace, manager.workspace);
        assert_eq!(previous.manager.properties, manager.properties);
        assert_eq!(previous.manager.ends, manager.ends);
        assert_eq!(previous.manager.cached_pair, manager.cached_pair);
        assert_eq!(previous.manager.cached_material, manager.cached_material);
    }
}
