# Long Beach (fourth circuit) and later finish gates

Status: 2026-10-17, Claude Code (night run). Long Beach, `tracks/usa/long_beach/route_0`,
is the fourth circuit after Donington, Nürburgring ([TRACK_GENERALIZATION.md](TRACK_GENERALIZATION.md))
and Istanbul Park ([ISTANBUL_PARK.md](ISTANBUL_PARK.md)). It is the first street circuit. It
drives solo and races with the 11-car AI field in the app. Its quick soak is run but not in the
default pre-push set, because tyre stacks fall through the road (section 5). The same session
also lets lap timing accept routes whose finish is at a later gate (section 6).

Evidence (ignored): this worktree's `local/fourth-track/`, copied to the main checkout's
`local/fourth-track/`.

## 1. Choice

Candidates from the F350 open list, checked on 8b8a48b:

| Track | What blocks it | Work needed |
|---|---|---|
| Spa-Francorchamps | One `track.jpk` chunk (of 173) has 9 materials; the VCQTC reader caps them at 8 (authored; the 4-bit triangle field allows 16). With the cap at 16, a prop mesh fails `nonfinite half UV` (a half-float stream with values like 636.5 and NaN, so not a UV stream); past that, the packed body-contact path refuses the chunk (`packed body material count exceeds bound`, `packed_contacts::materials_valid` requires indices < 8). | reader cap, a mesh stream layout and the packed-contact material bound, each needing static research |
| Jarama route 0 | The AI leaves the track at ~61 m/s (late brake-line pair, F304) | AI braking research; needs a live Jarama trace |
| **Long Beach** | nothing: every reader, lap timing and the AI already work; only the app needed an optional sponsor package | none in the formats |

Chosen: **Long Beach route 0**, because it needs no new format work. Spa's three blockers are
recorded above (evidence `local/fourth-track/spa-*.txt`); none of them was changed in the code.

## 2. Route data [V: asset tests]

| Piece | Long Beach route 0 | Test |
|---|---|---|
| Collision | 101 `track.jpk` chunks within the reader limits; keys CONC, GRAS, RMBL, SHTC, STDR; every key except GRAS mixes to the CAR_ row, as on the other circuits | `route::tests::circuit_barrier_keys_mix_to_the_car_row` |
| AI racing line | optimal line 3580.5 m (`touring`/`lac`) | `route::tests::fourth_circuit_sources_load` |
| Start grid | `slot_00`..`slot_11` all build a Lacetti runtime with lap timing | same |
| Lap / sector timing | 101 gates over 3655.78 m; finish gate 0, splits at gates 33 and 69 | same |
| Dynamic props | 1325 prop boxes; 124 breakable tyre stacks (single and double, blue/red/white); 275 tyre walls (`tyrewall_*`) need the joint rows the port lacks and stay static, as on Donington | same |
| Reset lines (F341) | a car flipped on `slot_00` is put back upright on the line (fired at tick 351, 125 ticks after it became still), drives off at 32.9 m/s | `player_reset::tests::a_flipped_player_car_is_reset_on_long_beach` |
| Start lights | `tracks/usa/long_beach/start_lights.xml`: Donington's board object `startlights_lights_horizontal`, but 7 stages over 5.0 s (textures 1..7), so races count down 5.0 s | `start_lights::tests::circuits_ship_their_own_start_sequences` |

The street walls are static prop boxes (concrete fences) and track geometry; none of the tyre
stacks has `track.jpk` geometry within 2 m sideways.

## 3. App

- Long Beach ships no `lemans_sponsors.pssg` (the three European tracks do). The app now skips
  that package when the track lacks it; any object binding that needs it still fails as
  "object texture unresolved", and none did.
- Static render (`--scene reference --view start`, `long-beach-start.png`, checked): the start
  straight between concrete walls with catch fences, the gantry and its light board, grandstand
  and city skyline. Track scene 518 draws (70 unresolved batch draws omitted, so the road is
  flat grey, as on Nürburgring and Istanbul), objects 2692 placed draws, 399 entity placements
  (210 empty-renderable instances omitted, the F304 fallback), `start_lights enabled=true
  stages=7 sequence_seconds=5`.
- Windowed race (`GRID_AUTOPILOT=1 grid-app --route tracks/usa/long_beach/route_0 --drive
  --time-trial-laps 1 --body-contacts CD01 --body-events BP01 --deformation DF01 --opponents 11
  --ai-characteristics TSV --drive-ticks 6000`, `long-beach-race-app.log`): 5.0 s countdown;
  autopilot lap 1 valid 1:22.763 (28.746 / 28.600 / 25.417); all 12 classified (winner AI 1
  1:23.024, player +1.369, last +7.936); no failure, retirement or auto-reset.
