//! The original's six-degree-of-freedom joint ("NeMouse", vtable D3ADFC):
//! constructor AEFEB0 over a world descriptor (built by A74670 from a
//! TEMPLATECONSTRAINT and its props). The joint is kept as the original's
//! 0x330-byte object, so the row stages (AF4890, AF71C0/AF9430) can read it
//! word for word; pointer words (vtable, bodies, world, list links) are not
//! part of the port's state. Research: docs/JOINTS_RESEARCH.md.
use crate::contact_groups::Body;

pub(crate) type V = [f32; 4];

/// The descriptor 0xA74670 hands to the constructor (0x168 bytes; words 0..3
/// are the world and body pointers and the fixed-body flag).
pub const DESCRIPTOR: usize = 0x168;
/// The joint object (0xA75EA0 allocates 0x330).
pub const SIZE: usize = 0x330;

/// The joint's words. Bytes the original leaves untouched keep the caller's
/// fill.
#[derive(Clone)]
pub struct Joint {
    pub words: [u32; SIZE / 4],
}

fn f(w: u32) -> f32 {
    f32::from_bits(w)
}

impl Joint {
    pub fn filled(byte: u8) -> Self {
        Self {
            words: [u32::from_ne_bytes([byte; 4]); SIZE / 4],
        }
    }
    pub(crate) fn get(&self, o: usize) -> f32 {
        f(self.words[o / 4])
    }
    pub(crate) fn set(&mut self, o: usize, v: f32) {
        self.words[o / 4] = v.to_bits();
    }
    pub(crate) fn vec(&self, o: usize) -> V {
        std::array::from_fn(|j| self.get(o + 4 * j))
    }
    pub(crate) fn set_vec(&mut self, o: usize, v: V) {
        for (j, x) in v.into_iter().enumerate() {
            self.set(o + 4 * j, x);
        }
    }
    pub(crate) fn set_byte(&mut self, o: usize, v: u8) {
        let mut b = self.words[o / 4].to_le_bytes();
        b[o % 4] = v;
        self.words[o / 4] = u32::from_le_bytes(b);
    }
    pub fn byte(&self, o: usize) -> u8 {
        self.words[o / 4].to_le_bytes()[o % 4]
    }
}

fn dw(d: &[u32], o: usize) -> u32 {
    d[o / 4]
}
fn df(d: &[u32], o: usize) -> f32 {
    f(d[o / 4])
}
fn dvec(d: &[u32], o: usize) -> V {
    std::array::from_fn(|j| df(d, o + 4 * j))
}
fn dbyte(d: &[u32], o: usize) -> u8 {
    d[o / 4].to_le_bytes()[o % 4]
}
pub(crate) fn bf(b: &Body, o: usize) -> f32 {
    f(b[o / 4])
}
pub(crate) fn bvec(b: &Body, o: usize) -> V {
    std::array::from_fn(|j| bf(b, o + 4 * j))
}

/// SUBSS -0.0, x: the negation, but a NaN operand comes back unchanged
/// (quieted) where Rust's `-0.0 - x` may flip its sign.
pub(crate) fn negate(x: f32) -> f32 {
    if x.is_nan() {
        f32::from_bits(x.to_bits() | 0x0040_0000)
    } else {
        -x
    }
}

/// 0x67C200.
pub(crate) fn cross(a: V, b: V) -> V {
    [
        a[1] * b[2] - a[2] * b[1],
        b[0] * a[2] - b[2] * a[0],
        b[1] * a[0] - a[1] * b[0],
        0.,
    ]
}

/// RSQRTPS with one Newton step: v x ((1 - (r s) r) r 0.5 + r), s the four-lane
/// square sum (x² + y²) + (z² + w²).
pub(crate) fn normalize(v: V) -> V {
    let s = (v[0] * v[0] + v[1] * v[1]) + (v[2] * v[2] + v[3] * v[3]);
    let r = crate::sse_approx::rsqrt(s);
    let factor = (1. - r * s * r) * r * 0.5 + r;
    v.map(|x| factor * x)
}

