# Retained physical frames with geometric contacts (F230)

Rust now preserves the external relative-point prefix versus world-space geometric suffix through retained physical frames. `physical_world::advance_with_geometry` retains bodies, active/dirty/group ownership, complete solver workspace/cache and unused contact slots. The existing pool API delegates with all active contacts external, preserving its behavior. This removes a concrete integration error: translating a world-space barrier point by the car's center again.

The original staged body-advance/group/solve boundary and Rust debug/release match **3,648 cases / 38,912 intervals / 51,932,160 complete bytes per profile**. Cases cover all 608 contact-producing source-policy placements from [F229](PACKED_GENERATOR_COMPLETE.md), pure geometric and changing mixed schedules, and 1/15/16 intervals. This is supplied-contact integration, not fresh geometric detection or playable barriers.

## Owned coordinate and lifetime contract

Each call supplies a stable-capacity slot array, active count and external-prefix count. Prefix points are relative to body one and are translated by its advanced center during grouping. Suffix points already occupy world coordinates and retain their bits. The prefix cannot exceed the active count; active count cannot exceed owned capacity. Capacity, owned links, active chains and dirty membership retain their existing admission checks. Inactive slots preserve their complete bytes. Inputs remain immutable on validation or publication failure.

The meaningful unit uses a nonzero body center: world point (100,200,300,1) remains unchanged, while relative point (100,200,300,0) becomes (102,203,304,1). It also checks an invalid prefix. The existing failed-publication test now exercises the geometric path and verifies complete prior world/pool/manager retention.

The runtime vehicle's ordinary four-wheel path still delegates through the previous all-relative contract. The new entry does not detect barriers, allocate a larger runtime pool, attach body shapes, generate wheel requests or schedule geometry.

## Original staged proof

New GTG2 `tools/probe_carried_geometry.c` executes unchanged AD7040 -> AD6C40 -> B77310 on retained owned buffers. Initial body and contact templates come from source Lacetti/Donington generator output, with the same explicitly cold-normalized body and authored finite material/velocity/solver settings used in F229. Each active contact is reconstructed from a supplied world-point template. For the external prefix, the harness subtracts the current pre-advance owner center; geometric suffix points retain world coordinates. The count schedule is authored, including pure geometric, mixed, all-relative and empty intervals.

These are fresh **records**, not observations of fresh collision detection. No whole AD8D70 call, broadphase, body-shape attachment, original wheel producer, callbacks or actual moving track query is claimed. Body and manager state stay retained between intervals; no body cold reset or solver reinitialization is performed.

The first two native drafts stopped safely at an ownership assertion after one successful case. A diagnostic line located the problem at world+0x7F4. Original AD6C40 increments that field for geometric contacts only, while AD8D70 resets 0x7F4/0x7F8/0x7FC before each frame. The staged harness had omitted those statistical resets and compared against total contacts. It now explicitly applies those three verified envelope resets and checks the suffix count. Private `20261010T1121170245051Z/ad8d70.c.txt` and `20261011T0920042177720Z/ad6c40.c.txt` establish the behavior. The third smoke and both full profiles passed. No original execution fault or Rust numerical mismatch occurred.

The original probe preserves its five snapshotted image-page restorations, executable-section comparison and no-import execution boundary. Geometry-pair canaries and owned body/contact/row/reference identities are checked. Friction-row references from earlier intervals retain their explicit ownership bounds before canonicalization. Reconstructed active contacts, inactive storage, body history, groups, materials, mass/inertia, solver rows/workspace/cache and all selected output fields are compared completely.

## Results and regressions

Per profile, independent semantic checks cover 16,248 world-space contact points, 2,432 relative-point lanes, 1,948 inactive slots and 2,432 predecessor-history pairs. Forty paired malformed/existing-output checks across both profiles pass, including invalid schedule counts, inactive nonfinite data and capacity. The audit rebuilds every request from source-linked evidence and checks 186 complete native/repeat/Rust output files, current executable hashes and both actual profile identities.

Ten historical shared native variants were separately rebuilt without overwriting pinned research probes. They and the current Rust debug/release executables reproduce **6,122 earlier cases / 207,840,804 bytes per run** across body advance, group reconstruction, shaped solving, whole/carried/reproduced worlds and all corrected packed source-contact handoffs. The final audit checks those 33 complete regression outputs too: 219 complete files overall. Previous fixtures are preserved, not reclassified as fresh detections.

All 175 regular workspace library tests pass; two purchased-source tests remain ignored by default. Both source driving tests were explicitly run and passed in release, including handbrake/reverse/reset/rollback. Formatting, all-target warnings-denied Clippy, three Python AST checks and Git whitespace pass. Existing Rust/Cargo 1.99.0, Python 3.12.14, MSVC 14.44.35207 / DevPrompt 17.14.41 / SDK 10.0.26100.0 were reused; no installation was required. One guessed Rust source path failed harmlessly and was corrected from inventory.

## Reproduction and private evidence

Build using `tools/build_carried_geometry_probe.cmd` and `tools/build_carried_geometry_regression.cmd`. Build `grid-physical-tick` in debug and release. Run `tools/validate_carried_geometry.py` with each explicit profile and a new ignored output directory; run `tools/replay_carried_geometry_regression.py`; audit both reports with `tools/audit_carried_geometry.py --regression`.

Final evidence remains private:

- `local/behavior/carried-geometry-{debug,release}-20261012-final-01/validation.json`
- `local/behavior/carried-geometry-audit-20261012-final-02.json`
- `local/behavior/carried-geometry-regression-20261012-final-01/validation.json`
- Matching `carried-geometry-*` build/test/Clippy logs and failed draft01/draft02 directories.
- Native SHA-256 `20e50a0511b9a201b6055536321fab740c7b9bd3034907835d832ab8a71d9059`
- Rust debug SHA-256 `14fd323e4176d7da21cde7a41165391f2ab23d868d69d6ceaaa8ee64ac5c98dd`
- Rust release SHA-256 `f82b3be2f575e79c2bb0c8621401f39b371db68a138bc7605ff4815eee2f795b`

Steam's 130 reference resources and executable remain unchanged at `2026-10-12T17:55:35.304760+00:00` (`local/scene/preservation-20261012-carried-geometry.json`). No UI input, settings changes or current approval blocker occurred. The existing playable application was not replaced with a barrier-capable build.

Next: retained manifold append/duplicate merging across triangles, then Rust witness/normal reorientation and fresh moving body/track detection feeding this mixed-coordinate retained physical path.
