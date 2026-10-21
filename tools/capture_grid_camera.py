"""Read-only camera sampler for the original GRID research copy (docs/CAMERA_VIEWS.md).

    python -I tools/capture_grid_camera.py scan SESSION.json CAR_HEX local/OUT.json
    python -I tools/capture_grid_camera.py capture SESSION.json SCAN.json SECONDS local/OUT.tsv

`scan` walks the committed private read-write memory (bounded to 512 MiB) for
in-race camera objects (Chase vtable 0xD00278, Mounted 0xD00008, Head
0xD00140) whose target (+0x64, vtable 0xCFF640) points at the car CAR_HEX
(target +8), then for each object at the view-manager entries that hold it
(entry +0 = the object, record at entry +0x10; 0x82D0B0). It checks that the
record's +0x30 is the object's FOV in radians.

`capture` writes one row whenever the car's physical block changes: QPC
seconds, world +0x7C4 (8 bytes: +0x7C8 is the step), car +0x1640 (16: speed
+0x1648), car +0xD230 (16: state +0xD234), the four wheels' +0x40..+0x4F,
physical +0x1D0 (0x60: pose, angular and linear velocity), car +0x10 (0x40:
the model-frame matrix of 0x9AE3A0; from header version CM02), and per view the
record +0x00..+0x14F (orientation +0x0, eye +0x10, FOV +0x30) and the object
+0x190 (heave) and +0x220..+0x23F (spin blend, held forward).

The process is opened with PROCESS_QUERY_INFORMATION | PROCESS_VM_READ only
(VirtualQueryEx needs the first), after the identity checks of
tools/capture_grid_field.py. Nothing is written to the game.
"""
import ctypes
import ctypes.wintypes as wt
import datetime
import hashlib
import json
import os
import struct
import sys
import time

PATH = r"C:\Users\debros\Tools\grid-research\grid-compat\grid.exe"
SHA = "b1da1b2d9469fb0c973dbaaad29b230c1567f212c54de849326f80b4aece34a6"
VTABLES = {0xD00278: "chase", 0xD00008: "mounted", 0xD00140: "head"}
FOV_OFFSET = {"chase": 0x118, "mounted": 0x110, "head": None}
TARGET_VTABLE = 0xCFF640
SCAN_LIMIT = 512 << 20

k32 = ctypes.WinDLL("kernel32", use_last_error=True)
k32.OpenProcess.restype = wt.HANDLE
k32.ReadProcessMemory.argtypes = [wt.HANDLE, wt.LPCVOID, wt.LPVOID, ctypes.c_size_t, ctypes.POINTER(ctypes.c_size_t)]
k32.QueryFullProcessImageNameW.argtypes = [wt.HANDLE, wt.DWORD, wt.LPWSTR, ctypes.POINTER(wt.DWORD)]
k32.GetProcessTimes.argtypes = [wt.HANDLE] + [ctypes.POINTER(wt.FILETIME)] * 4


class Region(ctypes.Structure):
    _fields_ = [("BaseAddress", ctypes.c_void_p), ("AllocationBase", ctypes.c_void_p),
                ("AllocationProtect", wt.DWORD), ("RegionSize", ctypes.c_size_t),
                ("State", wt.DWORD), ("Protect", wt.DWORD), ("Type", wt.DWORD)]


k32.VirtualQueryEx.argtypes = [wt.HANDLE, wt.LPCVOID, ctypes.POINTER(Region), ctypes.c_size_t]
k32.VirtualQueryEx.restype = ctypes.c_size_t


def open_game(session):
    s = json.load(open(session, encoding="utf-8-sig"))
    if s["Path"] != PATH or s["ExeSHA256"] != SHA:
        sys.exit("session is not the research copy")
    if hashlib.sha256(open(PATH, "rb").read()).hexdigest() != SHA:
        sys.exit("executable hash changed")
    h = k32.OpenProcess(0x0400 | 0x0010, False, int(s["Pid"]))
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
        if not 0x10000 <= addr < 0x7FFE0000 or not 0 < size <= (16 << 20):
            raise OSError(f"read outside bound {addr:x}")
        b = ctypes.create_string_buffer(size)
        got = ctypes.c_size_t()
        if not k32.ReadProcessMemory(h, ctypes.c_void_p(addr), b, size, ctypes.byref(got)) or got.value != size:
            raise OSError(f"read {addr:x}")
        return b.raw
    return read


def regions(h):
    """Committed private read-write regions, at most SCAN_LIMIT bytes."""
    addr, total = 0x10000, 0
    r = Region()
    while addr < 0x7FFE0000 and k32.VirtualQueryEx(h, ctypes.c_void_p(addr), ctypes.byref(r), ctypes.sizeof(r)):
        base, size = r.BaseAddress or 0, r.RegionSize
        if r.State == 0x1000 and r.Type == 0x20000 and r.Protect == 0x04 and size <= (64 << 20):
            if total + size > SCAN_LIMIT:
                sys.exit("scan limit exceeded")
            total += size
            yield base, size
        addr = base + size


