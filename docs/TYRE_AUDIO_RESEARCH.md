# Original tyre / surface audio (F276 research)

15 October 2026. Static analysis of the private unpacked analysis image
`local/compatibility/grid-unpacked.exe` (SHA256 `aac46a64…4514`, image base
0x400000). GRID was not launched, the Steam installation was not touched and no
audio was captured. Everything below is **instruction-level static evidence**
unless marked *inferred*. No game bytes are reproduced here; only addresses,
field offsets, names from the decoded tables and numeric constants.

Private evidence: Ghidra pseudocode under
`local/behavior/20261015T0217158750565Z`, `…T0218088699428Z`,
`…T0219543788307Z`, `…T0224345314390Z`, `…T0224539285980Z`; Capstone
disassembly and the helper scripts used under
`local/audio/tyre-audio-20261015/`.

## 1. Data objects

### CollisionInfo (`0x74` bytes, built by `45B700`, preset values by `459D30`)

| Offset | Field |
|---:|---|
| +00/+04/+08 | name string (cap, ptr, 0x20-byte inline buffer) |
| +28 | `AudioVariety*` named by `variety` (looked up in manager list `+4EF4`) |
| +2C/+30/+34 | preset-name string |
| +54 | minVel |
| +58 | maxVel |
| +5C | minVol (dB) |
| +60 | maxVol (dB) |
| +64 | minPitch (ratio) |
| +68 | maxPitch (ratio) |
| +6C | minTreble |
| +70 | maxTreble |

Values are first copied from the named `Preset`, then overridden by any of the
eight attributes present on the `CollisionInfo` element (`45B700`, `459D30`).
The manager keeps them in vector `+4F14..+4F18`. **No tyre routine reads +6C/+70**;
treble is loaded but unused for tyres (all Surfaces entries are 0 anyway).

### Audio-material preset (`0x48` bytes, allocated at `45BF80` in loader `45BC00`)

This is the object the caller called "the 0x48-byte material audio object". It
is built from `AudioMaterials/Presets/Preset` elements, not from `AudioMaterial`
elements. Stores at `45C1A6..45C1ED`:

| Offset | Field | Resolved from |
|---:|---|---|
| +00/+04/+08 | preset name | `name` |
| +28 | carscrape `CollisionInfo*` | `carscrapeColInfo`, list `+4F14` |
| +2C | scrape `CollisionInfo*` | `scrapeColInfo`, list `+4F14` |
| +30 | **rolling** `CollisionInfo*` | `rollingColInfo` |
| +34 | **peeling** `CollisionInfo*` | `peelingColInfo` |
| +38 | **sliding** `CollisionInfo*` | `slidingColInfo` |
| +3C | **skidding** `CollisionInfo*` | `skiddingColInfo` |
| +40 | kickup impact-variety* | `kickupImpactVariety`, list `+4F24` |
| +44 | **scrubbing** `CollisionInfo*` | `scrubbingColInfo` |

Unresolved names leave a null pointer. Presets live in manager vector `+4F34`.

### AudioMaterial (`0x34` bytes, loop at `45C2B8..45C4F3`)

| Offset | Field |
|---:|---|
| +00/+04/+08 | name (e.g. `TARMAC`) |
| +28 | preset* (by `preset`, vector `+4F34`) |
| +2C | kickupMultiplier (f32) |
| +30 | damping (f32; not read by any tyre routine found) |

Vector `+4F44..+4F48`. **The `AudioMaterial` loop reads only `name`, `preset`,
`kickupMultiplier` and `damping`.** The per-material `rollingColInfo=…`,
`peelingColInfo=…` etc. attributes present on entries such as COMPRESSED, DIRT,
ROCK, LEAF, TARMACALT, POTHOLE and WALL are ignored by the original loader;
those materials sound like their `preset` (e.g. DIRT → `fine` → FineRoll,
AbrPeel, AbrSlide, FineSkid, TarmacScrub). Strings 0xCB5640/30/20/10/0xCB55E8
have exactly one reference each, all inside the Preset loop. `4350C0` is a live
reload of `kickupMultiplier`/`damping` only. Behaviour when `damping` or
`kickupMultiplier` is absent (stack local reuse) is not established.

## 2. Surface selection per wheel

* Wheel contact producer `9C1660` stores, per physical wheel record
  (`body+1990+i*150`), `+C0 = materialTable[active].base + token` (resolved
  physical material, already documented as the grip material), `+C4 = hit
  object`, `+C8 = raw material token` (`9C1A8F..9C1AB9`; `9C16C0..C6` clears
  +C4/+C8 during reset).
