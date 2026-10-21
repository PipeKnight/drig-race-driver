"""Mine original-executable addresses from docs, the project log and Rust comments into symbols.tsv.

Every mention of an address inside the unpacked image (.text, .rdata, .data) is collected with
file:line, the log entry (F/G id) it sits in, the nearest confidence label ([D]/[V]/[I]/[O]) and
candidate names. Rows are then built per address:

  name        Rust item documented by the address (newest file wins), else a backticked
              identifier next to the address in a doc/log line, else a short prose phrase
  port        Rust locations outside crates/grid-probes (file:line and item), newest first
  doc         up to three docs/*.md:line, the docs that mention the address most
  entry       up to three log entries mentioning it, newest first
  conflicts   other distinct Rust/backtick names seen for the address

Bare all-digit tokens (e.g. 451770) are accepted only when they are a function entry in
local/re-cache/functions.tsv (tools/re/master.py functions); hex with 0x or letters is always taken.

  py -I tools/re/mine_symbols.py              # rewrite derived columns, keep curated names
  py -I tools/re/mine_symbols.py --fresh      # rebuild from scratch (seed)
"""
from __future__ import annotations

import argparse
import bisect
import re
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import re_common as rc  # noqa: E402

TEXT = (0x401000, 0xC88400)
DATA = (0xC89000, 0xFB3074)
RDATA_END = 0xDA5A00

TOKEN = re.compile(r"(?<![\w#+.$/\\-])(0[xX])?([0-9A-Fa-f]{6,8})(?![\w])")
GHIDRA_NAME = re.compile(r"\b(?:FUN|DAT|LAB|PTR|SUB|sub|thunk_FUN|switchD|caseD|PTR_FUN|PTR_LAB)_([0-9A-Fa-f]{6,8})\b")
LABEL = re.compile(r"\[(D|V|I|O)(?:[\s,:/][^\]]{0,60})?\]")
ENTRY_LINE = re.compile(r"^(?:#{1,4}\s+)?([FG]\d{1,4})\b[\s:—-]")
ENTRY_TAIL = re.compile(r"^#{2,4}\s.*\(([FG]\d{1,4})\)\s*$")
UNIT_AFTER = re.compile(r"^\s?(?:bytes?|B\b|KiB|MiB|ms|µs|us|s\b|m\b|km|Hz|kHz|ticks?|frames?|samples?|px|rows?|%|,\d)")
WORD_BEFORE_REJECT = re.compile(r"(?:seed|tick|ticks|run|sha256|hash|#|frame|sample|size|length|bytes|crc)\s*[:=]?\s*$", re.I)
RUST_ITEM = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:(?:const\s+|async\s+|unsafe\s+)*(fn)|(struct|enum|const|static|type|trait|mod|impl))\s+(?:<[^>]*>\s*)?([A-Za-z_]\w*)")
RUST_FN = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:const\s+|async\s+|unsafe\s+)*fn\s+([A-Za-z_]\w*)")
IDENT = re.compile(r"`([A-Za-z_][\w]*(?:::[A-Za-z_]\w*)*)(?:\(\))?`")
VTABLE_WORDS = re.compile(r"\b(?:vtable|vftable|vtbl|virtual table|vfptr)\b", re.I)
NOTE_PREFIXES = ("note:", "ghidra:")
PROSE_STOP = {"the", "a", "an", "of", "and", "or", "to", "in", "on", "at", "by", "with", "from", "is", "its", "it",
              "via", "calls", "call", "called", "see", "then", "when", "which", "that", "this", "as", "for", "but",
              "not", "same", "both", "each", "every", "also", "into", "after", "before", "only", "original", "port"}


class Mention:
    __slots__ = ("addr", "path", "line", "kind", "text", "entry", "label", "name", "name_rank", "port", "mtime", "lead")

    def __init__(self, **kw):
        for k in self.__slots__:
            setattr(self, k, kw.get(k))


