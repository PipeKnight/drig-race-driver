//! A world joint's descriptor from a `TEMPLATECONSTRAINT` and its
//! `TEMPLATECONSTRAINTPROPS` (docs/JOINTS_RESEARCH.md, phase 3): the template
//! reader's frame renormalisation (0x7CF4B0), the template's three frames
//! (0x7C7C00 with the props quaternions, 0x7C06B0/0x7C08E0/0x7C0B10), the
//! entity transform (0x7CC1E0), the props copy (0x7C04D0/0x7C7B10) and the
//! world's descriptor (0xA74670) that `joint::construct` (AEFEB0) reads.
use crate::joint::{DESCRIPTOR, V, compose, cross, normalize};

/// The constraint props the descriptor reads (the props object 0x7B7FC0
/// builds and 0x7D00F0 fills; offsets in the original object).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Props {
    /// `linearLimitAABBMin/Max` (+0x30/+0x40, lane 3 zero).
    pub linear_limit: [[f32; 3]; 2],
    /// `linearSpringAABBMin/Max` (+0x50/+0x60).
    pub linear_spring: [[f32; 3]; 2],
    /// `linearSpringK` (+0x70) and `linearSpringS` (+0x7C).
    pub spring_k: [f32; 3],
    pub spring_s: [f32; 3],
    /// `rotationType` (+0x88): 0 free, 1 plane, 2 cone.
    pub rotation_type: u32,
    /// `rotationLimitAngle`, `rotationSpringAngle`, `rotationSpringK`,
    /// `rotationSpringS`, `rotationTanSpringS` (+0x8C..+0x9C).
    pub rotation: [f32; 5],
    /// `twistType` (+0xA0): 1 circle, 0 free.
    pub twist_type: u32,
    /// `twistLimitAngle`, `twistSpringAngle`, `twistSpringK`, `twistSpringS`
    /// (+0xA4..+0xB0).
    pub twist: [f32; 4],
    /// `breakLinearImpulse` / `breakAngularImpulse` (+0xB4/+0xB8; `+inf` is
    /// FLT_MAX, 0x7D8600).
    pub break_linear: f32,
    pub break_angular: f32,
    /// `hardRotationalFriction` (+0xC0).
    pub hard_friction: f32,
    /// `breakIfAngExtentHit` (+0xC8): the descriptor gets the callback 0x7B81A0.
    pub break_at_angular_extent: bool,
    /// `linearRotation`, `angularRotation0`, `angularRotation1` (+0xCC, +0xDC,
    /// +0xEC), quaternions x y z w.
    pub rotations: [[f32; 4]; 3],
}

/// 0x7C06B0 (and its twins for +0xDC/+0xEC): the quaternion's rotation
/// matrix, rows (x, y, z, position), in the original's lane arithmetic.
pub fn quaternion_matrix(q: [f32; 4]) -> [V; 4] {
    let [x, y, z, w] = q;
    let (x2, y2, z2) = (x + x, y + y, z + z);
    let c = [
        (1. - y2 * y) - z2 * z,
        (1. - z2 * z) - x2 * x,
        (1. - x2 * x) - y2 * y,
    ];
    let b = [y2 * w + z * x2, z2 * w + x * y2, x2 * w + y * z2];
    let d = [y * x2 - z2 * w, z * y2 - x2 * w, x * z2 - y2 * w];
    [
        [c[0], b[1], d[2], 0.],
        [d[0], c[1], b[2], 0.],
        [b[0], d[1], c[2], 0.],
        [0., 0., 0., 1.],
    ]
}

/// 0x7CF4B0 reading `TEMPLATETRANSFORM` (row-major, row vectors): rows 0 and
/// 1 normalised (RSQRTPS with one Newton step, all four lanes), row 2 the
/// normalised cross product of the new rows 0 and 1, row 3 kept.
pub fn template_transform(m: &[f32; 16]) -> [V; 4] {
    let row = |k: usize| -> V { std::array::from_fn(|j| m[4 * k + j]) };
    let r0 = normalize(row(0));
    let r1 = normalize(row(1));
    [r0, r1, normalize(cross(r0, r1)), row(3)]
}

