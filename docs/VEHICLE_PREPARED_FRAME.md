# Post-grip interval through controls and forces — F173

10 October 2026. Independent Rust now joins surface slowdown, caller timers and equation reset, temporary inertia, roughness perturbation, steering/gears/brakes and propulsion/forces. Whole original helpers plus the recovered caller schedule agree with Rust for **288 authored initial +384 carried calls /10,338,048 complete record bytes per debug/release profile**. Source contact/suspension/grip is still a separate earlier boundary; this comparison does not execute whole A03670 or a driving lap.

## Ordering and state ownership

Whole original 9DFC50 consumes prepared wheel levers, source-style slowdown fields, additive slowdown and wheel speeds. Rust reuses the verified surface-drag library. The caller then updates speed timer 1F6C with its speed bands and endpoint rules, updates low-RPM timer 1F7C, and resets the equation header while preserving the upper three bytes of its flag word.

Whole 98FCA0 calculates temporary inertia. The incoming body inertia is saved to view 48..50 before substitution. Rust shares the already verified suspension implementation through crate-local visibility; its arithmetic is unchanged. Whole 9B44B0 sums roughness across present wheel materials, gates on speed above 8 and positive roughness, consumes the original RNG, and optionally perturbs three angular-velocity lanes. The stored single-precision constants and conversion boundary are retained. The fourth lane is unchanged.

The caller clears prior traction outputs at view 8C..98, then runs the [F172 control wrapper](VEHICLE_CONTROLS.md) and [F171 propulsion/force schedule](VEHICLE_PROPULSION.md). Force points and saved inertia are restored at the actual later boundary. One Rust call clones and commits the full body/view/solver/diagnostic state only after success. A library regression permits the earlier stages, then fails at physical-force input validation and checks complete rollback.

GF01 retains the bounded 15,384-byte diagnostic record, owned wheel/material/steering pointers and actual view/solver overlap. The original adapter executes unchanged helpers with explicit 53-bit x87 precision and real dependencies; interstitial scheduling is authored from aligned instructions. It restores lazy data pages and checks executable bytes, preserves existing output files and removes an owned new output on ordinary failure. There are no code stubs.

## Validation and failures

All initial and carried records match original/repeat/Rust exactly in both profiles. Each path carries its own full previous result. No original intermediate is supplied to the Rust path. Native stage observations across initial +carried inputs show **589 slowdown motion changes, 188 roughness RNG gates, 91 angular perturbations and 672 temporary-inertia changes**. This demonstrates active stages rather than only inactive comparisons. Final state coverage includes 183 gear changes, 351 brake changes, 831 wheel-steering changes, 87 body-event-count changes, 639 body-velocity changes and 2,549 wheel-spin changes.

**142 native +142 Rust rejection controls** pass with ordinary exits and no new output: inherited ownership/framing/control/force restrictions, zero mass/inertia/radius, singular inertia intervals, equal drag thresholds, roughness conversion overflow and nonfinite consumed fields. An additional late-stage control has positive slowdown with zero point velocity: original normalization produces nonfinite motion and the adapter rejects at its post-helper boundary; Rust rejects without committing. Stationary controls with no slowdown are valid and compare exactly.

The first numerical run already matched all672 calls. Its roughness-overflow rejection fixture left speed below the activation threshold, so the original legitimately succeeded. Correcting that fixture to activate roughness resolved the guard failure without rejecting the valid inactive branch. No original-code fault or numerical discrepancy occurred. An aggregation helper initially treated the integer guard count as a list; reporting was corrected, with no validation rerun or artifact mutation needed. A guessed grid-preview manifest read failed; inventory identified the existing grid-app crate.

Fresh shared F172 regressions retain all672 control/force calls in both profiles. **126 library tests** (20 formats, 1 tools, 105 world), formatting, Python AST parsing and all-target warnings-denied Clippy pass. The earlier inertia implementation was shared without arithmetic changes; no fresh isolated F168 run is claimed.

## Evidence and reproduction

- Rust: `crates/grid-world/src/vehicle_prepared_frame.rs`; GF01 in `grid-vehicle-force`; crate-local alternate-inertia visibility in `suspension_vehicle.rs`.
- Native: `tools/probe_vehicle_prepared.c`, `vehicle_prepared_probe.h`, `build_vehicle_prepared_probe.cmd`, prepared-only shared adapter additions. Validator: `tools/validate_vehicle_prepared.py` and shared propulsion comparison.
- Final private reports: `local/behavior/vehicle-prepared-20261010-debug-final-04/validation.json`, `vehicle-prepared-20261010-release-final-05/validation.json`. Earlier draft/final02/03 evidence is retained; final04/05 include native per-stage counters.
- Fresh controls regressions: `vehicle-controls-prepared-regression-20261010-debug-01/validation.json`, `vehicle-controls-prepared-regression-20261010-release-02/validation.json`.
- Tests: `local/behavior/vehicle-prepared-library-tests-20261010.log`.
- Static: `local/behavior/vehicle-prelude-constants-20261010.json`, `grip-force-segment-aligned-20261010.json`; whole-inertia survey `20261010T1407195943352Z/98fca0.c.txt`. Adjacent instruction bytes are excluded from selected-function interpretation.
- Native SHA-256 `ee3dae24d6ba4d85f27b8dd0788cb17e3ad5d5995bfb31ac4a7d68332f70c9b9`; Rust debug `4b7af7a0bd8222031d473f8485b06b639c11f42463aeaeedebae4b5b0e7dd018`; release `c93a998f9765c0e62669fc78cbbb2a69f1b8f4012e5bb24e744b5d9238bcd721`.

Build the native adapter and grid-vehicle-force, then run `tools/validate_vehicle_prepared.py local/behavior/NEW --profile debug` or `release`. Reused Rust/Cargo1.99.0, Python3.12.14, MSVC14.44.35207, SDK10.0.26100.0, Developer Prompt17.14.41, Ghidra12.1.4/JDK21.0.9/Capstone5.0.6. No installation.

Steam's130 resources and executable remain unchanged at **20:34:55.332781 UTC**, `local/scene/preservation-20261010-vehicle-prepared.json`. No desktop input or settings changes; expired automation remains paused.

## Remaining integration

Contacts, material records, initial force vectors and diagnostic seeds are supplied. Mode1/four wheels, inactive optional gear-search and conservative nonzero diagnostic gear ratios remain the supported branch. The material records here are authored; source-backed mechanics have been verified at the earlier grip boundary but are not yet joined to this adapter. Next combine source track queries and whole suspension/grip with this interval, carrying computed wheel materials and forces without overwriting them. Suspension already advances its selected pose; subsequent physical scheduling must avoid double advancement. Earlier control/force-vector preparation, linked physical-world timing, source vehicle initialization and Bevy driving remain open. The dated completion estimates remain unchanged.
