# Registered car-job integration

11 October 2026, F192. One unchanged registered `A0A290` car-job call now matches
the independent Rust `vehicle_job::calculate` across the complete selected
synchronization, source-contact driver and notification-one handling/publication
sequence. This establishes the appropriate car-side update for further world
integration. Task dispatch, live callback-manager behavior, geometric body
contacts, persistent world state and a drivable Bevy lap remain unfinished.

## Recovered job contract

Aligned instructions read the car at job `4`, delta at `8`, execute whole
`9BD7F0` synchronization, whole `A06A90` driver wrapper, then whole
`A03220(delta,1)`. The job finally tail-calls table slot `14` on the completion
object embedded at job `C`. The existing static registration table points to
this entry; no claim of observed live dispatch timing follows from that alone.

The native adapter owns the job, its completion table/object, full `21000` car,
linked `424` physical body and exact component backlink. An independently
authored completion observer only records invocation count and receiver identity.
It replaces no game or physics helper. Every accepted original call reaches it
exactly once with the precise embedded receiver. Job fields, table entries and
guard sentinel retain their expected values. This verifies the original tail
call's contract, not the original thread/task manager. Rust returns its complete
owned result after successful stages and leaves dispatch external.

The original job contains neither the extra `9A0` clear nor the final axis
projection/pose setters of the [synchronous wrapper](VEHICLE_OUTER_WRAPPER.md).
Positive driver updates themselves set `9A0` to one through suspension; zero
delta preserves its supplied value. The accepted job preserves the physical
matrices at `190..20F`, while publishing angular velocity at `210` and linear
velocity at `220` without the synchronous axis projection. Synchronization still
updates mass, inertia and inverse inertia. These distinctions are checked
independently from the complete byte comparison.

The API reuses the owned car/frame state type and checks prefix agreement,
empty physical links, finite center offset and the selected active state before
work. Existing synchronization, complete driver and mutable handling each
operate on private next-state storage. A late final steering-ramp failure test
proves rollback across synchronization, driver, publication and handling; its
successful control demonstrates that synchronized mass had changed. Source
queries are not invoked at zero delta.

## Whole-call validation

Both debug and release match **428 updates / 58,305,584 complete owner bytes /
58,884,240 observable bytes**: unchanged original, repeat and Rust. There are
192 independently carried updates and 96 zero-delta controls. All44 supported
installed source tables supply their previously verified gear/engine/shift
initialization; the remaining handling construction, input/history and owned
links are explicitly authored. Each branch carries its own full car and body.

Coverage includes 201 mode transitions, 213 mode-two outputs, 382 contact
slots, 1,311 spin changes, 256 engine-speed changes, 252 handling-level/effect
changes, 218 handling gear-history changes, 252 physical mass changes and zero
physical pose publications. The wrapper's expired native temporary view/solver
is excluded, as in its earlier full-call comparison; full car/physical state,
source material results and shared diagnostics remain compared.

Each profile passes 52 paired semantic-guard outcomes and 14 framing outcomes.
Guards cover source/ownership, foreign physical/mutable links, nonfinite inputs,
unsupported terminal/notification/style/collision settings, zero wear divisor
and framing/alias errors. Partial invalid outputs are removed. No native fault
or numerical mismatch occurred.

Consumed handling intermediates depend on the driver, so preflight executes
whole original sync → driver → notification-zero publication on owned scratch
storage. It then checks the actual consumed handling inputs. Complete entry
car, physical, view/solver, wrapper, component, owned world placeholder, service
table/entry and eight per-case global pages are restored before the sole
result-producing **whole `A0A290`** call. The source material pool is checked
unchanged and restored. No helper is substituted or patched. All executable
sections remain unchanged; eight lazy/global pages are restored on exit.

Fresh synchronous-wrapper regression also matches all428 cases and 36 semantic
guard/14 framing outcomes in both profiles after rebuilding the common native
adapter and Rust diagnostic. All **154 library tests**, formatting and
all-target Clippy with warnings denied pass.

Two failed draft validations are retained privately. Draft01's assumption that
`9A0` preserved the input failed despite original/Rust agreement: the positive
driver sets it to one. The independent assertion now checks driver-one versus
zero-delta preservation and the absence of a later extra clear. Draft02 safely
rejected a zero-delta fixture at native guard33 before job execution: older
driver-only fixtures left unread impact slots as arbitrary bytes, including
NaN/huge values. The joined handling reads those slots even when the driver
skips; zero-delta input impact/wear history is now explicitly initialized to
zero. Draft03 and final comparisons pass. This is fixture scope correction,
not evidence of an original compatibility failure.

Private evidence under `local/behavior/`:

- `car-job-{debug,release}-20261011-final-01/validation.json`
- `whole-vehicle-outer-{debug,release}-20261011-f192-regression-01/validation.json`
- Adjacent `*-source/{validation,shift-validation}.json` source audits
- `car-job-final-audit-20261011.json`
- `car-job-aligned-20261011.json` and earlier registration/live-code evidence

The CLI uses bounded `KF01` input/output, stored only under ignored `local/`.
No proprietary game data or captures are published. Existing pinned Python,
Rust, MSVC, Capstone and Bevy versions from [F191](VEHICLE_HANDLING.md) are
unchanged. No new tool installation, desktop input or settings changes occurred.
Original Steam executable and130 traced resources match baseline at
10:32:23.206462 UTC (`local/scene/preservation-20261011-car-job.json`).

## Remaining scope and next actions

The selected owned component is mode zero/query absent; notification442 is
false, status7 and terminal wheel flags are zero. Above-threshold collision
damage is rejected. Authored handling settings deliberately keep the collision
producer inactive; they are not claimed as the complete original constructor
or source HDM initialization. The actual paused race has queries present and
notification442 enabled, as documented in F191. Those live branch contracts
still need recovery before broad racing equivalence.

Next inspect component queries/healthy notification behavior and connect this
car-side result to original world advancement/contact production. Then establish
persistent multi-step world ownership and initialize the playable Bevy path from
source. The verified cold supplied-contact [physical sequence](GROUPED_PHYSICAL_FRAME.md)
is available for that integration; composing it arbitrarily with this job would
not establish the original scheduler order. Completion estimates are unchanged.
