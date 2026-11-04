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
- Do not interpret saved sun height as a day/night classifier. In the recovered selected-path
  basis, `direction_to_light.y = sin(height)`: 90° points along +Y (zenith), not the horizon.
  The cached synthetic original matrix and `track_lighting` cardinal tests agree; see
  [source direction evidence](SOURCE_SKY_LIGHTING.md#source-sun-vector-traced--18-october-2026).
  Night-labelled presets have `skyscale` 0 or 0.03 and dark fog, but those data do not prove a
  horizon sun or the live runtime weather/light state. The previous horizon interpretation
  was incorrect (G330).
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

## 5. Headlight renderer boundary (G350, 2 November 2026)

The earlier per-slot Headlights state was hosted by LeMansEvent. Ordinary
ForcedNight routes do not install that event, so night sky/environment selection
alone cannot provide shared beam activation. A general lighting adapter must
preserve the existing Timeline/clock authority and per-slot delays, handle pause
and restart once, and distinguish vehicle_lights tail/side markers from beams.

Source/API review finds that stock SpotLight can illuminate existing forward
PBR road materials. This is not a tested light implementation. Curve strengths
are not automatically lumens, and source optical anchors, cone, range, cookie
projection and photometry remain unresolved. No new shadow/reflection or
performance policy follows from this feasibility finding.

Private metadata survey: six representative cars and eighteen source packages.
Five provide named front lamp joints; selected STK packages do not. Their joint
translations are almost zero while lamp geometry is near the nose; those
translations cannot serve as source projector positions. Mazda787 pod/glass
draws overlap and must not create duplicate emitters. The complete survey is
`local/handoff/drafts/night-lamp-inventory03/`, with activation plan in
`night-headlight-state02/`. These are bounded observations, not an all-car light
implementation or original optical-parity proof.

The local G350 change separates activation into shared vehicle_lights State,
with actual field slots and a single retained event-clock update. On 3 November
2026, the actual 642-file freeze passed combined-feature all-target Clippy
(`dlaa,paint-specular`, warnings denied) and both named ignored ECS tests:
2 passed, 0 failed, 0 ignored. These exercise ordinary modes and separate marker
state, mixed slots, pause and zero-tick segment restart, plus the saved Le Mans
Timeline off/on/off against event-only scheduling. Receipts are retained in
`local/graphics/night-activation01/check01/`; this was a filtered CPU check,
not a full-suite gate or playable rebuild. No application/GPU run, projector,
street-light renderer, rendered road contribution or optical/parity completion
is implied.

## Source headlight preparation diagnostic (G356)

The source front-point writer0x00904440 selects a complete pair of part codes:
108/114 (`light_glass_fl/fr`), then107/113 (`light_pod_fl/fr`). It does not mix
one glass and one pod. If neither pair is complete it retains original defaults
(+/-.75,.60000002384,2.20000004768,1). A selected point is the saved node bounds
midpoint plus its transform translation, with W=1; no extra bound rotation is
inserted by this writer. Named TRANSFORM/BOUNDINGBOX schema and the relevant
node inheritance were checked, rather than interpreting arbitrary draw bounds.

Description parsing0x009EFB30 zeroes its record. Only inside a present PARTS
branch it initializes z offset=.2 and y=0, then reads m_light_ray_z_offset and
m_light_ray_y_offset. An absent attribute preserves that branch default; absent
PARTS leaves zero-initialized fields. The per-car accessor0x00988F10 reads its
owner's description link; it is not a singleton manager getter.

The selected source projector preparation0x00736020 averages the cached pair,
adds those y/z offsets and transforms the point through its indexed component
matrix. It also prepares a normalized local direction (0,modeY,.99620002508,0)
and perpendicular direction; source modes supply modeY=-.45 or-.8. This pins a
source frame construction, not Bevy cone/range/intensity. The0xB0 packed record
per accepted car does not establish a physical emitter count. Other values60/110
must not be called meters or used as photometric calibration without their
actual shader contract.

### Consumed saved-data tool

`tools/diagnose_headlight_points.py` reuses existing binxml/PSSG readers and
reports canonical LOD0 saved nodes, selected pair/default provenance, PARTS
presence/attribute provenance, midpoint and raw-model point with offsets.
Inputs are an explicit absolute --game-root and repeatable --model stems;
there is no model policy list. Output is create-new JSON under ignored local/.
--self-test needs no game assets. Selected malformed/nonfinite/inverted data
fails instead of silently selecting another semantic fallback. This strict
finite domain is diagnostic admission, not proof of original CRT parsing.

The source runtime part lookup selects first non-null among eight slots;
canonical LOD0 selection in this tool does not execute or prove that registration
order. Current Car.scene/model_offset and presentation/replay association remain
unverified. Do not apply model_offset twice, use rawRuntime.pose as a universal
render root, or describe this report as a world-space lamp. No dead metadata
fields were added to Car and no beam renderer/default was changed.

Actual CPU run:14authored controls passed and six model reports completed with
12stable input-file identities. Lacetti/BMW/Saleen/R10/787 select canonical glass
pairs; STK uses the original default pair. These are representative fixtures,
not implementation exceptions or complete model coverage. Offset storage is
castf32, midpoint computation uses Pythonf64; no CRT/SIMD/native bit parity.
Duplicate consumed offset attributes are rejected using preserved binary XML
rows before ElementTree can collapse them. Unrelated metadata remains untouched.
Optimized Python is refused so reader/self-test assertions cannot disappear.
Private result local/graphics/headlight-data01/diagnostic02.json and source
proof local/handoff/drafts/headlight-optics58/ retain exact hashes/limitations.

Next compare this typed source data to the actual loader's part/joint placement,
then close presentation and projector shader inputs before world illumination.
The earlier provisional root/pose helpers remain unapplied. Reflections, solar
shadows, cockpit and render-performance policies are unchanged.


### G363: bounded authored illumination consumer and actual Fill audit

The default-off headlight trial consumes the shared vehicle-light State, model
anchor and presented vehicle transform. Origin uses P*M*C; beam direction/up use
the prepared source frame through P. Six actual ECS/data tests cover transformed
anchors, independent slots/fade, Replay suppression and six purchased model
descriptors. These fixtures do not prove the original eight-slot runtime loader
association, source projector optics, photometry or rendered beam quality.
The SpotLight cone/range/lumens are explicitly authored trial parameters.

One purchased Environment/Fill/Timeline Rust fixture reports the currently
consumed conversion, without deriving new units: Haruna day ambient1093.2759
and back1132.9448; ForcedNight ambient9993.379 and back7953.389. Baseline direct
sun remains white18000lux. The inverse raw-sun-peak conversion can therefore
amplify night Fill. Saved scales are not a license to multiply raw RGB again:
static packing and effective Timeline binding must be resolved separately.
This is CPU evidence of current behavior, not original night renderer parity.
Combined-feature Clippy, six consumer tests and this single fixture passed;
full package checks and production driving/render validation remain separate.
Private receipts: local/graphics/night-headlight-consumer01/cpu02.


### G364: effective static and timeline coefficients

The selected original binding keeps packed bytes and scale separately. Static
completion replaces the saved scale with max(1,R,G,B), packs normalized RGB
using255 and truncation. Timeline RGB registration instead uses256; its byte
writer adds.5, truncates and clamps0..255. Independent scale curves update the
separate scalar. Reconstruction uses byte*(f32 1/255)*current scale. Absent
timeline keys preserve initialized bytes/derived scales, including individual
missing channels. Raw XML remains immutable provenance. The selected source
edges are741520/9ACE90/730F90 and742B00; private binder95 pins actual constants.

A bounded default-off renderer trial consumes that resolution through the same
LeMansEvent timeline/time used by sky and source activation. It updates actual
sun/back colour and illuminance, ambient and existing sourcePhong colour under
one explicitly supplied authored coefficient-to-Bevy unit. There is no new
clock or route/model policy. It preserves transforms, shadows, source lobe
weights/direction and reflection/IBL behavior. Existing static directions and
baked IBL remain limitations for a complete night/time-of-day feature.

Static f64 arithmetic is not an x87-80bit oracle. Existing interpolation time
precision and extreme/subnormal values remain separate limits. Three-family
metadata admission passed for23 purchased lighting files, not all render paths.
Eleven source-resolution/consumer tests passed, followed by the required670-test
gate and five matched final night/day captures (G364/G365). The explicit-unit
night case reduces excessive scene brightness; the current physical spotlight
produces only a one-byte maximum PNG difference and does not validate a beam.


### Source headlight projector footprint and cookie (G366)

Original producer0x00736E60 uploads camera-space origin and right/up/forward
divided by radial cutoff R, plus dimensionless2.5*clamp(weight,0,1). Selected
full/quick fragments form q from the visible camera-space position and that
basis. The front gate is .5*abs(q.x)<q.z and .5*abs(q.y)<q.z; cookie UV is
(.125*q.x/q.z+.25, -.25*q.y/q.z+.5), sampling the left atlas half. Attenuation
is saturate(1-length(q)), not inverse square. The selected shaders saturate
the projector sum before multiplying global light colour. Only their first
two sets use the inspected focus-normal factor. Range110 follows a component
identity comparison; it is not established as unconditional physics slot0.

Resource handler0x00732970 tries headlight.dds then headlight.tga and stores
the result at NightLights+94;0x0072E170 binds that same headlightMap. Purchased
containers have a1024x512BC3 atlas. Left-half RGB agrees across both inspected
contexts and is spatially coloured; alpha/mip data differs. Native sampler
filter/address/sRGB semantics remain unresolved after loader state overrides.
Load through generic track resource context, never a hardcoded track list.

The current neutral-white PBR SpotLight trial remains an authored approximation:
its circular cone and inverse-square falloff do not reproduce this contract.
Bevy0.19.1 native light textures require pbr_light_textures plus hardware support
and sample only red. Public extraction/custom-tag and guarded ShaderAssets
override APIs offer a possible shared prototype, not verified implementation.
Cookie RGB, source group saturation and selected normal policy need explicit
handling; no physical-unit calibration or source parity follows from geometry.


Selected road/infield and metallic-paint consumers add maskGBA to their diffuse
sum before albedo/AO; independent solar Phong uses maskR. Paint reflection and
sun lobes remain separate. The inspected reflective-road fragment has no mask
binding, and chrome/glass arithmetic is unverified. A shared aggregate-RGB
diffuse adapter is therefore the next renderer experiment; applying it to all
PBR families is an authored adaptation, not recovered universal material parity.
Private source proof: local/handoff/drafts/night-mask-consumers102/.


### Bounded source-cookie renderer trial (G369)

The optional source-projector feature enables Bevy pbr_light_textures and a
shared, live-only diagnostic consumer. All actual field slots reuse headlight_frame
model descriptors, append anchors and P*M*C presentation; activation uses the
existing vehicle_lights/LeMans clock. Explicit U, all-slot range and the named
source-constructor RGB snapshot are required, with U matching the coherent
source-lighting trial. No route/model whitelist or normal default is added.

The guarded adapter retains both existing Bevy shader asset IDs/import metadata.
It samples the actual route cookie's lossless left-half RGB crop, forms separate
local direct/transmission sums, clamps each once, then applies global RGB/U and
an authored PBR diffuse/AO response. Texture sampler/colour space, focus identity,
first-two-light normal policy, chrome/glass and reflective-road exceptions remain
limits. Snapshot RGB is not a live original lightColour resolver.

Actual GPU cookie upload precedes activation. Missing shaders/imports use the
native cache's two retry states; processing/module errors remain fatal.
A bounded positive receipt joins changed import hashes, a queued-to-compiled
Surface pipeline, LIGHT_TEXTURES and current public source-decal insertion.
Multidraw evidence is road-family phase participation; it does not establish
exact entity-bin membership or typed GPU-buffer/draw readback.

CPU admission/asset/registration tests25 pass. A matched solo BMW/Haruna capture
shows source-cookie illumination on road and barriers; all66 logged control rows
match. The actual view is baseline MSAA4/HDRfalse, not native-DLAA validation.
No original optical parity, normal-night integration or all-model completion is
claimed. Source right-axis comparison still needs the original camera determinant;
no unsupported horizontal cookie flip is applied. Refer to G369 for artifact
hashes, failed startup guard correction and private matched comparison paths.


The second matched fixture uses the native sixteen-car/six-model LeMans field,
existing36-second clock and autopilot. Actual03:00/headlights16 logs and4636
identical logged control rows accompany visible road/car lighting. The capture
uses1475ticks after the actual five-second race start sequence; the earlier
1350tick assumption was rejected and retained. These are representative live
fixtures, not all-route/material, full-weight readback or normal-window parity.
