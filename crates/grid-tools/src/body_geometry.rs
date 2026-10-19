//! Selected source moving mesh to cold Donington body contacts.
//! Private sampling preparation and complete original shape/material factories
//! remain external. No proprietary values are embedded in this adapter.
use crate::{contact_surfaces::Surfaces, track::Track};
use grid_formats::{binxml, car_collision};
use grid_world::{Error, collision_mesh, contact_groups, convex_distance, physical_pose};

/// Contacts per shape pair (props, boundary sheet, each road chunk): the
/// port's packed generator bound (the original allows 20, F318 A32).
const PAIR_CONTACTS: usize = 16;

/// The original's collision floor lift (0x9E0240 at 0x9E0674..0x9E072A [V]):
/// the operation order and single-precision rounding of the original.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FloorLift {
    bottom: f32,
    height: f32,
    inverse: f32,
    raised: f32,
}

impl FloorLift {
    /// `bottom`/`roof`: body +0xED68/+0xED94; `clearance`: +0xED60;
    /// `track`: the global set from `additional_ground_clearance`.
    pub fn new(bottom: f32, roof: f32, clearance: f32, track: f32) -> Result<Self, Error> {
        let height = roof - bottom;
        let inverse = 1. / height;
        // 0xD1728C = 0.176, 0xCB0D94 = 9999; the original's compares keep a
        // NaN, as clamp does.
        let g = ((track + f32::from_bits(0x3e34_3958)) - (clearance + bottom)).clamp(0., 9999.);
        let raised = (g + clearance) * inverse;
        if [bottom, roof, clearance, track, inverse, raised]
            .iter()
            .any(|v| !v.is_finite())
            || height <= 0.
        {
            return Err(Error("invalid collision floor lift"));
        }
        Ok(Self {
            bottom,
            height,
            inverse,
            raised,
        })
    }

    pub fn apply(&self, y: f32) -> f32 {
        ((((y - self.bottom) * self.inverse) * (1. - self.raised)) + self.raised) * self.height
            + self.bottom
    }
}

#[derive(Clone)]
pub struct BodyGeometry {
    shape: collision_mesh::MeshShape,
    local: physical_pose::Matrix,
    query: convex_distance::ContactSettings,
    source_points: Vec<[f32; 4]>,
    source_triangles: Vec<[u32; 3]>,
    source_offset: [f32; 4],
    vertical_extents: [f32; 2],
}

impl BodyGeometry {
    /// Atomically rebuild this actual contact geometry, update owned wheels and
    /// publish the reached damage view. This selected independent shape owner
    /// does not emulate the original full car allocator/graphics factory.
    pub fn publish_deformation(
        &mut self,
        state: &grid_world::vehicle_publication::State,
        delta: f32,
    ) -> Result<grid_world::vehicle_publication::State, Error> {
        let (rebuilt, next) = self.deformation_publication(state, delta)?;
        if let Some(geometry) = rebuilt {
            *self = geometry;
        }
        Ok(next)
    }

    /// [`Self::publish_deformation`] without changing this geometry: the
    /// rebuilt geometry, if the publication rebuilds it, and the next state.
    pub fn deformation_publication(
        &self,
        state: &grid_world::vehicle_publication::State,
        delta: f32,
    ) -> Result<(Option<Self>, grid_world::vehicle_publication::State), Error> {
        use grid_world::{
            deformation_grid::Grid, deformation_publication, vehicle_publication::VEHICLE_WORDS,
        };
        let b = &state.vehicle;
        if b.len() != VEHICLE_WORDS || ((b[0xc7f0 / 4] >> 16) & 255) > 1 {
            return Err(Error("invalid deformation geometry publication framing"));
        }
        if (b[0xc7f0 / 4] >> 16) & 255 == 0 {
            return Ok((None, deformation_publication::calculate(state, delta)?));
        }
        let dimensions: [usize; 3] = std::array::from_fn(|i| b[0xc714 / 4 + i] as usize);
        if dimensions.iter().any(|d| !(2..=16).contains(d))
            || dimensions.iter().product::<usize>() > 400
        {
            return Err(Error("invalid deformation geometry publication grid"));
        }
        let positions: Vec<_> = (0..dimensions.iter().product::<usize>())
            .map(|i| std::array::from_fn(|j| f32::from_bits(b[(0x4880 + i * 80) / 4 + j])))
            .collect();
        let grid = Grid {
            dimensions,
            spacing: f32::from_bits(b[0xc720 / 4]),
            minimum: std::array::from_fn(|j| f32::from_bits(b[0xc730 / 4 + j])),
            positions: &positions,
        };
        let mut geometry = self.clone();
        geometry.rebuild_from_deformation(&grid)?;
        let next = deformation_publication::after_geometry_rebuild(state, delta)?;
        Ok((Some(geometry), next))
    }
    /// Source NED, verified owner layout offset, and locally prepared private
    /// CD01 sampling data. Raw source Y bounds define this selected mesh center.
    /// The raw `collision_main` hull, without the original's floor lift
    /// (diagnostics and the raw-mesh comparisons); driving uses
    /// [`Self::read_on_track`].
    pub fn read(
        ned: &[u8],
        offset: [f32; 4],
        table: &[u8],
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Self::read_lifted(ned, offset, table, None)
    }

