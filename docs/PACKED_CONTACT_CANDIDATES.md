# Packed body-contact candidate collection (F233)

Rust now reconstructs the cold candidate stage feeding the Lacetti/Donington
body-contact generator. This is the prerequisite to using fresh Rust witnesses
instead of replaying original contact records. Driving-app barriers remain open.

## Selected behavior

`collision_mesh::Mesh::world_bounds` shares the existing original-order kind-7
four-accumulator transformed bounds reduction and margin expansion.
`packed_candidates::collect` implements the selected untransformed packed tree:

- Inclusive X/Z rectangle culling, ordered four-child descent and leaf order.
- Full triangle/box separating-axis overlap: coordinates, normalized plane and
  nine edge-cross-axis tests, preserving selected floating-point grouping.
- Cold generation-one stamps for triangle indices below 4,096, marked before
  overlap. Larger indices use the bounded visited list for deduplication.
- Rejected triangles enter the visited store; traversal and stamping continue
  after either store fills. Zero candidate capacity can still change stamps.
- The whole caller skips collection when contact or cache capacity is zero.

The library admits bounded finite geometry, finite ordered bounds, a valid tree
within depth/visit limits and at most 32,768 stored references. These are owned
safety bounds. The selected source comparison does not establish every possible
floating-point boundary or malformed original-game behavior.

`grid-packed-candidates` independently rebuilds the moving source mesh and reads
the purchased packed chunk from PC08 input. It consumes no native answer. Its
private output contains query bounds, accepted/visited counts, all 4,096 stamps,
generation/cache-use fields and both complete bounded arrays, including unused
slots. Source geometry and diagnostic outputs stay under ignored `local/`.

## Validation

Both actual Rust debug and release executables match all **2,642** reconstructed
F229 source/policy/capacity cases, byte for byte against the pinned complete
original and repeat captures. Per profile: **105,984,800 compared bytes**, 4,944
accepted references, 26,544 stored visits and 27,512 marked stamps summed across
cases. These are candidate counts, not contact counts.

The source/input/current-binary audit independently checks **10,568 complete
files** and 24 guard records. This primary comparison reuses original captures;
it does not claim a fresh execution of the packed generator. The previous F232
current-source native PC08 regression already reproduced 20 spaced whole cases.

The shared bounds refactor additionally passes fresh whole-original/repeat/Rust
attachment comparisons for all 180 source/authored cases in each build. Fresh
two-shape overlap regressions also pass all 720 cases per build (360 allocated
pairs each). The four regression reports audit 5,400 complete output files and
244 guard records across both builds.
The library suite has 181 regular tests passing; two source driving tests remain
default-ignored and were last explicitly run in F230. All-target warnings-denied
Clippy and formatting pass. Synthetic tests cover touching/separating triangles,
rejected visits, full/zero stores, high-index deduplication and bounded cycles.

Private primary records:

- `local/behavior/packed-candidates-debug-20261012-final-02/validation.json`,
  SHA-256 `2f1e04d6898aeb46f0bb01fe1cf3bf9e79ec6673f6096c628d8de45196a033d4`.
- Release counterpart SHA-256
  `9bda88cf614c973c6fbecdc4c42956ef73b1b435a9330e9be650055f627715ca`.
- `local/behavior/packed-candidates-audit-20261012-final-01.json`.
- Current collector debug SHA-256
  `5365f06ceb77d5ba74c1f52562e043c0a8d5fd860611d91d871c837d96cab2ae`;
  release `3e04821d588f76f9cd33663a1eaf211a75b3343ddf0cb7dc835593de18ad143e`.

Installed tools reused: Rust/Cargo 1.99.0, Python 3.12.14, MSVC 14.44.35207
(DevPrompt 17.14.41, SDK 10.0.26100.0), Ghidra 12.1.4 and JDK 21.0.9.
Reference image SHA-256
`aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514`;
Steam executable
`dd7b0933abf0d421bec3fa6f123319d3d3cc7221e5ef82b7a1201c3d1979e48f`.
All 130 tracked Steam resources and the executable remain unchanged at
2026-10-12 19:00:45.584932 UTC. No install, desktop input or settings change.

One synthetic cycle assertion initially used a query whose rectangle culled the
cycle before recursion. Expanding that fixture to the complete root tests the
intended depth guard. No source numeric mismatch or native fault occurred.

## Backface branch and next integration

Original B1AB30 checks packed bounds at 294/298/29C/2A0. The F228 constructor and
selected F229 descriptor initialize all four words to maximum finite float.
No finite ordinary point meets the lower X bound, so the nested B225E0 vertical
correction is inactive **in this fixture**. This is an observation about the
selected constructor state; later resource initialization or production extents
have not been recovered. A fresh generator must require that state explicitly,
or implement the additional correction before admitting other states.

Private surveys: `20261012T1842359328417Z` (extent/vertical correction),
`20261012T1844263152799Z` (cache/traversal) and
`20261012T1846038650006Z` (triangle overlap/quadrants), under `local/behavior/`.
Only independent authored Rust/scripts and this behavioral description are public.

Next compose collected triangle order with the retained `Context::contact`,
witness-derived depth/normal, [orientation and retained manifold](ORIENTED_TRIANGLE_MANIFOLD.md),
then [mixed physical contact solving](CARRIED_GEOMETRIC_PHYSICS.md). Full material
factories, populated correction bounds, moving-frame publication and runtime
barriers are separate outstanding work.