def git_mtimes() -> dict[str, int]:
    """Last commit time per path in one git log pass (newest first, so first seen wins)."""
    out = subprocess.run(["git", "log", "--format=@%ct", "--name-only", "--", "docs", "crates", "PROJECT_LOG.md", "log"],
                         cwd=rc.REPO, capture_output=True, text=True, encoding="utf-8").stdout
    times: dict[str, int] = {}
    t = 0
    for line in out.splitlines():
        if line.startswith("@"):
            t = int(line[1:])
        elif line and line not in times:
            times[line] = t
    return times


def load_functions():
    path = rc.cache_dir() / "functions.tsv"
    if not path.exists():
        return None
    entries, ends = [], []
    for line in path.read_text(encoding="utf-8").splitlines()[1:]:
        f = line.split("\t")
        entries.append(int(f[0], 16))
        ends.append(int(f[1], 16))
    return entries, ends


def containing(funcs, addr):
    if not funcs:
        return None
    entries, ends = funcs
    i = bisect.bisect_right(entries, addr) - 1
    if i >= 0 and addr <= ends[i]:
        return entries[i]
    return None


def in_image(v: int) -> bool:
    return TEXT[0] <= v < TEXT[1] or DATA[0] <= v < DATA[1]


def accept(m: re.Match, line: str, funcs) -> int | None:
    prefixed, digits = m.group(1), m.group(2)
    if len(digits) == 8 and not digits.startswith("00"):
        return None
    if len(digits) == 7:
        return None
    v = int(digits, 16)
    if not in_image(v):
        return None
    has_letter = any(c in "abcdefABCDEF" for c in digits)
    if not prefixed:
        if has_letter and digits.isalpha() and not digits.isupper():
            return None  # English words such as "accede"
        if has_letter and not (digits.isupper() or digits.islower()):
            return None  # mixed case hashes
        if not has_letter:
            if funcs is None or v not in set_of_entries(funcs):
                return None
            after = line[m.end():m.end() + 8]
            before = line[max(0, m.start() - 16):m.start()]
            if UNIT_AFTER.match(after) or WORD_BEFORE_REJECT.search(before) or re.search(r"\d[,.]$", before):
                return None
    return v


_ENTRY_SET = None


def set_of_entries(funcs):
    global _ENTRY_SET
    if _ENTRY_SET is None:
        _ENTRY_SET = set(funcs[0])
    return _ENTRY_SET


def snake(words: list[str]) -> str:
    s = "_".join(w.lower() for w in words)
    return re.sub(r"[^a-z0-9_]", "", s)


def identifier_like(name: str) -> bool:
    """Function-like identifiers only: snake_case with an underscore, a Rust path or CamelCase.

    Single words in backticks are usually XML keys or strings (`reverse`, `default`)."""
    last = name.split("::")[-1]
    if re.fullmatch(r"(?:0x)?[0-9A-Fa-f_]+", last) or len(last) < 4:
        return False
    if "::" in name:
        return True
    if re.fullmatch(r"[a-z][a-z0-9]*(?:_[a-z0-9]+)+", last):
        return True
    return bool(re.fullmatch(r"(?:[A-Z][a-z0-9]+){2,}", last))


def prose_name(line: str, start: int, end: int) -> str:
    """'Car listener pose (43F7A0)' -> car_listener_pose; '**Car listener 43F7A0**' -> car_listener."""
    before = line[:start].rstrip()
    before = re.sub(r"[(\[*`]+$", "", before).rstrip()
    words = re.findall(r"[A-Za-z][A-Za-z0-9'-]*", before[-60:])
    picked = []
    for w in reversed(words):
        if w.lower() in PROSE_STOP or len(picked) >= 3:
            break
        picked.insert(0, w.replace("'", "").replace("-", "_"))
    if picked and not all(len(w) <= 2 for w in picked) and not re.match(r"^(?:F\d+|[DVIO])$", picked[-1]):
        return snake(picked)
    return ""


