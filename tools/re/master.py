"""Manage the shared master Ghidra project in <main checkout>/local/ghidra-master.

  build      import local/compatibility/grid-unpacked.exe and run full auto-analysis (once, ~1 h)
  apply      apply docs/re/symbols.tsv names/plates to the master (writer; coordinator only)
  export     dump user names/comments of any project to TSV (read-only) for merge_symbols.py
  decompile  decompile functions containing addresses into local/re-cache (read-only)
  functions  write local/re-cache/functions.tsv (entry, end, name) used by tools/re (read-only)
  snapshot   copy the master to a private folder for interactive work that renames things

Only one Ghidra process may hold a project. Read-only commands wait for the lock to clear.
"""
from __future__ import annotations

import argparse
import hashlib
import shutil
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import re_common as rc  # noqa: E402

PROJECT_NAME = "grid-master"
PROGRAM = "grid-unpacked.exe"
SCRIPTS = rc.REPO / "tools" / "ghidra"


def wait_unlocked(project_dir: Path, name: str, timeout: float = 900) -> None:
    deadline = time.time() + timeout
    while rc.project_locked(project_dir, name):
        if time.time() > deadline:
            sys.exit(f"{project_dir / name} is still locked by another Ghidra process")
        print(f"waiting for {name}.lock ...", file=sys.stderr)
        time.sleep(10)


def logs(tag: str) -> list[str]:
    d = rc.master_project() / "logs"
    d.mkdir(parents=True, exist_ok=True)
    return ["-log", str(d / f"{tag}.log"), "-scriptlog", str(d / f"{tag}-script.log")]


def cmd_build(a) -> int:
    proj = rc.master_project()
    if (proj / f"{PROJECT_NAME}.gpr").exists() and not a.force:
        sys.exit(f"{proj} already exists; pass --force to re-import (deletes nothing else)")
    exe = rc.exe_path()
    digest = hashlib.sha256(exe.read_bytes()).hexdigest()
    if digest != rc.EXE_SHA256:
        sys.exit(f"{exe} sha256 {digest} != expected {rc.EXE_SHA256}")
    proj.mkdir(parents=True, exist_ok=True)
    args = [str(proj), PROJECT_NAME, "-import", str(exe), "-overwrite", "-max-cpu", str(a.cpu),
            "-analysisTimeoutPerFile", str(a.timeout)] + logs("build")
    args += ["-scriptPath", str(SCRIPTS)]
    if a.apply:
        args += ["-postScript", "ImportSymbols.java", str(rc.SYMBOLS), str(proj / "logs" / "import-report.txt")]
    args += ["-postScript", "FunctionTable.java", str(rc.cache_dir() / "functions.tsv")]
    return rc.ghidra_headless(args, a.maxmem, background=True)


def cmd_functions(a) -> int:
    proj = rc.master_project()
    wait_unlocked(proj, PROJECT_NAME)
    return rc.ghidra_headless([str(proj), PROJECT_NAME, "-process", PROGRAM, "-noanalysis", "-readOnly",
                               "-scriptPath", str(SCRIPTS), "-postScript", "FunctionTable.java",
                               str(rc.cache_dir() / "functions.tsv")] + logs("functions"), a.maxmem)


def cmd_apply(a) -> int:
    proj = rc.master_project()
    wait_unlocked(proj, PROJECT_NAME)
    tsv = Path(a.tsv or rc.SYMBOLS)
    report = proj / "logs" / "import-report.txt"
    return rc.ghidra_headless([str(proj), PROJECT_NAME, "-process", PROGRAM, "-noanalysis",
                               "-scriptPath", str(SCRIPTS), "-postScript", "ImportSymbols.java",
                               str(tsv), str(report)] + logs("apply"), a.maxmem)


def cmd_export(a) -> int:
    gpr = Path(a.project) if a.project else rc.master_project() / f"{PROJECT_NAME}.gpr"
    proj_dir, name = gpr.parent, gpr.stem
    wait_unlocked(proj_dir, name)
    out = Path(a.out)
    args = [str(proj_dir), name]
    args += ["-process", a.program] if a.program else ["-process"]
    args += ["-recursive", "-noanalysis", "-readOnly", "-scriptPath", str(SCRIPTS),
             "-postScript", "ExportSymbols.java", str(out), a.label or f"{proj_dir}"]
    return rc.ghidra_headless(args, a.maxmem)


