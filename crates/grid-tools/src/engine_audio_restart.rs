//! Original 423210 state transition with an explicit supplied playback service.
//! Commands describe PCM seeks; this module does not emulate the native backend.
use crate::engine_audio_control::{CONTROLLER_BYTES, CURVE_WORDS};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Command {
    pub slot: u32,
    pub resource: u32,
    pub sample: u32,
    pub flags: u32,
    pub group: u32,
    pub requested_microseconds: u32,
    pub accepted_microseconds: u32,
}
#[derive(Clone, Copy)]
pub struct PlaybackService {
    /// u32::MAX retains the requested seek; other values model backend replies.
    pub accepted_microseconds: [u32; 2],
    pub cooldown: f32,
    pub resource: u32,
    pub groups: [u32; 2],
}
fn u(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
fn f(b: &[u8], at: usize) -> f32 {
    f32::from_bits(u(b, at))
}
fn put(b: &mut [u8], at: usize, v: f32) {
    b[at..at + 4].copy_from_slice(&v.to_le_bytes());
}
fn index(v: f32) -> usize {
    ((v * 1024.) as i32).clamp(0, 1023) as usize
}

/// Resolved groups and accepted seeks are explicit boundary inputs, not a port
/// of the original device backend or group resolver.
pub fn restart(
    controller: &mut [u8; CONTROLLER_BYTES],
    curves: &[f32; CURVE_WORDS],
    category: usize,
    service: PlaybackService,
) -> Result<Vec<Command>, &'static str> {
    if category >= 3
        || !service.cooldown.is_finite()
        || !(0.0..=1.).contains(&service.cooldown)
        || service
            .accepted_microseconds
            .iter()
            .any(|v| *v != u32::MAX && *v > 60_000_000)
        || curves.iter().enumerate().any(|(i, v)| {
            !v.is_finite() || !(0.0..=crate::engine_audio::CURVE_MAX).contains(v) || (i % 2048 < 1024 && *v <= 0.)
        })
        || !f(controller, 0xc8).is_finite()
        || !(0.0..=1.).contains(&f(controller, 0xc8))
        || !f(controller, 0x134 + category * 4).is_finite()
        || f(controller, 0x134 + category * 4).abs() > 60.
        || controller[0x14c + category] > 1
        || controller[0x14f + category] > 1
    {
        return Err("engine restart domain");
    }
    let outgoing = u(controller, 0x1d4 + category * 4) as usize;
    let incoming = u(controller, 0x1e0 + category * 4) as usize;
    if ![0, 2].contains(&outgoing) || incoming != 2 - outgoing {
        return Err("engine restart pair domain");
    }
    for bank in 0..4 {
        let slot = bank * 3 + category;
        if !(1..=10_000_000).contains(&u(controller, 0x2c0 + slot * 4))
            || !(8000..=192000).contains(&u(controller, 0x290 + slot * 4))
        {
            return Err("engine restart sample domain");
        }
    }
    if controller[0x14c + category] == 0 || f(controller, 0x134 + category * 4) >= 0. {
        return Ok(Vec::new());
    }
    let base = if category == 0 { 0 } else { 2048 };
    let source_slot = outgoing * 3 + category;
    let time = curves[base + 1024 + index(f(controller, 0xc8))];
    let requested = ((f64::from(u(controller, 0x2c0 + source_slot * 4)) * f64::from(time)
        / f64::from(u(controller, 0x290 + source_slot * 4)))
        * 1_000_000.) as u32;
    let mut next = *controller;
    let mut commands = Vec::with_capacity(2);
    for emitter in 0..2 {
        let slot = (incoming + emitter) * 3 + category;
        let accepted = if service.accepted_microseconds[emitter] == u32::MAX {
            requested
        } else {
            service.accepted_microseconds[emitter]
        };
        let position = (f64::from(accepted) * f64::from(1e-6_f32))
            * f64::from(u(controller, 0x290 + slot * 4));
        put(&mut next, 0x230 + slot * 4, position as f32);
        commands.push(Command {
            slot: slot as u32,
            resource: service.resource,
            sample: u(controller, 0x154 + emitter * 12 + category * 4),
            flags: u(controller, 0x22c),
            group: service.groups[emitter],
            requested_microseconds: requested,
            accepted_microseconds: accepted,
        });
    }
    next[0x14f + category] = u8::from(next[0x14f + category] == 0);
    let slot = incoming * 3 + category;
    let position = f(&next, 0x230 + slot * 4) as i32;
    let time = ((f64::from(position) / f64::from(u(&next, 0x2c0 + slot * 4))) as f32).min(1.);
    let raw_pitch = f64::from(f(&next, 0xc8)) / f64::from(curves[base + index(time)]);
    if raw_pitch > 1.5 {
        return Err("engine restart logger-free domain");
    }
    let pitch = raw_pitch as f32;
    for offset in [0x260, 0x26c, 0x1a4, 0x1b0] {
        put(&mut next, offset + slot * 4, pitch);
    }
    next[0x14c + category] = 0;
    put(&mut next, 0x134 + category * 4, service.cooldown);
    *controller = next;
    Ok(commands)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn logger_boundary_failure_preserves_all_state_and_zero_cooldown_waits() {
        let mut c = [0x6b; CONTROLLER_BYTES];
        put(&mut c, 0xc8, 1.);
        put(&mut c, 0x134, -1.);
        c[0x14c] = 1;
        c[0x14f] = 0;
        c[0x1d4..0x1d8].copy_from_slice(&2_u32.to_le_bytes());
        c[0x1e0..0x1e4].copy_from_slice(&0_u32.to_le_bytes());
        for bank in 0..4 {
            let slot = bank * 3;
            c[0x290 + slot * 4..0x294 + slot * 4].copy_from_slice(&32000_u32.to_le_bytes());
            c[0x2c0 + slot * 4..0x2c4 + slot * 4].copy_from_slice(&100_u32.to_le_bytes());
        }
        let service = PlaybackService {
            accepted_microseconds: [u32::MAX; 2],
            cooldown: 0.125,
            resource: 1,
            groups: [1, 0],
        };
        let before = c;
        assert!(restart(&mut c, &[0.5; CURVE_WORDS], 0, service).is_err());
        assert_eq!(c, before);
        put(&mut c, 0xc8, 0.5);
        put(&mut c, 0x134, -0.);
        let before = c;
        assert!(
            restart(&mut c, &[0.5; CURVE_WORDS], 0, service)
                .unwrap()
                .is_empty()
        );
        assert_eq!(c, before);
        put(&mut c, 0x134, -0.001);
        let commands = restart(&mut c, &[0.5; CURVE_WORDS], 0, service).unwrap();
        assert_eq!(commands.len(), 2);
        assert_eq!(c[0x14f], 1);
        assert_eq!(c[0x14c], 0);
    }
}
