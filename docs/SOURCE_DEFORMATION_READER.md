# Direct source deformation reader

13 October 2026. `grid_formats::deformation` reads purchased NED FEMS/strength_main and the explicitly selected PSSG `LOD1_` joint bounds excluding aerials. `grid_tools::source_deformation` passes those independently decoded inputs to [the verified Rust initializer](DEFORMATION_SOURCE_INITIALIZATION.md). No capture, original executable or precomputed grid is read by this adapter.

Whole source output matches the independent Python reader followed by complete original initialization: **two models / 261,888 bytes per debug and release profile**, with original/repeat and two Rust runs. Each result includes selected/excluded counts, all eight bounds words, three complete A500 stages and complete 1054-byte settings. Lacetti selects 58 joints/excludes one aerial; BMW selects 63/excludes none. This is a selected player prototype policy, not proof of original factory selection or retained AI initialization.

The NED reader checks one direct FEMS/strength_main, at most 32 BOX records, affine matrices, positive sizes and bounded finite weights. It retains loader defaults, editor-extra gating, normal-map alias priority/derived reciprocal and the wing half multiplier. The graphics reader checks identity bundle/parent transforms, unique joint bounds, eight transformed corners, bounded finite payloads and strict reduction. Unknown source factory behavior is not inferred from matching output.

Three CLI guards per profile reject unknown models/missing source and preserve existing output. Audit reconstructs original requests and expected complete source outputs, verifies all eight files, five identities and current executable hashes. 194 regular libraries (25 formats +4 tools +165 world), formatting and all-target Clippy pass. A semantic parser test covers defaults, editor gating, alias precedence, invalid weights/extent vectors, duplicate attributes and invalid child ranges.

Private evidence under `local/behavior/`:

| Report | SHA-256 |
| --- | --- |
| deformation-source-debug-20261013-final-01/validation.json | 79bb9aed733d5aade11bbc5de82fb81b79568c966ce8005c12fc168a9c08bd26 |
| deformation-source-release-20261013-final-01/validation.json | 6512f8ca14cfceeaa31eb35b232a439d50691812ce2932272973aad76047fb26 |
| deformation-source-audit-20261013-final-01.json | ad90e422e6fe9beaa8ab977d802be7afcd242d329400f8e81e1dcbbb476b05b7 |

Rust debug diagnostic SHA `fd0f25b968e58a83ef8bb4b03376c94b8cbd680d38855e9547e3eca599ce5f62`; release `7c9942d2573729e417d058616eb058e8e96ba37da40c7e905bd12d2673197827`. Native initializer/image identities remain in the constructor records. Existing Python3.12.14/Rust1.99.0/MSVC14.44.35207/SDK10.0.26100.0/Ghidra12.1.4/JDK21.0.9/Capstone5.0.6 reused.

Failures retained: first source draft looked for `LOD1` instead of actual `LOD1_`, rejected both models without output; corrected after inspecting exact PSSG IDs. Initial Clippy flagged tests placed before the bounds reader; moved the test module to the end. A unit extent fixture was refined to three-component invalid vectors. No native fault, numeric tolerance change, install, desktop input or owner help was needed. Steam130 resources and original executable remained unchanged at **2026-10-13T15:09:07.739069Z**, private `local/scene/preservation-20261013-deformation-initialization.json`. Strengthened constructor audit `deformation-construction-audit-20261013-final-02.json` SHA `af22769a9ed6526d3c58c782ee48eb51dd72e896e2bfeb596d11966cf6eb6e8b` also passed.

## Next physical integration boundary

Original `A03030` dirty C7F2 calls **A01960**, clears C7F2, sets body4 and calls wheel update `9BB1E0`. A01960 is the collision/resource shape factory, not the already implemented suspension scalar consumer `9C32F0`. It can allocate/replace meshes and registrations. The whole publication caller A03220 then reaches `9EA4F0`, `9D1A90` and `9D1820` when body4 is set. Existing publication guards remain until this dependent state is handled.

The concrete next step is deformation-active `9B8B70 -> 9B8890 -> 9A39D0` source collision-point preparation, then owned mesh/shape rebuilding and the registered contact-to-grid callback. The sampler, mesh builder, shape update and callback arithmetic already have separate proofs; their composition and physical scheduling need validation. The current f428d0a23ce8 playable car remains rigid; unused reader/diagnostic changes do not require replacing it.
