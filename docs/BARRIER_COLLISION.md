# Barrier collision on every race route (F653)

30 October 2026, Claude Code. Trigger: a second-hand report that on Le Mans the barrier
collisions did not work and the car flew through the fences (FIDELITY_BACKLOG row 99, F641).
Task: find every place where a car can pass through a trackside barrier on the race routes,
classify each against the original, and fix general defects at the root cause.

Evidence (ignored, this worktree's `local/barrier/`): `sweep6/` (static sweep before the
fix), `sweep7/` (after), `compare-6-7.txt`, `impact/lm2/` (Le Mans), `impact/all2/` (all
routes before), `impact/all3/` (after), `cross/` (the two contact-free crossings re-run),
`trees/` (PSSG dumps of every `objects.ens` and `objecttypes.pssg`), `basic-survey.tsv`,
`basic-per-route.txt`, `standins.txt`, `route-table.md`. Static data and headless runs only;
the original was not launched.

## 1. Result

- **Root cause [V/D]: the port never collided with the routes' basic entities.**
  `objects.ens` holds two kinds of placement: `TEMPLATEENTITYINSTANCE` (tyre walls, cones,
  brake boards, breakable stacks; read since F283) and `TEMPLATEBASICENTITYINSTANCE`, which
  `grid_formats::entities::read` skipped. The basic entities are the fixed barriers: concrete
  blocks and walls, chain-link and catch fences, armco runs (Le Mans `armco_02_group*`,
  Jarama `armco_ax5vara`, Nürburgring and Spa armco), Yokohama's tall fences, ad boards,
  buildings, bridges and signs. 69,014 placements on the 90 race routes; their templates
  carry 2,558 collision shapes, every one infinite-mass. Their meshes are drawn through
  `ornaments.bin`, so the port showed them and the car drove into them until the invisible
  boundary sheet behind stopped it (or nothing did, where the sheet is missing).
- **Fix**: `entities::read_basic` reads them with the same template machinery (reference →
  objecttypes template → initial behaviour → subentity rigid bodies and shapes), and
  `entity_geometry::Props::read_route` adds every non-phantom shape as a fixed box,
  cylinder or sphere after the entity boxes. This replaces F284's authored ornament
  stand-ins (mesh-extent boxes for the `concrete_`, `chnfen`, `gg_ad_brd_`, `don_ad_brd_`
  ornament families, FIDELITY_BACKLOG row 17): the source shapes cover the same barriers
  and all the others.
- **The boundary sheet held everywhere it exists**: no contact-free sheet crossing on Le
  Mans in 698 branches; two in 4,736 branches over all routes before the fix, both at about
  12-30 m/s, both now stopped earlier by a basic-entity fence (section 5.2). After the fix
  one case remains in 4,662 branches: a 4 m position jump with no body contact (section 5.3).

## 2. Original behaviour [D]

- PSSG factories: `TEMPLATEENTITYINSTANCE` (string 0xCFAE5C) → 0x7D9580 (allocates 0x100,
  ctor 0x7B6BF0, vtable 0xCFA13C, type 0xE7A8F0); `TEMPLATEBASICENTITYINSTANCE` (string
  0xCFB110) → 0x7D95B0 (0xA0, ctor 0x7B67B0, vtable 0xCFA0A4, type 0xE7ADA0). Both derive
  from the class registered at 0xC79F90 / 0xC7A560 (parent 0xF5F450).
- 0x7D97B0 (the level's element visitor) links each loaded entity instance into the world's
  list at +0xBBFC (next +0xC4) and each basic instance into the list at +0xBBCC (next +0x90).
- 0x7D7B00 (level load) walks both lists: each instance's template adds its +0x60 count to
  the world's object total (+0xBCA0), which sizes the runtime pool.
- 0x7D6BA0 (level setup) calls 0x7CB440 on every basic instance. 0x7CB440 creates the
  instance's body (0xA74190, stored at +0x84), walks the template's initial components and
  their rigid-body shapes, and builds each shape's collision primitive by kind (box through
  0xA72CF0, the world collision-set box constructor of JOINTS_RESEARCH.md; the other kinds
  through 0xA72B30, 0xA73C00, 0xA73490, 0xA732C0, 0xA730F0), adding it with 0xA7D300. The
  same function then prints the "too many +inf mass objects" warning, so these are the
  scene's fixed objects.
