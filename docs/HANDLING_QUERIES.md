# Handling queries in the registered car job

11 October 2026, F193. The selected whole [registered car job](CAR_JOB.md)
now supports an owned optional handling-query list and enabled notifications
while deformation stays below the event threshold. This closes the query-object
and healthy-notification difference found in the paused player car. It does not
implement notification events, the original query producer or task manager,
geometric body contacts, persistent world scheduling or a drivable Bevy lap.

## Observed reference and recovered contract

The identity-checked, read-only private GRID snapshot at **10:38:51.034035 UTC**
has twelve registered cars, all with nonzero component query receivers and
**zero query counts**. Notification byte442 is one, status444 is seven and all
terminal wheel flags are zero. The player component mode is zero; AI components
are mode one. The selected adapter continues to own mode zero. This paused,
non-atomic observation establishes neither moving-race queries nor dispatch
timing. No keyboard/mouse input or settings changes were made.

Whole `98BFE0` reads the signed count at receiver888 and searches in order for
the first matching ID at `B3C + index*2E0`. It returns the record at
`8F0 + index*2E0`; handling consumes its status at record58. Empty lists return
no record. Duplicate IDs use the **first** record even when a later duplicate
has status four or five. The owned API bounds lists to64 records; this is our
selected safety bound, not a recovered maximum original capacity.

The whole `9F3E80` and `9C5B90` branches establish these effects:

- ID1F with status4/5 sets mutable10 to one after body wear/probability
  normalization, and skips the `338 * ramp(level10,33C)` drag contribution.
- ID45 or ID46 with status4/5 changes effect3BC to `clamp(1 - parameter2FC,0,1)`.
- A present query object always adds `parameter300 * positive_zero` to cap3A0
  before clamping, even with an empty list. An absent object skips that addition.
  The distinction can change the sign of a zero result.

The ID meanings remain unnamed; observed branch behavior does not establish
their higher-level gameplay meaning. The API owns immutable `{id,status}`
records and never dereferences captured process pointers. `None` means absent;
`Some(empty)` means a present receiver with count zero.

## Healthy notifications

Whole `9F2F80` first computes the maximum of angular and displacement ratios,
including four vector lanes and the original SSE reciprocal-root refinement.
Before clamping the published level or comparing it to prior damage, it checks
the **unclamped ratio** against `parameter288 * stored_f32(0.01)` using strict
greater-than. With notification442 enabled and status seven, a crossing may
enter event production through the activation and external message helpers.

Consequently, a105-percent threshold does not make all notifications inactive:
the ratio can exceed1.05 even though the published level caps at one. The new
API accepts442 zero or one, but rejects any enabled crossing before committing
state. Equality is accepted. A prior level above one does not itself trigger an
event; either signed-zero angular setting disables the entire visual update
and its event test. Terminal wheel flags remain zero and above-threshold
collision damage remains rejected. Event callbacks are not approximated.

The Rust wheel producer now exposes its existing raw ratio internally. Its
legacy level API keeps the same rounding, equality, monotonic and signed-zero
behavior. `vehicle_handling::{calculate_with_queries,publish_with_queries}` and
`vehicle_job::calculate_with_queries` share this producer and preserve complete
input on error. The legacy APIs retain their notification-disabled scope.
A test reaches a fourth-wheel event after earlier wear/RNG work and after
publication changed physical velocity; the complete caller input is preserved.

## Whole-call comparison

The native adapter executes the unchanged whole `A0A290` job, including original
sync, driver, notification-one handling/publication and `98BFE0` lookup. An
owned receiver has the exact count/stride/ID/status layout. Its complete storage
is checked unchanged, along with component linkage and the existing original
completion count/receiver contract. No car, physics or query helper is replaced.
Executable sections remain unchanged and the eight per-case global pages are
restored. Handling preflight uses whole original sync/driver/notification-zero
publication on scratch state, then restores all entry state before the sole
result-producing job call, as documented in F192.

