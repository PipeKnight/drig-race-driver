# Shared-world tick performance

15 October 2026, Claude Code. Goal: a 12-car shared-world race tick fast enough for the
original's 60 fps (16.7 ms frames) with headroom, **without changing any simulation result**.
Scope: `crates/grid-world` and `crates/grid-tools` physics/world code only. AI driving,
track/route loading and the app were not changed.

## Result

The per-tick cost is now about one seventh of what it was without deformation, and under a
quarter with deformation. Results are bit-identical to the starting revision.

| Configuration (12 cars, Donington) | Before (32e747e) | After | Speed-up |
|---|---|---|---|
| Shared world, no deformation | 8.8 ms/tick | 1.3 ms/tick | ~6.9x |
| Shared world, every car deforming (windowed race) | 10.3 ms/tick | 2.2 ms/tick | ~4.6x |

Method: `grid-opponent-race ROOT CD01 BP01 12 3000 ai` with `GRID_AI_CHARACTERISTICS`
(and `GRID_DEFORMATION_PARAMETERS` for the deforming row). Process CPU time is pinned to the
P-cores (affinity 0xFFFF on the i7-12700, 8P+4E), and runs alternate before/after. The
2.1 s startup (a 1-tick run) is subtracted and the result divided by 3,000 ticks. The machine
was shared with several other sessions, so loads up to 100% made single runs vary by up to 2x.
The table uses the quiet interleaved rounds.

After the rebase onto `origin/main` cae12d9 (F305), the same harness ran with the machine at
99–100% load, so absolute values are higher but the ratios match:

| Configuration | cae12d9 | After rebase | Speed-up |
|---|---|---|---|
| Solo car, whole 2-lap race (11,018 ticks) | 0.91 ms/tick (10.3 s CPU) | 0.08 ms/tick (1.2 s CPU) | ~11x |
| 12 cars | 10.4 ms/tick | 1.7 ms/tick | ~6x |
| 12 deforming cars | 14.1 ms/tick | 3.0 ms/tick | ~4.7x |

The original's 60 fps frame budget is 16.7 ms. The shared world now uses about 8% of it
without deformation and 13% with deformation, single-threaded.

## Identity checks

Before/after binaries are built from identical `grid-opponent-race` sources, so the comparison
is like for like. The baseline is the starting revision with only the race tool patched. After each rebase it
is `origin/main` with only the race tool patched: cae12d9 (F305), then 44f1087 (F308, race
rules and visual damage) and db7dcaa (F309 driving assists, the final base). The race tool
gained two diagnostics:
- `GRID_DEFORMATION_PARAMETERS=<DF01>` runs every car with source deformation, as the windowed
  race does;
- a final `state carN hash=…` line per car: FNV-1a over all vehicle and physical words, plus
  the deformation bulk-call and record counters.

Outputs were compared byte for byte with `cmp` after each optimization step:

| Scenario | Checked values |
|---|---|
| Solo AI, Donington, 14,000 ticks | laps exactly 88.42029812185638 / 86.28228501935392 s, state hash a4a35f899a490c36 |
| 12 cars, Donington, 2 laps | 11,765 ticks, car_contacts=579, classification, pair contacts, all 12 state hashes |
| 12 deforming cars, Donington, 2 laps | 11,988 ticks, car_contacts=874, dents (bulk calls/records), all state hashes |
| 12 cars, Nürburgring `--route tracks/europe/nurburgring/route_0` (after the F304 rebase) | 14,000 ticks, car_contacts=5360, full output and all state hashes |

The full test suite passes on 3852824 (F310 tyre smoke, presentation only, over db7dcaa): 316 tests with `--include-ignored` and absolute
private paths, 0 failed. Clippy (`-D warnings`) passes too. This includes
the shared-world tests that compare the multi-car runtime word for word with the single-car
runtime, with and without deformation.

## Profiling method

No ETW profiler (samply, WPR/xperf) can run in this non-elevated session. A new
independently authored user-mode sampler, `tools/sample_profile.py` (see
[TOOLCHAIN](TOOLCHAIN.md)), does the job:
- it suspends each thread of the target about every millisecond;
- it walks the stack with the system DbgHelp `StackWalkEx` (inline frames from the PDB);
- it reports self/inclusive shares, call paths and per-source-line shares (`--lines-in`).

Builds for profiling use `CARGO_PROFILE_RELEASE_DEBUG=limited cargo build --release
--target-dir target/profile`. Reports stay in the session scratchpad and are not published.
DbgHelp labels the statically linked CRT `memcpy`/`memset` as `_NLG_Return2`.

## Profile before (12 cars, 4,000 ticks, share of the race thread)

| Inclusive | Path |
|---|---|
| 70% | `Track::body_contacts`: static track/prop/boundary contacts of each car body |
| 60% | `packed_contacts::calculate` per overlapping chunk |
| 34% | `packed_candidates::collect_decoded`, of which about 22% re-checked every chunk vertex for finiteness on every call |
| 9% | `collision_mesh::Mesh::world_bounds`, recomputed for every chunk with the same pose |
| 15% | `vehicle_manager` (suspension rays, 135 KB vehicle-state clones) |
| 8% | `Vec` clones of vehicle state |
| 1.4% | car-pair hull triangles |

