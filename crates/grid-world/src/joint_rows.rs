//! The joint's solver rows: AF71C0 (two dynamic bodies) and AF9430 (one),
//! with the per-degree-of-freedom writer AEF780, the compliance B82550 and
//! the impulse bounds AFCBF0. Rows are the 0xC0-byte rows the contact rows
//! use (`contact_rows::Row`): words 0..15 the linear and angular Jacobians of
//! both bodies, 32 bias, 33 softness, 34/35 lower/upper impulse, 37 the
//! accumulated impulse, 40/41 the bodies' solver indices, 45 a batch length.
//! B7ACC0 initialises every row before (bias 0, the island's softness,
//! -FLT_MAX..FLT_MAX, bodies -1); words a joint does not write keep their
//! values. Research: docs/JOINTS_RESEARCH.md.
use crate::{
    contact_groups::Body,
    contact_rows::Row,
    joint::{
        Bodies, Joint, V, acos, bvec, clamp_unit, compose, cross, dot, inverse_rows, negate,
        normalize, pose, product, rotation, rows, swing_axis,
    },
};

const MAX: f32 = f32::MAX;

/// The part of B7ACC0's row context the joint rows use, with the pool
/// cursors as row indices.
#[derive(Clone, Debug, PartialEq)]
pub struct Context {
    /// +0x04, the inverse step the bias scales by.
    pub inverse_step: f32,
    /// +0x08, the island's error reduction.
    pub reduction: f32,
    /// +0x14, the step the impulse bounds scale by.
    pub step: f32,
    /// +0x24 / +0x28: rows in the current two-body / one-body batch.
    pub two_batch: u32,
    pub one_batch: u32,
    /// +0x34: rows written.
    pub total: u32,
    /// +0x38 / +0x40: next one-body / two-body row.
    pub one_cursor: usize,
    pub two_cursor: usize,
    /// +0x48 / +0x50: the current batch's first row.
    pub one_head: Option<usize>,
    pub two_head: Option<usize>,
}

/// Rows of both pools.
pub struct Pools<'a> {
    pub one: &'a mut [Row],
    pub two: &'a mut [Row],
}

fn rf(r: &Row, w: usize) -> f32 {
    f32::from_bits(r[w])
}
fn set(r: &mut Row, w: usize, v: f32) {
    r[w] = v.to_bits();
}
fn set_vec(r: &mut Row, w: usize, v: V) {
    for (k, x) in v.into_iter().enumerate() {
        r[w + k] = x.to_bits();
    }
}
fn neg(v: V) -> V {
    v.map(|x| f32::from_bits(x.to_bits() ^ 0x8000_0000))
}
#[allow(clippy::neg_multiply)] // Original multiplies by -1.0.
fn minus(v: V) -> V {
    v.map(|x| -1. * x)
}

/// B82550: error reduction and softness from spring stiffness `k` and damping
/// `c` (both already times the step); FLT_MAX or both zero keep the defaults.
fn compliance(k: f32, c: f32, reduction: f32, softness: f32, inverse_step: f32) -> (f32, f32) {
    if k != MAX && c != MAX && (k != 0. || c != 0.) {
        let k = (1. / inverse_step) * k;
        let f = 1. / (k + c);
        (f * k, f)
    } else {
        (reduction, softness)
    }
}

/// AFCBF0: impulse bounds from a break impulse.
fn bounds(row: &mut Row, limit: f32, ctx: &Context) {
    if limit != MAX {
        set(row, 34, negate(ctx.inverse_step * ctx.step * limit));
        set(row, 35, ctx.inverse_step * ctx.step * limit);
    }
}

/// Takes the next row of the pool the joint's bodies select.
fn next<'a>(two_body: bool, ctx: &mut Context, pools: &'a mut Pools<'_>) -> &'a mut Row {
    ctx.total += 1;
    if two_body {
        let i = ctx.two_cursor;
        ctx.two_cursor += 1;
        ctx.two_batch += 1;
        &mut pools.two[i]
    } else {
        let i = ctx.one_cursor;
        ctx.one_cursor += 1;
        ctx.one_batch += 1;
        &mut pools.one[i]
    }
}

