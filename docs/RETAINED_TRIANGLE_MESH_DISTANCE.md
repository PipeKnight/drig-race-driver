# Retained triangle–mesh distance state (F224)

12 October 2026. Rust now retains the mesh support cache and partially reset simplex between triangle queries. Original/repeat/Rust debug/release match **46,352 queries in 1,273 sequences**. This extends the [cold distance loop](TRIANGLE_MESH_DISTANCE.md) toward the multi-triangle contact generator. It does not yet implement the outer penetration test, contact manifold or moving runtime barriers.

## State ownership and behavior

`convex_distance::Context` owns the complete 0x120-byte simplex and seven cached vertex indices. `query` stages a clone and publishes it only after success; invalid input or failed convergence leaves the previous context unchanged. `reset` restores cold initialization. Existing `calculate` remains a cold convenience call. Use one context per mesh pair and reset when its topology changes. Indices are checked against the supplied mesh, but this API does not enforce mesh identity.

Whole original B81950 resets only simplex bytes0x60..0xAF, its point count, three status bytes, last-support vector, associated score and largest-point index. It retains difference vectors, weighted support, closest direction, backups, point scores and unused storage. B7EA60 performs this partial reset on every call. B41F80 initializes the seven-entry support descriptor once for the sequence, rather than once per triangle. All arithmetic and ordered adjacency behavior remain as documented for F223.

The added unit test verifies that a failed query preserves complete prior scratch/cache and that explicit reset reproduces the cold result. **172 workspace library tests pass**, one existing source-dependent test ignored; formatting and all-target warnings-denied Clippy pass.

## Original probe and diagnostic framing

`probe_triangle_mesh_sequence.c` selects PC05 in the shared checked native probe. `build_triangle_mesh_sequence_probe.cmd` builds separately named sequence and cold-regression executables, preserving the pinned F223 binaries. The original B41F80 executes once, followed by the selected triangle decoder/setter, whole B7EA60 and conditional B82380 for each query. All original code runs unchanged. Each witness buffer is explicitly reset to authored CD sentinels before its call, so failed queries cannot inherit earlier witnesses in this diagnostic.

PC05 contains the existing 40-byte PC01 header, a u32 count1..64, then count records of24 bytes. Each record contains triangle index u32, direction mode u32 and four f32 lanes. Mode1 supplies a direction; mode0 carries the previous returned direction. The first record requires mode1. Even ignored direction lanes must be finite/bounded with W0. The complete source mesh/body seed and packed chunk follow these records. Pose and margin are fixed within each sequence; the query sequence and optional margins are authored controls.

Native output contains the complete prior arena and final 0xA0 query, then count u32 and **count ×448-byte records**, each the complete 0x120 simplex plus 0xA0 query. Support pointers are canonicalized in every record. Rust emits just count and those records. Source reconstruction remains independent: the CLI rebuilds mesh topology, validates exact adjacency/span and decodes each source triangle. It never consumes original outputs as inputs.

The native probe checks unchanged body/mesh/chunk, owned canaries, all PE sections and restoration of five lazily written pages/protections. Sequence records have their own surrounding canaries. There is no live attachment, original entry-point/TLS/import execution, code replacement or Steam edit.

## Verification

`validate_triangle_mesh_sequence.py` selects the checked Lacetti row48 and the same245 Donington chunks used in F221/F223. Each sequence includes repeated initial triangles, source candidate triangles where present, source-index sampling, explicit zero/signed/mixed/scaled directions, carried directions and reversed traversal. These are authored diagnostic sequences, not a measurement of the original manifold's exact triangle visitation order.

