# Selected driver preparation and source-track integration — F178

11 October 2026, Europe/Kiev. Independent Rust now joins the earlier selected `A03670` preparation interval to source track contacts, suspension, controls, wheel forces, assistance and local chassis publication. Gravity and aerodynamic impulses reach suspension in the recovered order. This is a composed, selected driver path; complete vehicle initialization, physical-world scheduling and Bevy driving remain open.

## Recovered schedule

The selected prefix ends immediately before `9B2790` at `A03D55`. Aligned instructions, rather than regrouped decompiler expressions, establish this order:

1. Advance elapsed time, calculate the optional driven/non-driven wheel-speed brake adjustment, update the five-entry delta ring and copy linear/angular velocity into previous-velocity storage.
2. Whole `9B5E80` damps airborne driven-wheel spin or derives it from forward speed and radius, caches spin/contact state and conditionally clears throttle. Inverted vehicles additionally damp wheels without materials.
3. Whole `9904C0` computes four friction factors from lateral/longitudinal wheel force, mass, direction and tuning endpoints. The stored mass coefficient is `2.4525001049`, not an assumed decimal approximation. It publishes wheel and view factors.
4. Advance timers, preserve the current velocity, refresh speed using the original SIMD square-root refinement, update the ten-entry speed rings and acceleration/brake flag. Whole `9B6200` conditionally damps low-speed angular motion. Clear the selected assistance output and advance its timer.
5. Consume fuel from throttle, engine speed, maximum RPM and fuel tuning; clamp the remaining quantity at zero. Under explicit inactive flags, whole `9969C0` and `9B9DA0` make no changes. Whole `9B9450` applies the selected low-speed braking correction.
6. Whole `9E1290` applies gravity, lateral/up/forward aerodynamic drag, material-dependent rolling resistance and downforce impulses. It includes airborne downforce branches, the previous material's high-downforce flag, axle-weighted force placement and retained force publication. Gravity uses the observed 53-bit x87 intermediate before storing `f32`; vector reductions and drag products preserve instruction order. Force placement uses a lever vector, not a world-space point.
7. Whole `9BF690`, with receiver `body + 2EC`, advances the periodic phase/rate/timer response. Whole `9BDE30` saves RPM history and applies engine reaction around the selected physical body axis. Its `ret 4` proves the stack-float calling boundary despite misleading inferred signatures.

The complete joined API then runs [vector/contact/suspension/forces](VEHICLE_CONTACT_UPDATE.md), [post-force assistance](VEHICLE_POST_FORCE.md) and [local chassis publication](VEHICLE_CHASSIS.md). The preparation itself changes velocity and history, not physical pose. Existing suspension performs its own pose work; a future world bridge must not advance it twice.

## Contract and ownership

`grid-world::vehicle_driver_prepare::apply` accepts a complete owned force `Frame`, delta, gravity and four previous-material downforce flags. It supports an active four-wheel mode-one vehicle, engaged gear 1–9, assisted automatic transmission, inactive nitro/respawn and no clutch-mode transition. Driven wheels must be bounded and unique. Consumed operands must be finite; masses, inertias, radii and maximum RPM must be positive; friction endpoints must differ, aerodynamic alignment must remain below one, and ring/phase indices are bounded. These restrictions are explicit unsupported branches, not evidence of equivalence for every vehicle state.

All mutations occur on a clone and commit after finite output checks, including periodic state, cached spin, fuel, view factors and velocities. Arithmetic overflow returns a checked error. `vehicle_contact_update::apply_driver` resolves previous source material flags, prepares the frame and executes the complete F177 pipeline on another complete clone. A later error preserves body, view, solver, driven-wheel ownership, material multipliers and diagnostics. Query/material callback side effects are external to the owned frame and are not rolled back.

