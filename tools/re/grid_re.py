"""Address helpers for the unpacked GRID executable (local/compatibility/grid-unpacked.exe).

  lookup ADDR...       symbols.tsv row (or the containing function's row), live doc/code mentions,
                       earlier private decompiles under local/**/decomp, cached pseudocode
  dis ADDR [-n N]      capstone disassembly; --func disassembles the whole containing function
  xrefs ADDR           rel32 calls/jumps to ADDR and absolute 32-bit references (push/mov/tables)
  containing ADDR      function entry containing ADDR (master analysis; prologue scan fallback)
  decompile ADDR...    pseudocode via the master Ghidra project, cached in local/re-cache/<ENTRY>.c

Addresses are hex with or without 0x, image base 0x400000 (as in the docs). Python stdlib +
capstone 5.0.6 from local/tools/python. Example: py -I tools/re/grid_re.py lookup 0x7BBB70
"""
from __future__ import annotations

import argparse
import bisect
import re
import struct
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import re_common as rc  # noqa: E402


class Index:
    def __init__(self):
        self.rows = {int(r["address"], 16): r for r in rc.read_symbols()}
        self.entries, self.ends, self.fnames = [], [], []
        path = rc.cache_dir() / "functions.tsv"
        if path.exists():
            for line in path.read_text(encoding="utf-8").splitlines()[1:]:
                f = line.split("\t")
                self.entries.append(int(f[0], 16))
                self.ends.append(int(f[1], 16))
                self.fnames.append(f[2])

    def containing(self, addr: int, pe: rc.PE | None = None) -> tuple[int | None, str]:
        i = bisect.bisect_right(self.entries, addr) - 1
        if i >= 0 and addr <= self.ends[i]:
            return self.entries[i], "master analysis"
        if pe is None:
            return None, "no functions.tsv entry"
        return prologue_scan(pe, addr), "prologue scan (heuristic)"

    def name(self, addr: int) -> str:
        r = self.rows.get(addr)
        if r and r["name"]:
            return r["name"]
        i = bisect.bisect_left(self.entries, addr)
        if i < len(self.entries) and self.entries[i] == addr:
            return self.fnames[i]
        return ""


def prologue_scan(pe: rc.PE, addr: int, limit: int = 0x4000) -> int | None:
    """Walk back to the nearest int3/nop/ret padding followed by a typical prologue byte."""
    start = max(0x401000, addr - limit)
    blob = pe.read(start, addr - start + 1)
    for off in range(len(blob) - 1, 1, -1):
        if blob[off - 1] in (0xCC, 0x90) and blob[off - 2] in (0xCC, 0x90, 0xC3) and blob[off] in PROLOGUE:
            return start + off
    return None


PROLOGUE = {0x55, 0x53, 0x56, 0x57, 0x83, 0x81, 0x8B, 0x6A, 0x64, 0xA1, 0xD9, 0x51}


def mentions(addr: int, limit: int = 12) -> list[str]:
    """Live git grep of the address in docs, logs and Rust (catches rows added after mining)."""
    short = f"{addr:X}"
    pat = rf"(0x0*|\b0*|FUN_0*|DAT_0*){short}\b"
    out = subprocess.run(["git", "grep", "-n", "-i", "-I", "-E", pat, "--", "docs", "crates", "PROJECT_LOG.md", "log", ":!docs/re/symbols.tsv"],
                         cwd=rc.REPO, capture_output=True, text=True, encoding="utf-8", errors="replace").stdout.splitlines()
    return [line if len(line) < 220 else line[:217] + "..." for line in out[:limit]] + ([f"... {len(out) - limit} more"] if len(out) > limit else [])


