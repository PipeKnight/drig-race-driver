# Independent track ground queries

Recorded 6 October 2026. `grid-world` now provides a renderer-independent, standard-library-only static triangle index and finite-segment queries. This is independently implemented geometry infrastructure, not recovered GRID collision/contact behavior. It is not yet connected to vehicle forces or the viewer's spawn height.

## Contract

The mesh accepts up to one million triangles with finite f32 vertices. Input order remains the triangle identity; degenerate triangles are counted and omitted from the spatial index. Four-byte material codes and flags remain opaque metadata. No back-face culling, flags-based filtering, entity collision or material coefficients are inferred.

A median-centroid bounding-volume hierarchy has at most eight triangles per leaf. Queries accept finite endpoints within the f32 coordinate range, calculate intersections in f64, and return the nearest two-sided hit on a finite segment: fraction, position, source-winding unit normal, barycentric coordinates and metadata. Coplanar and zero-length segments return no hit. Relative parallel tolerance and hit-fraction tie tolerance are `1e-12`; barycentric edge tolerance is `1e-10`. Fractions within the tie tolerance choose the lower input triangle ordinal. Contact policy can later choose an upward-facing normal; this primitive preserves winding.

No original quadtree traversal, numeric precision, sweep shape or contact response is copied into this module. It indexes the verified VCQTC triangle geometry using our own hierarchy.

## Validation

Five independently authored synthetic tests cover analytic sloped-plane intersections, nearest-hit reversal, metadata/winding, endpoints, misses, coplanarity, invalid coordinates, degenerate/empty geometry, 128 analytic layers, stable duplicate-surface selection, shared edges/vertices and actual hierarchy pruning. Hierarchy bounds include conservative padding for the declared barycentric edge tolerance, so broad-phase culling cannot discard a tolerated edge hit.

On the original Donington route_0 resource, **546 queries all agree** with a separate Python implementation using plane intersections and projected 2D barycentric coordinates. The native implementation uses Moller–Trumbore with a hierarchy. Python reads original JPK/VCQTC bytes independently, rather than trusting native triangle output.

The cases include the known start anchor, origins and centres of all 21 grid markers, 71 gate midpoints, 389 route points, 20 oblique surface-normal casts and their reversals, two outside casts and a zero-length segment. **543 hit**, three miss. Triangle identity, material and flags agree; position, fraction, normal and barycentric coordinates agree within **1e-8**. These are geometry comparisons, not original-engine contact measurements or proof that every route point denotes a driving surface.

Mesh: **119,784 triangles**, **32,767 hierarchy nodes**, zero skipped degenerates. Native queries tested **8,627 triangles** in total, versus **65,402,064** triangle checks for a full scan. This is an operation count, not a runtime benchmark or claimed simulation tick capacity.

Initial warnings-denied Clippy found an unnecessary range-index loop; changed it to enumerated iteration. Workspace Clippy subsequently passes. Test/build/lint logs and the complete private source/query comparison live under `local/behavior/ground-*`; `tools/validate_ground_queries.py` preserves evidence using new files only. The diagnostic `grid-ground` refuses output outside `local/` or replacement of an existing report.

```powershell
cargo run -p grid-tools --bin grid-ground --locked -- 'C:\Program Files (x86)\Steam\steamapps\common\Grid' local/queries.tsv local/ground-new.tsv
```

Input rows are tab-separated name, start XYZ and end XYZ. Source hashes, all expected/native hits and uncertainty are recorded in `local/behavior/ground-validation/validation.json`.

## Original query investigation

Following the previously identified pose builder reaches ground/frame function `0x009de4e0`. Targeted decompilation now identifies wrapper `0x0096b7f0`: it forms horizontal endpoint bounds, calls candidate collector `0x0096b440` with capacity 25, invokes `0x00a6a530` for candidates and retains the smallest returned hit fraction. `0x009adc00` wraps this with vertical endpoints and converts the resulting fraction back into a surface height. Parameter/register types inferred by Ghidra are incomplete; these are static observations and need instruction/runtime checks.

Private outputs: `local/behavior/20261006T1407232213170Z/`; run log `ground-consumer-survey-run.txt`. Existing Ghidra 12.1.4 and JDK 21 were reused, without broad reanalysis or tool installation. Lower-level chunk traversal, surface filtering, wheel clearance and final spawn frame remain under investigation.

Later edge-padding validation again matches all 546 cases, with the same operation counts. After correcting unsupported collision upper-index handling, a fresh native report is byte-identical to the independently validated edge report (`ground-index-correction-report-comparison.json`). The separate [f32 triangle primitive](TRIANGLE_KERNEL.md) subsequently matches 568 isolated original-code cases exactly. That primitive has broader original tolerances than this f64 query; the two contracts are deliberately recorded separately until whole-query integration is validated.