- Runtime code treats both kinds alike through the entity's +0x2C instance pointer: world
  transform 0x7BF630 (entity +0x40.., basic +0x30..), 0x7C7410, 0x7BF770, 0x927530
  (component lookup by name for either kind), and 0x7D8B80 (the cast to a basic instance).
- Not traced: whether a basic entity's shape damage behaviour (35 templates, e.g. Le Mans
  `armco_02_main_section` with `bArmco` and four constrained pieces, Yokohama planks and
  crates, Haruna signs) breaks in the original. The port keeps those shapes intact.
- F357's reading "static ornaments have no collision data" (COLLISION_GAPS_RESEARCH.md B)
  was right for the ornaments themselves; it missed that 836 of Donington's 1,985 are the
  drawn copies of basic entities. The stand-ins were the authored substitute for exactly
  these shapes.
- 0x96E430 (ornament shader selection, `object_barrier` shader) and the ornaments.bin
  prototype word +48 (far-draw list) are render-only and played no part.

## 3. Method

1. **Static sweep** (`grid-barrier-sweep`, new probe): the centreline of each route every
   4 m, horizontal rays to both sides at 0.4 m and 1.0 m over the road, up to 150 m,
   against a triangle copy of the port's body collision world: boundary sheet
   (`boundarylines.cqtc`), packed road triangles (`track.jpk`, wall-like when |n.y| < 0.6),
   route prop boxes (`Props::read_route`), and, for comparison only, the visible ornament
   boxes (closest-LOD mesh extent of each `ornaments.bin` placement, prototypes laid out over
   30 m skipped). Per sample: the nearest surface of each kind, whether the sheet faces the
   car, "open" (nothing solid within 150 m), "void" (≥ 3 m without road under the ray before
   the first barrier), visible ornaments standing ≥ 1 m in front of the first collision
   surface, and the sheet's free vertical edges.