- Launcher `Play GRID Race Long Beach.cmd` with `settings/race-long-beach.ini` (race.ini with
  `[game] route = tracks/usa/long_beach/route_0`).

## 4. Headless behaviour

- Solo AI (1 car): 81.342 / 78.816 s, state `43e27d38e72618f8`. Trace `solo-trace.tsv`: two
  short off-track `Recovery` episodes at racing-line distance ~2664-2700 m (ticks 3748-3811
  and 8711-8736, ~36.5 m/s); the largest line error is 8.2 m at ~2571 m. Both laps valid.
- 12-car AI race (2 laps, `race12-ai.txt`): 11 cars 2 valid laps (winner 2:43.312, last
  classified +15.024 s); car 8 wrecked by terminal damage at tick 1643 (a first-lap crash on a
  narrow street circuit; not investigated further).

## 5. Soak

- Generator: quick seeds 17..20 (12, 1, 12, 6 cars) run Long Beach; a quarter of the long
  seeds move to Long Beach from their own stream (`seed ^ 0x10b3...`), after the Istanbul draw,
  so the other fields of a seed are unchanged. **The default quick set stays 1..16**: run the
  Long Beach quick set with `grid-soak quick --seeds 17..20`.
- Forced quick set (`grid-soak quick --route tracks/usa/long_beach/route_0 --seeds 1..12`,
  `soak-quick-forced/`): **5/12 pass, 7 fail**, every failure `out-of-bounds: prop piece falling
  through the world`, mostly at the stacks around (-512, -86), also (-390, 110) and (-369, 42).
- Diagnosis (replay of solo seed 10 with `GRID_SOAK_TRACE_BODY`): the stack pieces spawn 5-10 mm
  above the road (checked for every stack near the failures), upright, with ordinary placements
  (det 1). At tick 3844 the player drives over the stack at 34 m/s (208 car contacts the next
  tick). In that one tick the bottom tyres (cylinders 0.148 m tall, ~17 kg) are turned about 90°
  and pushed 0.4-0.7 m into the road under the car's floor, below the road surface (2.12), and
  then sink and free-fall. So it is the existing "thin piece crushed under a car" weakness of
  the prop contact solving (the F342/F351 residual family), not a loading error; street-circuit
  stacks sit at the edge of the racing surface, so cars run over them far more often than on
  Nürburgring. Not fixed here: it needs solver work in the soak-residuals area. Seeds 17..20
  join the default set once it is fixed.

## 6. Finish gates other than 0 (multi-route progress item)

The refused "multi-route" progress files are not multi-route: Donington routes 1-3, Nürburgring
route 3, Jarama route 1 and Le Mans 0-1 each have one forward route whose finish split is at a
later gate (read: Donington route 1 at gate 54, Nürburgring route 3 at 72, Jarama route 1
at 67, Le Mans route 0 at 157). `lap_progress::load_route` now re-indexes such a
circuit so the finish gate is gate 0: gates and points are rotated and their distances measured
from the finish, and the splits follow (`start_at_finish`). A finish already at gate 0 is left
exactly as read, so Donington, Nürburgring, Istanbul Park and Long Beach route 0 are unchanged.
Tests `lap_progress::tests::a_later_finish_gate_becomes_gate_zero` (synthetic) and
`routes_finishing_at_a_later_gate_load` (asset).

Solo AI with the re-indexing (`finish-*.txt`, 1 car, 24000 ticks):

| Route | Line | Result |
|---|---|---|
| donington/route_1 | 3156.5 m | 2 valid laps, 64.517 / 62.314 s |
| donington/route_2 | 3943.8 m | 2 valid laps, 87.945 / 86.251 s |
| donington/route_3 | 3152.0 m | 2 valid laps, 64.325 / 62.877 s |
| nurburgring/route_3 | 3549.8 m | 2 valid laps, 87.963 / 85.240 s |
| jarama/route_1 | 3829.4 m | 2 valid laps, 89.557 / 86.612 s |
| le_mans/route_0, route_1 | 13519 / 13472 m | no lap: the AI crashes and loses a wheel near the end of lap 1 (ticks 14320 / 13580) |

These routes are not in grid-soak or the launchers yet; they can be selected with `--route`.

## 7. Open

- Tyre-stack falls under cars (section 5); then add seeds 17..20 to the default quick set.
- Spa: the three blockers of section 1.
- Le Mans: the lap-1 crash; Jarama route 0: late braking.
- Istanbul turn-8 and Long Beach ~2.7 km recoveries: a comparison with the running original
  needs a live AI trace on those tracks, which needs input to the game (not done in a night run).
- The flat grey road (unresolved track batches) and the prototype launcher's Donington banner.
