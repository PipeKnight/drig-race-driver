//! Supplied-grid queries over untransformed packed shapes. Population is separate.
use crate::{Error, candidate_grid::Grid, packed_query::Mesh, packed_shape, rectangle_cull::Cache};

pub struct Shape<'a> {
    pub id: u32,
    pub mesh: Mesh<'a>,
    pub materials: &'a [[u8; 4]],
}

#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub shape: u32,
    pub contact: packed_shape::Hit,
}

pub struct Query {
    pub candidates: Vec<u32>,
    pub hit: Option<Hit>,
}

/// Collect at most 25 unique shapes, then query each over the original segment.
/// A strictly closer hit replaces the prior shape; equal hits keep the first.
/// The culling cache is shared across every candidate, including missed shapes.
pub fn query(
    grid: &Grid,
    shapes: &[Shape<'_>],
    start: [f32; 4],
    end: [f32; 4],
    cache: &mut Cache,
) -> Result<Query, Error> {
    if start.into_iter().chain(end).any(|v| !v.is_finite()) || shapes.len() > 512 {
        return Err(Error("invalid packed-world segment or shape count"));
    }
    if !grid.active {
        return Ok(Query {
            candidates: Vec::new(),
            hit: None,
        });
    }
    // IDs 1..=n in order are unique and allow direct lookup.
    let sequential = shapes
        .iter()
        .enumerate()
        .all(|(i, s)| s.id as usize == i + 1);
    if !sequential {
        for (i, shape) in shapes.iter().enumerate() {
            if shape.id == 0 || shapes[..i].iter().any(|s| s.id == shape.id) {
                return Err(Error("invalid or duplicate packed-world shape ID"));
            }
        }
    }
    let low = [start[0].min(end[0]), start[2].min(end[2])];
    let high = [start[0].max(end[0]), start[2].max(end[2])];
    let candidates = grid.candidates(low, high, 25)?;
    let mut hit: Option<Hit> = None;
    for id in &candidates {
        let shape = if sequential {
            (*id as usize).checked_sub(1).and_then(|i| shapes.get(i))
        } else {
            shapes.iter().find(|s| s.id == *id)
        }
        .ok_or(Error("packed-world candidate absent"))?;
        if let Some(contact) = packed_shape::query(&shape.mesh, shape.materials, start, end, cache)?
            && hit.is_none_or(|old| contact.fraction < old.contact.fraction)
        {
            hit = Some(Hit {
                shape: *id,
                contact,
            });
        }
    }
    Ok(Query { candidates, hit })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::candidate_grid::Tile;
    use std::collections::BTreeMap;
    #[test]
    fn equal_shape_hits_keep_first_and_later_candidates_still_run() {
        let vertices = [[0., 0., 0.], [1., 0., 0.], [0., 0., 1.]];
        let triangles = [[0, 1, 2]];
        let leaves = BTreeMap::from([(0, vec![0])]);
        let materials = [*b"TEST"];
        let shapes: Vec<_> = (1..=2)
            .map(|id| Shape {
                id,
                mesh: Mesh {
                    bounds: [0., -1., 0., 1., 1., 1.],
                    vertices: &vertices,
                    triangles: &triangles,
                    nodes: &[0x8000],
                    leaves: &leaves,
                },
                materials: &materials,
            })
            .collect();
        let mut grid = Grid {
            bounds: [0., 0., 1., 1.],
            active: true,
            tiles: vec![
                Tile {
                    exponent: 0,
                    cells: vec![0]
                };
                1024
            ],
        };
        grid.tiles[8 * 32 + 8].cells[0] = 2;
        grid.tiles[8 * 32 + 9].cells[0] = 1;
        let result = query(
            &grid,
            &shapes,
            [0.25, 1., 0.25, 1.],
            [0.3, -1., 0.25, 1.],
            &mut Cache::default(),
        )
        .unwrap();
        assert_eq!(result.candidates, vec![2, 1]);
        assert_eq!(result.hit.unwrap().shape, 2);
        grid.active = false;
        assert!(
            query(&grid, &shapes, [0.; 4], [1.; 4], &mut Cache::default())
                .unwrap()
                .hit
                .is_none()
        );
    }
}