/// AEF780: the limit (part 0) and spring (part 1) rows of one degree of
/// freedom whose state is at joint offset `dof`.
#[allow(clippy::too_many_arguments)]
fn dof_rows(
    j: &Joint,
    dof: usize,
    jacobians: [V; 4],
    two_body: bool,
    ctx: &mut Context,
    pools: &mut Pools<'_>,
    limit: f32,
) {
    let [lin_a, lin_b, ang_a, ang_b] = jacobians;
    for part in 0..2 {
        if j.byte(dof + 2 + part) == 0 {
            continue;
        }
        let row = next(two_body, ctx, pools);
        let reduction = if part == 1 {
            let (reduction, softness) = compliance(
                j.get(dof + 0x20) * ctx.step,
                j.get(dof + 0x24) * ctx.step,
                ctx.reduction,
                rf(row, 33),
                ctx.inverse_step,
            );
            set(row, 33, softness);
            reduction
        } else {
            ctx.reduction
        };
        set(
            row,
            32,
            ctx.inverse_step * reduction * j.get(dof + 8 + 4 * part),
        );
        if limit != MAX {
            set(row, 34, negate(ctx.inverse_step * ctx.step * limit));
            set(row, 35, ctx.inverse_step * ctx.step * limit);
        }
        if j.byte(dof + 4 + part) != 0 {
            row[34] = 0;
        }
        if j.byte(dof + 6 + part) != 0 {
            row[35] = 0;
        }
        set_vec(row, 0, lin_a);
        set_vec(row, 4, ang_a);
        row[40] = j.words[0x1c / 4];
        row[37] = 0;
        if two_body {
            set_vec(row, 8, lin_b);
            set_vec(row, 12, ang_b);
            row[41] = j.words[0x20 / 4];
        }
    }
}

/// The row of a friction or extra angular row: `axis` on body A, its
/// negation on B.
fn angular_row(row: &mut Row, j: &Joint, axis: V, two_body: bool) {
    row[0..4].fill(0);
    set_vec(row, 4, axis);
    row[40] = j.words[0x1c / 4];
    if two_body {
        row[8..12].fill(0);
        set_vec(row, 12, neg(axis));
        row[41] = j.words[0x20 / 4];
    }
}

/// Batch bookkeeping at entry: a joint with rows starts a new batch when its
/// bodies differ from the current batch's.
fn batch(j: &Joint, two_body: bool, ctx: &mut Context, pools: &mut Pools<'_>) {
    if j.words[0x18 / 4] == 0 {
        return;
    }
    if two_body {
        match ctx.two_head {
            None => ctx.two_head = Some(ctx.two_cursor),
            Some(h) => {
                let head = &mut pools.two[h];
                if j.words[0x1c / 4] != head[40] || j.words[0x20 / 4] != head[41] {
                    head[45] = ctx.two_batch;
                    ctx.two_batch = 0;
                    ctx.two_head = Some(ctx.two_cursor);
                }
            }
        }
    } else {
        match ctx.one_head {
            None => ctx.one_head = Some(ctx.one_cursor),
            Some(h) => {
                let head = &mut pools.one[h];
                if j.words[0x1c / 4] != head[40] {
                    head[45] = ctx.one_batch;
                    ctx.one_batch = 0;
                    ctx.one_head = Some(ctx.one_cursor);
                }
            }
        }
    }
}

/// Body centre (body +0x334 points at its +0x1C0 position).
fn centre(b: &Body) -> V {
    bvec(b, 0x1c0)
}

fn diff(a: V, b: V) -> V {
    std::array::from_fn(|k| a[k] - b[k])
}

