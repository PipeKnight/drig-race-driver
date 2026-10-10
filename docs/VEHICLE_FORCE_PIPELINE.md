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

## Prepared synchronized whole-update capture — F165

This is preparation, **not a completed live capture or whole-driver replay**.
Existing `tools/capture_reference_spawn.py` has an opt-in `wheel-force` boundary
at `9DF8B0`, requiring its already verified arm-running/hardware-breakpoint
workflow and exact private process path/hash/start identity. Aligned complete
caller inspection in `local/behavior/vehicle-force-caller-aligned-20261010.json`
shows the cdecl call at `A0417D`, return at `A04182`, then the caller's eight-byte
argument removal. The return breakpoint is conditioned on the captured stack
frame to exclude other vehicle workers using the same caller.

The decoder checks Standard-body registration, source-selected axes, four
wheels, supported mode, driven-wheel ownership and each wheel's matrix-owner
link. It captures fixed-size body `2600`, view `300` and solver `DD0` buffers,
material bytes, engine-limit scalar and mutable friction diagnostics before and
after the call. The material pointer used by `9B5660` is at **wheel+C0**, with
its multiplier at pointed+64; do not confuse it with wheel+64. The temporary
body/view/solver pointer identities must remain equal through return.

Entry solver counts may be stale from another car: only raw bounded bytes are
captured, with no variable-length dereferences based on those counts. Hardware
breakpoints leave code bytes unchanged; the existing checked exception-consume,
resume and detach lifecycle is retained. The tool sends no game input. Macros
remain a separate owner-authorized, foreground-guarded Computer Use workflow.

`tools/check_wheel_force_capture.py` passes one owned-memory fixture, retains
deliberately invalid stale solver counts, and rejects thirteen caller/delta/
registration/mode/ownership/pointer faults. These are synthetic decoder guards;
they do not prove debugger compatibility or original execution of the new mode.
The CLI help and Python AST checks pass. No native attachment, breakpoint,
game launch, keyboard/mouse input, settings edit or tool installation occurred.

Computer Use window inventory found ARC Raiders open and no GRID process.
Desktop availability was asked once and remains pending. The owner has returned,
so the expired unattended desktop window is not reused. Once available: inspect
the private reference window, load the documented Lacetti/Donington race,
record a fresh exact process session, arm six whole-update captures, use only
the observed-state guarded macro, then inspect pause state and detach evidence.
If activation/capture fails after the documented bounded recovery, preserve
failure evidence and continue offline rather than repeat blind input. Captured
bytes remain under ignored `local/`.

## Complete live capture and original replay — F165

The preparation-only status above is superseded by a successful owner-authorized
desktop run on 10 October. Observed race setup: Chevrolet Lacetti / Donington
Park Grand Prix / three laps / Nevada Autosport, Normal difficulty, Random grid,
four flashbacks, traction/braking/stability assists On and Automatic transmission.
The existing saved profile loaded. These settings were inspected without edits.

Computer Use's plain executable launch resolved to Steam app12750 and exposed no
targetable window. One retry using the explicit `process:` private-executable
identifier succeeded. This verifies that retry on this run, not universal launcher
compatibility. GRID PID13940 started `2026-10-10T13:38:26.3079980Z`, compatible
executable SHA `b1da1b2d9469fb0c973dbaaad29b230c1567f212c54de849326f80b4aece34a6`.
Fresh identity: `local/reference-race/force-20261010/session.json`. Existing
AutoHotkey2.0.29 handler22436 was bound to that exact process; guarded F5 resumed,
held throttle for a reported 2000ms, released and paused. Hardware debugging
disturbs game timing; this hold is not a simulated-tick measurement.

Six complete `9DF8B0` entry/return pairs are retained in
`local/behavior/wheel-force-live-20261010-01/`. Metadata SHA:
`e42ef2d763e2fae0de50bc6910d6ad8e03946af1a5c840c0a8a53dd2d4f1e2e5`.
Capture completed, code remained exact, the pending hardware stop was consumed
and resumed, and debugger detached with exit0. A subsequent screenshot showed
Pause Menu / Continue. The capture tool itself sends no input.

