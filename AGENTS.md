# Project working instructions

## User intent and authorization

Build an independent open-source engine for Race Driver: GRID (2008) using Rust and Bevy, reading assets locally from the owner's purchased Steam installation. No engine code existed when this project was transferred to this PC.

On 6 October 2026 the owner requested that all progress and findings be tracked in Markdown, and authorized reasonable actions to analyze/decompile the game and rewrite it in Rust, including installation of needed tools. This authorizes routine research, local diagnostic runs and tool setup; it is not authorization to publish original game assets or binary captures.

## Durable records

- Read `PROJECT_LOG.md` (the short state file) at the start of work. Entries are one file each in `log/` (`log/F0512.md` holds F512); grep `log/` for details (`grep -rn "F512 " log/`, `grep -rln "Redgate" log/`), never read all of it. `log/INDEX.md` lists every entry in one line.
- Record substantive work as a log entry before ending a turn: successful work, failed experiments, decisions, exact tool/build versions, evidence paths, uncertainty and next actions. Create it with `py -I tools/log_entry.py new "Title"` (next free number from freshly fetched origin/main and `log/`); do not edit `log/INDEX.md` or the generated index in `PROJECT_LOG.md` by hand. Edit the state sections of `PROJECT_LOG.md` only when an area's status, open problems or next actions change.
- Push flow: squash the work and its entry into one commit; `git fetch origin && git rebase origin/main` (if it stops on `log/` or `PROJECT_LOG.md`: `py -I tools/log_entry.py check --fix`, then `git rebase --continue`); `py -I tools/log_entry.py check --fix` (renumbers an entry whose number another session took, regenerates the indexes) and `git commit --amend` what it changed; `check` passes; push. On a rejected push, repeat from the fetch.
- Put detailed research in `docs/`; link it from the log entry. Do not rely on conversation memory as the only record of a finding.
- Distinguish observations from interpretations. Never promote a skipped test, tool installation or plausible hypothesis into verified compatibility.
- On 13 October 2026 the owner requested periodic playable prototype updates. Rebuild after verified driving changes, keep both launchers using the current release executable, and record the source revision, executable hash, checks and changes. Do not replace or close a prototype session the owner is currently running; continue independent work until the executable is available.

## Sessions and commits

- Commit as the owner: `git -c user.name='Evgeniy Lupashin' -c user.email=34810566+PipeKnight@users.noreply.github.com commit ...`. No `Co-Authored-By`, "Generated with" or other AI trailers. History was rewritten on 2026-10-21; never merge or push anything with pre-rewrite ancestry.
- Hand-off notes live in the session's own worktree, `local/handoff/<session-name>.md`. Child sessions cannot write to the main checkout, so do not copy them there.
- Long runs (builds, soaks, captures): start them in the background, end the turn and let the completion notification wake you once. Do not wake repeatedly to report the same running job.

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
- Check levels (docs/TOOLCHAIN.md, "Build and test cost"):
  - while working: tests of the changed package only (`cargo td -p CRATE`). One-off research probes live in `crates/grid-probes` and build only with `-p grid-probes` or `--workspace`;
  - before a push: `cargo t -- --include-ignored` with absolute GRID_* paths, `cargo clippy --workspace --all-targets -- -D warnings`, the solo-lap identity check (docs/TOOLCHAIN.md "Identity check": rebuild `grid-opponent-race` explicitly first, `cargo test` does not rebuild bin targets) and `grid-soak quick`, all in the release profile;
  - long soak runs and the separate `soak` profile: only overnight, or when the change targets robustness.
  Avoid building extra profiles you do not need.
- Check for existing tools before installing duplicates. Prefer official sources, pin and record versions, verify signatures/hashes where supplied, and prove useful operation against the actual GRID reference.

## Reverse-engineering base

- Before analysing an original address, run `py -I tools/re/grid_re.py lookup <addr>` (or grep `docs/re/symbols.tsv`) and read the doc it names. Decompile through the shared master project with `grid_re.py decompile`; the output is cached in the main checkout's `local/re-cache/`.
- After analysing, add or update rows in `docs/re/symbols.tsv` in the same commit as your doc (`py -I tools/re/mine_symbols.py` refreshes them from docs, the log and Rust comments, and never replaces an existing name).
- Do not create new Ghidra projects. Use the master `local/ghidra-master` read-only, or a `master.py snapshot` copy for renaming work, and merge its names back (docs/re/README.md).
