"""Read-only field sampler for the original GRID research copy
(docs/BRAKING_REFERENCE.md, docs/ROBUSTNESS_RESEARCH.md).

    python -I tools/capture_grid_field.py SESSION.json MANAGER.json SECONDS local/OUT.tsv

MANAGER.json is the output of tools/find_reference_vehicle_manager.ps1; every
vehicle it lists ("body" is the car object, "index" its manager slot,
"grid_index" its grid slot) is sampled. The process is opened with
VM_READ | QUERY_LIMITED_INFORMATION only, after the same identity checks as
tools/capture_grid_braking.py.

Header lines: `#FD01 pid=... cars=N`, then one `#car index grid car physical`
line per vehicle. Each row: QPC seconds, then per car (in header order) the hex
of car+0xE00 (0x80: published throttle +E1C, brake +E20, steering requests
+E3C/+E40.., clutch +E34), car+0x1960 (0x30: gear +196C) and physical+0x1D0
(0x60: pose matrix, angular +0x40 and linear +0x50 velocity). A row is
written whenever any car's physical block changed (one row per physics step).
"""
import ctypes
import ctypes.wintypes as wt
import datetime
import hashlib
import json
import os
import re
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


def vehicles(manager_text):
    """(index, grid_index, body) for every vehicle entry of the scan output."""
    found = re.findall(
        r'"index":\s*(\d+),\s*"vehicle":\s*"[0-9a-f]+",\s*"body":\s*"([0-9a-f]+)",\s*"kind":\s*\d+,\s*"grid_index":\s*(\d+)',
        manager_text)
    return [(int(i), int(g), int(b, 16)) for i, b, g in found]


def main():
    session, manager, seconds, out = sys.argv[1], sys.argv[2], float(sys.argv[3]), sys.argv[4]
    if "local" not in os.path.abspath(out).replace("\\", "/").split("/") or os.path.exists(out):
        sys.exit("output must be a new file under local/")
    if not 0.5 <= seconds <= 60:
        sys.exit("0.5..60 s")
    s = json.load(open(session, encoding="utf-8-sig"))
    if s["Path"] != PATH or s["ExeSHA256"] != SHA:
        sys.exit("session is not the research copy")
    if hashlib.sha256(open(PATH, "rb").read()).hexdigest() != SHA:
        sys.exit("executable hash changed")
    cars = vehicles(open(manager, encoding="utf-8-sig", errors="replace").read())
    if not 1 <= len(cars) <= 16 or len({c[2] for c in cars}) != len(cars):
        sys.exit("manager scan lists no usable vehicles")
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
    dt = datetime.datetime.fromisoformat(s["StartedUtc"].replace("Z", "").split(".")[0] + "+00:00")
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
    links = []
    for index, grid, car in cars:
        wrapper = ptr(car + 0xDB0)
        physical = ptr(wrapper + 8)
        if not 0x10000 < physical < 0x7FFE0000:
            sys.exit(f"car {car:x} links invalid")
        links.append((index, grid, car, wrapper, physical))
    rows = 0
    last = None
    end = time.perf_counter() + seconds
    with open(out, "x", encoding="ascii") as f:
        f.write(f"#FD01\tpid={s['Pid']}\tcars={len(links)}\n")
        for index, grid, car, _, physical in links:
            f.write(f"#car\t{index}\t{grid}\t{car:x}\t{physical:x}\n")
        while time.perf_counter() < end and rows < 30000:
            t = time.perf_counter()
            blocks = []
            for _, _, car, wrapper, physical in links:
                if ptr(car + 0xDB0) != wrapper or ptr(wrapper + 8) != physical:
                    sys.exit("car ownership changed")
                blocks.append((read(car + 0xE00, 0x80), read(car + 0x1960, 0x30), read(physical + 0x1D0, 0x60)))
            key = b"".join(p for _, _, p in blocks)
            if key != last:
                last = key
                f.write(f"{t:.6f}\t" + "\t".join(c.hex() + g.hex() + p.hex() for c, g, p in blocks) + "\n")
                rows += 1
            time.sleep(0.0005)
    print(json.dumps({"rows": rows, "cars": len(links), "out": out}))


main()