2. **Dynamic check** (`grid-barrier-impact`, new probe): the app's collision world
   (`Runtime::load`, body events, body geometry, deformation, dynamic props) with the AI
   driver lapping the route at a speed cap; every 250 m (Le Mans 150 m) the run is cloned and,
   on each side, steered 60 m sideways at the entry speed for 400 ticks. Per branch: first
   contact, sheet/prop/road/piece contacts and depths, the mean sheet normal along the wall
   direction, a sheet crossing per tick (a step that crosses a sheet triangle 0.5 m up), a
   fall (no ground for 15 ticks, or 3 m under it), the final offset. Lacetti capped at 30 m/s
   and Audi R10 at 70 m/s on all 90 race routes (Okutama's six fail to load, row 82).
3. Each finding classified against the original data and code (section 5).

## 4. Before and after

Static sweep, all routes (`compare-6-7.txt`): of the side samples that reach any collision
before terrain, **45,356 now meet collision nearer** (a basic entity in front of the sheet),
and 1,589 farther (where an F284 stand-in mesh box was larger than the source shape: Le Mans
`concrete_wall_01_longer` +1.5 m, Haruna `concrete_block_combo` whose source boxes are
0.14 m and 0.69 m high, Yokohama `concrete_block_a_group_07`, Detroit stadium
`concrete_fence_white_end`). Examples of the gain: Le Mans 4,697 samples (armco groups),
Yokohama route 6 6,087 (tall fences), Jarama 642 (armco), Spa route 3 2,220 (armco).

Prop boxes per route (`props=`): Donington route 0 2,066 → 1,993 (1,021 entity + 972
basic), Le Mans route 0 2,510 → 3,009, Washington route 0 8,975 → 2,339
(the group-part stand-ins are gone), Jarama route 0 2,327 → 2,859. Every route stays under
the original's 10,000 fixed-object pool (AD9220); the largest is Yokohama route 0 with 4,258.

Dynamic run (`impact/all2` → `impact/all3`):

| | branches | sheet crossings | falls | errors |
|---|---:|---:|---:|---:|
| before | 4,736 | 2 | 17 | 3 |
| after | 4,662 | 1 | 4 | 1 |

Shibuya first failed to load with the basic entities: its bridge decks (`bridge_glass_c`,
315 m) exceeded the port's authored 200 m box-edge sanity bound, now 1 km (`MAX_EDGE`).
Yokohama's arenas (routes 3-6) have no AI waypoints and the probe's fallback driver stalls
before the first branch, so they are covered by the static sweep only.

After the fix: 13 of the 17 falls are gone (the Haruna ones meet guard rails, concrete blocks
and buildings). The remaining four: Istanbul Park route 2 1,318 m and Long Beach route 4
1,593 m (R10, both unchanged from before: through the road at speed, not a barrier), Yokohama
route 1 823 m right (Lacetti, after 7 sheet contacts, ends 4.6 m on the track side: not a
barrier pass, not traced), and the Le Mans jump of section 5.3. The body-event pool error
remains on Donington route 0 (R10). Per-route table: section 6.

## 5. Findings

### 5.1 Le Mans (the report)

- Sheet: continuous, 9,706 triangles, two free ends at the loop seam. 698 branches (Lacetti
  30, R10 70 m/s, both routes, 150 m spacing): no sheet crossing. Deepest case: R10 at
  62 m/s, route 0 2,366 m left, 2.25 m peak sheet penetration (deformation's contact limit
  lets the body crush), held.
- Before the fix the visible armco (`armco_02_group`, `_longer`, `_main_section`, ends)
  stood a median 1.1 m (max ~2.6 m) in front of the sheet with no collision of its own, so
  the car visibly drove into and through the armco before the sheet stopped it. This is the
  most likely reading of "flew through the fences" [I]; now the armco boxes stop it.
- Route 0 about 6,104 m right: the collision terrain ends about 3 m before the sheet
  (x < ~1724 at z 1360-1400); a car there drops out of the world without touching a wall.
  The location-level `track.jpk` does not cover it either: original data [O], not changed.

### 5.2 The two contact-free sheet crossings (before the fix)

- Spa route 3 557 m right, Lacetti: crossed an SHTC triangle facing the car at tick 86 with
  no contact before; first contact at tick 361 at 10.6 m/s.
- Washington route 3 2,615 m right, R10: crossed at tick 175 after an earlier sheet hit
  (5 contacts), about 12 m/s; the ground also ends there.
- After the fix both branches meet a basic-entity fence first (Spa
  `barrier_meshfence_white_long`, 0.17 m in front of the sheet; Washington 20 prop contacts)
  and no longer reach the sheet. The sheet-only mechanism is not reproduced [I: the sheet has
  no continuous collision, row 10, so a thin corner of the hull can step through it].

### 5.3 A position jump through the sheet (after the fix)

Le Mans route 0, R10, branch at 8,689 m, right (reproduced with `GRID_BARRIER_TRACE=8689`,
`local/barrier/cross/lm0full.txt`). The car slides parallel to the sheet near 8,850 m at
about 33 m/s with no body contact (no sheet, prop or road contact, no pieces). Between ticks
194 and 195 its position jumps 4.2 m against its velocity, (729.56, 2509.95) → (733.52,
2507.81), across sheet triangle 8679 (a 150 m tall SHTC panel), and the velocity changes by
19 m/s in x in that tick. It then rolls on behind the sheet and falls when the ground ends.
No body contact explains the jump [O]; the trace does not record the wheels, so a wheel or
suspension contact against the near-vertical sheet is the leading candidate [I]. Not
traced further; one case in 4,600+ branches.

### 5.4 Other findings

| Finding | Where | Class | Status |
|---|---|---|---|
| Basic entities (fences, armco, walls, buildings) not solid | every route | port defect | **fixed** |
| Visible ornaments with no collision of their own (ad boards, lamp posts, traffic lights, trucks, containers) | all, mostly behind the sheet | original data: ornaments without an entity have no collision [D] | as original |
| Hillside without a sheet (10 of the 17 falls before) | Haruna touge routes | the barriers there are basic entities (guard rails, blocks, buildings) | **fixed** with them (no Haruna fall after) |
| Collision terrain ends before the sheet | Le Mans route 0 ~6,104 m right, Washington route 3 ~2,615 m | original data [O] | as original |
| Static "away" flags (sheet faces away from the car) | Yokohama, Donington | not a defect: the sheet contact normal always pushes back to the track (measured mean normal ≈ −1.0) | none |
| Deep crush into the sheet at high speed (2.25 m at 62 m/s) | Le Mans | port: deformation contact limit, fixed 16 ms step | row 10 |
| Body event pool overflow ("unsupported body event pool or query ownership") | Donington routes 0/2 and Milan route 3, R10 | port robustness defect | open follow-up |
| Okutama does not load | 6 routes | port reader (row 82) | open |
| Basic-entity damage behaviours (breakable armco, planks, crates, signs) | 35 templates | not ported: intact fixed shapes | open |

## 6. Per-route table

Before = F284 stand-ins (sweep6, impact all2); after = basic entities (sweep7, impact
all3). "Samples nearer / farther": side samples (4 m, two heights) whose first collision
moved more than 0.5 m. Open: samples with nothing solid within 150 m; void: a ≥ 3 m gap
without road under the ray before the first barrier (after). Branch columns:
branches/sheet crossings/falls/errors.

| Route | Length m | Props before → after | Samples nearer / farther | Open | Void | Lacetti 30 before → after (branches/through/fell/err) | R10 70 before → after |
|---|---:|---:|---:|---:|---:|---|---|
| europe/donington/route_0 | 4002 | 2066 → 1993 | 29 / 88 | 0 | 0 | 32/0/0/0 → 32/0/0/0 | 32/0/0/1 → 32/0/0/1 |
| europe/donington/route_1 | 3188 | 1895 → 1816 | 27 / 54 | 0 | 1 | 26/0/0/0 → 26/0/0/0 | 26/0/0/0 → 26/0/0/0 |
| europe/donington/route_2 | 3995 | 2083 → 2010 | 34 / 84 | 0 | 0 | 32/0/0/0 → 32/0/0/0 | 32/0/0/1 → 32/0/0/0 |
| europe/donington/route_3 | 3184 | 1912 → 1833 | 21 / 58 | 0 | 0 | 26/0/0/0 → 26/0/0/0 | 26/0/0/0 → 26/0/0/0 |
| europe/istanbul_park/route_0 | 5330 | 1689 → 2049 | 146 / 0 | 16 | 7 | 42/0/0/0 → 42/0/0/0 | 42/0/0/0 → 42/0/0/0 |
| europe/istanbul_park/route_1 | 3919 | 1629 → 1922 | 177 / 0 | 12 | 0 | 32/0/0/0 → 32/0/0/0 | 32/0/0/0 → 32/0/0/0 |
| europe/istanbul_park/route_2 | 5330 | 1689 → 2049 | 143 / 0 | 13 | 9 | 42/0/0/0 → 42/0/0/0 | 18/0/1/0 → 42/0/1/0 |
| europe/istanbul_park/route_3 | 3916 | 1629 → 1922 | 173 / 0 | 10 | 1 | 32/0/0/0 → 32/0/0/0 | 32/0/0/0 → 32/0/0/0 |
| europe/jarama/route_0 | 3896 | 2327 → 2859 | 642 / 0 | 0 | 0 | 32/0/0/0 → 32/0/0/0 | 32/0/0/0 → 32/0/0/0 |
| europe/jarama/route_1 | 3896 | 2283 → 2815 | 643 / 0 | 0 | 0 | 32/0/0/0 → 32/0/0/0 | 32/0/0/0 → 32/0/0/0 |
| europe/le_mans/route_0 | 13624 | 2510 → 3009 | 4697 / 171 | 1 | 0 | 108/0/0/0 → 108/0/0/0 | 106/0/2/0 → 106/1/1/0 |
| europe/le_mans/route_1 | 13553 | 2537 → 2954 | 4762 / 162 | 3 | 0 | 108/0/0/0 → 108/0/0/0 | 108/0/0/0 → 108/0/0/0 |
| europe/milan/route_0 | 4869 | 2340 → 2296 | 43 / 20 | 0 | 0 | 38/0/0/0 → 38/0/0/0 | 38/0/0/0 → 38/0/0/0 |
| europe/milan/route_1 | 3589 | 1857 → 1887 | 44 / 3 | 0 | 0 | 28/0/0/0 → 28/0/0/0 | 28/0/0/0 → 28/0/0/0 |
| europe/milan/route_2 | 1497 | 838 → 800 | 32 / 13 | 0 | 0 | 12/0/0/0 → 12/0/0/0 | 12/0/0/0 → 12/0/0/0 |
| europe/milan/route_3 | 4868 | 2342 → 2295 | 50 / 20 | 0 | 0 | 38/0/0/0 → 38/0/0/0 | 38/0/0/1 → 38/0/0/0 |
| europe/milan/route_4 | 3583 | 1857 → 1887 | 45 / 5 | 0 | 0 | 28/0/0/0 → 28/0/0/0 | 28/0/0/0 → 28/0/0/0 |
| europe/milan/route_5 | 1507 | 840 → 800 | 30 / 10 | 0 | 0 | 12/0/0/0 → 12/0/0/0 | 12/0/0/0 → 12/0/0/0 |
| europe/nurburgring/route_0 | 5144 | 2919 → 3420 | 224 / 1 | 4 | 0 | 40/0/0/0 → 40/0/0/0 | 40/0/0/0 → 40/0/0/0 |
| europe/nurburgring/route_1 | 3621 | 3005 → 3504 | 205 / 0 | 5 | 0 | 28/0/0/0 → 28/0/0/0 | 28/0/0/0 → 28/0/0/0 |
| europe/nurburgring/route_2 | 5143 | 2897 → 3398 | 237 / 0 | 3 | 0 | 40/0/0/0 → 40/0/0/0 | 40/0/0/0 → 40/0/0/0 |
| europe/nurburgring/route_3 | 3625 | 3000 → 3499 | 200 / 0 | 3 | 0 | 28/0/0/0 → 28/0/0/0 | 28/0/0/0 → 28/0/0/0 |
| europe/spa_francorchamps/route_0 | 6986 | 1552 → 2239 | 2218 / 0 | 3 | 0 | 56/0/0/0 → 56/0/0/0 | 56/0/1/0 → 56/0/0/0 |
| europe/spa_francorchamps/route_1 | 6987 | 1552 → 2241 | 2212 / 0 | 3 | 2 | 56/0/0/0 → 56/0/0/0 | 56/0/0/0 → 56/0/0/0 |
| europe/spa_francorchamps/route_2 | 6995 | 1547 → 2234 | 2228 / 0 | 2 | 0 | 56/0/0/0 → 56/0/0/0 | 56/0/1/0 → 56/0/0/0 |
| europe/spa_francorchamps/route_3 | 6987 | 1547 → 2238 | 2220 / 0 | 1 | 0 | 56/1/0/0 → 56/0/0/0 | 56/0/0/0 → 32/0/0/0 |
| japan/haruna/route_0 | 9260 | 2168 → 3201 | 281 / 21 | 309 | 1002 | 24/0/0/0 → 24/0/0/0 | 24/0/0/0 → 24/0/0/0 |
| japan/haruna/route_1 | 8999 | 2034 → 3164 | 278 / 11 | 287 | 646 | 48/0/0/0 → 48/0/0/0 | 48/0/0/0 → 48/0/0/0 |
| japan/haruna/route_2 | 8999 | 2036 → 3176 | 278 / 11 | 287 | 646 | 48/0/0/0 → 48/0/0/0 | 48/0/0/0 → 48/0/0/0 |
| japan/haruna/route_3 | 9074 | 2662 → 3731 | 285 / 14 | 113 | 543 | 52/0/1/0 → 52/0/0/0 | 52/0/1/0 → 52/0/0/0 |
| japan/haruna/route_4 | 9074 | 2662 → 3716 | 284 / 14 | 113 | 543 | 52/0/1/0 → 52/0/0/0 | 52/0/1/0 → 52/0/0/0 |
| japan/haruna/route_5 | 8817 | 1641 → 2648 | 268 / 15 | 347 | 844 | 30/0/1/0 → 30/0/0/0 | 30/0/1/0 → 30/0/0/0 |
| japan/haruna/route_6 | 8817 | 1639 → 2640 | 268 / 15 | 347 | 844 | 30/0/1/0 → 30/0/0/0 | 30/0/0/0 → 30/0/0/0 |
| japan/haruna/route_7 | 8999 | 1395 → 2451 | 193 / 12 | 485 | 1124 | 0/0/0/0 → 0/0/0/0 | 0/0/0/0 → 0/0/0/0 |
| japan/haruna/route_8 | 8999 | 1395 → 2457 | 193 / 12 | 485 | 1124 | 0/0/0/0 → 0/0/0/0 | 0/0/0/0 → 0/0/0/0 |
| japan/haruna/route_9 | 9258 | 3060 → 4226 | 284 / 11 | 51 | 338 | 70/0/1/0 → 70/0/0/0 | 70/0/2/0 → 70/0/0/0 |
| japan/okutama/route_0 | — | — | — | — | — | does not load (row 82) | |
| japan/okutama/route_1 | — | — | — | — | — | does not load (row 82) | |
| japan/okutama/route_2 | — | — | — | — | — | does not load (row 82) | |
| japan/okutama/route_3 | — | — | — | — | — | does not load (row 82) | |
| japan/okutama/route_4 | — | — | — | — | — | does not load (row 82) | |
| japan/okutama/route_5 | — | — | — | — | — | does not load (row 82) | |
| japan/shibuya/route_0 | 2060 | 1250 → 2924 | 7 / 69 | 0 | 0 | 16/0/0/0 → 16/0/0/0 | 16/0/0/0 → 16/0/0/0 |
| japan/shibuya/route_1 | 2043 | 1250 → 2916 | 9 / 82 | 0 | 0 | 16/0/0/0 → 16/0/0/0 | 6/0/0/0 → 4/0/0/0 |
| japan/shibuya/route_2 | 1339 | 844 → 1987 | 7 / 44 | 0 | 0 | 10/0/0/0 → 10/0/0/0 | 10/0/0/0 → 10/0/0/0 |
| japan/shibuya/route_3 | 1336 | 844 → 1981 | 9 / 68 | 0 | 0 | 10/0/0/0 → 10/0/0/0 | 10/0/0/0 → 10/0/0/0 |
| japan/shibuya/route_4 | 1884 | 1119 → 2792 | 10 / 51 | 0 | 0 | 16/0/0/0 → 16/0/0/0 | 16/0/0/0 → 16/0/0/0 |
| japan/shibuya/route_5 | 1893 | 1119 → 2789 | 5 / 42 | 0 | 0 | 16/0/0/0 → 16/0/0/0 | 16/0/0/0 → 16/0/0/0 |
| japan/shibuya/route_6 | 1151 | 702 → 1695 | 15 / 47 | 0 | 0 | 10/0/0/0 → 10/0/0/0 | 10/0/0/0 → 10/0/0/0 |
| japan/shibuya/route_7 | 1152 | 702 → 1692 | 8 / 42 | 0 | 0 | 10/0/0/0 → 10/0/0/0 | 10/0/0/0 → 10/0/0/0 |
| japan/yokohama_docks/route_0 | 2528 | 1673 → 4258 | 1335 / 0 | 0 | 0 | 12/0/0/0 → 12/0/0/0 | 10/0/0/0 → 10/0/0/0 |
| japan/yokohama_docks/route_1 | 2368 | 1299 → 4160 | 1360 / 22 | 0 | 0 | 14/0/0/0 → 14/0/1/0 | 14/0/0/0 → 14/0/0/0 |
| japan/yokohama_docks/route_2 | 1524 | 1092 → 3216 | 906 / 1 | 0 | 0 | 12/0/0/0 → 12/0/0/0 | 12/0/0/0 → 12/0/0/0 |
| japan/yokohama_docks/route_3 | 3043 | 429 → 2044 | 2159 / 9 | 59 | 0 | 0/0/0/0 → 0/0/0/0 | 0/0/0/0 → 0/0/0/0 |
| japan/yokohama_docks/route_4 | 2515 | 509 → 2270 | 1220 / 4 | 25 | 0 | 0/0/0/0 → 0/0/0/0 | 0/0/0/0 → 0/0/0/0 |
| japan/yokohama_docks/route_5 | 2864 | 535 → 2475 | 1501 / 7 | 52 | 0 | 2/0/0/0 → 2/0/0/0 | 2/0/0/0 → 2/0/0/0 |
| japan/yokohama_docks/route_6 | 9900 | 616 → 3306 | 6087 / 35 | 286 | 0 | 0/0/0/0 → 0/0/0/0 | 0/0/0/0 → 0/0/0/0 |
| japan/yokohama_docks/route_7 | 2517 | 1619 → 4114 | 1347 / 0 | 0 | 0 | 20/0/0/0 → 20/0/0/0 | 20/0/0/0 → 14/0/0/0 |
| japan/yokohama_docks/route_8 | 2365 | 1355 → 4242 | 1339 / 13 | 0 | 0 | 18/0/0/0 → 18/0/0/0 | 18/0/0/0 → 18/0/0/0 |
| japan/yokohama_docks/route_9 | 1525 | 1216 → 3410 | 912 / 1 | 0 | 0 | 6/0/0/0 → 4/0/0/0 | 12/0/0/0 → 12/0/0/0 |
| usa/detroit/route_0 | 4560 | 2159 → 3484 | 285 / 3 | 0 | 0 | 36/0/0/0 → 36/0/0/0 | 36/0/0/0 → 36/0/0/0 |
| usa/detroit/route_1 | 3655 | 1757 → 2948 | 267 / 7 | 1 | 0 | 30/0/0/0 → 30/0/0/0 | 30/0/0/0 → 30/0/0/0 |
| usa/detroit/route_2 | 2753 | 1188 → 1959 | 40 / 2 | 0 | 0 | 22/0/0/0 → 22/0/0/0 | 22/0/0/0 → 22/0/0/0 |
| usa/detroit/route_3 | 1499 | 624 → 1591 | 0 / 2 | 0 | 0 | 12/0/0/0 → 12/0/0/0 | 12/0/0/0 → 12/0/0/0 |
| usa/detroit/route_4 | 4555 | 2156 → 3507 | 271 / 6 | 0 | 0 | 36/0/0/0 → 36/0/0/0 | 36/0/0/0 → 36/0/0/0 |
| usa/detroit/route_5 | 3655 | 1728 → 2948 | 282 / 3 | 0 | 0 | 30/0/0/0 → 30/0/0/0 | 30/0/0/0 → 30/0/0/0 |
| usa/detroit/route_6 | 2753 | 1169 → 1928 | 43 / 4 | 2 | 0 | 22/0/0/0 → 22/0/0/0 | 22/0/0/0 → 22/0/0/0 |
| usa/detroit/route_7 | 1496 | 668 → 1641 | 0 / 0 | 0 | 0 | 12/0/0/0 → 12/0/0/0 | 12/0/0/0 → 12/0/0/0 |
| usa/detroit/route_8 | 970 | 611 → 1664 | 0 / 25 | 0 | 0 | 8/0/0/0 → 8/0/0/0 | 8/0/0/0 → 8/0/0/0 |
| usa/detroit/route_9 | 829 | 633 → 1628 | 0 / 17 | 0 | 0 | 4/0/0/0 → 4/0/0/0 | 4/0/0/0 → 4/0/0/0 |
| usa/long_beach/route_0 | 3653 | 1325 → 2796 | 0 / 4 | 0 | 0 | 30/0/0/0 → 30/0/0/0 | 30/0/0/0 → 30/0/0/0 |
| usa/long_beach/route_1 | 3828 | 1491 → 3062 | 0 / 3 | 0 | 0 | 30/0/0/0 → 30/0/0/0 | 30/0/0/0 → 30/0/0/0 |
| usa/long_beach/route_2 | 1315 | 554 → 1072 | 0 / 2 | 0 | 0 | 10/0/0/0 → 10/0/0/0 | 10/0/0/0 → 10/0/0/0 |
| usa/long_beach/route_3 | 3654 | 1381 → 2852 | 0 / 4 | 0 | 0 | 28/0/0/0 → 28/0/0/0 | 28/0/0/0 → 28/0/0/0 |
| usa/long_beach/route_4 | 3826 | 1575 → 3137 | 0 / 5 | 0 | 0 | 30/0/0/0 → 30/0/0/0 | 30/0/1/0 → 30/0/1/0 |
| usa/long_beach/route_5 | 1318 | 554 → 1072 | 0 / 1 | 0 | 0 | 10/0/0/0 → 10/0/0/0 | 10/0/0/0 → 10/0/0/0 |
| usa/san_fran/route_0 | 5291 | 1309 → 2270 | 2 / 9 | 0 | 0 | 42/0/0/0 → 42/0/0/0 | 42/0/0/0 → 42/0/0/0 |
| usa/san_fran/route_1 | 3793 | 1293 → 1832 | 9 / 7 | 0 | 0 | 30/0/0/0 → 30/0/0/0 | 30/0/0/0 → 10/0/0/0 |
| usa/san_fran/route_2 | 2924 | 1340 → 1638 | 0 / 7 | 0 | 0 | 24/0/0/0 → 24/0/0/0 | 24/0/0/0 → 24/0/0/0 |
| usa/san_fran/route_4 | 5284 | 1310 → 2245 | 2 / 12 | 0 | 0 | 42/0/0/0 → 42/0/0/0 | 4/0/0/0 → 4/0/0/0 |
| usa/san_fran/route_5 | 3787 | 1285 → 1813 | 5 / 7 | 2 | 0 | 30/0/0/0 → 30/0/0/0 | 30/0/0/0 → 30/0/0/0 |
| usa/san_fran/route_6 | 2922 | 1349 → 1627 | 0 / 9 | 1 | 0 | 24/0/0/0 → 24/0/0/0 | 24/0/0/0 → 24/0/0/0 |
| usa/washington/route_0 | 5265 | 8975 → 2339 | 169 / 11 | 0 | 0 | 42/0/0/0 → 42/0/0/0 | 22/0/0/0 → 42/0/0/0 |
| usa/washington/route_1 | 2872 | 1053 → 1060 | 0 / 8 | 1 | 0 | 22/0/0/0 → 22/0/0/0 | 22/0/0/0 → 22/0/0/0 |
| usa/washington/route_2 | 4329 | 8571 → 2062 | 569 / 7 | 0 | 0 | 34/0/0/0 → 34/0/0/0 | 34/0/0/0 → 18/0/0/0 |
| usa/washington/route_3 | 3380 | 8429 → 1841 | 169 / 20 | 1 | 0 | 26/0/0/0 → 26/0/0/0 | 26/1/1/0 → 26/0/0/0 |
| usa/washington/route_4 | 1647 | 2410 → 708 | 0 / 0 | 1 | 0 | 14/0/0/0 → 14/0/0/0 | 14/0/0/0 → 14/0/0/0 |
| usa/washington/route_5 | 5247 | 9013 → 2416 | 170 / 13 | 1 | 0 | 42/0/0/0 → 42/0/0/0 | 42/0/0/0 → 20/0/0/0 |
| usa/washington/route_6 | 2858 | 1029 → 1063 | 0 / 6 | 0 | 0 | 22/0/0/0 → 22/0/0/0 | 22/0/0/0 → 22/0/0/0 |
| usa/washington/route_7 | 4324 | 8553 → 2063 | 573 / 5 | 1 | 0 | 34/0/0/0 → 34/0/0/0 | 16/0/0/0 → 16/0/0/0 |
| usa/washington/route_8 | 3376 | 8432 → 1861 | 169 / 16 | 0 | 0 | 26/0/0/0 → 26/0/0/0 | 26/0/0/0 → 4/0/0/0 |
| usa/washington/route_9 | 1643 | 2422 → 709 | 1 / 2 | 1 | 0 | 14/0/0/0 → 14/0/0/0 | 14/0/0/0 → 14/0/0/0 |

## 7. Files

- `crates/grid-formats/src/entities.rs`: `read_basic` (shared `read_kind`).
- `crates/grid-tools/src/entity_geometry.rs`: basic boxes in `Props::read_route`;
  the stand-in code (`ornament_barriers`, `barrier_extent`, `barrier_parts`,
  `barrier_meshes`) removed; tests `donington_route_props_are_solid_boxes` (updated counts)
  and `basic_entities_leave_every_racing_line_open` (every circuit; replaces the Washington
  group test).
- `crates/grid-tools/src/dynamic_props.rs`: the spawn-press diagnostic counts basic boxes;
  `no_piece_spawns_pressed_down_by_a_basic_entity` (was `..._barrier_stand_in`).
- Probes: `crates/grid-probes/src/bin/grid-barrier-sweep.rs`, `grid-barrier-impact.rs`.
