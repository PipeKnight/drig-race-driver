//! Ordered unique registered owners from existing contact pools.
//! The original second pool requires two valid body owners per contact.
use crate::{Error, contact_collection::Contact};

#[derive(Clone, Copy)]
pub struct Owner {
    pub identity: u32,
    /// Whether owner +418 is populated; distinct from response callback +41c.
    pub registered: bool,
}
pub struct Input<'a> {
    pub owners: &'a [Owner],
    pub first_pool: &'a [Contact],
    pub second_pool: &'a [Contact],
}
pub fn collect(input: Input<'_>) -> Result<Vec<u32>, Error> {
    if input.owners.len() > 64
        || input.first_pool.len() > 512
        || input.second_pool.len() > 512
        || input.owners.iter().enumerate().any(|(i, o)| {
            o.identity == 0 || input.owners[..i].iter().any(|p| p.identity == o.identity)
        })
        || input
            .first_pool
            .iter()
            .chain(input.second_pool)
            .any(|c| c.identity == 0 || !input.owners.iter().any(|o| o.identity == c.first))
        || input
            .second_pool
            .iter()
            .any(|c| !c.second_is_body || !input.owners.iter().any(|o| o.identity == c.second))
    {
        return Err(Error("invalid owner collection input"));
    }
    let mut output = Vec::new();
    for identity in input
        .first_pool
        .iter()
        .map(|c| c.first)
        .chain(input.second_pool.iter().flat_map(|c| [c.first, c.second]))
    {
        if input
            .owners
            .iter()
            .any(|o| o.identity == identity && o.registered)
            && !output.contains(&identity)
        {
            output.push(identity);
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn c(first: u32, second: u32) -> Contact {
        Contact {
            identity: 10,
            first,
            second,
            second_is_body: true,
            disabled: [true, true],
        }
    }
    #[test]
    fn first_pool_then_both_second_owners_deduplicates_without_shape_filter() {
        let result = collect(Input {
            owners: &[
                Owner {
                    identity: 1,
                    registered: true,
                },
                Owner {
                    identity: 2,
                    registered: true,
                },
                Owner {
                    identity: 3,
                    registered: false,
                },
            ],
            first_pool: &[c(2, 1), c(3, 1), c(2, 1)],
            second_pool: &[c(1, 2), c(3, 1)],
        })
        .unwrap();
        assert_eq!(result, [2, 1]);
    }
    #[test]
    fn unsafe_native_second_pool_inputs_are_rejected() {
        let owners = [Owner {
            identity: 1,
            registered: true,
        }];
        for contact in [
            c(1, 0),
            Contact {
                second_is_body: false,
                ..c(1, 1)
            },
            c(0, 1),
        ] {
            assert!(
                collect(Input {
                    owners: &owners,
                    first_pool: &[],
                    second_pool: &[contact]
                })
                .is_err()
            );
        }
        assert!(
            collect(Input {
                owners: &owners,
                first_pool: &[c(0, 0)],
                second_pool: &[]
            })
            .is_err()
        );
    }
}
