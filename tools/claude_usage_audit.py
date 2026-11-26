#!/usr/bin/env python3
"""Offline audit of local Claude Code transcripts: context per call, cache misses, polling shell calls, large tool results, compactions and idle cross-session turns per session.

Streams the JSONL transcripts under ~/.claude/projects/<glob>/ (main sessions, child sessions in
worktrees and subagent files under <session>/subagents/) line by line and prints one JSON summary.
Python 3.10+, standard library only; nothing goes over the network:

    py -I tools/claude_usage_audit.py --since 2026-11-19 --out local/claude-audit/summary.json

With --out the JSON goes to that file and stdout gets only the verdict line; without it the JSON
goes to stdout and the verdict to stderr. docs/CLAUDE_TOKEN_AUDIT.md defines every metric.

Definitions in short: one API call is one unique assistant message id (a transcript stores one
record per content block, all with the same usage). Context of a call = input + cache_read +
cache_creation tokens. A turn starts at a user record that is not a tool result; its origin is
human, task-notification, side-sessions-event, peer (cross-session message) or other. A call
"observes" when every tool it calls only waits or looks (sleep/wait loops, log tails,
build_lock -Status, Monitor, TaskOutput, session status listings). An idle turn is a non-human
turn of at most --idle-max-calls calls that calls no tool except plan/wrap-up bookkeeping.

The owner's token and usage figures are private: keep the output under local/ and never put its
token numbers into the repository, the log or commit messages (AGENTS.md, F937). Process counts
and shares may be quoted; --public drops every token figure from the output.
"""
from __future__ import annotations

import argparse
import base64
from collections import Counter, defaultdict
from datetime import datetime, timezone
import hashlib
import heapq
import json
import math
import os
from pathlib import Path
import re
import statistics
import sys

SKIP_PREFIXES = (
    b'{"type":"api-request', b'{"type":"file-history', b'{"type":"cost-state',
    b'{"type":"queue-operation', b'{"type":"last-prompt', b'{"type":"atis-latch',
    b'{"type":"mode"', b'{"type":"tools-used',
)
SHELLS = {"Bash", "PowerShell"}
OBSERVE_TOOLS = {
    "Monitor", "TaskOutput", "BashOutput", "ScheduleWakeup",
    "mcp__ccd_session_mgmt__list_sessions", "mcp__ccd_session_mgmt__get_session",
    "mcp__ccd_session_mgmt__list_events", "mcp__terminal__read_terminal",
}
BOOKKEEPING_TOOLS = {
    "mcp__ccd_turn__update_plan", "mcp__ccd_turn__wrap_up", "mcp__ccd_session__mark_chapter",
    "TodoWrite", "TaskCreate", "TaskUpdate",
}
CROSS_SESSION_TOOLS = {
    "SendMessage", "mcp__ccd_session_mgmt__send_message", "mcp__ccd_session__start_session",
    "mcp__ccd_session__hand_off_to_session",
}
ORIGIN_MARKERS = (
    ("<side-sessions-event", "side-sessions-event"),
    ("<task-notification", "task-notification"),
    ("<cross-session-message", "peer"),
    ("<agent-message", "peer"),
    ("<ci-monitor-event", "ci-monitor-event"),
    ("<command-name>", "slash-command"),
)
WAITING_TEXT = re.compile(
    r"still (?:running|waiting|working|in progress|building|going)|no change|nothing new|"
    r"waiting for|keep waiting|will wait|unchanged|not finished yet|no update",
    re.IGNORECASE,
)
REMINDER = re.compile(r"<system-reminder>.*?</system-reminder>", re.DOTALL)

# ---- shell command classification -------------------------------------------------------

TRIVIAL = re.compile(
    r"^(?:cd|set-location|pushd|popd|echo|write-host|write-output|printf|date|get-date|true|:|"
    r"set|export|\$env:\w+\s*=|clear|cls)\b", re.IGNORECASE,
)
WAITERS = re.compile(r"^(?:sleep|start-sleep|timeout|wait-process|wait-job|wait|read-host)\b", re.IGNORECASE)
LOOP_WAIT = re.compile(r"\b(?:while|until|for|do)\b[\s\S]*\b(?:sleep|start-sleep)\b", re.IGNORECASE)
LOOKERS = re.compile(
    r"^(?:tail|get-process|tasklist|pgrep|ps|get-job|receive-job|nvidia-smi|get-counter|"
    r"get-ciminstance|get-wmiobject)\b", re.IGNORECASE,
)
LOCK_STATUS = re.compile(r"build_lock(?:\.ps1)?\b.*-status\b", re.IGNORECASE)
GIT_CHECK = re.compile(r"^git\s+(?:-C\s+\S+\s+)?(?:status|log|diff\s+--stat|branch|worktree\s+list|"
                       r"rev-parse|show\s+--stat|fetch)\b", re.IGNORECASE)