    /// [`Self::read`] with the original's collision floor lift (0x9E0240, F406):
    /// every hull point's height is remapped from [bottom, roof] to
    /// [bottom + clearance + g, roof], where clearance is the car's NED
    /// `m_groundClearence` and g = clamp(track + 0.176 − (clearance + bottom),
    /// 0, 9999) with `track` the route's `additional_ground_clearance`
    /// (database `track`, set through 0x997280 from 0x60BEF0). Bottom and roof
    /// are the `collision_main` extremes (body +0xED68/+0xED94).
    pub fn read_on_track(
        ned: &[u8],
        offset: [f32; 4],
        table: &[u8],
        track_clearance: f32,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Self::read_lifted(ned, offset, table, Some(track_clearance))
    }

    fn read_lifted(
        ned: &[u8],
        offset: [f32; 4],
        table: &[u8],
        track_clearance: Option<f32>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        if ned.len() > 16 * 1024 * 1024 || table.len() != 712 || &table[..8] != b"CD01\x2c\0\0\0" {
            return Err("invalid source body geometry framing".into());
        }
        let directions: [[f32; 4]; 44] = std::array::from_fn(|i| {
            std::array::from_fn(|j| {
                f32::from_le_bytes(
                    table[8 + i * 16 + j * 4..12 + i * 16 + j * 4]
                        .try_into()
                        .unwrap(),
                )
            })
        });
        if directions.iter().any(|d| {
            let squared = (d[0] * d[0] + d[1] * d[1]) + (d[2] * d[2] + d[3] * d[3]);
            d[3] != 0. || d.iter().any(|x| !x.is_finite()) || !(0.99..=1.01).contains(&squared)
        }) {
            return Err("invalid source body sampling directions".into());
        }
        let doc = binxml::read(ned)?;
        let source = car_collision::read(&doc)?;
        let lift = match track_clearance {
            Some(track) => Some(FloorLift::new(
                source.bounds.min[1],
                source.bounds.max[1],
                car_collision::ground_clearance(&doc)?,
                track,
            )?),
            None => None,
        };
        let points: Vec<_> = source
            .points
            .iter()
            .map(|p| [p[0], lift.map_or(p[1], |l| l.apply(p[1])), p[2], 1.])
            .collect();
        let mesh = collision_mesh::build_from_source(
            &points,
            &source.triangles,
            offset,
            [source.bounds.min[1], source.bounds.max[1]],
            0,
        )?;
        let span = mesh.bytes().len() as u32;
        let shape = mesh.initialize_shape(
            collision_mesh::ShapeSettings {
                span,
                scalar220: 0.,
                layer: 0,
                flag_c0: false,
            },
            0,
        )?;
        let center = offset[1] + (source.bounds.max[1] + source.bounds.min[1]) * 0.5;
        let local = [
            [1., 0., 0., 0.],
            [0., 1., 0., 0.],
            [0., 0., 1., 0.],
            [0., center, 0., 1.],
        ];
        Ok(Self {
            shape,
            local,
            query: convex_distance::ContactSettings {
                margin: 0.,
                directions,
            },
            source_points: points,
            source_triangles: source.triangles,
            source_offset: offset,
            vertical_extents: [source.bounds.min[1], source.bounds.max[1]],
        })
    }

