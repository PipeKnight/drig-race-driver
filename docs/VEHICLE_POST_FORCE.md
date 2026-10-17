# Vehicle post-force state and assistance — F176

11 October 2026, Europe/Kiev. Independent Rust now extends the [prepared source-contact update](VEHICLE_CONTACT_UPDATE.md) through speed refresh, RPM assistance/history, disabled-wheel clearing, suspension camber, rollover assistance, braking timer and a wheel response scalar. Original-versus-Rust comparisons cover both the complete selected interval and its integration with source contacts and forces. The following chassis-motion routine `9BE1E0`, earlier driver setup and whole-world execution remain open.

## Recovered boundary and order

Aligned `A041D5..A043D6` follows the force solve and inertia restoration:

1. Recompute body `1648` speed from all four `15C0` velocity lanes. Preserve the observed paired-product reduction, hardware `RSQRTPS` seed, refinement and explicit zero-result mask. This is separate from ordinary square-root rounding.
2. For a recent manual shift (`1974 < .25`, automatic byte `1970 == 0`), enabled clutch assistance (`1971`) and positive engine maximum, raise low RPM to the stored idle multiplier. Then call whole `990350` when assistance is enabled. Its throttle branch multiplies the RPM difference by two before the throttle factor, as the aligned instructions require.
3. Store RPM to `1F00` before subsequent disabled-state clearing. When scalar `226C != 0`, whole `990220` clears six accumulator words per selected wheel and clears RPM strictly below idle. Equality retains RPM.
4. Whole `996800` uses axle bits `21F4` to compute suspension camber: a rounded float displacement/span quotient, promoted to double for `BF6F00`, then rounded back to float and stored with opposite signs. Aligned evidence identifies an SSE double arctangent implementation; Ghidra's inferred integer return and unchanged argument in pseudocode are misleading. Rust uses double arctangent and matches the tested finite controls, including signed zero and very small/large slopes; universal equivalence across math libraries/platforms is not established.
5. Whole `9B9500`, enabled by byte `1F2A`, corrects angular velocity when the side or forward axis is sufficiently tilted and angular motion opposes recovery. The second axis comes from whole `98DCF0`, which copies the body's side basis. At the saturated factor, XYZ angular components are cleared before applying the correction; an applied correction publishes W as positive zero. Both branches run in order against the updated angular state.
6. Whole `9BF230` updates braking timer `E74`, then four wheel `+13C` responses from inverse mass, wheel load `+138`, refreshed speed and brake input. Preserve the difference-times-reciprocal operation before the blend; algebraic regrouping changes rounding. Constants around 293.15 and 923.15 suggest a temperature response, but the physical interpretation is not yet independently established.
7. Scalar `2274 != 0` clears final RPM after its history has already been stored. Stop immediately before whole `9BE1E0`.

Native adapters invoke unchanged original helpers. Interstitial scheduling, speed arithmetic and owned data are independently authored from aligned caller evidence; this is not execution of whole `A03670`.

## Implementation and ownership

`grid-world::vehicle_post_force::apply` clones the fixed body words and commits only after all checks. The selected contract requires exactly four wheels, finite consumed scalars/vectors, bounded positive delta, a finite camber quotient for each enabled axle, finite speed accumulation and finite resulting state. An inactive camber axle does not require a nonzero span. Stored addresses remain opaque and are never followed.

`vehicle_contact_update::apply_with_post_force` extends F175 on a full `Frame` clone. A late tail failure preserves body, view, solver, diagnostics and material bindings, including all already-computed suspension and force changes. `grid-vehicle-force` adds private source-backed `HF01` mode; earlier modes retain their contracts. `grid-vehicle-post-force` provides bounded body-only `TA01` diagnostics with private, create-new output.

`probe_vehicle_post_force.c` uses the existing hash-checked PE mapper, no entry point/import/TLS execution, no live injection, no helper replacements and executable-section identity checks. The five whole helpers take only the owned body and delta; no proprietary pointer is followed. Native `thiscall` dependencies are invoked using the established C fastcall bridge with dummy EDX and a stack float. Explicit embedded `asInvoker`, `uiAccess=false` manifest avoids requesting elevation. x87 is set to 53-bit, round-to-nearest; SSE scalar ordering is retained.

The joined reference composes three isolated unchanged-original adapters: F175 vector preparation/source suspension/grip, F173 controls/forces, then this tail. Rust performs one call. Original, repeated-original and Rust branches carry their own outputs; the authored previous-pose/velocity transfer is explicit. No extra pose advancement or external gravity is added. Material table installation, earlier control/history values and selected assistance settings remain supplied. Tilted source cases exercise active angular recovery inside the joined path.

## Verification

| Comparison, per profile | Calls | Complete record bytes | Guards |
|---|---:|---:|---:|
| Whole post-force interval: 525 initial + 1,024 carried | 1,549 | 15,074,868 | 102 native/Rust outcomes |
| Source vectors/contact/suspension/grip/forces plus tail: 120 initial + 192 carried | 312 | 4,799,808 | 71 outcomes |
| Fresh prior F175 regression | 312 | 4,799,808 | 69 outcomes |

