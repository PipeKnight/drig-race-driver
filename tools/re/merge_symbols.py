"""Merge names/comments exported by tools/ghidra/ExportSymbols.java into docs/re/symbols.tsv.

Never clobbers: an existing name stays and a different exported name is added to `conflicts`;
new addresses get a row with port "not ported"; comments are appended as "ghidra: ..." notes.
Ghidra placeholders (FUN_/LAB_/DAT_..., probe_<addr> from GridProbe) and PE-loader comments
(IMAGE_*, Rsrc_*, section names, import thunks) are skipped.

  py -I tools/re/merge_symbols.py local/re-harvest/*.tsv [--dry-run]
"""
from __future__ import annotations

import argparse
import glob
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import re_common as rc  # noqa: E402

PLACEHOLDER = re.compile(r"^(?:FUN|LAB|DAT|PTR|SUB|switchD|caseD|thunk_FUN|s|u|probe|Ordinal|entry|EXT)_[0-9A-Fa-f_]*$|^(?:entry|FID_conflict:.*)$")
LOADER_COMMENT = re.compile(r"IMAGE_|^Rsrc_|<<not bound>>|Size of resource|^\.(?:text|rdata|data|tls|rsrc|reloc|extra)$|^CONST$|^Exception |^Library Function|PDB Info|^Debug Info")
MAX_NOTE = 240


def load_exports(paths: list[str]) -> list[dict]:
    rows = []
    for pattern in paths:
        for path in sorted(glob.glob(pattern)) or [pattern]:
            lines = Path(path).read_text(encoding="utf-8").splitlines()
            if not lines:
                continue
            head = lines[0].split("\t")
            rows += [dict(zip(head, line.split("\t"))) for line in lines[1:] if line.strip()]
    return rows


def unescape(text: str) -> str:
    return text.replace("\\n", " / ").replace("\\t", " ").replace("\\\\", "\\")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("exports", nargs="+")
    ap.add_argument("--symbols", default=str(rc.SYMBOLS))
    ap.add_argument("--dry-run", action="store_true")
    a = ap.parse_args()
    table = {int(r["address"], 16): r for r in rc.read_symbols(Path(a.symbols))}
    added = named = conflicts = notes = skipped = 0
    for e in load_exports(a.exports):
        addr = int(e["address"], 16)
        name = (e.get("name") or "").split("::")[-1]
        comment = unescape(e.get("comment") or "").strip()
        if name and PLACEHOLDER.match(name):
            name = ""
        if comment and (LOADER_COMMENT.search(comment) or comment.startswith("[symbols.tsv]")):
            comment = ""
        if not name and not comment:
            skipped += 1
            continue
        row = table.get(addr)
        if row is None:
            row = {"address": rc.fmt(addr), "name": "", "kind": e.get("kind") or ("func" if addr < 0xC89000 else "global"),
                   "port": "not ported", "doc": "", "entry": "", "confidence": "", "conflicts": "", "comment": ""}
            table[addr] = row
            added += 1
        if name:
            if not row["name"]:
                row["name"] = name
                named += 1
            elif row["name"] != name and name not in row["conflicts"].split():
                row["conflicts"] = " ".join(filter(None, [row["conflicts"], name]))
                conflicts += 1
        if comment:
            note = f"ghidra: {comment[:MAX_NOTE]}"
            if note not in row["comment"]:
                row["comment"] = " | ".join(filter(None, [row["comment"], note]))
                notes += 1
    print(f"added={added} named={named} conflicts={conflicts} notes={notes} skipped={skipped}")
    if not a.dry_run:
        rc.write_symbols(list(table.values()), Path(a.symbols))
    return 0


if __name__ == "__main__":
    sys.exit(main())
