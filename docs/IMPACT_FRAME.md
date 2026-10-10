# Whole outer impact frame, without registered callbacks — F159

Whole original **B7B850**, repeat and one Rust `impact_frame::calculate` call match **695 scenarios /18,560 consumed normal/friction rows /3,895,485 output words per debug and release**. The original gathers supplied linked contacts/bodies, prepares bodies, scales inverse properties, invokes complete B7ACC0, visits naturally empty callback managers and performs final damping/clamping/pending-shift publication. No original instruction is skipped or helper replaced.

This is deliberately scoped: impact pools only; registration pointers +418/+41C/+420 zero; task dispatch, filtered gathering, optional correction and body-link/shape lists disabled by supplied state. Bodies use the ordinary awake +400=0 force-accumulation branch. Active callbacks, joints, linked-body shifts, shapes and authoritative contact generation remain open. **This outer function does not establish continuous pose prediction/advance or a drivable loop.** Earlier “final pose integration” backlog wording was provisional; recovered final helpers perform damping, limits and pending shifts. Existing `pose.rs` prediction is a separate contract to connect with actual scheduling.

512 authored controls plus183 handoffs reuse the **returned bodies, contacts and rows** of the prior F158 comparison, including5,840 contacts. Velocity, impulse and accumulated operands are retained; new body settings, local matrices and gravity are authored. Contact type/pool/link bytes are explicitly framed, and previous group-end bytes reset to zero for fresh construction. F158 returns are not supplied as an unchanged whole physical state: auxiliary body settings are overwritten deliberately. Both profiles freshly replay whole F158 original and rebuilt Rust, reproducing695cases/3,841,665words before using the handoffs. This also verifies the shared input-validation refactor.

## Whole-function behavior

B7B850 is thiscall with two stack arguments: linked group state and scalar delta. The comparison uses its unfiltered gathering branch with capacity16bodies/64contacts. Contact +0C determines one/two-body classification with the second-body pointer; +0F=1 selects the impact pools. The native harness owns the group, body/contact linked lists, gathered array, all pool arrays, row storage, cache, shapes used as contact operands and inverse-property/workspace arrays. Body registrations are actually zero, so original B77770/B78530/B78710 take their own no-owner branches. F151's active callback remains separately verified, not executed here.

Zero main iterations return status0 before preparation. Otherwise each body passes through:

1. B0FE60 sleep-timer update. Maximum-duration sentinel or linear/angular threshold crossing resets the timer; enabled force/torque exceedance uses strict comparison. Otherwise delta accumulates, clamps at the configured duration and reports asleep. Sleep is tested **before** new gravity/drag forces are added.
2. B12F80 adds `mass * gravity_scale * gravity` when enabled. Local XYZ quadratic drag uses sign-dependent coefficients, limits each force so its delta/inverse-mass contribution cannot overshoot local speed, transforms back and adds through original B0FB10. The supported +400=0 branch sets force-active even for a zero addition; wake-manager mutation is outside this adapter.
3. B0FC90 clears the impulse accumulator/+40C. Cached inverse mass and all16 inverse-inertia words copy from body +3F4/+110. The minimum positive inverse mass is retained.

If all bodies report asleep, status1 returns **after** preparation, before mass scaling/main solving/cleanup. Pending gravity/drag force remains. Otherwise cached inverse properties scale by the reciprocal of that minimum. B7ACC0 receives delta,1,1,minimum,reciprocal, preserving original multiplication/division order. Manager +118 becomes1 before this call. Capacity failure returns status1 with that flag and metadata publications retained, skipping final cleanup.

On main success, empty registered callbacks return naturally. The supported final branch clears force/torque vectors and active flags through B0FCD0/B0FD10. B13260 applies world linear deceleration and per-axis local angular damping; B134C0 limits world linear magnitude and local angular components. Their transforms retain all relevant four-lane grouping. The original scalar at CAFE50 is **negative zero**, which must survive subtraction in zero-limit and damping branches.

