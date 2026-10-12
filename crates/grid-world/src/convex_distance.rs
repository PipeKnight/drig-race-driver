//! Triangle/owned-mesh support distance iteration. Manifold and penetration
//! construction are separate. This implements the complete selected B7EA60 loop.
use crate::{Error, collision_mesh::Mesh, physical_pose::Matrix};

type V = [f32; 4];
const EPS: f32 = f32::from_bits(0x3586_37bd);
const CONVERGENCE: f32 = f32::from_bits(0x3586_37be);

fn dot(a: V, b: V) -> f32 {
    (a[0] * b[0] + a[1] * b[1]) + (a[2] * b[2] + a[3] * b[3])
}
fn sub(a: V, b: V) -> V {
    std::array::from_fn(|j| a[j] - b[j])
}
fn cross(a: V, b: V) -> V {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
        0.,
    ]
}

/// Barycentric closest-feature result, including the original active flags.
struct Feature {
    point: V,
    weights: V,
    active: [bool; 4],
}
fn triangle_feature(a: V, b: V, c: V, ab: V, ac: V) -> Feature {
    let feature = |mut point: V, weights, active| {
        point[3] = 1.;
        Feature {
            point,
            weights,
            active,
        }
    };
    let d1 = -dot(ab, a);
    let d2 = -dot(ac, a);
    if d1 <= 0. && d2 <= 0. {
        return feature(a, [1., 0., 0., 0.], [true, false, false, false]);
    }
    let d3 = -dot(ab, b);
    let d4 = -dot(ac, b);
    if 0. <= d3 && d4 <= d3 {
        return feature(b, [0., 1., 0., 0.], [false, true, false, false]);
    }
    let vc = d4 * d1 - d2 * d3;
    if vc <= 0. && 0. <= d1 && d3 <= 0. && EPS < d1 - d3 {
        let v = d1 / (d1 - d3);
        return feature(
            std::array::from_fn(|j| v * ab[j] + a[j]),
            [1. - v, v, 0., 0.],
            [true, true, false, false],
        );
    }
    let d5 = -dot(ab, c);
    let d6 = -dot(ac, c);
    if 0. <= d6 && d5 <= d6 {
        return feature(c, [0., 0., 1., 0.], [false, false, true, false]);
    }
    let vb = d2 * d5 - d6 * d1;
    if vb <= 0. && 0. <= d2 && d6 <= 0. && EPS < d2 - d6 {
        let w = d2 / (d2 - d6);
        return feature(
            std::array::from_fn(|j| w * ac[j] + a[j]),
            [1. - w, 0., w, 0.],
            [true, false, true, false],
        );
    }
    let va = d6 * d3 - d4 * d5;
    let d43 = d4 - d3;
    let d56 = d5 - d6;
    if va <= 0. && 0. <= d43 && 0. <= d56 && EPS < d56 + d43 {
        let w = d43 / (d56 + d43);
        return feature(
            std::array::from_fn(|j| w * (c[j] - b[j]) + b[j]),
            [0., 1. - w, w, 0.],
            [false, true, true, false],
        );
    }
    let sum = (va + vb) + vc;
    if EPS < sum.abs() {
        let inverse = 1. / sum;
        let w = inverse * vc;
        let v = inverse * vb;
        return feature(
            std::array::from_fn(|j| (v * ab[j] + a[j]) + w * ac[j]),
            [(1. - v) - w, v, w, 0.],
            [true, true, true, false],
        );
    }
    // Deliberate original degenerate fallback; not an exact one-third substitute.
    let third = f32::from_bits(0x3eaa_7efa);
    feature(
        std::array::from_fn(|j| (third * ab[j] + a[j]) + third * ac[j]),
        [f32::from_bits(0x3eab_020c), third, third, 0.],
        [true, true, true, false],
    )
}

