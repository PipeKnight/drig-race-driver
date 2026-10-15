# Ground effects: skid marks, wheel kick-up and ground cover

Recorded 15 October 2026 (F306). Static research of the original only: shipped data from the
purchased installation plus read-only Ghidra 12.1.4 decompilation of `grid-unpacked.exe`
(`-noanalysis -readOnly` on private copies of the phase3 project) and the hash-pinned analysis
image. No runtime capture was made. Private evidence lives in the main checkout under
`local/ground-effects/` (`skidmarks/`, `kickup/`, `grass/` with their scripts, decompiled
functions and dumps; `runs/` with validation logs and renders).

Labels: **[V]** verified in shipped data or disassembly; **[D]** read from decompiled code;
**[I]** inference. Port choices that were not recovered are marked **AUTHORED**.

Tyre smoke is a separate presentation (docs/TYRE_SMOKE_RESEARCH.md). Kick-up emitters that use one
of its `grid_tools::tyre_smoke::SMOKE_TEXTURES` (`SM33`, `SM33_dark`, `SM38`, `Wisps08_32`,
`Wheel_spin`) are drawn there and skipped here, so the two cover complementary emitters.

## 1. Skid marks

### Data

- [V] `skidmarks/{classic,drift,road,slick}_tm.xml` are byte-identical: root `offset="0.04"`
  and 35 `SURFACE` records of 24 attributes. Only the `.pssg` atlases differ. `test_tm.xml`
  differs (`offset="0.01"`, other fade values) and is unused.
- [V] The Lacetti row of `database.bin` has `vehicle_skid_type_id=4` = **road**, so it uses
  `road_tm`. Fleet totals: 32 road, 11 slick, 7 drift, 2 classic. [D] `6138D0` builds the
  name as type + `_tm`; the profile is global to the plugin.
- [V] Each record has three layers — `tread`, `skid`, `sideSkid`. Each layer has an atlas U
  range, NormalDepth and Fade lower/upper bounds and a width scale; there is also
  `wetLightingScale`.
  - Hard surfaces (CONC, STDR, STDM, STWT, DTDR, RTDR, RTDM, RTWT, RMBL, SHTR, SHRT, SHTC,
    MKNG, COBL): `skid=1`, `tread=0`, skid width scale 1.1, side-skid width scale 0.6 (0.9 on
    COBL).
  - Soft surfaces: sand, the gravels (LPDR, LGDR, HGDR, MGDR, MPDR, HPDR, DGDR), grass (GRAS,
    LEAF, SCRB, LSCR) and mud (LGDM, LGWT, MGDM, MGWT, `MUD `, DIRT, MDDM, MDWT). These have
    `tread=1` and `skid=1`. Width scales: tread 1.1, skid 2.0, side skid 2.25 (2.75 for mud).
  - DEFA marks nothing. `TARM` has no record.
- [D] Loader `740810`: 0x60-byte records keyed by surface name; `offset` goes to global
  `E4C68C` (default 0.01).
- [V] Atlas `diffuse.tga` and `normal.tga`: 1024×256 DXT5 with 10 mips, laid out in 64-px U
  bands. Bands 0–1 tarmac side-skid/skid, 1–2 cobble, then mud, gravel, sand and grass pairs.
  Bands 10–15 are empty. V runs along the mark.
- [V] Shader group `skidmarks`:
  - blend SRC_ALPHA / ONE_MINUS_SRC_ALPHA, depth test LEQUAL with no depth write;
  - pixel alpha = diffuse alpha × vertex COLOR.z;
  - the vertex normal is lerped toward the normal-mapped normal by COLOR.x;
  - two UV sets are crossfaded by COLOR.w.

### Emission (`73F860`, per car, AI included) [D]

- **Skidding test:** wheel `+AC` ÷ body mass `+15A0` > **2.0**.
  - `+AC` is a 10/s low-pass of `+A4` (`vehicle_propulsion`), the grip-overflow force
    (TYRE_SMOKE_RESEARCH.md §3.1).
  - The port's value reaches about 3.9 at launch wheelspin and about 5 in strong understeer,
    and stays near 0 when rolling.
