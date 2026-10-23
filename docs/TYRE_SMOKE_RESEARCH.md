# Tyre smoke — research (F-SMOKE-01)

15 October 2026, Claude Code. Owner task: research and reproduce the original tyre
smoke. Sources: shipped data of the purchased installation (read only) and static
analysis of the hash-identified private analysis PE `local/compatibility/grid-unpacked.exe`
(SHA-256 `aac46a64…4f34514`, base 0x400000) with Ghidra 12.1.4 headless `-readOnly`.
No game file, Steam install or Ghidra project was modified and no running game was
touched. Decompiler output, readable copies of the binary-XML particle systems, texture
previews and helper scripts are private under ignored `local/tyre-smoke/`. This document
contains only addresses, offsets, constants and structural descriptions.

Labels: **[V]** verified in disassembly/decompilation; **[D]** shipped data;
**[I]** inference, not verified. **AUTHORED** marks port choices that were not recovered. The port is described in
[TYRE_SMOKE.md](TYRE_SMOKE.md).

## 0. Summary

- Tyre smoke is the wheel **kickup** effect. The surface under each wheel selects a
  particle material (`surface_materials.xml` → `particleMaterials.xml`) whose `<kickup><dry>`
  names a particle system; tarmac uses `K_Tarmac` (player rear), `K_Tarmac_Front`
  (player front), `K_Tarmac_open` (open-wheel cars) and `K_AI_Tarmac` (AI rear) [D+V].
- One **held** system per wheel (two slots: forward and reverse spin) stays alive while the
  surface material is unchanged; every frame the driver rescales every emitter's live
  spawn rate and speed from a slip/speed/load intensity. Nothing is started or stopped per
  frame [V].
- On tarmac all speed multipliers are 0, so smoke comes **only from slip**: the smoothed
  grip-overflow force divided by car mass, above 1.7 m/s² (rear) / 3.5 m/s² (front);
  rate grows linearly and is capped at 5× the emitter's base rate [V].
- AI cars get **no front-wheel smoke** on tarmac, and their rear system is the cheaper
  `K_AI_Tarmac` with distance level of detail [V].
- `<birth start end>` are **seconds of system age**; the driver restarts the age while the
  wheel does not smoke, so each emitter appears after that many seconds of continuous
  smoking (Lower_Burnout 0.1 s … Smoke 1 s, Wisps 1 s) and then runs until the system is
  stopped [V; intent I].
- Particles are integrated in world space (except emitter-relative Burnout_circles) with
  gravity `0.01·influence·9.81` (positive down), linear drag `0.003·influence·rangeX/100`
  per second, age-curve size/alpha and linear colour [V]. Smoke **does not inherit** the car
  velocity except the Initial_cover / Lower_Burnout / AI emitters that declare
  `inheritemitter` [V].
- No wind or turbulence acts on tarmac smoke; only K_Gravel declares a wind force [D+V].
- Limits: 80 system slots, 10 births per system; config.xml `<limits>` names do not occur in
  the executable [V].

## 1. Shipped data

### 1.1 Files

| File | Format | Role |
|---|---|---|
| `effects/particleMaterials.xml` | plain XML | per particle material: collision effects and `<kickup><dry>/<wet>` wheel effect [D] |
| `surface_materials.xml` | binary XML | `MATERIAL` → `<PARTICLE name>` link (e.g. STDR → SMOOTHDRYTAR) [D] |
| `effects/vehiclepfxconfig.xml` | plain XML | kickup speed ranges, downforce / sliding_resistance effect scales [D] |
| `effects/tunePfx.xml` | plain XML | distance LOD and front-wheel factors (commented in the file) [D] |
| `effects/config.xml` | plain XML | limits 12000 particles / 60 emitters / 40 systems; particle types → shaders [D] |
| `effects/pfx_kickup_data_set.xml` | plain XML | loads the K_* systems through `ParticleManagerPlugin` [D] |
| `effects/particle_systems/K_*.xml` | binary XML | emitter library + one system with `<birth>` entries [D] |
| `effects/textures/pfx_textures.pssg` | PSSG | 28 particle textures (dxt1/dxt5/ui8x4) [D] |

`tools/dump_particle_systems.py` (independent) writes readable private copies of the
binary-XML systems (`local/tyre-smoke/particle_systems/`).

### 1.2 Kickup materials [D]

Each `<kickup><dry>` names four systems: `effect name` (player, rear / default),
`frontWheelName`, `openWheelName` and `AIName`, plus `multipliers high medium low
static rate speed` and `special tyreslip downforce`. Only `dry` blocks carry kickup data
for the tarmac family (damp/wet tarmac particle materials have none).