/// None means origin inside; Err means the original degenerate-plane guard.
fn tetrahedron_feature(p: [V; 4]) -> Result<Option<Feature>, ()> {
    let ab = sub(p[1], p[0]);
    let ac = sub(p[2], p[0]);
    let ad = sub(p[3], p[0]);
    let bc = sub(p[2], p[1]);
    let bd = sub(p[3], p[1]);
    let edges = [
        (p[0], ab, ac, ad),
        (p[0], ac, ad, ab),
        (p[0], ad, ab, ac),
        (p[1], bd, bc, ab.map(|v| -v)),
    ];
    let mut outside = [false; 4];
    for (i, (origin, u, v, opposite)) in edges.into_iter().enumerate() {
        let normal = cross(u, v);
        let denominator = dot(opposite, normal);
        if denominator * denominator < f32::from_bits(0x322b_cc77) {
            return Err(());
        }
        outside[i] = -dot(origin, normal) * denominator < 0.;
    }
    let faces = [[0, 1, 2], [0, 2, 3], [0, 3, 1], [1, 3, 2]];
    let mut best: Option<Feature> = None;
    let mut distance = f32::MAX;
    for (enabled, ids) in outside.into_iter().zip(faces) {
        if !enabled {
            continue;
        }
        let [a, b, c] = ids.map(|i| p[i]);
        let f = triangle_feature(a, b, c, sub(b, a), sub(c, a));
        let squared = dot(f.point, f.point);
        if squared < distance {
            let mut weights = [0.; 4];
            let mut active = [false; 4];
            for j in 0..3 {
                weights[ids[j]] = f.weights[j];
                active[ids[j]] = f.active[j];
            }
            best = Some(Feature {
                point: f.point,
                weights,
                active,
            });
            distance = squared;
        }
    }
    Ok(best)
}

