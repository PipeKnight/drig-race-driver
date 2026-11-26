#!/usr/bin/env python3
"""Offline audit of local Codex rollout logs: token totals, waits, repeated empty write_stdin calls and large tool output per project.

Reads ~/.codex/sessions rollout JSONL (plain or .zst) for the given project roots and prints one
JSON summary to stdout. Python 3.10+, standard library only (`zstd` on PATH for .zst files).
Nothing goes over the network and nothing is written except stdout:

    py -I tools/codex_usage_audit.py --project-root ~/.codex/worktrees --project-root C:/Dev/Reassembly --since 2026-11-01 > local/codex-audit/summary.json

A root matches every rollout whose cwd lies under it. Codex sessions usually run in their own
worktrees under ~/.codex/worktrees, so pass that folder too; the main checkout alone matches few threads.

Newer Codex builds run tools in code mode: a top-level `exec` call holds a JavaScript cell that
calls tools.exec_command, tools.write_stdin and others, and `wait` waits for a running cell. The
audit reads the calls inside each cell (inner_tool_calls_by_name) and reports the share of
top-level calls that only observe (observation_share_of_top_level_calls).

The owner's token and usage figures are private: keep the output under local/ and never put its
numbers into the repository, the log or commit messages (AGENTS.md, "Codex").
"""
from __future__ import annotations

import argparse
from collections import Counter, defaultdict
from contextlib import contextmanager
from datetime import datetime, timezone
import hashlib
import json
import math
import ntpath
import os
from pathlib import Path
import posixpath
import re
import shutil
import statistics
import subprocess
import sys

CORE = ("input_tokens", "cached_input_tokens", "output_tokens", "reasoning_output_tokens")
CACHE_WRITE = "cache_write_input_tokens"
EXEC = {"exec_command", "shell_command", "shell", "local_shell", "local_shell_call"}
CALLS = {
    "function_call", "custom_tool_call", "local_shell_call",
    "web_search_call", "image_generation_call", "tool_search_call",
}
OUTPUTS = {"function_call_output", "custom_tool_call_output", "tool_search_output"}
UNKNOWN = ("unknown", "unknown", "unknown")
COMPLETION = re.compile(r"^(?:turn_complete|task_complete|turn_aborted)$")
STATUS = re.compile(
    r"^(?:sleep|start-sleep|tail|wait-process|wait-job|pgrep|tasklist|get-process|"
    r"get-job|docker\s+(?:ps|logs)|kubectl\s+(?:get\s+pods|logs)|"
    r"(?:[\w./\\-]+[\\/])?build_lock\.ps1\b.*?\s-status\b)(?:\s|$)",
    re.IGNORECASE,
)
WRAPPED_LOCK_STATUS = re.compile(
    r"^(?:powershell(?:\.exe)?|pwsh(?:\.exe)?)\s+.*\s-file\s+"
    r"[^\s]*build_lock\.ps1\s+.*?-status\b", re.IGNORECASE,
)
WIN = re.compile(r"^(?:[A-Za-z]:|\\\\|//)")
# Codex code mode: one top-level `exec` call runs a JavaScript cell that calls tools.exec_command,
# tools.write_stdin and other tools inside it; `wait` and `sleep` only wait for a running cell.
CODE_CELLS = {"exec"}
WAITS = {"wait", "sleep"}
INNER_CALL = re.compile(r"\btools\.(\w+)\s*\(")
JS_STRING = r"(\"(?:[^\"\\]|\\.)*\"|'(?:[^'\\]|\\.)*'|`(?:[^`\\]|\\.)*`)"
JS_KEY = r"[\"']?{0}[\"']?\s*:\s*"
STILL_RUNNING = b"Script running"


def mapping(value):
    return value if isinstance(value, dict) else {}


