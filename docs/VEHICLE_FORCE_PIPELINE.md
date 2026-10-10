# Selected vehicle force pipeline — F164

10 October 2026. Static observations from the hash-pinned private unpacked x86
reference; this section does not establish whole-vehicle execution or a drivable
engine. The source selection is documented in [tyre selection](TYRE_SELECTION.md).
The installed Standard vehicle inputs and retained flags disable the alternative
coupled tyre axes. Completing that alternative alone would not reproduce the
selected Lacetti path.

The late vehicle update at `A03670` prepares its temporary force view before
`A00E10`, which includes the suspension wrapper `9F1E10` and later wheel history
and grip work. Surface drag at `9DFC50`, later steering coefficients and wheel
state work follow. `9DF8B0` then runs the selected wheel-force subsystem when
body `ADC` is zero. It is a cdecl function receiving body and incoming delta;
the aligned entry loads these stack arguments and sets EDI itself. Several
internal helpers use custom register arguments, which the inferred pseudocode
does not describe reliably.

| Stage | Observed responsibility | Execution status here |
|---|---|---|
| `9B4730` | Initialize brake/grip states, gear ratio and force limits | Static |
| `9B4F50` | Allocate solver variables and build sparse equations | Static |
| `9B5370` | Select small constraint groups, then solve remaining variables | Static |
| `9B5970` | Change force-limit states and re-solve | Static |
| `9DF540` | Select wheel-stop/drivetrain event time within remaining delta | Static |
| `9DF330` | Apply solved wheel forces to body velocity and wheel spin | Static |

`9DF8B0` repeats solves and time advances until the incoming interval is consumed.
Its mode-one branch temporarily adjusts driven-wheel inertia and later derives
engine speed from driven wheels. These branches have not been executed in an
isolated whole-driver fixture. Do not equate a successful linear-system replay
with this complete event/limit loop.

The view pointed to by body offset zero is temporary stack state assigned by
`A06A90`, not a durable object to read after parking the game. A synchronized
capture inside `9DF8B0` is needed. Its first pointer identifies a solver object.
Known view regions include wheel point velocities at `190`, longitudinal axes
at `1D0`, lateral axes at `210`, and relative lever vectors at `250`. Preparation
precedes intervening body updates; recomputing these vectors afterward would
change the original ordering. [Force-view evidence](FORCE_VIEW.md) supplies the
earlier verified boundary.

`9DF330` advances wheel spin from solved acceleration and stores longitudinal
and lateral forces. It forms each impulse in lateral-then-longitudinal order,
calls the original angular-impulse helper with the supplied relative lever,
and adds the mass-scaled impulse to linear velocity. It does not advance pose.
The [suspension composition](SUSPENSION_FRAME.md) and
[physical tick](PHYSICAL_TICK.md) therefore remain distinct integration work.

## Linear-system boundary

The solver holds at most 40 variables and 40 sparse rows in the observed
bounded callers. A variable occupies eight bytes at `10`: value, fixed byte,
temporary solved byte and two padding bytes. Each row at `150` has stride `4C`,
up to eight coefficients and variable IDs, a right-hand side and temporary
selection byte. Logical row IDs map through `D30`. Counts occupy `0`, `8`, `C`;
the complete bounded buffer ends at `DD0`.

`992320` appends an equation, substitutes already-fixed variables in input
order and excludes zero coefficients on unfixed variables. `9930A0` fixes a
variable, substitutes its first occurrence in each existing row and removes
that term by swapping in the last term. It retains stale storage beyond the
new term count. `9925E0` builds a dense matrix for an explicitly selected closed
row/variable group and invokes `991920`, publishing values only on success.
`992FD0` selects remaining nonempty/unmarked rows and unfixed/unsolved variables,
then clears temporary row and variable marks even after a failed solve.

`991920` uses single-precision elimination with row-norm-scaled pivot selection,
strict comparison and first-row tie retention. It checks each elimination
pivot for zero; the final back-substitution division is not separately guarded.
This is an observed implementation detail, not a recommendation to use it as
a general-purpose numerical solver. Exact arithmetic order and singular
outcomes require native comparison before a Rust equivalence claim.

## Private evidence and failures

Reference SHA-256:
`aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514`.
Existing Ghidra 12.1.4/JDK 21.0.9 and Capstone 5.0.6 were reused.

- Successful Ghidra surveys: `local/behavior/20261010T1153107866682Z/`,
  `20261010T1153595868676Z/` and `20261010T1156093520208Z/`.
- `20261010T1154587078423Z/` is a partial failure: four requested functions
  succeeded, guessed `993050` was not a function, and `993090` was not run.
  Successful files do not establish a successful complete survey.
