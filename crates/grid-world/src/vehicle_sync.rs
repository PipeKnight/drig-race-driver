//! Whole selected linked-state/mass/control synchronization before the car driver.
//! Own bounded deformation nodes as well as linked mass/control state.
//! Deformation production and wheel/manager scheduling remain external.
use crate::{Error, spawn::reference_seed};
pub const VEHICLE_WORDS: usize = 0xd240 / 4;
pub const PHYSICAL_WORDS: usize = 0x424 / 4;

#[derive(Clone)]
pub struct State {
    /// Owned car storage, including the original settings and history tail.
    /// The linked wrapper slot DB0 is zero at this pointer-free boundary.
    pub vehicle: Vec<u32>,
    pub physical: [u32; PHYSICAL_WORDS],
}
fn f(b: &[u32], o: usize) -> f32 {
    f32::from_bits(b[o / 4])
}
fn set(b: &mut [u32], o: usize, value: f32) {
    b[o / 4] = value.to_bits();
}
fn byte(b: &[u32], o: usize) -> u8 {
    (b[o / 4] >> (o % 4 * 8)) as u8
}
fn put_byte(b: &mut [u32], o: usize, v: u8) {
    let shift = o % 4 * 8;
    b[o / 4] = (b[o / 4] & !(255 << shift)) | ((v as u32) << shift);
}

fn deformation_dimensions(b: &[u32]) -> Result<[usize; 3], Error> {
    let dimensions = [b[0xc714 / 4], b[0xc718 / 4], b[0xc71c / 4]];
    if dimensions == [0; 3] {
        return Ok([0; 3]);
    }
    if dimensions.iter().any(|d| !(2..=16).contains(d)) || dimensions.iter().product::<u32>() > 400
    {
        return Err(Error("invalid synchronized deformation dimensions"));
    }
    Ok(dimensions.map(|d| d as usize))
}

/// Whole reached 9A0A70 traversal. The grid is body+2300, its nodes start
/// at descriptor2580, and the next vec4 in each 80-byte node is the rest point.
/// Both homogeneous lanes participate; the original uses SQRTSS, not RSQRT.
fn settled_strain(b: &[u32], dimensions: [usize; 3]) -> Result<[f32; 4], Error> {
    let [nx, ny, nz] = dimensions;
    let mut scores = [0f32; 4];
    for z in 0..nz {
        for x in 0..nx {
            let corner = (2 * x) / nx + 2 * ((2 * z) / nz);
            for y in 0..ny {
                let at = 0x4880 + ((z * ny + y) * nx + x) * 80;
                let mut d = [0f32; 4];
                for (lane, value) in d.iter_mut().enumerate() {
                    let position = f(b, at + lane * 4);
                    let rest = f(b, at + 16 + lane * 4);
                    if [position, rest]
                        .iter()
                        .any(|v| !v.is_finite() || v.abs() > 1_000_000.)
                    {
                        return Err(Error("invalid synchronized deformation node"));
                    }
                    *value = position - rest;
                }
                // SIMD rotations first add (y*y+x*x) and (w*w+z*z),
                // then add the rotated latter pair to the former pair.
                let squared = (d[3] * d[3] + d[2] * d[2]) + (d[1] * d[1] + d[0] * d[0]);
                if squared > scores[corner] {
                    scores[corner] = squared;
                }
            }
        }
    }
    Ok(scores.map(f32::sqrt))
}

/// Diagonal specialization of the observed cofactor inverse. The determinant
/// and cofactors still follow the original float operations and reciprocal seed;
/// replacing these with independent 1/diagonal divisions changes stored words.
fn mass_matrix(b: &[u32]) -> Result<(f32, [f32; 4], [f32; 4]), Error> {
    let scale = f(b, 0x1560);
    let mass = f(b, 0xfa8) * scale;
    let diagonal = [
        f(b, 0xfac) * scale,
        f(b, 0xfb0) * scale,
        f(b, 0xfb4) * scale,
        1.,
    ];
    let [a, c, d, _] = diagonal;
    let cd = c * d;
    let determinant = cd * a;
    if !mass.is_finite()
        || mass <= 0.
        || !scale.is_finite()
        || diagonal.iter().any(|v| !v.is_finite() || *v <= 0.)
        || !determinant.is_normal()
        || determinant <= 0.
    {
        return Err(Error("invalid synchronized mass/inertia"));
    }
    let seed = reference_seed(determinant, true)?;
    let inverse = (1. - seed * determinant) * seed + seed;
    let ad = a * d;
    let cofactors = [cd, ad, a * c, ad * c];
    let reciprocal = cofactors.map(|v| inverse * v);
    if !(1. / mass).is_finite() || reciprocal.iter().any(|v| !v.is_finite()) {
        return Err(Error("nonfinite synchronized inverse inertia"));
    }
    Ok((mass, diagonal, reciprocal))
}

