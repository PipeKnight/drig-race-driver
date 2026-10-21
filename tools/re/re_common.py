"""Shared paths and helpers for the tools/re reverse-engineering scripts.

All machine-private data (the unpacked executable, the master Ghidra project and the
decompile cache) lives in the MAIN checkout's ignored local/ folder, so every worktree
shares one copy. The main checkout is found through `git rev-parse --git-common-dir`.
"""
from __future__ import annotations

import os
import struct
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
EXE_SHA256 = "aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514"
IMAGE_BASE = 0x400000
GHIDRA_HOME = Path(r"C:\Users\debros\Tools\grid-research\ghidra_12.1.4_PUBLIC")
JDK = Path(r"C:\Program Files (x86)\Steam\steamapps\common\Songs of Syx Demo\jre")
SYMBOLS = REPO / "docs" / "re" / "symbols.tsv"
COLUMNS = ["address", "name", "kind", "port", "doc", "entry", "confidence", "conflicts", "comment"]


def main_root() -> Path:
    """The main checkout (owner of the shared ignored local/), even from a worktree."""
    env = os.environ.get("GRID_MAIN_ROOT")
    if env:
        return Path(env)
    try:
        common = subprocess.run(["git", "rev-parse", "--path-format=absolute", "--git-common-dir"],
                                cwd=REPO, capture_output=True, text=True, check=True).stdout.strip()
        return Path(common).parent
    except (OSError, subprocess.CalledProcessError):
        return REPO


def local() -> Path:
    return main_root() / "local"


def exe_path() -> Path:
    return Path(os.environ.get("GRID_UNPACKED_EXE", local() / "compatibility" / "grid-unpacked.exe"))


def master_project() -> Path:
    return local() / "ghidra-master"


def cache_dir() -> Path:
    return local() / "re-cache"


def import_capstone():
    """Capstone 5.0.6 is installed only under local/tools/python (docs/TOOLCHAIN.md)."""
    sys.path.insert(0, str(local() / "tools" / "python"))
    import capstone  # noqa: E402
    return capstone


def parse_addr(text: str) -> int:
    text = text.strip().lower().replace("_", "")
    if text.startswith("0x"):
        text = text[2:]
    value = int(text, 16)
    if value < IMAGE_BASE:  # accept RVAs and file-relative offsets written without the base
        value += IMAGE_BASE
    return value


def fmt(addr: int) -> str:
    return f"0x{addr:08X}"


def read_symbols(path: Path = SYMBOLS) -> list[dict]:
    """Plain tab-separated text: no quoting, so a cell never contains a tab or newline."""
    if not path.exists():
        return []
    lines = path.read_text(encoding="utf-8").splitlines()
    head = lines[0].split("\t")
    return [dict(zip(head, line.split("\t"))) for line in lines[1:] if line.strip()]


def write_symbols(rows: list[dict], path: Path = SYMBOLS) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    rows = sorted(rows, key=lambda r: int(r["address"], 16))
    with path.open("w", newline="\n", encoding="utf-8") as fh:
        fh.write("\t".join(COLUMNS) + "\n")
        for r in rows:
            fh.write("\t".join(clean(r.get(k, "")) for k in COLUMNS) + "\n")


def clean(value: str) -> str:
    return " ".join(str(value or "").replace("\t", " ").split())


class PE:
    """Minimal PE32 section map for the unpacked executable."""

    def __init__(self, path: Path | None = None):
        self.path = path or exe_path()
        self.data = self.path.read_bytes()
        e_lfanew = struct.unpack_from("<I", self.data, 0x3C)[0]
        nsec = struct.unpack_from("<H", self.data, e_lfanew + 6)[0]
        opt = struct.unpack_from("<H", self.data, e_lfanew + 20)[0]
        self.base = struct.unpack_from("<I", self.data, e_lfanew + 24 + 28)[0]
        self.sections = []
        off = e_lfanew + 24 + opt
        for i in range(nsec):
            name, vsize, va, rsize, raw = struct.unpack_from("<8sIIII", self.data, off + i * 40)
            self.sections.append((name.rstrip(b"\0").decode("latin1"), self.base + va,
                                  max(vsize, rsize), raw, rsize))

    def section(self, addr: int):
        for s in self.sections:
            if s[1] <= addr < s[1] + s[2]:
                return s
        return None

    def read(self, addr: int, size: int) -> bytes:
        s = self.section(addr)
        if s is None:
            raise ValueError(f"{fmt(addr)} is outside every section")
        delta = addr - s[1]
        avail = max(0, min(size, s[4] - delta))
        return self.data[s[3] + delta: s[3] + delta + avail] + b"\0" * (size - avail)

    def code_sections(self):
        # .text plus any section named like code; GRID's unpacked image keeps code in .text
        return [s for s in self.sections if s[0] in (".text",)] or self.sections[:1]


def ghidra_headless(args: list[str], maxmem: str = "4G", background: bool = False) -> int:
    env = dict(os.environ, JAVA_HOME=str(JDK), GHIDRA_HEADLESS_MAXMEM=maxmem)
    bat = GHIDRA_HOME / "support" / "analyzeHeadless.bat"
    # the long analysis runs below normal so it never competes with the owner; short calls
    # stay normal, otherwise a busy PC (game, rustc) starves them for many minutes
    flags = getattr(subprocess, "BELOW_NORMAL_PRIORITY_CLASS", 0) if background else 0
    # stdin from NUL: launch.bat pauses for a key after an error and would hang otherwise
    return subprocess.run([str(bat), *args], env=env, creationflags=flags, stdin=subprocess.DEVNULL).returncode


def project_locked(project_dir: Path, name: str) -> bool:
    """True while a live Ghidra process holds the project.

    Ghidra keeps <name>.lock plus a channel lock on <name>.lock~; files left by a killed
    process are stale and do not count."""
    lock, chan = project_dir / f"{name}.lock", project_dir / f"{name}.lock~"
    if not lock.exists():
        return False
    try:
        import msvcrt
        with open(chan, "r+b") as fh:
            try:
                msvcrt.locking(fh.fileno(), msvcrt.LK_NBLCK, 1)
            except OSError:
                return True
            msvcrt.locking(fh.fileno(), msvcrt.LK_UNLCK, 1)
            return False
    except (OSError, ImportError):
        return True