BUILD = re.compile(
    r"^(?:cargo|rustc|rustup|gradle|gradlew|\.?[\\/]?gradlew|pytest|ninja|cmake|msbuild|npm|"
    r"node)\b|prepush\.ps1|build_lock(?:\.ps1)?\b|build_\w+\.ps1|release_playable|"
    r"\bcargo\s|\btest(?:s)?\.py\b|grid-soak", re.IGNORECASE,
)
GIT = re.compile(r"^(?:git|gh)\b", re.IGNORECASE)
EDIT = re.compile(
    r"^(?:sed\s+-i|perl\s+-[a-z]*i|tee|set-content|add-content|out-file|mv|cp|rm|mkdir|"
    r"move-item|copy-item|remove-item|new-item|rename-item|touch|ln|chmod|patch)\b|"
    r"^cat\s*>|<<\s*['\"]?\w+['\"]?[\s\S]*>|>\s*[\w./\\$~-]+\.\w+\s*<<", re.IGNORECASE,
)
READ = re.compile(
    r"^(?:cat|head|sed|awk|grep|rg|find|ls|dir|get-childitem|gci|get-content|gc|type|"
    r"select-string|wc|jq|du|df|stat|which|where|get-item|test-path|resolve-path|"
    r"measure-object|get-filehash|sha256sum|md5sum|file|tree|realpath|readlink|"
    r"\(get-item|\(get-content|\[io\.file\]|import-csv|convertfrom-json)\b", re.IGNORECASE,
)
RUN = re.compile(
    r"^(?:py|python3?|powershell(?:\.exe)?|pwsh|&|\.[\\/]|adb|grid-app|target[\\/]|"
    r"start-process|bash|sh|cmd|invoke-|curl|scrcpy|ffmpeg|magick)", re.IGNORECASE,
)
TAIL_PIPE = re.compile(r"\|\s*(?:tail\b|select-object\s+-last\b|select\s+-last\b)", re.IGNORECASE)
PS_WRAPPED = re.compile(
    r"^(?:powershell|pwsh)(?:\.exe)?\s+(?:-\w+\s+(?:bypass\s+)?)*-command\s+(['\"])([\s\S]*)\1\s*$",
    re.IGNORECASE,
)


def statements(command):
    """Statements of a shell command (split on newlines, ;, && and ||), each with its pipeline."""
    out = []
    for part in re.split(r"\r?\n|;|&&|\|\|", command):
        part = part.strip()
        if part and not part.startswith("#"):
            out.append(re.sub(r"^\$[\w:]+\s*=\s*", "", part))
    return out


def classify_statement(text):
    first = text.split("|", 1)[0].strip()
    if TRIVIAL.match(first) or first in {"do", "done", "then", "fi", "else", "{", "}", ")"}:
        return "trivial"
    if LOCK_STATUS.search(first):
        return "observe"
    if WAITERS.match(first) or re.match(r"^(?:while|until)\b", first, re.IGNORECASE):
        return "wait"
    if LOOKERS.match(first) or re.search(r"get-content\b.*-(?:tail|wait)\b", first, re.IGNORECASE):
        return "observe"
    if TAIL_PIPE.search(text) and re.match(r"^(?:cat|get-content|gc|type)\b", first, re.IGNORECASE):
        return "observe"
    if GIT_CHECK.match(first):
        return "git_check"
    if BUILD.search(first):
        return "build_test"
    if GIT.match(first):
        return "git"
    if EDIT.search(text):
        return "edit"
    if READ.match(first):
        return "read"
    if RUN.match(first):
        return "run"
    return "other"


def classify_command(command):
    """One category per shell call: observe, build_test, run, git, git_check, edit, read, other."""
    command = command.strip()
    wrapped = PS_WRAPPED.match(command)
    if wrapped:
        command = wrapped.group(2)
    if LOOP_WAIT.search(command):
        return "observe"
    kinds = [kind for kind in map(classify_statement, statements(command)) if kind != "trivial"]
    if not kinds:
        return "other"
    looking = {"wait", "observe"}
    if "wait" in kinds and all(kind in looking | {"read", "git_check"} for kind in kinds):
        return "observe"
    if all(kind in looking for kind in kinds):
        return "observe"
    for kind in ("build_test", "run", "git", "edit", "git_check", "read"):
        if kind in kinds:
            return kind
    return "other"


# ---- helpers ------------------------------------------------------------------------------

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


def dist(values):
    if not values:
        return {"n": 0, "mean": None, "p50": None, "p95": None, "max": None}
    ordered = sorted(values)
    return {
        "n": len(ordered), "mean": round(statistics.fmean(ordered)),
        "p50": round(statistics.median(ordered)),
        "p95": ordered[math.ceil(0.95 * len(ordered)) - 1], "max": ordered[-1],
    }


def share(part, whole):
    return round(part / whole, 4) if whole else None


def head(text, limit=100):
    return re.sub(r"\s+", " ", text or "").strip()[:limit]


