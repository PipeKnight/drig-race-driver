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
    /// Parking (AD7D60: world +0x7D4 enable, +0x7E0 point, +0x7F0 scale);
    /// None: disabled.
    pub park: Option<([f32; 4], f32)>,
    /// Head of the parked list (manager +0x4C; one-based body id, links
    /// body +0x308 next and +0x30C previous).
    pub parked: u32,
}
#[derive(Clone)]
pub struct Output {
    pub groups: contact_groups::Output,
    pub manager: Manager,
    /// Supplied transient pool remains occupied until the wrapper consumes it.
    pub pool_used: usize,
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
/// B24D70 for one body: B0F2F0(-1) unlinks it from the active list
/// (+0x400 = -1), the velocities are zeroed and the force (B0FCD0), torque
/// (B0FD10) and impulse (B0FC90) accumulators cleared. The timer stays at
/// turnOffTime and the island links stay.
fn sleep_body(groups: &mut contact_groups::Output, i: usize) {
    let list = groups.bodies[i][0x400 / 4] as usize;
    if list > 1 {
        return;
    }
    let next = groups.bodies[i][0x300 / 4];
    let previous = groups.bodies[i][0x304 / 4];
    if previous == 0 {
        groups.world[7 + list] = next;
    } else {
        groups.bodies[previous as usize - 1][0x300 / 4] = next;
    }
    if next == 0 {
        groups.world[9 + list] = previous;
    } else {
        groups.bodies[next as usize - 1][0x304 / 4] = previous;
    }
    let raw = &mut groups.bodies[i];
    raw[0x300 / 4] = 0;
    raw[0x304 / 4] = 0;
    raw[0x400 / 4] = contact_groups::ASLEEP;
    for o in [0x210, 0x220, 0x240, 0x250, 0x260] {
        store(raw, o, [0.; 4]);
    }
    for o in [0x40c, 0x40d, 0x40e] {
        set_flag(raw, o, false);
    }
}

/// B24DF0 for one body: as B24D70, and B0F2F0(-2) marks it parked
/// (+0x413) at the head of the parked list.
fn park_body(groups: &mut contact_groups::Output, parked: &mut u32, i: usize) {
    if groups.bodies[i][0x400 / 4] > 1 {
        return;
    }
    sleep_body(groups, i);
    let id = i as u32 + 1;
    let raw = &mut groups.bodies[i];
    set_flag(raw, 0x413, true);
    raw[0x30c / 4] = 0;
    raw[0x308 / 4] = *parked;
    if *parked != 0 {
        groups.bodies[*parked as usize - 1][0x30c / 4] = id;
    }
    *parked = id;
}

/// AD6480, at the start of the tick: each parked asleep body back within
/// its radius of the park reference wakes its island; the walk restarts at
/// the head when that wake also took the next body off the list.
fn unpark(input: &mut contact_groups::Input, manager: &mut Manager) {
    let Some((point, scale)) = manager.park else {
        return;
    };
    let mut id = manager.parked;
    while id != 0 {
        let k = id as usize - 1;
        let mut next = input.bodies[k][0x308 / 4];
        let b = &input.bodies[k];
        if flag(b, 0x413)
            && b[0x400 / 4] == contact_groups::ASLEEP
            && !impact_frame::out_of_range(v(b, 0x1c0), point, scale, f(b, 0x3fc))
        {
            wake_island(input, &mut manager.parked, k);
            if next != 0 && !flag(&input.bodies[next as usize - 1], 0x413) {
                next = manager.parked;
            }
        }
        id = next;
    }
}

/// B24E70 on the island of asleep body `i` (root +0x18, first +8, next
/// +0x2FC): each asleep body's velocities are zeroed, it goes to the head of
/// the current active list (B0F2F0(+0x60), leaving the parked list) and its
/// timer restarts. Returns whether any body woke.
pub(crate) fn wake_island(input: &mut contact_groups::Input, parked: &mut u32, i: usize) -> bool {
    let bodies = &input.bodies;
    let root = if (bodies[i][0x2c / 4] & 255) != 0 && bodies[i][0x18 / 4] != 0 {
        bodies[i][0x18 / 4] as usize - 1
    } else {
        i
    };
    let mut members = Vec::new();
    let mut id = bodies[root][8 / 4];
    while id != 0 && members.len() <= bodies.len() {
        let k = id as usize - 1;
        if k >= bodies.len() || members.contains(&k) {
            break;
        }
        members.push(k);
        id = bodies[k][0x2fc / 4];
    }
    if !members.contains(&i) {
        members.push(i);
    }
    let mut woke = false;
    for k in members {
        if input.bodies[k][0x400 / 4] != contact_groups::ASLEEP {
            continue;
        }
        woke = true;
        let head = input.active_order.first().map_or(0, |&h| h as u32 + 1);
        if head != 0 {
            input.bodies[head as usize - 1][0x304 / 4] = k as u32 + 1;
        }
        contact_groups::leave_parked(&mut input.bodies, parked, k);
        let raw = &mut input.bodies[k];
        store(raw, 0x210, [0.; 4]);
        store(raw, 0x220, [0.; 4]);
        raw[0x300 / 4] = head;
        raw[0x304 / 4] = 0;
        raw[0x400 / 4] = input.selector;
        raw[0x3a0 / 4] = 0;
        input.active_order.insert(0, k);
    }
    woke
}

/// Whether world body `i` is asleep (+0x400 = -1).
pub fn asleep(out: &Output, i: usize) -> bool {
    out.groups.bodies[i][0x400 / 4] == contact_groups::ASLEEP
}

/// B24E70 between steps, on a completed world: the island of asleep body
/// `i` goes to the head of the current active list (world selector), its
/// velocities zeroed and its timers restarted. Joint creation (B825E0) and
/// removal (B82430) call it.
pub fn wake(out: &mut Output, i: usize) {
    let selector = out.groups.world[5];
    let mut input = contact_groups::Input {
        bodies: std::mem::take(&mut out.groups.bodies),
        contacts: Vec::new(),
        active_order: Vec::new(),
        selector,
        joints: Vec::new(),
    };
    let head = out.groups.world[7 + selector as usize];
    let before = input.bodies.len();
    wake_island(&mut input, &mut out.manager.parked, i);
    // `wake_island` linked the woken bodies to each other, newest first;
    // the last of them goes before the old head.
    if let Some(&last) = input.active_order.last() {
        input.bodies[last][0x300 / 4] = head;
        if head != 0 {
            input.bodies[head as usize - 1][0x304 / 4] = last as u32 + 1;
        } else {
            out.groups.world[9 + selector as usize] = last as u32 + 1;
        }
        out.groups.world[7 + selector as usize] = input.active_order[0] as u32 + 1;
    }
    debug_assert_eq!(before, input.bodies.len());
    out.groups.bodies = input.bodies;
}

/// Restart body `i`'s sleep timer (+0x3A0), as B82430 does for an awake
/// body whose joint goes.
pub fn restart_timer(out: &mut Output, i: usize) {
    out.groups.bodies[i][0x3a0 / 4] = 0;
}

/// Asleep bodies a contact names (A, and B for a two-body contact).
pub(crate) fn asleep_in(
    input: &contact_groups::Input,
    contacts: &[contact_groups::Contact],
) -> Vec<usize> {
    let mut out = Vec::new();
    for c in contacts {
        let single = (c[0xc / 4] & 255) != 0;
        let ids = [c[1], if single { 0 } else { c[8 / 4] }];
        for id in ids {
            if id == 0 {
                continue;
            }
            let i = id as usize - 1;
            if input
                .bodies
                .get(i)
                .is_some_and(|b| b[0x400 / 4] == contact_groups::ASLEEP)
                && !out.contains(&i)
            {
                out.push(i);
            }
        }
    }
    out
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
    calculate_tick_with_shapes(input, manager, &[])
}

pub fn calculate_tick_with_shapes(
    input: &contact_groups::Input,
    manager: &Manager,
    shapes: &[contact_groups::Shape],
) -> Result<Output, Error> {
    tick(
        input.clone(),
        manager.clone(),
        shapes,
        None,
        input.contacts.len(),
        None,
        &mut [],
    )
}

/// Advance bodies, then solve an external relative-point prefix followed by
/// world-space geometric contacts. Geometry generation remains external.
pub fn calculate_tick_with_geometry(
    input: &contact_groups::Input,
    manager: &Manager,
    shapes: &[contact_groups::Shape],
    external_count: usize,
) -> Result<Output, Error> {
    if external_count > input.contacts.len() {
        return Err(Error("invalid external contact prefix"));
    }
    tick(
        input.clone(),
        manager.clone(),
        shapes,
        None,
        external_count,
        None,
        &mut [],
    )
}

pub(crate) fn calculate_tick_carried(
    input: &contact_groups::Input,
    manager: &Manager,
    shapes: &[contact_groups::Shape],
    carry: &contact_groups::Carry,
    external_count: usize,
) -> Result<Output, Error> {
    if external_count > input.contacts.len() {
        return Err(Error("invalid external contact prefix"));
    }
    tick(
        input.clone(),
        manager.clone(),
        shapes,
        Some(carry),
        external_count,
        None,
        &mut [],
    )
}

/// Called after the bodies are predicted: it may move bodies first (the
/// continuous stage, crate::continuous), then returns the step's contacts.
pub(crate) type GeometryProvider<'a> = dyn FnMut(&mut [contact_groups::Body], usize) -> Result<Vec<contact_groups::Contact>, Error>
    + 'a;

pub(crate) fn calculate_tick_carried_with_provider(
    input: contact_groups::Input,
    manager: Manager,
    shapes: &[contact_groups::Shape],
    carry: &contact_groups::Carry,
    provider: &mut GeometryProvider<'_>,
) -> Result<Output, Error> {
    let external_count = input.contacts.len();
    tick(
        input,
        manager,
        shapes,
        Some(carry),
        external_count,
        Some(provider),
        &mut [],
    )
}
pub(crate) fn calculate_tick_carried_deforming(
    input: contact_groups::Input,
    manager: Manager,
    shapes: &[contact_groups::Shape],
    carry: &contact_groups::Carry,
    provider: &mut GeometryProvider<'_>,
    registrations: &mut [Option<&mut crate::deformation_world::Registration>],
) -> Result<Output, Error> {
    let external_count = input.contacts.len();
    tick(
        input,
        manager,
        shapes,
        Some(carry),
        external_count,
        Some(provider),
        registrations,
    )
}

fn validate(
    input: &contact_groups::Input,
    shapes: &[contact_groups::Shape],
    carry: Option<&contact_groups::Carry>,
) -> Result<(), Error> {
    if let Some(carry) = carry {
        contact_groups::validate_carried(input, shapes, carry)
    } else {
        contact_groups::validate_with_shapes(input, shapes)
    }
}

#[allow(clippy::too_many_arguments)]
fn tick(
    input: contact_groups::Input,
    manager: Manager,
    shapes: &[contact_groups::Shape],
    carry: Option<&contact_groups::Carry>,
    external_count: usize,
    provider: Option<&mut GeometryProvider<'_>>,
    registrations: &mut [Option<&mut crate::deformation_world::Registration>],
) -> Result<Output, Error> {
    validate(&input, shapes, carry)?;
    let delta = manager.parameters[0];
    if !delta.is_finite() || delta <= 0. || delta > 0.25 {
        return Err(Error("invalid grouped tick delta"));
    }
    let (mut input, mut manager) = (input, manager);
    unpark(&mut input, &mut manager);
    let mut update_next = vec![0; input.bodies.len()];
    let mut previous = 0;
    for &i in &input.active_order {
        update_next[i] = previous;
        previous = i as u32 + 1;
    }
    let active_order = input.active_order.clone();
    let mut next = input;
    let predict_timer = crate::stage_timing::Timer::start(crate::stage_timing::Stage::Predict);
    for &i in active_order.iter().rev() {
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
    drop(predict_timer);
    if let Some(provider) = provider {
        let capacity = carry
            .ok_or(Error("geometry provider requires retained capacity"))?
            .contact_capacity;
        let remaining = capacity
            .checked_sub(next.contacts.len())
            .ok_or(Error("invalid geometry capacity"))?;
        let mut contacts = provider(&mut next.bodies, remaining)?;
        if contacts.len() > remaining {
            return Err(Error("geometry provider exceeded remaining capacity"));
        }
        // B184F0: an asleep body in a pair with an awake one wakes its island
        // (B24E70 in AD7110) and the pairs are collected again, so the woken
        // bodies' contacts are solved this tick. A pair the second pass finds
        // with a body that is still asleep (another island) waits for the
        // next tick's broad phase.
        let mut woke = false;
        for i in asleep_in(&next, &contacts) {
            woke |= wake_island(&mut next, &mut manager.parked, i);
        }
        if woke {
            contacts = provider(&mut next.bodies, remaining)?;
            if contacts.len() > remaining {
                return Err(Error("geometry provider exceeded remaining capacity"));
            }
            let still = asleep_in(&next, &contacts);
            contacts.retain(|c| {
                let single = (c[0xc / 4] & 255) != 0;
                let named = |id: u32| id != 0 && still.contains(&(id as usize - 1));
                !named(c[1]) && (single || !named(c[8 / 4]))
            });
        }
        next.contacts.extend(contacts);
    }
    // AD6C40: the external pool's asleep bodies wake their islands.
    let external = next.contacts[..external_count.min(next.contacts.len())].to_vec();
    for i in asleep_in(&next, &external) {
        wake_island(&mut next, &mut manager.parked, i);
    }
    let mut out = calculate_impl(next, manager, shapes, carry, external_count, registrations)?;
    for (body, next) in out.groups.bodies.iter_mut().zip(update_next) {
        body[0x318 / 4] = next;
    }
    Ok(out)
}

pub fn calculate(input: &contact_groups::Input, manager: &Manager) -> Result<Output, Error> {
    calculate_with_shapes(input, manager, &[])
}

pub fn calculate_with_shapes(
    input: &contact_groups::Input,
    manager: &Manager,
    shapes: &[contact_groups::Shape],
) -> Result<Output, Error> {
    calculate_impl(
        input.clone(),
        manager.clone(),
        shapes,
        None,
        input.contacts.len(),
        &mut [],
    )
}

fn calculate_impl(
    input: contact_groups::Input,
    manager: Manager,
    shapes: &[contact_groups::Shape],
    carry: Option<&contact_groups::Carry>,
    external_count: usize,
    registrations: &mut [Option<&mut crate::deformation_world::Registration>],
) -> Result<Output, Error> {
    validate(&input, shapes, carry)?;
    if !registrations.is_empty() && registrations.len() != input.bodies.len() {
        return Err(Error("deformation registrations differ from owned bodies"));
    }
    // One CRT stream and one bulk owner-order toggle shared by every
    // registered car, consumed in group order.
    let mut shared_seed = registrations.iter().flatten().map(|r| r.random_seed).next();
    let mut shared_toggle = registrations
        .iter()
        .flatten()
        .map(|r| r.owner_toggle)
        .next()
        .unwrap_or(false);
    if manager.properties.len() != input.bodies.len()
        || manager.workspace.len() != input.bodies.len()
        || manager.rows.is_empty()
        || manager.rows.len() > crate::MAX_SOLVER_ROWS
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
    let pool_used = input.contacts.len();
    let mut manager = manager;
    let islands_timer = crate::stage_timing::Timer::start(crate::stage_timing::Stage::Islands);
    let mut groups = if let Some(carry) = carry {
        contact_groups::calculate_carried_owned(
            input,
            shapes,
            carry,
            external_count,
            &mut manager.parked,
        )?
    } else {
        contact_groups::calculate_with_geometry(&input, shapes, external_count)?
    };
    drop(islands_timer);
    let mut visits = Vec::new();
    // AD8130: status 1 sleeps (B24D70), status 2 parks (B24DF0), island by
    // island after every frame.
    let mut resting: Vec<(u32, Vec<usize>)> = Vec::new();
    let roots = chain(groups.world[1], &groups.bodies, 0x1c);
    for root in roots {
        let pack_timer = crate::stage_timing::Timer::start(crate::stage_timing::Stage::IslandPack);
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
                    two_bodies: !flag(c, 0xc) && c[8 / 4] != 0,
                    bodies: [
                        c[0x1c / 4] as usize,
                        if flag(c, 0xc) || c[8 / 4] == 0 {
                            0
                        } else {
                            c[0x20 / 4] as usize
                        },
                    ],
                    materials: [c[0x30 / 4] as usize, c[0x34 / 4] as usize],
                    shapes: [c[0x2c / 4], c[0x28 / 4]]
                        .map(|id| id.checked_sub(1).map(|i| shapes[i as usize].angular)),
                    special: flag(c, 0x73),
                    group_end: false,
                    disabled: flag(c, 0xe),
                    accumulated: f(c, 0x38),
                    previous_impulse: f(c, 0x3c),
                    point: v(c, 0x50),
                    normal: v(c, 0x60),
                }
            })
            .collect();
        // B7B850's joint pools: the group's joints with their bodies' local
        // indices (body +0x404 is the position in `ids`).
        let local = |id: u32| groups.bodies[id as usize - 1][0x404 / 4] as usize;
        let joint_ids = &groups.joint_groups[root];
        let joints = impact_step::Joints {
            bodies: if joint_ids.is_empty() {
                Vec::new()
            } else {
                ids.iter().map(|&i| groups.bodies[i]).collect()
            },
            list: joint_ids
                .iter()
                .map(|&k| {
                    let w = &groups.joints[k];
                    impact_step::IslandJoint {
                        joint: w.joint.clone(),
                        a: local(w.a),
                        b: (w.b != 0 && w.joint.byte(0xc) == 0).then(|| local(w.b)),
                        fixed: w.fixed.clone(),
                    }
                })
                .collect(),
        };
        // B7B850 tests the park range only for an island held by a one-body
        // or static contact.
        let anchored = || {
            contact_ids.iter().any(|&i| {
                let c = &groups.contacts[i];
                flag(c, 0xc) || c[8 / 4] == 0
            })
        };
        let park = manager
            .park
            .filter(|_| anchored())
            .map(|(point, scale)| impact_frame::Park {
                point,
                scale,
                radii: ids.iter().map(|&i| f(&groups.bodies[i], 0x3fc)).collect(),
            });
        let mut frame_input = impact_frame::Input {
            park,
            bodies: settings,
            gravity: manager.gravity,
            correction: manager.correction.clone(),
            step: impact_step::Input {
                parameters: manager.parameters,
                bodies,
                contacts,
                materials: manager.materials.clone(),
                // Moved, not copied: the frame hands it back (or the call fails
                // and this working copy of the manager is discarded).
                rows: std::mem::take(&mut manager.rows),
                capacity: manager.capacity,
                iterations: manager.iterations,
                initial_ends: manager.ends,
                workspace: manager.workspace[..ids.len()].to_vec(),
                cached_pair: manager.cached_pair,
                cached_material: manager.cached_material,
                joints,
            },
        };
        drop(pack_timer);
        let frame_timer =
            crate::stage_timing::Timer::start(crate::stage_timing::Stage::IslandFrame);
        // Selected scope: a registered car alone in its group (static peers) runs
        // both deformation phases; groups of several cars solve without them.
        let several_registered = ids.len() >= 2
            && ids
                .iter()
                .any(|&i| registrations.get(i).is_some_and(Option::is_some));
        let registered = match ids[..] {
            [only] => registrations.get_mut(only).and_then(|r| r.as_deref_mut()),
            _ => None,
        };
        let _kind_timer = crate::stage_timing::Timer::start(if several_registered {
            crate::stage_timing::count(
                crate::stage_timing::Stage::GroupContacts,
                contact_ids.len(),
            );
            crate::stage_timing::Stage::FrameGroup
        } else if registered.is_some() {
            crate::stage_timing::Stage::FrameCar
        } else {
            crate::stage_timing::Stage::FramePlain
        });
        let frame = if several_registered {
            // Car-to-car contact group: per-owner deformation phases.
            let mut taken: Vec<_> = ids.iter().map(|&i| registrations[i].take()).collect();
            let raw: Vec<_> = contact_ids.iter().map(|&i| groups.contacts[i]).collect();
            let bodies: Vec<_> = ids.iter().map(|&i| &groups.bodies[i]).collect();
            // The shared row buffer moves through the frame, as below.
            let rows = std::mem::take(&mut frame_input.step.rows);
            let result = crate::deformation_world::calculate_multi_group(
                &frame_input,
                &bodies,
                &raw,
                shapes,
                &mut taken,
                shared_seed.expect("registered seed"),
                shared_toggle,
                Some(rows),
            );
            for (&i, registration) in ids.iter().zip(taken) {
                registrations[i] = registration;
            }
            let (frame, contacts, seed, toggle) = result?;
            shared_seed = Some(seed);
            shared_toggle = toggle;
            for (&i, c) in contact_ids.iter().zip(contacts) {
                groups.contacts[i] = c;
            }
            frame
        } else if let Some(registration) = registered {
            let body = ids[0];
            let raw: Vec<_> = contact_ids
                .iter()
                .map(|&i| {
                    let mut c = groups.contacts[i];
                    c[1] = 1;
                    c
                })
                .collect();
            registration.random_seed = shared_seed.unwrap_or(registration.random_seed);
            let rows = std::mem::take(&mut frame_input.step.rows);
            let (frame, contacts) = crate::deformation_world::calculate_group(
                &frame_input,
                &groups.bodies[body],
                &raw,
                shapes,
                registration,
                Some(rows),
            )?;
            shared_seed = Some(registration.random_seed);
            for (&i, mut c) in contact_ids.iter().zip(contacts) {
                c[1] = body as u32 + 1;
                groups.contacts[i] = c;
            }
            frame
        } else {
            impact_frame::calculate_owned(frame_input)?
        };
        drop(_kind_timer);
        drop(frame_timer);
        let _store_timer =
            crate::stage_timing::Timer::start(crate::stage_timing::Stage::IslandStore);
        // B77310: a nonzero status clears the island's contacts' written-back
        // impulse (B0F0A0, +0x3C); AD8130 then sleeps a status-1 island and
        // parks a status-2 one (below, after every frame). Cars never sleep
        // (+0x3B4 FLT_MAX), so a status 1 with a car is a capacity overflow.
        if frame.status != 0 {
            // A car cannot pass the sleep test (turnOffTime FLT_MAX), so its
            // status 1 is a capacity overflow; nor leave a FLT_MAX radius.
            if frame.status == 1 && ids.iter().any(|&i| f(&groups.bodies[i], 0x3b4) == f32::MAX) {
                return Err(Error(
                    "unsupported grouped physical sleep or capacity status",
                ));
            }
            for &i in &contact_ids {
                groups.contacts[i][0x3c / 4] = 0;
            }
            resting.push((frame.status, ids.clone()));
        }
        for (&k, j) in groups.joint_groups[root]
            .iter()
            .zip(&frame.step.joints.list)
        {
            groups.joints[k].joint = j.joint.clone();
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
    for (status, ids) in &resting {
        for &i in ids {
            if *status == 1 {
                sleep_body(&mut groups, i);
            } else {
                park_body(&mut groups, &mut manager.parked, i);
            }
        }
    }
    if let Some(seed) = shared_seed {
        for registration in registrations.iter_mut().flatten() {
            registration.random_seed = seed;
            registration.owner_toggle = shared_toggle;
        }
    }
    Ok(Output {
        groups,
        manager,
        pool_used,
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
            joints: vec![],
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
            park: None,
            parked: 0,
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

        // A real producer-style special contact reaches row construction in an
        // earlier group, then a later pose error must publish neither result.
        let mut shaped = input.clone();
        let mut c = [0; 32];
        for (o, v) in [(4, 1), (0x28, 1), (0x2c, 2)] {
            c[o / 4] = v;
        }
        set_flag(&mut c, 0xf, true);
        set_flag(&mut c, 0x72, true);
        set_flag(&mut c, 0x73, true);
        c[0x38 / 4] = 0.25f32.to_bits();
        c[0x3c / 4] = f32::MAX.to_bits();
        c[0x64 / 4] = 1f32.to_bits();
        shaped.contacts.push(c);
        let shapes = [
            contact_groups::Shape {
                flag229: true,
                ..Default::default()
            },
            contact_groups::Shape {
                angular: true,
                ..Default::default()
            },
        ];
        contact_groups::validate_with_shapes(&shaped, &shapes).unwrap();
        assert!(calculate_with_shapes(&shaped, &manager, &shapes).is_err());
        assert!(calculate_tick_with_shapes(&shaped, &manager, &shapes).is_err());
        assert_eq!(shaped.bodies, before.bodies);
        assert_eq!(shaped.contacts, vec![c]);
        assert_eq!(manager.rows, scratch.rows);
        assert_eq!(manager.workspace, scratch.workspace);
        assert_eq!(manager.properties, scratch.properties);
        first.contacts.push(c);
        let good = calculate_with_shapes(&first, &smaller, &shapes).unwrap();
        assert!(good.visits[0].3);
        let contact = good.groups.contacts[0];
        assert_eq!(contact[8 / 4], 0);
        assert_eq!(contact[0x20 / 4], u32::MAX);
        assert_eq!(contact[0x18 / 4], 1);
        assert!(!flag(&contact, 0xe));
        assert!(flag(&contact, 0x73));
    }

    /// Two free bodies with no contact between them form one island through
    /// a joint (AD6C40 groups the joints first), whose rows then hold them.
    #[test]
    fn a_joint_merges_two_bodies_into_one_solved_island() {
        use crate::joint::{
            self, Bodies, Joint,
            tests::{body_at, descriptor},
        };
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
        bodies[1][0x1c0 / 4] = 2.5f32.to_bits();
        bodies[1][0x220 / 4] = 1f32.to_bits();
        let (ra, rb) = (body_at([0., 0., 0.]), body_at([2., 0., 0.]));
        let mut d = descriptor([1., 0., 0.], -0.05, 0.05);
        let mut j = Joint::filled(0);
        joint::construct(
            &mut j,
            &mut d,
            &Bodies {
                a: &ra,
                b: Some(&rb),
                fixed: false,
            },
            0,
        );
        let input = contact_groups::Input {
            bodies,
            contacts: vec![],
            active_order: vec![0, 1],
            selector: 0,
            joints: vec![contact_groups::WorldJoint {
                joint: j,
                a: 1,
                b: 2,
                fixed: None,
            }],
        };
        let manager = Manager {
            parameters: [1. / 60., 1., 1., 1., 1., 0., 0.2, 0.003, 1., 0.2, 0.2],
            gravity: [0.; 4],
            correction: None,
            materials: vec![[0.; 13]],
            rows: vec![[0; 48]; 8],
            capacity: 8,
            iterations: 5,
            ends: [0; 4],
            properties: vec![(1., identity); 2],
            workspace: vec![[[0.; 4]; 2]; 2],
            cached_pair: None,
            cached_material: [0.; 13],
            park: None,
            parked: 0,
        };
        let out = calculate(&input, &manager).unwrap();
        assert_eq!(out.visits.len(), 1);
        assert_eq!((out.visits[0].1, out.visits[0].2), (2, 0));
        let j = &out.groups.joints[0].joint;
        assert_eq!(out.groups.joint_groups[out.visits[0].0], vec![0]);
        assert_ne!(j.words[0x1c / 4], j.words[0x20 / 4]);
        assert!(j.words[0x20 / 4] < 2);
        assert!(f(&out.groups.bodies[1], 0x220) < 1.);
        assert!(f(&out.groups.bodies[0], 0x220) > 0.);
        // Without the joint the bodies stay apart and keep their velocities.
        let mut free = input.clone();
        free.joints.clear();
        let apart = calculate(&free, &manager).unwrap();
        assert_eq!(apart.visits.len(), 2);
        assert_eq!(f(&apart.groups.bodies[1], 0x220), 1.);
    }
}
