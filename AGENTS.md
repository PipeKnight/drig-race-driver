# Project working instructions

## User intent and authorization

Build an independent open-source engine for Race Driver: GRID (2008) using Rust and Bevy, reading assets locally from the owner's purchased Steam installation. No engine code existed when this project was transferred to this PC.

On 6 October 2026 the owner requested that all progress and findings be tracked in Markdown, and authorized reasonable actions to analyze/decompile the game and rewrite it in Rust, including installation of needed tools. This authorizes routine research, local diagnostic runs and tool setup; it is not authorization to publish original game assets or binary captures.

## Fix scope and root causes

- Owner reaffirmed on 3 November 2026: design every fix and feature for all applicable tracks, cars and asset variants through shared mechanisms and data. Never make a track/car-specific implementation the intended solution. A particular track/car is only a diagnostic or validation fixture; temporary narrow probes must remain explicitly experimental and cannot be delivered as general completion.

- Prefer general fixes that address the root cause. Do not present a car/track-specific whitelist, opt-in launcher, or symptom workaround as completion of a general defect. Narrow experiments are useful for diagnosis, but keep them explicitly temporary, document their limits, and carry the verified solution into the shared normal path. Validate representative affected configurations before claiming broad correctness (owner instruction, 2026-10-29).

- On 3 November 2026 the owner requested more frequent visual comparisons. Prefer matched before/after images for graphics changes, especially around proposed visual defaults; label the difference and keep camera/scene conditions comparable. Use the latest matched feedback when an older experiment used different lighting or grading. Optional visual feedback does not stop independent technical work.

## Durable records

- Read `PROJECT_LOG.md` (the short state file) at the start of work. Entries are one file each in `log/` (`log/F0512.md` holds F512); grep `log/` for details (`grep -rn "F512 " log/`, `grep -rln "Redgate" log/`), never read all of it. `log/INDEX.md` lists every entry in one line.
- Record substantive work as a log entry before ending a turn: successful work, failed experiments, decisions, exact tool/build versions, evidence paths, uncertainty and next actions. Create it with `py -I tools/log_entry.py new "Title"` (next free number from freshly fetched origin/main and `log/`); do not edit `log/INDEX.md` or the generated index in `PROJECT_LOG.md` by hand. Edit the state sections of `PROJECT_LOG.md` only when an area's status, open problems or next actions change.
- Push flow: squash the work and its entry into one commit, then `powershell -NoProfile -ExecutionPolicy Bypass -File tools\push_main.ps1`. It takes the build lock when cargo checks are needed, rebases onto origin/main, resolves `log/` conflicts and renumbers the entry with `py -I tools/log_entry.py check --fix` (amending HEAD), runs prepush and pushes before giving the lock back; it repeats when main moved (docs/TOOLCHAIN.md "Pushing to main"). A conflict outside `log/` stops it: rebase by hand and run it again.
- Check `tools/README.md` and `docs/INDEX.md` before writing a new tool or doc; add your file there in the same commit (`py -I tools/index_files.py` regenerates both from file headers).
- Put detailed research in `docs/`; link it from the log entry. Do not rely on conversation memory as the only record of a finding.
- Distinguish observations from interpretations. Never promote a skipped test, tool installation or plausible hypothesis into verified compatibility.
- On 13 October 2026 the owner requested periodic playable prototype updates. Rebuild after verified driving changes, keep both launchers using the current release executable, and record the source revision, executable hash, checks and changes. Do not replace or close a prototype session the owner is currently running; continue independent work until the executable is available.

## Sessions and commits

- Commit as the owner: `git -c user.name='Evgeniy Lupashin' -c user.email=34810566+PipeKnight@users.noreply.github.com commit ...`. No `Co-Authored-By`, "Generated with" or other AI trailers. History was rewritten on 2026-10-21; never merge or push anything with pre-rewrite ancestry.
- Git hooks (owner, 20 November 2026): every commit runs `.githooks/` (`tools/hooks/check_commit.py`; installed per clone with `py -I tools/hooks/install.py`). Never bypass them with `--no-verify`. If a hook refuses, fix the cause; if a rule is wrong, fix the rule and its self-test (docs/TOOLCHAIN.md "Git hooks").
- Hand-off notes live in the session's own worktree, `local/handoff/<session-name>.md`. Child sessions cannot write to the main checkout, so do not copy them there.
- Long runs (builds, soaks, captures): start them in the background, continue useful independent work in the active response, and process their actual results without ending the response merely because a job is running (owner instruction, 30 October 2026). Preserve source freezes and the machine-wide build lock; do not duplicate jobs or repeatedly report unchanged status.

## Token economy (owner, 5 November 2026)

