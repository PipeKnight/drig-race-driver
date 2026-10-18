"""Bounded read-only capture of the player car of the private GRID research copy
for the braking comparison (docs/BRAKING_REFERENCE.md).

    python -I tools/capture_grid_braking.py SESSION.json CAR_HEX SECONDS local/OUT.tsv

Opens the process with VM_READ | QUERY_LIMITED_INFORMATION only (no writes,
injection or suspension). Checks the executable path, SHA-256, start time and
module base from the session record, and the car -> wrapper (+0xDB0) ->
physical (+8) -> world (+0x32C) links before and during the capture.

Each row: QPC seconds, then hex of car+0xE00 (0x100 bytes: controls +E1C..,
clutch +E34, lock-up timer +E74), car+0x1140 (0x80: assist parameters
+1160..+1178, ABS mode +11BC), car+0x1550 (0x10: aid bytes +1558..+155A),
car+0x1960 (0x30: gear +196C, gearbox +1970..+1972, +197C, +1988), the four
wheel blocks car+0x1990 (4 x 0x150), physical+0x1D0 (0x60 pose/motion),
world+0x7C4 (8) and the engine/drivetrain block car+0x1840 (0x120: RPM +0x1894,
ratios +0x18F4..), car+0x1560 (0x100: mass +15A0, engine coupling mode +1598,
idle/clutch flag +1640, differential +1570..+1594) and car+0x17A0 (0x10:
engine inertia). A row is written only when the physical motion block or a
wheel block changed since the last row (one row per physics step).
"""
import ctypes
import ctypes.wintypes as wt
import hashlib
import json
import os
import struct
import sys
import time

PATH = r"C:\Users\debros\Tools\grid-research\grid-compat\grid.exe"
SHA = "b1da1b2d9469fb0c973dbaaad29b230c1567f212c54de849326f80b4aece34a6"
k32 = ctypes.WinDLL("kernel32", use_last_error=True)
k32.OpenProcess.restype = wt.HANDLE
k32.ReadProcessMemory.argtypes = [wt.HANDLE, wt.LPCVOID, wt.LPVOID, ctypes.c_size_t, ctypes.POINTER(ctypes.c_size_t)]
k32.QueryFullProcessImageNameW.argtypes = [wt.HANDLE, wt.DWORD, wt.LPWSTR, ctypes.POINTER(wt.DWORD)]
k32.GetProcessTimes.argtypes = [wt.HANDLE] + [ctypes.POINTER(wt.FILETIME)] * 4


def main():
    session, car, seconds, out = sys.argv[1], int(sys.argv[2], 16), float(sys.argv[3]), sys.argv[4]
    if not os.path.abspath(out).replace("\\", "/").split("/").count("local") or os.path.exists(out):
        sys.exit("output must be a new file under local/")
    if not 0.5 <= seconds <= 60:
        sys.exit("0.5..60 s")
    s = json.load(open(session, encoding="utf-8-sig"))
    if s["Path"] != PATH or s["ExeSHA256"] != SHA:
        sys.exit("session is not the research copy")
    if hashlib.sha256(open(PATH, "rb").read()).hexdigest() != SHA:
        sys.exit("executable hash changed")
    h = k32.OpenProcess(0x0010 | 0x1000, False, int(s["Pid"]))
    if not h:
        sys.exit("cannot open process")
    buf = ctypes.create_unicode_buffer(1024)
    n = wt.DWORD(1024)
    k32.QueryFullProcessImageNameW(h, 0, buf, ctypes.byref(n))
    if buf.value.lower() != PATH.lower():
        sys.exit("process path mismatch")
    ft = [wt.FILETIME() for _ in range(4)]
    k32.GetProcessTimes(h, *[ctypes.byref(f) for f in ft])
    created = (ft[0].dwHighDateTime << 32) | ft[0].dwLowDateTime
    started = s["StartedUtc"]
    # FILETIME of the recorded start (100 ns since 1601), compared to 1 s.
    import datetime
    dt = datetime.datetime.fromisoformat(started.replace("Z", "+00:00").split(".")[0] + "+00:00")
    rec = int((dt - datetime.datetime(1601, 1, 1, tzinfo=datetime.timezone.utc)).total_seconds() * 1e7)
    if abs(created - rec) > 2e7:
        sys.exit("process start time mismatch")

    def read(addr, size):
        b = ctypes.create_string_buffer(size)
        got = ctypes.c_size_t()
        if not k32.ReadProcessMemory(h, ctypes.c_void_p(addr), b, size, ctypes.byref(got)) or got.value != size:
            raise OSError(f"read {addr:x}")
        return b.raw

    ptr = lambda a: struct.unpack("<I", read(a, 4))[0]
    wrapper = ptr(car + 0xDB0)
    physical = ptr(wrapper + 8)
    world = ptr(physical + 0x32C)
    if not (0x10000 < physical < 0x7FFE0000 and 0x10000 < world < 0x7FFE0000):
        sys.exit("car links invalid")
    regions = [(car + 0xE00, 0x100), (car + 0x1140, 0x80), (car + 0x1550, 0x10), (car + 0x1960, 0x30),
               (car + 0x1990, 0x540), (physical + 0x1D0, 0x60), (world + 0x7C4, 8),
               (car + 0x1840, 0x120), (car + 0x1560, 0x100), (car + 0x17A0, 0x10)]
    rows = 0
    last = None
    end = time.perf_counter() + seconds
    with open(out, "x", encoding="ascii") as f:
        f.write(f"#BR01\tpid={s['Pid']}\tcar={car:x}\tphysical={physical:x}\tworld={world:x}\n")
        while time.perf_counter() < end and rows < 20000:
            t = time.perf_counter()
            if ptr(car + 0xDB0) != wrapper or ptr(wrapper + 8) != physical:
                sys.exit("car ownership changed")
            blocks = [read(a, n) for a, n in regions]
            key = blocks[4] + blocks[5]
            if key != last:
                last = key
                f.write(f"{t:.6f}\t" + "\t".join(b.hex() for b in blocks) + "\n")
                rows += 1
            time.sleep(0.0005)
    print(json.dumps({"rows": rows, "car": hex(car), "physical": hex(physical), "world": hex(world), "out": out}))


main()
