# Le Mans 24 Hours: Circuit de la Sarthe, time-limited race, classes, time of day

Status: 2026-10-18, Claude Code (docs/DISCIPLINES.md §5 item 7). Static analysis
of the local `grid-unpacked.exe` and the shipped data only; the original game
was not run. Evidence (ignored): this worktree's `local/lemans/` (runs, soak,
decoded data) and the main checkout's `local/lemans/` (Ghidra 12.1.4 headless,
read-only project copies of the drift-ai project; `decomp/`, `race-type3.md`,
`time-of-day.md`).

Tags: [V] observed in the files, the executable or a run; [D] database or
shipped data; [I] interpretation; AUTHORED: a port choice with no original
counterpart.

## 1. Track

### 1.1 Data [D]

| `track_model` | id | route | length | supports_time_of_day | headlights_on | vehicle_lights_on |
|---|---|---|---|---|---|---|
| `le_mans_circuit` | 6 | Route_0 | 13.7 | 1 | 0 | 0 |
| `le_mans_classic` | 25 | Route_1 | 13.5 | 0 | 0 | 0 |

`track` 28 `le_mans`, additional ground clearance 0. Only `le_mans` ships a
`daynight.xml` (also `street_lights.xml`, `headlights.pssg`). The four career
events `gt2/gt1/lmp2/lmp1_leman24` (event ids 13–16, discipline 11
`Endurance`, competition format 1 `single_race`) use track model 6 with
`num_laps` 8 and `time_limit_seconds` 720; player grid base/range LMP1 1/4,
LMP2 5/3, GT1 7/12, GT2 11/19. The free race `Le_Mans` (discipline 18) uses
pool 190 and `free_race_times` 12, 24, 60 min, 2, 4, 6, 12, 24 h.

### 1.2 Loading [V]

Every reader passes on route 0 since F399 (`grid-route-readers stages`):
collision 161 chunks, boundary, 20 grid slots, progress 158 gates, AI line
`touring/lac`, start lights (5.0 s, 7 stages), 1368 entity placements, 277
ornament prototypes, objects (1854 sources), props (2413 boxes), tracksplit,
land, trees, sky. F399's survey recorded a solo wreck on route 0; on
9bafbfb (F412) the solo Lacetti laps.

Route 0 (`route::LE_MANS`, the eleventh entry of `route::CIRCUITS`):
progress 158 gates over 13 629.96 m, finish gate 0, time splits at gates 57
and 99; AI line 13 519.05 m; breakables 173 tyre stacks and 38 brake boards;
1020 static armco, foam-post and tyre-wall pieces (joint rows not ported).

### 1.3 Races (headless, release, `grid-opponent-race`) [V]

| Run | Result |
|---|---|
| solo Lacetti, 2 laps | 236.938 / 238.615 s, no contact |
| 12 Lacettis, 2 laps (32000 ticks) | 10/12 finish (winner 7:44.080, lap 229.885 s); car 8 wrecked at tick 2384 (first corners), car 1 at 14390 |
| 12 GT/LMP cars (dbr, c6r, sal, c5r, lam, spk, esp, 911, c65, cre, b05, dbr), 26000 ticks | C65 and B05 (LMP2) and a DBR9 finish 2 laps (best 189.3 s); four cars lose a wheel or wreck before tick 13500; the C6R stops before the end of lap 1 (cause not traced) |
| route 1 (`le_mans_classic`) solo | wrecked at tick 13582; not raced yet |

`grid-soak quick --seeds 53..56` (2, 12, 9, 4 cars on route 0, 5000 ticks; the
seeds were 33..36, then 37..40, until the Demolition Derby and Spa took those). They stay
outside the default set (1..24); an eleventh of the long seeds move to Le Mans
from their own stream (`seed ^ 0x1e3a…`).

Launcher: `Play GRID Race Le Mans.cmd` (settings/race-le-mans.ini: two laps,
Lacetti, 11 opponents, back of the grid).

The 24-hour DBR9 field (below, §2.5) at Sarthe, 16 cars, 14000 ticks: 10
cars complete lap 1 (best 203.0 s); both Audi R10 LMP1s lap (205.9 / 209.8 s),
so the R10 does not ground here as the LC70 does; a 911 (tick 5344) and a
C65 (tick 13179) wreck; both C6Rs stop at 0.99 laps near (−775, −1160), as
the C6R did in the 12-car GT field; two 911s are slow (0.72 laps).

