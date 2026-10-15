//! Owned raw car/physical/contact adapter for the registered bulk callback.
//! `calculate` covers static first-owner contacts; `calculate_shared` takes
//! per-contact sides, the group's consumer records and pending response pairs
//! (two-body car contacts). Physical phase scheduling is external.
use crate::{
    Error, collision_callback, contact_preparation, deformation_impact::Node,
    deformation_settle::Edge, impact_generation, impact_producer as kernel,
    response_consumption as consumer, serial_callback,
};

#[derive(Clone)]
pub struct Parameters {
    /// Corner scale, unlimited static mass, limit scale, flag radius scale,
    /// record radius scale, radial scale.
    pub generation: [f32; 6],
    /// Gain, random gain, history range, history threshold, speed threshold.
    pub body: [f32; 5],
    /// Integration step, damping, motion scale.
    pub grid: [f32; 3],
    pub grid_iterations: u32,
    pub clip_each_iteration: bool,
    pub response_scale: f32,
    pub correction_gain: f32,
    pub subtract_baseline: bool,
    pub policy_global_factor: f32,
    pub policy_scale: f32,
    pub policy_table: [f32; 16],
    pub global_disabled: bool,
}
pub struct Output {
    pub owner: Vec<u32>,
    pub physical: [u32; 0x424 / 4],
    pub contacts: Vec<[u32; 32]>,
    pub random_seed: u32,
    pub handled: bool,
    pub records: usize,
}
fn f(b: &[u32], at: usize) -> f32 {
    f32::from_bits(b[at / 4])
}
fn v(b: &[u32], at: usize) -> [f32; 4] {
    std::array::from_fn(|i| f(b, at + i * 4))
}
fn byte(b: &[u32], at: usize) -> u8 {
    (b[at / 4] >> (at % 4 * 8)) as u8
}
fn set_byte(b: &mut [u32], at: usize, value: u8) {
    let shift = at % 4 * 8;
    b[at / 4] = (b[at / 4] & !(255 << shift)) | (u32::from(value) << shift);
}
fn store(b: &mut [u32], at: usize, value: [f32; 4]) {
    b[at / 4..at / 4 + 4].copy_from_slice(&value.map(f32::to_bits));
}

pub fn calculate(
    owner: &[u32],
    physical: &[u32; 0x424 / 4],
    contacts: &[[u32; 32]],
    settings: &[u32],
    scalar: f32,
    parameters: &Parameters,
    random_seed: u32,
) -> Result<Output, Error> {
    if contacts.iter().any(|c| c[1] != 0 || c[2] != 0) {
        return Err(Error("invalid owned bulk deformation roles"));
    }
    let consumed: Vec<_> = contacts
        .iter()
        .enumerate()
        .map(|(i, c)| consumer::Contact {
            identity: 1 + i as u32,
            first: Some(consumer::Owner {
                identity: 1,
                registered: true,
            }),
            second: None,
            second_is_body: false,
            accumulated: f(c, 0x38),
        })
        .collect();
    let selected: Vec<_> = (0..contacts.len()).collect();
    let (out, consumed) = calculate_shared(Shared {
        owner,
        physical,
        contacts,
        roles: &vec![Role::default(); contacts.len()],
        settings,
        scalar,
        parameters,
        random_seed,
        identity: 1,
        consumers: &consumed,
        selected: &selected,
        pending: &[],
    })?;
    if let Some(consumed) = consumed
        && !consumed.pending.is_empty()
    {
        return Err(Error("unexpected static bulk pending response"));
    }
    Ok(out)
}

/// One selected contact's side for the owner.
#[derive(Clone, Copy, Default)]
pub struct Role {
    /// The owner is the contact's second body: point, direction and normal
    /// are reversed and the own/other coefficients swap (+44/+40).
    pub reverse: bool,
    /// The other body's +360 for two-body contacts.
    pub counterpart_velocity: Option<[f32; 4]>,
}

pub struct Shared<'a> {
    pub owner: &'a [u32],
    pub physical: &'a [u32; 0x424 / 4],
    /// Selected raw contacts, in callback order.
    pub contacts: &'a [[u32; 32]],
    pub roles: &'a [Role],
    pub settings: &'a [u32],
    pub scalar: f32,
    pub parameters: &'a Parameters,
    pub random_seed: u32,
    /// Owner identity in `consumers`.
    pub identity: u32,
    /// All consumer records of the group; `selected[i]` corresponds to `contacts[i]`.
    pub consumers: &'a [consumer::Contact],
    pub selected: &'a [usize],
    pub pending: &'a [consumer::Pending],
}

