"""Decode original (tools/capture_grid_braking.py) and port
(grid-braking-replay) braking captures and summarise the braking phase
(docs/BRAKING_REFERENCE.md).

    python -I tools/analyze_braking.py CAPTURE.tsv [--rows]

Per row: time (original QPC seconds / port tick x 0.016), controls +E1C/+E20
(published throttle/brake) and +E3C/+E40 (requests), clutch +E34, lock-up
timer +E74, gear +196C, position, speed, per-wheel surface speed (+0x104 spin
x +0xE0 radius). Summary from the first row with brake request +E40 > 0.5:
speed, stopping distance (to < 0.5 m/s), time, locked-wheel rows (a wheel's
surface speed below 5 % of the car speed above 2 m/s), maximum +E74, and the
brake-request/published-brake pattern (modulation).
"""
import json
import math
import struct
import sys


def f(block, off):
    return struct.unpack_from("<f", block, off)[0]


def u(block, off):
    return struct.unpack_from("<I", block, off)[0]


def load(path):
    rows = []
    port = False
    for line in open(path, encoding="ascii"):
        if line.startswith("#"):
            port = line.startswith("#BR01port")
            continue
        p = line.rstrip("\n").split("\t")
        e00, b1140, b1550, b1960, wheels = (bytes.fromhex(x) for x in p[1:6])
        if port:
            t = int(p[0]) * 0.016
            pose = [float(x) for x in p[6].split(",")]
            vel = [float(x) for x in p[7].split(",")]
        else:
            t = float(p[0])
            phys = bytes.fromhex(p[6])
            pose = list(struct.unpack_from("<16f", phys, 0))
            vel = list(struct.unpack_from("<4f", phys, 0x50))
        speed = math.sqrt(sum(x * x for x in vel[:3]))
        surface = [f(wheels, i * 0x150 + 0x104) * f(wheels, i * 0x150 + 0xE0) for i in range(4)]
        rows.append({
            "t": t,
            "e1c": f(e00, 0x1C), "e20": f(e00, 0x20), "e34": f(e00, 0x34),
            "e3c": f(e00, 0x3C), "e40": f(e00, 0x40), "e74": f(e00, 0x74),
            "gear": u(b1960, 0xC), "aids": b1550[8:11].hex(), "abs": u(b1140, 0x7C),
            "params": [round(f(b1140, o), 4) for o in range(0x20, 0x3C, 4)],
            "pos": pose[12:15], "speed": speed, "surface": surface,
            "fwd": sum(a * b for a, b in zip(vel[:3], pose[8:11])),
        })
    return rows


def summary(rows):
    onset = next((i for i, r in enumerate(rows) if r["e40"] > 0.5), None)
    if onset is None:
        return {"rows": len(rows), "brake": None}
    r0 = rows[onset]
    stop = next((i for i in range(onset, len(rows)) if rows[i]["speed"] < 0.5 or rows[i]["fwd"] < 0), None)
    end = stop if stop is not None else len(rows) - 1
    dist = 0.0
    for a, b in zip(rows[onset:end], rows[onset + 1:end + 1]):
        dist += math.dist(a["pos"], b["pos"])
    locked = sum(1 for r in rows[onset:end + 1]
                 if r["speed"] > 2 and any(abs(s) < 0.05 * r["speed"] for s in r["surface"]))
    published = [round(r["e20"], 3) for r in rows[onset:end + 1]]
    changes = sum(1 for a, b in zip(published, published[1:]) if a != b)
    return {
        "rows": len(rows), "onset_t": round(r0["t"], 4), "v0": round(r0["speed"], 3),
        "gear0": r0["gear"], "aids": r0["aids"], "abs_mode": r0["abs"], "params": r0["params"],
        "stop_dist": round(dist, 3), "stop_time": round(rows[end]["t"] - r0["t"], 4),
        "stopped": stop is not None, "brake_rows": end - onset + 1, "locked_rows": locked,
        "max_e74": round(max(r["e74"] for r in rows[onset:end + 1]), 4),
        "published_brake_values": sorted(set(published)), "published_brake_changes": changes,
        "max_e34": round(max(r["e34"] for r in rows), 4),
        "e34_values": sorted({round(r["e34"], 4) for r in rows})[:10],
    }


if __name__ == "__main__":
    rows = load(sys.argv[1])
    print(json.dumps(summary(rows), indent=1))
    if "--rows" in sys.argv:
        for r in rows:
            print("%.4f thr=%.2f/%.2f brk=%.2f/%.2f clutch=%.2f e74=%.3f gear=%d v=%.3f surf=%s" % (
                r["t"], r["e3c"], r["e1c"], r["e40"], r["e20"], r["e34"], r["e74"], r["gear"], r["speed"],
                ",".join("%.2f" % s for s in r["surface"])))
