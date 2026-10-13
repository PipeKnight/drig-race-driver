# Active deformation through the complete physical frame

13 October 2026. `deforming_physical_frame::calculate` joins both verified registered deformation callbacks inside `impact_frame`: preparation before solving; bulk callback/response consumption after successful solving; then optional position correction and final damping, limits and shift/pose publication. Callback state is cloned and only returned on success. The earlier collection-disabled control remains separate.

Whole original **B7B850 -> B78530/B77850/9A0C90 -> B7ACC0 -> B78710/B784B0/A064A0/B77490 -> optional B7B5E0 -> final publication** matches Rust for the admitted one-body/static-first-pool domain. The original helpers and CRT execute unchanged. Both shapes are active for bulk collection; no callback is stubbed or skipped.

## Complete comparison

Each debug/release profile independently passes **512 cases /35,483,648 complete output bytes**, original/repeat/Rust exact. This includes the complete owned **D230-byte car context**, 424-byte physical body, all contacts and rows, properties/workspace/cache and owned CRT seed. D230 is the bounded context used by this probe; it is not a claim to reconstruct or compare the entire original car allocation/factory.

- 390 successful frames,22 started capacity failures,100 unstarted sleep/zero-iteration bypasses.
- 10 seed changes,56 changed node pools and98 changed contact38 arrays.
- 256 paired correction-off/on controls. Source driving correction values are4iterations/.01softness/1scale/0threshold/.8relaxation. Correction runs195 times, is active169 times and changes162 published poses. Every pair retains the exact same car context, seed and contact buffers, confirming the downstream placement.
- Lacetti/BMW source grids/settings, file-backed defaults, authored body/solver/contact/history state, zero through64 contacts, disable gates, mass/velocity/force/damping/drag/shift controls and both owned lookup tokens0/1. Token1 is the source factory's wrapper role; it is relocated into the private graph and restored afterward.
- 25 guard cases /71 rejection or existing-output preservation outcomes. Aliased globals must agree across preparation and bulk policy; foreign lookup/settings roles, invalid grids/edges/nodes, null shapes, dynamic contacts, invalid masses/factors/booleans and invalid optional-correction inputs reject before execution. No native fault counts as a guard.
- Reconstructed audit checks220 files and five current source identities per profile, current executables, all complete requests/outputs/guards, pair invariants, node rest/bounds/marker/padding preservation and car bytes outside descriptor/history mutation regions.

The native probe owns its car/embedded descriptor, settings, physical body, shapes, contact pools, rows, manager, wrapper and CRT thread data. Sixteen private data pages, seven name/address-verified Windows imports and two private TLS slots are restored/released. Descriptor/owner/settings canaries, immutable settings/body tails and complete restored mapped-image equality pass. Original entry point and loader initialization are not run.

The prior no-callback695-case/15,581,940-byte and preparation-only256-case/14,779,392-byte paths were replayed in both profiles with rebuilt diagnostics against their retained original answers. All match.198 regular libraries, formatting, warnings-denied all-target Clippy and Python syntax checks pass. Current playable **df108f6656a6** remains available; this new active path is not yet selected by the source driving loop.

## Evidence

Private `local/behavior/`:

| Evidence | SHA-256 |
| --- | --- |
| deforming-physical-frame-debug-20261013-final-03/validation.json | 8758c2239ee3d6d8a80c77d6683360fec56f9c8d1ba640feca15cd3b1cce3dba |
| deforming-physical-frame-release-20261013-final-03/validation.json | c6c3ee5afbac877aa1c3b2547567ea02838aba753019d9670660628d9f52ffd4 |
| deforming-physical-frame-audit-20261013-final-03.json | 412a8aeb83f2e538e633d56a17253d2fa13e394f414caf09bd5ca466c557cb53 |
| deforming-physical-frame-probe.exe | 11e4180f32b34855756280f7a9b3af0a8aa561caa8298a3806ed8946146ea643 |
| target/debug/grid-deforming-physical-frame.exe | e574dbd0c7da8bd0878fe4ff9ce12856fc16c84d97d4e8bdd12f8969410fd9db |
| target/release/grid-deforming-physical-frame.exe | 60e1eac1acb0acce2656859a40a5a34c59a06eda53a2957f1206894a9c4ed3c5 |

Original PE `aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514`. Input identities include it, the two verified source-grid/settings outputs and two purchased car CSVs. Tools: `deforming_frame_native.h`, `probe_deforming_physical_frame.c`, build script, validator/auditor and `grid-deforming-physical-frame`; PFD3 extends the private frame protocol. Reused Rust/Cargo1.99.0, Python3.12.14, MSVC14.44.35207/SDK10.0.26100.0; strict SSE2/x8753-nearest. No installs, UI input or original Steam/settings changes.

Initial PFD2 proof covered256 uncorrected cases per profile. Authoring history above the body-adjustment threshold widened it to five seed changes/profile. The source runtime's optional correction then motivated PFD3's512 paired cases. Original friction-row pointers are normalized over the constructed row count, rather than the optional pass's reset final span. Lookup token0/1 coverage was added after reading the existing source-factory adapter. These are explicit authored extensions, not additional live observations. An atomic CLI patch missed formatting context and was reapplied before compilation; Clippy identified a redundant closure, corrected before final checks. No native fault or numerical mismatch occurred in these active-frame pilots/final runs.

## Runtime integration remains

This closes the bounded physical-frame composition, not source-world scheduling, dynamic cars, sleeping/wake graphs, asynchronous workers, geometry production or dirty consumer publication. Next carry this registration/state through the actual grouped source world and preserve the existing car descriptor/history during source initialization.

The verified cold source constructor starts **all five descriptor dirty flags A4F0..A4F4 at1** (source proof's initialized Lacetti DS01 output), with360 flag bytes255 and40 inactive bytes0. Thus source initialization itself needs the wheel/shape/presentation startup consumers before the normal car-job publication guard can accept it. Enabling callbacks or clearing C7F2 alone would be incomplete. Finish the reached A03030/A03220 consumer contract, then stage those changes with collision geometry and presentation. No owner recording or other help is needed for this research.
