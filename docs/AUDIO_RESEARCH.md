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
| UI / HUD | fe_fx, audiohud (countdown, lights, drift, results) | none | none |
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
