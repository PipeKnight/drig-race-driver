# Selected whole serial physical world step

Checkpoint F197, 11 October 2026. Rust matches the unchanged original enclosing
`AD8D70` step followed by actual `A727A0` contact-pool consumption. This closes
the selected cold serial scheduling boundary around the previously verified
[wheel-contact handoff](WHEEL_PHYSICAL_HANDOFF.md). It does not yet establish a
persistent driving loop or the live asynchronous frame schedule.

## Observed lifecycle and selected contract

The original world step resets producer statistics, prepares workers, advances
active bodies, joins work, runs geometry queues, reconstructs contact groups,
solves them and completes publication. `B152E0` clears producer statistics at
`A78/A7C`; it does **not** clear the external wheel-contact count at `924`.
The enclosing wrapper `A727A0` dispatches `A726B0` for these records, then clears
`924`. With both collision-shape callback slots `21C` absent, dispatch returns
without invoking collision callbacks. Contact storage survives consumption.

The adapter owns all world/producer/worker/broadphase storage, both callback-free
shape identities and the supplied contacts. Physical bodies have no registered
geometric shapes; geometry worker queues therefore execute empty. Preallocated
workers avoid original allocation. Bodies are active and unsleepable, without
joints, wake requests, deferred pose publication, asynchronous dispatch or the
general registered-body pose cursor. These are explicit limitations, not claims
about the production world constructor. Original executable code stays unchanged;
the adapter restores five lazy-data pages and their protections.

The Rust entry `physical_world::calculate_cold` composes the verified advance,
grouping and solve stages, rejects unsupported deferred publication and reports
the transient pool consumed. Original full execution verifies that the selected
empty stages introduce no additional body, contact or solver changes. Native
retention checks cover the complete owned world/producer/worker buffers outside
documented mutations, broadphase and worker storage, backlinks and shape bytes.

## Validation

Original, repeated original and Rust agree in both debug and release:

- 465 scenarios, 2,012 bodies, 1,357 groups and 6,235 contacts; 26,058,360 output
  bytes per profile, including six lifecycle words after every case.
- 64 shape-filter combinations, 1,526 null-second-body contacts and 4,218 special
  contacts; 6,235 normal and 8,388 friction rows. All 2,012 poses change.
- Fifteen independent checks and 26 paired malformed-input outcomes per profile,
  including unsupported deferred publication. Pool count and producer statistics
  end at zero, world phase is idle, timestep bits are retained and wake queue is
  empty.
- Fresh whole car-manager production feeds 66 complete owners and 176 generated
  contacts into the enclosing world/wrapper, matching 311,872 bytes per profile.
  All 66 velocities and poses change; twenty independent checks pass. Each branch
  consumes its own manager output. Only identity normalization follows production.
  Cold configuration/templates precede production; material and physical timestep
  remain authored separately. This is not a common-cadence whole-frame claim.

Fresh shaped frame/tick regressions each pass all 465 scenarios and 24 paired
guard outcomes in both profiles. All 158 library tests, formatting and
warnings-denied all-target Clippy pass. The eight-report audit verifies 110
independent checks, 148 guard outcomes, current executable hashes, producer
execution results, upstream evidence hashes and three Python ASTs.

The audit's first draft assumed producer summaries had a `passed` field. They
instead expose repeat equality, differences and execution results. Corrected
the audit to inspect those actual fields; no failed native or numerical
comparison occurred. Draft and final evidence remain private.

## Reproduction and provenance

Build `tools/build_whole_physical_world_probe.cmd`; build Rust
`grid-physical-tick` in debug/release. Run existing validators with
`--whole-world`: `validate_shaped_physical_frame.py` and
`validate_wheel_physical_handoff.py`. `audit_physical_world.py` verifies their
saved reports and regressions against current binaries.

Private reports:
`local/behavior/whole-physical-world-{debug,release}-20261011-final-01`,
`wheel-whole-world-{debug,release}-20261011-final-01`,
`shaped-physical-{frame,tick}-{debug,release}-20261011-f197-regression-01` and
`whole-physical-world-final-audit-20261011.json`.
Static surveys are under `20261011T1536218867644Z`,
`20261011T1537216983100Z`, `20261011T1542065456033Z` and
`world-lifecycle-aligned-20261011.json`.

Whole-world native SHA256
`82f2affce954354156aecbfd601e952f7142551470c68298d000f6e984124420`;
Rust diagnostic debug
`6b267f0125273f7ef687175dfe7fdb5e50bd546fe7ce7a54c79cfb7c27a8b81a`, release
`58f96d9fa5b81fc497433078dc7bad0cddff60ef1be788f5b5c1bda5bf2ea155`.
Reused Python3.12.14, Rust/Cargo1.99.0, MSVC14.44.35207/SDK10.0.26100.0,
strict x86/SSE2, Ghidra12.1.4/JDK21.0.9 and Capstone5.0.6. No installation.
Steam's 130 reference resources and executable remain unchanged at
15:51:18.356475 UTC, `local/scene/preservation-20261011-whole-physical-world.json`.
No desktop input or settings changes; old overnight automation stays paused.

## Next work

Preserve active lists, dirty-list membership, embedded group links, manager
workspace/cache and reused contact storage across actual successive world steps.
Original `B0F150` retains dirty ownership when byte `412` is already set; clearing
it to satisfy a cold-entry guard would not reproduce the lifecycle. Complete
source initialization and connect vehicle/world updates to Bevy after that
boundary is compared. No drivable Rust lap exists yet; current work requires no
owner action.
