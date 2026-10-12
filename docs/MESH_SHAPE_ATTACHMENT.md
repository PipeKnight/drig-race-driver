# Complete first mesh-shape physical attachment (F217)

12 October 2026, Europe/Kiev. `Mesh::construct_and_attach` reproduces complete cold `B11F40` then complete `B142D0` with wake-up skipped, following the verified source coordinates, mesh, base and wrapper constructors. Original/repeated original/Rust debug/release each match **180 cases / 1,912,512 observable bytes**, with **104 paired guard outcomes** per profile. The initial Lacetti result is 14,576 bytes, including all owned objects and retained padding.

This is the first dynamic shape in a supplied one-cell world with initially empty endpoint and region lists. It proves registration and bounds, not geometry pairs, narrow-phase contact generation, barrier response or a complete lap. The source driving runtime still runs its prior wheel-contact scope. The earlier [static attachment research](PHYSICAL_SHAPE_ATTACHMENT.md) establishes the original function boundaries.

## Owned construction and publication

The registry and full world are distinct owned objects. Body32C points to the registry; registry18 points to the full world. Complete original kind7 bounds dispatch B42050, B159C0 region selection, ABC1B0 region membership, B15030 XYZ endpoint insertion and B11520 body refresh execute without code replacements or allocator adapters. Supplied diagonal inertia sets body411, bypassing shape-derived mass accumulation. Active and inactive construction are both checked; skipWake1 preserves inactive status.

The Rust path reuses the verified physical constructor and matrix composition. Original bounds use four independent SIMD min/max accumulators with tail points in slot0; operand grouping and equal-value selection are preserved. Margin expands XYZ only. ShapeD8/E0 and body328 establish the ownership list, shape198 becomes region1, all six endpoints receive coordinate/link values, and the dynamic region head references the shape. Original body refresh leaves the existing supplied-inertia derived values unchanged in this tested scope, including nonzero offsets and rotated poses.

Source cases read mass/inertia afresh from CSV columns3..6 and collision geometry/offset from NED and wheel layout. Shape flags/scalars, local poses, physical offsets, body yaw/translation and active state are explicit authored controls. Whole outer source-factory settings are not claimed. The one-cell world uses supplied scales1000 and counts1/1; this is an owned test configuration, not a recovered Donington broad-phase configuration.

Diagnostic tokens are checked identities, never dereferenced by Rust: shape1000, mesh1500, world12E0, wrapper13E0, material manager1400, bodyA000, registry9000, region arraysB000/B008. CM05 stores 544 header bytes followed by raw points and triangles. Output is 0x500 prefix, mesh, body0x430, mutated settings0x160, registry0x100, full world0xA80 and region arrays16 bytes. The prefix aliases the world's first0x100 bytes for compatibility with earlier diagnostics.

## Verification and limits

42 admitted installed source meshes with both fills plus96 authored controls cover local transform enabled/disabled, yaw, translation, center offsets, margin, span, material lookup and active state. Independent checks recompute transformed AABBs and verify ownership, counters, endpoint structure, cell heads, complete mesh topology and retained bytes. BOS zero-area faces11/47 remain explicit rejections in both fills; the missing same-stem variant stays recorded. Invalid layers, negative margin, physical pointers/counts/flags, mass/inertia/nonfinite values, framing and existing output files reject normally.

168 library tests pass (23 formats,2 tools,143 world; one source-dependent test ignored in the default run). The added semantic unit checks composed translations/expanded bounds, unchanged supplied inertia, inactive skip-wake behavior and region/endpoint ownership. Formatting, all-target warnings-denied Clippy and two Python AST checks pass. Clippy's original-operand-order addition warning uses a narrow documented allowance. An incorrect guessed report filename and source-driving filename, and wildcard search paths, failed harmlessly; corrected from inventories. No numerical mismatches or native faults occurred. The first identity-only proof was followed by broader rotated/offset cases before claiming equivalence.

All five native mesh-stage probes were rebuilt after shared protection-loop changes. Fresh debug/release CM04/CM03/CM02/CM01 suites each match180 cases, with78/64/50/36 guards respectively. A reusable independent auditor checks current executable hashes, source provenance, request hashes and all **5,400 original/repeat/Rust output files** across ten reports. Native attachment checks complete input/canaries, unchanged executable code and all mapped PE sections after restoring four modified data pages and their protections.

Private evidence under ignored `local/`:

- `behavior/mesh-shape-attach-20261012-draft-01/`: first actual original call, source fixture provenance and first Rust comparison.
- `behavior/mesh-shape-attach-debug-20261012-draft-02/validation.json`: expanded first debug proof.
- `behavior/{mesh-shape-attach,mesh-shape-base,mesh-shape,source-collision-mesh,collision-mesh}-{debug,release}-20261012-attach-final-01/validation.json`: final current binaries.
- `behavior/mesh-shape-attach-audit-20261012.json`: ten reports/5,400 complete outputs.
- `behavior/mesh-shape-attach-tests-20261012-final-01.txt`, `mesh-shape-attach-clippy-20261012-final-02.txt` and adjacent build logs.
- `scene/preservation-20261012-mesh-shape-attach.json`: 130 Steam resources and executable unchanged at 2026-10-12T04:10:59.836219+00:00.

| Final executable | SHA256 |
| --- | --- |
| Native attachment probe | `dea08d17b7027b3affd80c82303f715ebc9819212a01620592f876b9cf0d8d23` |
| Rust debug | `43a55ec6793fd09ff58dcdb404c9650f5b9ce27375ba13136341c0fbe30d2cb9` |
| Rust release | `d1c60763b1a96ca690815f39fc359c2f971d3399488f340b18781b92f4eca517` |

Existing Rust/Cargo1.99.0, MSVC14.44.35207 / SDK10.0.26100.0, Python3.12.14, Ghidra12.1.4 / JDK21.0.9 and Capstone5.0.6 reused. No installations, desktop input, original Steam or user setting changes. Next execute overlapping owned shapes with the original pair pool and validate real geometric contact generation before integrating body barriers into the retained driving world.