def label(value):
    if isinstance(value, dict):
        for key in ("effort", "reasoning_effort", "type"):
            if key in value:
                return label(value[key])
    if isinstance(value, str) and re.fullmatch(r"[a-zA-Z0-9_.:/-]{1,100}", value):
        return value
    return "unknown"


def as_time(value):
    parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    if parsed.tzinfo is None:
        raise ValueError("timestamp requires a UTC offset")
    return parsed.astimezone(timezone.utc)


def cli_time(value):
    try:
        return as_time(value + "T00:00:00Z" if len(value) == 10 else value)
    except ValueError as error:
        raise argparse.ArgumentTypeError("use YYYY-MM-DD (UTC) or ISO 8601 with an offset") from error


def normalize(path):
    if WIN.match(path):
        return ntpath.normcase(ntpath.normpath(path)).replace("\\", "/")
    return posixpath.normpath(path)


def roots_for(values):
    return [
        normalize(str(Path(value).expanduser().resolve()) if not WIN.match(value)
                  and not value.startswith("/") else value).rstrip("/")
        for value in values
    ]


def in_project(path, roots):
    if not isinstance(path, str) or not path:
        return False
    path = normalize(path)
    return any(path == root or path.startswith(root + "/") for root in roots)


def usage(value):
    raw = mapping(value)
    if any(type(raw.get(key)) is not int or raw[key] < 0 for key in CORE):
        return None
    result = {key: raw[key] for key in CORE}
    if type(raw.get(CACHE_WRITE)) is int and raw[CACHE_WRITE] >= 0:
        result[CACHE_WRITE] = raw[CACHE_WRITE]
    if (result["cached_input_tokens"] > result["input_tokens"]
            or result["reasoning_output_tokens"] > result["output_tokens"]):
        return None
    return result


def parse_args(value):
    if isinstance(value, str):
        try:
            return mapping(json.loads(value))
        except ValueError:
            return {}
    return mapping(value)


def command_text(args):
    value = args.get("cmd", args.get("command"))
    if isinstance(value, str):
        return value.strip()
    if isinstance(value, list) and all(isinstance(part, str) for part in value):
        if len(value) >= 3 and value[-2] in {"-c", "-lc", "-Command"}:
            return value[-1].strip()
        return " ".join(value).strip()
    return ""


def status_only(command):
    if re.search(r"[;&|<>\n\x60]|\$\(", command):
        return False
    return bool(STATUS.match(command) or WRAPPED_LOCK_STATUS.match(command))


def js_literal(text):
    """Value of a JavaScript string literal; template ${} parts stay as written."""
    quote, body = text[0], text[1:-1]
    if quote == '"':
        try:
            return json.loads(text)
        except ValueError:
            return body
    return re.sub(r"\\(.)", r"\1", body)


def call_span(source, start):
    """Argument text of a call whose opening parenthesis ends at `start`, skipping strings."""
    depth, index, quote = 1, start, None
    while index < len(source):
        char = source[index]
        if quote:
            if char == "\\":
                index += 1
            elif char == quote:
                quote = None
        elif char in "\"'`":
            quote = char
        elif char == "(":
            depth += 1
        elif char == ")":
            depth -= 1
            if depth == 0:
                return source[start:index]
        index += 1
    return source[start:]


def cell_calls(source):
    """(name, args) for every tools.NAME(...) call in a code-mode cell; args hold the fields we read."""
    calls = []
    for match in INNER_CALL.finditer(source or ""):
        text = call_span(source, match.end())
        args = {}
        for key in ("cmd", "command", "workdir", "chars"):
            found = re.search(r"(?<![\w$])" + JS_KEY.format(key) + JS_STRING, text)
            if found:
                args[key] = js_literal(found.group(1))
        session = re.search(r"(?<![\w$])" + JS_KEY.format("session_id") + r"([\w.\"'-]+)", text)
        if session:
            args["session_id"] = session.group(1).strip("\"'")
        calls.append((match.group(1), args))
    return calls