Bounded `KF02` extends the existing diagnostic record with presence/count and
64 ID/status slots. Absent lists must have count zero; unused slots must be
zero. All query data remains private under ignored `local/`. Complete car,
physical body, query records, materials and diagnostics are compared; the
original driver's expired temporary view/solver stays excluded.

Each profile compares444 original/repeat/Rust calls, including192 independently
carried updates,112 zero-delta calls,44 source-table audits and16 additional
controls. Complete owner comparison covers60,485,232 bytes and observable
comparison covers61,316,400 bytes per profile. Query coverage includes empty,
absent, status3/4/5/unsigned-maximum, duplicates, irrelevant IDs and64 entries
with a final matching record. Of444 calls,229 enable healthy notifications.

Both profiles pass64 paired semantic-guard outcomes and14 framing outcomes.
Fresh legacy comparisons each pass428 car-job,396 handling and396
notification-one publication calls per profile, with their existing guards.
All155 library tests, formatting and all-target Clippy with warnings denied
pass. A recursive audit of eight main and eight source-audit reports checks
1,924 conditions, including comparison results, guard/framing outcomes,
Python syntax and current executable hashes; all pass. The audit is
`local/behavior/query-car-job-final-audit-20261011.json`.

Eight independent checks verify signed-zero addition, status4/5 forcing and
drag exclusion, first-duplicate selection, ID45/46 factors, identical whole-job
results with healthy notifications off/on, exact threshold acceptance, prior
level independence and both signed-zero disabling settings. The guard suite
rejects a crossing only one angular-input ULP above the exact threshold, also
when prior level is already two. Malformed query framing and retention are
checked in addition to the inherited source/physical/handling guards.

The positive driver stress fixtures intentionally include arbitrary homogeneous
W lanes. Enabled-notification cases preserve these complete inputs and use an
explicitly authored4.5 displacement span; disabled cases retain.45. Separate
zero-driver controls use the recorded25.6879005 angular limit, .45 span and105
percent threshold. These controls are not a claim that the full moving game
has been reconstructed from captured live state.

Private evidence in `local/behavior/`:

- `handling-queries-live-20261011.json`
- `handling-query-notification-aligned-20261011.json`
- `handling-notification-constants-20261011.json`
- `query-car-job-{debug,release}-20261011-final-01/validation.json`
- Adjacent `*-source/{validation,shift-validation}.json` audits
- Fresh F193 legacy car-job/handling/publication regression directories

Failed drafts are retained. Draft01 safely rejected a synthetic deformation
crossing at native guard34; it exposed the deliberate arbitrary fourth lanes in
the old stress fixtures. Draft03 rejected uninitialized impact history when a
positive fixture was reused at zero delta; the driver then skips its producer,
so those newly consumed entry slots are initialized explicitly. Draft04's
independent off/on assertion included different per-case identity words despite
original/repeat/Rust byte agreement. The assertion now normalizes only that
framing identity and the intentionally different notification byte. No original
fault or numerical discrepancy was observed in these drafts.

Installed toolchain is unchanged: Python3.12.14, Rust/Cargo1.99.0,
MSVC14.44.35207 (Developer Prompt17.14.41, SDK10.0.26100.0), Capstone5.0.6,
Ghidra12.1.4/JDK21.0.9 and Bevy0.19.1. No tools were installed for this step.
The first incremental debug build emitted the existing OneDrive hard-link
fallback warning; subsequent builds use `CARGO_INCREMENTAL=0`. Original Steam
executable and130 traced resources still match baseline at14:30:58.674196 UTC
(`local/scene/preservation-20261011-query-car-job.json`).

## Next integration

Establish the original ordering between the registered car jobs and world
advance/contact production/solve, then carry that ownership across successive
physical steps. The verified [grouped physical sequence](GROUPED_PHYSICAL_FRAME.md)
can supply its existing solver stages once ordering and geometric contact
inputs are established. Source construction and the playable Bevy path follow;
the present renderer is still a static viewer.
