//! Supplied untransformed packed mesh traversal. World population is separate.
use crate::{
    Error,
    leaf_query::{Cursor, Hit},
    rectangle_cull::Cache,
};
use std::collections::BTreeMap;

pub struct Mesh<'a> {
    pub bounds: [f32; 6],
    pub vertices: &'a [[f32; 3]],
    pub triangles: &'a [[u16; 3]],
    pub nodes: &'a [u16],
    pub leaves: &'a BTreeMap<usize, Vec<u16>>,
}

fn split(b: [f32; 4]) -> [[f32; 4]; 4] {
    let x = (b[2] - b[0]) * 0.5;
    let z = (b[3] - b[1]) * 0.5;
    [
        [b[0], b[1] + z, b[2] - x, b[3]],
        [b[0] + x, b[1] + z, b[2], b[3]],
        [b[0], b[1], b[2] - x, b[3] - z],
        [b[0] + x, b[1], b[2], b[3] - z],
    ]
}

impl Mesh<'_> {
    /// Root rectangle culling, child masks computed before ordered recursion,
    /// and leaf hit state shared across the entire query. Caller retains cache.
    pub fn query(
        &self,
        start: [f32; 4],
        end: [f32; 4],
        cache: &mut Cache,
    ) -> Result<Option<Hit>, Error> {
        if self.nodes.is_empty()
            || self.nodes.len() > 1000000
            || self.vertices.len() > 1024
            || self.triangles.len() > 32768
            || self
                .bounds
                .into_iter()
                .chain(start)
                .chain(end)
                .chain(self.vertices.iter().flatten().copied())
                .any(|v| !v.is_finite())
            || (0..3).any(|i| self.bounds[i] >= self.bounds[i + 3])
        {
            return Err(Error("invalid packed-query mesh or segment"));
        }
        let direction = std::array::from_fn(|i| end[i] - start[i]);
        if direction.into_iter().any(|v: f32| !v.is_finite()) {
            return Err(Error("packed-query displacement overflow"));
        }
        let b = [
            self.bounds[0],
            self.bounds[2],
            self.bounds[3],
            self.bounds[5],
        ];
        let center = [(b[0] + b[2]) * 0.5, (b[1] + b[3]) * 0.5];
        let half = [(b[2] - b[0]) * 0.5, (b[3] - b[1]) * 0.5];
        if !cache.intersects(
            [start[0], start[2]],
            [direction[0], direction[2]],
            center,
            half,
        )? {
            return Ok(None);
        }
        let mut cursor = Cursor::new(start, direction);
        let mut stack = vec![(0usize, b, 0usize)];
        let mut visits = 0usize;
        while let Some((index, bounds, depth)) = stack.pop() {
            if cursor.hit.is_some_and(|h| h.zero_fraction) {
                break;
            }
            visits += 1;
            if depth > 64 || visits > 1000000 {
                return Err(Error("packed-query traversal bound exceeded"));
            }
            let node = *self
                .nodes
                .get(index)
                .ok_or(Error("packed-query node outside table"))?;
            if node == 0xffff {
                continue;
            }
            if node & 0x8000 != 0 {
                let order = self
                    .leaves
                    .get(&((node & 0x7fff) as usize))
                    .ok_or(Error("packed-query leaf absent"))?;
                cursor.visit(self.vertices, self.triangles, order)?;
            } else {
                let child = node as usize;
                if child + 4 > self.nodes.len() {
                    return Err(Error("packed-query children outside table"));
                }
                let quadrants = split(bounds);
                // Original consumer uses first child's half extents for all four.
                let q = quadrants[0];
                let half = [(q[2] - q[0]) * 0.5, (q[3] - q[1]) * 0.5];
                let mut accepted = [false; 4];
                for (i, q) in quadrants.iter().enumerate() {
                    let center = [(q[0] + q[2]) * 0.5, (q[1] + q[3]) * 0.5];
                    accepted[i] = cache.intersects(
                        [start[0], start[2]],
                        [cursor.delta[0], cursor.delta[2]],
                        center,
                        half,
                    )?;
                }
                for i in (0..4).rev() {
                    if accepted[i] {
                        stack.push((child + i, quadrants[i], depth + 1));
                    }
                }
            }
        }
        Ok(cursor.hit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_state_crosses_children_and_later_tie_wins() {
        let vertices = [[0., 0., 0.], [1., 0., 0.], [0., 0., 1.]];
        let triangles = [[0, 1, 2], [0, 1, 2]];
        let leaves = BTreeMap::from([(0, vec![0]), (3, vec![1])]);
        let mesh = Mesh {
            bounds: [0., -1., 0., 1., 1., 1.],
            vertices: &vertices,
            triangles: &triangles,
            nodes: &[1, 0x8000, 0x8003, 0xffff, 0xffff],
            leaves: &leaves,
        };
        let hit = mesh
            .query(
                [0.5, 1., 0.5, 1.],
                [0.5, -1., 0.5, 1.],
                &mut Cache::default(),
            )
            .unwrap()
            .unwrap();
        assert_eq!(hit.triangle, 1);
        assert_eq!(hit.fraction, 0.5);
    }
    #[test]
    fn cyclic_and_missing_children_are_bounded() {
        let leaves = BTreeMap::new();
        let mesh = Mesh {
            bounds: [0., -1., 0., 1., 1., 1.],
            vertices: &[],
            triangles: &[],
            nodes: &[0, 0xffff, 0xffff, 0xffff],
            leaves: &leaves,
        };
        assert!(
            mesh.query([0., 1., 1., 1.], [0., -1., 1., 1.], &mut Cache::default())
                .is_err()
        );
    }
}
