"""Export user names/comments from every Ghidra project under local/ (main checkout and worktrees).

Writes local/re-harvest/<slug>.tsv per project (read-only Ghidra runs, one at a time) and
prints a size table. Merge the results with merge_symbols.py. The old projects are not touched.
"""
from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import re_common as rc  # noqa: E402


def find_projects() -> list[Path]:
    main = rc.main_root()
    roots = [main / "local"] + sorted(p / "local" for p in (main / ".claude" / "worktrees").glob("*") if (p / "local").is_dir())
    out = []
    for r in roots:
        out += [g for g in r.rglob("*.gpr") if "ghidra-master" not in g.parts]
    return out


def size_mb(gpr: Path) -> float:
    rep = gpr.with_suffix(".rep")
    return sum(f.stat().st_size for f in rep.rglob("*") if f.is_file()) / 2**20 if rep.exists() else 0.0


def slug(gpr: Path) -> str:
    rel = gpr.relative_to(rc.main_root())
    return "_".join(rel.parts[:-1]).replace(".claude_worktrees_", "wt_").replace("local_", "")


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--list", action="store_true", help="only list projects and sizes")
    p.add_argument("--maxmem", default="3G")
    a = p.parse_args()
    out_dir = rc.local() / "re-harvest"
    out_dir.mkdir(parents=True, exist_ok=True)
    for gpr in find_projects():
        print(f"{size_mb(gpr):8.0f} MB  {gpr}")
        if a.list:
            continue
        out = out_dir / f"{slug(gpr)}.tsv"
        if out.exists():
            continue
        label = str(gpr.parent.relative_to(rc.main_root())).replace("\\", "/")
        subprocess.run([sys.executable, "-I", str(Path(__file__).with_name("master.py")), "--maxmem", a.maxmem,
                        "export", "--project", str(gpr), "--out", str(out), "--label", label],
                       stdout=subprocess.DEVNULL)
        print(f"          -> {out} ({sum(1 for _ in out.open(encoding='utf-8')) - 1 if out.exists() else 'FAILED'} rows)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
