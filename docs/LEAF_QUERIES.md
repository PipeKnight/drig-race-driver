# Supplied ordered packed-leaf queries (F085)

`grid-world::leaf_query::resolve` runs the verified f32 triangle primitive over a supplied leaf's triangle order. It preserves the original leaf consumer's segment-shortening and hit-replacement policy. Quadtree traversal, shape transforms, normal construction and material ownership are separate.

## Contract and original reference

Original `b23450` expands a leaf with `b1a370`, obtains packed triangle indices, decodes vertices with `b23250`, and calls `adc290` using the current shortened displacement. For every accepted hit it multiplies the local fraction by the accumulated fraction, stores that product, rebuilds all four displacement lanes from the original displacement and replaces the triangle token. **Equal-distance hits may replace earlier hits**. An exact zero accumulated fraction sets the stop flag and ends that leaf. Negative/above-one tolerance from the original primitive is retained.

The original decoder computes scales directly from supplied bounds using stored powers of two (1/2^24, 1/2^16), confirming the native source reader's arithmetic in this leaf path. The leaf lists use cumulative deltas, with254 advancing without emission and255 terminating. Native code receives the already decoded source lists/vertices; its 256-triangle limit and finite/index checks are authored guards. Original leaf scratch storage is bounded in the isolated harness before execution.

## Evidence

**148/148 cases exact debug/release**, with117hits,31misses and819checked hit words (triangle index, fraction, stop flag, four displacement lanes). Final evidence `local/behavior/leaf-query-20261007-03/validation.json`, `04/validation.json`. Original whole leaf consumer, expansion, packed decoder and triangle primitive execute without code patches, writable mapped pages, imports/TLS/entry initialization or live injection. Source offsets/counts are validated; runtime header pointers refer only to the private input copy. Probe SHA256 `f2c42b3349f70118e4e3ea53c38c059888aadb07d6a0ae2ea318afafb0b1759d`.

124cases use31actual Donington chunks/leaf lists, each with three centroid-normal casts and a displaced miss.24authored cases cover coincident triangles, layers, continuation254, repeated IDs, zero stopping and tolerance boundaries. Installed archive SHA is checked against the earlier130-resource preservation manifest before and after. All chunks/captures/reports stay ignored under local/.

Two analytic Rust tests independently establish later-equal replacement, exact-zero stopping, misses and invalid indices. All78project tests (47formats+31world), formatting and final warnings-denied Clippy pass; evidence `leaf-query-tests-20261007.txt`, `leaf-query-final-clippy-20261007.txt`. Initial Rust build rejected decimal literals without leading0; MSVC rejected a signed/unsigned comparison. Both were corrected before comparison. First Clippy rejects assignment spelling; changed multiplication to compound assignment and reran both original comparisons. Earlier01/02also match. These build/style failures are not game compatibility results.

Existing Rust1.99.0, strict MSVC14.44.35207/SDK10.0.26100.0, Python3.12.14, Ghidra12.1.4/JDK21.0.9 reused; no install. Authored tools `grid-leaf-query`, `probe_leaf_query.c`, `build_leaf_query_probe.cmd`, `validate_leaf_query.py`. Private static evidence `20261007T1630587257179Z/b23250.c.txt`, `packed-leaf-query-instructions-20261007.json`, `packed-leaf-constants-20261007.json`; initial tail disassembly began inside an instruction and is not used as proof.

This result does not validate the independent BVH as the original quadtree, nor original whole-world closest-hit behavior. Next: production candidate-grid capture, rectangle culling/quadrant traversal, complete packed-shape query and material-token linkage.
