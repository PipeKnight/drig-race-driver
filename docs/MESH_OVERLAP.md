# Two-mesh broad-phase registration and pair ownership (F218)

12 October 2026, Europe/Kiev. `AttachedShape::register_static_clone` registers a new static instance of the same mesh against the first dynamic shape proved in [F217](MESH_SHAPE_ATTACHMENT.md). Complete original B460F0 / ABC7B0 / B16D70 / B15030 / B15750 / B14EA0 and independently assembled Rust agree on **720 cases / 2,856,960 bytes per debug/release profile**. Each profile allocates360 actual broad-phase pairs and leaves360 cases unpaired;18 paired malformed/control/output-preservation outcomes pass.

This proves two kind7 meshes, one supplied region and a zero/one-slot pair pool. It does not prove narrow-phase contact points, source Donington barrier shape types, contact constraints, pair lifetime over moving frames, broad-phase world construction, runtime barrier response or a complete lap. The retained driving runtime remains in its prior wheel-contact scope.

## Original boundary and independent assembly

Native MP01 input contains a checked complete F217 seed plus translation, mask and pair-capacity controls. The validator verifies that seed and its source request against the F217 report and installed source provenance before executing. The native harness relocates the first shape's selected mesh/publication/region/endpoints into owned objects, clears material/callback slots, constructs the second static shape through the original constructor, publishes its shifted world pose and calls whole B16D70. The first dynamic body's nonnull owner is an owned zero-filled placeholder that remains unchanged; this stage does not reconstruct or advance a physical body.

Rust MO01 input contains the same controls and the original source CM05 request, **not native output**. It rebuilds source geometry, complete mesh/base/wrapper and first physical attachment independently, then registers the static clone. The already verified bounds reduction is shared with first attachment. Descriptor/local-transform, world counter and selected identity fields remain independently assembled. Material and callback slots are explicitly absent at this selected pair boundary.

The region has separate static/dynamic heads. Four endpoints on every axis stay sorted by coordinate, minima before maxima at equal values and older same-kind endpoints before newly inserted ones. B15750 checks layer masks, at least one dynamic owner and inclusive XYZ AABB overlap, then B14EA0 consumes a free pool entry. The pair's shape references are ordered by world IDs, linked into first-shape D0 / second-shape D4 and the world's allocated list. An empty pool returns no pair normally. Pair creation still occurs independently of later narrow-phase enable flags.

MP01 is32 control bytes followed by a verified CM05 result; MO01 is32 control bytes followed by a source CM05 request. Output is first shape0x250, second shape0x250, world0xA80, region heads16 bytes and pair80 bytes:3,968 total. Additional checked tokens are static shapeC000 and pairD000; mesh storage is shared. Canonical tokens identify typed owners only and are never dereferenced in Rust. This supplied diagnostic ownership graph is not the original resource allocator or complete world layout.

## Validation

All180 F217 source/control seeds receive same-position, partial-overlap, separated and blocked variants. Separation and partial overlap cover positive/negative X/Y/Z. Blocked cases cover zero masks, nonmatching layer bits and zero capacity. Original repeat and Rust compare every output byte. Independent checks walk all four sorted endpoint nodes per axis, verify reciprocal links and coordinates, recompute translated bounds within explicit rounding tolerance and derive expected pair presence from actual AABBs, mask and capacity. Native mesh and owner-placeholder bytes stay unchanged; all owned canaries and complete mapped PE section extents, including virtual tails, are checked after data/protection restoration. No code replacements, allocator adapters, native faults or numerical mismatches occurred.

168 library tests pass, including a semantic exact-face-touch test with original ordering, positive pair identity, just-separated/filter/capacity suppression, invalid translation rejection and unchanged first attachment. One source-dependent tools test remains ignored in the default suite. Formatting, all-target warnings-denied Clippy and two Python AST checks pass. Fresh current-binary first-attachment regressions each match180cases/1,912,512bytes/104guards after sharing the bounds helper. Independent current-hash/provenance/request audit checks **5,400 complete outputs** across four final reports and proves that each MP01/MO01 payload is exactly the corresponding native-seed/source request.

Initial CMD slash-path invocation failed before compilation; corrected to the existing Windows path convention. Missing float.h caused a warnings-denied compiler rejection; added before any execution. Bounds-helper extraction initially left two local variable references outside scope; corrected to read the stored bound words before rebuilding. All first four native/Rust controls then matched exactly. A later guessed survey-wrapper filename read failed harmlessly and was corrected from inventory.

One Ghidra owner survey was not executed because automatic approval review hit an account usage limit; no approval bypass or command execution occurred. After the owner explicitly requested Ghidra again, the normal approval route succeeded. The containing-function survey recovered B14550/B147E0 for eight supplied interior addresses, then stopped at B14E00 because no existing analyzed function contains that address. Earlier returned outputs remain private; no function was invented and this partial survey does not identify the world constructor. This is no longer an approval blocker.

Private evidence under ignored local/:

- `behavior/mesh-overlap-20261012-draft-01/`: first four actual original/Rust outputs and empty differences.
- `behavior/mesh-overlap-debug-20261012-draft-02/validation.json`: expanded first debug proof.
- `behavior/mesh-overlap-{debug,release}-20261012-final-02/validation.json`: final current probes.
- `behavior/mesh-shape-attach-{debug,release}-20261012-overlap-regression-01/validation.json`.
- `behavior/mesh-overlap-audit-20261012.json`: four reports /5,400 complete output files.
- `behavior/mesh-overlap-tests-20261012-final-01.txt`, `mesh-overlap-clippy-20261012-final-01.txt` and adjacent build/validation logs.
- `scene/preservation-20261012-mesh-overlap.json`:130 Steam resources/executable unchanged at 2026-10-12T08:25:50.568881+00:00.
- Original pair/overlap surveys0410095231117Z, geometry queues0412596895514Z, narrow-phase workers0422576933384Z and0424183117875Z; aligned `mesh-narrowphase-dispatch-aligned-20261012.json`.
- Partial containing survey0826148349543Z and `broadphase-owner-survey-20261012.txt`.

| Final executable | SHA256 |
| --- | --- |
| Native overlap probe | `93ee1fcb3df1ed8e76bbb25eea3431799cd0fb2aa23f21182f4751cc0ecbb4c2` |
| Rust debug | `889de69020656d5f48a9a2bd3c77f4354de0e6fe90682632fce6c10fb754b159` |
| Rust release | `12ac13bc26ea265c4ee3ad11dafca2587be44f6cf7b9c71faa69927607d72ae8` |

Rust/Cargo1.99.0, MSVC14.44.35207 / SDK10.0.26100.0, Python3.12.14, Ghidra12.1.4 / JDK21.0.9 and Capstone5.0.6 reused. No tool installations, desktop input, original Steam or user setting changes.

## Next integrated boundary

Existing original evidence establishes B18300 pair scheduling and B39730 worker iteration into B39500. B39500 checks shape flags and pair eligibility, selects an indirect type-pair entry, obtains space from a worker contact arena and calls the original narrow-phase kernel with old/current transforms. Its positive return stores pair20/24 and advances contact arena counters. Recover the actual type-pair table setup and worker arena, execute a positive complete original geometry case, then reproduce contacts in Rust and connect them to retained world solving. No contact-generation proof is claimed yet.
