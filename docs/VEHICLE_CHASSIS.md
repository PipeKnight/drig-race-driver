# Local chassis motion and source-update integration — F177

11 October 2026, Europe/Kiev. Independent Rust now extends the source contact, suspension, controls, wheel-force and assistance update through whole `9BE1E0 -> 9BDF30 -> 67C200`. This publishes a separate local chassis matrix at body `10..4C`, filtered acceleration at `50..5C` and two noise phases at `60/64`. It does not advance or replace the physical pose at `15E0..161F`.

## Recovered behavior

The complete original chassis routine consumes the current and previous linear velocity, physical side/forward axes, suspension heights, local wheel geometry, tuning coefficients and two owned sample tables. Its order is:

1. Filter acceleration with a clamped blend. Compute velocity difference times inverse delta before applying the blend. Project onto side/forward axes, clamp each projection and store the projected vector back as filtered history. Construct the visual offset from separately scaled projections.
2. Interpolate amplitude between two tuning endpoints according to speed. Advance both phases by `(rate * abs(delta)) * 20`. Subtract 60 once only when a phase is strictly greater than 60. Interpolate the first table to produce a clamped scale, then interpolate both tables to form X/Z noise.
3. Calculate suspension roll from left/right axle means and pitch from front/rear means. Combine roll, noise, up, acceleration offset and pitch in the observed instruction order. Normalize using the hardware `RSQRTPS` seed and original refinement.
4. Compare axle heights above their thresholds. The shared excess becomes translation height. Unequal excess produces a front or rear slope limit using wheel Z relative to the supplied center. When the normal exceeds that limit, reconstruct and normalize it from horizontal, perpendicular and up components.
5. Whole `9BDF30` clears normal W before constructing the basis. Whole `67C200` supplies cross products with positive-zero W. Calculate forward before conditionally negating all four side lanes when side X is negative; the forward vector is retained. Publish the normal times height as translation with W forced to one.

The phase boundary matters: exactly 60 is retained, so the first interpolation index reads the adjacent owned word. The next sample uses `(index + 1) % 60`. Rust and the native guard retain this behavior, require finite table storage through `248` and reject updated phases outside `[0,60]` before invoking original code. Replacing the comparison with `>=`, repeatedly wrapping arbitrary inputs or silently clamping the index would change the selected contract.

The SIMD reduction is `(Z product + W product) + (X product + Y product)`. Normalization preserves `(1 - (seed * square) * seed) * (seed * .5) + seed`. The acceleration, noise, slope and matrix operations retain their observed float grouping, including signed-zero publication. The decompiler's regrouping is not the arithmetic specification.

## Implementation and ownership

`grid-world::vehicle_chassis::apply` validates finite consumed operands, positive bounded delta, finite inverse delta, nonnegative projection caps, nonzero finite wheel spans, distinct interpolation endpoints and bounded updated phases. It clones the body, computes the complete stage, validates output and commits only on success. Nonfinite or singular normalization is rejected. Stored addresses remain opaque.

`vehicle_contact_update::apply_with_chassis` joins F175 vector preparation/source contacts, F173 controls/forces and F176 post-force assistance with this stage on a complete `Frame` clone. Failure preserves body, view, solver, material bindings and diagnostics. `grid-vehicle-force` adds source-backed `MF01` diagnostics; earlier modes retain their boundaries. Body-only `grid-vehicle-chassis` uses `MA01`, records of `0x2604` bytes, a maximum of 64 records and private create-new output.

The native adapter calls unchanged whole original helpers in the hash-checked isolated reference mapper. It executes no program entry point, imports or TLS callbacks, follows no foreign captured pointers and replaces no helper. One private lazy-math data page at `E58000` becomes writable with its prior protection saved; its original contents are restored before each case and on cleanup. Executable sections are compared against their initial bytes. The fastcall bridge with dummy EDX preserves ECX plus stack-float `thiscall`. The embedded manifest is `asInvoker`, `uiAccess=false`; x87 uses 53-bit nearest rounding.

The joined reference explicitly composes four adapters: source vector/suspension/grip, prepared controls/forces, post-force assistance, then chassis motion. Rust uses one API call. Each original/repeated-original/Rust branch carries its own outputs with the existing authored history transfer. Source geometry and full material mechanics are used. Earlier controls/history, tuning/table inputs and material-table binding remain supplied; this is not whole `A03670` or a world tick.

## Verification

| Comparison, per debug/release profile | Calls | Complete bytes | Guard outcomes |
|---|---:|---:|---:|
| Whole chassis: 320 initial + 512 carried | 832 | 8,097,024 | 92 |
| Source contacts/forces/assists/chassis: 120 initial + 192 carried | 312 | 4,799,808 | 75 |
| Fresh prior F176 post-force regression | 1,549 | 15,074,868 | 102 |
| Fresh prior F176 source integration regression | 312 | 4,799,808 | 71 |

