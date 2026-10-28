//! Explicit source-context admission only. Actual render-world native proof is separate.
use crate::{graphics, lamp_materials, lamp_signals, track_surface, window_response};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DiagnosticRoute {
    Donington0,
    Istanbul0,
}
impl DiagnosticRoute {
    fn from_selected(path: &str, subroute: u8) -> Option<Self> {
        if subroute != 0 {
            return None;
        }
        match path {
            grid_tools::route::DONINGTON => Some(Self::Donington0),
            grid_tools::route::ISTANBUL_PARK => Some(Self::Istanbul0),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum CaptureProfile {
    #[default]
    Image450,
    Manual,
}
impl CaptureProfile {
    pub(crate) fn parse(value: &str) -> Result<Self, &'static str> {
        match value {
            "image450" => Ok(Self::Image450),
            "manual" => Ok(Self::Manual),
            _ => Err("--native-smoke-profile needs image450 or manual"),
        }
    }
}
/// Raw argument presence, independent of a neutral effective control value.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ScriptFlags(u16);
impl ScriptFlags {
    pub(crate) fn note_arg(&mut self, arg: &str) {
        let bit = match arg {
            "--drive-ticks" => 1,
            "--render-fps" => 2,
            "--motion-capture" => 4,
            "--render" => 8,
            "--throttle" => 16,
            "--steering" => 32,
            "--handbrake" => 64,
            "--capture-timeout" => 128,
            "--render-fixed-step" => 256,
            "--motion-orbit-rate" => 512,
            "--benchmark-frames" => 1024,
            "--view" => 2048,
            "--camera" => 4096,
            "--look-at" => 8192,
            _ => 0,
        };
        self.0 |= bit;
    }
    fn any(self) -> bool {
        self.0 != 0
    }
}
/// Native selectors cannot be dropped by an earlier supervisor/probe branch.
/// Ordinary menu/career/sample-probe invocation is unchanged.
pub(crate) fn validate_entry_args(args: &[String]) -> Result<(), &'static str> {
    let selected = args
        .iter()
        .any(|a| a == "--native-smoke-boundary" || a == "--native-smoke-profile");
    if selected
        && args.iter().any(|a| {
            matches!(
                a.as_str(),
                "--menu"
                    | "--career"
                    | "--menu-screen"
                    | "--career-screen"
                    | "--from-menu"
                    | "--coverage-sample-probe"
            )
        })
    {
        return Err(
            "native smoke requires direct --drive; menu/career/screen/probe supervisor entry is not admitted",
        );
    }
    Ok(())
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Arm {
    Off,
    Control,
    Late,
}
impl Arm {
    pub(crate) fn parse(s: &str) -> Result<Self, &'static str> {
        match s {
            "off" => Ok(Self::Off),
            "control" => Ok(Self::Control),
            "late" => Ok(Self::Late),
            _ => Err("--native-smoke-boundary needs off, control or late"),
        }
    }
    // Deliberately no Off short-circuit: explicit Off selects this SAME context.
    pub(crate) fn validate(self, c: Context<'_>) -> Result<Validated, &'static str> {
        if !c.feature_dlaa
            || !c.graphics.enhanced()
            || c.graphics.anti_aliasing != 3
            || c.graphics.reflection_mode != 3
            || !c.graphics.source_ibl
            || !c.graphics.source_sky
            || !c.graphics.layered_track
            || c.graphics.width != 1920
            || c.graphics.height != 1080
        {
            return Err(
                "native smoke requires feature=dlaa/enhanced forward AA3/probe3/source IBL+sky+track/native1080",
            );
        }
        let Some(route) = DiagnosticRoute::from_selected(c.route, c.subroute) else {
            return Err("native smoke admits only BMW Donington0 or Istanbul0/sub0");
        };
        if !c.explicit_car || c.car != grid_tools::car::BMW {
            return Err("native smoke admits only explicitly selected BMW on the validated route");
        }
        if !c.scene.drive
            || !c.scene.track
            || c.scene.headless
            || c.scene.collision
            || c.scene.menu
            || !c.scene.solo
            || !c.scene.undamaged
            || !c.scene.keyboard
            || c.scene.lod != "LOD0_"
        {
            return Err(
                "native smoke requires solo undamaged LOD0 keyboard track driving, no menu/headless/collision/event",
            );
        }
        if !c.clean.explicit_data_dir
            || c.clean.loaded_files != 0
            || !c.clean.diagnostics_clean
            || !c.clean.no_perf
        {
            return Err(
                "native smoke needs explicit clean --data-dir/loaded_file_count0, no diagnostics except ground trace and no PERF/TIMING",
            );
        }
        match c.capture_profile {
            CaptureProfile::Image450 => {
                if !c.capture.image
                    || c.capture.ticks != Some(450)
                    || c.capture.fps != Some(30)
                    || !c.capture.motion
                    || !c.capture.exact_window
                    || c.capture.handbrake != Some(190)
                    || c.capture.throttle != 1.
                    || c.capture.steering != 1.
                {
                    return Err(
                        "native smoke Image450 requires Image450/fps30/motion190,90/HB190/throttle1/steering1",
                    );
                }
            }
            CaptureProfile::Manual => {
                if !c.capture.native_window
                    || c.capture.script_flags.any()
                    || c.capture.image
                    || c.capture.ticks.is_some()
                    || c.capture.fps.is_some()
                    || c.capture.motion
                    || c.capture.exact_window
                    || c.capture.handbrake.is_some()
                    || c.capture.throttle != 0.
                    || c.capture.steering != 0.
                    || c.graphics.capture_timeout_seconds.is_some()
                {
                    return Err(
                        "native smoke Manual rejects all explicit scripted/capture/clock flags, even neutral values",
                    );
                }
            }
        }
        let g = c.graphics;
        if g.layered_car
            || g.tread_normals
            || g.smooth_tyres
            || g.clearcoat_prepass_fix
            || g.source_body_occlusion
            || g.source_body_specular
            || g.white_body_specular
            || g.source_race_numbers
            || g.player_race_number.is_some()
            || g.paint_environment != 0.
            || c.paint_response_explicit
            || g.lamp_lenses != lamp_materials::Mode::Off
            || g.lamp_reflectors != lamp_materials::Mode::Off
            || g.lamp_signals != lamp_signals::Mode::Off
            || g.window_pilot != window_response::Mode::Off
        {
            return Err(
                "native smoke requires legacy car/off normals/source tyres/stock prepass and no body masks/numbers/paint/lamps/windows pilots",
            );
        }
        if g.smooth_wheel_motion
            || g.adaptive_wheel_exposure
            || g.wheel_shutter_seconds != 0.
            || g.wheel_geometry_shutter_seconds != 0.
            || g.wheel_max_arc.is_some()
            || g.motion_shutter != 0.
            || g.dlaa_reset_every_frame
            || g.thin_motion_antenna
        {
            return Err(
                "native smoke requires source wheel motion and all wheel/temporal/shutter pilots off",
            );
        }
        if !c.surface_specular_explicit
            || g.surface_specular != track_surface::Specular::Off
            || g.surface_view_mip_bias
            || g.asphalt_soften != 0.
            || g.asphalt_filter_strength != 1.
            || g.ground_edge_pilot
            || g.source_land
            || g.source_trees
            || g.tree_sheet_colours
            || g.foliage_coverage
            || g.track_grade
            || g.saved_grade.is_some()
            || g.saved_fog.is_some()
            || g.post_saturation_override.is_some()
            || g.moving_probe
            || g.benchmark_frames.is_some()
        {
            return Err(
                "native smoke requires explicit surface-specular off/land off/trees off and no surface/grade/probe-update/benchmark pilots",
            );
        }
        if !c.exclusions.phases_off
            || !c.exclusions.scene_pilots_off
            || !c.exclusions.camera_off
            || !c.exclusions.groups_ground
            || !c.exclusions.overrides_off
        {
            return Err(
                "native smoke requires Groups/ground, early ground/skid/compositor/mapped/audit/scene/camera/override pilots off",
            );
        }
        Ok(Validated {
            arm: self,
            route,
            capture_profile: c.capture_profile,
        })
    }
}
#[derive(Clone, Copy)]
pub(crate) struct Context<'a> {
    pub feature_dlaa: bool,
    pub graphics: &'a graphics::Settings,
    pub explicit_car: bool,
    pub car: &'a str,
    pub route: &'a str,
    pub subroute: u8,
    pub scene: Scene<'a>,
    pub clean: Clean,
    pub capture: Capture,
    pub capture_profile: CaptureProfile,
    pub exclusions: Exclusions,
    pub paint_response_explicit: bool,
    pub surface_specular_explicit: bool,
}
#[derive(Clone, Copy)]
pub(crate) struct Scene<'a> {
    pub drive: bool,
    pub track: bool,
    pub headless: bool,
    pub collision: bool,
    pub menu: bool,
    pub solo: bool,
    pub undamaged: bool,
    pub keyboard: bool,
    pub lod: &'a str,
}
#[derive(Clone, Copy)]
pub(crate) struct Clean {
    pub explicit_data_dir: bool,
    pub loaded_files: usize,
    pub diagnostics_clean: bool,
    pub no_perf: bool,
}
#[derive(Clone, Copy)]
pub(crate) struct Capture {
    pub image: bool,
    pub native_window: bool,
    pub motion: bool,
    pub script_flags: ScriptFlags,
    pub ticks: Option<u64>,
    pub fps: Option<u32>,
    pub exact_window: bool,
    pub handbrake: Option<u64>,
    pub throttle: f32,
    pub steering: f32,
}
#[derive(Clone, Copy)]
pub(crate) struct Exclusions {
    pub phases_off: bool,
    pub scene_pilots_off: bool,
    pub camera_off: bool,
    pub groups_ground: bool,
    pub overrides_off: bool,
}
/// Source/CLI context token ONLY; never proof of SDK support or native rendering.
/// Private fields prevent an unvalidated main guard exception.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Validated {
    arm: Arm,
    route: DiagnosticRoute,
    capture_profile: CaptureProfile,
}
impl Validated {
    pub(crate) fn matches_route(self, path: &str, subroute: u8) -> bool {
        DiagnosticRoute::from_selected(path, subroute) == Some(self.route)
    }
    pub(crate) fn route(self) -> DiagnosticRoute {
        self.route
    }
    pub(crate) fn capture_profile(self) -> CaptureProfile {
        self.capture_profile
    }
    pub(crate) fn arm(self) -> Arm {
        self.arm
    }
    pub(crate) fn installs_boundary(self) -> bool {
        self.arm == Arm::Late
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    pub(super) fn graphics() -> graphics::Settings {
        graphics::Settings {
            preset: graphics::Preset::Enhanced,
            anti_aliasing: 3,
            reflection_mode: 3,
            source_ibl: true,
            width: 1920,
            height: 1080,
            layered_car: false,
            tread_normals: false,
            smooth_tyres: false,
            clearcoat_prepass_fix: false,
            smooth_wheel_motion: false,
            source_land: false,
            ..Default::default()
        }
    }
    pub(super) fn context(g: &graphics::Settings) -> Context<'_> {
        Context {
            feature_dlaa: true,
            graphics: g,
            explicit_car: true,
            car: grid_tools::car::BMW,
            route: "tracks/europe/donington/route_0",
            subroute: 0,
            scene: Scene {
                drive: true,
                track: true,
                headless: false,
                collision: false,
                menu: false,
                solo: true,
                undamaged: true,
                keyboard: true,
                lod: "LOD0_",
            },
            clean: Clean {
                explicit_data_dir: true,
                loaded_files: 0,
                diagnostics_clean: true,
                no_perf: true,
            },
            capture_profile: CaptureProfile::Image450,
            capture: Capture {
                image: true,
                native_window: true,
                motion: true,
                script_flags: ScriptFlags::default(),
                ticks: Some(450),
                fps: Some(30),
                exact_window: true,
                handbrake: Some(190),
                throttle: 1.,
                steering: 1.,
            },
            exclusions: Exclusions {
                phases_off: true,
                scene_pilots_off: true,
                camera_off: true,
                groups_ground: true,
                overrides_off: true,
            },
            paint_response_explicit: false,
            surface_specular_explicit: true,
        }
    }
    const ARMS: [Arm; 3] = [Arm::Off, Arm::Control, Arm::Late];
    #[test]
    fn exact_shared_context_accepts_all_arms_but_only_late_registers_boundary() {
        let g = graphics();
        for arm in ARMS {
            let token = arm.validate(context(&g)).unwrap();
            assert_eq!(token.arm(), arm);
            assert_eq!(token.installs_boundary(), arm == Arm::Late);
        }
        assert_eq!(Arm::parse("off"), Ok(Arm::Off));
        assert!(Arm::parse("anything").is_err());
        // Control means ordinary-source admission control, NOT neutral late reorder.
    }
    #[test]
    fn actual_off_and_control_entry_register_no_boundary_or_renderer_state() {
        use bevy::prelude::*;
        let g = graphics();
        for arm in [Arm::Off, Arm::Control] {
            let mut app = App::new();
            super::super::install_native_diagnostic(&mut app, arm.validate(context(&g)).unwrap())
                .unwrap();
            assert!(!app.world().contains_resource::<super::super::LiveState>());
            assert!(
                !app.world()
                    .contains_resource::<super::super::NativeLatePolicy>()
            );
            assert!(
                !app.world()
                    .contains_resource::<crate::smoke_boundary_core::Proof>()
            );
            assert!(app.get_sub_app(bevy::render::RenderApp).is_none());
        }
        // This verifies registration semantics, not pixel identity or native GPU support.
    }
    #[test]
    fn every_arm_including_explicit_off_rejects_wrong_identity_feature_and_profile() {
        let g = graphics();
        for arm in ARMS {
            let base = context(&g);
            for bad in [
                Context {
                    feature_dlaa: false,
                    ..base
                },
                Context {
                    explicit_car: false,
                    ..base
                },
                Context { car: "lac", ..base },
                Context {
                    route: "tracks/europe/nurburgring/route_0",
                    ..base
                },
                Context {
                    subroute: 1,
                    ..base
                },
                Context {
                    clean: Clean {
                        loaded_files: 1,
                        ..base.clean
                    },
                    ..base
                },
                Context {
                    clean: Clean {
                        explicit_data_dir: false,
                        ..base.clean
                    },
                    ..base
                },
                Context {
                    clean: Clean {
                        no_perf: false,
                        ..base.clean
                    },
                    ..base
                },
                Context {
                    clean: Clean {
                        diagnostics_clean: false,
                        ..base.clean
                    },
                    ..base
                },
                Context {
                    capture: Capture {
                        image: false,
                        ..base.capture
                    },
                    ..base
                },
                Context {
                    capture: Capture {
                        exact_window: false,
                        ..base.capture
                    },
                    ..base
                },
                Context {
                    scene: Scene {
                        undamaged: false,
                        ..base.scene
                    },
                    ..base
                },
                Context {
                    exclusions: Exclusions {
                        phases_off: false,
                        ..base.exclusions
                    },
                    ..base
                },
                Context {
                    exclusions: Exclusions {
                        camera_off: false,
                        ..base.exclusions
                    },
                    ..base
                },
                Context {
                    exclusions: Exclusions {
                        groups_ground: false,
                        ..base.exclusions
                    },
                    ..base
                },
                Context {
                    paint_response_explicit: true,
                    ..base
                },
                Context {
                    surface_specular_explicit: false,
                    ..base
                },
            ] {
                assert!(
                    arm.validate(bad).is_err(),
                    "{arm:?} accepted incompatible source context"
                );
            }
        }
    }
    #[test]
    fn real_graphics_fields_reject_legacy_mask_temporal_and_surface_hazards_for_all_arms() {
        let changes: [fn(&mut graphics::Settings); 13] = [
            |g| g.anti_aliasing = 1,
            |g| g.reflection_mode = 0,
            |g| g.layered_car = true,
            |g| g.tread_normals = true,
            |g| g.smooth_tyres = true,
            |g| g.clearcoat_prepass_fix = true,
            |g| g.source_body_specular = true,
            |g| g.source_body_occlusion = true,
            |g| g.source_race_numbers = true,
            |g| g.motion_shutter = 0.1,
            |g| g.dlaa_reset_every_frame = true,
            |g| g.source_land = true,
            |g| g.surface_specular = track_surface::Specular::SourceBase,
        ];
        for change in changes {
            let mut g = graphics();
            change(&mut g);
            for arm in ARMS {
                assert!(
                    arm.validate(context(&g)).is_err(),
                    "{arm:?} accepted a material/profile hazard"
                );
            }
        }
    }
}

