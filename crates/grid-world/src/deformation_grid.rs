//! Ordered position sampling from the supplied regular deformation descriptor.
use crate::Error;

pub struct Grid<'a> {
    pub dimensions: [usize; 3],
    pub spacing: f32,
    pub minimum: [f32; 4],
    /// First four lanes of each descriptor-relative80-byte node, X fastest.
    pub positions: &'a [[f32; 4]],
}

impl Grid<'_> {
    pub fn sample(&self, point: [f32; 4]) -> Result<[f32; 4], Error> {
        if self.dimensions.iter().any(|d| !(2..=16).contains(d))
            || self.dimensions.iter().product::<usize>() > 400
            || self.positions.len() != self.dimensions.iter().product::<usize>()
            || !self.spacing.is_finite()
            || self.spacing <= 0.
            || self
                .minimum
                .into_iter()
                .chain(point)
                .chain(self.positions.iter().flatten().copied())
                .any(|v| !v.is_finite())
        {
            return Err(Error("invalid deformation grid input"));
        }
        let mut clamped = point;
        for (axis, value) in clamped.iter_mut().take(3).enumerate() {
            let lower = self.minimum[axis];
            let upper = ((self.dimensions[axis] - 1) as f32 - f32::from_bits(0x3a83126f))
                * self.spacing
                + lower;
            if !upper.is_finite() || upper <= lower {
                return Err(Error("collapsed deformation grid bounds"));
            }
            if *value < lower {
                *value = lower;
            }
            if *value > upper {
                *value = upper;
            }
        }
        let inverse = 1. / self.spacing;
        let scaled: [f32; 3] = std::array::from_fn(|i| inverse * (clamped[i] - self.minimum[i]));
        if scaled
            .iter()
            .enumerate()
            .any(|(i, v)| !v.is_finite() || *v < 0. || *v >= (self.dimensions[i] - 1) as f32)
        {
            return Err(Error("deformation cell outside bounded grid"));
        }
        let cell = scaled.map(|v| v as usize);
        let fraction: [f32; 3] = std::array::from_fn(|i| scaled[i] - cell[i] as f32);
        let node = |x, y, z| self.positions[x + self.dimensions[0] * (y + self.dimensions[1] * z)];
        let blend = |a: [f32; 4], b: [f32; 4], t: f32| -> [f32; 4] {
            std::array::from_fn(|i| t * (b[i] - a[i]) + a[i])
        };
        let plane = |z| {
            blend(
                blend(
                    node(cell[0], cell[1], z),
                    node(cell[0] + 1, cell[1], z),
                    fraction[0],
                ),
                blend(
                    node(cell[0], cell[1] + 1, z),
                    node(cell[0] + 1, cell[1] + 1, z),
                    fraction[0],
                ),
                fraction[1],
            )
        };
        let interpolated = blend(plane(cell[2]), plane(cell[2] + 1), fraction[2]);
        let result = std::array::from_fn(|i| (interpolated[i] - clamped[i]) + point[i]);
        if result.iter().any(|v| !v.is_finite()) {
            return Err(Error("nonfinite deformation grid result"));
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identity_grid_restores_query_outside_lookup_bounds() {
        let positions = [
            [0., 0., 0., 1.],
            [1., 0., 0., 1.],
            [0., 1., 0., 1.],
            [1., 1., 0., 1.],
            [0., 0., 1., 1.],
            [1., 0., 1., 1.],
            [0., 1., 1., 1.],
            [1., 1., 1., 1.],
        ];
        let mut grid = Grid {
            dimensions: [2; 3],
            spacing: 1.,
            minimum: [0.; 4],
            positions: &positions,
        };
        assert_eq!(grid.sample([-1., 2., 0.5, 7.]).unwrap(), [-1., 2., 0.5, 1.]);
        grid.spacing = 0.;
        assert!(grid.sample([0.; 4]).is_err());
    }
}
