"""Disk audit: which agent (Claude, Codex), project or shared data uses the disk, and what grew.

Read-only. Finds every git repository on this PC (a shallow scan of C:/, E:/, the home, Documents
and OneDrive folders, plus every folder the agents list as a project: ~/.codex/config.toml, the
projects in ~/.claude.json and the working folder of every Codex session) and measures each
repository's checkouts (owner taken from the path: Claude worktrees, Codex worktrees, main
checkouts), each checkout's cargo build dir under C:/cargo-build, agent project folders outside
any repository, this project's shared build folders (seed, prepush cache), local/ folders and
E:/DRIG-local, the Claude and Codex data folders (Codex sessions and Claude transcripts split by
project), the Android and Rust toolchains and the temp folder. Totals by owner and by project.
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
import re
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


try:
    from compression import zstd  # Python 3.14+; Codex stores older sessions as .jsonl.zst
except ImportError:
    zstd = None

SKIP_SCAN = {"windows", "program files", "program files (x86)", "programdata", "appdata",
             "node_modules", "target", "system volume information", "windowsapps", "cargo-build", "wpsystem"}


def norm(p):
    return os.path.normcase(os.path.abspath(str(p)))


def inside(p, parent):
    p, parent = norm(p), norm(parent)
    return p == parent or p.startswith(parent.rstrip(os.sep) + os.sep)


def short(p):
    s = str(p)
    if norm(s).startswith(norm(HOME) + os.sep):
        return "~" + s[len(str(HOME)):].replace("\\", "/")
    return s.replace("\\", "/")


def git(*args):
    try:
        r = subprocess.run(["git", *args], capture_output=True, text=True, encoding="utf-8",
                           errors="replace", timeout=60)
    except (OSError, subprocess.TimeoutExpired):
        return ""
    return r.stdout if r.returncode == 0 else ""


def first_line(path):
    with open(path, "rb") as h:
        if not path.endswith(".zst"):
            return h.readline(4 << 20)
        if zstd is None:
            raise ValueError("no zstd module")
        d, buf = zstd.ZstdDecompressor(), b""
        while b"\n" not in buf and len(buf) < (4 << 20):
            chunk = h.read(65536)
            if not chunk:
                break
            buf += d.decompress(chunk, max_length=1 << 20)
        return buf.split(b"\n", 1)[0]


def codex_sessions():
    """(folder, size, mtime, cwd) for every Codex session file; cwd from its first record."""
    out = []
    for folder in ("sessions", "archived_sessions"):
        root = HOME / ".codex" / folder
        for dirpath, _, names in os.walk(root):
            for n in names:
                f = os.path.join(dirpath, n)
                try:
                    st = os.stat(f)
                except OSError:
                    continue
                try:
                    cwd = json.loads(first_line(f))["payload"]["cwd"]
                except Exception:  # unreadable, truncated or another record type
                    cwd = None
                out.append((folder, st.st_size, st.st_mtime, cwd))
    return out


def agent_folders(sessions):
    """{norm path: (Path, set of agents)} for every folder the agents list as a project."""
    found = {}

    def add(p, agent):
        if p:
            real = os.path.realpath(p) if os.path.exists(p) else p  # the case on disk, not the config's
            found.setdefault(norm(p), (Path(real), set()))[1].add(agent)
    cfg = HOME / ".codex" / "config.toml"
    if cfg.is_file():
        for m in re.finditer(r"""^\[projects\.(?:'([^']+)'|"([^"]+)")\]""",
                             cfg.read_text(encoding="utf-8", errors="replace"), re.M):
            add(m.group(1) or m.group(2), "codex")
    try:
        for p in json.loads((HOME / ".claude.json").read_text(encoding="utf-8")).get("projects", {}):
            add(p, "claude")
    except (OSError, ValueError, AttributeError):
        pass
    for s in sessions:
        add(s[3], "codex")
    return found


