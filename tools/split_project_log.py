"""Split the old single-file PROJECT_LOG.md into one file per entry under log/.

Used once on 2026-10-21 (F532) to move the 1.4 MB log into log/; kept so the split can
be repeated and checked. Run with `py -I`:

    git show d927d415:PROJECT_LOG.md > old.md
    py -I tools/split_project_log.py old.md log            # write log/
    py -I tools/split_project_log.py old.md log --verify   # prove the split is lossless

Parsing rules (the old log mixed three formats):
- Main part, from the line after the title up to "## Objective and current state":
  an entry starts at a line beginning `F<digits>` or `## G<digits>`. A line beginning
  `G<digits>` starts an entry only below the last `## G` heading (the older one-paragraph
  graphics entries); above it such lines are paragraphs inside a `## G` entry.
- Legacy tail (the 6-9 October layout): an entry starts at a `##`/`###` heading or at a
  `G<digits>` line. A heading with no text before the next heading joins the next entry.
- An entry is its lines up to the next entry start, without trailing blank lines.

Files: entries with the same series and number go to one file, `log/F0531.md` or
`log/G0129.md`, in their original order, separated by one blank line. A legacy heading that
names one finding, "... (F103)", goes to `log/F0103.md`; other legacy sections go to
`log/legacy/NNN-slug.md` in file order. An entry that repeats an earlier entry of the same
number byte for byte is written once and reported.

--verify re-reads the old log and the written files and checks that every old entry sits,
unchanged and in its original order, in exactly one file, that the files contain nothing
else apart from the blank-line separators, and that the old log is rebuilt byte for byte
from the entries plus the separators it had.
"""

import argparse
import re
import sys
from pathlib import Path

TAIL_START = "## Objective and current state"


def parse(text):
    """Return (preamble, entries); entries are dicts with key, text, sep (text after it)."""
    lines = text.split("\n")
    tail = next(i for i, line in enumerate(lines) if line.startswith(TAIL_START))
    last_hash_g = max(i for i in range(tail) if re.match(r"## G\d+", lines[i]))
    starts = []
    for i, line in enumerate(lines):
        if i < 2:
            continue
        if i < tail:
            if re.match(r"F\d+", line) or re.match(r"## G\d+", line):
                starts.append(i)
            elif re.match(r"G\d+", line) and i > last_hash_g:
                starts.append(i)
        elif re.match(r"#{2,3} ", line) or re.match(r"G\d+", line):
            starts.append(i)
    # A legacy heading with nothing under it joins the next entry.
    merged = []
    for s in starts:
        if merged and merged[-1] >= tail and not "".join(lines[merged[-1] + 1 : s]).strip():
            continue
        merged.append(s)
    starts = merged
    preamble = "\n".join(lines[: starts[0]]) + "\n"
    entries = []
    legacy_no = 0
    for n, s in enumerate(starts):
        e = starts[n + 1] if n + 1 < len(starts) else len(lines)
        body = lines[s:e]
        while body and body[-1] == "":
            body.pop()
        # Everything after the entry's text up to the next entry: line ends and blank lines.
        sep = "\n" * (e - s - len(body) + (1 if n + 1 < len(starts) else 0))
        first = body[0]
        if s < tail or re.match(r"G\d+", first):
            m = re.match(r"(?:## )?([FG])(\d+)", first)
            key = "%s%04d" % (m.group(1), int(m.group(2)))
        else:
            # The entry's own heading: the first `###` one if a bare `##` heading was joined.
            heading = next((l for l in body if l.startswith("### ")), first)
            m = re.search(r"\(F(\d+)\)\s*$", heading)
            if m and heading.startswith("### "):
                key = "F%04d" % int(m.group(1))
            else:
                legacy_no += 1
                slug = re.sub(r"[^a-z0-9]+", "-", heading.lstrip("#").lower()).strip("-")[:60].strip("-")
                key = "legacy/%03d-%s" % (legacy_no, slug)
        entries.append({"key": key, "text": "\n".join(body), "sep": sep, "line": s + 1})
    assert rebuild(preamble, entries) == text, "parser does not cover the whole old log"
    return preamble, entries


def rebuild(preamble, entries, texts=None):
    return preamble + "".join((texts[i] if texts else en["text"]) + en["sep"] for i, en in enumerate(entries))


def group(entries):
    files, dups = {}, []
    for en in entries:
        parts = files.setdefault(en["key"], [])
        if en["text"] in parts:
            dups.append(en)
            continue
        parts.append(en["text"])
    return {k: "\n\n".join(v) + "\n" for k, v in files.items()}, dups


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("old_log")
    ap.add_argument("out_dir")
    ap.add_argument("--verify", action="store_true")
    a = ap.parse_args()
    text = Path(a.old_log).read_bytes().decode("utf-8").replace("\r\n", "\n")
    preamble, entries = parse(text)
    files, dups = group(entries)
    out = Path(a.out_dir)
    if not a.verify:
        (out / "legacy").mkdir(parents=True, exist_ok=True)
        for key, content in files.items():
            (out / (key + ".md")).write_bytes(content.encode("utf-8"))
        print("wrote %d files from %d entries" % (len(files), len(entries)))
        for d in dups:
            print("exact repeat written once: old line %d, %s" % (d["line"], d["key"]))
        return 0
    # Verify: walk old entries in order and consume them from the written files.
    pos, texts, bad = {}, [], 0
    contents = {}
    for p in sorted(out.glob("[FG][0-9]*.md")) + sorted((out / "legacy").glob("*.md")):
        key = p.relative_to(out).as_posix()[:-3]
        contents[key] = p.read_bytes().decode("utf-8").replace("\r\n", "\n")
    seen = {}
    for en in entries:
        c = contents.get(en["key"])
        if c is None:
            print("MISSING file for", en["key"]); bad += 1; texts.append(""); continue
        if en["text"] in seen.setdefault(en["key"], []):
            # Exact repeat: take the text from where the first copy sits in the file.
            i = c.find(en["text"] + "\n")
            texts.append(c[i : i + len(en["text"])])
            continue
        p = pos.get(en["key"], 0)
        if p:
            if c[p : p + 1] != "\n":
                print("bad separator in", en["key"]); bad += 1
            p += 1
        if c[p : p + len(en["text"])] != en["text"] or c[p + len(en["text"])] != "\n":
            print("MISMATCH", en["key"], "old line", en["line"]); bad += 1
        texts.append(c[p : p + len(en["text"])])
        seen[en["key"]].append(en["text"])
        pos[en["key"]] = p + len(en["text"]) + 1
    for key, c in contents.items():
        if key not in files:
            print("not from the old log, ignored:", key)
        elif pos.get(key, 0) != len(c):
            print("EXTRA bytes in", key, pos.get(key, 0), len(c)); bad += 1
    if rebuild(preamble, entries, texts) != text:
        print("REBUILD differs from the old log"); bad += 1
    print("old log %d bytes, %d entries -> %d files; exact repeats %d" % (
        len(text.encode("utf-8")), len(entries), len(contents), len(dups)))
    print("verify:", "FAILED (%d)" % bad if bad else "OK, old log rebuilt byte for byte from log/ files")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