B137E0 consumes an enabled pending shift at +380/+390, updates the owned position, published matrix and derived matrices through original B0F730/B100F0, then clears the shift. Its two linked-body propagation lists are empty here. `physical_pose::calculate` supplies independent publication with no rotation correction, no group-head orthonormalization and no shapes. The pure interface rejects conflicting basis/position inputs and nonfinite relevant results; its quarter-second bound is a diagnostic precondition, not an observed original frame clamp.

## Verification and evidence

| Coverage | Count |
|---|---:|
| Finished / capacity failures |472 /41|
| All-asleep bypasses / zero-iteration bypasses |81 /101|
| Pending-shift publications |1,009|
| Changed body velocity / forces / sleep timer |2,097 /2,620 /408|
| Consumed normal / friction rows |8,708 /9,852|
| Empty contact scenarios |57|

6 semantic checks verify complete zero-iteration retention, post-success force clearing, shift consumption, unstarted contact/row retention, unused rows and body opaque fields.18 malformed guards reject ordinarily in native and Rust, with rejected Rust output absent. Full0x424-byte bodies,128-byte contacts, every allocated192-byte row, return status, manager-start flag, cache, inverse properties and workspace compare; owned pointers canonicalize. The original code remains RX and all executable sections match; five temporarily writable data pages restore afterward. No imports, stubs or original executable changes are used.

109 library tests (20formats+1tools+88world), fmt, all-target warnings-denied Clippy and validator AST pass. Four boundary tests establish empty-awake gravity/cleanup, zero-iteration preservation, all-asleep prepared-force retention and inconsistent position rejection. Earlier F158 remains exact after extracting shared validation. The existing `grid-impact-frame` tool is a distinct supplied-pose contact-frame diagnostic and remains unchanged; this whole-physics diagnostic is **`grid-physical-frame`**.

Failures/uncertainty retained:

- Initial native run completed the zero-iteration case then faulted on its missing manager+94 gathered-contact array. Owned storage fixed the harness; original code unchanged. `impact-frame-20261010-debug-01` and log retain the failure.
- Intermediate02 completed all original/repeat scenarios and guards but had165 Rust negative-zero differences and no timer-change coverage. Reading file-backed CAFE50=0x80000000 explained the signs; preserving that subtraction and adding finite-duration timer controls produced exact03.
- Two iterator lints corrected before final rebuild. A source-generation assertion stopped because the old tool name occurs twice; no existing diagnostic was overwritten. A guessed Ghidra entry B12DA0 was not a function and the wrapper rejected the survey; only its successful B77850/B0FB10 outputs are used. A few guessed documentation paths did not exist; source index corrected.
- Initial aligned lengths omitted some returns; complete evidence supersedes them. No failed or partial survey is counted as execution verification.

| SHA256 | Value |
|---|---|
| Analysis PE |aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514|
| Final native probe |5c42ca441037f5bcad3e99c18320c7c9f2aeec19a4cd9d9bb23b2b2a81d96895|
| Shared request |b53b3b376e88643e592f3fe4194de5af307c5f80266155b6c376cc67654cfe1e|
| Debug diagnostic |62d406826c9fe6417616c3ed84831c9ad607ab0d20842b501cc16555195727a9|
| Release diagnostic |9f222dc932daecd8b98b16bf0c3a9b0a8ea72f1373a4ed2b7157446da3032304|

- Final reports: `local/behavior/impact-frame-20261010-{debug,release}-final-04/validation.json`.
- Outer survey: `local/behavior/20261010T0312197636405Z/b7b850.c.txt`.
- Body helpers: `local/behavior/20261010T0535523011863Z/`.
- Successful partial dependency outputs: `20261010T0536324061639Z/b77850.c.txt`, `b0fb10.c.txt`; failed survey log `outer-body-dependencies-20261010.log`.
- Complete helper alignment: `local/behavior/outer-body-step-complete-20261010.json`; lane update surveys `053817` and `054130`.
- Preservation: `local/scene/preservation-20261010-impact-frame.json`; original130Steam resources/executable unchanged.

Rust/Cargo1.99.0, Python3.12.14, MSVC14.44.35207/SDK10.0.26100.0, Ghidra12.1.4/JDK21.0.9, Capstone5.0.6 reused. No installation or desktop input/owner help. Proprietary evidence stays in ignored `local/`.

