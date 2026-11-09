# Graphics route audit

A rerunnable check of the normal playable picture on every raceable route.

9 November 2026, Claude Code. An audit of the normal playable picture on every raceable route: an
autopilot lap per route offscreen, chase and head-cam shots at fixed lap fractions, and one verdict
table for black regions, missing textures, the sky, shadow dropouts and errors. The tool is
rerunnable; the table below is the latest full run.

## Rerun

```
powershell -NoProfile -ExecutionPolicy Bypass -File tools\build_lock.ps1 cargo build --release -p grid-app
py -I tools/route_graphics_audit.py --out local/route-audit/NAME          # every route
py -I tools/route_graphics_audit.py --only donington,haruna --out ...     # a subset (slug substrings)
py -I tools/route_graphics_audit.py --list                                # the routes only
py -I tools/route_graphics_audit.py --reuse --out local/route-audit/NAME  # re-judge existing logs
```

It prints one line per route, the totals line `route_graphics_audit routes= pass= flagged=` and
writes `summary.tsv` and `summary.md` beside the per-route folders (`run.log`, `materials.tsv`,
`shots/NN-chase.png`, `shots/NN-cockpit.png`, half resolution). The shots are proprietary renders and
stay under ignored `local/`.

## Routes

`grid-app --menu-routes [DIR]` prints every layout the event menu offers (`event_menu::Catalog::routes`),
with the event types run on it and their cars. The menu catalogue already covers every launcher
event (test `every_launcher_event_is_offered_and_written_identically`), so this list is the set of
raceable routes (78 on 9 November 2026). With DIR it also writes, per route, the settings profile the
menu itself would write (`Catalog::route_choices`): the first event type offering the layout at that
type's defaults (car, field, laps, AI, `[private] body_contacts = derive`), and for a Touge layout
also the uphill direction (subroute 1, slug suffix `-up`). The audit runs `grid-app --settings
PROFILE --drive` with that profile, so it drives the same configuration a player gets from the
menu, with a fresh data dir: the default graphics preset and the chase_close camera.

## What is measured

`GRID_ROUTE_AUDIT=DIR` (crates/grid-app/src/route_audit.rs) shoots at lap fractions 0.08, 0.27,
0.46, 0.65 and 0.84 of the player's first lap, each time the chase view and then the head-cam after
a 12-frame settle, with the HUD hidden; the run ends after the last shot. The fraction is the
distance the player car has driven since the start lights over the route length
(`lap_progress::load_route`, the AI line's total distance). AUTHORED: the lap timer's passed gates
cannot serve, because it counts only the required gates (three at Donington); a route without an
AI line falls back to 60 s of driving per lap. Per shot it prints `route_audit_metrics`:

| Metric | Definition | Flag |
|---|---|---|
| black_blob | largest 4-connected region with no channel above 4 | > 0.010 of the picture in the chase view, > 0.060 in the head-cam (the dash is legitimately dark) |
| magenta | r > 180, b > 180, g < 80 | > 0.001 |
| top | mean luminance of the top eighth (the sky in the chase view) | < 35 |
| NaN/inf | non-finite colour reaches the output as black, so it shows in black_blob | as black_blob |

Per route:

| Check | Source | Flag |
|---|---|---|
| shots | `route_audit_metrics` lines | fewer than ten |
| water | scene line `omitted_water_draws=N` (land.pssg draws of water.fx the scene leaves out) against water.rs `water spawned draws=N` | the two differ |
| shadows | `shadow_dropout frame= cause=` (GRID_SHADOW_TRACE=1, shadow_cull.rs) | a cause other than `port_cull` or `frustum` after frame 200; those two cannot change sampled texels (F745, tools/shadow_dropouts.py). A dropout within frames -1..+2 of an audit view cut (`route_audit_cut frame=`) compares the caster sets of two different views and is counted apart (`cut_dropouts`), not flagged. `not_caster` or `hidden` on a track ornament's level (`ornament/L/ID`, and the source trees' `trees/ID`, which are ornament levels too) is ornament_lod.rs crossing the instance's cull distance D, as the original drops it there (0x96EC80, docs/TRACK_LOD.md): the part hands its shadow to a fading proxy (F713), then hides; counted apart (`ornament_culls`), not flagged |
| untextured | `untextured` rows of a second run with `--float-audit` (float_audit.rs: a drawn part whose material has no loaded diffuse texture) | any row |
| errors | `panicked`, `ERROR` lines | any |

