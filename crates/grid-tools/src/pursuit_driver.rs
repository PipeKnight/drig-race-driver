//! AUTHORED placeholder opponent driver, not original GRID AI: pure pursuit of
//! the purchased progress centreline with a curvature speed limit. Same policy
//! as the validated `grid-practice-drive` diagnostic; replaced once the original
//! AI driver is recovered (docs/AI_DRIVER_RESEARCH.md).
use crate::source_vehicle::Controls;

#[derive(Clone)]
pub struct PursuitDriver {
    points: Vec<(f64, [f64; 3])>,
    length: f64,
    nearest: Option<usize>,
    pub maximum_speed: f64,
    pub gain: f64,
}

impl PursuitDriver {
    pub fn new(progress: &grid_formats::progress::Progress, maximum_speed: f64) -> Self {
        Self {
            points: progress
                .points
                .iter()
                .map(|p| (f64::from(p.distance), p.position.map(f64::from)))
                .collect(),
            length: f64::from(progress.total_distance),
            nearest: None,
            maximum_speed,
            gain: 1.5,
        }
    }

    pub fn reset(&mut self) {
        self.nearest = None;
    }

    fn target_at(&self, distance: f64) -> [f64; 3] {
        let distance = distance.rem_euclid(self.length);
        let i = self
            .points
            .partition_point(|p| p.0 <= distance)
            .saturating_sub(1);
        let (a, from) = self.points[i];
        let (b, to) = if i + 1 < self.points.len() {
            self.points[i + 1]
        } else {
            (self.length, self.points[0].1)
        };
        let fraction = ((distance - a) / (b - a)).clamp(0., 1.);
        std::array::from_fn(|j| from[j] + fraction * (to[j] - from[j]))
    }

    /// Controls for the current car pose (4x4 row-major, translation 12..14)
    /// and linear velocity.
    pub fn controls(&mut self, pose: [f32; 16], velocity: [f32; 4]) -> Controls {
        let pose = pose.map(f64::from);
        let position = [pose[12], pose[13], pose[14]];
        let velocity = velocity.map(f64::from);
        let speed = (velocity[0] * velocity[0] + velocity[2] * velocity[2]).sqrt();
        let n = self.points.len();
        let mut best = (f64::INFINITY, 0., 0usize);
        for i in 0..n {
            if let Some(nearest) = self.nearest {
                let ahead = (i + n - nearest) % n;
                let behind = (nearest + n - i) % n;
                if ahead > 10 && behind > 5 {
                    continue;
                }
            }
            let (distance, from) = self.points[i];
            let (next_distance, to) = if i + 1 < n {
                self.points[i + 1]
            } else {
                (self.length, self.points[0].1)
            };
            let dx = to[0] - from[0];
            let dz = to[2] - from[2];
            let square = dx * dx + dz * dz;
            if square <= 1e-9 {
                continue;
            }
            let t = (((position[0] - from[0]) * dx + (position[2] - from[2]) * dz) / square)
                .clamp(0., 1.);
            let error =
                (position[0] - from[0] - t * dx).powi(2) + (position[2] - from[2] - t * dz).powi(2);
            if error < best.0 {
                best = (error, distance + t * (next_distance - distance), i);
            }
        }
        self.nearest = Some(best.2);
        let lookahead = (8. + speed * 0.7).clamp(8., 30.);
        let target = self.target_at(best.1 + lookahead);
        let dx = target[0] - position[0];
        let dz = target[2] - position[2];
        let local_x = dx * pose[0] + dz * pose[2];
        let curvature = 2. * local_x / (dx * dx + dz * dz).max(1.);
        let steering = (self.gain * (curvature * 2.65).atan() / 0.45).clamp(-1., 1.) as f32;
        let desired = self
            .maximum_speed
            .min((3.5 / curvature.abs().max(0.002)).sqrt())
            .max(5.);
        let error = desired - speed;
        Controls {
            throttle: if error < -0.5 {
                0.
            } else {
                (0.22 + error * 0.3).clamp(0., 1.) as f32
            },
            brake: if error < -0.5 {
                (-error * 0.25).clamp(0., 0.8) as f32
            } else {
                0.
            },
            steering,
            handbrake: false,
        }
    }
}
