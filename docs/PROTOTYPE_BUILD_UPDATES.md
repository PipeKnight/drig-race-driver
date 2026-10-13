# Playable build updates

Latest verified refresh: **d221dc28f774**, built 13 October at20:36:32 UTC. Ten source-driving and five Bevy checks pass, plus rendered collision and straight-road checks. The Body Contacts launcher enables physical grid/shape/wheel deformation and independently mapped body dents when its private DF01 input exists. Original graphical damage fidelity remains under research.

The owner requested periodic prototype updates on 13 October 2026. After verified driving changes, rebuild the release app and keep **Play GRID Prototype.cmd** and **Play GRID Body Contacts.cmd** using that executable. Preserve an active owner session and continue offline work until the app is closed.

`tools/update_prototype.ps1` checks the purchased-source driving tests and Bevy input/presentation/recovery tests, then builds the app offline using the existing Rust toolchain. It embeds the source revision in the window title and startup log. A build made with uncommitted changes is marked `dirty`; a plain Cargo build without this variable says `unversioned`.

Run `powershell -ExecutionPolicy Bypass -File tools/update_prototype.ps1` from the workspace. The helper reuses Rust from PATH or the existing standard user Cargo directory; it installs nothing. It restores its temporary environment variables afterward.

Every successful update writes ignored `local/playtest/build-UTCSTAMP/build.json` and logs. `local/playtest/current-build.json` points to the latest successfully checked build and records the full revision, executable hash, tool versions and checks. Rendering and owner laps are separately verified; a successful rebuild alone does not establish them. Game assets and compiled binaries remain private.

Use the existing launchers to try the current build. Changes that affect driving or limitations are recorded in `PROJECT_LOG.md` and linked research notes; documentation-only commits need not trigger a rebuild. The current prototype supports the owner-confirmed keyboard lap, reverse and Space handbrake. Optional body contacts, [selected registered body events](BODY_IMPACT_RUNTIME.md), [collision-driven deformation](DEFORMATION_DRIVING_INTEGRATION.md) and [independent body dents](BODY_DEFORMATION_PRESENTATION.md) are available through the body-contact launcher.

## Body-dent presentation refresh

Clean revision`d221dc28f77499221544e68ba71c101b4eda995c`, built`2026-10-13T20:36:32.3800787Z`; release SHA`61d9fc4c362512f845df10fe4189c35d6744089dd8fdd76593e87bedbec16b87`. Both launchers current; no active owner session replaced. Private build/checks manifest`local/playtest/build-20261013T2035533384757Z/`; ten source and five Bevy checks pass, including source mesh publication/failure/reset.

The600-tick collision render completes normally/no failures. All310 logged physical/deformation records match the prior rigid-body-graphicsd178 build exactly except wall time.81 source meshes update eight times, initially pristine at tick0; first changes at tick165 and last upload at175, with no unnecessary uploads afterward. First bulk/grid movement coincides with the first geometric body-contact sample at165; this is observed timing, not exclusive attribution to a particular crash. Image inspected; raster differences stay within the car/shadow region (29,239 pixels, bounds536,406..837,676), with normals/geometry/shadow effects not distinguished.

Private`body-dents-d221dc2-20261013.{txt,json,png}` log SHA`8dca61e827c32cf3f81c2659a955c67327cb5bea593e839394f0a020ed6cfdd1`, image SHA`ce25b339e4c8296bb48d35ab021803795e34a9d05d27de8949aacf84f7496ecc`. Reconstructable no-feedback report`body-dents-no-feedback-20261013-final-01.json` SHA`b4001861f5e6c0afa4ddafa0395068ff379db5e2f145a5cc75999028918d5b0d`; `tools/compare_driving_samples.py` checks all logged fields, not every authoritative byte or original rendering equivalence.

A separate300-tick/4.8-second straight-road run completes with21 samples/no failures, zero body contacts/bulk calls/records/grid displacement and unchanged event/CRT RNG. Its inspected image shows the car on the straight. Private`body-dents-straight-d221dc2-20261013` log SHA`144b7a497ef7a692336852d8b12f2ce3b9c88f6ba61afb71181a52c06feca8d5`, image SHA`4f63a0615c1694d78ad93485b7d1eccc824303f0bb7d022f30d28abf68cfd6a9`. This establishes no deformation in this unimpacted source scenario, not complete original-game calibration. Steam130 resources/executable still match at`2026-10-13T20:37:56.127017Z`; private preservation report`local/scene/preservation-20261013-body-dents-build.json`. Existing Pillow12.3.0 supplied read-only raster comparison; its deprecatedgetdata warning did not affect the result.

## Collision-driven deformation refresh

