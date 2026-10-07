//! Ordered supplied leaf triangles; shape traversal and normal construction are separate.
use crate::{Error, triangle};

#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub triangle: usize,
    pub fraction: f32,
    /// Current shortened displacement, including W.
    pub displacement: [f32; 4],
    pub zero_fraction: bool,
}

pub fn resolve(
    start: [f32; 4],
    direction: [f32; 4],
    vertices: &[[f32; 3]],
    triangles: &[[u16; 3]],
    order: &[u16],
) -> Result<Option<Hit>, Error> {
    if order.len() > 256
        || start
            .into_iter()
            .chain(direction)
            .chain(vertices.iter().flatten().copied())
            .any(|f| !f.is_finite())
    {
        return Err(Error("invalid leaf input or authored leaf-size bound"));
    }
    // Check all requested indices before executing the supplied leaf.
    for &index in order {
        let t = triangles
            .get(index as usize)
            .ok_or(Error("leaf triangle outside table"))?;
        if t.iter().any(|&i| i as usize >= vertices.len()) {
            return Err(Error("leaf vertex outside table"));
        }
    }
    let mut fraction = 1.;
    let mut delta = direction;
    let mut result = None;
    for &index in order {
        let t = triangles[index as usize];
        let points = t.map(|i| vertices[i as usize]);
        if let Some(hit) = triangle::query(
            start[..3].try_into().unwrap(),
            delta[..3].try_into().unwrap(),
            points,
        ) {
            fraction *= hit.fraction;
            delta = direction.map(|v| fraction * v);
            let zero_fraction = fraction == 0.;
            result = Some(Hit {
                triangle: index as usize,
                fraction,
                displacement: delta,
                zero_fraction,
            });
            if zero_fraction {
                break;
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn later_equal_hit_replaces_and_shortens_again() {
        let vertices = [[0., 0., 0.], [1., 0., 0.], [0., 0., 1.]];
        let triangles = [[0, 1, 2], [0, 1, 2]];
        let hit = resolve(
            [0.25, 1., 0.25, 1.],
            [0., -2., 0., 0.],
            &vertices,
            &triangles,
            &[0, 1],
        )
        .unwrap()
        .unwrap();
        assert_eq!(hit.triangle, 1);
        assert_eq!(hit.fraction, 0.5);
        assert_eq!(hit.displacement, [0., -1., 0., 0.]);
        let zero = resolve(
            [0.25, 0., 0.25, 1.],
            [0., -2., 0., 0.],
            &vertices,
            &triangles,
            &[0, 1],
        )
        .unwrap()
        .unwrap();
        assert_eq!(zero.triangle, 0);
        assert!(zero.zero_fraction);
    }
    #[test]
    fn missed_and_invalid_leaves() {
        let vertices = [[0., 0., 0.], [1., 0., 0.], [0., 0., 1.]];
        let triangles = [[0, 1, 2]];
        assert!(
            resolve(
                [2., 1., 2., 1.],
                [0., -2., 0., 0.],
                &vertices,
                &triangles,
                &[0]
            )
            .unwrap()
            .is_none()
        );
        assert!(resolve([0.; 4], [0.; 4], &vertices, &triangles, &[1]).is_err());
        assert!(
            resolve([0.; 4], [0.; 4], &vertices, &triangles, &[])
                .unwrap()
                .is_none()
        );
    }
}