def body_bytes(value):
    if isinstance(value, str):
        return value.encode("utf-8", errors="replace")
    if isinstance(value, (list, dict)):
        return json.dumps(value, ensure_ascii=False, sort_keys=True).encode("utf-8", errors="replace")
    return None


@contextmanager
def open_rollout(path):
    if path.suffix != ".zst":
        with path.open("rb") as stream:
            yield stream
        return
    try:
        from compression import zstd
    except ImportError:
        zstd = None
    if zstd is not None:
        try:
            with zstd.open(path, "rb") as stream:
                yield stream
        except zstd.ZstdError as error:
            raise OSError("could not fully decompress rollout") from error
        return
    binary = shutil.which("zstd")
    if not binary:
        raise OSError("compressed rollout requires Python 3.14 or zstd")
    with subprocess.Popen(
        [binary, "-dc", "--", str(path)], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
    ) as proc:
        with proc.stdout:
            yield proc.stdout
        if proc.wait() != 0:
            raise OSError("zstd could not fully decompress rollout")


class Totals:
    def __init__(self):
        self.tokens = Counter()
        self.sources = Counter()
        self.inputs = []
        self.cache_write_records = 0

    def add(self, values, source, single):
        self.tokens.update(values)
        self.sources[source] += 1
        self.cache_write_records += CACHE_WRITE in values
        if single:
            self.inputs.append(values["input_tokens"])

    def summary(self):
        tokens = {key: self.tokens[key] for key in CORE} if self.sources else {}
        if tokens:
            tokens["uncached_input_tokens"] = tokens["input_tokens"] - tokens["cached_input_tokens"]
            tokens["input_plus_output_tokens"] = tokens["input_tokens"] + tokens["output_tokens"]
            tokens["cache_write_reported_sum"] = (
                self.tokens[CACHE_WRITE] if self.cache_write_records else None
            )
        samples = sorted(self.inputs)
        return {
            "tokens": tokens, "usage_buckets": dict(self.sources),
            "cache_write_buckets_with_field": self.cache_write_records,
            "input_tokens_per_observed_response": {
                "samples": len(samples),
                "mean": round(statistics.mean(samples)) if samples else None,
                "p50": round(statistics.median(samples)) if samples else None,
                "p95": samples[math.ceil(0.95 * len(samples)) - 1] if samples else None,
            },
        }