Clean revision`d178b80b77be5a27c07012c12c4bbe9b86b3ffca`, built`2026-10-13T20:10:41.6334699Z`; release SHA`0d435e45999f1dc17d57a5deab91418d93a5ffbd5ac9403c7df80ab10138c420`. Both launchers use this executable. No active owner session was replaced. Private manifest/checks/build logs: `local/playtest/build-20261013T2010082098230Z/`. Ten source tests and five Bevy tests pass; DF01 provenance is included in the manifest.

The600-tick rendered powered turn completes normally with155 sampled records/no simulation failures,292 geometric body contacts,124 bulk callbacks/166 records,334 moved XYZ grid positions/max.28555748. CRT seed1 stays unchanged in this run; event RNG endsab049764. The inspected image shows the rigid body beside the Donington barrier; it does not demonstrate visible body-panel dents. Log SHA`5eff162164eedd731b2d0c705858fdc9e0607c6ba92ce5ea638ff0c13e58ae8a`, image SHA`60c8515c522abf776dc61106506e6a396ab780c21f23e6c2e9addf6e25868f17`; ignored`local/playtest/deformation-driving-d178b80-20261013.{txt,json,png}`. Known gamepad80070057 warning remains separate.

Original Steam130 resources/executable still match at`2026-10-13T20:10:37.456794Z`, recorded in`local/scene/preservation-20261013-deformation-driving-build.json`. DF01 re-extraction matches all252 bytes, source address/file/zero-fill extents and SHA`5d48e35d630315b50a7e07450501e6e90d43311a8552bc98367c3e69f9c93292`; the only zero-filled field is the one-byteE85FAD flag. Longer unattended damage stress validation follows separately.

The3600-tick/57.6 simulated-second powered-turn stress also completes normally:2787 sampled records/no failures,2731 bulk calls/171 records,334 moved nodes/max.28555748, CRT1/event RNG2c18a246 at exit. The car reaches the pit barrier and remains blocked while throttle stays held; this is repeated-contact stability, **not a driven lap**. The image was inspected. Private`deformation-stress-d178b80-20261013.{txt,json,png}` log SHA`39ae20b2053f9aba2b8a359d4dd37d6ce2f9d8ad917044947ef051c657e7c9b8`, image SHA`50d5709cf898da2801eb36cec5648d33242a148e18802311470053e9911238e6`.

## Initial updater validation

The first helper run on 13 October passed all five purchased-source driving tests and all three Bevy tests, then rebuilt the release executable. Formatting, all-target workspace Clippy with warnings denied, PowerShell AST parsing and whitespace checks also passed. Private evidence is `local/playtest/build-20261012T2257094306737Z/`; its manifest labels the uncommitted build `fe5d9cd9836e-dirty` and records SHA-256 `7de798e1f864de582a7dc79b419d6834f688c0146275a1495dc6350fd1c7896a`. A subsequent committed-source build supersedes it through the current-build manifest. This update adds build identification and maintenance tooling, with no driving-physics change.

The initial tool inventory found Cargo and Rust absent from the shell PATH. Both were already installed in `C:\Users\debros\.cargo\bin`; the fallback reuses Cargo 1.99.0 and Rust 1.99.0. No installation was needed. Bevy remains 0.19.1.

## First versioned playable update

The clean committed-source build is **c7a38624b983** (full revision `c7a38624b98342c99860ae2d6fe04a20ca43c141`), built at `2026-10-12T22:59:24.6974411Z`. Release executable SHA-256 is `8f9b3cee26ce4c069766decb13a3cab3325dcd525e8829a506f907049eb2c41e`. The same five source and three Bevy checks passed for this build; private evidence and manifest are under `local/playtest/build-20261012T2258565096574Z/`.

Its 600-tick powered Donington barrier run completed with 132 telemetry samples and no recorded simulation failure. The screenshot was inspected: the Lacetti and circuit render, with the car beside the barrier. This is a bounded scripted check, not a new manual lap. Files are `local/playtest/build-c7a38624b983-20261013.{txt,json,png}`; log SHA-256 `401bcf36fafecf6f3adcda4fa9a60e8efcc406cb0f825edca5bd836c4d6108b3`, image SHA-256 `a6e4035fbbb380ede67c844feda9fa5e0ee6ea4ee438486063e3f2c7ffa5ccf0`. The existing gamepad error `80070057` recurs, while the scripted driving run completes. Keyboard controls remain the tested path.

Both existing launchers still point at `target/release/grid-app.exe`. Documentation and diagnostic-only commits after the recorded revision do not change this playable executable. No owner action or additional recording is needed for the current research step.

## Current build: registered impacts