/// The point joint's row (point branch, one row when the distance passes
/// the rest length): along the anchor separation.
fn point_row(
    j: &Joint,
    bodies: &Bodies<'_>,
    two_body: bool,
    ctx: &mut Context,
    pools: &mut Pools<'_>,
) {
    let distance = j.get(0x244);
    let r = crate::sse_approx::rcp(distance);
    let d = diff(j.vec(0x100), j.vec(0xe0));
    let n: V = std::array::from_fn(|k| {
        let x = r * d[k];
        (1. - r * distance) * x + x
    });
    let ra = diff(j.vec(0xe0), centre(bodies.a));
    let ang_a = cross(ra, n);
    let row = next(two_body, ctx, pools);
    set_vec(row, 0, n);
    set_vec(row, 4, ang_a);
    if two_body {
        let rb = diff(j.vec(0x100), centre(bodies.b.expect("two-body joint")));
        set_vec(row, 8, neg(n));
        set_vec(row, 12, cross(n, rb));
        row[41] = j.words[0x20 / 4];
    }
    row[40] = j.words[0x1c / 4];
    row[37] = 0;
    set(
        row,
        32,
        ctx.reduction * ctx.inverse_step * (distance - j.get(0x240)),
    );
    set(row, 35, MAX);
    set(row, 34, 0.);
}

/// The friction rows' angles and axes for a plane joint (two rows) or the
/// others (three).
fn friction(j: &Joint, f: &[V; 4], t: &[V; 4]) -> (Vec<V>, Vec<f32>) {
    if j.words[0x25c / 4] == 1 {
        let mut u = normalize(cross(f[2], t[0]));
        let mut w = normalize(cross(u, f[2]));
        let d = (w[1] * t[0][1] + t[0][0] * w[0]) + (w[3] * t[0][3] + t[0][2] * w[2]);
        let mut c = d;
        if d < 0. {
            c = negate(d);
            w = minus(w);
            u = minus(u);
        }
        if 1. < c {
            c = 1.;
        }
        let a = acos(c);
        let mut rot = rotation(u, a);
        let turned = product(w, &rot);
        let test = (turned[1] * t[0][1] + t[0][0] * turned[0])
            + (turned[3] * t[0][3] + t[0][2] * turned[2]);
        if test < c {
            rot = rotation(u, negate(a));
        }
        let v = product(f[2], &rot);
        let c2 = clamp_unit((v[1] * t[2][1] + t[2][0] * v[0]) + (v[3] * t[2][3] + t[2][2] * v[2]));
        let mut swing = negate(acos(c2));
        if 0. < dot(t[1], v) {
            swing = negate(swing);
        }
        let rel = compose(f, &inverse_rows(t));
        let q = swing_axis(rel[2]);
        let c3 = clamp_unit(dot(rel[1], q));
        let mut twist = negate(acos(c3));
        if dot(rel[0], q) < 0. {
            twist = negate(twist);
        }
        (vec![w, f[2]], vec![swing, twist])
    } else {
        let c = clamp_unit(
            (f[2][1] * t[2][1] + f[2][0] * t[2][0]) + (f[2][3] * t[2][3] + f[2][2] * t[2][2]),
        );
        let a = negate(acos(c));
        let axis = if f32::from_bits(0x3f7f_be77) <= c.abs() {
            let seed: V = std::array::from_fn(|k| 10. * t[2][k] - 10. * f[2][k]);
            let axis = cross(seed, t[2]);
            let l = crate::joint::length(dot(axis, axis));
            if f32::from_bits(0x3a83_126f) <= l {
                let inverse = 1. / l;
                axis.map(|x| inverse * x)
            } else {
                t[0]
            }
        } else {
            normalize(cross(t[2], f[2]))
        };
        let axis2 = normalize(cross(f[2], axis));
        let rel = compose(f, &inverse_rows(t));
        let q = swing_axis(rel[2]);
        let c3 = clamp_unit(dot(rel[1], q));
        let mut twist = negate(acos(c3));
        if dot(rel[0], q) < 0. {
            twist = negate(twist);
        }
        (vec![axis, axis2, f[2]], vec![a, 0., twist])
    }
}