    /// Stage active grid sampling and a complete new collision mesh/shape.
    /// Preserve the previous shape, source inputs and query state on failure.
    /// The caller still owns dirty publication/physical callback scheduling.
    pub fn rebuild_from_deformation(
        &mut self,
        grid: &grid_world::deformation_grid::Grid<'_>,
    ) -> Result<(), Error> {
        let mesh = collision_mesh::build_from_deformed_source(
            &self.source_points,
            &self.source_triangles,
            self.source_offset,
            self.vertical_extents,
            grid,
            0,
        )?;
        let span = mesh.bytes().len() as u32;
        let shape = mesh.initialize_shape(
            collision_mesh::ShapeSettings {
                span,
                scalar220: 0.,
                layer: 0,
                flag_c0: false,
            },
            0,
        )?;
        self.shape = shape;
        Ok(())
    }

    pub fn pose(&self, body: &contact_groups::Body) -> Result<physical_pose::Matrix, Error> {
        let matrix = |o: usize| {
            std::array::from_fn(|i| {
                std::array::from_fn(|j| f32::from_bits(body[o / 4 + i * 4 + j]))
            })
        };
        Ok(physical_pose::calculate(&physical_pose::Input {
            basis: matrix(0x190),
            offset: std::array::from_fn(|j| f32::from_bits(body[0x230 / 4 + j])),
            local_matrices: [matrix(0x90), matrix(0x150)],
            shapes: vec![Some(self.local)],
            group_head: false,
            linear: [0.; 4],
            angular: [0.; 4],
        })?
        .shapes[0])
    }

    /// Diagnostic: current collision hull points in world coordinates.
    pub fn hull_world_points(&self, body: &contact_groups::Body) -> Result<Vec<[f32; 3]>, Error> {
        let m = self.pose(body)?;
        let mesh = self.shape.mesh();
        Ok((0..mesh.point_count())
            .filter_map(|i| mesh.point(i))
            .map(|p| {
                std::array::from_fn(|j| p[0] * m[0][j] + p[1] * m[1][j] + p[2] * m[2][j] + m[3][j])
            })
            .collect())
    }

    /// Current collision hull triangles in world coordinates (source triangle
    /// order over the mesh points), for another body's car-to-car queries.
    pub fn hull_world_triangles(
        &self,
        body: &contact_groups::Body,
    ) -> Result<Vec<[[f32; 3]; 3]>, Error> {
        let points = self.hull_world_points(body)?;
        if points.len() != self.source_points.len() {
            return Err(Error("collision hull point order differs from source"));
        }
        self.source_triangles
            .iter()
            .map(|t| {
                let mut out = [[0.; 3]; 3];
                for (slot, &index) in out.iter_mut().zip(t) {
                    *slot = *points
                        .get(index as usize)
                        .ok_or(Error("collision hull triangle index"))?;
                }
                Ok(out)
            })
            .collect()
    }

    /// Cold car-to-car contacts: this body's hull against another body's world
    /// hull triangles, through the shared decoded cold contact path.
    pub fn car_contacts(
        &self,
        body: &contact_groups::Body,
        other: &[[[f32; 3]; 3]],
        capacity: usize,
    ) -> Result<Vec<grid_world::contact_manifold::Contact>, Error> {
        self.car_contacts_hull(body, &WorldHull::new(other), capacity)
    }

    /// [`Self::car_contacts`] against a hull prepared once for several queries.
    pub fn car_contacts_hull(
        &self,
        body: &contact_groups::Body,
        other: &WorldHull,
        capacity: usize,
    ) -> Result<Vec<grid_world::contact_manifold::Contact>, Error> {
        if other.count == 0 || other.count > 4096 || capacity == 0 {
            return Ok(Vec::new());
        }
        let matrix = self.pose(body)?;
        let bounds = other.bounds;
        let car = self.shape.mesh().world_bounds(matrix, self.query.margin)?;
        if (0..3).any(|j| car[0][j] > bounds[j + 3] || car[1][j] < bounds[j]) {
            return Ok(Vec::new());
        }
        grid_world::packed_contacts::calculate_decoded_known(
            self.shape.mesh(),
            matrix,
            &other.mesh(),
            &other.materials,
            &grid_world::packed_contacts::Settings {
                query: convex_distance::ContactSettings {
                    margin: self.query.margin,
                    directions: self.query.directions,
                },
                shape_margin: 0.,
                contact_capacity: capacity.min(16),
                cache_enabled: true,
                candidate_capacity: 4096,
                sheet_mask: 0,
                correction_bounds: [f32::MAX; 4],
            },
            &grid_world::packed_contacts::Known {
                checked: other.checked,
                directions: grid_world::packed_contacts::directions_valid(&self.query),
                bounds: car,
            },
        )
    }

