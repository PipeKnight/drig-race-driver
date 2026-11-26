"""Disk audit: which agent (Claude, Codex) or shared project data uses the disk, and what grew.

Read-only. Measures every checkout of the project (owner taken from its path: Claude worktrees,
Codex worktrees, the main checkout), each checkout's cargo build dir under C:/cargo-build, the
shared build folders (seed, prepush cache), the main checkout's local/ folders, E:/DRIG-local,
the Claude and Codex data folders, the Android and Rust toolchains and the temp folder.
Hard-linked files under local/ and target/ count once. Every run saves a snapshot to
local/disk-audit/ and prints the growth since the previous snapshot, plus the bytes written in
the last --recent hours per entry, so a sudden fill shows its source even on a first run.
The last line is the verdict.

  py -I tools/disk_audit.py                 # full report, about 1-3 minutes
  py -I tools/disk_audit.py --recent 6 --top 40
  py -I tools/disk_audit.py --no-save       # do not write a snapshot
"""
import argparse
import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
HOME = Path.home()
LOCALAPPDATA = Path(os.environ.get("LOCALAPPDATA", HOME / "AppData" / "Local"))
APPDATA = Path(os.environ.get("APPDATA", HOME / "AppData" / "Roaming"))
BUILD_ROOT = Path("C:/cargo-build")
EXTERNAL_LOCAL = Path("E:/DRIG-local")
GB = 1024 ** 3


def main_checkout():
    """The primary working tree; snapshots live in its local/ whichever checkout runs this."""
    r = subprocess.run(["git", "-C", str(REPO), "rev-parse", "--path-format=absolute", "--git-common-dir"],
                       capture_output=True, text=True)
    return Path(r.stdout.strip()).parent if r.returncode == 0 and r.stdout.strip() else REPO


MAIN = main_checkout()


def owner_of(path):
    p = str(path).replace("\\", "/").lower()
    if "/.codex/" in p or p.endswith("/.codex"):
        return "codex"
    if "/.claude/worktrees/" in p:
        return "claude"
    return "shared"


def measure(path, cutoff, seen=None):
    """Bytes, files and bytes written since cutoff under path; skips junctions and symlinks.
    With seen (a set), files with several hard links count once across all calls."""
    total = recent = files = 0
    newest = 0.0
    stack = [str(path)]
    while stack:
        d = stack.pop()
        try:
            it = os.scandir(d)
        except OSError:
            continue
        with it:
            for e in it:
                try:
                    if e.is_symlink() or e.is_junction():
                        continue
                    if e.is_dir(follow_symlinks=False):
                        stack.append(e.path)
                        continue
                    st = e.stat(follow_symlinks=False)
                    size = st.st_size
                    if seen is not None and size > 65536:
                        full = os.stat(e.path, follow_symlinks=False)
                        if full.st_nlink > 1:
                            key = (full.st_dev, full.st_ino)
                            if key in seen:
                                continue
                            seen.add(key)
                except OSError:
                    continue
                total += size
                files += 1
                if st.st_mtime >= cutoff:
                    recent += size
                if st.st_mtime > newest:
                    newest = st.st_mtime
    return total, recent, files, newest


def checkouts():
    out = subprocess.run(["git", "-C", str(REPO), "worktree", "list", "--porcelain"],
                         capture_output=True, text=True).stdout
    paths = [Path(line[9:]) for line in out.splitlines() if line.startswith("worktree ")]
    known = {str(p).lower() for p in paths}
    # Folders left behind by removed worktrees still use disk.
    for parent in (MAIN / ".claude" / "worktrees", HOME / ".codex" / "worktrees"):
        if parent.is_dir():
            for c in parent.iterdir():
                if c.is_dir() and str(c).lower() not in known and not any(
                        str(c).lower() in k or k.startswith(str(c).lower() + os.sep) for k in known):
                    paths.append(c)
    return [p for p in paths if p.is_dir()]


def build_dir_of(checkout):
    if not (checkout / "Cargo.toml").is_file():
        return None
    cargo = shutil.which("cargo") or str(HOME / ".cargo" / "bin" / "cargo.exe")
    try:
        r = subprocess.run([cargo, "metadata", "--format-version", "1", "--no-deps", "--offline"],
                           cwd=checkout, capture_output=True, text=True, timeout=60)
        b = json.loads(r.stdout).get("build_directory")
    except (OSError, ValueError, subprocess.TimeoutExpired):
        return None
    return Path(b).resolve() if b else None