/// AF71C0 (`two_body`) or AF9430: the joint's rows after `prepare`.
pub fn write(
    j: &Joint,
    bodies: &Bodies<'_>,
    two_body: bool,
    ctx: &mut Context,
    pools: &mut Pools<'_>,
) {
    batch(j, two_body, ctx, pools);
    if j.byte(0x32e) != 0 {
        if j.words[0x18 / 4] == 1 {
            point_row(j, bodies, two_body, ctx, pools);
        }
        return;
    }
    let ma = pose(bodies.a);
    if 0. < j.get(0x250) {
        let f = compose(&rows(j, 0x180), &ma);
        let target = rows(j, 0x200);
        let t = if two_body {
            compose(&target, &pose(bodies.b.expect("two-body joint")))
        } else {
            target
        };
        let (axes, angles) = friction(j, &f, &t);
        for (axis, angle) in axes.into_iter().zip(angles) {
            let row = next(two_body, ctx, pools);
            angular_row(row, j, axis, two_body);
            row[37] = 0;
            set(row, 35, ctx.step * j.get(0x258));
            set(row, 34, negate(ctx.step * j.get(0x258)));
            set(row, 32, ctx.reduction * ctx.inverse_step * angle);
        }
    }
    let zero = [0.; 4];
    match j.words[0x25c / 4] {
        1 => {
            let n = j.vec(0x110);
            dof_rows(
                j,
                0x2dc,
                [zero, zero, n, minus(n)],
                two_body,
                ctx,
                pools,
                j.get(0x24c),
            );
            let row = next(two_body, ctx, pools);
            set(row, 32, ctx.reduction * ctx.inverse_step * j.get(0x240));
            bounds(row, j.get(0x24c), ctx);
            angular_row(row, j, j.vec(0x120), two_body);
            row[37] = 0;
        }
        2 => {
            let n = j.vec(0x110);
            dof_rows(
                j,
                0x2dc,
                [zero, zero, n, minus(n)],
                two_body,
                ctx,
                pools,
                j.get(0x24c),
            );
            if j.get(0x244) != 0. || j.byte(0x32c) != 0 {
                let row = next(two_body, ctx, pools);
                if j.byte(0x32c) == 0 {
                    let (_, softness) = compliance(
                        0.,
                        ctx.step * j.get(0x244),
                        ctx.reduction,
                        rf(row, 33),
                        ctx.inverse_step,
                    );
                    set(row, 33, softness);
                }
                row[32] = 0;
                bounds(row, j.get(0x24c), ctx);
                angular_row(row, j, j.vec(0x120), two_body);
                row[37] = 0;
            }
        }
        _ => {}
    }
    if j.words[0x260 / 4] == 1 {
        let n = j.vec(0x130);
        dof_rows(
            j,
            0x304,
            [zero, zero, n, minus(n)],
            two_body,
            ctx,
            pools,
            j.get(0x24c),
        );
    }
    let active = (0..3).any(|k| j.byte(0x266 + 0x28 * k) != 0 || j.byte(0x267 + 0x28 * k) != 0);
    if active {
        let ra: V = std::array::from_fn(|k| {
            (j.get(0xe0 + 4 * k) - centre(bodies.a)[k]) + j.get(0x60 + 4 * k)
        });
        let rb = if two_body {
            diff(j.vec(0x100), centre(bodies.b.expect("two-body joint")))
        } else {
            zero
        };
        for k in 0..3 {
            let axis = j.vec(0xb0 + 0x10 * k);
            let ang_b = cross(axis, rb);
            let ang_a = cross(ra, axis);
            dof_rows(
                j,
                0x264 + 0x28 * k,
                [axis, minus(axis), ang_a, ang_b],
                two_body,
                ctx,
                pools,
                j.get(0x248),
            );
        }
    }
}
