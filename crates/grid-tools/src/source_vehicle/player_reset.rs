//! The original's only reset path, for the local player's car
//! (ResetLinesManager, docs/RESET_AND_RESPAWN_RESEARCH.md 3): the car is
//! re-placed at rest on its line point once it has been nearly still on its
//! side or roof for 2 s. AI cars are never reset (F332).
//!
//! AUTHORED: the placement happens on the tick the detector fires. The
//! original first fades the screen out (`0.9 - clamp(0.02 |v|, 0, 0.4)` s
//! [I]) and places the car when it is black; the fade, the camera hold and
//! the pause menu's confirmation fade are not ported. The pause menu's Reset
//! Vehicle uses [`place`] (grid-app, docs/PAUSE_MENU.md).
use super::{Runtime, multi::MultiRuntime};
use crate::ai_driver::Line;
use grid_world::{Error, reset_lines::Detector};

/// Auto-reset state of the player's car. Shared by grid-app's drive tick and
/// the soak [`crate::soak::Session`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerReset {
    pub detector: Detector,
}

/// A performed reset: the line point and heading the car was placed at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reset {
    pub point: [f32; 3],
    pub heading: [f32; 3],
}

impl PlayerReset {
    /// After a successful tick: update the detector with the player's car
    /// and, when it fires, place the car at rest on its line point. With a
    /// shared world, `player` is the runtime held outside slot 0 of `shared`
    /// (grid-app and the soak Session swap it in for the common tick). A
    /// retired player is never reset.
    pub fn update(
        &mut self,
        player: &mut Runtime,
        shared: Option<&mut MultiRuntime>,
        line: &Line,
    ) -> Result<Option<Reset>, Error> {
        if shared.as_ref().is_some_and(|m| m.retirement(0).is_some()) {
            return Ok(None);
        }
        let pose = player.pose();
        let [v, w] = [player.velocity(), player.angular_velocity()];
        if !self.detector.update(
            [v[0], v[1], v[2]],
            [w[0], w[1], w[2]],
            pose[5],
            super::DELTA,
        ) {
            return Ok(None);
        }
        place(player, shared, line).map(Some)
    }

