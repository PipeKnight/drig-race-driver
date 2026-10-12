# Source triangle–mesh manifold (F226)

Follow-up: [F231](RETAINED_TRIANGLE_MANIFOLD.md) verifies retained append and
duplicate merging across triangles. The historical cold proof below remains
separate; reorientation, fresh detection and driving collision integration
remain open.

12 October 2026. Rust implements selected moving-mesh/static-triangle feature selection and complete AE2FA0 manifold assembly. Whole original/repeat/Rust debug/release agree on **2,425 cases /4,394 contact records /123,597,400 native bytes per profile**. This follows [F225 hit/penetration queries](TRIANGLE_MESH_CONTACT_QUERY.md). The combined packed dispatcher, retained multi-triangle manifold and driving integration remain open.

## Boundary and source reconstruction

`contact_manifold::calculate` takes the existing owned support context, independently initialized one-level mesh, current transform, an untransformed source triangle and explicit settings: normalized contact normal, fallback point/depth, mesh margin and capacity0..16. The mesh is moving; the triangle is static with zero margin. Other shape/feature types are outside this API. The complete context is staged, so a failed calculation cannot publish partial support state. Zero capacity preserves support state. This boundary accepts caller-supplied hit/reorientation results; it does not itself decide whether a pair collides.

B45310 transforms the negative normal into local space, performs ordered support-cache hill climbing, selects the incident face with the largest local plane-normal dot product, and returns its world normal and three vertices. B7C5D0 produces the source triangle normal and vertices. The feature's vertex count is3, but its type atB4 is **1**. The actual1/1 dispatcher entry D3AD98 is AEB630. Confusing count with type would select the wrong manifold routine.

AE2FA0 chooses and orients a reference plane, offsets vertices by the shape margins, and dispatches polygon clipping. AE24C0 forms normalized inward edge planes. AEB5C0/AE2000 clip the incident triangle against the reference plane and three edge planes, retaining original ordering, including polygon rotation when all vertices are inside. AEB630 clamps negative clipped depth to zero, merges nearby equal-normal contacts, respects capacity and invokes B0EE80 publication. With at most two contacts and spare capacity, AE2FA0 also tries its supplied fallback witness. B0D030 merges within original strict position/normal thresholds and updates larger penetration. Static-reference publication flips the stored normal and records the original orientation flag.

The Rust diagnostic independently rebuilds all source vertices, adjacency and face indices. It initializes outward face normals from the source geometry and checks every normal bit against the supplied source shape. It decodes each packed track triangle itself; no original output supplies Rust contacts or features. Original source geometry, face outputs and contact captures stay private.

## Native probe

PC07 extends the earlier cold request to84 bytes before source seed/chunk: the40-byte header, triangle index, four-float normal, four-float fallback point, depth and publication flag. The original feature functions run on separately guarded C0-byte buffers; their32-byte support descriptor is snapshotted/restored. Whole AE2FA0 then executes with the original feature functions from that same initial support state. Thus diagnostic feature inspection does not seed the actual manifold call. Capacity0 returns no manifold but still reports the separate feature inspection.

The native output retains the complete earlier arena, then the A0 query and both C0 feature buffers. Rust returns the full sixteen128-byte contact slots plus query/features: **2,592 bytes**. Undefined feature bytes retain explicit CD fill; unused contact slots retain whole original B0ECE0 templates. All source-body/mesh/chunk checks, owned canaries, full PE comparisons, lazy-page restoration and CREATE_NEW output protections remain. Triangle material/owner setup uses original AEE5F0/AEE610; publisher B0EE80 resolves the temporary kind13 triangle to its packed owner.

The direct boundary does not call the packed candidate cache. The checker asserts its untouched state, then normalizes only two unused cache lifecycle fields to reuse the existing complete arena checker. Publication flags are separately checked before the generic checker's constant-flag normalization. Recorded bytes are never modified.

## Cases and current verification

- 1,225 placements: 245 source chunks × five heights.
- 1,152 controls: 24 placements × eight authored unit normals × capacities 0/1/2/3/4/16, with supplied negative/zero/positive fallback depths and both publication flags.
- 48 margin controls: 24 placements × 0.05/0.5.
- Per profile: **4,394 contacts**,2,760 static-reference orientation flags,3,889 positive depths and255 negative supplied-fallback depths. These are deliberately authored manifold-boundary requests, including fallback points at separated placements; they are not counts of actual physical collisions or driving events.
- Complete original/repeat/debug/release contacts, cache and both feature records agree. **26 rejection/existing-output guards per profile** pass.
- **174 workspace library tests pass**, one existing source-dependent test ignored; formatting and all-target warnings-denied Clippy pass. Analytic clipping checks cover boundary intersections, all-inside ordering, orientation-aware duplicate matching and depth update.
- Fresh purchased-resource integration checks pass: source driving reset/controls/rollback and both Bevy keyboard/recovery/wheel-presentation tests. The current release driving build is available through [the launcher](TRY_THE_PROTOTYPE.md), with no manifold integration implied.

