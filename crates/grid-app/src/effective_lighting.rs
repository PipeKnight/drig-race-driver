//! Source coefficient encoding for the bounded lighting trial; no physical units.
//! Static f64 arithmetic is not proof of x87 80-bit boundary parity.
use grid_formats::track_environment::Lighting;
use grid_tools::time_of_day::{Mode, Timeline};

const FAMILIES: [(&str, &str); 3] = [
    ("backlight_suncolour", "backlight_sunscale"),
    ("backlight_backcolour", "backlight_scale"),
    ("backlight_ambientcolour", "backlight_ambientscale"),
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PackedFamily {
    pub bytes: [u8; 3],
    pub scale: f32,
}
impl PackedFamily {
    fn from_static(rgb: [f32; 3]) -> Result<Self, &'static str> {
        if !rgb.into_iter().all(|v| v.is_finite() && v >= 0.) {
            return Err("static source RGB requires finite nonnegative values");
        }
        let scale = rgb.into_iter().fold(1., f32::max);
        // Source742B00 replaces the saved scalar. f64 approximates its x87
        // reciprocal/multiply path; exact boundary parity remains unverified.
        let bytes = rgb.map(|c| ((f64::from(c) / f64::from(scale)) * 255.).trunc() as u8);
        Ok(Self { bytes, scale })
    }
    fn effective(self) -> Result<[f32; 3], &'static str> {
        let rgb = self
            .bytes
            .map(|b| (f32::from(b) * (1. / 255.)) * self.scale);
        if !rgb.into_iter().all(f32::is_finite) {
            return Err("effective source RGB overflow");
        }
        Ok(rgb)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PackedLighting {
    pub families: [PackedFamily; 3],
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct EffectiveLighting {
    pub sun: [f32; 3],
    pub back: [f32; 3],
    pub ambient: [f32; 3],
}
impl PackedLighting {
    /// Raw Lighting remains untouched; these are explicitly derived bytes/scales.
    pub(crate) fn from_static(raw: &Lighting) -> Result<Self, &'static str> {
        let mut families = [PackedFamily {
            bytes: [0; 3],
            scale: 1.,
        }; 3];
        for (out, (rgb_key, _)) in families.iter_mut().zip(FAMILIES) {
            let values = raw.values(rgb_key).ok_or("source RGB absent or invalid")?;
            let rgb: [f32; 3] = values
                .try_into()
                .map_err(|_| "source RGB needs exactly three channels")?;
            *out = PackedFamily::from_static(rgb)?;
        }
        Ok(Self { families })
    }
    /// Re-evaluate from immutable initialized bytes, never accumulate overlays.
    /// Time comes from existing LeMansEvent; this type owns no clock.
    pub(crate) fn resolve(
        self,
        mode: Mode,
        timeline: Option<&Timeline>,
        t: f64,
    ) -> Result<(Self, EffectiveLighting), &'static str> {
        let mut current = self;
        if mode == Mode::Timeline {
            if !t.is_finite() || !(0. ..1.).contains(&t) {
                return Err("timeline clock must be a finite day fraction");
            }
            let timeline = timeline.ok_or("timeline mode requires existing timeline")?;
            for (family, (rgb_key, scale_key)) in current.families.iter_mut().zip(FAMILIES) {
                for (byte, suffix) in family.bytes.iter_mut().zip(["r", "g", "b"]) {
                    if let Some(value) = timeline.value(&format!("{rgb_key}_{suffix}"), t) {
                        *byte = timeline_byte(value)?;
                    }
                }
                if let Some(scale) = timeline.value(scale_key, t) {
                    if !scale.is_finite() || scale < 0. {
                        return Err("timeline light scale requires finite nonnegative value");
                    }
                    family.scale = scale;
                }
            }
        }
        let [sun, back, ambient] = current.families.map(PackedFamily::effective);
        Ok((
            current,
            EffectiveLighting {
                sun: sun?,
                back: back?,
                ambient: ambient?,
            },
        ))
    }
}
fn timeline_byte(value: f32) -> Result<u8, &'static str> {
    // Original registered256 is applied to keys before interpolation. Scaling
    // the existing raw Curve output by this power of two matches ordinary
    // finite ranges; underflow/overflow/extended parser edges need separate proof.
    let scaled = value * 256.;
    let rounded = scaled + 0.5;
    if !value.is_finite() || !rounded.is_finite() {
        return Err("timeline colour exceeds finite diagnostic admission");
    }
    Ok(rounded.trunc().clamp(0., 255.) as u8)
}

