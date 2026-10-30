# Audio audit against the original (F422)

18 October 2026, Claude Code. Static analysis of the private unpacked image
`local/compatibility/grid-unpacked.exe` (SHA256 `aac46a64…4514`, base 0x400000)
plus decoding of the purchased `audio/` tables. GRID was not launched and no
audio was captured. Earlier audio work: [engine research](ENGINE_AUDIO_RESEARCH.md),
[engine control](ENGINE_AUDIO_CONTROL.md), [continuous engine](CONTINUOUS_ENGINE_AUDIO.md),
[tyre research](TYRE_AUDIO_RESEARCH.md), [tyre audio](TYRE_AUDIO.md),
[native capture](GRID_AUDIO_CAPTURE.md).

Labels: **[O]** observed in instructions or data; **[I]** interpretation.

Private evidence: Ghidra 12.1.4 pseudocode under `local/behavior/`
`20261018T1805095405303Z` (41FAE0 loader, 41F570 constructor, 42B570 reload),
`20261018T1806256670212Z` (427DC0, 427ED0 update), `20261018T1807357765524Z`
(4139F0 camera map, 411C10/411D40 mode set, 42DA50, 457BE0),
`20261018T1809287455802Z` (camera class constructors), `20261018T1817507475196Z`
(43EC80 audio start-up, A22480 backend init). Capstone listings and the Python
table decoders used are in the session scratchpad only; the decoded tables are
reproducible with `tools/binxml_tree.py`-style reading of the purchased files.

## 1. What the original has

`audio/dev.xml` lists 24 switchable systems **[O]**: panel, speech, gear,
aspiration, particulate, swooshes, zingers, risers ("rissets"), hud,
animatedObjects, playerCar, AICars, primaryImpacts, surfaces, ambience, engine,
displacement, suspension, scrape, damage, wind, crowd, music, trackProps.

Banks (`CMSamples.dic`, 124 banks / 2,858 samples) **[O]**:

| Area | Banks / data | Content |
|---|---|---|
| Engine | `eng_<car>.WIP` ×45, `cars/<car>engine.ini` | 6 loops dn/up/up_load × en/ex (F273) |
| Mechanical | `mec_<car>.WIP` ×45, `cars/<car>gear.ini` | 01–08_gear whine loops, clunk1–3, detloop1–3 (backfire), dump1–2 + pip1–2 (blow-off), turbo_hiss, turbo_spindle, start, stall |
| Generic mechanics | `car_genmec.WIP` | handbrake on/off, damage loops (brake, engine, gear, steering, suspension), rim roll/scrape, tyre blow/flap, wheel separation, steam, airbrake, revsig |
| Surfaces | `car_surfaces.WIP` | 185 loops: roll/peel/slide/skid/scrub per surface, kick-ups |
| Collisions | `car_collisions.WIP` (307), `CollisionInfos.bin`, `ImpactVarieties.bin` | land/side/front-back/top hits, yaw/pitch displacement, suspension, scrapes, part separation/dangling/crack/smash, glass and metal particulates, crumple, sweeteners |
| Track props | `env_trackprops.WIP` (300), `audioProps.bin` | break/impact/object sounds per prop type |
| Sense of speed | `env_swooshzing.WIP` | swooshes (medium, small, tunnel, over), zingers, WindSlow/WindFast |
| Crowd | `env_crowd.WIP` (129) | group and grandstand boo/excite/gasp/idle/scream |
| Ambience | `loc_{europe,japan,milan,usa,yoko}.WIP`, `autoplaybanks.bin` | ambience/race-ambience/wind/insect loops, one-shot birds, planes, distant |
| HUD | `audiohud.WIP`, `fe_fx.WIP`, `FEMappings.bin` | countdown, race lights, drift combo/score, flashback, results screens; front-end UI |
| Speech | `speech/<lang>/`, `eventSpeech.bin`, `audioEventSpeech.bin` | team radio / spotter events |
| Music | `Music/music01–08.WIM`, `raceend.WIM`, `replay1–2.WIM`, `femusic*.WIO`, `gar_level1–5.WIA` | race, race end, replay, front end, garage |
| Mix | `GroupModeVolumes.bin`, `effects.bin` | group levels per mode and camera (§2); effects presets (not decoded here) |

## 2. The group mix [O]

`41F570` builds a table of 21 group names (0x28-byte records at +0x600) in
this order: Exhaust 0, Engine 1, Turbo 2, Gearbox 3, Panel Coll 4, World Coll 5,
Wheels 6, Skids 7, Displacement 8, Other Mech 9, Race Music 10, Crowd 11,
Ambience 12, Global Ambience 13, Codriver 14, 3DSpeech 15, HUD 16, AI 17,
Music 18, Speech Level 19, Sense Of Speed 20. Modes (at +0x948): Normal 0,
Talking 1, Crashed 2, Paused 3, StartLine 4, Intro 5, EndOfRace 6,
PausedReplay 7, Transition 8, PlatformModifier 9. Cameras (at +0xAD8):
Internal 0, Bonnet 1, Chase 2, Detached 3, DetachedMultiCar 4, FrontEndCamera 5.
This confirms the group numbers in the tyre (413E50: player roll 6, others 7,
AI 0x11) and engine (4223D0: player Engine 1 / Gearbox 3 / Exhaust 0, AI 0x11
for all three) constructors.

Loader `41FAE0` reads `GroupModeVolumes/Platforms/PC`: Master volume to +C2C,
the PC `Mode` (PlatformModifier) to mode row 9; then every `Modes/Mode`
(attackTime to +BC8+4·mode, groups to +18+4·(21·mode+group)) and every
`Cameras/Camera` (groups to +360+4·(21·camera+group)). Unmatched names are
ignored; unset mode cells keep -48, camera cells 0 (41F570).

Per-frame `427ED0`, for each group g:

```
target = mode[current][g] + mode[9][g] + camera[c][g] + trim[g] + master'
```

`master'` (+C30) moves toward Master (or -48 when +C34 mutes) at 48 dB/s.
On a mode change the published value moves linearly in dB from the previous
mode's target over `attackTime(new) (+C3C)` seconds. The result is published
as the group's volume parameter.

PC values (dB):

| | Exhaust | Engine | Turbo | Gearbox | Wheels | Skids | AI | Crowd | Ambience | HUD | Music | Speech |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Master | -3 (all) | | | | | | | | | | | |
| PlatformModifier | 0 | 0 | 0 | 0 | -0.5 | -2 | 0 | -1 | -1 | -1 | -1 | -1 |
| Normal | 0 (all groups) | | | | | | | | | | | |
| Chase | -4 | -11 | -48 | -20 | -5.5 | -3 | -6 | -3 | -22 | -5 | -2.5 | -1 |
| Bonnet | -11 | -5 | -11 | -15 | -3.5 | -2 | -5 | -3 | -22 | -6 | -2.5 | -1 |
| Internal | -7 | -6 | -11 | -7 | -2.5 | -2 | -5.5 | -9 | -26 | -6 | -2.5 | -1 |

(Full 21-group rows for all modes and cameras are in the purchased file.)
Paused mutes every car/world group (-48) and keeps Global Ambience/HUD;
StartLine lowers AI by 3 dB; Crashed drops engine, exhaust, turbo, gearbox,
wheels and ambience by 24 dB; EndOfRace lowers mechanics and silences
ambience.

Steady race (Normal, Chase): Engine -14, Exhaust -7, Wheels -9, Skids -8,
AI -9 dB. The F275 "observed Chase offsets -14/-7" are exactly this table.

### Audio camera [O]

`4139F0` sets the audio camera from the active view object: a flag at
+0xFC → Internal; mode EndOfRace → DetachedMultiCar; otherwise by the view's
class type at +0x6C: 0 → Chase, 1 → Bonnet, 2 → DetachedMultiCar, 3 → Internal.
The type comes from base constructor `817D60(type)`: the Chase class (vtable
0xD00278, name "Chase") passes 0, Mounted (vtable 0xD00008) 1, Head (vtable
0xD00140) 3; other classes pass 2. So bumper and bonnet use the **Bonnet**
row, head-cam the **Internal** row, chase views the **Chase** row. The +0xFC
flag's meaning is not established.

### Volume sliders [O]

`4124A0(kind, v)` converts a slider value v in [0, 1] to dB:
v ≤ 0 or v < 0.1 → -48; v ≥ 1 → 0; otherwise -24·(1-v). `411DE0` writes it
as a trim: kind 0 (Effects) to groups 0–9, 11–13, 16, 17, 20; kind 2 (Speech)
to 14, 15, 19; kind 3 (Music) to 10, 18. The port previously scaled the
Effects slider linearly.

## 3. Backend level, distance and Doppler [O]

`A2DE87` calls `alDistanceModel(0)` (AL_NONE) and `alDopplerFactor(0)`:
OpenAL applies no distance attenuation or Doppler. `A23B70` computes the
level of each voice in software:

```
d = |emitter - listener|                     (A2E070, stored +C4)
if d >= MaxDistance:                level = -96.3 (silent)
else:
    dist = 0                                  for d <= MinDistance
    dist = floor·((d-min)/(max-min))^(1/rolloff)   otherwise
    level = voice + group + dist + direction term
    if level <= floor:              level = -96.3
gain = 10^(level/20)                           (A2C990, AL_GAIN 0x100A)
```

`floor` and `rolloff` are set by `A22480` from the `43EC80` defaults:
**floor -48 dB, rolloff 0.5** (exponent 2), clamped to [-70, -30] and
[0.1, 10] with "CMStreamSystem" warnings; speed of sound 343 is stored
alongside. No game file overrides them (searched install text/XML).
Min/max distances come from the AudioVariety presets (Engine/Exhaust 10/175,
surfaceloops 15/50, skidloops 10/20, crashimpact 4/40, ...), with the tyre
peel and skid overrides 10/50 (TYRE_AUDIO_RESEARCH §4).

A direction term (fpatan of the listener-relative position, five shapes
selected per group via A240D0: (cos+1-2), 10·(cos+1-2), -20|sin|, -20|cos|)
is added before the floor test. Which groups use which shape is not
established. Groups with flag +1C (2D groups) skip distance entirely.

Interpretation **[I]**: the listener is the rendered camera; 3D panning is
left to OpenAL (positions are still published), so the original output is
stereo/positional while the port mixes mono.

## 4. Gap table

| Area | Original [O] | Port before F422 | Port after F422 |
|---|---|---|---|
| Engine loops by RPM/load | 6 loops, 12 voices, 4371C0 scheduler (F275) | ported (F273–F275) | unchanged |
| Engine group level | Engine/Exhaust groups via table, camera-dependent | fixed Chase -14/-7 | table + audio camera + floor gate |
| Gear shift (wobble, mix, whine) | engine.ini [Physics] GearUp/GearDown mix/wobble, gear.ini GearWhine/Envelope, mec_<car> 0x_gear loops, clunks | none | none (next) |
| Turbo, blow-off, backfire | mec_<car> turbo_hiss/spindle, dump/pip, detloop; Turbo group | none | none |
| Tyres, player | 4 wheels × 5 categories, Wheels/Skids groups | ported (F282), authored -12 dB bus | Wheels/Skids levels, per-wheel distance, floor gate |
| Tyres, AI cars | wheel object 0, peel/slide/skid, AI group | none | ported |
| Kick-ups | 4443D0 one-shots, player | none | none |
| Collisions, scrapes, parts, glass | car_collisions + CollisionInfos/ImpactVarieties | none | none |
| Damage loops | car_genmec damage, rim, tyre blow | none | none |
| Handbrake | hbrakeon/off | none | none |
| Wind | WindSlow/WindFast, Sense Of Speed group | none | none |
| Swooshes, zingers | env_swooshzing, track-side trigger geometry | none | none |
| Other cars' engines | same engine model, AI group, distance law | own car model; authored 0.6·10/d | AI group + emitter distance law |
| Doppler | game-side Doppler parameter (+58); OpenAL factor 0 | none | none |
| 3D position/panning | OpenAL positional sources (stereo) | mono | mono |
| Direction term | A23B70 per-group shapes | none | none |
| Crowd, ambience | env_crowd, loc_*, autoplaybanks | none | none |
| Track props | env_trackprops/audioProps | none | none |
| UI / HUD | fe_fx, audiohud (countdown, lights, drift, results) | none | none; F452: lights, drift, flashback (§7.6) |
| Spotter / team radio | speech banks, eventSpeech | none | none |
| Music | WIM/WIO/WIA streams; Race Music/Music groups | none | none |
| Mix modes | Paused/StartLine/Crashed/EndOfRace/... with attack | pause silences | pause silences; other modes not driven |
| Sliders | dB law, three sliders → group trims | Effects linear | Effects dB law; Speech/Music have nothing to trim |

## 5. Ported in F422

- `grid_tools::audio_mix`: reads `GroupModeVolumes.bin` (`Table`), the
  427ED0 steady-state sum, the audio camera of a view kind, the 4124A0 slider
  law, the A23B70 distance law and floor gate. Asset test: Chase gives
  Engine -14, Exhaust -7, Wheels -9, Skids -8, AI -9.
- Engine mixer: `bus_db` (engine, exhaust) replaces the hard-coded offsets;
  voices at or below -48 dB after the group are silent (previously only the
  voice itself was tested against -48). Default `CHASE_BUS_DB` keeps offline
  probes at the old levels.
- Tyre mixer: per-wheel, per-category level; `BUS_GAIN` (-12 dB authored)
  removed. `Driver::opponent` runs the AI variant (wheel 0, no roll/scrub,
  no suspension term).
- App: the audio camera follows the shown view; the listener is the
  `ViewerCamera`; Effects Volume is a dB trim; opponents use the AI group
  and per-emitter distance (carsetup EngineBay/Exhaust offsets) and get tyre
  voices.

Interpretations and simplifications **[I]**:
- Only the Normal mode is driven (pause keeps its existing silence). Mode
  triggers (StartLine, Crashed, EndOfRace) are called from game flow at
  411C10/411D40 call sites not traced here.
- Master smoothing (48 dB/s from 0 to -3 at start-up) is not modelled.
- The direction term, 3D panning and Doppler are not ported (mono mix).
- The player car's emitters use the same distance law; with the chase
  camera they are inside the minimum distance.

## 6. Next

1. Gear-shift effects of 451770 (GearUp/GearDown mix and wobble from
   `<car>engine.ini` [Physics]) and gear whine (`<car>gear.ini`, mec loops).
2. Turbo/blow-off/backfire (aspiration system) for turbo cars.
3. Collision and scrape one-shots from the existing contact events.
4. Stereo output with the original positional model and Doppler.
5. Mode triggers (StartLine on the grid, EndOfRace after the finish).

## 7. HUD, collision and output-path research (F452)

19 October 2026, Claude Code. Static analysis of the same unpacked image by
three read-only research passes (capstone listings; Ghidra 12.1.4 output in the
worktree `local/gtfix/decomp/pan1`, `pan2`, `coll1`–`coll5`; decoded tables in
the main checkout `local/audio/` including the new `ImpactVarieties.tsv`).
GRID was not launched.

### 7.1 HUD (group 16) [O unless marked]

- One manager, `cmr6_audio_h_u_d_manager.cpp`, Play 0x43E770. Names map to
  sounds through `FEMappings.bin` (loader 0x45D4A0; 165 `Effect` entries
  `name variety volume pitch pan [monophonic]`; record 0x3C: +0x28 volume dB,
  +0x2C pitch, +0x30 pan, +0x34 variety, +0x38 monophonic).
- In a race the wrapper 0x440E10 prefixes `low_` (front-end flag
  `[mgr+0x29064] == 0`), e.g. `race_light_red` becomes `low_race_light_red`.
- Every entry has pan 0: group 0x10 (HUD), no position (2D). Pan ≠ 0 would
  use group 15, distances 1/50 and position (pan·10, 0, 0); the data has none.
- Volume clamp(v, −48, 0) dB, pitch clamp(p, 0.5, 2) (0xCB0BC0, 0xCAF88C,
  0xCB8DA4); the race lights' +6 dB therefore play at 0 dB.
- 20 polyphonic slots (+0x768, 12 bytes); with none free the play is dropped.
  In-race callers pass (a2 = 0, a3 = 1), so retriggering the same effect is
  allowed. Monophonic effects ping-pong between +0x864/+0x868; in a race only
  `low_leaderboard_ud_hover` is monophonic.
- Sample choice 0x413B80: uniform random without an immediate repeat.
- `audiohud` bank: 87 samples, 40 kHz. **Format flag 0x20 is interleaved
  stereo**: A29B0C tests format & 0x200000 and A29B89 selects
  AL_FORMAT_STEREO16 (0x1103) instead of MONO16 for WIP banks; 0x10 loops.
