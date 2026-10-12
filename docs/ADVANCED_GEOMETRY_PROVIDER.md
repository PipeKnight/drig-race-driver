# Geometry generation after physical pose advance (F235)

`physical_world::advance_with_geometry_provider` now exposes the missing timing
boundary for fresh body contacts. It advances and publishes retained bodies,
then invokes a provider with read-only advanced bodies and the remaining stable
contact capacity. Returned world-space contacts follow the supplied relative
wheel/external prefix, before grouping and solving. Unused slots retain their
bytes; the transient pool is consumed at completion. Existing APIs keep their
previous behavior through the same internal path.

The provider cannot mutate the authoritative body/pool inputs or the external
prefix. Excess returned contacts, invalid contacts, provider errors and later
solver failures return an error without publishing a new world. Mutable state
outside the provider's return value remains the caller's responsibility.

## Timing and ownership checks

A meaningful unit sets body X=2 and velocity X=5 at delta=0.01. The provider
observes advanced center and published X=2.05, with history X=2.0. It verifies
the remaining capacity, relative prefix translation, unchanged geometric world
point, inactive slot preservation and consumed pool. Provider error, capacity
overflow and late NaN rejection preserve the prior bodies, links, pool and
manager rows/workspace/cache. Existing late-publication rollback tests also pass.

The optional private diagnostic `grid-physical-tick INPUT local/OUTPUT
--geometry-provider` accepts GTG2 only. Its first cold interval retains the
previous path; subsequent intervals submit the selected world templates through
the new post-advance provider. This verifies timing/ownership composition with
the original staged solver. It does **not** perform fresh geometry detection.

## Original and regression proof

All 608 source-positive F229 placements, two reconstruction schedules and
1/15/16 intervals reproduce the original AD7040/AD6C40/B77310 outputs:
**3,648 cases / 38,912 intervals / 51,932,160 bytes per run**. Current Rust debug
and release each run both legacy and provider modes; the provider is called
35,264 times per profile. The original native stages are freshly rerun once
per batch and match the pinned original and repeat completely.

Per provider profile, semantic checks preserve 16,248 world points, 2,432
relative-point lanes, 1,948 inactive slots and 2,432 predecessor histories.
The source/current-hash audit checks 217 complete primary files and 40 malformed
or existing-output guard records. Eleven regression wires across ten original
native variants reproduce **6,122 earlier cases / 207,840,804 bytes per run**
against both current Rust profiles. Their 33 complete output files bring the
audit total to **250**.

All 183 regular workspace library tests, formatting, warnings-denied all-target
Clippy, Python AST and whitespace checks pass. Both normally ignored purchased
source driving tests were explicitly run in release and pass, including reset,
controls, failure rollback, handbrake and reverse. The playable executable has
not been replaced and still has wheel-only contacts.

Private records:

- `local/behavior/geometry-provider-20261012-final-01/validation.json`, SHA-256
  `fbeb7a2e3a70a6ad6f0fdff4452f2dead6863053818a0ad9df77cd847a6757c5`.
- `local/behavior/geometry-provider-regression-20261012-final-01/validation.json`,
  SHA-256 `4c7a2dd59238c2844cc04ab5d015cd94367a14b1114d7e25dacc3980c42bc80e`.
- `local/behavior/geometry-provider-audit-20261012-final-01.json`.
- Rust physical diagnostic debug SHA-256
  `5ce997d2b822e40bc0a4db8a12d7d0a964f5b988a4b90c0d6249f89eb1de1586`;
  release `61beb3fd00d98b7350b20d9e1ae7cdfcd1a2899a71d8ba82375d4e7c9e7247cb`.
- Matching build/library/Clippy/source-driving logs and
  `local/scene/preservation-20261012-geometry-provider.json`.

Installed Rust/Cargo 1.99.0, Python 3.12.14, MSVC 14.44.35207 /
DevPrompt 17.14.41 / SDK 10.0.26100.0 were reused. All 130 tracked original
Steam resources and the executable match their baseline at
2026-10-12 19:19:05.198286 UTC. No new failure, installation, desktop input,
settings change or current approval blocker occurred.

Next attach the source moving-body mesh and compose this provider with
[fresh packed contacts](FRESH_PACKED_CONTACTS.md). The vehicle wrapper still
owns a four-slot wheel pool, so runtime integration also requires a stable
larger pool, correct moving-shape offset/pose, source track identities and
validated solver capacity. Original broadphase scheduling, full material
factory, populated backface correction extents and barrier driving remain open.
