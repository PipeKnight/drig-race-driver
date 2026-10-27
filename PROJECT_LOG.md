# DRIG project log

The project's current state, open problems and next actions, plus an index of recent entries.
The entries themselves are one file each in [log/](log/INDEX.md): `log/F0532.md` holds F532.
Find details with `grep -rn "F512 " log/` or `grep -rln "Redgate" log/`; do not read all of log/.
Write new entries with `py -I tools/log_entry.py new "Title"` and run `check --fix` before every
push (AGENTS.md "Durable records"). Edit the state sections below only when a status changes.
Until 2026-10-21 (F532) the whole log was this one file; `git show d927d415:PROJECT_LOG.md` has it.

## Current state (2026-10-21, after F531)

Playable: `target/release/grid-app.exe` e57f2fd391e2 from source 6c8b4fad (origin/main a11cddf7 merged
locally), the portable copy and APKs a11cddf7de27 (F531). Work paused at 50 % weekly usage (F531);
child sessions resume on the owner's word. Every "Not checked" below is open, not verified.

### Driving and physics
- Source-faithful single-car physics (suspension, tyres, steering, gearing, damage, deformation,
  body contacts, joints for tyre walls and armco) ported function by function and bit-checked against
  the original where captures exist; history in log/ F139–F300 and docs/VEHICLE_*.md, docs/CONTACT_*.md.
- Fixed 16 ms world step (owner decision F286; backlog row 11). The original steps once per frame by
  the frame time (F543); the interactive app runs at most 4 ticks per frame, so a slow tick slows the
  game instead of collapsing FPS (F544, docs/FRAME_STEPPING.md). Prop pieces sleep (F541) and park
  far from the player (F546); the tick's whole-world and row-buffer copies are gone (F546): a
  parked piece costs about 1.3 µs per tick, an awake one about 16 µs.
- F530: contacts with a normal drifted off unit length no longer pause the race.
- Open: backlog row 90, the player car thrown 450 m by a body contact at Le Mans tyre walls (tick
  30431 of the run8 repro, `local/handoff/robustness.md`); continuous collision covers only props and
  lost wheels (row 10); Porsche 911 weak solo (row 2).

### AI
- The original's AI driver is ported (line following, braking lines, traction control and ABS cap
  F444, Redgate brake-line ratio on the cruise tables and Derby avoid F476, reverse byte F517,
  catch-up config F439, drift AI).
- Open: first-corner pile-ups with 12 cars (rows 30, 31: three "alongside" parts not ported, no live
  trace of an original overtake yet); Washington route 0 no-brake bend has no
  1:1 fix (row 41, F526); start-light reaction (row 35); autopilot wreck at Jarama route 1 (row 89).

### Events, career and race rules
- Modes: race on the 11 circuits of `route::CIRCUITS` (Donington, Nürburgring, Istanbul, Long Beach,
  Jarama, Yokohama, Spa, Milan, Shibuya, San Francisco F425, Circuit de la Sarthe F428) plus other routes
  such as Detroit and Washington (F520); time trial; drift (battle, GP, freestyle); touge incl. Haruna;
  Demolition Derby; Le Mans 24 Hours (race type 3, F429/F430). Circuit series F401/F457.
- Career (GRID World) screen with events, fields, payouts, head-to-heads, driver offers, eBay Motors,
  Flashback Bonus, Le Mans selection events and class positions (F424–F522).
- Open: career run 13 (Le Mans) not rerun after F530; Okutama does not load (row 82); retire timer and
  marshal flags (rows 45–47); Drift Battle stand-in cars (row 52).