def label_for(line: str, pos: int, para_label: str | None) -> str | None:
    best = None
    for lm in LABEL.finditer(line):
        if lm.start() <= pos:
            best = lm.group(1)
        elif best is None:
            best = lm.group(1)
    return best or para_label


def scan_markdown(path: Path, rel: str, kind: str, funcs, mtime: int, out: list, entry: str | None = None):
    para_label = None
    for no, line in enumerate(path.read_text(encoding="utf-8", errors="replace").splitlines(), 1):
        if kind == "log" and not re.match(r"log/[FG]\d+\.md$", rel):
            em = ENTRY_LINE.match(line) or ENTRY_TAIL.match(line)
            if em:
                entry = em.group(1)
        if not line.strip() or line.lstrip().startswith(("#", "- ", "* ", "|")) and not line.startswith("  "):
            para_label = None
        lab = LABEL.search(line)
        if lab and line.lstrip()[:1] in "-*" or (lab and lab.start() < 4):
            para_label = lab.group(1)
        hits = [(m.start(), m.end(), accept(m, line, funcs)) for m in TOKEN.finditer(line)]
        hits += [(m.start(), m.end(), int(m.group(1), 16)) for m in GHIDRA_NAME.finditer(line)
                 if in_image(int(m.group(1), 16))]
        for s, e, v in hits:
            if v is None:
                continue
            name, rank = "", 0
            # `name` (ADDR) / `name` at ADDR / ADDR `name` / ADDR (`name`)
            pre = line[max(0, s - 80):s]
            mb = re.search(r"`([A-Za-z_][\w]*(?:::[A-Za-z_]\w*)*)(?:\(\))?`\s*(?:\(|at\s+|=\s*|:\s*)?(?:0x)?$", pre)
            ma = re.match(r"^[`)]?\s*(?:\(|:|=|—|-)?\s*`([A-Za-z_][\w]*(?:::[A-Za-z_]\w*)*)(?:\(\))?`", line[e:e + 80])
            for cand in (mb, ma):
                if cand and identifier_like(cand.group(1)):
                    name, rank = cand.group(1), 2
                    break
            out.append(Mention(addr=v, path=rel, line=no, kind=kind, text=line.strip(), entry=entry,
                               label=label_for(line, s, para_label), name=name, name_rank=rank, port=None,
                               mtime=mtime, lead=False))


def scan_rust(path: Path, rel: str, funcs, mtime: int, out: list):
    lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    probe = "grid-probes" in rel
    module = rel.split("/src/", 1)[-1].removesuffix(".rs").replace("/", "::").removesuffix("::mod").removesuffix("lib").removesuffix("::main")
    crate = rel.split("/")[1] if rel.startswith("crates/") else ""
    current_fn = None
    for i, line in enumerate(lines):
        fm = RUST_FN.match(line)
        if fm:
            current_fn = fm.group(1)
        stripped = line.lstrip()
        if "//" not in line:
            continue
        is_doc = stripped.startswith("///") or stripped.startswith("//!")
        comment = line[line.index("//"):]
        hits = [(m, accept(m, comment, funcs)) for m in TOKEN.finditer(comment)]
        hits += [(m, int(m.group(1), 16)) for m in GHIDRA_NAME.finditer(comment)]
        hits = [(m, v) for m, v in hits if v is not None and in_image(v)]
        if not hits:
            continue
        item, item_kind, earlier = "", "", False
        if stripped.startswith("///"):
            bs = i
            while bs > 0 and lines[bs - 1].lstrip().startswith("///"):
                bs -= 1
            earlier = any(any(accept(m, lines[k], funcs) for m in TOKEN.finditer(lines[k])) for k in range(bs, i))
            j = i + 1
            while j < len(lines) and (lines[j].lstrip().startswith(("///", "#[", "//")) or not lines[j].strip()):
                j += 1
            im = RUST_ITEM.match(lines[j]) if j < len(lines) else None
            if any(lines[k].lstrip().startswith("#[test") for k in range(i + 1, j)) or "/tests/" in rel:
                im = None  # a test documents what it checks, it is not the port of the address
            item = im.group(3) if im else ""
            item_kind = "fn" if im and im.group(1) else (im.group(2) if im else "")
        for n, (m, v) in enumerate(hits):
            lead = bool(item) and not earlier and n == 0
            rank = 0
            if lead:
                code = TEXT[0] <= v < TEXT[1]
                rank = 4 if (code and item_kind == "fn") or (not code and item_kind in ("const", "static")) else 2
            if probe and rank:
                rank = 3
            where = f"{crate}::{module}" + (f"::{item}" if item else (f" (in {current_fn})" if current_fn and not is_doc else ""))
            out.append(Mention(addr=v, path=rel, line=i + 1, kind="probe" if probe else "rust", text=stripped,
                               entry=None, label=label_for(comment, m.start(), None), name=item if rank else "",
                               name_rank=rank, port=None if probe else where, mtime=mtime, lead=lead))


