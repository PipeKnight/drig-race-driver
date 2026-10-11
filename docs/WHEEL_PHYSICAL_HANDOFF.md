# Generated wheel contacts through complete physical solving

F196, 11 October 2026. Fresh complete car-manager output now feeds original-style
contact grouping, row construction, serial solving and pose publication. Both
debug/release and both variants, with and without preceding body advance, agree
exactly with the original executable. This closes the previous mismatch between
the wheel producer's records and the ordinary-impact-only solver adapter.

## Classification recovered from the original

`AD6250`, `B82850` and `B7B850` explicitly treat C=0 **with a null second body**
as single-body. C does not need to become one. The local second-body index is
MAX; the original physical owner remains null. Generated shape identities at
28/2C, shared material identity and special flag73 remain present.

After assigning local indices, `B24C90` calls whole `B0CCC0` for impact flagF=1.
E becomes zero, then becomes one if73 is zero, either shape217 is on, or one
shape229 and the other230 are both on. Selected car229-on/static230-off wheel
records therefore retain E=0. E controls downstream correction behavior; it does
not bypass initial row construction. Special73 also selects the original
material-capped bias branch. Row construction retains the original reversed
2C/28 order for angular-permission216 lookup. No contact flags are cleared to
make the records fit an earlier contract.

`contact_groups::Shape` owns the consumed216/217/229/230 flags. New
`validate_with_shapes`/`calculate_with_shapes` and grouped frame/tick variants
accept bounded one-based shape identities. The old empty-shape APIs retain
their ordinary-contact scope. Full grouping, shared manager arrays/cache,
global/local identities and complete body/contact storage remain intact.

## Differential and independent checks

Each variant/profile compares 465 scenarios, 2,012 bodies, 1,357 groups and
26,047,200 observable bytes. There are 6,235 contacts: 1,526 C0/null-second and
4,218 special contacts, producing 6,235 normal and 8,388 friction rows. Sixty-four
controls cover every combination of both shapes'217/229/230 bits; angular flags,
ordinary/special records and the maximum513-shape table are also exercised.
Ten independent checks and 24 paired malformed outcomes pass per variant/profile.
Body velocity changes1,978 times; pose changes1,192 without advance and all2,012
with advance. Complete original/repeat/Rust outputs are equal, including unused
rows and retained bytes.

The fresh producer handoff takes 66 independently carried manager input records
from F195. It authors valid cold physical configuration and zeroed contact
templates **before** rerunning the whole manager. This provides the previously
absent center basis, sleep/limit settings and pool initialization. Freed capacity
produces176 contacts rather than the155 from F195's mixed-capacity fixtures.
Each branch uses its own fresh manager output. Complete physical owner bytes and
generated geometry/flags/material/shape identities feed the physical stage;
only owned pointer/index identities are normalized. No velocity, mass, basis or
contact geometry is copied from another branch or substituted after production.

All176 generated records classify E=0 and retain73=1/C=0/nullsecond. Each
variant/profile compares66 manager calls,66 physical cases and310,288 physical
output bytes exactly. All66 body velocities and poses change, producing176
normal and198 friction rows. Fifteen independent checks include complete owner
handoff, identity/flag retention and generated-contact activity.

The physical stage supplies a shared authored material and timestep0.016. The
selected producer records include65 zero-driver calls and one positive-driver
call. These are linked verified stages, not a claim to have executed one whole
original frame with a common timestep. Source material initialization, positive
carried-frame cadence and persistent pools remain open.

## Native ownership, regression and failures

The native adapter owns up to513 shapes and calls whole unchanged
`AD6C40 → B77310 → B7B850`, optionally whole `AD7040` first. It validates flags,
identities and null-second framing before native access, checks all shape bytes
and actual retained body/shape/material pointers before normalization, and
retains the previous world/producer/manager/backlink/global checks. Original
code remains RX/unchanged; no imports or original helper replacements execute.
Five original lazy-data pages restore their bytes/protections on exit.

Fresh ordinary grouped frame/tick regression matches400cases/1,817bodies/
1,162groups/22,601,160bytes and28guard outcomes each variant/profile. Fresh
isolated grouper regression matches628cases and both selectors/arbitrary heads
per profile. All158 library tests, formatting and warnings-denied all-target
Clippy pass. The existing late-group rollback unit now includes a generated-style
special/null-second contact: the earlier group constructs rows successfully,
then a later pose failure preserves the whole input and shared manager for both
frame and tick APIs.

The first CLI build exposed swapped manager/shape arguments; fixed before
execution. First producer handoff completed natively but Rust safely rejected
nonfinite unused row lanes in random-byte templates. Finite row storage is now
initialized before the physical stage, respecting the established Rust contract.
No native fault or accepted numerical mismatch occurred. Failed drafts remain
private. Several guessed file reads failed harmlessly and were replaced by
inventory searches.

## Evidence and next step

Private final reports:
`local/behavior/shaped-physical-{frame,tick}-{debug,release}-20261011-final-01`,
`wheel-physical-{frame,tick}-{debug,release}-20261011-final-01`, their producer
requests/outputs, `grouped-physical-{frame,tick}-{debug,release}-20261011-f196-regression-01`
and `contact-groups-{debug,release}-20261011-f196-regression-01`.
`wheel-physical-final-audit-20261011.json` verifies14 reports/850 conditions,
current binaries, input provenance and two Python ASTs.

Native shaped frame SHA256 `b3ed46c6a709db9739cb84ab333c608c2c7e222f961ca5fd4a8f9d6861377a35`;
tick `c0eb3e1c9ca8c6147a58c12a33d20f712194978ce02b068bd36f3a3656f386a1`.
Rust physical diagnostic debug `fc11d235fe0f6d763beabdf71af7b170e0d73acad4c4859a03826c037c8cbbe2`;
release `5a08813f0f5d380f242631179172ecc0afb2fa89fc5e621f6682149d5a74ea45`.

Original classification evidence is private under `20261011T0924444079078Z`,
`20261011T0925239339348Z`, `20261011T0926582620606Z` and the previously verified
whole solver survey. Reused Python3.12.14, Rust/Cargo1.99.0,
MSVC14.44.35207/SDK10.0.26100.0 strictx86/SSE2 and Ghidra12.1.4/JDK21.0.9;
no installation. Steam130resources and executable unchanged15:35:46.975564UTC,
`local/scene/preservation-20261011-wheel-physical-handoff.json`. No desktop input
or settings changes. Expired overnight automation stays paused.

Next establish original pool/group lifecycle and carry physical results back
into subsequent car jobs without pretending that clearing cold ownership fields
reproduces the world reset. Then complete source initialization and connect the
running simulation to Bevy. No drivable Rust lap exists yet; no owner help is
required for the next offline step.
