//! Independent geometry queries and scoped, reference-verified behavior blocks.
//! The f64 static BVH and packed reference-style queries expose distinct semantics.
use std::{fmt, ops::Range};

pub mod assisted_steering;
pub mod candidate_grid;
pub mod collision_callback;
pub mod collision_dispatch;
pub mod contact_bias;
pub mod contact_collection;
pub mod contact_groups;
pub mod contact_inverse;
pub mod contact_material;
pub mod contact_preparation;
pub mod contact_ray;
pub mod contact_reduction;
pub mod contact_rows;
pub mod deformation_grid;
pub mod deformation_impact;
pub mod deformation_settle;
pub mod dynamics;
pub mod finalize;
pub mod force_equations;
pub mod force_view;
pub mod grid_population;
pub mod grip_history;
pub mod grouped_physical_frame;
pub mod impact_body;
pub mod impact_correction;
pub mod impact_frame;
pub mod impact_generation;
pub mod impact_producer;
pub mod impact_response;
pub mod impact_step;
pub mod leaf_query;
pub mod main_contact_response;
pub mod owner_collection;
pub mod packed_query;
pub mod packed_shape;
pub mod packed_world;
pub mod physical_constructor;
pub mod physical_pose;
pub mod physical_response;
pub mod physical_tick;
pub mod physical_world;
pub mod pose;
pub mod rectangle_cull;
pub mod response_consumption;
pub mod serial_callback;
pub mod setup;
pub mod spawn;
pub mod steering_coefficients;
pub mod steering_control;
pub mod steering_feedback;
pub mod steering_geometry;
pub mod steering_input;
pub mod surface_drag;
pub mod suspension;
pub mod suspension_frame;
pub mod suspension_prepare;
pub mod suspension_vehicle;
pub mod triangle;
pub mod tyre_friction;
pub mod tyre_grip;
pub mod tyre_response;
pub mod vehicle_bridge;
pub mod vehicle_chassis;
pub mod vehicle_contact_force;
pub mod vehicle_contact_update;
pub mod vehicle_controls;
pub mod vehicle_driver_prepare;
pub mod vehicle_force;
pub mod vehicle_gearing;
pub mod vehicle_grip;
pub mod vehicle_handling;
pub mod vehicle_job;
pub mod vehicle_manager;
pub mod vehicle_outer;
pub mod vehicle_post_force;
pub mod vehicle_prepared_frame;
pub mod vehicle_propulsion;
pub mod vehicle_publication;
pub mod vehicle_step;
pub mod vehicle_sync;
pub mod vehicle_world;
pub mod wheel_contacts;
pub mod wheel_coupling;
pub mod wheel_damage;
pub mod wheel_deformation;
pub mod wheel_frame;
pub mod wheel_history;
pub mod wheel_radius;
pub mod wheel_rotation;
pub mod wheel_step;

type Vector = [f64; 3];
const EPSILON: f64 = 1e-12;
const EDGE_EPSILON: f64 = 1e-10;
const MAX_TRIANGLES: usize = 1_000_000;

#[derive(Clone, Debug)]
pub struct Triangle {
    pub vertices: [[f32; 3]; 3],
    pub material: [u8; 4],
    pub flags: u8,
}

#[derive(Clone, Copy, Debug)]
pub struct Segment {
    pub start: Vector,
    pub end: Vector,
}

#[derive(Clone, Debug)]
pub struct Hit {
    /// Index into the supplied triangle array, including skipped degenerate entries.
    pub triangle: usize,
    /// Zero at segment start, one at segment end.
    pub fraction: f64,
    pub position: Vector,
    /// Unit geometric normal in source winding, not forced to face the query.
    pub normal: Vector,
    pub barycentric: Vector,
    pub material: [u8; 4],
    pub flags: u8,
}

