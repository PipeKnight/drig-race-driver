//! Supplied collision candidate grid; no shape intersection or grid construction.
use crate::Error;

#[derive(Clone, Debug)]
pub struct Tile {
    /// Original subdivision exponent: width is 2^exponent.
    pub exponent: u8,
    /// Row-major cells; zero denotes absence, other values are stable object IDs.
    pub cells: Vec<u32>,
}

#[derive(Clone, Debug)]
pub struct Grid {
    /// Minimum X/Z followed by maximum X/Z of the supplied reference bounds.
    pub bounds: [f32; 4],
    pub active: bool,
    pub tiles: Vec<Tile>,
}

impl Grid {
    pub fn coordinates(&self, point: [f32; 2]) -> Result<[u32; 2], Error> {
        let mut out = [0; 2];
        for axis in 0..2 {
            let extent = self.bounds[axis + 2] - self.bounds[axis];
            if !self.bounds[axis].is_finite()
                || !extent.is_finite()
                || extent <= 0.
                || !point[axis].is_finite()
            {
                return Err(Error("invalid candidate-grid bounds or point"));
            }
            let value = ((point[axis] - self.bounds[axis]) / extent) * 65536.;
            // Exclude SSE indefinite-integer overflow; Rust casts saturate instead.
            if !(-2147483648. ..2147483648.).contains(&value) {
                return Err(Error("candidate-grid conversion outside signed range"));
            }
            out[axis] = (value as i32).clamp(0, 65535) as u32;
        }
        Ok(out)
    }

    /// Ordered X/Z rectangle; first-seen unique nonzero IDs, at most 25.
    pub fn candidates(
        &self,
        low: [f32; 2],
        high: [f32; 2],
        capacity: usize,
    ) -> Result<Vec<u32>, Error> {
        if self.tiles.len() != 1024 || capacity > 25 || low[0] > high[0] || low[1] > high[1] {
            return Err(Error("invalid candidate-grid framing"));
        }
        // The cap is an authored bound. Production subdivision sizes are separate.
        for tile in &self.tiles {
            if tile.exponent > 6 || tile.cells.len() != 1usize << (2 * tile.exponent) {
                return Err(Error("invalid candidate-grid tile"));
            }
        }
        let a = self.coordinates(low)?;
        let b = self.coordinates(high)?;
        let mut ids = Vec::new();
        if !self.active || capacity == 0 {
            return Ok(ids);
        }
        for z in (a[1] >> 11)..=(b[1] >> 11) {
            for x in (a[0] >> 11)..=(b[0] >> 11) {
                let tile = &self.tiles[(z * 32 + x) as usize];
                let shift = 11 - tile.exponent;
                let width = 1u32 << tile.exponent;
                let local_a = [
                    (a[0] >> shift).saturating_sub(x * width),
                    (a[1] >> shift).saturating_sub(z * width),
                ];
                let local_b = [
                    (b[0] >> shift).saturating_sub(x * width).min(width - 1),
                    (b[1] >> shift).saturating_sub(z * width).min(width - 1),
                ];
                for cz in local_a[1]..=local_b[1] {
                    for cx in local_a[0]..=local_b[0] {
                        let id = tile.cells[(cz * width + cx) as usize];
                        if id != 0 && !ids.contains(&id) {
                            if ids.len() == capacity {
                                return Ok(ids);
                            }
                            ids.push(id);
                        }
                    }
                }
            }
        }
        Ok(ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clamping_order_duplicates_and_capacity() {
        let mut grid = Grid {
            bounds: [0., 0., 32., 32.],
            active: true,
            tiles: vec![
                Tile {
                    exponent: 0,
                    cells: vec![0]
                };
                1024
            ],
        };
        assert_eq!(grid.coordinates([-1., 40.]).unwrap(), [0, 65535]);
        grid.tiles[0].cells[0] = 7;
        grid.tiles[1].cells[0] = 7;
        grid.tiles[2].cells[0] = 4;
        grid.tiles[32].cells[0] = 9;
        assert_eq!(
            grid.candidates([0., 0.], [3., 1.], 25).unwrap(),
            vec![7, 4, 9]
        );
        assert_eq!(grid.candidates([0., 0.], [3., 1.], 2).unwrap(), vec![7, 4]);
        grid.active = false;
        assert!(
            grid.candidates([0., 0.], [32., 32.], 25)
                .unwrap()
                .is_empty()
        );
    }
    #[test]
    fn subdivision_edges_and_invalid_conversion() {
        let mut grid = Grid {
            bounds: [0., 0., 32., 32.],
            active: true,
            tiles: vec![
                Tile {
                    exponent: 1,
                    cells: vec![1, 2, 3, 4]
                };
                1024
            ],
        };
        assert_eq!(
            grid.candidates([0.5, 0.5], [0.5, 0.5], 25).unwrap(),
            vec![4]
        );
        assert_eq!(
            grid.candidates([0., 0.], [0.5, 0.5], 25).unwrap(),
            vec![1, 2, 3, 4]
        );
        assert!(grid.coordinates([f32::MAX, 0.]).is_err());
        grid.tiles[0].cells.pop();
        assert!(grid.candidates([0., 0.], [1., 1.], 25).is_err());
    }
}
