# Circuit-race series (Pro Tuned, Touring, GT1, GT2, Le Mans Series)

18 October 2026, Claude Code (F401). Item 3 of docs/DISCIPLINES.md §5: the free-race circuit series
on our five circuits. In the original these series are `single_race` events that differ only in their
cars and track pools (DISCIPLINES.md §1.2), so the work is cars, layouts, the field and launchers.

Labels: **[D]** shipped data; **[V]** seen in a disassembly/decompilation of the hash-pinned analysis
image (`local/compatibility/grid-unpacked.exe`) or verified by a run; **[I]** interpretation;
**AUTHORED** a choice of this port.

## Summary

| Series | Cars [D] | Status in the port |
|---|---|---|
| Touring | bmw, lac | races (as before) |
| Pro Tuned | sup, sky, nsx, 350 | **races**: 12-car fields on all 20 layouts |
| GT1 | dbr, c6r, sal, ccg, c5r, lam | **races since F406** (collision floor lift): 12-car fields on all 20 layouts |
| GT2 | spk, esp | **races since F406**: 12-car fields on all 20 layouts |
| Le Mans Series | lam, 911, c65, spk, dbr, lc7, b06, cre, b05, esp, sal, c5r, c6r | **F409: original field and grid, 15–17 cars, launcher (experimental)**; the LC70s in every field still ground and wreck |

New code:
- `grid_formats::database`: the whole NDBS `database.bin` decoded by `schema.bin` (78 tables).
- `grid_tools::car::Vehicle` / `Car::vehicle(root)`: a car's `vehicle` row (AI classes, statistics
  stem, class, drive type). `AiData::load_car` uses it, so every car uses its own
  `ai_vehicle_type_config` (ai_vehicle_config.xml class), `ai_vehicle_type_track` (route brake lines)
  and `ai_vehicle_type_statistics` (brake tables). Lacetti and BMW are `touring`/`touring`, as the fixed
  class before, so their runs are unchanged. grid-opponent-race keeps `GRID_AI_CLASS` as an override.
- `grid_tools::series`: `Series` (cars and track pool of a free-race discipline), `build_field` (the
  original's free-race field), `Mt` (the original's MT19937), `Event` (prepared race).
- grid-app: `[game] series = NAME` / `--series NAME`; launchers `Play GRID Series Pro Tuned.cmd`
  (settings/series-pro-tuned.ini, Donington National) and `Play GRID Series Touring.cmd`
  (settings/series-touring.ini, Istanbul Park route_3). `GRID_SERIES_SEED=N` fixes the field seed.
- Reader fixes: the mesh reader accepts a declared `maximumIndex` above the used maximum (one DBR9
  mesh); `source_deformation::load` no longer allows only `lac`/`bmw`.
- Deformation impact fix (`grid_world::deformation_impact`): the packed edge list pads its last group
  of four with edges on the spare node 399. For the Skyline (grid 6 × 5 × 13 = 390 nodes, 892 edges,
  three padding edges [399, 399]) and the NSX-R these edges give node 399 a NaN motion
  (1 / sqrt(0)), and the port's finite checks over all 400 nodes stopped the race at the first
  impact ("nonfinite deformation impact output", 12-car fields at tick 545–609). No grid node reads
  node 399 [V: the edge loop touches only its endpoints, the integration loop only the grid nodes
  padded to four], so the motion checks now cover the grid nodes only; positions, bounds and
  markers are still checked for all 400. [I] The original carries the same NaN in its spare node
  and never reads it. Lacetti/BMW grids never reached that path, so their results are unchanged.

## Car survey