def old_decompiles(addr: int) -> list[str]:
    """Earlier per-session pseudocode files whose name carries the address (private, ignored)."""
    key = f"{addr:x}"
    roots = [rc.local()] + [p / "local" for p in (rc.main_root() / ".claude" / "worktrees").glob("*") if (p / "local").is_dir()]
    hits = []
    for root in roots:
        dirs = [d for depth in ("*/", "*/*/", "*/*/*/", "*/*/*/*/") for d in root.glob(depth + "decomp*")]
        for d in dirs:
            if not d.is_dir():
                continue
            for f in d.rglob("*"):
                if f.is_file() and re.search(rf"(?<![0-9a-f]){key}(?![0-9a-f])", f.name.lower().replace("0x", "").lstrip("0")):
                    hits.append(str(f))
    return hits


def cmd_lookup(a) -> int:
    idx = Index()
    pe = rc.PE()
    for text in a.addr:
        addr = rc.parse_addr(text)
        print(f"== {rc.fmt(addr)}  section {(pe.section(addr) or ('?',))[0]}")
        row = idx.rows.get(addr)
        entry, how = idx.containing(addr)
        if row is None and entry is not None and entry != addr:
            print(f"   no row; inside {rc.fmt(entry)} {idx.name(entry)} ({how}, +0x{addr - entry:X})")
            row = idx.rows.get(entry)
        if row:
            for k in rc.COLUMNS[1:]:
                if row.get(k):
                    print(f"   {k:10} {row[k]}")
        else:
            print("   not in symbols.tsv: analyse it, then add a row (docs/re/README.md)")
        found = mentions(addr)
        if found:
            print("   mentions:")
            for m in found:
                print(f"     {m}")
        from master import cached_path  # noqa: E402
        cp = cached_path(entry if entry is not None else addr)
        print(f"   decompile cache: {cp if cp else 'none (tools/re/grid_re.py decompile ' + rc.fmt(addr) + ')'}")
        if a.old:
            for f in old_decompiles(addr)[:10]:
                print(f"   earlier decompile: {f}")
    return 0


def disassemble(pe: rc.PE, start: int, size: int, idx: Index, count: int | None = None):
    cs = rc.import_capstone()
    md = cs.Cs(cs.CS_ARCH_X86, cs.CS_MODE_32)
    code = pe.read(start, size)
    n = 0
    for insn in md.disasm(code, start):
        note = ""
        for tok in re.findall(r"0x[0-9a-f]{6,8}", insn.op_str):
            v = int(tok, 16)
            nm = idx.name(v)
            if nm:
                note += f"  ; {rc.fmt(v)} {nm}"
        print(f"{insn.address:08X}  {insn.bytes.hex():<20} {insn.mnemonic:<7} {insn.op_str}{note}")
        n += 1
        if count is not None and n >= count:
            break


def cmd_dis(a) -> int:
    idx = Index()
    pe = rc.PE()
    addr = rc.parse_addr(a.addr)
    if a.func:
        entry, how = idx.containing(addr, pe)
        if entry is None:
            sys.exit("no containing function known")
        i = bisect.bisect_left(idx.entries, entry)
        end = idx.ends[i] if i < len(idx.entries) and idx.entries[i] == entry else entry + 0x400
        print(f"; function {rc.fmt(entry)} {idx.name(entry)} ({how}) to {rc.fmt(end)}")
        disassemble(pe, entry, end - entry + 1, idx)
    else:
        disassemble(pe, addr, a.n * 15, idx, a.n)
    return 0


def text_va_blob(pe: rc.PE):
    s = [x for x in pe.sections if x[0] == ".text"][0]
    return s[1], pe.data[s[3]: s[3] + s[4]]


