# Claude Code token audit: where the calls go and how to make fewer, smaller ones

Measured 2026-11-26 by Claude Code with `tools/claude_usage_audit.py` (F945). It
repeats by script what F733 measured by hand on 5 November, and mirrors
`tools/codex_usage_audit.py` (F939/F940) for Codex.

The owner's token, cost and usage-limit figures are private (AGENTS.md, F937). This document
holds counts of calls, turns, compactions and images, and shares (percentages) only. The raw
JSON with token figures stays under `local/claude-audit/`.

## Method

```
py -I tools/claude_usage_audit.py --since 2026-11-19 --out local/claude-audit/since-2026-11-19.json
py -I tools/claude_usage_audit.py --since 2026-11-19 --public --out local/claude-audit/public.json
```

The script streams every `*.jsonl` under `~/.claude/projects/C--Dev-Reassembly*/` line by line:
the main checkout's sessions, every child session in a worktree folder and every subagent
transcript (`<session>/subagents/agent-*.jsonl`). Standard library only, nothing leaves the
machine. Records the metrics do not need (`api-request*`, `file-history*`, `cost-state`, queue
bookkeeping) are skipped unparsed. `--public` drops every token figure, so its output may be
quoted.

## Definitions

- **API call**: one unique assistant `message.id`. A transcript stores one record per content
  block (thinking, text, each tool_use), all carrying the same `usage`; the script counts the
  usage once. Message ids and tool ids are deduplicated across files, oldest transcript first,
  so history a fork copies counts once.
- **Context of a call**: `input_tokens + cache_read_input_tokens + cache_creation_input_tokens`,
  i.e. everything the model re-read for that call.
- **Turn**: starts at a user record that is not a tool result, not meta and not a compaction
  summary. Its **origin** is `human`, `task-notification` (a background job or a subagent
  finished), `peer` (a cross-session message, e.g. from the coordinator), `side-sessions-event`,
  `slash-command`, or `continuation` (a transcript that starts mid-turn).
- **Observe-only call**: an API call whose every tool call only waits or looks:
  a shell command classified `observe`, or `Monitor`, `TaskOutput`, `ScheduleWakeup`,
  session status listings, `read_terminal`. **Bookkeeping-only call**: only
  `update_plan`/`wrap_up`/`mark_chapter`.
- **Shell categories** (Bash and PowerShell, after unwrapping `powershell -Command`, split into
  statements on newlines, `;`, `&&`, `||`; a pipeline is judged by its first command):
  - `observe`: sleep/`Start-Sleep`/`timeout`/`Wait-Process`, any `while`/`until`/`for` loop that
    sleeps, `tail`, `Get-Content -Tail/-Wait`, `cat log | tail`, `Get-Process`/`tasklist`/`ps`,
    `build_lock.ps1 -Status`; also a sleep followed only by reads. A command started with
    `run_in_background` is never `observe`;
  - `build_test` (cargo, prepush, build_lock with a command, build scripts, gradle),
    `run` (py, powershell -File, `&`, adb, grid-app), `git`, `git_check` (status, log,
    diff --stat, fetch, rev-parse, worktree list), `edit` (sed -i, redirects, Set-Content,
    file moves), `read` (cat, head, grep, ls, Get-Content, Select-String), `other`.
  - The script also counts a shell command repeated in one transcript with byte-identical
    output (`shell_same_command_same_output_repeats`).
- **Cache rebuild event**: a call with at least 20,000 `cache_creation` tokens, attributed to:
  first call of a transcript, after a compaction, idle gap longer than the cache TTL (1 h here;
  5 min when the 5-minute bucket dominates), a model or effort change, or other.
- **Idle turn**: a non-human turn of at most 3 API calls that calls no tool except bookkeeping,
  e.g. a coordinator acknowledging a "still running" note. **Waiting-text turn**: such a short
  non-human turn whose last text says it is still waiting, unchanged or similar.
- **Large tool result**: at least 16 KiB of text in one `tool_result`; estimated tokens =
  bytes / 4. **Whole-file read**: `Read` without `offset`/`limit`. **Repeated read**: a later
  `Read` of a path already read in the same transcript.
- **Images**: image blocks in tool results and user messages; estimated tokens use the API rule
  (long edge scaled to 1568 px, then width × height / 750).

## Findings (26 November 2026)

Three windows were measured: all transcripts (the earliest from October), since 6 November
(after F733) and since 19 November. Counts and shares of API calls only.

| Measure | All | Since 6 Nov | Since 19 Nov |
|---|---|---|---|
| API calls (sessions + subagents) | 79,697 | 32,908 | 9,677 |
| Sessions / subagent transcripts with calls | 121 / 72 | 49 / 18 | 20 / 2 |
| Observe-only calls | 3.3 % | 2.3 % | 2.3 % |
| Bookkeeping-only calls | 1.3 % | 1.2 % | 2.2 % |
| Shell calls classified `observe` | 3.2 % | 2.1 % | 2.6 % |
| Shell calls classified `read` | 48.7 % | 54.5 % | 52.0 % |
| Compactions (auto / manual) | 711 / 73 | 611 / 9 | 182 / 5 |
| API calls per compaction | 102 | 53 | 52 |
| Idle turns / their share of calls | 608 / 0.8 % | 238 / 0.7 % | 159 / 1.7 % |
| Images read | 1,104 | 216 | 26 |

