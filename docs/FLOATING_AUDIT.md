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
- Small constant gaps not rendered yet, the next candidates for a port offset:
  - `lamp_pole_d_01` 0.09 m (171);
  - `post_tannoy_b_02` 0.13 m (108);
  - `gg_vehicle_truck_02_c_01` 0.34 m (120).