/// Inverse of a rigid pose (rows 0..2 rotation, row 3 position): the
/// transposed rotation, and `-p` through it with lane 3 = 1.
fn inverse(b: &Body) -> [V; 4] {
    let r0 = bvec(b, 0x190);
    let r1 = bvec(b, 0x1a0);
    let r2 = bvec(b, 0x1b0);
    let p = bvec(b, 0x1c0);
    let i0 = [r0[0], r1[0], r2[0], 0.];
    let i1 = [r0[1], r1[1], r2[1], 0.];
    let i2 = [r0[2], r1[2], r2[2], 0.];
    let (nx, ny, nz) = (-p[0], -p[1], -p[2]);
    let mut i3: V = std::array::from_fn(|k| (ny * i1[k] + nx * i0[k]) + nz * i2[k]);
    i3[3] = 1.;
    [i0, i1, i2, i3]
}

/// Row times matrix, as the inlined SSE product adds it:
/// (x I0 + z I2) + (w I3 + y I1).
pub(crate) fn product(m: V, i: &[V; 4]) -> V {
    let [x, y, z, w] = m;
    std::array::from_fn(|k| (x * i[0][k] + z * i[2][k]) + (w * i[3][k] + y * i[1][k]))
}

/// Rebuilds the frame at `o` (rows o, o+0x10, o+0x20): third = N(first x
/// second), second = N(third x first), first = N(first).
fn orthonormalize(j: &mut Joint, o: usize) {
    let third = normalize(cross(j.vec(o), j.vec(o + 0x10)));
    j.set_vec(o + 0x20, third);
    let second = normalize(cross(j.vec(o + 0x20), j.vec(o)));
    j.set_vec(o + 0x10, second);
    let first = normalize(j.vec(o));
    j.set_vec(o, first);
}

/// The two bodies: `b` is dynamic unless `fixed` (descriptor byte 0xC).
pub struct Bodies<'a> {
    pub a: &'a Body,
    pub b: Option<&'a Body>,
    pub fixed: bool,
}

impl Bodies<'_> {
    pub(crate) fn dynamic_b(&self) -> Option<&Body> {
        if self.fixed { None } else { self.b }
    }
}