Next integrate the existing optional correction into this whole manager; then active preparation/bulk callbacks, shape/link publication and actual pose-advance scheduling. Preserve natural bypasses and prove larger joined paths against original whole functions. Asset/vehicle initialization, contact generation and Bevy driving remain open; completion estimates unchanged. Temporary unattended work ends07:30UTC.

## Optional correction integration draft — F160

**Status: compiled native/debug draft only; numerical verification has not run.** This section records unfinished work and does not extend the verified F159 contract above. F159 commits `66c1444` and `9386493` were pushed to owner origin/main; its last independently confirmed head is `93864931e18dd1a51452030a0afb3f1a646da3e4`.

The uncommitted `impact_frame.rs` extension accepts optional correction settings and invokes the previously verified `impact_correction::calculate` after successful main solving, before force cleanup/damping/limits/pending-shift publication. It adapts normal rows, response workspace and pose publications while retaining main contact impulses and tangent rows. `main_contact_response::typed` becomes crate-visible for this adapter. Existing `grid-physical-frame` explicitly selects no correction. New `grid-corrected-physical-frame` supplies correction settings; the separate older `grid-impact-frame` remains untouched.

New authored research tools are `tools/probe_corrected_impact_frame.c`, `tools/build_corrected_impact_frame_probe.cmd` and `tools/validate_corrected_impact_frame.py`. The whole-original target remains B7B850 with naturally absent callbacks, empty other/joint pools and no body links/shapes. PFR2 adds five words after each original 144-byte PFR1 case header: correction iterations, softness, scale, threshold and relaxation. These initialize original manager +F4/+E0/+E4/+E8/+F0. Static file inspection established D85A50 as positive zero (bits00000000), used by the outer optional-pass relaxation guard. This is a static observation, not execution validation.

The native probe compiled with MSVC14.44.35207/SDK10.0.26100.0 (`/W4 /WX /O2 /MT /std:c17 /fp:strict /arch:SSE2`) and has SHA256 `d5061ab307ece7dae07d97e3a2915962facc6f5867f692a4377a7ce9e01acfbe`. The debug Rust diagnostic compiled with Rust/Cargo1.99.0. An initial unused-config warning revealed the new diagnostic still selected `None`; corrected to supply its parsed settings and rebuilt without that warning. Python3.12.14 is the intended validator runtime. No new tools were installed.

The first attempted comparison, targeting `local/behavior/corrected-impact-frame-20261010-debug-01`, **did not execute**. Automatic approval review reported that its service usage limit had been reached and could not complete review. It explicitly distinguished review failure from an unsafe-action determination. No bypass, alternate execution route or successful comparison is claimed. Release compilation, final tests/fmt/Clippy and native malformed-input guards remain pending. F159's existing passing tests do not validate the modified working tree.

Resume in a new authorized work session:

1. Read PROJECT_LOG.md and inspect the dirty tree; preserve the draft and this handoff.
2. Once normal approval review is available, run the new validator in debug. It first replays F159 original/new Rust as a regression, then compares complete corrected original/repeat/Rust outputs for the same 695 scenarios with varied correction settings.
3. Inspect complete normal-span reset, tangent-pointer canonicalization after span shrinking, inactive/active correction, pose publication followed by pending shifts, main impulse retention and cleanup. These are planned checks, not passed results.
4. Repair any mismatch, build release and repeat numerical/semantic/malformed checks. Run relevant library tests, formatting and all-target warnings-denied Clippy against the final state.
5. Record actual counts/hashes/failures and preservation evidence, then split verified research and Rust changes into focused commits and push owner origin. Continue toward active callbacks/contact production and actual vehicle/pose scheduling; no drivable-loop claim yet.

On the next resumed wake, the clock was 10 October 2026 10:58:50 UTC, already beyond the unattended deadline. No further research execution or desktop control was started. The temporary automation was paused through the app API, confirmed PAUSED; its deadline was not extended. The repository retains the F160 draft for later work. The project itself remains open.
