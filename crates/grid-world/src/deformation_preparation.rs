//! B77850 first-pool preparation for one registered owner and static peers.
//! The second pool is empty; owner ordering and dynamic peers are external.
use crate::{Error, deformation_contact_limit as limit};

pub struct Output {
    pub descriptor: Vec<u32>,
    pub contacts: Vec<[u32; 32]>,
    pub cursor: usize,
    pub complete: bool,
}

/// Contact first-owner tokens are 0 for this registered owner, 1 for unrelated.
/// Initial cursor, skip and budget apply only to this invocation; skipped and
/// unrelated records retain every byte. No input changes on a late failure.
#[allow(clippy::too_many_arguments)]
pub fn calculate(
    descriptor: &[u32],
    body: &[u32; limit::PHYSICAL_WORDS],
    center: [f32; 4],
    contacts: &[[u32; 32]],
    cursor: usize,
    budget: usize,
    skip: usize,
    parameters: limit::Parameters,
) -> Result<Output, Error> {
    let mut out = Output {
        descriptor: descriptor.to_vec(),
        contacts: contacts.to_vec(),
        cursor,
        complete: false,
    };
    // In place: an error discards `out`, so no input changes.
    (out.cursor, out.complete) = calculate_in(
        &mut out.descriptor,
        &mut None,
        body,
        center,
        &mut out.contacts,
        cursor,
        budget,
        skip,
        parameters,
    )?;
    Ok(out)
}

/// [`calculate`]'s checks alone, without copying the descriptor or contacts:
/// `Ok` exactly when [`calculate`] from cursor 0 with no budget would succeed.
/// `grid` caches the descriptor's grid verdict for [`calculate_in`] on the
/// same descriptor.
pub(crate) fn validate(
    descriptor: &[u32],
    grid: &mut Option<Result<usize, Error>>,
    body: &[u32; limit::PHYSICAL_WORDS],
    center: [f32; 4],
    contacts: &[[u32; 32]],
    parameters: limit::Parameters,
) -> Result<(), Error> {
    check(descriptor, grid, body, center, contacts, 0, 0, 0, parameters)
}

/// [`calculate`] writing the descriptor and contacts in place; returns the
/// cursor and whether it completed. On an error the checks failed before any
/// write, or a later contact failed and both buffers may be partly written.
/// `grid` is empty or [`validate`]'s cache for this descriptor.
#[allow(clippy::too_many_arguments)]
pub(crate) fn calculate_in(
    descriptor: &mut [u32],
    grid: &mut Option<Result<usize, Error>>,
    body: &[u32; limit::PHYSICAL_WORDS],
    center: [f32; 4],
    contacts: &mut [[u32; 32]],
    mut cursor: usize,
    mut budget: usize,
    mut skip: usize,
    parameters: limit::Parameters,
) -> Result<(usize, bool), Error> {
    check(
        descriptor, grid, body, center, contacts, cursor, budget, skip, parameters,
    )?;
    while cursor < contacts.len() {
        let c = &mut contacts[cursor];
        if c[1] == 0 {
            if skip != 0 {
                skip -= 1;
            } else {
                if budget == 0 {
                    break;
                }
                let (contact, cap) = limit::calculate_side_in(
                    descriptor,
                    grid,
                    body,
                    center,
                    c,
                    parameters,
                    limit::Side::default(),
                )?;
                *c = contact;
                if cap != f64::from(parameters.unlimited) {
                    // Native saves ST(0) to f32 before COMISS; the comparison
                    // against the unlimited sentinel itself retains ST(0).
                    let rounded = cap as f32;
                    if f32::from_bits(c[0x3c / 4]) > rounded {
                        c[0x3c / 4] = rounded.to_bits();
                    }
                }
                c[0x44 / 4] = parameters.unlimited.to_bits();
                budget -= 1;
            }
        }
        cursor += 1;
    }
    Ok((cursor, cursor == contacts.len()))
}

/// The bounds, descriptor and contact checks, caching the descriptor's grid
/// verdict for the limit in `grid`.
#[allow(clippy::too_many_arguments)]
fn check(
    descriptor: &[u32],
    grid: &mut Option<Result<usize, Error>>,
    body: &[u32; limit::PHYSICAL_WORDS],
    center: [f32; 4],
    contacts: &[[u32; 32]],
    cursor: usize,
    budget: usize,
    skip: usize,
    parameters: limit::Parameters,
) -> Result<(), Error> {
    if contacts.len() > crate::MAX_WORLD_CONTACTS
        || cursor > contacts.len()
        || budget > 10000
        || skip > 10000
    {
        return Err(Error("invalid deformation preparation bounds"));
    }
    // Validate the descriptor/body even for an empty collection. These owned
    // fields are required by this API's bounded domain, not native dereferences.
    let mut placeholder = [0; 32];
    placeholder[3] = 1;
    // One grid verdict for the descriptor: the limit never writes its grid.
    limit::validate_cached(
        descriptor,
        grid,
        body,
        center,
        &placeholder,
        parameters,
    )?;
    for c in contacts {
        if c[1] > 1 {
            return Err(Error("invalid preparation owner token"));
        }
        if c[1] == 0 {
            limit::validate_cached(descriptor, grid, body, center, c, parameters)?;
            if !f32::from_bits(c[0x3c / 4]).is_finite() {
                return Err(Error("invalid preparation existing cap"));
            }
        }
    }
    Ok(())
}