`grid-vehicle-force` adds private `DF01` prefix and source-backed `EF01` joined diagnostics. They retain the bounded 15,384-byte full record and maximum 64 records. Source mode binds original Donington collision geometry and full material mechanics, including the high-downforce boolean at material offset `75`. Diagnostic gravity is exactly `f32::from_bits(0x411CF5C3)`; the library takes it explicitly.

The isolated native reference calls whole unchanged helpers around independently authored caller bookkeeping. It does not execute whole `A03670`, import initialization or game entry points, replace helpers, inject into the live game or follow foreign captured addresses. Owned view/solver/wheel/material pointers are explicitly relocated and checked. Selected private data pages, including gravity at `E8EFD4`, are writable with saved contents/protections restored at cleanup. Executable sections remain byte-identical. Fastcall bridges supply ECX, dummy EDX and the stack float; x87 uses 53-bit nearest rounding. The manifest is embedded `asInvoker`, `uiAccess=false`.

The joined original comparison composes five adapters: prefix, source vectors/suspension/grip, prepared controls/forces, post-force assistance and chassis. Original, repeated-original and Rust each carry their own outputs; the existing explicit history transfer remains authored. Initialization, source tuning bindings and whole-world ordering are still supplied or pending.

## Verification

| Per debug/release profile | Calls | Complete bytes | Guard outcomes |
|---|---:|---:|---:|
| Prefix: 288 initial + 384 carried | 672 | 10,338,048 | 58 |
| Full selected source path: 140 initial + 192 carried | 332 | 5,107,488 | 79 |
| Fresh preceding F177 source regression | 312 | 4,799,808 | 75 |

Every compared word and repeated-original result matches. Prefix coverage includes 672 velocity, previous-velocity and periodic changes, 148 fuel changes, 379 downforce-publication changes, 309 RPM-history changes, 667 speed-ring changes, 320 friction changes and 235 wheel-spin changes. Cases exercise tilted/inverted bases, forward/reverse/low speed, airborne materials, high-downforce flags, fuel depletion, angular damping and phase/rate transitions.

The full source path has 332 velocity, pose, chassis and acceleration changes, 310 chassis-phase changes, 136 contacts/nonzero grips, 1,327 spin changes, 137 fuel changes, 167 early aerodynamic changes and 17 post-force angular recoveries. Twenty additional controls verify that a negative previous speed and a NaN in an overwritten view friction slot are correctly replaced before later readers. They are accepted, not mislabeled as malformed inputs.

Guards cover unsupported modes/flags, finite inputs, indices, positive physical parameters, late arithmetic overflow, framing, ownership and output preservation. Two new unit tests prove rollback after gravity/history writes on periodic reciprocal overflow and after the complete contact/control/force/assistance path reaches a late chassis error. All 136 library tests pass: 115 world, 20 formats and one tools. Formatting, warnings-denied all-target Clippy and Python syntax checks pass.

## Failed experiments and corrections

- The first adapter draft dispatched preparation before its intended SEH block and then incorrectly continued into propulsion. It rejected after four cases with code 31. Dispatch was moved to the correct exclusive guarded branch; this was an adapter error, not a game compatibility finding. Native unused-variable `/WX` and Rust borrow-binding errors were fixed before successful comparison.
- The joined draft rejected batch `initial-40`, local case 20 (global case 60), wheel `1CA8`: a signaling NaN inherited from an earlier fixture's unused padding was newly consumed by aerodynamic preparation. The fixture now explicitly initializes wheel `+78`. The guard was correct; no numerical implementation change was needed.
- Initial diagnosis mistakenly inspected local case 20 in `initial-0`, which held finite bytes. Added instrumentation and repeated runs appeared to change the failure, leading to unsupported calling-convention/file-corruption suspicions. A per-batch input audit located the correct batch. Those suspicions are withdrawn; retained private reports show the diagnosis. No native fault or verified memory-corruption condition occurred. A temporary assembly-listing command used invalid `/Fa:` syntax; correcting it produced private compiler evidence. Temporary floating-point diagnostics were removed.
- The first otherwise exact full comparison failed inherited guard expectations for negative previous speed and overwritten view NaN. These fields are legitimately replaced by the new prefix. They now have explicit accepted controls; separate NaN previous-speed/friction-input cases retain rejection coverage.
- Clippy proposed commuting observed additions and replacing original sign multiplication. Scoped allowances preserve arithmetic order. Excessive-precision diagnostic gravity literals were replaced by their exact stored bits, with final comparisons rerun.

