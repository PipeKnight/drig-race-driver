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
    /// The moved lattice nodes and the activity flag: the part of the field a
    /// replay records (the layout and rest lattice are fixed per car).
    pub fn current(&self) -> (&[[f32; 4]], bool) {
        (&self.current, self.active)
    }
    /// This car's field with a recorded lattice; the node count must match.
    pub fn with_current(&self, current: &[[f32; 4]], active: bool) -> Result<Self, Error> {
        if current.len() != self.current.len() || current.iter().flatten().any(|v| !v.is_finite()) {
            return Err(Error("recorded deformation lattice does not fit"));
        }
        let out = Self {
            dimensions: self.dimensions,
            spacing: self.spacing,
            minimum: self.minimum,
            current: current.to_vec(),
            rest: self.rest.clone(),
            active,
        };
        out.grid(false).prepare()?;
        Ok(out)
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
    /// Whether `owner` holds the field whose [`Self::key`] is `key`. The key
    /// holds every value `read` validates, so then `read` would succeed with
    /// that same key: a caller that keeps the key of its last successful read
    /// skips the read (two lattice copies, two validations) while the car's
    /// body is unchanged, which is most frames.
    pub fn unchanged(owner: &[u32], key: &[u32]) -> bool {
        if owner.len() != VEHICLE_WORDS || owner[0xc7ac / 4] >> 8 & 255 > 1 || key.len() < 9 {
            return false;
        }
        let count: usize = key[..3].iter().map(|&d| d as usize).product();
        let word = |at: usize| owner[at / 4];
        key.len() == 9 + 8 * count
            && (0..3).all(|i| word(0xc714 + i * 4) == key[i])
            && word(0xc720) == key[3]
            && u32::from(word(0xc7ac) >> 8 & 255 != 0) == key[4]
            && (0..4).all(|i| word(0xc730 + i * 4) == key[5 + i])
            && (0..count).all(|i| {
                let (current, rest) = (9 + i * 4, 9 + (count + i) * 4);
                owner[(0x4880 + i * 80) / 4..][..4] == key[current..current + 4]
                    && owner[(0x4890 + i * 80) / 4..][..4] == key[rest..rest + 4]
            })
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

#[cfg(test)]
mod tests {
    use super::*;

    fn owner() -> Vec<u32> {
        let mut owner = vec![0u32; VEHICLE_WORDS];
        for (i, d) in [2u32, 3, 2].into_iter().enumerate() {
            owner[0xc714 / 4 + i] = d;
        }
        owner[0xc720 / 4] = 0.5f32.to_bits();
        owner[0xc7ac / 4] = 1 << 8;
        for i in 0..12 {
            for a in 0..4 {
                owner[(0x4880 + i * 80) / 4 + a] = (i as f32 + a as f32 * 0.25).to_bits();
                owner[(0x4890 + i * 80) / 4 + a] = (i as f32 - a as f32).to_bits();
            }
        }
        owner
    }

    #[test]
    fn unchanged_matches_read_key() {
        let mut owner = owner();
        let key = Field::read(&owner).unwrap().key();
        assert!(Field::unchanged(&owner, &key));
        // Every word the key covers changes the answer, as it changes the key.
        let words = [0xc714, 0xc718, 0xc71c, 0xc720, 0xc730, 0xc73c, 0x4880, 0x4890 + 11 * 80 + 12];
        for at in words {
            let saved = owner[at / 4];
            owner[at / 4] ^= 1;
            let changed = Field::read(&owner).map(|f| f.key());
            assert!(!Field::unchanged(&owner, &key), "word {at:#x}");
            assert_ne!(changed.ok().as_ref(), Some(&key));
            owner[at / 4] = saved;
        }
        // An activity byte `read` refuses is never unchanged.
        owner[0xc7ac / 4] = 2 << 8;
        assert!(Field::read(&owner).is_err());
        assert!(!Field::unchanged(&owner, &key));
        owner[0xc7ac / 4] = 0;
        assert_eq!(
            Field::unchanged(&owner, &key),
            Field::read(&owner).unwrap().key() == key
        );
        // A word outside the field does not.
        let mut other = self::owner();
        other[0x4880 / 4 + 8] ^= 1;
        assert!(Field::unchanged(&other, &key));
        assert!(Field::read(&other).unwrap().key() == key);
    }
}