GENERIC = {"new", "default", "from", "mean", "update", "step", "run", "apply", "get", "set", "read", "parse", "load",
           "init", "reset", "build", "next", "tick", "eval", "value", "advance", "start", "finish", "clear", "goal"}


def qualified(m) -> str:
    """Last path segment; generic Rust method names get their module in front (continuous_mean)."""
    last = m.name.split("::")[-1]
    if m.kind in ("rust", "probe") and (last in GENERIC or len(last) <= 4) and m.port:
        module = m.port.split(" ")[0].split("::")[-2] if m.port.count("::") >= 2 else m.port.split("::")[-1]
        return f"{module}_{last}"
    return last


def classify(addr: int, mentions: list, funcs) -> tuple[str, str]:
    if TEXT[0] <= addr < TEXT[1]:
        c = containing(funcs, addr)
        if funcs is None or c == addr:
            return "func", ""
        if c is None:
            return "func", "not a function entry in the master analysis"
        return "label", f"inside {rc.fmt(c)}"
    if any(VTABLE_WORDS.search(m.text) for m in mentions):
        return "vtable", ""
    return "global", ""


def build_rows(mentions: list, funcs) -> dict[int, dict]:
    by_addr = defaultdict(list)
    for m in mentions:
        by_addr[m.addr].append(m)
    rows = {}
    for addr, ms in by_addr.items():
        kind, note = classify(addr, ms, funcs)
        named = sorted((m for m in ms if m.name_rank), key=lambda m: (m.name_rank, m.mtime or 0, -m.line), reverse=True)
        name = qualified(named[0]) if named else ""
        strong = {m.name.split("::")[-1] for m in named if m.name_rank >= 2}
        conflicts = sorted(strong - {name})
        ports = []
        for m in sorted((m for m in ms if m.kind == "rust"), key=lambda m: (not m.lead, -(m.mtime or 0))):
            p = f"{m.path}:{m.line} {m.port}"
            if m.path not in {x.split(':')[0] for x in ports}:
                ports.append(p)
        docs = defaultdict(list)
        for m in ms:
            if m.kind == "doc":
                docs[m.path].append(m.line)
        doc_list = [f"{p}:{min(ls)}" for p, ls in sorted(docs.items(), key=lambda kv: (-len(kv[1]), kv[0]))[:3]]
        entries = sorted({m.entry for m in ms if m.entry}, key=lambda e: (e[0] == "F", int(e[1:])), reverse=True)
        labels = [m.label for m in ms if m.label]
        conf = ""
        if labels:
            # strongest evidence first: verified, original-observed, derived, inferred
            for c in ("V", "O", "D", "I"):
                if c in labels:
                    conf = f"[{c}]"
                    break
        lead = sorted(ms, key=lambda m: ({"rust": 0, "doc": 1, "log": 2, "probe": 3}[m.kind], -(m.mtime or 0)))[0]
        snippet = re.sub(r"^[/!#*\-\s]+", "", lead.text)
        snippet = snippet if len(snippet) <= 160 else snippet[:157] + "..."
        comment = "; ".join(x for x in (note, snippet) if x)
        rows[addr] = {"address": rc.fmt(addr), "name": name, "kind": kind,
                      "port": " ; ".join(ports[:3]) if ports else "not ported",
                      "doc": " ; ".join(doc_list), "entry": " ".join(entries[:3]), "confidence": conf,
                      "conflicts": " ".join(conflicts[:4]), "comment": comment, "_mentions": len(ms)}
    return rows