1. **Polling is solved.** F733 found 28 % of shell calls only waited; now 2–3 % do, and only
   8 shell commands in all transcripts were re-run with byte-identical output while classified
   `observe`. The no-polling rule works and should stay.
2. **The number of calls times the context per call is the cost.** 97–99 % of the re-read
   context is cache reads; cache creation (writes) is 1–2.5 %. So the lever is fewer calls and
   a smaller context per call, not cache hits.
3. **The context per call is now capped by compaction.** Since 6 November every compaction
   fires at nearly the same context size (p50 and p95 within 2 %), and the mean context per
   call is 0.56 × the all-time mean; before that, single sessions ran at up to 6 × that size.
   The price is a compaction every 52 calls; each one re-reads the whole context once to
   summarise, then rebuilds the cache, and re-injects the session preamble (point 5).
4. **Reading through the shell is half of all shell calls.** 52–55 % of shell calls are `read`
   (`sed -n`, `cat`, `grep`, `ls`), typically one 40–200-line range per call. Bash produces 85 %
   of all tool-result text. The ten longest turns since 19 November ran 114–272 calls each
   (18 % of all calls in the window), mostly Bash reads of source files; every one of those
   calls re-reads the whole context.
5. **The preamble re-injected after each compaction is large.** Per injection, measured as
   rendered text: skill listing about 19 KB, project instructions (AGENTS.md plus the memory
   index) about 18 KB, deferred-tool list about 11 KB, agent listing about 4 KB, MCP server
   instructions about 1.6 KB. Together about 50 KB, i.e. a substantial part of a freshly
   compacted context, injected again after each of the 620 compactions since 6 November. The
   deferred-tool list names more than 100 connector tools (Notion, calendar, Chrome, meeting
   notes) that this project never calls.
6. **Coordinator idle turns are the main cross-session waste.** Since 19 November 158 of the
   222 `side-sessions-event` turns were idle (the coordinator read a child's note and did
   nothing else); they are 11 % of the coordinator's calls, each at the coordinator's full
   context. `still waiting` text turns are rarer (20 since 19 November).
7. **Cache rebuilds** after an idle gap longer than the 1-hour TTL (86 since 19 November) and
   after compactions (186) are about equal, and together about half of all cache writes. They
   are a small part of the total (point 2).
8. **Large single results and images are no longer a problem.** Since 19 November: 33 tool
   results of 16 KiB or more (most of them whole-file `Read`s of persisted tool outputs and
   long scripts), 33 repeated reads of an already-read file, 26 images (4 above the API's
   1568-px downscale edge).
9. **Almost all calls run at one setting.** Since 6 November 99.6 % of calls ran on Opus at
   high effort; the cheaper model or lower effort that AGENTS.md allows for mechanical work was
   used for 0.4 % of calls.

## Recommendations, ranked by expected effect

1. **Read in fewer, bigger shell calls** (point 4). Proposed AGENTS.md line under "Token
   economy": *"Gather what a step needs in one command: several files or ranges per call
   (`sed -n '1,80p;200,260p' a.rs; grep -n PATTERN b.rs`), `grep -n -C3` instead of paging a file;
   a turn that passes about 100 calls should stop and write a script that answers the question
   in one call."* Halving the shell `read` calls would remove about a fifth of all API calls.
2. **Trim the per-compaction preamble** (point 5), owner settings, no code: disable the
   connectors and plugins this project never uses for this folder (each removes its tools from
   the deferred list and its skills from the skill listing), and keep AGENTS.md and
   `MEMORY.md` short: move dated history out of AGENTS.md into docs, drop memory index lines
   that no longer apply. Each kilobyte saved there is saved again at every compaction and
   every session start.
3. **Coordinator: compact before waiting, answer notes silently** (point 6). When the
   coordinator has nothing to do but wait for children, `/compact` it first so each idle turn
   re-reads a small context; reply to a routine child note with no tool call and no text
   beyond one line; ask children to end their turns only at task end or when blocked, not
   after each background launch.
4. **Use the allowed cheaper settings** (point 9). Log entries, backlog rows, index updates and
   release notes on a lower effort or cheaper model, as AGENTS.md already permits; read-only
   sweeps in an `Explore` subagent so the parent context does not grow.
5. **Keep the compaction cap; avoid idle gaps over one hour in big sessions** (points 3 and 7).
   A session that will sit idle for hours should compact or be released first, so the cache
   rebuild after the gap is small.
6. **Tool output caps stay as they are** (point 8): verdict-only scripts, `head -c`/`grep`
   on logs, cropped images. They are already followed; no new rule needed.

Re-run the script after a rule change and compare the shares (`--since` the change date).
