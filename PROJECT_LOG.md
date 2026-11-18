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
- G426 verifies matched native aerial motion on opposite Donington turns, three-owner preparation, controlled Full smoke, and coherent native reset/fallback. Shibuya Reduced live preparation and Detroit ordinary history comparisons pass. A general antenna/fence compositor and owned optical coverage remain open.
- [Current graphics completion criteria](docs/GRAPHICS_ROADMAP_RU.md) cover DLAA detail, crowd, surface marks, delivery, original postprocessing and night lighting.
- G412 connects live exterior lamp emission to each source physical damage cluster. Real asymmetric damage and ordinary intact controls pass. See [night lamps](docs/NIGHT_LIGHTS.md).
- G411 verifies native stock Lacetti antenna ownership in opaque and depth/motion. Purchased aerials include both zero and positive reflection responses. See [thin antenna](docs/ANTENNA_TEMPORAL.md).
- G410 restores authored normals on the nonreflective DXT5 infield family. The purchased loader and matched Washington runtime pass. See [track surface](docs/SKY.md).
- G417 restores Automatic smoke with F836 frame timing in ordinary MSAA4 and native DLAA. Both the repaired smoke passes and GPU cascade measurements were observed.
- G425 enables shared native DLAA base RGBA filtering on ordinary textured rigid car glazing. Matched Donington, active Shibuya smoke and camera/restart checks pass, including the observed material-pipeline fallback. Antenna doubling and fence flicker remain open.
- G415 verifies reversible native thin-geometry routing in a clear fixture. G416 measures Stock rear reflection. G418/G420 verify real thin-detail phase owners. G422 verifies shared-group refusal and an intact stock boundary. G419 verifies controlled Reduced overlap. G421/G423 witness ordinary Reduced routing and local color contribution. G424 verifies one clear moving aerial. Broader motion and exact fence ownership remain open. See [thin antenna](docs/ANTENNA_TEMPORAL.md).
- G408 verifies ordinary spectator movement and flashes on Shibuya, two identical same-tick pause crops and restart/Flashback/replay lifecycle. See [crowd research](docs/CROWD_ANIMATION_RESEARCH.md).
- G407 restores the authored nonreflective track surface normal map in ordinary rendering, verified in forward, deferred and Native DLAA. See [track surface](docs/SKY.md).
- G406 fixes the source mesh coverage panic in deferred views by retaining its forward pass. Washington deferred and forward controls pass. See [static object materials](docs/STATIC_OBJECT_MATERIALS.md).
- G414 restores the tyre mark raw wheel-local point and pre-lift spacing, checked on grass, asphalt and Flashback.
- G398 and G400 restore travel-based skid seams and the original first quad/fade in live play and replay. Matched grass and mixed-field tests exercised both. See [Ground effects](docs/GROUND_EFFECTS_RESEARCH.md).
- G397 verifies cropped antenna coverage and the ordinary smoke tap. G403 verifies isolated Full smoke ordering on real shaft samples. Exact post-DLAA isolation and Reduced overlap remain open. See [thin antenna](docs/ANTENNA_TEMPORAL.md).
- G395 uses the source contact normal and surface-angle test for live and replay tyre marks. Matched native DLAA grass control has two fewer premature angular terminations, with the same contact gaps. Original visual comparison and strip cross-section remain open. See [Ground effects](docs/GROUND_EFFECTS_RESEARCH.md).
- G394 orders optional Type2 before SMAA and restores the source bloom exponent. G399 verifies the Le Mans effect with a fixed 16-car field. G401 orders Flashback Balance before SMAA. Original transfer remains open. See [original postprocess](docs/ORIGINAL_POSTPROCESS.md).
- G393 adds source idle standing course officials in ordinary races. Matched Donington and Detroit captures show pose animation. G402 resets the shared flag producer on Race Restart. G404 adds authored flag roles, motion and rigid rods. Exact cloth and native cull phase remain open. See [crowd research](docs/CROWD_ANIMATION_RESEARCH.md).
- G392 restores each vehicle bank's source skid normal atlas and the two-UV signed basis. Donington native DLAA, mixed Detroit baseline and active BMW flashback checks passed. Direct original comparison for grass gaps is open. See [Ground effects](docs/GROUND_EFFECTS_RESEARCH.md).
- G390 follows the source timeline sun direction. G399 verifies direction and car beams in fixed-field Le Mans pairs and ordinary Haruna day/night checks. G405 connects ordinary exterior lamp materials to source route requests. See [Le Mans](docs/LE_MANS.md).
- G389 corrects sky motion in the shared prepass. Actual native DLAA p99 reprojection error drops from 1.311 to 0.015 px in the audited background ROI. Production Win release and camera/restart checks passed; the separate antenna trail remains open. See [Native DLAA](docs/NATIVE_DLAA.md).
- G388 (main cf8c3b15/PR44) adds source walkers and moving marshals to ordinary races. Matched Milan/Yokohama captures, restart/replay lifecycle, 1,776 tests, four unchanged identities and 24 quick soaks passed. Source audio, camera flashes and near standing crowds are on main. Exact native cull re-entry timing and flag officials remain open.
- G384/G385 (main PR41/PR42) cover per-vehicle mark banks and replay continuity, source-driven night intensity, and the optional Type2 postprocess. Native colour-transfer confirmation and the Shibuya exposure mismatch remain open. The 4x and external-motion DLAA trials did not fix the antenna trail.
- G376 (main7dbc50b8/PR32): shared exact semantic/LOD-node/palette mapper consumed by the existing guarded lamp pilot; all60 source catalogue records independently match. Clippy/29 CPU tests pass, including88packages/264bundles/2592sections; publication gate PASS699tests/Clippy/build/render; no new glow or normal default change.
- G374 (main7dbc50b8/PR32 diagnostic): bounded current CPU mesh observer joins exact consecutive temporal captures; final combined-feature Clippy/14 CPU regressions pass, one actual native GPU candidate pair passes with148 stable CPU shapes; mandatory gate PASS699tests/Clippy/build/render. Antenna fix remains open.
- G373 (source research): saved lamp jointID-to-runtime Parts identity and postprocess effect/group dispatch verified; semantic section mapping, live glow and typed effect/colour-domain consumers remain open. No renderer/default change.
- G372 (mainf1000b77/PR30): shared emissive resource/material/sampler matching recovered; all44models/176packages census. General owned association adapter and source-atlas pilot lookup pass10 CPU tests/Clippy, including88packages/404materials; required package gate PASS688tests/Clippy/build/render; no new glow/default or completed-night claim.
- G371 records optional consecutive DLAA captures with verified warmup readiness. See the individual log.
- G369 (main3498713d/PR28; optional trial): shared original-cookie rectangular/linear headlight trial, explicit units/snapshot and actual all-slot presentation; combined-feature Clippy PASS29.17s after preserved compiler findings. 25CPU/ECS tests PASS; three solo and two native16-car LeMans night GPU arms PASS with visible source-cookie lighting and exact control rows. Normal defaults/full-night integration remain open; mandatory FULL gate PASS1433tests/24soaks/Clippy/build/render/four identities. No original-window access.
- G367/G368 (mainbfcba080, PR27): shared model-derived headlight geometry passes six retained regressions and required670-test gate/build/render; postprocess named texture/CPU-backing contracts documented, GPU colour-domain boundary remains open.
- G363–G365 (PR27): validated source headlight and lighting trials, plus warmup-aware captures. See individual logs.
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
- F776/F781 (in main) render street and vehicle lights from route and car data and apply the baseline
  night level and Le Mans timeline lighting. G384 resumes enhanced time-of-day and its verification.
