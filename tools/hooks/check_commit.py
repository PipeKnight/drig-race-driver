"""Git hook checks for every commit (pre-commit and commit-msg; docs/TOOLCHAIN.md "Git hooks").

Installed once per clone with `py -I tools/hooks/install.py` (sets core.hooksPath to .githooks,
whose shell scripts call this file). Python standard library only; checks the staged content, not
the working tree, and takes well under a second for a normal commit.

    check_commit.py pre-commit            # staged files
    check_commit.py commit-msg MSG_FILE   # the commit message

pre-commit fails on:
  - leftover conflict markers in staged lines (git diff --cached --check);
  - CR in a text file under third_party/ (vendored crates keep upstream LF bytes, F877);
  - files under local/, original game files by type (.pssg, .raw, .bik, ...), binaries
    (.exe, .dll, .pdb, .apk, ...) and any staged file over 5 MiB: proprietary assets and machine
    evidence stay in ignored local/ (AGENTS.md "Original reference and provenance");
  - a staged .py file that does not parse, or a staged .ps1 file that does not parse (Windows);
  - a relative Markdown link on an added line that points at no file (links into local/ skipped);
  - tools/log_entry.py check --no-fetch, when the commit touches log/ or PROJECT_LOG.md.
commit-msg fails on AI attribution trailers (Co-Authored-By an AI, "Generated with ...").

The checks of tools/prepush.ps1 (tests, Clippy, image smoke) still run before a push.
"""

import ast
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MAX_BYTES = 5 * 1024 * 1024

# Original game file types (Steam install, counted 2026-11-20) and binaries that never belong in
# the repository. .xml, .ini, .csv and .bin are not listed: the repository has its own.
GAME_TYPES = {
    "pssg", "raw", "jpk", "wip", "wim", "wia", "wio", "cqtc", "vis", "ens", "clm", "fpo", "ned",
    "ppts", "lng", "grs", "bik", "nfs", "clo", "vpo", "sfk", "per",
}
BINARY_TYPES = {"exe", "dll", "pdb", "lib", "obj", "so", "dylib", "apk", "aab", "rdc", "dmp", "gpr", "rep"}

LINK_RE = re.compile(r'\]\(\s*<?([^)\s>]+)>?(?:\s+"[^"]*")?\s*\)')
REF_RE = re.compile(r"^\s*\[[^\]]+\]:\s*(\S+)")
AI_TRAILER_RE = re.compile(
    r"^\s*(co-authored-by:.*(claude|anthropic|openai|codex|copilot|chatgpt|gpt-|gemini|cursor)"
    r"|\W*generated (with|by) .*(claude|codex|copilot|chatgpt))",
    re.IGNORECASE,
)


def git(*args, data=None):
    r = subprocess.run(["git", *args], cwd=ROOT, input=data, capture_output=True)
    return r.returncode, r.stdout


def staged():
    """(path, status) of staged added, copied, modified or renamed files."""
    _, out = git("diff", "--cached", "--name-status", "-z", "--diff-filter=ACMR", "--no-renames")
    parts = out.decode("utf-8", "replace").split("\0")
    return [(parts[i + 1], parts[i]) for i in range(0, len(parts) - 1, 2)]


def blobs(paths):
    """Staged bytes of each path, read in one git cat-file call."""
    if not paths:
        return {}
    req = "".join(":%s\n" % p for p in paths).encode("utf-8")
    _, out = git("cat-file", "--batch", data=req)
    result, pos = {}, 0
    for p in paths:
        nl = out.index(b"\n", pos)
        header = out[pos:nl].split()
        pos = nl + 1
        if len(header) < 3 or header[1] != b"blob":
            result[p] = b""
            continue
        size = int(header[2])
        result[p] = out[pos:pos + size]
        pos += size + 1
    return result


def added_lines(path):
    _, out = git("diff", "--cached", "-U0", "--no-color", "--", path)
    for line in out.decode("utf-8", "replace").splitlines():
        if line.startswith("+") and not line.startswith("+++"):
            yield line[1:]