| Particle material (surface codes) | systems | high/med/low/static | rate, speed | tyreslip, downforce |
|---|---|---|---|---|
| SMOOTHDRYTAR (STDR, SHTR), ROUGHDRYTAR (SHRT, RTDR), TARMAC, TARM_GEN, CONCRETE, ROADMARK | K_Tarmac / _Front / _open / K_AI_Tarmac | 0 / 0 / 0 / 0 | 1, 1 | 1.0, 2.0 |
| DUSTDRYTAR (DTDR) | same | 1 / 1 / 0 / 0 | 1, 1 | 1.0, 2.0 |
| RUMBLE | same | 1 / 0.5 / 0 / 0 | 1, 1 | 1.0, 2.0 |
| DIRT, gravel family | K_Gravel / K_AI_Gravel (LIGHTDRYGRAV: K_AI_LightDry) | 1 / 1 / 0.5 / 0 | 1, 1 | 1.0, 1.0 |
| grass family | K_Grass / K_AI_Grass | 0.5 / 1 / 0.5 / 0 | 1, 1 | 4.0, 1.0 |
| SPRK (rim on tarmac) | Wheel_Rim | 2 / 1 / 1 / 0 | 1, 0 | 0.0, 1.0 |

So on tarmac the speed multipliers are zero and smoke comes only from the tyre-slip
term; dust tarmac and rumble strips add a speed term. Grass and gravel select dirt
systems (out of scope here: dirt kick-up belongs to the ground-effects work).

