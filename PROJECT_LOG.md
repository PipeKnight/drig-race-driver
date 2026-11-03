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
- G361 (LOCAL research): shared crowd interleaved descriptor layout recovered: stride32, Instanced3 offset24. GPU normalization/variant writer and animation clock/pose remain open; no renderer change.
- G360 (main1344c24e, PR24): shared normal-view Reinhard/.9, settings/menu forwarding and HDR coverage fix; fresh F717 default649 tests/build/render and feature Clippy pass; earlier feature669 suite, matched MSAA4/DLAA pairs and positive BMW/Istanbul HDR4 smoke pass. Two normal launchers select Reinhard/.9; Delivered; owner rebuilds local playables. Exact old enhanced EV13 look and broad visual parity are not claimed.
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
- G319 (2026-10-28): graphics PR2 merged into main3d85e3c1. Matched native-DLAA glass history pair passes inputs; normal smoother than reset, so reset is not a fix. Fine glass/antenna shimmer open; scoped glass sampling diagnostic next. See LACETTI_GRAPHICS_MATERIALS.
- G318 (2026-10-28): integrated770ec ordinary artifact passes full1254tests/Clippy/four identities/24soak, fresh BMW Off/Live autopilot and real Window exit0. Fresh main260cc integrated and full1256-test gate/identities/24soak pass; main delivered via PR2/3d85e3c1; coordinator playable rebuild remains separate. Owner confirms gross glass contour gone on c833; fine overlap shimmer remains open. See TYRE_SMOKE_RESEARCH/LACETTI_GRAPHICS_MATERIALS.
- G316 (2026-10-28): latest contour feedback came from old AgX3503/no current-sort; both AgX/Reinhard now share checkedc833 and print build ID. Full contour acceptance stays open; no inward-pane hiding. Main integration gate is in progress separately. See LACETTI_GRAPHICS_MATERIALS.
- G313/G314 (2026-10-28): black rear-through-front contour traced to missing playable current sorting;4matched sun/sort controls show421..425m stale centres versus0error. Scoped manual automatic fix passes466tests/Clippy/release and actual Window; Reinhard launcher updated. C/R/all-angle feedback and main delivery remain open. See LACETTI_GRAPHICS_MATERIALS.
- G312 (2026-10-27): ordinary Reinhard admission corrected after actual exit1;466app tests/Clippy/release and independent package checks pass. Same private launcher updated; owner driving session exits0. Separate black windshield contour still open. AgX/default/.9saturation preserved. See ASPHALT_PAINT_REFLECTION_CHECK.
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
| [F717](log/F0717.md) | 2026-11-03 | Baseline track and infield specular from the original programs' Phong lobe |
| [F716](log/F0716.md) | 2026-11-03 | Baseline sun from the route's lighting.xml plus the original's back/ambient fill (owner's decision) |
| [F715](log/F0715.md) | 2026-11-03 | Dynamic prop parts tagged at spawn; static scenes hide unbroken pieces |
| [F714](log/F0714.md) | 2026-11-03 | F710 skinned-mesh render regression fixed; prepush render smoke |
| [F713](log/F0713.md) | 2026-11-03 | Ornament shadows fade across D (shadow proxies) |
| [F712](log/F0712.md) | 2026-11-03 | Baseline draws no source forest on any track; branch baseline-trees prepared |
| [F711](log/F0711.md) | 2026-11-03 | Row 99 recheck on main: the Le Mans route 0 R10 branch at 8,689 m and the route sweep hold |
| [F710](log/F0710.md) | 2026-11-03 | Barrier strips at impact: skinned damage meshes blend over their pieces' joints; the anchor offset is the body frame |
| [F709](log/F0709.md) | 2026-11-03 | track.vis decoded; Okutama raised grass is out of route reach; read_sun reads every lighting file |
| [F708](log/F0708.md) | 2026-11-03 | Car LOD level fades and remaining pop sources |
| [F707](log/F0707.md) | 2026-11-03 | Okutama grass per route: track.vis covers every grass patch; baseline-sun crops for the owner |
| [F706](log/F0706.md) | 2026-11-03 | Water and light shafts read the route's own lighting.xml (Haruna night routes) |
| [F705](log/F0705.md) | 2026-11-03 | Water and light shafts ported from the original's programs (rows 108, 107) |
| [F704](log/F0704.md) | 2026-11-03 | Playable e5e885362a85 for the PC (no APKs) |
| [F703](log/F0703.md) | 2026-11-02 | Ornament level and draw-distance fades (owner pop report) |
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

Older entries: [log/INDEX.md](log/INDEX.md).

<!-- log-index:end -->
