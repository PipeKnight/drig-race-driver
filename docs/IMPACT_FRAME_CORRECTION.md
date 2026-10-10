# Joined main and optional physical frame — F160

10 October 2026. Whole original **B7B850**, repeat and independent Rust `impact_frame::calculate` match **695 scenarios /18,560 normal and friction rows /3,895,485 output words in each debug and release build** with optional correction configured. This extends [F159's outer frame](IMPACT_FRAME.md) through the original optional B7B5E0 pass, followed by final force cleanup, damping, limits and pending-shift publication. No original instruction or helper is skipped/replaced.

The adapter still supplies impact pools only, no registered callbacks, empty joint/other pools and no linked bodies/shapes or wake-manager branch. All contact geometry, body roles and optional settings are authored controls; 183 prior handoffs retain returned F158 body/contact/row operands within the 695 F159 cases. This is complete execution of the specified branch, not live whole-frame validation or a drivable engine.

## Joined behavior

The outer optional guard requires nonzero manager +F4 iterations and +F0 relaxation greater than file-backed D85A50, observed positive zero. It follows successful main B7ACC0 and naturally empty impact callbacks; zero-main-iteration, all-asleep and capacity-failure branches bypass it. The independent API accepts `Option<Correction>` with iterations, softness, scale, threshold and relaxation. None preserves F159 behavior.

Normal rows from the main pass feed the previously verified [optional correction](IMPACT_CORRECTION.md). Contacts retain their original accumulated operands and disabled flags, in two-body then single-body order. Main impulse publications remain intact; optional correction resets/reuses normal-row coefficients and workspace, excludes tangent spans, and publishes corrected body/derived matrices only on its active branch. Final damping/limits and pending shifts use the newly corrected basis. Main cached inverse properties are retained while published torque matrices can change with orientation.

For **empty contact pools**, original span ends retain their previous values and can exceed the current allocation. The integration must derive the optional normal-row count from contact count, not stale span ends. It preserves rows, workspace and ends on this bypass. This was a real adapter defect found by the first execution and is covered by a regression test.

The separate `grid-corrected-physical-frame` diagnostic uses PFR2: original PFR1 framing plus five words after each 144-byte case header (iterations and four settings). Its native probe initializes original +F4/+E0/+E4/+E8/+F0. Output framing is unchanged except magic. Tangent pointers canonicalize across the full constructed row set even when optional correction reduces active span ends. Existing `grid-physical-frame` supplies None; the unrelated older `grid-impact-frame` is unchanged.

## Verification

| Coverage | Count |
|---|---:|
| Optional pass invoked / active / inactive |300 /217 /83|
| Scenarios with changed pose/matrices versus correction disabled |214|
| Zero optional iterations / nonpositive relaxation controls |100 /234|
| Main normal / friction rows |8,708 /9,852|
| Empty contact scenarios |57|

Invocation/activity counts are derived from the authored settings and original output branch predicates, not a new live trace. Every original/repeat/Rust output word matches, including all raw body/contact/row bytes with owned pointer canonicalization, manager ends, cache, inverse properties, workspace and return status. Original code stays RX and executable sections remain byte-identical; five writable lazy-data pages restore their protections. No imports, stubs or Steam installation modifications are used.

Seven semantic checks pass: bypass equality, main contact retention, tangent/unused row retention, cache/inverse-property retention, optional span reset, normal-row reset/softness and final force clearing. Eleven malformed requests reject ordinarily in both native and Rust, without native exception or rejected Rust output: truncation, trailing data, zero cases, excessive optional iterations, four NaN settings, registered callback, linked body and stale contact metadata. Fresh F159 original/rebuilt Rust regression matches all 695 scenarios and 3,895,485 words per build.

**111 library tests** (20 formats, 1 tools, 90 world), formatting, all-target warnings-denied Clippy and Python validator AST pass. New tests cover empty correction with stale span ends and the outer iteration/relaxation guards.

Failures retained:

- Overnight first execution was prevented by automatic approval review's service usage limit. It did not execute; no bypass. Owner resumed work later, and normal approval review then succeeded.
- Actual debug-01 execution completed original/repeat, then Rust panicked on an empty-contact case with stale span ends. Count selection fixed; debug-02 matches fully. Native fault was not involved.
- A directory labelled release-02 contains another **debug** run because `--profile release` was omitted. It is not release evidence. The explicit release-final-03 report below identifies release and its distinct executable hash.
- Earlier generated diagnostic discarded parsed optional settings; fixed and rebuilt before these comparisons. The historical draft handoff remains in IMPACT_FRAME.md and PROJECT_LOG.md; this completed verification supersedes that draft status.

| SHA256 | Value |
|---|---|
| Analysis PE |aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514|
| Native probe |d5061ab307ece7dae07d97e3a2915962facc6f5867f692a4377a7ce9e01acfbe|
| Shared request |7144ff633938b118bd4ff832200b62cf357ce45f9d8a404b927465708592a130|
| Debug diagnostic |d39a4d9a8a189f1c7313a9b2716d07fada3763e85b339df344c16a86e956fa87|
| Release diagnostic |bed964886d77ced9e4b36193948021e76940be218267ec6a0a6b579724ec347d|

Private reports: `local/behavior/corrected-impact-frame-20261010-debug-02/validation.json` and `corrected-impact-frame-20261010-release-final-03/validation.json`. Steam preservation at 11:08:17.487053 UTC confirms all 130 resources and original executable unchanged: `local/scene/preservation-20261010-corrected-impact-frame.json`.

Existing Rust/Cargo1.99.0, Python3.12.14 and MSVC14.44.35207/SDK10.0.26100.0 reused. No install or desktop control. Proprietary inputs, outputs and decompilation remain under ignored `local/`.

Next connect active preparation/impact callbacks and authoritative contact production, then actual vehicle/pose scheduling. Existing separate vehicle finalizer commits a weighted predicted pose before retaining raw-pass velocities; do not replace this with an assumed generic velocity advance. F160 does not change completion estimates or establish a drivable lap. Owner resumed the chat; the expired overnight automation remains paused.