    /// `R` restarts the whole event.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// The placement itself: the player's car at rest on the line point nearest
/// its position. Also the pause menu's Reset Vehicle (`c_pause_reset_car`,
/// the same ResetLinesManager placement [I]). A retired player is refused.
pub fn place(
    player: &mut Runtime,
    shared: Option<&mut MultiRuntime>,
    line: &Line,
) -> Result<Reset, Error> {
    let pose = player.pose();
    let (point, heading) = line.reset_target([pose[12], pose[13], pose[14]]);
    match shared {
        Some(multi) => {
            std::mem::swap(player, &mut multi.cars[0]);
            let result = multi.place_at_rest(0, point, heading);
            std::mem::swap(player, &mut multi.cars[0]);
            result?;
        }
        None => player.place_at_rest(point, heading)?,
    }
    Ok(Reset { point, heading })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_vehicle::Controls;
    use grid_world::reset_lines::length;
    use std::path::Path;

    /// The grid pose turned onto its roof, 1.5 m up.
    fn flipped(mut pose: [f32; 16]) -> [f32; 16] {
        for k in 0..3 {
            pose[k] = -pose[k];
            pose[4 + k] = -pose[4 + k];
        }
        pose[13] += 1.5;
        pose
    }

    const HOLD: Controls = Controls {
        throttle: 0.,
        brake: 0.,
        steering: 0.,
        handbrake: true,
    };

    fn speed(car: &Runtime) -> f32 {
        let v = car.velocity();
        (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
    }

    /// Upright, still, on the road and in first gear: the state right after
    /// the placement and after two more seconds.
    fn check_placed(car: &Runtime, reset: &Reset, rest: bool) {
        let p = car.pose();
        let drift = (p[12] - reset.point[0]).hypot(p[14] - reset.point[2]);
        // Heading along the line: forward row is the line direction.
        let along = p[8] * reset.heading[0] + p[10] * reset.heading[2];
        let mut cache = grid_world::rectangle_cull::Cache::default();
        let below = car
            .track()
            .query(
                [p[12], p[13], p[14], 1.],
                [p[12], p[13] - 3., p[14], 1.],
                &mut cache,
            )
            .unwrap()
            .hit
            .map(|h| 3. * h.contact.fraction);
        println!(
            "placed rest={rest} up_y={} drift={drift} along={along} speed={} ground_below={below:?} gear={}",
            p[5],
            speed(car),
            car.state.car.owner.vehicle[0x196c / 4]
        );
        assert!(p[5] > 0.95, "upright: {}", p[5]);
        assert!(along > 0.95, "heading along the line: {along}");
        assert!(below.is_some_and(|d| d < 1.), "on the road: {below:?}");
        if rest {
            assert!(speed(car) < 0.3, "at rest: {}", speed(car));
            assert!(drift < 0.5, "stays put: {drift}");
        } else {
            assert_eq!(speed(car), 0.);
            assert!(drift < 1e-3, "at the line point: {drift}");
            assert_eq!(car.state.car.owner.vehicle[0x196c / 4], 1);
            for i in 0..4 {
                let spin = car.state.car.owner.vehicle[(0x1990 + i * 0x150 + 0x104) / 4];
                assert_eq!(f32::from_bits(spin), 0.);
            }
        }
    }

    fn run(test: impl FnOnce() + Send + 'static) {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(test)
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS"]
    fn a_flipped_player_car_is_reset_upright_on_the_road_after_two_seconds() {
        run(|| {
            let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
            let root = Path::new(&root);
            let table =
                std::fs::read(std::env::var_os("GRID_CONTACT_DIRECTIONS").unwrap()).unwrap();
            let mut car = Runtime::lacetti(root, "slot_00", 200679).unwrap();
            let geometry = car.prepare_body_geometry(root, &table).unwrap();
            let line = Line::load(root, car.route().path()).unwrap();
            let damage = car.state.car.owner.vehicle[0xcdf0 / 4..0xce30 / 4].to_vec();
            car.set_pose_at_rest(flipped(car.pose()));
            let mut reset = PlayerReset::default();
            let mut still_since = None;
            let mut fired = None;
            for tick in 1..=1200u64 {
                car.tick_with_body_geometry(HOLD, &geometry).unwrap();
                let (v, w) = (car.velocity(), car.angular_velocity());
                let tilted = car.pose()[5] <= 0.17
                    && length([v[0], v[1], v[2]]) + length([w[0], w[1], w[2]]) <= 0.8;
                if !tilted {
                    still_since = None;
                } else if still_since.is_none() {
                    still_since = Some(tick);
                }
                if let Some(r) = reset.update(&mut car, None, &line).unwrap() {
                    fired = Some((tick, r));
                    break;
                }
            }
            let (tick, placed) = fired.expect("the auto-reset fires");
            println!("fired at tick {tick}, still and tilted since {still_since:?}");
            // 2 s of still-and-tilted (125 accumulating ticks), then the firing one.
            assert!(
                (125..=127).contains(&(tick - still_since.unwrap())),
                "{tick} {still_since:?}"
            );
            check_placed(&car, &placed, false);
            // The placement keeps the damage state.
            assert_eq!(
                car.state.car.owner.vehicle[0xcdf0 / 4..0xce30 / 4],
                damage[..]
            );
            for _ in 0..125 {
                car.tick_with_body_geometry(HOLD, &geometry).unwrap();
                assert_eq!(reset.update(&mut car, None, &line).unwrap(), None);
            }
            check_placed(&car, &placed, true);
            // Engine speed 0 after the placement: the car still drives off.
            let go = Controls {
                throttle: 1.,
                ..Default::default()
            };
            for _ in 0..250 {
                car.tick_with_body_geometry(go, &geometry).unwrap();
            }
            println!("drive-off speed after 4 s: {}", speed(&car));
            assert!(speed(&car) > 10., "drives off: {}", speed(&car));
        });
    }

    /// The same through the shared world: the player runtime is held outside
    /// slot 0 between ticks, as in grid-app and the soak Session.
    #[test]
    #[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS"]
    fn a_flipped_player_car_is_reset_in_the_shared_world() {
        run(|| {
            let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
            let root = Path::new(&root);
            let table =
                std::fs::read(std::env::var_os("GRID_CONTACT_DIRECTIONS").unwrap()).unwrap();
            let mut multi =
                MultiRuntime::lacetti(root, &["slot_00", "slot_01"], &table, None).unwrap();
            let line = Line::load(root, multi.cars[0].route().path()).unwrap();
            let pose = flipped(multi.cars[0].pose());
            multi.set_pose_at_rest(0, pose);
            let mut player = Runtime::lacetti(root, "slot_00", 200679).unwrap();
            std::mem::swap(&mut player, &mut multi.cars[0]);
            let mut reset = PlayerReset::default();
            let mut fired = None;
            for tick in 1..=1200u64 {
                std::mem::swap(&mut player, &mut multi.cars[0]);
                multi.tick(&[HOLD, HOLD]).unwrap();
                std::mem::swap(&mut player, &mut multi.cars[0]);
                if let Some(r) = reset.update(&mut player, Some(&mut multi), &line).unwrap() {
                    fired = Some((tick, r));
                    break;
                }
            }
            let (tick, placed) = fired.expect("the auto-reset fires");
            println!("shared world: fired at tick {tick}");
            check_placed(&player, &placed, false);
            for _ in 0..125 {
                std::mem::swap(&mut player, &mut multi.cars[0]);
                multi.tick(&[HOLD, HOLD]).unwrap();
                std::mem::swap(&mut player, &mut multi.cars[0]);
                assert_eq!(
                    reset.update(&mut player, Some(&mut multi), &line).unwrap(),
                    None
                );
            }
            check_placed(&player, &placed, true);
            assert!(multi.retirement(0).is_none() && multi.retirement(1).is_none());
        });
    }
}
