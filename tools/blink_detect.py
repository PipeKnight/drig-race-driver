"""Blink detector for consecutive window frames: large on-screen regions whose brightness jumps for one or a few frames and returns (row 115).

Usage: py -I tools/blink_detect.py FRAMES.rgb OUT_DIR [--tiles 32x18] [--min-tiles 4] [--max-len 3]

FRAMES.rgb is written by grid-app with `[diagnostics] window_frames = START,SECONDS`
(`GRID_WINDOW_FRAMES`, frame_pacing.rs): consecutive frames of the interactive window, each a
16-byte header (u32 frame, f32 real seconds, u16 width, u16 height, u32 full width) and the frame
box-averaged by 8 as RGB bytes.

Per frame the script computes the mean luminance of a grid of tiles. A tile *jumps* at frames f..f+n-1
(n <= --max-len) when it differs from frame f-1 by more than T, every frame of the run stays on the
same side, and frame f+n is back within T/2 of frame f-1. An event is a 4-connected region of at
least --min-tiles jumping tiles, at least two wide and two high, all jumping the same way with the same
start and length. `structure` is the lower correlation of the region's pixels with the frame before
and the frame after: near 1 when the same picture only got brighter or darker, lower when something
moved through it; --structure S keeps only events at or above S. T is calibrated on the run: 6 times
the median absolute one-frame tile change, and at least 6 (of 255). The camera's smooth motion
changes tiles gradually; a blink changes a region and changes it back.

Prints a verdict: the frames read, the threshold, the number of events and, per event, the frame
number, real time, length, tile box, mean jump, and a crop (previous | during | next frames,
cropped to the box with a margin, scaled x2) written to OUT_DIR. The last line splits the count into
events above --upper-rows (scenery; lighter ones are what a lost shadow gives) and below (in the chase
view the car, its tyre smoke and the road markings under it move fast in screen space, which a
brightness test cannot tell from a blink; check those crops). Exit 1 when events are found.
"""
import argparse
import struct
import sys
import zlib
from pathlib import Path


def frames(path):
    data = Path(path).read_bytes()
    pos = 0
    while pos + 16 <= len(data):
        frame, t, w, h, full = struct.unpack_from('<IfHHI', data, pos)
        size = w * h * 3
        if pos + 16 + size > len(data):
            break
        yield frame, t, w, h, data[pos + 16:pos + 16 + size]
        pos += 16 + size