## 2. Time-limited race (race type 3) and results

Static analysis (`local/lemans/race-type3.md`, decompiles `decomp/a1..a18`).

### 2.1 Which races [V]

Race setup 0x669700 writes the race type at 0x669B0D. Mode vf 0x98 ("Le Mans
24") selects type 3: the free race when its discipline is `Le_Mans`
(0x62A5D0, a name compare), the career when its byte +0x50 is set (0x67F5C0;
the setter was not found [I: the four `*_leman24` events]). Drift Freestyle
is type 3 too, with no clock scaling. `le_mans_series` stays a lap race
(type 1).

### 2.2 Clock [V]

- All times are int64 µs. End = 24 h (0x908300(24,0,0,0)); scale = 24 h /
  the event's real time limit (720 s → 120). The conversion of
  `time_limit_seconds` / `minutes` into the limit was not traced [I: s × 1e6].
- The race clock (manager +0x28) starts on the `racestart` event (green
  light, 0x63E300 → 0x90DE60) and gains dt per manager update; it freezes at
  finalisation; a flashback restores it (0x90DEC0).
- The scaled clock (0x840520) = clock × scale. Race over (0x90E1B0, type 3)
  when it reaches 24 h, i.e. when real race time reaches the limit.

### 2.3 Laps, expiry, classification [V]

- The gate handler 0x912170 records no finish in type 3; laps count up to
  the event laps (8), unlimited for 0 or ≥ 52 (0x90E020: 50 and a flag).
- At expiry the race ends at once (no last lap, no flag). Finalisation
  0x911F30 → 0x90F880 per car: when the human drove to the limit, every car
  still racing is classified (status 1) with time = the 24 h end. When the
  human ended early (race time < limit), the human retires and each AI car
  is projected to the limit lap by lap with the AI lap estimator and marked
  estimated (entry +0xBF4).
- Live order (0x908CF0/0x90B5B0): classified cars first, by progress (laps +
  lap fraction), then the rest by progress. Class position = rank within the
  class bucket `clamp(lemans_sort_order, 0, 5)` (vehicle +0x2E58, written by
  0x5ED920): LMP1 1, LMP2 2, GT1 3, GT2 4, other cars 0.
- Results order (0x4EC820, Le Mans only): every entry by progress × a
  race-wide float × (`lemans_progress_adjustment` if estimated), descending,
  with no finished-first rule. 0x4D1600 is the adjustment's only reader, so
  LMP1 1.08 / GT1 0.92 / GT2 0.84 only matter after an early human end.
- [V observation, I bug] 0x661830 takes best lap and time of result row i
  from classification position i instead of entry i (not ported).

### 2.4 Results screen [V]

Headers 0x52E2C0 (`le_mans_leaderboard`): Pos, Driver, Class
(`post_race_positions_table_car_class_header`), Time, Car, Team (+ Points in
the career). Row filler 0x53FE10: class cell `le_mans_%s` (vehicle class
`file_string`), time `%02u:%02u.%02u` in total minutes (24 h prints
`1440:00.00`), `--:--:--` for zero, `DNF` when not classified. The HUD
countdown 0x80E5E0 returns max(24 h − scaled clock, 0); its caller and
format were not found.

### 2.5 24-hour field [V]

0x663860 takes the `lemans_24` value to match: free race `Le_Mans` passes 1
(0x677009), `le_mans_series` 0 (0x677040), career events with
`use_lemans_selection` the career +0x50 byte. Both use the class-sorted grid
0x5BDF10. DBR9 24 h field [D]: 16 cars (dbr ×4, 911 ×3, c6r ×2, two Audi R10
LMP1s, four Courage C65 LMP2s, lam); Spyker 17.

### 2.6 Port

- `grid_tools::race::Standings::with_time_limit(seconds)`: no line finish,
  lap cap (`UNLIMITED_LAPS`), classification at the limit, `progress`, the
  type-3 `order`, `finalize` for an early human end (AUTHORED projection: the
  car's mean race speed, as in lap races; the estimator 0x67FFB0 is not
  ported), `lemans_results_order`, `class_positions`, `format_lemans_time`.
  No chequered flag in a time-limited race.
- `grid_tools::series`: `is_le_mans_24`, `build_lemans_field_for(db, car,
  lemans_24)`, `lemans_class` (class name, sort order, adjustment); series
  `le_mans` (the free race `Le_Mans`, matched case-insensitively) in
  `CIRCUIT_SERIES`; `Event::prepare_on_grid` builds its field and grid as for
  the Le Mans Series.
- grid-app: `[game] series = le_mans` makes the race time-limited
  (`[drive] time_limit`, default 720 s; laps default 8, the cap); the
  player's trial ends at the limit; `le_mans_event` keeps the clock, logs
  `drive_time_of_day` and `drive_lemans_result`, shows its own HUD line
  (clock, time remaining on the race clock, lights) and reorders the results
  by 0x4EC820 with the time on the race clock (the shared results table
  prints `1440:00.000`; the Class column comes from F413's `CarClasses`).
  The event menu offers it (`Kind::LeMans`, `free_race_times` minutes).
- Launcher `Play GRID Le Mans 24 Hours.cmd` (settings/le-mans-24.ini: DBR9,
  720 s, back of the GT1 group; no GRID_SERIES_EXPERIMENTAL since F420).
- AUTHORED / not ported: the career trigger (+0x50 setter), the race-wide
  results factor, the expiry observer (+0xF1E4), the HUD countdown's format,
  the clock while paused (the port's race time follows the simulation tick,
  so it stops while paused, as inferred for the original), the estimator.

## 3. Time of day

Static analysis (`local/lemans/time-of-day.md`, decompiles `decomp/b1..b9`,
reference model `scripts-b/tod_model.py`).

### 3.1 Mode and clock [V]

- Mode at track load (around 0x60C040, lighting manager 0xE85FE0 +0x8D58): 1
  if `supports_time_of_day`, else 2 if `headlights_on` (Haruna routes 1, 3,
  6, 8, 9: headlights always on), else 0. Only `le_mans` Route_0 has mode 1.
- Start time 15:00, fixed (0xCB3BA8, written only in the session constructor
  0x911DC3; no option, no random).
- Per frame 0x90D5A0: in a type-3 race `t = fmod(clock_µs × scale × 1e-6 /
  86400 + 15/24, 1)`; otherwise, in mode 1, t stays at 15/24. Replays use the
  replay clock. So the career's 720 s run the whole day: 15:00 → dusk →
  night → dawn → 15:00; free races 12 min ×120 … 24 h ×1.
- Day/night class 0x4178E0: day when 5 ≤ t·24 ≤ 20 [I: audio/ambience].

### 3.2 daynight.xml [V]

Reader 0x9DD900 (resource `daynight`): every `clock*` attribute gives a key
time `(hhmm % 100 + hhmm / 100 · 60) / 1440`; every other attribute
(case-insensitive) is its own sparse curve (a `…minus` suffix sets the
incoming value; the Le Mans file has none). Finalise 0x98CE20: a copy of the
last key a day earlier and of the first a day later, sort; only
`rotationangle` (period 360) and `skyTime` (period 1) get split keys where
the value jumps by more than half a period. Evaluate 0x9ACE90: linear from
key i's outgoing to key i+1's incoming value; floats as is, u8 colours
`clamp(round(v), 0, 255)` after the registered scale (×256 for 0..1
colours, ×1 for the grade colours), booleans `v > 0.5`. Attributes with keys
overwrite their lighting.xml values every frame; the rest keep them.

Le Mans data [V, `grid_tools::time_of_day` asset test]: 124 timepoints, 67
attributes; headlights wanted from 20:45:59 to 05:11:30; shadow maps from
05:11:27 to 21:26:30; sky slot 2 at 15:00. With the ×120 career race the
headlights come on at race time ≈ 173 s and go off at ≈ 425.5 s.

### 3.3 Headlights [V]

`CMR6Lighting` (0x735740) registers `enableHeadlights` (bool, > 0.5),
`lightStrength`, `headlightdirectional` (default 0.3), `lightColour_R/G/B`.
Global fade 0x73B890 per frame: when wanted, `F = max(F, 0) + dt`; when not,
`F = min(F, 9) − dt` until below 0, then every car's lights go off; within
0.25 s of a race (re)start it snaps (F = 9, or off). Intensity
`clamp(F, 0, 1)`. Race init 0x727B20: per car slot a delay `3 + rand_u8 ·
6/255` s; per car 0x736020: lights on when `(F − d_i) · 4 > 0`, projector
weight `clamp((F − d_i) · 4, 0.001, 1)`. Mode 2: always on.
`headlights.pssg` holds only the projector texture `headlight.tga`.
Night-emissive objects (`object_simple_night_emissive*`) take the `emissive`
curve in mode 1 (3.0 at night). `street_lights.xml` (163 lights: position,
direction, range 8–20, brightness 0.8–4, parsed by 0x732970) has no
time-based switching found [I: it follows the night light path].

### 3.4 Port

`grid_formats::daynight` (reader) and `grid_tools::time_of_day`: `Mode`,
`clock_scale`, `day_fraction`, `is_day`, `Curve` (wrap, unwrap, linear),
`Timeline` (`value`, `flag`, `values` — the renderer hand-off),
`sky_textures` (0x95FCE0: `f = fmod((skyTime + 10) · 6, 6)`, textures `f`
and `f + 1` of six, blend `frac(f)`), `Headlights` (fade, delays, per-car
weight). The app keeps the state in `le_mans_event::LeMansEvent` (also on
Sarthe lap races, where the clock stays at 15:00) and logs it. AUTHORED: the
per-car delays come from a fixed generator. Nothing is drawn: no app
headlight geometry exists to switch.

### 3.5 Renderer hand-off (graphics side, not ported here)

Per frame in mode 1, `Timeline::value(name, t)` gives:

| Attributes | Consumer |
|---|---|
| heightangle, rotationangle (spec_* absent) | sun direction: `Sun::direction_to_light` (track_lighting.rs, G082 mapping) |
| skyScale, skygamma; skyTime | sky `skyScaleAndInvGamma`; texture pair and blend (`sky_textures`; sky.pssg sky_lemans_1..4, slot order [I]) |
| shadow_strength, enableShadowSystem | shadow strength; shadow maps on/off (also lerps interior_ambient_shadow) |
| fogDistance, fogDensityCap, fogColourBrightness, fogColour_R/G/B | fog (lighting.xml names) |
| backlight_suncolour/backcolour/ambientcolour_R/G/B, backlight_sunscale/scale/ambientscale | backlight lighting (lighting.xml names) |
| pfx_sunBrightness, pfx_suncolour_*, pfx_ambient_brightness, pfx_ambient_*, pfx_unlitambient_* | particles |
| min/max_specular_power, max_specular_scale | specular |
| lumMin, lumMax, adaptMax, lumThreshold, toggleThreshold | tonemap / eye adaptation |
| intensity, mixMedium, mixLarge | bloom |
| contrast, colour, tintAmount, tintColour_*, vignetteAmount, vignetteColour_* | colour grade / vignette (colours 0..255) |
| luminanceFactor, luminanceFactorHeadcam | exposure (cockpit view: headcam) [I] |
| enableHeadlights, lightStrength, headlightdirectional, lightColour_* | headlight projectors: `lightBias = (0, d, 1 − d, strength)`, `lightColour × lightStrength`, beam y −0.45 (mode 1) / −0.8 (mode 2) |
| emissive | night-emissive objects |

Open on the graphics side: the projector maths and `renderScale`, the
exposure target 0xE5EBE8, renderer branches 0x96F420 / 0x971A20 in mode 1,
the street-light binding.

## 4. Open

- **AI at two places on route 0** (handed to the AI work: PROJECT_LOG F426, F427 and
  docs/BRAKING_REFERENCE.md). The solo Lacetti and BMW wreck in the last chicane before the
  line (arc ≈ 13 400, (−781, −1093), 0.988 laps): the AI line is ~7 m wide at ~52 m/s there.
  The outcome is marginal: with DF01 and the AI characteristics set, e548701 laps and 57b406d
  wrecks; without them both wreck (F426: the engine reset is not the cause). With 12 Lacettis
  on 77c585a every car completes lap 1, then car 0 wrecks there and 8 stall behind it. The
  DBR9 24 h field on 77c585a mostly wrecks at the Dunlop/Esses (1120..1150, −550..−680,
  ≈0.28 laps).
  The headless numbers in §1.3 are from 9bafbfb/9bd3832, before F417.
- Route 1 (`le_mans_classic`) wrecks solo; the C6R stops at about 0.99 laps
  near (−775, −1160) in both GT fields (cause not traced); four cars of the
  12-car GT field wreck early at speed.
- The career 24 h trigger, the HUD countdown's format, the race-wide results
  factor, the AI lap estimator 0x67FFB0.
- Rendering the time of day (section 3.5) and drawn headlights.
