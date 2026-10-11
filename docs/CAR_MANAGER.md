# Single-car manager and frame ordering

F194, 11 October 2026. Whole original/repeat/Rust comparison passes 428 manager
intervals in both debug and release: 58,305,584 complete owner bytes and
59,106,800 observable bytes per profile. This includes 192 carried intervals,
96 zero-delta calls and 44 source vehicle tables.

The original frame caller runs the car manager before the physical world
scheduler. This matters because the manager repairs the car basis and records
history after the jobs complete. Those history copies belong to the current
completed update; our earlier standalone job fixtures supplied history outside
the job. A full frame also has deferred world completion stages, so a simple
job/advance/solve concatenation is not established as the original frame.

## Observed original sequence

Aligned `5E7F30` calls `A02B70` with the stored float timestep and task manager,
optionally drains service callbacks through `A00AE0`, then calls world wrapper
`A72930` with the same timestep. The car manager:

1. Processes pending activation flags and deferred physical edits.
2. Runs inter-car factor setup `9B2330`.
3. Initializes 20 completion objects, signals idle slots, resets each scheduled
   job's event and dispatches through `A7D710`.
4. Waits for all 20 original completion objects through `A7D530`.
5. Drains deferred edits, calls conditional wheel constraint producer `9BAB10`,
   repairs forward/up/side vectors and copies pose and velocity history.

Direct dispatch is the original task manager's `+18 == 0` branch. It calls the
job table's `+4` entry `A0A290`; no authored replacement dispatcher or completion
observer produces the new comparison output. The original completion calls
the real Windows SetEvent. Original ResetEvent and WaitForMultipleObjects
imports are also bound by name using the existing isolated import adapter.
Twenty owned auto-reset events are created outside the mapped original code.
The original wait initializes its own handle list through the original getter.
After wait-all returns, all 20 events must be unsignalled and every table,
backlink, handle and wait-list pointer must retain its owned identity.

`9BAB10` is a constraint producer, **not a physical pose setter**. It requires
the producer enabled, a car wrapper and a nonterminal wheel with distance40
greater than limit50 and a nonzero other-body wrapperC4. It transforms a wheel
point by the published physical matrix and calls `A72400 -> B15E70`. C4 is a
pointer; interpreting its bits as a float does not identify its meaning.
`B15E70` appends a bounded 80-byte contact, makes its point relative to the
linked first body's `+334` origin and increments producer count924. Active
production is the next integration target, not part of this checkpoint.

## Selected Rust contract

`grid_world::vehicle_manager::calculate` takes the same owned car/physical,
handling queries and source contact callbacks as the complete car job. It
supports one registered car, empty deferred-edit lists, and an unchanged
activation transition (pending flags may be cleared). It validates consumed
single-car factor inputs; with no other cars the original factor ends at one.
It then runs the whole query-aware job and checks actual post-job values before
the inactive wheel-constraint branch.

Forward normalization consumes all four lanes. Up is normalized cross(forward,
old side), then side is normalized cross(new up, new forward). The original
SSE reciprocal-square-root seed and refinement grouping are preserved; repaired
vectors need not have exactly unit floating-point length. Pose15E0..161F copies
to B40..B7F, velocities15C0..15DF to B80..B9F. This tail does not publish the
physical pose. All changes remain private until success, including a late
degenerate-basis error after sync, mass publication and pending-flag changes.

Private TF01 extends the complete KF02 record without further tail storage.
DC8 is an owned presence marker0/1 in the interchange format; the native adapter
temporarily binds marker1 to the owned wrapper. Live process addresses are never
dereferenced by Rust. Positive contact fixtures currently omit DC8 to keep
production inactive. Zero-driver fixtures also exercise an installed producer
with absent other-body wrapperC4.

## Frame scheduler boundary

`A72930` selects substeps. Intermediate steps call whole `AD8D70`, then contact
callbacks `A727A0` and body service `A71C70`. The final step calls `AD8E30`,
which starts body advance and leaves world phase1. Later `AD8A40` phases join
advance, produce geometry contacts, group/solve and join final callbacks.
These are recovered static branches, not measured live cadence.

