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
| [F570](log/F0570.md) | 2026-10-25 | Disk cleanup: obsolete APKs, idle sessions' build dirs, uncited behavior captures |
| [F569](log/F0569.md) | 2026-10-24 | Playable 9c1bfe65e68f with F567-F568, portable copy and APKs 3cdbf523a7f0 |
| [F568](log/F0568.md) | 2026-10-24 | Derby frame rate measured against F561; debris contacts and lift_sunk on the compute pool, bit-identical |
| [F567](log/F0567.md) | 2026-10-24 | Playable 7360a4865f54 with F563-F566, portable copy and APKs a2b4991d6817 |
| [F566](log/F0566.md) | 2026-10-24 | Every broken piece on the phone tracks gets a draw: bundle roots, skinned damage meshes, suffixed names |
| [F565](log/F0565.md) | 2026-10-24 | Detached panels are world bodies: 9EC9B0 part decisions in the simulation, cars collide with the panels |
| [F564](log/F0564.md) | 2026-10-24 | Broken tyres drawn on their bodies and lifted out of the road |
| [F563](log/F0563.md) | 2026-10-23 | Detached panels settle lying flat: rigid box with the original's detached-part body parameters |
| [F562](log/F0562.md) | 2026-10-23 | Playable 14696d47fe1e with no mid-air parking (F561), portable copy and APKs 99fef6aca274 |
| [F561](log/F0561.md) | 2026-10-23 | No body parks in the air: far islands rest for turnOffTime before they stop; detached parts land on the track |
| [F560](log/F0560.md) | 2026-10-23 | Playable 67dbeb3a5158 with Render height Auto (F559), portable copy and APKs f2af457cca2c |
| [F559](log/F0559.md) | 2026-10-23 | Render height Auto (dynamic 480-720) on the Video screen, simulation-only CPU check, Auto cycle row removed |
| [F558](log/F0558.md) | 2026-10-22 | Playable fc0ff3aa7868 with resolution-only dynamic preset (F555-F556) and the tyre-stack chain fix (F557), portable copy and APKs 94e06d136784 |
| [F557](log/F0557.md) | 2026-10-22 | Pieces no longer break tyre stacks: the original's car-only break pairing (0x7C46A0) |
| [F556](log/F0556.md) | 2026-10-22 | Dynamic render height 480-720 rows on Android; UI, HUD and OSD drawn at the window's resolution |
| [F555](log/F0555.md) | 2026-10-22 | Mobile preset reduced to the render resolution (720 rows) and compute threads; phone Auto cycle table |
| [F554](log/F0554.md) | 2026-10-22 | Playable b89b4be10d30 with the phone menu fixes (F553), portable copy and APKs 9ac27f9a958a |
| [F553](log/F0553.md) | 2026-10-22 | Phone UI: Performance test shows all rows with Auto cycle first; full Fira Mono as the UI font so Nürburgring renders on Android |
| [F552](log/F0552.md) | 2026-10-22 | Playable 01f22b4c80d3 with the tyre-pile fix (F551), portable copy and APKs f883be0507f0 |
| [F551](log/F0551.md) | 2026-10-22 | Tyre piles sleep: original cylinder pair contacts and shell masses |
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

Older entries: [log/INDEX.md](log/INDEX.md).

<!-- log-index:end -->