/// Bulk callback for one owner. The output contacts carry the consumed +38 of
/// the selected contacts; the consumption result (all group records and
/// pending pairs) is `None` when the callback declined.
pub fn calculate_shared(input: Shared<'_>) -> Result<(Output, Option<consumer::Output>), Error> {
    let Shared {
        owner,
        physical,
        contacts,
        roles,
        settings,
        scalar,
        parameters,
        random_seed,
        identity,
        consumers,
        selected,
        pending,
    } = input;
    if roles.len() != contacts.len() || selected.len() != contacts.len() {
        return Err(Error("invalid shared bulk deformation roles"));
    }
    if !(0xd230 / 4..=0x21000 / 4).contains(&owner.len())
        || settings.len() < 0x1050 / 4
        || settings.len() > 0x2000 / 4
        || contacts.len() > 64
        || !scalar.is_finite()
        || physical[0x334 / 4] != 0x1c0
        || [0x418, 0x41c, 0x420].iter().any(|at| physical[at / 4] != 0)
        || owner[0xc7f8 / 4] != 0
        || byte(owner, 0xc7af) > 1
        || byte(physical, 0x390) > 1
        || contacts.iter().any(|c| byte(c, 0xc) > 1)
    {
        return Err(Error("invalid owned bulk deformation roles"));
    }
    let d = &owner[0x2300 / 4..0xc800 / 4];
    let edge_count = d[0xa410 / 4] as usize;
    if edge_count > 1200 {
        return Err(Error("invalid bulk deformation edge count"));
    }
    let edges: Vec<_> = (0..edge_count)
        .map(|i| Edge {
            endpoints: [(d[i * 2] & 0xffff) as usize, (d[i * 2] >> 16) as usize],
            length: f(d, i * 8 + 4),
        })
        .collect();
    let nodes: Vec<_> = (0..400)
        .map(|i| {
            let at = 0x2580 + i * 80;
            Node {
                position: v(d, at),
                rest: v(d, at + 16),
                bounds: v(d, at + 32),
                motion: v(d, at + 48),
                marker: f(d, at + 64),
            }
        })
        .collect();
    let flags: Vec<_> = (0..400).map(|i| byte(d, 0xa280 + i)).collect();
    let generated: Vec<_> = contacts
        .iter()
        .zip(roles)
        .map(|(c, role)| impact_generation::Contact {
            enabled: byte(c, 0x73) != 0,
            reverse: role.reverse,
            point: v(c, 0x50),
            direction: v(c, 0x60),
            value_38: f(c, 0x38),
            value_3c: f(c, 0x3c),
            value_40: f(c, 0x40),
            value_44: f(c, 0x44),
            counterpart_velocity: role.counterpart_velocity,
        })
        .collect();
    let p = parameters;
    let result = serial_callback::calculate(serial_callback::Input {
        owner: identity,
        contacts: consumers,
        selected,
        pending,
        callback: collision_callback::Input {
            intensity: f(d, 0xa494),
            descriptor_disabled: byte(d, 0xa4af) != 0,
            global_disabled: p.global_disabled,
            policy: contact_preparation::Policy {
                body_factor_a: f(owner, 0xd16c),
                body_factor_b: f(owner, 0xd22c),
                global_factor: p.policy_global_factor,
                linked_mass: f(physical, 0x3f0),
                scale: p.policy_scale,
                table: p.policy_table,
            },
            producer: kernel::Input {
                grid: kernel::Grid {
                    dimensions: std::array::from_fn(|i| d[0xa414 / 4 + i] as usize),
                    spacing: f(d, 0xa420),
                    minimum: v(d, 0xa430),
                    state: f(d, 0xa480),
                    corners: v(d, 0xa4b4),
                    window: std::array::from_fn(|i| f(d, 0xa4c4 + i * 4)),
                    flags: &flags,
                    dirty: std::array::from_fn(|i| byte(d, 0xa4f0 + i)),
                    nodes: &nodes,
                    edges: &edges,
                    mode: d[0xa4a0 / 4],
                    strain_threshold: f(settings, 0x1024),
                    strain_gain: f(settings, 0x1028),
                    displacement_threshold: f(settings, 0x1048),
                    integration_step: p.grid[0],
                    damping: p.grid[1],
                    motion_scale: p.grid[2],
                    iterations: p.grid_iterations,
                    counter: d[0xa450 / 4],
                    clip_each_iteration: p.clip_each_iteration,
                    offset: v(d, 0xa440),
                },
                generation: kernel::Generation {
                    pose: std::array::from_fn(|i| v(physical, 0x1d0 + i * 16)),
                    body_offset: v(owner, 0x1630),
                    body_b20: v(owner, 0xb20),
                    velocity: v(physical, 0x360),
                    supplied_scalar: scalar,
                    corner_scale: p.generation[0],
                    static_mass: p.generation[1],
                    limit_scale: p.generation[2],
                    flag_radius_scale: p.generation[3],
                    record_radius_scale: p.generation[4],
                    radial_scale: p.generation[5],
                    damage_threshold: f(settings, 0x1030),
                    damage_gain: f(settings, 0x1034),
                    projection: f(settings, 0x1038),
                    contacts: &generated,
                },
                body: kernel::Body {
                    angular: v(physical, 0x210),
                    linear: v(physical, 0x220),
                    point: [0.; 4],
                    direction: [0.; 4],
                    center: v(physical, 0x1c0),
                    history: std::array::from_fn(|i| f(owner, 0x290 + i * 4)),
                    history_extra: f(owner, 0x2e0),
                    mass: f(owner, 0x15a8),
                    gain: p.body[0],
                    random_gain: p.body[1],
                    history_range: p.body[2],
                    history_threshold: p.body[3],
                    speed_threshold: p.body[4],
                    random_seed,
                },
                response: kernel::Response {
                    values: None,
                    accumulator: v(physical, 0x380),
                    dirty: byte(physical, 0x390) != 0,
                    scale: p.response_scale,
                    correction_gain: p.correction_gain,
                    subtract_baseline: p.subtract_baseline,
                },
            },
        },
    })?;
    let mut out = Output {
        owner: owner.to_vec(),
        physical: *physical,
        contacts: contacts.to_vec(),
        random_seed,
        handled: false,
        records: 0,
    };
    if let Some(active) = result.callback.activation {
        out.owner[0xc7a0 / 4] = 1;
        out.owner[0xc754 / 4] = active.strength.to_bits();
        store(&mut out.owner, 0xc760, active.point);
        store(&mut out.owner, 0xc770, active.direction);
    }
    if let Some(r) = result.callback.producer {
        out.handled = true;
        out.records = r.generation.records.len();
        out.random_seed = r.body.random_seed;
        for (i, value) in r.generation.flags.into_iter().enumerate() {
            set_byte(&mut out.owner, 0xc580 + i, value);
        }
        for (i, value) in r.generation.dirty.into_iter().enumerate() {
            set_byte(&mut out.owner, 0xc7f0 + i, value);
        }
        out.owner[0xc780 / 4] = r.generation.state.to_bits();
        for (i, n) in r.grid.nodes.iter().enumerate() {
            store(&mut out.owner, 0x4880 + i * 80, n.position);
            store(&mut out.owner, 0x48b0 + i * 80, n.motion);
        }
        for (i, length) in r.grid.lengths.into_iter().enumerate() {
            out.owner[(0x2300 + i * 8 + 4) / 4] = length.to_bits();
        }
        store(&mut out.owner, 0xc740, r.grid.offset);
        out.owner[0xc750 / 4] = r.grid.counter;
        set_byte(&mut out.owner, 0xc7f0, u8::from(r.grid.dirty));
        store(&mut out.physical, 0x210, r.body.angular);
        store(&mut out.physical, 0x220, r.body.linear);
        store(&mut out.physical, 0x380, r.response.accumulator);
        set_byte(&mut out.physical, 0x390, u8::from(r.response.dirty));
        for (i, value) in r.body.history.into_iter().enumerate() {
            out.owner[(0x290 + i * 4) / 4] = value.to_bits();
        }
        out.owner[0x2e0 / 4] = r.body.history_extra.to_bits();
        let consumed = result
            .consumption
            .as_ref()
            .ok_or(Error("bulk callback omitted consumption"))?;
        for (c, &index) in out.contacts.iter_mut().zip(selected) {
            c[0x38 / 4] = consumed.contacts[index].accumulated.to_bits();
        }
    }
    Ok((out, result.consumption))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn disabled_bulk_retains_complete_opaque_owner_and_owned_seed() {
        let mut owner = vec![0x7fc01234; 0xd230 / 4];
        owner[0xc710 / 4] = 0;
        owner[0xc7f8 / 4] = 0;
        set_byte(&mut owner, 0xc7af, 1);
        let mut physical = [0; 0x424 / 4];
        physical[0x334 / 4] = 0x1c0;
        let mut contact = [0; 32];
        contact[3] = 1;
        contact[0x38 / 4] = 1f32.to_bits();
        let parameters = Parameters {
            generation: [0.; 6],
            body: [0.; 5],
            grid: [0.; 3],
            grid_iterations: 0,
            clip_each_iteration: false,
            response_scale: 0.,
            correction_gain: 0.,
            subtract_baseline: false,
            policy_global_factor: 0.,
            policy_scale: 0.,
            policy_table: [0.; 16],
            global_disabled: false,
        };
        let result = calculate(
            &owner,
            &physical,
            &[contact],
            &vec![0; 0x1050 / 4],
            0.016,
            &parameters,
            0x12345678,
        )
        .unwrap();
        assert_eq!(result.owner, owner);
        assert_eq!(result.physical, physical);
        assert_eq!(result.contacts, vec![contact]);
        assert_eq!(result.random_seed, 0x12345678);
        assert!(!result.handled);
        owner[0xc7f8 / 4] = 1;
        assert!(
            calculate(
                &owner,
                &physical,
                &[contact],
                &vec![0; 0x1050 / 4],
                0.016,
                &parameters,
                0x12345678
            )
            .is_err()
        );
        assert_eq!(owner[0xc7f8 / 4], 1);
    }
}
