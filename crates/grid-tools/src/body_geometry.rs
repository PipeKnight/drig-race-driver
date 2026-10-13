//! Selected source moving mesh to cold Donington body contacts.
//! Private sampling preparation and complete original shape/material factories
//! remain external. No proprietary values are embedded in this adapter.
use crate::{contact_surfaces::Surfaces, track::Track};
use grid_formats::{binxml, car_collision};
use grid_world::{Error, collision_mesh, contact_groups, convex_distance, physical_pose};

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
    /// Source NED, verified owner layout offset, and locally prepared private
    /// CD01 sampling data. Raw source Y bounds define this selected mesh center.
    pub fn read(
        ned: &[u8],
        offset: [f32; 4],
        table: &[u8],
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
        let points: Vec<_> = source
            .points
            .iter()
            .map(|p| [p[0], p[1], p[2], 1.])
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

    /// World points/normals retain the generator's publication orientation.
    /// Material IDs zero select the prototype's explicit shared solver material.
    pub fn contacts(
        &self,
        track: &Track,
        surfaces: &Surfaces,
        bodies: &[contact_groups::Body],
        remaining: usize,
    ) -> Result<Vec<contact_groups::Contact>, Error> {
        if bodies.len() != 1 {
            return Err(Error("body geometry requires one owned vehicle"));
        }
        let fresh = track.body_contacts(
            self.shape.mesh(),
            self.pose(&bodies[0])?,
            &self.query,
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
                c[0x3c / 4] = f32::MAX.to_bits();
                c[0x50 / 4..0x60 / 4].copy_from_slice(&hit.contact.point.map(f32::to_bits));
                c[0x60 / 4..0x70 / 4].copy_from_slice(&hit.contact.normal.map(f32::to_bits));
                c[0x70 / 4] = (u32::from(hit.contact.flipped) << 8) | (1 << 24);
                c
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