The latest clean build is **3443c31c8365** (full revision `3443c31c8365c0b1b881b24a8a90acde67fd5d14`), built at `2026-10-13T08:23:22.5543404Z`, executable SHA-256 `5e9875d9679167321d894f4ea36b516674a55e548924d0724c4744236992f301`. Six purchased-source tests and all three Bevy tests pass. Private manifest/logs: `local/playtest/build-20261013T0823033172500Z/`; the current-build pointer identifies this executable.

Use **Play GRID Body Contacts.cmd** for experimental registered body impacts and physical barriers; `EVENTS` appears in the title. It supplies the prepared BP01 data, whose hash is recorded with CD01 in the build manifest. The ordinary launcher retains its existing mode. Details, validation and limitations are in [body-impact runtime](BODY_IMPACT_RUNTIME.md).

The refreshed executable completed another600-tick source barrier run with132 telemetry samples,176 body contacts and no simulation failure. The log confirms event mode and a damage RNG change from`ac7e1d7f` to`2a73550a`. Additive version1 telemetry now records event mode, owned RNG, raw impact flag, suspension gain and nine damage levels; the existing summarizer accepts it. The impact flag is retained state, not a counter of events in that sample. This makes subsequent owner sessions inspectable without guessing from appearances.

The rendered screenshot was inspected and shows the Lacetti beside the Donington barrier; it is byte-identical to the earlier bounded image, consistent with no visual-deformation implementation in this change. Files: `local/playtest/body-events-3443c31c8365-20261013.{txt,json,png}`. Log SHA-256 `345de04c133925fbfabc4050e93e291576b815bbf308a115f768d0d49a626ca6`; image SHA-256 `a6e4035fbbb380ede67c844feda9fa5e0ee6ea4ee438486063e3f2c7ffa5ccf0`. The known gamepad80070057 message recurs; the run completes. All130 traced Steam resources and the original executable remain unchanged at`2026-10-13T08:24:30.442921Z` (`local/scene/preservation-20261013-body-event-runtime.json`).

An intermediate clean build`de6a9c6ada7c` also completed the600-tick event-enabled render. It was superseded by the additive telemetry update; both historical manifests and renders are retained. No active owner session was replaced or closed. No manual lap or original-game calibration is inferred from these scripted runs.

## Source deformation and collision-shape preparation refresh

Clean **c3696050d8f6** (full `c3696050d8f6817e75872f3981696bfc3897a042`), built `2026-10-13T16:09:05.0609108Z`, executable SHA `70fdd84c5711bab4e56061e8844bef18bc523178e4d74b374d6ad3a34c45b328`. Private build manifest `local/playtest/build-20261013T1608145415745Z/`. Eight source and three Bevy checks pass; the new source check verifies deformed mesh contacts and rollback. Both launchers still use this executable; no active owner session was replaced.

The 600-tick event/contact render completes normally with 132 telemetry records and no failure. The Lacetti/Donington image was inspected. Damage RNG remains the known sequence `ac7e1d7f -> 2a73550a`; final Y is -14.807243, matching the prior scripted path. Visible deformation is not enabled. Files `local/playtest/deformed-geometry-c3696050d8f6-20261013.{txt,json,png}`; log SHA `ce2302b050eb17fdde433f8d46616f3334beecf3f722ec855f6f9e54e3f14dd7`, image SHA `a6e4035fbbb380ede67c844feda9fa5e0ee6ea4ee438486063e3f2c7ffa5ccf0`. The known gamepad80070057 message recurs without preventing completion. Steam130 resources/executable unchanged at `2026-10-13T16:12:07.617029Z`, private `local/scene/preservation-20261013-deformed-body-build.json`.

## Physical callback phase refresh

Clean **df108f6656a6** (full `df108f6656a6d306f145665814ca7443b0d562a9`), built `2026-10-13T17:14:51.9710782Z`, release SHA `ae4a1d5facc1934f0f73dbbaff5dc719071f0b7462c318fb8f7b53ef5cf71cbc`. Private manifest `local/playtest/build-20261013T1714245827004Z/`; all eight source and three Bevy checks pass. Both launchers current, no active owner session replaced. The new internal callback phases retain existing driving behavior; active bulk deformation is still pending.

The600-tick event/contact run completes normally with132 telemetry samples/no failure; final Y-14.807243 and RNGac7e1d7f->2a73550a retain the known trajectory. Image inspected: rigid Lacetti beside the barrier. Files `local/playtest/deformation-phase-df108f6-20261013.{txt,json,png}`; log SHA `dc4ffed4c0e4caf4f6975fd6010796fbfa237c65d9c987719c9b8a989c114ad1`, image SHA `c0dd570f22b35fec81188222e35cc0cfc54570ace0646d96ab4c621403e09981`. Known gamepad80070057 recurs without preventing completion. Steam130 resources/executable unchanged `2026-10-13T17:18:16.169694Z`, private `local/scene/preservation-20261013-deformation-phase-build.json`.
