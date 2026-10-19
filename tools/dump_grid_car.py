"""Read-only one-shot dump of an original GRID car object (research copy only).

    python -I tools/dump_grid_car.py SESSION.json CAR_HEX local/OUT.bin [LENGTH_HEX]

Writes car+0 .. car+LENGTH (default 0x2400: controls, engine, gearbox and the
four wheel blocks at +0x1990) as raw bytes, plus OUT.json with the process,
the addresses, the world step (+0x7C4/+0x7C8 via car+0xDB0 -> +8 -> +0x32C)
and SHA-256 of the dump. Same identity checks as tools/capture_grid_braking.py;
the process is opened with VM_READ | QUERY_LIMITED_INFORMATION only.
"""
import ctypes
import ctypes.wintypes as wt
import datetime
import hashlib
import json
import os
import struct
import sys

PATH = r"C:\Users\debros\Tools\grid-research\grid-compat\grid.exe"
SHA = "b1da1b2d9469fb0c973dbaaad29b230c1567f212c54de849326f80b4aece34a6"
k32 = ctypes.WinDLL("kernel32", use_last_error=True)
k32.OpenProcess.restype = wt.HANDLE
k32.ReadProcessMemory.argtypes = [wt.HANDLE, wt.LPCVOID, wt.LPVOID, ctypes.c_size_t, ctypes.POINTER(ctypes.c_size_t)]
k32.QueryFullProcessImageNameW.argtypes = [wt.HANDLE, wt.DWORD, wt.LPWSTR, ctypes.POINTER(wt.DWORD)]
k32.GetProcessTimes.argtypes = [wt.HANDLE] + [ctypes.POINTER(wt.FILETIME)] * 4


def main():
    session, car, out = sys.argv[1], int(sys.argv[2], 16), sys.argv[3]
    length = int(sys.argv[4], 16) if len(sys.argv) > 4 else 0x2400
    if "local" not in os.path.abspath(out).replace("\\", "/").split("/") or os.path.exists(out):
        sys.exit("output must be a new file under local/")
    if not 0 < length <= 0x40000:
        sys.exit("length out of range")
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
    wrapper = ptr(car + 0xDB0)
    physical = ptr(wrapper + 8)
    world = ptr(physical + 0x32C)
    data = read(car, length)
    w = read(world + 0x7C4, 8)
    with open(out, "xb") as f:
        f.write(data)
    meta = {
        "pid": s["Pid"], "utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "car": hex(car), "length": hex(length), "physical": hex(physical), "world": hex(world),
        "world_7c4": struct.unpack_from("<I", w, 0)[0], "world_7c8_step_s": struct.unpack_from("<f", w, 4)[0],
        "sha256": hashlib.sha256(data).hexdigest(), "out": out,
    }
    with open(out + ".json", "x", encoding="utf-8") as f:
        json.dump(meta, f, indent=1)
    print(json.dumps(meta))


main()