#[cfg(test)]
mod route_manual_tests04 {
    use super::*;
    fn context<'a>(
        g: &'a graphics::Settings,
        path: &'a str,
        profile: CaptureProfile,
    ) -> Context<'a> {
        // Existing accepted fixture is defined in the sibling test module below.
        let mut c = super::tests::context(g);
        c.route = path;
        c.capture_profile = profile;
        if profile == CaptureProfile::Manual {
            c.capture = Capture {
                image: false,
                native_window: true,
                motion: false,
                script_flags: ScriptFlags::default(),
                ticks: None,
                fps: None,
                exact_window: false,
                handbrake: None,
                throttle: 0.,
                steering: 0.,
            };
        }
        c
    }
    #[test]
    fn exact_routes_profiles_and_arms_validate_and_tokens_bind_selected_route() {
        let g = super::tests::graphics();
        for path in [
            grid_tools::route::DONINGTON,
            grid_tools::route::ISTANBUL_PARK,
        ] {
            for profile in [CaptureProfile::Image450, CaptureProfile::Manual] {
                for arm in [Arm::Off, Arm::Control, Arm::Late] {
                    let token = arm.validate(context(&g, path, profile)).unwrap();
                    assert!(token.matches_route(path, 0));
                    assert!(!token.matches_route(path, 1));
                    assert_eq!(token.capture_profile(), profile);
                    assert_eq!(token.installs_boundary(), arm == Arm::Late);
                }
            }
        }
    }
    #[test]
    fn manual_explicit_neutral_flags_are_rejected_for_every_arm() {
        let g = super::tests::graphics();
        for flag in [
            "--throttle",
            "--steering",
            "--handbrake",
            "--drive-ticks",
            "--render-fps",
            "--motion-capture",
            "--render",
            "--capture-timeout",
            "--render-fixed-step",
            "--motion-orbit-rate",
            "--benchmark-frames",
            "--view",
            "--camera",
            "--look-at",
        ] {
            let mut c = context(&g, grid_tools::route::ISTANBUL_PARK, CaptureProfile::Manual);
            c.capture.script_flags.note_arg(flag);
            for arm in [Arm::Off, Arm::Control, Arm::Late] {
                assert!(arm.validate(c).is_err(), "{flag} unexpectedly admitted");
            }
        }
    }
    #[test]
    fn manual_non_native_window_policy_is_rejected_before_runtime() {
        let g = super::tests::graphics();
        let mut c = context(&g, grid_tools::route::ISTANBUL_PARK, CaptureProfile::Manual);
        c.capture.native_window = false;
        for arm in [Arm::Off, Arm::Control, Arm::Late] {
            assert!(arm.validate(c).is_err());
        }
    }
    #[test]
    fn native_foreign_entry_refusal_is_before_supervisors_but_ordinary_entry_unchanged() {
        for flag in [
            "--menu",
            "--career",
            "--menu-screen",
            "--career-screen",
            "--from-menu",
            "--coverage-sample-probe",
        ] {
            assert!(validate_entry_args(&[flag.into()]).is_ok());
            assert!(
                validate_entry_args(&[flag.into(), "--native-smoke-boundary".into(), "off".into()])
                    .is_err()
            );
            assert!(
                validate_entry_args(&[
                    "--native-smoke-profile".into(),
                    "manual".into(),
                    flag.into()
                ])
                .is_err()
            );
        }
        assert!(
            validate_entry_args(&[
                "--drive".into(),
                "--native-smoke-profile".into(),
                "manual".into()
            ])
            .is_ok()
        );
    }
}