A temporary counter build (since removed) found where the work was going. Per tick, the 12
cars made 2,952 candidate collections: each car's bounds overlapped about 245 track chunks,
because every chunk's root bounds span the whole circuit. They popped 23,034 tree nodes,
visited 445 leaf triangles and accepted only 6.4 triangles. They also ran 8,676 surface-sheet
lookups, each a linear key search. Nearly all of the body-contact time found nothing.

## Changes (all result-preserving)

1. **Static checks once** (`packed_candidates::mesh_valid`, `packed_contacts::check`). The
   per-call checks that depend only on immutable track data (every vertex finite and at most
   1e6, counts, root bounds, material indices below 8) run once per track chunk, the boundary
   and each prop box. Every check shares one error message, so evaluating them in a different
   order or earlier cannot change an outcome. `packed_contacts::Known` carries the body's
   `world_bounds` and the direction check across the queries of one body, which removes
   identical recomputation.
2. **Bit-set candidate stamps.** The collector deduplicates triangle IDs below 4096 with a
   512-byte bit set instead of zeroing and moving an 8 KB `[u16; 4096]` table per call. The
   public `Output::stamps` is rebuilt from the bit set and is identical. The `Known` query
   functions return only contacts.
3. **Provable chunk skip** (`Known::misses`).
   - `check` records the vertex bounds of every reachable triangle, but only when an unculled
     traversal of the chunk's tree meets no structural error (missing node, leaf, triangle or
     vertex; depth over 64; more than 1,000,000 visits). A culled traversal visits a subset
     of those nodes at the same depths, so it cannot fail either.
   - If every check passes and the body's bounds are more than 4 m beyond those triangle
     bounds on some axis, no triangle passes the collector's centred overlap test. Values are
     bounded by 1e6, where one f32 ulp is 0.0625, so 4 m covers the rounding.
   - Such a query therefore returns no contacts and no error. `Track::body_contacts` skips it,
     together with its surface-sheet lookups: `sheet` is now consulted only for shapes the body
     can reach, and the simulation's closure is pure.
   - A unit test (`known_queries_match_and_misses_are_empty`) compares the known and plain
     queries over a grid of poses and checks that every miss is empty.
4. **Prop box grid** (`entity_geometry::BoxIndex`). The 2,066 route prop boxes are bucketed in
   a 16 m x/z grid, and a query visits candidate boxes in ascending data order. A box whose
   bounds overlap the query is always a candidate; non-finite bounds cover the whole axis. The
   per-box AABB test, the capacity break and the order are unchanged. A unit test checks the
   superset property on random boxes, including NaN and infinite bounds.
5. **Wheel rays** (`packed_world::query_sequential`). These rays no longer build a ~300-entry
   shape list per ray and no longer run the O(n²) duplicate-ID check, which cannot fail for
   IDs 1..=n. Sequential IDs are looked up directly.
6. **Car pairs** (`body_geometry::WorldHull`). Each car's world hull triangles, vertex list,
   bounds, leaf tree and checks are built once per provider call, lazily at first use. The
   hull used to be rebuilt for every pair, about n²/2 times per call. Building lazily keeps
   any error at the same point.
7. **Fewer 135 KB state copies.**
   - The shared-world tick stages only the car, not each car's unused single-car world.
   - `vehicle_manager::calculate_with_world_{contacts,publication}_staged` and
     `vehicle_job::calculate_staged` take the staged copy by value, saving two clones per car
     per tick. The `&State` entry points are unchanged and still leave their input untouched
     on failure.
   - A deforming car copies its `BodyGeometry` only when a publication actually rebuilds it
     (`BodyGeometry::deformation_publication`).
   - The deformation registration's owner is swapped into the car and refilled with
     `clone_from`.
   - `deformation_world::calculate_group` clears and restores the owner's +C7F8 link in place,
     before any error returns, instead of cloning the owner.

## Profile after (12 cars)

The tick is now spread out:
- vehicle manager and suspension ray queries about 38%, still dominated by the remaining
  135 KB clones in `vehicle_handling`/`vehicle_publication`;
- the grouped physical frame about 14%;
- body contacts about 23%, mostly the prop grid and the few reachable chunks.

With deformation, `deformation_world`/`deforming_physical_frame` and their clones add about 25%.

## Not done / options

- **Parallel per-car stages.** The job part of the vehicle manager is independent per car. Only
  the wheel-contact pool offset depends on the cars before it, so jobs could run in parallel
  with an ordered commit. The workspace has no dependencies, and spawning scoped threads per
  tick (~20-50 µs each on Windows) costs as much as the work, so this would need a persistent
  worker pool. It is not needed for 60 fps now.
- **Remaining clones.** `vehicle_handling::calculate_owned`, `vehicle_publication`,
  `impact_frame` and the deforming frame still copy their inputs for their transactional
  "input unchanged on failure" contracts. Owned variants would need care because some read
  the original input after mutating the copy.
- **World copies.** `MultiRuntime::tick` still copies the shared world (`previous`) and its
  contact slots once per tick. That is cheap now compared with the rest of the tick.
