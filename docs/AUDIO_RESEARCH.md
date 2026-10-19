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
