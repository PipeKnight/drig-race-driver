# Registered body impacts in the driving runtime

The selected body-impact handler is now connected to the Rust driving world. **Play GRID Body Contacts.cmd** enables both physical contacts and the new event path when private BP01 data is present. The window title includes `EVENTS`; the standard launcher retains its existing mode. This adds verified damage/suspension event handling, not visual body deformation or arbitrary crash compatibility.

## Integration and ownership

`vehicle_world` retains the occupied transient count independently of the reusable pool capacity, including the wheel prefix and fresh geometry suffix. It dispatches the verified mode0 handler over the solved records in their original order, using impulse writeback at contact3C, world point50 and normal60. Registered car shape1 selects the side; static peer shapes have no callbacks. Both-side/dynamic peer registrations are rejected. The runtime owns empty notification slots explicitly; it does not interpret missing original tail bytes as registrations.

The staged owner follows the completed physical state and carries the damage RNG through every event. After successful dispatch, its frame prefix is synchronized. Any handler failure rejects the entire tick, including manager/solver/contact changes, owner/frame buffers, RNG, query cache and runtime metadata. Reset restores the initial world/car while retaining the configured event mode. The existing physical helper returns consumed pool storage privately before this join; callbacks in this admitted scope cannot observe that count. This composition is not a whole original registered-world frame proof, a full source resource/notification factory, or live asynchronous scheduling equivalence.

Original wrapperA727A0 also dispatches persistent chains after transient contacts; those chains remain absent in this world. Static peers, pointer-free projected state, bounded event points/velocities, supported damage effects and guarded later notifications remain explicit limits. Existing heavy-impact failure recovery still usesR to reset. There is no new claim of calibrated steering, tire behavior, visual deformation, opponent-car collisions or lap timing.

## Private scalar preparation

`tools/extract_body_impact_parameters.py local/body-impact-parameters.bin` reads six file-backed initial scalar words from the identified untouched Steam executable, with SHA-256 `dd7b0933abf0d421bec3fa6f123319d3d3cc7221e5ef82b7a1201c3d1979e48f`. It executes no original code and needs no decompilation image. BP01 is28 bytes (tag plus six scalars), SHA-256 `493380e174053568b81e90917fe8f55c0b2f43ee6c9a7715614f20b8c271946d`; its private JSON records source provenance. These defaults match the separate native-handler proof parameters. They are not arbitrary live-global overrides. No scalar values, binaries or original assets are committed.

The public bounded reader is `crates/grid-tools/src/body_impact_parameters.rs`. The app accepts `--body-events PRIVATE_BP01` with `--drive`; it may be combined with `--body-contacts PRIVATE_CD01`. The scheduled diagnostic accepts the optional BP01 path after its output directory and copies the exact parameter request into that private output. The updater now includes the private parameter file in its source checks and build manifest.

## Validation

Both debug and release pass the purchased-source600-tick/176-body-contact run, with a bitwise per-tick projected-state comparison after reset/replay. The projection includes complete car/physical/frame body/view/solver buffers, world bodies and stable contact pool, world heads, solver rows/workspace, owned RNG, cache plane, tick and contact-count metadata. It does not assert every manager/configuration field or an original-game trajectory. Reset restores the checked initial state.

An explicit valid-then-invalid geometric sequence proves late rollback. The first event has a finite solved impulse and demonstrably clears1568 on the same completed no-event control. The later point exceeds the supported event bound and fails. All checked pre-tick state survives unchanged. The bound was not relaxed and no failed original result was discarded as a success.

Six scheduled Donington approaches were then run in both profiles twice: **24 cases, 14,400 ticks**, with byte-identical complete per-tick TSV and selected initial/final buffers across profiles/repeats. First body contacts span approximately34..147km/h and30..467 contacts per run. **Every schedule changes the global damage RNG and selected damage-effect fields**. This is a successful connection of the body-event route; it does not infer visual damage or exact game calibration. A current-hash audit reconstructs requests and checks120 complete primary files, fourteen malformed-plan/output-preservation guards and all130 Steam resources plus the executable. Separate malformed BP01 length/tag/nonfinite guards pass in both profiles (six additional cases).

Callback-disabled controls in both profiles produce complete telemetry, initial/final buffers and plan files identical to saved F248 outputs (two600-tick runs). Thus the existing mode retains its checked behavior. All188 regular library tests, six purchased-source release tests, three Bevy tests (including event-enabled body geometry keyboard/pause/reset/recovery), formatting, all-target warnings-denied Clippy, PowerShell AST and three Python AST checks pass.

Private evidence:

- `local/behavior/body-event-runtime-{release,debug}-20261013-final-01.txt`: reset/replay and late-failure proof.
- `body-event-plans-20261013-final-01/validation.json`, SHA-256 `d2bc1ca34ee02121c61df5d821e836dbefe05bd720ef8f3cc85c8f110b0bf50a`; all source schedules/profiles/repeats.
- `body-event-plans-audit-20261013-final-01.json`: current source/input/output hashes and Steam preservation.
- `body-event-parameter-guards-20261013-final-01/validation.json` and `body-event-legacy-controls-20261013-final-01/validation.json`.
- `body-event-regular-tests-20261013-final-01.txt`, `body-event-runtime-clippy-20261013-final-01.txt`.
- Initial checked dirty app build `local/playtest/build-20261013T0813573699637Z/`, executable SHA-256 `cbb3a0617b5a253065ad68144bbdbd09531323378dd66e229370c0df76887f19`. The clean committed-source build and render are recorded in [prototype updates](PROTOTYPE_BUILD_UPDATES.md).

Tools remain Python3.12.14, Rust/Cargo1.99.0, Bevy0.19.1 and the previously recorded native research toolchain. No tool installation, desktop input, Steam modification or save/settings change was needed. Initial extraction incorrectly used the compatibility-copy reference identity and rejected before output; the purchased Steam identity corrected that mistake. Rust test literals lacked leading zeroes and were fixed before execution. The first late-fault fixture assumed wheel records at rest; the no-event control showed its bad record was first. A warmup also mistakenly used the four-slot nongeometry entry after attaching a20-slot pool; it was corrected to the geometry entry. The final fixture explicitly supplies two ordered geometric records and proves an earlier callback effect. All failed draft logs remain private, with no physics guard or numerical-tolerance relaxation.

Next verify sequential whole native registered-world composition on solved source records, then tackle reached notification/damage recovery and visual deformation. Owner help is not currently required; the new playable mode can be tried through the body-contact launcher after the recorded refresh.

The refreshed clean build is3443c31c8365, with private damage-state telemetry and a successful inspected600-tick event-enabled render. See the [build record](PROTOTYPE_BUILD_UPDATES.md) for its full revision/hash, manifest, image/log and final Steam-preservation timestamp. The source-plan audit SHA-256 is`1d4d5a82c4f9b23c74c68e79061871b42dddc7bdd168a3b0a48c390e62d5e137`.
