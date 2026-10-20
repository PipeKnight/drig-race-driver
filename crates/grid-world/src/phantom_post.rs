//! The close-drift flag's phantom post (docs/JOINTS_RESEARCH.md, "Close-drift
//! flags: the phantom post"): a pole segment the cars' boxes push aside.
//!
//! Ported from the original's post geometry in SSE lane order: the
//! segment-against-box separating-axis test 0x7BBB70, the closest points of
//! a segment and a box 0x7BC370 and the post push 0x7BCDA0. Verified against
//! the original in-process (tools/validate_phantom_post.py, FPP1).
use crate::sse_approx::{rcp, rsqrt};

pub type V4 = [f32; 4];

/// The post record's geometry (0x7BB910 writes it from the joint's pivot and
/// the pole body's centre).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Post {
    /// +0x10 / +0x20: AABB of the pivot and the tip.
    pub low: V4,
    pub high: V4,
    /// +0x30: the joint's world point.
    pub pivot: V4,
    /// +0x40: pivot + 2 x (centre - pivot).
    pub tip: V4,
    /// +0x50: the pole body's centre.
    pub centre: V4,
    /// +0x60: unit pivot-to-centre direction.
    pub direction: V4,
    /// +0x70: |centre - pivot|, the segment's half length.
    pub length: f32,
    /// +0x74: set when a push moved the post.
    pub hit: bool,
}

/// An oriented box: three axis rows and the centre row (a row-vector
/// matrix), and its half extents.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Box {
    pub axes: [V4; 3],
    pub centre: V4,
    pub half: V4,
}

/// Horizontal sum of the lane products in the SSE order (p3 + p2) + (p1 + p0).
fn dot(a: V4, b: V4) -> f32 {
    (a[3] * b[3] + a[2] * b[2]) + (a[1] * b[1] + a[0] * b[0])
}

fn sub(a: V4, b: V4) -> V4 {
    std::array::from_fn(|k| a[k] - b[k])
}

/// 0x67C200: the cross product, lane 3 zero.
fn cross(a: V4, b: V4) -> V4 {
    [
        a[1] * b[2] - a[2] * b[1],
        b[0] * a[2] - b[2] * a[0],
        b[1] * a[0] - a[1] * b[0],
        0.,
    ]
}

/// RSQRTPS with one Newton step towards the square root; zero stays zero.
fn root(n: f32) -> f32 {
    if n == 0. {
        return 0.;
    }
    let r = rsqrt(n);
    let s = r * n;
    (1. - s * r) * s * 0.5 + s
}

/// RCPPS with one Newton step, applied to `v / d`.
fn divide(v: V4, d: f32) -> V4 {
    let q = rcp(d);
    std::array::from_fn(|k| {
        let x = q * v[k];
        (1. - q * d) * x + x
    })
}

/// 0x7D8690: separation of [lo, hi] and [-r, r]; the smaller push out when
/// they overlap.
fn interval(lo: f32, hi: f32, low: f32, high: f32) -> Option<f32> {
    if lo <= high && low <= hi {
        Some(if (lo - high).abs() < (hi - low).abs() {
            lo - high
        } else {
            hi - low
        })
    } else {
        None
    }
}

