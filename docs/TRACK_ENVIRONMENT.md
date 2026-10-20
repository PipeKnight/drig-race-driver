# Track time of day, sky, fog and weather

19 October 2026, Claude Code. This inventories what the original's track and event data define
about time of day, weather, wet surfaces, sky and fog, and how the original picks among them. It
also covers the port's data model (`grid_formats::track_environment`, `grid_tools::environment`,
grid-app `track_environment::TrackEnvironment`).

This document covers data and selection only. Rendering belongs to the graphics side (Codex). The
Le Mans day/night clock and headlights belong to the Le Mans session (docs/LE_MANS.md §3,
`grid_tools::time_of_day`, `grid_formats::daynight`). Static research only: nothing was run against
the original.

Markers: **[D]** read directly from the executable or shipped data, **[V]** verified by a test,
**[I]** interpretation.

## 1. Evidence

- Shipped files:
  - every `tracks/**/lighting.xml`. Seven are text XML; the rest are sectioned binary XML, decoded
    with `grid-binxml` and `tools/binxml_tree.py` into this worktree's `local/weather/light/`;
  - `effects/weather_fx.xml` and `effects/weather_dataset.xml`;
  - `tracks/europe/le_mans/daynight.xml`;
  - the `track_model` table of `database/database.bin`, surveyed into
    `local/weather/track_model_flags.txt`, 99 rows.
- Executable: Ghidra 12.1.4 read-only pseudocode of the pinned unpacked image (`aac46a64…`), in
  `local/weather/decomp/w1`, `w2` (private).

## 2. What the data defines

### 2.1 Lighting presets (`lighting.xml`) [D]

`<lighting>` holds `<param name="value"/>` entries:

- sun height and rotation (`heightangle`, `rotationangle`, and the `spec_` copies);
- sky tone (`skyscale`, `skygamma`);
- the sky's rain layer (`sky_rain_offset/scale/map_speed/mask_*`, `sky_rain_colour`);
- shadow and AO scales;
- specular range and colour;
- backlight sun, back and ambient colours and scales;
- fog (`fogTopLevel`, `fogBaseLevel`, `fogDistance`, `fogDensityCap`, `fogColour`,
  `fogColourBrightness`);
- `HDRRange`;
- light shafts;
- particle colours;
- `headlights`.

The executable also reads an optional `<WEATHER rainfall=…>` element (0x72AC90, strings
`WEATHER` / `rainfall`). No shipped file has it.

One preset per track, plus per-route presets on Haruna:

| Lighting file | Sun height / rotation (deg) | skyscale / skygamma | fogDistance / cap | fogColour | HDRRange | Sky rain layer |
|---|---|---|---|---|---|---|
| europe/donington | 144.9 / 163.6 | 0.8 / 0.7 | 500 / 0.4 | 0.68 0.75 0.91 | 1.82 | — |
| europe/istanbul_park | 142.6 / 360 | 0.8 / 0.9 | 450 / 0.6 | 0.75 0.80 0.93 | 1.82 | yes |
| europe/jarama | 151.9 / 112.2 | 1.0 / 1.0 | 300 / 0.5 | 0.56 0.54 0.44 | 1.82 | — |
| europe/le_mans | 139.0 / 62.5 | 1.0 / 0.735 | 434 / 0.70 | 0.49 0.46 0.50 | 1.50 | — (daynight.xml drives it) |
| europe/milan | 136.3 / 102.9 | 0.7 / 0.8 | 350 / 0.3 | 0.45 0.57 0.64 | 1.10 | yes |
| europe/nurburgring | 140.5 / 275 | 0.75 / 1.0 | 400 / 0.3 | 0.80 0.80 0.95 | 1.82 | — |
| europe/spa_francorchamps | 146.7 / 220.6 | 0.75 / 1.0 | 600 / 0.7 | 0.64 0.69 0.80 | 1.60 | — |
| japan/haruna (routes 0, 2, 4, 5, 7) | 158.2 / 41.6 | 1.0 / 0.6 | 250 / 0.95 | 0.55 0.57 0.65 | 1.33 | — |
| japan/haruna/route_1, 3, 6, 8, 9 (night) | 90.5 / 135.3 | **0** / 1.0 | 1000 / 0.95 | 0 0 0 | 1.91 | — |
| japan/okutama | 146.7 / 77 | 0.65 / 0.9 | 2000 / 0.9 | 0.37 0.37 0.42 | 1.04 | yes |
| japan/shibuya (night) | 93.6 / 177.7 | **0.03** / 1.0 | 100 / 0.2 | 0.27 0.20 0 | 1.97 | — |
| japan/yokohama_docks | 156.6 / 299.2 | 0.5 / 0.6 | 300 / 0.8 | 0.35 0.34 0.37 | 1.92 | — |
| usa/detroit | 142.5 / 197.7 | 1.0 / 1.0 | 450 / 0.5 | 0.67 0.62 0.75 | 1.39 | yes |
| usa/long_beach | 131.1 / 176.9 | 0.8 / 0.9 | 400 / 0.8 | 0.45 0.45 0.51 | 1.73 | yes |
| usa/san_fran | 142.6 / 32.7 | 1.0 / 0.7 | 300 / 0.6 | 0.42 0.52 0.59 | 2.02 | yes |
| usa/washington | 154.3 / 56.1 | 1.2 / 0.45 | 400 / 0.5 | 0.48 0.53 0.74 | 1.17 | yes |

Notes:

- `headlights` is 0 in every file. The three front-end garages have presets too.
- Sun height 90° puts the sun at the horizon in the original's convention: heights of 131°–158° are
  daytime suns. The night presets have `skyscale` 0 or 0.03 (a black sky), black fog and a horizon
  sun [D values; I meaning].