* `444030` runs once after loading: for every physical material record it calls
  `4361C0`, which compares the record's name (`record+4` string) with every
  AudioMaterial name using exact, case-sensitive `A66720` (strcmp) and stores
  the first matching index as **int16 at record+28**, or -1 when none matches.
* Per frame `456640` calls `436170(wheel+C8)` when the token is non-zero:
  `record = table[active].base + token; idx = (int16)record+28; idx > -1 →
  AudioMaterial = vector+4F44[idx]` (otherwise null).
* Result stored at wheel-audio `+48`. If it differs from the previous value and
  is non-null, change flag `+4C` is set for this frame (`+4C` is cleared at the
  start of every `456640`). On change, `+178 = preset+40` (kickup variety).

## 3. Per-car / per-wheel structure and update chain

Car audio object (per vehicle) contains four wheel-audio objects of `0x1E0`
bytes at `car+30+i*1E0`. Initialiser `413E50` (called from loop `43C916..C94E`)
sets `W+40 = body+1990+i*150` (the physical wheel record, order RL, RR, FL, FR),
`W+44 = car`, `W+48 = 0`. Further fields:

| W offset | Meaning (verified) |
|---:|---|
| +00 | |car velocity| (m/s), written by `456640` |
| +40 / +44 / +48 / +4C | wheel*, car*, AudioMaterial*, surface-changed flag |
| +4D / +4E | event-mode flags (from game mode, `451B17`/`451B3C`; 0 in ordinary racing — *inferred*) |
| +54/+58 | roll voices (pair) |
| +5C/+60 | peel voices |
| +64/+68 | skid voices |
| +6C/+70 | slide voices |
| +74/+78 | scrub voices |
| +7C,+80 | voice flag passed to play: 1 player, 0 AI |
| +84,+88 | mixer group: player 6 (roll) / 7 (others); AI 0x11 both |
| +8C,+90,+94,+98,+9C | active slot index (0/1) for roll, peel, skid, slide, scrub; toggled by `414030(cat)` |
| +150 | car linear velocity copy (body linked +220 via `A69800`) |
| +110..+14F | car pose matrix (linked +1D0 via `A69630`) |
| +164 | pitch offset: rear wheels (anchor z<0) ±1/6 by anchor x sign (`413EB1..EC3`); front wheels not written |
| +168 | 1 - |cos β| from scrub/skid routine (β below) |
| +16C | kickup weight: reset 1.0 each frame, += 20·peel² |
| +17C | wheel+40 (suspension displacement) change since last frame (player only) |
| +1A0 | sound position (player: car-relative rotated anchor; AI: world — *inferred from structure*) |

Per frame `457BE0` (car audio update) computes speed from the linked linear
velocity and calls `456640(dt, &velocity, &pose)`:

* **player car** (`car+A4D8 != 0`): all four wheels;
* **other cars**: wheel object 0 only, and only while `car+97B1` (set at
  construction `43B3F1`, cleared by `41DAE0` once all tyre voices are stopped)
  or the `+4D/+4E` flags are set.

`456640` → `455410` (roll) → `44F330` (peel + slide) → `44FE60` (skid + scrub)
→ `4443D0` (kickup, player only). `dt` is a `double*` frame delta in seconds
(*inferred* from use as a rate multiplier).

## 4. Category formulas

Notation: `ω = |wheel+104|` (wheel angular speed, rad/s; `spin*radius(+E0)` is the
documented surface speed), `r = wheel+F4` (second radius word), `v = |W+150|`
(car speed, m/s), `d = W+17C`, `ci` = selected CollisionInfo,
`lerpCI(x)` = `44F160`/`44F010`: `t = clamp((x-minVel)/(maxVel-minVel),0,1)`,
`vol_dB = minVol + (maxVol-minVol)·t`, `pitch = minPitch + (maxPitch-minPitch)·t`
(linear in dB; handles reversed min/max). Setters clamp **volume to [-48, 0] dB**
(`42C100`) and **pitch to [0.5, 2.0]** (`42C150`). `lin2dB(x)` (`4138F0`):
x≤0 → -48; x≥1 → 0; else `20·log10(x)` floored at -48 (`413890`; `BF4FFC`
treated as log10 — *inferred* from the ×20 and range).

### Roll — preset+30, voices +54/+58, `455410`, player only

```
x = ω + d                                   (455431..455478)
if contact (wheel+C0 != 0) and material:
    (vol, pitch) = lerpCI(x)                via 44F160, read back into W+A0/W+A4
    fade = x < ci.minVel ? lin2dB(x/ci.minVel) : 0
    bump = 0.125 * (pose.y - smoothedY)     (W+1B0; smoothedY += 0.2*diff in 456640)
    setPitch(W+A4 - bump + W+164)
    W+18 = W+18 + 0.2*(W+A0 - W+18)         (per-frame one-pole), W+A0 = W+18
    setVolume(W+18 + fade)
else if no contact: W+A0 -= 32 dB/s * dt; setVolume(W+A0)
```

