# Triangle–mesh distance iteration (F223)

Follow-up: [F224 retained query sequences](RETAINED_TRIANGLE_MESH_DISTANCE.md) now implement and verify partial scratch reset and persistent support state. The cold-only scope, hashes and counts below describe the preserved F223 checkpoint.

12 October 2026. Rust implements the complete selected **cold triangle/mesh support-distance loop**. It matches the unchanged original on **1,801 cases**, including every byte of its retained simplex, support cache, direction, termination flags and conditional witnesses.

This is a required part of geometric contact generation. The outer collision test, penetration search, manifold construction and moving runtime integration remain unfinished. A distance-loop result is not a finished barrier contact or a drivable lap.

## Implementation and source

`crates/grid-world/src/convex_distance.rs` implements ordered triangle support, source-order mesh adjacency hill climbing, the seven-entry support cache, one-to-four-point closest-feature reduction, duplicate/convergence checks, backup restoration and conditional witness extraction. `grid-convex-distance` reads source requests, rebuilds the owned mesh from supplied points/faces and rejects topology differing from that construction. It decodes the packed triangle through the existing VCQTC reader. It never reads native results to generate Rust results.

The checked Lacetti attachment is row48, `cars\lac\lac.ned`, fill zero. Complete seed SHA-256: `96e860c443d94ef1714a4bb2bf7cb7feb6e91017969fccca807793395a95da83`. Provenance and the corrected earlier Nissan/Lacetti draft are in [packed contacts](PACKED_MESH_CONTACTS.md).

Donington source: `tracks/europe/donington/route_0/track.jpk`, SHA-256 `6e161e5bee8f946ff0622ca836af9071c9474cec7aef53609180b84319f9d8ef`. Existing linkage supplies 245 packed chunks without static transforms. Placements, directions and optional margins are authored controls; they do not measure live placement, material calibration or collision frequency. Source geometry is not convexified or repaired.

The original operation is whole **B7EA60**, with triangle kind13, mesh kind7, the source mesh transform, triangle support **B7C830** and transformed mesh support **B43390**. **B41F80** initializes the mesh support descriptor. **B82300** performs closest-simplex reduction; **B82380** extracts witnesses when the first termination flag is set.

The worker scratch is 0x570 bytes. Its distance simplex is **0xE0..0x1FF**, not the beginning of the scratch. B7F520's aligned instructions establish that offset. Outer query **B7F740** has hidden ECX scratch plus eleven stack arguments. Its boolean return is **AL**, not a defined full EAX value.

## Behavior retained

The port preserves source adjacency order and the first improving neighbor. A neighbor must improve support by more than the stored approximately 1e-6 threshold. Cached ties retain the earlier entry; largest-simplex-distance ties select the later entry.

The world support transform adds **`(w*T + y*Y) + (z*Z + x*X)`**. An initially left-associated expression produced small multi-ULP differences in four of the first five cases. Aligned B43815..B4385E proved the grouping; correcting it made all five complete results exact. No tolerance was widened.

Triangle reduction preserves the degenerate fallback coefficients stored approximately 0.333 and 0.334. Tetrahedron reduction retains the original squared plane threshold, stored 1e-8 (`0x322BCC77`), face order, strict closest-face selection and active-point compaction. A tetrahedron containing the origin retains its prior weighted support while setting its closest direction to zero.

The two termination flags remain explicit. This survey produces `[0,0]` 1,039 times, `[0,1]` 730 times and `[1,1]` 32 times. Only the last group requests witnesses at this boundary. These flags are not a completed manifold boolean.

The safe API admits a one-level mesh produced by `collision_mesh::build`, finite bounded inputs and **cold scratch/cache initialization**. Finite iteration guards allow 256 distance iterations and point-count-squared support steps; none fired on accepted cases. Carried scratch/cache across a full multi-triangle pair and the penetration search reusing that scratch are not implemented yet.

## Native probe and contracts

`tools/build_triangle_mesh_query_probe.cmd` builds PC04 `probe_triangle_mesh_distance.c`, PC03 `probe_triangle_mesh_query.c` and a separately named default packed-contact regression executable. It preserves the pinned F221 binary. PC04 executes whole unchanged B7EA60 and conditional B82380. PC03 executes whole unchanged B7F740 for subsequent outer-query work.

Both wrappers share the checked owned mesh/body/packed-source construction. Whole B19EE0 selects packed vertices, B1ABC0 decompresses them, and AEE620 writes the triangle shape. Original startup/scratch/triangle/contact-slot constructors execute first. Complete mesh, body and packed chunk remain unchanged; all owned canaries survive. The probe restores five lazily written image pages and protections, then compares every PE section against the original mapping. There are no code replacements, import/entry-point/TLS execution, live-process attachments or Steam edits.

PC04 extends the existing 40-byte PC01 header with triangle index u32 and four direction f32 lanes, followed by the complete seed and packed chunk. The native output retains the full previous fixed arena and candidate arrays, followed by a 0xA0 query record:

| Offset | Contents |
| --- | --- |
| 0x00 | Source triangle index |
| 0x04 | Zero in distance mode; AL hit value in outer mode |
| 0x08, 0x09 | Distance termination flags; zero in outer mode |
| 0x10 | Returned four-lane direction |
| 0x20, 0x30 | Witnesses or unchanged authored CD sentinels |
| 0x40 | Canonicalized shape identity and seven support vertex pointers |
| 0x60 | Three decoded world-space triangle vertices |

Rust emits the complete **0x120 simplex + 0xA0 query =448 bytes**. Only this algorithm result is compared to Rust; the rest of the native arena is independently checked by the existing empty-contact audit. This diagnostic record is not a recovered public asset format.

## Verification and evidence

