"""User-mode sampling profiler for the port's Windows x64 executables.

Starts a command, suspends each of its threads about every millisecond, walks
the stack with the system DbgHelp (StackWalkEx, including inline frames from
the PDB) and resumes it. No administrator rights, ETW or downloaded tools are
needed. Prints the hottest functions by self samples (leaf frame) and by
inclusive samples (anywhere on the stack), plus the hottest call paths.

usage: python -I tools/sample_profile.py [--interval-ms N] [--top N]
           [--filter TEXT] [--busy] [--out REPORT.txt] -- EXE ARG...

Build the target with symbols, e.g.
    CARGO_PROFILE_RELEASE_DEBUG=limited cargo build --release --target-dir target/profile
"""
import argparse
import collections
import ctypes
import ctypes.wintypes as wt
import os
import subprocess
import sys
import threading
import time

kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
dbghelp = ctypes.WinDLL("dbghelp", use_last_error=True)
winmm = ctypes.WinDLL("winmm")

THREAD_SUSPEND_RESUME = 0x0002
THREAD_GET_CONTEXT = 0x0008
THREAD_QUERY_INFORMATION = 0x0040
TH32CS_SNAPTHREAD = 0x4
CONTEXT_FULL = 0x10000B
IMAGE_FILE_MACHINE_AMD64 = 0x8664
ADDR_MODE_FLAT = 3


class ADDRESS64(ctypes.Structure):
    _fields_ = [("Offset", ctypes.c_uint64), ("Segment", ctypes.c_uint16), ("Mode", ctypes.c_uint32)]


class KDHELP64(ctypes.Structure):
    _fields_ = [
        ("Thread", ctypes.c_uint64),
        ("ThCallbackStack", ctypes.c_uint32),
        ("ThCallbackBStore", ctypes.c_uint32),
        ("NextCallback", ctypes.c_uint32),
        ("FramePointer", ctypes.c_uint32),
        ("KiCallUserMode", ctypes.c_uint64),
        ("KeUserCallbackDispatcher", ctypes.c_uint64),
        ("SystemRangeStart", ctypes.c_uint64),
        ("KiUserExceptionDispatcher", ctypes.c_uint64),
        ("StackBase", ctypes.c_uint64),
        ("StackLimit", ctypes.c_uint64),
        ("BuildVersion", ctypes.c_uint32),
        ("RetpolineStubFunctionTableSize", ctypes.c_uint32),
        ("RetpolineStubFunctionTable", ctypes.c_uint64),
        ("RetpolineStubOffset", ctypes.c_uint32),
        ("RetpolineStubSize", ctypes.c_uint32),
        ("Reserved0", ctypes.c_uint64 * 2),
    ]


class STACKFRAME_EX(ctypes.Structure):
    _fields_ = [
        ("AddrPC", ADDRESS64),
        ("AddrReturn", ADDRESS64),
        ("AddrFrame", ADDRESS64),
        ("AddrStack", ADDRESS64),
        ("AddrBStore", ADDRESS64),
        ("FuncTableEntry", ctypes.c_void_p),
        ("Params", ctypes.c_uint64 * 4),
        ("Far", wt.BOOL),
        ("Virtual", wt.BOOL),
        ("Reserved", ctypes.c_uint64 * 3),
        ("KdHelp", KDHELP64),
        ("StackFrameSize", ctypes.c_uint32),
        ("InlineFrameContext", ctypes.c_uint32),
    ]


class SYMBOL_INFOW(ctypes.Structure):
    _fields_ = [
        ("SizeOfStruct", ctypes.c_uint32),
        ("TypeIndex", ctypes.c_uint32),
        ("Reserved", ctypes.c_uint64 * 2),
        ("Index", ctypes.c_uint32),
        ("Size", ctypes.c_uint32),
        ("ModBase", ctypes.c_uint64),
        ("Flags", ctypes.c_uint32),
        ("Value", ctypes.c_uint64),
        ("Address", ctypes.c_uint64),
        ("Register", ctypes.c_uint32),
        ("Scope", ctypes.c_uint32),
        ("Tag", ctypes.c_uint32),
        ("NameLen", ctypes.c_uint32),
        ("MaxNameLen", ctypes.c_uint32),
        ("Name", ctypes.c_wchar * 1),
    ]


