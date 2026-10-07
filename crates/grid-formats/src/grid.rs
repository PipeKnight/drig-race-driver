//! Grid marker rectangles. Full vehicle placement/contact remains separate.
use crate::{Result, fail, pssg::Tree, scene};
#[derive(Debug)]
pub struct Marker {
    pub raw_transform: [f32; 16],
    pub corners: [[f32; 3]; 3],
    pub center: [f32; 3],
    /// Observed original slot consumer uses corner A minus corner C.
    pub forward: [f32; 3],
}
impl Marker {
    /// Mode-one front alignment with an explicitly chosen lateral displacement.
    /// front_extent includes the car's additive local Z offset. Random selection
    /// of lateral displacement and deformation-adjusted extents remain separate.
    pub fn front_aligned_center(&self, front_extent: f32, lateral: f32) -> Result<[f32; 3]> {
        if !front_extent.is_finite() || !lateral.is_finite() {
            return fail(0, "nonfinite grid alignment input");
        }
        let longitudinal: [f32; 3] =
            std::array::from_fn(|i| self.corners[0][i] - self.corners[2][i]);
        let transverse: [f32; 3] = std::array::from_fn(|i| self.corners[1][i] - self.corners[0][i]);
        let length = longitudinal.iter().map(|v| v * v).sum::<f32>().sqrt();
        let width = transverse.iter().map(|v| v * v).sum::<f32>().sqrt();
        if !length.is_finite()
            || !width.is_finite()
            || length <= f32::EPSILON
            || width <= f32::EPSILON
        {
            return fail(0, "degenerate grid alignment rectangle");
        }
        let displacement = length * 0.5 - front_extent;
        let center = std::array::from_fn(|i| {
            self.center[i]
                + longitudinal[i] / length * displacement
                + transverse[i] / width * lateral
        });
        if center.iter().any(|v| !v.is_finite()) {
            return fail(0, "grid alignment result exceeds finite f32 range");
        }
        Ok(center)
    }
    /// Diagnostic horizontal frame; preserves marker origin, not final spawn height.
    pub fn horizontal_preview(&self) -> Result<[f32; 16]> {
        let [x, _, z] = self.forward;
        let length = x.hypot(z);
        if length <= f32::EPSILON {
            return fail(0, "grid heading has no horizontal direction");
        }
        let x = x / length;
        let z = z / length;
        Ok([
            z,
            0.,
            -x,
            0.,
            0.,
            1.,
            0.,
            0.,
            x,
            0.,
            z,
            0.,
            self.raw_transform[12],
            self.raw_transform[13],
            self.raw_transform[14],
            1.,
        ])
    }
}
pub fn from_bounds(transform: [f32; 16], bounds: [f32; 6]) -> Result<Marker> {
    if transform
        .iter()
        .chain(bounds.iter())
        .any(|v| !v.is_finite())
        || bounds[0] >= bounds[3]
        || bounds[1] > bounds[4]
        || bounds[2] >= bounds[5]
    {
        return fail(0, "invalid grid marker bounds or transform");
    }
    let y = bounds[1] + (bounds[4] - bounds[1]) * 0.5;
    let corners = [
        scene::transform_point(&transform, [bounds[0], y, bounds[2]]),
        scene::transform_point(&transform, [bounds[3], y, bounds[2]]),
        scene::transform_point(&transform, [bounds[0], y, bounds[5]]),
    ];
    let direction: [f32; 3] = std::array::from_fn(|i| corners[0][i] - corners[2][i]);
    let length = direction.iter().map(|v| v * v).sum::<f32>().sqrt();
    let width: f32 = (0..3)
        .map(|i| (corners[1][i] - corners[0][i]).powi(2))
        .sum();
    if !length.is_finite() || length <= f32::EPSILON || !width.is_finite() || width <= f32::EPSILON
    {
        return fail(0, "degenerate world grid marker");
    }
    let center = std::array::from_fn(|i| {
        corners[2][i] + 0.5 * ((corners[0][i] - corners[2][i]) + (corners[1][i] - corners[0][i]))
    });
    Ok(Marker {
        raw_transform: transform,
        corners,
        center,
        forward: direction.map(|v| v / length),
    })
}
pub fn read(tree: &Tree<'_>, name: &[u8]) -> Result<Marker> {
    let nodes: Vec<_> = (0..tree.nodes.len())
        .filter(|&i| tree.name(i) == b"NODE" && tree.string(i, b"id").is_ok_and(|id| id == name))
        .collect();
    if nodes.len() != 1 {
        return fail(0, "expected unique grid marker");
    }
    let selected = nodes[0];
    let boxes: Vec<_> = tree.nodes[selected]
        .children
        .iter()
        .copied()
        .filter(|&i| tree.name(i) == b"BOUNDINGBOX")
        .collect();
    if boxes.len() != 1 {
        return fail(selected, "expected one grid marker bounding box");
    }
    let bytes = tree.nodes[boxes[0]].data.ok_or(crate::Error {
        offset: boxes[0],
        message: "grid marker bounds absent",
    })?;
    if bytes.len() != 24 {
        return fail(boxes[0], "grid marker bounds must be six floats");
    }
    let bounds =
        std::array::from_fn(|i| f32::from_be_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap()));
    from_bounds(scene::node_matrix(tree, selected)?, bounds)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rectangle_heading_is_negative_marker_z_with_proper_preview_basis() {
        let m = [
            0., 0., -1., 0., 0., 1., 0., 0., 1., 0., 0., 0., 10., 2., 20., 1.,
        ];
        let marker = from_bounds(m, [-0.5, 0., -1., 0.5, 0., 1.]).unwrap();
        assert_eq!(
            marker.corners,
            [[9., 2., 20.5], [9., 2., 19.5], [11., 2., 20.5]]
        );
        assert_eq!(marker.forward, [-1., 0., 0.]);
        assert_eq!(marker.center, [10., 2., 20.]);
        assert_eq!(marker.front_aligned_center(2., 0.).unwrap(), [11., 2., 20.]);
        assert_eq!(
            marker.front_aligned_center(2., 0.25).unwrap(),
            [11., 2., 19.75]
        );
        let frame = marker.horizontal_preview().unwrap();
        assert_eq!(scene::transform_point(&frame, [0., 0., 1.]), [9., 2., 20.]);
        assert_eq!(scene::transform_point(&frame, [1., 0., 0.]), [10., 2., 21.]);
    }
    #[test]
    fn rejects_degenerate_and_nonfinite_markers() {
        let m = [
            1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
        ];
        assert!(from_bounds(m, [0.; 6]).is_err());
        assert!(from_bounds(m, [0., 0., f32::NAN, 1., 1., 1.]).is_err());
        let mut collapsed = m;
        collapsed[10] = 0.;
        assert!(from_bounds(collapsed, [-1., 0., -1., 1., 0., 1.]).is_err());
    }
}
