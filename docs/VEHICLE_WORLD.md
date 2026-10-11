# Repeated vehicle manager and physical world

11 October 2026, F200. A complete selected car update now feeds wheel contacts
into the retained physical world, and the next car update reads the resulting
physical state. The original engine, an independent original repeat and Rust
match over 300 runs / 6,150 combined intervals / 42,636,000 observable bytes
**per debug and release profile**. This is the first verified repeated enclosing
vehicle/physical composition. It is not yet a drivable Bevy lap.

## Execution and ownership

The original harness calls unchanged `A02B70` (including original task dispatch,
car job, completion events and wait), then whole serial `AD8D70`, then actual
`A727A0` pool consumption. The same positive f32 timestep reaches both the car
and physical world. Car gravity uses the existing reference bits `411CF5C3`;
world gravity uses their negative Y equivalent. Native preflight copies are
restored before the result-producing sequence, as in the earlier car harness.

Complete car `21000`, physical body `424`, four reusable contact slots, group
links, active/dirty lists, shared material cache, properties, workspace and 16
solver rows survive between intervals. Contacts come from the actual car's
overtravel decision and the installed Donington geometry query. Counts and
contact points are no longer externally supplied per interval. No pool rearm,
cold ownership reset, pose replacement or artificial velocity feedback occurs
between calls. Native pointer identities are checked and normalized only at
the private final output boundary. Rust retains center identity `1C0` internally;
the old standalone job API continues to require its unlinked input contract.

The native advance stage `AD7040` rebuilds the owned world `8` update-list head
from active bodies. In this single-body scope the head is the physical body,
with body `318` null. An added assertion initially assumed this list stayed
empty and rejected an otherwise exact run; it now checks the actual owned head
and unchanged list/world binding. This advance list is separate from the
registered pose queue at world `24`/`28`, which is empty in this scope. The
earlier description as a notification pose list was incorrect. General
registered pose traversal and periodic normalization remain excluded.

Native audits also retain the complete car shape, wrapper and shared material;
check all source shapes still have kind11, null physical owner/callback and the
same material identity; verify worker/manager/body/update-list pointers; reject
foreign contact-array pointers; preserve manager configuration and unused
manager storage; retain all executable sections and restore protected data/IAT
state and completion events on return or failure.

## Comparison coverage

Twenty-five starting fixtures are selected from previously verified positive
car-manager inputs, including the known positive producing owner. Original,
repeat and Rust source outputs are checked before selection; their file hashes
are recorded. These are authored diagnostic configurations, not 25 live captures.
Each runs for 1, 2, 3, 4, 7, 8, 15, 16, 31, 32, 63 and 64 intervals. Timesteps
include approximately 0.000001 through 0.1 seconds; this tests scoped behavior,
not the live game's frame/substep selection.

Per profile:

- 6,150 actual enclosing manager/world intervals, 661 contacts appended,
  637 appended after the first interval, and 5,742 empty-contact intervals.
- 175 independently inspected consecutive-history pairs. The next car's
  pre-force velocity history equals the previous world output, including the
  established tiny-velocity cleanup. A naive direct position-equality check was
  rejected: suspension finalization also advances the temporary car pose.
- All 300 final physical poses, velocities and manager state differ from their
  initial input. Two final wheel-impact values exceed the former diagnostic
  one-million bound.
- 16 independent checks, including consumed pool, stored delta/idle lifecycle,
  selector/active-list parity, car history, unchanged query/configuration data,
  physical ownership, valid source/contact tokens and feedback.
- 44 paired guard outcomes cover malformed framing, step counts, common-delta
  mismatch, nonfinite operands, foreign/cache identities, row ends, unfinished
  world state, occupied pool, physical links and deferred publication. Rejected
  calls leave no output artifact.

All complete car and physical/pool/world wire data is compared. Expired native
temporary view/solver storage is excluded consistently with the earlier whole
car-manager boundary. Current outputs, original repeats and all other observable
bytes must match exactly; there is no numerical tolerance.

## Failures and repairs

The first Rust debug call overflowed the default Windows stack: composition
retains several complete frames on the stack. The diagnostic now runs on an
explicit bounded 16 MiB worker stack. Native first-step original/repeat outputs
had already completed exactly; no successful Rust comparison was claimed then.

