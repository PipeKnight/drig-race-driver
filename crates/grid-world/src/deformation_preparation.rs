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
    mut budget: usize,
    mut skip: usize,
    parameters: limit::Parameters,
) -> Result<Output, Error> {
    if contacts.len() > 64 || cursor > contacts.len() || budget > 10000 || skip > 10000 {
        return Err(Error("invalid deformation preparation bounds"));
    }
    // Validate the descriptor/body even for an empty collection. These owned
    // fields are required by this API's bounded domain, not native dereferences.
    let mut placeholder = [0; 32];
    placeholder[3] = 1;
    limit::validate(descriptor, body, center, &placeholder, parameters)?;
    for c in contacts {
        if c[1] > 1 {
            return Err(Error("invalid preparation owner token"));
        }
        if c[1] == 0 {
            limit::validate(descriptor, body, center, c, parameters)?;
            if !f32::from_bits(c[0x3c / 4]).is_finite() {
                return Err(Error("invalid preparation existing cap"));
            }
        }
    }
    let mut out = Output {
        descriptor: descriptor.to_vec(),
        contacts: contacts.to_vec(),
        cursor,
        complete: false,
    };
    while out.cursor < contacts.len() {
        let c = &mut out.contacts[out.cursor];
        if c[1] == 0 {
            if skip != 0 {
                skip -= 1;
            } else {
                if budget == 0 {
                    break;
                }
                let result = limit::calculate(&out.descriptor, body, center, c, parameters)?;
                out.descriptor = result.descriptor;
                *c = result.contact;
                if result.cap != f64::from(parameters.unlimited) {
                    // Native saves ST(0) to f32 before COMISS; the comparison
                    // against the unlimited sentinel itself retains ST(0).
                    let rounded = result.cap as f32;
                    if f32::from_bits(c[0x3c / 4]) > rounded {
                        c[0x3c / 4] = rounded.to_bits();
                    }
                }
                c[0x44 / 4] = parameters.unlimited.to_bits();
                budget -= 1;
            }
        }
        out.cursor += 1;
    }
    out.complete = out.cursor == contacts.len();
    Ok(out)
}
