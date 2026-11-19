//! Independent CPU body-dent presentation; the original renderer is not emulated.
use bevy::{
    camera::primitives::Aabb,
    mesh::{Indices, PrimitiveTopology, VertexAttributeValues, triangle_normal},
    prelude::*,
};
use grid_tools::body_deformation::{Field, Probes};
use std::sync::Mutex;
pub fn project(source: &Mesh, raw_bind: Mat4, field: &Field) -> Result<Mesh, &'static str> {
    if !raw_bind.is_finite() || raw_bind.determinant().abs() < 1e-8 {
        return Err("invalid body damage bind transform");
    }
    let inverse = raw_bind.inverse();
    if !inverse.is_finite() {
        return Err("invalid body damage inverse transform");
    }
    let Some(VertexAttributeValues::Float32x3(original)) =
        source.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        return Err("missing body damage source positions");
    };
    if original.is_empty()
        || original.len() > 1_000_000
        || original.iter().flatten().any(|v| !v.is_finite())
    {
        return Err("invalid body damage source positions");
    }
    let queries: Vec<_> = original
        .iter()
        .map(|p| {
            let p = raw_bind.transform_point3(Vec3::from_array(*p));
            [p.x, p.y, p.z, 1.]
        })
        .collect();
    let deltas = field.displacements(&queries).map_err(|e| e.0)?;
    if deltas.iter().all(|d| *d == [0.; 3]) {
        return Ok(source.clone());
    }
    let positions: Vec<_> = original
        .iter()
        .zip(deltas)
        .map(|(p, d)| {
            (Vec3::from_array(*p) + inverse.transform_vector3(Vec3::from_array(d))).to_array()
        })
        .collect();
    if positions
        .iter()
        .flatten()
        .any(|v| !v.is_finite() || v.abs() > 1e6)
    {
        return Err("body damage positions exceed bounds");
    }
    let mut mesh = source.clone();
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.compute_smooth_normals();
    let Some(VertexAttributeValues::Float32x3(normals)) = mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
    else {
        return Err("missing body damage normals");
    };
    if normals.iter().flatten().any(|v| !v.is_finite()) {
        return Err("nonfinite body damage normals");
    }
    Ok(mesh)
}

/// Whether two projections of one source draw the same: positions and
/// normals equal (the rest of the mesh is the source's).
pub fn same_shape(a: &Mesh, b: &Mesh) -> bool {
    a.attribute(Mesh::ATTRIBUTE_POSITION) == b.attribute(Mesh::ATTRIBUTE_POSITION)
        && a.attribute(Mesh::ATTRIBUTE_NORMAL) == b.attribute(Mesh::ATTRIBUTE_NORMAL)
        && a.indices() == b.indices()
}

/// A part's projection of one field, as [`project`] gives it: `mesh` is
/// none when `drawn` already shows it ([`same_shape`]), and `bounds` and
/// `moved` are those of the projection either way.
pub struct Projection {
    pub mesh: Option<Mesh>,
    pub bounds: Aabb,
    /// Whether its positions differ from the source's.
    pub moved: bool,
}

/// A part's fixed projection data, so that each field costs only what it
/// changes (F864): the bind-space points, the triangles, and where each point
/// samples the car's lattice with its rest sample (set by the layout, fixed
/// per car). Each field then samples only the current lattice, and normals
/// are summed as `compute_smooth_normals` sums them, without a mesh copy; the
/// result has the bits of [`project`]. One per part (its source and bind).
#[derive(Default)]
pub struct Dents(Mutex<Option<Option<Cache>>>);