- Triggers:
  - start lights: `StartLightsMessage` handler 0x63E710 (state 0 red, 1 green;
    skipped if the message's silent byte +8 is set). The sequence
    0x6CE350/0x6CE260 advances one stage every 1.0 s (0x6CE383) from stage 1;
    each advance posts red, and green at stage numStages−1. So numStages−2
    reds at 0, 1, … s and green at numStages−2 s (Donington 4 reds and green
    at 4 s; 7-stage tracks 5 reds and green at 5 s);
  - drift scorer (player only, sound object +0x78): `drift_score_up` every
    score step (0x643E3D); `drift_activate` when a drift is counted with combo
    0, else `drift_combo_x{min(combo+1, 10)}`; `drift_rating_x{r}` with r the
    rating before the increment (0x643F78); `drift_valid_close` on leaving the
    close zone armed (0x644038); `drift_combo_bank` at the bank unless banked by
    the race end (0x63C552); `drift_combo_fail` in fail_combo past its guard
    (0x6304BC); `drift_countdown_1` from `sfx` notes on the `Osd/dial2.pssg`
    combo-timer animations (not in code; not ported);
  - flashback: `system/states.bin` StateFlashbackTransition `playSample`
    `flashback_shot_1`, `_2`, `zoomup`, 1.0 s apart;
  - the OSD "timer" box (0x6AD220): `countdown_timer` on each whole second and
    `countdown_go` at 0 (which modes show the box was not traced; not ported);
  - race intro, outro and results screens: front-end code (not traced).
- There are no lap, split or position beeps (`split_pos` is only in the
  placeholder FIXMEVAR variety).
- HUD group levels: Master −3, PlatformModifier −1, modes Intro/Transition +5
  (Paused 0, so the HUD stays audible when paused), cameras Chase −5,
  Bonnet/Internal/Detached −6. A race light in Chase plays at −9 dB plus the
  Effects trim.

### 7.2 Output path, panning and Doppler [O unless marked]

- The game never calls `alListener*` and never sets AL_SOURCE_RELATIVE: the
  OpenAL listener stays at the origin facing −Z, and the game transforms every
  voice position into listener space itself (A23DC0): rel = (p − L)·M⁻¹ with M
  the group listener's orientation (translation zeroed). Orientations are
  converted with diag(1, 1, −1) (436030 → 42C3A0).
- AL_POSITION comes from two `alSource3f` sites in CMStream_Update (a2c93a 2D,
  a2cbd8 3D); AL_GAIN = 10^(level/20), 0 below −48 dB on the 3D path.
- Four listeners (433AA0): 0 2DFlat, 1 Car, 2 Camera, 3 3D_Speech.
  - Car listener: Exhaust 0, Engine 1, Turbo 2, Gearbox 3, Panel Coll 4,
    Wheels 6, Skids 7, Displacement 8, Other Mech 9 (and Codriver 14, 2D).
  - Camera listener: World Coll 5, Crowd 11, Ambience 12, AI 17.
  - 3D_Speech: Race Music 10, 3DSpeech 15.
  - Not3D (group flag +0x1C) on 2DFlat: 13, 14, 16, 18, 19, 20; no distance,
    direction or Doppler; played centred.
  - Car listener pose (43F7A0): Internal/Bonnet use the camera orientation with
    the car's velocity; Chase blends from the car pose to the camera with
    t = clamp((acos(camFwd·carFwd) − π/6)·3/π, 0, 1) (base point not
    reconstructed). The port measures player-car emitters from the camera.
- Direction term: the selector is the listener property `PolarPattern`
  (descriptor EBF730, +0x64), read by A240D0 inside A23B70; θ = atan2(x, z) in
  listener space; shapes 1: 10·(cos θ − 1), 2: −20|sin θ|, 4: −20|cos θ|,
  5: cos θ − 1, others 0. Its only writer is the listener constructor A30200
  (0) and no data file names it, so in the shipped game every listener is omni
  and **the term is always 0** [O reading, I conclusion].
- Doppler (3D path, a2cc1d–a2cd14): v = dot(pos, vel)/(|pos| + 0.01) (positive
  receding), f = c/(c + v) for v > −c else 0, c = 343; AL_PITCH =
  clamp(f · rate ratio, 0.5, 2). vel = (voice velocity − listener velocity) in
  listener space × the voice's Doppler property (+0x58, default 1). Engine
  voices get the car's rigid-body velocity, with Doppler 0 for the camera's own
  car outside the detached cameras (42EE20) [I on the condition
  `[obj+0x110]+0x6C == 0`]. Voices without a velocity (w = 987654) use the
  per-update position change, not divided by dt.
- The output layout belongs to OpenAL: the shipped `mixing="software"` opens
  "Generic Software"; the Speakers option is only logged ("will be over-ridden
  by Windows"). The pan law, speaker layout and any downmix are
  implementation-defined and cannot be reproduced 1:1.

### 7.3 Collisions [O unless marked]

- Per-contact listener 4563F0, registered per car (458EE8 → 996150) and called
  from the body handler 9EF360 with the solved contact: point, side-signed
  normal, other word, impact (+0x3C). `x = 10·impact/mass` (physical +0x3F0),
  then 4559C0(x, point, other).
- 4559C0: d = contact − car position (pose +A420); fdot =
  normalize(pose row 2)·normalize(d); local = d in the car frame (422200).
  - local.y > dim[+1B54] − 0.7: roof (PrimaryRoof and CarScrapeRoof, scrape
    mode 2); local.y < −dim[+1B28]: bottom, no sound. The dimensions come from
    `car+0x20 → +0x24 → 9960F0()` (meaning open).
  - Otherwise only when |d| > 0.5: fdot > 0.8 PrimaryFrontBack (mode 3),
    fdot < −0.8 PrimaryFrontBack (mode 4), else PrimarySides (mode 0 if
    fdot > 0, else 1).
  - Side and roof hits zero the airtime accumulator +A4B0.
- ImpactVarieties.bin (loader 45B040; children in file order give CI soft, med,
  hard): PrimaryFrontBack = carFBSoft/Med/Hard, PrimarySides = carSide*,
  PrimaryRoof = carTop*, PrimaryBot = carLand*, YawImpact, PitchImpact, Crump.
- 4535A0(IV, x, pos): nothing below soft.minVel; soft if x is in its range,
  else med if in its range, else hard. 435EA0: t = clamp((x − minVel)/(maxVel −
  minVel)); volume and pitch lerped (no random pitch); skipped below −42 dB;
  setters clamp to [−48, 0] dB and [0.5, 2]. Cooldown per variety: skipped
  while elapsed ≤ T; on a play T = (rand % 3 + 1)·0.1 s. Player hits above
  hard.minVel switch the mix to Crashed (411C10, attack 0.2 s). Then
  sweeteners: HighSweetenerInfo (x ≥ 40), LowSweetenerInfo (x ≥ 15), with
  the same per-variety cooldown, and silent in the shipped data (§11;
  corrected in F490, this line first said "without cooldown").
- Curves (CollisionInfos): FrontBack soft/med/hard 3–10/10–30/30–60,
  −3..0/−2..0/−1..0 dB, pitch 0.85–1.1; Sides/Roof 2–8.9/8.9–22.35/22.35–40,
  same dB, 0.85–1.2; High sweetener 40–75, −20..−9 dB; Low 15–50, 0 dB.
- Varieties: car{FB,Side,Top,Land}{Soft,Med,Hard} (fb_soft01–06, fb_med01–06,
  fb_hard01–09, side_/roof_/land_*01–03), preset crashimpact 4/40 m;
  LowSweeteners crashlfesweet 8/40; HighSweeteners crashimpact.
- Groups (43B1F0): player Panel Coll 4 (impacts, scrapes, sweeteners, parts)
  and Displacement 8 (yaw/pitch); AI cars group 17; props World Coll 5.
- Position: the player uses the car-relative offset d (Car listener); AI cars
  the world point.
- Landing: airtime in seconds (while 410850, airborne [I]) into PrimaryBot at
  touchdown.
- Scrape loop (453710, per frame 458780): one voice per car on the other
  surface's `carscrapeColInfo` (audio material); gates by mode (horizontal
  speed > 0.3 / 0.1 / 1.0), a contact this step and x > 1; envelope g = 0 at the start (§10),
  refreshed (g = 1 above 15 m/s, else +0.1) and halved per frame when slow or
  the impulse accumulator < 5; player volume lin2dB(|v|·0.04·gv) with 5 %
  −24 dB drops and −(rand % 9) dB jitter, smoothed by 1/3 per frame; pitch
  clamp((|v|/45 − 0.8)/0.7, 0, 1) (at least 0.5 by the setter). These
  per-frame rules depend on the frame rate.
- Displacement (4486E0, player, every frame): yaw = |Δω.z|, pitch = |Δω.x| of
  the world angular velocity per frame into the Yaw/Pitch varieties on group 8;
  frame-rate dependent (F320 rule).
- Not traced: part separation and impact sounds (43BB10, 43C0C0, 44D9D0),
  crump, glass, AI scrape volume, how the Crashed mode ends.

### 7.4 Wind, swooshes, kick-ups and handbrake [O unless marked]

Evidence: Ghidra output in the worktree `local/gtfix/decomp/wind1`–`wind4`; scratchpad `wind/`.

- **Wind**: CollisionInfos `WindSlow` (minVel 4.4, maxVel 80, −48..−10 dB, pitch 0.5) and
  `WindFast` (4.4–80, −48..−13 dB, pitch 0.5–0.7); varieties `wind_slow`/`wind_fast` in
  `env_swooshzing` (loops, preset 4_50: 5/50 m). The car-audio manager constructor 43CF10
  resolves them into M+0xF8/+0xFC (string references 43D4E8, 43D515), but no reader of those
  fields was found. The `wind` dev switch (+0x4C8C) is set and registered but never read, and
  no play call passes group 20 (Sense Of Speed). [I] In the shipped PC build the wind loops are
  loaded but never started; not ported.
- **Swooshes** (`cmr6_audio_swoosh_manager.cpp`, constructor 441F40, update 44E860): six
  varieties and `Swoosh_Info` (1.2–1.75, −12..−2 dB); triggers 44B1D0/44B2F0 on per-direction
  values whose writer was not found [I: distances from the listener to nearby geometry].
  Small swooshes play on Ambience 12, the others on World Coll 5. Zingers are −96 dB in the data
  (silent). Not ported.
- **Kick-ups** (4443D0, player only, per wheel and audio update):
  p0 = 0.05·v·(material token ≠ 0 ? kickupMultiplier·10 : 1); a per-wheel counter redraws
  scale = 0.005·(rand % 200) every 60 + rand % 180 updates; p = weight·p0·scale, where weight
  is 1 plus 20·peel² while peeling. If rand % 100 < (int)p, x = 0.1·(rand % 10) selects the
  ImpactVariety's first CollisionInfo (only it can match x ≤ 0.9): KickupSmall (−8..−3 dB,
  pitch 0.85–1.15) or, on grass, GrassKikCrunch. Samples s_sml1–15 / s_veg_crunch01–06 in
  `car_surfaces` (one-shots), group 6 (Wheels), preset kickup 10/20 m, no cooldown, Doppler 0.
  Probability is per frame (frame-rate dependent). Not ported yet.
- **Handbrake**: hbrakeon/hbrakeoff (car_genmec) are resolved into M+0x128/+0x12C, but no
  reader was found [I: unused like wind].
  Corrected in F582: 43C320 reads them (through [[car +0x2C] +0x18] = M+0x70, +0xB8/+0xBC) and
  plays them on the edges of input 9961B0(8), player only, from 457BE0 (§26).

### 7.5 Crowd and ambience [O unless marked]

Evidence: worktree `local/audio-crowd-research/`, `local/gtfix/decomp/crowd1`–`crowd8`.

- The location bank comes from `database.bin` `track_model.local_audio_ambience` (Donington
  and most of Europe `loc_europe`, Le Mans `loc_lemans`, Japan `loc_japan`, Yokohama
  `loc_yoko`, USA `loc_usa`). The autoplay manager 0x461430 looks up an AutoPlay of that name
  (case-insensitive); there is none for loc_europe/japan/lemans, so those start silent.
  `tracks/*/ambience_triggers.xml` boxes (`AMBIENCE_REVERB`, SETTING = AutoPlay name) switch
  the active AutoPlay when entered [I]; Donington has three boxes, all `loc_britcir`.
- There is no 2D bed in a race. 0x44D420 spawns at most one voice per tick:
  `random(0, 50) ≤ density·clamp(10·v, 10, 100)`, then a uniform sample, `random(0, 1) ≤
  probability`, a free slot of 96 and the maxLoopers limit. The voice is positioned near the
  listener (spawn geometry partly traced). Gain per tick: lerp(minVol, maxVol, fade·speed
  band) + `autoplay` CI (0 to −40 dB from 17 to 62 m/s) + lin2dB(fade over 30 ticks).
  Voices are freed beyond 117 m.
- Crowd (0x44E220): up to 35 of the visual crowd instances within 150 m of a point ahead of
  the car (70 m·linstep(1, 120, mph)); NormalCrowd / GrandStandCrowd presets by people count;
  excitement from `BroadcastMessage_EventMessage` (0x424B60), with overrides at the finish and
  in replays; gasps on `BroadcastMessage_CrowdRecoil` (0x41BCD0). Groups: Crowd 11,
  location Ambience 12.
- The port lacks the inputs (ambience triggers, autoplaybanks, crowd instances, race messages);
  not ported.

### 7.6 Ported in F452

- `grid_tools::hud_audio`: `FEMappings.bin` and `audioVarieties.bin` readers, the `audiohud`
  bank including stereo samples, the 20-slot player with the clamps above, and the start-light
  timing (`light_cue`). Asset test `shipped_hud_effects_load_and_play`.
- `grid_tools::drift::Scorer` queues the player's HUD cues (score step, activate/combo,
  rating, valid close, bank unless at race end, fail; at most 64 queued).
- grid-app `hud_sound`: one stereo source on the HUD group (Normal row, camera, Effects
  trim; it stays audible when paused, as the Paused row keeps 0 for HUD). Start lights follow
  the original's 1 s stage timer from the countdown start (red for each second up to
  stages − 3, green at GO), which differs from the port's visual board cadence. Drift cues come
  from the player's scorer; flashback flashes play shot 1, shot 2 and zoom up.
- Not ported: the OSD timer box countdown_timer/go (modes not traced), drift_countdown_1
  (pssg animation notes), race intro/outro/results screen sounds, the monophonic leaderboard
  hover.
- The sample choice uses an authored LCG for the uniform no-repeat pick (0x413B80 uses the game
  RNG) [I].

## 8. Low frame rates (F453)

19 October 2026, Claude Code. Owner report: on the phone the sound becomes choppy below about
30 fps (log main checkout `local/android/phone-logs-20261019-0520/log-20261019-052045.txt`: a
12-car Donington race at 19–23 fps; engine and tyre audio running at 48 kHz). The original was
not run.

**Cause in the port [V code, V test].** The vehicles tick at 62.5 Hz (16 ms) in FixedUpdate,
and the engine and tyre mixers run on the audio thread in 16 ms blocks (768 samples). The
parameters (RPM, throttle, tyre voice targets) were published as one latest value per
rendered frame. At 20 fps Bevy runs about three ticks back to back per 50 ms frame, so the
audio thread saw only the last of them: the pitch and level stepped every 50 ms instead of
every 16 ms. The engine block applies its step and gain abruptly, and tyre pitch is applied
unsmoothed, so the 20 Hz stepping is audible.

**Audio-thread load [V, desktop].** Probe `grid-audio-load ROOT SECONDS UPDATE_HZ` (grid-probes)
renders 12 engine mixers and 12 tyre mixers (player 4 wheels × 5 categories, opponents wheel 0
peel/slide/skid) in 10 ms callbacks. Result: about 10 % of one desktop core (median callback
1.0 ms, slowest 1.9 ms of 10 ms); with every tyre slot audible, 28 %. The update rate does not
change the cost. A phone core is slower and competes with a CPU-bound game, so callback
deadlines are tighter there [I]; this was not measured on the phone. The rodio/Bevy mixing of
25 sources is not included in the probe.

**The original [O/I].** Its audio parameters are written once per rendered frame (the
car-audio update and the 427ED0 mix from the game loop; CMStream_Update sets the OpenAL
source properties), with the engine scheduler stepped by the frame delta. Its world also steps
once per frame, so no ticks are skipped; OpenAL mixes in its own thread with buffered output
(§7.2). At 60 fps the original therefore updates engine and tyre parameters every 16.7 ms.
No extra parameter smoothing beyond OpenAL's own was found [I].

**Fix.**
- `grid_tools::audio_pacing::Paced`: the game pushes one value per vehicle tick and the audio
  thread takes one per 16 ms block. When nothing new has arrived the last value holds. More
  than 4 queued values skip ahead to the last 2, and at most 8 are kept, so the two clocks
  cannot drift apart.
- Engine (`engine_sound`): `publish` (FixedUpdate after `drive_tick`) queues the player's and
  each opponent's control per tick. A change without a tick (pause, mute, finish, failure) is
  queued from the frame update. RPM and throttle advance per block. Audibility and the reset
  generation still follow the newest control within 64 samples, so pause and mute stay
  immediate.
- Tyres (`tyre_sound`): the per-tick driver pushes its voice targets; the decoder takes one set
  per 16 ms block. Levels and audibility are unchanged.
- At 60 fps one tick runs per frame and the mixers see the same values as before; the physics
  and the identities are unaffected (audio never writes physics).
- Tyre mixer: a voice with target 0 and gain below 1e-6 only advances its cursor, which saves
  mixing work for silent voices without changing the output.
- Diagnostic `GRID_FRAME_CAP_HZ=20` (desktop and Android) sleeps at the end of each frame to
  reproduce a slow device's frame rate.
- Tests: `a_slow_frame_reaches_the_mixer_one_tick_per_block` (three ticks pushed in one frame
  reach three consecutive blocks), `audio_pacing` units, and the existing pause/mute/reset test.

**Not verified**: listening on the phone after the fix; whether the phone's audio callback
also underruns (an Android log of output underruns would show it). If it does, the next steps
are a larger output buffer and mixing the 25 sources into fewer.

## 9. Collision impacts (F479)

19 October 2026, Claude Code. Static analysis (§7.3 and the worktree's
`local/gtfix/decomp/colldims`). GRID was not launched.

