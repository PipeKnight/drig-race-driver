//! Explicit bounded source-projector admission; normal builds remain independent.
use std::ffi::OsString;
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Admitted {
    pub units: f32,
    pub range: f32,
    pub colour: [f32; 3],
}
#[cfg(feature = "source-projector")]
pub(crate) type Request = Admitted;
#[cfg(not(feature = "source-projector"))]
#[derive(Clone, Copy, Debug)]
pub(crate) struct Request;
#[derive(Default)]
struct Fields {
    units: Option<OsString>,
    range: Option<OsString>,
    snapshot: Option<OsString>,
}
fn text<'a>(field: Option<&'a OsString>, name: &str) -> Result<&'a str, String> {
    field
        .ok_or_else(|| format!("explicit source projector {name} required"))?
        .to_str()
        .ok_or_else(|| format!("non-Unicode source projector {name}"))
}
fn positive(value: &str) -> Result<f32, String> {
    let x = value
        .parse::<f32>()
        .map_err(|_| "source projector parameter is not a float")?;
    if !x.is_finite() || x <= 0. {
        return Err("source projector parameters must be finite positive".into());
    }
    Ok(x)
}
/// Consumed by requested(); all inputs explicit so tests never mutate global ENV.
fn admit(
    fields: &Fields,
    compiled: bool,
    shared_units: Option<f32>,
    native: Option<f32>,
) -> Result<Option<Admitted>, String> {
    if fields.units.is_none() && fields.range.is_none() && fields.snapshot.is_none() {
        return Ok(None);
    }
    if fields.units.is_none() {
        return Err("partial source projector request without explicit units".into());
    }
    if !compiled {
        return Err("source projector requires explicitly built source-projector=>bevy/pbr_light_textures feature".into());
    }
    if native.is_some() {
        return Err(
            "source projector and native PBR headlight pilot are mutually exclusive".into(),
        );
    }
    let request = Admitted {
        units: positive(text(fields.units.as_ref(), "units")?)?,
        range: positive(text(fields.range.as_ref(), "range")?)?,
        // Explicit source constructor snapshot only, not an invented XML field.
        colour: [1.; 3],
    };
    if shared_units.map(f32::to_bits) != Some(request.units.to_bits()) {
        return Err("source projector U must exactly match enabled GRID_SOURCE_LIGHTING_UNITS shared calibration".into());
    }
    if text(fields.snapshot.as_ref(), "coefficient snapshot")? != "source-constructor" {
        return Err("only explicit source-constructor snapshot admitted; current dynamic projector colour resolver absent".into());
    }
    // Read and admit the ACTUAL object returned to main, not separate checked metadata.
    if !request.range.is_finite()
        || request.range <= 0.
        || request
            .colour
            .iter()
            .any(|c| !c.is_finite() || *c < 0. || !(*c * request.units).is_finite())
    {
        return Err("source projector admitted coefficient domain invalid".into());
    }
    Ok(Some(request))
}
pub(crate) fn requested(
    shared_units: Option<f32>,
    native: Option<f32>,
) -> Result<Option<Request>, String> {
    let fields = Fields {
        units: std::env::var_os("GRID_SOURCE_PROJECTOR_UNITS"),
        range: std::env::var_os("GRID_SOURCE_PROJECTOR_RANGE"),
        snapshot: std::env::var_os("GRID_SOURCE_PROJECTOR_COLOUR_SNAPSHOT"),
    };
    let admitted = admit(
        &fields,
        cfg!(feature = "source-projector"),
        shared_units,
        native,
    )?;
    #[cfg(feature = "source-projector")]
    {
        Ok(admitted)
    }
    #[cfg(not(feature = "source-projector"))]
    {
        assert!(
            admitted.is_none(),
            "pure admission refuses requests without compiled feature"
        );
        Ok(None)
    }
}
#[cfg(feature = "source-projector")]
pub(crate) type Context<'w, 's> = crate::source_projector_present::Context<'w, 's>;
// Small SystemParam shim keeps main's drive_present signature valid in both builds.
// It installs no resource, queries no entities, changes no state and starts no timer.
#[cfg(not(feature = "source-projector"))]
#[derive(bevy::prelude::Resource)]
pub(crate) struct Disabled;
#[cfg(not(feature = "source-projector"))]
pub(crate) type Context<'w, 's> = (
    Option<bevy::prelude::Res<'w, Disabled>>,
    bevy::prelude::Local<'s, ()>,
);
#[cfg(feature = "source-projector")]
pub(crate) fn begin(c: &mut Context<'_, '_>) {
    crate::source_projector_present::begin(c);
}
#[cfg(not(feature = "source-projector"))]
pub(crate) fn begin(_c: &mut Context<'_, '_>) {}
#[cfg(feature = "source-projector")]
pub(crate) fn present(c: &mut Context<'_, '_>, slot: usize, pose: bevy::prelude::Mat4) {
    crate::source_projector_present::present(c, slot, pose);
}
#[cfg(not(feature = "source-projector"))]
pub(crate) fn present(_c: &mut Context<'_, '_>, _slot: usize, _pose: bevy::prelude::Mat4) {}

