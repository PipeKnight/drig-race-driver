//! Selected registered A0A290 car job: sync, full driver, notified publication.
//! A successful return is the owned completion boundary; OS task dispatch is external.
pub use crate::vehicle_outer::{Output, State};
use crate::{
    Error, contact_ray, suspension_prepare::Contact, vehicle_contact_force::Material,
    vehicle_force, vehicle_handling, vehicle_publication, vehicle_sync,
};

/// Preserves complete input on failure, including a late mutable-effect failure.
pub fn calculate(
    input: &State,
    pair: [usize; 2],
    delta: f32,
    gravity: f32,
    query: impl FnMut(usize, contact_ray::Ray) -> Result<Option<Contact>, Error>,
    material: impl FnMut(u32) -> Result<Option<Material>, Error>,
) -> Result<Output, Error> {
    if input.owner.vehicle.len() == vehicle_publication::VEHICLE_WORDS
        && input.owner.vehicle[(vehicle_handling::MUTABLE + 0x440) / 4] & 0xff0000 != 0
    {
        return Err(Error("unsupported handling branch"));
    }
    calculate_with_queries(input, pair, delta, gravity, None, query, material)
}

/// Registered job with owned handling lookup records and healthy notifications.
pub fn calculate_with_queries(
    input: &State,
    pair: [usize; 2],
    delta: f32,
    gravity: f32,
    handling_queries: Option<&[vehicle_handling::QueryRecord]>,
    query: impl FnMut(usize, contact_ray::Ray) -> Result<Option<Contact>, Error>,
    material: impl FnMut(u32) -> Result<Option<Material>, Error>,
) -> Result<Output, Error> {
    calculate_owned(
        input,
        pair,
        delta,
        gravity,
        handling_queries,
        query,
        material,
        false,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn calculate_owned(
    input: &State,
    pair: [usize; 2],
    delta: f32,
    gravity: f32,
    handling_queries: Option<&[vehicle_handling::QueryRecord]>,
    query: impl FnMut(usize, contact_ray::Ray) -> Result<Option<Contact>, Error>,
    material: impl FnMut(u32) -> Result<Option<Material>, Error>,
    world_center: bool,
) -> Result<Output, Error> {
    if input.owner.vehicle.len() != vehicle_publication::VEHICLE_WORDS
        || input.owner.vehicle[..input.frame.body.len()] != input.frame.body
    {
        return Err(Error("inconsistent car job ownership"));
    }
    let p = &input.owner.physical;
    if [0x328, 0x32c, 0x338, 0x33c, 0x340]
        .iter()
        .any(|&o| p[o / 4] != 0)
        || p[0x334 / 4] != if world_center { 0x1c0 } else { 0 }
        || p[0x400 / 4] == u32::MAX
        || p[0x230 / 4..0x240 / 4]
            .iter()
            .any(|&w| !f32::from_bits(w).is_finite())
    {
        return Err(Error("unsupported car job physical links"));
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
        .copy_from_slice(&synced.vehicle[..vehicle_force::BODY_WORDS]);
    let materials =
        crate::vehicle_step::apply(&mut next.frame, pair, delta, gravity, query, material)?;
    next.owner.vehicle[..vehicle_force::BODY_WORDS].copy_from_slice(&next.frame.body);
    next.owner = vehicle_handling::publish_with_queries(&next.owner, delta, handling_queries)?;
    next.frame
        .body
        .copy_from_slice(&next.owner.vehicle[..vehicle_force::BODY_WORDS]);
    Ok(Output {
        state: next,
        materials,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn late_handling_failure_keeps_sync_driver_and_publication_private() {
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
        vehicle[0xb10 / 4] = 1 << 24;
        vehicle[0xb14 / 4] = 1;
        vehicle[0x9a0 / 4] = 7;
        vehicle[0xc798 / 4] = 0.1f32.to_bits();
        let m = vehicle_handling::MUTABLE;
        for (o, x) in [
            (0x274, 0.65f32),
            (0x278, 0.95),
            (0x368, 0.8),
            (0x430, 1.),
            (0x434, 1.),
            (0x438, 1.),
        ] {
            vehicle[(m + o) / 4] = x.to_bits();
        }
        for i in 0..4 {
            vehicle[(m + 0x18 + i * 4) / 4] = 0.95f32.to_bits();
        }
        vehicle[(m + 0x444) / 4] = 7;
        let mut input = State {
            frame: vehicle_force::Frame {
                body: vehicle[..vehicle_force::BODY_WORDS].try_into().unwrap(),
                view: [0; vehicle_force::VIEW_WORDS],
                solver: [0; 884],
                driven: vec![0, 1],
                material_multipliers: [None; 4],
                diagnostics: [0.; 8],
            },
            owner: vehicle_publication::State {
                vehicle,
                physical,
                damage_random_state: None,
            },
        };
        let query = |_, _| panic!("zero delta must not query source contacts");
        let good = calculate(&input, [0, 1], 0., 9.81, query, |_| Ok(None)).unwrap();
        assert_eq!(good.state.owner.vehicle[0x9a0 / 4], 7);
        assert_eq!(good.state.owner.physical[0x3f0 / 4], 1f32.to_bits());
        input.owner.vehicle[(m + 0x274) / 4] = 0.95f32.to_bits();
        let before = input.clone();
        assert_eq!(
            calculate(&input, [0, 1], 0., 9.81, query, |_| Ok(None))
                .err()
                .unwrap()
                .0,
            "nonfinite steering control result"
        );
        assert_eq!(input.owner.vehicle, before.owner.vehicle);
        assert_eq!(input.owner.physical, before.owner.physical);
        assert_eq!(format!("{:?}", input.frame), format!("{:?}", before.frame));
    }
}
