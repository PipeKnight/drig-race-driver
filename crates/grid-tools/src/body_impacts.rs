//! Read-only body-contact notifications of the last tick (presentation only).
//!
//! The original body handler (9EF360 path) forwards one packet per solved
//! transient contact to its registered users: world point, side-signed normal
//! and the solved impact written back at contact +3C. The loose-part user
//! (9A7670 -> 98B430) consumes exactly point, normal and impact. This module
//! extracts the same three values from a world's solved contact records
//! without touching any simulation state.
use grid_world::contact_groups::Contact;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Impact {
    pub point: [f32; 3],
    /// Side-adjusted normal: negated when the car is the contact's second body.
    pub normal: [f32; 3],
    /// Solved contact +3C.
    pub impact: f32,
}

/// Contacts registered to world shape `token`, in record order. Records with
/// both or neither side registered to `token` are skipped, as are the
/// maximum-float placeholders the handler ignores.
pub fn collect(contacts: &[Contact], token: u32) -> Vec<Impact> {
    let f = |c: &Contact, o: usize| f32::from_bits(c[o / 4]);
    contacts
        .iter()
        .filter_map(|c| {
            let sign = match [c[0x28 / 4] == token, c[0x2c / 4] == token] {
                [true, false] => 1.,
                [false, true] => -1.,
                _ => return None,
            };
            let impact = f(c, 0x3c);
            if !impact.is_finite() || impact == f32::MAX {
                return None;
            }
            let point = std::array::from_fn(|j| f(c, 0x50 + 4 * j));
            let normal = std::array::from_fn(|j| sign * f(c, 0x60 + 4 * j));
            Some(Impact {
                point,
                normal,
                impact,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn side_selects_normal_sign_and_skips_placeholders() {
        let mut a: Contact = [0; 0x80 / 4];
        a[0x28 / 4] = 1;
        a[0x2c / 4] = 7;
        a[0x3c / 4] = 250f32.to_bits();
        a[0x50 / 4] = 1f32.to_bits();
        a[0x60 / 4 + 2] = 1f32.to_bits();
        let mut b = a;
        b[0x28 / 4] = 7;
        b[0x2c / 4] = 1;
        let mut placeholder = a;
        placeholder[0x3c / 4] = f32::MAX.to_bits();
        let mut other = a;
        other[0x28 / 4] = 3;
        let out = collect(&[a, b, placeholder, other], 1);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].normal, [0., 0., 1.]);
        assert_eq!(out[1].normal, [-0., -0., -1.]);
        assert_eq!(out[0].point, [1., 0., 0.]);
        assert_eq!(out[1].impact, 250.);
    }
}
