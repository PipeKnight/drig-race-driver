//! Default-off bounded baseline transparency experiment; not a smoke quality preset.
//! OIT stores individual fragments, avoiding whole-mesh ordering, but has finite capacity.
use bevy::{core_pipeline::oit::OrderIndependentTransparencySettings, prelude::*};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Resource)]
pub(crate) enum Mode { #[default] Off, Control, Oit16, Oit32, Oit32Pruned }
impl Mode {
    pub(crate) fn parse(value: &str) -> Result<Self, &'static str> {
        match value { "off" => Ok(Self::Off), "control" => Ok(Self::Control),
            "oit16" => Ok(Self::Oit16), "oit32" => Ok(Self::Oit32),
            "oit32-pruned" => Ok(Self::Oit32Pruned),
            _ => Err("--smoke-compositor needs off, control, oit16, oit32 or oit32-pruned") }
    }
    fn settings(self) -> Option<OrderIndependentTransparencySettings> {
        let average = match self { Self::Oit16 => 16., Self::Oit32 | Self::Oit32Pruned => 32., _ => return None };
        Some(OrderIndependentTransparencySettings { sorted_fragment_max_count: 32,
            fragments_per_pixel_average: average,
            // Conservative below the pinned OIT u8 alpha half-step: discarded
            // finite nonnegative values already pack to alpha0 in this compositor.
            // Not equivalent to stock float alpha blending or fixed-capacity output.
            alpha_threshold: if self == Self::Oit32Pruned { 0.49 / 255. } else { 0. } })
    }
}
pub(crate) fn install(app: &mut App, mode: Mode) {
    if mode == Mode::Off { return; }
    app.insert_resource(mode).add_systems(PostStartup, configure);
    println!("smoke_compositor mode={mode:?} scope=main-baseline-camera msaa=off sorted_fragments=32 capacity_average={:?} alpha_threshold={:?} capacity_overflow_unmeasured=true default=false", mode.settings().map(|s| s.fragments_per_pixel_average), mode.settings().map(|s| s.alpha_threshold));
}
fn configure(mut commands: Commands, mode: Res<Mode>, camera: Single<Entity, With<super::ViewerCamera>>) {
    let mut camera = commands.entity(*camera);
    // The control disables MSAA too; MSAA is unsupported by the pinned OIT implementation.
    camera.insert(Msaa::Off);
    if let Some(settings) = mode.settings() { camera.insert(settings); }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn off_touches_nothing_and_control_changes_only_msaa() {
        let mut app = App::new();
        let camera = app.world_mut().spawn((super::super::ViewerCamera, Msaa::Sample4)).id();
        install(&mut app, Mode::Off); app.update();
        assert!(!app.world().contains_resource::<Mode>());
        assert_eq!(*app.world().get::<Msaa>(camera).unwrap(), Msaa::Sample4);
        for mode in [Mode::Control, Mode::Oit16, Mode::Oit32, Mode::Oit32Pruned] {
            let mut app = App::new();
            let camera = app.world_mut().spawn((super::super::ViewerCamera, Msaa::Sample4)).id();
            install(&mut app, mode); app.update();
            assert_eq!(*app.world().get::<Msaa>(camera).unwrap(), Msaa::Off);
            let actual = app.world().get::<OrderIndependentTransparencySettings>(camera);
            if mode == Mode::Control { assert!(actual.is_none()); }
            else { let actual=actual.unwrap(); assert_eq!(actual.sorted_fragment_max_count,32);
                assert_eq!(actual.alpha_threshold,mode.settings().unwrap().alpha_threshold); assert_eq!(actual.fragments_per_pixel_average,mode.settings().unwrap().fragments_per_pixel_average); }
        }
    }
    #[test]
    fn pruned_threshold_stays_below_pinned_u8_alpha_quantizer_half_step() {
        let threshold=Mode::Oit32Pruned.settings().unwrap().alpha_threshold;
        assert_eq!(threshold.to_bits(),0x3afbdd25);
        let greatest_rejected=f32::from_bits(threshold.to_bits()-1);
        // Formula from pinned oit_draw.wgsl, not an alternative blend equation.
        for alpha in [0., f32::MIN_POSITIVE, 0.001, greatest_rejected] {
            assert!(alpha < threshold);
            assert_eq!((alpha.clamp(0.,1.)*255. + 0.5) as u32,0);
        }
        assert_eq!((greatest_rejected*255.).mul_add(1.,0.5) as u32,0);
        assert!(threshold < 0.5/255.);
        assert_eq!(Mode::Oit32.settings().unwrap().alpha_threshold,0.);
    }
    #[test]
    fn parser_is_explicit_and_default_off() {
        assert_eq!(Mode::default(),Mode::Off);
        for value in ["off","control","oit16","oit32","oit32-pruned"] { assert!(Mode::parse(value).is_ok()); }
        for value in ["on","oit","OIT32",""] { assert!(Mode::parse(value).is_err()); }
    }
}
