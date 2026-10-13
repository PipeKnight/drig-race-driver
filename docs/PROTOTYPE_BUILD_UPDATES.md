# Playable build updates

Latest verified refresh: **f428d0a23ce8**, built 13 October at 09:51:37 UTC. Seven source-driving and three Bevy checks pass, as does the 600-tick contact/event render. [Active-grid synchronization notes](DEFORMATION_SYNCHRONIZATION.md#playable-refresh) record the exact source revision, executable hash and private evidence. This update prepares deformation integration; visible meshes remain rigid.

The owner requested periodic prototype updates on 13 October 2026. After verified driving changes, rebuild the release app and keep **Play GRID Prototype.cmd** and **Play GRID Body Contacts.cmd** using that executable. Preserve an active owner session and continue offline work until the app is closed.

`tools/update_prototype.ps1` checks the purchased-source driving tests and Bevy input/presentation/recovery tests, then builds the app offline using the existing Rust toolchain. It embeds the source revision in the window title and startup log. A build made with uncommitted changes is marked `dirty`; a plain Cargo build without this variable says `unversioned`.

Run `powershell -ExecutionPolicy Bypass -File tools/update_prototype.ps1` from the workspace. The helper reuses Rust from PATH or the existing standard user Cargo directory; it installs nothing. It restores its temporary environment variables afterward.

Every successful update writes ignored `local/playtest/build-UTCSTAMP/build.json` and logs. `local/playtest/current-build.json` points to the latest successfully checked build and records the full revision, executable hash, tool versions and checks. Rendering and owner laps are separately verified; a successful rebuild alone does not establish them. Game assets and compiled binaries remain private.

Use the existing launchers to try the current build. Changes that affect driving or limitations are recorded in `PROJECT_LOG.md` and linked research notes; documentation-only commits need not trigger a rebuild. The current prototype supports the owner-confirmed keyboard lap, reverse and Space handbrake. Optional body contacts and [selected registered body events](BODY_IMPACT_RUNTIME.md) are available through the body-contact launcher. Visible deformation remains under research.

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
