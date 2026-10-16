# Tyre silhouette detail

15 October 2026, graphics worktree only. Owner explicitly requests smooth wheels and high detail rather than resolution alone. This is an independently authored presentation improvement, not recovered EGO rendering code.

## Source observation

Read-only `grid-formats` example `audit_wheel_geometry` reuses the validated PSSG/mesh/palette readers. Purchased Lacetti highLOD file, LOD0 and LOD1, 32 selected wheel/tyre/disc draws. Private reports: `local/graphics/wheel-geometry01.tsv`, `wheel-geometry02.tsv` (adds transformed bounding-box centers for repeatable cameras).

Nearest LOD tyre wall/tread outer samples have roughly 9-degree angular gaps (40 angular positions). Rims have a similar maximum gap; brake discs are 12-sided, roughly 30-degree gaps. LOD1 tyres have roughly 12-degree maximum gaps. The 98%-of-maximum-radius annulus metric is an audit approximation; sample count is not necessarily polygon side count, and it is not a measured screen-space error.

For a perfect circle joined with 9-degree chords, radial sag is about 0.308% of radius, approximately 0.94 mm at the source tread's 0.305 m radius. A 30-degree chord has about 3.4% sag. These geometric estimates explain potential close-view faceting; they are not a claim about the owner's current screenshot. First audit matched no joints because it omitted the `x0_`/`x1_` prefix; corrected to the existing viewer's exact kind/side mapping, then obtained 32 rows.

## Authored tyre refinement

Enhanced profile refines only `Tyre Wall` and `Tyre Tread` parts attached to wheels. Two subdivision rounds split every triangle into four and move each new edge midpoint onto the interpolation of endpoint radii around the local X axis. Axial position and UVs interpolate linearly; normals interpolate and normalize. Edge sharing is by vertex index, retaining separate vertices at UV seams. Original vertices and transforms remain untouched. Each tyre part has 16 times its original triangle count. Ideal constant-radius circumferential gaps fall from 9 to 2.25 degrees, reducing chord sag approximately sixteenfold.

This interpolation preserves the source's tyre cross-section samples; it does not invent tread geometry or replace texture detail. The bounding-box YZ center is an authored circularization assumption. Unsupported edges across the axis or over 90 degrees, absent/nonfinite streams and excessive topology are rejected before changing the mesh. This is deliberately restricted to known tyre surfaces; arbitrary rims/spokes, damaged bodywork and brake caps must not use this operation.

## Rotation presentation, 16 October

Owner requires good wheel appearance at 30 through 300 FPS. The physical clock is 62.5Hz (DELTA=0.016). Previously every display frame used exactly the latest physical phase, causing repeated wheel poses at high FPS. Enhanced now predicts only spin through the outstanding fixed-step remainder, capped to one physical step. Signed rad/s is read directly from source wheel+0x104: vehicle_propulsion integrates wheel+0x110 += spin*delta, vehicle_force updates spin. No wrapped-angle subtraction, road-speed approximation or persistent second wheel clock. Steering, suspension, damage orientation and physical state are unchanged. Apply the same policy to opponents. Pause, reset/start, failed simulation and completed scripted captures use exact physical phase. `--wheel-motion source|smooth` retains a control; baseline remains source.

This is bounded extrapolation, not a full vehicle interpolation scheme. It can differ from the next exact physical phase during rapid acceleration/braking by up to the prediction error of one16ms step. Two mathematical tests pass across30/60/144/300Hz, positive/negative/zero spin and multiple turns per step. Purchased-resource joint/no-feedback test passes with render samples; actual ECS remainder/pause/reset/source-control test passes. Initial test compile rejected an ambiguous float type, corrected to f32. AlltargetClippy passes. These establish phase and integration contracts, not visual acceptance of wheel blur at four real display rates. Spoke/lettering strobing still requires rotational exposure filtering; no claim it is fixed by phase prediction.

Physics, anchors, radii, source wheel orientation/rotation/deformation and collision geometry are unchanged. Refinement occurs at renderer startup, after source loading; it does not feed into vehicle state. Opponent instances share the refined render mesh. `--tyre-detail source|smooth` supports identical-profile comparisons; enhanced defaults smooth, baseline remains source.

## Validation and remaining work

Rust 1.99.0 / Bevy 0.19.1, installed tools reused, two build jobs. Synthetic cylinder checks radius/width, UV seam and angular refinement. Private source test checks all eight tyre parts in both LODs, original vertices, finite results and unchanged transforms: 2 tests passed. Initial vector conversion method-reference compilation error fixed with closures. Clippy constant chunk warnings fixed using `as_chunks`; release all-target Clippy passed. An unnecessary debug dependency check was interrupted and replaced with the populated release cache; not a completed check.

GPU pilot: matched native 3840×2160 close-ups `local/graphics/tyre-source01.png` and `tyre-smooth01.png`, camera `(2.0,0.44,1.36)`, target `(0.74,0.304,1.36)`, enhanced lighting/profile otherwise identical. Both inspected: smooth tyre edge, original rim/spokes retained; no visible detached tyre or newly introduced UV seam. Close-up still reveals low-resolution livery/tyre lettering and coarse wheel arch/brake detail. This confirms those need additional work, not a complete modern-quality wheel.

Matched 240-tick, 0.25-throttle Donington runs (`tyre-drive-source01`/`tyre-drive-smooth01`) exit normally. Final pose, velocity, contact hits, damage and handling state match exactly after excluding wall-clock duration. GPU logs show all eight intended tyre parts refined with no skipped part/error. First built executable header was `unversioned` because the compile-time `GRID_BUILD_REVISION` environment variable was omitted; these pilot captures are retained and the final versioned release must be verified separately.

Rim and brake-disc silhouettes remain original and require separate treatment. No claim that the complete wheel already meets the final quality contract. Next: source-sky IBL/material response, selective texture replacement, and targeted rim/disc geometry rather than blind subdivision of spokes or caps.

Final versioned release: source `3c5c0df23c51`, SHA256 `574e9142e028b96eb815ccde82805414fa3dd24a4977cae1395ddd3335cda613`; final matched 4K close-ups `tyre-source-final`/`tyre-smooth-final`, 240-tick Donington `tyre-drive-final`. Correct revision header, eight refinements/no skips/errors, pose/velocity/handling equal source-tyre pilot. Private `validation-tyre-final.json` and `current-build.json` hold hashes. Separate graphics launcher points at this release; the primary prototype is untouched. Manual/full-lap validation remains unobserved.