#[derive(Clone)]
struct Simplex {
    bytes: [u8; 0x120],
}
impl Simplex {
    fn new() -> Self {
        let mut s = Self { bytes: [0; 0x120] };
        s.vector(0xc0, [1., 0., 0., 0.]);
        s.reset();
        s
    }
    fn reset(&mut self) {
        self.bytes[0x60..0xb0].fill(0);
        self.bytes[0] = 0;
        self.bytes[0x11c] = 0;
        self.bytes[0x11d] = 0;
        self.bytes[0x11e] = 1;
        self.vector(0xf0, [f32::MAX, f32::MAX, f32::MAX, 1.]);
        self.put(0x100, 0);
        self.put(0x118, u32::MAX);
    }
    fn word(&self, o: usize) -> u32 {
        u32::from_le_bytes(self.bytes[o..o + 4].try_into().unwrap())
    }
    fn put(&mut self, o: usize, v: u32) {
        self.bytes[o..o + 4].copy_from_slice(&v.to_le_bytes());
    }
    fn value(&self, o: usize) -> f32 {
        f32::from_bits(self.word(o))
    }
    fn read(&self, o: usize) -> V {
        std::array::from_fn(|j| self.value(o + j * 4))
    }
    fn vector(&mut self, o: usize, v: V) {
        for (j, x) in v.into_iter().enumerate() {
            self.put(o + j * 4, x.to_bits());
        }
    }
    fn count(&self) -> usize {
        self.bytes[0] as usize
    }
    fn duplicate(&self, p: V) -> bool {
        let near = |q: V| {
            let d = sub(p, q).map(f32::abs);
            ((d[1] + d[0]) + d[2]) + d[3] < EPS
        };
        (0..self.count()).any(|i| near(self.read(0x10 + i * 16))) || near(self.read(0xf0))
    }
    fn add(&mut self, p: V, a: V) {
        let n = self.count();
        self.vector(0xf0, p);
        self.bytes[0x11e] = 1;
        self.vector(0x10 + n * 16, p);
        self.vector(0x60 + n * 16, a);
        let sq = dot(p, p);
        self.put(0x104 + n * 4, sq.to_bits());
        if self.value(0x100) <= sq {
            self.put(0x100, sq.to_bits());
            self.put(0x118, n as u32);
        }
        self.bytes[0] += 1;
    }
    fn remove(&mut self, i: usize) {
        self.bytes[0] -= 1;
        let n = self.count();
        self.vector(0x10 + i * 16, self.read(0x10 + n * 16));
        self.vector(0x60 + i * 16, self.read(0x60 + n * 16));
        self.put(0x104 + i * 4, self.word(0x104 + n * 4));
        if i as u32 == self.word(0x118) {
            self.bytes[0x11c] = 1;
        }
        if n as u32 == self.word(0x118) {
            self.put(0x118, i as u32);
        }
    }
    fn feature(&mut self, f: Feature) -> bool {
        let n = self.count();
        let supports: [V; 4] = std::array::from_fn(|i| self.read(0x60 + i * 16));
        let a = std::array::from_fn(|j| {
            let v = (f.weights[0] * supports[0][j] + f.weights[1] * supports[1][j])
                + f.weights[2] * supports[2][j];
            if n == 4 {
                v + f.weights[3] * supports[3][j]
            } else {
                v
            }
        });
        self.vector(0xb0, a);
        let mut p = f.point;
        p[3] = 0.;
        self.vector(0xc0, p);
        for i in (0..4).rev() {
            if self.count() > i && !f.active[i] {
                self.remove(i);
            }
        }
        f.weights.iter().all(|&v| 0. <= v)
    }
    fn reduce(&mut self) -> bool {
        if self.bytes[0x11e] == 0 {
            return self.bytes[0x11d] != 0;
        }
        self.bytes[0x11e] = 0;
        self.vector(0xe0, self.read(0xc0));
        self.vector(0xd0, self.read(0xb0));
        let valid = match self.count() {
            1 => {
                self.vector(0xb0, self.read(0x60));
                self.vector(0xc0, self.read(0x10));
                true
            }
            2 => {
                let a = self.read(0x10);
                let b = self.read(0x20);
                let d = sub(a, b);
                let t = dot(d, a);
                if t <= 0. {
                    self.vector(0xb0, self.read(0x60));
                    self.vector(0xc0, a);
                    self.remove(1);
                } else {
                    let sq = dot(d, d);
                    if t < sq {
                        let t = t / sq;
                        let sa = self.read(0x60);
                        let sb = self.read(0x70);
                        self.vector(0xb0, std::array::from_fn(|j| (sb[j] - sa[j]) * t + sa[j]));
                        self.vector(0xc0, std::array::from_fn(|j| t * (b[j] - a[j]) + a[j]));
                    } else {
                        self.vector(0xb0, self.read(0x70));
                        self.vector(0xc0, b);
                        self.remove(0);
                    }
                }
                true
            }
            3 => {
                let a = self.read(0x10);
                let b = self.read(0x20);
                let c = self.read(0x30);
                self.feature(triangle_feature(a, b, c, sub(b, a), sub(c, a)))
            }
            4 => match tetrahedron_feature(std::array::from_fn(|i| self.read(0x10 + i * 16))) {
                Ok(Some(f)) => self.feature(f),
                Ok(None) => {
                    self.vector(0xc0, [0.; 4]);
                    true
                }
                Err(()) => false,
            },
            _ => false,
        };
        self.bytes[0x11d] = valid as u8;
        valid
    }
    fn closest(&mut self, direction: &mut V) -> bool {
        let valid = self.reduce();
        *direction = self.read(0xc0);
        if self.bytes[0x11c] != 0 {
            self.bytes[0x11c] = 0;
            self.put(0x100, 0);
            self.put(0x118, u32::MAX);
            for i in 0..self.count() {
                if self.value(0x100) <= self.value(0x104 + i * 4) {
                    self.put(0x100, self.word(0x104 + i * 4));
                    self.put(0x118, i as u32);
                }
            }
        }
        valid
    }
    fn restore(&mut self, direction: &mut V) {
        self.bytes[0x11e] = 0;
        self.vector(0xc0, self.read(0xe0));
        self.vector(0xb0, self.read(0xd0));
        *direction = self.read(0xc0);
    }
    fn witnesses(&mut self) -> [V; 2] {
        self.reduce();
        let mut a = self.read(0xb0);
        let mut b = sub(a, self.read(0xc0));
        a[3] = 1.;
        b[3] = 1.;
        [a, b]
    }
}

/// Complete support iteration result. The two flags retain original semantics
/// without treating a distance-loop termination as a finished contact manifold.
pub struct Output {
    pub direction: V,
    pub flags: [bool; 2],
    pub witnesses: Option<[V; 2]>,
    pub support_vertices: [usize; 7],
    pub simplex_bytes: [u8; 0x120],
}

