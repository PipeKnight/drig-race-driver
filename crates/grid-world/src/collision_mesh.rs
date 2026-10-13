//! Owned collision mesh topology corresponding to selected whole B44E80.
//! Pointer fields are checked relative offsets, never process addresses.
//! Supplied geometry is not convexified. Selected first-shape attachment is explicit.
use crate::{Error, spawn};

#[derive(Clone)]
pub struct Mesh {
    bytes: Vec<u8>,
    points: usize,
    faces: usize,
}
impl Mesh {
    pub fn point_count(&self) -> usize {
        self.points
    }
    pub fn face_count(&self) -> usize {
        self.faces
    }
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
/// Selected B444D0 shape settings. Names retain unknown field semantics.
#[derive(Clone, Copy)]
pub struct ShapeSettings {
    pub span: u32,
    pub scalar220: f32,
    pub layer: u8,
    pub flag_c0: bool,
}

/// Complete retained supplied shape storage and its owned mesh.
/// This update does not perform base construction or physical registration.
#[derive(Clone)]
pub struct MeshShape {
    shape: [u8; 0x250],
    mesh: Mesh,
}
impl MeshShape {
    pub fn shape_bytes(&self) -> &[u8] {
        &self.shape
    }
    pub fn mesh(&self) -> &Mesh {
        &self.mesh
    }
}

impl Mesh {
    // Preserve original SIMD operand order and multiply-by-minus-one operation.
    #[allow(clippy::assign_op_pattern, clippy::neg_multiply)]
    pub fn initialize_shape(
        mut self,
        settings: ShapeSettings,
        fill: u8,
    ) -> Result<MeshShape, Error> {
        if settings.span == 0
            || settings.span > 16000
            || !settings.scalar220.is_finite()
            || settings.scalar220.abs() > 1e6
            || ![0, 0xcd].contains(&fill)
        {
            return Err(Error("invalid mesh shape settings"));
        }
        let points: Vec<_> = (0..self.points).map(|i| self.point(i).unwrap()).collect();
        let mut center = [0.; 4];
        for p in &points {
            for j in 0..4 {
                center[j] = p[j] + center[j];
            }
        }
        let reciprocal = 1. / self.points as f32;
        center = center.map(|x| reciprocal * x);
        let planes = 0x60 + self.points * 48;
        for i in 0..self.faces {
            let tri = std::array::from_fn(|j| word(&self.bytes, planes + i * 32 + 16 + j * 4));
            let n = cross(&points, tri);
            let sq = squared(n);
            let seed = spawn::reference_seed(sq, false)?;
            let factor = (1. - (seed * sq) * seed) * (seed * 0.5) + seed;
            let mut normal = n.map(|x| factor * x);
            let delta: [f32; 4] = std::array::from_fn(|j| points[tri[0] as usize][j] - center[j]);
            let dot = (delta[0] * normal[0] + delta[1] * normal[1])
                + (delta[2] * normal[2] + delta[3] * normal[3]);
            if dot < 0. {
                normal = normal.map(|x| -1. * x);
            }
            if !dot.is_finite() || normal.iter().any(|x| !x.is_finite()) {
                return Err(Error("mesh shape normal exceeds range"));
            }
            for (j, x) in normal.iter().enumerate() {
                put(&mut self.bytes, planes + i * 32 + j * 4, x.to_bits());
            }
        }
        let mut shape = [fill; 0x250];
        shape[0x19] = settings.layer;
        shape[0xc0] = u8::from(settings.flag_c0);
        put(&mut shape, 0x220, settings.scalar220.to_bits());
        // Canonical owned mesh token in shape+mesh diagnostic representation.
        put(&mut shape, 0x240, 0x250);
        put(&mut shape, 0x244, align16(settings.span as usize) as u32);
        Ok(MeshShape { shape, mesh: self })
    }
}

/// Complete selected B460F0/B23D70 construction with an owned empty world.
/// Tokens identify only the diagnostic ownership graph; no allocation/registration.
pub struct ConstructedShape {
    prefix: [u8; 0x500],
    mesh: Mesh,
}
impl ConstructedShape {
    pub fn prefix_bytes(&self) -> &[u8] {
        &self.prefix
    }
    pub fn mesh(&self) -> &Mesh {
        &self.mesh
    }
}

impl Mesh {
    pub fn construct_shape(
        self,
        descriptor: [u8; 0x84],
        controls: [u32; 5],
        fill: u8,
    ) -> Result<ConstructedShape, Error> {
        let mut d = descriptor;
        if word(&d, 0) != 0
            || word(&d, 0x5c) != 0
            || controls[0] > 1000000
            || controls[1] > 1
            || controls[4] > 1000000
            || (0x10..0x50).step_by(4).any(|o| {
                let v = f32::from_bits(word(&d, o));
                !v.is_finite() || v.abs() > 10000.
            })
            || [0x58, 0x60, 0x68].iter().any(|&o| {
                let v = f32::from_bits(word(&d, o));
                !v.is_finite() || v.abs() > 1e6
            })
            || [0x50, 0x51, 0x52, 0x53, 0x54, 0x55, 0x71, 0x79, 0x7a]
                .iter()
                .any(|&o| d[o] > 1)
        {
            return Err(Error("invalid mesh shape constructor settings"));
        }
        let updated = self.initialize_shape(
            ShapeSettings {
                span: word(&d, 4),
                scalar220: f32::from_bits(word(&d, 0x68)),
                layer: d[0x80],
                flag_c0: d[0x54] != 0,
            },
            fill,
        )?;
        let mut p = [fill; 0x500];
        put(&mut p, 0, 0x7000);
        put(&mut p, 0x10, 7);
        for o in [0x231, 0x216, 0x215, 0x217, 0x218, 0x16, 0x229, 0x230, 0x60] {
            p[o] = 0;
        }
        p[0x15] = 1;
        p[0x228] = 1;
        for o in [0x184, 0x188, 0xec, 0xe8, 0x21c, 0x224, 0x22c] {
            put(&mut p, o, 0);
        }
        put(&mut p, 0x198, u32::MAX);
        for r in 0..4 {
            for c in 0..4 {
                put(
                    &mut p,
                    0x70 + r * 16 + c * 4,
                    if r == c { 1f32.to_bits() } else { 0 },
                );
            }
        }
        for j in 0..4 {
            put(&mut p, 0xb0 + j * 4, 0x1070 + j as u32 * 16);
        }
        p[0x229] = d[0x71];
        put(&mut p, 0x22c, word(&d, 0x74));
        p[0x230] = d[0x78];
        p[0x216] = d[0x7a];
        for r in 0..3 {
            put(&mut d, 0x1c + r * 16, 0);
        }
        put(&mut d, 0x4c, 1f32.to_bits());
        for (dst, src) in [
            (0x17, 0x52),
            (0x18, 0x53),
            (0xc0, 0x54),
            (0xc1, 0x55),
            (0x228, 0x70),
            (0x15, 0x79),
            (0x190, 0x64),
            (0x19, 0x80),
            (0x1a, 0x81),
            (0x14, 0x51),
        ] {
            p[dst] = d[src];
        }
        for (dst, src) in [(0xc4, 0x58), (0x224, 0x6c), (0x220, 0x68), (0x18c, 0x60)] {
            put(&mut p, dst, word(&d, src));
        }
        put(&mut p, 0xcc, 0x12e0);
        let material = if controls[1] == 0 {
            0
        } else if word(&d, 0x7c) == controls[3] && word(&d, 0x7c) != controls[2] {
            0x1440
        } else {
            0x1420
        };
        put(&mut p, 0xc8, material);
        put(&mut p, 0x194, controls[0]);
        for (o, value) in [
            (0x170, 0x1130),
            (0x174, 0x1130),
            (0x178, 0x1140),
            (0x17c, 0x1150),
            (0x180, 0x1160),
        ] {
            put(&mut p, o, value);
        }
        if d[0x50] == 0 {
            p[0x214] = 0;
        } else {
            p[0xf0..0x130].copy_from_slice(&d[0x10..0x50]);
            for r in 0..4 {
                for c in 0..4 {
                    put(
                        &mut p,
                        0x130 + r * 16 + c * 4,
                        if r == c { 1f32.to_bits() } else { 0 },
                    );
                }
            }
            p[0x214] = 1;
        }
        for i in 0..6 {
            let o = 0x19c + i * 20;
            put(&mut p, o, 0);
            put(&mut p, o + 4, 0);
            put(&mut p, o + 8, 0x1000);
            put(&mut p, o + 12, 0);
            p[o + 16] = u8::from(i % 2 == 0);
        }
        for o in (0x20..0x40).chain(0xd0..0xe8).step_by(4) {
            put(&mut p, o, 0);
        }
        p[0x19] = updated.shape[0x19];
        p[0xc0] = updated.shape[0xc0];
        put(&mut p, 0x220, word(&updated.shape, 0x220));
        put(&mut p, 0x240, 0x1500);
        put(&mut p, 0x244, word(&updated.shape, 0x244));
        // Complete original wrapper/backlink, retaining its final eight fill bytes.
        put(&mut p, 0x21c, 0x13e0);
        for (o, value) in [
            (0x3e0, 0x7001),
            (0x3e4, controls[4]),
            (0x3e8, 0x1000),
            (0x3ec, 0),
            (0x3f0, 0),
            (0x3f4, 0x1000),
        ] {
            put(&mut p, o, value);
        }
        p[0x2e0..0x3e0].fill(0);
        put(&mut p, 0x2f4, 0x1400);
        put(&mut p, 0x360, controls[0] + 1);
        p[0x400..0x460].fill(0);
        put(&mut p, 0x404, 0x1420);
        put(&mut p, 0x408, 2);
        put(&mut p, 0x40c, 0x20);
        p[0x410] = controls[1] as u8;
        put(&mut p, 0x420, controls[2]);
        put(&mut p, 0x440, controls[3]);
        put(&mut d, 0, 0x1500);
        put(&mut d, 0x5c, 0x12e0);
        p[0x250..0x2d4].copy_from_slice(&d);
        Ok(ConstructedShape {
            prefix: p,
            mesh: updated.mesh,
        })
    }
}

/// Complete selected cold physical construction then first shape attachment.
/// One cell, empty endpoint lists, supplied diagonal inertia and skip-wake=true.
/// This does not generate geometry pairs or advance the physical world.
pub struct AttachedShape {
    shape: ConstructedShape,
    body: crate::physical_constructor::Output,
    root: [u8; 0xa80],
    cells: [u8; 16],
}
impl AttachedShape {
    pub fn shape(&self) -> &ConstructedShape {
        &self.shape
    }
    pub fn body(&self) -> &crate::physical_constructor::Output {
        &self.body
    }
    pub fn root_bytes(&self) -> &[u8] {
        &self.root
    }
    pub fn cell_bytes(&self) -> &[u8] {
        &self.cells
    }