/// AEFEB0 with B825E0/B827F0's non-pointer fields. `descriptor` is updated
/// in place as the original does (lane masks of its frames). `world_7d0` is
/// the world's +0x7D0 word the base copies to +0x4C.
pub fn construct(
    joint: &mut Joint,
    descriptor: &mut [u32; DESCRIPTOR / 4],
    bodies: &Bodies<'_>,
    world_7d0: u32,
) {
    let d = descriptor;
    // B827F0 / B825E0.
    joint.words[0x1c / 4] = u32::MAX;
    joint.words[0x20 / 4] = u32::MAX;
    joint.set_byte(0xc, dbyte(&d[..], 0xc));
    joint.set_byte(0x24, dbyte(&d[..], 0xd));
    for o in [0x10, 0x14, 0x18, 0x2c, 0x30, 0x38, 0x40, 0x50] {
        joint.words[o / 4] = 0;
    }
    joint.set_byte(0x48, 0);
    joint.words[0x4c / 4] = world_7d0;
    // AEFEB0.
    joint.set_byte(0xf, 0);
    let point = dbyte(&d[..], 0x164);
    joint.set_byte(0x32e, point);
    if point != 0 {
        construct_point(joint, d, bodies);
        return;
    }
    for k in 0..12 {
        let o = 0x10 + k * 0x10 + 0xc;
        d[o / 4] = if k % 4 == 3 { 1f32.to_bits() } else { 0 };
    }
    let inv_a = inverse(bodies.a);
    let rows = [(0x70, 0x10), (0x80, 0x20), (0x90, 0x30), (0xa0, 0x40)];
    for (to, from) in rows {
        joint.set_vec(to, product(dvec(&d[..], from), &inv_a));
    }
    let rows = [(0x180, 0x50), (0x190, 0x60), (0x1a0, 0x70), (0x1b0, 0x80)];
    for (to, from) in rows {
        joint.set_vec(to, product(dvec(&d[..], from), &inv_a));
    }
    let last;
    if let Some(b) = bodies.dynamic_b() {
        let inv_b = inverse(b);
        joint.set_vec(0xf0, product(dvec(&d[..], 0x40), &inv_b));
        joint.set_vec(0x100, [0., 0., 0., 1.]);
        joint.set_vec(0x1c0, product(dvec(&d[..], 0x90), &inv_b));
        joint.set_vec(0x1d0, product(dvec(&d[..], 0xa0), &inv_b));
        joint.set_vec(0x1e0, product(dvec(&d[..], 0xb0), &inv_b));
        last = product(dvec(&d[..], 0xc0), &inv_b);
    } else {
        let p = dvec(&d[..], 0x40);
        joint.set_vec(0xf0, p);
        joint.set_vec(0x100, p);
        joint.set_vec(0x1c0, dvec(&d[..], 0x90));
        joint.set_vec(0x1d0, dvec(&d[..], 0xa0));
        joint.set_vec(0x1e0, dvec(&d[..], 0xb0));
        last = dvec(&d[..], 0xc0);
    }
    joint.set_vec(0x1f0, last);
    orthonormalize(joint, 0x70);
    orthonormalize(joint, 0x180);
    orthonormalize(joint, 0x1c0);
    for o in (0x1c0..0x200).step_by(4) {
        joint.words[(o + 0x40) / 4] = joint.words[o / 4];
    }
    let copies = [
        (0x274, 0xe0),
        (0x27c, 0xd0),
        (0x278, 0x100),
        (0x280, 0xf0),
        (0x284, 0x110),
        (0x288, 0x11c),
        (0x29c, 0xe4),
        (0x2a4, 0xd4),
        (0x2a0, 0x104),
        (0x2a8, 0xf4),
        (0x2ac, 0x114),
        (0x2b0, 0x120),
        (0x2c4, 0xe8),
        (0x2cc, 0xd8),
        (0x2c8, 0x108),
        (0x2d0, 0xf8),
        (0x2d4, 0x118),
        (0x2d8, 0x124),
        (0x2ec, 0x12c),
    ];
    for (to, from) in copies {
        joint.words[to / 4] = dw(&d[..], from);
    }
    joint.set(0x2f4, negate(df(&d[..], 0x12c)));
    joint.words[0x2f0 / 4] = dw(&d[..], 0x130);
    joint.set(0x2f8, negate(df(&d[..], 0x130)));
    for (to, from) in [
        (0x2fc, 0x134),
        (0x300, 0x138),
        (0x25c, 0x128),
        (0x244, 0x13c),
        (0x260, 0x140),
        (0x314, 0x144),
    ] {
        joint.words[to / 4] = dw(&d[..], from);
    }
    joint.set(0x31c, negate(df(&d[..], 0x144)));
    joint.words[0x318 / 4] = dw(&d[..], 0x148);
    joint.set(0x320, negate(df(&d[..], 0x148)));
    for (to, from) in [
        (0x324, 0x14c),
        (0x328, 0x150),
        (0x248, 0x154),
        (0x24c, 0x158),
    ] {
        joint.words[to / 4] = dw(&d[..], from);
    }
    joint.set_vec(0x60, [0., 0., 0., 1.]);
    for o in (0x110..0x140).step_by(4) {
        joint.words[o / 4] = 0;
    }
    for (k, o) in (0xb0..0xf0).step_by(4).enumerate() {
        joint.set(o, if k % 5 == 0 { 1. } else { 0. });
    }
    joint.words[0x240 / 4] = 0;
    for o in (0x140..0x180).step_by(4) {
        joint.words[o / 4] = 0;
    }
    joint.set_byte(0x32c, 0);
    joint.set_byte(0x32d, 0);
    let stiffness = df(&d[..], 0x15c);
    joint.set(0x250, stiffness);
    if 0. < stiffness {
        for o in [0x2fc, 0x300, 0x324, 0x328, 0x244] {
            joint.words[o / 4] = 0;
        }
    }
    joint.words[0x254 / 4] = dw(&d[..], 0x160);
}