def find_repos(folders):
    """Every git repository found: [{name, this, common, main, worktrees}]; worktrees may be gone."""
    tops = []
    roots = [(Path("C:/"), 2), (Path("E:/"), 2), (HOME, 2), (HOME / "Documents", 3), (HOME / "OneDrive", 3),
             (HOME / "source" / "repos", 2)]
    roots += [(p, 2) for p, _ in folders.values()]
    done = {}
    for root, depth in roots:
        stack = [(root, depth)]
        while stack:
            d, dp = stack.pop()
            if done.get(norm(d), -1) >= dp or not d.is_dir():
                continue
            done[norm(d)] = dp
            if (d / ".git").exists():
                tops.append(d)
                continue
            if dp == 0:
                continue
            try:
                with os.scandir(d) as it:
                    kids = [Path(e.path) for e in it if e.is_dir(follow_symlinks=False) and not e.is_junction()
                            and not e.name.startswith((".", "$")) and e.name.lower() not in SKIP_SCAN]
            except OSError:
                continue
            stack += [(k, dp - 1) for k in kids]
    # A folder the agents list may sit inside a repository (a subfolder or a worktree).
    tops += [p for p, _ in folders.values() if p.is_dir()]
    repos = {}
    for t in tops:
        common = git("-C", str(t), "rev-parse", "--path-format=absolute", "--git-common-dir").strip()
        if not common or norm(common) in repos:
            continue
        wts = [Path(line[9:]) for line in git("-C", str(t), "worktree", "list", "--porcelain").splitlines()
               if line.startswith("worktree ")]
        if wts:
            repos[norm(common)] = {"common": common, "main": wts[0], "worktrees": wts}
    out = list(repos.values())
    this_common = norm(git("-C", str(REPO), "rev-parse", "--path-format=absolute", "--git-common-dir").strip())
    names = [r["main"].name for r in out]
    for r in out:
        r["this"] = norm(r["common"]) == this_common
        r["name"] = r["main"].name if r["this"] or names.count(r["main"].name) == 1 else \
            f"{r['main'].name} ({short(r['main'].parent)})"
    out.sort(key=lambda r: (not r["this"], r["name"].lower()))
    return out


def project_resolver(repos, folders):
    """path -> project: the repository it belongs to, else the agent folder, else the path itself."""
    prefixes = [(str(w), r["name"]) for r in repos for w in r["worktrees"]]
    for p, _ in folders.values():
        if not any(inside(p, w) for w, _ in prefixes):
            if inside(p, HOME / "Documents" / "Codex"):  # the Codex app's per-thread scratch folders
                name = "~/Documents/Codex (Codex scratch folders)"
            else:
                name = short(p) + ("" if p.exists() else " (gone)")
            prefixes.append((str(p), name))
    prefixes.sort(key=lambda x: -len(x[0]))

    def resolve(p):
        for w, name in prefixes:
            if inside(p, w):
                return name
        return short(p) + ("" if Path(p).exists() else " (gone)")
    return resolve, prefixes


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


