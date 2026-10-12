//! Owned collision mesh topology corresponding to selected whole B44E80.
//! Pointer fields are checked relative offsets, never process addresses.
//! Supplied geometry is not convexified or registered as a physical shape.
use crate::{Error, spawn};

pub struct Mesh {
    bytes: Vec<u8>,
    pub points: usize,
    pub faces: usize,
}
impl Mesh {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn point(&self, i: usize) -> Option<[f32; 4]> {
        (i < self.points).then(|| {
            std::array::from_fn(|j| f32::from_bits(word(&self.bytes, 0x60 + i * 32 + j * 4)))
        })
    }
    pub fn face_normal(&self, i: usize) -> Option<[f32; 4]> {
        (i < self.faces).then(|| {
            std::array::from_fn(|j| {
                f32::from_bits(word(&self.bytes, 0x60 + self.points * 48 + i * 32 + j * 4))
            })
        })
    }
}
fn word(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn put(b: &mut [u8], o: usize, value: u32) {
    b[o..o + 4].copy_from_slice(&value.to_le_bytes());
}
fn align16(n: usize) -> usize {
    (n + 15) & !15
}
fn cross(points: &[[f32; 4]], t: [u32; 3]) -> [f32; 4] {
    let [a, b, c] = t.map(|i| points[i as usize]);
    let ab: [f32; 3] = std::array::from_fn(|i| b[i] - a[i]);
    let ac: [f32; 3] = std::array::from_fn(|i| c[i] - a[i]);
    [
        ab[1] * ac[2] - ab[2] * ac[1],
        ac[0] * ab[2] - ac[2] * ab[0],
        ac[1] * ab[0] - ac[0] * ab[1],
        0.,
    ]
}
fn squared(v: [f32; 4]) -> f32 {
    (v[0] * v[0] + v[1] * v[1]) + (v[2] * v[2] + v[3] * v[3])
}

/// Selected whole 9B8B70 -> 9B8890 coordinate adjustment, deformation disabled.
/// `vertical_extents` are source bottom/roof. The mesh retains the car's local
/// X/Z offset while Y is centered for the separate shape transform.
pub fn prepare_points(
    points: &[[f32; 4]],
    offset: [f32; 4],
    vertical_extents: [f32; 2],
) -> Result<Vec<[f32; 4]>, Error> {
    if !(4..=80).contains(&points.len())
        || offset
            .iter()
            .chain(&vertical_extents)
            .any(|x| !x.is_finite() || x.abs() > 10000.)
        || offset[3] != 0.
        || vertical_extents[0] > vertical_extents[1]
        || points
            .iter()
            .any(|v| v.iter().any(|x| !x.is_finite() || x.abs() > 100000.) || v[3] != 1.)
    {
        return Err(Error("invalid source collision adjustment"));
    }
    let mut floor = points[0][1];
    for p in &points[1..] {
        if p[1] < floor {
            floor = p[1];
        }
    }
    let center = offset[1] + (vertical_extents[1] + vertical_extents[0]) * 0.5;
    let result: Vec<[f32; 4]> = points
        .iter()
        .map(|p| {
            let y = if p[1] < floor { floor } else { p[1] };
            [
                (offset[0] + p[0]) - 0.,
                (offset[1] + y) - center,
                (offset[2] + p[2]) - 0.,
                1.,
            ]
        })
        .collect();
    if result
        .iter()
        .flatten()
        .any(|x| !x.is_finite() || x.abs() > 100000.)
    {
        return Err(Error("source collision adjustment exceeds range"));
    }
    Ok(result)
}

pub fn build_from_source(
    points: &[[f32; 4]],
    triangles: &[[u32; 3]],
    offset: [f32; 4],
    vertical_extents: [f32; 2],
    fill: u8,
) -> Result<Mesh, Error> {
    build(
        &prepare_points(points, offset, vertical_extents)?,
        triangles,
        fill,
    )
}

/// Build complete supplied mesh bytes, preserving the selected zero/CD fill in
/// untouched padding. Source geometry adjustment, allocation and shape factory
/// are external. Adjacency order follows source triangle and corner order.
pub fn build(points: &[[f32; 4]], triangles: &[[u32; 3]], fill: u8) -> Result<Mesh, Error> {
    let p = points.len();
    let t = triangles.len();
    if !(4..=80).contains(&p)
        || !(4..=110).contains(&t)
        || ![0, 0xcd].contains(&fill)
        || points
            .iter()
            .any(|v| v.iter().any(|x| !x.is_finite() || x.abs() > 100000.) || v[3] != 1.)
    {
        return Err(Error("invalid collision mesh framing"));
    }
    let mut normals = Vec::with_capacity(t);
    for &tri in triangles {
        if tri.iter().any(|&i| i as usize >= p)
            || tri[0] == tri[1]
            || tri[1] == tri[2]
            || tri[0] == tri[2]
        {
            return Err(Error("invalid collision mesh triangle"));
        }
        let n = cross(points, tri);
        let sq = squared(n);
        if !sq.is_finite() || !(1e-20..=1e20).contains(&sq) {
            return Err(Error("invalid collision mesh normal"));
        }
        let seed = spawn::reference_seed(sq, false)?;
        let product = seed * sq;
        let length = (1. - product * seed) * (product * 0.5) + product;
        let factor = -1. / length;
        let normal = n.map(|x| factor * x);
        if normal.iter().any(|v| !v.is_finite()) {
            return Err(Error("collision mesh normal exceeds range"));
        }
        normals.push(normal);
    }
    let mut neighbors = vec![Vec::<usize>::new(); p];
    let mut incident = vec![Vec::<usize>::new(); p];
    for i in 0..p {
        for (j, tri) in triangles.iter().enumerate() {
            if !tri.contains(&(i as u32)) {
                continue;
            }
            incident[i].push(j);
            for &k in tri {
                let k = k as usize;
                if k != i && !neighbors[i].contains(&k) {
                    neighbors[i].push(k);
                }
            }
        }
    }
    if neighbors.iter().map(Vec::len).sum::<usize>() > p * 10 {
        return Err(Error("collision mesh adjacency capacity exceeded"));
    }
    let vertices = 0x60;
    let aux = vertices + p * 32;
    let faces = aux + p * 16;
    let adjacent = faces + t * 32;
    let incidence = adjacent + align16(p * 40);
    let length = incidence + align16(t * 12);
    let mut b = vec![fill; length];
    put(&mut b, 0, p as u32);
    put(&mut b, 4, t as u32);
    put(&mut b, 8, 1);
    put(&mut b, 12, 1);
    for i in 0..16 {
        put(
            &mut b,
            0x10 + i * 4,
            if i % 5 == 0 { 1f32.to_bits() } else { 0 },
        );
    }
    put(&mut b, 0x50, 1f32.to_bits());
    put(&mut b, 0x54, vertices as u32);
    put(&mut b, 0x58, faces as u32);
    // B42540 returns the least absolute coordinate among the six source
    // extrema. This field is deliberately not mislabeled a bounding radius.
    let mut min = points[0];
    let mut max = min;
    for point in &points[1..] {
        for j in 0..3 {
            if point[j] < min[j] {
                min[j] = point[j];
            }
            if max[j] < point[j] {
                max[j] = point[j];
            }
        }
    }
    let mut extent = min[0].abs();
    for v in [min[1], min[2], max[0], max[1], max[2]] {
        if v.abs() < extent {
            extent = v.abs();
        }
    }
    put(&mut b, 0x5c, extent.to_bits());
    let mut edge_cursor = 0;
    let mut face_cursor = 0;
    for (i, point) in points.iter().enumerate() {
        let v = vertices + i * 32;
        let a = aux + i * 16;
        for (j, x) in point.iter().enumerate() {
            put(&mut b, v + j * 4, x.to_bits());
        }
        put(&mut b, v + 0x10, a as u32);
        put(&mut b, v + 0x14, incident[i].len() as u32);
        put(&mut b, v + 0x18, (incidence + face_cursor * 4) as u32);
        put(&mut b, a, neighbors[i].len() as u32);
        put(&mut b, a + 4, (adjacent + edge_cursor * 4) as u32);
        for &j in &neighbors[i] {
            put(
                &mut b,
                adjacent + edge_cursor * 4,
                (vertices + j * 32) as u32,
            );
            edge_cursor += 1;
        }
        for &j in &incident[i] {
            put(&mut b, incidence + face_cursor * 4, (faces + j * 32) as u32);
            face_cursor += 1;
        }
    }
    for (i, (tri, normal)) in triangles.iter().zip(normals).enumerate() {
        let f = faces + i * 32;
        for (j, x) in normal.iter().enumerate() {
            put(&mut b, f + j * 4, x.to_bits());
        }
        for (j, &x) in tri.iter().enumerate() {
            put(&mut b, f + 0x10 + j * 4, x);
        }
    }
    Ok(Mesh {
        bytes: b,
        points: p,
        faces: t,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_mesh_centers_height_and_keeps_owned_source_unchanged() {
        let raw = [
            [1., 3., 2., 1.],
            [-1., 3., -2., 1.],
            [1., 1., -2., 1.],
            [-1., 1., 2., 1.],
        ];
        let before = raw;
        let triangles = [[0, 1, 2], [0, 3, 1], [0, 2, 3], [1, 3, 2]];
        let offset = [-0., -0.375, -0.25, 0.];
        let mesh = build_from_source(&raw, &triangles, offset, [1., 3.], 0xcd).unwrap();
        assert_eq!(raw, before);
        for (i, point) in raw.iter().enumerate() {
            let adjusted = mesh.point(i).unwrap();
            assert_eq!(adjusted, [point[0], point[1] - 2., point[2] - 0.25, 1.]);
            // Shape-local translation restores the offset geometry position.
            assert_eq!(adjusted[1] + 1.625, point[1] - 0.375);
        }
        assert!(prepare_points(&raw, offset, [3., 1.]).is_err());
        assert!(prepare_points(&raw, [0., 0., 0., 1.], [1., 3.]).is_err());
        assert!(prepare_points(&raw, [0., f32::NAN, 0., 0.], [1., 3.]).is_err());
        let huge = raw.map(|mut p| {
            p[0] = 100000.;
            p
        });
        assert!(prepare_points(&huge, [1., 0., 0., 0.], [1., 3.]).is_err());
    }

    #[test]
    fn tetrahedron_preserves_topology_padding_and_normal_winding() {
        let p = [
            [1., 1., 1., 1.],
            [-1., -1., 1., 1.],
            [-1., 1., -1., 1.],
            [1., -1., -1., 1.],
        ];
        let t = [[0, 1, 2], [0, 3, 1], [0, 2, 3], [1, 3, 2]];
        let mesh = build(&p, &t, 0xcd).unwrap();
        for (i, point) in p.into_iter().enumerate() {
            assert_eq!(mesh.point(i), Some(point));
            assert_eq!(word(mesh.bytes(), 0x60 + i * 32 + 0x14), 3);
            assert_eq!(word(mesh.bytes(), 0x60 + 4 * 32 + i * 16), 3);
            assert_eq!(
                &mesh.bytes()[0x60 + i * 32 + 28..0x60 + i * 32 + 32],
                &[0xcd; 4]
            );
        }
        for (i, tri) in t.into_iter().enumerate() {
            let n = mesh.face_normal(i).unwrap();
            let c = cross(&p, tri);
            assert!((squared(n) - 1.).abs() < 1e-6);
            assert!(n.iter().zip(c).map(|(a, b)| a * b).sum::<f32>() < 0.);
            assert_eq!(n[3], 0.);
        }
        assert!(mesh.point(4).is_none() && mesh.face_normal(4).is_none());
        let mut bad = p;
        bad[1] = bad[0];
        assert!(build(&bad, &t, 0).is_err());
        bad = p;
        bad[0][0] = f32::NAN;
        assert!(build(&bad, &t, 0).is_err());
        bad = p;
        bad[0][3] = 0.;
        assert!(build(&bad, &t, 0).is_err());
        assert!(build(&p, &t, 1).is_err());
    }
}