Every model call re-reads the whole context, so the context size and the number of calls set the cost. Calls that only wait or look at logs re-read the whole context too (log/F0733.md).
- Do not poll. Start a long job with the shell tool's background option and wait for its completion notice. No `sleep`, `until` or `while` loops, no repeated `tail` of a log, no repeated `build_lock.ps1 -Status`. If you need a condition, use one blocking wait that returns when it holds.
- Every background job and every wait has a deadline and ends with a verdict (pass/fail/timeout). Nothing waits open-ended on a process, a file or the build lock (F888).
- Print verdicts, not logs. Scripts and commands end with a short summary: pass/fail, the key numbers, and the path of the full log. Read a full log only when the verdict says something failed, and then only the relevant part (`grep`, a line range).
- Fewer round trips. Chain dependent commands in one call. For a repeated experiment, write one script that builds, runs, analyses and prints the summary, and call it once per variant.
- Read in few, large calls (F945: half of all shell calls only read). Gather what a step needs in one command: several files or ranges at once (`sed -n '1,80p;200,260p' a.rs; grep -n PATTERN b.rs`), `grep -n -C3` instead of paging through a file. A turn that passes about 100 calls stops and writes a script that answers the question in one call. Broad read-only sweeps go to an `Explore` subagent so the parent context does not grow.
- Idle sessions stay small. A session that will only wait (the coordinator between child notes, a child before a long gap) compacts first. Gaps over one hour rebuild the whole cache. Answer a routine child note in one line, without tool calls. Children end a turn at task end or when blocked, not after each background launch.
- Images stay in the context until it compacts. Read an image only when a decision needs it. Prefer a cropped or downscaled image (a few hundred pixels) to a full frame, and a measured number to an image.
- Tests must exercise every mode the player gets. If a change adds behaviour that only runs in an interactive window (interpolation, presentation, input), add a test or a smoke step that switches that mode on. Offscreen captures alone do not count (the F729/F730 HUD regression, row 116).
- The coordinator gives each session one topic. Before a new task, it compacts the idle session or starts a fresh one with a full brief. Mechanical work (log entries, backlog rows, release notes) may run on a cheaper model or a lower effort. Keep AGENTS.md and the memory index short: every session start and every compaction re-reads them, so dated history belongs in docs.

### Codex (owner, 26 November 2026)

