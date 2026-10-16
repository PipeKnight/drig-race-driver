//! Stable filtering of existing contact pools; geometric contact detection is external.
use crate::Error;

#[derive(Clone, Copy)]
pub struct Contact {
    pub identity: u32,
    pub first: u32,
    pub second: u32,
    /// When false, the second identity denotes a different object kind.
    pub second_is_body: bool,
    pub disabled: [bool; 2],
}
pub struct Input<'a> {
    pub owner: u32,
    pub capacity: usize,
    pub initial: &'a [u32],
    pub first_pool: &'a [Contact],
    pub second_pool: &'a [Contact],
}
pub fn collect(input: Input<'_>) -> Result<Vec<u32>, Error> {
    if input.owner == 0
        || !(1..=80).contains(&input.capacity)
        || input.initial.len() > input.capacity
        || input.first_pool.len() > crate::MAX_WORLD_CONTACTS
        || input.second_pool.len() > crate::MAX_WORLD_CONTACTS
        || input
            .first_pool
            .iter()
            .chain(input.second_pool)
            .any(|c| c.identity == 0)
    {
        return Err(Error("invalid contact collection input"));
    }
    let mut output = input.initial.to_vec();
    for (pool, second) in [(input.first_pool, false), (input.second_pool, true)] {
        for c in pool {
            if (c.first == input.owner || (second && c.second_is_body && c.second == input.owner))
                && !c.disabled[0]
                && !c.disabled[1]
                && output.len() < input.capacity
            {
                output.push(c.identity);
            }
        }
    }
    Ok(output)
}