`vehiclepfxconfig.xml` kickup speed ranges [D]: high ≥ 15, medium 10–15, low 4–10,
static < 4 (m/s [I]); `downforce effect 1.0`, `sliding_resistance effect 1.0`.
`tunePfx.xml` [D]: ignoreDist 400, max 0.7, min 0.2, reductionDist 100, frontWheel 1.0,
frontWheelAi 0.75, maxParticlePeak 100 (meanings as documented in the file's comments).

### 1.3 Tarmac smoke systems [D]

`K_Tarmac` has eight emitters (all `sprite`, `InverseView` billboards, particle type
`smokeHalf` → shader `pfx_half_smoke`, lit stream, except Burnout_circles):

| # | name | texture | rate /s | life s (var %) | speed (var), divergence ° | size start→end, type | alpha start/mid/end, in/out | colour start→end | forces | birth start–end | relPosition |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | Smoke | SM33 | 12 | 3 (33) | 1 (0.5), 108 | 0.4→2.0 quadDecel, var 75 | .38/.79/0, .2/.5 | .50 .52 .54 → .47 .49 .50 | grav −1, drag 100/400 | 1–3 | −.35 .2 −.4 |
| 2 | Initial_cover | SM38 | 26 | 1 (40) | 1 (0.5), 40, inherit .34 | 0.2→0.4 cubicDecel | 0/.31/0, .01/.1 | .38 .40 .41 | grav 20, drag 100/2000 | 0.5–1 | −.1 .1 −.2 |
| 3 | Wisps | Wisps08_32 (2×2 atlas, frequency) | 3 | 6 (33) | 1 (0.5), 26 | 0.5→5 linear | 0/–/0, .5/.25 | .44 .46 .47 | drag 100/500 | 1–4 | 0 .2 .1 |
| 4 | Smoke_effect | SM33_dark | 24 | 2 (50) | 2 (1.5), 154 | 0.22→2 cubicDecel | 0/–/0, .1/.5 | .46 .48 .50 → .38 .40 .41 | drag 100/1000 | 0.5–6 | −.7 .7 .5 |
| 5 | Drift_effect | SM38 | 28 ×2 | 0.2 (50) | 2 (1), 15, inherit .77 | 0.22→1.2 cubicDecel | .39/.84/0, .05/.1 | .49 .50 .53 → .44 | drag 100/5000 | 0.75–3 | −.4 .7 .8 |
| 6 | Burnout_circles | Wheel_spin | 20 | 0.2 (50) | 0 | 0.4→0.3 cubicDecel | default/–/0, .1/.1 | .77 → .66 | angular drag; spin −1080°/s; parentRelative | 0.2–3 | .05 .3 0 |
| 7 | Lower_Burnout | SM38 | 36 | 0.1 (40) | 0, 40, inherit .65 | 0.1→0.3 cubicDecel | .13/.46/0, –/.04 | .45 .49 .51 | grav −20, drag 100/1000 | 0.1–2 | 0 .1 −.2 |
| 8 | Initial_cover | SM38 | 28 | 1 (20) | 2 (1), 79, inherit .34 | 0.3→0.6 cubicDecel | 0/.69/0, .1/.8 | .38 .40 .41 | grav 30, drag 100/2000 | 0.2–0.5 | −.2 .1 −.2 |

Burnout_circles is `basicDepthAlphaBlendHalf`, `HorizonAligned` (flat on the ground).
`K_Tarmac_open` (open-wheel cars) is the same layout with lower alpha and rates and
earlier birth starts; `K_Tarmac_Front` has only the two Initial_cover emitters (0.5–1
and 0.2–0.5); `K_AI_Tarmac` has three cheaper emitters (Smoke 0.1–0.5 with inherit 0.78,
Wisps 0.2–4, Initial_cover 0–0.5).

Textures [D]: SM33, SM33_dark and Wisps08_32 are `ui8x4` 256², SM38 and Wheel_spin are
`dxt5` 256², all with full mip chains. Colour channels are near white; the smoke shape
is in alpha. Wisps08_32 is a 2×2 atlas of four wisp shapes.

## 2. Loaders [V]

- `0x5BBDC0` parses one `PARTICLE`; its kickup loop reads only the `<dry>` child (wet is
  never parsed). Record (0xF4 bytes, copied by `0x5E20C0` into material+0x130): names with
  presence bytes (front +0xB4, open +0xB5, AI +0xB6), fadetime, tint, multipliers
  high/medium/low/static/rate/speed, tyreslip, downforce.
- `0x66A020` parses `vehiclepfxconfig.xml`; the kickup speed-range **max** attributes go to
  +0xC6C (static 4), +0xC68 (low 10), +0xC64 (medium 15). `downforce effect` (+0xC70) and
  `sliding_resistance effect` (+0xC74) have no readers besides the parser.
- `0x66FA80` (secondary base mgr+4 of the vehicle PFX manager, constructor `0x66F940`,
  vtable 0xCEA358) parses `tunePfx.xml` into mgr+0x234..+0x24C; constructor defaults
  (`0x67C9B0`) 200/0.5/0.1/40/0.75/0.5/200 are overridden by the file.
- Particle library: loader `0x8ABC20`, emitter defaults `0x875620`, `<emitter>` parser
  `0x8A49A0`, `<System>` parser `0x898EB0`. All angles are degrees. The strings
  `totalParticles`/`totalEmitters`/`totalSystems` do not occur in the executable.

## 3. Kickup driver

Call chain [V]: VehiclePFXManager update `0x6758B0` → per car `0x6587A0(body, car, wheel
states, …, dt, distance)` → per wheel inputs `0x6574F0` (wheel = body+0x1990+i·0x150) →
per emitter intensity `0x6358B0`.

### 3.1 Inputs per wheel (0x6574F0) [V]

- **slip** = wheel+0xAC / mass (body+0x15A0; 1/1700 if mass ≤ 0). wheel+0xAC is the 0.1 s
  low-pass of wheel+0xA4, the grip-overflow force `clamp(|F|/grip − 1, 0, 1)·grip` [V; the
  port computes both in `vehicle_force.rs` / `vehicle_propulsion.rs`].
  - ×2 when the handbrake byte body+0x1640 and the wheel's handbrake byte wheel+0xFC are set.
  - Rear wheels (anchor z ≤ 0) add `|wheel+0x148|·0.2·2.2·max(0, (body+0x1008 − 0.12)·2.0833
    − 0.17)` (2.2 = mgr+0x224, 0.17 = mgr+0x228). wheel+0x148 is not identified [open].
  - `0x6587A0` then lifts every rear wheel below `max − half` by `half = (max − min)/2` of
    the rear slips.
- **load** = wheel+0x138; **speed** = body+0x1648 (same for all wheels); **velocity** = body
  linear velocity (emitter inherit velocity).
- **material** (state+0x174) is updated only while the wheel has contact (wheel+0xC0 and
  +0xC8 non-zero); a blown tyre with sparks hardness > 0 switches to another lookup [V;
  rim sparks I].

### 3.2 Intensity (0x6358B0) [V]

Thresholds are manager constants without XML source: rear lo 1.7 / hi 9.5, front lo 3.5 /
hi 9.0 m/s².

```
loadF = L < 1000 ? L·0.001 : (L > 2500 ? 1 + clamp((L − 2500)·0.002, 0, 1) : 1)
slipT = tyreslip · max(0, s − lo)
frac  = clamp((s − lo)/(hi − lo), 0, 1)
(band, t) = 0x67C590(speed): <4 → (0, v/4); <10 → (1, (v−4)/6); <15 → (2, (v−10)/5); else (3, 0)
spdT  = band 0: static·t | 1: lerp(static, low, t) | 2: lerp(low, medium, t) | 3: medium
        (the `high` multiplier is never used)
lodF  = player: (budget + 1)/2;  AI: d ≤ red ? max − (max − min)·d/red : min  (×3 if car+0x64)
frontF= rear 1; front: player frontWheel, AI frontWheelAi
rate  = min(baseRate·(spdT·loadF + slipT)·k.rate·frontF·lodF, 5·baseRate)
speed = clamp(baseSpeed·(spdT + slipT)·k.speed, 0, 2·baseSpeed)
emitter frac (+0x224) = clamp(frac + spdT, 0, 1);  emitter inherit velocity = body velocity
```

- Without contact (wheel+0xC0 = 0) rate and speed are 0. Emitters with base rate ≤ 0 are
  skipped. The emitter named `Drift_effect` runs only on rear wheels while the drift object
  at mgr+0x90 has an entry for the car (drift events) [V; drift mode I].
- With mgr+0x90 active, AI cars with car+0x65 get rate ×0.15 and frac ×0.6 [V].
- The per-particle copy of frac (particle +0x6C) multiplies the particle alpha [I: the
  renderer's alpha lane reads emitter/particle +0x224-derived scale].
- System time is reset to 0 whenever a non-drift emitter's live rate is below 0.001
  (`0x65A3DA`).
- **downforce** (`special downforce`, material+0x220) is never read in this chain; fadetime
  is reachable only on a path whose guard fields are always 0 [V].

### 3.3 Budget and distance [V]

- `budget = 1 − min(|generated|/maxParticlePeak, 1)` from the particle manager's counters
  (+0x398/+0x39C); a rising budget moves half-way per frame, a falling one applies at once.
- Effective ignore distance = ignoreDist·budget (cars farther from the camera have their
  effects stopped, `0x6298D0`); effective reduction distance = reductionDist·budget.

### 3.4 System choice (0x658F45..0x659001) [V]

| Wheel | Car | System |
|---|---|---|
| front, material has frontWheelName | player | frontWheelName |
| front, material has frontWheelName | AI | none |
| front, material has AIName but no frontWheelName | AI | none |
| rear, material has AIName | AI | openWheelName if open-wheel and present, else AIName |
| any other | any | openWheelName if open-wheel and present, else effect name |

Open-wheel = body+0xEDA8 ≠ 0 [I]. Two slots per wheel: reverse when wheel+0x104 ≤ −0.02.
Systems are created held (`0x8A99C0`), released with `0x886840(fx, 1)`, and respawned only
when the material changes.

### 3.5 Placement [V; axis meaning partly I]

- Contact point in the car frame: wheel anchor (wheel+0) with the suspension travel
  (wheel+0x40), the deformation offset body+0x1650+16·i, y − radius (wheel+0xE0) and
  y + hit-material+0xA4.
- Rotation: yaw by −atan(wheel+0x2C / wheel+0x28) (steering direction [I]); Z mirrored
  (diag 1, 1, −1) unless the wheel spins backwards, so effect +Z points rearward when
  driving forward; X mirrored on wheels with local x < 0.
- The frame is car-local and combined with the effect's parent node (the car).

## 4. Particle runtime

Functions [V]: system update `0x898DC0`, births `0x898AD0`, create emitter `0x8981B0` /
constructor `0x892480`, emitter update `0x89F1B0`, spawn `0x890A10` (position `0x872DF0`,
velocity `0x873880`), particle update `0x8A9CC0`, render `0x886BD0`; create one-shot /
held `0x8A98B0` / `0x8A99C0` (80 slots), re-request `0x8A9AF0`, stop `0x882EC0`, release
`0x886840`. Random: LCG `x = x·0x41C64E6D + 0x40584C0F`, `u = (x & 0xFFFF)/65536`
(`0x869360`).

### 4.1 Births and emitters [V]

- Up to 10 births (stride 0x60). Each birth spawns its emitter once, when system age
  (sys+0x478) ≥ start; emitter duration = end − start, or unlimited if end < 0 or the
  system is held (sys+0x4D0). A per-birth flag prevents a second spawn.
- Emitter frame: rotation = birth rows (relOrientation, `Rx·Ry·Rz`, row-vector) × system
  rotation; translation = relPosition + system translation (relPosition is **not** rotated
  by the system orientation, 0x898B7D–0x898C9A). Emitters emit along local +Z.
- Stop: hold cleared, every emitter's duration set to its current age; existing particles
  live out their life.
- Spawn count per update = floor(rate·dt + carry); `number` is a one-time burst. Particles
  of one update are spread from the previous to the current emitter position
  (frac i/(n−1), a single particle 1.0).
- Shapes (sizes are full extents): `pivot` → origin; `rectangle` → uniform point, length
  along local Y, width along X; `box` → uniform point in the box (length Y, width X,
  height Z), same code for `surface` and `volume`.
- Speed = live speed + variation·U(−1, 1); `direction="orientation"`: local +Z rotated by
  Rx(a)·Ry(b), a, b ~ U(−div, div) degrees (a square cone). `inheritemitter`: v +=
  inheritamount·emitter velocity.

### 4.2 Particle update (0x8A9CC0) [V]

1. age += dt; dies at age ≥ life (life = life·(1 + U(−var, var)/100)).
2. gravity a = 0.01·influence·(0, −9.81, 0) (absent influence = 100).
3. wind only with a wind field and a `wind` force (not in tarmac systems).
4. v += a·dt; drag if |v| > 0.01: v −= v·|dt|·0.003·influence·rangeX·0.01 (K_Tarmac Smoke
   1.2/s, Initial_cover 6/s, Wisps 1.5/s, Smoke_effect 3/s, Drift_effect 15/s).
5. position += v·dt.
6. angular drag likewise, clamped to [0, 1] per step; angle += (1 − 0.25·age/life)·ω·dt,
   ω = rate + variance·U(−1, 1) deg/s; initial angle ± initialRotationVariance.
7. Size: f = age/life; w = lerp(initial, final, F(f)) with F linear t, `cubicAccelerate` t²,
   `cubicDecelerate` 1 − (1−t)², `quadraticAccelerate` t³, `quadraticDecelerate` 1 − (1−t)³
   (names swapped against exponents). scaleVariance and growthPercentage apply only
   without scaleType. Absent sizes default to 1.
   Detail recorded on 15 October 2026 (F316, docs/GROUND_EFFECTS_RESEARCH.md §2 "Particle
   size"). Without a scaleType (code 0):
   - w = initial × (1 + v/100) × growth^⌊25·age⌋;
   - v is a whole percent drawn in ±scaleVariance at spawn;
   - the defaults are growth 1 and scaleVariance 1;
   - finalWidth/finalHeight are never read, so `-1` or K_Gravel's 4.0 have no effect.
   This port already follows the rule. The stored w/h is the **half** extent: the drawn
   quad is 2w × 2h (§4.3, F319).
8. Alpha (seconds against the varied life L): lerp(start, middle) over inTime, hold middle,
   lerp(middle, end) over the last outTime; absent values are 1 / 0 s. Colour: linear from
   start to end over life.
9. Texture frame: `frequency` atlases choose a frame once at spawn from the cumulative
   frequencies (Wisps08_32: ~97 % frame 3); UV column = frame % widthInBlocks.
10. Rendering: InverseView billboards from the camera basis, HorizonAligned flat on the
    ground; corners at ±w, ±h, so the drawn edge is 2 × size (§4.3) [V]. Near-camera fade
    between two unresolved distances; alpha < 0.001 dropped.

### 4.3 Quad extent (renderer 0x886BD0) [V]

Recorded 15 October 2026 (F319). Static only (capstone on the hash-pinned PE, private dumps
in `local/particle-extent/disasm/`). The question was whether the stored particle size is the
full quad edge or the half extent. **It is the half extent: the drawn quad is 2 × width by
2 × height.**

- **Same struct.** Update `8A9CC0` writes width/height to particle +0x20/+0x24
  (`0x8AB458`, `0x8AB477`: initial × (1 + v·0.01) × growth^⌊25·age⌋, no factor ½) and the
  angle to +0x28 (`0x8AB2DF`). The renderer reads the same particle (`[esp+0xA4]`, from the
  list at `0x8872C3`): angle +0x28 (`0x8875CB`), width/height +0x20/+0x24 at the corners.
- **Corners.** `0x88B3DB..0x88B7EE` builds each vertex from (±w, ±h, 0, 0) (the sign from
  −0.0 at `0xCAFE50` minus the size), multiplied by the rows at `esp+0x180/0x190/0x1A0`,
  plus row `esp+0x1B0`, plus the particle offset. Only rows 0 and 1 matter (z = 0). The
  result then goes through the inverse of the node transform (3×3 cofactors over `rcpps`
  of the determinant, `0x88B1D6..0x88B2AB`), which the draw's world matrix cancels.
- **Rows = spin R × billboard basis B** (`0x88ACDA..0x88AE00`): row i = Rᵢ·B, row 3 =
  R₃·B + B₃.
- **Spin R** (`esp+0x220..0x250`):
  - |angle| < 1e-4 (`0x8875D4..0x8875FC`): identity (`0x887602..0x8876D8`).
  - Otherwise an axis–angle (Rodrigues) rotation (`0x8876F7..0x887B67`): sin/cos by a minimax
    polynomial after reduction by 2/π, rows c·eᵢ ± s·[a]× + (1 − c)·aᵢ·a.
  - Axis a = emitter +0x190, normalised once at parse time (`0x8A5D00..0x8A5D2F`, rsqrt plus
    one Newton step). +0x188 is `axisType` (random3D 0, WorldSpace 1, ParticleSpace 2,
    SpeedSpace 3, parser `0x8A5A7C`), not the billboarding type.
- **Billboard basis B** (emitter +0x84 = `billboardingType`, parser `0x8A4DBC`:
  InverseView 1, CameraFacing 2, OrientationAligned 3, VelocityAligned 4, HorizonAligned 5,
  ZAxisAligned 6, VelocityFacing 7, VelocityFacingX 8). Dispatch: jump table `0x88CC34`
  before the loop, and `cmp [edi+0x84]` at `0x888918`, `0x889233`, `0x8897D0`, `0x889D4E` and
  `0x88A8D4` inside it.
  - **InverseView** (all K_Tarmac smoke and K_Gravel Large_Stones): table entry
    `0x886FF9`: B = the camera rows copied at entry (`0x886C51..0x886CD9`) from manager
    +0x19F100/+0x19F110/+0x19F120, with zero translation. `setCamera 0x868D60` fills
    those rows from camera +0x90/+0xA0/+0xB0 (position +0xC0 → +0x19F130).
  - **CameraFacing** (`0x88AB37..0x88AC59`): d = normalise(camera position − particle),
    B = `4695B0`(d).
  - **HorizonAligned** (table `0x886E5F`): B = `4695B0`(world up, optionally through the
    node rotation `0x868940`).
  - `4695B0` is an orthonormal look-at frame: up = (0, 1, 0), or (0, 0, 1) when |d·up| > 0.9;
    row 0 = normalise(d × up), row 1 = normalise(d × row 0), row 2 = d, row 3 = (0, 0, 0, 1).
    Rows 0 and 1 are unit and perpendicular even if d is not.
- **Camera rows are unit.** The race camera path `5FA440` builds its matrix from three
  rsqrt-normalised vectors (`0x5FA826`, `0x5FA959`, `0x5FAA9E`; cross products in between).
  `864770` only permutes and negates rows: (row 1, row 0, −row 2, row 3). `63A370` then
  derives the camera matrices with general 4×4 inverses (`rcpps` of the determinant,
  `0x63A7CD`, `0x63ACC7`) and stores the world rows at camera +0x90..+0xC0 (`0x63ACEF..`).
  Inverses and row permutations of a rigid matrix stay rigid, so B has unit,
  perpendicular rows.
  - [I] Other camera callers of `63A370` (`5BCDCF`, `5C8023`, `5E09BC`, `5F3F09/5F4558`,
    `63B010`, `6430B7/64311E`) were not traced. A camera with scaled rows would also scale
    the view-space depth that the near fade and LOD use, so a rigid camera is the only
    consistent reading.

Consequence: for InverseView, CameraFacing and HorizonAligned billboards, the on-screen
quad edge is 2 × the stored size. K_Tarmac Smoke (0.4 → 2.0) ends 4 m across; K_Gravel
stones (0.015 ± 70 %) are about 1–5 cm. Both ports drew the size as the full edge (half the
original) until F319.

## 5. Open

- wheel+0x148 meaning; car+0x64/+0x65; drift object at mgr+0x90.
- Exact renderer use of the per-particle frac and lighting sample (TEXCOORD1 via
  mgr+0x3880), queue classification, selected reduced-target configuration, shader
  sampling and near-fade distances. Section 7 narrows saved blend states, depth
  substitution, resource creation and frame composition; live parity remains open.
- Whether the inherit velocity is refreshed every frame for emitters already created
  (the driver writes it per frame).
- No runtime trace of the original confirms any of this; the live game was not touched.

## 6. Evidence (private)

`local/tyre-smoke/`: `particle_systems/` (readable copies), `surface_materials.xml`,
`textures/` (DDS/PNG previews), `decomp/` and `kickup-driver/decomp/kd1`, `kd2`
(Ghidra output for 0x5BBDC0, 0x66A020, 0x66FA80, 0x898EB0, 0x6358B0, 0x6574F0, 0x6587A0,
0x67C590, 0x898AD0, 0x898DC0, 0x89F1B0 and the pfx functions in `decomp/pfx1..pfx8`),
disassembly dumps, `scripts/` (pe.py, calls.py, decomp.ps1). `kickup-driver/ghidra-copy/`
is a read-only copy of the Ghidra project made while the shared one was locked; it can be
deleted.


## 7. Reduced smoke pass and composition (23 October 2026)

This extends the earlier particle-runtime research with static renderer control-flow
and selected saved shader states. It reuses the same hash-identified analysis PE and
read-only shared Ghidra master/cache. Four additional functions were decompiled across
bounded follow-ups: `0x737D20`, `0x738920`, `0x738E80` and `0x62E350`; other links below
come from existing cached evidence or bounded disassembly. No original live GPU trace
or selected-path comparison was obtained. Function names proposed below are inferred
labels, not original symbols. This section contains independently written structural
descriptions; purchased shader programs and decompiler output remain private.

### 7.1 Saved render states [D]

The selected metadata comes from the purchased `shaderpack/shaderpack.pssg`, SHA-256
`b6b29b80397d12def9c90f5afcff9ea8f335d07da3dc81e293273b468c9029a6`.
Numeric zero booleans are disabled, rather than absent attributes.

| Saved group / pass mask | Colour source / destination factors | Depth test / write | Other saved state |
|---|---|---|---|
| `carglass` / 1 | `SRC_ALPHA / ONE_MINUS_SRC_ALPHA` | LEQUAL / on | `car_glass` layer |
| `car_light_glass` / 1 | `SRC_ALPHA / ONE_MINUS_SRC_ALPHA` | LEQUAL / on | `decals` layer |
| `car_light_pod` / 1 | blending off | LEQUAL / on | `car_layer2`, distinct from the lens cover |
| `pfx_half_smoke` / 1 and 16 | `ONE / ONE_MINUS_SRC_ALPHA` | off / off | separate alpha `ZERO / ONE_MINUS_SRC_ALPHA` |
| `pfx_compose` / 1 | `ONE / SRC_ALPHA` | off / off | separate alpha `ONE / ZERO` |
| `pfx_compose_no_alpha` / 1 | `ONE / SRC_ALPHA` | off / off | alpha writes disabled |
| `skidmarks` / 1 | `SRC_ALPHA / ONE_MINUS_SRC_ALPHA` | LEQUAL / off | skips velocity/depth, environment and shadow passes |

`smokeHalf` maps to `pfx_half_smoke` with a lit particle stream in shipped config.
Its layer flags include `halfSizePFXBuffer` and skipping ordinary Colour. Burnout_circles
instead uses `basicDepthAlphaBlendHalf`; the smoke-only state is not established for
all ground effects. The two saved half-smoke variants link the half-edge and half
fragment-program families and accept `depthmap`. Saved glass/lens depth writes also
exist in other pass masks, but this does not establish their active intact/damaged
variant, depth target or execution position.

With zero RGB/unit alpha initialization, half-smoke's separate alpha factors would
accumulate transmittance and compose's colour factors would implement
`smoke_rgb + transmittance * scene_rgb` [I]. This interpretation has strong supporting
clear arguments below, but backend clear semantics and shader arithmetic remain open.
Saved priorities (`0x70000000` for glass/cover, `0x80000000` for half smoke/compose)
are not enough to reconstruct the queue classifier or comparator.

### 7.2 Resources and dimensions [V; API format meanings open]

`0x737D20` is a particle resource/configuration callback, not the per-frame renderer.
Its config branch creates reduced colour (`+0x90`), alpha (`+0x94`), depth (`+0x98`),
temporary blur (`+0xA8`) targets and a reduced render node (`+0xAC`, copied to `+0x108`).
Shader handles at `+0xBC/+0xC0` select depth 2x/4x reduction; `+0xD0/+0xD4` select
compose/compose_no_alpha. The half-size layer handle is stored at `+0xE8`.

The callback reads configured width/height (defaults 1280/720), shifts both right by
TWO, and passes these quarter-axis dimensions to at least the first reduced-colour
target creator. The name `halfSizePFXBuffer` therefore does not establish half width
and half height. Other creator arguments reuse recovered stack temporaries; universal
per-target dimensions and active quality selection remain unverified. Creator format
enums `0xC` and `0x19` have not been mapped to backend formats.

Setup also binds separate colour/alpha/temp inputs to horizontal/vertical blur shader
families. Resource binding proves that path exists; it does not prove every frame
uses the blur branch.

### 7.3 Depth substitution and reduced accumulation [V]

`0x738920` reduces scene depth. Stage zero resolves `VelocityDepthPlugin`, obtains its
source texture through the object at plugin `+0x7C`, virtual accessor `+0x18`, and uses
the 4x shader (`+0xC0`). Later stages use the previous reduced texture with the 2x
shader (`+0xBC`). The configured default count `+0xA4=1` supports one first 4x stage;
it is not a claim about every original quality path. Sampler0 and dimension inputs
are bound before the fullscreen submission.

`0x728040` binds the final reduced-depth texture as `depthmap` for three shader entries,
including the half-smoke entry. `0x738C00` restores a full-source texture through the
same plugin object's virtual accessor `+0x14`. The distinct accessors have not been
proved equivalent resolved/unresolved depth, and the producer's contents—including
vehicle glass/lens inclusion—remain untraced.

`0x738D60` binds reduced colour at MRT slot0 and reduced alpha at slot1, sets the
viewport, passes float tuple `(0,0,0,1)` to renderer virtual `+0xA4`, then calls virtual
`+0x34` with 1. It substitutes reduced depth, submits the particle tree with mask
`0x10`, restores full depth and unbinds the two targets. This is concrete zero-RGB /
unit-alpha initialization evidence, qualified by the unresolved backend meaning of
those virtual clear calls and MRT channel handling.

`0x738E80` performs reduced-pass preparation: downsample first, conditional accumulation,
then edge-related or blur-related work under manager options. `0x728170` is edge work,
not final composition; `0x738C00` is depth restoration, not composition. Option names
are not inferred from field offsets alone.

### 7.4 Final composition and world groups [V; material classification open]

`0x72E7D0` binds reduced colour to sampler0 of its supplied shader and draws it.
`0x7390F0` is the separate composition/debug entry. Its ordinary composition branch
passes `+0xD4`, the saved **compose_no_alpha** variant, when manager option `+0x388`
is enabled. The alpha-writing `+0xD0` state must not be silently substituted for that
variant. The function also has debug-target display branches; their presence does
not mean the original was running a debug mode.

World constructor `0x5C8080` calls common layer-owner base `0x64BE80`, then installs
primary vtable `0xCE088C`. The base resolves `car_glass` into `+0xDC` but is not proved
vehicle-specific. `0xCE0880` is the secondary interface at object `+0x130`, not this
primary vtable. The frame dispatch is:

- `0x5C8370` supplies the world `+0x450` descriptor to `0x63A300`.
- That path runs preparation (`0x5C85D0`), builds a node table (`0x62E220`), performs
  additional partitioning (`0x62E350`) and enters camera/render wrapper `0x6430A0`.
- `0x63AEB0` invokes the reduced-particle preparation hook (`0x5C8570` to `0x738E80`)
  before main colour-target setup, then world-colour traversal `0x5C8A10`.

The builder maps descriptor groups `+0xB8/+0xC8/+0xD0/+0xD8` to node-table fields
`+0x40/+0x44/+0x48/+0x4C`. Producer/partition evidence narrows their meaning:

| Node-table field | Established contribution or use | Not established |
|---|---|---|
| `+0x40` | preparation passes descriptor `+0xB8` to `FERenderPlugin` | exclusive contents; car-glass membership |
| `+0x44` | preparation passes descriptor `+0xC8` to `SkidmarksPlugin` | that every item is a tyre skid |
| `+0x4C` | `0x62E350` passes its address to particle hook `0x728500`; option `+0x386` extracts the half-size layer into particle `+0xEC` | that the source or remainder contains only smoke |

`0x62E350` does not read the base car-glass handle `+0xDC`; it is not the missing
car-glass classifier. In `0x5C8A10`, exact compose calls at `0x5C8F1A` and `0x5C8FAA`
have two conditional orders: `+0x4C remainder -> compose -> +0x40`, or
`+0x40 -> +0x4C remainder -> compose`. Earlier scene submissions also occur. These
orders are real, but neither branch proves which side contains car glass or lens
covers. Startup/update plugin order in `renderPriority.xml` is not draw-pass order.

### 7.5 Limits and next separating evidence

The original has an explicit reduced smoke/depth/composition path; generic OIT or
forcing every smoke draw last would be authored alternatives, not recovered parity.
The following remain open:

1. Initial material/layer classification into world descriptor groups, specifically
   `car_glass`/lens-cover membership relative to final compose.
2. `VelocityDepthPlugin` production and the meaning/content of its two accessors,
   including vehicle transparent-depth contributions.
3. Backend MRT clear semantics, actual target formats/dimensions and active quality
   options; exact half-edge/compose shader sampling and near fade.
4. A live original trace/reference for the selected BMW smoke scenario and damaged /
   inside-camera variants. Static saved states are not runtime overrides.

Private source evidence is indexed in
`local/handoff/smoke-source-pass-order.md` and `drafts/smoke-source-order/` beside it;
selected purchased-state metadata is in `smoke-source-pass-order-shadergroups.json`.
Shared pseudocode caches for the four new decompiles are under the main checkout's
`local/re-cache/`. None of that proprietary raw evidence belongs in public source.
