# Track readers: every shipped route (F399)

Status: 2026-10-18, Claude Code. Goal: make more of GRID's tracks load in the
port, priority Yokohama (freestyle arenas, Drift Battle), Detroit (Demolition
Derby, street races), Spa, Haruna. Result: Yokohama, Spa, Milan, Shibuya, San
Francisco and Detroit now pass every headless reader; Yokohama route 0 is the
sixth circuit ([YOKOHAMA.md](YOKOHAMA.md)). Existing tracks are unchanged (solo
lap times identical before and after, section 4).

Evidence (ignored, this worktree's `local/track-readers/`): `survey-before/`
(today's main, before any change), `survey-readers2/` (after), the stage and
stream outputs below, `jpk` scan notes.

Tags: [V] observed in the files, the executable or a run; [I] interpretation;
AUTHORED: a port choice with no original counterpart.

## 1. Tools

- `tools/route_survey.sh EXE OUT [TICKS] [GLOB]`: the solo-AI survey
  (`grid-opponent-race --route R ROOT CD01 BP01 1 16000 ai`) over every shipped
  route but the front-end garages; one line per route.
- `grid-route-readers` (grid-probes, read-only):
  - `stages ROOT ROUTE` runs each reader of the headless race and the app on its
    own file (collision archive, boundary, grid, progress, AI line, start
    lights, entities, ornaments, objects, props, tracksplit, land, trees, sky)
    and prints `ok` or the error, so a failure names its file;
  - `streams FILE` lists every render stream kind/format and the sources with
    nonfinite half2 ST words;
  - `nodes FILE ELEMENT [N]` prints PSSG nodes of one element with attributes.

## 2. Survey before (main 2393a9e, solo AI, 16000 ticks)

`local/track-readers/survey-before.txt` [V]. The error is what grid-opponent-race
printed; the file is from `grid-route-readers stages`.

| Track (routes) | Error | File |
|---|---|---|
| spa_francorchamps (0-3) | `nonfinite half UV` (before: `VCQTC packed count limit exceeded`) | `objects.pssg`; `route_N/track.jpk` chunk `qt_11_11_10_01_11` (9 materials) |
| milan (0-5) | `unsupported collision primitive` | `objecttypes.pssg` (`TEMPLATESHAPESPHERE`) |
| usa/detroit (0-7) | `unsupported collision primitive` | `objecttypes.pssg` (spheres) |
| usa/detroit (8-9) | `unsupported mesh stream kind/format` | `objects.pssg` (`Normal/half4`) |
| japan/haruna (all) | `collision archive entry count unsupported` | `route_N/track.jpk` (532 chunks) |
| japan/okutama (all) | `expected one typed entity child` | `route_N/objects.ens` |
| japan/shibuya, usa/san_fran, usa/washington (all) | `unsupported mesh stream kind/format` | `objects.pssg` |
| japan/yokohama_docks (all) | `nonfinite half UV` | `objects.pssg` |

Since F350's survey, Donington routes 1-3 and Nürburgring route 3 lap (the lap
timing was generalised in between).

## 3. Reader changes

### 3.1 Half-float texture coordinates may be NaN [V]

`grid-route-readers streams` on `objects.pssg` [V]: Yokohama has 10 sources
whose ST/half2 stream holds 0xFFFF words (one source: the first 567 of 1487
vertices are all FFFF,FFFF); Spa 3 sources with 0x7FFF/0xFFFF words; Detroit 1;
Haruna's `tracksplit.pssg` too. Both words are binary16 NaN. The reader rejected
the whole file. Now `mesh::read_meshes` decodes them exactly (NaN kept, as the
normals already were) and counts the vertices in `Mesh::nonfinite_texcoords`.
[I] The original uploads the halves to the GPU unchanged, so these vertices
sample undefined texels; whether they are referenced by drawn triangles was not
checked. No app fallback was added (rendering is Codex's area): the Yokohama
start renders without visible defects.

### 3.2 `Normal/half4` [V]

Detroit's interleaved 48-byte objects (`!WOB`, `!LOB`, …; offset 20 within the
stride) store normals as four halves. The reader takes x, y, z. Tangent/Binormal
`half4` were already skipped by the existing "other encodings" rule.

### 3.3 Sphere collision shapes [V]

`TEMPLATESHAPESPHERE` with `radius`, `shell` 0.02, `props physparams.xml#sBalloon`
(Milan 9 nodes: `col_sphere_low/med/top_shape`, radius 0.36-0.91; Detroit 2,
radius 1.64 and 2.24). `entities::Primitive::Sphere { radius }`. Static spheres
and cylinders are route collision since row 20 (F462, BOX_CONTACTS.md "Cylinders
and spheres"); the app logs `entities collision_spheres=N`. A sphere as a breakable
piece leaves the template unported.

### 3.4 Sixteen VCQTC materials; sheet mask is a dword [V]

The packed triangle's material is `b & 15`, so a chunk can name 16 keys; Spa's
`qt_11_11_10_01_11` has 9. The authored limit was 8. The original tests the
sheet flag as `test dword [shape+0x290], 1 << (b1 & 15)` at 0xAEAFE5 (Capstone
5.0.6 on `local/compatibility/grid-unpacked.exe`: `movzx ecx, byte [ecx+1]; and
ecx, 0xf; shl edx, cl; test [ebx+0x290], edx`). `packed_contacts::Settings::
sheet_mask` is now `u32` (was `u8`); `Track` allows 16 keys per chunk. Values for
≤ 8 materials are unchanged.

### 3.5 More than 512 collision chunks

Haruna's `track.jpk` has 532 entries (`jpk` scan [V]); `Track::read` and
`grid_population::build` allowed 512 (an inspector bound, not an original
limit). Now 4096. The population grid's own cell bound (1024 × 1024) still
applies.

### 3.6 Group barrier prototypes (AUTHORED stand-in)

Detroit's `concrete_block_long_01` and Washington's `chnfen_e_group1` are one
ornament placement each (identity, Detroit y offset -0.06) of a mesh laid out
in world space over 146 m and 540 m [V]. The F357 barrier stand-in made one box
of the whole extent and failed its 100 m check. When the whole extent fails and
the prototype has one placement, each connected part (1 mm weld) of at most
10 m horizontally becomes its own box; longer or flat parts stay non-solid.
Every prototype of the five earlier circuits passes the old check, so their
boxes are unchanged. Washington then has more than 8192 route boxes and fails
`unsupported entity collision box count` on routes 0, 2, 5 and 7 (open).

### 3.7 Breakables standing on a cylinder

Yokohama's `gg_barrel.max` (77) breaks, but its intact shape is a cylinder,
which is not a static route box, so `Catalog::read` failed with "breakable
entity without a collision box". Such templates are now counted in
`Catalog::unported` (kept static, as armco), like other templates the port
cannot break. Since row 20 (F462) cylinders are route shapes, so the 77
barrels break.

## 4. Survey after

`local/track-readers/survey-readers2.txt` [V] (before the barrel change, which
only affects the breakable catalog):

| Track | Loads | Solo laps (2 requested) |
|---|---|---|
| donington 0-3, nurburgring 0-3, istanbul_park 0-3, long_beach 0-5, jarama 0-1 | yes | the 22 output files (laps, state hashes, contacts) byte-identical to the survey before |
| **yokohama_docks** 0, 1, 7, 8, 9 | yes | 2 valid (route 0: 72.320 / 71.185 s); route 2 one valid |
| yokohama_docks 3 | progress: `unknown progress split kind` (`start`) | — |
| yokohama_docks 4-6 | progress: `only circuit progress supported` (`point_to_point`) | — |
| shibuya 0-7 | yes | 2 valid on every route |
| milan 0, 2, 4, 5 | yes | 2 valid (route 0: 122.747 / 120.058 s); route 1 DNF after 1 lap; route 3 progress `start` split |
| **usa/detroit** 1-7 | yes | 2 valid (route 1: 91.590 / 89.295 s) |
| usa/detroit 0 | yes | wrecked, 0 laps |
| usa/detroit 8-9 (stadium) | yes | 1 valid of 2 |
| usa/san_fran 0, 1, 6 | yes | 2 valid; 2, 5 DNF; 4 wrecked |
| usa/washington 1, 4, 6, 9 | yes | 2 valid; 3, 8 wrecked; 0, 2, 5, 7 box bound |
| spa_francorchamps 0-3 | yes | retired, 0 laps (AI, not a reader; open) |
| le_mans 0-1, jarama 0 | yes | wrecked, 0 laps (as before) |
| haruna 0-9 | no | `grids.pssg` `expected unique grid marker`; progress `point_to_point`; AI `links do not form one loop`; `tracksplit.pssg` NaN UVs now read |
| okutama 0-5 | no | `objects.ens` `expected one typed entity child` |

12-car AI races, 2 laps, DF01 and the characteristics [V]
(`local/track-readers/detroit{1,8}-12car.txt`):

- Detroit route 8 (`detroit_stad_L`, the Demolition Derby stadium): 11 cars 2
  valid laps, car 11 one; winner 1:37.136, last +35.792; 15057 car-contact
  records.
- Detroit route 1 (`detroit_v1`): 9 cars retire with terminal damage between
  ticks 6480 and 12830, most near (-45..-84, 553..562) at about 1.95 laps;
  only car 7 finishes (4:36.208). Not a reader fault; open.

## 5. Open

- ~~Point-to-point progress, the `start` split, Haruna's subroute grids and
  open AI line~~: done in F407 ([TOUGE.md](TOUGE.md)); Haruna routes 0-8 and
  Milan 3 now lap, the Yokohama arenas run Freestyle in the app.
- Okutama `objects.ens`: an entity with more than one typed child.
- ~~Spa: the solo AI retires~~: the packed-contact material check still stopped
  at 8 keys; fixed in F425 ([SPA.md](SPA.md)). Spa, Milan, Shibuya and San
  Francisco route 0 race ([STREET_CIRCUITS.md](STREET_CIRCUITS.md)).
- Detroit route 0 and the Washington wrecks; Detroit stadium laps.
- Washington's route box count after the group parts (section 3.6).
- ~~Static cylinders and spheres are not route collision~~: route shapes since
  F462 (BOX_CONTACTS.md "Cylinders and spheres").