- **Layer choice:**
  - A skidding wheel needs the surface's skid flag. It uses side-skid when 1 − |cos β| ≥ 0.5
    (β is the angle between the wheel heading and the contact velocity), otherwise skid.
  - A wheel that is not skidding needs the tread flag (soft surfaces only).
  - Otherwise the strip ends.
- **Surface key:** the wheel's raw material token (`+C8`).
- **Points:**
  - A point is laid when the wheel has moved more than 0.1 m.
  - The point is the wheel position lowered by the radius in the car frame, transformed to
    world, then raised by `offset` along the contact normal.
  - Left wheels mirror the U range.
- **A strip ends when:** the wheel loses contact; the surface is unmapped or not allowed; the LOD
  mode forbids marks (beyond LOD 4 none, LOD 2–4 rear wheels only); a reset happens; or the
  direction turns by more than 10° (`73F0A0`).
- **Vertices (`739570`):**
  - half width = 0.5 × tyre width (settings `+10A4`) × layer width scale;
  - V advances by ΔU × 4 × step / width, which keeps the texel aspect.
- **Colour bytes:**
  - byte0 = ramp × Fade (ramp 0, 0.5, 1 over the first quads);
  - byte1 = wetLightingScale;
  - byte2 = NormalDepth;
  - byte3 = 0 at the previous point and 255 at the current one.
  - The Fade lerp parameter is never written, so Fade = LowerBound in practice.
  - Ending a strip zeroes byte2 of the last point and halves the one before it.
- **Pool:** one global pool of 240 strips × 32 quads (20 cars × 4 wheels).
  - A full strip chains into a new one.
  - Allocation order: the free list, then retiring strips, then steal the active strip farthest
    from the camera.
  - There is no time limit. While free + retiring strips < 40, the farthest strip retires and
    fades by 25/255 per update, one quad at a time from its oldest end.

[I] The COLOR declaration type is `uchar4`, not the particle system's explicit
`uint_color_argb`, so the bytes most likely reach the shader unswizzled. On that reading:
- alpha = texture alpha × byte2 (NormalDepth 1.0, with the end and retirement fades);
- byte0 (Fade, 30/255–210/255) only sets the normal-map strength.

The other reading (D3DCOLOR) would scale alpha by 0.12 on tarmac. With a band-average texture
alpha of 0.4–0.5, that would make marks nearly invisible, so it is rejected.

## 2. Wheel kick-up (dirt, grass, gravel, sand)

### Selection [V data, D code]

Surface code → `PARTICLE` name (`surface_materials.xml`) → `particleMaterials.xml`
`<kickup><dry>`. The `<wet>` block is ignored by parser `5BBDC0`.

| Surfaces | Player | AI | Multipliers high/med/low/static | tyreslip |
|---|---|---|---|---|
| GRAS, LSCR, SCRB | K_Grass | K_AI_Grass | 0.5/1/0.5/0 | 1 (4 on LSCR/SCRB) |
| MGDR, HGDR, DGDR, GRAV, DIRT, (TRAP 2/1/1/0) | K_Gravel | K_AI_Gravel | 1/1/0.5/0 | 1 |
| LGDR | K_Gravel | K_AI_LightDry | 1/1/0.5/0 | 1 |
| LPDR, MPDR, HPDR | K_Pea_Gravel | K_AI_P_Gravel | 3/1/0.5/0 | 1 |
| SAND | K_Sand | none: K_Sand on all wheels | 1.5/1/0.5/0 | 1 |
| Tarmac family, RMBL | K_Tarmac* (smoke) | K_AI_Tarmac | mostly 0 | — |
| MUD, MDDM, MDWT, COMP, CATG, LEAF, HOLE | none | — | — | — |

Further rules [D]:
- AI front wheels never get a kick-up effect (`004B1F60`), except where no AI variant exists.
- Player front wheels use factor 1.0; AI front wheels use 0.75 (`tunePfx.xml`).
- `fadetime` is 0 everywhere.
- The AI variants `K_AI_Gravel`, `K_AI_LightDry` and `K_AI_P_Gravel` contain smoke only.