Solo Donington GP runs, `grid-opponent-race --cars STEM ROOT CD01 BP01 1 6000 ai` (private outputs in the
main checkout's `local/series/survey`, `survey2`) [V]:

| Car | With the fixed `touring` AI class (before) | With its own database classes |
|---|---|---|
| sup, sky, nsx, 350 (streetgt) | under 0.6 lap in 3000 ticks, no failure | 1 valid lap each (84.9–86.6 s first lap) |
| sal, c5r (gt1) | no failure | 1 valid lap (86.9 / 88.7 s) |
| ccg (gt1) | wrecked at tick 1536 | 1 valid lap (79.5 s) |
| c6r, lam, dbr (gt1) | lam wrecked, dbr **failed to load** | c6r wrecked, lam DNF, dbr 0.66 lap |
| spk, esp, 911 (gt2) | esp wrecked, others slow | none completes a lap |
| c65, lc7, b06, cre, b05 (lmp) | b06/c65 wrecked | none passes 0.35 lap |

Longer solo runs (14000 ticks, 2 laps) [V]: ccg and c5r complete 2 laps on Donington GP, Nürburgring
GP and Istanbul route_1 (ccg 77.7 s, c5r 78.1 s second lap at Donington); sal stalls at the
Nürburgring with terminal damage; sup completes 2 laps on Donington and Istanbul.

What fails [V, `GRID_SOAK_TRACE_BODY`]: the Courage LC70 body touches the road on the start straight at
40 m/s (7–16 body contacts per tick, wheel compression 0.036 m), loses speed under full throttle and
spins at Redgate. The Lacetti, S7R and Spyker have no body contacts there. The GT2/GT1 failures happen
at Redgate/Hollywood at 35–40 m/s with full steering lock. The CSVs [D] give the LMPs and GT2 cars
much more `downforce` (column 12: lac 0.5, sal 1.2, spk 5, c65 6.5, lc7 9).
[I] Downforce compresses the suspension until the body floor touches the track, and the body
contact takes load and grip from the tyres. Whether the original filters body–road contacts, uses a
lower body friction or another ride height is not known; it lies in the shared vehicle and contact
code, so this change does not touch it. It is the next step for GT2/Le Mans cars.

Everything else loaded without reader changes: tuning CSV, NED, wheel geometry, suspension,
damage regions, deformation grids (built for sup, sky, nsx, 350, sal, ccg, c5r), liveries, engine
banks (`audio/eng_<stem>.WIP` exists for every car) and AI extents. App render
(`--car sup --opponents 3 --opponent-cars sky,nsx,350`, Donington route_1, 420 ticks): Supra in livery 00,
Skyline beside it, HUD "Toyota Supra / Donington race", damage parts read for all four models [V,
`local/series/protuned-race.png`].

`series::RACEABLE` (AUTHORED) lists the cars that race fields: lac, bmw, sup, sky, nsx, 350. A series
event that draws another car stops with a message unless `GRID_SERIES_EXPERIMENTAL=1`.

## GT and Le Mans cars: the collision floor lift (F406) [V unless marked]

The F401 cars that failed (GT1, GT2, prototypes) touched the road with their body hull at speed. The
cause is a missing step of the original body collision shape, found statically (Ghidra 12.1.4
headless read-only, Capstone 5.0.6; decompilations in the worktree's `local/gtfix/decomp/`).

**The original body shape.** Car factory 0xA01960 builds the body collision pieces with 0x9E0240 (three
call sites, 0xA01D1D/0xA01DBB/0xA01DFB) and only then adjusts them with 0x9B8B70 and builds the meshes
(0xB23E40 → 0xB44E80, which the port verified on raw input). 0x9E0240:
- starts from a synthetic 16-point car hull built by 0x995D10 from the description extents (body
  +0xED64..+0xEDA4: centre, bottom, front/rear, sides, corner Zs, mid/roof heights);
- with body +0xBA0 = 2 (every Standard car) splits it into `m_subdivX` × `m_subdivZ` slabs (3 × 3 on
  every car [D]) at `m_subdivX_pos`/`m_subdivZ_pos` (+0xED48..+0xED5C) and builds each slab's polytope
  from planes (0x9BF7D0, 0xA15F50);
- snaps every polytope vertex to the nearest point of the `collision_main` mesh (0x99D080 on body
  +0xE300, the copied mesh; 0xAE0010 point–triangle distance);
- **lifts the floor** (0x9E0674..0x9E072A): with bottom = +0xED68, roof = +0xED94, h = roof − bottom,
  clearance = +0xED60 and track = global 0xE85FC8,
  `g = clamp((track + 0.176) − (clearance + bottom), 0, 9999)`, `a = (g + clearance) / h`,
  `y' = (((y − bottom) / h) · (1 − a) + a) · h + bottom` — the hull's floor moves to
  bottom + clearance + g and the roof stays;
- rebuilds the planes from the lifted points and applies `m_flatten_bottom/front/back` (all 0 except
  the C6-R's flatten_back [D]).

**Inputs.** `m_groundClearence` (NED COLLISION) is read by 0x9EFB30 into description +0x1B20 = body
+0xED60, preset 0.17 (0xCE770C); every shipped car declares 0.07 or 0.05 [D]. The global 0xE85FC8 is set
only by 0x997280, called once from 0x60BEF0 right after it reads the database column
`additional_ground_clearance` (0x60C00E) [V; that the passed value is that column is [I], the
decompiler's stack view is confused there]: 0.05 on the city tracks (Long Beach, Yokohama, ...), 0
elsewhere [D]. Constants 0xD1728C = 0x3E343958 (0.176), 0xCB0D94 = 9999 [V]. So on our circuits every
car's collision floor sits at max(bottom + clearance, 0.176) in NED space (0.226 at Long Beach): the
Lacetti's raw floor 0.103 rises 7 cm, the LC70's 0.047 rises 13 cm.

**What the port did.** `BodyGeometry::read` fed the raw `collision_main` points to 0x9B8B70/0xB44E80
(docs/SOURCE_COLLISION_MESH.md says so: "not proof that the entire original outer factory produces
these exact settings"). **Port (F406):** `body_geometry::FloorLift` applies the lift with the original's
operation order and f32 rounding to the raw points (`BodyGeometry::read_on_track`, used by
`Runtime::prepare_body_geometry_with`, so every car, single and multi runtimes, and the app);
`car_collision::ground_clearance` and `Route::additional_ground_clearance` read the inputs. The same
bottom and roof enter the remap as in the original, so the lifted floor and roof heights equal the
original's exactly; the hull's sides still come from the raw mesh. **Not ported (open):** the
synthetic hull, the 3 × 3 pieces, the vertex snap and the flatten options — the body is still one convex
mesh. `BodyGeometry::read` keeps the raw hull for the existing raw-mesh comparisons.

**Results** (release, Donington GP solo, 12000 ticks, `local/gtfix/survey/`) [V]:

| Car | Before (F401) | Floor lift |
|---|---|---|
| lac, bmw | 2 laps | 2 laps, **identical** times (88.420 / 86.282 s) |
| sup | 2 laps | 2 laps (86.58 / 84.73 s) |
| sal, ccg, c5r, c6r, dbr, lam (GT1) | sal/ccg/c5r 1 lap in 6000 ticks; c6r wrecked, lam DNF, dbr 0.66 lap | all 2 laps, 75.3–82.9 s |
| spk, esp (GT2) | no lap | 2 laps (77.6 / 94.0 s) |
| 911 | stalled | 1 lap (98.4 s), understeers |
| c65, cre, b05 (LMP2/LMP1) | under 0.35 lap | 2 laps, 70.6–73.1 s |
| lc7, b06 (LMP1) | under 0.15 lap | still wrecked |

Controls: with the lift switched off in a test build, Pro Tuned soak seed 10 (solo Supra at the
Nürburgring, F401's open fall-through) fails again; with it, it passes — the same low floor caused it.

**LMP1 (open).** The LC70 (downforce 9) and B06 (15) compress their suspension 0.15–0.26 m at 55–66 m/s
(`GRID_SOAK_TRACE_BODY`; travel 0.23–0.25 m), so even the lifted floor touches the road. Checked and
ruled out [V]: the downforce scale body +0x2268 is a cold 1.0 (+0xD1AC), so the verified aero path
gives 18·v² N for the B06 in the original too; column 193 (`visual_limit_max_wheel_height`, body
+0x1F84) is read only by the wheel presentation 0x9FA6E0; the body–road material is already the
original CAR_ row (friction 0.4, equal to the mixed pair). Next: the synthetic 9-piece hull through a
native probe of 0x9E0240, and suspension travel stops.

**Fields, launchers and checks (F406)** [V], release, cargo/rustc 1.99.0, on 9d9398b (tests and clippy again on be41b7b):
- 12-car fields, 20000 ticks, all 20 layouts (`local/gtfix/routes/`): GT1 (sal, ccg, c5r, c6r, dbr,
  lam cycled) 10–12 of 12 finish 2 laps on 16 layouts, 8 at Donington GP, 5 and 9 at Jarama; GT2
  (spk, esp) 10–12 on 17 layouts, 9 at both Jarama layouts, 2 at Long Beach GP. Lacetti control:
  12/12 on 17 layouts, 11 at Jarama route_1, 8 and 10 on the Long Beach reverse layouts (F401: 0, 5
  and 3 — F402's robustness work also landed in between).
- `series::RACEABLE` now includes the six GT1 and two GT2 cars; launchers
  `Play GRID Series GT1.cmd` (settings/series-gt1.ini, Nürburgring GP, DBR9) and
  `Play GRID Series GT2.cmd` (settings/series-gt2.ini, Istanbul Park route_1, Spyker). App run of the
  GT1 profile (GRID_AUTOPILOT, seed 3, 9000 ticks): field dbr_05, dbr_06, dbr_07, c6r_04, c6r_04,
  sal_02..04, ccg_00..02, valid lap 1:52.759 (`local/gtfix/series-gt1*`).
- Tests 674 passed, 0 failed (`--include-ignored`, absolute GRID_* paths); new: `FloorLift`,
  purchased ground clearances, track clearances. `a_drift_battle_ai_car_drifts_and_is_scored`
  changed: on Long Beach route_1 the BMW now slides up to 22° without spinning and the drift
  detector does not latch within the test's 2600 ticks (before: a 179° spin, 50 detected ticks); the
  test now asserts slip > 20° and no spin.
- Clippy workspace all targets clean.
- Identity: solo `grid-opponent-race ROOT CD01 BP01 1 11200 ai` sha256 aad9a07a…e41b unchanged (the
  Lacetti's floor never touches the road on a clean lap). 12-car `… 12 11200 ai` changed from
  54abbecb…2a2b to **290589bbe6387834235a646b50b92eb82fb70b5fd0961a6ed2dea5729f7c18d0**: the lifted
  hulls meet each other differently (car contacts 681 → 461; winner car0 2:56.576 → 2:56.720).
- `grid-soak quick` (default, 24 seeds since F402): 23/24 — seed 14 (12 cars, Istanbul GP): prop
  piece 36, at rest off the track (57 m from the centreline), is nudged at tick ~1350 and sinks
  through the ground. It passes with the lift switched off: the lift changes the chaos trajectories
  and exposes the open prop-piece family (docs/ROBUSTNESS_RESEARCH.md §3).
- GT soaks (`--car dbr --opponent-cars sal,ccg,c5r,c6r,dbr,lam`, `--car spk --opponent-cars
  spk,esp`, Pro Tuned): 23/24, 21/24, 23/24. Failures: Long Beach seed 17 prop piece sinks (GT1, Pro
  Tuned); GT2 seeds 7, 8, 22: the player's Spyker, driven into a wall at full throttle and lock by the
  chaos driver at 47 m/s, ends on its nose and sinks through the road (contacts 3–5 → 0). Without the
  lift the GT2 cars hardly reached speed, so these runs were not exercised before. Open robustness
  residuals; the AI-driven fields above do not show them.
- Pro Tuned seed 10 (F401's open fall-through): passes; fails with the lift off.
- Drift Battle (F389's measure, 8 BMWs, 16000 ticks, `local/gtfix/drift/`): spins/lap Long Beach
  25.1 → 9.4, Nürburgring 11.1 → 10.1, Istanbul 12.0 → 13.5; stalled seconds 544 → 212, 343 → 276,
  316 → 321; score sums 2,566 → 443, 64,381 → 109,871, 44,980 → 83,312. Other changes landed since
  F389, so this is an observation, not an attribution.

**Le Mans Series field** (research subagent, static; decompilations in the worktree's
`local/gtfix/lms/`) [V unless marked]: 0x676F50 calls 0x663860 for mode `le_mans_series`. The field is
every `lemans_restrictions` row with `player_vehicle_id` = the player's vehicle and `lemans_24` = 0, in
storage order, each an AI car in its entrant livery, skipping the row in the player's own livery; no
RNG, and `max_cars` is not used, so fields have 12–17 cars [D: 15–17 for our cars, each with 2–3
LC70s]. Grid 0x5BDF10: one MT19937 draw `genrand() % n` per car swaps it with that index, then a
stable bubble sort by `vehicle_class.lemans_sort_order` (LMP1 1, LMP2 2, GT1 3, GT2 4) and the player
goes to the front/back/middle of its class group (or stays). Not ported: the port's grids hold at most
12 cars and the LC70 does not race yet.

## Le Mans Series and the LMP1 cars (F409) [V unless marked]

**Le Mans Series port.** `series::build_lemans_field` (0x663860: the player's entry, then every
`lemans_restrictions` row of the player's counting vehicle with `lemans_24` = 0 in storage order,
skipping the row in the player's own livery), `series::lemans_sort_order` (`vehicle_class`) and
`series::lemans_grid` (0x5BDF10: one `genrand() % n` swap per slot, stable bubble sort by class key,
player to the front/back/middle of its class group). `Event::prepare_on_grid` returns the opponents
in slot order and the player's slot; grid-app applies it (`grid_position = back` → back of the class
group, else its front: AUTHORED mapping; the original's option comes from the front end). AUTHORED:
the player's livery is `<stem>_00` (the front end chooses it in the original) and the grid seed
(`Mt::new(seed + 2)`; the original uses the clock). Field sizes [D]: DBR9 15, Spyker 16 (one row is
the player's own livery in both).

Limits raised to the routes' 20 grid slots [D: every route's grids.pssg has slot_00..slot_19]:
`MultiRuntime` (was 12), `Car::list`, grid-opponent-race (1..20 cars), grid-app `--opponents 1..19`,
the results table (header + 20 rows). Deformation readers now accept a negative strength-box damage
weight: the C65, B05, LC70 and R10 cabins have damage −1 [D], and 0x9E7CE0 only multiplies by it (the
port's 0..100 range was an authored guard).

Launcher `Play GRID Series Le Mans.cmd` (settings/series-le-mans.ini, Donington GP, DBR9, back of
the GT1 group). Every Le Mans Series field holds 2–3 LC70s, which still ground (below), so the
launcher sets GRID_SERIES_EXPERIMENTAL=1. Not ported: the results screen's Class column
(`post_race_results_lemans`) and `lemans_progress_adjustment`.

**LMP1 (still open).** Findings (static + headless):
- The 3 × 3 body pieces of 0x9E0240 are built with margins `param_2 = param_3 = 0.02` (0xA01DA4..
  0xA01DBB, constant 0xCB0934): every plane gets +0.02, and the outer ones −0.02 again, so the outer
  faces — the floor included — do not move; only the faces between pieces overlap by 0.02. The
  9-piece hull would not change the floor height that F406 already reproduces. **Verified-equivalent
  for the floor and roof (coordinator's decision, F410): not ported**; the pieces' sides, the vertex
  snap and the 0.02 overlaps between pieces remain unported and are not expected to be measurable.
- The suspension step 0x9F1E10 with its travel stops is already verified 1:1 (docs/SUSPENSION_VEHICLE.md).
- Body +0x2214 (aero scale) is reset to the same value as the air density in 0xA09CE0 (≈ 1).
- Diagnostic (temporary build, not kept): with the floor raised a further 0.15 m the LC70 and B06
  complete 2 laps at Donington (70.5 / 74.6 s); at the shipped floor they wreck. The 911's single
  lap does not change with the floor (another cause).
- [I] So the LMP1s' 0.15–0.26 m compression puts the lifted floor on the road; the remaining
  difference to the original is in what that contact does (the port's body–road contact scheduling
  is authored, docs/DONINGTON_BODY_CONTACTS.md) or in an aero input not found statically. A live
  trace of the original is needed to decide; the coordinator will schedule it with the owner. Until
  then the LC70 and B06 stay behind GRID_SERIES_EXPERIMENTAL, and so does the Le Mans Series
  launcher, because every field holds LC70s (gate preferred over an authored substitution).

**Checks (F409)**: headless 17-car mixed field (c65, 911, dbr, cre, b05, lam, sal, c5r, c6r, spk, esp
cycled; DF01) at Donington: 15/17 finish 2 laps. The DBR9 Le Mans Series field (15 cars, 3 LC70s),
20000 ticks: Donington 7/15, Istanbul 13/15, Jarama 10/15, Nürburgring 11/15 finish 2 laps; the LC70s
(cars 0–2) wreck or stall. `grid-soak quick --seeds 1..8 --cars 16 --max-cars 16` with the LMS cars:
7/8 (seed 4: a prop piece sinks, the open family). App (GRID_SERIES_SEED 5): field
`lc7_02, lc7_00, lc7_01, c65_02, c65_03, c65_01, c65_00, dbr_01, dbr_02, sal_00, lam_00, 911_00..02`,
player slot 11 (back of GT1), HUD 12 / 15; a 1-lap run finished 9th / 15 (`local/gtfix/series-lms*`).

## Layouts

Every circuit-series pool [D] (`free_race_track_pool`, pools `pro_tuned`, `touring`, `gt1`, `gt2`)
lists all 20 layouts of our circuits (Le Mans Series: the four GP layouts):

| Track | route_0 | route_1 | route_2 | route_3 | route_4 | route_5 |
|---|---|---|---|---|---|---|
| Donington | donington_gp | donington_nat | donington_gp_r | donington_nat_r | | |
| Nürburgring | nurburgring_gp | nurburgring_spr | nurburgring_gp_r | nurburgring_spr_r | | |
| Istanbul Park | istanbul_gp | istanbul_int | istanbul_gp_r | istanbul_int_r | | |
| Long Beach | long_beach_gp | longbeach_cl | longbeach_sp | longbeach_gp_r | longbeach_cl_r | longbeach_sp_r |
| Jarama | jarama_gp | jarama_r | | | | |

Yokohama route_0 (F399, the sixth circuit) is in the Pro Tuned pool as yoko_v1; it was not part of
this survey. `Series::supported_layouts` lists the routes of the five surveyed tracks
(`SURVEYED_TRACKS`) plus the exact `route::CIRCUITS` routes; `Event::prepare` accepts any pool layout.

No route code change was needed: `Route::new` already reads any route folder (gates, start grid, AI
line). Survey `local/series/routes/` (`route_survey.sh`, 4 jobs) [V]:
- **4 Lacettis, 16000 ticks, all 20 layouts**: every layout loads, races and classifies. The racing
  line lengths match the database lengths (e.g. donington_nat 3156 m / 3.1 km, longbeach_sp 1282 m /
  1.3 km). Car 0's second laps: Donington 86.8 / 62.6 / 86.2 / 62.8 s, Istanbul 109.1 / 84.4 / 115.5 /
  82.4 s, Nürburgring 110.6 / 83.7 / 112.5 / 85.4 s, Long Beach 79.3 / 94.2 / 31.4 / 78.6 / 89.6 / 30.2 s,
  Jarama 98.5 / 89.5 s. 17 layouts had 4 of 4 finishers; 3 had one car not finishing.
- **12 Pro Tuned cars (sup,sky,nsx,350 cycled), 20000 ticks**: 11–12 of 12 finish on every Donington,
  Istanbul (route_2: 9) and Nürburgring layout and both Long Beach sprints; the Long Beach GP/cl
  layouts (3–8 of 12) and Jarama GP (5) end in pile-ups (8,000–47,000 car contacts).
- **12 Lacettis on the same problem layouts** (control): Jarama route_1 0/12, Long Beach route_3 5/12,
  route_4 3/12, while Long Beach route_0/1 and Jarama route_0 finish 11–12. So the pile-ups are AI
  traffic behaviour on those layouts, not the new cars [I]; the playable Jarama/Long Beach profiles
  (F357, F361) run with DF01 and other settings.
- **12 GT1 cars**: sal,ccg,c5r: 0–10 of 12 finish on each layout; ccg,c5r only: 2–6 of 12.

## Grid fill [V unless marked]

Static trace of the free-race path (Ghidra 12.1.4 headless, Capstone 5.0.6; private decomp and scripts
in the main checkout's `local/series/gridfill/`):

**RNG.** MT19937. Seeding 0xA68440: `mt[0] = seed | 1`, `mt[i] = mt[i-1] * 69069` (4357 when never
seeded); generation 0xA682A0 and tempering 0xA68380 are the standard ones (mag01 at 0xE51CA4).
0xA683E0 (= 0xA68410) returns `lo + genrand() % (hi - lo)`. The generators are objects, not one global
stream: the vehicle shuffle uses a local generator seeded QPC × 10000 (0x660229), the grid one seeded
from `session+0x241C` = QPC × 10000 (0x669700, used in 0x5D7630), and the field manager `mgr+0x1C` is
seeded with milliseconds (0x4B1E20) in its constructor 0x642730. [I] Original fields cannot be replayed.

**Call chain.** 0x67B790 → 0x62A5F0 (reads the `free_race_track_pool` row: `max_cars`, pool name) →
0x65FE40 (`free_race_teams`) → 0x676F50(player vehicle, player livery, discipline). 0x676F50 takes the
free-race path unless a mode virtual (+0x98) is true or the mode is `le_mans_series` (0x663860, not
traced). The free-race path is 0x65FF70 (vehicles) → 0x6742C0 → 0x664630 (AI entries) and 0x66D350
(player).

**Vehicles (0x65FF70).** Every `free_race_restrictions` row whose pool *name* is the current pool
(the discipline id is not used). A vehicle is kept if it counts towards achievements (or comes from a
DLC/patch package), has an eligible livery (0x637780; eligible = not `not_in_free_race`, file name
without `_hero`/`_user`, 0x62BE40), its `ai_vehicle_type_statistics` does not contain "banned", and,
when `match_vehicle_class` is set, its class is the player's. While more than four remain, index
`rng(0, len) % len` is removed. The player's vehicle (or a counting row of the same model) overwrites
the first entry when missing. `limit_to_free_race_discipline` is not read for AI cars.

**Entries (0x664630).** The 16 `free_race_drivers` are reduced to N = `max_cars` by removing
`mgr.rng(0, len) % len`. With k = max(N >> 2, 1), entry i gets driver i, vehicle `(i / k) % len`
(skipping forward to one with liveries) and that vehicle's eligible livery `i % min(liveries, k)`.
So a field is up to four models in blocks of N/4 cars with fixed liveries. The car count is exactly
`max_cars`. The player overwrites the first entry of a randomly drawn team (0x66D350,
`rng(0, teams)`), keeping their own livery; [I] the 16 drivers are in different teams, so this is a
uniformly drawn entry. No opponent-count option exists on this path.

**Grid (0x5D7630 → 0x5B60F0).** The player's slot from the option: 1 → n − 1, 2 → n / 2,
3 → `rng(0, n) % n`, else 0. Every other car takes slots 0, 1, 2, … in car order, skipping the player's.
`grid_order` "random" is never read. [V] 0x5BE470 and 0x5C2AC0 (RACE_RULES_RESEARCH §5) belong to
the **career** grid (0x5C7D40): there the remaining AI cars sort by
`n − 1 − trunc((n − 1) · clamp(skill + U(−0.2, 0.2), 0, 1))` (ranked by 0x5B6340), and 0x5BDF10
(Le Mans series) shuffles, sorts and places the player in their class group.

Data worked through [D]: Touring keeps vehicles 9 (BMW) and 10 (Lacetti); 55 and 58 do not count
towards achievements and have no eligible livery. A 12-car field is BMW ×3 (bmw_00, bmw_02, bmw_03),
Lacetti ×3 (lac_00..02), BMW ×3, Lacetti ×3. Pro Tuned: Supra, Skyline, NSX-R, 350Z ×3 each. GT1:
six eligible vehicles, two removed at random.

**Port.** `series::build_field` implements the above; `Event::prepare` removes the entry the player
took and gives the rest, in order, as opponents 1..n; the app's opponent k then takes the k-th free
grid slot (as the original's car order) and wears the recovered livery package (`bmw_02` → `livery_02`).
AUTHORED: the seeding (vehicle draws `Mt::new(seed)`, manager `Mt::new(seed + 1)`, seed from
GRID_SERIES_SEED or the clock), keeping fewer than `max_cars − 1` opponents when the settings ask for
fewer (the first ones), and the player's grid slot (the app's existing `grid_position`). Not ported:
drivers' names and skills (the port has no driver roster), the free_race_teams list (no reader found).

Unknown: the reader of the free_race_teams list (M+0x228); how the front/middle/back/random option maps
to the slot option; whether car index follows entry order (0x649C60); 0x663860 and 0x649E90; the
cursor order of `vehicle_livery` rows ([I] table order, which reproduces bmw_00/02/03).

## Verification

Release, cargo/rustc 1.99.0 [V]:
- Tests: `cargo test --workspace --release --lib --test "*" --bin grid-app --no-fail-fast -- --include-ignored`
  with absolute GRID_* paths: 666 passed (on eee1f35), 0 failed. New: `database` (2), `car::vehicle_rows_give_the_ai_classes`,
  `series` (3: database pools and cars, MT19937 against the reference `sgenrand(4357)` outputs, the
  touring/pro_tuned/gt1 fields), `deformation_impact::spare_node_padding_edges_do_not_fail_the_step`,
  settings `series_names_a_circuit_series`; the shipped-profile test parses both new profiles.
- `cargo clippy --release --workspace --all-targets -- -D warnings`: clean.
- Identity: solo `grid-opponent-race ROOT CD01 BP01 1 11200 ai` sha256 aad9a07a…e41b and 12-car
  `… 12 11200 ai` sha256 54abbecb…2a2b, both unchanged.
- `grid-soak quick` (default): 20/20.
- `grid-soak quick --car sup --opponent-cars sup,sky,nsx,350` (DF01 on): 19/20. Seed 10 (solo Supra,
  Nürburgring GP, chaos input) sits wedged at the track with 0–10 body contacts from about tick 1000
  and drops through the world at tick 2850–3004 (`local/series/soak/quick-protuned/seed-10.*`). Not
  investigated further: a contact-robustness residual of the shared contact code, open.
- `grid-soak quick --seeds 1..4 --route R --car sky --opponent-cars sup,sky,nsx,350` on donington
  route_1 and route_3, nurburgring route_3, istanbul_park route_2, long_beach route_2 and route_5:
  24/24.
- Headless 12-car Pro Tuned fields with DF01 on Donington National (sky only, nsx only, mixed):
  12 of 12 finish 2 laps each (were stopped at the first impact before the deformation fix).
- App (`grid-app --settings settings/series-pro-tuned.ini`, GRID_AUTOPILOT=1, GRID_SERIES_SEED=42, CD01/BP01/DF01
  and the characteristics table, 12500 ticks): field `sup_00,sup_02,sky_00,sky_01,sky_02,nsx_01,nsx_02,
  nsx_03,350_00,350_10,350_11`, the player from the back of the grid reaches lap 3 of 3 in 10th place,
  best lap 1:05.093, 531 deformation bulk calls (`local/series/series-pro-tuned-end.png`, log next to
  it). Touring profile (seed 7, 2400 ticks on istanbul_int_r): field
  `bmw_00,bmw_03,lac_00,lac_01,lac_02,bmw_00,bmw_02,bmw_03,lac_00,lac_01,lac_02`, runs.
- Not checked: windowed play with sound and input, the new cars' engine sound by ear, a phone build.

## Next (after F406)

- LMP1 grounding (above); the synthetic 9-piece body hull; then the Le Mans Series field and grid
  (recovered above) with more than 12 cars.

## Next (F401)

- GT2/Le Mans/GT1 cars: why the body grounds and grip goes at speed (body–road contact filtering,
  body friction, ride height; shared vehicle/contact code).
- Le Mans Series field (0x663860) and grid (0x5BDF10); the career grid (0x5C7D40) for career events.
- The front/middle/back/random option of the free race mapped to the recovered slot options.
- AI pile-ups on Long Beach GP/cl and their reverse layouts and on Jarama, also with Lacettis.
