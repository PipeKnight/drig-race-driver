//! Complete selected mode0 body handler with explicit notification ownership.
//! Dynamic other-car response is excluded; original pointers never enter State.
use crate::{Error, collision_damage, vehicle_publication::State};

#[derive(Clone, Copy)]
pub struct Parameters {
    pub impact_scale: f32,
    pub timer_cap: f32,
    pub linear_squared_threshold: f32,
    pub suspension_scale: f32,
    pub suspension_gain: f32,
    pub suspension_offset: f32,
}
#[derive(Clone, Copy)]
pub struct Event {
    pub point: [f32; 4],
    pub normal: [f32; 4],
    pub impact: f32,
    pub parameter: f32,
    pub other_registration: u32,
    pub other_word: u32,
}
pub struct Output {
    pub state: State,
    pub random_state: u32,
    /// Ordered owned packets for the explicit registered notification users.
    /// The first word is the canonical current-car identity, never a pointer.
    pub notifications: Vec<[u32; 14]>,
}
fn f(b: &[u32], o: usize) -> f32 {
    f32::from_bits(b[o / 4])
}
fn byte(b: &[u32], o: usize) -> u8 {
    (b[o / 4] >> (o % 4 * 8)) as u8
}

/// The caller establishes the registered side and absence of a dynamic peer.
/// Notification slots belong to a separate owned context, not the car projection.
pub fn calculate(
    input: &State,
    event: Event,
    side: usize,
    parameters: Parameters,
    users: &[Option<u32>; 11],
    random_state: u32,
) -> Result<Output, Error> {
    let p = parameters;
    if input.vehicle.len() != crate::vehicle_publication::VEHICLE_WORDS
        || side > 1
        || input.vehicle[0xdb0 / 4] != 0
        || byte(&input.vehicle, 0x308) != 0
        || [0xb13, 0xb14].iter().any(|&o| byte(&input.vehicle, o) > 1)
        // 9EF360 has no point domain (only finiteness is kept); the normal
        // bound is a port sanity guard on a direction.
        || event.point.iter().any(|v| !v.is_finite())
        || event
            .normal
            .iter()
            .any(|v| !v.is_finite() || v.abs() > 1000.)
        || event.point[3] != 1.
        || !event.impact.is_finite()
        || !(0. ..=0.1).contains(&event.parameter)
        || (event.impact != f32::MAX && !(0. ..=1_000_000_000.).contains(&event.impact))
        || [
            p.impact_scale,
            p.timer_cap,
            p.suspension_scale,
            p.suspension_gain,
            p.suspension_offset,
        ]
        .iter()
        .any(|v| !(0. ..=1000.).contains(v))
        || !(0. ..=1_000_000_000.).contains(&p.linear_squared_threshold)
        || !(0. ..=1_000_000_000.).contains(&f(&input.vehicle, 0x15a0))
        || !(0. ..=1000.).contains(&f(&input.vehicle, 0x15a4))
        || [0x2e8, 0x3d0]
            .iter()
            .any(|&o| !(-1000. ..=1000.).contains(&f(&input.vehicle, o)))
        || input.physical[0x220 / 4..0x230 / 4]
            .iter()
            .any(|&v| !(-10000. ..=10000.).contains(&f32::from_bits(v)))
    {
        return Err(Error("unsupported body impact context"));
    }
    // The selected owned context admits the complete damage descriptor even
    // when its two dispatch flags disable the reached damage call.
    collision_damage::calculate(input, event.point, 0., false, random_state)?;
    let mut out = Output {
        state: input.clone(),
        random_state,
        notifications: Vec::new(),
    };
    if event.impact == f32::MAX {
        return Ok(out);
    }
    out.state.vehicle[0x1568 / 4] = 0;
    let mask = 0xff << 8; // Byte BA5 in its containing word.
    out.state.vehicle[0xba5 / 4] &= !mask;
    if byte(&input.vehicle, 0xb13) != 0 && byte(&input.vehicle, 0xb14) != 0 {
        let result = collision_damage::calculate(
            &out.state,
            event.point,
            p.impact_scale * event.impact,
            false,
            random_state,
        )?;
        out.state = result.state;
        out.random_state = result.random_state;
        out.state.vehicle[0xba5 / 4] |= u32::from(result.applied) << 8;
    }
    let b = &mut out.state.vehicle;
    if event.impact > f(b, 0x15a0) && f(b, 0x2e8) < p.timer_cap {
        b[0x2e8 / 4] = p.timer_cap.to_bits();
    }
    let a: [f32; 4] = std::array::from_fn(|j| f(&input.physical, 0x220 + j * 4));
    let squared = a.map(|v| v * v);
    let magnitude = (squared[0] + squared[1]) + (squared[2] + squared[3]);
    if magnitude < p.linear_squared_threshold {
        let gain = (1. - (f(b, 0x15a4) * event.impact) * p.suspension_scale).clamp(0., 1.);
        let target = gain * p.suspension_gain + p.suspension_offset;
        b[0x3d0 / 4] = f(b, 0x3d0).max(0.).min(target).to_bits();
    }
    let normal = event
        .normal
        .map(|v| v.to_bits() ^ if side == 1 { 0x8000_0000 } else { 0 });
    for &user in users.iter().flatten() {
        let mut packet = [0; 14];
        packet[..3].copy_from_slice(&[1, user, event.other_registration]);
        packet[3..7].copy_from_slice(&event.point.map(f32::to_bits));
        packet[7..11].copy_from_slice(&normal);
        packet[11..].copy_from_slice(&[
            event.other_word,
            event.impact.to_bits(),
            event.parameter.to_bits(),
        ]);
        out.notifications.push(packet);
    }
    Ok(out)
}