class IMAGEHLP_LINEW64(ctypes.Structure):
    _fields_ = [
        ("SizeOfStruct", wt.DWORD),
        ("Key", ctypes.c_void_p),
        ("LineNumber", wt.DWORD),
        ("FileName", ctypes.c_wchar_p),
        ("Address", ctypes.c_uint64),
    ]


class THREADENTRY32(ctypes.Structure):
    _fields_ = [
        ("dwSize", wt.DWORD),
        ("cntUsage", wt.DWORD),
        ("th32ThreadID", wt.DWORD),
        ("th32OwnerProcessID", wt.DWORD),
        ("tpBasePri", wt.LONG),
        ("tpDeltaPri", wt.LONG),
        ("dwFlags", wt.DWORD),
    ]


kernel32.OpenThread.restype = wt.HANDLE
kernel32.OpenThread.argtypes = [wt.DWORD, wt.BOOL, wt.DWORD]
kernel32.SuspendThread.argtypes = [wt.HANDLE]
kernel32.ResumeThread.argtypes = [wt.HANDLE]
kernel32.GetThreadContext.argtypes = [wt.HANDLE, ctypes.c_void_p]
kernel32.CreateToolhelp32Snapshot.restype = wt.HANDLE
kernel32.CreateToolhelp32Snapshot.argtypes = [wt.DWORD, wt.DWORD]
kernel32.Thread32First.argtypes = [wt.HANDLE, ctypes.POINTER(THREADENTRY32)]
kernel32.Thread32Next.argtypes = [wt.HANDLE, ctypes.POINTER(THREADENTRY32)]
kernel32.CloseHandle.argtypes = [wt.HANDLE]
kernel32.OpenProcess.restype = wt.HANDLE
kernel32.OpenProcess.argtypes = [wt.DWORD, wt.BOOL, wt.DWORD]
dbghelp.SymInitializeW.argtypes = [wt.HANDLE, wt.LPCWSTR, wt.BOOL]
dbghelp.SymSetOptions.argtypes = [wt.DWORD]
dbghelp.SymRefreshModuleList.argtypes = [wt.HANDLE]
dbghelp.StackWalkEx.argtypes = [
    wt.DWORD, wt.HANDLE, wt.HANDLE, ctypes.POINTER(STACKFRAME_EX), ctypes.c_void_p,
    ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p, wt.DWORD,
]
dbghelp.SymFromInlineContextW.argtypes = [
    wt.HANDLE, ctypes.c_uint64, wt.ULONG, ctypes.POINTER(ctypes.c_uint64), ctypes.c_void_p,
]
dbghelp.SymGetLineFromInlineContextW.argtypes = [
    wt.HANDLE, ctypes.c_uint64, wt.ULONG, ctypes.c_uint64, ctypes.POINTER(wt.DWORD),
    ctypes.POINTER(IMAGEHLP_LINEW64),
]
FUNCTION_TABLE = ctypes.cast(dbghelp.SymFunctionTableAccess64, ctypes.c_void_p)
MODULE_BASE = ctypes.cast(dbghelp.SymGetModuleBase64, ctypes.c_void_p)


def threads_of(pid):
    snap = kernel32.CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0)
    entry = THREADENTRY32()
    entry.dwSize = ctypes.sizeof(THREADENTRY32)
    found = []
    ok = kernel32.Thread32First(snap, ctypes.byref(entry))
    while ok:
        if entry.th32OwnerProcessID == pid:
            found.append(entry.th32ThreadID)
        ok = kernel32.Thread32Next(snap, ctypes.byref(entry))
    kernel32.CloseHandle(snap)
    return found