    /// The car hull against a round piece (kind 5 or 3) at its world pose:
    /// AE8AD0 with the piece as the static side (`round_contacts`).
    pub fn car_contacts_round(
        &self,
        body: &contact_groups::Body,
        shape: &grid_world::round_contacts::RoundShape,
        capacity: usize,
    ) -> Result<Vec<grid_world::contact_manifold::Contact>, Error> {
        if capacity == 0 {
            return Ok(Vec::new());
        }
        let matrix = self.pose(body)?;
        let car = self.shape.mesh().world_bounds(matrix, self.query.margin)?;
        let (pose, hull, margin) = match shape {
            grid_world::round_contacts::RoundShape::Cylinder(c) => (
                c.matrix,
                convex_distance::Hull::Cylinder {
                    radius: c.radius,
                    half_height: c.half_height,
                },
                c.margin,
            ),
            grid_world::round_contacts::RoundShape::Sphere(s) => (
                s.matrix,
                convex_distance::Hull::Sphere { radius: s.radius },
                s.margin,
            ),
        };
        let other = hull.world_bounds(pose, margin)?;
        if (0..3).any(|j| car[0][j] > other[1][j] || car[1][j] < other[0][j]) {
            return Ok(Vec::new());
        }
        grid_world::round_contacts::contacts(
            self.shape.mesh(),
            matrix,
            shape,
            &self.query,
            capacity.min(16),
        )
    }

    /// World points/normals retain the generator's publication orientation.
    /// Material IDs zero select the prototype's explicit shared solver material.
    pub fn contacts(
        &self,
        track: &Track,
        surfaces: &Surfaces,
        bodies: &[contact_groups::Body],
        remaining: usize,
    ) -> Result<Vec<contact_groups::Contact>, Error> {
        Ok(self
            .contacts_with_props(track, surfaces, bodies, remaining)?
            .into_iter()
            .map(|(c, _)| c)
            .collect())
    }

    /// As [`Self::contacts`], with the route prop box of each prop contact.
    /// A breakable prop's threshold is the contact impulse limit (+0x3C).
    pub fn contacts_with_props(
        &self,
        track: &Track,
        surfaces: &Surfaces,
        bodies: &[contact_groups::Body],
        remaining: usize,
    ) -> Result<Vec<(contact_groups::Contact, Option<usize>)>, Error> {
        if bodies.len() != 1 {
            return Err(Error("body geometry requires one owned vehicle"));
        }
        let fresh = track.body_contacts(
            self.shape.mesh(),
            self.pose(&bodies[0])?,
            &self.query,
            PAIR_CONTACTS,
            remaining,
            |code| surfaces.sheet(code),
        )?;
        Ok(fresh
            .into_iter()
            .map(|hit| {
                let mut c = [0; 32];
                c[1] = 1;
                c[0xc / 4] = 0x01000000;
                c[0x1c / 4] = u32::MAX;
                c[0x20 / 4] = u32::MAX;
                c[0x24 / 4] = 1;
                c[0x28 / 4] = 1;
                c[0x2c / 4] = hit.shape + 1;
                c[0x38 / 4] = hit.contact.penetration.to_bits();
                c[0x3c / 4] = hit.limit.to_bits();
                c[0x50 / 4..0x60 / 4].copy_from_slice(&hit.contact.point.map(f32::to_bits));
                c[0x60 / 4..0x70 / 4].copy_from_slice(&hit.contact.normal.map(f32::to_bits));
                c[0x70 / 4] = (u32::from(hit.contact.flipped) << 8) | (1 << 24);
                (c, hit.prop)
            })
            .collect())
    }

    /// Whether `result`, returned by [`Self::contacts_with_props`] for some
    /// `remaining`, is also what it returns for the smaller `room`.
    ///
    /// The capacity reaches the query only as each pair query's room,
    /// `PAIR_CONTACTS.min(remaining - found)` with `found` at most the final
    /// count, and as a stop once `found == remaining`. When `room` leaves at
    /// least `PAIR_CONTACTS` free after every contact found, every pair query
    /// gets `PAIR_CONTACTS` under both capacities and neither stops early, so
    /// the two queries take identical steps. Lets the multi-car tick query
    /// every car at once and keep the order-dependent capacity in its
    /// sequential pass.
    pub fn reusable_with<T>(result: &Result<Vec<T>, Error>, room: usize) -> bool {
        matches!(result, Ok(found) if found.len() + PAIR_CONTACTS <= room)
    }