- Open: the original post-process chain (row 77), the enhanced
  profile only for Lacetti on Donington (row 78), strobing wheels (row 81).
- Earlier foundations are indexed in G379–G383 for DLAA hints, crowd readers, lamp ownership and source lighting.

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
| [F848](log/F0848.md) | 2026-11-18 | Start hitch: blit pipelines pre-warmed, render thread yields to compiles under the cover |
| [F847](log/F0847.md) | 2026-11-18 | Car transparent draws: no merge without a visible change; reflection overlays bound the gain |
| [F846](log/F0846.md) | 2026-11-18 | Ground cover: stable billboard slots write only the changed instances |
| [F845](log/F0845.md) | 2026-11-18 | Main thread: skid marks write only the changed quads |
| [F844](log/F0844.md) | 2026-11-18 | Main thread: flashback checkpoints pack the live words on the compute pool |
| [F843](log/F0843.md) | 2026-11-18 | Main thread: skid-mark and smoke-boundary caches, slow-frame split |
| [F842](log/F0842.md) | 2026-11-17 | Prop breaks copy one switch per box, not every box |
| [F841](log/F0841.md) | 2026-11-17 | Tree facing: automatic CPU fallback decided from adapter limits, oversize meshes on the CPU |
| [F840](log/F0840.md) | 2026-11-17 | Frame-split gpu_begin poll: removing it only moves the work into submit |
| [F839](log/F0839.md) | 2026-11-17 | Main thread: parallel continuous sweeps, unchanged-damage skips, burst diagnostics |
| [F838](log/F0838.md) | 2026-11-17 | Tree billboards faced on the GPU: a compute pass writes the mesh slabs |
| [F837](log/F0837.md) | 2026-11-17 | Mesh uploads per frame: crowd cards, car batches and part meshes stream in place |
| [F836](log/F0836.md) | 2026-11-17 | Frame split in frame_stats for the Mac performance log |
| [F835](log/F0835.md) | 2026-11-16 | Fence through smoke: scaled 3D image shown opaque |
| [F834](log/F0834.md) | 2026-11-16 | Owner review 2026-11-16: Mac performance, accidental session, fence through smoke |
| [F833](log/F0833.md) | 2026-11-12 | Prepare GPLv3 licensing change for future DRIG versions |
| [F832](log/F0832.md) | 2026-11-12 | Discover installed DRIG resources for private macOS rebuilds |
| [F827](log/F0827.md) | 2026-11-11 | Playable 62992f28d9ec with F826 and APKs |
| [F826](log/F0826.md) | 2026-11-11 | Post-race stats screen in the front-end art |
| [F825](log/F0825.md) | 2026-11-11 | Results and drift screens: the pill strip in the front-end art |
| [F824](log/F0824.md) | 2026-11-11 | Shared front-end reflection cube helper |
| [F823](log/F0823.md) | 2026-11-10 | Drift scoreboard in the front-end art |
| [F822](log/F0822.md) | 2026-11-10 | Results screens slide in and out |
| [F821](log/F0821.md) | 2026-11-11 | Loading stat panel and particle emitter evidence |
| [F820](log/F0820.md) | 2026-11-10 | Front-end camera stick spring |
| [F819](log/F0819.md) | 2026-11-10 | Loading reflection box space (static evidence) |
| [F818](log/F0818.md) | 2026-11-11 | Car dust, scratch and damage gates and GhostCarValues |
| [F817](log/F0817.md) | 2026-11-10 | Tyre walls with a static back row break; lift_sunk skips bodies that do not collide with fixed geometry |
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

Older entries: [log/INDEX.md](log/INDEX.md).

<!-- log-index:end -->
