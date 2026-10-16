"""Audit private offscreen motion frames against their request-clock metadata.

Not a display-FPS measurement or a perceptual quality score. Uses only the
standard library; original images and derived metadata must remain private.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
import re
import struct

ROW = re.compile(r"frame=(\d+) tick=(\d+) fixed_remainder=([^ ]+) frame_seconds=([^ ]+) phases=\[(.*)\] spins=\[(.*)\]")
DELTA = struct.unpack("f", struct.pack("f", 0.016))[0]

def audit(directory, fps, changing=False):
    rows = (directory / "frames.txt").read_text(encoding="utf-8").splitlines()[1:]
    files = sorted(directory.glob("*.png"))
    if not 2 <= len(rows) <= 300 or len(rows) != len(files):
        raise ValueError("frame/metadata count mismatch or unbounded capture")
    times, ticks, hashes = [], [], []
    for index, (row, file) in enumerate(zip(rows, files)):
        m = ROW.fullmatch(row)
        if m is None or int(m[1]) != index or file.name != f"{index:04}.png":
            raise ValueError("non-consecutive frame metadata or filenames")
        tick, remainder, seconds = int(m[2]), float(m[3]), float(m[4])
        phase = [float(x) for x in m[5].split(",")]
        spin = [float(x) for x in m[6].split(",")]
        if len(phase) != 4 or len(spin) != 4 or not all(map(math.isfinite, [remainder, seconds, *phase, *spin])):
            raise ValueError("invalid wheel metadata")
        if not 0 <= remainder < DELTA + 1e-6 or abs(seconds - 1 / fps) > 2e-6:
            raise ValueError("invalid request clock or fixed remainder")
        times.append(tick * DELTA + remainder)
        ticks.append(tick)
        hashes.append(hashlib.sha256(file.read_bytes()).hexdigest())
    error = max(abs(b - a - 1 / fps) for a, b in zip(times, times[1:]))
    if error > 2e-6:
        raise ValueError("request sequence skipped time or simulation stopped")
    unique = len(set(hashes))
    if changing and unique != len(files):
        raise ValueError("moving sequence contains repeated images")
    return {"directory": str(directory), "virtual_fps": fps, "frames": len(files),
            "unique_images": unique, "ticks": [ticks[0], ticks[-1]],
            "request_seconds": [times[0], times[-1]], "maximum_request_dt_error": error,
            "sha256": hashes, "scope": "request-frame clock; asynchronous GPU readback; not monitor FPS"}

def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("directory", type=Path)
    p.add_argument("--fps", type=int, required=True)
    p.add_argument("--require-changing-images", action="store_true")
    p.add_argument("--output", type=Path, required=True)
    a = p.parse_args()
    if not 30 <= a.fps <= 300:
        p.error("virtual FPS must be30..300")
    if "local" not in a.directory.resolve().parts or "local" not in a.output.resolve().parts:
        p.error("game-derived evidence must stay beneath local/")
    result = audit(a.directory, a.fps, a.require_changing_images)
    with a.output.open("x", encoding="utf-8") as f:
        json.dump(result, f, indent=2)
    print(f"PASS frames={result['frames']} unique={result['unique_images']} ticks={result['ticks']} max_dt_error={result['maximum_request_dt_error']:.3g}s")

if __name__ == "__main__":
    main()
