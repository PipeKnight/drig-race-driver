# Main contact response — F157, 10 October 2026

Complete main preparation and all four normal/friction iteration spans now match independent Rust in debug and release. The original path is B77CB0 → B79EE0 (including B78D30 and770AB0) → B791B0, with original BF3B70 workspace clearing. Every original function executes unchanged; each comparison samples full rows after bias preparation, after response construction/normalization, and after iteration, plus the final body workspace.

This takes **supplied constraint rows and inverse body properties**. It is not the whole B7ACC0 manager, impulse writeback, vehicle integration, geometric contact production or a live physical frame. The 183 construction handoffs preserve numeric/opaque row words from freshly replayed [F156](CONTACT_ROWS.md), but explicitly reframe batches and pointer identities. They exclude 950 normal rows with reversed impulse limits and their 729 dependent friction rows; those authored controls lie outside this adapter's accepted input domain. The retained handoff has 4,890 normals and 3,777 friction rows. Its body workspace, inverse properties, scales and iteration count remain authored. No live-frame provenance is implied.

## Recovered behavior

Manager +80 supplies a 32-byte workspace per body; +84 supplies a 64-byte inverse-inertia matrix; +88 supplies inverse mass; +8C supplies rows. Ordered span ends are +C4 (two-body normals), +BC (single-body normals), +C8 (two-body friction), +C0 (single-body friction). Batch heads supply integer +B4 lengths; body indices must be constant within each batch. Supported adapters bound 256 rows, 64 bodies, 32 iterations, 1,024 scenarios and 32 MiB input.

B77CB0 calculates row +90 from provisional bias +80 times the supplied bias scale, minus the ordered Jacobian/workspace projection. It scales row +84 by the supplied softness scale. The two-body projection adds linear A, angular A, linear B, angular B in that order, using adjacent-pair four-lane SSE dot products. The workspace's caller units are unresolved: static B7ACC0 shows inverse-time-scaled velocities and enabled external-force terms. Do not label this a recovered complete velocity update.

B78D30 constructs unscaled response vectors: inverse mass times each linear Jacobian, and three four-lane matrix-row dot products for each angular Jacobian, forcing angular output W to positive zero. It reads the matrix's fourth column but does not use its fourth row. B79EE0 then computes the ordered Jacobian/response dot sum, stores it at +B0, and forms `factor = relaxation / (dot_sum + softness)`. It scales the consumed Jacobians and bias, sets impulse +94 to positive zero, stores `softness * factor` at +9C and `factor` at +AC. Responses remain unscaled; unused single-body vectors and opaque words survive. This establishes what the saved +AC factor means before the separate optional renormalization in [F153](PHYSICAL_RESPONSE.md).

B791B0 always clears every body workspace, including empty rows or zero iterations. Each iteration visits the four spans in order. Normal rows clamp directly to their limits and publish a cumulative impulse prefix at +98, resetting the prefix per body batch. Friction rows multiply their limits by the **current referenced normal prefix**, read through +A8 for each row. They update impulse +94 but **preserve +98**. Negative-prefix authored controls retain the original ordered comparisons even when the scaled limits reverse. Both bodies are cached at batch entry and published A then B, preserving aliased-index overwrite behavior.

The Rust implementation shares this iterator with existing normal-only/optional-correction paths. It validates batches and references before indexing body operands, rejects nonfinite consumed operands, singular normalization and overflowing results, and leaves the caller's input unchanged on error. This is a bounded supported-domain contract, not arbitrary-input equivalence.

## Evidence

| Check | Result per debug/release profile |
|---|---|
| Complete original, repeat and one Rust call | 695 scenarios / 34,517 rows / 4,996,126 words exactly equal |
| Authored controls / constructed-row handoffs | 512 / 183 scenarios |
| Normal / friction rows | 11,438 / 23,079 |
| Nonzero normal / friction impulses | 4,563 / 12,325 |
| Changed body workspace | 417 scenarios |
| Empty / zero-iteration scenarios | 57 / 74 |
| Negative normal prefixes | 520 rows |
| Semantic checks / malformed guards | 5 / 25 pass; malformed requests exit ordinarily without native faults |
| Fresh construction regression | Original and Rust still match 711 scenarios / 5,052,810 words |
| Shared normal-only regression | Original and rebuilt Rust still match 701 scenarios / 714,170 words |
| Shared optional-manager regression | Original and rebuilt Rust still match 708 scenarios / 2,451,234 words |
| Library checks | 101 tests: 20 formats + 1 tools + 80 world; fmt and all-target warnings-denied Clippy pass |

No native or numerical failure occurred in this comparison. An initial Rust array-size inference compile error was corrected. Final-03 validation could not open a guessed prior-regression directory; corrected it to the actual verified F153 `*-01` directory, then reran both complete comparisons and regressions. Those interrupted runs are not successful final evidence. Initial matrix instruction ranges stopped before the return; final range includes the actual770C91 return and is authoritative.

Final reports: `local/behavior/main-contact-response-20261010-{debug,release}-final-04/validation.json`. Original helper survey: `local/behavior/20261010T0501398983214Z/`; matrix survey050455. Complete preparation alignment: `main-response-aligned-20261010.json`; complete matrix alignment: `main-response-matrix-final-20261010.json`. The earlier F153 complete iteration alignment supplies the full four-span code.

| Artifact | SHA256 |
|---|---|
| Private analysis PE | aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514 |
| Final native probe | e90f912bd17e1449606209bef2aae680151add18125d5995a6b4e9f15ffca921 |
| Final request | 4d3c310723f82f5d1d6ae7131cfdc5b8f43fb1feac92caf5fdc1f2c9c63a06ce |
| Debug Rust | 92c75d17c4d5233c9c08de90461541f827f0eb42af26d5cd86e007a1c5c078c4 |
| Release Rust | 77a469b55ed57b75801a9a3c4122ce46012474d400aef130e31a97634fcc01d7 |

Native code/manager/inverse-property invariance checks pass; executable sections remain RX and byte-identical. Original memset dispatch remains file value0. No entry point, import/TLS initialization, helper stub, code write or skipped original branch. Authored source is in `main_contact_response.rs`, its diagnostic CLI, the shared `physical_response.rs` iterator and matching probe/build/validator tools. Proprietary bytes and decompilation remain ignored under `local/`.

Reused Rust/Cargo1.99.0, Python3.12.14, MSVC14.44.35207 / Windows SDK10.0.26100.0, Ghidra12.1.4 / JDK21.0.9 and Capstone5.0.6. No installation or desktop input. Native strict SSE2/W4/WX/O2/MT flags and Rust debug/release builds unchanged.

Next compare **whole B7ACC0 with impact pools only**, including row initialization/group metadata, construction, preparation, solving, contact impulse writeback and enabled external-force/velocity updates. B0EBB0/B0EC60 call original B0FC00 to add the normal impulse to body +240 and set +40C. Main capacity failure and empty-contact bypasses must remain observable. Other joint pools, timing, initialization, full outer update and Bevy driving remain open; estimates unchanged. No owner answer is needed for the next offline work. Unattended deadline remains 10 October07:30 UTC.