def tiles(w, h, rgb, tx, ty):
    sums = [0.0] * (tx * ty)
    counts = [0] * (tx * ty)
    for y in range(h):
        row = (y * ty // h) * tx
        base = y * w * 3
        for x in range(w):
            i = base + x * 3
            lum = 0.2126 * rgb[i] + 0.7152 * rgb[i + 1] + 0.0722 * rgb[i + 2]
            k = row + x * tx // w
            sums[k] += lum
            counts[k] += 1
    return [s / max(c, 1) for s, c in zip(sums, counts)]


def structure(before, during, region, tx, ty):
    """Correlation of the region's pixel luminances in two frames: near 1 when the same picture only got
    brighter or darker (a lost shadow, a lighting jump), low when something moved through it."""
    w, h, a = before
    _, _, b = during
    tiles_in = set(region)
    xs, ys = [], []
    for y in range(h):
        row = (y * ty // h) * tx
        for x in range(w):
            if row + x * tx // w in tiles_in:
                i = (y * w + x) * 3
                xs.append(0.2126 * a[i] + 0.7152 * a[i + 1] + 0.0722 * a[i + 2])
                ys.append(0.2126 * b[i] + 0.7152 * b[i + 1] + 0.0722 * b[i + 2])
    n = len(xs)
    mx, my = sum(xs) / n, sum(ys) / n
    sxy = sum((x - mx) * (y - my) for x, y in zip(xs, ys))
    sxx = sum((x - mx) ** 2 for x in xs)
    syy = sum((y - my) ** 2 for y in ys)
    return sxy / (sxx * syy) ** 0.5 if sxx > 0 and syy > 0 else 0.


def write_png(path, w, h, rows):
    def chunk(kind, body):
        return struct.pack('>I', len(body)) + kind + body + struct.pack('>I', zlib.crc32(kind + body) & 0xffffffff)
    raw = b''.join(b'\0' + bytes(r) for r in rows)
    Path(path).write_bytes(b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', w, h, 8, 2, 0, 0, 0))
                           + chunk(b'IDAT', zlib.compress(raw, 9)) + chunk(b'IEND', b''))


def crop(shots, box, tx, ty, path, scale=2):
    """Side by side crops of `shots` (w, h, rgb) to the tile box with one tile of margin."""
    x0, y0, x1, y1 = box
    parts = []
    for w, h, rgb in shots:
        xa, xb = max(0, (x0 - 1) * w // tx), min(w, (x1 + 2) * w // tx)
        ya, yb = max(0, (y0 - 1) * h // ty), min(h, (y1 + 2) * h // ty)
        rows = []
        for y in range(ya, yb):
            line = bytearray()
            for x in range(xa, xb):
                i = (y * w + x) * 3
                line += rgb[i:i + 3] * scale
            rows += [line] * scale
        parts.append(rows)
    height = min(len(p) for p in parts)
    gap = bytes([255, 0, 255]) * 4
    joined = [gap.join(p[y] for p in parts) for y in range(height)]
    write_png(path, len(joined[0]) // 3, height, joined)


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument('frames')
    ap.add_argument('out')
    ap.add_argument('--tiles', default='32x18')
    ap.add_argument('--min-tiles', type=int, default=4)
    ap.add_argument('--max-len', type=int, default=3)
    ap.add_argument('--k', type=float, default=4.0, help='a jump must exceed K times the local median change of its tile')
    ap.add_argument('--upper-rows', type=int, default=11,
                    help='tile rows above which an event counts as scenery (the chase car sits below)')
    ap.add_argument('--structure', type=float, default=0.,
                    help='minimum correlation of the region with the frames before and after (0: report only)')
    ap.add_argument('--window', type=int, default=31, help='frames over which the local change of a tile is taken')
    args = ap.parse_args()
    tx, ty = (int(v) for v in args.tiles.split('x'))
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    meta, lum, shots = [], [], []
    # Captures can arrive out of order; the header carries the frame number.
    for frame, t, w, h, rgb in sorted(frames(args.frames), key=lambda f: f[0]):
        meta.append((frame, t))
        lum.append(tiles(w, h, rgb, tx, ty))
        shots.append((w, h, rgb))
    n = len(lum)
    if n < 3:
        sys.exit(f'FAIL: only {n} frames in {args.frames}')
    gaps = sum(1 for i in range(1, n) if meta[i][0] != meta[i - 1][0] + 1)
    tn = tx * ty
    steps = [[0.0] * n for _ in range(tn)]  # steps[k][i]: |tile k at frame i - frame i-1|
    for i in range(1, n):
        for k in range(tn):
            steps[k][i] = abs(lum[i][k] - lum[i - 1][k])
    flat = sorted(v for k in range(tn) for v in steps[k][1:])
    floor = max(4.0, 4 * flat[len(flat) // 2])
    half = args.window // 2

    def noise(k, f):
        """The tile's own typical one-frame change around frame f (median; a blink does not move it)."""
        window = sorted(steps[k][max(1, f - half):min(n, f + half + 1)])
        return window[len(window) // 2]

    busy_limit = 0.25
    events = []
    skipped = 0
    for f in range(1, n - 1):
        moving = sum(1 for k in range(tn) if steps[k][f] > floor) / tn
        if moving > busy_limit:
            skipped += 1  # a camera cut or a fast pan: not smooth motion
            continue
        for length in range(1, args.max_len + 1):
            if f + length >= n:
                break
            jumping = {}
            for k in range(tn):
                t = max(floor, args.k * noise(k, f))
                before, after = lum[f - 1][k], lum[f + length][k]
                if abs(after - before) > t / 2:
                    continue
                run = [lum[f + j][k] - before for j in range(length)]
                if all(d > t for d in run):
                    jumping[k] = 1
                elif all(d < -t for d in run):
                    jumping[k] = -1
            seen = set()
            for k in jumping:
                if k in seen:
                    continue
                region, stack = [], [k]
                seen.add(k)
                while stack:
                    c = stack.pop()
                    region.append(c)
                    x, y = c % tx, c // tx
                    for nx, ny in ((x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)):
                        m = ny * tx + nx
                        if 0 <= nx < tx and 0 <= ny < ty and jumping.get(m) == jumping[k] and m not in seen:
                            seen.add(m)
                            stack.append(m)
                xs, ys = [c % tx for c in region], [c // tx for c in region]
                # A large object covers an area; a one-tile band is an edge moved by the camera's bob.
                if len(region) >= args.min_tiles and max(xs) > min(xs) and max(ys) > min(ys):
                    jump = sum(lum[f][c] - lum[f - 1][c] for c in region) / len(region)
                    cover = min(structure(shots[f - 1], shots[f], region, tx, ty),
                                structure(shots[f + length], shots[f], region, tx, ty))
                    if cover >= args.structure:
                        events.append((f, length, (min(xs), min(ys), max(xs), max(ys)), len(region), jump, cover))
    # Keep the shortest explanation of each start frame and region box.
    events.sort(key=lambda e: (e[0], e[1]))
    kept, starts = [], set()
    for e in events:
        key = (e[0], e[2])
        if key not in starts:
            starts.add(key)
            kept.append(e)
    print(f'frames {n} ({meta[0][0]}..{meta[-1][0]}, {meta[-1][1] - meta[0][1]:.1f} s, {gaps} gaps), tiles {tx}x{ty}, '
          f'floor {floor:.1f} (median step {flat[len(flat) // 2]:.2f}), K {args.k}, '
          f'{skipped} frames skipped as non-smooth (over {busy_limit:.0%} of tiles changing)')
    for i, (f, length, box, size, jump, cover) in enumerate(kept):
        path = out / f'blink-{i:03}-frame{meta[f][0]}.png'
        crop([shots[f - 1], shots[f], shots[f + length]], box, tx, ty, path)
        print(f'  frame {meta[f][0]} t={meta[f][1]:.3f}s len={length} tiles={size} box={box} jump={jump:+.1f} structure={cover:.2f} crop={path}')
    upper = [e for e in kept if e[2][3] < args.upper_rows]
    lighter = sum(1 for e in upper if e[4] > 0)
    print(f'{"FAIL" if kept else "PASS"}: {len(kept)} blink events; {len(upper)} above row {args.upper_rows} '
          f'(scenery: {lighter} lighter, {len(upper) - lighter} darker), {len(kept) - len(upper)} lower '
          f'(chase view: the car, its smoke and the road under it)')
    sys.exit(1 if kept else 0)


if __name__ == '__main__':
    main()