Every compared word and repeated-original result matches. Standalone coverage includes 802 matrix changes, 527 filtered-acceleration changes, 780 phase changes and 698 nonzero translations. Controls include speed/blend clipping, tilted bases, retained fourth lanes, unequal front/rear heights, zero rates and phases around/exactly 60. The physical pose is independently asserted unchanged by the chassis stage.

The joined path has 312 chassis-matrix and acceleration changes, 292 phase changes, 101 source contacts/nonzero grips, 1,227 wheel-spin changes and 17 active post-force angular recoveries. It preserves the physical pose produced by the preceding source suspension stage. Guards cover nonfinite operands, unsafe phases, zero spans, invalid delta, framing and existing-output preservation. A late arithmetic-overflow case returns a checked failure from both original and Rust adapters. Unit tests prove local/physical separation, rollback after acceleration computation and whole-frame rollback after completed contact, controls, solver and assistance execution.

All **134 library tests** pass: 113 world, 20 formats and one tools. Formatting and all-target warnings-denied Clippy pass. Python syntax and the final report audit are recorded with private evidence.

## Failures, scope and next work

- Native compilation initially rejected a shadowed local under `/WX`; it was renamed before execution. An unnecessary Rust parenthesis warning was removed. Refactoring the shared integration test removed its implicit `f32` type anchor; an explicit float type fixed the compile error before running it.
- Clippy proposed commuting an addition and replacing the explicit original sign multiplication. Scoped allowances retain the observed operand order; the repeated check passes. No numerical mismatch or native fault occurred in the chassis comparisons.
- The first debug draft had 84 guard outcomes. Final comparisons add late arithmetic overflow, malformed framing and output preservation, giving 92. The joined draft already matched; final reports use the complete 75-outcome guard suite.
- The shared native adapter was rebuilt and both preceding F176 comparisons rerun in both profiles. A changed adapter hash reflects the shared-source dispatch change; original executable code remains byte-identical during each run.
- No whole driver, full source vehicle initialization, physical-world connection, Bevy driving or completed lap is claimed. The dated progress estimates remain unchanged. Next recover the earlier `A03670` state/control/history interval and authoritative world transfer; do not advance the pose twice when joining existing source suspension and physical scheduling.

## Private evidence and reproducibility

All reference bytes and generated records remain ignored under `local/behavior/`:

- Whole chassis finals: `vehicle-chassis-20261011-debug-final-01/validation.json`, `vehicle-chassis-20261011-release-final-01/validation.json`.
- Joined finals: `vehicle-contact-chassis-20261011-debug-final-01/validation.json`, `vehicle-contact-chassis-20261011-release-final-01/validation.json`.
- Fresh regressions: `vehicle-post-force-chassis-regression-20261011-debug-01/`, corresponding `release-01/`, and `vehicle-contact-assists-chassis-regression-20261011-debug-01/`, corresponding `release-01/`.
- Surveys: `20261010T2054199422959Z/9be1e0.c.txt`, `20261010T2057582065641Z/9bdf30.c.txt`, `20261007T1659177524167Z/67c200.c.txt`. Aligned `vehicle-chassis-aligned-20261011.json`, corrected tail `vehicle-chassis-tail-aligned-20261011.json` and constants `vehicle-chassis-constants-20261011.json`. The first tail range started before an instruction boundary; ignore its first two artifacts and use the corrected aligned tail.
- Test log: `vehicle-chassis-library-tests-20261011.log`. Preservation: `local/scene/preservation-20261011-vehicle-chassis.json`, **11 October 00:09:39.139804 UTC**, all 130 original resources and executable unchanged.

Reference SHA-256 `aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514`. Native chassis adapter `dcd1acc2ac1af95b60fee4ca0465a13b68149a6f6476d4cd272ef432fe1fece9`; chassis CLI debug `e0f567d2c30440be69c759d74b64a725231872658b3eb6a5f422bd2418a38121`, release `35261b009aa2ce5b617c879b764247339c26f491a26da003f26c772114b67371`. Rebuilt F176 native adapter `053adf83fc38bf171ca167999f107d23729a6ad326635af719ea7f52822490cb`. Per-execution hashes are retained in each report.

Existing tools reused: Rust/Cargo 1.99.0, Python 3.12.14, MSVC 14.44.35207, Developer Prompt 17.14.41, Windows SDK 10.0.26100.0, Ghidra 12.1.4/JDK 21.0.9 and Capstone 5.0.6; Bevy 0.19.1. Native flags `/W4 /WX /O2 /MT /std:c17 /fp:strict /arch:SSE2`. No installation, desktop input or game/settings edits. Desktop permission was renewed during this work; the expired overnight automation remains paused.

Build using `build_vehicle_chassis_probe.cmd`, the existing prior adapter scripts and `cargo build -p grid-probes --bin grid-vehicle-chassis --bin grid-vehicle-force --locked` (also release). Run `validate_vehicle_chassis.py` and `validate_vehicle_contact_chassis.py` with fresh private output directories and explicit `--profile debug|release`. Regress using the preceding F176 validators.
