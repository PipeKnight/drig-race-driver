# Camera and cockpit roadmap

16 October 2026: owner requests a detailed realistic cockpit and a configurable camera suite inspired by Neck FX, retaining the pleasant original GRID movement. This is graphics-worktree scope; physics remains read-only input. No camera implementation or cockpit quality milestone is claimed by this document.

## Verified starting point

Update 17 October 2026 (F349): the original's five in-race views, look-back and the saved view are ported from each car's cameras.xml; see [CAMERA_VIEWS.md](CAMERA_VIEWS.md). The paragraph below describes the state before that.

Our current driving camera in grid-app/main.rs is an authored chase transform (car-relative target/offset, world-up look-at), right-mouse free look and C look-back. It does not implement original head/body dynamics or cockpit rendering.

Read-only purchased cars/lac/cameras.xml is sectioned binary XML, not plain XML. Existing tools/validate_binxml.py decoded 20 views; private source hash, structural statistics and per-view parameters are in local/graphics/lacetti-camera-audit01.json. First plain-text attempt yielded binary noise; decoded tree is the evidence used here. Earlier search also used a nonexistent drive.rs and a Windows glob; corrected to main.rs and directory searches.

The Head/head-cam entry specifies highResInterior and DisableDriverHead, separate body/head inertia, damping, centering forces and displacement limits, FOV, near plane, tilt, movement/bump parameters and look limits. Chase views have lag, suspension response, roll/tilt and shake controls. These are observed configuration fields: equations, physical units, FOV convention and runtime use still require tracing. In this Lacetti entry autoLookAngleInterest is zero and headBuffeting is false; neither the string table nor field names prove that every effect is active. User-observed corner displacement is a reference to measure, not a verified reconstruction.

Neck FX primary documentation describes per-car head rotation bounds, pan limits and look-back offsets, editable during race/replay: https://github.com/ac-custom-shaders-patch/acc-extension-config/wiki/Cars-%E2%80%93-Neck-FX . That page is not a full current feature inventory. Attempted raw config URL returned an error; no settings inferred from it. Found GRID camera-position mods are inspiration candidates only, not installed, executed or copied; independent implementation is planned.

## Ordered implementation

1. Audit original camera definitions and coordinate/FOV conventions. Trace how original Head and Chase use acceleration, angular motion and suspension signals; compare braking, slalom, kerb and impact recordings. Separate direct configuration observations from authored or recovered equations.
2. Establish useful static cockpit: load the correct high-detail interior, eye point, near plane and visibility layers. Verify view through glass, hands/head visibility and dashboard scale. Exterior LOD alone is not assumed to be a finished cockpit. Inspect mirrors, instruments, steering animation and interior material/texture availability before promising them.
3. Introduce shared camera rigs: cockpit, bonnet, bumper, near/far chase. Each has independent saved FOV, seat/anchor position and orientation, look limits, smoothing and effect strengths; switching preserves settings. Display vertical and horizontal FOV clearly for the current aspect ratio; provide a monitor-size/distance calculator. Speed-dependent FOV is optional, with a fixed-FOV mode.
4. Add independent six-degree head/body response driven by actual local vehicle acceleration and angular motion, plus suspension/kerb and impact response. Model damped return and bounded displacement, use simulation/interpolation timing correctly and separate gravity from inertial acceleration. Reset filter history on teleport/restart and handle pause/large dt. Head motion must not change physics or amplify numerical solver noise.
5. Add optional corner gaze, drift/velocity gaze, horizon stabilization and manual look. Keep their gains, limits, delay and return separate; steering-only forced head turning is not a sufficient default model. Clamp the head inside plausible cabin limits without harsh snapping.
6. Provide live controls, versioned local profiles per car and camera, reset/import/export and presets: GRID-inspired, restrained realistic, static, cinematic. Every shake/sway effect can be disabled independently and globally. TrackIR/OpenTrack-style input is a later adapter, not required for the first cockpit.
7. Polish cockpit presentation: high-detail dashboard and readable instruments, animated steering/hands where assets allow, functional mirrors, convincing windshield and cabin light. Authored replacements or derivatives stay private when based on original assets. Coordinate data requirements with the engine agent without editing its checkout.

## Validation and future rendering integration

Use reproducible rest/slalom/braking/kerb/impact traces at multiple render rates, with identical vehicle state and bounded camera displacement. Verify pause/reset/switch behaviour, free-look blending, cabin clipping, unusual aspect ratios and original-vs-enhanced captures. Log rig/FOV/settings with captures. Original feel is judged against reference motion; exact equivalence is not claimed before measurement.

Ensure camera cuts reset TAA and future DLSS history; previous/current view-projection, jitter and motion vectors must describe the final composed camera transform. Cockpit and mirrors need deliberate visibility, exposure, render-layer and reflection rules. RTX/DLSS remain future work, not prerequisites for useful cameras.