def check_pre_commit():
    problems, notes = [], []
    files = staged()
    paths = [p for p, _ in files]

    # Conflict markers (whitespace is not a failure here or in prepush).
    _, out = git("-c", "core.whitespace=cr-at-eol", "diff", "--cached", "--check")
    for line in out.decode("utf-8", "replace").splitlines():
        if line.endswith("leftover conflict marker"):
            problems.append("conflict marker: " + line)

    # Size and type: proprietary or machine-private files.
    _, out = git("cat-file", "--batch-check", data="".join(":%s\n" % p for p in paths).encode("utf-8")) if paths else (0, b"")
    sizes = {}
    for p, line in zip(paths, out.decode("utf-8", "replace").splitlines()):
        f = line.split()
        sizes[p] = int(f[2]) if len(f) >= 3 and f[1] == "blob" else 0
    for p in paths:
        ext = p.rsplit(".", 1)[-1].lower() if "." in p.rsplit("/", 1)[-1] else ""
        if p == "local" or p.startswith("local/"):
            problems.append("%s: local/ is private and ignored; it must not be committed" % p)
        elif ext in GAME_TYPES:
            problems.append("%s: original game file type .%s; game assets stay under local/" % (p, ext))
        elif ext in BINARY_TYPES:
            problems.append("%s: binary file type .%s; builds, dumps and tools stay under local/ or target/" % (p, ext))
        elif sizes.get(p, 0) > MAX_BYTES:
            problems.append("%s: %.1f MiB staged (limit %d MiB); large files stay under local/" % (p, sizes[p] / 1048576, MAX_BYTES // 1048576))

    # Content checks on small text files.
    wanted = [p for p in paths if sizes.get(p, 0) <= MAX_BYTES and (
        p.startswith("third_party/") or p.endswith((".py", ".ps1")))]
    content = blobs(wanted)
    for p in wanted:
        data = content.get(p, b"")
        if p.startswith("third_party/") and b"\r" in data and b"\0" not in data:
            problems.append("%s: CR line endings in a vendored file; re-check out third_party (F877)" % p)
        if p.endswith(".py"):
            try:
                ast.parse(data, p)
            except SyntaxError as e:
                problems.append("%s:%s: Python does not parse: %s" % (p, e.lineno, e.msg))
    ps1 = [p for p in wanted if p.endswith(".ps1")]
    if ps1:
        problems += check_powershell(ps1, content, notes)

    # Relative Markdown links on added lines (the working-tree file must exist).
    for p in paths:
        if not p.endswith(".md"):
            continue
        base = (ROOT / p).parent
        for line in added_lines(p):
            targets = LINK_RE.findall(line)
            m = REF_RE.match(line)
            if m:
                targets.append(m.group(1))
            for t in targets:
                if re.match(r"^[a-zA-Z][a-zA-Z0-9+.-]*:", t) or t.startswith("#"):
                    continue
                t = re.sub(r":\d+(-\d+)?$", "", re.sub(r"%([0-9A-Fa-f]{2})", lambda m: chr(int(m.group(1), 16)), t.split("#")[0]))
                if not t or re.search(r"(^|/)local/", t):
                    continue
                target = ROOT / t.lstrip("/") if t.startswith("/") else base / t
                if not target.exists():
                    problems.append("%s: broken link (%s)" % (p, t))

    # Log entries and indexes.
    if any(p.startswith("log/") or p == "PROJECT_LOG.md" for p in paths):
        r = subprocess.run([sys.executable, "-I", str(ROOT / "tools" / "log_entry.py"), "check", "--no-fetch"],
                           cwd=ROOT, capture_output=True, text=True)
        if r.returncode != 0:
            tail = (r.stdout + r.stderr).strip().splitlines()[-5:]
            problems.append("tools/log_entry.py check failed (fix: py -I tools/log_entry.py check --fix): " + " | ".join(tail))
    return problems, notes


def check_powershell(paths, content, notes):
    if os.name != "nt":
        notes.append("PowerShell parse skipped (not Windows): " + ", ".join(paths))
        return []
    problems = []
    with tempfile.TemporaryDirectory() as tmp:
        names = []
        for i, p in enumerate(paths):
            f = Path(tmp) / ("%d.ps1" % i)
            f.write_bytes(content.get(p, b""))
            names.append(str(f))
        listed = ",".join("'%s'" % n.replace("'", "''") for n in names)
        script = (
            "$i=0; foreach ($f in @(" + listed + ")) { $t=$null; $e=$null; "
            "[void][System.Management.Automation.Language.Parser]::ParseFile($f,[ref]$t,[ref]$e); "
            "foreach ($x in @($e)) { '{0}:{1}: {2}' -f $i,$x.Extent.StartLineNumber,$x.Message }; $i++ }"
        )
        try:
            r = subprocess.run(["powershell", "-NoProfile", "-NonInteractive", "-Command", script],
                               capture_output=True, text=True, timeout=60)
        except (OSError, subprocess.TimeoutExpired) as e:
            notes.append("PowerShell parse skipped: %s" % e)
            return []
        for line in r.stdout.splitlines():
            m = re.match(r"^(\d+):(\d+): (.*)$", line)
            if m:
                problems.append("%s:%s: PowerShell does not parse: %s" % (paths[int(m.group(1))], m.group(2), m.group(3)))
    return problems


def check_commit_msg(msg_file):
    text = Path(msg_file).read_text(encoding="utf-8", errors="replace")
    problems = []
    for line in text.splitlines():
        if line.startswith("#"):
            continue
        if AI_TRAILER_RE.search(line):
            problems.append("AI attribution in the commit message: %r (AGENTS.md: no AI trailers)" % line.strip())
    return problems, []


def main():
    if len(sys.argv) < 2 or sys.argv[1] not in ("pre-commit", "commit-msg"):
        print(__doc__)
        return 2
    if sys.argv[1] == "pre-commit":
        problems, notes = check_pre_commit()
    else:
        if len(sys.argv) < 3:
            print("commit-msg needs the message file")
            return 2
        problems, notes = check_commit_msg(sys.argv[2])
    for n in notes:
        print("%s hook note: %s" % (sys.argv[1], n))
    if problems:
        print("%s hook: commit refused (%d problem%s):" % (sys.argv[1], len(problems), "" if len(problems) == 1 else "s"))
        for p in problems:
            print("  - " + p)
        print("Fix them and commit again (docs/TOOLCHAIN.md \"Git hooks\").")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
