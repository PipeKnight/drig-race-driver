//! Independent visual displacement policy over the verified source grid.
//! No original graphical shader, detached-part or renderer equivalence claim.
use grid_world::{Error, deformation_grid::Grid, vehicle_publication::VEHICLE_WORDS};
pub struct Field {
    dimensions: [usize; 3],
    spacing: f32,
    minimum: [f32; 4],
    current: Vec<[f32; 4]>,
    rest: Vec<[f32; 4]>,
    active: bool,
}
impl Field {
    pub fn read(owner: &[u32]) -> Result<Self, Error> {
        if owner.len() != VEHICLE_WORDS || owner[0xc7ac / 4] >> 8 & 255 > 1 {
            return Err(Error("invalid visual deformation owner"));
        }
        let dimensions: [usize; 3] = std::array::from_fn(|i| owner[0xc714 / 4 + i] as usize);
        if dimensions.iter().any(|d| !(2..=16).contains(d))
            || dimensions.iter().product::<usize>() > 400
        {
            return Err(Error("invalid visual deformation dimensions"));
        }
        let count = dimensions.iter().product::<usize>();
        let f = |at| f32::from_bits(owner[at / 4]);
        let out = Self {
            dimensions,
            spacing: f(0xc720),
            minimum: std::array::from_fn(|i| f(0xc730 + i * 4)),
            current: (0..count)
                .map(|i| std::array::from_fn(|a| f(0x4880 + i * 80 + a * 4)))
                .collect(),
            rest: (0..count)
                .map(|i| std::array::from_fn(|a| f(0x4890 + i * 80 + a * 4)))
                .collect(),
            active: owner[0xc7ac / 4] >> 8 & 255 != 0,
        };
        out.grid(false).prepare()?;
        out.grid(true).prepare()?;
        Ok(out)
    }
    fn grid(&self, rest: bool) -> Grid<'_> {
        Grid {
            dimensions: self.dimensions,
            spacing: self.spacing,
            minimum: self.minimum,
            positions: if rest { &self.rest } else { &self.current },
        }
    }
    /// Complete cache key includes consumed current/rest nodes and layout only.
    pub fn key(&self) -> Vec<u32> {
        self.dimensions
            .into_iter()
            .map(|d| d as u32)
            .chain([self.spacing.to_bits(), u32::from(self.active)])
            .chain(self.minimum.map(f32::to_bits))
            .chain(
                self.current
                    .iter()
                    .chain(&self.rest)
                    .flatten()
                    .map(|v| v.to_bits()),
            )
            .collect()
    }
    /// Delta relative to the pristine source lattice avoids baseline rounding
    /// shifts and cumulative mesh drift. XYZ are the chosen visual policy.
    pub fn displacements(&self, points: &[[f32; 4]]) -> Result<Vec<[f32; 3]>, Error> {
        if points.len() > 1_000_000 || points.iter().flatten().any(|v| !v.is_finite()) {
            return Err(Error("invalid visual deformation queries"));
        }
        if !self.active {
            return Ok(vec![[0.; 3]; points.len()]);
        }
        let current_grid = self.grid(false);
        let rest_grid = self.grid(true);
        let current = current_grid.prepare()?;
        let rest = rest_grid.prepare()?;
        points
            .iter()
            .map(|&p| {
                let a = current.sample(p)?;
                let b = rest.sample(p)?;
                let delta = std::array::from_fn(|i| a[i] - b[i]);
                if delta.iter().any(|v| !v.is_finite()) {
                    Err(Error("nonfinite visual deformation displacement"))
                } else {
                    Ok(delta)
                }
            })
            .collect()
    }
}
