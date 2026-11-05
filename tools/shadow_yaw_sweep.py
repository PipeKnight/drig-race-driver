"""Tree shadow coverage of a fixed ground patch against the camera's yaw (row 122): it must not change with the view.

Usage: py -I tools/shadow_yaw_sweep.py EXE OUT_DIR [--target X,Y,Z] [--radius 18] [--height 14]
       [--yaws 0,45,...] [--size 2560x1369] [--window 0.16] [--tolerance 0.05] [--reuse]

Renders one offscreen frame per yaw (`grid-app --route tracks/usa/washington/route_0 --scene donington
--resolution SIZE --camera EYE --look-at TARGET --render PNG`, GRID_AUDIO=off), the camera on a circle of
RADIUS around TARGET at HEIGHT above it, so TARGET stays at the image centre. In the central window (WINDOW
of the width, as a square) it measures the ground's mean luminance and its shadowed fraction: pixels below
0.6 of the lit level, the 90th-percentile luminance of all yaws' windows together (one reference, so a
window wholly in shadow counts as shadowed). The sun and the trees do not move, so the shadowed fraction
must stay within TOLERANCE across the yaws; the verdict prints both per yaw, the spread, and PASS or FAIL.
--skip YAWS leaves out views where something else covers the patch (say why in the record); --reuse
measures existing PNGs in OUT_DIR without rendering. Pure Python (no numpy).
"""
import argparse
import math
import os
import struct
import subprocess
import sys
import zlib
from pathlib import Path


def read_png(path):
    """8-bit RGB or RGBA PNG -> (width, height, channels, rows as bytearrays)."""
    data = Path(path).read_bytes()
    pos, idat, w = 8, b'', 0
    while pos < len(data):
        length, kind = struct.unpack_from('>I4s', data, pos)
        body = data[pos + 8:pos + 8 + length]
        if kind == b'IHDR':
            w, h, depth, colour = struct.unpack_from('>IIBB', body)
            if depth != 8 or colour not in (2, 6):
                sys.exit(f'FAIL: {path}: only 8-bit RGB/RGBA PNGs')
            ch = 3 if colour == 2 else 4
        elif kind == b'IDAT':
            idat += body
        pos += 12 + length
    raw = zlib.decompress(idat)
    stride = w * ch
    rows, prev = [], bytearray(stride)
    for y in range(h):
        f = raw[y * (stride + 1)]
        line = bytearray(raw[y * (stride + 1) + 1:(y + 1) * (stride + 1)])
        if f == 1:
            for i in range(ch, stride):
                line[i] = (line[i] + line[i - ch]) & 255
        elif f == 2:
            for i in range(stride):
                line[i] = (line[i] + prev[i]) & 255
        elif f == 3:
            for i in range(stride):
                left = line[i - ch] if i >= ch else 0
                line[i] = (line[i] + ((left + prev[i]) >> 1)) & 255
        elif f == 4:
            for i in range(stride):
                a = line[i - ch] if i >= ch else 0
                b, c = prev[i], prev[i - ch] if i >= ch else 0
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                line[i] = (line[i] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 255
        rows.append(line)
        prev = line
    return w, h, ch, rows


def window_luminance(path, window):
    w, h, ch, rows = read_png(path)
    half = int(w * window / 2)
    lum = []
    for y in range(h // 2 - half, h // 2 + half):
        row = rows[y]
        for x in range(w // 2 - half, w // 2 + half):
            i = x * ch
            lum.append(0.2126 * row[i] + 0.7152 * row[i + 1] + 0.0722 * row[i + 2])
    return (w, h), lum


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument('exe')
    ap.add_argument('out')
    ap.add_argument('--target', default='398,1,359')
    ap.add_argument('--radius', type=float, default=18.)
    ap.add_argument('--height', type=float, default=14.)
    ap.add_argument('--yaws', default='0,45,90,135,180,225,270,315')
    ap.add_argument('--size', default='2560x1369')
    ap.add_argument('--window', type=float, default=0.16)
    ap.add_argument('--tolerance', type=float, default=0.05)
    ap.add_argument('--reuse', action='store_true')
    ap.add_argument('--skip', default='', help='yaws to leave out (an occluder covers the patch)')
    args = ap.parse_args()
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    tx, ty, tz = (float(v) for v in args.target.split(','))
    env = dict(os.environ, GRID_AUDIO='off')
    skip = {float(v) for v in args.skip.split(',') if v.strip()}
    windows = []
    for yaw in (float(v) for v in args.yaws.split(',') if float(v) not in skip):
        png = out / f'yaw-{yaw:05.1f}.png'
        if not args.reuse:
            png.unlink(missing_ok=True)
            a = math.radians(yaw)
            eye = f'{tx + args.radius * math.sin(a):.3f},{ty + args.height:.3f},{tz + args.radius * math.cos(a):.3f}'
            cmd = [str(Path(args.exe).resolve()), '--route', 'tracks/usa/washington/route_0', '--scene', 'donington',
                   '--resolution', args.size, '--camera', eye, '--look-at', args.target, '--render', str(png)]
            with open(out / f'yaw-{yaw:05.1f}.log', 'w') as log:
                subprocess.run(cmd, stdout=log, stderr=subprocess.STDOUT, env=env, timeout=300)
        if not png.exists():
            print(f'  yaw {yaw:5.1f}: no frame (see {png.with_suffix(".log")})')
            continue
        size, lum = window_luminance(png, args.window)
        windows.append((yaw, size, lum, png))
    if windows:
        pooled = sorted(v for w in windows for v in w[2])
        lit = pooled[int(len(pooled) * 0.9)]
    results = []
    for yaw, size, lum, png in windows:
        mean, shadowed = sum(lum) / len(lum), sum(1 for v in lum if v < 0.6 * lit) / len(lum)
        results.append((yaw, mean, shadowed))
        print(f'  yaw {yaw:5.1f}: {size[0]}x{size[1]} mean {mean:6.1f} shadowed {shadowed:.3f}  {png}')
    if len(results) < 2:
        sys.exit('FAIL: fewer than two frames')
    fractions = [r[2] for r in results]
    means = [r[1] for r in results]
    spread = max(fractions) - min(fractions)
    verdict = 'PASS' if spread <= args.tolerance else 'FAIL'
    print(f'{verdict}: shadowed fraction {min(fractions):.3f}..{max(fractions):.3f} (spread {spread:.3f}, '
          f'tolerance {args.tolerance}), mean luminance {min(means):.1f}..{max(means):.1f}, lit level {lit:.0f}, '
          f'{len(results)} yaws' + (f' (skipped {sorted(skip)})' if skip else ''))
    sys.exit(0 if verdict == 'PASS' else 1)


if __name__ == '__main__':
    main()