### Audio
- Engine (continuous mixer, gear shifts F528, every car's bank loads F519), tyres, collisions (F479),
  scrape (F488), glass (F511), landing and thumps (F502/F504), stereo panning (F492), mix modes
  StartLine/Crashed/EndOfRace (F493/F496/F500).
- Open: gear clunk, backfire, whine and turbo (researched in F529, next for car-series,
  `local/handoff/car-series.md`); wind, crowd, beeps, speech, music (row 70); engine still sounds
  artificial to the owner (row 72); never compared by ear on this build.

### Graphics
- Codex's workstream (G entries, latest G129 of 2026-10-18, docs/GRAPHICS_*.md); playables include
  Codex's local graphics commits. Every circuit draws its own sky and layered road (F515, docs/SKY.md).
- Open: night lighting (only the sky darkens), the original post-process chain (row 77), the enhanced
  profile only for Lacetti on Donington (row 78), strobing wheels (row 81).

### Android and platforms
- macOS F577: native arm64 release build and code-only `DRIG.app` packaging pass
  on macOS 15.7.7, with no engine changes. CLI help and the menu's expected
  missing-installation exit pass; Finder launch, graphics, input, audio and
  actual races are not tested (no game data). See [docs/MACOS.md](docs/MACOS.md).
- Android P9–P12: code-only APK importing the user's own GRID folder (F405), bundled event APKs
  (F418), touch/tilt/pad controls (P11, F404), per-system profiling (F395); docs/ANDROID.md.
- Open: the a11cddf7de27 APKs are not tested on the phone; CPU-bound at about 30–34 FPS (row 85);
  pad rumble and phone vibration are not felt on the phone yet (row 22).

### Tooling and process
- Build gate `tools/build_gate.ps1` and nightly soak (F524); shared seed build and rust-lld (F467);
  `tools/prune_build_dirs.ps1` (F438); grid-soak harness (docs/SOAK_TESTING.md); check levels and the
  identity check in docs/TOOLCHAIN.md.
- F532: the log is split into log/ with `tools/log_entry.py`; open capture work is listed in
  [docs/CAPTURE_BACKLOG.md](docs/CAPTURE_BACKLOG.md).

## Open problems

The full list is [docs/FIDELITY_BACKLOG.md](docs/FIDELITY_BACKLOG.md) (rows 1–90; "Top 10 by gameplay
impact" at its head). The most visible now:
1. AI first-corner pile-ups (rows 30, 31).
2. Missing sounds and the artificial engine (rows 70–72).
3. Force feedback, wheel input and pad rumble on Windows (F602), pad and phone vibration on Android (F604) need the owner's wheel test (row 22).
4. Le Mans body-contact blow-up (row 90).
5. Drift Battle stand-in cars and low AI drift scores (rows 36, 52).
Work that needs a live recording of the original grid.exe: [docs/CAPTURE_BACKLOG.md](docs/CAPTURE_BACKLOG.md).

## Next actions

- On the owner's word, resume the paused children (F531): robustness row 90, then body sleeping
  and tyre walls; car-series gear clunk/backfire/whine/turbo; career rerun of event 13.
- Manual lap and sound on speakers on playable e57f2fd391e2; the a11cddf7de27 APKs on the phone.
- AI alongside parts (rows 30/31) to end the first-corner pile-ups.

## Where to look

- Every document by subsystem: [docs/INDEX.md](docs/INDEX.md); every tool by purpose:
  [tools/README.md](tools/README.md).
- Plans: [docs/IMPLEMENTATION_PLAN.md](docs/IMPLEMENTATION_PLAN.md),
  [docs/NEXT_MILESTONES.md](docs/NEXT_MILESTONES.md) (14 October, partly historical).
- Builds and checks: [docs/TOOLCHAIN.md](docs/TOOLCHAIN.md), [docs/PROTOTYPE_BUILD_UPDATES.md](docs/PROTOTYPE_BUILD_UPDATES.md),
  [docs/TRY_THE_PROTOTYPE.md](docs/TRY_THE_PROTOTYPE.md), [docs/DISK_USAGE.md](docs/DISK_USAGE.md).
- Areas: [docs/AI_DRIVER_PORT.md](docs/AI_DRIVER_PORT.md), [docs/AUDIO_RESEARCH.md](docs/AUDIO_RESEARCH.md),
  [docs/CAREER_RESEARCH.md](docs/CAREER_RESEARCH.md), [docs/DISCIPLINES.md](docs/DISCIPLINES.md),
  [docs/SERIES.md](docs/SERIES.md), [docs/ANDROID.md](docs/ANDROID.md), [docs/GRAPHICS_INTEGRATION.md](docs/GRAPHICS_INTEGRATION.md),
  [docs/ROBUSTNESS_RESEARCH.md](docs/ROBUSTNESS_RESEARCH.md), [docs/RUNTIME_LIMITS_AUDIT.md](docs/RUNTIME_LIMITS_AUDIT.md).
- Original installation and evidence rules: AGENTS.md; legacy 6–9 October layout (objective, F001–F095
  findings table, hypotheses, document map): [log/legacy/](log/INDEX.md#legacy-sections-69-october-2026-layout).

## Recent entries

<!-- log-index:start (generated by tools/log_entry.py; do not edit by hand) -->

| Entry | Date | Title |
|---|---|---|
| [F630](log/F0630.md) | 2026-10-27 | Cockpit: dash gauges (digital readouts, motec bars, shift lights, needles) as the original |
| [F629](log/F0629.md) | 2026-10-27 | Speech audio name default Jack (M_46) |
| [F628](log/F0628.md) | 2026-10-27 | Playable 6a19dc55687c with F610-F626, portable copy and APKs 081b8353acf7 |
| [F627](log/F0627.md) | 2026-10-27 | Cockpit: steering wheel turned by the interior Steering clip, NeAnim reader |
| [F626](log/F0626.md) | 2026-10-27 | Faster pre-push: Clippy first, one lock hold per gate, pass cache, per-circuit race tests |
| [F625](log/F0625.md) | 2026-10-27 | Cockpit: rear-view mirror rendered as the original (512x128 every frame), cost at the phone's resolution |
| [F624](log/F0624.md) | 2026-10-27 | Speech part 3 step 3: championship and career values |
| [F623](log/F0623.md) | 2026-10-27 | Speech part 3 step 2: the player-name phrases (audio name) |
| [F622](log/F0622.md) | 2026-10-27 | Speech part 3 step 1: the lapping lines from the track order |
| [F621](log/F0621.md) | 2026-10-27 | Owner request: PC graphics options as in the original, later |
| [F620](log/F0620.md) | 2026-10-27 | AI car wedged against Washington tyre stacks: stuck retire requests removal |
| [F619](log/F0619.md) | 2026-10-27 | Camera: every view against the F470 captures with the final code; live captures still needed |
| [F618](log/F0618.md) | 2026-10-27 | Camera: the player's cockpit in the head-cam, no car in the bumper views |
| [F617](log/F0617.md) | 2026-10-27 | Speech part 2 step 5: the namedAI name phrase |
| [F616](log/F0616.md) | 2026-10-27 | Start hitch: prop pieces share meshes; OSD camera without sun cascades; cascade rule checked |
| [F615](log/F0615.md) | 2026-10-27 | The Viper race at Washington in every bundled APK: menu entry race_viper, traced data, stage check |
| [F614](log/F0614.md) | 2026-10-27 | Speech part 2 step 4: driver identities, key drivers and the teammate |
| [F613](log/F0613.md) | 2026-10-27 | Camera: Change View blends as in a race, every view runs every frame |
| [F612](log/F0612.md) | 2026-10-27 | Camera: spin hold proven on a deterministic scene, the chase knock, and the original's per-frame clock (camera_fps) |
| [F611](log/F0611.md) | 2026-10-27 | Speech part 2 step 3: accidents and AI events by race entry |
| [F610](log/F0610.md) | 2026-10-27 | Playable 3f4382d3cc41 with F604-F609, portable copy and APKs f07fcafb0dad |
| [F609](log/F0609.md) | 2026-10-27 | Grid start: opponent car levels by distance as in the original |
| [F608](log/F0608.md) | 2026-10-27 | Bundled APKs carry every file their event reads: traced file lists, speech lines, a stage check |
| [F607](log/F0607.md) | 2026-10-27 | Every route's own road: routesplit.pssg drawn; Washington's missing main straight and Milan's start |
| [F606](log/F0606.md) | 2026-10-27 | AI planned offset at the car and the limit clamp's far-side case: Donington |
| [F605](log/F0605.md) | 2026-10-27 | Playable f1e871287420 with F593-F603 and portable copy; APKs deferred |
| [F604](log/F0604.md) | 2026-10-27 | Android pad rumble and phone vibration from the GameFFB task |
| [F603](log/F0603.md) | 2026-10-27 | Phones without BC: OSD textures made before the upload path was chosen are converted; desktop check with BC off the device |
| [F602](log/F0602.md) | 2026-10-27 | Force feedback port: GameFFB, DirectInput wheel input and force, XInput rumble |
| [F601](log/F0601.md) | 2026-10-27 | AI corner lift only above the brake-line target: Spa La Source |
| [F600](log/F0600.md) | 2026-10-27 | Grid start: GPU pass timer and opponent shadow bodies |
| [F599](log/F0599.md) | 2026-10-27 | Speech part 2 step 2: the player's damage, performance, strategy and wrong-way lines |
| [F598](log/F0598.md) | 2026-10-27 | Force feedback research: the original GameFFB pipeline, snapshot builder and device output |
| [F597](log/F0597.md) | 2026-10-27 | Row 90 rechecked on current main: the Le Mans 24 Hours race runs to its end |
| [F596](log/F0596.md) | 2026-10-27 | Speech part 2 research: damage, performance, accidents, teammate and key-driver parameters |
| [F595](log/F0595.md) | 2026-10-27 | Speech part 1 ported: race start, position calls and finish lines |
| [F594](log/F0594.md) | 2026-10-27 | Speech research: triggers, choice and queue of the race engineer's lines |
| [F593](log/F0593.md) | 2026-10-27 | Playable b90f145101b3 with F581-F592, portable copy and APKs 3c7a3516e27a |
| [F592](log/F0592.md) | 2026-10-27 | Skid marks stream only changed strips |
| [F591](log/F0591.md) | 2026-10-27 | Single static parts gather in coarse cells |
| [F590](log/F0590.md) | 2026-10-27 | Race HUD: no countdown box at race starts |
| [F589](log/F0589.md) | 2026-10-27 | OSD timer box sounds are online-only; speech and music sized |
| [F588](log/F0588.md) | 2026-10-27 | Shadow and bounds passes skip batched and static meshes |
| [F587](log/F0587.md) | 2026-10-26 | Early-race tick cost: whole vehicle-record copies cut |
| [F586](log/F0586.md) | 2026-10-27 | Player damage and suspension sounds ported |
| [F585](log/F0585.md) | 2026-10-26 | AI first corners: view-room fix, alongside parts, observer list and path scan ported; live queue comparison |
| [F584](log/F0584.md) | 2026-10-26 | Car part batching |
| [F583](log/F0583.md) | 2026-10-26 | Engine audio: the handbrake ported (hbrakeon/hbrakeoff on the player's handbrake edges); wind is never started |
| [F582](log/F0582.md) | 2026-10-26 | Engine audio: opponents' backfire and gear whine; the original's 120-voice pool |
| [F581](log/F0581.md) | 2026-10-26 | Playable 975c018e7e2c with F569-F580, portable copy and APKs a2fc9b444690 |
| [F580](log/F0580.md) | 2026-10-26 | Ground cover faces the camera in the vertex shader; skid marks stream in place |
| [F579](log/F0579.md) | 2026-10-26 | Physics cost growth over a race: per-contact copies in car groups cut |
| [F578](log/F0578.md) | 2026-10-26 | Engine audio: the gear whine ported (GearPreset loop per gear, envelope, upshift wobble) |
| [F577](log/F0577.md) | 2026-10-26 | Native macOS build and code-only application bundle |
| [F576](log/F0576.md) | 2026-10-26 | Static world batching |
| [F575](log/F0575.md) | 2026-10-26 | Engine audio: backfire, turbo and the load path (applied throttle, lift-off burble, rev limiter) ported |
| [F574](log/F0574.md) | 2026-10-26 | Dormant prop pieces skip the per-entity passes |
| [F573](log/F0573.md) | 2026-10-26 | Desktop Auto render height and compute threads on the desktop |
| [F572](log/F0572.md) | 2026-10-26 | Engine audio: gear clunk ported (GearClucks from the car's mec bank); the whine plays; shift effects reach grid-app |
| [F571](log/F0571.md) | 2026-10-26 | Auto render height probes each step and returns to 720 when the rows are not what makes the frame late |

Older entries: [log/INDEX.md](log/INDEX.md).

<!-- log-index:end -->
