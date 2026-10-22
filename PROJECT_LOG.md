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
- Android P9–P12: code-only APK importing the user's own GRID folder (F405), bundled event APKs
  (F418), touch/tilt/pad controls (P11, F404), per-system profiling (F395); docs/ANDROID.md.
- Open: the a11cddf7de27 APKs are not tested on the phone; CPU-bound at about 30–34 FPS (row 85);
  no force feedback or wheel layer on any platform (row 22).

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
3. No force feedback, wheel layer or rumble (row 22).
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
| [F550](log/F0550.md) | 2026-10-22 | Playable 3bc513002bff with the mobile preset (F549), portable copy and APKs 32a08aa8e4a4 |
| [F549](log/F0549.md) | 2026-10-22 | Mobile preset: Android default for 60 FPS (720 rows, one shadow cascade, small props culled, shorter ground cover, more compute threads); perf_test … |
| [F548](log/F0548.md) | 2026-10-22 | Floating tyre report: not reproduced, diagnostics |
| [F547](log/F0547.md) | 2026-10-21 | Playable 945575e57651 with the cheaper tick and parking (F546), portable copy and APKs 6087fbcafa30 |
| [F546](log/F0546.md) | 2026-10-21 | Physics tick cost and parking |
| [F545](log/F0545.md) | 2026-10-21 | Playable 512364f77245 with the catch-up cap (F544), portable copy and APKs ab8bcfafbc9e |
| [F544](log/F0544.md) | 2026-10-21 | Interactive app runs at most 4 fixed ticks per frame |
| [F543](log/F0543.md) | 2026-10-21 | Physics steps per rendered frame: original one step of the frame delta, port catch-up, no port |
| [F542](log/F0542.md) | 2026-10-21 | Playable c4b2673f4cfc with body sleeping (F541), portable copy and APKs ee1880b472d2 |
| [F541](log/F0541.md) | 2026-10-21 | Rigid-body sleeping for prop pieces and lost wheels |
| [F540](log/F0540.md) | 2026-10-21 | APKs 239583f319e3: drift and drift-profile builds share one bundle |
| [F539](log/F0539.md) | 2026-10-21 | Bundled APK races start: Donington bundle carries the database; profile APK on the drift bundle |
| [F538](log/F0538.md) | 2026-10-21 | grid-app --headless: a full event at CPU speed, bit-identical to the window; tick time by stage and world size |
| [F537](log/F0537.md) | 2026-10-21 | A held lost wheel no longer teleports the pieces of a later break (row 90) |
| [F536](log/F0536.md) | 2026-10-21 | Android seed reuse: build_android.ps1 drops MSYSTEM/CYGWIN so cargo-ndk paths match the seed |
| [F535](log/F0535.md) | 2026-10-21 | Indexes of every tool and document: tools/README.md and docs/INDEX.md, generated by tools/index_files.py |
| [F534](log/F0534.md) | 2026-10-21 | Pre-push tiers, machine-wide build lock and the release script |
| [F533](log/F0533.md) | 2026-10-21 | Shared reverse-engineering base: docs/re/symbols.tsv, tools/re, one master Ghidra project |
| [F532](log/F0532.md) | 2026-10-21 | Project log split into one file per entry; tools/log_entry.py numbers, checks and indexes them; capture backlog |
| [F531](log/F0531.md) | 2026-10-20 | Playable e57f2fd391e2 with F521–F530, the portable copy refreshed, APKs a11cddf7de27, and a pause of several days |
| [F530](log/F0530.md) | 2026-10-20 | Contacts whose normal drifted off unit length no longer pause the race |
| [F529](log/F0529.md) | 2026-10-20 | Engine audio research: the Car listener (43F7A0), the gear clunk, the backfire and the whine/turbo wiring |
| [F528](log/F0528.md) | 2026-10-20 | Engine audio: gear shifts of 451770 ported: RPM dip, off-load crossfade and pitch wobble |
| [F527](log/F0527.md) | 2026-10-20 | Opponent engine mix confirmed offline against the original's group table |
| [F526](log/F0526.md) | 2026-10-20 | Row 41, Washington route 0: the AI's no-brake bend explained from the shipped data; no 1:1 fix found; row 41 text updated |
| [F524](log/F0524.md) | 2026-10-20 | Build gate: launcher smoke, memory growth, tick-cost budget and a nightly long soak of the real app |
| [F523](log/F0523.md) | 2026-10-20 | Washington after F520: the Lacetti wreck 1 km into route 0 was driving and is gone since F517; routes 0, 2, 3, 5, 7 and 8 re-surveyed |
| [F522](log/F0522.md) | 2026-10-20 | Career: head-to-heads 73, 75 and 77 checked from the career side after F520, and paid as the original |
| [F521](log/F0521.md) | 2026-10-20 | Playable b4e9267f47b1 with F519–F520, the portable copy refreshed, and APKs dcc22d914f21 |
| [F520](log/F0520.md) | 2026-10-20 | Career locations: Washington GP route 0 (event 73) and the Haruna head-to-head roads, routes 9 and 4 (events 75, 77), load and race |
| [F519](log/F0519.md) | 2026-10-20 | Every raceable car's engine bank loads; one car's failing bank or decoder silences only that car |
| [F518](log/F0518.md) | 2026-10-20 | Playable 5d8f3f454655 with F514–F517, and the portable copy refreshed |
| [F517](log/F0517.md) | 2026-10-20 | AI reverse byte +0x1E: the car-side consumer is found and ported |
| [F516](log/F0516.md) | 2026-10-20 | Silent engine on the F513 playable fixed: BMW engine decoders no longer fail at idle; race-start audio mix test |
| [F515](log/F0515.md) | 2026-10-20 | Every circuit draws its own sky and the layered road surface, in both presets; Le Mans' sky follows the time of day; the "unresolved road batches" were … |
| [F514](log/F0514.md) | 2026-10-20 | Career: winning the International head-to-head unlocks every car (the original's 0x65AD90); head_to_head_rewards is never read; a longer unattended Le Mans … |
| [F513](log/F0513.md) | 2026-10-20 | Playable 141433900886 with F491–F512, and a private portable copy for another Windows PC |
| [F512](log/F0512.md) | 2026-10-20 | Angular corrections and rotations past 64 rad turn as in the original |
| [F511](log/F0511.md) | 2026-10-20 | Window glass sounds: WindScreen/Window Crack and Smash when a window cracks or smashes |
| [F510](log/F0510.md) | 2026-10-20 | Chase drift blend weight read from the car (car +0xFE0, 0 for every shipped car); the bonnet's 4.7 mm forward residual was capture sampling |
| [F509](log/F0509.md) | 2026-10-20 | Close-drift flags break into a static base and a phantom pole post |
| [F508](log/F0508.md) | 2026-10-20 | Bonnet camera shake: the speed-gated bob of the Mounted views, from a deterministic noise table |
| [F507](log/F0507.md) | 2026-10-20 | Career: the Le Mans selection events are offered, with their multi-class fields, so class positions and routine B run in the app |
| [F506](log/F0506.md) | 2026-10-20 | Close-drift flag post geometry ported and verified bit for bit |
| [F505](log/F0505.md) | 2026-10-20 | The 42D580 one-shot queue is network replication, not a mixing cap |
| [F504](log/F0504.md) | 2026-10-20 | Yaw and pitch displacement thumps |
| [F503](log/F0503.md) | 2026-10-20 | Close-drift flags researched: the original breaks them into a kinematic phantom post |
| [F502](log/F0502.md) | 2026-10-20 | Landing sound after airtime |
| [F501](log/F0501.md) | 2026-10-20 | Joints, phase 3b: armco breaks, hinged to the world and to the neighbouring section, with its override bodies and link propagation |
| [F500](log/F0500.md) | 2026-10-20 | EndOfRace mix mode on the results screen |
| [F499](log/F0499.md) | 2026-10-20 | Camera look input: Look Left/Right/Up/Down turn the chase, bumper/bonnet and head-cam views as in the original |
| [F498](log/F0498.md) | 2026-10-20 | Kick-ups: the player's wheels throw up stones and grass |
| [F497](log/F0497.md) | 2026-10-20 | Track view Catmull-Rom line, the AI handbrake byte 0x77A110, no 1/R pedal flip; the Jarama "hang" was a wrecked autopilot |
| [F496](log/F0496.md) | 2026-10-20 | Crashed mix mode: the player's hard hits duck engine, exhaust and wheels |
| [F495](log/F0495.md) | 2026-10-20 | Career routine B and class positions: the Le Mans 24 Hours pays by class position from the whole reputation pot |
| [F494](log/F0494.md) | 2026-10-20 | Career Flashback Bonus: unused flashbacks pay career money per race, and the payout lists the original's bank rows |
| [F493](log/F0493.md) | 2026-10-20 | StartLine mix mode: AI −3 dB through the start sequence |
| [F492](log/F0492.md) | 2026-10-20 | Stereo panning of the car sounds |
| [F491](log/F0491.md) | 2026-10-19 | Playable 293914533e9c with F485–F490 and Codex's local graphics commits |
| [F490](log/F0490.md) | 2026-10-20 | Collision mix: silent sweeteners, their cooldown, roof test without the offset; opponent engine loudness diagnosed |

Older entries: [log/INDEX.md](log/INDEX.md).

<!-- log-index:end -->