Read-only live snapshot `local/behavior/car-world-scheduling-live-20261011.json`
at 14:37:31.357488 UTC confirms 20 job slots, 12 exact registered car identities,
the original job/completion tables, the same stored world/job delta, 20 wait-list
identities, empty deferred-edit lists, enabled producers and no current wheel
distance crossing. Both world async modes are enabled. The snapshot cannot
prove temporal ordering or the selected serial contract during normal play.

## Evidence and limits

Reports: `local/behavior/car-manager-{debug,release}-20261011-final-01/validation.json`.
Every compared basis and both history ranges change in all428 cases; velocity
changes348, factor252, pending flag clears126, mode changes201, engine255,
handling252. There are392 wheel contact slots,396 query-present intervals,
214 notification-enabled intervals and48 zero-driver installed producers.
Thirty-six paired malformed/framing outcomes per profile pass, including a
degenerate authoritative forward basis and an active wheel constraint. Invalid
outputs are removed. Original code remains unchanged RX; eight per-case global
pages and the complete import page are restored. Events are closed, and all
successful waits consume all20 event signals.

Fresh KF02 regression passes444 whole-job calls per profile with all64 semantic
guard outcomes,14 framing outcomes and eight independent query/notification
checks: `query-car-job-{debug,release}-20261011-f194-regression-01/validation.json`.
Full library tests157, fmt and all-target warnings-denied Clippy pass.
`local/behavior/car-manager-final-audit-20261011.json` checks20 reports and1132
conditions, including exact comparisons, guard outcomes, current executable
hashes and changed Python ASTs. Original130 source resources and executable
still match at14:57:21.459994UTC in
`local/scene/preservation-20261011-car-manager.json`.

Native manager SHA256 `5ee50d6341a6624a6460caf496e5d85a2a660400330b7b5b2e39ff5b3f552359`;
Rust debug `aada490bd1307c3f9ab70df358354ab37ce20e785cd3187b65dd7f4216d65ef1`;
release `19af3f4e82b429c76bdb1d2c5ab1f1617b5c95923741dfee6d7cd14f773bb170`.

Failed drafts remain private. Draft01 exits80000001 without a captured EIP.
Draft02's first-chance observer records C0000005 at relocated A7D736 reading
D1543C: authored table pointers missed the actual mapped image relocation.
Rebasing the owned pointers fixes it; original code is unchanged. Draft03
safely rejects its fourth case because the selected contact would produce a
constraint. The final fixture explicitly omits positive-case DC8 and exercises
installed inactive production at zero delta. No stack-exhaustion diagnosis or
stack workaround is claimed. Unit assertions were corrected for refined-rsqrt
rounding and zero-delta timer preservation before the final tests passed.

Private static evidence: `20261011T1433384595310Z/`,
`20261011T1434086142050Z/`, `20261011T1434391828053Z/`,
`20261011T1434597869128Z/`, `20261011T1435496843492Z/`,
`20261011T1445568125338Z/`, `car-manager-whole-aligned-20261011.json` and
`car-manager-wait-aligned-20261011.json`, all under `local/behavior/`.
Pinned analysis image SHA256:
`aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514`.

Installed tools reused: Rust/Cargo1.99.0, Bevy0.19.1, Python3.12.14,
MSVC14.44.35207/SDK10.0.26100.0, Ghidra12.1.4/JDK21.0.9 and Capstone5.0.6.
No input, settings, installation or Steam-file changes. Proprietary evidence
remains ignored under local/.

This is not multiple-car slipstream, activation rebuilding, deferred physical
edits, active wheel constraints, asynchronous worker-pool equivalence, the whole
world scheduler, full source initialization or a drivable Bevy lap. See
[complete job/query scope](HANDLING_QUERIES.md) and
[grouped physical solving](GROUPED_PHYSICAL_FRAME.md).