class Sampler:
    def __init__(self, process_handle, pid, interval):
        self.process = process_handle
        self.pid = pid
        self.interval = interval
        self.threads = {}
        self.cycles = {}
        self.stacks = collections.Counter()
        self.samples = 0
        self.line_counts = collections.Counter()
        self.leaf_lines = collections.Counter()
        raw = ctypes.create_string_buffer(1232 + 16)
        self.context = (ctypes.addressof(raw) + 15) & ~15
        self.context_raw = raw
        self.names = {}
        self.lines = {}

    def refresh_threads(self):
        for tid in threads_of(self.pid):
            if tid not in self.threads:
                handle = kernel32.OpenThread(
                    THREAD_SUSPEND_RESUME | THREAD_GET_CONTEXT | THREAD_QUERY_INFORMATION, False, tid
                )
                if handle:
                    self.threads[tid] = handle

    def busy(self, tid, handle):
        # A thread whose cycle count has not moved since its last sample was
        # waiting the whole time; skipping it keeps the rate up with many
        # idle pool threads (Bevy).
        cycles = ctypes.c_uint64()
        if not kernel32.QueryThreadCycleTime(handle, ctypes.byref(cycles)):
            return True
        moved = self.cycles.get(tid) != cycles.value
        self.cycles[tid] = cycles.value
        return moved

    def sample_thread(self, handle):
        if kernel32.SuspendThread(handle) == 0xFFFFFFFF:
            return None
        try:
            ctypes.c_uint32.from_address(self.context + 0x30).value = CONTEXT_FULL
            if not kernel32.GetThreadContext(handle, self.context):
                return None
            rip = ctypes.c_uint64.from_address(self.context + 0xF8).value
            rsp = ctypes.c_uint64.from_address(self.context + 0x98).value
            frame = STACKFRAME_EX()
            frame.StackFrameSize = ctypes.sizeof(STACKFRAME_EX)
            frame.AddrPC.Offset = rip
            frame.AddrPC.Mode = ADDR_MODE_FLAT
            frame.AddrFrame.Offset = rsp
            frame.AddrFrame.Mode = ADDR_MODE_FLAT
            frame.AddrStack.Offset = rsp
            frame.AddrStack.Mode = ADDR_MODE_FLAT
            stack = []
            for _ in range(128):
                if not dbghelp.StackWalkEx(
                    IMAGE_FILE_MACHINE_AMD64, self.process, handle, ctypes.byref(frame),
                    self.context, None, FUNCTION_TABLE, MODULE_BASE, None, 0,
                ):
                    break
                if frame.AddrPC.Offset == 0:
                    break
                stack.append((frame.AddrPC.Offset, frame.InlineFrameContext))
            return tuple(stack)
        finally:
            kernel32.ResumeThread(handle)

    def name(self, key):
        if key in self.names:
            return self.names[key]
        address, inline = key
        buffer = ctypes.create_string_buffer(ctypes.sizeof(SYMBOL_INFOW) + 2048 * 2)
        info = SYMBOL_INFOW.from_buffer(buffer)
        info.SizeOfStruct = ctypes.sizeof(SYMBOL_INFOW)
        info.MaxNameLen = 2047
        displacement = ctypes.c_uint64()
        if dbghelp.SymFromInlineContextW(
            self.process, address, inline, ctypes.byref(displacement), ctypes.addressof(info)
        ):
            offset = SYMBOL_INFOW.Name.offset
            name = ctypes.wstring_at(ctypes.addressof(info) + offset, info.NameLen)
        else:
            name = f"0x{address:x}"
        self.names[key] = name
        return name


    def line(self, key):
        if key in self.lines:
            return self.lines[key]
        address, inline = key
        info = IMAGEHLP_LINEW64()
        info.SizeOfStruct = ctypes.sizeof(IMAGEHLP_LINEW64)
        displacement = wt.DWORD()
        if dbghelp.SymGetLineFromInlineContextW(
            self.process, address, inline, 0, ctypes.byref(displacement), ctypes.byref(info)
        ):
            text = f"{os.path.basename(info.FileName or '?')}:{info.LineNumber}"
        else:
            text = "?"
        self.lines[key] = text
        return text