def collect(funcs) -> list:
    times = git_mtimes()
    mentions: list = []
    repo = rc.REPO
    for p in sorted((repo / "docs").rglob("*.md")):
        rel = p.relative_to(repo).as_posix()
        if rel.startswith(("docs/re/", "docs/reference/", "docs/templates/")):
            continue
        scan_markdown(p, rel, "doc", funcs, times.get(rel, 0), mentions)
    # log/F0530.md and log/G0129.md hold one entry each (F532 layout); log/legacy/ and the
    # old monolithic PROJECT_LOG.md carry their entry ids in header lines
    logs = [repo / "PROJECT_LOG.md"]
    if (repo / "log").is_dir():
        logs += [p for p in sorted((repo / "log").rglob("*.md")) if p.name != "INDEX.md"]
    for p in logs:
        rel = p.relative_to(repo).as_posix()
        fm = re.fullmatch(r"([FG])0*(\d+)\.md", p.name)
        scan_markdown(p, rel, "log", funcs, times.get(rel, 0), mentions, f"{fm.group(1)}{fm.group(2)}" if fm else None)
    for p in sorted((repo / "crates").rglob("*.rs")):
        rel = p.relative_to(repo).as_posix()
        if "/target/" in rel:
            continue
        scan_rust(p, rel, funcs, times.get(rel, 0), mentions)
    return mentions


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--fresh", action="store_true", help="ignore the existing symbols.tsv")
    ap.add_argument("--out", default=str(rc.SYMBOLS))
    ap.add_argument("--min-mentions", type=int, default=1)
    a = ap.parse_args()
    funcs = load_functions()
    if funcs is None:
        print("note: local/re-cache/functions.tsv missing; bare all-digit addresses are skipped", file=sys.stderr)
    mentions = collect(funcs)
    rows = build_rows(mentions, funcs)
    old = {} if a.fresh else {int(r["address"], 16): r for r in rc.read_symbols(Path(a.out))}
    merged = []
    for addr in sorted(set(rows) | set(old)):
        new, prev = rows.get(addr), old.get(addr)
        if new is None:
            merged.append(prev)  # curated row no longer mentioned in text: keep it
            continue
        if new["_mentions"] < a.min_mentions and prev is None:
            continue
        if prev:
            # a name already in symbols.tsv (curated, or merged from Ghidra) wins over a mined one
            if prev.get("name"):
                if new["name"] and new["name"] != prev["name"]:
                    new["conflicts"] = " ".join(sorted((set(new["conflicts"].split()) | {new["name"]}) - {prev["name"]}))
                new["name"] = prev["name"]
            if prev.get("kind") in ("vtable", "struct"):
                new["kind"] = prev["kind"]
            if not new["confidence"]:
                new["confidence"] = prev.get("confidence", "")
            if prev.get("conflicts"):
                new["conflicts"] = " ".join(sorted((set(new["conflicts"].split()) | set(prev["conflicts"].split())) - {new["name"]}))
            notes = [x for x in (prev.get("comment") or "").split(" | ") if x.startswith(NOTE_PREFIXES)]
            new["comment"] = " | ".join([new["comment"]] + notes)
        merged.append(new)
    rc.write_symbols(merged, Path(a.out))
    named = sum(1 for r in merged if r["name"])
    ported = sum(1 for r in merged if r["port"] != "not ported")
    kinds = defaultdict(int)
    for r in merged:
        kinds[r["kind"]] += 1
    print(f"mentions={len(mentions)} rows={len(merged)} named={named} ported={ported} kinds={dict(kinds)} -> {a.out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