### Intensity (`6358B0`, bands `67C590`) [D]

- **B (speed band, from car speed):**
  - below 4 m/s: static·v/4;
  - 4–10 m/s: lerp static → low;
  - 10–15 m/s: lerp low → medium;
  - 15 m/s and above: **medium**. The high multiplier is never used, because the fraction is
    forced to 0 at `67C5FB`.
- **L (wheel load, N):** L/1000 below 1000; 1 up to 2500; 1 + (L − 2500)·0.002 above, capped at 2.
- **S (slip):** tyreslip · max(0, skid − lo), where skid is the same `+AC`/mass quantity.
- **Rate** = (B·L + S) · baseRate · rateMult · F · D, capped at 5 × baseRate.
- **Speed** = (B + S) · baseSpeed · speedMult, clamped to 0..2 × baseSpeed.
- **Distance:**
  - `tunePfx`: ignore distance 400 m, reduction distance 100 m;
  - AI distance factor D: 0.7 → 0.2 over the reduction distance, then 0.2;
  - player D ≈ 1.
- **Stop:** no emission when wheel `+C0` is 0.
- [V] Global limit 12,000 particles (`effects/config.xml`).
- The per-axle thresholds `lo`/`hi` (manager `+214..+220`) were not found in this research;
  the tyre-smoke research recovered them as rear 1.7/9.5 and front 3.5/9.0
  (docs/TYRE_SMOKE_RESEARCH.md §3.2), and the debris port uses those `lo` values.

### Debris emitters [V, decoded from the binary XML; parser `8A49A0`: divergence in degrees, force influence in %, frame `frequencies` normalised weights]

| System / emitter | Texture (frames) | Rate/s | Life s | Speed, cone, inherit | Gravity / drag % | Collision, bounce | Width | Alpha |
|---|---|---|---|---|---|---|---|---|
| K_Grass Dusty mud | mud_grass03 | 48 | 0.2 ±33 % | 2±1, 25.7°, 0.225 | 200/100 | yes, 0.127 | 0.25→0.05 linear | 0.70/0.60/0 |
| K_Grass Muddy_Bits | mud_grass_bits04 (2×2) | 20 | 4.0 ±12.5 % | 6±2, 16.4°, 0.364 | 200/100 | yes, 0 | 0.06 ±71 %, constant | 0/–/0 (in 0.2, out 0.4) |
| K_AI_Grass Grassy Clumps, Dusty mud | mud_grass03 (2×2) | 4, 6 | 0.2, 1.0 | 6±2 / 2±1 | 200/100, 80/100 | no / – | 0.25 ±44 % constant / 0.2→0.1 linear | – |
| K_Gravel Large_Stones | Stones_Single01 (3×3) | 12 | 0.8 ±40 % | 3±1.44, 25.7°, 0.636 | 200/100 | yes, 1.0 | 0.015 ±70 %, constant (finalWidth 4.0 unused) | 1/1/1 (in 0.1, out 0.6) |
| K_Pea_Gravel Large_Stones | Stones_Single01 (3×3) | 96 | 1.0 ±80 % | 5±3, 14.7°, 0.106 | 200/100 | yes, 0.623 | 0.02→0.02 linear | 0.75/0.54/0.24 |
| K_Sand Sand | Sand02 | 24 | 0.3 ±48 % | 3.5±0.19, 18.7°, 0.364 | 150/100 | no | 0.15 ±35 %, constant | end 0 |

Births: emitters sit 0.1–0.3 m above the contact, tilted −0.3° to −20° (`relOrientation` x).
Drag rates (0.003 × influence × rangeX × 0.01 per second, below): K_Grass Dusty mud 6/s,
Muddy_Bits 2.4/s; K_Gravel Large_Stones 0.6/s; K_Pea_Gravel stones 3/s; K_Sand Sand 0.9/s.
Alpha `inTime`/`outTime` are seconds (TYRE_SMOKE_RESEARCH.md §4.2).

### Particle size (recorded 15 October 2026, F316)

