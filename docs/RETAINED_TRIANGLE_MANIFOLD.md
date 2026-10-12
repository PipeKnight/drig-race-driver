# Retained triangle manifold append

Follow-up: [F232](ORIENTED_TRIANGLE_MANIFOLD.md) composes and validates original
witness reorientation with this retained manifold path.

F231, 12 October 2026. Retained manifold append is verified at the selected
source boundary. This is not a runtime barrier implementation. Builds reuse Python 3.12.14, Rust/Cargo 1.99.0,
MSVC 14.44.35207 / Developer Prompt 17.14.41 / SDK 10.0.26100.0, with strict
SSE2 floating-point native compilation. No tools installed or desktop used.

## Boundary and observations

The original whole AE2FA0 manifold entry accepts a write cursor, remaining
capacity and arena head. Its selected triangle/triangle feature handler AEB630
scans prior contacts as well as new ones for duplicates. A merge updates the
existing penetration without republishing its point, normal or flags. After
clipping, AE2FA0 decides whether to add its fallback using the **total** retained
count, not just the number added by this call. The unoriented direct-fallback
branch does not apply that clipping count threshold. A full arena returns
without feature/cache mutation from AE2FA0.

Private original surveys used:

- `local/behavior/20261012T1126269008807Z/ae2fa0.c.txt`
- `local/behavior/20261012T1425086290090Z/aeb630.c.txt`

Rust `contact_manifold::calculate_append` stages its context, clones existing
contacts, admits a bounded total of sixteen, and returns the complete manifold.
`Settings.capacity` is **remaining** capacity. The cold `calculate` entry
delegates with an empty manifold. Publication metadata remains caller-owned;
retained duplicate merges only change penetration. Two new unit tests cover
total-count fallback, the unoriented branch, duplicate depth updates, full-arena
preservation, and failure after staged feature support with no publication.

## Diagnostic composition

`tools/probe_manifold_sequence.c` uses the existing owned source geometry harness
with a new PC09 wire. Up to 64 steps carry a triangle index, normal-reset mode,
normal, fallback witness/depth and publication flag. The 48-byte request records
follow the count at offset 40. Seed/chunk bytes follow all records. Capacity is
the total arena capacity. Arena contacts and the seven-entry support cache
persist throughout a sequence; there is no implicit clear between triangles.

Each interval captures all sixteen 128-byte contacts, the A0 query record and
both C0 diagnostic features: A20 bytes, preceded by a sequence count. Query +4
is total used count; +8 is the diagnostic newly appended count. The native
probe also retains its complete final owned arena output. Existing PC01-PC08
formats remain separate. Native feature inspection saves/restores the support
cache before calling unchanged AE2FA0, as in F226. Diagnostic feature fill is
explicitly reset each step. Original code is not replaced; five original data
pages are restored and every mapped original section is compared afterward.
Contact/query/feature/sequence and other existing owned canaries are checked;
the source mesh/chunk/body are checked for unintended changes.

The Rust CLI rebuilds the moving mesh and packed triangles from source inputs,
retains contacts/cache and publication flags, and serializes the same interval
records. Neither implementation consumes the other's output as input.

Normals, triangle order, depth, fallback witnesses and flags are **authored
boundary controls**. The sequence does not implement packed traversal, hit
reorientation, actual moving-frame detection, source material factory identity,
or runtime collisions. For the related retained physical stages see
[F230](CARRIED_GEOMETRIC_PHYSICS.md).

## Work and evidence

First smoke: `local/behavior/manifold-sequence-debug-20261012-smoke-01/validation.json`.
It passed original/repeat/Rust debug across 56 cases / 3,136 intervals, with
48 depth merges, 6,368 retained-record checks, 2,045 full-arena intervals and
19 guards. This is a smoke result, not the full source sweep.

Relevant commands are
`tools/build_manifold_sequence_probe.cmd`, `tools/validate_manifold_sequence.py`,
`tools/replay_manifold_sequence_regression.py`, and
`tools/audit_manifold_sequence.py`. Final evidence remains under ignored
`local/behavior/manifold-sequence-*`.

## Final verification

All 245 source chunks at five authored triangle-derived placements supply
1,225 eight-step sequences. Another 1,152 sequences cover eight supplied normals,
six capacities (0/1/2/3/4/16), and 16/64 steps across 24 spaced source placements.
Sequences include repeated and successive triangle indices, deeper and shallower
fallback controls, alternating publication flags, and normal-reset/retain modes.
The selected body transform is fixed within each sequence; these are not moving
vehicle trajectories or newly detected collisions.

