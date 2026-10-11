# Physical world state across successive steps

Checkpoint F198, 11 October 2026. The selected serial physical world now carries
its own state through up to 64 successive updates, matching unchanged original
execution. This extends the [whole world boundary](WHOLE_PHYSICAL_WORLD.md).
Contacts are supplied initially and consumed by the real wrapper; subsequent
steps have an empty active pool. Repeated external contact production and car
jobs are the next integration task.

## Ownership contract

Original `B0F150` inserts a body into the dirty list only when byte `412` is off.
Once on, dirty head and both embedded links remain unchanged. Group rebuilding
clears group membership and body-chain links, then creates/attaches/merges groups
from the current contacts and active list. Other retained embedded fields,
including old manager links on nonroots, must survive until their actual writer
runs. Active lists alternate selectors each step.

Rust now has a carried ownership contract separate from cold initialization.
It bounds all retained body/contact identities, checks the active permutation
against actual next/previous links, validates the entire dirty chain and exact
flag membership, and distinguishes contact-storage capacity from active count.
It preserves old fields rather than zeroing them to pass cold guards.
`physical_world::advance` consumes a completed world and returns a new one with
retained pool storage, manager properties, cache, row storage and workspace.
Failure leaves its input intact.

The original adapter runs each complete sequence on the **same buffers**. It
does not serialize/normalize between updates, clear dirty flags, reconstruct
lists, reset the manager or rearm the consumed pool. First-step friction-row
references remain actual owned pointers until final serialization. Original
world and worker retention checks still apply. Only final identity normalization
makes original/repeated-original/Rust outputs comparable.

## Verified comparisons

Each debug/release profile matches:

- 483 independently initialized runs from 161 selected seeds, covering 5,934
  original world/wrapper calls and 5,451 carried updates. Outputs contain
  1,881 bodies and 4,311 retained initial contact records; 26,549,532 bytes match.
- Runs include lengths 1, 2, 3, 4, 7, 8, 15, 16, 31, 32, 63 and 64. All 64
  shape-filter controls and the 513-shape table remain covered. Complete body,
  contact, row, property, workspace, cache, list and lifecycle bytes match.
- Nineteen independent checks and 30 paired framing/guard outcomes. Dirty
  membership, consumed contact slots and row storage match each seed's first
  completed step. Selectors alternate correctly. History is checked against
  368 available initial/consecutive predecessor outputs; other intermediate
  history pairs were not captured separately.
- Fresh 66 whole car-manager updates produce 176 contacts. Each branch's own
  full owner/pool then feeds 198 physical runs, 2,340 world calls and 2,142 carried
  updates, matching 936,408 bytes. Twenty-four checks include 152 available
  initial/consecutive history pairs and complete owner provenance. The 528
  contact records in output reflect three runs per seed, not 528 unique contacts.

The physical stage still uses a separately authored timestep/material and cold
configuration before manager production. There is no new contact generation
after the first step and no repeated car-manager/world loop in this checkpoint.
Geometry, registered pose cursor, sleeping, joints, deferred publication,
callbacks and asynchronous modes remain outside this selected contract.

Fresh cold whole-world and shaped frame/tick regressions each pass 465 scenarios
in both profiles. Two new unit tests reject dirty cycles, foreign active links
and an unconsumed pool, and preserve complete carried body/world/pool/manager
state on publication failure. All 160 library tests, formatting and
warnings-denied all-target Clippy pass. Ten-report current-executable/upstream
audit verifies 156 independent checks, 208 guard outcomes and three Python ASTs.

The strict native build caught a shadowed local counter before execution; renamed
it. Initial small/full debug and fresh handoff drafts passed. No native fault or
accepted numerical mismatch occurred. Cargo briefly waited on its build lock
when independent verification overlapped; all builds completed.

## Evidence and tools

Private final reports:
`local/behavior/carried-physical-world-{debug,release}-20261011-final-01`,
`wheel-carried-world-{debug,release}-20261011-final-01`,
`whole-physical-world-{debug,release}-20261011-f198-regression-01`,
`shaped-physical-{frame,tick}-{debug,release}-20261011-f198-regression-01` and
`carried-physical-world-final-audit-20261011.json`.

Build `tools/build_carried_physical_world_probe.cmd`; run the shaped and wheel
handoff validators with `--carried`. Native SHA256
`8b8618e34a8d8b0b82b995a458885b609751f626d7ce233e567a282c9784888d`;
Rust physical diagnostic debug
`948410eae19ce429386fe2e202071c95d8e2d31d214061646c43a864f10d9279`, release
`dba526f054d6b49ab1ead8165afc1f9e9767ac13824d2afb0aa6c98867f2e42f`.
Installed versions reused: Python3.12.14, Rust/Cargo1.99.0,
MSVC14.44.35207/SDK10.0.26100.0 strict x86/SSE2. Static evidence and earlier
Ghidra/Capstone versions are indexed in the preceding world document.
Steam's 130 reference resources and executable remain unchanged at
16:03:50.382206 UTC, `local/scene/preservation-20261011-carried-physical-world.json`.
No desktop input/settings changes or installations. Overnight automation remains
paused. No owner help is required to continue repeated production and vehicle
integration; the first drivable Rust lap remains unimplemented.
