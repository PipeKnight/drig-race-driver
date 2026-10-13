# Supplied-bounds deformation layout

13 October 2026. The independent Rust `deformation_layout` module reproduces complete original `9A2A70` / `989490` / `98A7D0` output for the admitted supplied-bounds domain. This completes the first construction boundary in [the initialization backlog](DEFORMATION_CONSTRUCTION.md). It does not initialize node limits, positive markers or edges, and is not connected to the playable car yet.

## Contract and validation

The API owns an entire A500-byte descriptor, up to 32 bounds records, five spacing/fallback scalars, one flag and an opaque settings token. Bounds XYZ must be finite within +/-1000; input W is deliberately opaque. Reduction uses strict comparisons, retaining the first equal signed-zero word. Expansion, X-only fallback, the SSE reciprocal-square-root seed/refinement and the mandatory first spacing growth follow aligned evidence. Counts are truncated span/spacing plus two; the spacing loop stops at a product of at most 400, then recenters the grid. Active current/rest positions, node30 vectors and flag bytes are initialized; node20 limits, node40 markers, edge storage and inactive padding survive.

Admission limits are explicit Rust/probe safety constraints, not original game error behavior: margin 0..10, spacing factor .005..1, growth 1.001..2, fallback components -100..-.001, nondegenerate aggregate span, at most 4096 spacing iterations. Thin layouts can have an individual dimension above 16; the separately verified vehicle/sampling APIs retain their narrower domain. No silent dimension cap was added.

`grid-deformation-layout` uses bounded GL01 framing and create-new ignored-local output. `probe_deformation_layout.c` executes the complete original functions against owned bounds-bearing contexts without starting the executable or running imports/TLS. Seven private mapped pages are controlled and restored; owned canaries, source context immutability and the complete restored mapped image are checked. No original helper is replaced.

Both debug and release, independently:

- 281 original/repeat/Rust cases, **11,869,440 complete descriptor bytes** exact.
- 64,894 active node resets, 48 empty-bound cases and 28 wide-dimension fixtures.
- First-equal signed zeros, opaque W NaNs/infinities, 32 bounds records, inverted-X fallback, valid aggregate bounds despite inverted individual records, varying token/flag and arbitrary preserved padding.
- 19 guard cases / 53 rejection or output-preservation outcomes. Invalid requests create no output; existing outputs remain unchanged.
- Current-file audit reconstructs requests, checks all 69 files and three identities per profile, and independently checks ownership and complete output.

191 regular library tests, formatting and all-target Clippy passed. Existing Python 3.12.14, Rust/Cargo 1.99.0, MSVC 14.44.35207 (SDK 10.0.26100.0), Ghidra 12.1.4/JDK 21.0.9 and Capstone 5.0.6 were reused. Native builds use strict SSE2 floating point, 53-bit nearest x87 and a 16 MiB stack. No installation, desktop input, settings change or Steam mutation was needed.

Private final evidence under `local/behavior/`:

| Report | SHA-256 |
| --- | --- |
| deformation-layout-debug-20261013-final-02/validation.json | aadf3e5106bb6fe1bd3dd83cda983e03840b785632603f3749cf3be470ba676a |
| deformation-layout-release-20261013-final-02/validation.json | 1dfb17c57e0420f7644f80845c2aacb98588281211f4a8889692ae88979bd4c0 |
| deformation-layout-source-debug-20261013-final-01/comparison.json | 18442d421dfcbe3599fcb1b20797784f17a9ae042120586b080e6f7aa4723aa0 |
| deformation-layout-source-release-20261013-final-01/comparison.json | ae9dda1b29161b366555fbe66755a1f5c848f79562f2262e290441fb41cebb5e |

`deformation-layout-complete-audit-20261013-final-01.json` audits all four reports. Native probe hash: `28006b81434b94dbc6337679faf9ce2594321ef9676dc1a52644321e998929f1`; Rust debug: `a7587031bbdbbfc04371084d0dfb418535c5562bb419093e6f91f5baecb81b81`; release: `1aba4163d527e88a17ae2927fef97e62cb42809a5345366cecc8725c2059e2e4`. Earlier successful smaller-domain drafts remain private and are superseded by final-02.

## Source matching remains a separate question

`compare_source_deformation_layout.py` independently reads purchased Lacetti/BMW NED and high/low PSSG assets, explores collision/strength bounds and transformed graphical LOD selections, and compares complete original/repeat/Rust layouts for 100 candidates / 4,224,000 bytes per profile. Provenance audits reconstruct those requests and verify six source assets plus two retained captures.

Only the **player Lacetti highLOD LOD1 without the aerial** matches every layout field and every rest-position word in both retained snapshots. Declared joint bounds, decoded stream ranges and indexed used vertices produce three equivalent candidates: six matching combinations mean two observations times three methods, not six different cars. There are 2,880 unique matched player rest-position words. Pristine current positions also match; the later impact capture has expected current-position changes. AI grids remain unmatched.

The first investigative script incorrectly treated manager `kind` as car model; it identifies player/AI class. Corrected comparisons uniquely identify model through seven independent NED collision extent words. Drafts with the incorrect assignment remain private; final reports use the corrected mapping. Collision-main bounds and the union of 24 strength boxes do not reproduce the layout. Excluding the aerial is still an explicit investigative selection, not proof of the original resource factory's selection rule.

Original static loader references distinguish FEMS/strength_main from collision_main. `99CC00` admits up to 32 strength BOX records (80 hex bytes each); `9EFB30` reads FEMS scalar settings. Their source-guided independent reader and the whole original initialization baseline are documented in [source initialization](DEFORMATION_SOURCE_INITIALIZATION.md). Next: independently port mode-zero cell volumes/limits/markers and edges, then dirty wheel/setup rebuilding and transactional physical-impact integration. Keep the current f428d0a23ce8 playable build until an actual runtime change passes its checks.
