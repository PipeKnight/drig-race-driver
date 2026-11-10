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
- Open: first-corner losses with 12 cars (rows 30, 31: with the race manager the matrix loses 22,
  F662; Nürburgring 0 front row at a closing exit; the original's Nürburgring first corner matches
  the port); Washington route 0 no-brake bend has no 1:1 fix (row 41, F526).

### Events, career and race rules
- Modes: race on the 11 circuits of `route::CIRCUITS` (Donington, Nürburgring, Istanbul, Long Beach,
  Jarama, Yokohama, Spa, Milan, Shibuya, San Francisco F425, Circuit de la Sarthe F428) plus other routes
  such as Detroit and Washington (F520); time trial; drift (battle, GP, freestyle); touge incl. Haruna;
  Demolition Derby; Le Mans 24 Hours (race type 3, F429/F430). Circuit series F401/F457.
- Career (GRID World) screen with events, fields, payouts, head-to-heads, driver offers, eBay Motors,
  Flashback Bonus, Le Mans selection events and class positions (F424–F522).
- Front end: event menu (Race Day) with Race Options Difficulty and Grid Position; every screen
  fades through black with "Loading" and hands off to the next window without a desktop gap;
  launcher races return to the menu (F756, docs/SCREEN_FLOW.md). The career screen does not use it yet.
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
- G383 (current source): general explicit-coordinate selected crowd clip→dynamic named pose→existing preview path compiles;61CPUtests including six actual resource banks/30modifiers plus734-test package gate/Clippy/release/render pass after shared malformed-normal repair. Matched actual selected-clip Control/Pose GPU pair renders coherent seated pose with background byte-identical. Original clock/Delta/reference, populated animation, flashes/audio remain open. Diagnostic is not a production crowd completion.
- G382 (main ee77df5e/PR36): shared static linked packed-SUN specular colour corrects the constructor-white inference; 23-file source corpus and22focused tests pass, mandatory crate gate715tests/Clippy/defaultbuild/render PASS. Matched Don/STK are identical; Haruna rock highlights change, body nearly unchanged. BMW capture completes but its supervisor OSexit is unverified after an exit race; original aggregate failure retained. Full night/DLAA/crowd remain open.
- G381 (main671a921b/PR35): shared per-slot/owned-family signals and actualHIGH/LOW bank receipts pass FULL1480tests/24soaks/fouridentities and featureClippy/release, including actual44-model data and real reflection-overlay lifecycle. Shared source-material scenes admit all models/routes structurally; DefaultOff. Four zero-window emission arms and recovered offline/independent visual review pass (BMW plus six-model16-car night field); native focus/sampler/photometry/complete-night remain qualified.
- G380 (main671a921b/PR35): shared categorical HLA parser passes13tests/Clippy and actual184-record purchased catalogue after correcting null FactorValue handling. Native actor names/source setup linked statically; general animation/clock/pose/render integration remains open.
- G379 (negative DLAA experiment): matched default/J/K inputs and native creation receipts pass; J worsens antenna trails, K close to default. No hint/default or dependency change delivered; thin-detail quality remains open. Final experimental source/evidence retained privately.
- G376 (main7dbc50b8/PR32): shared exact semantic/LOD-node/palette mapper consumed by the existing guarded lamp pilot; all60 source catalogue records independently match. Clippy/29 CPU tests pass, including88packages/264bundles/2592sections; publication gate PASS699tests/Clippy/build/render; no new glow or normal default change.
- G374 (main7dbc50b8/PR32 diagnostic): bounded current CPU mesh observer joins exact consecutive temporal captures; final combined-feature Clippy/14 CPU regressions pass, one actual native GPU candidate pair passes with148 stable CPU shapes; mandatory gate PASS699tests/Clippy/build/render. Antenna fix remains open.
- G373 (source research): saved lamp jointID-to-runtime Parts identity and postprocess effect/group dispatch verified; semantic section mapping, live glow and typed effect/colour-domain consumers remain open. No renderer/default change.
- G372 (mainf1000b77/PR30): shared emissive resource/material/sampler matching recovered; all44models/176packages census. General owned association adapter and source-atlas pilot lookup pass10 CPU tests/Clippy, including88packages/404materials; required package gate PASS688tests/Clippy/build/render; no new glow/default or completed-night claim.
- G371 (main6d7b3ddd/PR29; diagnostic only): optional exact consecutive temporal capture after actual Update warmup readiness; combined-feature Clippy and8 CPU lifecycle/frame tests PASS; separate native history/raster matrix domains verified. No motion-vector algorithm change/antenna fix; actual2RAW/4HDR consecutive native GPU pair PASS with exact history/body joins; final current-source package gate PASS683tests/Clippy/build/render; antenna quality remains open.
- G369 (main3498713d/PR28; optional trial): shared original-cookie rectangular/linear headlight trial, explicit units/snapshot and actual all-slot presentation; combined-feature Clippy PASS29.17s after preserved compiler findings. 25CPU/ECS tests PASS; three solo and two native16-car LeMans night GPU arms PASS with visible source-cookie lighting and exact control rows. Normal defaults/full-night integration remain open; mandatory FULL gate PASS1433tests/24soaks/Clippy/build/render/four identities. No original-window access.
- G367/G368 (mainbfcba080, PR27): shared model-derived headlight geometry passes six retained regressions and required670-test gate/build/render; postprocess named texture/CPU-backing contracts documented, GPU colour-domain boundary remains open.
- G365 (mainbfcba080, PR27; trials default off): final screenshots wait for shared pipeline warm-up Done; actual phase/capture regression included in670-test passing gate; five warm-up-preserving final captures PASS. No warm-up algorithm change.
- G364 (mainbfcba080, PR27; trials default off): coherent source-lighting trial applied after recovered static/timeline byte-scale binding;feature Clippy and11 new CPU/ECS tests PASS; required670-test gate/build/render plus feature Clippy andfive matched night/day captures PASS. One explicit authored unit, same event clock, no normal default change; sun/Fill/Phong updated together, source directions/CSM/reflections unchanged.
- G363 (mainbfcba080, PR27; trials default off): shared source-point headlight consumer staged as default-off bounded trial; six actual CPU consumer tests plus one purchased night/day Fill fixture and feature Clippy PASS. Required crate gate PASS657 tests/Clippy/build/Washington render with explicit test stack/thread settings; ordinary300tick production driving registration PASS (one windowless frame, pilot off). General night radiometry, optical quality and original channel/timing joins remain open; no visual default change.
- G362 (main7e92e024, PR26): owner prefers the ordinary tone/sky/car colour in matched images. Both normal launchers restore baseline defaults; Reinhard/.9 remains optional. More frequent matched visual comparisons recorded in AGENTS.md; no renderer or smoke-fix reversal.
- G361 (main2b49aff3, PR25 research): shared crowd interleaved descriptor layout recovered: stride32, Instanced3 offset24. GPU normalization/variant writer and animation clock/pose remain open; no renderer change.
- G360 (main1344c24e, PR24): shared normal-view Reinhard/.9, settings/menu forwarding and HDR coverage fix; fresh F717 default649 tests/build/render and feature Clippy pass; earlier feature669 suite, matched MSAA4/DLAA pairs and positive BMW/Istanbul HDR4 smoke pass. Shared display controls remain available; forced Reinhard/.9 defaults are withdrawn in G362 after owner comparison. Exact old enhanced EV13 look and broad visual parity are not claimed.
- G356 (main190c8012, PR20): shared source headlight point diagnostic uses original pair/default/offset rules;14controls and6model data reports PASS. Runtime slot/loader/presentation join and beam optics remain open; no illumination change.
- G355 (main90d30de3, PR19 research): registered crowd NODE/JOINTNODE copy and finalise/update eligibility documented; existing rig preserves helper locals. Actual clock consumer/current child state still unknown. No renderer/reset/audio change.
- G354 (main481bfe54, PR18 research): common road-response and lighting specular-colour contracts documented. Infield/scalars do not identify grass; saved material response composition remains open. No appearance change.
- G353 (main299b3e0f, PR17): per-model tread readiness/normal binding applied for mixed fields; feature Clippy and actual mixed-model CPU regression PASS; required grid-app gate PASS636tests/Clippy; bounded mixed-field GPU integration PASS with exact independent Lac/BMW uploads and STK refusal; wheel quality/all-track rendering open. No enhanced-guard removal.
- G352 (main de791263, PR16): shared regional/generic crowd metadata resolver now consumed by the existing diagnostic preview; 27 metadata controls plus consumer preservation check PASS on fresh main; full1388tests/4identities/24soaks PASS. Normal animated crowd rendering/clock/LOD remain open.
- G351 (main de791263, PR16): general optional-land admission applied; valid metadata-only supplements can be empty while required/nonempty geometry stays strict. Old-base Clippy/27formats/1app PASS; latest main integrated with one strict predicate for append/surface/water, merged feature adapters and full1388tests/4identities/24soaks PASS. No rendered/profile expansion claim.
- G350 (main de791263, PR16): shared all-slot headlight activation applied; Le Mans retains its clock, ordinary ForcedNight gains state. Combined-feature Clippy and two real-ECS regressions PASS; no playable/GPU beam check. Beam rendering/optics/night quality remain open. Four dash consumer reads only; Claude overlap recorded.
- G345/G349 (main de791263, PR16): conservative-MV quality remains negative (antenna traces and93 foreground outliers). Revised fragment audit passes Clippy/14tests/release and one strict GPU join/pipeline attribution; physical mesh/triangle unknown, parked. Model-derived tread lookup compiled, broader rendering unverified. See NATIVE_DLAA/G349.
- G346/G348 (main de791263, PR16): Nov3 original capture confirms three current crowd banks share ROOTNODE/+1C callback and current actor/provider scene association. General all-track renderer remains open (existing preview Detroit-only); clock/reference and complete postprocess still open. Original settings restored exactly.
- G347 (2026-11-02): routine own-folder cleanup retired97 uncited superseded dumps (3.035GiB logical), deduplicated1273 files; cited evidence/runtime inputs retained. Four permission errors left unchanged. Nov3 also retired only old own4.08GiB Cargo cache; current cache/SDK/evidence preserved, about97GiB free afterward.
- G343 (2026-10-30): bounded default-off CPU submission observer passes561tests/Clippy/featurebuild and exact600tick12STK Off/On RGBA/player controls. SelectedRaw/Standard main+shadow calls observed; GPUcost/counts unknown. See PERFORMANCE.
- G342 (2026-10-30): ordinaryL4/S4 shared sampler and registeredprobe pass15selectedtests, workspace1295tests/Clippy and purchasedL4 195coordinates/49keys/980unchanged bytes. No gameplayclock/provider/pose claim; see CROWD_ANIMATION_RESEARCH.
- G341 (2026-10-30): centred antenna raw/HDR localizes extra traces to EarlyPostProcess before Motion;34covered shaft pixels have rigidMVp99.0143px. Priorframe strictcamera join inconclusive; approximate background transport compatible, no fix yet. See NATIVE_DLAA.
- G340 (2026-10-30): shared skid-layer crossfade removes authored Kind-change ends while preserving true contact breaks; F645/F646 integration passes561tests/Clippy/combined release and3freshGPU cases. Native player/contact/frame controls exact, zero safety recoveries. Sparse pixel residuals/remaining raggedness and later reduced-use proof open; delivered PR12/main52fb1447. See GROUND_EFFECTS_RESEARCH.
- G337 (2026-10-30): shared source mesh-family coverage replaces both opaque stand exceptions; selected Detroit/Donington baseline+nativeDLAA runs restore framing/net. FreshF644 integration passes555tests/Clippy/combined release and3GPU runs including BMWsmoke; delivered viaPR11/main47b0b33f. ExactCSMalpha<.5; nativequarter is adaptation; finepattern/glassaux/device limits remain. See STATIC_OBJECT_MATERIALS.
- G333 (2026-10-30): ornament visibility selection now installs once in shared setup, fixing hidden static reference levels without a LOD bypass.548tests/Clippy/combined release and3defaultLOD GPU runs pass; ordinary12-car player records/finalPNG equal G331. Foundation source coverage remains open; see TRACK_LOD.
- G331 (2026-10-29): source object_simple_mesh two-sided policy restored in shared loading; reverse Detroit fence visible again. Default547/feature555 tests, Clippy/release and two-sided plus ordinary12-car GPU checks pass. Static-reference ornament LOD lifecycle defect remains separate; see STATIC_OBJECT_MATERIALS.
- G329 (2026-10-29): general automatic smoke composition in rendered driving, no car/track whitelist; actual attachments select MSAA/HDR paths. Default544/feature552 tests+Clippy/release,4matched GPU pairs and internal5C/R pass; selected skid/glass/lamp ghosts attenuate. Broader damage/foreground/device/manual/performance limits remain; see TYRE_SMOKE_RESEARCH.
- G326 (completed2026-10-29): baseline DLAA black grass-card regression fixed by existing soft single-sample material;539tests/Clippy/release and exact two-arm450tick replay pass. Ordinary MSAA4 image byte-identical; no smoke/lighting/globaltemporal claim. See NATIVE_DLAA.
- G325 (2026-10-28): maintained main-EXE normal-menu DLAA and enhanced1920 glass launchers;538tests/Clippy/featurebuild, real menu→race native300ticks and fresh moving Control/Zero1800tick/90frame controls pass. ScopedZero reduces selected heater moire with softening; global/all-angle/manual acceptance stays open. Current smoke sources alreadymain; see NATIVE_DLAA/LACETTI_GRAPHICS_MATERIALS.
- G324 (2026-10-28): guarded glass sampling controls close535tests/default+combined Clippy/release and oldFresnel startup; same-wrapper bias0 improves selected heater detail, owner static approval only. Strict Off-Control neutrality fails and stays inconclusive. Main source delivered; CDev playable still83566e33-dirty/bd48 morning release. Shibuya dark sky/hard shadows recorded for later. See LACETTI_GRAPHICS_MATERIALS/GRAPHICS_DEFECTS.
- G311 (2026-10-27): one finite forearm sweep passes466app tests/Clippy/build,31Extract/3callbacks/5PNGs after preserved compile/lint failures. Coherent bend, exact neutral/final controls; native clock/full animation/default remain open. See CROWD_ANIMATION_RESEARCH.
- G310 (2026-10-27): same-artifact angular asphalt plus accepted gentle Motion passes2nativeDLAA1800tick controls and strict physical/ledger/motion equivalence; selected frames look continuous. Owner accepts moving road quality; antenna doubling remains open. R/C/multitrack/default acceptance open. See ASPHALT_PAINT_REFLECTION_CHECK.
- Graphics records: log/G*.md and docs/GRAPHICS_ROADMAP_RU.md; optional graphics builds remain separate from the general playable. Every circuit draws its own sky and layered road (F515, docs/SKY.md).
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
| [F816](log/F0816.md) | 2026-11-10 | Livery designer 3D pill carousel on fe3d |
| [F815](log/F0815.md) | 2026-11-10 | Car sun terms and glass shadow floor from materials.xml (row 100) |
| [F814](log/F0814.md) | 2026-11-10 | Race OSD lap-time stacker re-flow and offline layouts (Le Mans time left, best-lap popup) |
| [F813](log/F0813.md) | 2026-11-10 | Loading world cube reflection |
| [F812](log/F0812.md) | 2026-11-10 | Loading camera noise rotation |
| [F811](log/F0811.md) | 2026-11-10 | Pause Restart darkening; pause tone map blocked on row 77 |
| [F810](log/F0810.md) | 2026-11-10 | Results screen in the front-end art (row 69): fe3d materials, pause_root camera, row driver, DXT3 textures |
| [F809](log/F0809.md) | 2026-11-10 | GRID World garage scene and livery 3D preview: static research |
| [F808](log/F0808.md) | 2026-11-10 | Sponsor designer: slots, replace, remove, swap, best-sponsor fill |
| [F807](log/F0807.md) | 2026-11-10 | Livery designer carousels: clamp, dead end, Randomise |
| [F806](log/F0806.md) | 2026-11-10 | Reset lines: Haruna final boundaries disqualify; tall fence plates are not raised boxes |
| [F805](log/F0805.md) | 2026-11-10 | Recovery obstruction toggle decrements on the toggle tick (0x78EE70) |
| [F804](log/F0804.md) | 2026-11-10 | Pieces with several shapes break (compound bodies, B11520) |
| [F803](log/F0803.md) | 2026-11-10 | Tyre, whine and collision sound replayed |
| [F802](log/F0802.md) | 2026-11-10 | Car reflections: the original's material tuning and SpecularValues floors |
| [F801](log/F0801.md) | 2026-11-10 | Car reflections: the track's static map by default |
| [F800](log/F0800.md) | 2026-11-10 | cargo td for every package through the build lock |
| [F799](log/F0799.md) | 2026-11-10 | Team livery checked on a race car |
| [F798](log/F0798.md) | 2026-11-10 | Career team livery on the race cars |
| [F797](log/F0797.md) | 2026-11-10 | Replay sound and highlight monitors traced; engine sound replayed |
| [F796](log/F0796.md) | 2026-11-10 | Damage parts, glass, lights and shards replayed; crash replay tested with a real wreck |
| [F795](log/F0795.md) | 2026-11-10 | Flashback count across career multi-race events |
| [F794](log/F0794.md) | 2026-11-10 | Replay and Flashback round 5: automatic crash replay, Restart reload, bonus, TERMINAL flag, checkpoint heap |
| [F793](log/F0793.md) | 2026-11-10 | Career team sum: an AI car without a team counts for the player (0x660E90) |
| [F792](log/F0792.md) | 2026-11-10 | Career season leaderboards: model, save and Rankings pages |
| [F791](log/F0791.md) | 2026-11-10 | Loading world: glyph instances grafted, screen panels left to their screens |
| [F790](log/F0790.md) | 2026-11-10 | Career screen sounds and bars; window shots recheck |
| [F789](log/F0789.md) | 2026-11-10 | Several shape behaviours of one prop break; Shibuya route_2 diagnosis |
| [F788](log/F0788.md) | 2026-11-10 | Popup overlay sounds; the race pause sound has no caller |
| [F787](log/F0787.md) | 2026-11-10 | Front-end glyph grafting for the post-race screens (row 69) |
| [F786](log/F0786.md) | 2026-11-10 | Career rival tag writer |
| [F785](log/F0785.md) | 2026-11-10 | HUD: front-end colour streams, drift ring shake and roll-up, rival name tag |
| [F784](log/F0784.md) | 2026-11-10 | 3D loading world, pause bloom, race-intro bars scoped |
| [F783](log/F0783.md) | 2026-11-10 | Drift GP and Freestyle drift cars, scaled joined props, on_track chords and gate progress |
| [F782](log/F0782.md) | 2026-11-10 | HUD: team-mate name tag; original post-race art research (rows 68, 69) |
| [F781](log/F0781.md) | 2026-11-10 | Night level, tree sheet shadows, world clear colour |
| [F780](log/F0780.md) | 2026-11-10 | Replay/Flashback round 4: terminal window from the crash, TerminalFX_01 wreck grade, linear PPS blend, packed solver rows |
| [F779](log/F0779.md) | 2026-11-10 | Front-end hand-off in the one-window host, loading bars, circle and sounds, pause art findings |
| [F778](log/F0778.md) | 2026-11-10 | Replay and Flashback round 3: TERMINAL cameras, camera defaults, packed checkpoints |
| [F777](log/F0777.md) | 2026-11-10 | HUD: track_mask minimap, world-record pill, drift OSD groups |
| [F776](log/F0776.md) | 2026-11-10 | Night lights: street lights, night shader term and car headlight beams on every night route |
| [F775](log/F0775.md) | 2026-11-10 | Replay frame compaction and the Flashback transition effects |
| [F774](log/F0774.md) | 2026-11-10 | Career season end, Le Mans entry, team name and livery (row 60) |
| [F773](log/F0773.md) | 2026-11-10 | Launcher race to menu hand-off; window-show hang and ready-marker fixes |
| [F772](log/F0772.md) | 2026-11-10 | Menu and screen sounds; speech race and event win counts |
| [F771](log/F0771.md) | 2026-11-10 | Career GRID World front end: pages, offers, rewards, supervised loop |
| [F770](log/F0770.md) | 2026-11-10 | HUD: touge penalty pill, brake light, wreck delay; name tags, boost and stacker traced |
| [F769](log/F0769.md) | 2026-11-10 | Replayed skid marks, smoke and debris; replay slow motion and frame step; Flashback drops the abandoned future's effects |
| [F768](log/F0768.md) | 2026-11-09 | Drift Battle: original drift field, unread drift settings, drift soak |
| [F767](log/F0767.md) | 2026-11-09 | One-window front end: menu, loading screen and race in one process |
| [F766](log/F0766.md) | 2026-11-09 | Crowd gasps and the AI engine Treble low-pass (rows 70, 72) |
| [F765](log/F0765.md) | 2026-11-09 | Graphics audit of all routes; night and container emissive objects drawn |
| [F764](log/F0764.md) | 2026-11-09 | Flashback rewinds the marshal flags; career/free-race set-up labels |
| [F763](log/F0763.md) | 2026-11-09 | Race HUD: split pills, needle blur, PB pill, OSD and results time formats from static RE |
| [F762](log/F0762.md) | 2026-11-09 | Player reset job ported 1:1 (row 43) |
| [F761](log/F0761.md) | 2026-11-09 | Ambient fill at the original's weight: compensate Bevy's 0.45 diffuse ambient response (row 125) |
| [F760](log/F0760.md) | 2026-11-09 | Graphics route audit: every raceable route, autopilot lap, chase and head-cam verdicts |
| [F759](log/F0759.md) | 2026-11-09 | Circuit lap estimator 0x67FFB0 and marshal flags (rows 44, 47) |
| [F758](log/F0758.md) | 2026-11-09 | Haruna's vertical streaks are light-shaft cards |
| [F757](log/F0757.md) | 2026-11-09 | Car shadows in race views and Haruna's wet layer |

Older entries: [log/INDEX.md](log/INDEX.md).

<!-- log-index:end -->