def decompile(addrs: list[int], maxmem: str = "3G", refresh: bool = False) -> int:
    cache = rc.cache_dir()
    cache.mkdir(parents=True, exist_ok=True)
    todo = []
    for addr in addrs:
        if refresh:
            for ext in (".c", ".at", ".miss"):
                (cache / f"{addr:08X}{ext}").unlink(missing_ok=True)
        if cached_path(addr) is None:
            todo.append(addr)
    if not todo:
        return 0
    proj = rc.master_project()
    if not (proj / f"{PROJECT_NAME}.gpr").exists():
        sys.exit(f"master project missing: {proj} (run tools/re/master.py build)")
    wait_unlocked(proj, PROJECT_NAME)
    return rc.ghidra_headless([str(proj), PROJECT_NAME, "-process", PROGRAM, "-noanalysis", "-readOnly",
                               "-scriptPath", str(SCRIPTS), "-postScript", "DecompileCache.java", str(cache)]
                              + [f"{x:08X}" for x in todo] + logs("decompile"), maxmem)


def cached_path(addr: int) -> Path | None:
    cache = rc.cache_dir()
    direct = cache / f"{addr:08X}.c"
    if direct.exists():
        return direct
    at = cache / f"{addr:08X}.at"
    if at.exists():
        entry = cache / f"{at.read_text().strip()}.c"
        if entry.exists():
            return entry
    if (cache / f"{addr:08X}.miss").exists():
        return cache / f"{addr:08X}.miss"
    return None


def cmd_decompile(a) -> int:
    addrs = [rc.parse_addr(x) for x in a.addr]
    rc_ = decompile(addrs, a.maxmem, a.refresh)
    for addr in addrs:
        p = cached_path(addr)
        print(f"{rc.fmt(addr)}\t{p if p else 'FAILED'}")
        if p and a.print and p.suffix == ".c":
            print(p.read_text(encoding="utf-8"))
    return rc_


def cmd_snapshot(a) -> int:
    src = rc.master_project()
    dst = Path(a.dest)
    if dst.exists():
        sys.exit(f"{dst} exists")
    wait_unlocked(src, PROJECT_NAME)
    shutil.copytree(src, dst, ignore=shutil.ignore_patterns("logs", "*.lock", "*.lock~"))
    print(f"copied to {dst}; open {dst / (PROJECT_NAME + '.gpr')}, then export names with:\n"
          f"  py -I tools/re/master.py export --project {dst / (PROJECT_NAME + '.gpr')} --out <tsv>")
    return 0


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--maxmem", default="4G")
    sub = p.add_subparsers(dest="cmd", required=True)
    b = sub.add_parser("build"); b.add_argument("--force", action="store_true")
    b.add_argument("--cpu", type=int, default=4); b.add_argument("--timeout", type=int, default=21600)
    b.add_argument("--apply", action="store_true", help="also run ImportSymbols after analysis")
    ap = sub.add_parser("apply"); ap.add_argument("--tsv")
    e = sub.add_parser("export"); e.add_argument("--project", help=".gpr path (default master)")
    e.add_argument("--program", help="program name; default every program in the project")
    e.add_argument("--out", required=True); e.add_argument("--label")
    d = sub.add_parser("decompile"); d.add_argument("addr", nargs="+")
    d.add_argument("--print", action="store_true"); d.add_argument("--refresh", action="store_true")
    sub.add_parser("functions")
    s = sub.add_parser("snapshot"); s.add_argument("dest")
    a = p.parse_args()
    return {"build": cmd_build, "apply": cmd_apply, "export": cmd_export,
            "decompile": cmd_decompile, "functions": cmd_functions, "snapshot": cmd_snapshot}[a.cmd](a)


if __name__ == "__main__":
    sys.exit(main())
