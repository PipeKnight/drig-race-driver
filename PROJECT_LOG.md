# DRIG project log

The project's current state, open problems and next actions, plus an index of recent entries.
The entries themselves are one file each in [log/](log/INDEX.md): `log/F0532.md` holds F532.
Find details with `grep -rn "F512 " log/` or `grep -rln "Redgate" log/`; do not read all of log/.
Write new entries with `py -I tools/log_entry.py new "Title"` and run `check --fix` before every
push (AGENTS.md "Durable records"). Edit the state sections below only when a status changes.
Until 2026-10-21 (F532) the whole log was this one file; `git show d927d415:PROJECT_LOG.md` has it.

## Current state (2026-10-21, after F531)

Playable: `target/release/grid-app.exe` e57f2fd391e2 from source 6c8b4fad (origin/main a11cddf7 merged
locally), the portable copy and APKs a11cddf7de27 (F531). Work paused for several days (F531);
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
- G408/G427 verify ordinary spectator movement, flashes and replay lifecycle. G427 restores the flash schedule after Flashback. See [crowd research](docs/CROWD_ANIMATION_RESEARCH.md).
- G407 restores the authored nonreflective track surface normal map in ordinary rendering, verified in forward, deferred and Native DLAA. See [track surface](docs/SKY.md).
- G406 fixes the source mesh coverage panic in deferred views by retaining its forward pass. Washington deferred and forward controls pass. See [static object materials](docs/STATIC_OBJECT_MATERIALS.md).
- G414 restores the tyre mark raw wheel-local point and pre-lift spacing, checked on grass, asphalt and Flashback.
- G398 and G400 restore travel-based skid seams and the original first quad/fade in live play and replay. Matched grass and mixed-field tests exercised both. See [Ground effects](docs/GROUND_EFFECTS_RESEARCH.md).
- G397 verifies cropped antenna coverage and the ordinary smoke tap. G403 verifies isolated Full smoke ordering on real shaft samples. Exact post-DLAA isolation and Reduced overlap remain open. See [thin antenna](docs/ANTENNA_TEMPORAL.md).
- G395 uses the source contact normal and surface-angle test for live and replay tyre marks. Matched native DLAA grass control has two fewer premature angular terminations, with the same contact gaps. Original visual comparison and strip cross-section remain open. See [Ground effects](docs/GROUND_EFFECTS_RESEARCH.md).
- G394 orders optional Type2 before SMAA and restores the source bloom exponent. G399 verifies the Le Mans effect with a fixed 16-car field. G401 orders Flashback Balance before SMAA. G428 records the original Shibuya frame. See [original postprocess](docs/ORIGINAL_POSTPROCESS.md).
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
- v0.1.0 published 2026-11-24 (F909): GitHub Latest release, tag on 1ed1b050; Windows exe, Linux tar.gz and AppImage; the macOS DMG follows.
  Windows player build F906: one exe without the `dev-tools` feature, portable
  `DRIG-data` beside the exe, player flags (`--help`, docs/SETTINGS.md section 6), exe, notices .txt and SHA256SUMS (no zip) from
  `tools/package_desktop.ps1`. Open: career not in the menu, career window preferences (row 147), developer flags still parse, in-app update.
- macOS F577: native arm64 release build and code-only `DRIG.app` packaging pass
  on macOS 15.7.7, with no engine changes. CLI help and the menu's expected
  missing-installation exit pass; Finder launch, graphics, input, audio and
  actual races are not tested (no game data). See [docs/MACOS.md](docs/MACOS.md).
- Android P9–P12: code-only APK importing the user's own GRID folder (F405), bundled event APKs
  (F418), touch/tilt/pad controls (P11, F404), per-system profiling (F395); docs/ANDROID.md.
- Open: lean default 33–38 FPS on a cool phone, throttled after 2 min to about 25 (cap 30: 28.6, 1.2 % frames ≥50 ms), dips are heavy views rounded to 50 ms (F933);
  MIUI votes the app to 60 Hz above its setFrameRate hint, only the phone's settings lift it; no sustained mode or Game Mode on MIUI (F933); Game Turbo game-list membership adds about 3.4 heavy GPU ms and ADPF opens no session (F948);
  pad rumble and phone vibration are not felt on the phone yet (row 22).
- Phone proxy (F919): `tools/phone_proxy.ps1 -Rev A -Compare B` predicts the Xiaomi frame delta from a
  desktop run in about 8 minutes (noise about 2 ms with 3 runs); scene-work gains are an upper bound.