/// AEFEB0's point branch (descriptor byte 0x164 set): anchors relative to
/// the bodies' mass centres (+0x230) in body space and their distance.
fn construct_point(joint: &mut Joint, d: &[u32; DESCRIPTOR / 4], bodies: &Bodies<'_>) {
    for o in (0x140..0x180).step_by(4) {
        joint.words[o / 4] = 0;
    }
    joint.words[0x254 / 4] = 0;
    let centre = |b: &Body, v: V| -> V {
        let c = bvec(b, 0x230);
        std::array::from_fn(|k| v[k] - c[k])
    };
    let a_anchor = centre(bodies.a, dvec(&d[..], 0xd0));
    joint.set_vec(0xa0, a_anchor);
    let mut b_anchor = dvec(&d[..], 0xe0);
    if let Some(b) = bodies.dynamic_b() {
        b_anchor = centre(b, b_anchor);
    }
    joint.set_vec(0xf0, b_anchor);
    let rotate = |b: &Body, v: V| -> V {
        std::array::from_fn(|k| {
            (v[3] * bf(b, 0x1c0 + 4 * k) + v[1] * bf(b, 0x1a0 + 4 * k))
                + (v[2] * bf(b, 0x1b0 + 4 * k) + v[0] * bf(b, 0x190 + 4 * k))
        })
    };
    let a_world = rotate(bodies.a, a_anchor);
    joint.set_vec(0xe0, a_world);
    let b_world = match bodies.dynamic_b() {
        Some(b) => rotate(b, b_anchor),
        None => b_anchor,
    };
    joint.set_vec(0x100, b_world);
    let delta: V = std::array::from_fn(|k| a_world[k] - b_world[k]);
    let s =
        (delta[3] * delta[3] + delta[2] * delta[2]) + (delta[1] * delta[1] + delta[0] * delta[0]);
    let length = if s == 0. {
        0.
    } else {
        let r = crate::sse_approx::rsqrt(s);
        let product = r * s;
        (1. - product * r) * product * 0.5 + product
    };
    joint.set(0x240, length);
}

/// SSE horizontal dot product: (p0 + p1) + (p2 + p3).
pub(crate) fn dot(a: V, b: V) -> f32 {
    (a[0] * b[0] + a[1] * b[1]) + (a[2] * b[2] + a[3] * b[3])
}

pub(crate) fn rows(j: &Joint, o: usize) -> [V; 4] {
    std::array::from_fn(|k| j.vec(o + 0x10 * k))
}

pub(crate) fn pose(b: &Body) -> [V; 4] {
    [
        bvec(b, 0x190),
        bvec(b, 0x1a0),
        bvec(b, 0x1b0),
        bvec(b, 0x1c0),
    ]
}

/// Length by RSQRTPS: p = r s, ((1 - p r) p) 0.5 + p; zero for s = 0.
pub(crate) fn length(s: f32) -> f32 {
    if s == 0. {
        return 0.;
    }
    let r = crate::sse_approx::rsqrt(s);
    let p = r * s;
    (1. - p * r) * p * 0.5 + p
}

/// Inverse of a rigid frame given as rows (rotation rows 0..2, position 3).
pub(crate) fn inverse_rows(g: &[V; 4]) -> [V; 4] {
    let i0 = [g[0][0], g[1][0], g[2][0], 0.];
    let i1 = [g[0][1], g[1][1], g[2][1], 0.];
    let i2 = [g[0][2], g[1][2], g[2][2], 0.];
    let (nx, ny, nz) = (-g[3][0], -g[3][1], -g[3][2]);
    let mut i3: V = std::array::from_fn(|k| (ny * i1[k] + nx * i0[k]) + nz * i2[k]);
    i3[3] = 1.;
    [i0, i1, i2, i3]
}

fn count_row(j: &mut Joint) {
    j.words[0x14 / 4] = j.words[0x14 / 4].wrapping_add(1);
    j.words[0x18 / 4] = j.words[0x18 / 4].wrapping_add(1);
}