- **1,225 base sequences:** all245 chunks at five authored heights.
- **48 margin controls:**24 selected seeds ×0.05/0.5 margins.
- **46,352 queries /48 witness results /27,947 support-cache changes** per profile, complete original/repeat/Rust match.
- Each native sweep produces **85,164,220 bytes**. Every final arena is independently checked; the final query/scratch must equal the final per-step record.
- Separate one-query and64-query boundaries match original/repeat/debug/release.
- **15 guard records/profile:**13 paired malformed/framing inputs plus separate existing native/Rust output preservation. Invalid later modes, indices and directions publish no partial output.
- Both rebuilt Rust profiles replay all **1,801 cold cases /32 guards** from F223. The newly compiled cold native probe reproduces all **91,101,784 original bytes** exactly.
-48 cold-versus-retained comparisons use exactly the same source triangle, pose, margin and incoming direction. **32 scratch differences and40 cache differences** occur; returned flags/directions/witness bytes do not differ in these selected comparisons. Do not infer a physics change or numerical discrepancy from retained unused-state differences.
- Final audit checks **13,145 complete outputs /94 recorded guard outcomes**, current binary/report/script hashes, reconstructed source requests and every per-step triangle's decoded bytes against the separate packed-source reader.

Private evidence under `local/behavior/`:

- `triangle-mesh-sequence-debug-20261012-final-01/validation.json`
- `triangle-mesh-sequence-release-20261012-final-01/validation.json`
- `triangle-mesh-sequence-regression-20261012-final-01/validation.json`
- `triangle-mesh-sequence-cold-debug-20261012-final-01/validation.json`
- `triangle-mesh-sequence-cold-release-20261012-final-01/validation.json`
- `triangle-mesh-sequence-audit-20261012-final-01.json`
- `triangle-mesh-sequence-library-tests-20261012.txt`
- `triangle-mesh-sequence-build-20261012-draft-01.txt`
- `triangle-mesh-sequence-20261012-draft-01/validation.json`: first258-query smoke.

## Research, versions and remaining work

Installed Ghidra **12.1.4** with JDK **21.0.9** succeeded. Survey `20261012T1051023834709Z` resolves original B7D1B0 triangle batch support, B43880 mesh batch separation and B814E0 triangle normal. Together with the earlier B7ED10 survey, this shows a sampled-direction penetration search followed by up to two translated distance calls and witness adjustment. It is not evidence of an EPA/polytope algorithm. Original startup direction data remains private in `penetration-directions-20261012-draft-01.json`; independent construction and the outer port are still unverified. These research findings are separate from the completed retained-distance proof.

Other installed versions: Python **3.12.14**, Capstone **5.0.6**, Rust/Cargo **1.99.0**, MSVC **14.44.35207**, Developer Prompt **17.14.41**, Windows SDK **10.0.26100.0**. Native flags `/W4 /WX /O2 /MT /std:c17 /fp:strict /arch:SSE2`; Rust `CARGO_INCREMENTAL=0`.

| Executable | SHA-256 |
| --- | --- |
| Native sequence | `726f306d89445c3fde341adcc0f206883ebc464005d2db92902bda44d68e84d1` |
| New cold native regression | `f69ba70cca1cc89bf676499651b17ba45f427cfc3fd2b44b9d5832eb11d3e179` |
| Rust debug | `3853d5742a623c9ac1b1c2e15bc918b88a176edc2b0fd2d1dffa09f3acda2b82` |
| Rust release | `744351d9c16dd61776aaa392298674e4500d43d005085ddc52546a0f5abcab0c` |

No new compiler, original execution, comparison or guard failure occurred in this milestone. Prior cold-kernel failures remain documented in F223. No new tools were installed. All130 tracked Steam resources and the executable match baseline at **2026-10-12 11:06:50.661396 UTC**, `local/scene/preservation-20261012-triangle-mesh-sequence.json`.

Next: complete the source-owned outer/penetration query and manifold, then fresh moving body/track contacts into retained physical solving and the Bevy driver. This proof fixes the state boundary needed by that integration; it does not establish contact generation or a full drivable lap. No owner help, desktop use, settings changes or current approval blocker.
