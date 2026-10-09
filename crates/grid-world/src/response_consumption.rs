//! Consume supplied callback responses with the original deferred-pair ordering.
//! Contact creation, callback registration and task scheduling are external.
use crate::Error;

#[derive(Clone, Copy, Debug)]
pub struct Owner {
    pub identity: u32,
    /// Whether the linked owner's +41c callback slot is populated.
    pub registered: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Contact {
    pub identity: u32,
    pub first: Option<Owner>,
    pub second: Option<Owner>,
    pub second_is_body: bool,
    pub accumulated: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Pending {
    pub contact: u32,
    pub value: f32,
}

pub struct Input<'a> {
    pub owner: u32,
    pub contacts: &'a [Contact],
    /// Ordered indices into contacts. Repeated references remain meaningful.
    pub selected: &'a [usize],
    pub response: &'a [f32],
    pub pending: &'a [Pending],
}

pub struct Output {
    pub contacts: Vec<Contact>,
    pub response: Vec<f32>,
    /// Active pairs only; unused native array slots are not modeled.
    pub pending: Vec<Pending>,
}

#[allow(clippy::assign_op_pattern)] // Preserve pending/current and current/accumulator order.
pub fn consume(input: Input<'_>) -> Result<Output, Error> {
    if input.owner == 0
        || input.contacts.len() > 512
        || input.selected.len() > 80
        || input.selected.len() != input.response.len()
        || input.pending.len() > 240
        || input.contacts.iter().enumerate().any(|(i, c)| {
            c.identity == 0
                || !c.accumulated.is_finite()
                || input.contacts[..i].iter().any(|p| p.identity == c.identity)
                || [c.first, c.second]
                    .into_iter()
                    .flatten()
                    .any(|o| o.identity == 0)
        })
        || input.selected.iter().any(|&i| i >= input.contacts.len())
        || input.response.iter().any(|v| !v.is_finite())
        || input.pending.iter().any(|p| {
            !p.value.is_finite() || !input.contacts.iter().any(|c| c.identity == p.contact)
        })
    {
        return Err(Error("invalid response consumption input"));
    }
    // An identity denotes one owner. Reject conflicting callback registrations.
    let mut owners: Vec<Owner> = Vec::new();
    for owner in input
        .contacts
        .iter()
        .flat_map(|c| [c.first, c.second])
        .flatten()
    {
        if owners
            .iter()
            .any(|o| o.identity == owner.identity && o.registered != owner.registered)
        {
            return Err(Error("inconsistent response owner registration"));
        }
        owners.push(owner);
    }
    let mut output = Output {
        contacts: input.contacts.to_vec(),
        response: input.response.to_vec(),
        pending: input.pending.to_vec(),
    };
    for (position, &index) in input.selected.iter().enumerate() {
        let contact = &mut output.contacts[index];
        let other_registered = [
            contact.first,
            contact.second.filter(|_| contact.second_is_body),
        ]
        .into_iter()
        .flatten()
        .any(|o| o.identity != input.owner && o.registered);
        if !other_registered {
            // This branch neither looks up pending pairs nor clamps negatives.
            contact.accumulated = output.response[position] + contact.accumulated;
        } else if let Some(pair) = output
            .pending
            .iter()
            .position(|p| p.contact == contact.identity)
        {
            output.response[position] = output.pending[pair].value + output.response[position];
            let value = output.response[position] + contact.accumulated;
            contact.accumulated = if value < 0. { 0. } else { value };
            output.pending.swap_remove(pair);
        } else if output.pending.len() < 240 {
            output.pending.push(Pending {
                contact: contact.identity,
                value: output.response[position],
            });
        }
        if !contact.accumulated.is_finite() || !output.response[position].is_finite() {
            return Err(Error("response consumption exceeds finite range"));
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contact(identity: u32) -> Contact {
        Contact {
            identity,
            first: Some(Owner {
                identity: 1,
                registered: true,
            }),
            second: Some(Owner {
                identity: 2,
                registered: true,
            }),
            second_is_body: true,
            accumulated: 0.,
        }
    }

    #[test]
    fn two_owners_defer_then_combine_and_clamp() {
        let first = consume(Input {
            owner: 1,
            contacts: &[contact(10)],
            selected: &[0],
            response: &[-3.],
            pending: &[],
        })
        .unwrap();
        assert_eq!(first.contacts[0].accumulated, 0.);
        assert_eq!(first.pending.len(), 1);
        let second = consume(Input {
            owner: 2,
            contacts: &first.contacts,
            selected: &[0],
            response: &[1.],
            pending: &first.pending,
        })
        .unwrap();
        assert_eq!(second.response, [-2.]);
        assert_eq!(second.contacts[0].accumulated, 0.);
        assert!(second.pending.is_empty());
    }

    #[test]
    fn direct_path_is_unclamped_and_leaves_pending_untouched() {
        let mut c = contact(10);
        c.second_is_body = false;
        let result = consume(Input {
            owner: 1,
            contacts: &[c],
            selected: &[0, 0],
            response: &[-2., -3.],
            pending: &[Pending {
                contact: 10,
                value: 9.,
            }],
        })
        .unwrap();
        assert_eq!(result.contacts[0].accumulated, -5.);
        assert_eq!(result.pending[0].value, 9.);
    }

    #[test]
    fn consumption_swaps_last_pair_and_retains_negative_zero() {
        let mut contacts = [contact(10), contact(11), contact(12)];
        contacts[0].accumulated = -0.;
        let result = consume(Input {
            owner: 1,
            contacts: &contacts,
            selected: &[0],
            response: &[-0.],
            pending: &[
                Pending {
                    contact: 10,
                    value: -0.,
                },
                Pending {
                    contact: 11,
                    value: 1.,
                },
                Pending {
                    contact: 12,
                    value: 2.,
                },
            ],
        })
        .unwrap();
        assert_eq!(
            result.contacts[0].accumulated.to_bits(),
            (-0.0_f32).to_bits()
        );
        assert_eq!(
            result.pending.iter().map(|p| p.contact).collect::<Vec<_>>(),
            [12, 11]
        );
    }

    #[test]
    fn invalid_registrations_and_overflow_leave_input_unchanged() {
        let mut contacts = [contact(10), contact(11)];
        contacts[1].first.as_mut().unwrap().registered = false;
        assert!(
            consume(Input {
                owner: 1,
                contacts: &contacts,
                selected: &[],
                response: &[],
                pending: &[],
            })
            .is_err()
        );
        contacts[1].first.as_mut().unwrap().registered = true;
        contacts[0].second = None;
        contacts[0].accumulated = f32::MAX;
        assert!(
            consume(Input {
                owner: 1,
                contacts: &contacts,
                selected: &[0],
                response: &[f32::MAX],
                pending: &[],
            })
            .is_err()
        );
        assert_eq!(contacts[0].accumulated, f32::MAX);
        assert!(
            consume(Input {
                owner: 1,
                contacts: &contacts,
                selected: &[2],
                response: &[0.],
                pending: &[],
            })
            .is_err()
        );
    }
}
