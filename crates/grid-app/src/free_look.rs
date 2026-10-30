//! AUTHORED free-look chase camera (owner request; not an original GRID
//! feature): hold the right mouse button and move the mouse to orbit the chase
//! camera around the car, without a yaw limit; release it and the camera eases
//! back behind the car. Presentation only, never feeds the simulation.
//! The head-cam turns the driver's view with the same gesture (owner request,
//! backlog row 101; the original only has the Look keys there).
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;

/// Current free-look angles relative to the normal chase view (radians).
#[derive(Resource, Default, Clone, Copy, Debug, PartialEq)]
pub struct FreeLook {
    pub yaw: f32,
    pub pitch: f32,
}
/// Offscreen diagnosis of temporal trails during the owner's existing orbit gesture.
#[derive(Resource)]
pub struct CaptureOrbitRate(pub f32);

/// Radians per mouse count, horizontal and vertical (owner: vertical softer).
const SENSITIVITY: f32 = 0.005;
const PITCH_SENSITIVITY: f32 = 0.002;
/// Pitch range: slightly below the normal height up to a raised view.
const PITCH: [f32; 2] = [-0.15, 0.45];
/// Exponential return rate (1/s) after the button is released.
const RETURN_RATE: f32 = 6.;
/// The head-cam's yaw either way: out of the side windows. Further round,
/// the interior models show untextured backs never meant to be seen.
pub const HEAD_YAW: f32 = std::f32::consts::FRAC_PI_2;

fn wrap(a: f32) -> f32 {
    (a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

/// Applies one frame of mouse input (`held`, motion in counts) or the return.
pub fn step(look: &mut FreeLook, held: bool, motion: Vec2, dt: f32) {
    if held {
        // Mouse right turns the view right: the camera swings to the car's left.
        look.yaw = wrap(look.yaw - motion.x * SENSITIVITY);
        // Mouse down raises the camera (owner preference after a playtest).
        look.pitch = (look.pitch + motion.y * PITCH_SENSITIVITY).clamp(PITCH[0], PITCH[1]);
    } else {
        let keep = (-RETURN_RATE * dt).exp();
        look.yaw = wrap(look.yaw) * keep;
        look.pitch *= keep;
        if look.yaw.abs() < 1e-4 && look.pitch.abs() < 1e-4 {
            *look = FreeLook::default();
        }
    }
}

/// Diagnostics `free_look = yaw,pitch` (radians; legacy `GRID_FREE_LOOK`) holds a fixed view for scripted renders.
fn scripted() -> Option<FreeLook> {
    let value = crate::settings::diagnostics().free_look.clone()?;
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
    orbit_rate: Option<Res<CaptureOrbitRate>>,
) {
    if let Some(rate) = orbit_rate {
        let base = fixed.get_or_insert_with(scripted).unwrap_or_default();
        *look = FreeLook {
            yaw: wrap(base.yaw + rate.0 * time.elapsed_secs()),
            pitch: base.pitch,
        };
        return;
    }
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

/// Holds the free look inside the head-cam's yaw range, so a drag past it
/// stops there instead of winding up.
pub fn hold_head(look: &mut FreeLook) {
    look.yaw = look.yaw.clamp(-HEAD_YAW, HEAD_YAW);
}

/// The head-cam's view `rotation` turned by the free look: yaw about the
/// car's `up` axis through the eye, then pitch about the view's right axis
/// (positive looks down, as positive pitch raises the chase camera).
pub fn turn_head(look: Option<&FreeLook>, up: Vec3, rotation: Quat) -> Quat {
    match look {
        Some(l) if l.yaw != 0. || l.pitch != 0. => {
            Quat::from_axis_angle(up, l.yaw.clamp(-HEAD_YAW, HEAD_YAW)) * rotation * Quat::from_rotation_x(-l.pitch)
        }
        _ => rotation,
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
        // Dragging down raises the camera, clamped at the top of the range.
        step(&mut look, true, Vec2::new(0., 10_000.), 1. / 60.);
        assert_eq!(look.pitch, PITCH[1]);
        step(&mut look, true, Vec2::new(0., -10_000.), 1. / 60.);
        assert_eq!(look.pitch, PITCH[0]);
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

    #[test]
    fn head_cam_turns_with_the_drag_and_stops_at_the_side_window() {
        // A car rolled a little, its view straight ahead (Bevy cameras look
        // along -Z; the car's forward is +Z, its left +X).
        let body = Quat::from_rotation_z(0.2);
        let forward = body * Quat::from_rotation_y(std::f32::consts::PI);
        let up = body * Vec3::Y;
        let view = |look: &FreeLook| body.inverse() * (turn_head(Some(look), up, forward) * Vec3::NEG_Z);
        assert_eq!(turn_head(Some(&FreeLook::default()), up, forward), forward);
        // Dragging right looks to the car's right (-X), keeping the horizon.
        let mut look = FreeLook::default();
        step(&mut look, true, Vec2::new(100., 0.), 1. / 60.);
        let dir = view(&look);
        assert!(dir.x < -0.4 && dir.y.abs() < 1e-5, "{dir}");
        // Dragging down looks down.
        step(&mut look, true, Vec2::new(0., 100.), 1. / 60.);
        assert!(view(&look).y < -0.1, "{}", view(&look));
        // A long drag stops at the limit; the release returns ahead.
        step(&mut look, true, Vec2::new(500., 0.), 1. / 60.);
        hold_head(&mut look);
        assert_eq!(look.yaw, -HEAD_YAW);
        for _ in 0..240 {
            step(&mut look, false, Vec2::ZERO, 1. / 60.);
        }
        assert_eq!(turn_head(Some(&look), up, forward), forward);
    }
}