def scan(session, car_hex, out):
    if "local" not in os.path.abspath(out).replace("\\", "/").split("/") or os.path.exists(out):
        sys.exit("output must be a new file under local/")
    s, h = open_game(session)
    read = reader(h)
    ptr = lambda a: struct.unpack("<I", read(a, 4))[0]
    f32 = lambda a: struct.unpack("<f", read(a, 4))[0]
    objects, words = [], {}
    for base, size in regions(h):
        try:
            data = read(base, size)
        except OSError:
            continue
        for i in range(0, size - 3, 4):
            w = struct.unpack_from("<I", data, i)[0]
            if w in VTABLES:
                objects.append((base + i, VTABLES[w]))
        words[base] = data
    if car_hex == "auto":
        # The car every in-race camera targets (CAR_HEX "auto": exactly one).
        cars = set()
        for obj, _ in objects:
            try:
                target = ptr(obj + 0x64)
                if 0x10000 < target < 0x7FFE0000 and ptr(target) == TARGET_VTABLE:
                    cars.add(ptr(target + 8))
            except OSError:
                pass
        if len(cars) != 1:
            sys.exit(f"camera targets name {len(cars)} cars: {sorted(hex(c) for c in cars)}")
        car = cars.pop()
    else:
        car = int(car_hex, 16)
    views = []
    for obj, kind in objects:
        try:
            target = ptr(obj + 0x64)
            if not 0x10000 < target < 0x7FFE0000 or ptr(target) != TARGET_VTABLE or ptr(target + 8) != car:
                continue
        except OSError:
            continue
        needle = struct.pack("<I", obj)
        entries = []
        for base, data in words.items():
            at = data.find(needle)
            while at != -1:
                if at % 4 == 0 and base + at != obj:
                    entries.append(base + at)
                at = data.find(needle, at + 1)
        for entry in entries:
            record = entry + 0x10
            try:
                fov = f32(record + 0x30)
            except OSError:
                continue
            want = FOV_OFFSET[kind]
            expected = f32(obj + want) * 0.017453292 if want else None
            views.append({"kind": kind, "object": f"{obj:x}", "entry": f"{entry:x}", "record": f"{record:x}",
                          "record_fov": fov, "object_fov_rad": expected,
                          "fov_match": expected is not None and abs(fov - expected) < 1e-4})
    wrapper = ptr(car + 0xDB0)
    physical = ptr(wrapper + 8)
    world = ptr(physical + 0x32C)
    result = {"pid": s["Pid"], "car": f"{car:x}", "physical": f"{physical:x}", "world": f"{world:x}",
              "objects": len(objects), "views": views}
    with open(out, "x", encoding="utf-8") as f:
        json.dump(result, f, indent=1)
    print(json.dumps(result, indent=1))


def capture(session, scan_path, seconds, out):
    if "local" not in os.path.abspath(out).replace("\\", "/").split("/") or os.path.exists(out):
        sys.exit("output must be a new file under local/")
    if not 0.5 <= seconds <= 60:
        sys.exit("0.5..60 s")
    s, h = open_game(session)
    read = reader(h)
    ptr = lambda a: struct.unpack("<I", read(a, 4))[0]
    sc = json.load(open(scan_path, encoding="utf-8"))
    if sc["pid"] != s["Pid"]:
        sys.exit("scan belongs to another process")
    car, physical, world = (int(sc[k], 16) for k in ("car", "physical", "world"))
    views = [(v["kind"], int(v["object"], 16), int(v["record"], 16)) for v in sc["views"] if v["fov_match"] or v["kind"] == "head"]
    if not views:
        sys.exit("no views in the scan")
    wrapper = ptr(car + 0xDB0)
    rows, last = 0, None
    end = time.perf_counter() + seconds
    with open(out, "x", encoding="ascii") as f:
        f.write(f"#CM02\tpid={s['Pid']}\tcar={car:x}\tphysical={physical:x}\tworld={world:x}\n")
        for kind, obj, record in views:
            f.write(f"#view\t{kind}\t{obj:x}\t{record:x}\n")
        while time.perf_counter() < end and rows < 40000:
            t = time.perf_counter()
            if ptr(car + 0xDB0) != wrapper or ptr(wrapper + 8) != physical:
                sys.exit("car ownership changed")
            p = read(physical + 0x1D0, 0x60)
            if p == last:
                time.sleep(0.0003)
                continue
            last = p
            parts = [read(world + 0x7C4, 8), read(car + 0x1640, 16), read(car + 0xD230, 16)]
            parts += [read(car + 0x1990 + i * 0x150 + 0x40, 16) for i in range(4)]
            parts.append(p)
            parts.append(read(car + 0x10, 0x40))
            for kind, obj, record in views:
                parts += [read(record, 0x150), read(obj + 0x190, 4), read(obj + 0x220, 0x20)]
            f.write(f"{t:.6f}\t" + "\t".join(x.hex() for x in parts) + "\n")
            rows += 1
    print(json.dumps({"rows": rows, "views": len(views), "out": out}))


def main():
    if len(sys.argv) == 5 and sys.argv[1] == "scan":
        scan(*sys.argv[2:])
    elif len(sys.argv) == 6 and sys.argv[1] == "capture":
        capture(sys.argv[2], sys.argv[3], float(sys.argv[4]), sys.argv[5])
    else:
        sys.exit(__doc__)


main()