def entries():
    """(label, owner, path, dedup_hardlinks, exclude_children)."""
    rows = []
    cos = checkouts()
    build_owner = {}
    main_co = next((c for c in cos if c.resolve() == MAIN.resolve()), None)
    for co in cos:
        own = owner_of(co)
        is_main = co is main_co
        name = "main checkout" if is_main else (co.parent.name if co.name == "Reassembly" else co.name)
        outer = [o for o in cos if o is not co and o is not main_co and str(co).lower().startswith(str(o).lower() + os.sep)]
        if outer:
            name = f"{outer[0].name}/.../{co.name}"
        # A checkout inside another worktree is already counted there (but its build dir is not).
        nested = any(o is not co and o is not main_co and str(co).lower().startswith(str(o).lower() + os.sep)
                     for o in cos)
        if nested:
            pass
        elif is_main:
            rows.append((f"{name}: target", own, co / "target", True, ()))
            local = co / "local"
            if local.is_dir():
                for c in local.iterdir():
                    if c.is_dir() and not c.is_junction() and not c.is_symlink():
                        rows.append((f"{name}: local/{c.name}", "shared", c, True, ()))
            rows.append((f"{name}: rest", own, co, False, ("target", "local", ".claude")))
        else:
            rows.append((f"{own} worktree: {name}", own, co, True, ()))
        b = build_dir_of(co)
        if b:
            build_owner[str(b).lower()] = (own, name)
    if BUILD_ROOT.is_dir():
        for top in BUILD_ROOT.iterdir():
            if not top.is_dir():
                continue
            if len(top.name) != 2:  # seed, prepush-cache, perf-android, build-lock
                rows.append((f"cargo-build: {top.name}", "shared", top, False, ()))
                continue
            for h in top.iterdir():
                if h.is_dir():
                    own, name = build_owner.get(str(h.resolve()).lower(), ("orphan", "no live checkout"))
                    rows.append((f"build dir {top.name}/{h.name[:8]} ({name})", own, h, False, ()))
    if EXTERNAL_LOCAL.is_dir():
        for c in EXTERNAL_LOCAL.iterdir():
            rows.append((f"E:/DRIG-local/{c.name}", "shared", c, True, ()))
    claude_home = HOME / ".claude"
    if claude_home.is_dir():
        for c in claude_home.iterdir():
            rows.append((f"~/.claude/{c.name}", "claude", c, False, ()))
    codex_home = HOME / ".codex"
    if codex_home.is_dir():
        for c in codex_home.iterdir():
            if c.name != "worktrees":
                rows.append((f"~/.codex/{c.name}", "codex", c, False, ()))
    for label, own, p in [
        ("Temp/claude", "claude", LOCALAPPDATA / "Temp" / "claude"),
        ("AppData/Local/Claude-Data", "claude", LOCALAPPDATA / "Claude-Data"),
        ("AppData/Local/Claude", "claude", LOCALAPPDATA / "Claude"),
        ("AppData/Local/claude-cli-nodejs", "claude", LOCALAPPDATA / "claude-cli-nodejs"),
        ("AppData/Roaming/Claude", "claude", APPDATA / "Claude"),
        ("AppData/Local/OpenAI", "codex", LOCALAPPDATA / "OpenAI"),
        ("AppData/Roaming/Codex", "codex", APPDATA / "Codex"),
        ("Temp (other)", "system", LOCALAPPDATA / "Temp"),
        ("Android SDK", "toolchain", LOCALAPPDATA / "Android"),
        ("~/.gradle", "toolchain", HOME / ".gradle"),
        ("~/.android", "toolchain", HOME / ".android"),
        ("~/.cargo", "toolchain", HOME / ".cargo"),
        ("~/.rustup", "toolchain", HOME / ".rustup"),
    ]:
        if p.is_dir():
            rows.append((label, own, p, False, ("claude",) if label == "Temp (other)" else ()))
    return rows


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--recent", type=float, default=24.0, help="hours for the 'written recently' column")
    ap.add_argument("--top", type=int, default=25)
    ap.add_argument("--min-gb", type=float, default=0.5, help="hide entries smaller than this")
    ap.add_argument("--no-save", action="store_true")
    args = ap.parse_args()

    t0 = time.time()
    cutoff = t0 - args.recent * 3600
    seen = set()
    results = []
    for label, own, path, dedup, exclude in entries():
        if exclude:
            total = recent = files = 0
            newest = 0.0
            with os.scandir(path) as it:
                for e in it:
                    if e.name in exclude or e.is_symlink() or e.is_junction():
                        continue
                    if e.is_dir(follow_symlinks=False):
                        t, r, f, n = measure(e.path, cutoff, seen if dedup else None)
                    else:
                        try:
                            st = e.stat(follow_symlinks=False)
                        except OSError:
                            continue
                        t, r, f, n = st.st_size, st.st_size if st.st_mtime >= cutoff else 0, 1, st.st_mtime
                    total += t; recent += r; files += f; newest = max(newest, n)
        else:
            total, recent, files, newest = measure(path, cutoff, seen if dedup else None)
        results.append({"label": label, "owner": own, "path": str(path), "bytes": total,
                        "recent": recent, "files": files, "newest": newest})

    snap_dir = MAIN / "local" / "disk-audit"
    prev = None
    if snap_dir.is_dir():
        snaps = sorted(snap_dir.glob("*.json"))
        if snaps:
            try:
                prev = json.loads(snaps[-1].read_text(encoding="utf-8"))
            except (OSError, ValueError):
                prev = None
    prev_bytes = {r["path"].lower(): r["bytes"] for r in prev["entries"]} if prev else {}
    prev_label = {r["path"].lower(): r["label"] for r in prev["entries"]} if prev else {}

    drives = {}
    for d in ("C:/", "E:/"):
        if os.path.exists(d):
            u = shutil.disk_usage(d)
            drives[d[0]] = {"total": u.total, "free": u.free}

    print("Drives: " + "; ".join(f"{k}: {v['free'] / GB:.1f} GB free of {v['total'] / GB:.0f} GB"
                                 for k, v in drives.items()))
    if prev:
        for k, v in drives.items():
            pv = prev.get("drives", {}).get(k)
            if pv:
                print(f"  {k}: free changed {(v['free'] - pv['free']) / GB:+.1f} GB since {prev['time']}")
    by_owner = {}
    for r in results:
        o = by_owner.setdefault(r["owner"], [0, 0])
        o[0] += r["bytes"]; o[1] += r["recent"]
    print(f"\nBy owner (size / written in the last {args.recent:g} h):")
    for own, (b, rc) in sorted(by_owner.items(), key=lambda kv: -kv[1][0]):
        print(f"  {own:9} {b / GB:8.1f} GB  {rc / GB:7.1f} GB")
    shown = [r for r in results if r["bytes"] >= args.min_gb * GB]
    shown.sort(key=lambda r: -r["bytes"])
    hdr = f"{'size GB':>8} {'recent':>7} {'delta':>7}  owner     entry"
    print(f"\nLargest entries ({len(shown)} of {len(results)} over {args.min_gb:g} GB):\n{hdr}")
    for r in shown[:args.top]:
        key = r["path"].lower()
        delta = (r["bytes"] - prev_bytes[key]) / GB if key in prev_bytes else None
        ds = f"{delta:+7.1f}" if delta is not None else "    new" if prev else "      -"
        print(f"{r['bytes'] / GB:8.1f} {r['recent'] / GB:7.1f} {ds}  {r['owner']:9} {r['label']}")
    grew = sorted(((r["bytes"] - prev_bytes.get(r["path"].lower(), 0), r) for r in results
                   if prev and (r["bytes"] - prev_bytes.get(r["path"].lower(), 0)) > 0.2 * GB), key=lambda x: -x[0])
    if grew:
        print(f"\nGrew since {prev['time']}:")
        for d, r in grew[:10]:
            print(f"  {d / GB:+7.1f} GB  {r['owner']:9} {r['label']}")
    if prev:
        live = {r["path"].lower() for r in results}
        gone = [prev_label[k] for k in prev_bytes if k not in live and prev_bytes[k] > 0.5 * GB]
        if gone:
            print("\nRemoved since last run: " + ", ".join(gone[:10]))

    stamp = time.strftime("%Y%m%d-%H%M%S")
    if not args.no_save:
        snap_dir.mkdir(parents=True, exist_ok=True)
        (snap_dir / f"{stamp}.json").write_text(json.dumps(
            {"time": time.strftime("%Y-%m-%d %H:%M"), "drives": drives, "entries": results}, indent=1),
            encoding="utf-8")
    top_recent = max(results, key=lambda r: r["recent"])
    owners_recent = ", ".join(f"{o} {v[1] / GB:.1f}" for o, v in sorted(by_owner.items(), key=lambda kv: -kv[1][1]) if v[1] >= 0.05 * GB)
    c = drives.get("C")
    print(f"\ndisk_audit: C: {c['free'] / GB:.1f} GB free; written in {args.recent:g} h (GB): {owners_recent or 'none'}; "
          f"top writer: {top_recent['label']} ({top_recent['owner']}) {top_recent['recent'] / GB:.1f} GB; "
          f"{time.time() - t0:.0f} s" + ("" if args.no_save else f"; snapshot local/disk-audit/{stamp}.json"))
    return 0


if __name__ == "__main__":
    sys.exit(main())
