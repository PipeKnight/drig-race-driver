# Triangle–mesh contact query (F225)

12 October 2026. Rust now implements the selected complete outer triangle/mesh query, including sampled penetration recovery and witness adjustment. Original/repeat/Rust debug/release agree on **48,153 queries and 4,140 hits per profile** across the installed Lacetti mesh and all245 selected Donington chunks. This extends [retained distance state](RETAINED_TRIANGLE_MESH_DISTANCE.md). It does not yet produce the contact manifold or body/barrier collisions in the driving prototype.

## Selected behavior and ownership

`convex_distance::Context::contact` stages its simplex, seven-entry support cache and ten extra sampling vectors, publishing only a successful operation. A valid miss still updates retained state. Invalid input or failed convergence preserves the previous context. `ContactSettings` supplies mesh margin and44 private source sampling directions. The selected triangle is untransformed, with zero triangle margin. Finite nondegenerate triangle winding is required if penetration recovery is entered. Context ownership and mesh-identity limitations remain as described in F224.

The complete selected B7F740 path first replaces a tiny incoming direction, normalizes it with the original SSE reciprocal-square-root refinement, then evaluates the support-depth pretest. It calls the existing distance loop with witness extraction deferred until the original matching point. A separated result uses direct witness extraction and margin adjustment. An overlapping result restores the incoming direction and enters B7ED10:44 startup directions plus the supplied direction and its negative, three mesh basis directions and their negatives, and the triangle normal and its negative. Minimum support depth selects the axis. Up to two translated distance calls then refine witnesses and perform the original displacement, margin and tangential adjustments.

The returned direction is the original closest vector; callers must not assume it is a normalized contact normal. Ordered comparisons, support-cache mutation, x86 SSE reciprocal/reciprocal-square-root seeds, operand order and transform addition grouping are retained. This is the observed sampled-direction algorithm; no EPA/polytope implementation is inferred. The earlier cold and retained distance APIs preserve their output framing and behavior.

## Private sampling preparation

`extract_contact_directions.py` is a bounded static research preparation tool. It recognizes344 instructions in the identified private analyzed C7DAE0 initializer through its first return, simulates16 stack bytes and XMM data moves, and reads55 scalar words from the unchanged Steam executable's file-backed data. Unknown instructions/addresses, duplicate writes and uninitialized stack reads fail closed. It executes no original code.

The resulting44 W0 approximately unit vectors occupy704 bytes. All704 match the independently captured whole original startup table. The private CD01 configuration contains magic/count and those vectors, totaling712 bytes. Table SHA-256: `14d6823a677259f013e1318d2952a14df37bb95f1f013ea66ac4e7482f931f48`; configuration: `ea8d8bff5270d5b75410f822c3e784a66399b7aae4e39365f8f54747639179c7`. Evidence: `local/behavior/contact-directions-source-20261012-final-02/validation.json`.

This preparation still depends on the private analyzed initializer. It is not an installation-only engine startup loader. Exact independent construction from geometric formulas has not been proved; the source values contain rounding that must not be replaced with idealized values without validation. Original sampling data remains ignored under `local/` and is supplied explicitly to the public Rust API; no original table is embedded in public source.

## Native boundary and diagnostic framing

PC03 is the existing cold original outer query. New PC06 adds retained outer sequences using the shared checked probe. B41F80 initializes support state once, then the original decoder/setter and whole B7F740 execute per query. All original code remains unchanged. Witness buffers are reset to authored CD sentinels before each call.

PC06 uses the PC05 count1..64 and24-byte explicit/carried-direction records, with its distinct magic. Rust independently reconstructs source mesh topology and each packed triangle. Every result contains complete **0x120 simplex +0xA0 query +0xA0 extra sampling vectors =608 bytes**, plus a count prefix for sequences. The native output additionally retains the complete checked source arena. Canonical pointers, surrounding canaries, all PE sections and restored lazily written pages/protections are checked. No entry point, TLS, live attachment or original import execution is involved. A separately named rebuilt PC05 regression executable proves that shared-probe generalization preserves the previous distance behavior.

## Verification

- **Cold:**1,801 cases/queries,756 hits,730 queries writing penetration extra scratch,91,101,784 native bytes/run. Includes all245 chunks at five heights and576 direction/margin controls.
- **Retained:**1,273 sequences/46,352 queries,3,384 hits,92,580,540 native bytes/run. Includes source candidates, repeats, explicit/carried directions, reversed traversal and margin controls.20,589 records contain nonzero retained extra scratch; this is not a penetration-call count because that scratch persists.
- Complete original/repeat/debug/release608-byte results match for every query. Separate one-query and64-query sequence boundaries also match.
- Cold guards:41 records/profile; retained guards:23 records/profile, including eight sampling-only guards in each. Malformed input and existing outputs fail without publication.
- Both current Rust profiles replay all1,801 cold distance cases and1,273 retained distance sequences/46,352 queries exactly. Rebuilt PC05 reproduces all85,164,220 F224 native bytes.48 grouped regression guard records pass.
- Final audit checks **25,873 complete output files /176 recorded guards**, current executable/report/script hashes, rebuilt requests, every decoded source triangle against the separate packed reader, full original arenas and independently reconstructed sampling.
- **173 workspace library tests pass**, one existing source-dependent test ignored. Formatting and all-target warnings-denied Clippy pass. The new analytic test covers finite overlapping witnesses, valid separation and transactional rejection of a degenerate penetration triangle.

