# Complete owned collision-mesh construction (F212)

12 October 2026, Europe/Kiev. `grid_world::collision_mesh::build` reproduces selected whole original `B44E80` on supplied points and triangle indices. This adds the complete mesh topology needed by body shapes: source vertices, opposite-winding unit normals, triangle indices, unique ordered vertex neighbors, incident faces, initial transform/header fields and retained padding. All references are checked relative offsets into owned storage. No captured address is followed.

This is a mesh builder, not contact detection or a convex hull generator. It preserves supplied faces and does not prove that the input is convex, closed or manifold. Source adjustment, allocation, shape registration, broad phase, narrow phase, collision material assignment and barrier response remain separate. The driving runtime still has no geometric body/barrier contacts.

## Original boundary and arithmetic

The outer car factory `A01960` adjusts collision-grid vertices through `9B8B70`, then calls allocation/layout wrapper `B23E40`, whose whole mesh initialization is `B44E80`. The independent native probe calls unchanged `B44E80` with an owned layout matching the observed wrapper. It executes original cross product, SSE normalization, extent reduction and topology loops. Allocation and source coordinate adjustment are excluded from this proof.

Layout is a96-byte header,32-byte vertices,16-byte per-vertex adjacency headers,32-byte faces, aligned40-bytes-per-vertex neighbor capacity, and aligned12-bytes-per-face incidence capacity. Actual neighbor pointers deduplicate in triangle/corner encounter order; incident faces retain input order. Unused capacity and padding preserve the supplied zero/CD fill. The native probe verifies input arrays/settings unchanged, boundary sentinels, expected pointer regions and total counts before canonicalizing pointers.

Normals use the original cross and refined SSE reciprocal-root seed to obtain length, then multiply by **negative reciprocal length**. They oppose supplied triangle winding. The header5C value is the least absolute coordinate among the six XYZ extrema, computed by whole `B42540`; it is not labeled a radius. An initial pseudocode-only layout reading was corrected against aligned wrapper instructions before execution. The first successful Lacetti comparison matches all8,912 bytes.

The native executable maps only the identified private analysis PE, never its entry point or a live game process. One lazy-data page is snapshotted, writable during execution and restored with protection. Executable sections stay unchanged/RX. No code replacements, imports or allocator adapters are used. Outputs are created only after guards and restoration succeed, without overwriting existing files.

## Validation and source rejection

Each final debug/release suite matches original, repeated original and Rust across **180 cases /892,992 complete bytes**:42 installed source meshes with zero/CD fill, plus96 authored transformed, translated, reordered and reversed-winding cube controls. Independent checks cover complete topology, input vertex words, identity/header/extents, normal length/sign, ordered adjacency/incidence, all retained padding and capacity tails.

All43 same-stem Standard source meshes were examined. Raw `cars/bos/bos.ned` has coincident vertices2 and54 and zero-area faces11 and47. Both native preflight and Rust reject it in both fills, giving four recorded source rejection outcomes per profile. This is explicit unsupported input for this safe builder, not evidence that GRID cannot run BOS. Original source adjustment/deformation or original degenerate-output behavior has not been executed for that case. No face is silently deleted and no successful BOS comparison is claimed. The separate `vie.csv` variant has no same-stem NED and remains explicitly skipped.

Each profile passes36 additional paired checks: malformed size/tag/count/fill, nonfinite/range/W input, invalid/duplicate/coincident indices, too-small normals, adjacency capacity, and preservation of an existing output file. All reject normally; native faults do not count as guards.164 library tests pass, including tetrahedron topology/padding/winding and invalid geometry checks. Formatting, warnings-denied all-target Clippy and validator Python syntax pass. The final independent audit verifies current executable/report/request hashes and all1,080 complete original/repeat/Rust output files.

Private evidence:

- `local/behavior/collision-mesh-{debug,release}-20261012-final-02/validation.json` and adjacent execution logs.
- `local/behavior/collision-mesh-audit-20261012.json`.
- `local/behavior/collision-mesh-source-admission-20261012.json` and draft01 source rejection details.
- `local/behavior/collision-mesh-20261012-draft-01/`: initial8,912-byte Lacetti proof.
- `local/behavior/collision-mesh-{debug,release}-validation-20261012-final-02.txt`, library/Clippy/build logs.
- `local/behavior/body-shape-layout-aligned-20261012.json`, `collision-mesh-normal-aligned-20261012-v2.json`, `collision-mesh-constants-20261012.json`.
- Original surveys `20261012T0248299907061Z`, `0250121549659Z`, `0252237251351Z`, `0302461545897Z`, `0303587391988Z`.
- `local/scene/preservation-20261012-collision-mesh.json`: all130 Steam resources/executable unchanged03:05:35.865041UTC.

Retained failures: strict C build rejected a shadowed page variable before execution, then passed after renaming. Expanded draft01 stopped at the BOS source preflight; draft02 completed all admitted numerical comparisons but failed when rejection-report metadata reused the `kind` key. Reporting was corrected and both final suites rerun. One instruction read began inside an instruction; its prefix is not evidence, and the aligned v2 range replaces it. Guessed source paths/Windows wildcard searches failed harmlessly. Ghidra's sandbox Java-cache launch failed before analysis; approved ordinary escalation succeeded. The later guessed `B3D4C0` entry failed decompilation while the other three requested functions succeeded; no behavior is inferred from that failed entry.

Existing Rust/Cargo1.99.0, MSVC14.44.35207/SDK10.0.26100.0, Python3.12.14, Ghidra12.1.4/JDK21.0.9 and Capstone5.0.6 reused. No installs, desktop input or game settings changes.

| Artifact | SHA256 |
| --- | --- |
| Native mesh probe | `0b249f60d148868b57dfecab88bea037223856dd3f4884ca33132c72712695ba` |
| Rust debug diagnostic | `7ce7c8da7df7acbedad898b8905b55ba7c1612156e06b6158ba6389ddc24d7ed` |
| Rust release diagnostic | `8efb079e8c22ca9a806728a9c9a48614db7aa7be82010b1521bacfad892c9724` |

Exact source hashes are retained in the final report/audit. The next step is the enclosing source coordinate adjustment and mesh-shape attachment, followed by actual geometric contacts through the retained physical world. Those steps must be validated before claiming barrier support or a first lap.

F213 follow-up: [source coordinate adjustment](SOURCE_COLLISION_MESH.md) now validates the deformation-off supplied-setting path before whole mesh construction. Initial shape attachment has a separate normal reorientation step; it remains under investigation.