def cmd_xrefs(a) -> int:
    idx = Index()
    pe = rc.PE()
    target = rc.parse_addr(a.addr)
    base, text = text_va_blob(pe)
    hits = []
    # rel32 call (E8), jmp (E9) and jcc (0F 80..8F)
    for opcode, length in ((b"\xe8", 5), (b"\xe9", 5)):
        pos = text.find(opcode)
        while pos != -1:
            if pos + length <= len(text):
                rel = struct.unpack_from("<i", text, pos + length - 4)[0]
                if base + pos + length + rel == target:
                    hits.append((base + pos, "call" if opcode == b"\xe8" else "jmp"))
            pos = text.find(opcode, pos + 1)
    pos = text.find(b"\x0f")
    while pos != -1:
        if pos + 6 <= len(text) and 0x80 <= text[pos + 1] <= 0x8F:
            rel = struct.unpack_from("<i", text, pos + 2)[0]
            if base + pos + 6 + rel == target:
                hits.append((base + pos, "jcc"))
        pos = text.find(b"\x0f", pos + 1)
    # absolute references anywhere in the image (code immediates, vtables, data pointers)
    needle = struct.pack("<I", target)
    for name, va, vsize, raw, rsize in pe.sections:
        if name in (".rsrc", ".reloc"):
            continue
        blob = pe.data[raw: raw + rsize]
        pos = blob.find(needle)
        while pos != -1:
            hits.append((va + pos, "abs32 in " + name))
            pos = blob.find(needle, pos + 1)
    cs = rc.import_capstone()
    md = cs.Cs(cs.CS_ARCH_X86, cs.CS_MODE_32)
    print(f"xrefs to {rc.fmt(target)} {idx.name(target)}: {len(hits)} candidate(s)")
    for va, kind in sorted(hits):
        entry, how = idx.containing(va)
        where = f"in {rc.fmt(entry)} {idx.name(entry)}" if entry is not None else "(no known function)"
        insn_text = ""
        if kind.startswith("abs32 in .text"):
            # the 4 bytes sit inside an instruction that starts up to 7 bytes earlier
            for back in range(1, 8):
                ins = next(md.disasm(pe.read(va - back, 16), va - back), None)
                if ins and ins.address + ins.size >= va + 4 and ins.address < va:
                    insn_text = f"{ins.address:08X} {ins.mnemonic} {ins.op_str}"
                    break
        elif not kind.startswith("abs32"):
            ins = next(md.disasm(pe.read(va, 16), va), None)
            insn_text = f"{ins.mnemonic} {ins.op_str}" if ins else ""
        print(f"  {va:08X}  {kind:<16} {where:<45} {insn_text}")
    if not idx.entries:
        print("note: local/re-cache/functions.tsv missing (tools/re/master.py functions); containing functions unknown")
    return 0


def cmd_containing(a) -> int:
    idx = Index()
    pe = rc.PE()
    for text in a.addr:
        addr = rc.parse_addr(text)
        entry, how = idx.containing(addr, pe)
        print(f"{rc.fmt(addr)}\t{rc.fmt(entry) if entry is not None else '-'}\t{idx.name(entry) if entry is not None else ''}\t{how}")
    return 0


def cmd_decompile(a) -> int:
    from master import cached_path, decompile  # noqa: E402
    addrs = [rc.parse_addr(x) for x in a.addr]
    code = decompile(addrs, refresh=a.refresh)
    for addr in addrs:
        p = cached_path(addr)
        print(f"{rc.fmt(addr)}\t{p if p else 'FAILED'}")
        if p and a.print and p.suffix == ".c":
            print(p.read_text(encoding="utf-8"))
    return code


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("lookup"); s.add_argument("addr", nargs="+")
    s.add_argument("--old", action="store_true", help="also search earlier local/**/decomp folders")
    s = sub.add_parser("dis"); s.add_argument("addr"); s.add_argument("-n", type=int, default=30)
    s.add_argument("--func", action="store_true")
    s = sub.add_parser("xrefs"); s.add_argument("addr")
    s = sub.add_parser("containing"); s.add_argument("addr", nargs="+")
    s = sub.add_parser("decompile"); s.add_argument("addr", nargs="+")
    s.add_argument("--print", action="store_true"); s.add_argument("--refresh", action="store_true")
    a = ap.parse_args()
    return {"lookup": cmd_lookup, "dis": cmd_dis, "xrefs": cmd_xrefs, "containing": cmd_containing,
            "decompile": cmd_decompile}[a.cmd](a)


if __name__ == "__main__":
    sys.exit(main())