fn mesh_support(
    mesh: &Mesh,
    matrix: Matrix,
    direction: V,
    cache: &mut [usize; 7],
) -> Result<V, Error> {
    let local = [
        dot(direction, matrix[0]),
        dot(direction, matrix[1]),
        dot(direction, matrix[2]),
        0.,
    ];
    let mut slot = 0;
    let mut score = dot(local, mesh.point(cache[0]).unwrap());
    for (i, &cached_vertex) in cache.iter().enumerate().take(6).skip(1) {
        let next = dot(local, mesh.point(cached_vertex).unwrap());
        if score < next {
            score = next;
            slot = i;
        }
    }
    let mut vertex = cache[slot];
    // Mesh::build owns one hierarchy level; adjacency retains source corner order.
    for _ in 0..mesh.point_count() * mesh.point_count() {
        let p = mesh.point(vertex).unwrap();
        let bytes = mesh.bytes();
        let word = |o| u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap()) as usize;
        let aux = word(0x60 + vertex * 32 + 0x10);
        let count = word(aux);
        let neighbors = word(aux + 4);
        let mut next = None;
        for i in 0..count {
            let other = (word(neighbors + i * 4) - 0x60) / 32;
            if dot(local, p) + EPS < dot(local, mesh.point(other).unwrap()) {
                next = Some(other);
                break;
            }
        }
        if let Some(other) = next {
            vertex = other;
            continue;
        }
        cache[6] = vertex;
        cache[slot] = vertex;
        return Ok(std::array::from_fn(|j| {
            (p[3] * matrix[3][j] + p[1] * matrix[1][j])
                + (p[2] * matrix[2][j] + p[0] * matrix[0][j])
        }));
    }
    Err(Error("mesh support did not converge"))
}

/// Source-owned triangle versus a one-level constructed collision mesh.
/// This does not run the outer feature test, penetration search or manifold.
pub fn calculate(
    mesh: &Mesh,
    matrix: Matrix,
    triangle: [V; 3],
    margin: f32,
    direction: V,
) -> Result<Output, Error> {
    Context::new().query(mesh, matrix, triangle, margin, direction)
}