TarmacRoll: 4..155 rad/s → -40..-12 dB, pitch 0.75..1.2 (≈1.2..46 m/s for a
0.3 m tyre). On surface change the old roll voice is stopped (`A23350`), the slot
toggles and the new one is started at -24 dB (`44F290`); the following frame
re-issues it with the same Bank/Sample index (backend vfunc +20) at -48 dB and
the 0.2 smoother brings it up. Exact effect of that re-issue is not established.

### Peel (wheelspin) and Slide (locking) — `44F330`, all processed wheels

```
ωg     = v / r                              (6.283/((r*6.283)/v), 44F330 prologue)
W+184  = W+184 + (ω - W+184)/3 ; floor 0.01 (smoothed |spin|)
ratio  = max(ωg,0) / W+184
peel   = max(0, 1 - ratio)      (*2.5 if W+4E)
slide  = max(0, ratio - 1)
if v < 10: peel *= 0.1v ; slide *= 0.1v
frac   = wheel anchor z > 0 ? handling(body+E78)+154 : 1 - that
peel  *= wheel_load(+138) / (mass(body+15A0) * frac * 4.905)
```

* **Slide** (preset+38, voices +6C/+70): while in contact,
  `x = slide - 25·W+168`; voice started if absent (`44F010`), else
  `lerpCI(x)`; then a pitch/volume jitter `j` smoothed 0.1/frame from
  `(rand()%n - n/2)/30`, `n = max(1, int(166·W+168))`: pitch += 0.5j,
  volume -= j. Volume set, pitch = W+164 + pitch. Not in contact: -1200 dB/s.
* **Peel** (preset+34, voices +5C/+60): only while `peel > ci.minVel`:
  `peel += cos(0.1·φ)·(sin φ + 1)·0.5·0.125` with `φ = wheel+110` (wheel
  rotation phase; `BF523A`/`BF4E44` are SSE2 libm cos/sin — *inferred* from the
  64-entry quadrant offset 16 vs 0); start or `lerpCI(peel)`; volume
  `vol_new + 0.02·(vol_new - vol_prev)` (`44FCE0..44FD14`); MaxDistance 50
  (150 if W+4E), MinDistance 10 (50 if W+4E); `W+16C += 20·peel²`.
  Below minVel: fade -120 dB/s, stop at ≤ -48. No contact: -1 dB per frame.

TarmacPeel 0.1..0.6 → -9..-1 dB; TarmacSlide 0.25..60 → -48..0 dB.

### Skid and Scrub (lateral) — `44FE60`

```
β-term s = 1 - |dot(normalize(vel), normalize(pose row 2))|  (row 2 = car z;
            z is longitudinal because anchor z selects front/rear above)
W+168 = s
a = s * (v < 5 ? 0.2v : 1)  + d   (+2.5 if W+4D)       -> skid input
b = s * (v < 30 ? v/30 : 1) + d   (+2.5 if W+4D)       -> scrub input
if ω < 10: a *= 0.1ω ; b *= 0.1ω
W+18C = W+18C + (a - W+18C)/13            (skid smoother, 0.0769)
b' = (rand()%100 < 7) ? 0 : b ; W+190 = W+190 + (b' - W+190)/3
skidX  = W+18C ; if v > 20: skidX = min(1, skidX * min(1.8, 1 + 0.05(v-20)))
scrubX = W+190 ; excess = scrubX - ci.maxVel ; if excess>0 scrubX -= 2·excess ; floor 0
```

* **Scrub** (preset+44, voices +74/+78, player only): 4-point curve (`414060`)
  velocity knots `{0, minVel, maxVel, minVel+maxVel}` → volume `{-48, minVol,
  maxVol, -48}`, pitch `{0.9·minPitch, minPitch, maxPitch, 1.1·maxPitch}`;
  then volume += `lin2dB(m)`, `m = ((cos(0.01φ)+1)/2)·((sin(0.1φ)+1)/2)·0.5+0.5`,
  `m *= 0.125v` when v<8; volume forced to -47 when scrubX < minVel. No material:
  -12 dB/s fade. TarmacScrub 0.02..0.07 (β≈11°..21°) → -8.5..-1.5 dB.