    /// Contact query settings (margin and sampling directions).
    /// The current collision hull in its shape frame (see [`Self::pose`]).
    pub fn mesh(&self) -> &collision_mesh::Mesh {
        self.shape.mesh()
    }

    pub fn query(&self) -> &convex_distance::ContactSettings {
        &self.query
    }
}

/// Another body's world hull triangles as one packed leaf, prepared once for
/// several car-to-car queries.
pub struct WorldHull {
    count: usize,
    bounds: [f32; 6],
    vertices: Vec<[f32; 3]>,
    triangles: Vec<[u16; 3]>,
    leaves: std::collections::BTreeMap<usize, Vec<u16>>,
    materials: Vec<u8>,
    checked: grid_world::packed_contacts::Checked,
}

impl WorldHull {
    pub fn new(other: &[[[f32; 3]; 3]]) -> Self {
        let mut bounds = [f32::MAX, f32::MAX, f32::MAX, f32::MIN, f32::MIN, f32::MIN];
        let mut vertices = Vec::with_capacity(other.len() * 3);
        for t in other {
            for p in t {
                for j in 0..3 {
                    bounds[j] = bounds[j].min(p[j]);
                    bounds[j + 3] = bounds[j + 3].max(p[j]);
                }
                vertices.push(*p);
            }
        }
        let count = other.len();
        let mut hull = Self {
            count,
            bounds,
            vertices,
            triangles: Vec::new(),
            leaves: Default::default(),
            materials: Vec::new(),
            checked: Default::default(),
        };
        // Queries use only hulls of 1..=4096 triangles.
        if (1..=4096).contains(&count) {
            hull.triangles = (0..count)
                .map(|i| [i as u16 * 3, i as u16 * 3 + 1, i as u16 * 3 + 2])
                .collect();
            hull.leaves = std::collections::BTreeMap::from([(0, (0..count as u16).collect())]);
            hull.materials = vec![0; count];
            hull.checked = grid_world::packed_contacts::check(&hull.mesh(), &hull.materials);
        }
        hull
    }

    fn mesh(&self) -> grid_world::packed_query::Mesh<'_> {
        grid_world::packed_query::Mesh {
            bounds: self.bounds,
            vertices: &self.vertices,
            triangles: &self.triangles,
            nodes: &[0x8000],
            leaves: &self.leaves,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floor_lift_raises_the_bottom_and_keeps_the_roof() {
        // Lacetti collision_main: bottom 0.103433, roof 1.420987, clearance
        // 0.07, Donington (track 0): the floor goes to 0.176.
        let lift = FloorLift::new(0.103433, 1.420987, 0.07, 0.).unwrap();
        assert!((lift.apply(0.103433) - 0.176).abs() < 1e-6);
        assert!((lift.apply(1.420987) - 1.420987).abs() < 1e-6);
        // A low prototype floor is raised more; a city track adds 0.05.
        let lmp = FloorLift::new(0.047, 1.44, 0.07, 0.).unwrap();
        assert!((lmp.apply(0.047) - 0.176).abs() < 1e-6);
        let city = FloorLift::new(0.103433, 1.420987, 0.07, 0.05).unwrap();
        assert!((city.apply(0.103433) - 0.226).abs() < 1e-6);
        // A floor already above track + 0.176 keeps bottom + clearance.
        let high = FloorLift::new(0.2, 1.4, 0.07, 0.).unwrap();
        assert!((high.apply(0.2) - 0.27).abs() < 1e-6);
        // Heights stay ordered.
        assert!(lift.apply(0.5) < lift.apply(0.6));
        assert!(FloorLift::new(1., 1., 0.07, 0.).is_err());
    }

    #[test]
    fn malformed_sampling_rejected_before_resource_decoding() {
        assert!(BodyGeometry::read(&[], [0.; 4], &[]).is_err());
        let mut table = vec![0; 712];
        table[..8].copy_from_slice(b"CD01\x2c\0\0\0");
        assert_eq!(
            BodyGeometry::read(&[], [0.; 4], &table)
                .err()
                .unwrap()
                .to_string(),
            "invalid source body sampling directions"
        );
        table[8..12].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(BodyGeometry::read(&[], [0.; 4], &table).is_err());
        table.push(0);
        assert!(BodyGeometry::read(&[], [0.; 4], &table).is_err());
    }
}