#[cfg(test)]
mod tests {
    use super::*;
    fn fields(u: Option<&str>, r: Option<&str>, s: Option<&str>) -> Fields {
        Fields {
            units: u.map(Into::into),
            range: r.map(Into::into),
            snapshot: s.map(Into::into),
        }
    }
    fn valid() -> Fields {
        fields(Some("2"), Some("10"), Some("source-constructor"))
    }
    #[test]
    fn absent_request_keeps_native_control_allowed_and_needs_no_feature() {
        assert_eq!(
            admit(&Fields::default(), false, None, Some(1500.)).unwrap(),
            None
        );
        assert_eq!(admit(&Fields::default(), true, None, None).unwrap(), None);
    }
    #[test]
    fn every_nonempty_partial_field_subset_refuses_without_defaults() {
        for bits in 1..7 {
            let f = fields(
                (bits & 1 != 0).then_some("2"),
                (bits & 2 != 0).then_some("10"),
                (bits & 4 != 0).then_some("source-constructor"),
            );
            assert!(admit(&f, true, Some(2.), None).is_err(), "bits={bits}");
        }
    }
    #[test]
    fn explicit_snapshot_accepted_and_actual_values_preserved() {
        assert_eq!(
            admit(&valid(), true, Some(2.), None).unwrap(),
            Some(Admitted {
                units: 2.,
                range: 10.,
                colour: [1.; 3]
            })
        );
    }
    #[test]
    fn missing_feature_native_conflict_and_missing_or_mismatched_shared_unit_refuse() {
        assert!(admit(&valid(), false, Some(2.), None).is_err());
        assert!(admit(&valid(), true, Some(2.), Some(1.)).is_err());
        for shared in [None, Some(1.), Some(2.00001), Some(f32::NAN)] {
            assert!(admit(&valid(), true, shared, None).is_err());
        }
    }
    #[test]
    fn both_numeric_fields_refuse_nonfinite_nonpositive_and_malformed() {
        for x in [
            "0", "-0", "-1", "NaN", "inf", "-inf", "1e99", "", "junk", " 2",
        ] {
            assert!(
                admit(
                    &fields(Some(x), Some("10"), Some("source-constructor")),
                    true,
                    Some(2.),
                    None
                )
                .is_err(),
                "units={x}"
            );
            assert!(
                admit(
                    &fields(Some("2"), Some(x), Some("source-constructor")),
                    true,
                    Some(2.),
                    None
                )
                .is_err(),
                "range={x}"
            );
        }
    }
    #[test]
    fn snapshot_never_silently_uses_an_unknown_source_field_or_auto_default() {
        for s in [
            "",
            "auto",
            "headlight_colour",
            "1 1 1",
            "Source-Constructor",
        ] {
            assert!(
                admit(
                    &fields(Some("2"), Some("10"), Some(s)),
                    true,
                    Some(2.),
                    None
                )
                .is_err()
            );
        }
    }
    #[cfg(any(windows, unix))]
    #[test]
    fn all_three_fields_reject_non_unicode_without_global_environment_changes() {
        #[cfg(windows)]
        let invalid = {
            use std::os::windows::ffi::OsStringExt;
            OsString::from_wide(&[0xd800])
        };
        #[cfg(unix)]
        let invalid = {
            use std::os::unix::ffi::OsStringExt;
            OsString::from_vec(vec![0xff])
        };
        assert!(invalid.to_str().is_none());
        for field in 0..3 {
            let mut f = valid();
            match field {
                0 => f.units = Some(invalid.clone()),
                1 => f.range = Some(invalid.clone()),
                _ => f.snapshot = Some(invalid.clone()),
            };
            assert!(admit(&f, true, Some(2.), None).is_err());
        }
    }
}