Every compared word matches the original and repeated original in debug and release. Standalone coverage: 589 speed changes, 129 RPM changes, 566 RPM-history changes, 535 angular changes, 589 braking-timer changes, 1,176 camber changes and 6,154 wheel-response changes. Joined tilted controls: 312 vector/pose/velocity changes, 186 scale changes, 101 source contacts/nonzero grips, 1,227 spin changes, 289 speed refreshes, 288 RPM-history changes, **17 post-force angular changes**, 624 camber changes and 1,111 wheel-response changes.

Guards include invalid scalar/vector/count/span/speed, header/count/truncation/trailing framing, late arithmetic overflow and pre-existing output preservation. Joined guards additionally reject a tail-only nonfinite input after successful original source contact and force stages. Unit tests prove disabled RPM/history ordering, four-lane speed consumption, late arithmetic rollback and full-frame rollback after completed contact/control/force execution. All **131 library tests** pass (20 formats, one tools, 110 world); formatting, all-target warnings-denied Clippy and Python AST checks pass.

## Failures and limits

- Initial Rust mutable/immutable argument borrowing was fixed with an explicit saved RPM. C rejects `__thiscall` function-pointer syntax in this C configuration; the established fastcall bridge with dummy EDX preserves the observed ABI. An ambiguous float literal in the new integration unit fixture was typed before executing that test.
- The first isolated authored native batch terminated with failfast **`0xC0000409` / 3221226505** and left a private partial output. Its random body padding had been allowed to stand in for wheel count `1620`. Case 7, with disabled clearing enabled, supplied positive count **771,478,463** to an original loop that writes wheel accumulators without a bound. Static instructions demonstrate that this input exceeds the owned fixture; the 65,536-byte partial output is consistent with failure after the earlier cases. No fault instruction was captured, so the exact failure site remains an inference. Both adapters now require exactly four before original execution, every authored fixture initializes it, and the paired count guard plus all corrected comparisons pass. This was a probe-input failure, not evidence of game compatibility.
- The first corrected debug draft (512 + 1,024) already matched. Subsequent controls add signed-zero/small/large camber slopes and inactive zero-span acceptance. Early joined debug/release controls matched but had zero angular recoveries; tilted controls now verify the active branch in the composed source path. Early reports remain private and are not the final tilted evidence.
- Fixed file-backed globals `D16770`, `D167A4` and `D15458..68` are the identified reference values. Arbitrary runtime changes to these globals, other vehicle implementations and whole-world scheduling are not covered. No full driver, source initialization, physical contact production, Bevy driving or lap-completion claim follows from these tests. Dated project estimates are unchanged.

## Private evidence and reproducibility

All original bytes, disassembly, decompilation and generated records stay under ignored `local/`:

- Main helper survey: `local/behavior/20261010T2054199422959Z/`; dependency survey: `20261010T2057582065641Z/`. Aligned `vehicle-tail-aligned-20261010.json`, `vehicle-tail-helpers-aligned-20261010.json`, `vehicle-camber-math-aligned-20261010.json` and `vehicle-post-force-endpoints-20261010.json`; constants `vehicle-tail-constants-20261010.json`. The endpoints file starts its second range two bytes before an instruction boundary; ignore the first two decoded artifacts and use aligned helper evidence for the preceding instructions.
- Whole interval finals: `vehicle-post-force-20261011-debug-final-04/validation.json`, `vehicle-post-force-20261011-release-final-03/validation.json`.
- Joined tilted finals: `vehicle-contact-assists-20261011-debug-final-03/validation.json`, `vehicle-contact-assists-20261011-release-final-04/validation.json`.
- Fresh F175 regression: `vehicle-contact-update-assists-regression-20261011-debug-01/`, `vehicle-contact-update-assists-regression-20261011-release-02/`.
- Library test log: `vehicle-post-force-library-tests-20261011.log`. Steam preservation: `local/scene/preservation-20261011-vehicle-post-force.json`, **10 October 21:11:55.374330 UTC**, all 130 resources and executable unchanged.

Reference PE SHA-256 `aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514`; native tail adapter `a669d8a2c425c27c1b5d0e5420ceb50c47d34bbbd5c4152d1d1251fc440ae971`. Tail CLI debug `4eaf5cac793bf6af9b612d9260e1c3b8902bfb62ee5e1e8a1f6de9f60a64f489`, release `b23d6708b32e473c23d3f213deed2d48d5764aeaa7ceb90040b33a3d7adec578`. Joined CLI hashes are recorded in each execution report.

Installed tools reused: Rust/Cargo 1.99.0, Python 3.12.14, MSVC 14.44.35207, Developer Prompt 17.14.41, Windows SDK 10.0.26100.0, Ghidra 12.1.4/JDK 21.0.9, Capstone 5.0.6. Native `/W4 /WX /O2 /MT /std:c17 /fp:strict /arch:SSE2`; Bevy remains pinned at 0.19.1. No installation, desktop input or game/settings edits; expired overnight automation remains paused.

Reproduce using existing build scripts, `cargo build -p grid-probes --bin grid-vehicle-force --bin grid-vehicle-post-force --locked` (and `--release`), then the corresponding `validate_vehicle_post_force.py`, `validate_vehicle_contact_assists.py` and prior `validate_vehicle_contact_update.py` with new private output directories and explicit `--profile debug|release`.

Next recover whole `9BE1E0 -> 9BDF30` chassis-motion publication and its matrix dependency, then the missing earlier `A03670` setup/source vehicle initialization and physical-world connection needed for Bevy driving.
