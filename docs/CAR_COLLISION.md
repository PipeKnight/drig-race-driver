# Car collision geometry and placement bounds

Recorded 7 October 2026. `grid-formats::car_collision` independently reads the named `COLLISION/GEOMETRY` child with id `collision_main` from purchased car `.ned` files. These points and triangles describe car collision geometry separately from track VCQTC and rendered PSSG geometry. This reader supplies local bounds; collision response, convex decomposition and damage deformation remain unimplemented.

## Why the named defaults did not match

Lacetti and BMW both declare front/rear center Z defaults of +1.9/-1.9 and side X defaults of +0.8/-0.8 in the COLLISION attributes. Saved live body fields differ. Following writes to the temporary description reveals this chain:

1. `0x009efb30` reads named COLLISION fields, then invokes `0x009cca70` on description `+0x10c0`.
2. The child loader selects `GEOMETRY` with id `collision_main`, reading `POINT.pos` float triples and `TRIANGLE.points` index triples. Points are stored as vectors beginning at shape `+0x10`; point count is at `+0`, triangle count at `+0x510` and indices at `+0x514`.
3. If this loader succeeds, `0x009e2030` calls `0x0099d180`, which reduces the stored points with SIMD minimum/maximum. It replaces center X, bottom Y, front Z, side X values, rear Z and roof Y in the description. Other corner/roof-edge fields use additional segment/triangle queries and remain outside our implemented contract.
4. Body construction copies the description at body `+0xd240`. Selected extents therefore appear at body `+0xed64`, `+0xed68`, `+0xed6c`, `+0xed70`, `+0xed74`, `+0xed88`, `+0xed94`.

Center X is the f32 mean of min/max X. The other six selected words come directly from coordinate extrema. The reader preserves later-operand selection on equal minima/maxima, including signed zero. It rejects nonfinite coordinates, invalid indices, missing/duplicate/ambiguous selected elements and unsupported geometry children. Supported limits are 80 points and 110 triangles; oversized inputs are rejected rather than silently truncated. Missing custom geometry is not treated as permission to fabricate a mesh or use defaults.

## Evidence and validation

Lacetti has 51 points / 98 triangles; BMW has 57 / 110. Their selected bounds are:

| Car | Minimum XYZ | Maximum XYZ |
|---|---|---|
| Lacetti | -0.904889, 0.103433, -2.237876 | 0.913279, 1.420987, 2.241661 |
| BMW | -0.946025, 0.056228, -2.237876 | 0.947784, 1.349684, 2.258862 |

Independent Python decoding matches every native point, triangle and selected bound for **43 same-stem Standard source pairs**. The separate `vie.csv` variant lacks same-stem geometry and remains explicitly skipped. The seven selected bounds match **84 f32 words across all twelve saved reference cars** exactly. This is live agreement for Lacetti/BMW; other cars have source/native agreement only.

The same independent check verifies the corrected wheel-radius override: **96 captured words** at initialized wheel `+0xe0` and `+0xf4` match `.ned` radii. Thirteen source pairs have different early CSV and final `.ned` values. [Wheel geometry](WHEEL_GEOMETRY.md) records the correction and its impact on the prelude's scope.

Two synthetic mesh tests exercise analytic bounds, ignored named defaults, signed zero, invalid coordinates/indices, ambiguity, duplicate attributes, child ranges and point limits. **43 format tests and seven world tests pass (50 total)**. Native build succeeds. Final workspace lint is recorded separately in the project log after completion.

Implementation: `crates/grid-formats/src/car_collision.rs`; CLI: `grid-car-collision INPUT.ned local/OUTPUT.tsv`; independent check: `tools/validate_car_collision.py`. Private reports: `local/behavior/native-car-collision-validation/validation.json` and its 43 native TSVs. Snapshot: `general-vehicle-manager-20261006T1602122991209Z.json`, captured after a race restart and pause, **not at the exact initializer boundary**. Source hashes and snapshot hash are included in the validation report.

Targeted Ghidra 12.1.4/JDK 21 evidence: `local/behavior/20261007T0027097042111Z/`, `20261007T0027332799781Z/`, `20261007T0028010449806Z/`, `20261007T0028185999365Z/`; all outputs are private. An incorrect diagnostic filename `address-context.txt` was corrected to the actual `contexts.tsv`; a PowerShell glob passed to rg was replaced by directory-based searching. Neither failed read supplied evidence.

Continuation: mode-one alignment and ground/clearance arithmetic are implemented; twelve actual return-boundary matrices reproduce exactly from recorded inputs. Source-only inputs retain two one-ULP X differences. The optional deformation-adjusted extent and live marker corners remain unresolved. [Evidence and scope](LIVE_SPAWN_CAPTURE.md).
