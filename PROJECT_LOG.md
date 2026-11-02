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
- G319 (2026-10-28): graphics PR2 merged into main3d85e3c1. Matched native-DLAA glass history pair passes inputs; normal smoother than reset, so reset is not a fix. Fine glass/antenna shimmer open; scoped glass sampling diagnostic next. See LACETTI_GRAPHICS_MATERIALS.
- G318 (2026-10-28): integrated770ec ordinary artifact passes full1254tests/Clippy/four identities/24soak, fresh BMW Off/Live autopilot and real Window exit0. Fresh main260cc integrated and full1256-test gate/identities/24soak pass; main delivered via PR2/3d85e3c1; coordinator playable rebuild remains separate. Owner confirms gross glass contour gone on c833; fine overlap shimmer remains open. See TYRE_SMOKE_RESEARCH/LACETTI_GRAPHICS_MATERIALS.
- G316 (2026-10-28): latest contour feedback came from old AgX3503/no current-sort; both AgX/Reinhard now share checkedc833 and print build ID. Full contour acceptance stays open; no inward-pane hiding. Main integration gate is in progress separately. See LACETTI_GRAPHICS_MATERIALS.
- G313/G314 (2026-10-28): black rear-through-front contour traced to missing playable current sorting;4matched sun/sort controls show421..425m stale centres versus0error. Scoped manual automatic fix passes466tests/Clippy/release and actual Window; Reinhard launcher updated. C/R/all-angle feedback and main delivery remain open. See LACETTI_GRAPHICS_MATERIALS.
- G312 (2026-10-27): ordinary Reinhard admission corrected after actual exit1;466app tests/Clippy/release and independent package checks pass. Same private launcher updated; owner driving session exits0. Separate black windshield contour still open. AgX/default/.9saturation preserved. See ASPHALT_PAINT_REFLECTION_CHECK.
- G311 (2026-10-27): one finite forearm sweep passes466app tests/Clippy/build,31Extract/3callbacks/5PNGs after preserved compile/lint failures. Coherent bend, exact neutral/final controls; native clock/full animation/default remain open. See CROWD_ANIMATION_RESEARCH.
- G310 (2026-10-27): same-artifact angular asphalt plus accepted gentle Motion passes2nativeDLAA1800tick controls and strict physical/ledger/motion equivalence; selected frames look continuous. Owner accepts moving road quality; antenna doubling remains open. R/C/multitrack/default acceptance open. See ASPHALT_PAINT_REFLECTION_CHECK.
- G309 (2026-10-27): default-off angular asphalt F0 trial passes464app tests/Clippy/build and4static controls: restrained lowangle brightening, overhead PNG/RGB exact. Moving/Motion combination and original/default parity remain open. See ASPHALT_PAINT_REFLECTION_CHECK.
- G308 (2026-10-27): first stored S4 forearm-key hypothesis passes464app tests/Clippy/build and2static views;104vertices deform coherently and neutral PNG equals G307. Native DeltaYes/reference/clock/full animation remain open. See CROWD_ANIMATION_RESEARCH.
- G304 (2026-10-27): pinned F591/F592 integrated; full1139tests/Clippy/4identities/24soak, combined build and new2native BMW smoke controls pass with actually visible Off tyre curves attenuated in Late. Conditional cold-upload delivery/Manual/general/default remain open. See PERFORMANCE/TYRE_SMOKE_RESEARCH.
- G301 (2026-10-27): optional main-view AgX/Reinhard comparison passes460app tests/Clippy/combined-feature build and2native4K captures. Reinhard makes selected Lacetti paint more saturated; AgX default retained. One-component probe residual recorded; no original/moving/default parity claim. See ASPHALT_PAINT_REFLECTION_CHECK.
- G299 (2026-10-27): existing Lacetti Fresnel/current-sort native-DLAA fixture shows cracked glass before scripted R and intact glass/restored body afterward. One early-capture failure retained; corrected run and independent audit pass. Manual/Smashed/general remain open. See LACETTI_GRAPHICS_MATERIALS.
- G293 (2026-10-26): owner accepts the BMW/Istanbul baseline smoke fix; critical glass/lens ghosts resolved in this trial, minor appearance deferred. 438 app tests/Clippy, paired Image and actual Window cold-start/one-C/R checks pass. Owner log archived in full; Motion/Lacetti preserved. NativeDLAA/general/damage remain open; see TYRE_SMOKE_RESEARCH.
- G290 (2026-10-26): four native-DLAA HDR skid runs pass camera/reset/physics/runtime checks; selected tyre strips attenuate through smoke with clean local controls. Same-Off variation reproduces31 changed Control frames, but55 broad residual pairs remain unresolved. G291 verifies material compatibility; default promotion remains open; see TYRE_SMOKE_RESEARCH.
- G288 (2026-10-26): default-off native HDR skid replay now preserves the actual transparency dependency and refuses unsupported first-frame graph/condition states. Baseline26, native-DLAA27, full app433 tests and strict Clippy pass; release/GPU/visual smoke controls still pending. Accepted Motion unchanged. See TYRE_SMOKE_RESEARCH.
- G283 (2026-10-25): default-off stadium backing coverage passes418app tests/Clippy, six native GPU cases and two exact ordinary regressions after fixing late restoration. Owner likes recovered structural depth; lower points/fence visibility and moving/DLAA coverage remain open. See STATIC_OBJECT_MATERIALS.
- G281 (2026-10-25): default-off owned ground-skid early pass passes407app tests/Clippy and9native GPU runs across Derby/BMW/Lacetti camera+reset. Ground curves attenuate through smoke; bounded terminal/restoration and physics controls pass. Sparse Off/Control residuals remain unresolved, car/lens/net ghosts open; no enhanced/DLAA/default promotion. See TYRE_SMOKE_RESEARCH.
- G279 (2026-10-25): isolated default-off Detroit front-sheet alpha/depth experiment passes379app tests/Clippy and six native GPU arms. Off/Control images exact; eight source material components/two visible ready targets verified. Fine pattern remains; backingOpaque/fence fixed, no complete foundation/default promotion. Stored PNGsRGB expose no alpha; see STATIC_OBJECT_MATERIALS.
- G277 (2026-10-24): F568 main integrated; combined1028tests/Clippy/4physics identities/24soak and fresh BMW90/Derby120 GPU pairs pass. Recorded simulation/phase controls match G276; sparse unresolved pixel differences retained. F569 docs merged separately; dark wheel rings remain open, defaults/playable unchanged.
- G276 (2026-10-24): separately guarded BMW90 smoke boundary passes373app tests/Clippy, both90-frame GPU controls and same-exe Derby120 regression. Dense-cloud rear-glass/lens/skid ghosts attenuate with all17surfaces/physics preserved; dark wheel rings and damaged/general coverage remain open. DefaultOff/playable unchanged.
- G275 (2026-10-24): bounded default-off current camera/shadow foliage observer passes365app tests/Clippy. Moving controls have matching traces and zero hidden CPU-union visits with six probe views;150/151images exact, eight pixels unresolved despite exact Off repeat. Extra OSD camera invalidates private runner inventory assumption; original failure retained, no skip/FPS/default promotion.
- G274 (2026-10-24): default-off grouped-direct smoke boundary passes357app tests/Clippy and same-build120-frame GPU/stage/restoration proof. Native0065 driver safety net now attenuates through smoke;0074/0090 unchanged and dark wheel rings/BMW remain open. No default/playable or source-halfres promotion.
- G272 (2026-10-24): bounded frozen crowd-flash material validated after fixing source-sky sort bias;344 app tests/Clippy and3 GPU arms pass, exact Off/Control. Baseline Lacetti glass historical pair matches exactly. Animated scheduler and enhanced/probe regression remain open; no playable launcher changed.
- G269 (2026-10-24): exact four animation-data leaf/parent pairs now enter production PSSG trees; B1/KA stay opaque. Full986tests/Clippy/4identities/24soak and production-linked five-bank282ordinary bit/reencode corpus pass. No evaluator, pose/clock/arm-motion or flash acceptance; private artifact-wrapper failure retained separately from successful gate.
- G266 (2026-10-24): bounded screenshot-entity ledger links all120actual Derby PNGs to Extract/phase/readback;329app tests/Clippy and3GPUprocesses pass. Skids follow part of queued smoke in62frames and precede all in58; queued empty groups and sparse pixel-repeat variability explicitly retained. No sort/material fix yet; next is a per-frame-guarded ground priority pilot.
- G258–G260 (2026-10-24): bounded native-DLAA fence captures preserve more near wire with Mask35, but distant stipple remains; ordinary Mask(.5) and accepted Motion stay unchanged. G258 required checks complete (942workspace/317feature tests, Clippy, identities and24soak); G259 source animation timer/packet bridge researched, not rendered arm motion. Default-off Detroit USA static crowd preview passes12focused tests/Clippy and renders visible spectators in2bounded views (G260); fresh-main full gate passes957tests/Clippy/4identities/24soak and6GPUprocesses, no animation/flash acceptance.
- G243–G247 (2026-10-23): ordinary Derby surface controls now have a bounded source-F0 diagnostic; matched Detroit road captures reduce broad sheen, while terrain classification and grass remain under validation. BMW smoke reproduction separates stale car sort centres from indivisible smoke groups: per-sprite plus current-car sorting attenuates glass/lenses in selected native captures, preserving the cloud. Default modes/accepted Motion launcher unchanged; skid-through-smoke, cost, broader vehicle/reset coverage and original pass parity remain open. Finite-capacity OIT overflow is verified and that trial is rejected as a fix. Source reduced-pass research is in docs/TYRE_SMOKE_RESEARCH.md.
- G239/G240 (2026-10-23): real C/R coverage exercises five cameras and reset; source-proven bumper/bumper_reverse selected-model suppression restores unobstructed main view without moving the source camera. Main-camera visibility only; 286 tests/default+feature Clippy and matched native controls pass. Fresnel/current sorting remain bounded pilots; original per-pass/detached parity and manual input acceptance remain open. Dedicated rendering-performance agent is auditing grass, shadows and static-scene costs without lowering accepted quality. See docs/CAMERA_VIEWS.md.
- G237/G238 (2026-10-23): rear-through-front contour traced to stale retained transparent centres during movement. Default-off --window-sort current refreshes only player glass; sampled native-DLAA phase now has current centres/front-before-rear order without changing alpha/PBR/depth. Selected moving contours soften; owner-approved Fresnel and actual Cracked cases pass bounded captures. Accepted Motion launcher unchanged; manual R/C, rendered reset/Smashed, other cars and general promotion remain open. See docs/LACETTI_GRAPHICS_MATERIALS.md.
- G222 (2026-10-22): separate Motion DLAA comparison adds gentle directional blur after native DLAA, with original road texture sampling and unchanged defaults. 280app tests, default/feature Clippy,9+4GPU controls, paired complete89.037003031s laps and CD01 smoke pass. Near-stationary PNGs match exactly. G223 owner approves road uniformity and car clarity in Play GRID Graphics Motion DLAA.cmd. Rare under-car side-view trails are explicitly low priority; investigate the newly reported front-windscreen outline seen through rear glass. R/C-specific acceptance remains open.
- G220/G221: default-off gradual road RGB filtering is tested at native4K/virtual300; retained as an experiment, not an owner-approved fix. Strong earlier Asphalt DLAA blur was rejected and is not promoted. Sampled motion/depth supports the bounded road reprojection checks; this is not universal history correctness.
- G217/G218: Redgate floating fork was opaque decal.fx geometry. Source vertex/texture alpha and non-opaque shadows restore the faded marking. 285feature/280default tests, both Clippy variants, native4K/SMAA/DLAA and paired complete laps pass. Separate Decals DLAA comparison remains available; owner acceptance is open.
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
| [F702](log/F0702.md) | 2026-11-02 | Invisible barriers: anchor-only entity renderables draw their enclosing root's intact LOD on every route |
| [F701](log/F0701.md) | 2026-11-03 | No AI countersteer threshold in the original; with the same inputs the port's car yaws less |
| [F700](log/F0700.md) | 2026-11-02 | Publish macOS rebuild scripts and engine-only preview |
| [F699](log/F0699.md) | 2026-11-02 | Black drops and floating grass: god-ray cards not drawn as objects, land on every preset, water counted (rows 102, 107, 108) |
| [F698](log/F0698.md) | 2026-11-02 | Car glass: transparent items sort by their current position; the windscreen no longer shows over the rear window |
| [F697](log/F0697.md) | 2026-11-02 | Body events after a wake: the solved contact count, not the sum of both collections (Donington chaos world error) |
| [F696](log/F0696.md) | 2026-11-02 | Playable a8cef46b9294 with DLAA and APKs a8cef46b9294 |
| [F695](log/F0695.md) | 2026-11-02 | Old APKs are deleted when new ones are built |
| [F694](log/F0694.md) | 2026-11-02 | Release gate: paint-specular test missed the reflections field |
| [F693](log/F0693.md) | 2026-11-02 | Shibuya route 0 first-corner rise bisected: seed spread from a tick-1 steering difference; slide steering follows the ported law |
| [F692](log/F0692.md) | 2026-11-02 | Captured first-corner state injected into the port: contact and tyres reproduce the original |
| [F691](log/F0691.md) | 2026-11-02 | Continuous collision against route boxes and round primitives (AE34B0): the step-size question answered |
| [F690](log/F0690.md) | 2026-11-02 | Start-line matrix after F681: Yokohama Docks fields pass the drift-flag bases; Shibuya 0 losses predate F680 |
| [F689](log/F0689.md) | 2026-11-02 | Runtime float check of broken prop pieces; ornament and LOD gaps are the original's data (row 102) |
| [F688](log/F0688.md) | 2026-11-02 | Car reflections: the player's driver out of the cube; overlays of opaque parts before the glass; cracked glass as the original's program |
| [F687](log/F0687.md) | 2026-11-02 | Cockpit wheel turns at most 2 lock-fractions per second on the keyboard (owner request, row 105) |
| [F686](log/F0686.md) | 2026-11-02 | Pinned first-corner contact: port against the original capture; slip, yaw and gear in the field trace |
| [F685](log/F0685.md) | 2026-11-02 | All-route floating and texture audit; hidden damage pieces, raised pavement and mounted boards triaged (row 102) |
| [F684](log/F0684.md) | 2026-11-02 | Drivers' suit and helmet colours from the livery's driver_suit |
| [F683](log/F0683.md) | 2026-11-02 | Nürburgring grid-start capture: the original's leader slows by side contact |
| [F682](log/F0682.md) | 2026-11-02 | Camera FOV setting; cockpit view 10 degrees wider by default (owner request, row 103) |
| [F681](log/F0681.md) | 2026-10-31 | Close-drift flags take no contact outside the drift formats (row 104) |
| [F680](log/F0680.md) | 2026-10-31 | Box contacts beside long barrier boxes: an inconsistent distance result, the mesh frame and the pair direction (row 99) |
| [F679](log/F0679.md) | 2026-11-02 | Car reflections: the cube was black since F676 (faces not drawn); masked strength checked against the original |
| [F678](log/F0678.md) | 2026-10-31 | Cockpit: dash backlight at night |
| [F677](log/F0677.md) | 2026-10-31 | Sun shadows in the original's layout: three maps, the player car's map, shadow quality setting |
| [F676](log/F0676.md) | 2026-10-31 | Car reflections: paint masks, per-view cost, damage and level crops |
| [F675](log/F0675.md) | 2026-10-31 | Paused and Intro mix modes from the state flow; race music research (row 71 and the music sizing) |
| [F674](log/F0674.md) | 2026-10-31 | Nürburgring and Jarama first-corner losses: inside car clamped to the line beside a car; field trace detail |
| [F673](log/F0673.md) | 2026-10-31 | Doppler for the opponent engines and the crowd loops: the game's own CMStream_Update shift (backlog row 71) |
| [F672](log/F0672.md) | 2026-10-31 | Cockpit: drivers through the windows and in opponents' cars |
| [F671](log/F0671.md) | 2026-10-31 | Every car finishes on its own line crossing; Okutama routes 2 and 5 count both laps |
| [F670](log/F0670.md) | 2026-10-31 | Car reflections: dynamic cube and per-material response (off by default) |
| [F669](log/F0669.md) | 2026-10-31 | A wheel on a material without mechanics reads the original's initializer record (row 9) |
| [F668](log/F0668.md) | 2026-10-31 | Jarama route 1 autopilot finishes again; known-failing asset test restored |
| [F667](log/F0667.md) | 2026-10-31 | Crowds ported: NormalCrowd/GrandStandCrowd on the crowd cells, excited by ReactiveCrowd (backlog row 70) |
| [F666](log/F0666.md) | 2026-10-31 | Head-cam mouse look-around, an owner-requested deviation (backlog row 101) |
| [F665](log/F0665.md) | 2026-10-31 | Cockpit: driver g-force lean |
| [F664](log/F0664.md) | 2026-10-30 | Cockpit: gear-change and handbrake reaches, lever, H-pattern knob |
| [F663](log/F0663.md) | 2026-10-30 | Okutama loads: a template without an initial behaviour places an empty entity, as the original |
| [F662](log/F0662.md) | 2026-10-31 | Race manager settles most first-corner matrix losses; original Nürburgring first corner captured; in-air steering blend |
| [F661](log/F0661.md) | 2026-10-31 | Light clustering skipped without clusterables; minimap marker order fixed |
| [F660](log/F0660.md) | 2026-10-31 | Car reflections: the original's environment map and the plan |
| [F659](log/F0659.md) | 2026-10-30 | Long Beach route 1 leader: squeezed outside into a closing exit; line probe limits |
| [F658](log/F0658.md) | 2026-10-30 | Pack tick candidates: piece contacts already parallel, piece-island parallelism stopped |
| [F657](log/F0657.md) | 2026-10-30 | Owner playtest Viper Washington: car reflections, cockpit mouse look, floating track pieces (rows 100-102) |
| [F656](log/F0656.md) | 2026-10-30 | Location ambience ported: the track's AutoPlay field around the player (backlog row 70) |
| [F655](log/F0655.md) | 2026-10-30 | Washington pack tick profile: piece islands, not car contacts or basic boxes |
| [F654](log/F0654.md) | 2026-10-30 | Opponent dents: parallel projection, batched parts stay in the main world |
| [F653](log/F0653.md) | 2026-10-30 | Barrier collision sweep: basic entities collide (fences, armco, walls on every route) |
| [F652](log/F0652.md) | 2026-10-30 | Sun shadows: casters culled per cascade by receiver, shadow passes timed, the original's cascades traced |
| [F651](log/F0651.md) | 2026-10-30 | First-corner matrix with breakable props; start-line routes; flag bases |
| [F650](log/F0650.md) | 2026-10-30 | Cockpit: the wheel follows the front wheels' applied steering (row 94) |
| [F649](log/F0649.md) | 2026-10-30 | Shibuya start swerve: the start racing line before the start line |
| [F648](log/F0648.md) | 2026-10-30 | Collision and damage loops no longer hold after a wreck, the race end or a pause (backlog row 95) |
| [F647](log/F0647.md) | 2026-10-30 | Cockpit: pedals and the general interior clip animator |
| [F646](log/F0646.md) | 2026-10-30 | Playable dc4408b43366 with DLAA and APKs dc4408b43366 |
| [F645](log/F0645.md) | 2026-10-30 | Tyre smoke drawn at half resolution with the original's soft fade |
| [F644](log/F0644.md) | 2026-10-30 | Tyre smoke lit once per particle as the original |
| [F643](log/F0643.md) | 2026-10-30 | Tyre smoke rendering cost measured (SMOKE_RENDER_COST.md) |

Older entries: [log/INDEX.md](log/INDEX.md).

<!-- log-index:end -->