* **Skid** (preset+3C, voices +64/+68): when `skidX ≥ minVel`:
  `t = invlerp(minVel,maxVel, skidX - 5d)` (`413990`), pitch = lerp(min/maxPitch)
  + smoothed jitter (`(rand()%n - n/2)/30`, n = max(1,int(66·skidX)), 0.1/frame);
  volume = lerp(min/maxVol) + wobble + `lin2dB(v<8 ? 0.125v : 1)`; the wobble is
  re-drawn when timer `W+1D4` (-= dt) expires: timer = `0.005·(rand()%100) -
  0.04·min(v,100)`, floor 0.015 s; wobble = `-0.001·(rand()%2200)·0.4·min(v,100)` dB.
  MaxDistance 50 (150 if W+4D), MinDistance 10; W+4D also adds +18 dB. Below minVel:
  volume -47. TarmacSkid 0.045..0.25 (β≈17°..41°) → -9..-1 dB, pitch 0.77..1.25.

### Kickup — `4443D0`, player only

Every frame: `p = W+16C · 0.05v · (wheel+C8 != 0 ? 10·kickupMultiplier : 1) ·
W+1BC`, where `W+1BC = 0.005·(rand()%200)` is redrawn every `60 + rand()%180`
frames. If `rand()%100 < int(p)`, a one-shot is played (`435EA0`) at intensity
`0.1·(rand()%10)`, choosing among the kickup variety's three CollisionInfos the
one whose [minVel,maxVel] contains it.

## 5. Voices, varieties and looping

* Each category owns two voice slots; on a surface change (`+4C`) the current
  voice is stopped (`412E00`/`A23350`), the index toggles (`414030`) and the new
  material's CollisionInfo starts on the other slot. `413FA0` stops all ten.
* Start (`44F010` → `435940` → `435690`): bank id from variety+30 name
  (`412E50`); sample from `413B80`: one sample → it; mode +4 = 0 (no data
  attribute observed; *inferred* default) picks uniformly with `A68410(0,n)`
  rejecting an immediate repeat; mode 1 or n = 2 cycles sequentially. Variety
  `+1E0` distance preset supplies MinDistance(+28)/MaxDistance(+2C); Tarmac
  varieties use `surfaceloops` 15/50 (roll) and `skidloops` 10/20; peel and
  skid then override with 10/50 as above. Velocity (Doppler) parameter is set
  to 0 on start. Parameters: Volume E9C190, Pitch E9C18C, Position E9C1B0,
  MinDistance E9C1A8, MaxDistance E9C1A4, Velocity E9C1A0, Bank Index E9C198,
  Sample Index E9C1B4 (registration strings at C7B545..C7B68A).
* Per-voice counts: player 4 wheels × 5 categories × 2 slots (+ kickup
  one-shots); typically one live voice per category per wheel.

## 6. Bank loader facts (car_surfaces)

* Sample descriptor dword +4: **low 20 bits = sample rate** (`A29CA9`,
  `A286A9` mask 0xFFFFF), **bit 20 = loop** (`A2A140..A2A17A`: OpenAL
  `alSourcei(src, 0x1007 AL_LOOPING, 1)`; error string "failed to set loop";
  `A2333F` loop query). The dictionary's "flags 0x0010" high halfword is that
  bit: all car_surfaces samples are looped; 40 kHz is used as the source rate.
* The 4-byte prefix skip at `A29C74..A29C80` depends on the bank-extension
  format code (WIP → 0x1101/0x1103, `A29B6E..A29B89`), not on flags or the
  prefix value. So car_surfaces WIP records also drop their first four bytes
  (00000000) before PCM16 — same branch as the engine banks.

## 7. Verified vs inferred summary

Verified by instructions: all layouts in §1–3, wheel/material selection chain,
roll/peel/slide/skid/scrub input formulas and constants, lerp/clamp/dB rules,
voice pairs, variety selection, loop bit, prefix skip.

Inferred: units (m/s, rad/s, seconds) from documented physics fields; pose
row 2 = longitudinal axis; `BF523A`=cos / `BF4E44`=sin; `BF4FFC`=log10;
event flags `+4D/+4E` zero in ordinary races; position frame; default variety
mode. Not established: effect of the roll re-issue path, sound backend
attenuation/filters, `damping` use, AI wheel index choice beyond "wheel object 0".

## 8. Implementation guidance

A faithful first implementation needs per wheel: material → preset lookup by
exact physical-material name, five looping voices driven by the formulas above,
dB-linear `lerpCI`, clamps, the listed smoothers and fades, random no-repeat
sample choice per start, and loop playback at the descriptor rate after the
4-byte prefix. Player car: 4 wheels incl. roll/scrub/kickup; AI: one wheel,
peel/slide/skid only.