- Aligned driver/caller inspection:
  `local/behavior/vehicle-force-driver-aligned-20261010.json`.
  `9DF8B0` returns at `9DFC11`, `9DFC2D` and `9DFC42`; `9DFC50` is separate.
  `A00E10` returns at `A01901`; subsequent decoded functions are not its body.
- Aligned equation ABI inspection:
  `local/behavior/force-equations-aligned-20261010.json`.
  `992320` returns with 16 stack bytes, `9930A0` with eight,
  `9925E0` with 16, and `992FD0` with none. `991920` receives count in ECX,
  matrix in EDX, then output and right-hand-side pointers on the stack.

## Joined independent equation core

`grid-world::force_equations::System` now joins equation addition, fixed-value
substitution, selected closed-group solving and the remaining solve. It retains
the exact sparse term order, stale removed tail slots, logical row identity for
append-only construction, fixed flags and temporary selection/solved flags.
The dense arithmetic uses f32 with the observed multiplication/addition order.
Failure marks selected rows but does not publish new variable values. Remaining
solves clear temporary flags even on a numerical failure.

The private native probe calls whole original `992320`, `9930A0`, `9925E0` and
`992FD0`, including actual `991920` and the original stack-allocation helper.
No helper stubs, executable patches, writable mapped data pages, import/TLS
initialization or live injection are used. Both original executions verify all
mapped executable sections unchanged. The harness owns and bounds all pointers.

**394 authored scenarios / 7,498 operation snapshots / 6,635,730 words match
exactly against original and repeat in both debug and release.** Every operation
compares its normalized success byte and all 884 words of the bounded `DD0`
buffer, not only solved values. Coverage includes 0–40 variables, permutations,
pre/post-construction fixes, duplicates, zero coefficients, arbitrary closed
small-group selections, repeated remaining solves, extra ignored equations,
scaled systems, three numerical failure snapshots and unchecked-last-pivot
nonfinite results. Eight named semantic checks and fifteen malformed-input
guards pass in each profile. These are authored controls, not live vehicle
equations or observed production frequencies.

Final private reports:
`local/behavior/force-equations-20261010-debug-final-04/validation.json` and
`local/behavior/force-equations-20261010-release-final-05/validation.json`.
Earlier debug02/release03 also pass; final04/05 include the bounded total-operation
guard and named semantic results. The profile is mandatory and checked against
the actual executable path. MSVC 14.44.35207, SDK 10.0.26100.0,
Python 3.12.14 and Rust/Cargo 1.99.0 were reused. Native build uses C17,
`/W4 /WX /O2 /MT /fp:strict /arch:SSE2`; Rust uses its normal debug/release
profiles. No additional tools were installed.

All **117 library tests** (20 formats, one tools, 96 world), fmt and all-target
warnings-denied Clippy pass. Three meaningful core tests cover carried fixed
values/cleanup, unclosed group rejection without mutation, and distinct zero
pivot outcomes. Initial Clippy arithmetic-assignment warnings are explicitly
allowed only on the elimination function to preserve verified operand order.
Python AST validation passes. Steam's 130 tracked resources and executable
remain unchanged at `2026-10-10T12:14:20.589033Z`, recorded in
`local/scene/preservation-20261010-force-equations.json`.

Initial debug01 stopped at native guard error16 before the unsafe original
remaining solve: the failed selected group had marked every row but left
unresolved variables. The control was underdetermined at that second boundary.
It now supplies separate closed retry rows. This was a harness/control rejection,
not an original exception or numerical mismatch. The failed output is retained.
A PowerShell brace-list syntax error prevented one read command from executing;
it was corrected. No numerical mismatches or native faults occurred.

The public API rejects unsafe underdetermined or unclosed selections before
mutation. The diagnostic accepts create-new output only under ignored `local/`,
8 MiB input, 1,024 cases, 128 operations per case and 20,000 operations total.
These authored safety bounds are not claims about every original input. The
reference's unchecked last pivot is preserved; a successful return can publish
infinity/NaN and does not establish a physically usable vehicle result.

Whole `9B5370` vehicle group selection, `9B4F50` vehicle row production,
`9B5970` limit transitions/logical row remapping, and `9DF8B0` event advancement
are still open. Next capture a synchronized selected-force boundary and join
these around the verified core. No first driving, complete simulation or
completion-estimate increase is claimed.

| Final artifact | SHA-256 |
|---|---|
| Authored requests | `0c47bea5c494418bfa3c76c996502769c5b9b4c1c881a02db61027adf03eb850` |
| Native joined probe | `42df62ce7f7b3f82e1ff108ba4bc83d5c0bc7d6b53369b1b2d2d2032e4570b52` |
| Rust debug diagnostic | `68756fd6ad7fca72c140e3609d92d4da3246b638409b8a56fa00ddeb4c5ae824` |
| Rust release diagnostic | `5fb24908cc77d85817dce11338d9372ac8d490f97821e5aadcb7b7abec9efec5` |