/// 0x7BBB70: the segment (centre, direction, half length) against the box
/// over the box's three axes and their cross products with the segment;
/// on overlap the push of least magnitude along its axis.
pub fn penetration(post: &Post, b: &Box) -> Option<V4> {
    let d = sub(post.centre, b.centre);
    let mut depth = [0f32; 6];
    for (k, &axis) in b.axes.iter().enumerate() {
        let distance = dot(axis, d);
        let reach = dot(axis, post.direction).abs() * post.length;
        let (lo, hi) = (distance - reach, distance + reach);
        let (h, low) = (b.half[k], -b.half[k]);
        if h < lo || hi < low {
            return None;
        }
        let (a, c) = (hi - low, lo - h);
        depth[k] = if a.abs() <= c.abs() { a } else { c };
    }
    let mut axes = [[0f32; 4]; 3];
    for k in 0..3 {
        let c = cross(b.axes[k], post.direction);
        axes[k] = c;
        let n = (c[3] * c[3] + c[2] * c[2]) + (c[1] * c[1] + c[0] * c[0]);
        let length = root(n);
        if length <= f32::from_bits(0x3727_c5ac) {
            depth[3 + k] = depth[k] * 2.;
            continue;
        }
        let axis = divide(c, length);
        axes[k] = axis;
        let distance = dot(axis, d);
        let reach = dot(axis, post.direction).abs() * post.length;
        let e = [dot(axis, b.axes[0]), dot(axis, b.axes[1]), dot(axis, b.axes[2])];
        let radius = e[2].abs() * b.half[2] + b.half[0] * e[0].abs() + b.half[1] * e[1].abs();
        depth[3 + k] = interval(distance - reach, distance + reach, -radius, radius)?;
    }
    let mut best = depth[0];
    let mut index = usize::from(depth[1].abs() < depth[0].abs());
    if depth[1].abs() < depth[0].abs() {
        best = depth[1];
    }
    for (k, value) in depth.iter().enumerate().take(5).skip(2) {
        if value.abs() < best.abs() {
            index = k;
            best = *value;
        }
    }
    if best.abs() <= depth[5].abs() {
        depth[5] = best;
        if index < 3 {
            let s = -best;
            return Some(b.axes[index].map(|x| s * x));
        }
    } else {
        index = 5;
    }
    let s = -depth[5];
    Some(axes[index - 3].map(|x| s * x))
}

/// 0x7BC370: the closest points of the segment `a`..`b` and the box (on the
/// segment, on the box). In the box frame each axis is reflected so the
/// segment runs non-negative along it and classified against the faces;
/// the distance is a piecewise quadratic in the segment parameter, walked
/// from 0 through the face crossings until its slope turns non-negative.
pub fn closest(a: V4, b: V4, shape: &Box) -> (V4, V4) {
    let to_box = |v: V4| [dot(v, shape.axes[0]), dot(v, shape.axes[1]), dot(v, shape.axes[2])];
    let offset = sub(a, shape.centre);
    let segment = sub(b, a);
    let mut p = to_box(offset);
    let mut d = to_box(segment);
    let square = d.map(|x| x * x);
    let mut sign = [1f32; 3];
    // Face class per axis: -1 below the low face, 0 inside, 1 above the
    // high face; `t` the parameter of the next face crossing.
    let mut class = [0i8; 3];
    let mut t = [0f32; 3];
    let half = shape.half;
    for i in 0..3 {
        if d[i] < 0. {
            p[i] = -p[i];
            d[i] = -d[i];
            sign[i] = -1.;
        }
        t[i] = if d[i] <= 0. {
            class[i] = 0;
            2.
        } else if -half[i] <= p[i] {
            class[i] = if p[i] <= half[i] { 0 } else { 1 };
            (half[i] - p[i]) / d[i]
        } else {
            class[i] = -1;
            (-half[i] - p[i]) / d[i]
        };
    }
    let weight = |class: &[i8; 3]| -> [f32; 3] {
        std::array::from_fn(|i| if class[i] == 0 { 0. } else { square[i] })
    };
    let c = weight(&class);
    let mut slope = ((-0. - c[0] * t[0]) - c[1] * t[1]) - c[2] * t[2];
    let mut s = 0f32;
    if slope < 0. {
        let mut found = false;
        loop {
            let mut next = 1f32;
            if s < t[0] && t[0] < 1. {
                next = t[0];
            }
            if s < t[1] && t[1] < 1. && t[1] < next {
                next = t[1];
            }
            if s < t[2] && t[2] < 1. && t[2] < next {
                next = t[2];
            }
            let c = weight(&class);
            let at = (next - t[2]) * c[2] + ((next - t[1]) * c[1] + (next - t[0]) * c[0]);
            if at < 0. {
                for j in 0..3 {
                    if t[j] == next {
                        class[j] += 1;
                        t[j] = (half[j] - p[j]) / d[j];
                    }
                }
                s = next;
                slope = at;
            } else {
                s -= slope / ((at - slope) / (next - s));
                found = true;
            }
            if 1. <= s {
                if !found {
                    s = 1.;
                }
                break;
            }
            if found {
                break;
            }
        }
    }
    let on_segment = std::array::from_fn(|k| s * segment[k] + a[k]);
    let mut q = [0f32; 4];
    q[3] = 1.;
    for i in 0..3 {
        let x = (d[i] * s + p[i]) * sign[i];
        q[i] = if -half[i] <= x {
            if half[i] < x { half[i] } else { x }
        } else {
            -half[i]
        };
    }
    let [r0, r1, r2] = shape.axes;
    let r3 = shape.centre;
    let on_box =
        std::array::from_fn(|k| (q[2] * r2[k] + q[0] * r0[k]) + (q[1] * r1[k] + q[3] * r3[k]));
    (on_segment, on_box)
}

