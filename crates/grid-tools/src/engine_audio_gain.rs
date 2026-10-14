//! Reconstructed 42DE60 crossfade shaping, with separately checked math helpers.
//! Values are decibels for a pair of voices; whole gain/backend behavior is absent.

pub fn crossfade_db(phase: f32) -> Result<[f32; 2], &'static str> {
    if !phase.is_finite() || !(0.0..=1.).contains(&phase) {
        return Err("engine crossfade phase domain");
    }
    if phase <= 0.0001 {
        return Ok([0., -48.]);
    }
    if phase >= 0.9999 {
        return Ok([-48., 0.]);
    }
    // Preserve the native constant, whose last bits differ from std PI/2.
    let angle = f64::from(phase * f32::from_bits(0x3fc9_0fd8));
    let cosine = angle.cos() as f32;
    let sine = angle.sin() as f32;
    Ok([
        ((f64::from(cosine).log10() as f32) * 20.).max(-48.),
        ((f64::from(sine).log10() as f32) * 20.).max(-48.),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_invalid_phase_and_retains_source_endpoint_policy() {
        for phase in [f32::NAN, f32::INFINITY, -0.0001, 1.0001] {
            assert!(crossfade_db(phase).is_err());
        }
        assert_eq!(crossfade_db(0.).unwrap(), [0., -48.]);
        assert_eq!(crossfade_db(0.0001).unwrap(), [0., -48.]);
        assert_eq!(crossfade_db(0.9999).unwrap(), [-48., 0.]);
        assert_eq!(crossfade_db(1.).unwrap(), [-48., 0.]);
    }
}
