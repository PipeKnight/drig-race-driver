"""Read-only sampler of the original GRID AI's brake-line state (research copy only).

    python -I tools/capture_grid_ai_brake.py discover SESSION.json MANAGER.json local/OUT.json
    python -I tools/capture_grid_ai_brake.py capture SESSION.json local/DISCOVER.json SECONDS local/OUT.tsv
    python -I tools/capture_grid_ai_brake.py probe SESSION.json local/DISCOVER.json SECONDS local/OUT.bin

probe: dumps whole driver objects of cars that are part braking, to locate
the AI's published control packet offline (see probe()).

discover: finds the 12 AI driver objects by the track-view vtable 0xCF7FAC at
driver+0x2DD0 (constructor 0x79ECD0), checks each one's own car state
*(driver-0x40), matches drivers to the vehicles of MANAGER.json (output of
tools/find_reference_vehicle_manager.ps1) by position, and records the car
state's vtable entries +0x10/+0x1C (wheel-speed array and count, read by the
AI ABS cap 0x77A2E0) and a 0x300-byte dump of each car state for offline
resolution of the wheel-speed array.

capture: one row per physics step (a row is written when any car's physical
block changes), with QPC seconds, world +0x7C4/+0x7C8 and per driver:
request (driver+0x2CE0) +0x20 distance modifier, +0x24 hold time, +0x28
distance to the brake line, +0x30 target speed, +0x34 brake-line id; track view
(driver+0x2DD0) +0x68 look-ahead, +0x6C curvature; current behaviour vtable
*(driver+0x2DC0); car state +0x20 centre, +0xD0 speed, +0x144 table grip; the
vehicle's published throttle +E1C, brake +E20, steering +E30 and gear +196C;
when DISCOVER.json names it, the wheel-speed array; the whole request block
(driver+0x2CE0, 0xC8 bytes: brake cap +0xA0, flags +0xA4..); and any
DISCOVER.json "extra" [offset-from-driver hex, length] windows.

The process is opened with VM_READ | QUERY_LIMITED_INFORMATION only, after the
same identity checks as tools/capture_grid_braking.py. Outputs must be new
files under local/.
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
VIEW_VTABLE = 0xCF7FAC
k32 = ctypes.WinDLL("kernel32", use_last_error=True)
k32.OpenProcess.restype = wt.HANDLE
k32.ReadProcessMemory.argtypes = [wt.HANDLE, wt.LPCVOID, wt.LPVOID, ctypes.c_size_t, ctypes.POINTER(ctypes.c_size_t)]
k32.QueryFullProcessImageNameW.argtypes = [wt.HANDLE, wt.DWORD, wt.LPWSTR, ctypes.POINTER(wt.DWORD)]
k32.GetProcessTimes.argtypes = [wt.HANDLE] + [ctypes.POINTER(wt.FILETIME)] * 4


class MBI(ctypes.Structure):
    _fields_ = [("BaseAddress", ctypes.c_void_p), ("AllocationBase", ctypes.c_void_p),
                ("AllocationProtect", wt.DWORD), ("RegionSize", ctypes.c_size_t),
                ("State", wt.DWORD), ("Protect", wt.DWORD), ("Type", wt.DWORD)]


k32.VirtualQueryEx.argtypes = [wt.HANDLE, ctypes.c_void_p, ctypes.POINTER(MBI), ctypes.c_size_t]
k32.VirtualQueryEx.restype = ctypes.c_size_t


def new_local(out):
    if "local" not in os.path.abspath(out).replace("\\", "/").split("/") or os.path.exists(out):
        sys.exit("output must be a new file under local/")


def open_checked(session):
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
    return s, h


def reader(h):
    def read(addr, size):
        b = ctypes.create_string_buffer(size)
        got = ctypes.c_size_t()
        if not k32.ReadProcessMemory(h, ctypes.c_void_p(addr), b, size, ctypes.byref(got)) or got.value != size:
            raise OSError(f"read {addr:x}")
        return b.raw
    return read


def vehicles(manager_text):
    found = re.findall(
        r'"index":\s*(\d+),\s*"vehicle":\s*"[0-9a-f]+",\s*"body":\s*"([0-9a-f]+)",\s*"kind":\s*\d+,\s*"grid_index":\s*(\d+)',
        manager_text)
    return [(int(i), int(g), int(b, 16)) for i, b, g in found]


def car_links(read, car):
    ptr = lambda a: struct.unpack("<I", read(a, 4))[0]
    wrapper = ptr(car + 0xDB0)
    physical = ptr(wrapper + 8)
    if not 0x10000 < physical < 0x7FFE0000:
        sys.exit(f"car {car:x} links invalid")
    return wrapper, physical, ptr(physical + 0x32C)


def discover(session, manager, out):
    new_local(out)
    s, h = open_checked(session)
    read = reader(h)
    ptr = lambda a: struct.unpack("<I", read(a, 4))[0]
    hits = []
    addr = 0x10000
    m = MBI()
    pattern = struct.pack("<I", VIEW_VTABLE)
    while addr < 0x7FFE0000 and k32.VirtualQueryEx(h, ctypes.c_void_p(addr), ctypes.byref(m), ctypes.sizeof(m)):
        base, size = m.BaseAddress or 0, m.RegionSize
        if m.State == 0x1000 and m.Protect in (0x04, 0x40) and size <= 0x10000000:
            for off in range(0, size, 0x100000):
                n = min(0x100000, size - off)
                try:
                    chunk = read(base + off, n)
                except OSError:
                    continue
                i = chunk.find(pattern)
                while i != -1:
                    if i % 4 == 0:
                        hits.append(base + off + i)
                    i = chunk.find(pattern, i + 4)
        addr = base + size
    drivers = []
    for hit in hits:
        driver = hit - 0x2DD0
        try:
            own = ptr(driver - 0x40)
            speed = struct.unpack("<f", read(own + 0xD0, 4))[0]
            centre = struct.unpack("<3f", read(own + 0x20, 12))
        except OSError:
            continue
        if 0 <= speed < 150 and all(abs(c) < 1e5 for c in centre):
            drivers.append((driver, own, centre))
    cars = vehicles(open(manager, encoding="utf-8-sig", errors="replace").read())
    poses = []
    for index, grid, car in cars:
        wrapper, physical, world = car_links(read, car)
        pose = struct.unpack("<16f", read(physical + 0x1D0, 0x40))
        poses.append((index, grid, car, physical, world, pose[12:15]))
    rows = []
    for driver, own, centre in drivers:
        best = min(poses, key=lambda p: sum((a - b) ** 2 for a, b in zip(p[5], centre)))
        gap = sum((a - b) ** 2 for a, b in zip(best[5], centre)) ** 0.5
        vt = ptr(own)
        rows.append({
            "driver": f"{driver:x}", "own": f"{own:x}", "centre": centre,
            "car": f"{best[2]:x}", "car_index": best[0], "grid_index": best[1], "match_m": gap,
            "own_vtable": f"{vt:x}", "own_vfunc_10": f"{ptr(vt + 0x10):x}", "own_vfunc_1c": f"{ptr(vt + 0x1C):x}",
            "own_dump": read(own, 0x300).hex(),
        })
    world = poses[0][4] if poses else 0
    meta = {"pid": s["Pid"], "utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
            "hits": len(hits), "drivers": rows, "world": f"{world:x}", "wheel_array": None}
    with open(out, "x", encoding="utf-8") as f:
        json.dump(meta, f, indent=1)
    print(json.dumps({"hits": len(hits), "drivers": len(rows),
                      "matches": [(r["driver"], r["car_index"], round(r["match_m"], 2)) for r in rows]}))


def capture(session, disc, seconds, out):
    new_local(out)
    if not 0.5 <= seconds <= 60:
        sys.exit("0.5..60 s")
    s, h = open_checked(session)
    read = reader(h)
    ptr = lambda a: struct.unpack("<I", read(a, 4))[0]
    d = json.load(open(disc, encoding="utf-8"))
    if d["pid"] != s["Pid"]:
        sys.exit("discovery is from another process")
    # wheel_array: {"pointer": offset of the array pointer in the car state, or
    # "inline": offset of the inline array, "count": n}; set by hand after
    # resolving vfunc +0x10/+0x1C statically.
    wheels = d.get("wheel_array")
    drv = []
    for r in d["drivers"]:
        driver, own, car = int(r["driver"], 16), int(r["own"], 16), int(r["car"], 16)
        if ptr(driver + 0x2DD0) != VIEW_VTABLE or ptr(driver - 0x40) != own:
            sys.exit("driver layout changed")
        wrapper, physical, world = car_links(read, car)
        drv.append((driver, own, car, physical, world))
    world = drv[0][4]
    rows = 0
    last = None
    end = time.perf_counter() + seconds
    with open(out, "x", encoding="ascii") as f:
        f.write(f"#AB01\tpid={s['Pid']}\tdrivers={len(drv)}\twheels={json.dumps(wheels)}\n")
        for i, (driver, own, car, physical, _) in enumerate(drv):
            f.write(f"#driver\t{i}\t{driver:x}\t{own:x}\t{car:x}\t{physical:x}\n")
        while time.perf_counter() < end and rows < 30000:
            t = time.perf_counter()
            parts = []
            phys = []
            for driver, own, car, physical, _ in drv:
                req = read(driver + 0x2CE0 + 0x20, 0x18)
                view = read(driver + 0x2DD0 + 0x68, 8)
                beh = read(driver + 0x2DC0, 4)
                cs = read(own + 0x20, 0x10) + read(own + 0xD0, 4) + read(own + 0x144, 4)
                pub = read(car + 0xE1C, 0x18) + read(car + 0x196C, 4)
                p = read(physical + 0x1D0, 0x60)
                phys.append(p)
                wb = b""
                if wheels:
                    n = int(wheels["count"])
                    a = ptr(own + int(wheels["pointer"], 16)) if "pointer" in wheels else own + int(wheels["inline"], 16)
                    wb = read(a, 4 * n)
                full = read(driver + 0x2CE0, 0xC8)
                extra = b"".join(read(driver + int(o, 16), int(n)) for o, n in d.get("extra", []))
                parts.append((req + view + beh + cs + pub + wb + full + extra).hex())
            key = b"".join(phys)
            if key != last:
                last = key
                w = read(world + 0x7C4, 8)
                f.write(f"{t:.6f}\t{w.hex()}\t" + "\t".join(parts) + "\n")
                rows += 1
            time.sleep(0.0005)
    print(json.dumps({"rows": rows, "drivers": len(drv), "out": out}))


def probe(session, disc, seconds, out):
    """Dump whole driver objects (driver-0x40 .. +0x4FA0) of cars that are part
    braking (vehicle +E20 in 0.2..0.9), with the vehicle's +E1C/+E20 at the
    same moment, at most 3 dumps per car."""
    new_local(out)
    if not 0.5 <= seconds <= 120:
        sys.exit("0.5..120 s")
    s, h = open_checked(session)
    read = reader(h)
    ptr = lambda a: struct.unpack("<I", read(a, 4))[0]
    d = json.load(open(disc, encoding="utf-8"))
    if d["pid"] != s["Pid"]:
        sys.exit("discovery is from another process")
    drv = [(int(r["driver"], 16), int(r["car"], 16)) for r in d["drivers"]]
    for driver, _ in drv:
        if ptr(driver + 0x2DD0) != VIEW_VTABLE:
            sys.exit("driver layout changed")
    dumps = {}
    end = time.perf_counter() + seconds
    with open(out, "xb") as f:
        while time.perf_counter() < end and sum(dumps.values()) < 3 * len(drv):
            for i, (driver, car) in enumerate(drv):
                if dumps.get(i, 0) >= 3:
                    continue
                pub = read(car + 0xE1C, 8)
                brake = struct.unpack_from("<f", pub, 4)[0]
                if 0.2 < brake < 0.9:
                    body = read(driver - 0x40, 0x4FE0)
                    after = read(car + 0xE1C, 8)
                    f.write(struct.pack("<II", i, driver) + pub + after + body)
                    dumps[i] = dumps.get(i, 0) + 1
            time.sleep(0.002)
    print(json.dumps({"dumps": dumps, "out": out}))


def main():
    if len(sys.argv) == 5 and sys.argv[1] == "discover":
        discover(sys.argv[2], sys.argv[3], sys.argv[4])
    elif len(sys.argv) == 6 and sys.argv[1] == "probe":
        probe(sys.argv[2], sys.argv[3], float(sys.argv[4]), sys.argv[5])
    elif len(sys.argv) == 6 and sys.argv[1] == "capture":
        capture(sys.argv[2], sys.argv[3], float(sys.argv[4]), sys.argv[5])
    else:
        sys.exit(__doc__)


main()