/// AUTHORED Bevy fill adapter: one external calibration shared by all scenes
/// and times. Does not re-normalize against each preset's sun peak.
/// It preserves the EXISTING approximate lobe ambient+back/2, back/2.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AuthoredFill {
    pub ambient_rgb: [f32; 3],
    pub ambient_brightness: f32,
    pub back_rgb: [f32; 3],
    pub back_lux: f32,
}
impl EffectiveLighting {
    pub(crate) fn authored_fill(
        self,
        units_per_coefficient: f32,
    ) -> Result<AuthoredFill, &'static str> {
        if !units_per_coefficient.is_finite() || units_per_coefficient <= 0. {
            return Err("authored shared calibration must be finite and positive");
        }
        if !self
            .sun
            .into_iter()
            .chain(self.back)
            .chain(self.ambient)
            .all(|c| c.is_finite() && c >= 0.)
        {
            return Err("authored fill requires finite nonnegative source coefficients");
        }
        let ambient = std::array::from_fn(|i| self.ambient[i] + self.back[i] / 2.);
        let peak = |rgb: [f32; 3]| rgb.into_iter().fold(0., f32::max);
        let normal = |rgb: [f32; 3]| {
            let p = peak(rgb);
            if p > 0. { rgb.map(|c| c / p) } else { [0.; 3] }
        };
        let out = AuthoredFill {
            ambient_rgb: normal(ambient),
            ambient_brightness: peak(ambient) * units_per_coefficient / std::f32::consts::PI,
            back_rgb: normal(self.back),
            back_lux: peak(self.back) * units_per_coefficient / 2.,
        };
        if !out.ambient_brightness.is_finite() || !out.back_lux.is_finite() {
            return Err("authored fill calibration overflow");
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use grid_tools::time_of_day::Curve;
    fn source() -> Lighting {
        let mut raw = Lighting::default();
        for (key, _) in FAMILIES {
            raw.params.insert(key.into(), "2 1 0.5".into());
        }
        raw
    }
    fn timeline(values: &[(&str, f32)]) -> Timeline {
        Timeline {
            curves: values
                .iter()
                .map(|&(key, v)| (key.into(), Curve::new(&[(0, v)], None)))
                .collect(),
        }
    }
    #[test]
    fn static_hdr_replaces_saved_scales_and_keeps_raw_values() {
        let mut raw = source();
        for (_, key) in FAMILIES {
            raw.params.insert(key.into(), "7".into());
        }
        let original = raw.clone();
        let packed = PackedLighting::from_static(&raw).unwrap();
        for f in packed.families {
            assert_eq!(
                f,
                PackedFamily {
                    bytes: [255, 127, 63],
                    scale: 2.
                }
            );
        }
        assert_eq!(raw, original);
        let (_, e) = packed.resolve(Mode::None, None, f64::NAN).unwrap();
        assert_eq!(e.sun[0], 2.);
        assert!((e.sun[1] - 254. / 255.).abs() < 1e-6);
    }
    #[test]
    fn partial_timeline_preserves_derived_scale_and_other_initial_bytes() {
        let packed = PackedLighting::from_static(&source()).unwrap();
        let tl = timeline(&[("backlight_suncolour_r", 0.5), ("backlight_scale", 3.)]);
        let (p, _) = packed.resolve(Mode::Timeline, Some(&tl), 0.).unwrap();
        assert_eq!(
            p.families[0],
            PackedFamily {
                bytes: [128, 127, 63],
                scale: 2.
            }
        );
        assert_eq!(
            p.families[1],
            PackedFamily {
                bytes: [255, 127, 63],
                scale: 3.
            }
        );
        assert_eq!(p.families[2], packed.families[2]);
        assert_eq!(
            packed.resolve(Mode::ForcedNight, Some(&tl), 0.).unwrap().0,
            packed
        );
    }
    #[test]
    fn timeline_zero_is_an_update_not_a_missing_channel() {
        let packed = PackedLighting::from_static(&source()).unwrap();
        let tl = timeline(&[
            ("backlight_ambientcolour_b", 0.),
            ("backlight_ambientscale", 0.),
        ]);
        let (p, e) = packed.resolve(Mode::Timeline, Some(&tl), 0.).unwrap();
        assert_eq!(p.families[2].bytes, [255, 127, 0]);
        assert_eq!(e.ambient, [0.; 3]);
    }
    #[test]
    fn timeline_byte_rounding_clipping_and_invalid_admission_are_explicit() {
        for (v, b) in [
            (-1., 0),
            (0., 0),
            (1. / 512., 1),
            (0.5, 128),
            (1., 255),
            (2., 255),
        ] {
            assert_eq!(timeline_byte(v).unwrap(), b);
        }
        for v in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, f32::MAX] {
            assert!(timeline_byte(v).is_err());
        }
        let packed = PackedLighting::from_static(&source()).unwrap();
        assert!(packed.resolve(Mode::Timeline, None, 0.).is_err());
        assert!(
            packed
                .resolve(Mode::Timeline, Some(&timeline(&[])), 1.)
                .is_err()
        );
    }
    #[test]
    fn source_validation_rejects_missing_negative_and_extra_rgb_channels() {
        for value in ["-1 0 0", "NaN 0 0", "1 2 3 4", "1 2"] {
            let mut raw = source();
            raw.params
                .insert("backlight_suncolour".into(), value.into());
            assert!(PackedLighting::from_static(&raw).is_err());
        }
        assert!(PackedLighting::from_static(&Lighting::default()).is_err());
    }
    #[test]
    fn authored_fill_preserves_shared_magnitude_and_black_limits() {
        let e = EffectiveLighting {
            sun: [1.; 3],
            back: [0.2; 3],
            ambient: [0.1; 3],
        };
        // Arbitrary UNIT TEST scale, never a proposed photometric default.
        let a = e.authored_fill(10.).unwrap();
        let b = EffectiveLighting {
            sun: e.sun.map(|x| x / 10.),
            back: e.back.map(|x| x / 10.),
            ambient: e.ambient.map(|x| x / 10.),
        }
        .authored_fill(10.)
        .unwrap();
        assert!((a.ambient_brightness / b.ambient_brightness - 10.).abs() < 1e-5);
        assert!((a.back_lux / b.back_lux - 10.).abs() < 1e-5);
        let z = EffectiveLighting {
            sun: [0.; 3],
            back: [0.; 3],
            ambient: [0.; 3],
        }
        .authored_fill(10.)
        .unwrap();
        assert_eq!(z.ambient_brightness, 0.);
        assert_eq!(z.back_lux, 0.);
        assert_eq!(z.ambient_rgb, [0.; 3]);
        for u in [0., -1., f32::NAN, f32::INFINITY] {
            assert!(e.authored_fill(u).is_err());
        }
    }
    #[test]
    #[ignore = "requires absolute GRID_GAME_ROOT; CPU source coefficients only"]
    fn purchased_day_night_and_timeline_resolve_without_radiometric_default() {
        use grid_tools::{environment::Environment, route::Route};
        let root = std::path::PathBuf::from(
            std::env::var_os("GRID_GAME_ROOT").expect("absolute GRID_GAME_ROOT required"),
        );
        assert!(root.is_absolute());
        let mut static_pair = Vec::new();
        for path in [
            "tracks/japan/haruna/route_0",
            "tracks/japan/haruna/route_1",
            "tracks/europe/le_mans/route_0",
        ] {
            let route = Route::new(path).unwrap();
            let env = Environment::for_route(&root, &route).unwrap();
            let packed = PackedLighting::from_static(&env.lighting).unwrap();
            let tl = Timeline::load(&root, &route).unwrap();
            for t in if env.mode() == Mode::Timeline {
                vec![15. / 24., 0.]
            } else {
                vec![0.]
            } {
                let (current, e) = packed.resolve(env.mode(), tl.as_ref(), t).unwrap();
                assert!(
                    e.sun
                        .into_iter()
                        .chain(e.back)
                        .chain(e.ambient)
                        .all(|c| c.is_finite() && c >= 0.)
                );
                println!(
                    "effective_lighting_fixture path={path} mode={:?} t={t} source={} packed={current:?} effective={e:?} source_units=dimensionless bevy_calibration=UNSET static_x87_boundary_parity=UNPROVEN",
                    env.mode(),
                    env.lighting_file
                );
                if env.mode() != Mode::Timeline {
                    static_pair.push(e);
                }
                if let Some(tl) = tl.as_ref() {
                    for (family, (_, scale_key)) in current.families.iter().zip(FAMILIES) {
                        if let Some(value) = tl.value(scale_key, t) {
                            assert_eq!(family.scale.to_bits(), value.to_bits());
                        }
                    }
                }
            }
        }
        assert_eq!(static_pair.len(), 2);
        let peak = |c: [f32; 3]| c.into_iter().fold(0., f32::max);
        assert!(peak(static_pair[1].sun) < peak(static_pair[0].sun));
        assert!(peak(static_pair[1].back) < peak(static_pair[0].back));
        assert!(peak(static_pair[1].ambient) < peak(static_pair[0].ambient));
    }
    #[test]
    fn multi_key_wrap_matches_independently_prescaled_curve_for_all_rgb_families() {
        // Independent reference follows original key registration before interpolation.
        // Production uses existing raw Timeline; these controls cover moderate normal
        // f32 inputs, wrap/interior and half-byte boundaries, NOT subnormal/global parity.
        for (family, _) in FAMILIES {
            for (channel, keys) in [
                ("r", vec![(0, 0.125), (360, 0.5), (1020, 0.875)]),
                (
                    "g",
                    vec![
                        (120, 0.5 / 256.),
                        (600, 0.5 / 256. + 1. / 1048576.),
                        (1080, 0.5 / 256. - 1. / 1048576.),
                    ],
                ),
                ("b", vec![(60, -0.25), (540, 0.75), (1200, 1.25)]),
            ] {
                let name = format!("{family}_{channel}");
                let raw = Curve::new(&keys, None);
                let scaled_keys: Vec<_> = keys.iter().map(|&(m, v)| (m, v * 256.)).collect();
                let prescaled = Curve::new(&scaled_keys, None);
                let timeline = Timeline {
                    curves: [(name.clone(), raw)].into_iter().collect(),
                };
                let initial = PackedLighting::from_static(&source()).unwrap();
                for t in [
                    0.,
                    1. / 48.,
                    1. / 12.,
                    0.125,
                    0.25,
                    0.375,
                    0.5,
                    0.625,
                    0.75,
                    0.875,
                    0.9999,
                ] {
                    let reference = (prescaled.value(t) + 0.5).trunc().clamp(0., 255.) as u8;
                    let actual = timeline_byte(timeline.value(&name, t).unwrap()).unwrap();
                    assert_eq!(actual, reference, "family={family} channel={channel} t={t}");
                    let (resolved, _) =
                        initial.resolve(Mode::Timeline, Some(&timeline), t).unwrap();
                    let fi = FAMILIES.iter().position(|(key, _)| *key == family).unwrap();
                    let ci = ["r", "g", "b"]
                        .iter()
                        .position(|&key| key == channel)
                        .unwrap();
                    assert_eq!(resolved.families[fi].bytes[ci], reference);
                }
            }
        }
        let half = Curve::new(&[(0, 0.5), (360, 0.5)], None);
        assert_eq!((half.value(0.125) + 0.5).trunc() as u8, 1);
    }
}
