//! AUTHORED free-look chase camera (owner request; not an original GRID
//! feature): hold the right mouse button and move the mouse to orbit the chase
//! camera around the car, without a yaw limit; release it and the camera eases
//! back behind the car. Presentation only, never feeds the simulation.
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;

/// Current free-look angles relative to the normal chase view (radians).
#[derive(Resource, Default, Clone, Copy, Debug, PartialEq)]
pub struct FreeLook {
    pub yaw: f32,
    pub pitch: f32,
}

/// Radians per mouse count.
const SENSITIVITY: f32 = 0.005;
/// Pitch range: slightly below the normal height up to a high overhead view.
const PITCH: [f32; 2] = [-0.2, 1.1];
/// Exponential return rate (1/s) after the button is released.
const RETURN_RATE: f32 = 6.;

fn wrap(a: f32) -> f32 {
    (a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

/// Applies one frame of mouse input (`held`, motion in counts) or the return.
pub fn step(look: &mut FreeLook, held: bool, motion: Vec2, dt: f32) {
    if held {
        // Mouse right turns the view right: the camera swings to the car's left.
        look.yaw = wrap(look.yaw - motion.x * SENSITIVITY);
        look.pitch = (look.pitch - motion.y * SENSITIVITY).clamp(PITCH[0], PITCH[1]);
    } else {
        let keep = (-RETURN_RATE * dt).exp();
        look.yaw = wrap(look.yaw) * keep;
        look.pitch *= keep;
        if look.yaw.abs() < 1e-4 && look.pitch.abs() < 1e-4 {
            *look = FreeLook::default();
        }
    }
}

/// `GRID_FREE_LOOK=yaw,pitch` (radians) holds a fixed view for scripted renders.
fn scripted() -> Option<FreeLook> {
    let value = std::env::var("GRID_FREE_LOOK").ok()?;
    let (yaw, pitch) = value.split_once(',')?;
    Some(FreeLook {
        yaw: yaw.trim().parse().ok()?,
        pitch: pitch.trim().parse::<f32>().ok()?.clamp(PITCH[0], PITCH[1]),
    })
}

/// Update system; real time keeps the camera responsive while paused.
pub fn update(
    buttons: Option<Res<ButtonInput<MouseButton>>>,
    motion: Option<Res<AccumulatedMouseMotion>>,
    time: Res<Time<Real>>,
    mut look: ResMut<FreeLook>,
    mut fixed: Local<Option<Option<FreeLook>>>,
) {
    if let Some(view) = *fixed.get_or_insert_with(scripted) {
        *look = view;
        return;
    }
    let held = buttons.is_some_and(|b| b.pressed(MouseButton::Right));
    let delta = motion.map_or(Vec2::ZERO, |m| m.delta);
    step(&mut look, held, delta, time.delta_secs());
}

/// Car-local camera offset from the look-at point: `offset` rotated by the
/// free-look angles. `side` is -1 for the look-back view (pitch then raises the
/// camera the same way).
pub fn offset(look: Option<&FreeLook>, offset: Vec3, side: f32) -> Vec3 {
    match look {
        Some(l) if l.yaw != 0. || l.pitch != 0. => {
            Quat::from_rotation_y(l.yaw) * Quat::from_rotation_x(l.pitch * side) * offset
        }
        _ => offset,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drag_rotates_without_limit_and_release_returns() {
        let mut look = FreeLook::default();
        // Drag right by 2000 counts (10 rad): yaw wraps instead of stopping.
        for _ in 0..20 {
            step(&mut look, true, Vec2::new(100., 0.), 1. / 60.);
        }
        assert!(look.yaw.abs() <= std::f32::consts::PI);
        assert_ne!(look.yaw, 0.);
        // Vertical drag is clamped.
        step(&mut look, true, Vec2::new(0., -10_000.), 1. / 60.);
        assert_eq!(look.pitch, PITCH[1]);
        // Two seconds after release the view is back behind the car.
        for _ in 0..120 {
            step(&mut look, false, Vec2::new(50., 50.), 1. / 60.);
        }
        assert_eq!(look, FreeLook::default());
    }

    #[test]
    fn swing_direction_and_neutral_offset() {
        let base = Vec3::new(0., 2., -9.);
        assert_eq!(offset(Some(&FreeLook::default()), base, 1.), base);
        // Dragging right moves the camera toward the car's left (+X).
        let mut look = FreeLook::default();
        step(&mut look, true, Vec2::new(100., 0.), 1. / 60.);
        assert!(offset(Some(&look), base, 1.).x > 0.);
        // Positive pitch raises the camera, also in the look-back view.
        let up = FreeLook {
            yaw: 0.,
            pitch: 0.5,
        };
        assert!(offset(Some(&up), base, 1.).y > base.y);
        let back = Vec3::new(0., 2., 9.);
        assert!(offset(Some(&up), back, -1.).y > back.y);
    }
}
