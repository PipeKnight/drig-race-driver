//! Selected complete A09A80 update with owned linked state and source contacts.
//! Empty shapes/manager and stable setup; no physical advance or world dispatch.
use crate::{
    Error, contact_ray, suspension_prepare::Contact, vehicle_contact_force::Material,
    vehicle_force::Frame, vehicle_publication, vehicle_sync,
};

#[derive(Clone)]
pub struct State {
    pub owner: vehicle_publication::State,
    /// Body words must agree with the owner's prefix. Addresses are opaque tags.
    pub frame: Frame,
}
pub struct Output {
    pub state: State,
    pub materials: [Option<Material>; 4],
}

/// Synchronize → driver wrapper → decay/velocity publication → final setters.
/// The input remains unchanged on failure, including failures after contact queries.
pub fn calculate(
    input: &State,
    pair: [usize; 2],
    delta: f32,
    gravity: f32,
    query: impl FnMut(usize, contact_ray::Ray) -> Result<Option<Contact>, Error>,
    material: impl FnMut(u32) -> Result<Option<Material>, Error>,
) -> Result<Output, Error> {
    if input.owner.vehicle.len() != vehicle_publication::VEHICLE_WORDS
        || input.owner.vehicle[..input.frame.body.len()] != input.frame.body
    {
        return Err(Error("inconsistent outer vehicle ownership"));
    }
    let synced = vehicle_sync::calculate(&vehicle_sync::State {
        vehicle: input.owner.vehicle[..vehicle_sync::VEHICLE_WORDS].to_vec(),
        physical: input.owner.physical,
    })?;
    let mut next = input.clone();
    next.owner.vehicle[..vehicle_sync::VEHICLE_WORDS].copy_from_slice(&synced.vehicle);
    next.owner.physical = synced.physical;
    next.frame
        .body
        .copy_from_slice(&synced.vehicle[..crate::vehicle_force::BODY_WORDS]);
    let materials =
        crate::vehicle_step::apply(&mut next.frame, pair, delta, gravity, query, material)?;
    next.owner.vehicle[..crate::vehicle_force::BODY_WORDS].copy_from_slice(&next.frame.body);
    next.owner = vehicle_publication::calculate(&next.owner, delta)?;
    next.owner.vehicle[0x9a0 / 4] &= !255;
    next.frame
        .body
        .copy_from_slice(&next.owner.vehicle[..crate::vehicle_force::BODY_WORDS]);
    let published = crate::vehicle_bridge::calculate(
        &crate::vehicle_bridge::State {
            physical: next.owner.physical,
            vehicle: publication_frame(next.frame.body),
            shapes: Vec::new(),
        },
        &crate::vehicle_bridge::Settings {
            notify: true,
            advance: None,
            correction: None,
            group_head: false,
        },
    )?;
    // A09A80 has no getters after the setters. Preserve the car's own pose words;
    // canonicalization belongs to the linked physical publication only.
    next.owner.physical = published.written.physical;
    Ok(Output {
        state: next,
        materials,
    })
}

fn publication_frame(
    mut words: [u32; crate::vehicle_force::BODY_WORDS],
) -> crate::suspension_prepare::State {
    // Actual CAF400/410/420 masks select X/Y/Z, respectively. The synchronous
    // wrapper clears these lanes in local copies, not W and not the car fields.
    for o in [0x15c0, 0x15d4, 0x1610, 0x1618] {
        words[o / 4] = 0;
    }
    crate::suspension_prepare::State { words }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn late_notification_failure_keeps_complete_input_unchanged() {
        let mut vehicle = vec![0; vehicle_publication::VEHICLE_WORDS];
        let mut physical = [0; vehicle_publication::PHYSICAL_WORDS];
        for o in [0xfa8, 0xfac, 0xfb0, 0xfb4, 0x1560] {
            vehicle[o / 4] = 1f32.to_bits();
        }
        for i in 0..4 {
            physical[0x1d0 / 4 + i * 5] = 1f32.to_bits();
        }
        vehicle[0x1620 / 4] = 4;
        vehicle[0xba0 / 4] = 2;
        vehicle[0x20fb4 / 4] = 1;
        vehicle[0x9a0 / 4] = 7;
        let mut input = State {
            frame: Frame {
                body: vehicle[..crate::vehicle_force::BODY_WORDS]
                    .try_into()
                    .unwrap(),
                view: [0; crate::vehicle_force::VIEW_WORDS],
                solver: [0; 884],
                driven: vec![0, 1],
                material_multipliers: [None; 4],
                diagnostics: [0.; 8],
            },
            owner: vehicle_publication::State { vehicle, physical },
        };
        let query = |_, _| panic!("zero delta must skip source queries");
        let good = calculate(&input, [0, 1], 0., 9.81, query, |_| Ok(None)).unwrap();
        assert_eq!(good.state.frame.body[0x9a0 / 4], 0);
        assert_eq!(input.frame.body[0x9a0 / 4], 7);
        input.owner.physical[0x400 / 4] = u32::MAX;
        let before = input.clone();
        let result = calculate(&input, [0, 1], 0., 9.81, query, |_| Ok(None));
        assert_eq!(
            result.err().unwrap().0,
            "unsupported vehicle/physical bridge state"
        );
        assert_eq!(input.owner.vehicle, before.owner.vehicle);
        assert_eq!(input.owner.physical, before.owner.physical);
        assert_eq!(format!("{:?}", input.frame), format!("{:?}", before.frame));
    }
    #[test]
    fn synchronous_publication_projects_axes_without_changing_car_words() {
        let words = std::array::from_fn(|i| i as u32 + 1);
        let projected = publication_frame(words);
        for (i, word) in words.into_iter().enumerate() {
            assert_eq!(
                projected.words[i],
                if [0x15c0, 0x15d4, 0x1610, 0x1618].contains(&(i * 4)) {
                    0
                } else {
                    word
                }
            );
        }
        assert_ne!(words[0x1610 / 4], 0);
    }
}