Current-source native regressions reproduce **9,990 complete earlier outputs** across manifold, cold/retained distance, cold/retained outer query and historical packed-dispatcher fixtures. Current debug/release each also reproduce 6,148 earlier distance/query cases, including both 46,352-query retained suites. All 168 grouped regression guards pass. The final independent audit verifies **36,836 complete outputs and 220 guards**, current executable/report/source hashes, reconstructed requests and every selected triangle through a separate source reader. Historical packed output equality remains subject to the descriptor limitation below.

Private evidence under `local/behavior/`:

- `triangle-manifold-debug-20261012-final-01/validation.json`
- `triangle-manifold-release-20261012-final-02/validation.json`
- `triangle-manifold-regression-20261012-final-01/validation.json`
- `triangle-manifold-audit-20261012-final-01.json`
- `triangle-manifold-debug-20261012-draft-02/validation.json`:70 cases/104 contacts, exact.
- `triangle-manifold-native-20261012-draft-01/`: first five original/repeat/Rust cases, contacts1/1/3/3/3, complete results exact.
- `triangle-manifold-library-tests-20261012.txt`
- `triangle-manifold-build-20261012-draft-01.txt`, `draft-02.txt`
- `triangle-manifold-regression-build-20261012-01.txt`, `02.txt`

## Failures, tool versions and limits

The first regression-variant compile caught a misplaced conditional terminator in the new native tag selection: PC07 compiled and executed, but another macro branch failed C1022. The selection was flattened and all six current-source probe variants then compiled. The primary PC07 executable/evidence was preserved. A separately named current-source PC07 rebuild is replayed against every primary case to verify that the conditional correction preserves its whole output. Earlier F221–F225 executables are not overwritten. Exploratory guessed file reads were corrected from inventory. No original manifold execution or selected Rust comparison failed.

The first independent audit caught a wrong source-report key (`source_chunk` instead of `chunk`). After that correction it also caught that the directory named release-final-01 actually contained a second debug run: the validator's release option had been omitted. That run is retained as superseded evidence, never relabelled. A fresh explicit `--profile release` sweep in release-final-02 passed all 2,425 cases and 26 guards against the actual release executable. The final audit checks profile names and current executable hashes and passes with that corrected report. Earlier progress text claiming both profiles before this audit was premature.

Ghidra **12.1.4**, JDK **21.0.9**, Python **3.12.14**, Capstone **5.0.6**, Rust/Cargo **1.99.0**, MSVC **14.44.35207**, Developer Prompt **17.14.41**, SDK **10.0.26100.0** were reused. Native `/W4 /WX /O2 /MT /std:c17 /fp:strict /arch:SSE2`; Rust `CARGO_INCREMENTAL=0`. No new tool installation, desktop input or original Steam/settings edit. Fresh original 130-resource/executable preservation passed at **2026-10-12 15:07:05.314318 UTC**; private `local/scene/preservation-20261012-triangle-manifold.json`.

| Executable | SHA-256 |
| --- | --- |
| Primary native PC07 | `06500e9cfd38c3c6c775b031769ce57ba1d1d6e8cdd339a04c5ec8885b8293c0` |
| Current-source PC07 regression | `0ffc2f532c546bb6bcc1faf36692beccfc8ac04388f130c0dba7e7e7338b732a` |
| Rust debug | `7b7e26a4da2e276f79a8fafdeb3d442f5a41b5e3cce3d7f3f0c71f49b0cbcfe6` |
| Rust release | `fc341e61c984035ce7f5cc6b75fd9b96d4cf9ad2c4c7e951f989377599471501` |

Surveys `20261012T1424404646859Z`, `1425086290090Z`, `1426056628825Z`, `1426486894365Z` resolve the actual functions; one additional valid AE93E0 output was unnecessary and has no claimed role. Aligned `manifold-entry-aligned-20261012-01.json`, `manifold-abi-aligned-20261012-01.json` and `manifold-constants-20261012-01.json` establish byte returns, argument count, scalar thresholds and arithmetic grouping.

## Newly identified earlier fixture limitation

While reviewing the next packed-dispatcher integration, aligned AEA1F0 instructions at AEAFE5 confirm a read of **packed shape+290**. The older F221/F222 helper allocates only280 descriptor bytes followed immediately by its header. That read therefore aliases header+10 (a packed bounds word) in those historical fixtures, rather than an explicitly owned shape flag. Existing whole-output regressions prove preservation of that fixture, not correct source ownership of this field.

The previously identified original packed constructor B1F900 writes290 as a bitmask derived from each resolved material's byte38, the source physical `sheet` attribute already decoded by F201. The old zero-scalar diagnostic material would imply a zero mask if initialized by that constructor. Source material mapping and the real constructor field must be verified before using whole packed dispatcher results as a generator acceptance baseline. F221/F222 normal-selection claims are narrowed accordingly. Their solver-stage byte comparisons remain valid for the supplied contacts; they do not establish that those contacts used source-correct sheet behavior.

F223–F226 direct triangle/mesh boundaries do not read packed owner+290; their selected proofs and the current wheel-ray driving prototype are unaffected by this layout issue. Evidence: `packed-normal-mask-aligned-20261012-01.json` and the earlier private `20261007T1745327734647Z/b1f900.c.txt`. Next work must give the packed descriptor sufficient separate storage, validate constructor-derived sheet bits and compare the corrected whole dispatcher, then join hit/reorientation/retained manifold and fresh moving solving. This needs offline research, not owner action.
