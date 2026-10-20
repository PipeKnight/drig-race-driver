"""Validate paired private HDR audit packets and compare bounded native-pixel ROIs.

Preview uses shared Reinhard+sRGB, not the game's AgX/colour grade. Raw evidence
is never changed. Ratios describe spatial contrast, not reconstruction quality.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct

import numpy as np
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
WIDTH, HEIGHT = 1024, 512
PACKET_BYTES = 48 + WIDTH * HEIGHT * 16


def read_packet(path, stage):
    if path.stat().st_size != PACKET_BYTES:
        raise ValueError("Unexpected v1 bounded colour packet length")
    raw = path.read_bytes()
    h = struct.unpack_from("<12I", raw)
    version, frame, tick, role, width, height, x, y, w, z, r0, r1 = h
    if version != 1 or role != stage or not 1 <= tick <= 1_000_000 or r0 or r1:
        raise ValueError("Invalid colour packet identity/version/reserved fields")
    if not (0 < w <= WIDTH and 0 < z <= HEIGHT and x + w <= width and y + z <= height):
        raise ValueError("Invalid bounded colour dimensions/origin")
    pixels = w * z
    rgba = np.frombuffer(raw, "<f4", pixels * 4, 48).reshape(z, w, 4)
    if not np.isfinite(rgba).all():
        raise ValueError("Nonfinite HDR samples")
    if not np.any(np.abs(rgba[:, :, :3]) > 1e-8):
        raise ValueError("Blank HDR colour packet")
    if any(raw[48 + pixels * 16:]):
        raise ValueError("Nonzero reserved padding")
    return h, rgba, hashlib.sha256(raw).hexdigest()


def roi_stats(rgb):
    colour = rgb.astype(np.float64)
    luma = colour @ np.array([0.2126, 0.7152, 0.0722])
    p = np.pad(luma, 1, mode="reflect")
    smooth = (p[:-2, :-2] + 2*p[:-2, 1:-1] + p[:-2, 2:]
              + 2*p[1:-1, :-2] + 4*p[1:-1, 1:-1] + 2*p[1:-1, 2:]
              + p[2:, :-2] + 2*p[2:, 1:-1] + p[2:, 2:]) / 16
    mean = float(luma.mean())
    rms = float(np.sqrt(np.mean((luma-smooth)**2)))
    return dict(mean_linear_rgb=colour.mean(axis=(0, 1)).tolist(),
                mean_linear_luminance=mean, highpass_rms_linear=rms,
                relative_highpass=rms/abs(mean) if abs(mean) > 1e-12 else None,
                minimum=float(colour.min()), maximum=float(colour.max()),
                negative_channels=int(np.count_nonzero(colour < 0)))


def preview(rgb):
    linear = np.maximum(rgb.astype(np.float64), 0)
    mapped = linear / (1 + linear)
    srgb = np.where(mapped <= 0.0031308, mapped*12.92,
                    1.055*np.power(mapped, 1/2.4)-0.055)
    return Image.fromarray(np.rint(np.clip(srgb, 0, 1)*255).astype(np.uint8))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--roi", action="append", required=True,
                        help="name:x,y,width,height in full main-view pixels")
    args = parser.parse_args()
    private = (ROOT / "local").resolve(strict=True)
    source = args.input.resolve(strict=True)
    output = args.output.resolve()
    if not source.is_relative_to(private) or not output.is_relative_to(private):
        parser.error("Inputs and output must stay under this checkout's ignored local/")
    if output.exists():
        parser.error("Refusing evidence replacement")
    before, a, sha_a = read_packet(source/"before.bin", 1)
    after, b, sha_b = read_packet(source/"after.bin", 2)
    if before[:3] != after[:3] or before[4:] != after[4:]:
        raise ValueError("Before/after frame, tick, dimensions or ROI mismatch")
    x, y, w, h = before[6:10]
    rows = []
    names = set()
    for text in args.roi:
        name, coords = text.split(":", 1)
        box = tuple(map(int, coords.split(",")))
        if not name or name in names or len(box) != 4:
            raise ValueError("ROI names must be unique with x,y,width,height")
        names.add(name)
        rx, ry, rw, rh = box
        if not (rw >= 3 and rh >= 3 and x <= rx and y <= ry and rx+rw <= x+w and ry+rh <= y+h):
            raise ValueError("ROI is outside paired capture or smaller than 3x3")
        sl = (slice(ry-y, ry-y+rh), slice(rx-x, rx-x+rw), slice(0, 3))
        first, second = roi_stats(a[sl]), roi_stats(b[sl])
        ratio = second["highpass_rms_linear"]/first["highpass_rms_linear"] if first["highpass_rms_linear"] > 1e-12 else None
        rows.append(dict(name=name, full_view_box=box, before=first, after=second,
                         after_before_highpass_ratio=ratio))
    report = dict(tool_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), format="authored-temporal-colour-v1", frame=before[1], tick=before[2],
                  view_size=before[4:6], origin=[x, y], extent=[w, h],
                  source_sha256=dict(before=sha_a, after=sha_b), rows=rows,
                  preview="Shared Reinhard+sRGB; not renderer AgX/grade or calibrated original colours",
                  limits="Spatial contrast only; input jitter/output reconstruction phase differ. Pair brackets EarlyPostProcess; identify active passes before assigning a cause.")
    output.mkdir(parents=True, exist_ok=False)
    (output/"analysis.json").write_text(json.dumps(report, indent=2, allow_nan=False), encoding="utf-8")
    image = Image.new("RGB", (2*w, h+30))
    draw = ImageDraw.Draw(image)
    for i, (name, data) in enumerate([("Before early post", a), ("After early post", b)]):
        image.paste(preview(data[:, :, :3]), (i*w, 30))
        draw.text((i*w+8, 8), name+" / shared diagnostic mapping", fill="white")
    image.save(output/"hdr-pair-preview.png")
    print(json.dumps(dict(frame=before[1], tick=before[2], origin=[x, y], extent=[w, h], rois=len(rows))))


if __name__ == "__main__":
    main()
