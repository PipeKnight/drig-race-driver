//! The player driver's g-force lean (docs/COCKPIT.md "Lean", 0x857F20):
//! two points rigid with the driver model, at the torso and 0.35 m above
//! it, each drive a head-cam spring (Spring S, 0x83E6D0) with their
//! acceleration; the springs' offsets, scaled, clamped and squared with
//! their sign, weight the driver clips 5..16 (0x851CA0, 0x851DC0,
//! 0x851EE0, 0x852660).
use crate::camera::{Spring, SpringState, spring_step};
use bevy::prelude::*;

/// The low point's height in the driver frame (0xCAFE64); its x is the
/// interior's D_Lhand_Dummy x (post-init 0x860100).
const LOW_HEIGHT: f32 = 0.75;
/// The high point above the low one (0xCF7F18).
const HIGH_ABOVE: f32 = 0.35;
/// Driver post-init 0x860100: spring A (high, driver +0x70) and B (low,
/// +0x100); B keeps the base constructor's sy = 0.
const HIGH: Spring = Spring { scale: Vec2::new(1., 1.), damping: 13., inertia: 10., centering: 10., max_offset: 1. };
const LOW: Spring = Spring { scale: Vec2::new(1., 0.), damping: 20., inertia: 1., centering: 3., max_offset: 1. };
/// The offsets' output scales (+0xF0..+0xF8, +0x180/+0x184).
const HIGH_SCALE: Vec3 = Vec3::new(2., 2., 0.5);
const LOW_SCALE: Vec3 = Vec3::new(1., 1., 0.5);
/// Ticks stepped at most in one frame (a longer gap is a seek).
const MAX_TICKS: u64 = 8;

/// The two springs and the lean clips' weights.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Lean {
    high: SpringState,
    low: SpringState,
    /// The points' world velocities at the last update.
    last: Option<[Vec3; 2]>,
    /// The animblend weights 0..11 of clips 5..16.
    pub weights: [f32; 12],
}

impl Lean {
    /// Advances `ticks` physics ticks ending with the driver model at
    /// `frame` (world), the car body moving at `velocity` and `angular`
    /// about `origin`. `anchor_x` is the low point's x.
    ///
    /// The original finite-differences the points' world positions once per
    /// rendered frame (0x840710); the port takes their rigid-body velocity
    /// and steps once per tick, as the head-cam springs (F320).
    pub fn update(&mut self, anchor_x: f32, frame: Mat4, origin: Vec3, velocity: Vec3, angular: Vec3, ticks: u64) {
        let rotation = Mat3::from_mat4(frame);
        let low = frame.transform_point3(Vec3::new(anchor_x, LOW_HEIGHT, 0.));
        let high = low + HIGH_ABOVE * rotation.y_axis;
        let now = [high, low].map(|p| velocity + angular.cross(p - origin));
        let last = *self.last.get_or_insert_with(|| {
            // The first call has no velocity history (0x840710 gives 0).
            self.high.prev = now[0];
            self.low.prev = now[1];
            now
        });
        let ticks = ticks.min(MAX_TICKS);
        for k in 1..=ticks {
            let t = k as f32 / ticks as f32;
            let dt = grid_tools::source_vehicle::DELTA;
            spring_step(&HIGH, &mut self.high, dt, rotation, last[0].lerp(now[0], t));
            spring_step(&LOW, &mut self.low, dt, rotation, last[1].lerp(now[1], t));
        }
        self.last = Some(now);
        // Offset / max · scale, clamped, squared with its sign.
        let square = |s: &SpringState, p: &Spring, scale: Vec3| {
            let n = (s.off / p.max_offset * scale).clamp(Vec3::splat(-1.), Vec3::ONE);
            n * n.abs()
        };
        let (a, b) = (square(&self.high, &HIGH, HIGH_SCALE), square(&self.low, &LOW, LOW_SCALE));
        let pos = |x: f32| x.max(0.);
        self.weights = [
            pos(-b.x),
            pos(b.x),
            pos(-b.z),
            pos(b.z),
            pos(b.y),
            pos(-b.y),
            pos(-a.x),
            pos(a.x),
            pos(-a.z),
            pos(a.z),
            pos(a.y),
            pos(-a.y),
        ];
    }
}