/// Owned per-mesh-pair distance state. Queries preserve the support cache and
/// the scratch fields that the original partial reset leaves intact.
#[derive(Clone)]
pub struct Context {
    simplex: Simplex,
    cache: [usize; 7],
}
impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}
impl Context {
    pub fn new() -> Self {
        Self {
            simplex: Simplex::new(),
            cache: [0; 7],
        }
    }
    /// Use one context per mesh pair; reset it if the mesh/topology changes.
    /// Failure leaves its complete state intact.
    pub fn query(
        &mut self,
        mesh: &Mesh,
        matrix: Matrix,
        triangle: [V; 3],
        margin: f32,
        direction: V,
    ) -> Result<Output, Error> {
        if self.cache.iter().any(|&v| v >= mesh.point_count()) {
            return Err(Error("distance cache index outside supplied mesh"));
        }
        let mut staged = self.clone();
        let output = run_query(
            mesh,
            matrix,
            triangle,
            margin,
            direction,
            &mut staged.simplex,
            &mut staged.cache,
        )?;
        *self = staged;
        Ok(output)
    }
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

#[allow(clippy::neg_multiply)] // Preserve the original multiply-by-minus-one.
fn run_query(
    mesh: &Mesh,
    matrix: Matrix,
    triangle: [V; 3],
    margin: f32,
    mut direction: V,
    simplex: &mut Simplex,
    cache: &mut [usize; 7],
) -> Result<Output, Error> {
    if matrix
        .iter()
        .flatten()
        .chain(triangle.iter().flatten())
        .chain(direction.iter())
        .any(|v| !v.is_finite() || v.abs() > 100000.)
        || triangle.iter().any(|p| p[3] != 1.)
        || direction[3] != 0.
        || !margin.is_finite()
        || !(0. ..=1e6).contains(&margin)
    {
        return Err(Error("invalid convex distance input"));
    }
    simplex.reset();
    let mut flags = [false, true];
    let mut previous = f32::MAX;
    let mut bound = f32::MAX;
    let margin_squared = margin * margin;
    let mut finished = false;
    for _ in 0..256 {
        let opposite = direction.map(|v| -1. * v);
        let scores = triangle.map(|v| dot(v, opposite));
        let a = if scores[1] <= scores[0] && scores[2] <= scores[0] {
            triangle[0]
        } else if scores[0] <= scores[1] && scores[2] <= scores[1] {
            triangle[1]
        } else {
            triangle[2]
        };
        let b = mesh_support(mesh, matrix, direction, cache)?;
        let difference = sub(a, b);
        let projection = dot(direction, difference);
        if 0. < projection && bound < projection * projection {
            flags[1] = false;
            finished = true;
            break;
        }
        if simplex.duplicate(difference) || previous - projection <= previous * CONVERGENCE {
            flags[0] = true;
            finished = true;
            break;
        }
        simplex.add(difference, a);
        if !simplex.closest(&mut direction) {
            flags[0] = true;
            simplex.restore(&mut direction);
            finished = true;
            break;
        }
        let sq = dot(direction, direction);
        if sq <= EPS {
            finished = true;
            break;
        }
        if previous - sq <= previous * EPS {
            flags[0] = true;
            simplex.restore(&mut direction);
            finished = true;
            break;
        }
        bound = sq * margin_squared;
        if simplex.count() == 4 {
            finished = true;
            break;
        }
        previous = sq;
    }
    if !finished {
        return Err(Error("convex distance did not converge"));
    }
    let witnesses = flags[0].then(|| simplex.witnesses());
    if direction
        .iter()
        .chain(simplex.read(0xc0).iter())
        .any(|v| !v.is_finite())
    {
        return Err(Error("nonfinite convex distance result"));
    }
    Ok(Output {
        direction,
        flags,
        witnesses,
        support_vertices: *cache,
        simplex_bytes: simplex.bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tetrahedron() -> Mesh {
        crate::collision_mesh::build(
            &[
                [1., 1., 1., 1.],
                [-1., -1., 1., 1.],
                [-1., 1., -1., 1.],
                [1., -1., -1., 1.],
            ],
            &[[0, 1, 2], [0, 3, 1], [0, 2, 3], [1, 3, 2]],
            0,
        )
        .unwrap()
    }
    const IDENTITY: Matrix = [
        [1., 0., 0., 0.],
        [0., 1., 0., 0.],
        [0., 0., 1., 0.],
        [0., 0., 0., 1.],
    ];
    #[test]
    fn separated_features_and_margin_witnesses() {
        let mesh = tetrahedron();
        let triangle = [[-8., 4., -8., 1.], [8., 4., -8., 1.], [0., 4., 8., 1.]];
        let separated = calculate(&mesh, IDENTITY, triangle, 0., [0., 1., 0., 0.]).unwrap();
        assert_eq!(separated.flags, [false, false]);
        assert!(separated.witnesses.is_none());
        let close = calculate(&mesh, IDENTITY, triangle, 4., [0., 1., 0., 0.]).unwrap();
        assert!(close.flags[0]);
        let [a, b] = close.witnesses.unwrap();
        assert!((a[1] - 4.).abs() < 1e-5 && (b[1] - 1.).abs() < 1e-5);
        assert!((dot(sub(a, b), sub(a, b)) - 9.).abs() < 1e-4);
        assert_eq!(a[3], 1.);
        assert_eq!(b[3], 1.);
    }
    #[test]
    fn intersection_and_invalid_controls_are_distinct() {
        let mesh = tetrahedron();
        let triangle = [[-8., 0., -8., 1.], [8., 0., -8., 1.], [0., 0., 8., 1.]];
        let r = calculate(&mesh, IDENTITY, triangle, 0., [1., 0., 0., 0.]).unwrap();
        assert_eq!(r.flags, [false, true]);
        assert!(dot(r.direction, r.direction) <= EPS);
        assert!(r.witnesses.is_none());
        assert!(calculate(&mesh, IDENTITY, triangle, -1., [1., 0., 0., 0.]).is_err());
        assert!(calculate(&mesh, IDENTITY, triangle, 0., [f32::NAN, 0., 0., 0.]).is_err());
    }
    #[test]
    fn persistent_failure_rolls_back_and_reset_reproduces_cold_query() {
        let mesh = tetrahedron();
        let triangle = [[-8., 0., -8., 1.], [8., 0., -8., 1.], [0., 0., 8., 1.]];
        let mut context = Context::new();
        context
            .query(&mesh, IDENTITY, triangle, 0., [1., 0., 0., 0.])
            .unwrap();
        let before = context.clone();
        assert!(
            context
                .query(&mesh, IDENTITY, triangle, -1., [1., 0., 0., 0.])
                .is_err()
        );
        assert_eq!(context.simplex.bytes, before.simplex.bytes);
        assert_eq!(context.cache, before.cache);
        context.reset();
        let reset = context
            .query(&mesh, IDENTITY, triangle, 0., [1., 0., 0., 0.])
            .unwrap();
        let cold = calculate(&mesh, IDENTITY, triangle, 0., [1., 0., 0., 0.]).unwrap();
        assert_eq!(reset.simplex_bytes, cold.simplex_bytes);
        assert_eq!(reset.support_vertices, cold.support_vertices);
    }
}
