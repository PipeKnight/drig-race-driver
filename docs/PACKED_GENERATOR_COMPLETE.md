# Nonaliased packed generator and physical handoff (F229)

The whole original B39500 -> AEA1F0 packed contact generator now has a separate 0x2C0 packed-shape allocation and explicit sheet mask. **2,642 original/repeat cases match all 131,030,960 output bytes per run**, producing 5,039 contacts across source-policy and authored controls. Changing source flags to all-sheet changes complete contact records in 277 placements. This demonstrates why the old short-descriptor alias could not establish source-correct normal selection.

The source-policy cases alone produce 3,134 contacts, versus 2,393 in the historical aliased fixture, with contact records differing in 278 source placements. Both have 608 positive placements; changed manifold records/counts must not be mistaken for newly detected collisions. The historical fixture remains reproducible, while these results replace it as the selected nonaliased source-policy generator reference.

The 608 contact-producing source-policy placements also pass through the existing original and Rust physical solver in debug and release: **1,824 cases, 6,268 contact uses and 25,564,928 complete bytes per profile**. Every enabled placement changes body response relative to its empty-contact control. These are composed cold physical-frame checks, not runtime barriers or a Rust contact generator. The playable build still uses wheel contacts only.

## Corrected fixture and limits

PC08 requests explicitly append a sheet mask after the existing placement fields. `tools/probe_packed_mesh_complete.c` selects the full allocation without changing historical PC01 or direct PC03..PC07 layouts. The generic owned-shape helper defaults to its previous 0x280 allocation unless the new variant explicitly requests 0x2C0. Source header storage follows the complete allocation, so shape+0x290 no longer aliases the header's bounds.

The fixture supplies the verified mask and four maximum-float words at 0x294..0x2A0. It appends the complete additional 0x40 descriptor bytes to the previous output format, allowing independent checks of mask, initialized and untouched tail bytes. Original code, canaries, image restoration, body/mesh/source immutability and CREATE_NEW guards remain enforced.

This is still an **explicit geometry fixture**: it supplies the other legacy base settings, cold worker, zero material coefficients, identity static transform, one moving Lacetti mesh and selected source-placement centers. It does not execute the complete resource factory or substitute all constructor outputs. In particular, constructor-derived bounds are not newly installed here. The separately verified [whole constructor](PACKED_SHAPE_CONSTRUCTOR.md) establishes mask ownership; this fixture establishes the generator's behavior with explicitly owned mask storage. No full production material-table or live allocation identity is claimed.

The source masks use the exact-key surface-table/first-record-fallback policy from F228, including its 18 MUD-NUL versus MUD-space occurrences and all-zero selected masks. Other material coefficients remain authored zeros at the generator boundary. This does not establish how a complete merged original material manager changes dispatch paths.

Each of the 1,225 original placement cases runs with source mask and all-sheet control. The 192 capacity controls run with source mask, for 2,642 cases. The validator reconstructs original Lacetti seed, Donington chunk, triangle-derived center and all capacity fields from purchased source plus hashed prior evidence before forming requests. The final audit rechecks every input, both complete outputs and source-dependent policy. Nine malformed/existing-output guards pass.

## Physical integration evidence

`tools/validate_mesh_contact_handoff.py --packed-track --packed-complete` selects only the source-mask records from the corrected fixture, checking its hashed Lacetti provenance. All 608 positive source placements are covered once per Rust profile in two 304-placement batches, respecting the existing 1,024-case diagnostic bound. Each produces enabled, correction-disabled and empty cases. Shape/material identities are explicitly converted to the existing owned solver tokens; body links are cold-normalized and finite velocities/material/solver settings are authored.

Original/repeat/Rust comparisons include the complete body/contact/group/solver outputs, with world-space contact points and normals preserved. Correction-disabled cases retain the main response and set the expected flag. All 608 enabled cases per profile differ from empty body-response controls. The audit rebuilds the complete handoff requests and verifies all 12 output files, 1,216 source-placement uses and 96 guards across the four runs. This proves the corrected supplied geometry can drive the physical solver; it does not prove fresh detection, persistent contact lifecycle, mixed wheel/geometry frames or complete AD8D70 scheduling.

Existing debug/release Rust solver executables were reused unchanged, with SHA-256 `f21fe54c11b8934b6948e4afdc5fa9970e14028583badf5df74d80e9af85d3e6` and `cfcc01fe37189de3c30add9209260e2aa2fde5c433dea2d36518b3fbceee5a30`. They were executed for both actual profiles; no new Rust source or numerical behavior was introduced at this checkpoint.

## Regression checks, failures and reproduction

Six shared probe variants rebuild with installed MSVC 14.44.35207 / DevPrompt 17.14.41 / SDK 10.0.26100.0, C17, strict floating point, SSE2 and warnings as errors. `tools/replay_packed_mesh_complete_regression.py` reproduces **all 1,417 historical packed outputs** plus ten spaced cases for each of the five direct distance/query/manifold variants: 1,467 complete outputs / 72,012,088 bytes and 142 guards. This is full historical packed coverage and sampled direct-boundary coverage, not a new full direct-query sweep or new Rust regression claim. The independent audit checks every retained output and current source/native/reference hash.

The first validator attempt used a guessed prior-report name lacking `lacetti`, failing before original execution. The path was corrected from the actual earlier validator. The ten-case smoke then passed, followed by the complete sweep and audit. A few guessed filename reads failed harmlessly; file inventory corrected them. A shell-quoted one-line statistics command failed Python parsing without writing evidence; the comparison was moved into the auditable validator and passed. There was no original execution fault, numerical mismatch, approval rejection or new tool installation. Python 3.12.14 was reused. All new/changed Python ASTs and Git whitespace checks pass.

Build using `tools/build_packed_mesh_complete_probe.cmd` and `tools/build_packed_mesh_complete_regression.cmd`. Run `tools/validate_packed_mesh_complete.py`, then the regression replay in a distinct ignored directory; audit with `tools/audit_packed_mesh_complete.py --regression`. Run the existing handoff validator with the new flag, each actual `--profile`, `--limit 304` and offsets 0/304; audit with `tools/audit_mesh_contact_handoff.py`. All outputs remain private under ignored `local/`.

Private final evidence:

- `local/behavior/packed-complete-20261012-final-01/validation.json`, SHA-256 `4e7ee1b733285fc68726c9f5f15d148c33cd92e2c19c87e893299cc9c812af34`
- `local/behavior/packed-complete-audit-20261012-final-03.json`: 5,284 complete outputs / 9 guards plus 1,467 regression outputs / 142 guards, including the old/new contact comparison
- `local/behavior/packed-complete-regression-20261012-final-01/validation.json`
- `local/behavior/packed-complete-handoff-{debug,release}-20261012-final-01-{a,b}/validation.json`
- `local/behavior/packed-complete-handoff-audit-20261012-final-01.json`
- Native PC08 SHA-256 `551ac0bd8201b63274e4bc51bbe07f9574e99efb1011ef72c596e2bf71ee7986`
- Build/validation text logs with the matching `packed-complete` prefix.

Steam's 130 reference resources and executable remain unchanged at `2026-10-12T16:12:14.857904+00:00`, recorded in `local/scene/preservation-20261012-packed-complete.json`. No desktop input or settings changes were used.

Next: Rust query-witness/normal reorientation and multi-triangle manifold append, followed by fresh moving body/track queries and carried physical geometry. The existing carried group path still treats every contact point as relative; it must preserve the external-wheel prefix versus world-space geometric suffix before runtime integration. A drivable owner lap and current handbraking are confirmed separately in [the playtest record](PLAYTEST_2026_10_12.md).