The first wider run rejected a later fixture at step2 with the older generic
handling collision error. More specific guards established the cause: a finite
impact `2,657,254.75` exceeded an arbitrary diagnostic operand bound, rather than
the original damage threshold. Clamped handling delta is at most .02, so a
finite f32 impact has a finite product. Rust/native validation now accepts that
finite operand while retaining the original threshold rejection. A unit checks
the large accepted impact, the active-damage rejection and nonfinite rejection.
A temporary authored damage-threshold increase did not resolve the issue and
was removed. Final fixtures retain their earlier threshold10000.

The first expanded guard run caught Rust accepting initial row end17 where the
native harness required <=16; wire validation now agrees. An independent check
incorrectly assumed contact activation byte70 stayed zero; solving legitimately
writes boolean0/1, while bytes71..73 retain0/1/1. The check was corrected without
changing any numerical output. Native final02's added ownership audit exposed
the advance update-list head described above. No F200 native access violation or
accepted original/Rust numerical mismatch occurred.

An input inventory initially included other source-initialization wire types;
exact TF02 tag/length filtering fixed it before execution. Rust test float syntax
and a Clippy precision warning were fixed before final checks. Failed drafts,
guard failures and intermediate reports remain private. Earlier commentary/log
wording that placed the handling failure in the first16 cases was incorrect:
the recorded failing batch starts at combined-128; preceding batches passed.

## Reproduction and evidence

Authored tools: `probe_vehicle_world.c`, `vehicle_world_record.h`,
`vehicle_world_probe.h`, `build_vehicle_world_probe.cmd`,
`validate_vehicle_world.py`, `audit_vehicle_world.py`. The private TF03 wire adds
3500 bytes to TF02, with bounded 1..64 steps and owned world/solver configuration.
Input/output remains under ignored `local/`.

Final evidence:

- `local/behavior/vehicle-world-{debug,release}-20261011-final-03/`
- `local/behavior/wheel-car-manager-{debug,release}-20261011-f200-regression-01/`
- `local/behavior/reproduced-physical-world-{debug,release}-20261011-f200-regression-01/`
- `local/behavior/vehicle-world-final-audit-20261011.json`: four reports,
  32 checks, 136 guards, both Python ASTs and current executable/upstream hashes.
- `local/behavior/vehicle-world-physical-regression-audit-20261011.json`: two
  reports, 30 checks, 68 guards, four ASTs and current provenance.
- `local/scene/preservation-20261011-vehicle-world.json`: all130 traced Steam
  resources and original executable unchanged at16:55:37.900482UTC.

Fresh complete car-manager regressions match428 updates/24guards/profile;
fresh repeated producer/world regressions match198 runs/2,772worldsteps/profile.
Workspace library tests pass161 total (138world/22formats/1tools); formatting
and warnings-denied all-target Clippy pass. Existing installed tools reused:
Rust/Cargo1.99.0, Python3.12.14, MSVC14.44.35207, DevPrompt17.14.41,
SDK10.0.26100.0. Bevy0.19.1 remains the viewer dependency.

Current SHA256:

| Artifact | SHA256 |
| --- | --- |
| Native combined probe | `e119ad6367f16284d76134cc6227463f8723d4335c609398539f23b182c2be96` |
| Rust debug vehicle diagnostic | `513329119427be311b0399601dd6fcfe1aa6d0368725f485e8217524792aa24f` |
| Rust release vehicle diagnostic | `8c3f77ace1180cd1009ca3ae3a9a66f3963d9ef0b1a2261a979b0410ec6ff242` |
| Native manager regression | `31a393feeff226cc8089a4c541e940db38b1ed4fc3e19b9130b437d162356bcd` |
| Analysis executable | `aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514` |

Other input/probe hashes are stored in the private reports. The original Steam
executable remains `dd7b0933abf0d421bec3fa6f123319d3d3cc7221e5ef82b7a1201c3d1979e48f`.

## Remaining work

Physical world material/correction settings, row templates and cold ownership
are authored before the first manager call. Car/static shape metadata is the
previously selected recorded scope; general body broadphase, chassis/barrier
collisions, deformation/damage notifications and live asynchronous final-step
ordering remain outside it. Controls stay fixed during each diagnostic run.

Next: source-initialize a complete Lacetti owner, establish the resource-backed
physical material and spawn/world configuration, then connect this retained
update to Bevy fixed-time input, car pose, chase camera and lap gates. Keep the
live frame/substep and registered pose-queue differences explicit. No owner
action is needed for these offline steps. Completion estimates remain provisional
and unchanged; successful function comparisons are not a percentage of a game.