- "Sky rain layer" means `sky_rain_scale_u` = 20 and `sky_rain_map_speed_u` = 0.1: a scrolling
  cloud or overcast layer of the sky shader (`skyRain*` constants), not rain. The executable's
  `sky_main` and `sky_env` shaders are the graphics side's (docs/SOURCE_SKY_LIGHTING.md).

### 2.2 Route flags (`track_model`) [D]

| Flag | Routes |
|---|---|
| `supports_time_of_day` = 1 | le_mans Route_0 only |
| `headlights_on` = 1 | haruna Route_1, 3, 6, 8, 9 |
| `vehicle_lights_on` = 1 | 28 rows: Haruna Route_0–9 (all 10), Shibuya Route_0–7 and Yokohama Docks Route_0–9 |
| all 0 | the other 70 rows |

`Okutama` has no light flags. Its preset is a daytime one.

The Le Mans session recovered the selection (0x60C040): mode 1 is the daynight.xml timeline when
`supports_time_of_day` is set; otherwise mode 2 (forced night, headlights always on) when
`headlights_on` is set; otherwise 0, fixed lighting. At race start (0x596440), the original reads
`vehicle_lights_on` and `headlights_on` from the row. When the track is not a headlights track, it
passes `vehicle_lights_on` to the cars' light state (0x952010, 0xA08DA0). The branch order comes from
damaged pseudocode [I].

### 2.3 Event settings

The event and career tables have **no time-of-day or weather column** (schema survey: only
`track_model` has light flags) [D]. An event picks its environment only through its route. The night
Haruna routes 1/3/6/8/9 are the Midnight Touge layouts (docs/TOUGE.md: format 6
`midnight_touge`).

The 24 h clock of a time-limited Le Mans race is the Le Mans session's: it starts at 15:00 with a
scale of 86400 s / limit.

### 2.4 Weather and wet surfaces

- **The code supports rain** [D]:
  - the lighting file's `WEATHER rainfall` (0x72AC90);
  - the `WeatherFxPlugin` dataset (`effects/weather_dataset.xml`: `weather_fx.xml`,
    `rainfall.pssg`, `windscreen_fx.pssg`);
  - the `environment_rain` sound, started when the rain flag is set (0x5C7860);
  - the `rainEffectLevel` material constant, reset to 0 on material setup (0x9D01E0);
  - `wetLightingScale` in shaders;
  - surface labels `wet tarmac` and `wet gravel`;
  - an ambience-trigger parser with `condition` DRY/RAIN and `timeOfDay` DAY/NIGHT (0x4570E0,
    0x45A690). No loose shipped XML uses those attributes [D grep]. The tracks'
    `ambience_triggers.xml` hold reverb zones only.
- **The shipped data disables it** [D]:
  - no lighting file has `WEATHER`;
  - `weather_fx.xml` has `<rainfall enable="0">`, and its windscreen `drop_enable="0"` and
    `wiper_enable="0"`. The windscreen dust layer stays enabled.
- So **every shipped race is dry, and there are no wet surfaces in play** [D data; I: no other path
  enables rain]. This matches the retail game, which has no weather option.

## 3. The port

- `grid_formats::track_environment` [V unit and shipped-file tests]:
  - `read_lighting` reads both the text and the binary form into every `param`, plus `WEATHER`;
  - typed accessors: `sun()`, `scalar`, `rgb`, `headlights()`, `sky_rain()`;
  - `read_weather_fx` reads the rainfall and windscreen switches and keeps every attribute.
- `grid_tools::environment::Environment::for_route(root, route)` [V shipped routes]:
  - the route folder's `lighting.xml` when present, else the track's [I order];
  - the `track_model` flags;
  - `weather_fx`;
  - `mode()`, the Le Mans session's `time_of_day::Mode` from the same row (a test checks that it
    equals `Mode::for_route` on the shipped routes); `night()` (forced night), `vehicle_lights()`
    and `raining()` (always false with shipped data).
- grid-app `track_environment::install` inserts the `TrackEnvironment` resource when a drive starts.
  It logs:

  ```
  drive_environment version=1 lighting=<file> mode= night= vehicle_lights= rain= sun=h,r sky=scale,gamma fog=dist,cap sky_rain=
  ```

- Nothing renders differently. The simulation is untouched: the resource is read by nothing in the
  driving systems.

### 3.1 Hand-off to the renderer (Codex)

Read `Res<TrackEnvironment>`:

- `.0.lighting` holds every preset value of the route's lighting file. Use `scalar` / `rgb` /
  `values` by the names in §2.1. The existing graphics readers (`track_lighting::read_sun`,
  `read_fog`) read only the text files. This reader also covers the binary presets of Jarama,
  Milan, Haruna, Okutama, Shibuya, Yokohama, San Francisco and Washington.
- `.0.mode()`: `Timeline` → drive the lighting from `time_of_day::Timeline::values(t)` (Le Mans,
  docs/LE_MANS.md §3.5); `ForcedNight` → `.0.night()`, so the night presets need car headlights.
- `.0.vehicle_lights()` → car tail and side lights on.
- `.0.lighting.sky_rain()` and the `sky_rain_*` values → the sky's scrolling cloud layer.
- `.0.raining()` → always false. No rain rendering is needed for shipped content.

## 4. Open

- The loader's search order for route versus track `lighting.xml` and `sky.pssg` (F515 uses route first for both, docs/SKY.md §2).
- The vehicle-light branch order.
- Where the ambience `condition` / `timeOfDay` XML lives: packed, or unused.