/// 0x7C7C00: the template's frames in the entity: the transform times the
/// linear and the two angular rotation matrices (+0x40, +0x80, +0xC0).
pub fn template_frames(transform: &[V; 4], props: &Props) -> [[V; 4]; 3] {
    props
        .rotations
        .map(|q| compose(transform, &quaternion_matrix(q)))
}

/// The joint request 0x7CC1E0 fills (0x7D8C40 constructs it): words the
/// world's descriptor (0xA74670) reads, other words zero.
pub const REQUEST: usize = 0x174;

#[derive(Clone, PartialEq)]
pub struct Request {
    pub words: [u32; REQUEST / 4],
}

/// The callback 0x7C04D0 stores for `breakIfAngExtentHit`.
pub const ANGULAR_EXTENT_CALLBACK: u32 = 0x7b81a0;

/// 0x7C7B10 with 0x7C04D0: the template's frames and its props, the limit
/// boxes, springs and break impulses times the entity `scale` (FLT_MAX kept).
pub fn request(frames: &[[V; 4]; 3], props: &Props, scale: f32, display: bool) -> Request {
    let mut w = [0u32; REQUEST / 4];
    let boxes = [
        props.linear_limit[0],
        props.linear_limit[1],
        props.linear_spring[0],
        props.linear_spring[1],
    ];
    for (k, b) in boxes.iter().enumerate() {
        for i in 0..4 {
            // Lane 3 is the reader's zero.
            w[(0x10 + 0x10 * k) / 4 + i] = (scale * b.get(i).copied().unwrap_or(0.)).to_bits();
        }
    }
    for i in 0..3 {
        w[0x50 / 4 + i] = (props.spring_k[i] * scale).to_bits();
        w[0x5c / 4 + i] = (props.spring_s[i] * scale).to_bits();
    }
    for (k, frame) in frames.iter().enumerate() {
        for (r, row) in frame.iter().enumerate() {
            for (i, &v) in row.iter().enumerate() {
                w[(0x70 + 0x40 * k + 0x10 * r) / 4 + i] = v.to_bits();
            }
        }
    }
    w[0x130 / 4] = props.rotation_type;
    for (i, &v) in props.rotation.iter().enumerate() {
        w[0x134 / 4 + i] = v.to_bits();
    }
    w[0x148 / 4] = props.twist_type;
    for (i, &v) in props.twist.iter().enumerate() {
        w[0x14c / 4 + i] = v.to_bits();
    }
    let impulse = |v: f32| if v == f32::MAX { v } else { v * scale };
    w[0x15c / 4] = impulse(props.break_linear).to_bits();
    w[0x160 / 4] = impulse(props.break_angular).to_bits();
    w[0x164 / 4] = props.hard_friction.to_bits();
    w[0x168 / 4] = if props.break_at_angular_extent {
        ANGULAR_EXTENT_CALLBACK
    } else {
        0
    };
    w[0xc / 4] = u32::from(display);
    Request { words: w }
}