**Dimensions [O].** 4559C0 reads its roof/floor heights through
`[[audio car +0x20] +0x24]` → 0x9960F0 (`lea eax, [ecx+0xD240]`): the car description copied to
body +0xD240. Description +0x1B28 / +0x1B54 are body +0xED68 / +0xED94, written by 0x9E2030
from 0x99D180, the minimum and maximum of the `collision_main` points (CAR_COLLISION.md). So
the floor height is the lowest point's y and the roof the highest point's y, in model
coordinates (the Lacetti's floor is +0.103, as read live in BRAKING_REFERENCE). The tests are
at 455D7A (roof: local y > roof − 0.7, 0xCB0834) and 455DAD (floor: −floor > local y).

**Port** (`grid_tools::collision_audio`, grid-app `collision_sound`):
- Data: `car_collisions` (307 samples), the car_collisions AudioVarieties with their preset
  distances, CollisionInfos (presets plus per-info overrides) and ImpactVarieties (soft, med,
  hard in child order).
- Listener, once per vehicle tick (FixedUpdate after `drive_tick`, as F453), for every car:
  - contacts come from `Runtime::impacts()` solo and `multi.contact_impacts(k)` in a field;
  - x = 10·impact/mass;
  - `classify` (4559C0) uses the car frame of the physical pose, with heights compared in
    model coordinates (body − vehicle +0x1630) [I: the audio object's car matrix taken as the
    physical pose] (F490 removed this conversion, §11);
  - the soft/med/hard pick and curve follow 4535A0/435EA0;
  - one-shots quieter than −42 dB are skipped, and per-variety cooldowns of
    (rand % 3 + 1)·0.1 s are shared by all cars;
  - sweeteners at x ≥ 40 (High) and x ≥ 15 (Low).
- Mix: the player's one-shots play on Panel Coll measured from the car (the Car listener [I:
  the car's pose]), AI cars' on the AI group measured from the camera; A23B70 distance law
  with the variety's preset (crashimpact 4/40 m, crashlfesweet 8/40 m). One mono source with
  up to 32 voices [I]; the level is fixed when a voice starts.
- Not yet: the scrape loop (needs the other surface's audio material), landing (PrimaryBot by
  airtime), yaw/pitch displacement, the Crashed mix mode, part/glass sounds, stereo.
- Tests: `contacts_are_classified_like_4559c0`, `curves_interpolate_and_clamp`,
  `shipped_collision_data_and_listener` (the hard front hit plays carFBHard plus both
  sweeteners, the cooldown skips a repeat, the mixer plays).
- Not heard in a windowed race.

## 10. Scrape loop (F488)

19 October 2026, Claude Code. Static analysis: Capstone 5.0.6 listings of 0x453710, 0x4563F0,
0x436170, 0xA69930 and 0x413890 from `local/compatibility/grid-unpacked.exe`, the Ghidra
pseudocode of §7.3 (worktree `local/gtfix/decomp/coll4/{453931,455ebd,458c84}`,
`wind3/448810`), and constants read from the shipped `grid.exe`. GRID was not launched.

**Call site [O].** 4559C0 calls 453710 *before* the impact one-shot, and only when 436170 maps
the listener's material argument to an AudioMaterial. That argument is the other side's
material word of the event (9EF360 passes event[3 − side], which B0CFC0 fills from contact
+0x30/+0x34). The info is that AudioMaterial's preset `carscrapeColInfo`
(`*(*(material+0x28)+0x28)`); the roof passes a fixed entry of the car's impact table instead
(`*(…+0x18)+0xC`, taken as CarScrapeRoof [I]). Modes: sides 0 when the longitudinal cosine
is > 0 (0xD85A50 = 0), else 1; roof 2; front 3; back 4.

**Which info [O, data].** The AudioMaterial loop reads only `preset` (TYRE_AUDIO_RESEARCH §1),
so WALL, CAR_* and the other per-material `carscrapeColInfo` attributes are ignored. Every
preset names CarScrapeSideRoad except `grass` (CarScrapeSideOther). All three scrape infos use
the `scraping` preset (minVel 0, maxVel 1, minVol −24, maxVol −18 dB).

**453710 [O].**
- Speed gate by mode: horizontal speed (the car velocity with y masked, mask 0xCAF410) ≥ 0.3
  (modes 0/1, 0xCB0DCC), 1.0 (mode 2) or 0.1 (others). It also needs the audio car's body
  (+0x24), A69930 true (body +0x40C set this step; it returns the impulse accumulator +0x240,
  to which B0FC00 adds each solved contact's +3C × normal) and x > 1.
- No voice yet: the envelope +0xA328 = 0, the info is stored at +0xA324, the voice starts on its
  variety at the contact, the mode is stored, return. The info is never replaced, so a car
  keeps the surface of its first scrape for the race.
- Otherwise: if g < 1 it becomes 1 at a horizontal speed above 15 (0xCB3BA8), else g + 0.1;
  the position is the contact, replaced for mode 0 by 1.5 × the pose row at +0xA3F0 (the x
  axis), mode 1 by −1.5 × it, mode 2 by the +0xA400 row (y axis); then 448810 and the
  pitch/volume/position setters.

**458780, every frame for every car [O].** With a voice: g ≤ 0.1 sets the voice to −48 dB
(0xCB0BC0) and nothing else; otherwise g halves when |v| < 0.5 or |impulse| < 5 (0xCB093C),
the position is placed by the stored mode, and 448810 and the setters run.

**448810, the player only (+0xA4D8) [O].** t = the envelope's place between the info's minVol
(+0x5C) and maxVol (+0x60), 1 above both, so 1 for the shipped data. Pitch: s = |v|/45
(0xCB4AA0); 0 for s ≤ 0.8, (s − 0.8)/0.7 below 1.5, else 1 (the setter raises it to 0.5).
Level: lin2dB(|v| × 0.04 × t) (0x413890: −48 at or below 0, 0 at or above 1), −24 dB when
rand % 100 < 5, − (rand % 9) dB, then +0xA31C += (target − +0xA31C) / 3.

**Port** (`grid_tools::collision_audio` Listener/Mixer, grid-app `collision_sound`):
- Per vehicle tick: contacts first (453710 inside the listener), then `scrape_frame` (458780),
  as the original's step then frame order; the per-frame rules run per tick [I, F453 pacing].
- The other side's AudioMaterial: the port's contact material words are solver indices (always
  0), so for a packed track contact the triangle under it is queried again with a 0.25 m ray
  along the normal (`Runtime::track_surface_at`) and its mechanics name is the AudioMaterial;
  another car is CAR_; the boundary sheet and route props are taken as WALL [I]. All of these
  except grass resolve to CarScrapeSideRoad, so the choice only matters on grass.
- The impulse accumulator is summed from the car's solved records (impact × side normal), set
  when the car has any [I].
- AI cars run the same level and pitch rules [I: the original updates neither for AI voices,
  their level is not traced]. Emitter: the car plus the mode's offset for every car (the
  player is measured from the car, the AI group from the camera) [I: for AI cars the original
  writes the offset as a world position].
- One looping voice per car in the collision mixer, one sample picked at start; gain and pitch
  glide to the tick's targets over about 10 ms [I].
- Tests: `scrapes_follow_453710_and_458780` (start at 0, +0.1 / 1 above 15 m/s, offsets by
  mode, halving without impulse, gates, the AI group, roof info);
  `shipped_collision_data_and_listener` also checks the material → info map and that the loop
  keeps playing.
- Not yet: landing (PrimaryBot by airtime), yaw/pitch displacement, Crashed mix mode, stereo.
- Not heard in a windowed race (no rendered app runs at the coordinator's request).

## 11. Collision mix (F490)

20 October 2026, Claude Code. The owner on the F486 build: collisions are very loud, and
several at once or close together are unpleasant. Static analysis (Capstone 5.0.6 on
`local/compatibility/grid-unpacked.exe`: 42D580, 435940, 4535A0 tail, 43B9C0–43BAE4,
45DA46, A66760 → BF5F65) and the shipped data; a 12-car headless race traced through the
listener (`GRID_COLLISION_AUDIO_TRACE`, worktree `local/mix/`). GRID was not launched.

**Group levels match [O].** GroupModeVolumes, PC: Master −3, PlatformModifier Panel Coll
−1, AI 0; Normal 0; cameras Panel Coll Chase −6, Bonnet −5, Internal −5; AI −6/−5/−5.5.
So the player's impacts play at −10 dB (Chase), opponents' at −9, the player's engine at
−14 and exhaust at −7. The port already used this table.

**Sweeteners are silent in the shipped game [O].** 43B9C0–43BAE4 fill the car audio object:
+0xA6A8 = variety ParticulateMetal (read by 448810), +0xA6B0 = variety LowSweeteners (never
read again), +0xA6AC = variety HighSweeteners (only its +4 set to 1), +0xA69C =
CollisionInfo HighSweetenerInfo, +0xA6A0 = LowSweetenerInfo. 4535A0 plays the sweeteners
through 44F010 with the *info*, so the voice uses the info's variety. Both infos name
`variety=dummy`. Names are matched with A66760 → BF5F65 (CRT `_stricmp`), so "dummy"
resolves to `Dummy` (bank env_swooshzing, preset dummy 1/2 m), which has no Samples:
nothing plays. F479 played LowSweeteners (0 dB, crashlfesweet) and HighSweeteners on every
hit above x 15/40; that added extra, loud layers on top of the hits.

**Sweetener cooldown [O].** 4535A0 passes 1 as 44F010's seventh argument, which 435940
reads as its cooldown flag. With it set, the variety's timer (manager +0x50 + idx·0x28) must
have run past T, and T restarts at (rand % 3 + 1)·0.1 s, as for the impacts. F479 had no
cooldown here. Both infos share the Dummy variety, so they share one cooldown.

**Roof/floor heights [O, correction].** 4559C0 compares the contact's height in the audio
pose's frame directly with the description heights (description +0x1B28/+0x1B54). F479
subtracted the body offset (vehicle +0x1634, −0.39 m on the Lacetti), an assumption the code
does not make. With it, door-height car-to-car contacts (pose-frame y ≈ 0.38) were "roof"
hits (y > 1.42 − 0.7) and played carTop alongside carSide from the same crash. Underbody
contacts also missed the floor test. The 12-car trace (6000 ticks, Donington, DF01 and
characteristics), before → after:
- carTop one-shots 11 → 0;
- sweeteners played 4 → 0 (now "dummy", skipped by the mixer);
- roof scrape frames 117 → 0;
- side/front/back one-shots 18 → 21;
- at most 3 one-shots per tick, none roof.

**Request cap [O, not ported].** 42D580 queues a one-shot only while manager +0x5B4C is
below 15, and increments it. No reset or decrement was found by displacement search (only
the constructor zeroes it), so its lifetime is open. It is not ported; per-variety cooldowns
keep the port well below 15 at a time.

**Still [I] / open.** The −42 dB gate and curves are unchanged. AL_GAIN per source and
the software mixer's summing and clipping belong to OpenAL. The port sums one-shots linearly
in one mono source. The Crashed mode (player hit above hard.minVel: engine, exhaust, wheels
−24 dB, attack 0.01 s) is still not ported, and with it the collisions would stand out
*more*. No rendered run (at the coordinator's request), so the result is not heard.

**Opponent engines louder than the player's [O table, I conclusion].** The owner reported
that an opponent sometimes sounds louder than the player's own car. Within the 10 m minimum
distance every engine emitter is at 0 dB distance attenuation (A23B70 law, Engine/Exhaust
preset 10/175 m), so the level is the group mix:
- Chase: player Engine −14, Exhaust −7; every AI voice (engine and exhaust) −9. A close
  opponent's engine layer is 5 dB above the player's own.
- Bonnet: player Engine −8, Exhaust −14; AI −8. The opponent's exhaust is 6 dB above the
  player's.
- Internal: Engine −9, Exhaust −10, AI −8.5.

So with the original's table a car beside the player can be louder than the player's own
car, as heard. The port measures the player's emitters from the camera, not the Car
listener, but both are within 10 m, which gives the same 0 dB. Several opponents within 10 m
add without a limit in the port; no AI engine voice limit was found in 451770 or 4223D0 (all
AI engine voices are on group 17). Port gaps that change this:
- the StartLine mode (AI −3 dB, attack 1.5 s) is not ported (row 71; its trigger and end are
  not traced). At the start every opponent is within 10 m and plays 3 dB louder than in the
  original;
- there is no positional panning (row 71). In the original a car beside the player sits on one
  side instead of on top of the player's engine.

To settle it live, read from grid.exe with a car alongside at the start and mid-race:
- the current mode index;
- the published group volumes of groups 0, 1 and 17 (427ED0's per-group parameter);
- each engine voice's AL_GAIN (the CMStream_Update 3D path) for the player and that opponent.

## 12. Stereo panning (F492)

20 October 2026, Claude Code. Static: §7.2 (A23DC0, CMStream_Update, PolarPattern) and the
backend's `alDistanceModel(AL_NONE)` with Doppler factor 0 (A2DE87, `audio_mix`). GRID was
not launched.

**What the original does [O].** The game moves every 3D voice into its listener's space
itself: rel = (p − L)·M⁻¹ with the listener's orientation. It hands that to OpenAL as
AL_POSITION, with the OpenAL listener fixed at the origin. The direction term is always 0
(PolarPattern is never set), distance is the game's own software law, and Doppler is off. The
only spatial cue left to OpenAL is the panning of that position. With `mixing="software"` that
is the "Generic Software" device, whose pan law is implementation-defined.

**Port.**
- `audio_mix::pan_gains` maps a listener-space position (x right, z forward) to equal-power
  gains on the horizontal azimuth, p = sin(azimuth), front and back alike, scaled by √2 so a
  centred voice keeps unit gain [I: the law].
- `grid-app::stereo` turns a mono decoder into interleaved stereo with a shared `Pan`, read
  every 64 frames, the gains gliding over about 10 ms [I].
- Listeners (433AA0):
  - the Camera listener for the AI group (opponent engines, tyres, impacts, scrapes) is the
    rendered camera;
  - the Car listener for the player's groups is at the player's car, facing the camera's way
    [I: 43F7A0's chase blend from the car pose to the camera is not reconstructed].
- Per source:
  - opponent engine and tyres: one pan per car, from the car's position;
  - the player's engine (EngineBay (0, 0, 4), Exhaust (0, 0, −4)) and its mixed four wheels:
    centred [I: per-emitter and per-wheel panning would need stereo inside those mixers];
  - collisions: each one-shot is panned by its contact point at its start (the player's from
    the car, AI cars' from the camera), and each car's scrape loop by its emitter every tick;
  - HUD: already stereo and 2D (centred).
- Tests: `pan_is_equal_power_on_the_azimuth`, `mono_frames_become_panned_stereo_pairs`,
  `pan_follows_the_camera_orientation`, and the collision mixer's hard-right one-shot and
  centred scrape loop in `shipped_collision_data_and_listener`.
- Not heard (no rendered run).

## 13. Mix modes: StartLine (F493)

20 October 2026, Claude Code. Static: 411C10, 427ED0, 5791A0/579210, 41F9xx and the decoded
`system/states.bin` / `flow.bin` (`local/pause-menu/`). GRID was not launched.

**Who sets the mode [O].** No code pushes StartLine (4) as a constant; the 18 callers of
411C10 set Crashed (2, duration 0.2 s, from 4535A0 and 436480), Paused (3), PausedReplay (7),
Normal (0) or return to the remembered mode (+C24). StartLine comes from the race flow:
the state class with vtable CD77E0 reads `mode` (name lookup 4709D0 → 411AE0) and
`start_race` (5791A0); its enter method 579210 calls 411C10 through 470E40 and, with
`start_race`, 4120E0 (race-started flags, no mix change). `states.bin` defines eight
`StateAudioSetState` states (Intro, Paused, StartLine, Normal, PausedReplay, Transition,
Normal with `start_race`, EndOfRace). In the career and test-drive flows of `flow.bin`:
- `c_pre_set_audio_startline` wraps the pre-race sequence: head-cam decision, pan to the
  vehicle, then `startseq` (the start lights);
- `startseq`'s NEXT is `c_pre_set_audio_normal` = `set_audio_normal_start_race`, the green
  light;
- a restart from the pause menu goes `restart_race` → `c_pause_set_audio_startline` → the
  pre-race again.

**411C10 with duration 0 (persistent) [O].** Ignored if the mode is unchanged. Extra fade
time +C3C: 2.5 s (CB092C) when the fade being left came from Transition and the new mode is
Paused, 1.5 s (CB0BC4) when it came from EndOfRace and the new mode is StartLine. Then +BF4 =
old mode, +BF0 = new mode, the +BF8 timer restarts. With a nonzero duration (Crashed) the
mode is transient: 427ED0 returns to +C24 when the timer passes the duration minus the
attack.

**427ED0 per frame [O].** For each of the 21 groups, target = mode + platform + camera +
master' + trim. A group at its target counts as settled. Otherwise:
- while +BF4 ≠ +BF0: level = lerp(old mode's target, target, e / (attack[new] + extra));
- after that: level = lerp(current level, target, e / 1.5) (CB0BC4), e the time since the
  mode change, clamped to the target at t ≥ 1.
When 20 or more groups are settled the fade ends (+BF4 = +BF0, +C3C = 0).

**Consequence [O].** StartLine differs from Normal only in AI (−3 dB; attack 1.5 s, Normal's
0.15 s; asset test). On the first frame 20 groups are settled, so the attack-time fade ends at
once and AI follows the second law, both into StartLine and back to Normal: about 99.8 % of
the 3 dB within 0.5 s at 60 frames per second, exact by 0.8 s.

**Port.**
- `audio_mix::ModeMix`: 411C10's persistent set (with both extra times) and 427ED0's two laws
  on each group's mode term; `group_db` adds platform, camera, master and trim. The terms
  evolve as the published levels do while camera and trim stay constant [I]. Stepped at a
  fixed 60 Hz so the fade does not depend on the frame rate [I: the original steps once per
  frame].
- `grid-app`: `AudioMode` resource; `mix_mode` sets StartLine while the time trial's phase
  is Countdown (the start lights, also after a restart) and Normal otherwise, then advances
  with real time. The engine, tyre, collision and HUD levels use it. Free practice has no
  start sequence and stays Normal; the pre-race pan to the car is not part of the port, so
  StartLine starts with the countdown [I].
- Not driven: Paused (the port's pause keeps its own silence), Crashed, EndOfRace, Intro,
  Transition, master start-up smoothing.
- Tests: `start_line_lowers_ai_with_the_427ed0_fade` (first frame, 0.5 s, settle, back to
  Normal, idempotent set) and the asset test (StartLine = Normal except AI, attack 1.5).

### 13.1 Crashed (F496)

Static: 4535A0, 435EA0/435B20, 436300, 411C10, 427ED0; the Crashed row of `GroupModeVolumes.bin`
(`local/mix/gmv.txt`). GRID was not launched.

**Trigger [O].** 4535A0, after the soft-range test and only for the player's listener (+A4D8):
when x > the hard info's minVel it calls 411C10(Crashed, 0.2 s) (CB0830). If that returns true,
the one-shot's flag to 435EA0 becomes 0; 435EA0 passes it to 435B20 as its ninth argument, and
435B20 checks the per-variety cooldown only when that flag and its seventh are set. So the
crash hit itself is never cooled down. The other call site, 436300 (the "game broadcaster"
handler of queued play messages, registered at 45DB99), sets Crashed for a message whose +24
flag is set; that path is not ported.

**411C10 with a duration [O].** Refused (false) while the current mode is Paused, PausedReplay
or Transition, or when the mode is already current. The hold +C18 is the duration minus the
attack of the remembered mode +C24 when the duration is longer, else the duration; +C24 is
read before it is updated. +C24 then becomes the interrupted mode when that one was
persistent and not Paused, EndOfRace, PausedReplay or Transition. +C21 = (duration == 0).
427ED0, while +C21 is false and the timer passes the hold, calls 411C10(+C24, 0) and publishes
nothing that frame.

**Crashed row [O].** attackTime 0.01; −24 dB on Exhaust, Engine, Turbo, Gearbox, Wheels,
Ambience and Sense Of Speed; AI −6 dB; the rest 0. Eight groups differ, so the attack-time fade
runs fully.

**Consequence.** From Normal (attack 0.15): the engine, exhaust and wheels drop by 24 dB within
one frame, hold for 0.05 s (0.2 − 0.15), then return linearly over 0.15 s from the Crashed
level. From StartLine the hold is also 0.05 s (+C24 still Normal), and the return takes 1.5 s.
A hit while Crashed neither extends it nor bypasses the cooldown.

**Port.** `ModeMix::set_for` (the transient 411C10) and the 427ED0 return; `Listener::
contacts_crashing` asks for Crashed through a closure (the app's `Listening::crash`), and the
accepted hit skips its cooldown. `AudioMode` holds the mix behind a lock, and `mix_mode` sets
the flow's mode only when the flow changes, so a Crashed in between returns by itself.
`grid-opponent-race` keeps `contacts` (no mix). Tests: `crashed_ducks_the_engine_and_returns`
and the crash part of `shipped_collision_data_and_listener`. Not heard.

### 13.2 EndOfRace (F500)

Static: `local/pause-menu/flow.xml`, the EndOfRace row (`local/mix/gmv.txt`), 411C10. GRID was not
launched.

- [O] `c_post_set_audio_endofrace` (`set_audio_endofrace`) is the parent state of
  `results_show_screen` and the results screens. Entering the results through any link
  enters it, so the mix changes when the results appear, not at the finish line. Restart
  goes `restart_race` → `set_audio_startline`. The replays from the results use Transition
  and PausedReplay instead.
- [O] Row: attackTime 1.0; Exhaust and Engine −4, Turbo, Gearbox and Other Mech −13, Panel
  and World Coll −7, Wheels −10, Skids −7, Displacement −10, Crowd −3, Ambience −48,
  speech groups and AI −4, Sense Of Speed −7; Race Music, Global Ambience, HUD and Music 0.
  So the cars drop by 4 to 13 dB over 1 s, and the ambience goes.
- [O] 411C10 adds CB0BC4 (1.5 s) when the fade being left came from EndOfRace and the new
  mode is StartLine: a restart from the results fades back over 1.5 + 1.5 s.
- Port: `mix_mode` sets EndOfRace while the app's post-race stage is Results or a
  confirmation over it; the stats screen, drift scoreboard and wreck screen stay in the
  previous mode [I: their flow nodes lie outside the EndOfRace state]. Test: the EndOfRace
  part of `crashed_ducks_the_engine_and_returns`. Not heard.

## 14. Kick-ups (F498)

20 October 2026, Claude Code. Static: 4443D0 (Ghidra `local/gtfix/decomp/coll3/44465e.c.txt`),
44F330 (W+16C), docs/TYRE_AUDIO_RESEARCH.md §1–4; data dumped to worktree `local/kick/`
(`mat.tsv` audioMaterials, `iv.tsv` ImpactVarieties, `ci.tsv` CollisionInfos). GRID was not
launched.

**Recipe [O]** (player only, per wheel and frame, after skid/scrub):
- skipped without a kick-up ImpactVariety (W+178, set from the preset on a material change);
- v = |W+150| (car speed); surface = 10·kickupMultiplier of the current AudioMaterial when
  the wheel's raw token (wheel+C8) is non-zero, else 1;
- W+1B8 counts down; below 1 the scale W+1BC = 0.005·(rand % 200) and the counter
  60 + rand % 180 are redrawn;
- p = W+16C · 0.05v · surface · scale, where W+16C is reset to 1 each frame and 44F330 adds
  20·peel² with the wobbled peel inside its active branch (44FD88..44FDA5);
- if rand % 100 < (int)p: x = 0.1·(rand % 10); nothing below the first CollisionInfo's minVel,
  else the first, second or third whose range holds x; 435EA0 with the cooldown flag 0, so only
  the −42 dB gate; group W+84 (Wheels 6), position W+1A0.

**Data [O].** Every preset names `Kickup` (KickupSmall 0.01–9.42 −8..−3 dB, Med, Big) except
`grass` (`GrassKik`, whose first child is GrassKikCrunch 0.01–15.65 −10..−7 dB). With
x ≤ 0.9 only the first info plays, at about −8 dB. Multipliers: TARMAC 0.043, SMOOTHDRYTAR
0.040, GRASS 3, GRAVEL and GRAVELTRAP 6, METAL 0.001; TARMAC_SPARK spells the attribute
`KickupMultiplier`, which the loader does not read. Variety preset `kickup` 10/20 m.

**Consequence.** On gravel at 50 m/s p = 150·scale, so a kick-up starts most frames. On
tarmac p ≈ 1.1·scale, and int(p) is 1 only for the top scales: rare single ticks. Airborne the
last surface's variety still plays with surface factor 1.

**Port.** `tyre_audio`:
- `Bank::kick` (preset ImpactVariety + material multiplier; an absent multiplier is 0 [I]);
- `Driver::kick_up` on the wheel's own draw sequence, once per vehicle tick [I: the original
  runs per frame];
- a sixth per-wheel voice `KICK` that the mixer plays once from its start at the level fixed
  then;
- `bus_levels` puts it on Wheels with the kickup distances.

The player's tyre source is centred (§12), so kick-ups are too. Tests:
`kick_ups_follow_4443d0_and_play_once` and the asset checks in
`source_launch_peels_then_rolls_reproducibly`. Not heard.

## 15. Landing (F502)

20 October 2026, Claude Code. Static: 455FA0 (Ghidra `local/gtfix/decomp/wind3/455FA0.c.txt`), its caller
457BE0 (458C74), 410850, 990FC0, constants D85A50 and CB0DE4; carBot curves in worktree `local/kick/ci.tsv`.
GRID was not launched.

- [O] 457BE0 sets +A5EB = 410850(): airborne when no wheel record has a contact (wheel+C0 == 0
  for every wheel; 990FC0).
- [O] 455FA0, per frame (skipped when the car's +20 → +6C is 3):
  - airborne: when +A4B0 == 0 it restarts the timer +A4B8 and sets +A4B0 = 0.01 (CB0DE4);
    otherwise +A4B0 = the timer's elapsed seconds. With +A614 or +A618 above 0.5 it also
    calls 454560 (not traced);
  - grounded: a positive +A4B0 goes to 4535A0(PrimaryBot, x = +A4B0, position), then
    +A4B0 = 0. The position is the car origin for the player (Car listener) and the pose
    position for AI cars.
- [O] 4559C0 zeroes +A4B0 on side and roof contacts, so a scrape in the air restarts the
  airtime.
- [O] PrimaryBot: carBotSoft 0.175–0.37 s (−4.5..0 dB), Med 0.37–0.57 s (−3..0), Hard 0.57–2 s
  (−3..0); varieties carLand*. Below 0.175 s nothing plays. As a 4535A0 call, a player landing
  after more than 0.57 s also asks for Crashed (§13.1), and the per-variety cooldown applies.
- Port: `collision_audio::airborne` and `Listener::landing` once per vehicle tick (the clock
  advances by the tick, [I] for the original's frame timer); the app's collision tick and
  `grid-opponent-race`'s trace call it after the contacts. Tests: the landing part of
  `shipped_collision_data_and_listener`. Not heard.

## 16. Yaw and pitch displacement (F504)

20 October 2026, Claude Code. Static: 4108B0 (from 457BE0), 4486E0 (Ghidra `local/gtfix/decomp/wind3/4486E0.c.txt`,
called at 458C84 for the player only), A69820; Yaw/Pitch curves in worktree `local/kick/ci.tsv`. GRID was not launched.

- [O] 4108B0, per frame: ω = A69820() = the physical body's +0x210, its world angular velocity.
  +A614 = |ω.x|, +A618 = |ω.z| (455FA0's spinning test). +A610 = |prev.x − ω.x| and
  +A60C = |prev.z − ω.z| against the previous frame's ω (+A2E0), which is then replaced.
- [O] 4486E0: +A60C into the car's ImpactVariety slot 1 (YawImpact) and +A610 into slot 2
  (PitchImpact). For each, when the value exceeds the soft info's minVel, the info whose range
  holds it, else the hard one, goes to 435EA0 with the cooldown on, at +7E0, group +C
  (Displacement 8).
- [O] Curves: Yaw/Pitch Soft 0.25–0.6 (−4..−2.5 dB), Med 0.6–1.5 (−3..−1), Hard 1.5–3 (0 dB).
  Varieties YawSoft…PitchHard in car_collisions.
- Consequence: a jolt that changes the angular velocity by more than 0.25 rad/s between frames
  thumps, for example kerbs, landings and spins. It depends on the frame length, so the port
  runs it per vehicle tick (16 ms, close to 60 fps) [I, F320 rule].
- Port: `collision_audio::angular_velocity` and `Listener::displacement` (the player only, at
  the car). Called by the app's collision tick and by `grid-opponent-race`'s trace. Test: the
  displacement part of `shipped_collision_data_and_listener`. Not heard.

## 17. Part sounds and the car-audio groups (research, not ported)

20 October 2026, Claude Code. Static: 43B1F0 (car-audio constructor), 458EE0..458F1C, 43D850,
43BB10, 43BF50, 9FFE7D..A00123. GRID was not launched.

- [O] Group slots set by 43B1F0 (+0..+18): player 4 Panel Coll, 2 Turbo, 9 Other Mech,
  8 Displacement, 1 Engine, 3 Gearbox, 6 Wheels; AI cars 17 (AI) in every slot. 4486E0 uses
  +C (Displacement), 43BB10 +0 (Panel Coll), 43BF50 +18 (Wheels).
- [O] 458EFC registers 43D850 (data: the car-audio object) at body +7BF4/+7BF8. The damage
  code calls it at 9FFEA0 and A00123 with a part index and the part's record (+0x50 / a
  transformed position). 43D850 forwards to 43BB10(part, record).
- [O] 43BB10: if the per-part CollisionInfo at +97BC + 4·part is set, 435EA0 plays it at x = 1.0
  (so at its curve's level for 1.0) at the part position from record +30 (minus the car
  position +A420 for the player, flag +A4D8), with the cooldown on, group slot +0. Otherwise it
  logs an error with the part index. No static writer of +97BC was found; the table probably
  comes from the per-part ImpactVarieties (CBonnetImpact, CBootImpact, CBumperImpact,
  CDoorImpact, CExhaustImpact, CWingImpact) by a pointer loop [I].
- [O] 43BF50(pos): slot 9 of the car's ImpactVariety table (+24) at x = |car velocity|
  (+A480), no cooldown, group slot +18 (Wheels). Its caller and the slot's variety were not
  traced.
- Next: find the +97BC writer (search for stores relative to a pointer into +97BC, or break on
  it in a capture) and the part order of the damage code, then port part sounds through the
  port's detached-part events.

### 17.1 The 42D580 queue is network replication (F505)

Static: 435B20 (435C5F..435E97), 42D580, 436300, 45DA40..45DB9F. GRID was not launched.

- [O] 435B20 always plays the one-shot locally through 421B70 (bank index from 412E50). After
  that it calls 42D580 only while the audio manager's role word (+24 → +24) is 0 and its
  own argument +30 is 0. 42D580 refuses when the role is non-zero, the group is 16 (HUD), the
  counter +5B4C has reached 15, or the mix mode is Paused. Otherwise it posts a "game
  broadcaster" message (CB31C0) and adds 1 to +5B4C.
- [O] +5B4C is written in only two places: set to 0 in the manager's constructor (45DA46)
  and incremented at 42D627. The counter never goes back down.
- [O] 436300, the "game broadcaster" handler registered at 45DB99, plays a received message
  only when the role is 1, and only then applies its Crashed flag (+24).
- [I] Interpretation: the role is the network side (0 sends, 1 receives) of a replication of
  one-shots to other machines; in an offline race nothing is received.
- Consequence for the port: the cap of 15 and the 436300 Crashed path do not limit or change
  what an offline player hears. Nothing to port. The FIDELITY_BACKLOG note about the cap is
  closed.

### 17.2 Window glass (F511)

Static: 43C0C0, 43C250, 43C2B0, 43CE90, 43CF10..43D5A0 (loader), 448F75, 4486E0, 41DC90,
4109F0, 9E9B03..9E9BF8, 435EA0; `CollisionInfos.bin`. GRID was not launched.

- [O] The car-audio manager's loader (43CF10, called at 459448) names its CollisionInfos and
  ImpactVarieties in order from manager +0x74: YawImpact, PitchImpact, CarScrapeRoof, …,
  then +0xE0 WindScreenCrack, +0xE4 WindScreenSmash, +0xE8 WindowCrack, +0xEC WindowSmash,
  +0xF0 HeadLightCrack, +0xF4 HeadLightSmash, +0xF8 WindSlow, +0xFC WindFast,
  +0x100 ParticulateGlass. A car reads them through [car +0x2C] +0x18 = manager +0x70
  (4486E0 reads YawImpact at +4).
- [O] 43CE90 is registered at 448F75 for the "game broadcaster" messages. It finds the car by
  the message's +4 handle, and then:
  - type 1 (+0x10) calls 43C250(part +8): WindScreenCrack for part 0x51, else WindowCrack;
  - type 2 calls 43C2B0(part): it first sets a per-window flag (4109F0, car +A5D8..+A5DD),
    then plays WindScreenSmash for 0x51, else WindowSmash.

  Both play at x = 1.0, with the cooldown off, on group slot +0 (Panel Coll for the player,
  AI for others).
- [O] The window parts are 0x4E..0x53 (the damage code tests 0x4F..0x53 for state 2 at 9E9B06).
  41DC90 places each one from the car's axes: z·a + x·b + 0.5·y, plus the car position unless
  the car is the player (whose sounds are car-relative). The (a, b) pairs are:

  | Part | (a, b) |
  | --- | --- |
  | 0x4E | (−1, 0) |
  | 0x4F | (−1, −1) |
  | 0x50 | (−1, 1) |
  | 0x51 | (1, 0) |
  | 0x52 | (1, −1) |
  | 0x53 | (1, 1) |

  This is the port's WINDOWS order (b, bl, br, f, fl, fr), so 0x51 is window_f, as
  MECHANICAL_DAMAGE.md already had for 9E99B0.
- [O] All four infos use the cracksmash preset: minVel = maxVel = 1, −7 dB, pitch 0.95–1.1.
  For minVel = maxVel, 435EA0 sets t = 0 while x ≤ maxVel (435EEF), so the windows play at
  −7 dB, pitch 0.95. The port's curve used t = 1 for an empty range and is corrected.
- [I] The sender of the type 1/2 messages was not found. Type 1 is taken as "the window cracked"
  and type 2 as "it smashed", by the info names and by 9D12B0's states 1 and 3. The port
  plays one sound per rise of its window state. A window that goes straight from intact to
  smashed plays only the smash.
- Not ported: HeadLightCrack/Smash and ParticulateGlass (the per-part tables +9F0C and +9B64 of
  43C0C0/44D9D0 have no static writer, like +97BC in §17); WindSlow/WindFast (§7: never
  started).

## 18. Engine banks of every raceable car (F519)

- [O] Before F519, eleven raceable cars' banks failed to load (`grid_tools::engine_audio_continuous::Bank::load`), and grid-app then installed no engine sound for the whole field:
  - `unsupported engine curve extent/range`: dbr (Aston Martin DBR9) and esp (Audi R8). Their idle `RevsFromTime` curves start at 1.0058 and 1.0293; the port required [0, 1].
  - `continuous setup/PCM rate or extent mismatch`: sal, lam, spk, lc7, 390, je4, cha, vip, zon. One idle or throttle recording is one frame shorter than carsetup's frame count (eng_sal throttle: 454,681 PCM frames, carsetup 454,682).
- Reader change:
  - Curve values are accepted in [0, 2] (`engine_audio::CURVE_MAX`), in the INI reader and in the controller, scheduler and pair-restart input checks. [I] The original reads the curves as written; the played pitch is clamped by the voice setter 42C150 to [0.5, 2] (§F516), so a curve slightly above 1 only shifts a lookup.
  - A recording one frame shorter than carsetup is accepted. The controller's length (0x2C0) is the PCM's own, so the cursor stops at the last real sample. [I] One frame at 32 kHz is inaudible either way.
- Isolation (grid-app `engine_sound`):
  - `load_banks` loads one bank per field model; a failure logs `engine_audio version=2 event=bank_failed car=<stem> error="..."` and leaves that model `None`. Only that car is silent: the player's source is spawned only with a bank, and opponents without one are skipped.
  - Each opponent's decoder has its own failure flag. Its mixer goes silent until the next reset and the update logs `engine_audio version=2 event=decoder_failed car=<index> muted=car_only`; only the player's failure still mutes the engine mix.
- Tests: `every_loadable_engine_runs_through_the_start` loads every `series::RACEABLE` car and stk and panics on any load failure; `engine_sound::tests::a_failed_bank_silences_only_its_car` loads lac, a non-existent model and bmw, and checks the spawned sources.

## 19. Opponent mix confirmed offline (F527)

20 October 2026, Claude Code (task 10, item §3.5 of the car-series hand-off). Headless, no audio device;
GRID was not launched. Evidence (ignored): kind-pike worktree `local/mix19/test.log`.

**Test.** `race_start_audio_mix_is_audible` (grid-app main.rs, `race_start_mix`) now adds an opponent
of the same model 3 m beside the player. Its engine voice (`engine_sound::TestVoice`, the same
`EngineSource` decoder as in the app) gets the player's control on every vehicle tick, the AI group
level from the live `AudioMode` (`engine_sound::bus_levels`, group 17, 3 m → 0 dB distance term), and
the pan of a car 3 m to the side heard from the chase_close camera ((0, 1.72, −4.74) in the car; azimuth
32°, `pan_gains`). So the two voices differ only by what the mix gives them.

**Group levels [V against the table].** Chase, PC, both cars:
- last countdown tick (StartLine after its 1.5 s fade): player Engine −14, Exhaust −7; opponent −12 on
  both layers;
- end of the run (Normal): player −14 / −7; opponent −9 / −9.
The test fails if any of these is off by 0.05 dB.

**As heard [O, port].** Opponent against the player's own engine, in dB (power of both channels; left;
right, the near side):

| car | window | power | left | right |
|---|---|---|---|---|
| BMW | countdown, full throttle (StartLine) | −3.03 | −6.36 | −1.17 |
| BMW | after GO + 0.5 s, full throttle (Normal) | +0.28 | −3.04 | +2.14 |
| Lacetti | countdown (StartLine) | −5.82 | −9.14 | −3.96 |
| Lacetti | after GO (Normal) | −1.84 | −5.17 | +0.02 |

The difference between the cars comes from the engine/exhaust balance of each bank: the player's
exhaust layer is 2 dB over the opponent's and its engine layer 5 dB under.

**Distance.** The player's emitters are measured from the camera in the port, the Car listener at the
car in the original (§12). It makes no audible difference: the Engine/Exhaust law (10/175 m, exponent 2)
gives −48·((d−10)/165)² dB, so the EngineBay emitter 4 m ahead of the car seen from chase_far
(about 11.3 m away with the full 2 m tilt fall-back) is at −0.003 dB.

**Conclusion [I].** The port plays the original's table. With it, a car of the same model beside
the player is about as loud as the player's own car after the start, and up to 2 dB louder in the
near ear; on the grid it is 3–6 dB quieter. The owner's "an opponent sometimes sounds louder than
my car" is the table, not a port error. Not checked against the original's output (that needs a
live reading, §11).

## 20. Gear shifts in 451770 (F528)

20 October 2026, Claude Code (task 10, item §3.3, first part). Static: Ghidra 12.1.4 headless
(`local/gtfix/decomp/gear1`–`gear3`: 451770 and its helpers), Capstone 5.0.6 on
`local/compatibility/grid-unpacked.exe` (0x410000–0x460000). GRID was not launched. Render evidence
(ignored): kind-pike worktree `local/shift/`.

**Layout [O].** The engine-audio controller is the car-audio object +0x8660 (42F770: `lea ecx,
[esi+0x8660]`); its property pointer +0x118 is the car-audio object +0x1190. The `[Physics]` loader
423BD6 reads the keys in file order through 416040, a case-insensitive prefix match (`_stricmp`).
So the 11 files that spell `fGearupShiftTime` (390, 787, b05, b06, c65, cre, df3, fmj, je4, lc7,
r10; value 0.03) are read. Fields used by the shift code:

| key | controller | property |
|---|---|---|
| fGearUpShiftTime | +0x80 | |
| GearUpMixTime, GearUpMixMinIdle | +0x84, +0x88 | |
| fUpshiftEffectFadeIn/Out | +0x8C, +0x90 | |
| fGearDownShiftTime, fGearDownEffectTime | +0x94, +0x98 | |
| fGearDownMinLoad, fGearDownMinThrottle | +0x9C, +0xA0 | |
| GearUpMixVolMult, GearUpWobbleLevel, FadeOutTime, Rate | | +0x7D1C, +0x7D20, +0x7D24, +0x7D28 |
| fGearDownWobbleLoudness, Rate, Length | | +0x7D34, +0x7D38, +0x7D3C |
| fUpWobbleStartDelay, fDownWobbleStartDelay | | +0x7D40, +0x7D44 |
| fWobbleRandomDown, fWobbleRandomUp | | +0x7D48, +0x7D4C |

Property +0x7D18 (car-audio +0x8EA8) is the "random" of the wobble. 42F770 sets it to 1.0 and no
other instruction in the image writes it, so (2r − 1) = 1 and the random terms are fixed: ×1.75
with the shipped 75 %.

**451770 order [O].** 422A40 (RPM map, C8 = +0x48); +0xC and +8 += |dt|; 414750; 4148A0; 414970;
the upshift-mix envelope (+0x10, +0xB8); 451580; 422CA0; 4150D0; 414C70; 436640; 451260; 422F60;
4152A0; 4371C0.

**414750, the gear change [O].** Gear = vehicle +0x196C (0 neutral, 1.. forward, 9 reverse). Every
frame +0 (the scale of the physical RPM ratio in 422A40) recovers by 0.003, up to 1. On a change:
- +0x314 (wobble loudness) = fGearDownWobbleLoudness · clamp(1 − gear/6, 0, 1). Until the first
  change it is GearUpWobbleLevel (constructor);
- up (new > old): +0 = (rand % 1000) · 0.00005 + 0.95, an RPM dip of up to 5 %; down: +0 = 1;
- the shift counts unless it is between two of {neutral, first, reverse}. Then +4 = up, +0xC = 0
  (time since the shift), and for the player (+0x110 +0x6C = 0) +0x310 = 1.

**451580 [O].** +0x1A0 = clamp(0.9 · +0x1A0, −1, 1). During an upshift, for fGearUpShiftTime, 0.5 is
then subtracted (down to −1.5); during a downshift, for fGearDownShiftTime, 0.1 is added. 42DE60
adds it to the load crossfade phase ((2·throttle − 1) + offset + 1)/2, clamped. So an upshift
crossfades to the off-load recordings for 0.13 s (Lacetti), and a downshift leans on-load.
- The RPM blend of +0x48 with the held +0x58 (t² up, t down) only reaches 414C70/451260, not the
  pitch: C8 was published before it.
- +0x19C is only decayed and never read.
- At the end of a player upshift, 4511E0 plays the car-audio variety table entry +0xB4 at the +0x100
  emitter: the gear clunk. Not ported; it needs the mec bank.

**4150D0, the wobble [O].** +0x2F8 multiplies every voice pitch in 42DE60. With t = +0xC:
- up: after fUpWobbleStartDelay, for t' < GearUpWobbleFadeOutTime:
  1 + sin(1.75 · GearUpWobbleRate · t' · 2π) · (1 − t'/FadeOut) · +0x314;
- down: after fDownWobbleStartDelay, for t' < fGearDownWobbleLength:
  1 + sin(fGearDownWobbleRate · t' · 2π) · 1.75 · (1 − t'/Length) · +0x314.

BF4E44 is the CRT SSE2 sine (a tiny argument returns itself). +0x2F4 (GearUpMixVolMult during
GearUpMixTime) is written and never read. GearUpWobbleLevel only seeds +0x314.

**Port.**
- `grid_tools::engine_audio_shift` (`Settings::read`, `Shift::frame`) runs 414750, 451580's +0x1A0
  and 4150D0 after `engine_audio_control::update` in the continuous mixer. It writes +0, +0x1A0 and
  +0x2F8 for the publication.
- `Control` carries the gear (bits 25–28). The 42DE60 domain check allows |+0x1A0| ≤ 1.5.
- [I]: the dip's random sequence (xorshift instead of A682A0); the constructor state again after a
  reset (so a start counts neutral → gear); one frame per 16 ms block.

**Not ported.**
- the upshift-mix envelope +0xB8 (it caps 414C70's load +0x44, which the port does not compute);
- the downshift throttle blip in +0xB0 (4148A0/422CA0; the port keeps +0xB0 at 1);
- the rev limiter (414970, +0xB4);
- the throttle smoothing and +0x330 of 422CA0;
- the clunk (4511E0) and the backfire (451260);
- all of 414C70, 436640 and 422F60.

**Render [O, port].** Test `an_upshift_changes_the_lacetti_only_from_the_shift`: Lacetti, full
throttle, second gear at 0.7 of maximum RPM for 1 s, then third at 0.55, against a held second.
- The samples are equal up to the shift.
- Afterwards the RMS falls from 0.133 to 0.075 for 50–150 ms (the off-load crossfade).
- The dominant frequency moves 195 → 160 Hz, with a swing of 155–166 Hz between about 170 and
  300 ms (the wobble: 8.75 Hz, ±10 % in third).
- `local/shift/lacetti-{held,upshift}.wav` (sha256 5b7ad9ca…7704, ff14eac2…0b02).

Not compared with the original.

## 21. Car listener, clunk, backfire and whine wiring (research, F529)

20 October 2026, Claude Code (task 10, rest of §3.3 and input for §3.4). Static only: Ghidra
(`local/gtfix/decomp/gear4`, `gear5`, `lis1`), Capstone, decoded `audioVarieties` and
`carsetup.bin` (`local/gear/carsetup.tsv`). GRID was not launched. Nothing ported here.

**Car listener 43F7A0 [O unless marked].** It runs for the player vehicle (manager +0x34D88 → +0x24).
- Inputs:
  - the car pose (body +0x1D0) and velocity (body +0x220);
  - the camera rows and position (written by 412100, one call behind);
  - the camera displacement per frame (kept only below 8 m, CB0C44, as a teleport guard).
- With d = camera − car, the published position is car-relative and never the absolute camera:
  - views [+0x1724] 3/4, with +0x34D60 clear and [+0x16F8] ≠ 6: 0.25·d with the camera's
    orientation;
  - view 2: t = clamp((θ − π/6)·3/π, 0, 1), with θ the angle between the camera and car forward
    vectors. Orientation = slerp(car, camera, t). Position = virt + t·(d − virt), where virt is
    the point at d's height and horizontal distance straight behind the car;
  - otherwise: d with the camera's orientation.
- Velocity: the camera's displacement / dt (views 2–4), else the car's velocity. There is no
  smoothing state.
- [I] The value of +0x1724 is a view index whose mapping to chase, bonnet, bumper and head-cam is
  not tied down. 3/4 look like the interior views, 2 the chase.
- [I] The car's own emitters are then placed relative to the car origin.
- The listener 2 sibling 43F570 uses the camera position, plus 0.25·d in views 3/4.

Consequences for the port [I]:
- At the default branch the port's "player emitters measured from the camera" are the same
  distances.
- In view 2 the distance from the car is kept (|virt| = |d|), and the orientation follows the car
  until the camera is turned more than 30° from it. The port pans the player's sounds by the
  camera's orientation (§12), which matches only while t = 1.

**Clunk (GearClucks, 4511E0).**
- [O] At the end of a player upshift, GearClucks plays clunk1–3 at the GearBox emitter. The
  emitter is (0, 0, 4) in every one of the 45 cars, on group +0x120 (Gearbox 3 for the player).
  The preset GearStick is min 3.5 / max 4.0 m.
- [I] Heard from the Car listener:
  - chase views: d is the camera offset (5–7 m behind), so the gearbox is 9–11 m away: silent;
  - views 3/4 at 0.25·d: bumper (0, 0.73, 1.77) gives 3.56 m, −0.7 dB; bonnet (0, 1.11, 0.86)
    gives 3.8 m, −17 dB; head-cam (0.36, 1.10, −0.19) gives 4.06 m, silent.
  So the shipped clunk is an interior/bumper-view sound, which depends on the unresolved view
  mapping.

**Backfire (Detonations, 451260).** [O] detloop1–3 at the Exhaust emitter, preset ExhaustDet
10/40 m. It triggers when any of these holds:
- the throttle drop per frame +0x304 ≤ −0.25 while +0x20 > 0.25;
- within 0.2 s after an upshift;
- the rev limiter is active (+0xD0).

It then repeats on a random timer, (rand % 300)·0.01 + 0.1 s, with a chance that falls as the
timer runs over. Each start also sets vehicle +0x21DC = 8, and 0xB at the end [I: exhaust flames].
Its inputs need 4148A0 (+0x304 from the applied throttle, vehicle +0xE1C) and 414970.

**Load path inputs [O].** 009961D0(1) is vehicle +0xE1C, the applied throttle. It is the input of
4148A0 (+0xB0, +0x24, +0x304) and 422CA0 (+0xCC smoothing). 422CA0's neutral-gear and off-throttle
targets read +0x3C, which 414C70 writes, so a faithful +0xCC needs 414C70 ported too.

**Whine.**
- [O] carsetup.bin `GearPresets`, read by 4265D0 into +0x268 + 4·preset:
  - holden (mec_ply), truck (mec_dbr), andy (mec_r10), staff (mec_s15);
  - each has Gear1..6 (truck 8) with a sample (01_gear…), basePitch and maxPitch;
  - each car names a `GearPreset`.
- [O] gear.ini `[GearWhine]` (42F880) stores six floats at car-audio +0xD10…+0xD3C. The five
  `GearWhineVolume*` keys are read and discarded. The `Gearwhine` AudioVariety has no samples
  (preset 6/10 m).
- [O] No float read of +0xD10…+0xD3C was found in 0x410000–0x460000.
- Whether the whine plays at all is not established. Next: the readers of +0x268 (the preset
  table) and of the variety table entry +0xB0.

**Turbo.**
- [O] carsetup `Turbo` Spidle (basePitch 0.75, gradient 1.5) and Hiss (1.0, 0.0) are identical for
  all 45 cars; readers 41EC91…41ED1B and 4273A1…42742B.
- [O] The varieties are TurboHiss and TurboSpindle (loops, 6/20 m), TurboDump (dump1–2, 6/60 m)
  and TurboPip (pip1–2, 5/20 m).
- The update that plays them is not traced.

## 22. Gear clunk ported; car mec banks; the whine plays (F572)

26 October 2026, Claude Code (car-series, owner task "port the engine sounds of F529", item 1;
findings for items 3 and 4). Static: `tools/re/grid_re.py` dis/decompile (cache
`local/re-cache`), Capstone displacement scans; offline renders. GRID was not launched.

**View mapping of 43F7A0 [O].** 4139F0 stores the audio camera at group-mix +0xC1C and reads the
mix mode at +0xBF0; the group mix sits at manager +0xB08, so 43F7A0's [+0x1724] is the audio
camera (§13: Internal 0, Bonnet 1, Chase 2, Detached 3, DetachedMultiCar 4) and [+0x16F8] the mix
mode (6 = EndOfRace). 454C39 forces camera 3 while +0x34D60 is set. So:
- Chase: the slerp branch (§21);
- Detached/DetachedMultiCar (trackside), not EndOfRace and +0x34D60 clear: 0.25·d;
- Internal and Bonnet (head-cam, bumper, bonnet), and EndOfRace: d, the camera relative to the car.

This corrects §21's [I] reading of views 3/4 as interior views. For the player's mounted and head
views the original's emitter distances equal the port's camera-to-emitter distances.

**Clunk audibility [O from data].** GearBox (0, 0, 4), GearStick 3.5/4 m, curve GearClucks 0 dB /
pitch 1 at x = 1: Lacetti bumper (0, 0.73, 1.77) 2.35 m → 0 dB; bonnet (0, 1.11, 0.86) 3.33 m →
0 dB; head-cam (0.36, 1.10, −0.19) 4.35 m → silent; chase 9–11 m → silent. The level is the
Gearbox group (Normal/Bonnet/PC −18 dB in `GroupModeVolumes`).

**Trigger [O].** 451580 (after the 451580 load offset): while +4 (up) is set and +0xC ≥
fGearUpShiftTime (+0x80), if +0x310 is set it calls 4511E0 and clears +0x310. 414750 sets +0x310
on every counted change of the player. So one clunk per counted upshift, on the first frame past
fGearUpShiftTime (Lacetti 0.13 s: the 9th frame).

**Which bank [O].** 4511E0 passes `[[car-audio +0x128] +0x2C] +0x14` as 44F010's last argument;
44F010 hands it to 435940 as the bank, and 435940 uses the variety's own bank (412E50 on the
template `mec_dbr`) only when it is −1. The +0x2C object is the car record built by 41E1A0 in the
carsetup loader 41E2E0:
- +0x10 engine bank, +0x14 mech bank: the backend's id (vtable +0x38, 41E747/41E785) of the car's
  `EngineBank`/`MechBank` names ("bad Engine/Mech bank" messages 41E7CE, 41E817);
- +0x18 the variety table (manager +0x70);
- +0x20 the car's GearPreset (410C00 finds it by name, 41E684).

So every car plays its own `mec_*` bank. The 45 banks differ: the GearClucks samples come in at
least eight sets (lengths 7270–20020 frames at 40 kHz, `local/mech/test1.log`). All banks hold
the same 22 sample names, so the template's names resolve in each.

**Port.**
- `grid_tools::mech_audio`: the six CollisionInfos with their varieties' samples (resolved by name
  in the car's mec bank) and preset distances; `Voices` plays one-shots (44F010, one handle per
  info) at curve level + group + A23B70 distance, pitch from the curve; 413B80 pick without repeat
  [I: RNG].
- `engine_audio_shift::Effects.clunk`: the 451580 trigger above. The continuous mixer starts
  GearClucks at x = 1 when it fires and the mixer has mechanical voices.
- grid-app: only the player's decoder gets voices (`EngineBank::mech`, loaded from its carsetup
  MechBank). Levels per frame: Gearbox group (mode, audio camera, Effects Volume trim) and the
  GearBox emitter's distance from the listener. Mono, centred like the engine (§12) [I: the
  original positions it at the gearbox].
- Fix: grid-app's decoder copied only rpm and throttle from each tick's control, not the gear, so
  F528's shift effects never played in the game (the offline tests drove the mixer directly). The
  decoder test now checks the gear.

**Render [port, offline].** `GRID_ENGINE_AUDIO_TRACE` (new in grid-opponent-race, output
unchanged) records car 0's engine-audio control per tick; the probe `grid-mech-audio` replays it
through the mixer at the Lacetti's bumper view (Normal mode, Bonnet camera: engine −8 dB, exhaust
−14 dB, Gearbox −18 dB). Solo Lacetti, Donington route_0, 5600 ticks (lap 86.8 s): 20 counted
upshifts, each with a clunk 9 blocks after the change, about 0.27 s long, difference RMS
0.034–0.038 against an engine RMS of 0.08–0.14 in the same window (`local/mech/render-01/`).
The AI lap stays below 0.93 of the maximum RPM, so it never reaches the limiter.

**The whine plays [O].** This answers §21's open question:
- 457BE0 calls 4518E0 for every car (player and AI) after 451770, on the object at car-audio
  +0x84C.
- 4518E0 first starts the voice +0x850 (437A20: preset Gear1 sample from bank record +0x14), then
  each frame:
  - 416820: wheel speeds → speed (+0x1C, smoothed 0.1), mph (+0x20, 0..200); −0.125 on +0x54 per
    wheel off the ground;
  - 4169A0: two 128-entry tables of the object [+0xC] (+0x104, +0x304) over its period +0x100 →
    +0x44/+0x48;
  - 4378A0: on a gear change 4377F0 restarts the voice with the preset entry (gear − 1)·12 + 8;
    after an upshift a decaying sine on the pitch (+0x50) from [+0xC] +0x520…+0x534;
  - 4166E0: +0x4C from [+0xC] +0x514/+0x538/+0x53C and the damage term vehicle +0x185C;
  - below 10 mph +0x54 scales down; then 44BAD0.
- 44BAD0: neutral → −48 dB. Otherwise pitch = lerp(preset basePitch, maxPitch) over the wheel
  speed between the gear's ratios (vehicle +0x191C/+0x1944/+0x18F0/+0x18F4/+0x1918), times +0x50
  and a load term; volume = lin2db(+0x54 · load term · the Gearwhine CollisionInfo over mph 5–50)
  + vehicle +0x185C − 1 + record +0xC8 (the car's gear VolumeTrim); Min/MaxDistance from the
  Gearwhine preset (6/10 m).
- So the earlier scan was wrong: the variety table entry +0xB0 is read (44BBxx), through a register
  the scan did not follow. gear.ini `[GearWhine]` at car-audio +0xD10… is still unread; the
  parameters 44BAD0 uses live in the [+0xC] object, whose loader is not traced yet.
- Not ported yet: it needs the [+0xC] loader, the wheel and gear-ratio inputs and the AI group.

**Turbo [O, static].** 457BE0 calls 44D840 only while car-audio +0xA4D8 is set: 43B1F0's sixth
argument, `[car +0x6C] == 0`, the player. 44D840 runs the turbo model 416BC0 on the object at
car-audio +0x800, then one-shots TurboDump (+0x819) and TurboPip (+0x818) and the loops Spindle
(44D560, voice +0xA39C) and Hiss (44D6D0, +0xA3A0), all at the EngineBay emitter. Port pending.

## 23. Backfire, turbo and the load path ported (F575)

26 October 2026, Claude Code (car-series, owner task items 2 and 4). Static (`tools/re/grid_re.py`,
Capstone on the unpacked image) and offline renders; GRID was not launched.

**Correction of §20–§22 [O].** 43C6D7 calls the [GearWhine]/[Envelope] loader 4376B0 → 42F880
with `this` = car-audio +0x8B0, and 43C6F1 stores car-audio +0x8F10 at +0xA494. So the engine
controller is car-audio +0x8F10 (+0x8B0 + 0x8660), and the "car-audio" offsets of §20–§22 that came
from 42F770/42F880 (+0x8660, +0x1190, +0xD10…) are relative to +0x8B0. The [GearWhine] floats sit
at car-audio +0x15C0…, which is the whine object's [+0xC] (+0x10B0, 416A70) +0x510…: the whine
reads them (4166E0, 4378A0, 4518E0). 423600 loads the whine's `[Envelope]` (Length +0x100,
Volume +0x104, Pitch +0x304, 128 entries each) into +0x10B0.

**4148A0 [O].** With a = 9961D0(1), the applied throttle (vehicle +0xE1C): +0xB0 = a; +0x304 =
a − +0x308; +0x308 = a; +0x24 = (1 − dt)·+0x24 + a·dt. When +0x24 > 0.55 and a < 0.1: +0x28 =
max(+0x28, +0x6C · clamp((+0x24 − 0.55)·2.857) · 0.1) and +0x24 = a. +0x6C is 1.0 (reset 422470,
4224CF).

**The burble in 422A40 [O].** After the smoothing: if +0x28 > 0, b = sin(+0x2C · 125.68) · +0x28
(BF4E44 on the float product), the RPM before the idle remap gains b, +0xBC = b (b − (+0x48 − 1)
when the sum passes 1, which is then clamped to 1), and +0x28 −= 0.25·dt, clamped to 0…999.
451770 adds dt to +0x2C after 422A40. So a lift from full throttle wobbles the RPM at 20 Hz by up
to ±0.1, fading over 0.4 s.

**414970, the limiter [O].** +0x324 += dt. Below +0x38 = 0.99, or in fifth gear and up: +0x64,
+0xD0, +0xB4 = 0. Otherwise +0xD0 = 1, +0xB4 ≤ 0.63, a counter +0xD4 toggles +0x64 every second
frame, and +0xB4 = +0x64 ? +0xB4 + 0.05 : 0; 422A40 subtracts +0xB4 from the remapped RPM. So the
limiter cuts 0, 0.05, 0.1, 0 every four frames. It also drives the exhaust-flame state vehicle
+0x21DC (8 on, 0xB after 0.5 s; not ported).

**451260, the backfire [O].** Player groups (422547): +0x11C Engine, +0x120 Gearbox, +0x124
Exhaust; opponents 17 (AI) for all three. 411120 sets +0x334 on every car but the focused one, so
the focused car plays at x = 1 (−11 dB, pitch 1.1) and the others at x = 0 (−30 dB, pitch 1).
- Start, when the handle +0x21C is free: +0x304 < −0.25 with +0x20 > 0.25, or +0xC < 0.2 after an
  upshift, or the limiter (+0xD0). Detonations (detloop1–3, ExhaustDet 10/40 m) at the Exhaust
  emitter; despite the names the samples carry no loop flag (only turbo_hiss and turbo_spindle
  do), so each start is one pop; active +0x318; timer +0x31C = (rand % 300)·0.01 + 0.1 s.
- While active: past 0.2 s from a shift, a throttle rise of more than 0.25 in a frame, +0x20 ≤
  0.55 or a shift 0.2–0.4 s old stop it (A23350). Else the timer runs down; at 0 the voice stops
  and +0x320 restarts it, timer (rand % 300)·0.01, once the handle is free.
- +0x20 is +0x48 after 414C70 (with a fall limit while vehicle +0x1971 is set).

**The turbo [O].** 44D840 runs only while car-audio +0xA4D8 (the player). Inputs: RPM controller
+0xC8, load +0xB0, and on the frame of a counted shift (+0xC = 0) state 1 (up) or 2 (down); vehicle
+0x1860 zeroes RPM and load. 416BC0 on car-audio +0x800 (constants checked in the disassembly):
- +0x30 demand += (min(2·rpm, 1)·load − demand)·16·dt; a = 0.66·demand, below 0.33 ×5 capped at
  0.33, capped at 0.33 below half RPM; a kick +0x38 (×0.97 a frame, added to a; +0.05 up to 0.75
  only when the load jumps by more than 0.5 from below 0.5);
- flutter phase +0x34 += rpm⁵·load; boost +0x1C = (sin(phase)·0.04·rpm² + a)·4 (published to the
  dashboard);
- upshift: boost ×0.8, level +0x14 ×0.75, Spindle +0 ×0.75, TurboPip (+0x818), demand ×0.05;
  downshift with demand > 0.5, or a throttle drop over 0.25 in a frame with demand > 0.5 and no
  pip: level ×0.75, Spindle ×0.25, boost ×0.6, demand ×0.05, TurboDump (+0x819);
- level += dt·d with d = demand − level, doubled when not positive; Hiss level +0xC = level; Hiss
  pitch +0x10 = +0x48·rpm + +0x44; slow +8 += (demand − slow)·0.5·dt; Spindle pitch +4 =
  +0x40·slow + +0x3C; Spindle level +0 += (load − +0)·dt.
- 43B285 copies the car record's +0x70…+0x7C (carsetup Spidle 0.75/1.5, Hiss 1.0/0.0 in all cars)
  to +0x83C…+0x848.
- 44D840 then: a misfire zeroes +0x830 when rand % 1000 < 100·damage² (engine damage 99A4D0(1));
  TurboDump/TurboPip one-shots at x = 1 in group +0xA3C4 (= +0xA4D8 ≠ 0: Engine) at the EngineBay
  emitter; Spindle 44D560 and Hiss 44D6D0 loops (voices +0xA39C/+0xA3A0) at lin2db(level) + the
  CollisionInfo's maximum volume (−12 / −15 dB), pitch +4 / +0x10, TurboLoops 6/20 m, group
  car-audio +4 (Turbo for the player, 43B8E0).

**Port.**
- `grid_tools::engine_audio_load`: `throttle` (4148A0) and `limiter` (414970) on the controller
  bytes; `engine_audio_control::update` plays the burble (the old "+0x28 = 0" domain check is
  gone).
- `Control::throttle` is now the applied throttle (`Control::applied_throttle`, vehicle +0xE1C) for
  the player and the opponents; in the AI lap it equals the driver's throttle at every tick.
- `mech_audio::Backfire` (451260) and `mech_audio::Turbo` (416BC0 and the rest of 44D840) in the
  player's mixer; `Voices` gained loops (the sample's loop flag), caller-set level and pitch
  (435690/444280) and stop (A23350). `Levels` carries four groups.
- Not ported: the exhaust flames (+0x21DC), the misfire (no engine damage reaches the audio),
  vehicle +0x1860, opponents' backfires (they have no mechanical voices), 422CA0's throttle
  smoothing and downshift blip, 414C70.

## 24. The gear whine ported (F578)

26 October 2026, Claude Code (car-series, owner task item 3). Static (`tools/re/grid_re.py`
decompile/dis, cache `local/re-cache`) and offline renders; GRID was not launched.

**It plays [O].** §22 established that 457BE0 calls 4518E0 for every car. The rest of the chain,
read in full here:
- 457BE0 passes car-audio +0x7D0 as the position. 451770 (`Effects`) copies the same vector to
  the controller's +0x100, which the clunk (4511E0, `lea edx, [ecx+0x100]`) also uses. So the whine
  sits at the GearBox emitter, (0, 0, 4) in every car. +0xA480 (the velocity) goes to 42C220.
- Group: the voice's +0x18 (416A70): Gearbox (3) for the player, AI (0x11) for the others.
- Distances: the Gearwhine variety's preset, 6/10 m (44BAD0 reads them through the variety table
  +0xB0 → +0x28 → +0x1E0).

**4518E0, per frame [O].** The object is car-audio +0x84C, its block [+0xC] car-audio +0x10B0.
- 437A20, first frame only: starts the voice +0x850 with GearPreset entry 0 (`[record +0x20] +8`,
  bank record +0x14), clears the envelope position +0x50C, the gear +0x504 and the lift timer
  +0x540.
- +0x54 = 1, then 416820:
  - the mean over the driven wheels (+0x18D8, count +0x18F0) of 990FB0·990DB0, i.e. wheel
    +0xE0·+0x104, is smoothed into +0x24 (0.1 a frame);
  - +0x1C follows the old +0x24, also at 0.1 a frame (m/s);
  - +0x20 follows the old +0x1C × 2.2369363 at 0.1 a frame (mph), clamped to 0…200, and is 0
    while vehicle +0x1FA0 (fuel) is 0;
  - each driven wheel without contact (990FD0: wheel +0xC0 = 0) takes 0.125 off +0x54.
- +0x50C += (+0x20 / MPHForNormalEnvelopeRate)·dt, less the envelope Length once it reaches it.
  4169A0 reads Volume and Pitch (128 entries each, linear between neighbours, wrapping) at
  +0x50C / Length into +0x44 and +0x48.
- 4378A0, with g = vehicle +0x196C and t = vehicle +0x1974:
  - +0x50 = 1; in neutral, +0x54 = 0 and nothing else;
  - on a change of gear: +0x508 = (g > +0x504 and g ≠ 9), +0x504 = g, and 4377F0 restarts the
    voice with entry g − 1 (reverse: entry 0);
  - if t ≤ StartDelay or the change was not an upshift, a new random +0x520 = (rand & 0xFF)/256;
  - else, with u = t − StartDelay and L = Length·g: while u < L, +0x50 += sin(2π·Rate·u) ·
    ((2·+0x520 − 1)·Randomness·0.01 + 1) · (1 − u/L) · Loudness.
- 4166E0, with p = vehicle +0x185C (the drivetrain's throttle):
  - below 0.5 (negative counts as 0) the lift timer +0x540 += dt; at 0.5 or more after a lift,
    +0x544 = (rand & 0xFF)·0.0017647 + 0.05 s and +0x540 = 0;
  - +0x4C = (1 − s·o·(1 − p))·MaxLowLoadEffectMultiple + (1 − MaxLowLoadEffectMultiple), with
    o = clamp((+0x540 − +0x544)·100) and s = clamp((FadedOut − mph)/(FadedOut − FullVolume)).
- Below 10 mph, +0x54 ×= mph·0.1. Then 44BAD0.

**44BAD0 [O].** In neutral the voice's volume parameter is set to −48 dB and nothing else. Else,
with g (reverse as first):
- the shift speeds: up = +0x191C[g], down = +0x1944[g]; an up speed above 90 (the top gear's
  10 000) is replaced by +0x1918[g]·(+0x18F4[g] / +0x18F0[g]), the previous gear's up speed scaled
  by the ratios;
- t = (+0x1C − down)/(up − down); base pitch = lerp(entry basePitch, maxPitch, t), unclamped;
- volume factor v = 0.5·+0x44 + 0.5 and pitch factor q = 0.1·+0x48 + 0.9, each pulled towards 1
  by +0x4C (exactly 1 at +0x4C ≥ 1); above the up speed v −= t − 1;
- 413130 over the Gearwhine CollisionInfo (minVel 5, maxVel 50, minVol 1.0, maxVol 0.6): below
  +0x1C = 5 m/s, lerp(0, 1.0, +0x1C/5); above, lerp(1.0, 1.0, …) = 1.0. maxVol 0.6 is never
  read, so the intended fade over 5–50 m/s does not happen (an original data/code mismatch,
  ported as is);
- level = lin2db(+0x54·v·factor) + (vehicle +0x185C − 1) + car record +0xC8 (carsetup VolumeTrims
  `gear`, −1 dB in the Lacetti); pitch = +0x50·q·base pitch.

**Parameters [O].**
- carsetup `GearPresets`: four presets (holden, truck, andy, staff), Gear1…6 (truck 8), each with
  basePitch, maxPitch and the sample `0N_gear`; the car names one (`GearPreset`, record +0x20).
  The Lacetti uses holden: 0.75–1.12, 0.85–1.023, 0.95–1.023, 0.9–1.05, 0.9–1.123, 1.0–1.2.
- gear.ini (one file content shared by all 45 cars): `[GearWhine]` EnvelopeFadedOutSpeed 80,
  EnvelopeFullVolumeSpeed 40, MaxLowLoadEffectMultiple 1, MPHForNormalEnvelopeRate 40;
  `[Envelope]` Length 7 s with 128-entry Volume (0.16–1.0) and Pitch (0.96–1.14). FreqAt200mph/B,
  MaxBalance and VolumeSpeed are loaded but 4518E0 does not read them.
- engine.ini `[Physics]` GearWhine = 3 in all 45 cars, so 42F770 reads fUpshiftWhineLength 0.01
  (one car 0.4), Loudness 0.075, Randomness 75, StartDelay 0.01 (one car 0.1), Rate 10.

**Port.**
- `grid_formats::audio_car_setup`: the car's GearPreset entries (`Setup::gears`).
- `grid_tools::engine_audio_whine`: `Settings` (preset, gear.ini, engine.ini, CollisionInfo,
  trim), `Input::vehicle` (the vehicle words above), `Whine::frame` (4518E0 with 437A20, 416820,
  4169A0, 4378A0, 4166E0 and 44BAD0) giving the entry, level and pitch. `Voice::pack` carries it in
  one word, with a start counter so a repeated or dropped tick restarts the sample once per gear.
  rand is an xorshift [I: the CRT sequence is not reproduced].
- `mech_audio`: the bank also loads `01_gear`…`08_gear` as the `Gearwhine` info (6/10 m, Gearbox
  group, GearBox emitter); `Voices::whine` restarts on a new start or entry and otherwise only sets
  level and pitch. [O] Every `01_gear`…`08_gear` in the 45 banks carries the WIP loop flag, so the whine loops
  for as long as the voice lives; in the Lacetti's bank (`mec_lac`, the same samples as `mec_ply`) gears
  1–6 are 1.0–1.7 s loops at 40 kHz and gears 7–8 are 116-frame stubs.
- grid-app: the player's whine runs once per vehicle tick on the main thread (dt 0.016 s, reset
  per segment) and rides the engine mailbox with the tick's control; the decoder hands it to the
  mixer. The race tool's `GRID_ENGINE_AUDIO_TRACE` gains a `whine` column (the packed word).
- [I] The original runs 4518E0 per rendered frame, so its 0.1-per-frame smoothing depends on the
  frame rate; the port runs it per 16 ms tick like the engine controller.
- Not ported: opponents' whines (AI group, x = 0; no mechanical voices for opponents yet), the
  velocity 42C220 gives the voice, the 0 mph frequencies.

**Render [port, offline].** Solo Lacetti, Donington, the same lap as §23 (race output and the
first five trace columns are identical): the `whine` column holds a voice on every tick, 16
restarts (one per gear change, counter mod 16). Under the AI's throttle (0.8–0.96) the voice level
is −1.0 to −4 dB (mean −1.2 to −1.8 dB per gear; gear trim −1 dB), with −50 dB at the start
(0 mph). Pitch spans per gear: 1st 0.75–1.10, 2nd 0.84–1.06, 3rd 0.95–1.04, 4th 0.91–1.07,
5th 0.93–1.12. At the bumper view (GearBox 2.35 m, 0 dB distance term; Gearbox group −18 dB) the
whine adds an RMS of 0.027–0.032 against the engine's 0.12–0.15 (`local/mech/render-03/whine.wav`
minus `clunk.wav`). The other variants are bit-identical to `render-02`. The authored 4 s tail
has no whine, since it has no vehicle. Not compared with the original by ear or capture.

## 25. Opponents' mechanical sounds; the voice pool (F582)

26 October 2026, Claude Code (car-series, owner task "the remaining missing sounds", item 1). Static
(`tools/re/grid_re.py`) and offline renders; GRID was not launched.

**Which mechanical sounds other cars play [O].**
- Clunk: player only. 414750 sets +0x310 only when the car's +0x6C is 0 (41487F: `cmp [edx+0x6c],
  0; jne`), and 451580 plays GearClucks only while +0x310 is set.
- Turbo: player only (44D840 runs while car-audio +0xA4D8, §22).
- Backfire: every car. 451770 runs 451260 for each car's controller; 411120 sets +0x334 on every
  car but the focused one, so other cars play Detonations at x = 0 (−30 dB, pitch 1.0), ExhaustDet
  10/40 m, on their +0x124 group (AI 0x11).
- Whine: every car (§22, §24); 416A70 gives other cars group 0x11 and voice priority +8 = 0.

**The voice pool [O].** 43EC80 sets the backend's voice count from `system/hardware_settings_config.xml`
`<audio_card><audio mixing=… voices=…>`: software mixing (the shipped setting, `voices="120"`)
uses 120, otherwise `voices` (default 50, capped by the card). The sfx_quality options set 119, 120
and 121. The VoiceManager update A32B70 binds waiting voices to free channels after sorting them
(A32DE0, a shell sort on A23530); with no channel free, A31E30 steals the first playing voice
that the waiting one outranks (A23530: higher priority +0x40, then +0x70), else the voice is
dropped ("can't find anything less important"). The priority is StartVoice's argument (435690's
fourth, the whine's +8: 0 for other cars). No variety or CollisionInfo carries a priority.
- [I] So other cars' whines are the first voices stolen once 120 play. The port has no voice
  pool for any sound (engines, tyres, collisions, mechanical); with 12 cars it plays everything.

**Port.**
- `engine_audio_continuous::Mixer::player`: the clunk and the turbo only for the player; the
  backfire at x = 1 for the player and x = 0 for others.
- grid-app loads every field model's mec bank and whine settings; each opponent's decoder has its
  mechanical voices; every car's whine runs per vehicle tick on the main thread (`CarWhine`); each
  opponent's levels are the AI group (with the Effects trim) at its own GearBox, Exhaust and
  EngineBay emitters, measured from the camera like its engine.
- Probe `grid-mech-audio`: variants `opponent-base` and `opponent`, the traced Lacetti as an
  opponent 3 m from every emitter (AI group, Bonnet camera −8 dB).

**Render [port, offline].** `local/mech/render-04/`: the player variants are bit-identical to
`render-03`. As an opponent at 3 m, its whine and backfire add RMS 0.085–0.10 against its engine's
0.15–0.19, three times what the player's own add at the bumper (0.03): the AI group is −8 dB in
the Bonnet camera, the player's Gearbox group −18 dB, and 3 m is inside the whine's 6 m minimum
distance. The table makes a close opponent's whine louder than the player's own [I: as the
original's table gives it; not compared with the original by ear or capture].
- Not ported: the voice pool and stealing (all sounds), opponents' turbo and clunk (the original
  has none).

## 26. The handbrake ported; wind is never started (F583)

26 October 2026, Claude Code (car-series, owner task item 4). Static (`tools/re/grid_re.py`
decompile/dis, cache `local/re-cache`) and offline renders; GRID was not launched.

**43C320 [O].** 457BE0 calls it once a frame for the player's car only (458C92, inside the
car-audio +0xA4D8 block).
- The input is 9961B0(8), the byte at vehicle +0xE38: the port's handbrake byte
  (`source_vehicle.rs`, `controls.handbrake`).
- A latch of three bytes: +0xA5E8 pulled, +0xA5E9 released, +0xA5EA played. The constructor 43B1F0
  sets pulled 0, released 1 and played 0 (43B5FD..43B60A).
- With the input set and released set: pulled 1, released 0 (played stays). With the input clear
  and pulled set: released 1, pulled 0, played 0.
- While played is clear: pulled plays `hbrakeon` (M+0x128, read as [P+0xB8], 43C3A8), else released
  plays `hbrakeoff` (M+0x12C, [P+0xBC], 43C515); then played = 1 (43C677).
- [O] So the first frame plays `hbrakeoff` once, before any pull.
- The one-shot goes through 421B70 at 0 dB, pitch 1, Doppler 0, at the Cockpit emitter
  car-audio +0x7E0, group car-audio +8 (Other Mech, 9), priority +0xA3C4.
- Varieties (car_genmec): `hbrakeon` (handon1, handon2) and `hbrakeoff` (handoff1, handoff2), preset
  Handbrake 4/8.5 m.
- This corrects §7.4: the varieties are read through P = M+0x70, not unused.

**Wind [O].** No reader of M+0xF8/+0xFC (`WindSlow`, `WindFast`), as [P+0x88]/[P+0x8C] or
directly, in all of .text (a P-tracking scan) or in the 685 decompiled audio functions of
the audio code range (`local/sounds-research/callers.md`). Group 20 (Sense Of Speed) is used once, by the front-end ambience loop 44A9D0,
so §7.4's "no play call passes group 20" holds for races only. Wind stays unported: the original
does not play it in races.

**Port.**
- `mech_audio`: the bank also reads `car_genmec.WIP` and loads the `hbrakeon`/`hbrakeoff`
  varieties with a constant curve (0 dB, pitch 1) and their preset distances.
  - Route (Other Mech group, Cockpit emitter).
  - `Levels` gains the Other Mech group and the Cockpit distance.
- `engine_audio_continuous::Mixer`: `handbrake` input and the three-byte latch above, player only;
  `reset` sets it as 43B1F0 does.
- grid-app:
  - the player's mailbox word carries the handbrake byte in bit 60;
  - the mixer level of Other Mech is published with the other player groups;
  - the Cockpit distance comes from the car setup's cockpit offset.
- The race tool's `GRID_ENGINE_AUDIO_TRACE` gains a `handbrake` column. The probe
  `grid-mech-audio` gains a `handbrake` variant, with the handbrake pulled in the authored tail.

**Render [port, offline].** `local/mech/render-05/`, the same solo Lacetti lap. Since F579 the
lap differs from §24's from tick 1518; the race tool built from origin/main gives the same race
and trace as this one. The AI never pulls the handbrake (the trace column is 0 on all 5600 ticks),
so the `handbrake` variant (Other Mech −11.5 dB, Cockpit 1.91 m from the bumper ear) has three
events over `base`:
- the first frame's `hbrakeoff` at 0 s (RMS 0.055, peak 0.26);
- the authored pull at 90.10 s (`hbrakeon`, RMS 0.055);
- the release at 91.10 s (`hbrakeoff`, RMS 0.065).
Not compared with the original by ear or capture.

## 27. Damage and suspension sounds of the player's car ported (F586)

26 October 2026, Claude Code (car-series, owner task item 4). Static (`grid_re.py`
decompile/dis, cache `local/re-cache`) and offline renders; GRID was not launched.

457BE0 calls, for the player's car only (car-audio +0xA4D8 set), 4560E0, 43C320 (§26), 44DF60,
453A70 and 4562F0 every frame; none was ported before. With P = [[car-audio +0x2C] +0x18] = M+0x70
(§7.4) the CollisionInfos they read are P+0x10 SuspensionUp*, P+0x14 SuspensionDown*, P+0x18
DamageBreak, P+0x28 DamageSuspension, P+0x2C DamageSteering (an impact-variety table: +0x28 Start,
+0x2C End, +0x30 EndDammy) and P+0xC8 damageEngineLoop (car_genmec).

**Inputs [O].** Car-audio +0x24 is the vehicle.
- 9961D0(i) = vehicle +0xE18 + 4i, negated for i = 6 (×[CB0948] = −1): 2 = +0xE20 the brake,
  6 = −(+0xE30) the steering. 9961B0(8) = +0xE38 the handbrake (§26).
- The damage object is [car-audio +0xA338] + 0xAB20 = vehicle +0xCDF0 (41DB20: +0xA338 =
  98DA40(vehicle) = vehicle +0x22D0), the component levels of MECHANICAL_DAMAGE.md [I: offset
  arithmetic, consistent with the port's `0xcdf0 + 4i` levels]:
  99A4D0(i) = +0xCDF0 + 4i (3 = engine temperature), 99A540(i) = wheels +0xCE08 + 4i (i < 4, else
  0), 99A580(i) = suspension +0xCE18 + 4i, 99A5C0(i) = brakes +0xCE28 + 4i.
- Wheel i's record: vehicle +0x1990 + 0x150·i; its position is transformed by the car matrix
  car-audio +0xA3F0 (or 422200 in the other camera branch).
- r = vehicle +0x1894 / +0x17A4, the engine speed over its maximum.

**44DF60, engine damage loop [O].** l = 99A4D0(3), the engine temperature level.
- l ≤ 0.1: stops the voice +0xA3A4.
- Otherwise the first frame starts `damageEngineLoop` (`dam_engine`, preset enginedamage) with the
  car's +0xA3C4 priority, group car-audio +0x10 (Engine) and returns.
- Later frames: v = 1, or (1 − (r − 0.3))² for r ≥ 0.3; v = clamp(min(v, l), 0, …); v = 0 while
  [+0xA494] +0xD8 is set; pitch = clamp((r − 0.3)·0.5 + 0.5, 0.5, 2); level lin2db(v), position
  emitter +0x7B0 (444280 clamps the level to −48…0 dB and the pitch to 0.5…2).

**453A70 / 44DB50, brake damage [O].** b = 9961D0(2), the brake pedal.
- b ≤ 0.1: stops every wheel's voice +0xA340 + 4i.
- Otherwise for each wheel with brake damage d = 99A5C0(i) > 0.1: the first frame starts
  `DamageBreak` (`dam_brake`, group car-audio +8 = Other Mech) at −48 dB.
- Later frames, at the wheel's position: s = |car velocity| (A69800 on [vehicle +0xDB0]: body +0x220,
  the linear velocity); a smoothed brake +0xA350+4i follows b at
  0.2 a frame (zeroed while wheel +0xC8 = 0, no contact); x = s·0.2 below 5 m/s, else 1;
  level lin2db(smoothed·x·d·0.75); pitch = 413130 (a clamped lerp) from 1.05 down to 0.975 over
  0–5 m/s, then 0.975 up to 1.025 over 22.5–40 m/s, − 0.05 + 0.05·b.
- 44DB50 reads `mechanical_damage_gain` only in a removed block (Ghidra: unreachable).

**4560E0 / 4441F0 / 453DA0, suspension [O].** Per wheel, with w = wheel record +0x40 (the
suspension travel [I]) and the previous value at car-audio +0xA5C8+4i:
- u = (w − previous)·100 (per frame); u = 0 ends the whole loop for that frame (a `return`).
- u > SuspensionUp minVel: 4441F0 plays the SuspensionUp soft/med/hard CI (by u) once, latched in
  the per-wheel byte manager +0x4E68 + 6·slot + i until u falls back; u < −SuspensionDown minVel:
  the same with SuspensionDown and |u|; else the latch clears.
- 453DA0 with u·0.01: if suspension damage 99A580(i) ≥ 0.2, x = damage·|car-audio velocity
  +0xA480 (four components)|·|u·0.01| starts or updates the `DamageSuspension` one-shot (CI 0.01–2.5,
  −48…0 dB, pitch 0.9–1.1) at the wheel, group car-audio +0xC (Displacement).

**4562F0 / 454430 / 44ED30, steering damage [O].** m = max(99A540(2), 99A540(3)) (the rear
wheels' damage [I: wheel order]), s = 9961D0(6).
- m ≤ 0.3 or |s| ≤ 0.05: stops the voice +0xA370 (after one more 454430(0)).
- Otherwise the steering smoothed at +0xA4A8 follows s at 0.1 a frame; 454430(|smoothed|) starts or
  updates the DamageSteering loop at the Cockpit emitter +0x7E0, group car-audio +8: the CI is the
  first of Start (0–0.8), End (0.8–1.0), EndDammy whose range holds x; level and pitch interpolate
  its range. While the voice plays and x > End's minVel + k (k = 0.1, or 0 at x = 0), the counter
  +0xA4AC grows by [CAF888] = 1 a frame and x += sin(c·[CB4418] = 1/3)·sin(c·0.1)·k (BF4E44 is the
  CRT sine, §20).

**Constants [O]** (read from `local/compatibility/grid-unpacked.exe`): the brake pitch ends
[CB4AFC] 1.05, [CB503C] 0.975, [CB5044] 1.025, the bend [CB5048] 22.5 with slope [CB5040]
0.0571 (1/17.5), the speed limit [CB093C] 5.0, the brake term [CB082C] 0.05; the grind step
[CAF888] 1.0, [CAFCD4] 0.1, [CB4418] 1/3.

**Port.**
- `grid_tools::damage_audio`:
  - `Input::vehicle` reads the words above, and the body velocity for the speed.
  - `Damage::frame` runs 4560E0, 44DF60, 453A70 and 4562F0 in 457BE0's order, once per vehicle tick. It returns the loops (engine, four brakes, steering), loop stops and one-shots, each with its place (EngineBay, Cockpit or wheel i) and group.
- `collision_audio`:
  - the bank also loads the `car_genmec` samples its varieties name;
  - `Curve::contains` and `Listener::cooled` (435EA0 with its cooldown) are public;
  - the mixer's loops take slots (`LOOP_SLOTS` and up for the damage loops) and can stop.
- grid-app `collision_sound`:
  - the player's damage sounds run after its contacts each tick;
  - a wheel's place is its record anchor through the car pose, and EngineBay and Cockpit are the carsetup offsets;
  - Engine and Other Mech are heard from the camera, as the engine and mechanical voices are; Displacement is heard from the car, as the player's other collision one-shots are [I];
  - a new segment resets the state and stops the loops.
- [I] Two simplifications:
  - 453DA0's playing one-shot is updated by 44F160 in the original. The port instead blocks a new one for the variety's longest sample length.
  - [+0xA494] +0xD8, which silences the engine loop, is not traced and stays clear.
- Not ported: the per-frame rates (the original runs these per rendered frame; the port per 16 ms tick); opponents (the original has none).

**Shipped thresholds [O]** (asset test `shipped_damage_sounds_load`). SuspensionUp Soft/Med/Hard:
minVel 2/6/15, maxVel 6/15/30, −7…−4/−3.5…−1/−2…0 dB. As §7.4 found, the Up CIs name the Down
varieties and the Down CIs the Up varieties. A bump therefore needs at least 2 cm of travel in one
frame. DamageSteering Start 0–0.8 (−50…−2 dB), End 0.8–1 (−30…−2), EndDammy 1–1 (−40…−2).
DamageSuspension at 0.01–2.5 starts at −48 dB, so with the −8.5 dB Displacement group (Bonnet
camera, Normal mode) x must exceed about 0.5 (for example damage 0.8 · 20 m/s · 4 cm) to clear the
−48 dB floor of A23B70.

**Render** (`grid-probes` `grid-damage-audio`, `local/damage/render-02`, `analysis.txt`; Lacetti,
bumper view, authored 16 s script, mono; `off` = every damage level 0). Seen in the trace and the
levels, not by listening:
- 0–4 s engine loop (temperature 0.6, revs 0.3→1): off silent, on −30.1 dBFS RMS.
- 4–8 s brakes (0.8 pedal from 30 m/s, brake damage 0.6): off silent, on −18.8 dBFS RMS.
- 8–12 s steering sweep ±0.6 (rear wheel damage 0.5): off silent, on −46.7 dBFS RMS (Start CI).
- 12–16 s bumps of 8 and 20 cm at 20 m/s (suspension damage 0.8): off has 9 bump one-shots, on
  has the same 9 plus 8 DamageSuspension one-shots (17), at −9 to −14 dBFS peak per bump in both.
  The per-bump peaks differ between off and on because the random sample pick diverges.
- An earlier render (`render-01`) with 1.5 cm a tick bumps and damage 0.5 played no one-shot.
  Both thresholds above explain that, and the script was changed.

## 28. The OSD timer box sounds are online-only (research, F589)

27 October 2026, Claude Code (car-series, owner task item 2: countdown and HUD beeps). Static only
(`grid_re.py` dis/xrefs/decompile; the scripts `$TEMP` and the full `.text` listing are not kept); GRID
was not launched. The start lights, drift cues and flashback (§7.6, F452) are already ported; what
remained was the OSD timer box of §7.1.

**The box [O].** The `object/messagebox=counter` widget (vtable 0xCEC0B8, constructor 0x69FF90,
names `object/timer_segment=seg1..4.anim`, `timer_stash.anim`, `go`, `messagebox_anim.anim`):
- Start 0x6A0390 (from the deferred OSD command 0x6C0070, queued by 0x6C1240): +0x140 = the
  remaining time, segment timers +0x144…+0x150, last number +0x15C = −1, done +0x154 = 0. If the
  message's second value is ≥ [0xD85A50] and below the remaining time, the update is run 100 times
  with +0x160 set (a silent fast-forward).
- Update 0x6AD220 (vtable slot 1): remaining −= dt; n = floor(remaining); when n changes the box
  prints `%d` and plays HUD `sfx` `countdown_timer` (n > 0), or `countdown_go` (n ≤ 0, then done).
  The sound needs the OSD's sound object and is skipped during the fast-forward.

**Who starts it [O].** 0x6B0790 queues the start. Its two callers, 0x6B6CE0 (OSD start; sets +0x2A8)
and 0x6BF070 (OSD update while +0x2A8), ask the provider at OSD +0x214 through its vtable +0x1C:
`bool get(float *remaining, int *total, char **text, bool *shown)`. The provider is set from
[[race +0xC0] +4] (0x6B0540, 0x6B6B20, 0x6B8FB0) or passed to the `playerspositioninterface`
constructor 0x6B5240.
- The race-session base 0x7F0500 (vtable 0xCFF378, constructor 0x7FC2B0, built by 0x67A9F9; its
  slot 9 is the Le Mans countdown 0x80E5E0 of LE_MANS.md §2.4) clears `shown` and returns false:
  no timer.
- The only override found, 0x6F9D70 (vtable 0xCEFE10, constructors 0x6E78A0 and 0x6EC3D0, beside the
  `racedriver-pc.live…demonware.net` hosts), belongs to the online session. It reports the box once
  (+0x95), with total 30 and remaining = min(server ms (0x6E4230) / 1000, 30), only in session modes
  4 or 0xB (0x46DBF0, an XOR-protected word).
- [I] The override search covered every function referenced from .rdata whose first return is
  `ret 0x10` and that stores a byte and a dword through pointer arguments. A longer override with a
  different shape could have been missed.

**Conclusion [I].** `countdown_timer` and `countdown_go` belong to an online 30 s timer, not to
single-player race starts (those use the start lights, §7.1). The port has no online play, so
there is nothing to port. Not ported, and correctly so for offline play.

Side finding: the port shows `object/messagebox=counter` with 3, 2, 1, GO at every race start
(`race_osd.rs`, from the time-trial countdown). In the original, single-player race sessions do not
start the box. Fixed in F590: races no longer show it.
`drift_countdown_1` stays unported: it comes from `Osd/dial2.pssg` animation notes on the drift
combo timer, and the port does not show that group.

**Sizes of the speech and music items [O data, I estimates]** (F589, for the owner's ordering):
- **Music (large).** The original plays race music. 0x4127C0 (the `cmr6_audio_music_manager`
  stream chooser) picks `audio/music/raceend.WIM` at the race end. While +0x121 is set it picks
  `music0%i.WIM` with i = rand & 7 + 1. In a replay it picks `replay%i.WIM`, starting at channel
  pair (n & 3)·2. A `race_music_option` (0x4A9E80, `DataRaceMusicToggle`) switches it; its default
  was not traced. Eleven WIM streams of 27–30 MiB each (330 MiB) are in CMSamples.dic at 32 kHz with
  flags 0xF0. The data is compressed (7.7 bits/byte, no Ogg, RIFF or Vorbis framing; no PCM channel
  layout from 1 to 8 channels is smooth), and the CMStream system decodes it with a "compression
  factor". Porting needs that codec reverse-engineered first, then the stream manager (start, stop,
  fades, stems).
- **Speech (large).** `audio/speech/<en|fr|ge|it|sp>/`, 2.5 GiB in total; English has 3814 lines,
  585 MiB, in 21 folders (garage, highlights, intro, damage, position, rules, teammates 518,
  performance, accidents, named AI, strategy, end of race, touge, drift, derby, Le Mans, start race,
  AI names 541, player names). The lines are headerless PCM16 mono, which decodes trivially. The
  work is in the triggers: `eventSpeech.bin` (290 KB) and `audioEventSpeech.bin` (148 KB) map race
  events to lines, read by `audio_event_speech_manager` and `cmr6_audio_speech_manager`. That needs
  their formats and the event semantics (position, gaps, teammates, damage, accidents, strategy),
  most of which the port has only in part. A first slice (start race, position, end of race) is
  medium; the full spotter is several pushes. The rule engine and parameters: docs/SPEECH_AUDIO_RESEARCH.md (F594).

## 29. Loops held after the race end, a wreck or a pause (backlog row 95)

30 October 2026, Claude Code (car-series; owner report F639: after critical damage and the race
end, the sound can freeze in its last state and loop until exit). Static and offline; GRID was
not launched.

**The port's path [V code, test].**
- `Paced` (`audio_pacing`) and the collision mixer hold the last value they were given. The
  engine and tyre voices stop on their own: an Update system mutes them when the race is frozen,
  paused or failed (F434, `Listening::stopped`).
- The collision source's loops have no such system. That covers each car's scrape (453710/458780)
  and the player's damage loops (engine, brake, steering, suspension; 457BE0). Their level arrives
  only from `collision_sound::tick`, once per vehicle tick.
- After terminal damage, grid-app retires the player and, in a field race, the opponents
  (AUTHORED), so `standings.over()` holds. `drive_tick` then returns at once, and so does the
  collision tick. Whatever scrape or damage loop played at the wreck loops at its last level
  until exit. The same holds at any race end that freezes the cars mid-scrape, during a pause,
  and after a drive failure.
- Test `looping_voices_fall_silent_when_the_ticks_stop` (grid-app, purchased data): a loop fed
  each tick as a scrape is.
  - Without the fix: engine and tyres are silent within 1 s of a pause or a failure, but the
    collision loop keeps its level.
  - With the fix: all three are silent, and the loop returns when the ticks run again.

**The original [O static].**
- The cars keep simulating after a finish and a wreck; there is no race-end freeze. 0x458D00
  calls 457BE0 (the per-frame car audio, 458780 inside) for every one of the 20 car audio slots
  every frame. A scrape voice with envelope g ≤ 0.1 is set to −48 dB (0xCB0BC0), and g halves
  each frame while the car is slow or has no impulse (§10). So a resting wreck's scrape fades
  out by itself.
- After 457BE0, 0x458D00 marks a car whose state (+0x20 → +0x6C) is 3 as an AI car audio
  (+0xA6B4 = 1, [+0xA494]+0x334 = 1). These are the flags 0x411120 sets for every car but the
  listened one: not a stop.
- Pause: the Paused mix mode drops every car and world group to −48 dB (§2).
- The voices end when the race is torn down, not at the finish line. EndOfRace only lowers the
  groups when the results appear (§13.2).

**Fix.** `collision_audio::Mixer::hush` makes every loop glide to silence and keeps its place.
grid-app's `collision_sound::audibility` (Update) sends it once when the ticks stop: race frozen,
paused or failed, the condition the engine and tyres use. The next tick's levels bring the loops
back, and the tick clears the flag. It logs `collision_audio version=2 event=hush`. One-shots
play out as before. [I] Silence stands in for the fade the original's continuing simulation would
give. Not heard (muted).

## 30. Location ambience ported (backlog row 70)

30 October 2026, Claude Code. Static reading of the shared master (cache `local/re-cache/` of the
main checkout: 0x453200, 0x44D420, 0x4477F0, 0x447FC0, 0x4611C0, 0x461430) and the disassembly of
0x448004–0x4483A5. The original was not run. This corrects §7.5 on the AutoPlay choice.

**The object [O].** Constructor 0x4611C0 (vtable 0xCB510C; update slot 0x453200) builds the in-race
ambience object with 96 emitter slots of 0x50 bytes at +0x18E4. Its AutoPlay sub-object sits at
+0x1E0. 0x461430 is that sub-object's load step: mode 2 asks the track for its
`local_audio_ambience` name, looks it up with 0x433310 (case-insensitive) and stores the AutoPlay
at +0x21C and its bank at +0x220.

**AutoPlay choice [O static].** Every caller of 0x433310 was traced. They are:
- 0x461430: the location ambience, chosen once at load;
- 0x433738/0x433748: two fixed lookups (crowd, not ported here);
- 0x4392D0: the dev menu;
- 0x45A690: the autoplaybanks loader;
- 0x45D758: the front end.

The ambience-trigger boxes never pick an AutoPlay, so the §7.5 reading that boxes "switch the
active AutoPlay when entered" is withdrawn: their SETTING is not used for the AutoPlay [I: it
belongs to the reverb zones]. Locations without an AutoPlay stay silent.

The track models' `local_audio_ambience` values [V, test `shipped_ambience_per_track`]:

| Location | AutoPlay | Tracks |
|---|---|---|
| loc_usa | yes | Long Beach, San Francisco, Detroit, Washington |
| loc_yoko | yes | Yokohama Docks |
| loc_europe | none | Nürburgring, Spa, Istanbul, Jarama, Milan, Donington |
| loc_japan | none | Haruna, Okutama, Shibuya |
| loc_lemans | none (no `loc_lemans.WIP` either) | Le Mans |
| test | none | front end, freerace, skipfe |

So only the five US and Yokohama tracks have race ambience. The AutoPlays loc_milan,
loc_mil_tun, loc_tun_usa, loc_sf, loc_water, loc_japan1/2, loc_tun_jap, loc_britcir,
loc_spacir, loc_yokoHP and loc_yokoDocks exist in autoplaybanks but no track model names them
[O data; I: unused in the shipped game].

**Per tick [O].** 0x453200 runs while the object is active. Its listener is the first car whose
+0x6C is 0, the human player (+0x3A74 = car +0x24 → +0xDB0):
- 0xA69630 copies the body matrix into +0x3A30 (right), +0x3A40 (up), +0x3A50 (forward) and
  +0x3A60 (position). These are the port's `Runtime::pose()` rows (physical +0x1D0).
- +0x3A70 = |velocity| (0xA69800; the port's `velocity()`, physical +0x220), in m/s.
- Then 0x44D420 runs: a spawn (0x4477F0), then the emitter update (0x447FC0).

**Spawn, 0x4477F0 [O unless marked].**
1. k = clamp(10·speed, 10, 100). Nothing spawns unless random(0, 50) ≤ density·k. For loc_usa
   (density 0.12) that is 2.4 % of ticks when parked and 24 % at 10 m/s or more.
2. A uniform sample of the AutoPlay is picked, kept if random(0, 1) ≤ its preset's probability.
   Its condition and timeOfDay must match; no shipped Sample sets them.
3. A free slot is taken (0x40FD60). For a looping sample (the bank sample's format bit 20, the
   port's WIP flag 0x10), the spawn returns if the looper count is already above maxLoopers;
   otherwise the count rises.
4. r = random(minSpawiningDistance, maxSpawiningDistance). The position is the car's plus r
   times a vector whose components are each uniform between a and a + 4·forward, where
   a = −right or +right with odds 1/2 each (range(0, 4) < 2). The vector lies beside the car,
   up to four forward units ahead [I: the per-component ordering of 0x4682A0].
5. Start the sample (0x43E4B0) at its preset's min/max distance, looping per its flag, with the
   level of the update below without the fade.

**Update, 0x447FC0, per occupied slot [O].**
1. Count the tick and measure the distance d from the emitter to the car.
2. d > 117 m (0xCB4A84): stop the voice, lower the looper count for a loop, free the slot.
3. Otherwise, a one-shot whose voice has ended and with d > 10 m (0xCAFA58) plays again with
   odds 1 in 600 (random % 0x258). On the other 599 ticks its level is not touched.
4. The level:
   - band = linstep(minSpeed, maxSpeed, speed) when minSpeed < maxSpeed, else 1;
   - fade = min(1, ticks/30);
   - t = clamp(fade·band, 0, 1);
   - dB = lerp(minVol, maxVol, t) + master + lin2db(fade);
   - master = lerp(minVol, maxVol, linstep(minVel, maxVel, speed)) of the AutoPlay's
     `masterCollisionInfo` (`autoplay` for the locations), applied only above its minVel
     [I: the offsets +0x54..+0x60 are minVel, maxVel, minVol, maxVol in Curve order].
   - The location presets are minVol −1 and maxVol −48 over 0–50 m/s, so a parked car hears the
     loops at about −1 dB, and they fade out as the car speeds up.
5. The voice is then mixed on the Ambience group (12) from the Camera listener (§7.2), with the
   A23B70 distance law and the preset's min/max distance.

**Port.**
- `grid_tools::ambience_audio`:
  - `location` reads the route's `track_model.local_audio_ambience`;
  - `AutoPlay::load/parse` reads autoplaybanks.bin (presets merged into samples), the WIP bank
    and the master curve;
  - `Field::tick` is 0x44D420 with the spawn and update above;
  - `Mixer` plays one voice per slot, looped or once, with the collision loops' gain glide.
- grid-app `ambience_sound`:
  - installed when the location has an AutoPlay; logs `ambience_audio version=1 event=queued`
    or `event=none`;
  - its FixedUpdate tick runs after `drive_tick` around the player's car and sends levels with
    the group mix, the distance law and the camera pan;
  - when the ticks stop (pause, failure, the frozen race end) it hushes, as §29.

**[I] and gaps.**
- The game's random generator is not reproduced; a seeded LCG stands in.
- The free slot is taken as the first free one.
- A finished one-shot is detected by its length in the field, not by the voice handle.
- The tick is the port's 62.5 Hz vehicle tick for the original's frame (§8).
- The crowds (0x44E220 with NormalCrowd/GrandStandCrowd, Crowd group 11) are not ported yet.

## 31. Crowds ported (backlog row 70)

31 October 2026, Claude Code. Static reading of the disassembly. The original was not run. The
work-up was 0x44E220, 0x4499A0, 0x449FA0, 0x41F280/0x411460, 0x41F2D0, 0x411600, 0x41BCD0,
0x427730/0x4336E3 and the visual crowd's cell loader 0x769D50/0x963D90/0x97A630.

**The object [O].**
- The crowd object (audio manager +0x24DD0, constructor 0x427730) has 35 emitter slots (+0x40,
  stride 0x60). Every slot plays on group 11 (Crowd, E+0x18, 0x4277AD), on the Camera listener.
- At load (0x4336E3) it looks up the AutoPlays NormalCrowd (+0x1C) and GrandStandCrowd (+0x20),
  bank `env_crowd`, and the CollisionInfos `group_gasp`, `grand_gasp`, `gaspControl` and
  `gaspDistControl`.

**Cells [O, I where marked].**
- The audio's "instances" are the visual crowd's grid cells (manager +0x268, stride 0x70), built
  from `crowd_standing.bin` by 0x769D50 and 0x963D90. Only cells without moving instances are
  entered in the grid map. +0x30/+0x40 hold the box and +0x50 its centre.
- +0x14 counts the instances actually placed (0x97A630). [I] All standing instances are placed.
- +0x60 is the share of placed instances whose anim-selector type is 7. [I] That is
  `Grandstand`, the seated sets `m_gr`/`f_gr` (maps `maleSit`/`femaleSit`), so
  people·(+0x60) > 20 means "more than 20 seated spectators".
- The query 0x756E70 scans the grid, x outer and z inner, for cells whose box lies within 150 m of
  the query point. It keeps at most 250, each with the squared distance to its box, then
  0x771D30 sorts the list nearest first. That is a Shell sort with the gaps 8191, 4095, …, 3, 1
  (table 0xD08870–0xD088A0, strict less-than, not stable).
- The order matters. The spawn leaves at the first slot whose cell lies beyond its sample's
  MaxDistance, and slot i takes entry i mod count. Before the sort was ported, the unsorted scan
  order put a far cell first on Washington's grid, and no voice ever started there [V, test].

**Per tick, 0x44E220 [O].**
1. 0x411600 settles the excitement (below).
2. mph of the car (drift_mph).
3. The query point:
   - normally: car position + forward·70·linstep(1, 120, mph);
   - in the mix modes Intro (5) and EndOfRace (6): the camera position, with the forward kept.
4. The query, then the people summed over all results.
5. The count is capped at 35. The bank is GrandStandCrowd when the total exceeds 2500 people,
   else NormalCrowd.
6. If density·clamp(mph, 80, 100) > random(0, 100), spawn (0x4499A0).
7. 0x41F280 stops each looping voice the car is farther from than 0.75·its MaxDistance.
8. If a recoil is pending, the gasp step (0x449FA0) runs.

**Spawn, 0x4499A0 [O].**

Setup:
- s = clamp(mph/100, 0, 1).
- The slots reserved for excited samples (+0x15A4) drop to count/2 when the count is below
  them.
- Intro/EndOfRace set the target excitement to 1.3, s = 0.4 and reserve 30 slots.
- A branch on the race interface's +0x24 == 1 is the network receiver's [I]; it is not taken
  offline.

Then, for each free slot i of 35 (cell = result i mod count):
- skip cells with ≤ 5 people;
- the "grand" bank applies per cell (the seated test);
- above s = 0.5 (and outside EndOfRace), skip cells behind the car;
- pick a uniform sample. If the cell lies beyond the sample's MaxDistance, **leave the whole
  spawn** (0x449CA2);
- excitement band:
  - while the current excitement x is outside [0, 1], the reserved slots take only samples whose
    min/maxSpeed contains x (the excited and booing samples);
  - otherwise the reserved slots stay empty, and the others take only samples whose band
    contains s;
- the people count must lie in the sample's min/maxSpawiningDistance (a head-count window);
- the probability roll; the loop limit maxLoopers;
- t = linstep(minSpawn, maxSpawn, people):
  - level = lerp(minVol, maxVol, t), plus the master curve (`crowd`) over mph when not excited;
  - pitch = lerp(0.95, 1.05, t), not random;
- a one-shot below −32 dB is not started;
- the voice plays at the cell centre with the sample's min/max distance. Its level and pitch
  never change after the start. Its Doppler property is 0.5 for loops once the start lights have
  begun (+0x15A0), else 0 (Doppler not modelled, §7.2).

**Excitement [O].**
- 0x424B60 (BroadcastMessage_EventMessage) calls 0x41F2D0(level = message `userData`,
  hold = `queueValue`). The messages are those of the speech data's ReactiveCrowd event, the only
  `isEventTrigger` event. 0x45EC00 puts each one whose conditions hold into the external queue
  (+0x110, up to maxMsgInQueue). 0x44BE90 sends the queue's front unless it was the last sent,
  then empties the queue.
- 0x41F2D0 reserves 25 slots for a level above 1, else 10. It takes the level and hold only once
  the previous hold has run out.
- 0x411600 copies the target into the current value. Once a non-zero hold has run out it zeroes
  only the current value, so the current value then alternates between the target and 0 on each
  tick until the next event [O; ported as is].
- The 26 ReactiveCrowd messages [V, test]:
  - starting grid 1.3;
  - overtaking and slipstream 1.2–1.3 (1 s);
  - drifts 1.2–1.4 (2 s); lost combo −1.1;
  - sat on track and wrong way −1;
  - finish line 1.2, last 1.3, team 1.4, finish without damage 1.3;
  - normal 0, normalGood 1.3.

**Gasps (CrowdRecoil) [O, not ported].** The visual crowd broadcasts a recoil (0x764A40). After
it, 0x449FA0 plays `group_gasp`/`grand_gasp` on query cell j, one per tick. The level is the sum
of three curves: gaspControl over intensity, the variety's own curve over people, and
gaspDistControl over the distance from the recoil. The port's visual crowd raises no recoil, so
the gasps have no trigger and are left out.

**Port.**
- `grid_tools::crowd_audio`: `Cells` (grid, seated count, query), `Bank` (both AutoPlays over
  one decoded `env_crowd`), `Crowd` (excitement, tick, spawn, loop stop).
- `ambience_audio::AutoPlay::parse_shared` shares the decoded samples between AutoPlays.
- `speech::Manager` now evaluates ReactiveCrowd into the external queue (`SLICE` += ReactiveCrowd)
  and reports `Output::crowd`, logged `speech_audio version=1 event=crowd`.
- ReactiveCrowd reads three collector parameters the speech slices had not needed [O]:
  - `teammateLap` (0x45F590 getter, collector +0x42C): the teammate record's lap;
  - `currentDriftType` (0x4193E0, +0x158) and `lostCombo` (0x419400, +0x184): the player
    record's +0x4C and +0x78. 0x438C10 fills them from the car's drift scorer (0x64DD50 →
    entry +0x60) only when it finds one (0x4390F7–0x439107):
    - `currentDriftType` = the scorer's ratings reached (+0x20) − 1, so −1 between drifts,
      0 for `osd_drift`, 1 great … 6 legend;
    - `lostCombo` = the scorer's combo-failed byte (+0x4C), set by 0x630490 and cleared by the
      next score step.
  - Without a scorer the record keeps its values; a record clear sets 0.
  - The port copies `drift::Scorer::{rating, combo_failed}` of the player's drift entry
    (`RaceView::drift`).
- grid-app `crowd_sound`: installed on every route with `crowd_standing.bin`. It drains the
  speech's crowd events and sets each voice's gain each tick from the camera (group Crowd, the
  distance law, the pan). It hushes when the ticks stop. Logs `crowd_audio version=1 event=queued`.

**[I] and gaps.**
- The game's RNG is not reproduced.
- A finished one-shot is detected by its length.
- The excitement needs the speech manager, which runs only in races with opponents.
- The gasps are not ported.