Final private evidence under `local/behavior/`:

- `triangle-mesh-outer-debug-20261012-final-01/validation.json`
- `triangle-mesh-outer-release-20261012-final-01/validation.json`
- `triangle-mesh-outer-sequence-debug-20261012-final-01/validation.json`
- `triangle-mesh-outer-sequence-release-20261012-final-01/validation.json`
- `triangle-mesh-outer-regression-20261012-final-01/validation.json`
- `triangle-mesh-outer-audit-20261012-final-01.json`
- `triangle-mesh-outer-library-tests-20261012.txt`
- `triangle-mesh-outer-sequence-build-20261012-draft-01.txt`
- `triangle-mesh-outer-rust-20261012-draft-01/execution.json`: first five cold queries, hits0/0/1/1/0, complete results exact.
- `triangle-mesh-outer-debug-20261012-draft-02/validation.json` and `triangle-mesh-outer-sequence-debug-20261012-draft-02/validation.json`:20 cold and740 retained-query smoke checks, exact.

## Failures, versions and preservation

The initial source-preparation attempt rejected the first instruction from the still-packed Steam code section. It wrote no output; a following missing-file check also failed. The corrected reader uses the identified private analyzed initializer and Steam scalar data. An atomic patch initially failed because formatting had wrapped its usage-string context; inspection and reapplication corrected it. The first degenerate-triangle unit test exposed an infinite reciprocal-square-root seed becoming NaN: explicit finite/positive normalization admission fixed transactional rejection, and all actual source cases subsequently passed. Clippy rejected three arithmetic style patterns; narrow documented allowances preserve original floating-point order, after which warnings-denied checks passed. Draft interior instruction ranges were not trusted and were superseded by aligned entry walks. A guessed nonexistent module read was corrected using the file inventory. A sandboxed remote check failed with a DNS/thread startup error; approved escalation succeeded. This was not an approval rejection. No native execution or source-suite comparison failed.

Installed Ghidra **12.1.4**, JDK **21.0.9**, Python **3.12.14**, Capstone **5.0.6**, Rust/Cargo **1.99.0**, MSVC **14.44.35207**, Developer Prompt **17.14.41**, SDK **10.0.26100.0** were reused. Native flags `/W4 /WX /O2 /MT /std:c17 /fp:strict /arch:SSE2`; Rust `CARGO_INCREMENTAL=0`.

| Executable | SHA-256 |
| --- | --- |
| Native PC06 | `7b8dbeb3cbec0042d7ddd8c79a2dbcb01c6c672277124d86a44e88265d18454c` |
| New PC05 regression | `ab39403105abedb8bf24650b62e916655362f7d30868b91b7ad1bd3ef0b7418c` |
| Rust debug | `131e8c399e0a9611d86747ae9f632a827a12d5be9d5fb47bd082ddccb4b8c72d` |
| Rust release | `51ba1ef038a63786d7b58c592798ca6041928543a30d32a82b4732716e82dc3f` |

Pinned cold PC03 SHA: `4fc36149b0b0a3a409ad597aec9b34859b0a90494d535c2b364166542c446e0d`. Private analyzed image SHA: `aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514`. Steam executable SHA: `dd7b0933abf0d421bec3fa6f123319d3d3cc7221e5ef82b7a1201c3d1979e48f`.

All130 tracked Steam resources and executable match baseline at **2026-10-12 11:31:07.391412 UTC**, `local/scene/preservation-20261012-triangle-mesh-outer.json`. No installation, desktop input or settings changes, and no owner help/current approval blocker.

## Next integration boundary

Ghidra survey `20261012T1126269008807Z` successfully resolves AEE640 normal/witness reorientation, AE2FA0 feature/manifold assembly, B45310 mesh feature selection and B7C5D0 triangle feature selection. Private aligned evidence is in `outer-support-aligned-20261012.json`, `outer-feature-aligned-20261012.json` and `outer-margin-aligned-20261012.json`. These findings identify the next original boundary; they do not establish a Rust manifold.

Next: validate and port feature selection, reorientation and manifold publication, then generate fresh moving body/track contacts and preserve them through carried physical solving in the Bevy driver. The existing keyboard prototype can drive but has no body/barrier collisions. A full drivable Lacetti/Donington lap remains open.