#[derive(Debug)]
pub struct Query {
    pub hit: Option<Hit>,
    pub tested_triangles: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(pub &'static str);
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for Error {}

#[derive(Clone, Copy, Debug)]
struct Bounds {
    min: Vector,
    max: Vector,
}
impl Bounds {
    fn empty() -> Self {
        Self {
            min: [f64::INFINITY; 3],
            max: [f64::NEG_INFINITY; 3],
        }
    }
    fn extend(&mut self, point: Vector) {
        for (axis, value) in point.into_iter().enumerate() {
            self.min[axis] = self.min[axis].min(value);
            self.max[axis] = self.max[axis].max(value);
        }
    }
    fn entry(self, segment: Segment, direction: Vector, limit: f64) -> Option<f64> {
        let mut near: f64 = 0.;
        let mut far = limit.min(1.);
        for (axis, delta) in direction.into_iter().enumerate() {
            if delta == 0. {
                if segment.start[axis] < self.min[axis] || segment.start[axis] > self.max[axis] {
                    return None;
                }
            } else {
                let a = (self.min[axis] - segment.start[axis]) / delta;
                let b = (self.max[axis] - segment.start[axis]) / delta;
                near = near.max(a.min(b));
                far = far.min(a.max(b));
                if near > far + EPSILON {
                    return None;
                }
            }
        }
        Some(near)
    }
}

#[derive(Debug)]
enum Branch {
    Leaf(Range<usize>),
    Children([usize; 2]),
}
#[derive(Debug)]
struct Node {
    bounds: Bounds,
    branch: Branch,
}

#[derive(Debug)]
pub struct Mesh {
    triangles: Vec<Triangle>,
    order: Vec<usize>,
    nodes: Vec<Node>,
    pub skipped_degenerate: usize,
}

fn subtract(a: Vector, b: Vector) -> Vector {
    std::array::from_fn(|i| a[i] - b[i])
}
fn dot(a: Vector, b: Vector) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: Vector, b: Vector) -> Vector {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn points(t: &Triangle) -> [Vector; 3] {
    t.vertices.map(|p| p.map(f64::from))
}
fn triangle_normal(t: &Triangle) -> Vector {
    let [a, b, c] = points(t);
    cross(subtract(b, a), subtract(c, a))
}

impl Mesh {
    pub fn new(triangles: Vec<Triangle>) -> Result<Self, Error> {
        if triangles.len() > MAX_TRIANGLES {
            return Err(Error("triangle limit exceeded"));
        }
        let mut order = Vec::with_capacity(triangles.len());
        for (i, t) in triangles.iter().enumerate() {
            if t.vertices.iter().flatten().any(|v| !v.is_finite()) {
                return Err(Error("nonfinite triangle vertex"));
            }
            let normal = triangle_normal(t);
            if dot(normal, normal) > 0. {
                order.push(i);
            }
        }
        let skipped_degenerate = triangles.len() - order.len();
        let mut result = Self {
            triangles,
            order,
            nodes: Vec::new(),
            skipped_degenerate,
        };
        if !result.order.is_empty() {
            result.build(0..result.order.len());
        }
        Ok(result)
    }

    pub fn triangle_count(&self) -> usize {
        self.triangles.len()
    }
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    fn build(&mut self, range: Range<usize>) -> usize {
        let mut bounds = Bounds::empty();
        for &id in &self.order[range.clone()] {
            for p in points(&self.triangles[id]) {
                bounds.extend(p);
            }
        }
        // Preserve conservative culling for the barycentric edge tolerance.
        for axis in 0..3 {
            let padding = (bounds.max[axis] - bounds.min[axis]) * (2. * EDGE_EPSILON)
                + bounds.max[axis].abs().max(bounds.min[axis].abs()) * (8. * f64::EPSILON);
            bounds.min[axis] -= padding;
            bounds.max[axis] += padding;
        }
        let index = self.nodes.len();
        self.nodes.push(Node {
            bounds,
            branch: Branch::Leaf(range.clone()),
        });
        if range.len() > 8 {
            let axis = (0..3)
                .max_by(|&a, &b| {
                    (bounds.max[a] - bounds.min[a]).total_cmp(&(bounds.max[b] - bounds.min[b]))
                })
                .unwrap();
            let triangles = &self.triangles;
            let center =
                |id: usize| points(&triangles[id]).iter().map(|p| p[axis]).sum::<f64>() / 3.;
            let mid = range.start + range.len() / 2;
            self.order[range.clone()].select_nth_unstable_by(range.len() / 2, |&a, &b| {
                center(a).total_cmp(&center(b)).then(a.cmp(&b))
            });
            let children = [self.build(range.start..mid), self.build(mid..range.end)];
            self.nodes[index].branch = Branch::Children(children);
        }
        index
    }

    /// Closest two-sided segment hit. Equal fractions within 1e-12 choose input order.
    /// Coplanar/zero-length segments yield no hit; nonfinite/out-of-f32-range endpoints fail.
    pub fn query(&self, segment: Segment) -> Result<Query, Error> {
        if segment
            .start
            .into_iter()
            .chain(segment.end)
            .any(|v| !v.is_finite() || v.abs() > f32::MAX as f64)
        {
            return Err(Error("invalid segment endpoint"));
        }
        let direction = subtract(segment.end, segment.start);
        let mut result = Query {
            hit: None,
            tested_triangles: 0,
        };
        if self.nodes.is_empty() || dot(direction, direction) == 0. {
            return Ok(result);
        }
        let mut stack = vec![0];
        while let Some(index) = stack.pop() {
            let limit = result.hit.as_ref().map_or(1., |h| h.fraction + EPSILON);
            let node = &self.nodes[index];
            if node.bounds.entry(segment, direction, limit).is_none() {
                continue;
            }
            match &node.branch {
                Branch::Leaf(range) => {
                    for &id in &self.order[range.clone()] {
                        result.tested_triangles += 1;
                        if let Some(hit) = self.intersect(id, segment, direction)
                            && result.hit.as_ref().is_none_or(|old| {
                                hit.fraction < old.fraction - EPSILON
                                    || ((hit.fraction - old.fraction).abs() <= EPSILON
                                        && hit.triangle < old.triangle)
                            })
                        {
                            result.hit = Some(hit);
                        }
                    }
                }
                Branch::Children(children) => {
                    let entries =
                        children.map(|i| self.nodes[i].bounds.entry(segment, direction, limit));
                    match entries {
                        [Some(a), Some(b)] => {
                            let [near, far] = if a <= b {
                                *children
                            } else {
                                [children[1], children[0]]
                            };
                            stack.push(far);
                            stack.push(near);
                        }
                        [Some(_), None] => stack.push(children[0]),
                        [None, Some(_)] => stack.push(children[1]),
                        [None, None] => {}
                    }
                }
            }
        }
        Ok(result)
    }

    fn intersect(&self, id: usize, segment: Segment, direction: Vector) -> Option<Hit> {
        let triangle = &self.triangles[id];
        let [a, b, c] = points(triangle);
        let ab = subtract(b, a);
        let ac = subtract(c, a);
        let normal = cross(ab, ac);
        let p = cross(direction, ac);
        let determinant = dot(ab, p);
        if determinant.abs() <= EPSILON * (dot(normal, normal) * dot(direction, direction)).sqrt() {
            return None;
        }
        let origin = subtract(segment.start, a);
        let u = dot(origin, p) / determinant;
        let q = cross(origin, ab);
        let v = dot(direction, q) / determinant;
        if u < -EDGE_EPSILON || v < -EDGE_EPSILON || u + v > 1. + EDGE_EPSILON {
            return None;
        }
        let fraction = dot(ac, q) / determinant;
        if !(-EPSILON..=1. + EPSILON).contains(&fraction) {
            return None;
        }
        let fraction = fraction.clamp(0., 1.);
        let length = dot(normal, normal).sqrt();
        Some(Hit {
            triangle: id,
            fraction,
            position: std::array::from_fn(|i| segment.start[i] + direction[i] * fraction),
            normal: normal.map(|v| v / length),
            barycentric: [1. - u - v, u, v],
            material: triangle.material,
            flags: triangle.flags,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn triangle(y: f32) -> Triangle {
        Triangle {
            vertices: [[-2., y, -2.], [2., y, -2.], [0., y, 2.]],
            material: *b"TEST",
            flags: 3,
        }
    }
    fn segment(y1: f64, y2: f64) -> Segment {
        Segment {
            start: [0., y1, 0.],
            end: [0., y2, 0.],
        }
    }
    fn near(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-10, "{a} != {b}");
    }
    #[test]
    fn closest_hit_reverses_with_segment_and_preserves_metadata() {
        let mesh = Mesh::new(vec![triangle(0.), triangle(2.)]).unwrap();
        let hit = mesh.query(segment(4., -4.)).unwrap().hit.unwrap();
        assert_eq!(hit.triangle, 1);
        near(hit.fraction, 0.25);
        assert_eq!(hit.position, [0., 2., 0.]);
        assert_eq!(hit.normal, [0., -1., 0.]);
        assert_eq!(hit.material, *b"TEST");
        assert_eq!(hit.flags, 3);
        near(hit.barycentric.iter().sum(), 1.);
        assert_eq!(
            mesh.query(segment(-4., 4.)).unwrap().hit.unwrap().triangle,
            0
        );
    }
    #[test]
    fn oblique_segment_on_sloped_triangle_matches_analytic_plane() {
        let mesh = Mesh::new(vec![Triangle {
            vertices: [[-2., -2., -2.], [2., 2., -2.], [0., 0., 2.]],
            material: *b"SLOP",
            flags: 0,
        }])
        .unwrap();
        // Plane y=x; segment (0,3,0) -> (1,-1,0) reaches it at t=3/5.
        let hit = mesh
            .query(Segment {
                start: [0., 3., 0.],
                end: [1., -1., 0.],
            })
            .unwrap()
            .hit
            .unwrap();
        near(hit.fraction, 0.6);
        near(hit.position[0], 0.6);
        near(hit.position[1], 0.6);
        near(dot(hit.normal, hit.normal), 1.);
    }
    #[test]
    fn boundaries_misses_invalid_and_degenerate_geometry() {
        let mut flat = triangle(0.);
        flat.vertices[2] = flat.vertices[1];
        let mesh = Mesh::new(vec![flat, triangle(0.)]).unwrap();
        assert_eq!(mesh.skipped_degenerate, 1);
        assert_eq!(
            mesh.query(segment(0., -2.)).unwrap().hit.unwrap().triangle,
            1
        );
        near(
            mesh.query(segment(2., 0.)).unwrap().hit.unwrap().fraction,
            1.,
        );
        for s in [
            segment(0., 0.),
            segment(2., 1.),
            Segment {
                start: [-1., 0., 0.],
                end: [1., 0., 0.],
            },
            Segment {
                start: [5., 2., 0.],
                end: [5., -2., 0.],
            },
        ] {
            assert!(mesh.query(s).unwrap().hit.is_none());
        }
        assert!(mesh.query(segment(f64::NAN, 0.)).is_err());
        let mut invalid = triangle(0.);
        invalid.vertices[0][0] = f32::INFINITY;
        assert!(Mesh::new(vec![invalid]).is_err());
        assert!(
            Mesh::new(vec![])
                .unwrap()
                .query(segment(2., -2.))
                .unwrap()
                .hit
                .is_none()
        );
    }
    #[test]
    fn hierarchy_selects_analytic_layer_and_stable_seam() {
        let mut triangles: Vec<_> = (0..128).map(|i| triangle(i as f32)).collect();
        triangles.push(triangle(127.));
        let mesh = Mesh::new(triangles).unwrap();
        for i in 0..128 {
            let hit = mesh
                .query(segment(i as f64 + 0.5, i as f64 - 0.5))
                .unwrap()
                .hit
                .unwrap();
            assert_eq!(hit.triangle, i);
            near(hit.fraction, 0.5);
        }
        let query = mesh.query(segment(128., 126.5)).unwrap();
        assert_eq!(query.hit.unwrap().triangle, 127);
        assert!(query.tested_triangles < mesh.triangle_count() / 2);
    }

    #[test]
    fn shared_edges_vertices_and_tolerated_boundary_are_not_culled() {
        let a = Triangle {
            vertices: [[0., 0., 0.], [1., 0., 0.], [0., 0., 1.]],
            material: *b"EDGE",
            flags: 0,
        };
        let b = Triangle {
            vertices: [[1., 0., 0.], [1., 0., 1.], [0., 0., 1.]],
            material: *b"EDGE",
            flags: 0,
        };
        let mesh = Mesh::new(vec![a, b]).unwrap();
        for (x, z) in [(0.5, 0.5), (0., 0.), (-1e-11, 0.5)] {
            let hit = mesh
                .query(Segment {
                    start: [x, 1., z],
                    end: [x, -1., z],
                })
                .unwrap()
                .hit
                .unwrap();
            assert_eq!(hit.triangle, 0);
            near(hit.fraction, 0.5);
        }
    }
}