def entries(cutoff):
    """Rows: dict(label, owner, project, path, dedup, exclude, values); values are precomputed sizes."""
    rows = []

    def add(label, owner, project, path, dedup=False, exclude=(), values=None):
        rows.append({"label": label, "owner": owner, "project": project, "path": str(path),
                     "dedup": dedup, "exclude": tuple(exclude), "values": values})
    sessions = codex_sessions()
    folders = agent_folders(sessions)
    repos = find_repos(folders)
    resolve, prefixes = project_resolver(repos, folders)
    this_name = next((r["name"] for r in repos if r["this"]), MAIN.name)
    mains = [r["main"] for r in repos]

    live = [(co, r) for r in repos for co in r["worktrees"] if co.is_dir()]
    known = [co for co, _ in live]
    # Folders left behind by removed worktrees still use disk.
    for parent in [m / ".claude" / "worktrees" for m in mains] + [HOME / ".codex" / "worktrees"]:
        if parent.is_dir():
            for c in parent.iterdir():
                if c.is_dir() and not any(inside(k, c) or (inside(c, k) and not inside(parent, k)) for k in known):
                    live.append((c, None))
    codex_wt = HOME / ".codex" / "worktrees"

    def disp(co):  # Codex keeps worktrees as ~/.codex/worktrees/<name>/<repository>
        return co.parent.name if norm(co.parent.parent) == norm(codex_wt) else co.name
    build_owner = {}
    for co, r in live:
        own = owner_of(co)
        if own == "shared" and r is not None and not r["this"]:
            # Another repository's checkout belongs to the agent that works there, if only one does.
            agents = set().union(*[a for p, a in folders.values() if inside(p, r["main"]) or inside(r["main"], p)])
            own = next(iter(agents)) if len(agents) == 1 else "shared"
        is_main = r is not None and co == r["main"]
        project = r["name"] if r else resolve(co)
        rname = "" if (r is None or r["this"]) else r["name"] + "/"
        name = "main checkout" if is_main else disp(co)
        if r is None:
            name += " (unregistered)"
        # A checkout inside another worktree is already counted there (but its build dir is not).
        outer = [o for o, _ in live if o != co and o not in mains and inside(co, o)]
        if outer:
            name = f"{disp(outer[0])}/.../{co.name}"
        elif is_main:
            prefix = "main checkout" if r["this"] else r["name"]
            excl = ["target", ".claude"]
            if (co / "target").is_dir():
                add(f"{prefix}: target", own, project, co / "target", True)
            if r["this"]:
                local = co / "local"
                if local.is_dir():
                    for c in local.iterdir():
                        if c.is_dir() and not c.is_junction() and not c.is_symlink():
                            add(f"{prefix}: local/{c.name}", "shared", project, c, True)
                excl.append("local")
            add(f"{prefix}: rest" if r["this"] else f"{prefix}: checkout", own, project, co, False, excl)
        else:
            add(f"{own} worktree: {rname}{name}", own, project, co, True)
        b = build_dir_of(co)
        if b:
            build_owner[norm(b)] = (own, rname + name, project)

    # Agent project folders outside every repository.
    covered = [co for co, _ in live]
    extra = sorted((p for p, _ in folders.values() if p.is_dir() and not inside(HOME, p)
                    and not any(inside(p, c) for c in covered)), key=lambda p: len(str(p)))
    outer_folders = []
    for p in extra:
        if not any(inside(p, o) for o in outer_folders):
            outer_folders.append(p)
    for p in outer_folders:
        agents = folders[norm(p)][1]
        own = next(iter(agents)) if len(agents) == 1 else "shared"
        inner = [c for c in covered if inside(c, p)]
        if any(c.parent != p for c in inner):
            continue  # a repository deeper inside: measuring the folder would count it twice
        add(f"{own} folder: {short(p)}", own, resolve(p), p, False, [c.name for c in inner])

    if BUILD_ROOT.is_dir():
        for top in BUILD_ROOT.iterdir():
            if not top.is_dir():
                continue
            if len(top.name) != 2:  # seed, prepush-cache, perf-android, build-lock
                add(f"cargo-build: {top.name}", "shared", this_name, top)
                continue
            for h in top.iterdir():
                if h.is_dir():
                    own, name, project = build_owner.get(norm(h.resolve()),
                                                         ("orphan", "no live checkout", "(orphan build dirs)"))
                    add(f"build dir {top.name}/{h.name[:8]} ({name})", own, project, h)
    if EXTERNAL_LOCAL.is_dir():
        for c in EXTERNAL_LOCAL.iterdir():
            add(f"E:/DRIG-local/{c.name}", "shared", this_name, c, True)

    claude_home = HOME / ".claude"
    if claude_home.is_dir():
        for c in claude_home.iterdir():
            if c.name != "projects":
                add(f"~/.claude/{c.name}", "claude", "(Claude app)", c)
        # Transcripts: one folder per working folder, named by its path with every other character as '-'.
        enc = sorted(((re.sub(r"[^A-Za-z0-9]", "-", w), name) for w, name in prefixes), key=lambda x: -len(x[0]))
        pdir = claude_home / "projects"
        groups = {}
        if pdir.is_dir():
            for d in pdir.iterdir():
                project = next((name for e, name in enc if d.name == e or d.name.startswith(e + "-")),
                               "(other Claude folders)")
                g = groups.setdefault(project, [0, 0, 0, 0.0])
                t, rc, f, n = measure(d, cutoff)
                g[0] += t; g[1] += rc; g[2] += f; g[3] = max(g[3], n)
        for project, v in groups.items():
            add(f"~/.claude/projects: {project}", "claude", project, f"{pdir}#{project}", values=tuple(v))
    codex_home = HOME / ".codex"
    if codex_home.is_dir():
        for c in codex_home.iterdir():
            if c.name not in ("worktrees", "sessions", "archived_sessions"):
                add(f"~/.codex/{c.name}", "codex", "(Codex app)", c)
        groups = {}
        for folder, size, mtime, cwd in sessions:
            project = resolve(cwd) if cwd else "(unattributed sessions)"
            g = groups.setdefault((folder, project), [0, 0, 0, 0.0])
            g[0] += size; g[1] += size if mtime >= cutoff else 0; g[2] += 1; g[3] = max(g[3], mtime)
        for (folder, project), v in groups.items():
            add(f"~/.codex/{folder}: {project}", "codex", project, f"{codex_home / folder}#{project}",
                values=tuple(v))
    for label, own, project, p in [
        ("Temp/claude", "claude", "(Claude app)", LOCALAPPDATA / "Temp" / "claude"),
        ("AppData/Local/Claude-Data", "claude", "(Claude app)", LOCALAPPDATA / "Claude-Data"),
        ("AppData/Local/Claude", "claude", "(Claude app)", LOCALAPPDATA / "Claude"),
        ("AppData/Local/claude-cli-nodejs", "claude", "(Claude app)", LOCALAPPDATA / "claude-cli-nodejs"),
        ("AppData/Roaming/Claude", "claude", "(Claude app)", APPDATA / "Claude"),
        ("AppData/Local/OpenAI", "codex", "(Codex app)", LOCALAPPDATA / "OpenAI"),
        ("AppData/Roaming/Codex", "codex", "(Codex app)", APPDATA / "Codex"),
        ("Temp (other)", "system", "(system)", LOCALAPPDATA / "Temp"),
        ("Android SDK", "toolchain", "(toolchains)", LOCALAPPDATA / "Android"),
        ("~/.gradle", "toolchain", "(toolchains)", HOME / ".gradle"),
        ("~/.android", "toolchain", "(toolchains)", HOME / ".android"),
        ("~/.cargo", "toolchain", "(toolchains)", HOME / ".cargo"),
        ("~/.rustup", "toolchain", "(toolchains)", HOME / ".rustup"),
    ]:
        if p.is_dir():
            add(label, own, project, p, False, ("claude",) if label == "Temp (other)" else ())
    return rows, repos


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
    rows, repos = entries(cutoff)
    for row in rows:
        label, own, path, dedup, exclude = row["label"], row["owner"], row["path"], row["dedup"], row["exclude"]
        if row["values"]:
            total, recent, files, newest = row["values"]
        elif exclude:
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
        results.append({"label": label, "owner": own, "project": row["project"], "path": str(path), "bytes": total,
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
    by_project = {}
    for r in results:
        o = by_project.setdefault(r["project"], [0, 0])
        o[0] += r["bytes"]; o[1] += r["recent"]
    print(f"\nBy project ({len(repos)} git repositories found; size / written in the last {args.recent:g} h):")
    for p, (b, rc) in sorted(by_project.items(), key=lambda kv: -kv[1][0]):
        if b >= 0.05 * GB:
            print(f"  {b / GB:8.1f} GB  {rc / GB:7.1f} GB  {p}")
    print("Repositories: " + "; ".join(
        f"{r['name']} ({short(r['main'])}, {sum(1 for w in r['worktrees'] if w.is_dir())} checkouts)" for r in repos))
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
            {"time": time.strftime("%Y-%m-%d %H:%M"), "drives": drives,
             "repos": [{"name": r["name"], "main": str(r["main"]), "worktrees": [str(w) for w in r["worktrees"]]}
                       for r in repos], "entries": results}, indent=1),
            encoding="utf-8")
    top_recent = max(results, key=lambda r: r["recent"])
    top_project = max(by_project.items(), key=lambda kv: kv[1][0])
    owners_recent = ", ".join(f"{o} {v[1] / GB:.1f}" for o, v in sorted(by_owner.items(), key=lambda kv: -kv[1][1]) if v[1] >= 0.05 * GB)
    c = drives.get("C")
    print(f"\ndisk_audit: C: {c['free'] / GB:.1f} GB free; written in {args.recent:g} h (GB): {owners_recent or 'none'}; "
          f"top writer: {top_recent['label']} ({top_recent['owner']}) {top_recent['recent'] / GB:.1f} GB; "
          f"largest project: {top_project[0]} {top_project[1][0] / GB:.1f} GB; "
          f"{time.time() - t0:.0f} s" + ("" if args.no_save else f"; snapshot local/disk-audit/{stamp}.json"))
    return 0


if __name__ == "__main__":
    sys.exit(main())