def simplify(name):
    # Drop generic arguments for readability.
    out, depth = [], 0
    for ch in name:
        if ch == "<":
            depth += 1
        elif ch == ">":
            depth -= 1
        elif depth == 0:
            out.append(ch)
    return "".join(out)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--interval-ms", type=float, default=1.0)
    parser.add_argument("--top", type=int, default=40)
    parser.add_argument("--filter", default="", help="keep only stacks containing this text")
    parser.add_argument("--out")
    parser.add_argument(
        "--busy", action="store_true",
        help="sample only threads that ran since their last sample (skips idle pool threads)",
    )
    parser.add_argument(
        "--lines-in", action="append", default=[],
        help="also attribute samples to source lines of the innermost frame of this function",
    )
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    if not command:
        parser.error("missing command")
    command[0] = os.path.abspath(command[0])
    winmm.timeBeginPeriod(1)
    child = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    output = []
    reader = threading.Thread(target=lambda: output.extend(child.stdout), daemon=True)
    reader.start()
    process = kernel32.OpenProcess(0x1F0FFF, False, child.pid)
    sampler = Sampler(process, child.pid, args.interval_ms / 1000)
    dbghelp.SymSetOptions(0x2 | 0x10)  # UNDNAME | LOAD_LINES
    time.sleep(0.3)
    symbols_ready = bool(dbghelp.SymInitializeW(process, os.path.dirname(os.path.abspath(command[0])), True))
    started = time.perf_counter()
    last_refresh = 0
    while child.poll() is None:
        now = time.perf_counter()
        if now - last_refresh > 1.0:
            sampler.refresh_threads()
            if symbols_ready:
                dbghelp.SymRefreshModuleList(process)
            last_refresh = now
        for tid, handle in list(sampler.threads.items()):
            if args.busy and not sampler.busy(tid, handle):
                continue
            stack = sampler.sample_thread(handle)
            if stack:
                # Resolve names while the process (and its modules) are alive.
                named = tuple(simplify(sampler.name(f)) for f in stack)
                sampler.stacks[named] += 1
                if args.filter and not any(args.filter in n for n in named):
                    continue
                for target in args.lines_in:
                    for f, n in zip(stack, named):
                        if target in n:
                            sampler.line_counts[(target, n, sampler.line(f))] += 1
                            break
                sampler.leaf_lines[(named[0], sampler.line(stack[0]))] += 1
                sampler.samples += 1
        time.sleep(sampler.interval)
    elapsed = time.perf_counter() - started
    reader.join(timeout=5)
    winmm.timeEndPeriod(1)

    stacks = {s: n for s, n in sampler.stacks.items() if not args.filter or any(args.filter in f for f in s)}
    total = sum(stacks.values())
    self_counts = collections.Counter()
    inclusive = collections.Counter()
    for stack, n in stacks.items():
        self_counts[stack[0]] += n
        for f in set(stack):
            inclusive[f] += n
    lines = [
        f"command: {' '.join(command)}",
        f"wall {elapsed:.1f} s, {sampler.samples} samples, {total} after filter '{args.filter}'",
        "",
        "self %  function (leaf frame, inline frames separate)",
    ]
    for f, n in self_counts.most_common(args.top):
        lines.append(f"{100 * n / max(total, 1):6.2f}  {f}")
    lines += ["", "incl %  function"]
    for f, n in inclusive.most_common(args.top * 2):
        lines.append(f"{100 * n / max(total, 1):6.2f}  {f}")
    lines += ["", "top call paths (leaf first, 8 frames)"]
    paths = collections.Counter()
    for stack, n in stacks.items():
        paths[" <- ".join(stack[:8])] += n
    for p, n in paths.most_common(args.top // 2):
        lines.append(f"{100 * n / max(total, 1):6.2f}  {p}")
    lines += ["", "self % by leaf source line"]
    for (f, line), n in sampler.leaf_lines.most_common(args.top):
        lines.append(f"{100 * n / max(total, 1):6.2f}  {line}  {f}")
    for target in args.lines_in:
        lines += ["", f"incl % by source line of the innermost '{target}' frame"]
        rows = [(k, n) for k, n in sampler.line_counts.items() if k[0] == target]
        for (t, f, line), n in sorted(rows, key=lambda r: -r[1])[: args.top]:
            lines.append(f"{100 * n / max(total, 1):6.2f}  {line}  {f}")
    lines += ["", "program output (tail):"] + [l.decode(errors="replace").rstrip() for l in output[-30:]]
    report = "\n".join(lines)
    if args.out:
        with open(args.out, "w", encoding="utf-8") as f:
            f.write(report + "\n")
    print(report)
    return child.returncode


if __name__ == "__main__":
    sys.exit(main())