/// The player driver's lean state.
#[derive(Resource, Default)]
pub struct Leaning {
    pub lean: Option<Lean>,
    /// The segment and tick of the last update.
    last: Option<(u64, u64)>,
}

/// Steps the lean by the ticks since the last frame; a restart (new
/// segment, fewer ticks) or a new cockpit starts over.
pub(crate) fn update(
    driving: Option<Res<crate::Driving>>,
    cockpit: Option<Res<crate::cockpit::Cockpit>>,
    mut leaning: ResMut<Leaning>,
) {
    let (Some(driving), Some(cockpit)) = (driving, cockpit) else {
        *leaning = Leaning::default();
        return;
    };
    let Some(driver) = &cockpit.driver else {
        return;
    };
    let (segment, tick) = (driving.segment, driving.runtime.ticks);
    let restart = cockpit.is_changed() || leaning.last.is_none_or(|(s, t)| s != segment || tick < t);
    let ticks = match leaning.last {
        Some((_, t)) if !restart => tick - t,
        _ => 0,
    };
    if restart {
        leaning.lean = Some(Lean::default());
    }
    leaning.last = Some((segment, tick));
    let body = Mat4::from_cols_array(&driving.runtime.pose());
    let [vx, vy, vz, _] = driving.runtime.velocity();
    let [wx, wy, wz, _] = driving.runtime.angular_velocity();
    if let Some(lean) = &mut leaning.lean {
        lean.update(
            driver.lean_anchor(),
            body * driver.root(),
            body.w_axis.truncate(),
            Vec3::new(vx, vy, vz),
            Vec3::new(wx, wy, wz),
            ticks,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(lean: &mut Lean, accel: Vec3, seconds: f32) -> Vec<[f32; 12]> {
        let mut v = Vec3::ZERO;
        let mut out = Vec::new();
        for _ in 0..(seconds / grid_tools::source_vehicle::DELTA).round() as usize {
            v += accel * grid_tools::source_vehicle::DELTA;
            lean.update(0.48, Mat4::IDENTITY, Vec3::ZERO, v, Vec3::ZERO, 1);
            out.push(lean.weights);
        }
        out
    }

    /// At rest nothing leans; braking (deceleration along −z, the driver's
    /// forward being +z) weights the forward clips 14 (high) and, past 2 g,
    /// 8 (low); a left turn (acceleration toward +x, the driver's left)
    /// weights clips 5 and 11 [D 0x851CA0..0x851EE0].
    #[test]
    fn lean_clips_follow_the_acceleration() {
        let mut lean = Lean::default();
        assert!(run(&mut lean, Vec3::ZERO, 1.).iter().all(|w| w.iter().all(|&w| w == 0.)));
        let peak = |ws: &[[f32; 12]], k: usize| ws.iter().map(|w| w[k]).fold(0., f32::max);
        let braking = run(&mut Lean::default(), Vec3::new(0., 0., -25.), 1.);
        assert!(peak(&braking, 9) > 0. && peak(&braking, 3) > 0., "{:?}", braking.last());
        assert_eq!(peak(&braking, 8) + peak(&braking, 2), 0.);
        let left = run(&mut Lean::default(), Vec3::new(10., 0., 0.), 1.);
        assert!(peak(&left, 6) > 0.05, "{:?}", left.last());
        assert_eq!(peak(&left, 7), 0.);
        // 1 g does not move the low spring (it needs about 2 g).
        assert_eq!(peak(&left, 0), 0.);
        // The low spring has no vertical freedom: clips 9 and 10 never weigh.
        let up = run(&mut Lean::default(), Vec3::new(0., 30., 0.), 1.);
        assert_eq!(peak(&up, 4) + peak(&up, 5), 0.);
        assert!(peak(&up, 11) > 0.);
    }
}
