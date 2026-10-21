//! Both registered deformation phases for a contact group of several bodies
//! (car-to-car contacts), per docs/CAR_TO_CAR_DEFORMATION_RESEARCH.md:
//! owners in B77770 order (first pool by +04, then two-body contacts by +04
//! and +08, registered bodies only, first occurrence); preparation per owner
//! and side (static contacts then two-body contacts, shared +3C minimum,
//! side-B coefficient +44, deformable peers set +0E); bulk per owner with the
//! owner order reversed on alternate multi-owner invocations (F5E1A4), side-B
//! contacts reversed, one pending response list (B77490) and one CRT stream
//! consumed in bulk owner order (serial-branch choice for a single thread).
//! Each owner writes only its own body; peers are read only.
use crate::{
    Error, contact_groups::Shape, deformation_callback_state as bulk,
    deformation_contact_limit as limit, impact_frame, impact_step,
    response_consumption as consumer,
};

const PHYSICAL: usize = 0x424 / 4;

#[derive(Clone)]
pub(crate) struct Owner {
    /// Local body index in the group.
    pub local: usize,
    pub owner: Vec<u32>,
    pub physical: [u32; PHYSICAL],
    pub settings: Vec<u32>,
    pub limit: limit::Parameters,
    pub bulk: bulk::Parameters,
    pub bulk_invoked: bool,
    pub records: usize,
}

#[derive(Clone)]
struct Context {
    owners: Vec<Owner>,
    /// Raw group bodies (local order), read as peers.
    bodies: Vec<[u32; PHYSICAL]>,
    contacts: Vec<[u32; 32]>,
    shapes: Vec<Shape>,
    scalar: f32,
    random_seed: u32,
    toggle: bool,
}

fn byte(b: &[u32], at: usize) -> u8 {
    (b[at / 4] >> (at % 4 * 8)) as u8
}
fn vec4(b: &[u32], at: usize) -> [f32; 4] {
    std::array::from_fn(|i| f32::from_bits(b[at / 4 + i]))
}
fn two_body(c: &[u32; 32]) -> bool {
    byte(c, 0xc) == 0 && c[2] != 0
}
/// Local bodies (first, second) of a contact.
fn locals(c: &[u32; 32]) -> (usize, Option<usize>) {
    (
        c[0x1c / 4] as usize,
        two_body(c).then_some(c[0x20 / 4] as usize),
    )
}

impl Context {
    fn owner_of(&self, local: usize) -> Option<usize> {
        self.owners.iter().position(|o| o.local == local)
    }
    /// B77770: static pool by first body, then two-body pool by first and
    /// second body; registered owners only, first occurrence wins.
    fn order(&self) -> Vec<usize> {
        let mut order = Vec::new();
        let mut push = |local: usize| {
            if let Some(k) = self.owner_of(local)
                && !order.contains(&k)
            {
                order.push(k);
            }
        };
        for c in self.contacts.iter().filter(|c| !two_body(c)) {
            push(locals(c).0);
        }
        for c in self.contacts.iter().filter(|c| two_body(c)) {
            let (a, b) = locals(c);
            push(a);
            push(b.expect("two-body contact"));
        }
        order
    }
    /// The owner's contacts: static pool (first body) then two-body pool
    /// (either body), each with its side.
    fn selection(&self, local: usize) -> Vec<(usize, bool)> {
        let mut out = Vec::new();
        for (i, c) in self.contacts.iter().enumerate() {
            if !two_body(c) && locals(c).0 == local {
                out.push((i, false));
            }
        }
        for (i, c) in self.contacts.iter().enumerate() {
            let (a, b) = locals(c);
            if two_body(c) && a == local {
                out.push((i, false));
            } else if two_body(c) && b == Some(local) {
                out.push((i, true));
            }
        }
        out
    }
}