All six calls use mode1, four wheels and delta word `3bc8e061`
(approximately0.006130263675 seconds). Each changes all six linear/angular XYZ
words and all four wheel-spin words. View success byte54 is1 in every return.
One return has logical-row count20 / physical-row count16, while the others have
20/20; all have25 variables. This directly exercises active row compaction.
The six distinct body identities have not been mapped to player versus AI or
vehicle model. The player's selected Lacetti does not establish that every
captured body is a Lacetti.

`tools/probe_vehicle_force.c` executes the complete original routine and its real
dependencies in an isolated process, with hash-checked analysis PE, owned
buffers, no entry/import/TLS execution, no helper replacements and no executable
page modifications. Explicit relocations cover body/view/solver, wheel matrix
owners, driven-wheel identities and material aliases. Those fields are checked
afterward and restored to captured identities for comparison. Captured engine
limit and friction diagnostics are supplied; six data pages are temporarily
writable and restored, including the existing lazy SSE-mask pages. Every
executable section is compared against its pre-call bytes.

The live layout puts solver at **view+2B0**. Thus the fixed300-byte view capture
overlaps the first50 hexadecimal bytes of the DD0-byte solver. The owned replay
preserves this alias and material-pointer aliases; it neither masks nor drops
the overlapping output bytes. Initial replay01 used separate buffers and had199
differing bytes, all in that overlap, despite exact body/full-solver outputs.
Correcting the allocation produced full exactness in02 and final03. The first
native build also rejected a signedness warning under `/WX`; corrected before
execution. No native exception or numerical mismatch remains.

Final report:
`local/behavior/vehicle-force-replay-20261010-final-03/validation.json`.
Original and independent repeat reproduce all **92,312 framed bytes** exactly
against the six live returns, including complete body/view/solver/materials and
diagnostics. Fifteen malformed-fixture checks reject before original execution.
Probe SHA `448ddafb7307af0348ff2cb57753c3cccdf65988249b082f92a70de22b0aac0c`;
request SHA `8552f09f9c710db53c9c2d1eba2232aa70b4a4cd5cbcac5946c2337e06aeb605`;
expected/original/repeat SHA
`df36804ef6e1ec94fc33ac475112eb746ed344c61471cc4cae3b4115e5bdc3ec`.
MSVC14.44.35207 / SDK10.0.26100.0 / Python3.12.14 were reused; no installation.

This is whole-original replay evidence, **not yet independent Rust vehicle-force
equivalence**. Mode0/2, player identity, broad driving states and live time/input
cadence remain unverified. Next join row production, group selection, active
limits and event advancement around F164's Rust equation core, preserving logical
row remapping and stale storage. Fresh successful Ghidra survey
`local/behavior/20261010T1351380646699Z/` recovers whole9922A0 compaction,
9DF330 force application,9DF540 event advancement and9B5520 grip limiting.

## Joined Rust vehicle-force update — F166

`grid_world::vehicle_force::apply` now implements the whole selected four-wheel
**mode1** update around the verified equation core. It initializes brake/grip
state, temporarily adds drivetrain inertia to driven wheels, builds all vehicle
rows, selects wheel groups, enforces brake/grip limits, compacts logical rows,
re-solves, advances wheel-stop events and applies lateral/longitudinal impulses
through the existing verified angular-response code. It restores wheel inertia
and derives/clamps engine speed. Prepared velocity/axis/lever vectors are supplied
and retained in original caller order; pose is not advanced here.

The equation core now retains raw inactive storage and padding, resets only the
actual counters/mode byte, allocates variables, supports explicit unfixing and
value changes, and preserves logical identity through swap-last physical row
compaction. Deletion updates **all40** logical-map entries, including stale ones,
as the original does. Removed physical tail storage survives; new rows overwrite
only fields the original overwrites. Selected solves resolve logical IDs, while
the remaining solve operates on physical row order.

