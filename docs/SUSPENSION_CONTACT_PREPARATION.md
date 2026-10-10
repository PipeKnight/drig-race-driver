# Whole suspension preparation with source contacts — F167

Recorded 10 October 2026. Rust now executes the complete supported `9E41F0` preparation: previous-frame correction, wheel reset and geometry, external velocity rates, temporary body prediction, four real source-world casts and surface mechanics, pose restoration, response coupling and contact-speed publication. This connects existing verified components at their actual caller boundary. The full `9F1E10` wrapper, downstream forces and the physical manager are not yet one verified driving loop.

`grid-world::suspension_prepare::apply` operates on bounded owned state and a world-query callback. It never follows captured addresses. `grid-suspension-contact` supplies that callback using the purchased Donington `track.jpk` and `surface_materials.xml`, with one shared query cache across four wheel rays. The source-specific material index is preserved; object and triangle-slot addresses are normalized to presence, so their individual identity is outside the comparison.

## Contract and state ownership

- When the retained frame flag is set, transform wheel anchors through previous and current poses and compare their projected velocities. Previous pose is body `B40..B7F`, with older velocities `B80/B90`; the current pre-update velocities are `B20/B30`. Correct wheel displacement/speed and the retained floor before resetting accumulators. These three velocity snapshots have distinct roles.
- Clip incoming displacement only against wheel `4C`, copy spring/speed, clear the contact flag's low byte and four accumulators, then recompute geometry. Calculate current point speeds and `(current velocity − B20/B30) / delta`; restore `B20/B30` as the working velocities.
- Save physical state, predict pose, recompute wheel geometry, and perform real packed-world queries plus accepted-contact material noise, radius and normal processing. Restore the saved pose, recompute geometry again, move retained floor into the working floor and predicted floor into the next-floor field.
- Reuse the original four-wheel coupling contract with inertia from context `6F0`. Publish the two response matrices, baseline wheel speeds and `(baseline speed * delta + next floor − retained floor) / delta`. An absent floor sentinel makes that last term zero. Clear the clock.
- The body state-copy helper `A15580` uses x87 `FLD/FSTP` for its first two words. An authored signaling-NaN control at body `15A4` therefore becomes a quiet NaN on restoration. Rust preserves that bit-level side effect; other opaque padding remains unchanged.

The adapter supports four wheels, extra tyre axes off, the global static-world query branch and material-table offset zero. It requires finite consumed state, positive mass/inertia/radii, ordered travel bounds, and `0 < delta <= .1` with finite reciprocal. These are authored diagnostic guards. Dynamic preselected shapes and missing source mechanics are outside this API. Owned state commits only on success; callbacks may mutate their query cache and must stage it separately if cache rollback is needed.

## Whole-original comparison

The native adapter executes whole unchanged `9E41F0`, including original pose, geometry, real `9C1660/96B7F0`, packed traversal/material/radius/normal and `9E3970` coupling helpers. It reuses the independently built source-world and source-mechanics fixture; it does not replace query results or any helper. The original PE entry point/imports/TLS do not run. No live injection, code patch or Steam modification occurs. Code remains RX and every executable section is compared afterward. Four private lazy-data pages have bytes/protections restored; the world fixture's private data protections are restored before the isolated image is discarded.

Original, repeat and Rust match **120 varied cases plus 16 sequences of eight carried preparations: 248 calls / 603,136 complete body words per debug/release profile**. Initial cases produce **275 accepted wheel contacts**. Controls cover first/subsequent frame flags, moving and rotated previous/current poses, tilted suspension axes, signed-zero angular velocity, delta 1e-6 through .1, varied finite mass/inertia, radius samples, source surfaces, clock/speed/car factors, retained floors and arbitrary opaque padding. Each carried branch consumes its own previous output; caller pose/history advancement is explicitly authored. These are controlled sequences, not captured road trajectories or game ticks.

Six independent semantic checks cover restored pose and previous velocities, cleared clock/accumulators, exact external velocity rates, and consistent contact presence. **18 malformed cases reject in both original and Rust adapters** without creating an output; native rejection precedes original execution. An integration test proves state rollback on a world-query error and invalid delta.

Final private reports:

- `local/behavior/suspension-contact-20261010-debug-final-05/validation.json`.
- `local/behavior/suspension-contact-20261010-release-final-06/validation.json`.
- Existing source-contact regression: `source-contact-20261010-preparation-regression-debug-03/` and `release-04/`: **512 cases / 30,720 words**, all exact in each profile.

| Artifact | SHA-256 |
|---|---|
| Whole-preparation probe | `38a52d18d300c542231ea954214b3d2411f7183d275ace2a8f20251640bbbea9` |
| Debug diagnostic | `29500362ddd300cf530134563cd5dde392862ccfb3569fdda7048d395b1d58a2` |
| Release diagnostic | `60196b1a25bded6a3379657def8f9514df1aaf4edaabf0f89c1d5c5768c2e004` |
| Shared request | `746ce573a29482800efd2c8d4ad5ec0d60ea9d83a56cbe2edc0aa5e4444e90b0` |
| Original/repeat/Rust initial output | `00a3bf89b59f2c719ddbc80c9c17a534d5e6e55cddbc8331690fd641f5580f51` |
| Final carried output | `3c3c5d3ccecd664933bf62f72e78c77b9557af339296520a7c87331c8be7c10a` |

Public tools are `probe_suspension_prepare.c`, `suspension_prepare_probe.h`, its build script and `validate_suspension_contact.py`; the existing source-contact probe has only a conditional compile-time entry adapter. SP01 uses an eight-byte tag/count header and at most 128 records, each index/delta plus `2600` hex bytes of body state. Consumed native pointers must be zero, are relocated only to owned buffers, checked after the call, then normalized. Source/evidence/executable hashes are pinned in the reports. All proprietary fixtures and numeric evidence remain ignored below `local/`.

## Failures, validation and next work

Initial compilation caught an unused cache variable under `/WX`; fixed before execution. The first yaw-only controls matched; widening to tilted/rotated history exposed one word in case105/body15A4, identical in debug/release. Aligned `A15580` instructions established x87 NaN quieting, which corrected the mismatch without masking output. Earlier failed reports `debug-final-03` / `release-final-04` are retained and superseded. A range-loop lint and two constant-chunk iterator lints were corrected; no native fault occurred.

All **120 library tests** (20 formats, one tools, 99 world), formatting, all-target warnings-denied Clippy and Python AST checks pass. Reused Rust/Cargo1.99.0, Python3.12.14, MSVC14.44.35207/SDK10.0.26100.0, developer prompt17.14.41, Ghidra12.1.4/JDK21.0.9 and Capstone5.0.6. Native flags remain strict SSE2 `/W4 /WX /O2 /MT /std:c17 /fp:strict /arch:SSE2`; x87 precision53/nearest as in previous source-contact comparisons. No tool installation or desktop input.

Fresh static evidence: six successful functions under `local/behavior/20261010T1407195943352Z/`; complete aligned preparation/coupling/geometry in `suspension-prepare-aligned-20261010.json`, and aligned state-copy evidence in `suspension-state-copy-aligned-20261010.json`. Steam preservation at **14:24:12.174763 UTC** confirms 130 resources plus the executable unchanged: `local/scene/preservation-20261010-suspension-contact.json`.

Next execute the actual outer `9F1E10` wrapper, including inertia setup and complete preparation→substeps→finalization, then join selected tyre forces and authoritative physical-body publication. Bevy input/camera and the first driven lap remain pending. This result does not change the dated completion estimate or establish synchronized live suspension behavior. The expired overnight automation remains paused.