impl Dents {
    /// [`project`] of `field`, from this part's fixed data.
    pub fn project(
        &self,
        source: &Mesh,
        raw_bind: Mat4,
        field: &Field,
        drawn: Option<&Mesh>,
    ) -> Result<Projection, &'static str> {
        let mut cache = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if cache.is_none() {
            *cache = Some(Cache::new(source, raw_bind)?);
        }
        let Some(Some(cache)) = cache.as_mut() else {
            // A mesh this way does not take: projected whole.
            let mesh = project(source, raw_bind, field)?;
            let bounds = bevy::camera::primitives::MeshAabb::compute_aabb(&mesh)
                .ok_or("invalid projected body bounds")?;
            let moved = mesh.attribute(Mesh::ATTRIBUTE_POSITION) != source.attribute(Mesh::ATTRIBUTE_POSITION);
            let mesh = (!drawn.is_some_and(|d| same_shape(d, &mesh))).then_some(mesh);
            return Ok(Projection { mesh, bounds, moved });
        };
        let dented = cache.dent(field)?;
        let positions = dented.as_ref().map_or(&cache.original, |(p, _)| p);
        let bounds = Aabb::enclosing(positions.iter().map(|p| Vec3::from_slice(p))).unwrap_or_default();
        let moved = *positions != cache.original;
        let shown = drawn.is_some_and(|d| {
            matches!(d.attribute(Mesh::ATTRIBUTE_POSITION), Some(VertexAttributeValues::Float32x3(p)) if p == positions)
                && match &dented {
                    Some((_, n)) => matches!(d.attribute(Mesh::ATTRIBUTE_NORMAL), Some(VertexAttributeValues::Float32x3(m)) if m == n),
                    None => d.attribute(Mesh::ATTRIBUTE_NORMAL) == source.attribute(Mesh::ATTRIBUTE_NORMAL),
                }
                && d.indices() == source.indices()
        });
        let mesh = (!shown).then(|| {
            let mut mesh = source.clone();
            if let Some((p, n)) = dented {
                mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, p);
                mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, n);
            }
            mesh
        });
        Ok(Projection { mesh, bounds, moved })
    }
}

struct Cache {
    original: Vec<[f32; 3]>,
    queries: Vec<[f32; 4]>,
    inverse: Mat4,
    triangles: Vec<[usize; 3]>,
    /// Where the queries sample the last layout seen; none when it refuses
    /// one (each field is then sampled whole and refuses it too).
    probes: Option<Probes>,
}