impl impact_frame::Callbacks for Context {
    fn prepare(
        &mut self,
        step: &mut impact_step::Input,
        _: &mut [impact_frame::Body],
    ) -> Result<(), Error> {
        for k in self.order() {
            let local = self.owners[k].local;
            let mut body = self.owners[k].physical;
            body[0x334 / 4] = 0;
            let center = vec4(&body, 0x1c0);
            let parameters = self.owners[k].limit;
            let mut descriptor = self.owners[k].owner[0x2300 / 4..0xc800 / 4].to_vec();
            for (i, second) in self.selection(local) {
                let raw = self.contacts[i];
                let mut tokenless = raw;
                tokenless[1] = 0;
                tokenless[2] = 0;
                let peer_local = match locals(&raw) {
                    (a, Some(b)) => Some(if second { a } else { b }),
                    _ => None,
                };
                let peer = peer_local.map(|p| limit::Peer {
                    body: &self.bodies[p],
                    center: vec4(&self.bodies[p], 0x1c0),
                    deformable: self.owner_of(p).is_some(),
                });
                let result = limit::calculate_side(
                    &descriptor,
                    &body,
                    center,
                    &tokenless,
                    parameters,
                    limit::Side { second, peer },
                )?;
                descriptor = result.descriptor;
                let mut c = result.contact;
                c[1] = raw[1];
                c[2] = raw[2];
                if result.cap != f64::from(parameters.unlimited) {
                    let rounded = result.cap as f32;
                    if f32::from_bits(c[0x3c / 4]) > rounded {
                        c[0x3c / 4] = rounded.to_bits();
                    }
                }
                if peer.is_none() {
                    // Only the first pool resets the second-side slot.
                    c[0x44 / 4] = parameters.unlimited.to_bits();
                }
                self.contacts[i] = c;
            }
            self.owners[k].owner[0x2300 / 4..0xc800 / 4].copy_from_slice(&descriptor);
        }
        for (c, raw) in step.contacts.iter_mut().zip(&self.contacts) {
            c.previous_impulse = f32::from_bits(raw[0x3c / 4]);
            c.disabled = byte(raw, 0xe) != 0;
        }
        Ok(())
    }

    fn solved(
        &mut self,
        step: &mut impact_step::Output,
        bodies: &mut [impact_frame::Body],
    ) -> Result<(), Error> {
        for (c, raw) in step.contacts.iter().zip(&mut self.contacts) {
            raw[0x3c / 4] = c.previous_impulse.to_bits();
            raw[0x38 / 4] = c.accumulated.to_bits();
        }
        let mut order = self.order();
        if order.len() >= 2 {
            self.toggle = !self.toggle;
            if self.toggle {
                order.reverse();
            }
        }
        let registered = |local: usize| self.owners.iter().any(|o| o.local == local);
        let mut consumers: Vec<consumer::Contact> = self
            .contacts
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let (a, b) = locals(c);
                consumer::Contact {
                    identity: 1 + i as u32,
                    first: Some(consumer::Owner {
                        identity: 1 + a as u32,
                        registered: registered(a),
                    }),
                    second: b.map(|b| consumer::Owner {
                        identity: 1 + b as u32,
                        registered: registered(b),
                    }),
                    second_is_body: b.is_some(),
                    accumulated: f32::from_bits(c[0x38 / 4]),
                }
            })
            .collect();
        let mut pending: Vec<consumer::Pending> = Vec::new();
        for k in order {
            let local = self.owners[k].local;
            let selection: Vec<(usize, bool)> = self
                .selection(local)
                .into_iter()
                .filter(|&(i, _)| {
                    let c = &self.contacts[i];
                    ![c[0x28 / 4], c[0x2c / 4]]
                        .into_iter()
                        .any(|id| self.shapes[id as usize - 1].flag230)
                })
                .take(80)
                .collect();
            if selection.is_empty() {
                continue;
            }
            let o = &mut self.owners[k];
            o.physical[0x210 / 4..0x220 / 4]
                .copy_from_slice(&step.bodies[local].contact.angular.map(f32::to_bits));
            o.physical[0x220 / 4..0x230 / 4]
                .copy_from_slice(&step.bodies[local].contact.linear.map(f32::to_bits));
            o.physical[0x380 / 4..0x390 / 4]
                .copy_from_slice(&bodies[local].pending_shift.map(f32::to_bits));
            o.physical[0x390 / 4] =
                (o.physical[0x390 / 4] & !255) | u32::from(bodies[local].shift_active);
            o.physical[0x334 / 4] = 0x1c0;
            let contacts: Vec<_> = selection.iter().map(|&(i, _)| self.contacts[i]).collect();
            let roles: Vec<_> = selection
                .iter()
                .map(|&(i, second)| {
                    let (a, b) = locals(&self.contacts[i]);
                    bulk::Role {
                        reverse: second,
                        counterpart_velocity: b
                            .map(|b| vec4(&self.bodies[if second { a } else { b }], 0x360)),
                    }
                })
                .collect();
            let selected: Vec<_> = selection.iter().map(|&(i, _)| i).collect();
            let (result, consumed) = bulk::calculate_shared(bulk::Shared {
                owner: &o.owner,
                physical: &o.physical,
                contacts: &contacts,
                roles: &roles,
                settings: &o.settings,
                scalar: self.scalar,
                parameters: &o.bulk,
                random_seed: self.random_seed,
                identity: 1 + local as u32,
                consumers: &consumers,
                selected: &selected,
                pending: &pending,
            })?;
            o.owner = result.owner;
            o.physical = result.physical;
            self.random_seed = result.random_seed;
            if result.handled {
                o.bulk_invoked = true;
                o.records += result.records;
            }
            if let Some(consumed) = consumed {
                consumers = consumed.contacts;
                pending = consumed.pending;
                for (raw, c) in self.contacts.iter_mut().zip(&consumers) {
                    raw[0x38 / 4] = c.accumulated.to_bits();
                }
            }
            step.bodies[local].contact.angular =
                std::array::from_fn(|i| f32::from_bits(o.physical[0x210 / 4 + i]));
            step.bodies[local].contact.linear =
                std::array::from_fn(|i| f32::from_bits(o.physical[0x220 / 4 + i]));
            bodies[local].pending_shift =
                std::array::from_fn(|i| f32::from_bits(o.physical[0x380 / 4 + i]));
            bodies[local].shift_active = o.physical[0x390 / 4] & 255 != 0;
        }
        for (c, raw) in step.contacts.iter_mut().zip(&self.contacts) {
            c.accumulated = f32::from_bits(raw[0x38 / 4]);
        }
        Ok(())
    }
}