pub fn calculate(input: &State) -> Result<State, Error> {
    let mut next = input.clone();
    calculate_in(&mut next.vehicle, &mut next.physical)?;
    Ok(next)
}

/// [`calculate`] in place. Every check precedes the first write except the
/// settled-strain grid's, so an error there leaves the record partly written.
/// Each word read after a write is one this update never writes.
pub(crate) fn calculate_in(
    vehicle: &mut [u32],
    physical: &mut [u32; PHYSICAL_WORDS],
) -> Result<(), Error> {
    let b = &*vehicle;
    if b.len() != VEHICLE_WORDS
        || b[0xdb0 / 4] != 0
        || b[0xc7ec / 4] >= 10
        || physical[0x1d0 / 4..0x230 / 4]
            .iter()
            .any(|w| !f32::from_bits(*w).is_finite())
        || (0..10).any(|i| !f(b, 0xc7c4 + i * 4).is_finite())
    {
        return Err(Error("unsupported vehicle synchronization input"));
    }
    let dimensions = deformation_dimensions(b)?;
    const COPIES: [(usize, usize); 21] = [
        (0x2264, 0xd198),
        (0x2268, 0xd1ac),
        (0x2280, 0xd1b0),
        (0x22b0, 0xce04),
        (0x2284, 0xd1c8),
        (0x2288, 0xd1e0),
        (0x228c, 0xd1f8),
        (0x2290, 0xd210),
        (0x2294, 0xd19c),
        (0x22a0, 0xd1c4),
        (0x22a4, 0xd1dc),
        (0x22a8, 0xd1f4),
        (0x22ac, 0xd20c),
        (0x22b4, 0xd1bc),
        (0x22b8, 0xd1d4),
        (0x22bc, 0xd1ec),
        (0x22c0, 0xd204),
        (0x22c4, 0xd1a8),
        (0x22c8, 0xd194),
        (0x2298, 0xd1a0),
        (0x229c, 0xd1a4),
    ];
    if COPIES.iter().any(|&(_, o)| !f(b, o).is_finite())
        || [
            0xfa8, 0xfac, 0xfb0, 0xfb4, 0x1560, 0xc7b4, 0xc7b8, 0xc7bc, 0xc7c0,
        ]
        .iter()
        .any(|&o| !f(b, o).is_finite())
    {
        return Err(Error("nonfinite vehicle synchronization settings"));
    }
    let (mass, diagonal, inverse) = mass_matrix(b)?;
    let b = vehicle;
    for (target, source, count) in [(0x15c0, 0x220, 4), (0x15d0, 0x210, 4), (0x15e0, 0x1d0, 16)] {
        b[target / 4..target / 4 + count]
            .copy_from_slice(&physical[source / 4..source / 4 + count]);
    }
    for base in [0x15c0, 0x15d0] {
        if (0..3).all(|i| f(b, base + i * 4).abs() < f32::from_bits(0x29e12e13)) {
            b[base / 4..base / 4 + 3].fill(0);
        }
    }
    let index = (b[0xc7ec / 4] + 1) % 10;
    b[0xc7ec / 4] = index;
    set(b, 0xc7c4 + index as usize * 4, 0.);
    let mut maximum = f(b, 0xc7c4);
    for i in 1..10 {
        if maximum < f(b, 0xc7c4 + i * 4) {
            maximum = f(b, 0xc7c4 + i * 4);
        }
    }
    if maximum > 3. {
        put_byte(b, 0xc7b1, 1);
    } else if maximum < 0.5 && byte(b, 0xc7b1) != 0 {
        let scores = settled_strain(b, dimensions)?;
        put_byte(b, 0xc7b1, 0);
        for (i, score) in scores.into_iter().enumerate() {
            set(b, 0xc7b4 + 4 * i, score);
        }
    }
    let p = physical;
    set(p, 0x3f0, mass);
    set(p, 0x3f4, 1. / mass);
    for (base, values) in [(0x90, diagonal), (0x150, inverse)] {
        p[base / 4..base / 4 + 16].fill(0);
        for (i, v) in values.into_iter().enumerate() {
            set(p, base + i * 20, v);
        }
    }
    for (to, from) in COPIES {
        b[to / 4] = b[from / 4];
    }
    let disabled_control = if b[0xd234 / 4] != 7 { 1. } else { 0. };
    set(b, 0x226c, disabled_control);
    let disabled = (0..4).any(|i| byte(b, 0x22e4 + i) != 0);
    set(b, 0x2270, if disabled { 1. } else { 0. });
    for i in 0..4 {
        put_byte(b, 0x2278 + i, byte(b, 0x22e4 + i));
        put_byte(b, 0x227c + i, byte(b, 0x22e0 + i));
    }
    let stabilization = byte(b, 0xd1b4) as f32;
    set(b, 0x2274, stabilization);
    for (to, from) in [
        (0x22cc, 0xd1cc),
        (0x22cd, 0xd1e4),
        (0x22ce, 0xd1fc),
        (0x22cf, 0xd214),
    ] {
        put_byte(b, to, byte(b, from));
    }
    let inactive = u8::from(byte(b, 0xb13) == 0 || byte(b, 0xb14) == 0);
    put_byte(b, 0xc7af, inactive);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn active_grid() -> State {
        let mut s = State {
            vehicle: vec![0; VEHICLE_WORDS],
            physical: [0; PHYSICAL_WORDS],
        };
        for o in [0xfa8, 0xfac, 0xfb0, 0xfb4, 0x1560] {
            set(&mut s.vehicle, o, 1.);
        }
        for (o, n) in [(0xc714, 3), (0xc718, 2), (0xc71c, 3)] {
            s.vehicle[o / 4] = n;
        }
        put_byte(&mut s.vehicle, 0xc7b1, 1);
        for (node, displacement) in [
            (0, [3., 4., 0., 0.]),
            (2, [0., 0., 0., 6.]),
            (12, [0., 8., 15., 0.]),
            (14, [1., 2., 2., 0.]),
        ] {
            for (i, v) in displacement.into_iter().enumerate() {
                set(&mut s.vehicle, 0x4880 + node * 80 + 4 * i, v);
            }
        }
        s
    }

    #[test]
    fn settling_strain_reads_all_four_corners_and_homogeneous_lane() {
        let s = active_grid();
        let result = calculate(&s).unwrap();
        assert_eq!(byte(&result.vehicle, 0xc7b1), 0);
        assert_eq!(
            std::array::from_fn::<_, 4, _>(|i| f(&result.vehicle, 0xc7b4 + 4 * i)),
            [5., 6., 17., 3.]
        );
        assert_eq!(byte(&s.vehicle, 0xc7b1), 1);
        assert_eq!(
            result.vehicle[0x4880 / 4..0xc580 / 4],
            s.vehicle[0x4880 / 4..0xc580 / 4]
        );
    }

    #[test]
    fn invalid_reached_node_rejects_without_mutation_but_skipped_nodes_are_opaque() {
        let mut s = active_grid();
        s.vehicle[(0x4880 + 17 * 80 + 28) / 4] = f32::NAN.to_bits();
        let before = s.vehicle.clone();
        assert_eq!(
            calculate(&s).err().unwrap().0,
            "invalid synchronized deformation node"
        );
        assert_eq!(s.vehicle, before);
        put_byte(&mut s.vehicle, 0xc7b1, 0);
        assert!(calculate(&s).is_ok());
        put_byte(&mut s.vehicle, 0xc7b1, 1);
        set(&mut s.vehicle, 0xc7c4, 0.5);
        assert!(calculate(&s).is_ok());
        set(&mut s.vehicle, 0xc7c4, 3.);
        assert!(calculate(&s).is_ok());
        set(&mut s.vehicle, 0xc7c4, 3.0001);
        assert!(calculate(&s).is_ok());
    }
    #[test]
    fn linked_publication_is_authoritative_and_tiny_cleanup_keeps_w() {
        let mut s = State {
            vehicle: vec![0; VEHICLE_WORDS],
            physical: [0; PHYSICAL_WORDS],
        };
        for o in [0xfa8, 0xfac, 0xfb0, 0xfb4, 0x1560] {
            set(&mut s.vehicle, o, 1.);
        }
        set(&mut s.vehicle, 0x15c0, 99.);
        set(&mut s.physical, 0x220, 1e-14);
        set(&mut s.physical, 0x22c, 7.);
        set(&mut s.physical, 0x200, 25.);
        s.vehicle[0x100 / 4] = 0xdeadbeef;
        let result = calculate(&s).unwrap();
        assert_eq!(f(&result.vehicle, 0x15c0), 0.);
        assert_eq!(f(&result.vehicle, 0x15cc), 7.);
        assert_eq!(f(&result.vehicle, 0x1610), 25.);
        assert_eq!(result.vehicle[0x100 / 4], 0xdeadbeef);
        assert_eq!(f(&s.vehicle, 0x15c0), 99.);
    }
}
