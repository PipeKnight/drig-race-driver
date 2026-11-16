//! Recovered Color+uchar4 declaration boundary (BDC513..BDC52E).
//! This does not implement source normal-atlas shading or every scalar domain.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct SupportedSourcePacking {
    fade: f32,
    wet: u8,
    normal_depth: u8,
}
fn byte(value: f32) -> Option<u8> {
    if !value.is_finite() || !(0. ..=1.).contains(&value) {
        return None;
    }
    // CVTTSS2SI then low-byte extraction. Domain is deliberately bounded;
    // no saturation/clamp or NaN conversion is claimed for other inputs.
    Some((value * 255.).trunc() as u8)
}
fn rgba(raw: [u8; 4]) -> [f32; 4] {
    [raw[2], raw[1], raw[0], raw[3]].map(|v| f32::from(v) / 255.)
}
impl SupportedSourcePacking {
    pub(super) fn new(fade: f32, wet: f32, normal: [f32; 2]) -> Option<Self> {
        // Existing purchased enabled layers have equal normal-depth bounds.
        // Variable-bound interpolation requires its actual source control;
        // never substitute a guessed fraction for an unsupported profile.
        if normal[0] != normal[1] {
            return None;
        }
        byte(fade)?;
        Some(Self {
            fade,
            wet: byte(wet)?,
            normal_depth: byte(normal[0])?,
        })
    }
    pub(super) fn valid(self) -> bool {
        byte(self.fade).is_some()
    }
    #[cfg(test)]
    pub(super) fn normal_depth(self) -> u8 {
        self.normal_depth
    }
    pub(super) fn clear_normal(&mut self) {
        self.normal_depth = 0;
    }
    pub(super) fn half_normal(&mut self) {
        self.normal_depth /= 2;
    }
    pub(super) fn retire_normal(&mut self) {
        self.normal_depth = self.normal_depth.saturating_sub(25);
    }
    pub(super) fn gpu(self, ramp: f32, current: bool) -> [f32; 4] {
        assert!(matches!(ramp, 0. | 0.5 | 1.));
        rgba([
            byte(ramp * self.fade).expect("supported finite product"),
            self.wet,
            self.normal_depth,
            if current { 255 } else { 0 },
        ])
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn declaration_swizzles_asymmetric_bytes_and_normalizes_once() {
        assert_eq!(
            rgba([30, 61, 211, 127]),
            [211., 61., 30., 127.].map(|x| x / 255.)
        );
        assert_eq!(rgba([0; 4]), [0.; 4]);
        assert_eq!(rgba([255; 4]), [1.; 4]);
        assert_eq!(rgba([255, 0, 0, 0]), [0., 0., 1., 0.]);
    }
    #[test]
    fn supported_packing_truncates_rejects_instead_of_clamping() {
        assert_eq!(byte(0.5), Some(127));
        assert_eq!(byte(0.25), Some(63));
        for v in [f32::NAN, f32::INFINITY, -0.001, 1.001] {
            assert_eq!(byte(v), None);
        }
        assert!(SupportedSourcePacking::new(1., 1., [0.1, 0.9]).is_none());
        let p = SupportedSourcePacking::new(0.25, 0.5, [0.75; 2]).unwrap();
        assert_eq!(p.gpu(0.5, true), [191., 127., 31., 255.].map(|v| v / 255.));
    }
}