/// One degree of freedom's limit and spring state (AEED10): flags at
/// bytes 0..7, the limit and spring errors at +8/+0xC, upper and lower limit
/// at +0x10/+0x18, spring band at +0x14/+0x1C, spring coefficients at
/// +0x20/+0x24. Each active part adds a row (joint +0x14 and +0x18).
// Negated compares keep the original's unordered (NaN) outcomes.
#[allow(clippy::neg_cmp_op_on_partial_ord, clippy::needless_late_init)]
fn limit(j: &mut Joint, o: usize, value: f32) {
    let upper = j.get(o + 0x10);
    for b in 0..8 {
        j.set_byte(o + b, 0);
    }
    j.words[(o + 8) / 4] = 0;
    j.words[(o + 0xc) / 4] = 0;
    let mut hit = false;
    if value < upper {
        let lower = j.get(o + 0x18);
        if !(lower < value) {
            j.set_byte(o + 2, 1);
            j.set_byte(o, 0);
            j.set(o + 8, value - lower);
            if upper != lower {
                j.set_byte(o + 6, 1);
            }
            hit = true;
        }
    } else {
        j.set_byte(o + 2, 1);
        j.set_byte(o, 1);
        j.set(o + 8, value - upper);
        if upper != j.get(o + 0x18) {
            j.set_byte(o + 4, 1);
        }
        hit = true;
    }
    if hit {
        count_row(j);
    }
    let k = j.get(o + 0x20);
    if k != 0. || j.get(o + 0x24) != 0. {
        let band;
        let high = j.get(o + 0x14);
        if value <= high {
            let low = j.get(o + 0x1c);
            if low <= value {
                return;
            }
            if j.byte(o + 2) != 0 && k == 0. {
                return;
            }
            j.set_byte(o + 1, 0);
            band = low;
        } else {
            if j.byte(o + 2) != 0 && k == 0. {
                return;
            }
            j.set_byte(o + 1, 1);
            band = high;
        }
        j.set_byte(o + 3, 1);
        j.set(o + 0xc, value - band);
        count_row(j);
    }
}

/// CRT acos (0xBF4067) on the x87 double, rounded to the f32 store.
pub(crate) fn acos(c: f32) -> f32 {
    f64::from(c).acos() as f32
}

/// 0x772670 (the verified `pose::rotation`) as rows.
pub(crate) fn rotation(axis: V, angle: f32) -> [V; 4] {
    crate::pose::rotation(axis, angle).unwrap_or([[0.; 4]; 4])
}

/// The original's compare-and-select (a NaN passes through).
#[allow(clippy::manual_clamp)]
pub(crate) fn clamp_unit(c: f32) -> f32 {
    if 1. < c {
        1.
    } else if c < -1. {
        -1.
    } else {
        c
    }
}

/// AF1F10: the swing taking local z to `v` as a quaternion, applied to the
/// y axis; that rotated axis with lane 3 cleared.
pub(crate) fn swing_axis(v: V) -> V {
    let q = if f32::from_bits(0xbf7f_df3b) <= v[2] {
        let f = 1. / ((v[2] + 1.) * 2.).sqrt();
        [-v[1] * f, v[0] * f, 0., (v[2] + 1.) * f]
    } else {
        [1., 0., 0., 0.]
    };
    let [q0, q1, q2, q3] = q;
    let z2 = q2 + q2;
    [
        (q1 + q1) * q0 - q3 * z2,
        1. - (z2 * q2 + q0 * (q0 + q0)),
        q3 * (q0 + q0) + q1 * z2,
        0.,
    ]
}

/// Row-wise `rows x m`.
pub(crate) fn compose(left: &[V; 4], m: &[V; 4]) -> [V; 4] {
    std::array::from_fn(|k| product(left[k], m))
}

/// The limit edge a re-seat turns back to: 0.8 of the upper or lower limit
/// at `o` when `value` lies beyond it.
fn reseat_edge(j: &Joint, upper: usize, lower: usize, value: f32) -> Option<f32> {
    let mut edge = j.get(upper) * 0.8;
    let over = edge < value || {
        edge = j.get(lower) * 0.8;
        value < edge
    };
    (over && value - edge != 0.).then_some(value - edge)
}