def image_dims(data):
    """(width, height) from base64 PNG/JPEG/GIF/WebP data, or None."""
    try:
        raw = base64.b64decode(data[:64] + "=" * (-len(data[:64]) % 4))
        if raw[:8] == b"\x89PNG\r\n\x1a\n":
            return int.from_bytes(raw[16:20], "big"), int.from_bytes(raw[20:24], "big")
        if raw[:6] in (b"GIF87a", b"GIF89a"):
            return int.from_bytes(raw[6:8], "little"), int.from_bytes(raw[8:10], "little")
        if raw[:2] == b"\xff\xd8":
            raw = base64.b64decode(data[:262144] + "=" * (-len(data[:262144]) % 4))
            index = 2
            while index + 9 < len(raw):
                if raw[index] != 0xFF:
                    index += 1
                    continue
                marker = raw[index + 1]
                if marker in (0xC0, 0xC1, 0xC2):
                    return (int.from_bytes(raw[index + 7:index + 9], "big"),
                            int.from_bytes(raw[index + 5:index + 7], "big"))
                index += 2 + int.from_bytes(raw[index + 2:index + 4], "big")
    except (ValueError, IndexError):
        pass
    return None


def image_tokens(dims):
    """Approximate API image tokens: long edge scaled to 1568 px, then width*height/750."""
    if not dims or not all(dims):
        return 1600
    width, height = dims
    scale = min(1.0, 1568 / max(width, height))
    return min(1600, math.ceil(width * scale * height * scale / 750))


def text_size(content):
    """(text bytes, [image base64 strings]) of a tool_result or message content."""
    if isinstance(content, str):
        return len(content.encode("utf-8", "replace")), []
    size, images = 0, []
    for block in content if isinstance(content, list) else []:
        if not isinstance(block, dict):
            continue
        if block.get("type") == "image":
            data = (block.get("source") or {}).get("data")
            if isinstance(data, str):
                images.append(data)
        elif block.get("type") == "text":
            size += len(str(block.get("text", "")).encode("utf-8", "replace"))
        else:
            size += len(json.dumps(block, ensure_ascii=False).encode("utf-8", "replace"))
    return size, images


def block_text(content):
    if isinstance(content, str):
        return content
    return "".join(
        block.get("text", "") for block in content if isinstance(block, dict)
        and block.get("type") == "text"
    ) if isinstance(content, list) else ""


# ---- aggregation --------------------------------------------------------------------------

TOKEN_KEYS = ("input_tokens", "cache_read_input_tokens", "cache_creation_input_tokens", "output_tokens")


class Usage:
    def __init__(self):
        self.tokens = Counter()
        self.contexts = []

    def add(self, values, context):
        self.tokens.update(values)
        self.contexts.append(context)

    def summary(self, public):
        out = {"calls": len(self.contexts)}
        if public:
            total = sum(self.contexts)
            out["cache_read_share_of_context"] = share(self.tokens["cache_read_input_tokens"], total)
            out["cache_creation_share_of_context"] = share(self.tokens["cache_creation_input_tokens"], total)
            return out
        out["tokens"] = dict(self.tokens)
        out["context_tokens_sum"] = sum(self.contexts)
        out["context_per_call"] = dist(self.contexts)
        return out


class Session:
    def __init__(self, key):
        self.key = key
        self.title = None
        self.agent_name = None
        self.project = None
        self.sidechain = False
        self.files = set()
        self.usage = Usage()
        self.tools = Counter()
        self.shell = Counter()
        self.observe_calls = 0
        self.bookkeeping_calls = 0
        self.compactions = []
        self.turns = Counter()
        self.idle_turns = 0
        self.idle_calls = 0
        self.idle_context = 0
        self.result_bytes = 0
        self.images = 0
        self.reads = Counter()
        self.first = None
        self.last = None
        self.models = Counter()


class Turn:
    __slots__ = ("session", "origin", "start", "calls", "context", "max_context", "tools",
                 "observe", "last_text", "largest", "prompt_bytes", "reminder_bytes")

    def __init__(self, session, origin, start, prompt_bytes, reminder_bytes):
        self.session, self.origin, self.start = session, origin, start
        self.calls = self.context = self.max_context = self.observe = 0
        self.tools = Counter()
        self.last_text = ""
        self.largest = (0, "", "")
        self.prompt_bytes, self.reminder_bytes = prompt_bytes, reminder_bytes


