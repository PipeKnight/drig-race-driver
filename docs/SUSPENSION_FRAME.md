# Joined suspension frames and carried state — F163

Recorded 10 October 2026. `grid-world::suspension_frame` joins four-wheel body coupling, the delta-dependent suspension substep schedule, complete two-pass body finalization and supplied post-solver rate contributions. It is a renderer-independent frame API. Geometry, contact state and rates are supplied; this does not establish a complete vehicle update or a drivable car.

The original routines execute consecutively in an isolated, hash-pinned private image: whole `9E3970` coupling, repeated whole `9C1F60` substeps, then whole `9E4B10` finalization, including their original dependencies. No helper is replaced. The harness authors the schedule and rate additions from the previously compared F064 wrapper contract; it does **not** execute whole `9F1E10`, its preparation `9E41F0`, drivetrain initialization or the complete car job. This boundary is deliberate and remains explicit.

## Joined contract

1. Generate four baseline body-axis speeds and the two normal-response matrices from supplied body state, wheel world points, suspension axes and projection axes. Coupling uses incoming inertia; the original then installs supplied alternate inertia in its body.
2. Select `max(1, trunc(delta * 999.9999389648438))` substeps. Compute the reciprocal count and sub-delta in f32. Start weight at one and subtract the reciprocal after each call. Accumulate all suspension, travel-stop and normal-contact impulses across the frame.
3. Finalization uses the incoming whole-frame delta. Original `9F1E10` adds that delta to the reset context clock once before the loop; summing rounded sub-deltas would be a different contract. Complete finalization commits pose from weighted impulses and velocities from raw impulses, retains alternate inertia, publishes wheel displacement/speed/floor difference/force, consumes all four per-wheel accumulators and clears the clock.
4. Apply the supplied pre-solver linear/angular rates with the observed whole-frame additions. Preserve the committed pose, including its separate half-delta rate correction. Do not predict pose again from final velocity.
5. `carry` retains resulting body, wheel displacement/speed, contact floors/speeds/flags and force, while transferring consumed accumulators. Callers must refresh geometry and contact/rate inputs for actual motion. The diagnostic deliberately retains fixed world points and axes across its controlled sequences; those sequences are not physical road trajectories.

The wheel travel denominator and finalizer clip share original wheel offset `+4c`. The joined API requires their bits to agree rather than changing the original field between stages. It also requires body position to agree bitwise with basis row 3. Existing finite, mass/inertia, projection and travel-envelope restrictions remain authored diagnostic guards.

The installed Standard CSV rows and retained production flags all disable the extra coupled tyre-axis branch; see [production selection](TYRE_SELECTION.md). This frame therefore follows that observed suspension branch. Longitudinal/lateral vehicle forces still come from later production consumers and are not established by this comparison. Simply enabling the alternative coupled-axis branch would not reproduce the reference Lacetti's selected model.

## Comparison and evidence

Original, repeated original and one Rust frame-sequence call match **216 scenarios, 2,552 frames and 357,280 compared words** exactly, separately in debug and explicit release. Fixtures include 32 controls, 24 combinations of previously verified recorded numeric inputs and 160 seeded scenarios. Eight controls carry 120 frames; recorded combinations carry 12 and seeded scenarios carry eight. Substep counts range from one to 249, including the `.01` nine-step boundary and approximately 1/60 sixteen-step scheduling. Recorded combinations are supplied boundary replays, not synchronized live captures.

Compared output includes schedule count/sub-delta/last weight, all 36 coupling words, 40 pre-finalization wheel/accumulator words and 61 final body/wheel/clock words. Five independent semantic checks cover cleared clock, forwarded displacement/speed, consumed accumulators, f32 force publication from raw impulse and nontrivial carried pose/force changes. There are 10,208 wheel force changes and 2,552 pose changes. Ten ordinary malformed-input controls reject invalid frame counts, nonfinite/zero delta, mass/inertia, scale, alternate inertia, contact flag and shared travel disagreement without creating an output.

Private final reports:

- Debug: `local/behavior/suspension-frame-20261010-debug-final-02/validation.json`.
- Explicit release: `local/behavior/suspension-frame-20261010-release-explicit-03/validation.json`.
- Initial debug comparison: `suspension-frame-20261010-debug-01/validation.json`.
- `suspension-frame-20261010-release-final-02/validation.json` is **mislabelled debug evidence**: its invocation omitted `--profile release`, and the report records debug. It is retained and excluded from release claims. The validator now requires an explicit `--profile` and prints that profile with its summary to prevent repetition.

The retained input files are checked against their previous validation hashes: `isolated-wheel-step-20261007-05/input.tsv` and `isolated-finalize-20261007-03/input.tsv`. Source and execution hashes are retained in each new report. All game-derived numeric data and outputs remain ignored under `local/`.

| Artifact | SHA-256 |
|---|---|
| Analysis PE | `aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514` |
| Native joined probe | `2307097bb1b31c3106647785a7075506e92b9cdf943f42bde40adfccfc9b678c` |
| Shared request | `2f30f9fd1ce1367026d5dcffb86c07ee71e264bb8e245d3c60d62d56fa561b68` |
| Debug diagnostic | `a7475f4931bac9b899886fc1db4440770c56ecce6cd113da20e39f10b5b3e641` |
| Release diagnostic | `706466e9d29eb0c00edb494dc0138a288181d0e40d3e318001e1e84dedf817ab` |

The release diagnostic hash is recorded in the explicit-release report. The native mapper executes no entry point/import/TLS initialization, keeps code RX, temporarily permits writes only on four private lazy-data pages, restores their bytes/protections and verifies every executable section remains unchanged. No original installation or live process is modified.

Independent authored tooling: `probe_suspension_frame.c`, its build script, `validate_suspension_frame.py` and the private-output-only `grid-suspension-frame` diagnostic. TSV cases contain a name, hexadecimal u32 frame count (1–120), 41 f32 body/frame words and four 35-word wheel records. Outputs retain every frame rather than only its last state. Run the validator with an explicit `--profile debug` or `--profile release` and a new output directory below `local/`.

## Verification, limitations and next work

All **114 library tests** (20 formats, one tools, 93 world), formatting, all-target warnings-denied Clippy and Python AST parsing pass. The new integration test carries two frames, checking the nine-substep boundary, committed translation, unchanged raw velocity, retained alternate inertia and consumed impulses. No pre-existing arithmetic module was modified. Initial added test literals omitted Rust's leading zero; formatting rejected them, and the corrected source passes. No native fault or numerical mismatch occurred. A temporary Markdown newline edit removed more historical plan text than intended; the diff caught it and the original bytes were restored before applying the single-paragraph checkpoint change.

Tools reused: Rust/Cargo 1.99.0, Python 3.12.14, MSVC 14.44.35207 / SDK 10.0.26100.0, Visual Studio developer prompt 17.14.41. Native flags `/W4 /WX /O2 /MT /std:c17 /fp:strict /arch:SSE2`. No installation or desktop control. Steam preservation at **11:49:43.010696 UTC** confirms all 130 traced resources and original executable unchanged: `local/scene/preservation-20261010-suspension-frame.json`.

Next recover and join the source-selected downstream wheel-force path and contact preparation, retaining the temporary force-view lifetime and caller order. Then connect its authoritative state transfer to the already joined [physical tick](PHYSICAL_TICK.md), with geometric contact production and Bevy controls/camera/restart. Whole vehicle/world initialization, active callbacks, long reference driving trajectories and a timed lap remain open. Completion estimates are unchanged; the expired overnight automation remains paused.