Original, repeated original and independently rebuilt Rust match exactly for
each actual debug/release profile:

| Check | Per profile |
| --- | ---: |
| Sequences | 2,377 |
| Retained calls | 55,880 |
| Final published contacts | 6,803 |
| Existing-contact depth increases | 1,575 |
| Retained metadata/point/normal preservation checks | 116,821 |
| Full-arena intervals | 30,191 |
| Complete native bytes per original/repeat run | 266,001,404 |
| Malformed/existing-output guards | 24 |

A separate 288-sequence sweep actually changes normals each call, cycling
all eight normals or reversing the same normal, with capacities 3/16 and
8/16/64 steps. Original/repeat/both current Rust builds match all 8,448 calls /
36,577,152 native bytes per run, including 12 depth increases and 41,841 retained
record checks. This complements the reset/retain tests; a different unused
supplied normal alone does not establish a change in the active normal.

All 2,425 preceding cold manifold cases reproduce in the rebuilt native and both
current Rust profiles. Twenty spaced cases from each of five earlier distance,
outer-query and legacy packed boundaries also reproduce: 2,525 complete native
outputs and 2,505 Rust outputs per profile. This is full cold-manifold coverage
and sampled earlier-boundary coverage, not a new full sweep of every preceding
query or PC08. Six existing shared variants rebuild with warnings as errors.

The independent audit rebuilds requests from pinned source inputs, checks the
purchased track/archive provenance, separately decodes every primary selected
triangle, checks current binary/script hashes and every output file, compares
profiles, and rechecks all rejection/existing-output evidence. It passes
14,262 primary + 1,152 changing-normal + 7,535 regression complete files:
22,949 complete files and 48 primary guards. All 177 regular workspace library
tests pass (two source tests remain ignored by default), as do fmt, all-target
warnings-denied Clippy, four Python AST checks and Git whitespace checks. The
three manifold tests pass explicitly. Source driving tests were already run
explicitly in F230; they were not repeated here. The playable executable was
not rebuilt or replaced by this diagnostic work.

## Failures and private evidence

The first complete debug/release runs passed valid cases and nineteen guards.
A subsequent targeted malformed control exposed a diagnostic admission gap:
on a mode-zero step Rust ignored an invalid supplied zero normal, while the
native harness rejected it. Evidence:
`local/behavior/manifold-sequence-late-normal-20261012-draft-01/finding.json`
(native exit 8/no output; Rust exit 0). Rust now validates supplied manifold
normals before considering reset mode. Five late-control guards were added.
Both actual builds, full sweeps and regressions were rerun in `final-02` and
supersede `final-01` as the current-binary proof. No valid-case numerical
mismatch or original execution fault occurred.

A guessed private survey timestamp, doc name and module name failed harmlessly
during research; inventories supplied the correct paths before use. An atomic
documentation patch had an invalid extra context and was reapplied without it.
No Steam/settings mutation, new installation or approval rejection occurred.

Final private evidence:

- `local/behavior/manifold-sequence-debug-20261012-final-02/validation.json`,
  SHA-256 `8df4ef230614f1083d9eeae4e9aa8bdeb43b54c6a0cca55de0cc4e4738715741`
- `local/behavior/manifold-sequence-release-20261012-final-02/validation.json`,
  SHA-256 `c6fa8f1d8071e2e12e1f2ed10046598543cdff85fb7260a5195c80a21e3f33ee`
- `local/behavior/manifold-sequence-regression-20261012-final-02/validation.json`
- `local/behavior/manifold-turns-20261012-final-01/validation.json`
- `local/behavior/manifold-sequence-audit-20261012-final-01.json`
- Native PC09 SHA-256 `4750857450ded6057c36f77d2e48ef569bda66d5b2d691db52eacf97933780ad`
- Rust debug SHA-256 `994e52d0b10abb2251f780fac7c786b98cba09da5225c69fee0ff48341450fda`
- Rust release SHA-256 `ecd0710846bf282ee77c3cea72264111a1975e8d540c7e6f2823843fca9b1c84`
- Matching `manifold-sequence-*` build/test/Clippy text logs and
  `local/scene/preservation-20261012-retained-manifold.json`.

All 130 tracked Steam resources and the reference executable are unchanged at
`2026-10-12T18:23:19.036216+00:00`.

Next: [witness/normal reorientation](CONTACT_REORIENTATION_RESEARCH.md), then
fresh moving-body/track queries before attaching geometry to the driving
runtime. No owner action or additional recording is currently required.