/// 0x7CC1E0's frame transform and 0xA74670: the constructor's descriptor
/// for the request of an entity at `entity` (row vectors). Each frame's
/// position is first multiplied lane by lane by the entity's
/// `position_scale` (0x4B1BD0). `fixed`: B is the world's fixed object
/// (`body2uri` "none"/"world"). Words 0..2 (world and body pointers) stay
/// zero; byte 0x164 (a point joint) stays clear, as the request's +0x170.
pub fn descriptor(
    request: &Request,
    entity: &[V; 4],
    position_scale: [f32; 3],
    fixed: bool,
) -> [u32; DESCRIPTOR / 4] {
    let r = &request.words;
    let mut d = [0u32; DESCRIPTOR / 4];
    for k in 0..3 {
        let mut frame: [V; 4] = std::array::from_fn(|row| {
            std::array::from_fn(|i| f32::from_bits(r[(0x70 + 0x40 * k + 0x10 * row) / 4 + i]))
        });
        for (v, s) in frame[3].iter_mut().zip(position_scale) {
            *v *= s;
        }
        for (row, v) in compose(&frame, entity).iter().enumerate() {
            for (i, &x) in v.iter().enumerate() {
                d[(0x10 + 0x40 * k + 0x10 * row) / 4 + i] = x.to_bits();
            }
        }
    }
    // Limit and spring boxes, spring K and S.
    d[0xd0 / 4..0x110 / 4].copy_from_slice(&r[0x10 / 4..0x50 / 4]);
    d[0x110 / 4..0x128 / 4].copy_from_slice(&r[0x50 / 4..0x68 / 4]);
    d[0x128 / 4] = match r[0x130 / 4] {
        0 => 0,
        1 => 1,
        _ => 2,
    };
    d[0x12c / 4..0x140 / 4].copy_from_slice(&r[0x134 / 4..0x148 / 4]);
    d[0x140 / 4] = u32::from(r[0x148 / 4] != 0);
    d[0x144 / 4..0x160 / 4].copy_from_slice(&r[0x14c / 4..0x168 / 4]);
    d[0x160 / 4] = r[0x168 / 4];
    d[0xc / 4] = u32::from(fixed) | (u32::from(r[0xc / 4] != 0) << 8);
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    const IDENTITY: [f32; 4] = [0., 0., 0., 1.];

    fn props() -> Props {
        Props {
            linear_limit: [[0., 0., -1.], [0., 0., 0.]],
            linear_spring: [[0.; 3]; 2],
            spring_k: [0., 0., 100000.],
            spring_s: [0., 0., 10000.],
            rotation_type: 1,
            rotation: [0.1, 0., 0., 0., 0.],
            twist_type: 1,
            twist: [0.1, 0., 0., 0.],
            break_linear: f32::MAX,
            break_angular: f32::MAX,
            hard_friction: 0.,
            break_at_angular_extent: false,
            rotations: [IDENTITY; 3],
        }
    }

    #[test]
    fn quaternion_matrix_is_the_row_vector_rotation() {
        assert_eq!(
            quaternion_matrix(IDENTITY),
            [[1., 0., 0., 0.], [0., 1., 0., 0.], [0., 0., 1., 0.], IDENTITY]
        );
        // A quarter turn about z takes x to y in row-vector form.
        let h = std::f32::consts::FRAC_1_SQRT_2;
        let m = quaternion_matrix([0., 0., h, h]);
        assert!((m[0][1] - 1.).abs() < 1e-6 && m[0][0].abs() < 1e-6);
        assert!((m[1][0] + 1.).abs() < 1e-6);
    }

    #[test]
    fn a_tyre_wall_centre_spring_descriptor() {
        // NeonConstraint_c_subent's transform, placed at (100, 0, 50).
        let t = [
            -0.000, -1.000, 0.001, 0.000, 1.000, -0.000, 0.000, 0.000, -0.000, 0.001, 1.000, 0.000,
            2.026, 0.467, 0.002, 1.000,
        ];
        let transform = template_transform(&t);
        let frames = template_frames(&transform, &props());
        assert_eq!(frames[0], frames[1]);
        let entity = [[1., 0., 0., 0.], [0., 1., 0., 0.], [0., 0., 1., 0.], [100., 0., 50., 1.]];
        let d = descriptor(&request(&frames, &props(), 1., false), &entity, [1.; 3], false);
        let at = |o: usize| f32::from_bits(d[o / 4]);
        assert_eq!([at(0x40), at(0x44), at(0x48), at(0x4c)], [102.026, 0.467, 50.002, 1.]);
        assert_eq!(at(0xd8), -1.);
        assert_eq!(at(0x118), 100000.);
        assert_eq!(at(0x124), 10000.);
        assert_eq!(d[0x128 / 4], 1);
        assert_eq!(d[0x140 / 4], 1);
        assert_eq!(at(0x154), f32::MAX);
        assert_eq!(d[0xc / 4], 0);
    }
}