class Audit:
    def __init__(self, args):
        self.args = args
        self.quality = Counter()
        self.sessions = {}
        self.total = Usage()
        self.by_model = defaultdict(Usage)
        self.by_kind = defaultdict(Usage)
        self.seen_messages = set()
        self.seen_tools = set()
        self.seen_results = set()
        self.tools = Counter()
        self.shell = Counter()
        self.shell_background = 0
        self.shell_examples = defaultdict(list)
        self.repeat_same_output = Counter()
        self.call_kinds = Counter()
        self.large = []
        self.result_stats = Counter()
        self.result_by_tool = defaultdict(Counter)
        self.images = Counter()
        self.image_sources = Counter()
        self.image_bytes = []
        self.whole_reads = []
        self.cache_events = defaultdict(Counter)
        self.compactions = []
        self.turn_origins = defaultdict(Counter)
        self.idle = Counter()
        self.waiting_text = Counter()
        self.top_turns = []
        self.cross = Counter()
        self.cross_sizes = defaultdict(list)
        self.attachments = defaultdict(Counter)
        self.reminders = Counter()
        self.subagent_spawns = Counter()

    def within(self, stamp):
        return ((self.args.since is None or stamp >= self.args.since)
                and (self.args.until is None or stamp < self.args.until))

    def session(self, key):
        if key not in self.sessions:
            self.sessions[key] = Session(key)
        return self.sessions[key]

    # -- one transcript file ------------------------------------------------------------------

    def scan(self, path, stream):
        sidechain_file = path.parent.name == "subagents"
        project = (path.parents[2] if sidechain_file else path.parent).name
        base_session = path.parents[1].name if sidechain_file else path.stem
        key = f"{base_session}/{path.stem}" if sidechain_file else base_session
        session = self.session(key)
        session.project, session.sidechain = project, sidechain_file
        session.files.add(path.name)
        pending = {}          # tool_use id -> call info
        calls = {}            # message id -> [observing flags]
        call_turn = {}
        last_output = {}      # (command) -> digest
        turn = None
        prev_stamp = None
        prev_model = None
        compacted = False
        last_context = None
        read_first = set()
        for number, line in enumerate(stream, 1):
            if line.startswith(SKIP_PREFIXES):
                continue
            try:
                row = json.loads(line)
            except ValueError:
                self.quality["invalid_lines"] += 1
                continue
            if not isinstance(row, dict):
                continue
            kind = row.get("type")
            if kind == "custom-title":
                session.title = str(row.get("customTitle"))[:80]
                continue
            if kind == "agent-name":
                session.agent_name = str(row.get("agentName"))[:80]
                continue
            try:
                stamp = as_time(row["timestamp"])
            except (KeyError, ValueError, TypeError, AttributeError):
                continue
            selected = self.within(stamp)
            if kind == "attachment":
                if selected:
                    self.count_attachment(row)
                continue
            if kind == "system":
                if row.get("subtype") == "compact_boundary":
                    compacted = True
                    if selected:
                        meta = row.get("compactMetadata") or {}
                        pre = meta.get("preTokens")
                        item = {"session": key, "trigger": meta.get("trigger"),
                                "pre_tokens": pre if isinstance(pre, int) else last_context,
                                "last_call_context": last_context, "time": stamp.isoformat()}
                        session.compactions.append(item)
                        self.compactions.append(item)
                continue
            message = row.get("message")
            if not isinstance(message, dict):
                continue
            if kind == "assistant":
                if row.get("isSidechain"):
                    session.sidechain = True
                mid = message.get("id") or row.get("requestId")
                model = message.get("model") or "unknown"
                content = message.get("content") if isinstance(message.get("content"), list) else []
                new_call = mid not in self.seen_messages and model != "<synthetic>"
                if new_call and mid:
                    self.seen_messages.add(mid)
                    if selected:
                        usage = message.get("usage") or {}
                        values = {name: usage.get(name) for name in TOKEN_KEYS}
                        if all(isinstance(value, int) and value >= 0 for value in values.values()):
                            context = (values["input_tokens"] + values["cache_read_input_tokens"]
                                       + values["cache_creation_input_tokens"])
                            effort = row.get("effort") if isinstance(row.get("effort"), str) else "default"
                            label = f"{model}/{effort}"
                            self.total.add(values, context)
                            self.by_model[label].add(values, context)
                            self.by_kind["subagent" if session.sidechain else "session"].add(values, context)
                            session.usage.add(values, context)
                            session.models[label] += 1
                            session.first = session.first or stamp
                            session.last = stamp
                            self.cache_event(usage, values, stamp, prev_stamp, compacted,
                                             prev_model not in (None, label), key)
                            compacted = False
                            prev_stamp, prev_model, last_context = stamp, label, context
                            calls[mid] = []
                            if turn is None:
                                turn = Turn(key, "continuation", stamp, 0, 0)
                            call_turn[mid] = turn
                            turn.calls += 1
                            turn.context += context
                            turn.max_context = max(turn.max_context, context)
                        else:
                            self.quality["assistant_without_usage"] += 1
                for block in content:
                    if not isinstance(block, dict):
                        continue
                    if block.get("type") == "text" and turn is not None:
                        turn.last_text = str(block.get("text", ""))[-400:]
                    if block.get("type") != "tool_use" or not selected:
                        continue
                    tid = block.get("id")
                    if tid in self.seen_tools:
                        continue
                    self.seen_tools.add(tid)
                    info = self.count_tool(session, block, turn)
                    pending[tid] = info
                    if mid in calls:
                        calls[mid].append(info["observe"] if not info["bookkeeping"] else None)
                continue
            if kind != "user" or not selected:
                continue
            content = message.get("content")
            blocks = content if isinstance(content, list) else []
            results = [b for b in blocks if isinstance(b, dict) and b.get("type") == "tool_result"]
            if results:
                for block in results:
                    tid = block.get("tool_use_id")
                    if tid in self.seen_results:
                        continue
                    self.seen_results.add(tid)
                    info = pending.pop(tid, None)
                    self.count_result(session, block, info, last_output, read_first, turn, stamp)
                continue
            if row.get("isCompactSummary"):
                size, _ = text_size(content)
                self.result_stats["compact_summary_records"] += 1
                self.result_stats["compact_summary_bytes"] += size
                continue
            if row.get("isMeta"):
                continue
            text = content if isinstance(content, str) else block_text(content)
            _, images = text_size(content)
            for data in images:
                self.count_image(session, data, "user_message")
            origin = (row.get("origin") or {}).get("kind") if isinstance(row.get("origin"), dict) else None
            stripped = REMINDER.sub("", text).lstrip()
            if origin in (None, "human"):
                for marker, name in ORIGIN_MARKERS:
                    if marker in stripped[:400]:
                        origin = name
                        break
            origin = origin or "human"
            reminder_bytes = sum(len(m.encode("utf-8", "replace")) for m in REMINDER.findall(text))
            self.reminders["records"] += reminder_bytes > 0
            self.reminders["bytes"] += reminder_bytes
            if turn is not None:
                self.close_turn(turn)
            prompt_bytes = len(stripped.encode("utf-8", "replace"))
            turn = Turn(key, origin, stamp, prompt_bytes, reminder_bytes)
            session.turns[origin] += 1
            if origin != "human":
                self.cross[f"incoming_{origin}"] += 1
                self.cross_sizes[f"incoming_{origin}"].append(prompt_bytes)
        if turn is not None:
            self.close_turn(turn)
        for mid, flags in calls.items():
            real = [flag for flag in flags if flag is not None]
            if not flags:
                self.call_kinds["text_only"] += 1
            elif not real:
                self.call_kinds["bookkeeping_only"] += 1
                session.bookkeeping_calls += 1
            elif all(real):
                self.call_kinds["observe_only"] += 1
                session.observe_calls += 1
            else:
                self.call_kinds["work"] += 1

    # -- pieces -----------------------------------------------------------------------------

    def cache_event(self, usage, values, stamp, prev_stamp, compacted, model_changed, key):
        created = values["cache_creation_input_tokens"]
        if created < self.args.cache_miss_tokens:
            return
        split = usage.get("cache_creation") if isinstance(usage.get("cache_creation"), dict) else {}
        ttl = 300 if split.get("ephemeral_5m_input_tokens", 0) > split.get("ephemeral_1h_input_tokens", 0) else 3600
        gap = (stamp - prev_stamp).total_seconds() if prev_stamp else None
        if gap is None:
            reason = "first_call_in_transcript"
        elif compacted:
            reason = "after_compaction"
        elif gap > ttl:
            reason = f"idle_gap_over_{'1h' if ttl == 3600 else '5m'}_ttl"
        elif model_changed:
            reason = "model_or_effort_change"
        elif gap > 300:
            reason = "gap_5m_to_ttl"
        else:
            reason = "within_ttl_other"
        self.cache_events[reason]["events"] += 1
        self.cache_events[reason]["cache_creation_tokens"] += created

    def count_attachment(self, row):
        attachment = row.get("attachment") if isinstance(row.get("attachment"), dict) else {}
        name = str(attachment.get("type", "unknown"))
        rendered = row.get("rendered")
        if isinstance(rendered, (str, list, dict)):
            data = rendered if isinstance(rendered, str) else json.dumps(rendered, ensure_ascii=False)
            self.attachments[name]["rendered_records"] += 1
            self.attachments[name]["rendered_bytes"] += len(data.encode("utf-8", "replace"))
        else:
            self.attachments[name]["unrendered_records"] += 1

    def count_tool(self, session, block, turn):
        name = str(block.get("name", "unknown"))
        args = block.get("input") if isinstance(block.get("input"), dict) else {}
        self.tools[name] += 1
        session.tools[name] += 1
        info = {"name": name, "head": "", "category": None, "observe": name in OBSERVE_TOOLS,
                "bookkeeping": name in BOOKKEEPING_TOOLS, "path": None, "whole": False, "command": None}
        if name in SHELLS:
            command = str(args.get("command", ""))
            category = classify_command(command)
            if args.get("run_in_background"):
                self.shell_background += 1
                if category == "observe":
                    category = "other"
            info.update(category=category, observe=category == "observe", head=head(command),
                        command=command)
            self.shell[category] += 1
            session.shell[category] += 1
            if len(self.shell_examples[category]) < self.args.examples:
                self.shell_examples[category].append(head(command, 120))
        elif name == "Read":
            info["path"] = str(args.get("file_path", ""))
            info["whole"] = args.get("offset") is None and args.get("limit") is None
            info["head"] = head(info["path"])
        elif name in ("Grep", "Glob"):
            info["head"] = head(str(args.get("pattern", "")))
        elif name in ("Agent", "Task"):
            self.subagent_spawns[str(args.get("subagent_type", "default"))] += 1
            self.cross_sizes["subagent_prompt"].append(len(str(args.get("prompt", "")).encode("utf-8")))
            info["head"] = head(str(args.get("description", "")))
        if name in CROSS_SESSION_TOOLS:
            body = args.get("message", args.get("prompt", args.get("remaining_plan", "")))
            body = body if isinstance(body, str) else json.dumps(body, ensure_ascii=False)
            self.cross[f"outgoing_{name}"] += 1
            self.cross_sizes[f"outgoing_{name}"].append(len(body.encode("utf-8", "replace")))
        if turn is not None:
            turn.tools[name] += 1
            turn.observe += info["observe"]
        return info

    def count_image(self, session, data, source):
        dims = image_dims(data)
        self.images["count"] += 1
        self.images["base64_bytes"] += len(data)
        self.images["estimated_tokens"] += image_tokens(dims)
        if dims and max(dims) > 1568:
            self.images["over_1568px_long_edge"] += 1
        self.image_sources[source] += 1
        self.image_bytes.append(len(data) * 3 // 4)
        session.images += 1

    def count_result(self, session, block, info, last_output, read_first, turn, stamp):
        size, images = text_size(block.get("content"))
        name = info["name"] if info else "unmatched"
        for data in images:
            self.count_image(session, data, name)
        self.result_stats["results"] += 1
        self.result_stats["text_bytes"] += size
        self.result_by_tool[name]["results"] += 1
        self.result_by_tool[name]["text_bytes"] += size
        session.result_bytes += size
        if turn is not None and size > turn.largest[0]:
            turn.largest = (size, name, info["head"][:60] if info else "")
        if size >= self.args.large_result_bytes:
            self.result_stats["large_results"] += 1
            self.result_by_tool[name]["large_results"] += 1
            item = (size, name, info["head"] if info else "", session.key, stamp.isoformat())
            if len(self.large) < self.args.top:
                heapq.heappush(self.large, item)
            else:
                heapq.heappushpop(self.large, item)
        if not info:
            return
        if info["command"] is not None:
            digest = hashlib.sha256(json.dumps(block.get("content"), sort_keys=True).encode()).digest()
            command = info["command"]
            if last_output.get(command) == digest:
                self.repeat_same_output[info["category"]] += 1
            last_output[command] = digest
        if info["path"]:
            path = info["path"].replace("\\", "/").lower()
            session.reads[path] += 1
            if path in read_first:
                self.result_stats["repeated_read_calls"] += 1
                self.result_stats["repeated_read_bytes"] += size
            read_first.add(path)
            if info["whole"]:
                self.result_stats["whole_file_reads"] += 1
                if size >= self.args.large_result_bytes:
                    self.result_stats["whole_file_reads_large"] += 1
                    self.result_stats["whole_file_reads_large_bytes"] += size
                    self.whole_reads.append((size, info["path"][-100:], session.key))

    def close_turn(self, turn):
        if turn.calls == 0:
            return
        origin = turn.origin
        stats = self.turn_origins[origin]
        stats["turns"] += 1
        stats["calls"] += turn.calls
        stats["context_tokens"] += turn.context
        stats["observe_calls"] += turn.observe
        stats["prompt_bytes"] += turn.prompt_bytes
        stats["reminder_bytes"] += turn.reminder_bytes
        real_tools = {name for name in turn.tools if name not in BOOKKEEPING_TOOLS}
        idle = (origin not in ("human", "continuation") and not real_tools
                and turn.calls <= self.args.idle_max_calls)
        session = self.sessions[turn.session]
        if idle:
            self.idle["turns"] += 1
            self.idle["calls"] += turn.calls
            self.idle["context_tokens"] += turn.context
            self.idle[f"by_origin_{origin}"] += 1
            session.idle_turns += 1
            session.idle_calls += turn.calls
            session.idle_context += turn.context
        if (origin not in ("human", "continuation") and turn.calls <= self.args.idle_max_calls
                and WAITING_TEXT.search(turn.last_text)):
            self.waiting_text["turns"] += 1
            self.waiting_text["calls"] += turn.calls
            self.waiting_text["context_tokens"] += turn.context
            self.waiting_text["idle_turns"] += idle
        tools = ", ".join(f"{name.split('__')[-1]}×{count}" for name, count in turn.tools.most_common(3))
        size, tool, what = turn.largest
        description = (
            f"{origin} turn: {turn.calls} calls, max context {turn.max_context // 1000}k, "
            f"tools [{tools or 'none'}], {turn.observe} observe tool calls"
            + (f", largest result {size // 1024} KiB from {tool} '{what}'" if size else "")
        )
        item = (turn.context, turn.start.isoformat(), turn.session, origin, turn.calls, description)
        if len(self.top_turns) < self.args.top:
            heapq.heappush(self.top_turns, item)
        else:
            heapq.heappushpop(self.top_turns, item)

    # -- output -------------------------------------------------------------------------------

    def session_summary(self, session, public):
        usage = session.usage.summary(public)
        shell_calls = sum(session.shell.values())
        tools = ", ".join(f"{name.split('__')[-1]}×{count}" for name, count in session.tools.most_common(3))
        repeated = sum(count - 1 for count in session.reads.values() if count > 1)
        out = {
            "session": session.key, "title": session.title or session.agent_name,
            "project_dir": session.project, "subagent": session.sidechain,
            "first": session.first.isoformat() if session.first else None,
            "last": session.last.isoformat() if session.last else None,
            "models": dict(session.models), **usage,
            "turns_by_origin": dict(session.turns),
            "observe_only_calls": session.observe_calls,
            "observe_only_share_of_calls": share(session.observe_calls, len(session.usage.contexts)),
            "bookkeeping_only_calls": session.bookkeeping_calls,
            "shell_calls": shell_calls,
            "shell_observe_share": share(session.shell["observe"], shell_calls),
            "idle_turns": session.idle_turns, "idle_turn_calls": session.idle_calls,
            "compactions": len(session.compactions), "images": session.images,
            "repeated_reads": repeated, "tool_result_bytes": session.result_bytes,
        }
        if not public:
            out["idle_turn_context_tokens"] = session.idle_context
            out["compaction_pre_tokens"] = [item["pre_tokens"] for item in session.compactions]
            mean = out["context_per_call"]["mean"]
            out["dominated_by"] = (
                f"{len(session.usage.contexts)} calls at mean context {(mean or 0) // 1000}k; "
                f"top tools {tools or 'none'}; {session.shell['observe']} observe shell calls; "
                f"{session.idle_turns} idle turns; {len(session.compactions)} compactions"
            )
        return out

    def summary(self):
        public = self.args.public
        shell_calls = sum(self.shell.values())
        calls = sum(self.call_kinds.values())
        sessions = [s for s in self.sessions.values() if s.usage.contexts]
        ranked = sorted(sessions, key=lambda s: -sum(s.usage.contexts))
        cache = {}
        for reason, counter in self.cache_events.items():
            item = {"events": counter["events"]}
            if not public:
                item["cache_creation_tokens"] = counter["cache_creation_tokens"]
            cache[reason] = item
        idle_turn_keys = {k: v for k, v in self.idle.items() if public is False or "tokens" not in k}
        report = {
            "files": dict(self.quality),
            "sessions_with_calls": sum(1 for s in sessions if not s.sidechain),
            "subagent_transcripts_with_calls": sum(1 for s in sessions if s.sidechain),
            "usage": self.total.summary(public),
            "usage_by_model_effort": {k: v.summary(public) for k, v in sorted(self.by_model.items())},
            "usage_by_kind": {k: v.summary(public) for k, v in sorted(self.by_kind.items())},
            "api_calls_by_kind": dict(self.call_kinds),
            "observe_only_share_of_calls": share(self.call_kinds["observe_only"], calls),
            "bookkeeping_only_share_of_calls": share(self.call_kinds["bookkeeping_only"], calls),
            "tool_calls_by_name": dict(self.tools.most_common()),
            "shell_calls": shell_calls,
            "shell_calls_by_category": dict(self.shell.most_common()),
            "shell_observe_share": share(self.shell["observe"], shell_calls),
            "shell_background_launches": self.shell_background,
            "shell_same_command_same_output_repeats": dict(self.repeat_same_output),
            "shell_examples_by_category": dict(self.shell_examples),
            "cache_creation_events": {
                "threshold_tokens": self.args.cache_miss_tokens, "by_reason": cache,
            },
            "compactions": {
                "count": len(self.compactions),
                "by_trigger": dict(Counter(item["trigger"] for item in self.compactions)),
                "sessions_with_compactions": len({item["session"] for item in self.compactions}),
            },
            "turns_by_origin": {
                origin: {k: v for k, v in stats.items() if not (public and "tokens" in k)}
                for origin, stats in sorted(self.turn_origins.items())
            },
            "idle_turns": idle_turn_keys,
            "waiting_text_turns": {k: v for k, v in self.waiting_text.items()
                                   if not (public and "tokens" in k)},
            "cross_session": {
                "counts": dict(self.cross),
                "body_bytes": {k: dist(v) for k, v in sorted(self.cross_sizes.items())},
            },
            "subagent_spawns_by_type": dict(self.subagent_spawns),
            "system_reminders_in_prompts": dict(self.reminders),
            "attachments_by_type": {k: dict(v) for k, v in sorted(
                self.attachments.items(), key=lambda kv: -kv[1]["rendered_bytes"])},
            "tool_results": dict(self.result_stats),
            "tool_results_by_tool": {k: dict(v) for k, v in sorted(
                self.result_by_tool.items(), key=lambda kv: -kv[1]["text_bytes"])},
            "large_results_top": [
                {"bytes": size, "estimated_tokens": size // 4, "tool": tool, "what": what,
                 "session": session, "time": stamp}
                for size, tool, what, session, stamp in sorted(self.large, reverse=True)
            ],
            "whole_file_reads_large_top": [
                {"bytes": size, "path_tail": path, "session": session}
                for size, path, session in sorted(self.whole_reads, reverse=True)[:self.args.top]
            ],
            "images": {**dict(self.images), "by_source": dict(self.image_sources),
                       "decoded_bytes": dist(self.image_bytes)},
            "top_sessions_by_context": [self.session_summary(s, public) for s in ranked[:self.args.top]],
            "sessions": [self.session_summary(s, public) for s in ranked],
        }
        if public:
            report["images"].pop("estimated_tokens", None)
            for item in report["large_results_top"]:
                item.pop("estimated_tokens", None)
        else:
            report["compactions"]["pre_tokens"] = dist(
                [item["pre_tokens"] for item in self.compactions if isinstance(item["pre_tokens"], int)])
            report["top_turns_by_context"] = [
                {"context_tokens": ctx, "start": start, "session": session, "origin": origin,
                 "calls": n, "description": description}
                for ctx, start, session, origin, n, description in sorted(self.top_turns, reverse=True)
            ]
        return report


def verdict(report, out):
    shell = report["shell_observe_share"]
    calls = report["observe_only_share_of_calls"]
    idle = report["idle_turns"].get("turns", 0)
    text = (
        f"claude-usage-audit: {'OK' if report['usage']['calls'] else 'NO CALLS'} "
        f"files={report['files'].get('files_read', 0)} sessions={report['sessions_with_calls']} "
        f"subagents={report['subagent_transcripts_with_calls']} calls={report['usage']['calls']} "
        f"observe_calls={calls} shell_observe={shell} idle_turns={idle} "
        f"compactions={report['compactions']['count']} images={report['images'].get('count', 0)}"
    )
    return text + (f" -> {out}" if out else "")


def first_stamp(path):
    """First record timestamp of a transcript (ISO text sorts in time order), or ''."""
    try:
        with path.open("rb") as stream:
            for _, line in zip(range(200), stream):
                found = re.search(rb'"timestamp":"([^"]+)"', line)
                if found:
                    return found.group(1).decode("ascii", "replace")
    except OSError:
        pass
    return ""


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--projects-dir", type=Path,
                        default=Path(os.environ.get("CLAUDE_CONFIG_DIR") or Path.home() / ".claude") / "projects")
    parser.add_argument("--project-glob", action="append",
                        help="project folder glob under --projects-dir, repeatable (default C--Dev-Reassembly*)")
    parser.add_argument("--transcript", action="append", type=Path,
                        help="explicit transcript file, repeatable; overrides discovery")
    parser.add_argument("--since", type=cli_time, help="inclusive ISO 8601 timestamp or YYYY-MM-DD (UTC)")
    parser.add_argument("--until", type=cli_time, help="exclusive ISO 8601 timestamp or YYYY-MM-DD (UTC)")
    parser.add_argument("--large-result-bytes", type=int, default=16384)
    parser.add_argument("--cache-miss-tokens", type=int, default=20000,
                        help="cache_creation tokens in one call that count as a cache rebuild")
    parser.add_argument("--idle-max-calls", type=int, default=3)
    parser.add_argument("--top", type=int, default=15)
    parser.add_argument("--examples", type=int, default=5, help="example commands per shell category")
    parser.add_argument("--public", action="store_true",
                        help="drop every token figure; only counts and shares remain")
    parser.add_argument("--out", type=Path, help="write the JSON here; stdout gets the verdict")
    args = parser.parse_args()
    if args.large_result_bytes < 1 or args.top < 1 or args.examples < 0:
        parser.error("invalid threshold, --top or --examples")
    if args.since and args.until and args.since >= args.until:
        parser.error("--since must be before --until")
    paths = args.transcript
    if not paths:
        base = args.projects_dir.expanduser()
        paths = [path for pattern in (args.project_glob or ["C--Dev-Reassembly*"])
                 for folder in base.glob(pattern) if folder.is_dir()
                 for path in folder.rglob("*.jsonl")]
    audit = Audit(args)
    # Oldest transcript first, so history a forked session copies counts once, under the
    # session that made the call (message ids and tool ids are deduplicated across files).
    for path in sorted({p.expanduser().resolve() for p in paths},
                       key=lambda p: (first_stamp(p), str(p))):
        try:
            with path.open("rb") as stream:
                audit.scan(path, stream)
            audit.quality["files_read"] += 1
        except OSError as error:
            print(f"{path.name}: {error}", file=sys.stderr)
            audit.quality["files_unreadable"] += 1
    report = audit.summary()
    report["window_utc"] = {
        "since_inclusive": args.since.isoformat() if args.since else None,
        "until_exclusive": args.until.isoformat() if args.until else None,
    }
    text = json.dumps(report, ensure_ascii=False, indent=2)
    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(text + "\n", encoding="utf-8")
        print(verdict(report, args.out))
    else:
        print(text)
        print(verdict(report, None), file=sys.stderr)
    if audit.quality["files_unreadable"]:
        return 2
    return 0 if report["usage"]["calls"] else 1


if __name__ == "__main__":
    sys.exit(main())