Aligned loads corrected two potentially misleading static interpretations:
low-speed brake limiting activates below **3.0**, with wheel spin magnitude below
0.5, and uses wheel **+E0** radius / max(grip,10) /0.9. Body-response row coefficient
from CB22B0 is **-0.2**, not-1. The initial Rust draft used-1 and differed on937
live bytes; correcting that coefficient yields full live equality. The earlier
borrow-check failure was fixed before execution. One operand-order Clippy allow
is scoped to the update function; a diagnostic iterator warning was corrected
using bounded array chunks. Failed intermediate files remain private.

Both final debug and release independently match whole original/repeat outputs:

- Six actual complete live calls: all92,304record bytes, plus the8-byte framing.
- 288 varied authored private controls:4,430,592record bytes. These cover positive,
  reverse, zero/signed-zero/tiny wheel spin, zero/negative/large brake inputs,
  airborne/low/high grip, material presence/multiplier, one-to-four driven wheels
  in varied order, ratios/gears, low-speed threshold, delta1e-6..0.1, inertia/mass,
  and deliberately stale solver counts/padding/row tails/maps.176compact rows,
  120mark wheel-stop transitions and252change body velocity.
- 32 independently carried sequences of12updates:5,907,456record bytes. Each
  branch consumes its own previous return, preserving complete state/storage.
  Prepared geometry/vectors remain fixed; these are arithmetic state sequences,
  not road trajectories.
- Fifteen native malformed-input guards and19Rust guards pass. Rust validates
  consumed finite inputs and positive mass/inertia/radius, executes on a copy,
  and creates private diagnostic output only after every case succeeds. Invalid
  inputs leave caller state unchanged. Event/limit iteration bounds64 are
  authored safety limits, not measured original maxima.

Total: **678whole updates /10,430,352record bytes** per profile. No vehicle-level
solver failure occurs in these controls; F164 separately verifies core singular
outcomes. Do not generalize this result to every possible singular vehicle.
Final reports:
`local/behavior/vehicle-force-rust-20261010-debug-final-04/validation.json` and
`local/behavior/vehicle-force-rust-20261010-release-final-05/validation.json`.
Earlier debug01/final02 and releasefinal03 also pass but pin prior binaries.
Final Rust diagnostic hashes: debug
`d801de0e473c26ccfe22c9f17f7a65410e9b44cc4fca3d18a002a735b6b08631`, release
`3c4bc540e41bfbf1edae837019c59ee0d85713e82494874f6fb6f25f55bbad24`.
Authored aggregate request SHA
`2cad51da5db82559e172616c58258a596644122e44a45ae8c4e8a83c03733c63`;
carried final SHA
`1c3f47ade17c763d74d3522b4d360b1a8c6998f46331bf74e36f16464f9d00a3`.
The aggregate fixture is split into bounded48-case native/CLI batches.

Fresh F164 regressions still match394cases/7,498snapshots/6,635,730words per
debug/release after the shared core changes:
`local/behavior/force-equations-20261010-compaction-debug-06/validation.json` and
`force-equations-20261010-compaction-release-07/validation.json`.
All119library tests (20formats/1tools/98world), fmt, all-target warnings-denied
Clippy and Python AST checks pass. Existing Rust/Cargo1.99.0, Python3.12.14,
MSVC14.44.35207/SDK10.0.26100.0, Ghidra12.1.4/JDK21.0.9 and Capstone5.0.6 reused.
No new desktop input/install/settings edits after F165. Steam130resources and
executable remain unchanged at14:04:00.923152UTC:
`local/scene/preservation-20261010-vehicle-force.json`.

This closes the selected mode1 force/limit/event boundary, not complete driving.
Modes0/2, synchronized player identity and long driving trajectories are open.
Next prioritize the real suspension/contact preparation wrapper and authoritative
vehicle-to-physical state transfer; join those to Bevy input/camera only once the
whole ordering is checked. Merely chaining supplied-boundary components would
not establish original contact production or world timing. Completion estimates
remain unchanged, and the expired overnight automation stays paused.
