# Supplied packed-node traversal (F089)

`grid-world::packed_query::Mesh::query` combines the verified horizontal cull and ordered leaf consumer across a supplied untransformed packed mesh. Source nodes and decoded leaves/positions come from the bounded VCQTC reader. This implements the inspected quadrant traversal rather than the viewer's independent BVH.

## Contract

Root rectangle uses minX/minZ/maxX/maxZ, center `(min+max)*.5` and half extents `(max-min)*.5`. Original `b23630` stops for zero-fraction flag; ffff is empty, high-bit nodes select leaf offsets, other words select four consecutive child-node indices. Quadrant order is upper-Z/left-X, upper-Z/right-X, lower-Z/left-X, lower-Z/right-X.

Subdivision uses half differences with distinct min+half and max-half arithmetic, preserving their f32 rounding. For all four child culls, original uses the first child's half extents. It calculates every child's acceptance mask **before recursing into the first accepted child**. Leaf hits shorten the shared segment afterward; later masks of that parent are retained, while deeper culls use the shortened displacement. Original cached rejection plane persists across the query. The native stack pushes children in reverse to execute the same order.

Leaf arithmetic was factored into a private shared cursor; the public supplied-leaf API remains unchanged. Native finite/size/depth/visit/index guards are authored:≤64depth,≤1millionvisits,≤256triangles per leaf. The isolated original probe validates reachable nodes, leaf expansions and cycles before execution; original traversal itself has no such authored bounds.

## Controlled comparison

**2,964/2,964cases and8,277/8,277words exact debug/release**:759hits/2,205misses.2,940cases cover **every245Donington source chunks**, each with three centroid-normal casts, a far miss and eight root/corner/quartile casts;24authored cases include repeated/layer/continuation leaves under a four-child root. Outputs include hit/miss, triangle index, accumulated fraction, zero-stop flag, current four-lane displacement and resulting cull cache. Final evidence `local/behavior/packed-query-20261007-02/validation.json`, `03/validation.json`; initial31chunk/148case run01also matches.

The isolated probe calls **original root-cull `adb910` and whole traversal `b23630`**, with original subdivision `b19ff0`, leaf expansion, packed decoder and triangle primitive. The outer `b23800` shape wrapper is not executed in this comparison; root setup is authored from inspected behavior. No stubs or code patches; only isolated cull-cache page ec2000 writable/restored, code RX. No imports/TLS/entry/live injection. Probe SHA `4727660c4c41e25c9b1464f3614844911e833349a270c482d93f9a8144c75828`.

Supplied-leaf regression after cursor refactor remains148/148 /819hitwords exact debug/release (`leaf-query-20261007-05/`, `06/`). All82tests (47formats+35world), warnings-denied workspace Clippy and formatting pass, `packed-query-tests-20261007.txt`, `packed-query-clippy-20261007.txt`. Initial cyclic-node test point left the repeated shrinking quadrant, so culling correctly pruned it; moved the control to the shared corner to exercise the depth guard. Failure remains in `packed-query-initial-tests-20261007.txt`. Python compiles; existing pinned tools reused, no game launch/profile/settings changes.

Authored tools `grid-packed-query`, `probe_packed_query.c`, `build_packed_query_probe.cmd`, `validate_packed_query.py`. Private instructions `packed-traversal-instructions-20261007.json`, `packed-shape-query-instructions-20261007.json`, earlier quadrant decompilation `20261006T1416328260182Z/b19ff0.c.txt`. Additional original cross helper survey confirms ordinary XYZ cross with W=0; normal construction is next, not part of this result.

Complete shape normals/material tokens, nonzero shape transforms, original whole-world candidate dispatch/ties and grid population remain separate. [Production linkage](PRODUCTION_GRID.md) establishes all245captured track shape transforms are zero in that reference run, not a universal property of game shapes.
