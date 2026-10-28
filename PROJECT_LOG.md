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
- G271 (2026-10-24): default-off bounded neutral C/T composition passes336app tests/Clippy, exact4pixelGPUblend probe and120stage/PNG joins. Smoke draws unchanged;116/120+finalexact, one absentrepeat reproduces4of5residualvalues, one remains unresolved. Composition foundation only, no strict image-neutrality/source-smoke/FPS/default promotion.
- G269 (2026-10-24): exact four animation-data leaf/parent pairs now enter production PSSG trees; B1/KA stay opaque. Full986tests/Clippy/4identities/24soak and production-linked five-bank282ordinary bit/reencode corpus pass. No evaluator, pose/clock/arm-motion or flash acceptance; private artifact-wrapper failure retained separately from successful gate.
- G268 (2026-10-24): two moving controls on pinnedG265 have exact61image pairs/traces, but zero repeat keys across69240sphere+360cylinder visits at real33..57m/s. Simple exact-key cache rejected for this driving case; geometry/upload-preserving alternatives remain open, no FPS gain claimed. Initial private timestep-parser failure retained and corrected before a fresh pair.
- G267 (2026-10-24): explicit mapped-only GroundFirst priority passes331app tests/Clippy and two matched120-frame GPU sequences. All120captured phases prove skid before queued smoke; native0074 sharp dark strips attenuate, while already-before0090 remains exactly unchanged. Order explains part of the defect; reduced composition and dark wheel rings remain open. Defaults/launchers unchanged.
- G266 (2026-10-24): bounded screenshot-entity ledger links all120actual Derby PNGs to Extract/phase/readback;329app tests/Clippy and3GPUprocesses pass. Skids follow part of queued smoke in62frames and precede all in58; queued empty groups and sparse pixel-repeat variability explicitly retained. No sort/material fix yet; next is a per-frame-guarded ground priority pilot.
- G265 (2026-10-24): default-off foliage CPU/key census retains every vertex expansion/upload.323app tests/Clippy pass; eight clean static GPU controls have identical final images. Actual577spherical+3cylindrical instances repeat exact keys while stationary; no cache, driving benefit or FPS improvement claimed. Earlier CPU-loop-disturbed timing is explicitly rejected. See docs/PERFORMANCE.md.
- G262–G264 (2026-10-24): matched Derby controls trace white skid-band glare through smoke to added PBR reflectance; selected source skid pass has no corresponding direct F0 lobe. Default removal passes the full gate (964tests/Clippy/4identities/24soak) and two ordinary-Off 120-frame GPU controls; white glare disappears, dark sharp marks remain open. G263 bounded ordinary L4/S4 reader also passes actual public-module282-packet fields/bits/reencode checks and100reject controls; no production bank admission or skeletal pose/animation acceptance.
- G258–G260 (2026-10-24): bounded native-DLAA fence captures preserve more near wire with Mask35, but distant stipple remains; ordinary Mask(.5) and accepted Motion stay unchanged. G258 required checks complete (942workspace/317feature tests, Clippy, identities and24soak); G259 source animation timer/packet bridge researched, not rendered arm motion. Default-off Detroit USA static crowd preview passes12focused tests/Clippy and renders visible spectators in2bounded views (G260); fresh-main full gate passes957tests/Clippy/4identities/24soak and6GPUprocesses, no animation/flash acceptance.
- G248 (2026-10-23): bounded skid-order control separates queue offset0 from preserved raster bias50. Native BMW smoke controls attenuate the protruding black curves; 302app tests/default+featureClippy and5GPUcontrols pass. Global strip-mesh limitation, broader/reset coverage and cost remain open; diagnostic defaultOff, accepted Motion unchanged.
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

Older entries: [log/INDEX.md](log/INDEX.md).

<!-- log-index:end -->