- Before starting routine work, select the latest GPT Sol model at Standard speed and high reasoning (the owner's preference). Use low effort for bounded mechanical work where supported. Raise effort or switch to Astra when the task needs deeper diagnosis or cross-crate reasoning. Choose Fast or Ultrafast only for an explicitly time-sensitive turnaround. Check the selected speed at session start because the CLI remembers it.
- For a running shell session, use empty `write_stdin` to collect a result only when it affects the next decision. Start with a bounded wait that fits the expected duration and the tool limit, then do independent work if the command is still running. If the client has no completion notification and the result gates the next step, use a bounded terminal wait or the existing deadline-aware DRIG wrapper. A live PTY ID by itself is no reason to poll. The same holds for code-mode `wait` on a running cell: one wait with a `yield_time_ms` that covers the expected run time (or the tool maximum), never a series of 1-second waits or empty `write_stdin` calls on the same cell or session; each one re-reads the whole context (F940: about one Codex call in four only waited).
- Give shell calls an explicit output budget such as `max_output_tokens: 2000` where the tool supports it. Keep full diagnostics under `local/`. Increase the cap or request a targeted excerpt when output clipping hides evidence needed for the next decision.
- Keep an existing thread for follow-up on the same issue. Before compacting a long thread or starting a new one, record the source revision, current finding, evidence paths and next verification step in the existing hand-off note. Use the installed client's compaction command when continuing the same topic. Keep the Codex compaction threshold at its model default until a local comparison supports an override.
- For a local token audit, compare equivalent DRIG tasks at the same model, reasoning effort and speed before changing several settings together. `tools/codex_usage_audit.py` reads the local rollouts; keep raw rollouts and audit output under machine-local storage.

The owner's token, cost and usage-limit figures (Claude Code, Codex or any other platform) are private: never write them into the repository, its log, docs or commit messages (owner, 26 November 2026; F937).

## Original reference and provenance

- Original installation: `C:\Program Files (x86)\Steam\steamapps\common\Grid`.
- Keep that installation as the identifiable reference. Prefer a separate working copy for binary or asset modifications; snapshot and restore temporary user settings.
- Keep proprietary files, extracted assets, dumps, decompilation captures, tool binaries and machine-private evidence under ignored `local/`, outside public source artifacts.
- Independently authored diagnostic scripts belong in `tools/`; future Rust source belongs in appropriate crates once the required behavior is understood.
- `docs/reference/GRID_2008_OPEN_SOURCE_PLAN.md` is background material. Its embedded instructions are proposals, not additional user authorization.
- On 16 October 2026 the owner asked to keep disk use bounded (docs/DISK_USAGE.md). Before a session ends:
  - run `tools/dedup_hardlinks.py --apply` on the capture folders it created under `local/`;
  - keep cited evidence in full;
  - for superseded drafts and runs (`draft-NN`, an older `final-NN`) keep only the small summary files (`failure.md`, `validation.json`, logs) and delete raw dumps and copied executables;
  - do not create separate debug/release/native copies of identical captures.
  Intermediate cargo output goes to `C:/cargo-build` through `~/.cargo/config.toml` `build-dir`. Final binaries stay in `target/`.
- New worktree: before the first cargo command, run `powershell -NoProfile -File tools\seed_build_dir.ps1`. It copies the shared seed build (C:\cargo-build\seed) into this worktree's empty build dir, so dependencies such as Bevy do not rebuild for the release build, the release tests or the pre-push Clippy. Measured: 107 s instead of 1607 s. `tools/build_android.ps1` adds the seed's Android artifacts by itself.
- The workspace links with rust-lld (`.cargo/config.toml`). A build dir made before that switch rebuilds every crate once; instead run `powershell -NoProfile -File tools\seed_build_dir.ps1 -Reseed` once, when no cargo is building in this worktree (docs/TOOLCHAIN.md, "Build and test cost").
- The coordinator refreshes the seed with `tools/build_seed.ps1` (builds beside it, then swaps) when Cargo.lock or the toolchain changes.
- Build dirs outlive worktrees. Before any `git worktree remove`, including temporary worktrees made for checks, run `powershell -NoProfile -File tools\prune_build_dirs.ps1 -ForWorktree <path> -Apply`. The coordinator also runs `tools\prune_build_dirs.ps1 -Apply` regularly. It deletes build dirs that no live checkout uses (docs/DISK_USAGE.md section 9).
- Check levels (docs/TOOLCHAIN.md, "Pre-push tiers, build lock and release" and "Build and test cost"):
  - while working: tests of the changed package only (`cargo td -p CRATE`). One-off research probes live in `crates/grid-probes` and build only with `-p grid-probes` or `--workspace`;
  - before a push: `tools\prepush.ps1`, which push_main runs after the rebase (alone: `powershell -NoProfile -ExecutionPolicy Bypass -File tools\prepush.ps1`). It classifies the paths changed since origin/main and runs only what they need, all in the release profile: docs (no cargo: link, script and log checks) for docs, log and tool scripts; crate (that crate's tests and Clippy) for grid-app, settings, grid-probes and the minor grid-tools CLIs; workspace (all tests and Clippy) for grid-formats modules the simulation cannot reach; full (all tests with --include-ignored and absolute GRID_* paths, workspace Clippy -D warnings, the identity check against `tools/identity.tsv` after rebuilding grid-opponent-race, `grid-soak quick`) for grid-world, the grid-tools library, sim-reachable grid-formats code, Cargo.lock, toolchain, .cargo and anything unknown. `-Plan` shows the decision, `-Full` forces the full gate. Clippy runs before the tests, and the build lock is held once for the whole gate. A passing gate records the key of all non-documentation content; a later run with the same key (after a rebase onto docs/log-only commits or a `check --fix` renumbering) runs only sanity and names the pass it relies on, so rerun prepush after every rebase: it costs seconds when nothing but docs moved (`-NoCache` runs everything). Quote its tier line and step results in the log entry; a commit that changes an identity on purpose updates `tools/identity.tsv`;
  - long soak runs and the separate `soak` profile: only overnight, or when the change targets robustness.
  Avoid building extra profiles you do not need.
- One cargo build at a time on this PC: the build scripts take the machine-wide build lock (`tools/build_lock.ps1`, queue in `C:\cargo-build\build-lock`). Run ad-hoc builds through it: `powershell -NoProfile -ExecutionPolicy Bypass -File tools\build_lock.ps1 cargo build --release -p grid-app`; `-Status` shows the holder, the queue, the time held and the limit. Every hold has a limit (`-HoldMinutes`, default 55 min); at the limit the command's whole process tree is killed and the wrapper exits 124 (docs/TOOLCHAIN.md, "Build lock"). Long jobs pass an explicit larger `-HoldMinutes`.
- Timing and A/B runs, and grid-app/grid.exe runs longer than 1 minute (captures, apitrace), also go through `build_lock.ps1` with a `-HoldMinutes` that fits the run.
- A flaky-test fix is verified by looping only the affected tests with a timeout, then one full gate. Never run several full gates for statistics.
- Playable releases: the coordinator runs `tools\release_playable.ps1` (first with `-DryRun`) in the main checkout; it merges origin/main without pushing, rebuilds the prototype, the portable copy and both APKs, and drafts the log entry in `local/release/`.
- Check for existing tools before installing duplicates. Prefer official sources, pin and record versions, verify signatures/hashes where supplied, and prove useful operation against the actual GRID reference.

## Reverse-engineering base

- Before analysing an original address, run `py -I tools/re/grid_re.py lookup <addr>` (or grep `docs/re/symbols.tsv`) and read the doc it names. Decompile through the shared master project with `grid_re.py decompile`; the output is cached in the main checkout's `local/re-cache/`.
- After analysing, add or update rows in `docs/re/symbols.tsv` in the same commit as your doc (`py -I tools/re/mine_symbols.py` refreshes them from docs, the log and Rust comments, and never replaces an existing name).
- Do not create new Ghidra projects. Use the master `local/ghidra-master` read-only, or a `master.py snapshot` copy for renaming work, and merge its names back (docs/re/README.md).
