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

pub(crate) fn validate(input: &Input<'_>) -> Result<(), Error> {
    // Identities sorted once: duplicate and pending lookups in O(n log n), the
    // same verdicts as the pairwise scans (a pile in a car's group has
    // hundreds of contacts, validated twice per owner and tick).
    let mut identities: Vec<u32> = input.contacts.iter().map(|c| c.identity).collect();
    identities.sort_unstable();
    if input.owner == 0
        || input.contacts.len() > crate::MAX_WORLD_CONTACTS
        || input.selected.len() > 80
        || input.selected.len() != input.response.len()
        || input.pending.len() > 240
        || input.contacts.iter().any(|c| {
            c.identity == 0
                || !c.accumulated.is_finite()
                || [c.first, c.second]
                    .into_iter()
                    .flatten()
                    .any(|o| o.identity == 0)
        })
        || identities.windows(2).any(|w| w[0] == w[1])
        || input.selected.iter().any(|&i| i >= input.contacts.len())
        || input.response.iter().any(|v| !v.is_finite())
        || input
            .pending
            .iter()
            .any(|p| !p.value.is_finite() || identities.binary_search(&p.contact).is_err())
    {
        return Err(Error("invalid response consumption input"));
    }
    // An identity denotes one owner. Reject conflicting callback registrations.
    let mut owners: Vec<(u32, bool)> = input
        .contacts
        .iter()
        .flat_map(|c| [c.first, c.second])
        .flatten()
        .map(|o| (o.identity, o.registered))
        .collect();
    owners.sort_unstable();
    if owners
        .windows(2)
        .any(|w| w[0].0 == w[1].0 && w[0].1 != w[1].1)
    {
        return Err(Error("inconsistent response owner registration"));
    }
    Ok(())
}

#[allow(clippy::assign_op_pattern)] // Preserve pending/current and current/accumulator order.
pub fn consume(input: Input<'_>) -> Result<Output, Error> {
    validate(&input)?;
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

    /// The pairwise checks `validate` replaced (F-entry of the tick-growth work).
    fn validate_pairwise(input: &Input<'_>) -> Result<(), Error> {
        if input.owner == 0
            || input.contacts.len() > crate::MAX_WORLD_CONTACTS
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
        Ok(())
    }

    #[test]
    fn sorted_validation_matches_the_pairwise_checks() {
        // Small identity ranges, so duplicates, missing pending contacts,
        // zero identities and conflicting registrations all occur.
        let mut seed = 0x2545_f491_u32;
        let mut next = |n: u32| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed % n
        };
        let (mut accepted, mut rejected) = (0, 0);
        for _ in 0..20_000 {
            let owner = |next: &mut dyn FnMut(u32) -> u32| {
                (next(4) != 0).then(|| Owner {
                    identity: next(5),
                    registered: next(5) != 0,
                })
            };
            let contacts: Vec<Contact> = (0..next(12))
                .map(|_| Contact {
                    identity: next(16),
                    first: owner(&mut next),
                    second: owner(&mut next),
                    second_is_body: next(2) == 0,
                    accumulated: if next(40) == 0 { f32::NAN } else { 0.5 },
                })
                .collect();
            let selected: Vec<usize> = (0..next(4)).map(|_| next(14) as usize).collect();
            let response = vec![1.; selected.len()];
            let pending: Vec<Pending> = (0..next(3))
                .map(|_| Pending {
                    contact: next(16),
                    value: 1.,
                })
                .collect();
            let input = Input {
                owner: next(3),
                contacts: &contacts,
                selected: &selected,
                response: &response,
                pending: &pending,
            };
            let (a, b) = (validate(&input), validate_pairwise(&input));
            assert_eq!(a.as_ref().err().map(|e| e.0), b.as_ref().err().map(|e| e.0));
            if a.is_ok() {
                accepted += 1;
            } else {
                rejected += 1;
            }
        }
        assert!(accepted > 100 && rejected > 100, "{accepted} {rejected}");
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