pub(crate) struct Output {
    pub frame: impact_frame::Output,
    pub owners: Vec<Owner>,
    pub contacts: Vec<[u32; 32]>,
    pub random_seed: u32,
    pub toggle: bool,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn calculate(
    frame: &impact_frame::Input,
    owners: Vec<Owner>,
    bodies: Vec<[u32; PHYSICAL]>,
    contacts: &[[u32; 32]],
    shapes: &[Shape],
    random_seed: u32,
    toggle: bool,
    rows: Option<Vec<crate::contact_rows::Row>>,
) -> Result<Output, Error> {
    if frame.bodies.len() != bodies.len()
        || frame.step.bodies.len() != bodies.len()
        || contacts.len() != frame.step.contacts.len()
        || owners.is_empty()
        || owners.iter().any(|o| {
            o.local >= bodies.len()
                || o.physical[0x334 / 4] != 0x1c0
                || o.owner[0xc7f8 / 4] != 0
                || o.physical[0x3f0 / 4] != frame.bodies[o.local].mass.to_bits()
        })
        || contacts.iter().zip(&frame.step.contacts).any(|(r, c)| {
            let (a, b) = locals(r);
            c.two_bodies != b.is_some()
                || c.bodies[0] != a
                || c.bodies[1] != b.unwrap_or(0)
                || a >= bodies.len()
                || b.is_some_and(|b| b >= bodies.len() || b == a)
                || c.accumulated.to_bits() != r[0x38 / 4]
                || c.previous_impulse.to_bits() != r[0x3c / 4]
                || [r[0x28 / 4], r[0x2c / 4]]
                    .into_iter()
                    .any(|id| id == 0 || id as usize > shapes.len())
        })
        || owners.iter().any(|o| {
            o.limit.corner_scale.to_bits() != o.bulk.generation[0].to_bits()
                || o.limit.unlimited.to_bits() != o.bulk.generation[1].to_bits()
                || o.limit.damage_scale.to_bits() != o.bulk.policy_scale.to_bits()
                || o.limit.global_disabled != o.bulk.global_disabled
        })
    {
        return Err(Error(
            "invalid deforming group correspondence or aliased globals",
        ));
    }
    let context = Context {
        owners,
        bodies,
        contacts: contacts.to_vec(),
        shapes: shapes.to_vec(),
        scalar: frame.step.parameters[0],
        random_seed,
        toggle,
    };
    let (frame, c) = impact_frame::calculate_with_callbacks_rows(frame, rows, &context)?;
    Ok(Output {
        frame,
        owners: c.owners,
        contacts: c.contacts,
        random_seed: c.random_seed,
        toggle: c.toggle,
    })
}