### Tooling and process
- Build gate `tools/build_gate.ps1` and nightly soak (F524); shared seed build and rust-lld (F467);
  `tools/prune_build_dirs.ps1` (F438); grid-soak harness (docs/SOAK_TESTING.md); check levels and the
  identity check in docs/TOOLCHAIN.md.
- F913: GitHub Actions CI without game data (docs/TOOLCHAIN.md, "Continuous integration").
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

- Distribution rows 129-135 (F858): v0.1.0 published (F909); open: macOS DMG on the release, career window preferences (147), in-app update (131), career entry in the menu, Android single APK, the release/debug split in release_playable and the APK scripts (132).
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
| [F963](log/F0963.md) | 2026-11-27 | Phone GPU batch 7: mirror cost measured, mirror-only cut switches |
| [F962](log/F0962.md) | 2026-11-27 | Phone: pre-rotation pair, mirror cost in the cockpit, Continue fix confirmed |
| [F961](log/F0961.md) | 2026-11-26 | Phone GPU batch 6: the fast shadow filter becomes the mobile default |
| [F960](log/F0960.md) | 2026-11-26 | Phone A/B night: shadow map, fill light, mip skip, shadow filter, auto rows, cull, mirror |
| [F959](log/F0959.md) | 2026-11-26 | Metal GPU timestamps fixed (owner PR 87) |
| [F958](log/F0958.md) | 2026-11-26 | Mac traces of build 1546e211 (Donington, Istanbul): 120 Hz cadence judder |
| [F957](log/F0957.md) | 2026-11-26 | Owner report: choppy mix of several car engines in the chase camera (backlog row 154) |
| [F956](log/F0956.md) | 2026-11-26 | Phone GPU batch 5: mip skip 2 >=1024 + 1 becomes the mobile default |
| [F955](log/F0955.md) | 2026-11-26 | Phone GPU batch 4: shadow lookups cost ~5 ms; fast shadow filter probe |
| [F954](log/F0954.md) | 2026-11-26 | Disk audit covers all repositories and agent projects on the PC |
| [F953](log/F0953.md) | 2026-11-26 | Bundle list generator keeps played speech lines in the speech section |
| [F952](log/F0952.md) | 2026-11-26 | Disk audit by owner: tools/disk_audit.py |
| [F951](log/F0951.md) | 2026-11-26 | Android bundle lists regenerated for G435's route cloth files |
| [F950](log/F0950.md) | 2026-11-26 | Phone fill-light probe (partial): about 1 heavy ms |
| [F949](log/F0949.md) | 2026-11-26 | Token economy rules from the Claude session audit |
| [F948](log/F0948.md) | 2026-11-26 | Phone GPU batch 1: lighting, render360, shadows and tonemap win on the phone; Game Turbo game list costs 3.4 GPU ms; Continue fixed |
| [F947](log/F0947.md) | 2026-11-26 | Phone warm chain: main thread bound when warm; tonemap dropped |
| [F946](log/F0946.md) | 2026-11-26 | Phone batch 1 verdicts: lighting is the lights, not env maps; -Shadows bench probe |
| [F945](log/F0945.md) | 2026-11-26 | Claude Code token audit script and findings |
| [F944](log/F0944.md) | 2026-11-26 | Static driver figures off by default until the phone A/B |
| [F943](log/F0943.md) | 2026-11-26 | Android CPU batch 2: lazy Transit, coalesced mesh upload, static driver figures |
| [F942](log/F0942.md) | 2026-11-26 | Phone mip skip: owner's combined look (mipskip2_1024+1) with a 64 px floor |
| [F941](log/F0941.md) | 2026-11-26 | AGENTS.md: Codex waits use one long yield, no 1-second polling |
| [F940](log/F0940.md) | 2026-11-26 | Codex audit reads code-mode exec cells and wait calls |
| [F939](log/F0939.md) | 2026-11-26 | Codex usage audit script in tools/; Codex defaults updated |
| [F938](log/F0938.md) | 2026-11-26 | Codex working rules in AGENTS.md |
| [F937](log/F0937.md) | 2026-11-26 | Owner usage figures removed from the repo |
| [F936](log/F0936.md) | 2026-11-26 | Phone GPU batch 2: texture mip skip at upload (mipskipN_M) |
| [F935](log/F0935.md) | 2026-11-26 | Android CPU batch 1: thread placement, thermal governor, parallel encoder finish and ADPF target switches |
| [F934](log/F0934.md) | 2026-11-26 | Phone GPU batch 1: size cull, tonemap and ADPF switches, GPU clock sampler |
| [F933](log/F0933.md) | 2026-11-26 | Phone: lean default verified, dip anatomy, MIUI 60 Hz vote, physics stage split, thermal soaks (cap 30, sustained mode), bench env switches |
| [F932](log/F0932.md) | 2026-11-26 | Dynamic resolution: a GPU-bound frame time in the hold band counts as late (Mac countdown) |
| [F931](log/F0931.md) | 2026-11-26 | Steam Deck estimate from E-core and UHD 770 stand-ins |
| [F930](log/F0930.md) | 2026-11-26 | frame_split GPU readback: no slot reuse with a map outstanding (Mac crash) |
| [F929](log/F0929.md) | 2026-11-25 | Mac logs 22:26: GPU-query crash is a frame_split readback bug; countdown GPU-bound at the ceiling; memory pressure ruled out |
| [F928](log/F0928.md) | 2026-11-25 | Lean render profile as the Android default; 120 Hz pacing A/B |
| [F927](log/F0927.md) | 2026-11-25 | Phone GPU split rows and Android GPU research |
| [F926](log/F0926.md) | 2026-11-25 | Mac in-race dips: camera-flash pipelines under the warm-up cover, process counters per window |
| [F925](log/F0925.md) | 2026-11-25 | Performance test switches: owner phone run 080140 never changed them; order-proof apply and a front-end perf log line |
| [F924](log/F0924.md) | 2026-11-25 | Mac frame-dip capture steps: redirect a Terminal start to a file, developer build only |
| [F923](log/F0923.md) | 2026-11-25 | Mac frame dips: cockpit mirror pipelines warmed under the cover |
| [F922](log/F0922.md) | 2026-11-25 | GPU unit tests run in their own process: NVIDIA D3D12 driver crash in the grid-app test process |
| [F921](log/F0921.md) | 2026-11-25 | Android: Viper cockpit wheel fixed (file name case); tyre walls after a hit diagnosed, not fixed |
| [F920](log/F0920.md) | 2026-11-24 | Performance test: single-threaded executor switches for the main and render worlds |
| [F919](log/F0919.md) | 2026-11-25 | Phone proxy: predicted Xiaomi frame from a phone-like desktop run |
| [F918](log/F0918.md) | 2026-11-25 | Unified per-run logs on all platforms |
| [F917](log/F0917.md) | 2026-11-24 | CI: Intel-only SSE host comparison, vendor-independent digest test |
| [F916](log/F0916.md) | 2026-11-24 | Keep macOS player packaging and portable race logs |
| [F915](log/F0915.md) | 2026-11-24 | Xiaomi Performance test A/B: shadows and scene off gain nothing, fixed per-frame CPU overhead wins |
| [F914](log/F0914.md) | 2026-11-24 | CI badge in the README |
| [F913](log/F0913.md) | 2026-11-24 | GitHub Actions CI: Linux Clippy and data-free tests, Windows Clippy |
| [F912](log/F0912.md) | 2026-11-24 | Xiaomi profile APK 57c966e1: where the 38 ms phone frame goes |
| [F911](log/F0911.md) | 2026-11-24 | README graphics line reworded; ROADMAP optional graphics entry |
| [F910](log/F0910.md) | 2026-11-24 | README for players: download, bug reports, roadmap file |
| [F909](log/F0909.md) | 2026-11-24 | Release v0.1.0 published |
| [F908](log/F0908.md) | 2026-11-24 | Linux v0.1.0 release packages from 1ed1b050 and their WSLg smoke |
| [F907](log/F0907.md) | 2026-11-24 | Linux v0.1.0 packages from e4e5b0c9; --game-root fix for player starts |
| [F906](log/F0906.md) | 2026-11-24 | DRIG v0.1.0 Windows player build: portable DRIG-data, dev-tools feature, player flags, exe release |
| [F905](log/F0905.md) | 2026-11-24 | Atmosphere and SSAO bind groups through the bind-group cache |
| [F904](log/F0904.md) | 2026-11-24 | Backlog: DualSense haptics row 146, release check note in row 131, no-zip release rule |

Older entries: [log/INDEX.md](log/INDEX.md).

<!-- log-index:end -->
