# Playable build updates

Latest verified refresh: **99984583fe3f-dirty**, built 15 October 2026 ~03:19 UTC, release SHA256
3e73714daa56c2e2a681819ec7357e4ea54267a70ae3ab34c09bacd55ac686df, manifest
local/playtest/build-20261015T0319056811039Z/. Tyre walls, armco, cushions, chicane tyre stacks
and brake-marker boards are now solid ([route props](ROUTE_PROPS_COLLISION.md)); impacts happen
at the visible barrier instead of the invisible sheet behind it. Includes tyre audio. Source
driving and Bevy checks pass; autonomous three laps unchanged. Owner crash playtest pending.

Previous refresh: **6ffba7dc15fd-dirty**, built 15 October 2026 ~02:45 UTC, release SHA256
539082e9fffc7a2583a4701e71a3ec17f6c7201dfc19ab52ca33ba52b29f8717, manifest
local/playtest/build-20261015T0245060785757Z/. Adds [tyre and surface audio](TYRE_AUDIO.md)
(roll, wheelspin peel, locking slide, skid, scrub per wheel; M mutes engine and tyres).
Source driving and Bevy checks pass; offline tyre render peak .337. Not listened to on the
audio device overnight; owner listening pending.

Previous refresh: **60a59772f965-dirty**, built 15 October 2026 02:16:30 UTC, release SHA256
b58d6d41ceee6f24eb7b2aac7ca36b844b8553347617326658d0c8e268917a54, manifest
local/playtest/build-20261015T0216104356687Z/. The engine now idles on the grid and the time-trial
countdown holds the car on the handbrake so throttle revs it; releasing at GO gives launch wheelspin.
Source driving and Bevy checks pass; an offscreen 290-tick time-trial launch render was inspected.
See [standing start and grid hold](LAUNCH_AND_GRID_START.md). Owner launch playtest pending. The helper
needs `git` on PATH and PowerShell 7 (Windows PowerShell 5.1 treats Cargo's stderr as failure).

Previous refresh: **0cb4405fc491-dirty**, built15October(client date),14October22:16:17UTC. New **Play GRID Time Trial.cmd** selects three valid laps with countdown, frozen results and private personal bests; the existing launchers keep free practice. Eleven source-driving and seven Bevy checks pass. Three complete diagnostic event traces match across captured debug/release runs; the600-tick render preserves all310 logged physical/deformation records frombc90482. See [time-trial rules, checks and limits](TIME_TRIAL_EVENT.md). Audio is unchanged and remains an approximate original-style implementation, deferred at the owner's request.

The owner requested periodic prototype updates on 13 October 2026. After verified driving changes, rebuild the release app and keep **Play GRID Prototype.cmd** and **Play GRID Body Contacts.cmd** using that executable. Preserve an active owner session and continue offline work until the app is closed.

`tools/update_prototype.ps1` checks the purchased-source driving tests and Bevy input/presentation/recovery tests, then builds the app offline using the existing Rust toolchain. It embeds the source revision in the window title and startup log. A build made with uncommitted changes is marked `dirty`; a plain Cargo build without this variable says `unversioned`.

Run `powershell -ExecutionPolicy Bypass -File tools/update_prototype.ps1` from the workspace. The helper reuses Rust from PATH or the existing standard user Cargo directory; it installs nothing. It restores its temporary environment variables afterward.

Every successful update writes ignored `local/playtest/build-UTCSTAMP/build.json` and logs. `local/playtest/current-build.json` points to the latest successfully checked build and records the full revision, executable hash, tool versions and checks. Rendering and owner laps are separately verified; a successful rebuild alone does not establish them. Game assets and compiled binaries remain private.

Use the existing launchers to try the current build. Changes that affect driving or limitations are recorded in `PROJECT_LOG.md` and linked research notes; documentation-only commits need not trigger a rebuild. The current prototype supports the owner-confirmed keyboard lap, reverse and Space handbrake. Optional body contacts, [selected registered body events](BODY_IMPACT_RUNTIME.md), [collision-driven deformation](DEFORMATION_DRIVING_INTEGRATION.md) and [independent body dents](BODY_DEFORMATION_PRESENTATION.md) are available through the body-contact launcher.


## PCM boundary correction refresh

Revision75fd3ca515c6b8650d047fa339f5cb1c21d033a5, display75fd3ca515c6-dirty,
built2026-10-14T11:57:34.5763423Z; release executable SHA256
e10c79c314be27f90d6089aa7ab208cac8acf4f1465c51d7f2031e4d1b4b52b7.
Private manifest local/playtest/build-20261014T1156520116338Z/;
eleven purchased-source driving and six Bevy checks pass. Both existing launchers
use this executable. No owner prototype session was closed or replaced. All GRID
implementation was committed; unrelated settings-recovery documentation remains
preserved and accounts for the dirty marker.

The original-loader-confirmed fix removes four metadata bytes from each decoded
recording. It does not replace the failed granular mixing policy, add the newly
recovered gain/EFX processing or establish matched sound. M mute remains usable.
An explicit one-second Windows output check passes; evidence
local/audio/engine-device-check-75fd3ca-20261014.txt SHA256
76f856408224eeb43ca82571ddfb0d3aa88e2133a84e6ae5e69ce7a82259591e.

The600-tick collision/deformation render completes with155samples/no failure.
All310 physical/deformation records match8f73dd1 exactly excluding wall time;
the image is byte-identical and was visually inspected. Nine mesh-update log
entries include an unchanged startup record; first changed tick165, last175.
Private log local/playtest/engine-pcm-prefix-75fd3ca-20261014.txt SHA256
9c249871f48615d255e9453bbd108dd9d29561db874256217ec2320dd153f189;
PNG SHA256 f861a5c44e8e366873f891fb1f6831d8c152270746e9e3973139f1d6c172b517;
comparison engine-pcm-prefix-no-feedback-20261014.json SHA256
786393621bb39b60179847c6fe8e20dacf21c42b5a88315456b56972cc3c7742.

Original GRID is still running privately for reference capture. The earlier
minimized/input guard cleared on fresh window selection: observed Race Day state
was readable and guarded macros navigated to Touring Cars/Lacetti. The optional
manual-restore request is now unnecessary; no owner action is required by this
build update. Original Steam130resources/executable remained unchanged at
2026-10-14T11:51:59.642110UTC.

## Experimental engine-sound refresh

Revision8f73dd1656ceb9e30bfebefd562f27be4fbc89b0, display8f73dd1656ce-dirty,
built2026-10-14T01:20:33.7224928Z; executable SHA256
1237ae9e0929d1da26cb5748050de26adf0f0cfc1d6b2683c326f8699726f66d.
Private manifest local/playtest/build-20261014T0119358871489Z/;
11source-driving and6Bevy checks pass. Both launchers use this executable;
no owner session was replaced. GRID code was committed; concurrent unrelated
settings-recovery notes remain untouched and explain the dirty marker.

The source audio decoder is verified, while the first independent granular
mixer is an audible experiment. The owner drove3,795ticks in the baseline
mode with286sampled observations and no simulation failures, confirmed pitch
response, and rejected the tone as mosquito-like compared with original GRID.
This is a failed fidelity check; it must not be reported as matched audio.
The earlier short body-contact session ends in CtrlC before playback-start
confirmation. M mutes; P fades to silence; R resets sound ownership.

An explicit one-second Windows sink check passes (local/audio/engine-device-check-8f73dd1-20261014.txt),
and the owner's baseline log independently includes output_started.600-tick
silent render exits normally/155samples/zero failures, all310 logged physical/
deformation records match f80d2be exactly. Image inspected: readable practice
HUD, Lacetti/Donington/pit barrier. Eight mesh uploads/firstchanged165/last175;
o audio device is opened by offscreen rendering.

Private render log local/playtest/engine-audio-8f73dd1-20261014.txt SHA256
88e528c5bfe94d0b6bc6d21dee6f7f8ec1542eacad7228d461c45589e4c34416;
PNG f861a5c44e8e366873f891fb1f6831d8c152270746e9e3973139f1d6c172b517;
comparison local/playtest/engine-audio-no-physics-feedback-20261014.json
SHA256 d66465c52748135a3be300fb70bcb17ad805a1e34e2a97c37c69e737475d3d9f.
Owner-log summary local/playtest/owner-engine-sound-20261014.json SHA256
c633976eaca0c29626d6fa32e04715a429c4986ceec95defe0ce7150bac29af1.
Steam130resources/executable still match at2026-10-14T09:42:44.351326UTC;
local/scene/preservation-20261014-engine-audio.json.

## Practice timing and on-screen display refresh

Revision `f80d2be1fc6d536b5395a7ae22b02c4efb3131d8`, displayed `f80d2be1fc6d-dirty`, built `2026-10-14T00:02:36.4792416Z`; executable SHA `343be29637318782c7c7ab5e1e4957270d2d34a120633bc8d926546469b31af7`. Private manifest `local/playtest/build-20261014T0002135803710Z/`; eleven source-driving and five Bevy checks pass. Both launchers use the current executable. No owner session was replaced.

The source implementation was committed; unrelated settings-recovery documentation was preserved uncommitted in the shared workspace, so the helper correctly labels the build dirty. This is not described as a clean build. An independent diagnostic driver was started after this build and is separate from its executable.

A 600-tick collision render exits normally with 155 samples and zero failures. All 310 physical/deformation records match the previous adb7755 release exactly excluding wall time. Nine mesh-upload records include an extra unchanged startup upload; first changed tick165 and last175 remain the same. The inspected screenshot shows legible current-lap/sector/last/best feedback, speed/gear and controls. Private `local/playtest/lap-hud-f80d2be-20261014.txt` SHA `0006703e989c6ddf45f1540934ad509b3d88e6bfb28c6dd2e46b329eb0f27532`; image SHA `349646f3cf08e408e7d3b7e4b9c943700e0966abf0490412d37b65625d894b0b`; comparison `local/playtest/lap-hud-no-physics-feedback-20261014.json` SHA `22afb7b2b945cd56e5c7741437cef9a205b8e57cef59b7a59039b99514f588f4`.

The first new log summary rejected the emitted `running` status because the parser incorrectly expected `valid`; corrected and the actual render summary passes. This was a diagnostic-parser failure, not an app/timer failure. No fresh owner lap is claimed.

## Terminal wheel-damage recovery refresh

Clean revision `adb7755a141a7b0cee7ff3e6c82d5cf241287e1c`, built `2026-10-13T23:41:33.3630106Z`; release SHA `749304d30b2757e1de39a1fbc4a8ecdaaa96287cb4d4dcee92fd5bc7f7fbd99a`. Both existing launchers use this executable. Private build evidence `local/playtest/build-20261013T2341109119757Z/`: ten source and five Bevy checks pass; no owner session was replaced.

Both old owner failure sequences reproduce all 490 successful sampled poses/velocities exactly in debug, release and repeated release, then continue through terminal transitions at ticks 2148 and 2696 and 300 further ticks. The source state disables driving on the following tick; the app reports the affected wheel and R recovery. Full original end-of-race notifications and detached parts remain outside this selected empty-receiver context.

The clean executable also completes a 600-tick collision render with 155 samples and no failures. All 310 logged physical/deformation records are exactly equal to the previous d221 build excluding wall time. Eight body-mesh uploads, first change 165/last 175, remain unchanged. The inspected image shows the Lacetti beside the Donington pit barrier. This healthy-contact render is separate from the critical-state control replay and Bevy recovery checks.

Private render log `local/playtest/damage-terminal-adb7755-20261014.txt` SHA `bdaec421e766ca2c68cf2a025ab0b9663a56f0a097b01689851c4c269aee0ecb`; image SHA `ce25b339e4c8296bb48d35ab021803795e34a9d05d27de8949aacf84f7496ecc`. Read-only replay comparison `local/playtest/damage-terminal-healthy-replay-20261014.json` SHA `0e849518c39da885d3ed987c2943683850f8521ba37649dc4ec7850c9b076aab`. Reconstructed critical control-replay audit `local/playtest/damage-freeze-replay-audit-20261014-final-03.json` SHA `1951c2af8c1147c76cfae82283b4c32ecec606a8bc316fab570250c67ad63ce0` checks eight current identities and all six output files (5,464,578 bytes). No fresh owner playtest of this corrected build is claimed.

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


## Continuous six-recording audio refresh

Sourcebc90482b4a379f7c0fb670c97340b0fd1c6ea90e/displaybc90482b4a37-dirty,
built2026-10-14T17:54:31.5772117Z; executableSHA256
8aad8b8532156c56f43cfb9a33b325815d4ad9f7ff0b767d81a9d59422f456b6.
Manifest local/playtest/build-20261014T1753534341298Z/build.json
(the canonical pointer is local/playtest/current-build.json). Eleven purchased
source-driving and six Bevy checks pass. Both existing launchers use the current
release executable; no owner prototype session was open or closed. All engine
source was committed; preserved unrelated settings notes explain the dirty tag.

Continuous PCM bank loading, twelve-voice scheduler, source idle/max mapping
and gain/seek composition replace the first128msgrain policy. The original
full451770 effects and actual device/bus/spatial/filter implementation remain
incomplete. Observed routing offsets and independent mono resampling are
experimental; sound quality needs owner listening, not a1:1claim.
Decoder failures signal the main thread and mute instead of silently proceeding;
R reconstructs presentation, M toggles mute.

Explicit Windows device output test passes for one second:
local/audio/engine-device-check-bc90482-20261014-final01.txt SHA256
ea7cfb5d3a209f5595ff3ffed6fd98ab2ee3a9edcda4cfab0fbb8de067152fd4.
The first invocation used unsupported --root and exited before output; retained
as a command error, corrected to documented --game-root in the accepted run.

600tick render ends normally/155samples/no failures. All310logged physical
and deformation records equal75fd3ca excluding wall time. Initial image
comparison failed because this invocation omitted --include-batches:
365track draws rather than the old413. This was an unmatched experiment,
not a renderer regression; both first image and rejected comparison are kept.
Repeating with matching flags gives a byte-identical image, visually inspected
via the identical output; no rendering implementation changed.
Accepted matched log SHA256
6c15099bf392cd26ef81b2c1389c102bfe335056cfd01fe9a573c4996bfe135b;
PNG SHA256f861a5c44e8e366873f891fb1f6831d8c152270746e9e3973139f1d6c172b517.
local/playtest/engine-continuous-no-feedback-matched-20261014.json SHA256
b1669217b513f4eed3543c530918ddc3eff60202ca06ac3e27b1385b51a5f1d8.

Steam130resources+original executable remain unchanged at17:56:17.923002UTC:
local/scene/preservation-20261014-continuous-audio.json.
No settings changes/installations/approval rejections. Fixed-RPM audition is
local/audio/continuous-20261014-release01/fixed-rpm.wav; source-driving audition
source-drive.wav. Listening is pending. Next reconstruct full-frame load/gear/
wobble and original postmix/spatial behavior, using feedback to prioritize
audible differences; avoid claiming a filter cause from the ambient recording.


## Finite time-trial refresh

See [F276 build and validation evidence](TIME_TRIAL_EVENT.md#playable-refresh-and-regression-evidence).
Source 0cb4405fc4916a46d12478adc45168972a2fc50c / executable SHA256 5aa01a5f727a0adbc75956869ac4331ab6b35044a1412ad116cc24a6684591df.
All three launchers use this release. No owner session was replaced; saved
bests stay under ignored local/playtest/. Full original handling calibration
and owner completion/PB observation remain open.