    /// Register a static instance of the same mesh against this first dynamic shape.
    /// The original bounds/endpoint/pair path is selected; no narrow phase or tick.
    pub fn register_static_clone(
        &self,
        shift: [f32; 4],
        mask: u32,
        pair_capacity: bool,
    ) -> Result<MeshOverlap, Error> {
        if shift.iter().any(|x| !x.is_finite() || x.abs() > 10000.) || shift[3] != 0. {
            return Err(Error("invalid static mesh translation"));
        }
        let mut descriptor: [u8; 0x84] = self.shape.prefix[0x250..0x2d4].try_into().unwrap();
        put(&mut descriptor, 0, 0);
        put(&mut descriptor, 0x5c, 0);
        descriptor[0x50] = 0;
        let mesh = Mesh {
            bytes: self.shape.mesh.bytes.clone(),
            points: self.shape.mesh.points,
            faces: self.shape.mesh.faces,
        };
        let id = word(&self.shape.prefix, 0x194) + 1;
        let mut second = mesh.construct_shape(descriptor, [id, 0, 0, 0, 0], 0xcd)?;
        let mut pose = std::array::from_fn(|r| {
            std::array::from_fn(|c| {
                f32::from_bits(word(&self.shape.prefix, 0x130 + r * 16 + c * 4))
            })
        });
        for j in 0..3 {
            pose[3][j] += shift[j];
        }
        for (i, v) in pose.into_iter().flatten().enumerate() {
            put(&mut second.prefix, 0x130 + i * 4, v.to_bits());
            put(&mut second.prefix, 0x70 + i * 4, v.to_bits());
        }
        update_bounds(&mut second, pose)?;
        let mut first: [u8; 0x250] = self.shape.prefix[..0x250].try_into().unwrap();
        put(&mut first, 0xc8, 0);
        put(&mut first, 0x21c, 0);
        put(&mut second.prefix, 0x21c, 0);
        put(&mut second.prefix, 0x198, 1);
        for o in [
            0xb0, 0xb4, 0xb8, 0xbc, 0x170, 0x174, 0x178, 0x17c, 0x180, 0x1a4, 0x1b8, 0x1cc, 0x1e0,
            0x1f4, 0x208,
        ] {
            let token = word(&second.prefix, o);
            put(&mut second.prefix, o, token + 0xb000);
        }
        let mut root = self.root;
        put(&mut root, 0x80, id + 1);
        for i in 0..32 {
            put(&mut root, 0x974 + i * 4, mask);
        }
        let mut cells = self.cells;
        put(&mut cells, 4, 0xc000);
        let mut pair = [0; 0x50];
        put(&mut root, 0x914, if pair_capacity { 0xd000 } else { 0 });
        for axis in 0..3 {
            // Stable equal-coordinate ordering: minima before maxima, older
            // same-kind endpoints before newly inserted endpoints.
            let mut nodes = [(0usize, false), (0, true), (1, false), (1, true)];
            let coordinate = |(owner, maximum): (usize, bool)| {
                let shape = if owner == 0 {
                    &first[..]
                } else {
                    &second.prefix[..]
                };
                f32::from_bits(word(shape, 0x20 + usize::from(maximum) * 16 + axis * 4))
            };
            nodes.sort_by(|&a, &b| {
                coordinate(a)
                    .partial_cmp(&coordinate(b))
                    .unwrap()
                    .then_with(|| a.1.cmp(&b.1))
                    .then_with(|| a.0.cmp(&b.0))
            });
            let token = |(owner, maximum): (usize, bool)| {
                (if owner == 0 { 0x1000 } else { 0xc000 })
                    + 0x19c
                    + axis as u32 * 40
                    + u32::from(maximum) * 20
            };
            put(&mut root, 0x900 + axis * 4, token(nodes[0]));
            for (i, &(owner, maximum)) in nodes.iter().enumerate() {
                let shape = if owner == 0 {
                    &mut first[..]
                } else {
                    &mut second.prefix[..]
                };
                let offset = 0x19c + axis * 40 + usize::from(maximum) * 20;
                put(shape, offset, nodes.get(i + 1).map_or(0, |&n| token(n)));
                put(
                    shape,
                    offset + 4,
                    i.checked_sub(1).map_or(0, |j| token(nodes[j])),
                );
                let bound = word(shape, 0x20 + usize::from(maximum) * 16 + axis * 4);
                put(shape, offset + 12, bound);
            }
        }
        let overlap = (0..3).all(|j| {
            f32::from_bits(word(&first, 0x20 + j * 4))
                <= f32::from_bits(word(&second.prefix, 0x30 + j * 4))
                && f32::from_bits(word(&second.prefix, 0x20 + j * 4))
                    <= f32::from_bits(word(&first, 0x30 + j * 4))
        });
        if pair_capacity && mask & (1 << second.prefix[0x19]) != 0 && overlap {
            put(&mut root, 0x914, 0);
            put(&mut root, 0x910, 0xd000);
            put(&mut first, 0xd0, 0xd000);
            put(&mut second.prefix, 0xd4, 0xd000);
            put(&mut pair, 0x28, 0x1000);
            put(&mut pair, 0x2c, 0xc000);
        }
        Ok(MeshOverlap {
            first,
            second: second.prefix[..0x250].try_into().unwrap(),
            root,
            cells,
            pair,
        })
    }
}

/// Retained two-shape broad-phase ownership graph, without contact generation.
pub struct MeshOverlap {
    pub first: [u8; 0x250],
    pub second: [u8; 0x250],
    pub root: [u8; 0xa80],
    pub cells: [u8; 16],
    pub pair: [u8; 0x50],
}

impl Mesh {
    pub fn construct_and_attach(
        self,
        descriptor: [u8; 0x84],
        controls: [u32; 5],
        physical_settings: [u32; 88],
        fill: u8,
    ) -> Result<AttachedShape, Error> {
        if descriptor[0x80] > 31 || f32::from_bits(word(&descriptor, 0x68)) < 0. {
            return Err(Error("invalid physical shape layer or margin"));
        }
        let mut body = crate::physical_constructor::initialize(&physical_settings, 0, fill)?;
        let mut shape = self.construct_shape(descriptor, controls, fill)?;
        let published = std::array::from_fn(|r| {
            std::array::from_fn(|c| f32::from_bits(body.body[(0x1d0 + r * 16 + c * 4) / 4]))
        });
        let pose = if shape.prefix[0x214] == 0 {
            published
        } else {
            let local = std::array::from_fn(|r| {
                std::array::from_fn(|c| f32::from_bits(word(&shape.prefix, 0xf0 + r * 16 + c * 4)))
            });
            crate::physical_pose::multiply(local, published)
        };
        for (i, v) in pose.into_iter().flatten().enumerate() {
            put(&mut shape.prefix, 0x130 + i * 4, v.to_bits());
            put(&mut shape.prefix, 0x70 + i * 4, v.to_bits());
        }
        update_bounds(&mut shape, pose)?;
        put(&mut shape.prefix, 0xd8, 0xa000);
        put(&mut shape.prefix, 0xe0, 0);
        put(&mut shape.prefix, 0x198, 1);
        let mut root = [0; 0xa80];
        root[..0x100].copy_from_slice(&shape.prefix[0x2e0..0x3e0]);
        for (o, v) in [
            (0x940, 1000f32.to_bits()),
            (0x944, 1000f32.to_bits()),
            (0x948, 1),
            (0x94c, 1),
            (0x950, 0xb000),
            (0x954, 0xb008),
        ] {
            put(&mut root, o, v);
        }
        for j in 0..3 {
            let start = 0x19c + j * 40;
            let end = start + 20;
            put(&mut shape.prefix, start, 0x1000 + end as u32);
            put(&mut shape.prefix, start + 4, 0);
            put(&mut shape.prefix, end, 0);
            put(&mut shape.prefix, end + 4, 0x1000 + start as u32);
            let low = word(&shape.prefix, 0x20 + j * 4);
            let high = word(&shape.prefix, 0x30 + j * 4);
            put(&mut shape.prefix, start + 12, low);
            put(&mut shape.prefix, end + 12, high);
            put(&mut root, 0x900 + j * 4, 0x1000 + start as u32);
        }
        body.body[0x328 / 4] = 0x1000;
        body.body[0x32c / 4] = 0x9000;
        for (i, v) in body.world.iter_mut().enumerate() {
            if i * 4 != 0x60 && *v != 0 {
                *v = 0xa000;
            }
        }
        body.world[0x18 / 4] = 0x12e0;
        let mut cells = [0; 16];
        put(&mut cells, 12, 0x1000);
        Ok(AttachedShape {
            shape,
            body,
            root,
            cells,
        })
    }
}

// Original kind7 bounds reduction; the supplied publication is already checked.
fn update_bounds(
    shape: &mut ConstructedShape,
    pose: crate::physical_pose::Matrix,
) -> Result<(), Error> {
    let margin = f32::from_bits(word(&shape.prefix, 0x220));
    let [low, high] = shape.mesh.world_bounds(pose, margin)?;
    for j in 0..4 {
        put(&mut shape.prefix, 0x20 + j * 4, low[j].to_bits());
        put(&mut shape.prefix, 0x30 + j * 4, high[j].to_bits());
    }
    Ok(())
}

impl Mesh {
    /// Current transformed mesh bounds, including the selected shape margin.
    #[allow(clippy::assign_op_pattern)] // Preserve the original expansion operand order.
    pub fn world_bounds(
        &self,
        pose: crate::physical_pose::Matrix,
        margin: f32,
    ) -> Result<[[f32; 4]; 2], Error> {
        if !margin.is_finite()
            || !(0. ..=1e6).contains(&margin)
            || pose
                .iter()
                .flatten()
                .any(|x| !x.is_finite() || x.abs() > 1e5)
        {
            return Err(Error("invalid mesh bounds controls"));
        }
        let mut lows = [[f32::MAX, f32::MAX, f32::MAX, 1.]; 4];
        let mut highs = [[-f32::MAX, -f32::MAX, -f32::MAX, 1.]; 4];
        // Original four independent accumulators, with remaining vertices in slot0.
        let grouped = self.points / 4 * 4;
        for i in 0..self.points {
            let v = self.point(i).unwrap();
            let slot = if i < grouped { i % 4 } else { 0 };
            let transformed: [f32; 4] = std::array::from_fn(|j| {
                (v[2] * pose[2][j] + v[0] * pose[0][j]) + (v[3] * pose[3][j] + v[1] * pose[1][j])
            });
            for j in 0..4 {
                if lows[slot][j] >= transformed[j] {
                    lows[slot][j] = transformed[j];
                }
                if highs[slot][j] <= transformed[j] {
                    highs[slot][j] = transformed[j];
                }
            }
        }
        let mut low = lows[0];
        let mut high = highs[0];
        for i in 1..4 {
            for j in 0..4 {
                if low[j] >= lows[i][j] {
                    low[j] = lows[i][j];
                }
                if high[j] <= highs[i][j] {
                    high[j] = highs[i][j];
                }
            }
        }
        for j in 0..4 {
            let expansion = if j < 3 { margin } else { 0. };
            low[j] -= expansion;
            high[j] = expansion + high[j];
        }
        if low.iter().chain(&high).any(|x| !x.is_finite()) || (0..3).any(|j| low[j] > high[j]) {
            return Err(Error("invalid physical shape bounds"));
        }
        Ok([low, high])
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

/// Whole selected deformation-active source adjustment. The floor is reduced
/// from the raw input before sampling, rather than from the deformed vertices.
pub fn prepare_deformed_points(
    points: &[[f32; 4]],
    offset: [f32; 4],
    vertical_extents: [f32; 2],
    grid: &crate::deformation_grid::Grid<'_>,
) -> Result<Vec<[f32; 4]>, Error> {
    // Reuse the deformation-off input admission without consuming its output.
    prepare_points(points, offset, vertical_extents)?;
    let mut floor = points[0][1];
    for p in &points[1..] {
        if p[1] < floor {
            floor = p[1];
        }
    }
    let center = offset[1] + (vertical_extents[1] + vertical_extents[0]) * 0.5;
    let mut result = Vec::with_capacity(points.len());
    for point in points {
        let deformed = grid.sample(*point)?;
        let y = if deformed[1] < floor {
            floor
        } else {
            deformed[1]
        };
        let p = [
            (offset[0] + deformed[0]) - 0.,
            (offset[1] + y) - center,
            (offset[2] + deformed[2]) - 0.,
            1.,
        ];
        if p.iter().any(|x| !x.is_finite() || x.abs() > 100000.) {
            return Err(Error("deformed collision adjustment exceeds range"));
        }
        result.push(p);
    }
    Ok(result)
}

pub fn build_from_deformed_source(
    points: &[[f32; 4]],
    triangles: &[[u32; 3]],
    offset: [f32; 4],
    vertical_extents: [f32; 2],
    grid: &crate::deformation_grid::Grid<'_>,
    fill: u8,
) -> Result<Mesh, Error> {
    build(
        &prepare_deformed_points(points, offset, vertical_extents, grid)?,
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
    fn active_source_floor_is_raw_before_sampling() {
        let points = [
            [-1., -1., -1., 1.],
            [1., -1., -1., 1.],
            [0., 1., -1., 1.],
            [0., 0., 1., 1.],
        ];
        let mut positions = Vec::new();
        for z in 0..2 {
            for y in 0..2 {
                for x in 0..2 {
                    positions.push([
                        x as f32 * 2. - 1.,
                        y as f32 * 2. - 1.5,
                        z as f32 * 2. - 1.,
                        1.,
                    ]);
                }
            }
        }
        let grid = crate::deformation_grid::Grid {
            dimensions: [2; 3],
            spacing: 2.,
            minimum: [-1., -1., -1., 1.],
            positions: &positions,
        };
        let adjusted = prepare_deformed_points(&points, [0.; 4], [-1., 1.], &grid).unwrap();
        assert_eq!(adjusted[0][1], -1.);
        assert_eq!(adjusted[1][1], -1.);
        assert!((adjusted[2][1] - 0.5).abs() < 1e-6);
        assert_eq!(points[2][1], 1.);
    }
    #[test]
    fn first_attachment_composes_bounds_and_preserves_supplied_inertia_without_waking() {
        let points = [
            [1., 1., 1., 1.],
            [-1., -1., 1., 1.],
            [-1., 1., -1., 1.],
            [1., -1., -1., 1.],
        ];
        let faces = [[0, 1, 2], [0, 3, 1], [0, 2, 3], [1, 3, 2]];
        let mut descriptor = [0; 0x84];
        put(&mut descriptor, 4, 1);
        for i in 0..4 {
            put(&mut descriptor, 0x10 + i * 20, 1f32.to_bits());
        }
        descriptor[0x50] = 1;
        put(&mut descriptor, 0x44, 2f32.to_bits());
        put(&mut descriptor, 0x68, 0.25f32.to_bits());
        let mut settings = [0; 88];
        settings[0] = 1000f32.to_bits();
        for (i, x) in [1000f32, 2000., 3000., 1.].into_iter().enumerate() {
            settings[(0x10 + i * 20) / 4] = x.to_bits();
            settings[(0xf0 + i * 20) / 4] = 1f32.to_bits();
        }
        settings[0x60 / 4] = 1;
        settings[0x130 / 4] = 1; // Published pose, inactive body.
        settings[0x120 / 4] = 10f32.to_bits();
        settings[0x128 / 4] = (-4f32).to_bits();
        let cold = crate::physical_constructor::initialize(&settings, 0, 0xcd).unwrap();
        let attached = build(&points, &faces, 0xcd)
            .unwrap()
            .construct_and_attach(descriptor, [0; 5], settings, 0xcd)
            .unwrap();
        let p = attached.shape().prefix_bytes();
        let bounds: [f32; 8] = std::array::from_fn(|i| f32::from_bits(word(p, 0x20 + i * 4)));
        assert_eq!(bounds, [8.75, 0.75, -5.25, 1., 11.25, 3.25, -2.75, 1.]);
        assert_eq!(attached.body().body[0x400 / 4], u32::MAX);
        assert_eq!(
            &attached.body().body[0x90 / 4..0x190 / 4],
            &cold.body[0x90 / 4..0x190 / 4]
        );
        assert_eq!(attached.body().body[0x328 / 4], 0x1000);
        assert_eq!(word(p, 0xd8), 0xa000);
        assert_eq!(word(attached.cell_bytes(), 12), 0x1000);
        assert_eq!(word(attached.root_bytes(), 0x900), 0x119c);
        assert_eq!(word(p, 0x19c), 0x11b0);
        assert_eq!(word(p, 0x1b4), 0x119c);
        assert_eq!(settings[0x64 / 4], 0);
        // Inclusive AABB contact: a new minimum precedes an old maximum at
        // the shared face. Capacity and filtering independently suppress pairs.
        let touch = attached
            .register_static_clone([2.5, 0., 0., 0.], 1, true)
            .unwrap();
        assert_eq!(word(&touch.root, 0x910), 0xd000);
        assert_eq!(word(&touch.first, 0x19c), 0xc19c);
        assert_eq!(word(&touch.second, 0x19c), 0x11b0);
        assert_eq!(word(&touch.pair, 0x28), 0x1000);
        assert_eq!(word(&touch.pair, 0x2c), 0xc000);
        for (shift, mask, capacity) in [
            ([2.5001, 0., 0., 0.], 1, true),
            ([0.; 4], 0, true),
            ([0.; 4], 1, false),
        ] {
            let result = attached
                .register_static_clone(shift, mask, capacity)
                .unwrap();
            assert_eq!(word(&result.root, 0x910), 0);
            assert_eq!(result.pair, [0; 0x50]);
        }
        assert!(
            attached
                .register_static_clone([f32::NAN, 0., 0., 0.], 1, true)
                .is_err()
        );
        assert_eq!(word(attached.root_bytes(), 0x910), 0);
    }

    #[test]
    fn constructed_shape_has_owned_wrapper_material_identity_and_world_counter() {
        let p = [
            [1., 1., 1., 1.],
            [-1., -1., 1., 1.],
            [-1., 1., -1., 1.],
            [1., -1., -1., 1.],
        ];
        let t = [[0, 1, 2], [0, 3, 1], [0, 2, 3], [1, 3, 2]];
        let mut descriptor = [0; 0x84];
        put(&mut descriptor, 4, 17);
        for i in 0..16 {
            put(
                &mut descriptor,
                0x10 + i * 4,
                if i % 5 == 0 { 1f32.to_bits() } else { 0 },
            );
        }
        put(&mut descriptor, 0x1c, 2f32.to_bits());
        put(&mut descriptor, 0x7c, 8);
        let before = descriptor;
        let made = build(&p, &t, 0xcd)
            .unwrap()
            .construct_shape(descriptor, [41, 1, 7, 8, 23], 0xcd)
            .unwrap();
        assert_eq!(descriptor, before);
        let b = made.prefix_bytes();
        assert_eq!(word(b, 0x194), 41);
        assert_eq!(word(b, 0x360), 42);
        assert_eq!(word(b, 0xc8), 0x1440);
        assert_eq!(word(b, 0x21c), 0x13e0);
        assert_eq!(word(b, 0x3e8), 0x1000);
        assert_eq!(word(b, 0x3e4), 23);
        assert_eq!(&b[0xf0..0x170], &[0xcd; 0x80]);
        assert_eq!(word(b, 0x250 + 0x1c), 0);
        assert_eq!(word(b, 0x250 + 0x4c), 1f32.to_bits());
        assert_eq!(made.mesh().point_count(), 4);
        assert_eq!(made.mesh().face_count(), 4);
        for controls in [[41, 0, 7, 8, 23], [41, 1, 7, 9, 23]] {
            let m = build(&p, &t, 0)
                .unwrap()
                .construct_shape(descriptor, controls, 0)
                .unwrap();
            assert_eq!(
                word(m.prefix_bytes(), 0xc8),
                if controls[1] == 0 { 0 } else { 0x1420 }
            );
        }
        put(&mut descriptor, 0x5c, 1);
        assert!(
            build(&p, &t, 0)
                .unwrap()
                .construct_shape(descriptor, [41, 1, 7, 8, 23], 0)
                .is_err()
        );
    }

    #[test]
    fn shape_planes_enclose_a_convex_mesh_and_preserve_other_storage() {
        let p = [
            [1., 1., 1., 1.],
            [-1., -1., 1., 1.],
            [-1., 1., -1., 1.],
            [1., -1., -1., 1.],
        ];
        let t = [[0, 1, 2], [0, 3, 1], [0, 2, 3], [1, 3, 2]];
        let raw = build(&p, &t, 0xcd).unwrap();
        let before = raw.bytes().to_vec();
        let shape = raw
            .initialize_shape(
                ShapeSettings {
                    span: 17,
                    scalar220: -0.,
                    layer: 255,
                    flag_c0: true,
                },
                0xcd,
            )
            .unwrap();
        assert_eq!(word(shape.shape_bytes(), 0x244), 32);
        assert_eq!(word(shape.shape_bytes(), 0x220), (-0.0f32).to_bits());
        assert_eq!(shape.shape_bytes()[0x19], 255);
        assert_eq!(shape.shape_bytes()[0xc0], 1);
        assert_eq!(&shape.shape_bytes()[0x248..], &[0xcd; 8]);
        for (i, tri) in t.iter().enumerate() {
            let n = shape.mesh().face_normal(i).unwrap();
            let anchor = p[tri[0] as usize];
            for vertex in p {
                let distance = (0..3).map(|j| n[j] * (vertex[j] - anchor[j])).sum::<f32>();
                assert!(distance <= 1e-6);
            }
        }
        let planes = 0x60 + 4 * 48;
        for (i, &byte) in before.iter().enumerate() {
            if !(i >= planes && i < planes + 4 * 32 && (i - planes) % 32 < 16) {
                assert_eq!(shape.mesh().bytes()[i], byte);
            }
        }
        for settings in [
            ShapeSettings {
                span: 0,
                scalar220: 0.,
                layer: 0,
                flag_c0: false,
            },
            ShapeSettings {
                span: 1,
                scalar220: f32::INFINITY,
                layer: 0,
                flag_c0: false,
            },
        ] {
            assert!(
                build(&p, &t, 0)
                    .unwrap()
                    .initialize_shape(settings, 0)
                    .is_err()
            );
        }
    }

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