`validate_triangle_mesh_distance.py` runs original/repeat/Rust comparisons. `replay_triangle_mesh_distance.py` checks the final rebuilt debug binary against hash-checked native evidence. `audit_triangle_mesh_distance.py` reconstructs requests and checks every complete output and current executable hash.

- **1,225 base cases**: one selected source triangle per245 chunks, five authored heights each.
- **576 controls**: 24 seeds × three margins × eight directions: zero, signed axes, mixed direction and a scaled axis.
- Each native run produces **91,101,784 bytes /1,801 cases**. Original/repeat and Rust debug/release match exactly at the selected boundary.
- Simplex sizes: one point365, two154, three559, four723. **32 witness cases**.
- Each full profile checks31 paired malformed/framing cases and separate native/Rust existing-output preservation. Final debug and release replays each check32 Rust outcomes. Two additional short-span Rust guards reject before adjacency access. Combined audit: **15,825 complete files /130 recorded guard outcomes**.
- Rebuilt default probe reproduces **all1,417 F221 cases /69,092,472 bytes** exactly.
- Current PC03 native smoke: five complete original/repeat cases, **two hits and three misses**, unchanged failed-witness sentinels and checked decoded vertices. No Rust outer-query claim.
- **171 workspace library tests pass**, one existing source-dependent test ignored. Two new analytic tests distinguish separation, margin witnesses and intersection and reject invalid controls. Formatting and all-target warnings-denied Clippy pass.

Private evidence under `local/behavior/`:

- `triangle-mesh-distance-debug-20261012-final-01/validation.json`: full native/repeat/initial debug sweep.
- `triangle-mesh-distance-debug-20261012-final-03/validation.json`: final rebuilt debug replay/guards.
- `triangle-mesh-distance-release-20261012-final-01/validation.json`: full native/repeat/current release.
- `triangle-mesh-distance-release-20261012-final-02/validation.json`: final rebuilt release replay/guards.
- `triangle-mesh-distance-span-guard-20261012/validation.json`: both current binaries safely reject a shortened mesh span before adjacency reads.
- `triangle-mesh-query-regression-20261012-final-01/validation.json`: all prior generation cases.
- `triangle-mesh-distance-audit-20261012-final-03.json`: complete current-hash/output/request audit.
- `triangle-mesh-outer-smoke-20261012-final-01/validation.json`: current PC03 native smoke.
- `triangle-mesh-distance-library-tests-20261012.txt`: complete library-test result.
- `triangle-mesh-query-build-20261012-draft-04.txt`: current three native builds.

Ghidra evidence remains private in `20261012T1009401187765Z`, `20261012T1010246950122Z`, `20261012T1017152894083Z`, `20261012T1021041260774Z` and the earlier linked survey directories. Bounded instruction evidence: `triangle-simplex-abi-20261012.json`, `triangle-mesh-kernel-abi-20261012.json`, `triangle-mesh-support-20261012.json`, `triangle-mesh-query-return-20261012.json`. Pseudocode alone was not treated as a reliable ABI/arithmetic specification.

## Versions, failures and limits

Installed tools reused: Ghidra **12.1.4**, JDK **21.0.9**, Python **3.12.14**, Capstone **5.0.6**, Rust/Cargo **1.99.0**, MSVC **14.44.35207**, Developer Prompt **17.14.41**, SDK **10.0.26100.0**. Native flags: `/W4 /WX /O2 /MT /std:c17 /fp:strict /arch:SSE2`; Rust uses `CARGO_INCREMENTAL=0`.

| Executable | SHA-256 |
| --- | --- |
| Native distance | `8adb15aa2d95c12dbb1c0105cd859973ea68a2f6a8ebb62f67ff40d38dee74cb` |
| Native outer | `4fc36149b0b0a3a409ad597aec9b34859b0a90494d535c2b364166542c446e0d` |
| Default regression | `a20ab53cd800fa64a2b327cbe04dd3ed794e6b3a0f8c2802ffea8a4a6d0c0d29` |
| Final Rust debug | `13d48c70db5aeccd89699a7fa97104fd01d4a8154b7282ce3a40cce931a6832e` |
| Rust release | `47bd0833c351eff9a56e59038c8395a0dd511e7d3621859db64dce824c5f81a5` |

Draft01 outer probe failed its error21 check because its C signature treated AL as full EAX. No output was published; canaries/image restoration passed. Aligned returns resolved the ABI and draft02 passed five placements. A typed tetrahedron threshold was replaced by the actual original value before final builds. A Python replacement assertion wrote nothing; its unchanged mismatch report was retained, then the actual transform expression was patched. Clippy caught range-index and multiply-minus-one style warnings; iteration spelling was improved and a narrow documented allowance preserves original multiplication. A final CLI review found that a shorter supplied mesh span could leave regenerated adjacency reads outside the admitted mesh. An explicit exact-span preflight now rejects it before those reads; both rebuilt profiles replay all1,801 cases and the separate short-span guard. One guessed `crates/grid-core` read failed; file inventory found `grid-world`. A documentation patch used the wrong existing heading; the atomic patch wrote nothing and was reapplied with the observed heading. These are tooling/research failures, not game compatibility failures.

Original Steam executable SHA remains `dd7b0933abf0d421bec3fa6f123319d3d3cc7221e5ef82b7a1201c3d1979e48f`; private analysis image SHA remains `aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514`. All130 tracked resources and the executable match baseline at **2026-10-12 10:38:43.188477 UTC**, in `local/scene/preservation-20261012-triangle-mesh-distance.json`.

No desktop input, settings changes, installation or owner help was required. Ghidra succeeded; no current approval blocker. Next: retained scratch/support state and full outer/penetration query, manifold generation, then fresh moving contacts integrated with physical solving. Runtime barriers and a full drivable Lacetti/Donington lap remain open.