No source installation/settings edits, desktop input or installation occurred. Existing tools were reused. The original executable and all 130 traced resources match the saved baseline at 11 October 00:33:54.212768 UTC. The expired overnight automation remains paused. Dated project estimates are unchanged; no drivable lap is claimed.

## Evidence and reproduction

All original bytes, generated records and compiler listings remain ignored under `local/`:

- Prefix finals: `local/behavior/vehicle-driver-prepare-20261011-debug-final-02/validation.json` and corresponding `release-final-02`.
- Full source finals: `local/behavior/vehicle-driver-20261011-debug-final-02/validation.json` and corresponding `release-final-02`.
- Prior regressions: `vehicle-contact-chassis-driver-regression-20261011-debug-01/` and corresponding `release-01/`.
- Failure evidence: prefix `debug-draft-01`; joined `debug-draft-01` through `06`, diagnostic per-batch audit `debug-draft-06`, corrected numerical/old-guard run `debug-draft-07`. Diagnostic `vehicle-driver-fp-*` files and `vehicle-driver-inlined-20261011.asm` are private, superseded diagnostic artifacts.
- Existing prefix survey `20261008T0136288512690Z/a03670.c.txt`; helpers `20261008T0034485705914Z`, `20261010T1153107866682Z` and six-entry `20261011T0011468105388Z`. Aligned evidence `vehicle-driver-preparation-aligned-20261011.json`, `vehicle-driver-helpers-aligned-20261011.json`, corrected aerodynamic tail `vehicle-aerodynamic-tail-aligned-20261011.json`, constants `vehicle-driver-constants-20261011.json`.
- Preservation `local/scene/preservation-20261011-vehicle-driver.json`; final audit `local/behavior/vehicle-driver-final-audit-20261011.json` records exact final executable hashes and reports.

Final prefix adapter SHA-256 `62ab9148232102670ab1a7779a3d78ee2d225be63f15f5cee5466a9bc6510411`; rebuilt prepared adapter `99551276c810131212e6a66b0ffb4727339d2370e58ef1267b1b8df66157a6d2`; force CLI debug `5e5598eced6571b852b2419caef6ecca2bbb27c936fc3ba619eb942129c7ab61`, release `6d8483be362949f6ca1df717b038482b8709830c07ba6ed6ee961be01a922d38`.

Reference SHA-256: `aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514`. Steam executable SHA-256: `dd7b0933abf0d421bec3fa6f123319d3d3cc7221e5ef82b7a1201c3d1979e48f`. Existing Rust/Cargo 1.99.0, Python 3.12.14, MSVC 14.44.35207, Developer Prompt 17.14.41, Windows SDK 10.0.26100.0, Ghidra 12.1.4/JDK 21.0.9, Capstone 5.0.6 and Bevy 0.19.1. Native flags `/W4 /WX /O2 /MT /std:c17 /fp:strict /arch:SSE2`.

Build `build_vehicle_driver_prepare_probe.cmd` and existing four downstream adapters; build `grid-vehicle-force` with Cargo in debug/release. Run `validate_vehicle_driver_prepare.py`, `validate_vehicle_driver.py` and prior `validate_vehicle_contact_chassis.py` with fresh private output directories and explicit `--profile debug|release`.

Next: close source vehicle initialization and sparse real gearbox tables, then join the authoritative physical-world/body bridge and fixed-step schedule. Integrate that renderer-independent update into Bevy input/camera/restart/checkpoints rather than adding unrelated isolated components.
