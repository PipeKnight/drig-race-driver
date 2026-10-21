# Research toolchain

## Android toolchain — 17 October 2026 (P9)

Checked first: no Android SDK, NDK, adb, Gradle, `JAVA_HOME` or `ANDROID_HOME` existed on PATH, in `%LOCALAPPDATA%\Android` or in `C:\Users\debros\Tools`. The only JDK found is the 21.0.9 runtime bundled with the Songs of Syx Demo game (Phase 0). A game's private runtime can change with that game's updates, so a separate JDK was installed. Everything is under `C:\Users\debros\Tools` (not the repository, not OneDrive), and the downloads are kept in `Tools\downloads\`. Used by `tools/build_android.ps1` ([ANDROID.md](ANDROID.md)).

| Component | Version | Location | Source and verification |
|---|---|---|---|
| Android SDK Command-line Tools | 22.0 (`commandlinetools-win-15859902_latest.zip`) | `Tools\android-sdk\cmdline-tools\latest` | https://developer.android.com/studio "Command line tools only"; SHA-256 `90ae805d20434428bffcb699c290860f19bb5f66a67e6b330067e3de801fb04a` matches the page |
| Platform-Tools (adb) | 37.0.1 (adb 1.0.41, 37.0.1-15733141) | `Tools\android-sdk\platform-tools` | `sdkmanager` from Google's repository; sdkmanager checks each archive against the checksum in the repository manifest |
| Build-Tools | 36.1.0 | `Tools\android-sdk\build-tools\36.1.0` | sdkmanager |
| SDK Platform | `platforms;android-36` revision 2 | `Tools\android-sdk\platforms\android-36` | sdkmanager; the plan's targetSdk 36 |
| NDK | r30 LTS, 30.0.16248370 | `Tools\android-sdk\ndk\30.0.16248370` | sdkmanager (`ndk;30.0.16248370`); the version listed on https://developer.android.com/ndk/downloads |
| JDK | Eclipse Temurin 21.0.12.1+1 LTS (`OpenJDK21U-jdk_x64_windows_hotspot_21.0.12.1_1.zip`) | `Tools\jdk\jdk-21.0.12.1+1` | Adoptium API / GitHub release; SHA-256 `f9d6e191ab098c0d416e7d588a24420a8621cd2f4720dab2459b8b7b2d2d8b4e` matches the published `.sha256.txt` |
| Gradle | 9.8.1 (`gradle-9.8.1-bin.zip`) | `Tools\gradle\gradle-9.8.1` | https://services.gradle.org; SHA-256 `dce76f55f8e251a3a1f130eb120f30b3d271de2b76c9b0729d316b5a1b6dc01f` matches the published `.sha256`. AGP 9.4 needs Gradle 9.6.0 or newer (https://developer.android.com/build/releases/about-agp) |
| Android Gradle Plugin | 9.4.1 | Gradle cache `Tools\gradle-home` | pinned in `android/gradle/libs.versions.toml`, from Google Maven at build time. No Gradle wrapper, so no binary jar in Git: the script calls the pinned Gradle |
| GameActivity | `androidx.games:games-activity:4.4.0` | Gradle cache | android-activity 0.6.1 bundles the 4.4.0 C sources and names 4.4.0 in its README. 4.4.1 and 4.4.2 exist but are not used |
| JUnit (F365) | `junit:junit:4.13.2` (testImplementation) | Gradle cache, from Maven Central | desktop unit tests of the first-run copy, `gradle -p android testDebugUnitTest` |
| cargo-ndk | 4.1.2 | `Tools\cargo-ndk-4.1.2\bin` | `cargo install cargo-ndk --version 4.1.2 --locked --root ...` from crates.io (cargo checks the registry checksums) |
| Rust target | `aarch64-linux-android` for rustc 1.99.0 | rustup | already installed in P0 (PLATFORMS_AND_INPUT_RESEARCH.md 1.1) |

Licences: on 17 October 2026 the owner approved, in chat, accepting the Android SDK License and the Android SDK Preview License. `yes | sdkmanager --licenses` accepted all eight offered licences. The six that were not approved (Google TV, Android XR, ARM DBT, Glass GDK, microXR, MIPS system image) were withdrawn at once by deleting their files from `android-sdk\licenses`. Only `android-sdk-license` and `android-sdk-preview-license` remain. None of the six covers an installed package.

## Identity check (F458)

The solo and 12-car identities in PROJECT_LOG are the sha256 of the race tool's standard output.
- Rebuild the binary first: `cargo test` builds test harnesses, not the bin targets, so a
  `target/release/grid-opponent-race.exe` from before a rebase silently gives the old hash
  (19 October 2026: a post-F444 tree reported the pre-F444 solo hash).
- Inputs: the purchased ROOT `C:/Program Files (x86)/Steam/steamapps/common/Grid`; CD01
  `local/body-contact-directions.bin` (sha256 ea8d8bff…79c7); BP01
  `local/body-impact-parameters.bin` (sha256 493380e1…946d). No other GRID_* variable affects the
  race tool unless named below.

```
cargo build --release -p grid-tools --bin grid-opponent-race
# GRID_DEFORMATION_PARAMETERS and GRID_AI_CHARACTERISTICS unset:
target/release/grid-opponent-race "$ROOT" CD01 BP01 1 11200 ai | sha256sum    # solo
target/release/grid-opponent-race "$ROOT" CD01 BP01 12 11200 ai | sha256sum   # 12-car
# and the 12-car again with both variables set (DF01 and the characteristics)
```

`tools/prepush.ps1` (full tier) runs these three and compares them with `tools/identity.tsv`.
A commit that changes an identity on purpose updates that file and names the change in its log entry.

## Pre-push tiers, build lock and release (F534)

The full pre-push gate (release tests with `--include-ignored`, workspace Clippy, the identity
check and `grid-soak quick`) took 15–24 min even for pushes that changed only Markdown (F526,
F529). `tools/prepush.ps1` runs only what the changed files need:

```
powershell -NoProfile -ExecutionPolicy Bypass -File tools\prepush.ps1          # check before a push
powershell -NoProfile -ExecutionPolicy Bypass -File tools\prepush.ps1 -Plan    # classification only
powershell -NoProfile -ExecutionPolicy Bypass -File tools\prepush.ps1 -Full    # the whole gate
```

It fetches origin, takes every path changed since the merge base with origin/main (committed,
uncommitted and untracked; renames as both names), classifies each and runs the highest tier any
path needs. It prints each path's tier and reason, the tier line ("tier crate [grid-app] because
…"), each step's result and time (and its wait for the build lock), and writes the logs and
`summary.md` to `local/prepush/<time>/`. It warns when HEAD is not rebased on origin/main.
Exit code 0 when every check passed.

| Tier | Runs | Chosen for |
|---|---|---|
| docs | no cargo: `git diff --check` (conflict markers fail, whitespace is listed), relative Markdown links on added lines must exist (links into `local/` are skipped), changed `.ps1` files must parse, changed `.py` files must parse, `tools/log_entry.py check` when that tool exists | `docs/`, `log/`, `*.md`, LICENSE, .git* files, launchers `*.cmd` (the build gate smoke-runs them), `android/` (no cargo check covers it: build an APK), `tools/` scripts that no crate source names |
| crate | that crate's release tests with `--include-ignored` and `cargo clippy --release -p CRATE --all-targets -- -D warnings` | `crates/grid-app/**` and `settings/**` (grid-app's tests read the profiles); `crates/grid-probes/**` (Clippy only); grid-tools binaries other than grid-opponent-race and grid-soak; grid-formats `tests/`/`examples/` |
| workspace | the whole release test suite and workspace Clippy | grid-formats modules the simulation cannot reach; a `tools/` file named by crate source |
| full | workspace + `cargo build --release -p grid-tools --bin grid-opponent-race --bin grid-soak`, the identity check against `tools/identity.tsv`, `grid-soak quick` | `crates/grid-world/**`, the grid-tools library and its race/soak binaries, sim-reachable grid-formats modules and `lib.rs`, Cargo.lock, Cargo.toml files, build.rs, rust-toolchain, `.cargo/`, `tools/identity.tsv`, and any path the table does not name |

Why these rules. The crate graph is grid-formats → grid-tools → grid-app and grid-world →
grid-tools; grid-world has no dependencies, grid-probes depends on formats, world and tools, and
nothing depends on grid-app or grid-probes. grid-opponent-race and grid-soak are grid-tools
binaries, so no grid-app or grid-probes change can move an identity or a soak result; grid-app's
own behaviour is covered by its tests and by the build gate before each playable. For
grid-formats the script computes at every run which modules are reachable from grid-tools or
grid-world (`grid_formats::` paths and `use grid_formats::{…}` lists, then closed over the
crate's own `crate::`/`super::` references in both directions, since a module that uses
reachable code could add impls to reachable types). On 21 October 2026, 40 of 47 modules were
reachable; only action_map, etc2, graphics_grade, language, skidmarks, track_lighting and
tree_placements take the workspace tier. The grid-tools library is always full (inherent impls
and trait impls in any module can reach the race tool without a path to it). The decision
table is in the header of `tools/prepush.ps1`; when unsure it goes up a tier.

Not covered by any tier, as before: the Android build (`tools\build_android.ps1 -Clippy` checks
the `cfg(target_os = "android")` code) and the real app (`tools\build_gate.ps1`).

**Measured** (21 October 2026, this PC, other sessions building; logs in this worktree's
`local/prepush/`):

| Change | Tier | Wall time | Steps |
|---|---|---|---|
| a Markdown line in docs/ | docs | 2.3 s (fetch included) | sanity 0–1 s |
| a comment in crates/grid-app/src/main.rs | crate [grid-app] | 536 s | grid-app recompile + tests 509 s, Clippy -p grid-app 20 s |
| the same edit, `-Full` | full | 1281 s | tests 718 s, Clippy 38 s, race/soak build 1 s, identity 144 s, soak 371 s |
| nothing to compile, `-Full` | full | — | tests 608 s (grid-app 425 s, grid-tools 173 s, the rest 8 s), identity 74 s, soak 274 s |
| first run in a seeded worktree | full | — | tests 998 s (our four crates compiled), Clippy 56 s, build 53 s |

grid-app's own asset tests (425 s with nothing to compile) dominate the grid-app tier, so that tier is
about 2.4x faster than the full gate, not more. Speeding up those tests is engine work and outside
this tooling.

### Build lock

The PC is memory-bound, so cargo builds run one at a time across all checkouts.
`tools/build_lock.ps1` holds `C:\cargo-build\build-lock\lock` open without write sharing for
the duration of a build; Windows closes the handle when the holder dies, so a crash never leaves
a stuck lock (a leftover file of a dead pid is reported as stale and taken over). Waiters queue
in `C:\cargo-build\build-lock\queue\` first come first served (entries of dead pids are
dropped) and print their queue position and the holder (pid, purpose, checkout, minutes).
prepush.ps1 takes it per cargo step, update_prototype.ps1 for its tests and build,
build_android.ps1 around cargo ndk and Gradle, release_playable.ps1 around each APK build, and
build_gate.ps1 for the perf phase (so locked builds wait instead of disturbing the CPU timing;
after `-PerfWaitMinutes` perf runs without it and a case over budget is `noisy`). A child script
under a holder (GRID_BUILD_LOCK = the holder's pid) runs without queueing behind it. Builds
started outside the lock are not stopped; the holder prints how many cargo/rustc processes run
outside it. build_seed.ps1 does not take it (its 2-h build would block every session).

Ad-hoc builds through the lock (the exit code is the command's):

```
powershell -NoProfile -ExecutionPolicy Bypass -File tools\build_lock.ps1 cargo build --release -p grid-app
powershell -NoProfile -ExecutionPolicy Bypass -File tools\build_lock.ps1 -Status
```

### Release (coordinator)

`tools/release_playable.ps1`, in the main checkout only (it never pushes; the main checkout holds
Codex's unpushed graphics commits):

```
powershell -NoProfile -ExecutionPolicy Bypass -File tools\release_playable.ps1 -DryRun   # preflight + plan
powershell -NoProfile -ExecutionPolicy Bypass -File tools\release_playable.ps1           # release
```

Preflight refuses a linked worktree (`-AllowWorktree` for tests), tracked changes, a merge with
origin/main that `git merge-tree` reports as conflicting, a running prototype or portable copy,
and missing private inputs. Then: merge origin/main as the owner with the usual "merge:
origin/main (Fa-Fb) into Codex's local main for the playable rebuild" message (`-NoMerge`);
`tools/update_prototype.ps1` (`-NoPrototype`); optionally `tools/build_gate.ps1` (`-Gate`; a
failure stops before the copies); the exe, changed `settings/*.ini` and non-Graphics launchers
into `local/portable/GRID-Reassembly/` (`-NoPortable`); both private APKs from a temporary
detached worktree of origin/main in `local/release/apk-src-<rev>`, named
`grid-<rev>-bundled-PRIVATE-drift-nurburgring-debug.apk` and `…-PRIVATE-profile-debug.apk` in
`local/android/apk/` with their manifests, after which its build dir is pruned
(`prune_build_dirs.ps1 -ForWorktree`) and the worktree removed (`-NoApk`, `-NoProfileApk`,
`-KeepApkWorktree`); the APK test-order text (new names and sizes, the "updated" time, the upper
end of the F range, and the previous revision in the no-data-copy lists when the bundle version
is unchanged; the feature summary stays manual; `-NoTestOrder`); and a draft log entry in
`local/release/<time>.md` (revisions, merged F range and commits, exe sha256, APK sizes and
sha256, bundle version, step times) with the step logs in `local/release/<time>/`.
The profile APK carries the default (Donington) bundle as in the earlier releases (`-ProfileEvent`).
Merges that conflict stop the script before anything changes. Since F532 this includes
PROJECT_LOG.md in the main checkout, where Codex's commits still prepend `## Gnnn` entries.
Resolve by hand: keep origin/main's file and turn each local entry into
`py -I tools/log_entry.py new --series G`.

Tested 21 October 2026 on a scratch clone of the main checkout (`-AllowWorktree` not needed;
`-NoPrototype`, a copied exe). Preflight refused tracked changes, an add/add conflict and the
PROJECT_LOG.md conflict of the real main checkout (c8dd04d1 vs 8d0799e1). The full run merged
8d0799e1 (F532), refreshed a portable copy, built both APKs in `local/release/apk-src-8d0799e17389`
(waiting 533 s for the build lock behind a prepush test run), copied and renamed them, pruned that
build dir (5.46 GiB) and removed the worktree. It then rewrote the test-order text: names, sizes,
"updated" time, F455–F532, and a11cddf7de27 added to the no-copy lists because the bundle version
ecd7977054c8 is unchanged. It also wrote the draft. APK step 3318 s: drift 1865 s, profile 1441 s;
the profile then still used the drift bundle, fixed afterwards. Evidence:
`local/release-test-evidence/` of the checks-and-release worktree.

**The seed's Android artifacts did not apply.** Both APK builds in the fresh build dir rebuilt
android-activity and the 30 Bevy crates above it. The cause is android-activity's build script
(cc crate), which tracks `CXX_aarch64-linux-android` etc. The seed recorded them with
backslashes (`C:\Users\…\clang++`, seed built 19.10), while this build and the coordinator's
APK build dir of 20.10 (`11\021f3a9841e698`) have forward slashes. cargo-ndk 4.1.2 builds the
value with `Path::join` from ANDROID_NDK_HOME (backslashes in build_android.ps1), so the
forward-slash form comes from the launching environment (likely Git Bash); not proven. Until the
Android part of the seed is rebuilt from the same shell as the release (or build_android.ps1
pins these variables), each release APK pays this rebuild.

## Build gate (F524)

End-to-end checks of the real app that the coordinator runs before every rebuild of the PC
playable, the portable copy and the APKs. Unit tests and `grid-soak` drive the simulation
headless; the gate runs the built `grid-app.exe` itself, so it also sees the audio, rendering,
the launcher profiles and the event menu. It never runs the original grid.exe.

```
cargo build --release -p grid-app -p grid-tools --bin grid-app --bin grid-soak
powershell -NoProfile -ExecutionPolicy Bypass -File tools\build_gate.ps1            # 1-3, 18-24 min
powershell -NoProfile -ExecutionPolicy Bypass -File tools\build_gate_nightly.ps1    # 4, overnight
```

Exit code 0 when nothing unexpected failed. One line per check (`gate_result NAME
pass|xfail|noisy|fail DETAIL SECONDS REASONS`), then `gate_end`. `noisy` is a perf case over budget
while other sessions' compilers ran: not a failure, not verified either; re-run
`tools\build_gate.ps1 -SkipMemory -SkipSmoke` when no build runs. Evidence goes to
`local/build-gate/gate-<time>/`:
`summary.md`, `summary.tsv`, `perf.log` and one folder per app run (its data dir, `stdout.log`,
`stderr.log`, `command.txt`; the memory run also `memory.tsv`); the replay spool
(`replay-cache`, up to 190 MB) is kept only for a failed run.

**How the app runs are driven.** Each run gets its own data dir with `[osd] fps = on` (the
`frame_stats` line every second), `[sound] effects = 0`, `[front_end] show = off` and
`[diagnostics] autopilot = on`, `results_continue = 3`, plus `GRID_AUTOPILOT=1` and
`--drive-ticks`, so the ported AI drives the player's car and nothing touches the desktop's
keyboard or mouse. The windows open minimized. Audio stays on (the decoders and mixer run as in
play, at zero trim), and the gate also mutes every audio session of its own processes in the
Windows mixer. Other grid-app processes (the owner's prototype, other sessions) are never
muted, stopped or counted. Memory is the private bytes (commit) of the process tree, sampled
every second from `Win32_Process`, with its peak. A run is stopped (process tree killed) 2 s after
`drive_script_complete`, at once on "driving paused" or a panic, or at its timeout. The AI
characteristics table comes from `local/` of the main checkout when the worktree has none.

| Part | What runs | Fails on |
|---|---|---|
| 1. perf | `grid-soak perf --budget tools/build_gate/perf_budget.tsv`: one full-field quick seed per track (13; Milan seed 41 is the heavy case), 3000 ticks, alone, before the app runs | CPU time per tick over 1.3x the recorded baseline, or wall p99 over 3x. A case whose scenario changed since the budget (another digest, e.g. after a behaviour commit such as F517) is `changed`: checked at 2x the old baseline only, and the gate prints `gate_perf_rerecord` |
| 2. memory | `tools/build_gate/memory-race.ini`: 12 Lacettis at Donington, 12 laps, for 10 min of driving (`-MemoryMinutes`), stopped on the wall clock. Not the Le Mans 24 Hours: a pile-up there (full-1: 538 prop bodies) drops it to about 1 FPS and 20 ticks/s, and its field had no engine sound before F519. A pile-up can still slow this race too (full-3: 134 bodies, 49.7 ticks/s, xfail); the wall-clock stop keeps the fit at 8 minutes | private bytes growing faster than `-SlopeLimit` 1.0 MB/s (least squares after a 2-min warm-up), plus every smoke check |
| 3. smoke | every `settings/*.ini` (the `Play GRID*.cmd` launchers; Codex's Graphics launchers are not in it), 60 s of driving each (`-DriveSeconds`), and `menu-default`: `grid-app --menu` with `GRID_MENU_AUTO` (docs/EVENT_MENU.md), i.e. `Play GRID.cmd`'s default event | an `ERROR` log line; "driving paused"; an audio failure (`decoder_failed`, `bank_failed`, "Engine audio unavailable", or a WARN/ERROR from bevy_audio/rodio/cpal or about failed audio); a panic; an early exit; a timeout; peak private bytes over `-MemoryCapMB` 4096; average FPS under `-FpsFloor` 10 (gross only: four gate windows share the GPU, and in full-3 a run at 13 FPS kept real time; the sim rate is the real check); sim rate under `-RealtimeFloor` 56 ticks/s (real time is 62.5) |

Not covered: the career screen (`Play GRID Career.cmd`; the career session's unattended
`GRID_CAREER_AUTO` runs), the audio race-start test (car-series session) and Android.

**Runtime.** full-4 (20 October 2026, F523 + this gate, 4 jobs, one other grid-app open):
1084 s in all, perf 161 s, the memory race and 38 smoke runs 922 s; 52 checks, 0 failed, 4 xfail
(F501 pile-ups). full-3 (other sessions building): 1425 s, perf 401 s. The app runs start only while 4 GB of physical
memory are free (`-MinFreeGB`), so a loaded PC stretches the phase; `-Only REGEX`,
`-SkipPerf`, `-SkipMemory` and `-SkipSmoke` select parts.

**Part 4, the nightly soak** (`tools\build_gate_nightly.ps1`, overnight only, `-Hours` 8,
2 jobs): `repro` replays every private career repro (`local/repro/*/career-race.ini`, 50,000
scripted ticks, `-ReproRepeats` 2), `events` runs every launcher profile at full length (its
laps or time limit; the Le Mans 24 Hours' 720 s first) in rounds with
`GRID_SERIES_SEED = -SeedBase + round` until the deadline, and `soak` runs `grid-soak long`
for `-SoakHours` (0 skips it). It fails like the smoke runs; a race that ends with its results
passes. Timeouts allow 20 ticks/s plus 150 s, since a 500-body pile-up runs at about 21.
`summary.md` is rewritten after each run; the logs of passing runs are trimmed. nightly-repro-1
(19 min) reproduced the F514 manifold pause from `lemans-angular-13445` at tick 27782; F512
had removed the angular pauses, and the other two repros ran into 514-body pile-ups.

**Proof that the memory check catches F482.** A build with the F482 leak put back (opponent
engines on `PlaybackSettings::LOOP`; `local/build-gate/leak-f482/`) grew 4.68 MB/s on
`memory-race.ini` (2,841 to 3,597 MB in 5 min) and failed; the clean build in full-3 fitted
-0.11 MB/s over 480 s.

**Why CPU time for perf.** The tick fans out over every core (`grid_tools::parallel`), and the
gate shares the PC with other sessions' builds. Two multi-threaded runs of the same 3000 ticks
on 20 October 2026 gave Milan 14.4 and 9.3 ms per tick, Spa 3.2 and 1.4 ms, Long Beach 1.2 and
1.5 ms: no budget can separate a regression from that. `grid-soak perf` therefore runs each case
with `GRID_THREADS=1` (bit-identical results, `parallel.rs`) and sums the process CPU time
(`GetProcessTimes`) inside each `Session::step`. CPU time alone was still not stable:
- the i7-12700 is hybrid, and logical processors 16–19 (E-cores) spend twice the CPU time of a
  P-core on the same tick (Haruna seed 30, pinned to one processor: 0.30 and 0.31 ms on 2 and 3,
  0.62 and 0.66 ms on 17 and 19), so the gate pins perf with `--affinity FFFF`;
- even pinned, a pass with 8 compiler/grid processes running gave Spa 11.7 ms instead of 1.6–1.9
  and Milan 23.8 instead of 8.9–10.5 (likely kernel time from page faults on this memory-bound
  PC; not proven). Such noise does not repeat, a regression does: a case over budget runs again
  up to `--retries` (2) times and keeps its best.

The budget (`tools/build_gate/perf_budget.tsv`) is the best of two pinned passes on 20 October
2026 on F519 (perf-7); Milan seed 41 (11.4 ms CPU per tick, 111 prop bodies) and San Francisco
seed 49 (8.5 ms, 122 bodies) are the heavy cases, the others 0.4–3.1 ms. A row also stores the
scenario digest and peak body count. The next pass (perf-8) ran beside another session's cargo
build that started during it: 6 of 13 tracks came out 7–50% over even after the retries. So
the gate waits up to `-PerfWaitMinutes` (15) for running compilers (rustc, cargo, link,
rust-lld, clippy-driver) before perf, and names any still running in the reason of a case over
budget (a compiler that starts and ends inside the perf phase is not seen). Re-record the budget with
`tools\build_gate.ps1 -RecordBudget` (written to the run folder; copy it over
`tools/build_gate/perf_budget.tsv`) after an intended cost change.

**Expected failures.** `tools/build_gate/expected.tsv` (run regex, check, reason regex,
reference) turns a known problem into `xfail`; an entry not seen in a run is listed as
`gate_expected_not_seen` (fixed, or intermittent). Slowness reasons carry the run's peak prop
body count ("prop bodies up to N"), so the known cost of broken props is matched only where a
pile-up happened, and a slow race without one still fails.

## Build and test cost — 16 October 2026

Owner request: parallel sessions build slowly and load the CPU. Findings and changes:

- **Tests are cheap; builds are not.** In a full release run with `--include-ignored` (local/visual-damage/tests-f306.log), all test bodies take about 85 s together; 63 s of that is the grid-tools asset tests. 161 of the 170 test executables are empty: they are the unit-test harnesses of the grid-tools probe binaries, and none of them has a `#[test]`. Plain `cargo test --workspace` still compiles and links every one.
- **Alias.** Repository `.cargo/config.toml` defines `cargo t`, which is `test --workspace --release --lib --test * --bin grid-app`. Add `-- --include-ignored` for the asset tests. It runs every real test: library unit tests, `grid-formats/tests`, and the grid-app binary tests. It skips the empty probe harnesses. The flag set was checked on a throwaway workspace: a library, an integration test and an app binary ran; the probe binaries were not built as tests. `cargo td -p CRATE` is the dev-profile variant for quick iteration.
- **Alias in worktrees (17 October 2026, P9).** A worktree under `C:\Dev\Reassembly\.claude\worktrees\` also reads the main checkout's `.cargo/config.toml`, and cargo merges the two `t` alias arrays. `cargo t` then fails at once: "the argument '--workspace' cannot be used multiple times". In a worktree, spell it out: `cargo test --workspace --release --lib --test "*" --bin grid-app --no-fail-fast -- --include-ignored`.
- **sccache v0.18.0, measured and not adopted.**
  - Source: official GitHub release `sccache-v0.18.0-x86_64-pc-windows-msvc.zip`, SHA-256 `8965c74d…348363d`, matching the published `.sha256`. A first download through Invoke-WebRequest was truncated (5.9 of 9.7 MB) and failed the check. curl gave the full file.
  - Unpacked at `C:\Users\debros\Tools\sccache-dl-v0.18.0\x\…\sccache.exe`. It is not enabled.
  - `cargo build --release -p grid-formats` (3 crates):
    - same build dir after `cargo clean`: 3/3 hits, 22.9 s → 3.7 s;
    - a different build dir: 0 hits, 17.7 s / 16.3 s. `SCCACHE_BASEDIRS=C:\cargo-build` did not help.
  - Because every worktree has its own `C:/cargo-build/{workspace-path-hash}` build dir (F328), sccache cannot share dependency builds between sessions. Cargo already covers rebuilds within one directory.
  - A single shared build dir would serialise parallel sessions on cargo's lock, so it is not used.
- **Priority.** While many sessions build at once, the coordinator can keep `cargo`/`rustc`/`link` and test processes at BelowNormal priority (a temporary background loop, no system setting), so the desktop stays responsive. **Pitfall (19 October 2026):** career's release build of grid_app hung twice for about 100 min, its rustc using 26–74 CPU-seconds, both times when launched from a PowerShell script (`Start-Process -NoNewWindow`, redirected output) with BelowNormal set on cargo and inherited by rustc. The same build from Bash at Normal priority finished. Also that day, career's cargo (pid 48544) sat at 0 CPU-seconds for 11 min with `~/.cargo/.package-cache` open while two other cargos queued on it ("Build speed", package-cache lock). Cause not proven (starvation at BelowNormal or that launch path). Do not lower the priority of cargo/rustc while other sessions compile.
- **Build-dir seeding, adopted.** Cargo's fingerprints for registry dependencies depend on neither the build dir nor the workspace path. So a copy of another checkout's build dir makes them Fresh, and only our workspace crates rebuild.
  - Measured on a throwaway workspace: 1 s instead of 32 s.
  - Measured on this repository: release `cargo build -p grid-app` in a fresh worktree, with 6 other sessions building:
    - unseeded: 1607 s, 345 crates compiled;
    - seeded: 107 s, 4 compiled (grid-formats/world/tools/app), 341 Fresh. The robocopy seed took 2 s for 2.3 GB.
  - `tools/seed_build_dir.ps1` does this. It skips `incremental`, keeps timestamps, and does nothing if the build dir is non-empty. The default source is `C:\cargo-build\seed`, a release build of main 6301772 kept by the coordinator.
  - The seed has only the release profile, so dev/check/Clippy artifacts still build once per worktree. If a dependency changes, the stale seed is harmless: that crate simply rebuilds. (19 October 2026: the seed now also holds the release tests, the release Clippy and Android; see "Build speed" below.)
- **Probes moved (17 October 2026, F352).** See "Research probes crate" below.

### Build speed — 19 October 2026

Owner: "сборка стала занимать очень много времени… возможно можно какую-то часть кэшировать, пересобирать лишь необходимое". Each change below was measured before and after and kept only if faster with unchanged results. Tools: `tools/time_build.ps1` (wall clock plus, sampled every 2 s, the number of other rustc/link processes ("load"), CPU % and the lowest free RAM); raw records in this worktree's `local/build-speed/times.jsonl` (zealous-solomon-d3c368, moved to the main checkout's `local/worktree-evidence/` on archive). cargo/rustc 1.99.0, MSVC link.exe 14.44.35207, rust-lld from the 1.99.0 toolchain.

**The PC is memory-bound, not CPU-bound.** During every measurement the commit charge was 56–59 of 65 GB and free RAM 0.1–1 GiB (destiny2 8.8 GB private, firefox 9.6 GB, Claude 6.3 GB, ChatGPT 3 GB, rustc 6 GB). CPU averaged 22–49 %. The same `touch main.rs; cargo build --release -p grid-app` took 282, 665, 1073 and 553 s in a row. Sampling the grid-app rustc showed about 1.1 busy cores even at `-j 12` (19 threads), and 0.2 cores while free RAM was lowest. Numbers below are therefore paired with their load; only large differences count.

| # | Change | Before | After | Decision |
|---|---|---|---|---|
| 1 | Android artifacts in the seed (both APK variants and `cargo ndk` Clippy); `build_android.ps1` adds them to its build dir (`seed_build_dir.ps1 -Android`) | APK build in a build dir with the desktop seed but no Android artifacts: normal 2515 s, `-Profile` 3836 s (steps of the seed build, other sessions building); the coordinator saw 857 s for one profile APK on b098463 | a fresh build dir seeded from the new seed (desktop copy 4 s, `-Android` top-up 8 s): normal APK 288 s (only our four crates compiled), `-Profile` 207 s (grid-app only), `-Clippy` 26 s, normal APK again after Clippy 230 s (grid-app only; no Bevy rebuild). Load 0–3, free RAM 2.3–6.7 GiB (firefox 9.5, destiny2 9.3, Claude 7.2 GB) | adopted |
| 2 | Release test and release Clippy artifacts in the seed (`tools/build_seed.ps1` builds exactly the pre-push commands) | first `cargo clippy --release --workspace --all-targets` in a seeded worktree: 1260 s, 283 crates checked (load 5, free RAM 0.8 GiB); in the seed build itself 739 s. First `cargo test … --no-run`: 808 s (load 8, 0.7 GiB) | the same worktree reseeded from the new seed (our four crates compile each time, by design): `cargo build --release` 323 s (was 599 s on the old seed), tests `--no-run` 257 s, first Clippy 39 s (4 crates checked). Load 1, free RAM 8–10 GiB, firefox 9.3 / destiny2 9.3 / Claude 7.4 GB; the "before" runs had load 5–8 and 0.4–0.8 GiB free, so part of the gain is the quieter PC, but Clippy no longer checks 283 dependencies | adopted |
| 3 | rust-lld for x86_64-pc-windows-msvc (repository `.cargo/config.toml`) | grid-app.exe link, the same objects: link.exe 27.0 / 23.0 / 25.1 s; grid-opponent-race: 1.14 / 0.92 / 1.05 s | rust-lld 2.5 / 2.6 / 2.3 s; 0.71 / 0.62 / 1.00 s | adopted |
| 4 | Higher `jobs` or a machine-wide jobserver | `touch main.rs; cargo build --release -p grid-app -j 4`: 282 s, 1073 s | `-j 12`: 665 s, 553 s | rejected; `jobs = 4` unchanged |
| 5 | `grid-soak quick` default jobs: a third of the hardware threads, 2..6 (6 here) | `--jobs 2`: 1279 s (load 6, free RAM 6.4 GiB) | `--jobs 6`: 417 s (load 4, 6.5 GiB); the same 24 per-seed digests, statuses, ticks and losses | adopted |
| 6 | `incremental = true` for release | — | — | not measured |

**1–2. One seed for every command a session runs.** `tools/build_seed.ps1 -Checkout <clean checkout of main> [-Swap]` builds into `C:\cargo-build\seed-next` (through `CARGO_BUILD_BUILD_DIR`, so the live seed stays valid) and only then swaps it in. Steps: `cargo build --release`; `cargo test --workspace --release --lib --test * --bin grid-app --no-run`; `cargo clippy --release --workspace --all-targets -- -D warnings`; `build_android.ps1` and `build_android.ps1 -Profile` (the `profile` feature enables bevy/trace and bevy/debug, so Bevy gets other hashes); `cargo ndk … clippy --release --lib -p grid-app`. `seed.json` records the revision, rustc and each step's time. Measured steps of the first lld seed (from f89efd2, the same load as above): release from nothing 71 min 57 s, tests 17 min 51 s, Clippy 12 min 19 s, normal APK 41 min 55 s, `-Profile` APK 63 min 56 s, `cargo ndk` Clippy 3 min 31 s; 5.43 GiB (desktop part 2.8, Android 2.7). That Clippy ran without JAVA_HOME and spoiled the APK artifacts (below), so the Android part was redone with `-AndroidOnly` from 7e50a53 in one environment: APK 31 min 36 s, `-Profile` 13 min 12 s, `-Clippy` 3 min 19 s. Rebuilding the whole seed after a Cargo.lock change therefore takes about 2–2.5 h on this PC under the usual load; run it when few sessions build.
- `seed_build_dir.ps1` copies the desktop part (default) or adds only the files a build dir lacks (`-Android`, never overwriting one). Android artifacts are not copied by default (2.7 GiB): `tools/build_android.ps1` adds them before each `cargo ndk` build (`-NoSeed` skips that).
- **Our own crates are never seeded.** Cargo hashes workspace crates relative to the workspace root, so `grid-app-<hash>` is the same unit in every worktree, and its freshness is then decided by file times alone. Observed: a worktree with an uncommitted grid-soak change, reseeded from a seed built after that edit, finished `cargo build --release` with nothing compiled, keeping the seed's code. `build_seed.ps1` deletes the fingerprints and outputs of grid-formats/world/tools/app/probes, and `seed_build_dir.ps1` excludes their `.fingerprint`/`build` dirs when copying (this also covers older seeds). The registry crate `grid` stays. After the fix the same reseed compiled exactly grid-formats, grid-world, grid-tools and grid-app.
- **One JAVA_HOME for every Android build.** jni-sys 0.3.1's build script declares `rerun-if-env-changed=JAVA_HOME`. When it reruns, its output is newer than every Android crate built before, and cargo rebuilds all of them (jni → ndk-sys → android-activity → winit → all of Bevy's render crates: 44 units, 48 min here) even if JAVA_HOME returns to its old value. The first lld seed's last step, a bare `cargo ndk … clippy` without JAVA_HOME, spoiled its APK artifacts that way (`cargo -v`: "Dirty jni-sys v0.3.1: info of dependency `jni-sys` changed"). Hence `build_android.ps1 -Clippy`: run the Android Clippy through the script, which sets the pinned JDK, never as a bare `cargo ndk` from a shell with another or no JAVA_HOME.
- `seed_build_dir.ps1 -Reseed` replaces a build dir with a fresh seed copy (refuses while a cargo holds its `.cargo-build-lock`): 6.5 s instead of rebuilding every dependency after the linker switch.

**3. rust-lld.** `cargo rustc … -- -C save-temps --print link-args` kept the objects and printed the exact link line; `link.exe` 14.44.35207 and `rust-lld -flavor link` then linked the same inputs (vcvars64 environment), three times each.
- Checks: both grid-opponent-race executables give exactly the F448 identities (solo 6bc60af8…2cd6, 12-car 8f932cff…181f, 12-car with DF01 and characteristics d5eec9ba…1103). `tools/sample_profile.py` symbolizes both PDBs alike (the same functions and shares), and with a `debuginfo=limited` build `--lines-in main` gives the same source lines for both linkers. A rust-lld build of F454 (5004cf9 + this change) gives the canonical F454 identities of the "Identity check" procedure: solo 7b01ee41…81cd, 12-car e70bc44c…0e59.
- Cost: **cargo treats a linker change as a change of every unit** ("the config settings changed", seen on a throwaway serde workspace), so each build dir rebuilds all dependencies once. Hence the seed is built with rust-lld, and existing worktrees run `seed_build_dir.ps1 -Reseed` once.
- The linker key is scalar, so the P9 alias merging does not apply: a worktree's own `.cargo/config.toml` wins over the main checkout's.

**4. Jobs.** Rejected; `~/.cargo/config.toml` is unchanged (`jobs = 4`).
- The extra threads are not used: a seeded build compiles a serial chain (grid-formats → grid-world → grid-tools → grid-app), and the grid-app rustc used about 1.1 cores with 12 tokens (LLVM workers waiting on the single-threaded front end and IR generation).
- On this memory-bound PC more parallel LLVM work only adds RAM: the `-j 12` runs dipped to 0.1 GiB free.
- A shared GNU-make jobserver also needs `CARGO_MAKEFLAGS` in every cargo's environment, which running Claude/Codex sessions do not get, and a killed cargo leaks semaphore tokens for good.

**5. Quick soak.** Each scenario is its own process (150–320 MB peak, seeds 1 and 3), so parallelism cannot change a result; the 24 digests, statuses, ticks and losses were compared seed by seed. `--jobs N` still overrides; long runs keep 2. F445's Nürburgring set-up cost (`scene::read_renderable_draws`) was in grid-app's prop drawing; grid-soak (grid-tools) never calls it.

**6. Incremental release builds** were not measured. Unlike 1–5 they change the generated code of every release executable, including the prototype, so they would need their own performance and identity validation.

**Other observations.**
- **Package-cache lock.** Several cargo runs waited minutes on "Blocking waiting for file lock on package cache" (the seed build three times; a release build in this worktree 9 min; also G133). That lock is `~/.cargo/.package-cache`, taken exclusively during resolution and downloads by every cargo on the PC. On 19.10 09:44–09:55 a career-session cargo sat at 0 s CPU with that file open while two more queued, then went on. The holder was not identified. Restart Manager queries (which processes have the file open) every 2 s from 09:52 showed only that cargo and the two waiters; a non-blocking lock probe at 09:56 found the file free. `local/build-speed/package-cache-holders.log` keeps the record. Over 5 h of that logging every hold lasted 3 s or less except two, both by career's PowerShell-launched offline grid-app build (the BelowNormal launch path of the "Priority" pitfall above): 09:52–09:57 with two cargos queued behind it, and 42 s at 11:32. Likely a priority inversion: a starved low-priority cargo holding the machine-wide lock stops every other session's cargo at its start. Not proven; one more reason not to lower cargo's priority.
- **Soak page faults.** Two running quick-soak scenarios caused about 180 000 page faults/s, nearly all demand-zero, and the System process used 34 % CPU zeroing pages. That points at per-tick allocations large enough to go back to the OS. Not investigated here (engine code).

## Research probes crate — 17 October 2026 (F352)

The 158 one-off research probes moved with `git mv` from `crates/grid-tools/src/bin` to the new workspace crate `crates/grid-probes`. Their names, sources and outputs are unchanged, and they still build to `target/<profile>/<name>.exe`, so the `tools/validate_*.py` scripts that run those executables are unaffected.

- **Kept in grid-tools** (working tools): `grid-tools` itself, `grid-soak`, `grid-opponent-race` and `grid-private-parameters` (`tools/update_prototype.ps1`).
- **Not a default member.** `cargo build` and `cargo test` at the root skip the probes. Anything with `--workspace` (`cargo t`, the pre-push Clippy) still includes them.
- **Building a probe:** `cargo build --release -p grid-probes [--bin NAME]`. `cargo build --bin NAME` from the root also works. The `hw-sse` feature is forwarded.
- **No test harnesses.** Every probe has a `[[bin]] test = false` entry, since none has a test. A new probe works without an entry, and gets a harness until one is added.
- **References updated:**
  - `tools/*.py` that hash a probe's source now name `crates/grid-probes/src/bin/…`. Their recorded identity keys change on the next run.
  - docs reproduction commands now say `-p grid-probes`.
  - PROJECT_LOG history is left as written.

Measured on this PC: wall clock, release, right after `touch crates/grid-tools/src/lib.rs`, with other sessions building at the same time. "load" is the number of rustc/cargo/link processes running at the start. Logs and tables are in the main checkout's `local/grid-probes/`.

| command | before | after |
|---|---|---|
| `cargo build --release` (root) | 101.4 s (load 3) | 56.7 s (load 2) |
| `cargo test --workspace --release --no-run` | 47.8 s, 173 test executables (load 3) | 29.7 s, 15 (load 0) |
| `cargo t --no-run` (alias flags) | 27.2 s, 11 (load 3) | 33.1 s, 11 (load 3) |
| `cargo clippy --workspace --all-targets --release -D warnings` | 14.9 s (load 0) | 18.9 s (load 3) |
| `cargo build --release -p grid-tools` | 35.9 s (load 0) | 12.5 s (load 4) |
| `cargo build --release -p grid-probes` | (part of the rows above) | 38.1 s (load 0) |

Reading:
- Ordinary builds that used to relink all 161 binaries roughly halve (grid-tools alone: about a third).
- A plain workspace test build no longer compiles 158 empty harnesses.
- `cargo t` and Clippy cover the same targets as before. Their differences are within the noise of concurrent builds.

## Sampling profiler — 15 October 2026

The session is not elevated, so samply, WPR/xperf and other ETW profilers cannot sample, and none of them besides the inbox `wpr.exe` is installed. Nothing was downloaded. Added the independently authored `tools/sample_profile.py`, run with the bundled Python 3.12.14 `-I`. It uses only `ctypes` and the Windows 11 system `dbghelp.dll`:
- it suspends each target thread about every millisecond and walks the stack with `StackWalkEx`, including inline frames;
- it symbolizes with `SymFromInlineContextW` / `SymGetLineFromInlineContextW` against the PDB of a `CARGO_PROFILE_RELEASE_DEBUG=limited` release build;
- it reports self/inclusive shares, call paths and `--lines-in FUNCTION` per-line shares.

Proved by profiling the 12-car `grid-opponent-race` (about 250 samples/s on the race thread). The resulting optimizations and their verification are in [TICK_PERFORMANCE](TICK_PERFORMANCE.md). Known artifact: the statically linked CRT `memcpy`/`memset` shows as `_NLG_Return2`.

## Video analysis decoder — 9 October 2026

After checking PATH, existing research tools, bundled runtimes and Python modules, no FFmpeg/ffprobe, PyAV or OpenCV decoder was found in those locations. Installed official PyAV **19.0.0** Windows wheel `av-19.0.0-cp312-abi3-win_amd64.whl` only into ignored `local/tools/video-python`, with bundled Python **3.12.14** and no global interpreter change. SHA-256 **96e4c0981ec21cadfef4c0b840d04a5e3c3be821894aa6a7d7a8ee870ba6b727** matches [official PyPI metadata](https://pypi.org/pypi/av/19.0.0/json). Wheel/source record: `local/tools/video-decoder/source.json`. Proved useful operation by decoding all18,021AV1frames and AAC audio in the owner's actual GRID recording and exporting170timestamped samples. [Analysis, versions, evidence and limits](DONINGTON_RACE_VIDEO_2026-10-09.md). This validates local video analysis, not recording-time capture-hook behavior or Rust game compatibility.

## Gameplay recorder inventory — 9 October 2026

Bounded PATH/default-location/uninstall/research-directory checks found no OBS installation; this is not a full-drive search. Read-only system inventory identifies Xbox Game Bar `7.326.10010.0`, NVIDIA App `11.0.9.251`, NVIDIA RTX4070 driver `32.0.16.1692` and Intel UHD770 driver `32.0.101.7088`. Initial sandbox Appx/CIM queries denied access; escalated inventory succeeded. Existing portable AHK remains `2.0.29`. No new tools installed or actual gameplay recording tested. [Owner recording guide](GAMEPLAY_RECORDING_GUIDE.md) starts with Game Bar and offers OBS as an optional consistent recorder; official OBS download listed `32.2.2` on inspection. Inventory evidence: `local/recordings/recorder-inventory-20261009.json`. These are inventory/recommendation claims, not validated GRID recording compatibility.

Updated: 6 October 2026 (Europe/Kiev).

## Initial inventory

See [Phase 0 report](PHASE0.md#tools-checked-before-additions) for search scope and limitations. Absence means not found in inspected locations, not an exhaustive scan of every drive.

| Component | Version / location | Validation |
|---|---|---|
| Python | Codex bundled 3.12.14 | PE inventory, dump parsing and scripts run successfully |
| Git | Codex bundled 2.53.0.windows.3 | `git --version` |
| Node | Codex bundled 24.19.0 | `node --version`; npm absent from PATH |
| Existing JDK | OpenLogic 21.0.9, 64-bit; `C:\Program Files (x86)\Steam\steamapps\common\Songs of Syx Demo\jre` | Both java and javac report 21.0.9; Ghidra compatibility pending |
| ProcDump | 12.01; `local/tools/procdump` | Mini/full dumps captured actual GRID startup exception |
| Capstone | 5.0.6; `local/tools/python` | Runtime x86 instruction and caller disassembly |

ProcDump official source, Microsoft signature and ZIP hash are recorded in `local/tools/procdump-source.json`. Capstone was installed only with `--target local/tools/python`, not into the bundled interpreter's environment.

## Phase 1 additions and validation

| Tool | Pinned release and location | Proven operation |
|---|---|---|
| Ghidra | 12.1.4, `C:\Users\debros\Tools\grid-research\ghidra_12.1.4_PUBLIC` | Imports the captured runtime image as x86 and decompiles four known functions |
| x64dbg / x32dbg | Release 2026.05.27, `local/tools/x64dbg/release`; x86 `headless.exe` used | Attaches to GRID and reads the first/last-chance access violation; early call-site breakpoint remains unproven |
| Process Monitor | Microsoft 4.11, `local/tools/procmon` | Captures startup and exports actual GRID resource/configuration operations |

Downloads came from the official projects. Ghidra ZIP SHA-256 `ddac49f903da9d5bac833e5cc79395098b9c33cfd3279be5f31bd00387d2d4db` and x64dbg ZIP SHA-256 `d41966dfc5b435a372798245300ca0ab7bb8e48bdbf48512c6fb20fcca427697` match their GitHub release asset digests. Procmon ZIP SHA-256 `80a6442b46af762ed1432f6fec3f7e20366bed62a2522b3486503398a40a1128`; all extracted executables have Valid Microsoft Corporation Authenticode signatures. Exact source URLs and verification results are in `local/tools/*-source.json`.

### Ghidra

The pinned 12.1.4 release requires Java 21 and accepts Python 3.9–3.14. It successfully uses the existing game-bundled JDK 21.0.9. The development branch's Java 25 requirement does not apply to this release. Requirements were read from the [pinned release documentation](https://github.com/NationalSecurityAgency/ghidra/blob/Ghidra_12.1.4_build/GhidraDocs/GettingStarted.md), saved at `local/phase1/ghidra-release-requirements.md`.

First installation under the Cyrillic workspace path caused Log4j JAR-URL XML parsing errors. Decompilation still completed, but normal application logging did not. The installation was moved, without replacement or deletion of original game files, to the ASCII user Tools path above. Reopening the same project then produced normal logs and four `GRID_PROBE_OK` results. No duplicate JDK was installed.

`tools/export_runtime_image.py` exports a private **memory-layout snapshot**, not a playable/patched executable. Image size 15,925,248 bytes; captured bytes 15,917,056. An uncaptured 8,192-byte range at RVA `0xed8000` is zero-filled and listed in `runtime-image.json`. It is outside the selected functions and is not evidence of observed bytes. Snapshot SHA-256: `2269a72b142fb731d9251d4db26812d67e3106beb58d1b5cad68bbf33073d6cd`.

Ghidra raw importer configuration: base `0x400000`, processor `x86:LE:32:default`, compiler spec `windows`. Broad automatic analysis is disabled for this first probe; known entry points are disassembled and decompiled explicitly by `tools/ghidra/GridProbe.java`.

| Entry VA | Validation |
|---|---|
| `0x004b3960` | Pseudocode contains a bounded 32-byte format helper, explicit NUL at byte 31; agrees with the observed call and instructions |
| `0x0049d760` | Pseudocode calls that helper and traverses/removes a keyed object; generated types include artifacts and require review |
| `0x00bf6e1a` | Pseudocode shows buffer/format-argument checks and forwarding to the formatting routine |
| `0x00c01650` | Pseudocode reads a format-string argument; has type-propagation/overlapping-global warnings |

The outputs prove useful decompilation, not recovery of original source or correctness of every inferred type. Original symbols are unavailable. Derived pseudocode stays under `local/phase1/decompilation/`; the saved project is `local/phase1/ghidra-project/grid-runtime.gpr`.

Commands from the project root:

```powershell
$python = 'C:\Users\debros\.cache\codex-runtimes\codex-primary-runtime\dependencies\python\python.exe'
& $python tools\export_runtime_image.py local\phase0\dumps\grid.exe_261006_001718.dmp --out-dir local\phase1\runtime
.\tools\run_ghidra_probe.ps1
# Reopen an existing imported project:
.\tools\run_ghidra_probe.ps1 -Existing
```

The exporter refuses to overwrite an existing snapshot. The wrapper selects JDK only for its process and restores `JAVA_HOME` afterwards. The post-script has been compiled and run by Ghidra.

### Process Monitor

Capture command: `Procmon64.exe /AcceptEula /Quiet /Minimized /BackingFile <private startup.pml> /Runtime 20`, followed by a Steam launch. Export: `/OpenLog <startup.pml> /SaveAs <startup-all.private.csv>`. The capture and export completed successfully. The driver/tool worked even though the spawning shell reported a non-administrator token; no claim is made about the precise elevation path. The monitor was terminated after capture/export.

The raw capture includes other system activity and remains private. `tools/summarize_procmon.py` streams the exported CSV and writes only GRID rows to `startup-grid.csv` plus a structured summary. Recorded counts: **2,052,174 total events**, **1,006,217 GRID events**, PID **3444**. The capture covers approximately 00:31:41–00:31:59 on 6 October, local time, and ends while GRID is querying display configuration. No GRID Process Exit appears in this capture; it is not the complete crash timeline.

Confirmed successful opens/reads include:

- `win_000.nfs`
- `system/applicationMemoryMap.xml`, `system/updateMemoryMap.xml`, `system/renderMemoryMap.xml`
- the user's `hardwaresettings/hardware_settings_config.xml`
- `system/hardware_settings_restrictions.xml`, `system/hardware_settings_options.xml`
- original Bink, D3DX9 and XInput libraries

The run includes many repeated video-registry queries, 656 unsuccessful `d3d.dll` probes in SysWOW64, and local DLL probes that are followed by successful system-library loads. A missing user-level `hardware_settings_restrictions.xml` probe also has a successful installation-level restriction-file access. These are observations; none has been established as the cause. Access-denied and other result codes require context rather than automatic classification as fatal errors. Instrumented startup may be substantially slower than the uninstrumented baseline.

### Native debugger

The pinned package includes x86 and x64 GUI debuggers and headless launchers. The x86 launcher supports `-pid`, `-c`, `-cf`, `-userdir`, `-workingDir` and `-help`. Its pinned source was read to verify the stdin command interface and `exit` handling; saved at `local/phase1/x64dbg-headless-source.cpp`.

Two actual GRID probes were performed:

1. **Late attachment, PID 4300:** attach succeeded at `EIP=0x00c01796`; the debugger confirmed `0xc0000005`, read address `0x10`. A subsequently set cleanup breakpoint was not hit, because execution had already passed it. Evidence: `x32dbg-startup.log`, `x32dbg-probe.json`.
2. **Pre-initialized debugger, PID 34196:** attached earlier at `EIP=0x76fa8ae9`, then set a hardware execution breakpoint at `0x0049fd2d`. The target did not hit it within the probe's bounded wait; the debugger was detached and exited. Evidence: `x32dbg-startup-early.log`, `x32dbg-probe-early.json`. This proves earlier attachment, not successful observation of the call or the initiating failure.

`tools/probe_debugger.py` refuses an existing GRID session, launches the original through Steam, attaches only to that new process, records results and detaches/exits in cleanup. No on-disk executable patch was made. Hardware breakpoints are chosen because loader writes can overwrite a software breakpoint placed too early.

Next debugger work: give the instrumented process a sufficiently long monitored startup interval, or break closer to the suspect object's creation/failure, and record the first initialization failure before cleanup. Do not report the earlier call-site breakpoint as validated yet.

## Decisions

- Prefer the existing JDK if compatible with the selected Ghidra release. Do not install a duplicate merely because Java is missing from PATH.
- GRID is x86. Use an x86 debugging session even when the host or debugger launcher is 64-bit.
- Protected on-disk `.text` differs from runtime bytes. A static project on the original binary is useful for identity/imports but cannot be assumed to decompile the runtime crash code correctly.
- Keep original executables, loaded-memory samples and derived decompilation private under `local/`.
- Rust/MSVC setup is now validated; graphics/audio/physics libraries will be selected when their concrete workload is established.
- Immediate research tools and native compilation work. A dedicated unpacked-PE project/loader trace and reference driving remain pending, so full Phase 1/2 exit criteria are not marked complete.

Primary references: [Ghidra releases](https://github.com/NationalSecurityAgency/ghidra/releases), [x64dbg releases](https://github.com/x64dbg/x64dbg/releases), [Microsoft Process Monitor](https://learn.microsoft.com/en-us/sysinternals/downloads/procmon), [Rust installation](https://rust-lang.org/tools/install/).

## Later compatibility dependencies and private static reference

Installed official portable NTCore 4GB Patch 1.0.0.1, Steamless release 3.1.0.3 and GridMTFix v0.3 resources for owner-authorized isolated experiments. CLI file-version field for Steamless reports 3.1.0.0. Source URLs, local hashes, unsigned status and actual run outcomes are in [memory-patch research](COMPATIBILITY_MEMORY_PATCHES.md). These old downloads provide no upstream asset digests; do not claim publisher-hash verification.

Steamless identified SteamStub variant 2.1 in the purchased executable and produced a private unpacked copy. This is an additional static-analysis reference; the original Steam installation remains unchanged. Its SHA-256 and derived manifest variants are recorded in that report. A per-process legacy-codepage variant survives the bounded startup probes; menu/race verification remains pending.

Owner subsequently confirms that the legacy-codepage variant reaches the main menu. Race/driving remains untested; this improves the original-game reference without completing every Phase 1/2 capability.

Earlier debugger target PID 34196 was later found alive with `Steam Error`; it was matched to our saved PID/start time and closed before compatibility probes. The previous empty-process snapshot did not guarantee that it remained closed.

## Native build installation checkpoint

On 6 October 2026, compiler inventory again found no Rust/Cargo/MSVC. The Winget WindowsApps alias could not execute. Started official installers using `tools/install_native_build_tools.ps1`:

- VS 2022 Build Tools release-channel bootstrapper from `https://aka.ms/vs/17/release/vs_buildtools.exe`, valid Microsoft signature, with VCTools, x86/x64 tools and SDK component `Microsoft.VisualStudio.Component.Windows11SDK.26100`. Target `C:\BuildTools\GRID`; no automatic restart. Component IDs checked against [Microsoft's directory](https://learn.microsoft.com/en-us/visualstudio/install/workload-component-id-vs-build-tools?view=vs-2022).
- Rustup from its official x86_64 MSVC distribution, published SHA-256 verified. Requested minimal profile, rustfmt and Clippy, no persistent PATH modification. Current download reports Rust 1.99.0; pin actual version after installation/build validation.

Both installers are still running at this checkpoint. Source/provenance JSON, logs and eventual result JSON: `local/tools/native-installers/`. Do not mistake a partial installation tree or successful download for a tested native toolchain. Check exit codes, then run Cargo and a native smoke build.

Later cleanup check: Rust installation completed with exit code 0, no reboot requested (`rust-result.json`, 01:20:31 local). Cargo/native build validation has not run. MSVC result was still absent at that check; retain its in-progress status until inspected.

## Native build validation — resumed session

The preceding installation checkpoints are historical. Both installers completed with exit code 0: Rust at `2026-10-05T22:20:31Z`, MSVC at `2026-10-05T22:25:41Z`. VS reports a complete, launchable installation and no reboot required.

| Component | Actual validated version |
|---|---|
| Rust | `rustc 1.99.0 (b940084d7 2026-09-28)`; LLVM 23.1.1; x86_64-pc-windows-msvc |
| Cargo | `1.99.0 (5f94df478 2026-08-27)` |
| VS Build Tools 2022 | 17.14.41 (September 2026), installation version 17.14.37710.0; `C:\BuildTools\GRID` |
| MSVC tools | 14.44.35207 |
| Windows SDK | 10.0.26100.0 |

The private `local/native-smoke` Cargo program built, linked and printed `GRID_NATIVE_BUILD_OK`, exit 0. Evidence: `local/tools/native-installers/{build-validated,vswhere-validated}.json`. The first public resource workspace subsequently built and passed six synthetic tests, formatting and Clippy with warnings denied. Actual GRID integration: 222 inspected files accepted.

`rust-toolchain.toml` pins 1.99.0 with rustfmt/Clippy; the named version was installed and tested rather than relying on a moving stable alias. No persistent PATH change was made. For PowerShell commands add `C:\Users\debros\.cargo\bin` to the current process PATH. Cargo discovers the installed MSVC linker. Bevy is not selected/installed yet. OneDrive reported an incremental-cache hard-link warning and copied files instead; compilation succeeded.

The separate unpacked-PE project now imports/saves correctly and supports targeted decompilation of two car/track candidates. Its bounded broad analysis remains partial; exact project identity, candidate addresses, provenance and limitations are in [loader research](LOADER_RESEARCH.md).

## Verified macro runtime

Installed official portable AutoHotkey 2.0.29 after bounded inventory found no existing installation. Runtime reports 2.0.29; pinned ZIP SHA-256 `b2d0200724a6b6ad22c965c939c5e5a2c64a35d1ccb455a3ca3f8ce415c5a296` matches the [official release digest](https://github.com/AutoHotkey/AutoHotkey/releases/tag/v2.0.29). Private executable `local/tools/autohotkey/AutoHotkey64.exe` has no Authenticode signature; publisher-digest verification is the provenance check used here. No system installer, driver or file association was added. Source metadata: `local/tools/autohotkey-source.json`.

`tools/install_research_tool.ps1 -Tool autohotkey` supports the pinned portable package. Independently authored `tools/grid_macros.ahk` and guarded `tools/run_grid_macro.ps1` are operational: menu navigation, bound hotkey, throttle, combined throttle/right and pause demonstrated. PowerShell parsing and actual script execution pass; the Rust input helper remains separately untested. See [macro setup](MACRO_SETUP.md).

Procmon 4.11 also completed the bounded race-load capture and native CSV export (exit 0), followed by streaming analysis. Detailed counts, verified resource hashes and coverage limits: [reference dependencies](REFERENCE_DEPENDENCIES.md).

## Live debugger and controlled numerics — 7 October

Installed x32dbg headless release 2026.05.27 is validated with disposable x86 breakpoint/detach/survival tests and twelve actual GRID placement-boundary captures. Explicit pause left a verified thread suspend count of one in a failed trial; guarded recovery restored menus. Upstream [pause-race fix PR 3906](https://github.com/x64dbg/x64dbg/pull/3906) supports this interpretation. The official latest release API still returns the installed version. Capture now supports `--arm-running`; original GRID survival after the successful capture remains unverified. [Detailed trials and limits](LIVE_SPAWN_CAPTURE.md).

Existing MSVC/Rust/Ghidra/Python tools build and execute isolated marker and outer dynamics probes. Debug/release Rust outer arithmetic agrees in 139 controlled cases; [dynamics contract](VEHICLE_UPDATE.md). Nothing newly installed for these continuations. Earlier statements about Bevy not yet being selected are historical; current pinned viewer is Bevy 0.19.1.

The same strict MSVC/SSE2 mapper now runs supplied-impulse (205 cases), conditional suspension (225 cases), point-response (253 cases), conditional wheel-coupling (192 cases) and coupled substep/contact/travel-stop (212 cases) helpers, all exactly matching Rust debug/release. Guarded read-only live context inspection and a bounded 2,000 ms throttle sequence also completed; profile files unchanged and display config restored. Exact tool versions and numerical scope remain recorded in [vehicle update](VEHICLE_UPDATE.md). No additional installations.

Supplied-state pose prediction (226 cases) and conditional two-pass finalization (200 cases) now match original whole helpers in debug/release; 62 project tests and Clippy pass. Same tools/versions, no new installations. [Scope and evidence](VEHICLE_UPDATE.md).

The initial suspension reader and unstubbed wheel initializer now compare 44 sources /968 words exactly in both build profiles, while 24 guarded runtime observations explicitly retain three changed fields. Existing tools reused; 63 tests and Clippy pass. New bounded validator `tools/validate_suspension_initialization.py`.

Preload (216 cases), suspension setup subset (200), range/default helpers (88), and combined retained-runtime chain (24 observations) now pass exact debug/release comparisons. **66 tests** and Clippy pass. All existing tools reused. Windows installer-name heuristic blocked the initially named setup probe before execution; renamed `suspension-prepare-probe.exe` runs without elevation. Detailed provenance and limits in [vehicle update](VEHICLE_UPDATE.md).

## Android ARM64 Rust target — 16 October 2026

Installed Rust std for `aarch64-linux-android` with rustup 1.29.1 for the pinned toolchain `1.99.0-x86_64-pc-windows-msvc` (`rustup target add aarch64-linux-android`; official rustup distribution, which verifies component hashes itself). It is used only for `cargo check`/`clippy --target aarch64-linux-android` into ignored `local/platforms/target-android`. No Android NDK, SDK, JDK, Gradle or cargo-ndk is installed. `grid-app` therefore stops at the NDK clang that blake3 and the GameActivity glue need. See [platforms and input](PLATFORMS_AND_INPUT_RESEARCH.md) section 1.1. The SSE measurement probes in `local/platforms/sse-approx/` were built with the same pinned 1.99.0 toolchain; nothing else was installed.
