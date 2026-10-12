# Playable build updates

The owner requested periodic prototype updates on 13 October 2026. After verified driving changes, rebuild the release app and keep **Play GRID Prototype.cmd** and **Play GRID Body Contacts.cmd** using that executable. Preserve an active owner session and continue offline work until the app is closed.

`tools/update_prototype.ps1` checks the purchased-source driving tests and Bevy input/presentation/recovery tests, then builds the app offline using the existing Rust toolchain. It embeds the source revision in the window title and startup log. A build made with uncommitted changes is marked `dirty`; a plain Cargo build without this variable says `unversioned`.

Run `powershell -ExecutionPolicy Bypass -File tools/update_prototype.ps1` from the workspace. The helper reuses Rust from PATH or the existing standard user Cargo directory; it installs nothing. It restores its temporary environment variables afterward.

Every successful update writes ignored `local/playtest/build-UTCSTAMP/build.json` and logs. `local/playtest/current-build.json` points to the latest successfully checked build and records the full revision, executable hash, tool versions and checks. Rendering and owner laps are separately verified; a successful rebuild alone does not establish them. Game assets and compiled binaries remain private.

Use the existing launchers to try the current build. Changes that affect driving or limitations are recorded in `PROJECT_LOG.md` and linked research notes; documentation-only commits need not trigger a rebuild. The current prototype supports the owner-confirmed keyboard lap, reverse and Space handbrake. Optional body contacts and selected wheel-impact damage are experimental; the separate body-impact callback remains under research.

## Initial updater validation

The first helper run on 13 October passed all five purchased-source driving tests and all three Bevy tests, then rebuilt the release executable. Formatting, all-target workspace Clippy with warnings denied, PowerShell AST parsing and whitespace checks also passed. Private evidence is `local/playtest/build-20261012T2257094306737Z/`; its manifest labels the uncommitted build `fe5d9cd9836e-dirty` and records SHA-256 `7de798e1f864de582a7dc79b419d6834f688c0146275a1495dc6350fd1c7896a`. A subsequent committed-source build supersedes it through the current-build manifest. This update adds build identification and maintenance tooling, with no driving-physics change.

The initial tool inventory found Cargo and Rust absent from the shell PATH. Both were already installed in `C:\Users\debros\.cargo\bin`; the fallback reuses Cargo 1.99.0 and Rust 1.99.0. No installation was needed. Bevy remains 0.19.1.
