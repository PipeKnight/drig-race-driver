//! Original horizontal segment/rectangle rejection policy and cached plane.
use crate::Error;

#[derive(Clone, Copy, Debug, Default)]
pub struct Cache {
    pub plane: u8,
}
impl Cache {
    /// X/Z vectors; displacement is the finite segment, half_extents are positive.
    pub fn intersects(
        &mut self,
        start: [f32; 2],
        displacement: [f32; 2],
        center: [f32; 2],
        half_extents: [f32; 2],
    ) -> Result<bool, Error> {
        if self.plane > 3
            || start
                .into_iter()
                .chain(displacement)
                .chain(center)
                .chain(half_extents)
                .any(|v| !v.is_finite())
            || half_extents.into_iter().any(|v| v < 0.)
        {
            return Err(Error("invalid rectangle-cull input"));
        }
        let origin: [f32; 2] = std::array::from_fn(|i| start[i] - center[i]);
        if origin.into_iter().any(|v| !v.is_finite()) {
            return Err(Error("rectangle-cull origin overflow"));
        }
        let mut near = 0.;
        let mut far = 1.;
        let mut plane = self.plane;
        let mut sign = if plane & 1 != 0 { -1. } else { 1. };
        for _ in 0..4 {
            let axis = (plane >> 1) as usize;
            let delta = displacement[axis] * sign;
            let boundary = -(origin[axis] * sign) - half_extents[axis];
            let reject = if delta <= 0. {
                if delta >= 0. {
                    boundary > 0.
                } else if delta * near < boundary {
                    true
                } else {
                    if delta * far < boundary {
                        far = boundary / delta;
                    }
                    false
                }
            } else if delta * far < boundary {
                true
            } else {
                if delta * near < boundary {
                    near = boundary / delta;
                }
                false
            };
            if reject {
                self.plane = plane;
                return Ok(false);
            }
            plane = (plane + 1) & 3;
            sign *= -1.;
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn boundary_parallel_and_rejection_cache() {
        let mut cache = Cache::default();
        assert!(
            cache
                .intersects([1., 0.], [0., 1.], [0., 0.], [1., 1.])
                .unwrap()
        );
        assert_eq!(cache.plane, 0);
        assert!(
            !cache
                .intersects([2., 0.], [0., 1.], [0., 0.], [1., 1.])
                .unwrap()
        );
        assert_eq!(cache.plane, 1);
        assert!(
            !cache
                .intersects([-2., 0.], [0., 1.], [0., 0.], [1., 1.])
                .unwrap()
        );
        assert_eq!(cache.plane, 0);
        assert!(
            cache
                .intersects([-2., 0.], [4., 0.], [0., 0.], [1., 1.])
                .unwrap()
        );
        assert_eq!(cache.plane, 0);
    }
    #[test]
    fn corner_touch_zero_segment_and_invalid_bounds() {
        let mut cache = Cache { plane: 3 };
        assert!(
            cache
                .intersects([-2., 0.], [2., 2.], [0., 0.], [1., 1.])
                .unwrap()
        );
        assert!(
            cache
                .intersects([0., 0.], [0., -0.], [0., 0.], [0., 0.])
                .unwrap()
        );
        assert!(
            cache
                .intersects([0., 0.], [0., 0.], [0., 0.], [-1., 1.])
                .is_err()
        );
    }
}