impl Cache {
    /// The part's fixed data; `project`'s error for an invalid bind or
    /// source; none for a mesh not projected this way (not an indexed
    /// triangle list, or bounds precomputed).
    fn new(source: &Mesh, raw_bind: Mat4) -> Result<Option<Self>, &'static str> {
        if !raw_bind.is_finite() || raw_bind.determinant().abs() < 1e-8 {
            return Err("invalid body damage bind transform");
        }
        let inverse = raw_bind.inverse();
        if !inverse.is_finite() {
            return Err("invalid body damage inverse transform");
        }
        let Some(VertexAttributeValues::Float32x3(original)) =
            source.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            return Err("missing body damage source positions");
        };
        if original.is_empty()
            || original.len() > 1_000_000
            || original.iter().flatten().any(|v| !v.is_finite())
        {
            return Err("invalid body damage source positions");
        }
        if source.primitive_topology() != PrimitiveTopology::TriangleList || source.final_aabb.is_some() {
            return Ok(None);
        }
        // `compute_smooth_normals` takes whole triples, in index order.
        let triangles: Vec<[usize; 3]> = match source.indices() {
            Some(Indices::U16(i)) => i.as_chunks::<3>().0.iter().map(|t| t.map(usize::from)).collect(),
            Some(Indices::U32(i)) => i.as_chunks::<3>().0.iter().map(|t| t.map(|i| i as usize)).collect(),
            None => return Ok(None),
        };
        if triangles.iter().flatten().any(|&i| i >= original.len()) {
            return Ok(None);
        }
        let queries = original
            .iter()
            .map(|p| {
                let p = raw_bind.transform_point3(Vec3::from_array(*p));
                [p.x, p.y, p.z, 1.]
            })
            .collect();
        Ok(Some(Self { original: original.clone(), queries, inverse, triangles, probes: None }))
    }

    /// `project`'s positions and normals of `field`; none when every
    /// displacement is zero (`project` then returns the source).
    #[allow(clippy::type_complexity)]
    fn dent(&mut self, field: &Field) -> Result<Option<(Vec<[f32; 3]>, Vec<[f32; 3]>)>, &'static str> {
        if !self.probes.as_ref().is_some_and(|p| field.fits(p)) {
            self.probes = field.probes(&self.queries).ok();
        }
        let deltas = match &self.probes {
            Some(probes) => field.displacements_at(probes),
            None => field.displacements(&self.queries),
        }
        .map_err(|e| e.0)?;
        if deltas.iter().all(|d| *d == [0.; 3]) {
            return Ok(None);
        }
        let positions: Vec<[f32; 3]> = self
            .original
            .iter()
            .zip(deltas)
            .map(|(p, d)| (Vec3::from_array(*p) + self.inverse.transform_vector3(Vec3::from_array(d))).to_array())
            .collect();
        if positions.iter().flatten().any(|v| !v.is_finite() || v.abs() > 1e6) {
            return Err("body damage positions exceed bounds");
        }
        // `compute_smooth_normals`, term by term.
        let mut sums = vec![Vec3::ZERO; positions.len()];
        for &[a, b, c] in &self.triangles {
            let (pa, pb, pc) = (Vec3::from(positions[a]), Vec3::from(positions[b]), Vec3::from(positions[c]));
            let weight = |u: Vec3, w: Vec3| {
                if u.length_squared() * w.length_squared() > f32::EPSILON { u.angle_between(w) } else { 0.0 }
            };
            let (wa, wb, wc) = (weight(pb - pa, pc - pa), weight(pa - pb, pc - pb), weight(pa - pc, pb - pc));
            let normal = Vec3::from(triangle_normal(positions[a], positions[b], positions[c]));
            sums[a] += normal * wa;
            sums[b] += normal * wb;
            sums[c] += normal * wc;
        }
        let normals: Vec<[f32; 3]> = sums.into_iter().map(|n| n.try_normalize().unwrap_or(Vec3::ZERO).to_array()).collect();
        if normals.iter().flatten().any(|v| !v.is_finite()) {
            return Err("nonfinite body damage normals");
        }
        Ok(Some((positions, normals)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{asset::RenderAssetUsages, camera::primitives::MeshAabb};
    use grid_world::vehicle_publication::VEHICLE_WORDS;

    const DIMS: [usize; 3] = [4, 3, 3];

    /// A field over a 4 x 3 x 3 lattice of spacing 0.5 from (-1, -1, -1):
    /// rest nodes on the grid, current ones moved by `moved`.
    fn field(moved: &[[f32; 3]], active: bool) -> Field {
        let mut owner = vec![0u32; VEHICLE_WORDS];
        for (i, d) in DIMS.into_iter().enumerate() {
            owner[0xc714 / 4 + i] = d as u32;
        }
        owner[0xc720 / 4] = 0.5f32.to_bits();
        owner[0xc7ac / 4] = u32::from(active) << 8;
        for a in 0..3 {
            owner[0xc730 / 4 + a] = (-1f32).to_bits();
        }
        for (i, m) in moved.iter().enumerate() {
            let at = [i % 4, i / 4 % 3, i / 12].map(|c| c as f32 * 0.5 - 1.);
            for a in 0..3 {
                owner[(0x4880 + i * 80) / 4 + a] = (at[a] + m[a]).to_bits();
                owner[(0x4890 + i * 80) / 4 + a] = at[a].to_bits();
            }
            owner[(0x4880 + i * 80) / 4 + 3] = 1f32.to_bits();
            owner[(0x4890 + i * 80) / 4 + 3] = 1f32.to_bits();
        }
        Field::read(&owner).unwrap()
    }

    /// A bent sheet through the lattice and past its edge, with a repeated
    /// corner and a triangle whose corners coincide.
    fn sheet() -> Mesh {
        let n = 14;
        let positions: Vec<[f32; 3]> = (0..n * n)
            .map(|k| {
                let (x, z) = ((k % n) as f32 / (n - 1) as f32, (k / n) as f32 / (n - 1) as f32);
                [x * 2.2 - 1.1, (x * 3.).sin() * 0.3 + z * 0.2, z * 1.2 - 1.05]
            })
            .collect();
        let mut indices = Vec::new();
        for z in 0..n - 1 {
            for x in 0..n - 1 {
                let k = (z * n + x) as u16;
                let n = n as u16;
                indices.extend([k, k + n, k + 1, k + 1, k + n, k + n + 1]);
            }
        }
        indices.extend([5, 5, 6, 7, 7, 7]);
        let normals = vec![[0., 1., 0.]; positions.len()];
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
            .with_inserted_indices(Indices::U16(indices))
    }

    fn bits(mesh: &Mesh, attribute: bevy::mesh::MeshVertexAttribute) -> Vec<[u32; 3]> {
        let Some(VertexAttributeValues::Float32x3(v)) = mesh.attribute(attribute) else { panic!() };
        v.iter().map(|p| p.map(f32::to_bits)).collect()
    }

    #[test]
    fn dents_have_the_bits_of_a_whole_projection() {
        let source = sheet();
        let bind = Mat4::from_rotation_translation(Quat::from_rotation_y(0.4), Vec3::new(0.05, -0.1, 0.02));
        let (dents, shown) = (Dents::default(), Dents::default());
        let mut moved = vec![[0f32; 3]; 36];
        let mut seed = 12345u32;
        let mut next = || {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            seed >> 8
        };
        let mut counts = [0; 3];
        for step in 0..60 {
            let mut active = true;
            match step {
                0 => {}
                20 | 40 => moved.fill([0.; 3]),
                25 => active = false,
                _ => {
                    for _ in 0..1 + next() % 3 {
                        let node = next() as usize % 36;
                        let axis = next() as usize % 3;
                        moved[node][axis] += (next() % 200) as f32 * 1e-3 - 0.1;
                        if next() % 7 == 0 {
                            moved[node] = [0.; 3];
                        }
                    }
                }
            }
            let field = field(&moved, active);
            let whole = project(&source, bind, &field).unwrap();
            let step_result = dents.project(&source, bind, &field, None).unwrap();
            let mesh = step_result.mesh.unwrap();
            assert_eq!(bits(&mesh, Mesh::ATTRIBUTE_POSITION), bits(&whole, Mesh::ATTRIBUTE_POSITION), "step {step}");
            assert_eq!(bits(&mesh, Mesh::ATTRIBUTE_NORMAL), bits(&whole, Mesh::ATTRIBUTE_NORMAL), "step {step}");
            assert_eq!(mesh.indices(), whole.indices());
            let bounds = whole.compute_aabb().unwrap();
            assert_eq!(step_result.bounds, bounds);
            assert_eq!(step_result.moved, whole.attribute(Mesh::ATTRIBUTE_POSITION) != source.attribute(Mesh::ATTRIBUTE_POSITION));
            // A part whose mesh shows the projection is not written.
            assert!(shown.project(&source, bind, &field, Some(&whole)).unwrap().mesh.is_none());
            counts[usize::from(step_result.moved)] += 1;
        }
        // Pristine and dented fields both came up.
        assert!(counts[0] >= 3 && counts[1] >= 40, "{counts:?}");
        // A refused field leaves the next one whole, and still exact.
        let saved = moved[17];
        moved[17][1] = 1e10;
        assert!(project(&source, bind, &field(&moved, true)).is_err());
        assert!(dents.project(&source, bind, &field(&moved, true), None).is_err());
        moved[17] = saved;
        let whole = project(&source, bind, &field(&moved, true)).unwrap();
        let mesh = dents.project(&source, bind, &field(&moved, true), None).unwrap().mesh.unwrap();
        assert_eq!(bits(&mesh, Mesh::ATTRIBUTE_POSITION), bits(&whole, Mesh::ATTRIBUTE_POSITION));
        assert_eq!(bits(&mesh, Mesh::ATTRIBUTE_NORMAL), bits(&whole, Mesh::ATTRIBUTE_NORMAL));
    }
}
