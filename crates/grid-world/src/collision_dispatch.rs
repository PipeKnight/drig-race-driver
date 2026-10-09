//! Serial collision dispatch plan from supplied pools; no task execution.
use crate::{Error, contact_collection, owner_collection};

pub struct Selection {
    pub owner: u32,
    pub contacts: Vec<u32>,
}
pub struct Output {
    pub owners: Vec<u32>,
    pub next_reverse: u8,
    /// Empty contact selections do not invoke the serial callback wrapper.
    pub calls: Vec<Selection>,
}
pub fn plan(input: owner_collection::Input<'_>, reverse: u8) -> Result<Output, Error> {
    let first_pool = input.first_pool;
    let second_pool = input.second_pool;
    let mut owners = owner_collection::collect(input)?;
    if owners.len() > 32 {
        return Err(Error("owner list exceeds serial manager storage"));
    }
    let next_reverse = if owners.len() > 1 {
        u8::from(reverse == 0)
    } else {
        reverse
    };
    if owners.len() > 1 && next_reverse != 0 {
        owners.reverse();
    }
    let mut calls = Vec::new();
    for &owner in &owners {
        let contacts = contact_collection::collect(contact_collection::Input {
            owner,
            capacity: 80,
            initial: &[],
            first_pool,
            second_pool,
        })?;
        if !contacts.is_empty() {
            calls.push(Selection { owner, contacts });
        }
    }
    Ok(Output {
        owners,
        next_reverse,
        calls,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn alternating_order_preserves_eligible_owner_with_empty_selection() {
        let owners = [
            owner_collection::Owner {
                identity: 1,
                registered: true,
            },
            owner_collection::Owner {
                identity: 2,
                registered: true,
            },
        ];
        let contacts = [
            contact_collection::Contact {
                identity: 10,
                first: 1,
                second: 2,
                second_is_body: true,
                disabled: [false; 2],
            },
            contact_collection::Contact {
                identity: 11,
                first: 2,
                second: 1,
                second_is_body: true,
                disabled: [true, false],
            },
        ];
        let run = |reverse| {
            plan(
                owner_collection::Input {
                    owners: &owners,
                    first_pool: &contacts,
                    second_pool: &[],
                },
                reverse,
            )
            .unwrap()
        };
        let first = run(0);
        assert_eq!(first.owners, [2, 1]);
        assert_eq!(first.calls.len(), 1);
        assert_eq!(first.calls[0].owner, 1);
        assert_eq!(first.calls[0].contacts, [10]);
        let second = run(first.next_reverse);
        assert_eq!(second.owners, [1, 2]);
        assert_eq!(second.next_reverse, 0);
    }
    #[test]
    fn zero_or_one_owner_preserves_raw_toggle_byte() {
        for count in 0..=1 {
            let owners = [owner_collection::Owner {
                identity: 1,
                registered: true,
            }];
            let contacts = [contact_collection::Contact {
                identity: 10,
                first: 1,
                second: 0,
                second_is_body: false,
                disabled: [false; 2],
            }];
            let output = plan(
                owner_collection::Input {
                    owners: &owners,
                    first_pool: &contacts[..count],
                    second_pool: &[],
                },
                255,
            )
            .unwrap();
            assert_eq!(output.next_reverse, 255);
            assert_eq!(output.owners.len(), count);
        }
    }
}
