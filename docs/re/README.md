# Shared reverse-engineering base

One place for what we know about addresses in the original `grid.exe`, so no session analyses
the same function twice. Started in F533 (21 October 2026).

## Workflow

**Before analysing an address**

1. `py -I tools/re/grid_re.py lookup 0x7BBB70` prints its `symbols.tsv` row (name, port, doc, F
   entries, confidence), live mentions in docs/log/Rust, and cached pseudocode. Add `--old` to
   also list earlier per-session decompiles under `local/**/decomp*` (slower).
   A plain `grep -i 7BBB70 docs/re/symbols.tsv` works too.
2. If it is already documented, read that doc first and continue from it.
3. Need pseudocode: `py -I tools/re/grid_re.py decompile 0x7BBB70 --print`. Output goes to
   `local/re-cache/<ENTRY>.c` in the main checkout and is reused by every worktree.

**After analysing**

4. Add or update rows in `docs/re/symbols.tsv` **in the same commit as your doc**, or re-run
   `py -I tools/re/mine_symbols.py` after writing the doc (it refreshes port/doc/entry/confidence
   for every address, adds new ones and never replaces a name that is already there).
5. If you renamed things in a Ghidra copy, export and merge them (below).

## Files

| Path | What |
|---|---|
| `docs/re/symbols.tsv` | Committed index, one row per original function/global (see columns) |
| `tools/re/grid_re.py` | `lookup`, `dis` (capstone), `xrefs`, `containing`, `decompile` |
| `tools/re/mine_symbols.py` | Mines addresses from `docs/`, `PROJECT_LOG.md`, `log/` and Rust comments |
| `tools/re/merge_symbols.py` | Merges a Ghidra `ExportSymbols` TSV into `symbols.tsv` without clobbering |
| `tools/re/master.py` | `build`, `apply`, `export`, `functions`, `decompile`, `snapshot` for the master project |
| `tools/re/harvest_projects.py` | Exports every old `local/**/*.gpr` project for merging |
| `tools/ghidra/ImportSymbols.java` | Applies `symbols.tsv` names + a `[symbols.tsv]` plate comment |
| `tools/ghidra/ExportSymbols.java` | Dumps user-defined names and comments to TSV |
| `tools/ghidra/FunctionTable.java` | Writes `local/re-cache/functions.tsv` (entry, end, name) |
| `tools/ghidra/DecompileCache.java` | Decompiles into `local/re-cache` |

Private (ignored, main checkout only, found from any worktree through `git rev-parse --git-common-dir`):
`local/ghidra-master/` (master project), `local/re-cache/` (pseudocode, `functions.tsv`),
`local/re-harvest/` (exports of old projects). Never commit pseudocode or exported comments that
quote it.

## symbols.tsv columns

Plain tab-separated UTF-8, no quoting, sorted by address. Addresses use the image base
0x400000, as in the docs.

| Column | Meaning |
|---|---|
| `address` | `0x00XXXXXX` |
| `name` | Our name. Mined only from a Rust item whose doc comment starts with the address, or from an identifier in backticks next to the address (`` `free_race_teams` (0x65FE40) ``). Generic Rust names get the module prefix (`continuous_mean`). Empty when nobody named it |
| `kind` | `func` (function entry), `label` (inside a function), `global`, `vtable`, `struct` |
| `port` | Rust `file:line crate::module::item` (up to three, newest first) or `not ported`. `crates/grid-probes` is not a port |
| `doc` | Up to three `docs/*.md:line`, the docs that mention the address most |
| `entry` | Up to three project-log entries (F/G ids) that mention it, newest first |
| `confidence` | Strongest label next to any mention: `[V]` verified > `[O]` observed in the original > `[D]` derived > `[I]` inferred; empty if no mention is labelled |
| `conflicts` | Other names seen for the address (Rust/backtick names, Ghidra exports) |
| `comment` | The leading mention's text; `note: ...` (hand-written) and `ghidra: ...` (merged) parts are kept on re-mining |

The mined columns are a navigation aid, not evidence: the doc is the record. Line numbers drift
as docs are edited; `lookup` greps live.

Mining rules: hex with `0x`, `FUN_/DAT_` names or A–F letters is taken when it falls in `.text`,
`.rdata` or `.data` (0x401000–0xC88400, 0xC89000–0xFB3074). Bare all-digit tokens such as
`451770` are taken only if they are a function entry in `local/re-cache/functions.tsv`, and not
next to units (`ms`, `bytes`, ...) or words such as `seed`/`tick`.

## Master Ghidra project

`C:\Dev\Reassembly\local\ghidra-master\grid-master.gpr`, program `grid-unpacked.exe`
(`local/compatibility/grid-unpacked.exe`, sha256 `aac46a64…4f34514`), Ghidra 12.1.4, JDK 21.0.9,
full auto-analysis without a time limit (the old `local/phase3` project stopped at 120 s).

Only one Ghidra process can open a project. The rule:

- **Read-only use** (decompile, export, `functions`): `master.py`/`grid_re.py` pass `-readOnly`
  and wait while `grid-master.lock` exists. Batch many addresses per call: each call pays ~30 s
  of JVM start-up.
- **Work that renames or retypes**: `py -I tools/re/master.py snapshot local/<topic>/ghidra`
  copies the master; work there (GUI or headless), then
  `py -I tools/re/master.py export --project local/<topic>/ghidra/grid-master.gpr --out local/<topic>/names.tsv`
  and `py -I tools/re/merge_symbols.py local/<topic>/names.tsv`; commit `symbols.tsv`.
- **Writing the master**: only the coordinator, when no session is reading:
  `py -I tools/re/master.py apply` re-applies `symbols.tsv`; then `master.py functions` and
  `py -I tools/re/mine_symbols.py`. Delete `local/re-cache/*.c` entries if names should refresh in
  pseudocode headers.

Rebuild from scratch (35 min of analysis on an otherwise quiet PC, 4 GB heap; run alone, the
PC is memory-bound): `py -I tools/re/master.py build --cpu 2`, then `master.py apply` and
`master.py functions`. `build` runs Ghidra at below-normal priority so it never competes with the
owner; under full CPU load it can stall. Short calls (decompile, export) run at normal priority.

## Tools and versions

- Python: any 3.12+ (`py -I` launcher 3.14.4, or the bundled 3.12.14); stdlib only, plus
- Capstone 5.0.6 from `local/tools/python` (installed with `--target`, docs/TOOLCHAIN.md); the
  scripts add that folder to `sys.path` themselves.
- Ghidra 12.1.4 headless at `C:\Users\debros\Tools\grid-research\ghidra_12.1.4_PUBLIC`, JDK 21.0.9
  from the Songs of Syx Demo `jre`, as in `tools/run_ghidra_*.ps1`.
- `grid_re.py xrefs` is a byte scan: rel32 `call`/`jmp`/`jcc` to the address and absolute 32-bit
  values equal to it in `.text`/`.rdata`/`.data`. It finds direct references only; for
  indirect calls through vtables look up the vtable row. Ghidra's own reference list:
  `tools/run_ghidra_address_refs.ps1`.
