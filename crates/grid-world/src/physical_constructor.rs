//! Complete selected cold body constructor with supplied diagonal mass tensor.
//! Owned empty-world registration; allocation, shapes and resource setup separate.
use crate::{Error, physical_pose, spawn};
pub struct Output {
    pub body: [u32; 268],
    pub settings: [u32; 88],
    pub world: [u32; 64],
}
fn f(b: &[u32], o: usize) -> f32 {
    f32::from_bits(b[o / 4])
}
fn set(b: &mut [u32], o: usize, value: f32) {
    b[o / 4] = value.to_bits();
}
fn byte(b: &[u32], o: usize) -> u8 {
    (b[o / 4] >> (8 * (o % 4))) as u8
}
fn set_byte(b: &mut [u32], o: usize, value: u8) {
    let shift = 8 * (o % 4);
    b[o / 4] = (b[o / 4] & !(255 << shift)) | (u32::from(value) << shift);
}
/// Address slots are checked identities: owned body offsets plus one, external
/// world10000000, world body references one. Tokens are never dereferenced.
pub fn initialize(input: &[u32; 88], selector: u32, fill: u8) -> Result<Output, Error> {
    if selector > 1
        || ![0, 0xcd].contains(&fill)
        || input[0x60 / 4] != 1
        || input[0x64 / 4] != 0
        || input[0x134 / 4] != 0
        || input[0x68 / 4..0x90 / 4].iter().any(|&v| v != 0)
        || (0x130..0x134).any(|o| byte(input, o) > 1)
        || (0..0x160)
            .step_by(4)
            .filter(|o| ![0x60, 0x64, 0x130, 0x134].contains(o))
            .any(|o| {
                !f(input, o).is_finite()
                    || (!(0xa0..=0xb0).contains(&o) && o != 0xec && f(input, o).abs() > 1_000_000.)
            })
        || f(input, 0) <= 0.
    {
        return Err(Error("invalid physical constructor settings"));
    }
    for r in 0..4 {
        for c in 0..4 {
            let o = 0x10 + r * 16 + c * 4;
            let v = f(input, o);
            if (r == c && (v <= 0. || (r == 3 && v != 1.))) || (r != c && input[o / 4] != 0) {
                return Err(Error("unsupported physical constructor tensor"));
            }
        }
    }
    let mut s = *input;
    for r in 0..3 {
        set(&mut s, 0xfc + r * 16, 0.);
    }
    set(&mut s, 0x12c, 1.);
    let mut b = [u32::from_le_bytes([fill; 4]); 268];
    // Explicit base/control initialization; untouched storage keeps its fill.
    for o in [
        0, 4, 0x10, 0x14, 0x1c, 0x24, 0x28, 0x30, 0x2f0, 0x2f4, 0x2fc, 0x300, 0x304, 0x308, 0x30c,
        0x310, 0x314, 0x31c, 0x320, 0x324, 0x328, 0x3a0, 0x404, 0x408, 0x418, 0x41c, 0x420, 0x424,
        0x428, 0x42c,
    ] {
        b[o / 4] = 0;
    }
    for o in [8, 0xc, 0x18] {
        b[o / 4] = 1;
    }
    b[0x20 / 4] = 1;
    for (o, v) in [
        (0x2c, 1),
        (0x2d, 0),
        (0x390, 0),
        (0x40c, 0),
        (0x40d, 0),
        (0x40e, 0),
        (0x40f, byte(&s, 0x132)),
        (0x410, byte(&s, 0x133)),
        (0x411, 1),
        (0x412, 0),
        (0x413, 0),
        (0x415, 0),
        (0x416, 1),
    ] {
        set_byte(&mut b, o, v);
    }
    for o in (0x240..0x270).chain(0x350..0x390).step_by(4) {
        b[o / 4] = 0;
    }
    b[0x32c / 4] = 0x1000_0000;
    for (o, p) in [
        (0x80, 0x70),
        (0x84, 0x40),
        (0x88, 0x50),
        (0x8c, 0x60),
        (0x330, 0x200),
        (0x334, 0x1c0),
        (0x338, 0x190),
        (0x33c, 0x1a0),
        (0x340, 0x1b0),
    ] {
        b[o / 4] = p + 1;
    }
    for (dest, source) in [
        (0x394, 0xe0),
        (0x398, 0xe4),
        (0x39c, 0xe8),
        (0x3b4, 0xa0),
        (0x3b8, 0xa4),
        (0x3bc, 0xa8),
        (0x3c0, 0xac),
        (0x3c4, 0xb0),
        (0x3c8, 0xb4),
        (0x3cc, 0xb8),
        (0x3d0, 0xbc),
        (0x3d4, 0xd8),
        (0x3d8, 0xc0),
        (0x3dc, 0xc4),
        (0x3e0, 0xc8),
        (0x3e4, 0xcc),
        (0x3e8, 0xd0),
        (0x3ec, 0xd4),
        (0x3f0, 0),
        (0x3f8, 0xdc),
    ] {
        b[dest / 4] = s[source / 4];
    }
    for (dest, source) in [(0x3a4, 0x98), (0x3a8, 0x9c), (0x3ac, 0x90), (0x3b0, 0x94)] {
        set(&mut b, dest, f(&s, source) * f(&s, source));
    }
    let limit = f(&s, 0xec);
    set(
        &mut b,
        0x3fc,
        if limit == f32::MAX {
            f32::MAX
        } else {
            limit * limit
        },
    );
    b[0x90 / 4..0xd0 / 4].copy_from_slice(&s[0x10 / 4..0x50 / 4]);
    b[0x230 / 4..0x240 / 4].copy_from_slice(&s[0x50 / 4..0x60 / 4]);
    set(&mut b, 0x23c, 0.);
    let [a, c, d] = [f(&s, 0x10), f(&s, 0x24), f(&s, 0x38)];
    let cd = c * d;
    let determinant = cd * a;
    if !determinant.is_normal() || determinant <= 0. {
        return Err(Error("invalid physical constructor determinant"));
    }
    let seed = spawn::reference_seed(determinant, true)?;
    let inverse = (1. - seed * determinant) * seed + seed;
    let ad = a * d;
    let diagonal = [cd, ad, a * c, ad * c].map(|x| inverse * x);
    for (r, &diagonal_value) in diagonal.iter().enumerate() {
        for c in 0..4 {
            set(
                &mut b,
                0x150 + r * 16 + c * 4,
                if r == c { diagonal_value } else { 0. },
            );
        }
    }
    set(&mut b, 0x3f4, 1. / f(&s, 0));
    let published = if byte(&s, 0x130) == 0 {
        std::array::from_fn(|r| std::array::from_fn(|c| f32::from(r == c)))
    } else {
        std::array::from_fn(|r| std::array::from_fn(|c| f(&s, 0xf0 + r * 16 + c * 4)))
    };
    let matrices = [0x90, 0x150]
        .map(|o| std::array::from_fn(|r| std::array::from_fn(|c| f(&b, o + r * 16 + c * 4))));
    let pose = physical_pose::from_publication(
        published,
        std::array::from_fn(|i| f(&b, 0x230 + i * 4)),
        matrices,
        &[],
    )?;
    for (o, m) in [
        (0x40, published),
        (0x190, pose.corrected),
        (0x1d0, published),
        (0x270, pose.corrected),
        (0x2b0, published),
        (0xd0, pose.derived[0]),
        (0x110, pose.derived[1]),
    ] {
        for (i, v) in m.into_iter().flatten().enumerate() {
            set(&mut b, o + i * 4, v);
        }
    }
    b[0x210 / 4..0x220 / 4].copy_from_slice(&s[0x140 / 4..0x150 / 4]);
    b[0x220 / 4..0x230 / 4].copy_from_slice(&s[0x150 / 4..0x160 / 4]);
    let active = byte(&s, 0x131) != 0;
    b[0x400 / 4] = if active { selector } else { u32::MAX };
    let mut world = [0; 64];
    world[0x60 / 4] = selector;
    world[0x24 / 4] = 1;
    if active {
        world[(0x3c + selector as usize * 4) / 4] = 1;
        world[(0x44 + selector as usize * 4) / 4] = 1;
    }
    // Numeric words may not overflow; opaque fill and address tokens are retained.
    if (0x90..0x2f0).step_by(4).any(|o| !f(&b, o).is_finite())
        || (0x3a4..=0x3b0).step_by(4).any(|o| !f(&b, o).is_finite())
        || !f(&b, 0x3f4).is_finite()
        || !f(&b, 0x3fc).is_finite()
    {
        return Err(Error("nonfinite physical constructor output"));
    }
    Ok(Output {
        body: b,
        settings: s,
        world,
    })
}
