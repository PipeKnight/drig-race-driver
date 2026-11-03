# Floating and untextured track pieces: the all-route audit (row 102)

Status: 2026-11-02, Claude Code (session fervent-napier). Owner's request (F657): on Washington
some pieces hang in the air and part of a kerb seems slightly above the road. Every track should
have no floating elements and no missing textures, checked automatically where possible.

## The check

`grid-app --route R --scene donington --float-audit local/OUT.tsv` assembles the route the way
the playable draws it and checks every drawn part against the track collision (`track.jpk`)
below it, then exits. `sh tools/float_audit_all.sh [OUT]` runs it over every shipped route folder
that has a `track.jpk`, with one TSV and log per route and a summary line each
(default `local/float-audit/summary.txt`).

- Objects and entities (materials `objects/...`): each drawn instance's base, which is its lowest
  referenced vertices (within 2 cm of the lowest), against the collision below (a ray from 1 m
  above down to 30 m below). It is also checked against the drawn track scene and route split
  below. An instance is *held* when its base is within 5 cm of either surface, or below it while
  its top is above it (a post, or a fence's lower half). An instance that touches a held instance
  is held too: one of its vertices lies within 5 cm of a triangle of the other. This makes a sign
  on its pole, or a fence top on its sunk lower half, held.
- The other instances are reported as `floating` (base above the surface), `sunk` (entirely
  below it) or `no_ground` (no collision within 30 m below), with their gap and size.
- Damage pieces (`dynamic/piece/...`) are skipped. They stay hidden until their prop breaks, and
  then they follow their physics bodies, so their rest pose is not a placement.
- The route split (`route/...`) and the track scene: every vertex more than 5 cm and less than
  1 m above the collision is counted. Route-split vertices are listed as `route_vertex` rows.
- `untextured` rows: a drawn material with no texture binding, or bound to a texture that is not
  loaded.

The TSV columns are `kind part material drawn_gap x y z gap size`. Source:
`crates/grid-app/src/float_audit.rs`.

## Results (all routes, 2026-11-02)

- No route has an untextured material (`untextured_materials=0` on every route).
- The 5 frontend garage folders have no track files and are skipped.
- 92 routes: 579,194 instances, 506,948 held, 18,823 floating, 34 sunk, 53,389 no_ground (most outside the collision bounds). Per-route totals are in the summary file. Representative routes:

| route | instances | held | floating | sunk | no_ground | route vertices raised |
|---|---|---|---|---|---|---|
| Washington 0 | 7050 | 5776 | 357 | 0 | 917 | 2669 of 47964 |
| Okutama 0 | 4622 | 4156 | 253 | 0 | 213 | 3138 of 199946 |
| Donington 0 | 8987 | 8194 | 282 | 0 | 511 | 0 of 400 |

## Triage

The port places objects and entities by composing the data's own transforms: the `objects.ens`
placement, then the PSSG node chain up to the renderable root. Kerb and road meshes and the
collision come unchanged from the route's own files. A gap that comes out the same for every
instance of a type, and agrees with the data's transforms, is therefore in the original data. A
port placement bug would show as a constant offset that the data's transforms do not explain.

1. **Audit false positive, fixed in the audit: hidden damage pieces.** In the first all-route
   run, 13,209 instances were reported floating. The largest groups were Armco damage rails such
   as `armco_main_section_large_a_01_skin` (Okutama 0: 237 floating at about +0.36 m, and 414
   sunk at about -0.32 m). The skinned draw chain is exact at the bind pose: the
   INVERSEBINDMATRIX translations equal the negated joint world positions, for example
   `link_constrain_post` (0.045, 0.304). So the rails sit at their bind-space height, mounted on
   the posts 0.30-0.46 m up. The intact mesh of the same entity has its posts set 0.32 m into the
   ground, as authored. The pieces are `Visibility::Hidden` until the prop breaks, so the audit
   now skips them.
   The first all-route run also missed tops of objects set into the ground. Detroit's
   `concrete_fence_white_end_01` (285 at 0.92 m) is the upper half of a fence whose lower half
   `concrete_fence_white_end_02` is set into the ground. Partly sunk instances now hold what touches them,
   which brought the floating total from 33,800 to 18,823. Objects that stand on raised pavement
   or kerbs drawn as objects remain listed although they rest visibly: Milan's
   `gg_vehicle_parked_cars_group_06_01`, 0.15 m over the collision
   (`local/r39/crops/milan0-parked.png`).
2. **Authored, as in the original: structures above the ground.** Most of what still floats is
   mounted on something the contact test does not join. The main groups:
   - sponsor boards on scaffolds (`sponsor_01/02/event`, 1-12 m);
   - speakers and lamps on poles (`speaker_a_02` 4.3 m, `gg_electric_speaker_01_b_01` 4.3 m);
   - start lights (4.8 m);
   - grandstand and tent sub-meshes that rest on other sub-meshes 1-3 m up;
   - traffic signs (about 1.9 m);
   - bridge decks (3.7 m).

   Their gaps are consistent within each type and follow the data's transforms.
3. **Authored, as in the original: Washington's raised pavement (the likely "kerb").**
   - About 2,300 of the 2,669 raised route-split vertices on Washington 0 sit 0.10-0.15 m above
     the collision.
   - In the densest group, the route split is flat at y 8.165 over collision at 8.065, exactly
     0.100 m, near (384, 44). The render
     (`local/r39/crops/w0-kerb-380-40.png`) shows the street pavement and its curb. The
     collision there is flat (the step is visual only).
   - Both meshes come unchanged from the original's `routesplit.pssg` and `track.jpk`.
   - The same 0.10 m step appears on the other Washington materials (`track4_vara/4_varb`,
     `ID10 to 1`, `ID4 to 3`).
   - The `feature1` groups at 0.5-0.7 m are the side faces of the concrete barrier blocks, which
     stand on the road (`local/r39/crops/w0-feature1.png`). The vertex count includes upright
     geometry like this.
4. **Open, needs the original to compare: bunting without ropes.** On Okutama, `bunting_a_01!0`
   pennants (5-15 m) hang in a line with no rope drawn between them
   (`local/r39/crops/oku0-bunting.png`). The object has only two ripple-animated shaders,
   `bunting_a_01` and `bunting_a_01!0`, and no separate rope draw. Either the rope is in the
   texture (and lost to alpha or mips here), or the original also shows none. This was not
   resolved.

No port placement bug (transform, pivot, LOD or parent offset) was found among the checked
groups, so nothing in the shared placement path was changed. The owner's runtime "pieces in the
air" are not identified. Candidates are the bunting, and broken pieces in a race.

## Not done

- The `no_ground` instances (mostly outside the collision bounds) are not rendered or classified
  one by one.
- A comparison with the original's screenshots at the same spots needs a capture. The original
  is not run from this session.

## Small gaps under ornaments: authored (2026-11-02)

Three groups had small gaps that could have been a constant port offset. All three are
ornaments: they are placed from `route_N/ornaments.bin`, not from entities.

- **Measured.** For each group, the base sits the same height above the drawn surface and above
  the collision (the two agree within 2 mm). The gap varies from instance to instance:
  - `lamp_pole_d_01` (Detroit): 0.06-0.15 m;
  - `post_tannoy_b_02` (Donington): 0.07-0.31 m;
  - `gg_vehicle_truck_02_c_01` (Donington): 0.06-0.10 m.

  A port offset would be constant. The `lamp_pole_d Root` node chain is identity throughout
  (ROOTNODE, LODVISIBLERENDERNODE and LOD lists).
- **The original.** Its ornament loader 0x980830 copies each stored 56-byte record's twelve
  floats into a 4x4 matrix with no change. It then calls 0x96F3C0 (create an ornament
  instance). That function reads the translation only for its bounds and uniform-scale tests,
  and passes the matrix unchanged to 0x963BA0. Neither function makes a ground or collision
  query (pseudocode in the shared RE cache).
- **Conclusion.** The original draws these ornaments where the file puts them, as the port
  does, so the gaps are authored. [I]: the claim that nothing later moves an ornament rests on
  these two functions and their direct callees, not on a capture.

## Runtime: broken prop pieces on Washington (2026-11-02)

The owner saw pieces in the air while racing. The dynamic case was checked on Washington route
0 in three ways. `GRID_FLOAT_CHECK=local/OUT.tsv` (grid-app, `dynamic_props::float_check`)
rewrites OUT every 60 frames with each live piece's:
- lowest drawn vertex against the road below (`drawn_gap`);
- collision hull's lowest point against the road (`hull_gap`, from `PropWorld::road_depths`);
- gap to the top of another piece's box under it (`piece_gap`, approximate).

1. **Solo, scripted drive into `tyrestack_white:0` (16,000 ticks).** Two stacks broke at
   ticks 9699/9701 (18 pieces).
   - At tick 15960, every piece's drawn base is 1-3 cm below its hull bottom. The mesh follows
     its body, with no offset between drawn and simulated shapes.
   - Nine tyres rest on the road (hull 3 cm, the contact shell).
   - The raised ones (0.5-0.96 m) rest on other tyres (`piece_gap` about 0), or lie within 2.5 m
     of the stopped car, where the final frame shows a tyre on it
     (`local/r40/solo-tyre2.png`).
2. **12 cars, scripted player into `tyrestack_white_double:0` (16,000 ticks).** The player and
   the AI broke five stacks (93 pieces).
   - Eight raised tyres have no piece box under them. Four of them belong to stack 757, which an
     AI car broke, beside the concrete blocks and the neighbouring intact stacks
     (`local/r40/stack757-static.png`).
3. **Physical support check, headless.** `grid-opponent-race --route
   tracks/usa/washington/route_0` ran with 12 cars, `GRID_DYNAMIC_PROPS=1`,
   `GRID_PROP_TARGET=tyrestack_white_double:0` and `GRID_HANG_CHECK=1000` (F561 wake test). Each
   check wakes every asleep piece in a cloned world for 120 ticks.
   - Result: 0 unsupported at every check from tick 1000 to 14000, up to 120 bodies (108
     pieces, 84 asleep). No asleep piece falls more than 0.2 m when woken.
   - So the raised tyres in runs 1-2 lean on cars, walls, intact stacks or other tyres.

Pieces start at their intact placement by construction: a piece is drawn at
`body now x start^-1 x rest`, so at the break it is exactly where the intact mesh was. Intact
breakables are static and covered by the static audit. Lower ornament LOD levels: some
buildings and `gg_vehicle_parked_cars_01_b_01` float 0.2-0.4 m at level 1 or 2 where level 0
rests. These are the original's simplified meshes, drawn only at a distance.

No runtime floating was found on Washington, so nothing was changed in the simulation. Not
covered:
- detached car panels and wheels, which turn off after 1 s (VISUAL_DAMAGE_PARTS_RESEARCH);
- breakables that Washington route 0 does not have (boards, cushions);
- what exactly the owner saw. A screenshot or position from the owner would settle that.

## Things in the air during races: god-ray cards and grass without land (2026-11-02)

Two things the owner saw floating had sources the instance audit did not cover. Both are fixed
in the shared loading path for every route.

- **Black "drops" on Washington** (owner screenshots 29-32 and 35, FIDELITY_BACKLOG row 107).
  - Reproduced on a8cef46b9294: Viper, 12-car race, autopilot, tick 4100. A static render from
    the same camera shows them too, so they are static geometry. `--geometry-ray` names
    `ornament/0/!hOB`, material `objects/godray_tree!0`.
  - Its shader group is `god_rays_single_sided.fx`; `god_rays.fx` appears on 7 tracks and the
    single-sided variant on 4. The shaderpack draws both alpha-blended (`SRC_ALPHA`,
    `ONE_MINUS_SRC_ALPHA`) with `object_light_shafts.cg.fp_light_shafts`. In the original they
    are faint light shafts coloured from shader constants.
  - The port drew them as plain textured objects, so the dark `godrays4.tga` appeared as black
    spindles.
  - `object_materials::light_shaft_materials` now lists them. `append_objects` counts them
    (`omitted_light_shaft_draws`) and does not draw them.
  - Open: a light-shaft shader port (fragment program not disassembled).
- **Flecks in the air on Okutama, and the black void with floating buildings on Washington**
  (screenshot 33, row 108).
  - Ground-cover grass (`ground_detail.rs`) is scattered on the triangles of `grass.grs`.
    Those triangles follow the terrain in `land.pssg`.
  - `land.pssg` was drawn only in the enhanced preset, which only Donington route 0 allows. So
    on every other route the grass on land hung in the air. Washington's buildings stand on
    land and hung above the empty riverbed area, which showed the clear colour.
  - Land and its layered infield are now drawn on every preset. `land_has_draws` skips Milan's
    and Shibuya's land files, which hold no render instances.
  - `water.fx` draws (sea and rivers, 7 tracks: a normal map and constants `deepColour`,
    `shallowColour`, `reflectionColour`, Fresnel bias and power, and bump scroll) have no port.
    They are counted (`omitted_water_draws`, with their bounds) instead of failing the load, so
    Washington's river now shows its bed. Water stays open in row 108.

The audit now also covers ground cover (`grass_patch` rows). For every painted `grass.grs`
vertex (any density above 0) it measures the gap to the nearest of the collision and the drawn
surfaces below it. A vertex with nothing below is checked upwards: a surface up to 29 m above
means the vertex is buried, not floating. Each patch with vertices more than 0.25 m above every
surface gets a row: the centroid, the largest gap and the count. The summary line adds
`grass_vertices` and `grass_raised`.

F709 adds two columns to each `grass_patch` row: the nearest raised vertex's distance to the
route's gate line (`progress_track.xml` gate centres, closed on circuits) and the number of
raised vertices within the farthest `ground_cover.xml` draw distance of it. The summary line
adds `grass_raised_in_reach`. `grass.grs` is shared by a track's routes, so a short route
reports raised grass it never comes near: Okutama routes 1, 2, 4 and 5 have none in reach
(docs/TRACK_VIS.md).
