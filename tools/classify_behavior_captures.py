"""Classify local/behavior entries by whether the project's text records cite them.

Usage:
  python -I tools/classify_behavior_captures.py <repo> <behavior-sizes.tsv> <out_dir> [extra checkout or file ...]

<behavior-sizes.tsv> comes from
  tools/disk_report.ps1 -Children local\\behavior -Tsv <file>
Extra arguments add other checkouts (worktrees, the graphics checkout) or single notes
files to the citation corpus. Citations are matched as whole path components, with
{a,b} brace lists expanded and * globs matched. Classes:
  cited                     named somewhere (whole folder or a file inside it)
  uncited-sibling-of-cited  not named, but a run of the same family is (superseded/duplicate)
  uncited-family            no run of the family is named (unknown; keep by default)
Read-only for the repo; writes behavior_classified.tsv into out_dir.
"""
import fnmatch
import os
import re
import sys
from collections import defaultdict

repo, tsv, out = sys.argv[1:4]
extra_roots = sys.argv[4:]
GiB = 1 << 30

# Text corpus: log, docs, agents file, independently authored tools.
corpus_files = []
for r0 in [repo] + extra_roots:
    if os.path.isfile(r0):
        corpus_files.append(r0)
        continue
    corpus_files += [os.path.join(r0, "PROJECT_LOG.md"), os.path.join(r0, "AGENTS.md")]
    for base, exts in (("log", (".md",)), ("docs", (".md", ".txt", ".tsv", ".json")), ("tools", (".py", ".ps1", ".cmd", ".rs", ".h", ".c", ".cpp", ".md", ".txt", ".json"))):
        for root, dirs, files in os.walk(os.path.join(r0, base)):
            dirs[:] = [d for d in dirs if d != "__pycache__"]
            for f in files:
                if f.lower().endswith(exts):
                    corpus_files.append(os.path.join(root, f))

BRACE = re.compile(r"\{([^{}\s]{1,200})\}")


def expand(tok, limit=256):
    m = BRACE.search(tok)
    if not m:
        return [tok]
    opts = m.group(1).split(",")
    res = []
    for o in opts:
        res.extend(expand(tok[: m.start()] + o + tok[m.end():], limit))
        if len(res) > limit:
            break
    return res


TOKEN = re.compile(r"[A-Za-z0-9_.{},*\\/-]+")
text_by_file = {}
expanded_tokens = defaultdict(set)  # token -> files
for path in corpus_files:
    try:
        with open(path, encoding="utf-8", errors="replace") as fh:
            t = fh.read()
    except OSError:
        continue
    for tok in TOKEN.findall(t):
        for e in expand(tok) if "{" in tok else [tok]:
            expanded_tokens[e.replace("\\", "/")].add(path)

# Map each path component sequence to the token set for fast lookup.
# A behavior entry is cited if some token contains "<name>" as a whole path component
# (bounded by start, "/" or end), or the token equals the name.
component_hits = defaultdict(set)  # entry name -> set of "sub path" strings cited (may be "")
component_files = defaultdict(set)
for tok, files in expanded_tokens.items():
    parts = [p for p in tok.split("/") if p]
    for i, p in enumerate(parts):
        component_hits[p].add("/".join(parts[i + 1:]))
        component_files[p] |= files

rows = []
with open(tsv, encoding="utf-8-sig") as fh:
    header = fh.readline().rstrip("\n").split("\t")
    for line in fh:
        v = line.rstrip("\r\n").split("\t")
        rows.append(dict(zip(header, v)))


def family(name):
    n = re.sub(r"-(debug|release)(?=-|$)", "", name)
    n = re.sub(r"-(draft|final|smoke)-\d+$", "", n)
    n = re.sub(r"-f\d+-regression-\d+$", "", n)
    n = re.sub(r"-\d{2}$", "", n)
    return n


def kind(name):
    if re.search(r"-draft-\d+", name):
        return "draft"
    if re.search(r"-smoke-\d+", name):
        return "smoke"
    if re.search(r"-f\d+-regression-\d+", name):
        return "regression-rerun"
    if re.search(r"-final-\d+", name):
        return "final"
    if re.match(r"^\d{8}T\d+Z?$", name):
        return "timestamped"
    if re.search(r"\.(exe|obj|pdb|ilk|lib|exp)$", name, re.I):
        return "probe-build"
    if "." in name:
        return "file"
    return "other"


# Glob citations such as `contact-bias-20261010-*-final-02/validation.json`.
names = [r["name"] for r in rows]
for comp in [c for c in list(component_hits) if "*" in c and len(c.replace("*", "")) >= 8]:
    for nm in fnmatch.filter(names, comp):
        component_hits[nm] |= {h or "(glob)" for h in component_hits[comp]}
        component_files[nm] |= component_files[comp]

out_rows = []
for r in rows:
    name = r["name"]
    size = int(r["bytes"])
    hits = component_hits.get(name, set())
    cited = bool(hits)
    sub = sorted(h for h in hits if h)
    out_rows.append({
        "name": name, "bytes": size, "files": int(r["files"]), "cited": cited,
        "whole": "" in hits, "cited_sub": ";".join(sub[:20]), "family": family(name), "kind": kind(name),
        "ext": r.get("ext", ""), "sources": ";".join(sorted(component_files.get(name, set()))[:5]),
    })

# Family-level citation: an uncited entry whose family has a cited sibling is a likely duplicate/superseded run.
fam_cited = defaultdict(bool)
for o in out_rows:
    if o["cited"]:
        fam_cited[o["family"]] = True
for o in out_rows:
    if o["cited"]:
        o["class"] = "cited"
    elif fam_cited[o["family"]]:
        o["class"] = "uncited-sibling-of-cited"
    else:
        o["class"] = "uncited-family"

os.makedirs(out, exist_ok=True)
with open(os.path.join(out, "behavior_classified.tsv"), "w", encoding="utf-8") as fh:
    cols = ["name", "bytes", "files", "class", "kind", "family", "whole", "cited_sub", "sources", "ext"]
    fh.write("\t".join(cols) + "\n")
    for o in sorted(out_rows, key=lambda o: -o["bytes"]):
        fh.write("\t".join(str(o[c]) for c in cols) + "\n")

agg = defaultdict(lambda: [0, 0])
for o in out_rows:
    a = agg[(o["class"], o["kind"])]
    a[0] += 1
    a[1] += o["bytes"]
tot = sum(o["bytes"] for o in out_rows)
print(f"total {tot / GiB:.2f} GiB in {len(out_rows)} entries")
for k in sorted(agg, key=lambda k: -agg[k][1]):
    print(f"{k[0]:26s} {k[1]:17s} {agg[k][0]:5d} {agg[k][1] / GiB:7.2f} GiB")
cls = defaultdict(int)
for o in out_rows:
    cls[o["class"]] += o["bytes"]
for k, v in cls.items():
    print(f"CLASS {k}: {v / GiB:.2f} GiB")
# Cited entries cited only through files inside (not as a whole folder)
only_sub = [o for o in out_rows if o["cited"] and not o["whole"]]
print(f"cited only via inner files: {len(only_sub)} entries, {sum(o['bytes'] for o in only_sub) / GiB:.2f} GiB")