Static, from the hash-pinned `grid-unpacked.exe` (capstone disassembly and the existing Ghidra
output in the main checkout's `local/tyre-smoke/`).

- [V] Parser `8A49A0` (`<agescale>` branch at `0x8A5DDE`) stores, in the emitter:
  - `initialWidth` +0x1AC and `initialHeight` +0x1B0;
  - `finalWidth` +0x1B4 and `finalHeight` +0x1B8;
  - `scaleVariance` +0x1BC;
  - `scaleType` +0x1C4 as a code: linear 1, cubicAccelerate 2, cubicDecelerate 3,
    quadraticAccelerate 4, quadraticDecelerate 5. Absent or unknown leaves 0.
  - `<scale growthPercentage>` goes to +0x1C0.
  - The values are stored raw, with no unit conversion.
- [V] Emitter defaults `875620`: the five size floats and growth are 1.0; scaleType is 0.
- [V] Spawn `890A10` (`0x890CB9..0x890D32`):
  - the particle width/height (+0x20/+0x24) start at the initial values;
  - a whole percent v = trunc(U(−sv, sv)) is stored as a short at +0x80, where sv is
    scaleVariance.
- [V] Particle update `8A9CC0` (`0x8AB3ED..0x8AB527`):
  - **scaleType 0:** width = initialWidth × (1 + v·0.01) × growth^⌊25·age⌋. The integer power
    is `8B5320`; age is in seconds. `finalWidth`/`finalHeight` are **never read**.
  - **scaleType 1–5:** width = initial + (final − initial)·F(t), with t = clamp(age/life, 0, 1)
    and F = t, t², 1−(1−t)², t³, 1−(1−t)³. v and growth are not used.
- Consequences [V for the shipped data]:
  - Every debris emitter whose final size is −1 (Muddy_Bits, Grassy Clumps, Sand) or 4.0
    (K_Gravel Large_Stones) has no scaleType, so its final size is ignored. The **gravel stones
    stay 0.015 × (1 ± 0.70)**, about 0.5–2.6 cm, with growth 1.
  - The F313 port read 0.015→4.0 as a linear lerp in metres. Opaque stone sprites grew to
    about 4 m (up to 6.8 m with the variance) within 0.8 s. This is the "big flat pictures"
    on gravel and dirt that the owner reported.
  - `finalWidth −1` has no meaning in the original: it only appears on emitters that do not
    read it.
  - The tyre-smoke port (`grid_tools::tyre_smoke`, `sprites`) already implements this rule.
    Its only differences: it does not truncate v to a whole percent, and it defaults a
    missing scaleVariance to 0 instead of 1. No shipped emitter without a scaleType omits
    scaleVariance, so neither difference has an effect.
- [V] `frequencies` are per-frame weights. The parser normalises them to sum 1, then
  accumulates them, so the K_Gravel values (which only look cumulative) give frame 8 about
  80 %, frame 7 16 % and frame 6 3 %. `grid_formats::kickup` already normalises them the same
  way.
- [D, extent open] Renderer `886BD0`: the camera-facing corner code (`0x88B3DB..0x88B7EE`) puts
  the corners at (±width, ±height) in the particle basis. The basis comes from normalised
  rotation rows (`0x88AC29` normalise → `4695B0`). [I] If that basis is orthonormal, the
  drawn quad is 2 × size: a 3 cm stone, and twice the current smoke sizes. Both ports
  currently draw the size as the full quad edge. This is not adopted until it is confirmed.
- [D] The other appearance properties match the port's approach:
  - Large_Stones is `InverseView` (a camera-facing billboard in the original as well);
  - its particle type `lit` uses shader `pfx_lit` with the lit stream (normal + TEXCOORD1
    light sample) and no depth/soft variant (`litDepth` is the Pea_Gravel/Muddy_Bits type);
  - its alpha is opaque (absent start/middle/end = 1), shaped by the texture alpha.
  The original stones are flat camera-facing sprites too; what made them look like flat
  pictures in the port was their size.

## 3. Ground cover (grass)

- [V] `grass.grs` holds painted terrain triangles, not instance records. All 13 track files
  parse with this layout:
  - `u32 patchCount`;
  - per patch:
    - AABB `f32 min[3], max[3]`;
    - `u32 nverts, ntris`;
    - 12-byte vertices: u16 xyz quantised into the AABB, then 6 bytes = 12 density nibbles
      (type_01..type_12, /15);
    - u16 triangle indices;
    - padding to 4 bytes, which is omitted at the end of the file.
  - Loader `97EAB0`/`94FEA0`.
- [V] Donington: 1,054 patches, 30,468 vertices, 37,168 triangles.
  - 30,457 of the vertices lie exactly on `tracksplit` render triangles.
  - Painted area is 202,741 m², and type_01 has weight on 156,630 m².
- [D] Scatter `977500 → 96CFA0 → 95BC90 → 95AF90`:
  - **Grid:** one candidate per 0.5 m cell, jittered ±0.1 m, on the axis the triangle normal
    mostly faces (threshold 0.707).
  - **Type:** the per-triangle type weight is the mean of the corner nibbles. A type is picked
    by cumulative weight, then accepted with probability equal to its weight.
  - **Placement:** an inside-triangle test, with height taken from the triangle plane.
  - **Sprite:** picked by weight. Width = base × U(width range); height and sink = base ×
    U(height range).
  - **Mirror:** UVs are randomly mirrored left/right (`94FAF0`).
  - **Randomness:** LCG `x = x·0x41C64E6D + 0x40584C0F`, value `(x & 0xFFFF)/65536`.
- [V] LOD subsets: the `groundcover` option n=4 splits cells by the table at `D11698`, indexed
  `[(v & 3) + (u & 3)·4]` = `0 1 2 3 / 3 2 1 0 / 1 0 3 2 / 2 3 0 1`.
- [V] `ground_cover.xml` Donington draw distances: `lod0..3 = 145/30/15/5` m, `fadestart 0.7`.
  Code defaults are 150/40/20/10 (`733DE0`). [I] Subset k uses lod_k.
- [V] `ground_cover.pssg` (Donington):
  - one type, `type_01` with userdata `16.0, 0` (mode 0 = billboard, 1 = fixed mesh);
  - four sprites `green_grass_01..04` (0.39–1.14 m wide, 0.19–0.26 m tall). Their userdata
    is weight, (height range), (width range), followed by 90.0, 0.8, 1.0, which are unknown;
  - texture `gc_billboard.tga`, 512² DXT5.
- [D] Rendering:
  - camera-facing billboards (BillboardMatrix rows), sheared by the terrain slope;
  - alpha = 1 up to fadestart·D, then linear to 0 at D;
  - no blending, alpha-to-coverage, depth writes;
  - lit by N·L with the sun, plus the screen-space shadow mask;
  - no wind input [V].
  - Instances regenerate when the camera moves more than 10 m (`97EDE0`).
- [V] Terrain detail:
  - The Donington grass terrain (`track_surface_simple.fx`, `land.pssg` `track_infield.fx`)
    lerps two 512² diffuse maps on vertex colour and tiles UV0 every ~4 m.
  - A macro map on UV1 carries ambient occlusion in its alpha.
  - `TSkidsMap` is a static painted-skid layer weighted by vertex alpha. It is separate from
    dynamic marks.
  - None of this is a separate detail-texture pass. The port's terrain shading is unchanged.

## 4. Port (crates/grid-app)

Shared modules:
- `wheel_ground.rs` samples every car once per successful tick. Per wheel it records: contact
  token and surface code (`Surfaces::code`), contact point (wheel centre lowered by the radius
  in the car frame), car-up normal, steered heading, contact velocity, excess = `+AC`/mass,
  1 − |cos β|, width `+F8`, load `+138`, car speed.
- `particles.rs` is a CPU billboard store.
- `ground_effect_assets.rs` reads PSSG textures at runtime via
  `grid_formats::texture::read_selected`, which skips texture formats it does not need.

Format readers (with unit tests), all reading the install at runtime:
- `grid_formats::skidmarks`;
- `grid_formats::kickup` (particle materials, binary particle systems, surface → PARTICLE);
- `grid_formats::ground_cover` (`.grs`, distances, sprites).

**Skid marks (`skid_marks.rs`)** — `road_tm.xml` and `road_tm.pssg` `diffuse.tga`, for all cars:

| Area | Behaviour | Source |
|---|---|---|
| Layer choice | Original rule, as in section 1 | [D] |
| Pool and points | 240×32 pool, 0.1 m points, 10° break, chained strips, far-strip stealing and retirement fade, end taper, mirrored U on left wheels, V advance | [D] |
| Alpha | Texture alpha × end/retire fade | [I] |
| Normal map | Not reproduced | AUTHORED |
| Two-UV crossfade on layer changes | Replaced by a new strip | AUTHORED |
| Per-car LOD restriction | Not reproduced (all wheels of all cars) | AUTHORED |
| Tyre width | Wheel `+F8` (the `.ned` width, 0.27 m front for the Lacetti); the original reads settings `+10A4`, which the port does not fill | AUTHORED mapping |

**Wheel debris (`wheel_debris.rs`)** — the systems for every surface come from the shipped XML,
and the textures from `pfx_textures.pssg`. 11 systems; 5 texture/lighting slots.

| Area | Behaviour | Source |
|---|---|---|
| Player/AI variant, AI front-wheel rule, band B, load L, slip S, rate/speed caps, F, D, ignore distance, 12,000 limit | Original rules | [D] |
| Slip threshold `lo` | Rear 1.7, front 3.5 (TYRE_SMOKE_RESEARCH.md §3.2) | [V] |
| Drag | v −= v·dt·0.003·influence·rangeX·0.01 (F316; was an AUTHORED 1/s) | [V] |
| Gravity | 9.81 × influence % | [V] |
| Size | Original rule above: scaleType 0 = initial × (1 + v/100) × growth^⌊25·age⌋, else lerp by F(age/life); quad edge = size (extent open) | [V] (F316) |
| Alpha | inTime/outTime in seconds of the varied life (F316; were life fractions) | [V] |
| Emitter axis | Up, tilted by the birth pitch, with a random cone of the divergence angle | [I] |
| Collision | Bounce on the spawn wheel's ground height | AUTHORED |
| Lighting | Lit/unlit by particle type, alpha blended | AUTHORED |

**Ground cover (`ground_detail.rs`)** — for any route that ships the three files (Donington
531,621 instances; Nürburgring 539,669):

| Area | Behaviour | Source |
|---|---|---|
| Scatter | Original rules: cell grid, jitter, dominant-axis rasterisation, type pick and accept, sprite weights, scale ranges, UV mirroring, subset table | [D] |
| Draw distances and fade | lod0..3 and fadestart per subset | [D]/[I] |
| Per-cell RNG seed | Original seed sequence depends on regeneration order | AUTHORED |
| Billboard mesh | One camera-facing mesh, rebuilt when the camera moves or turns (the original orients billboards in the vertex shader every frame) | AUTHORED |
| Terrain-slope shear | Not reproduced | AUTHORED |
| Fixed-mesh types (mode 1) | Not rendered | AUTHORED |
| Fog | Not reproduced | AUTHORED |
| Blending | `AlphaToCoverage` with the terrain normal for lighting | as original |

**Switches:**
- `GRID_GROUND_EFFECTS=0` disables all three effects (A/B checks).
- `GRID_GROUND_TRACE=1` prints per-wheel samples, debris counts and the mean per-frame CPU
  cost.
- `GRID_EFFECTS_CAMERA=x,y,z,lx,ly,lz` fixes the camera for scripted renders.

## 5. Validation (renders under `local/ground-effects/runs/`)

- **Launch** (`launch-v3.png`): front-wheel spin marks at the grid. Excess force/mass is about
  3.9 at launch; the strips end when grip returns.
- **Hard turn** (`turn-final.png`, throttle 1, steering 0.6): curved understeer arcs.
- **12-car autopilot start** (`race-start-12.png`): launch marks from every AI car.
- **Off-track on grass** (`grass-offtrack-final.png`):
  - mud clumps and dust from K_Grass;
  - grass tread marks (tread layer: grass marks while rolling);
  - about 170 live particles, 406 emitted in 64 ticks.
- **Static grass** (`grass-static-a2.png`, `grass-static-start-v1.png`,
  `grass-nurburgring-start.png`):
  - green tufts on the Donington verges;
  - the first render showed black back-lit tufts from mirrored geometry, fixed by mirroring UVs
    as the original does.
- **Windowed 12-car race** (11 AI, damage, autopilot, 3,600 ticks = 57.6 s simulated; rebased
  on F310, so tyre smoke, damage parts and assists are included; build `CDDA6A28961B43FE…`):
  - effects on: 57.7 s wall clock after a 4.5 s startup;
  - effects off (`GRID_GROUND_EFFECTS=0`): 57.6 s;
  - no stderr errors, about 52 frames/s;
  - per-frame mean cost: grass 0.6–1.25 ms, skid mesh 0.01–0.2 ms, debris 0.01 ms,
    per-tick sampling/emission 0.03–0.07 ms.
- **Failures fixed on the way:**
  - The first build ray-probed the track for every wheel, costing 0.7–2.8 ms per frame. It now
    uses the original's wheel-derived point.
  - Empty dynamic meshes made Bevy's mesh allocator log a slab use-after-free thousands of
    times per run, which slowed frames. Every effect mesh now keeps one invisible degenerate
    triangle (`effect_mesh`).
  - Earlier A/B runs under 100 % CPU from parallel builds varied from 58 to 74 s.
- Grass load takes about 0.1 s (`.grs` read 9 ms).
- **Gravel size fix (F316)**, renders in the main checkout's `local/ground-effects/gravel-size/`.
  Before = the F315 playable `7f22c32` (sha256 `c34ffe711fc4…`); after = this change.
  - **Where the gravel is:** Donington `route_0/track.jpk` has K_Gravel surface LGDR
    (7,078 triangles) and pea-gravel MPDR (6,217). Strips of both lie beyond the first
    straight, at about (−490…−600, −15…−75).
  - **Run:** `--drive --drive-ticks 652/660 --throttle 1 --steering -0.02` crosses LGDR at
    ticks 636–660, at about 50 m/s.
  - **Results:**
    - `lgdr-before-7f22c32.png` (chase camera): brown stone sprites several metres across
      behind the car.
    - `lgdr-after.png`: only specks at the wheel.
    - `lgdr-side-*.png` (`GRID_EFFECTS_CAMERA`): half-metre stone quads at the wheels before; after,
      only specks (stones of about 0.5–2.6 cm) at this 8 m distance.
    - Grass (`grass-side-*.png`, throttle 0.5, steering −0.3, tick 272; 400 emitted /
      169 live in both runs): small mud bits both before and after. They stay visible
      longer after the change, because the alpha fades are now seconds rather than life
      fractions.
  - No runtime capture of the original exists; the claim is "matches the recovered rule".
    The size convention (half or full extent) remains open.

## 6. Open items

- Wheel `+AC` is the 0.1 s low-pass of the grip-overflow force (TYRE_SMOKE_RESEARCH.md §3.1).
  The kick-up handbrake ×2 and the rear drive-slip term from that section are not in the debris
  port.
- The tyre-width settings field `+10A4`.
- Skid COLOR channel mapping (runtime capture would settle it); skid normal map and two-UV
  crossfade; per-car mark LOD.
- Emitter direction convention: the tyre-smoke research has emitters along local +Z of a
  rearward wheel frame, while the debris port emits along the tilted up axis. Also open: birth
  timelines (`start`/`end`), spin (`initialRotationVariance` is not parsed).
- Quad extent: is the stored size the half or the full edge (renderer corners at ±size,
  section 2)? This affects tyre smoke and debris alike.
- Ground-cover fixed meshes (mode 1, other tracks); the sprite fields 90/0.8/1.0 and the type
  float 16.0; GPU billboarding to avoid CPU mesh rebuilds; terrain-slope shear and fog.
- Unify `grid_formats::kickup` with the overlapping `grid_tools::tyre_smoke` parsers (both on
  main now; the debris module already shares its `SMOKE_TEXTURES`).