/// AF4890: the joint's world frames, limit and spring states and row counts
/// for the current body poses (+0x14 rows, +0x18 rows of the first pass,
/// +0x54 the angular rows).
// Original multiplies by -1.0; negated compares keep NaN outcomes.
#[allow(
    clippy::neg_multiply,
    clippy::neg_cmp_op_on_partial_ord,
    clippy::needless_range_loop
)]
pub fn prepare(joint: &mut Joint, bodies: &Bodies<'_>) {
    let j = joint;
    let a = bodies.a;
    let ma = pose(a);
    let dynamic_b = bodies.dynamic_b();
    if j.byte(0x32e) != 0 {
        j.set_vec(0xe0, product(j.vec(0xa0), &ma));
        let pb = match dynamic_b {
            Some(b) => product(j.vec(0xf0), &pose(b)),
            None => j.vec(0xf0),
        };
        j.set_vec(0x100, pb);
        let d: V = std::array::from_fn(|k| j.get(0xe0 + 4 * k) - j.get(0x100 + 4 * k));
        let s = (d[3] * d[3] + d[2] * d[2]) + (d[1] * d[1] + d[0] * d[0]);
        let distance = length(s);
        j.set(0x244, distance);
        // COMISS/JB: an unordered compare counts as below.
        if distance >= j.get(0x240) {
            j.words[0x18 / 4] = 1;
            j.words[0x14 / 4] = 1;
        }
        return;
    }
    let fa = compose(&rows(j, 0x70), &ma);
    for (k, row) in fa.iter().enumerate() {
        j.set_vec(0xb0 + 0x10 * k, *row);
    }
    let f = compose(&rows(j, 0x180), &ma);
    let g: [V; 4];
    if let Some(b) = dynamic_b {
        let mb = pose(b);
        j.set_vec(0x100, product(j.vec(0xf0), &mb));
        g = compose(&rows(j, 0x1c0), &mb);
        if 0. < j.get(0x250) {
            let mut va = j.vec(0xa0);
            va[3] = 0.;
            let mut vb = j.vec(0xf0);
            vb[3] = 0.;
            let lb = length(dot(vb, vb));
            let la = length(dot(va, va));
            j.set(
                0x258,
                (bf(b, 0x3f0) + bf(a, 0x3f0)) * (la + lb) * j.get(0x250),
            );
        }
    } else {
        g = rows(j, 0x1c0);
        if 0. < j.get(0x250) {
            let mut va = j.vec(0xa0);
            va[3] = 0.;
            let la = length(dot(va, va));
            j.set(0x258, la * bf(a, 0x3f0) * j.get(0x250));
        }
    }
    let reseat = j.byte(0x32d) != 0;
    let mut r = [[0.; 4]; 4];
    if reseat {
        j.set_byte(0x32d, 0);
        r = [f[0], f[1], f[2], [0., 0., 0., 1.]];
    }
    j.set_vec(0x130, f[2]);
    let d: V = std::array::from_fn(|k| j.get(0x100 + 4 * k) - j.get(0xe0 + 4 * k));
    let mut e = [dot(d, fa[0]), dot(d, fa[1]), dot(d, fa[2]), 0.];
    j.words[0x14 / 4] = 0;
    j.words[0x18 / 4] = 0;
    for k in 0..3 {
        let o = 0x264 + 0x28 * k;
        limit(j, o, e[k]);
        // 0x4B4440 writes the offset back clamped by the limit error.
        e[k] -= j.get(o + 8);
    }
    let linear_rows = j.words[0x18 / 4];
    j.set_vec(0x60, product(e, &fa));
    match j.words[0x25c / 4] {
        1 => {
            let axis = normalize(cross(f[2], g[0]));
            j.set_vec(0x120, axis);
            let mut n = normalize(cross(j.vec(0x120), f[2]));
            j.set_vec(0x110, n);
            let dotted = (g[0][1] * n[1] + n[0] * g[0][0]) + (g[0][3] * n[3] + n[2] * g[0][2]);
            let mut c = dotted;
            if dotted < 0. {
                c = negate(dotted);
                n = n.map(|x| -1. * x);
                j.set_vec(0x110, n);
                let flipped = j.vec(0x120).map(|x| -1. * x);
                j.set_vec(0x120, flipped);
            }
            if 1. < c {
                c = 1.;
            }
            let mut angle = acos(c);
            j.set(0x240, angle);
            let axis = j.vec(0x120);
            let mut rot = rotation(axis, angle);
            let turned = product(n, &rot);
            let test = (turned[1] * g[0][1] + turned[0] * g[0][0])
                + (turned[3] * g[0][3] + turned[2] * g[0][2]);
            if test < c {
                angle = negate(j.get(0x240));
                j.set(0x240, angle);
                rot = rotation(axis, angle);
            }
            let v = product(f[2], &rot);
            let cosine =
                clamp_unit((v[1] * g[2][1] + v[0] * g[2][0]) + (v[3] * g[2][3] + v[2] * g[2][2]));
            let mut swing = negate(acos(cosine));
            let side = (v[1] * g[1][1] + v[0] * g[1][0]) + (v[3] * g[1][3] + v[2] * g[1][2]);
            if 0. < side {
                swing = negate(swing);
            }
            limit(j, 0x2dc, swing);
            count_row(j);
            if reseat && let Some(turn) = reseat_edge(j, 0x2ec, 0x2f4, swing) {
                let axis = product(j.vec(0x110), &rot);
                r = compose(&r, &rotation(axis, turn));
            }
        }
        2 => {
            let c = clamp_unit(
                (f[2][1] * g[2][1] + g[2][0] * f[2][0]) + (f[2][3] * g[2][3] + g[2][2] * f[2][2]),
            );
            let angle = negate(acos(c));
            if f32::from_bits(0x3f7f_be77) <= c.abs() {
                let seed: V = std::array::from_fn(|k| 10. * g[2][k] - 10. * f[2][k]);
                let axis = cross(seed, g[2]);
                j.set_vec(0x110, axis);
                let l = length(dot(axis, axis));
                if f32::from_bits(0x3a83_126f) <= l {
                    let inverse = 1. / l;
                    j.set_vec(0x110, axis.map(|x| inverse * x));
                } else {
                    j.set_vec(0x110, g[0]);
                }
            } else {
                j.set_vec(0x110, normalize(cross(g[2], f[2])));
            }
            j.set_vec(0x120, normalize(cross(f[2], j.vec(0x110))));
            limit(j, 0x2dc, angle);
            j.set_byte(0x32c, u8::from(!(0. < j.get(0x2ec) - j.get(0x2f4))));
            if j.get(0x244) != 0. || j.byte(0x32c) != 0 {
                count_row(j);
            }
            if reseat && let Some(turn) = reseat_edge(j, 0x2ec, 0x2f4, angle) {
                r = compose(&r, &rotation(j.vec(0x110), turn));
            }
        }
        _ => {}
    }
    if j.words[0x260 / 4] == 1 {
        let t = compose(&f, &inverse_rows(&g));
        let q = swing_axis(t[2]);
        let c = clamp_unit(dot(t[1], q));
        let mut twist = negate(acos(c));
        if dot(t[0], q) < 0. {
            twist = negate(twist);
        }
        limit(j, 0x304, twist);
        if reseat && let Some(turn) = reseat_edge(j, 0x314, 0x31c, twist) {
            r = compose(&r, &rotation(r[2], turn));
        }
    }
    if reseat {
        let n = compose(&r, &inverse_rows(&g));
        let l = rows(j, 0x1c0);
        for (k, row) in n.iter().enumerate() {
            j.set_vec(0x200 + 0x10 * k, product(*row, &l));
        }
    }
    if 0. < j.get(0x250) {
        let extra = if j.words[0x25c / 4] == 1 { 2 } else { 3 };
        j.words[0x18 / 4] = j.words[0x18 / 4].wrapping_add(extra);
        j.words[0x14 / 4] = j.words[0x14 / 4].wrapping_add(extra);
    }
    j.words[0x54 / 4] = j.words[0x18 / 4].wrapping_sub(linear_rows);
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn body_at(p: [f32; 3]) -> Body {
        let mut b = [0u32; 0x424 / 4];
        for (k, row) in [
            [1., 0., 0., 0.],
            [0., 1., 0., 0.],
            [0., 0., 1., 0.],
            [p[0], p[1], p[2], 1.],
        ]
        .into_iter()
        .enumerate()
        {
            for (i, v) in row.into_iter().enumerate() {
                b[0x190 / 4 + k * 4 + i] = f32::to_bits(v);
            }
        }
        b
    }

    /// A descriptor whose three frames are the identity at `at`, with
    /// linear limits `lower..upper` on every axis.
    pub(crate) fn descriptor(at: [f32; 3], lower: f32, upper: f32) -> [u32; DESCRIPTOR / 4] {
        let mut d = [0u32; DESCRIPTOR / 4];
        for frame in 0..3 {
            for k in 0..4 {
                let row: V = if k == 3 {
                    [at[0], at[1], at[2], 1.]
                } else {
                    std::array::from_fn(|i| if i == k { 1. } else { 0. })
                };
                for (i, v) in row.into_iter().enumerate() {
                    d[(0x10 + frame * 0x40 + k * 0x10) / 4 + i] = v.to_bits();
                }
            }
        }
        for axis in 0..3 {
            d[0xd0 / 4 + axis] = lower.to_bits();
            d[0xe0 / 4 + axis] = upper.to_bits();
        }
        d[0x154 / 4] = f32::MAX.to_bits();
        d[0x158 / 4] = f32::MAX.to_bits();
        d
    }

    #[test]
    fn frames_go_into_each_body_space() {
        let (a, b) = (body_at([1., 2., 3.]), body_at([1., 2., 5.]));
        let mut d = descriptor([1., 2., 4.], 0., 0.);
        let mut j = Joint::filled(0xcd);
        let bodies = Bodies {
            a: &a,
            b: Some(&b),
            fixed: false,
        };
        construct(&mut j, &mut d, &bodies, 0);
        // RSQRTPS normalisation: one ulp short of 1.
        let x = j.vec(0x70);
        assert!(
            (x[0] - 1.).abs() < 1e-6 && x[1] == 0. && x[2] == 0.,
            "{x:?}"
        );
        assert_eq!(j.vec(0xa0), [0., 0., 1., 1.]);
        assert_eq!(j.vec(0xf0), [0., 0., -1., 1.]);
        assert_eq!(j.words[0x1c / 4], u32::MAX);
    }

    /// A locked linear joint pulled apart along x gets one limit row per
    /// axis that is off its limit (here x only past the upper limit).
    #[test]
    fn a_stretched_lock_limits_the_stretched_axis() {
        let (a, b) = (body_at([0., 0., 0.]), body_at([2., 0., 0.]));
        let mut d = descriptor([1., 0., 0.], -0.05, 0.05);
        let mut j = Joint::filled(0);
        let bodies = Bodies {
            a: &a,
            b: Some(&b),
            fixed: false,
        };
        construct(&mut j, &mut d, &bodies, 0);
        let moved = body_at([2.5, 0., 0.]);
        let bodies = Bodies {
            a: &a,
            b: Some(&moved),
            fixed: false,
        };
        prepare(&mut j, &bodies);
        // x: 0.5 past the upper limit; y and z inside.
        assert_eq!(j.byte(0x264 + 2), 1);
        assert_eq!(j.byte(0x264), 1);
        assert!((j.get(0x264 + 8) - 0.45).abs() < 1e-6);
        assert_eq!(j.byte(0x28c + 2), 0);
        assert_eq!(j.words[0x14 / 4], 1);
        // Two-body rows: one row along x with the limit error as bias.
        let mut one = [[0u32; 48]; 4];
        let mut two = [[0u32; 48]; 4];
        let mut ctx = crate::joint_rows::Context {
            inverse_step: 60.,
            reduction: 0.2,
            step: 1. / 60.,
            impulse_scale: 1.,
            two_batch: 0,
            one_batch: 0,
            total: 0,
            one_cursor: 0,
            two_cursor: 0,
            one_head: None,
            two_head: None,
        };
        crate::joint_rows::write(
            &j,
            &bodies,
            true,
            &mut ctx,
            &mut crate::joint_rows::Pools {
                one: &mut one,
                two: &mut two,
            },
        );
        assert_eq!((ctx.total, ctx.two_cursor, ctx.two_head), (1, 1, Some(0)));
        let axis = two[0][0..4]
            .iter()
            .map(|&w| f32::from_bits(w))
            .collect::<Vec<_>>();
        assert!(
            (axis[0] - 1.).abs() < 1e-6 && axis[1] == 0. && axis[2] == 0.,
            "{axis:?}"
        );
        assert!((f32::from_bits(two[0][32]) - 60. * 0.2 * 0.45).abs() < 1e-4);
    }
}