class Audit:
    def __init__(self, args):
        self.args = args
        self.roots = roots_for(args.project_root)
        self.quality = Counter()
        self.tools = Counter()
        self.inner_tools = Counter()
        self.observing_calls = 0
        self.flags = Counter()
        self.examples = defaultdict(list)
        self.compactions = 0
        self.selected_threads = set()
        self.seen_records = set()
        self.seen_calls = set()
        self.seen_legacy = set()
        self.total = Totals()
        self.groups = defaultdict(Totals)
        self.thread_totals = defaultdict(Totals)
        self.large_outputs = 0
        self.max_output = 0
        self.output_bytes = 0
        self.outputs = 0

    def within(self, stamp, cwd):
        return (in_project(cwd, self.roots)
                and (self.args.since is None or stamp >= self.args.since)
                and (self.args.until is None or stamp < self.args.until))

    def flag(self, name, location):
        self.flags[name] += 1
        if len(self.examples[name]) < self.args.examples:
            self.examples[name].append(location)

    def account(self, values, source, single, context, thread):
        self.total.add(values, source, single)
        self.groups[context].add(values, source, single)
        self.thread_totals[thread].add(values, source, single)

    def scan(self, path, stream):
        cwd = ""
        context = UNKNOWN
        thread = path.name.removesuffix(".zst")
        turn = None
        fork = False
        previous = {}
        pending = {}
        repeated = {}
        polled = set()
        contexts = {}
        legacy = []
        records = []
        specific_turns = set()
        fallback_window = False
        for number, line in enumerate(stream, 1):
            try:
                row = mapping(json.loads(line))
                stamp = as_time(row["timestamp"])
                payload = mapping(row["payload"])
            except (ValueError, TypeError, KeyError, AttributeError):
                self.quality["invalid_or_incompatible_lines"] += 1
                continue
            kind = row.get("type")
            event = payload.get("type")
            if kind == "session_meta":
                thread = str(payload.get("id") or thread)
                cwd = payload.get("cwd", cwd)
                fork = bool(payload.get("forked_from_id"))
                continue
            if kind == "turn_context":
                cwd = payload.get("cwd", cwd)
                context = (label(payload.get("model")), label(payload.get("effort")),
                           label(payload.get("service_tier")))
                turn = payload.get("turn_id") or turn
                if turn:
                    contexts[turn] = (cwd, context)
                continue
            if kind == "event_msg" and event in {"turn_started", "task_started"}:
                turn = payload.get("turn_id")
                repeated.clear()
                polled.clear()
            selected = self.within(stamp, cwd)
            if selected:
                self.selected_threads.add(thread)
            location = {"file": path.name, "line": number}
            if kind == "token_usage_record":
                record_turn = payload.get("turn_id") or turn
                values = usage(payload.get("usage"))
                response_id = payload.get("response_id")
                if values is not None and isinstance(response_id, str) and response_id:
                    records.append((stamp, record_turn, cwd, context, response_id, values))
                    specific_turns.add(record_turn)
                elif selected:
                    self.quality["invalid_token_usage_records"] += 1
            elif kind == "event_msg" and event == "token_count":
                info = mapping(payload.get("info"))
                total = usage(info.get("total_token_usage"))
                if total is None:
                    if selected and info:
                        self.quality["invalid_token_count_totals"] += 1
                    continue
                has_baseline = bool(previous)
                delta = {key: value - previous.get(key, 0) for key, value in total.items()}
                if previous and CACHE_WRITE not in previous:
                    delta.pop(CACHE_WRITE, None)
                previous = total
                if min(delta.values()) < 0 or usage(delta) is None:
                    if selected:
                        self.quality["legacy_counter_discontinuities"] += 1
                    continue
                if not any(delta.values()):
                    continue
                if selected:
                    last = usage(info.get("last_token_usage"))
                    single = bool(last) and all(last[key] == delta[key] for key in CORE)
                    if not has_baseline and not single:
                        self.quality["legacy_initial_totals_without_baseline"] += 1
                        continue
                    legacy.append((stamp, turn, context, delta, single))
                    fallback_window = True
            elif selected and kind == "compacted":
                self.compactions += 1
            elif selected and kind == "response_item" and event in CALLS:
                call_id = payload.get("call_id") or payload.get("id")
                name = label(payload.get("name") or event)
                call_key = (name, call_id) if isinstance(call_id, str) and call_id else None
                if call_key and call_key in self.seen_calls:
                    continue
                if call_key:
                    self.seen_calls.add(call_key)
                self.tools[name] += 1
                args = parse_args(payload.get("arguments", payload.get("input")))
                if event == "local_shell_call":
                    args = mapping(payload.get("action"))
                fingerprint, observing, call_kind = None, False, name
                if name in CODE_CELLS:
                    source = payload.get("input")
                    if not isinstance(source, str):
                        source = args.get("code") if isinstance(args.get("code"), str) else ""
                    inner = cell_calls(source)
                    results = []
                    for inner_name, inner_args in inner:
                        self.inner_tools[inner_name] += 1
                        results.append(self.classify(inner_name, inner_args, cwd, location, polled))
                    observing = bool(results) and all(seen for _, seen in results)
                    if len(results) == 1:
                        fingerprint = results[0][0]
                elif name in WAITS:
                    observing = True
                    self.flag("cell_wait" if name == "wait" else "sleep_call", location)
                    cell = args.get("cell_id")
                    if name == "wait" and type(cell) in {int, str}:
                        key = ("cell", str(cell))
                        if key in polled:
                            self.flag("repeated_wait_same_cell_within_turn", location)
                        polled.add(key)
                else:
                    fingerprint, observing = self.classify(name, args, cwd, location, polled)
                if observing:
                    self.observing_calls += 1
                if isinstance(call_id, str) and call_id:
                    pending[call_id] = (fingerprint, location, call_kind)
            elif kind == "response_item" and event in OUTPUTS:
                call_id = payload.get("call_id")
                call = pending.pop(call_id, None) if isinstance(call_id, str) else None
                if call:
                    self.count_output(payload, call, repeated)
            if kind == "event_msg" and isinstance(event, str) and COMPLETION.match(event):
                repeated.clear()
                polled.clear()
                # Late counters may still refer to this turn until the next turn starts.
        for stamp, record_turn, record_cwd, recorded_context, response_id, values in records:
            record_cwd, recorded_context = contexts.get(record_turn, (record_cwd, recorded_context))
            if self.within(stamp, record_cwd) and response_id not in self.seen_records:
                self.seen_records.add(response_id)
                self.account(values, "response_records", True, recorded_context, thread)
        if fork and fallback_window:
            self.quality["fork_rollouts_with_legacy_counters"] += 1
        for stamp, record_turn, recorded_context, delta, single in legacy:
            if record_turn in specific_turns or (record_turn is None and specific_turns):
                self.quality["legacy_buckets_superseded"] += 1
                continue
            if fork and not self.args.include_legacy_forks:
                self.quality["unstable_legacy_fork_buckets_excluded"] += 1
                continue
            key = (thread, stamp, tuple(delta.items()))
            if key not in self.seen_legacy:
                self.seen_legacy.add(key)
                self.account(delta, "legacy_matching_delta" if single else "legacy_aggregate_delta",
                             single, recorded_context, thread)

    def classify(self, name, args, cwd, location, polled):
        """Return (fingerprint, observing) for one shell-like call, flagging waits and polls."""
        if name in EXEC:
            command = command_text(args)
            fingerprint = (normalize(str(args.get("workdir") or cwd)), command) if command else None
            if status_only(command):
                self.flag("observation_or_wait_command", location)
                return fingerprint, True
            return fingerprint, False
        if name == "write_stdin" and "session_id" in args and args.get("chars", "") == "":
            session = args.get("session_id")
            self.flag("empty_write_stdin", location)
            fingerprint = None
            if type(session) in {int, str}:
                fingerprint = ("process", str(session))
                if fingerprint in polled:
                    self.flag("repeated_empty_write_stdin_within_turn", location)
                polled.add(fingerprint)
            return fingerprint, True
        return None, False

    def count_output(self, payload, call, repeated):
        fingerprint, location, kind = call
        data = body_bytes(payload.get("output", payload.get("tools")))
        if data is None:
            return
        if kind == "wait" and STILL_RUNNING in data[:200]:
            self.flag("wait_returned_still_running", location)
        self.outputs += 1
        self.output_bytes += len(data)
        self.max_output = max(self.max_output, len(data))
        if len(data) >= self.args.large_output_bytes:
            self.large_outputs += 1
            self.flag("large_persisted_tool_output", location)
        if fingerprint is None:
            return
        digest = hashlib.sha256(data).digest()
        if repeated.get(fingerprint) == digest:
            self.flag("same_command_and_output_within_turn", location)
        repeated[fingerprint] = digest

    def summary(self):
        shell_names = EXEC | {"write_stdin"}
        shell = sum(self.tools[key] + self.inner_tools[key] for key in shell_names)
        observations = self.flags["observation_or_wait_command"] + self.flags["empty_write_stdin"]
        top_level = sum(self.tools.values())
        return {
            "rollout_files": dict(self.quality),
            "threads_in_window": len(self.selected_threads),
            "compacted_markers": self.compactions, "usage": self.total.summary(),
            "usage_by_context": [
                {"model": model, "effort": effort, "service_tier_raw": tier, **totals.summary()}
                for (model, effort, tier), totals in sorted(self.groups.items())
            ],
            "top_threads_by_input": [
                {"thread_id": thread, **totals.summary()}
                for thread, totals in sorted(
                    self.thread_totals.items(), key=lambda item: -item[1].tokens["input_tokens"]
                )[:10]
            ],
            "tool_calls_by_name": dict(self.tools.most_common()),
            "shell_calls_including_write_stdin": shell,
            "observation_candidate_share_of_shell_calls": (
                round(observations / shell, 4) if shell else None
            ),
            # Calls made inside code-mode `exec` cells (tools.exec_command, tools.write_stdin, ...).
            "inner_tool_calls_by_name": dict(self.inner_tools.most_common()),
            # Each top-level call is one model response that re-reads the context. A call only
            # observes when it is wait/sleep, a status-only command, an empty write_stdin, or an
            # exec cell whose every inner call is one of those.
            "top_level_tool_calls": top_level,
            "top_level_observation_calls": self.observing_calls,
            "observation_share_of_top_level_calls": (
                round(self.observing_calls / top_level, 4) if top_level else None
            ),
            "persisted_tool_outputs": {
                "records": self.outputs, "bytes": self.output_bytes, "largest_bytes": self.max_output,
                "large_threshold_bytes": self.args.large_output_bytes,
            },
            "candidate_flags": dict(self.flags),
            "candidate_examples": dict(self.examples),
        }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--project-root", action="append", required=True,
                        help="checkout or worktree root, repeat for additional worktrees")
    parser.add_argument("--codex-home", type=Path,
                        default=Path(os.environ.get("CODEX_HOME") or Path.home() / ".codex"))
    parser.add_argument("--rollout", action="append", type=Path,
                        help="explicit rollout file, repeatable; overrides automatic discovery")
    parser.add_argument("--since", type=cli_time, help="inclusive ISO 8601 timestamp")
    parser.add_argument("--until", type=cli_time, help="exclusive ISO 8601 timestamp")
    parser.add_argument("--large-output-bytes", type=int, default=16384)
    parser.add_argument("--examples", type=int, default=5, help="max example locations per flag")
    parser.add_argument("--include-legacy-forks", action="store_true",
                        help="include ambiguous legacy fork totals, for isolated diagnostics")
    args = parser.parse_args()
    if args.large_output_bytes < 1 or args.examples < 0:
        parser.error("invalid output threshold or example count")
    if args.since and args.until and args.since >= args.until:
        parser.error("--since must be before --until")
    paths = args.rollout
    if not paths:
        base = args.codex_home.expanduser()
        paths = [
            path for folder in ("sessions", "archived_sessions")
            for path in (base / folder).rglob("rollout-*.jsonl*")
            if path.name.endswith((".jsonl", ".jsonl.zst"))
            and not (path.suffix == ".zst" and path.with_suffix("").exists())
        ]
    audit = Audit(args)
    for path in sorted({path.expanduser().resolve() for path in paths},
                       key=lambda item: (item.name, str(item))):
        try:
            with open_rollout(path) as stream:
                audit.scan(path, stream)
            audit.quality["files_read"] += 1
        except (OSError, EOFError) as error:
            print(f"{path.name}: {error}", file=sys.stderr)
            audit.quality["files_unreadable_or_incomplete"] += 1
    report = audit.summary()
    report["window_utc"] = {
        "since_inclusive": args.since.isoformat() if args.since else None,
        "until_exclusive": args.until.isoformat() if args.until else None,
    }
    print(json.dumps(report, ensure_ascii=False, indent=2))
    if audit.quality["files_unreadable_or_incomplete"]:
        return 2
    if not audit.selected_threads:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