/// 0x7BCDA0: push the post out of the box, at most three passes; each turns
/// the post about its pivot towards the box point nearest its pushed copy.
pub fn push(post: &mut Post, shape: &Box) {
    for _ in 0..3 {
        let Some(v) = penetration(post, shape) else {
            return;
        };
        let a = std::array::from_fn(|k| 2. * v[k] + post.pivot[k]);
        let b = std::array::from_fn(|k| 2. * v[k] + post.tip[k]);
        let (on_segment, on_box) = closest(a, b, shape);
        let moved = sub(a, on_segment);
        if root(dot(moved, moved)) < f32::from_bits(0x3dcc_cccd) {
            return;
        }
        let v = sub(on_box, post.pivot);
        let n = dot(v, v);
        if root(n) < f32::from_bits(0x3727_c5ac) {
            return;
        }
        let r = rsqrt(n);
        let scale = (1. - r * n * r) * r * 0.5 + r;
        post.direction = v.map(|x| scale * x);
        let length = post.length;
        post.centre = std::array::from_fn(|k| length * post.direction[k] + post.pivot[k]);
        post.tip = std::array::from_fn(|k| length * post.direction[k] * 2. + post.pivot[k]);
        post.hit = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn upright() -> Post {
        // A flag pole: pivot on the ground, centre 1.465 m up.
        let length = 1.465;
        Post {
            low: [0., 0., 0., 0.],
            high: [0., 2. * length, 0., 0.],
            pivot: [0., 0., 0., 1.],
            tip: [0., 2. * length, 0., 1.],
            centre: [0., length, 0., 1.],
            direction: [0., 1., 0., 0.],
            length,
            hit: false,
        }
    }

    fn car(x: f32) -> Box {
        Box {
            axes: [[1., 0., 0., 0.], [0., 1., 0., 0.], [0., 0., 1., 0.]],
            centre: [x, 0.9, 0., 1.],
            half: [2.2, 0.6, 0.9, 0.],
        }
    }

    #[test]
    fn a_box_clear_of_the_pole_leaves_it() {
        let mut post = upright();
        assert!(penetration(&post, &car(3.)).is_none());
        push(&mut post, &car(3.));
        assert_eq!(post, upright());
    }

    #[test]
    fn a_box_over_the_pole_bends_it_away_about_the_pivot() {
        let mut post = upright();
        // The car's nose 0.4 m past the pole along +x, its floor 0.3 m up.
        let shape = car(-1.8);
        assert!(penetration(&post, &shape).is_some());
        push(&mut post, &shape);
        assert!(post.hit);
        assert_eq!(post.pivot, upright().pivot);
        // Still unit length and pointing up, now leaning along the push.
        let d = post.direction;
        assert!((d[0] * d[0] + d[1] * d[1] + d[2] * d[2] - 1.).abs() < 1e-3);
        assert!(d[1] > 0. && d[0].abs() > 0.1, "{d:?}");
        for (c, x) in post.centre.iter().zip(d).take(3) {
            assert!((c - post.length * x).abs() < 1e-5);
        }
    }

    #[test]
    fn the_closest_points_of_a_segment_through_a_box_meet() {
        let (on_segment, on_box) = closest([-5., 0.9, 0., 1.], [5., 0.9, 0., 1.], &car(0.));
        for k in 0..3 {
            assert!((on_segment[k] - on_box[k]).abs() < 1e-5, "{on_segment:?} {on_box:?}");
        }
    }
}