Notes, which explain a verdict but do not flag:

- `night`: the run's `drive_environment` line has `night=true` (ForcedNight, the Midnight Touge
  layouts Haruna 1/3/6/8/9, docs/TRACK_ENVIRONMENT.md). Their lighting file sets `skyscale` 0, so the
  sky is black on purpose: a black region touching the top edge and the dark top band are not
  flagged there. AUTHORED.
- `wrecked`: the result line says the player car wrecked; the lap ends early, so fewer than ten
  shots is not flagged.
- `stalled`: the car has driven less than 1 m after 1500 ticks (25 s), for example a freestyle
  drift event without rivals, where no autopilot drives the player car. route_audit.rs then takes
  the shot fractions from the 60 s clock instead (`route_audit_stall`), so the route still gets its
  ten shots of the start area.

The offscreen run uses `--render-fps 30` (the diagnostic render's lower bound) and 1280x720; the
shots are saved at half that. Thresholds are AUTHORED from the Donington calibration run: its ten
shots have no black region above 0.0001 of the picture and a chase-view sky band of 189 to 201.

## Results

Run of 9 November 2026 (`local/route-audit/full2`, 4949 s): the F761 build (the ambient fix of
FIDELITY_BACKLOG row 125, cut logging), judged by the tool as committed. yokohama_docks route_5 is
the rerun with the stall fallback (`local/route-audit/stall`). **78 routes, 78 pass.**

- No black textures: no route has an untextured drawn part (`--float-audit`), no magenta, and no
  black region above 0.010 of a chase view or 0.060 of a head-cam view, except the black sky of the
  night routes (Haruna 1 and 9, noted `night`; the sky is black on purpose, `skyscale` 0).
- Water: on all 44 routes with water, every water.fx draw of land.pssg is drawn by water.rs.
- Shadows: no caster dropout that can change the picture. The 24 ornament events are instances
  crossing their cull distance D (counted apart, see the shadows row above); no dropout fell at an
  audit cut.
- The first run (`local/route-audit/full`, the build before F761 without cut logging) had the same
  result for textures, water and the sky; its 13 shadow flags were the same ornament and source tree D crossings (re-judged with `--reuse`:
  only yokohama_docks route_5 stays flagged, that build had no stall fallback).
- Coverage: Spa route_3 wrecked in this run after 4 of its 10 shots (the autopilot's crash,
  noted `wrecked`); the first run has its ten shots and the same verdict otherwise. AI wrecks
  vary between runs.

Found and not fixed (FIDELITY_BACKLOG):

- Row 126: the macro AO darkens only the ambient light in the port, the whole lit colour in the
  original, so ground under a building shadow is darker than the original makes it (Milan route_3).
- Row 127: the Midnight Touge routes are lit like midday under their black night sky; the baseline
  scales every route's sun peak to daylight. Needs an original night capture and the exposure model.

| Route | Verdict | Shots | Worst black region | Min sky | Water drawn/source | Dropouts at a cut | Ornament culls | Notes |
|---|---|---|---|---|---|---|---|---|
| europe-donington-route_0 | PASS | 10 | 0.000 | 189 | 0/0 | 0 | 0 | - |
| europe-nurburgring-route_0 | PASS | 10 | 0.000 00-chase | 162 | 0/0 | 0 | 0 | - |
| europe-istanbul_park-route_0 | PASS | 10 | 0.000 | 164 | 0/0 | 0 | 0 | - |
| usa-long_beach-route_0 | PASS | 10 | 0.000 | 123 | 1/1 | 0 | 0 | - |
| europe-jarama-route_1 | PASS | 10 | 0.000 | 136 | 0/0 | 0 | 1 | - |
| japan-yokohama_docks-route_0 | PASS | 10 | 0.000 | 109 | 1/1 | 0 | 1 | - |
| europe-spa_francorchamps-route_0 | PASS | 10 | 0.000 | 114 | 0/0 | 0 | 0 | - |
| europe-milan-route_0 | PASS | 10 | 0.000 | 140 | 0/0 | 0 | 0 | - |
| japan-shibuya-route_0 | PASS | 10 | 0.000 | 90 | 0/0 | 0 | 0 | - |
| usa-san_fran-route_0 | PASS | 10 | 0.000 | 136 | 1/1 | 0 | 0 | - |
| europe-le_mans-route_0 | PASS | 10 | 0.000 | 161 | 0/0 | 0 | 0 | - |
| usa-washington-route_0 | PASS | 10 | 0.000 | 124 | 1/1 | 0 | 0 | - |
| usa-long_beach-route_1 | PASS | 10 | 0.000 | 124 | 1/1 | 0 | 0 | - |
| europe-jarama-route_0 | PASS | 10 | 0.000 | 121 | 0/0 | 0 | 0 | - |
| japan-okutama-route_0 | PASS | 10 | 0.000 | 60 | 1/1 | 0 | 1 | - |
| usa-san_fran-route_1 | PASS | 10 | 0.000 | 132 | 1/1 | 0 | 0 | - |
| usa-detroit-route_3 | PASS | 10 | 0.000 | 110 | 1/1 | 0 | 0 | - |
| usa-washington-route_1 | PASS | 10 | 0.000 | 126 | 1/1 | 0 | 0 | - |
| europe-milan-route_2 | PASS | 10 | 0.000 | 140 | 0/0 | 0 | 0 | - |
| europe-spa_francorchamps-route_1 | PASS | 10 | 0.000 | 137 | 0/0 | 0 | 0 | - |
| europe-nurburgring-route_1 | PASS | 10 | 0.000 00-chase | 168 | 0/0 | 0 | 0 | - |
| europe-istanbul_park-route_1 | PASS | 10 | 0.000 | 166 | 0/0 | 0 | 0 | - |
| europe-donington-route_1 | PASS | 10 | 0.000 | 182 | 0/0 | 0 | 0 | - |
| europe-le_mans-route_1 | PASS | 10 | 0.000 | 96 | 0/0 | 0 | 0 | - |
| japan-yokohama_docks-route_1 | PASS | 10 | 0.000 | 120 | 1/1 | 0 | 2 | - |
| japan-shibuya-route_2 | PASS | 10 | 0.000 | 152 | 0/0 | 0 | 0 | - |
| japan-okutama-route_1 | PASS | 10 | 0.000 | 101 | 1/1 | 0 | 0 | - |
| usa-detroit-route_1 | PASS | 10 | 0.000 | 123 | 1/1 | 0 | 0 | - |
| usa-long_beach-route_2 | PASS | 10 | 0.000 | 133 | 1/1 | 0 | 0 | - |
| europe-milan-route_1 | PASS | 10 | 0.000 | 123 | 0/0 | 0 | 0 | - |
| japan-yokohama_docks-route_2 | PASS | 10 | 0.000 | 114 | 1/1 | 0 | 0 | - |
| usa-washington-route_4 | PASS | 10 | 0.000 | 114 | 1/1 | 0 | 0 | - |
| europe-nurburgring-route_2 | PASS | 10 | 0.001 00-chase | 152 | 0/0 | 0 | 0 | - |
| japan-shibuya-route_4 | PASS | 10 | 0.000 | 99 | 0/0 | 0 | 1 | - |
| europe-donington-route_2 | PASS | 10 | 0.000 | 189 | 0/0 | 0 | 0 | - |
| japan-okutama-route_2 | PASS | 10 | 0.000 | 129 | 1/1 | 0 | 0 | - |
| europe-istanbul_park-route_2 | PASS | 10 | 0.000 | 172 | 0/0 | 0 | 0 | - |
| europe-spa_francorchamps-route_2 | PASS | 10 | 0.000 | 143 | 0/0 | 0 | 0 | - |
| usa-san_fran-route_4 | PASS | 10 | 0.000 | 138 | 1/1 | 0 | 0 | - |
| usa-long_beach-route_4 | PASS | 10 | 0.000 | 122 | 1/1 | 0 | 0 | - |
| europe-milan-route_3 | PASS | 10 | 0.000 04-chase | 117 | 0/0 | 0 | 4 | - |
| japan-yokohama_docks-route_7 | PASS | 10 | 0.000 | 117 | 1/1 | 0 | 0 | - |
| europe-nurburgring-route_3 | PASS | 10 | 0.000 | 153 | 0/0 | 0 | 0 | - |
| japan-shibuya-route_6 | PASS | 10 | 0.000 | 89 | 0/0 | 0 | 0 | - |
| japan-okutama-route_3 | PASS | 10 | 0.000 | 57 | 1/1 | 0 | 0 | - |
| europe-donington-route_3 | PASS | 10 | 0.000 | 187 | 0/0 | 0 | 0 | - |
| europe-istanbul_park-route_3 | PASS | 10 | 0.000 | 160 | 0/0 | 0 | 0 | - |
| europe-spa_francorchamps-route_3 | PASS | 4 | 0.000 | 150 | 0/0 | 0 | 0 | wrecked,shots 4/10 |
| usa-san_fran-route_5 | PASS | 10 | 0.000 | 129 | 1/1 | 0 | 0 | - |
| usa-detroit-route_4 | PASS | 10 | 0.000 | 115 | 1/1 | 0 | 0 | - |
| europe-milan-route_5 | PASS | 10 | 0.000 | 139 | 0/0 | 0 | 0 | - |
| japan-yokohama_docks-route_8 | PASS | 10 | 0.000 | 111 | 1/1 | 0 | 0 | - |
| usa-long_beach-route_3 | PASS | 10 | 0.000 | 120 | 1/1 | 0 | 0 | - |
| japan-shibuya-route_1 | PASS | 10 | 0.000 | 127 | 0/0 | 0 | 1 | - |
| japan-okutama-route_4 | PASS | 10 | 0.000 | 132 | 1/1 | 0 | 0 | - |
| usa-san_fran-route_6 | PASS | 10 | 0.000 | 128 | 1/1 | 0 | 0 | - |
| usa-detroit-route_7 | PASS | 10 | 0.000 | 103 | 1/1 | 0 | 0 | - |
| usa-long_beach-route_5 | PASS | 10 | 0.000 | 119 | 1/1 | 0 | 0 | - |
| europe-milan-route_4 | PASS | 10 | 0.000 | 108 | 0/0 | 0 | 2 | - |
| japan-yokohama_docks-route_9 | PASS | 10 | 0.000 | 122 | 1/1 | 0 | 2 | - |
| japan-shibuya-route_3 | PASS | 10 | 0.000 | 98 | 0/0 | 0 | 0 | - |
| japan-okutama-route_5 | PASS | 10 | 0.000 | 157 | 1/1 | 0 | 0 | - |
| usa-detroit-route_5 | PASS | 10 | 0.000 | 120 | 1/1 | 0 | 6 | - |
| usa-washington-route_6 | PASS | 10 | 0.000 | 111 | 1/1 | 0 | 0 | - |
| japan-shibuya-route_5 | PASS | 10 | 0.000 04-chase | 107 | 0/0 | 0 | 2 | - |
| usa-detroit-route_6 | PASS | 10 | 0.000 | 104 | 1/1 | 0 | 0 | - |
| usa-washington-route_9 | PASS | 10 | 0.000 | 100 | 1/1 | 0 | 0 | - |
| japan-shibuya-route_7 | PASS | 10 | 0.000 | 91 | 0/0 | 0 | 0 | - |
| japan-yokohama_docks-route_3 | PASS | 10 | 0.000 | 151 | 1/1 | 0 | 0 | - |
| japan-yokohama_docks-route_4 | PASS | 10 | 0.000 | 106 | 1/1 | 0 | 0 | - |
| japan-yokohama_docks-route_5 | PASS | 10 | 0.000 | 110 | 1/1 | 0 |  | stalled (rerun with the stall fallback) |
| japan-yokohama_docks-route_6 | PASS | 10 | 0.000 | 115 | 1/1 | 0 | 0 | - |
| japan-haruna-route_1 | PASS | 10 | 0.087 02-chase | 50 | 1/1 | 0 | 0 | night |
| japan-haruna-route_4 | PASS | 10 | 0.000 | 114 | 1/1 | 0 | 0 | - |
| japan-haruna-route_9 | PASS | 10 | 0.187 01-chase | 10 | 1/1 | 0 | 0 | night |
| japan-haruna-route_9-up | PASS | 10 | 0.187 01-chase | 28 | 1/1 | 0 | 1 | night |
| usa-detroit-route_8 | PASS | 10 | 0.000 | 136 | 1/1 | 0 | 0 | - |
| usa-detroit-route_9 | PASS | 10 | 0.000 | 143 | 1/1 | 0 | 0 | - |
