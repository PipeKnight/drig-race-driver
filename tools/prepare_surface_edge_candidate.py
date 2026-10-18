"""Prepare an audited curb/asphalt RGB transition; source alpha stays separate.

The donor mapping is authored from a geometry/UV audit, never guessed globally.
Writes only under this checkout's existing ignored local/. No GPU/game writes.
"""
import argparse
import hashlib
import io
import json
from pathlib import Path
import struct

import numpy as np
from PIL import Image, __version__ as pillow_version

from prepare_texture_candidate import encoded, linear


def settings(start, end, donor_u, density):
    values = np.asarray([start, end, donor_u, density], dtype=np.float64)
    if not np.isfinite(values).all() or not (0 <= start < end <= 1):
        raise ValueError("finite transition must satisfy 0 <= start < end <= 1")
    if not 0 <= donor_u <= 1 or not 0 < density <= 8:
        raise ValueError("donor U must be0..1 and positive density <=8")
    return values


def derive(carrier, donor, start, end, donor_u, density):
    """Return complete opaque BGRA8 DDS; runtime must restore source mask."""
    settings(start, end, donor_u, density)
    if len(carrier) < 128 or len(carrier) > 512 * 1024**2 or carrier[:4] != b"DDS ":
        raise ValueError("bounded DDS RGB carrier required")
    fields = {p: struct.unpack_from("<I", carrier, p)[0]
              for p in [4, 12, 16, 28, 76, 80, 84, 88, 92, 96, 100, 104]}
    if (fields[4], fields[76], fields[80], fields[84], fields[88],
            fields[92], fields[96], fields[100], fields[104]) != (
            124, 32, 0x41, 0, 32, 0xff0000, 0xff00, 0xff, 0xff000000):
        raise ValueError("carrier must be BGRA8 with explicit alpha")
    h, w = fields[12], fields[16]
    if any(n < 1 or n > 8192 or n & (n - 1) for n in [w, h]):
        raise ValueError("power-of-two carrier dimensions <=8192 required")
    sizes = []
    mw, mh = w, h
    while True:
        sizes.append((mw, mh))
        if mw == mh == 1:
            break
        mw, mh = max(1, mw // 2), max(1, mh // 2)
    if fields[28] != len(sizes) or len(carrier) != 128 + sum(x*y*4 for x, y in sizes):
        raise ValueError("carrier must have an exact complete mip chain")
    cursor = 128
    for x, y in sizes:
        rgba = np.frombuffer(carrier, dtype=np.uint8, count=x*y*4, offset=cursor).reshape(y, x, 4)
        if np.any(rgba[:, :, 3] != 255):
            raise ValueError("packed/cutout carrier alpha cannot be processed as RGB")
        cursor += x*y*4
    if not 128 <= len(donor) <= 512 * 1024**2 or donor[:4] != b"DDS ":
        raise ValueError("bounded purchased DDS donor required")
    with Image.open(io.BytesIO(donor)) as image:
        if any(n < 1 or n > 8192 for n in image.size):
            raise ValueError("donor dimensions outside bound")
        road = np.asarray(image.convert("RGB"))
    with Image.open(io.BytesIO(carrier)) as image:
        original = np.asarray(image.convert("RGB"))
    u = (np.arange(w) + .5) / w
    v = (np.arange(h) + .5) / h
    xs = (donor_u + (u-end)*density)*road.shape[1] - .5
    ys = v*road.shape[0] - .5
    xi, yi = np.floor(xs).astype(int), np.floor(ys).astype(int)
    fx, fy = (xs-xi)[None, :, None], (ys-yi)[:, None, None]
    pixels = linear(road)
    rw, rh = road.shape[1], road.shape[0]
    sampled = (pixels[yi[:, None] % rh, xi[None, :] % rw]*(1-fx)*(1-fy)
               + pixels[yi[:, None] % rh, (xi[None, :]+1) % rw]*fx*(1-fy)
               + pixels[(yi[:, None]+1) % rh, xi[None, :] % rw]*(1-fx)*fy
               + pixels[(yi[:, None]+1) % rh, (xi[None, :]+1) % rw]*fx*fy)
    t = np.clip((u-start)/(end-start), 0, 1)
    blend = (t*t*(3-2*t))[None, :, None]
    base = encoded(linear(original)*(1-blend) + sampled*blend)
    if not np.array_equal(base[:, u < start], original[:, u < start]):
        raise ValueError("pixels outside transition changed")
    payload = []
    values = linear(base)
    for mw, mh in sizes:
        rgb = encoded(values)
        rgba = np.concatenate((rgb, np.full((mh, mw, 1), 255, np.uint8)), axis=2)
        payload.append(rgba[:, :, [2, 1, 0, 3]].tobytes())
        if mh > 1:
            values = values.reshape(mh//2, 2, mw, 3).mean(axis=1)
        if mw > 1:
            values = values.reshape(values.shape[0], mw//2, 2, 3).mean(axis=2)
    output = carrier[:128] + b"".join(payload)
    return output, base, {
        "size": [w, h], "mips": sizes,
        "unchanged_base_columns": int((u < start).sum()),
        "changed_base_pixels": int(np.any(base != original, axis=2).sum()),
        "recipe": {"transition": [start, end], "donor_u_at_edge": donor_u,
                   "u_density": density, "v": "same V, periodic bilinear linear light",
                   "blend": "smoothstep linear RGB", "global_mean_rebalance": False},
        "alpha": "opaque RGB carrier; runtime separately restores exact-reference source mask",
        "scope": "authored mapped edge, not global seam repair/source shader recovery",
    }


def read_checked(path, expected):
    if len(expected) != 64 or any(c not in "0123456789abcdef" for c in expected.lower()):
        raise ValueError("a complete SHA256 is required")
    if not 128 <= path.stat().st_size <= 512 * 1024**2:
        raise ValueError("input outside DDS size bound")
    data = path.read_bytes()
    if hashlib.sha256(data).hexdigest() != expected.lower():
        raise ValueError("input SHA256 mismatch")
    return data


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("carrier", type=Path)
    p.add_argument("donor", type=Path)
    p.add_argument("output_prefix", type=Path)
    p.add_argument("--carrier-sha256", required=True)
    p.add_argument("--donor-sha256", required=True)
    p.add_argument("--transition", type=float, nargs=2, required=True)
    p.add_argument("--donor-u-at-edge", type=float, required=True)
    p.add_argument("--u-density", type=float, required=True)
    a = p.parse_args()
    settings(*a.transition, a.donor_u_at_edge, a.u_density)
    private = (Path(__file__).resolve().parent.parent / "local").resolve(strict=True)
    parent = a.output_prefix.parent.resolve(strict=True)
    if not parent.is_relative_to(private):
        p.error("output must remain under this checkout's existing local/")
    prefix = parent / a.output_prefix.name
    paths = [Path(str(prefix) + suffix) for suffix in [".dds", ".png", ".json"]]
    if any(path.exists() or path.is_symlink() for path in paths):
        p.error("refusing to replace an existing candidate")
    carrier = read_checked(a.carrier, a.carrier_sha256)
    donor = read_checked(a.donor, a.donor_sha256)
    output, base, record = derive(carrier, donor, *a.transition, a.donor_u_at_edge, a.u_density)
    record.update({
        "carrier": str(a.carrier.resolve()), "carrier_sha256": a.carrier_sha256.lower(),
        "donor": str(a.donor.resolve()), "donor_sha256": a.donor_sha256.lower(),
        "output_sha256": hashlib.sha256(output).hexdigest(),
        "script_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "versions": {"numpy": np.__version__, "pillow": pillow_version},
        "status": "candidate; actual rendering/adjacency acceptance required",
    })
    preview = io.BytesIO()
    Image.fromarray(base).save(preview, format="PNG")
    for path, data in zip(paths, [output, preview.getvalue(), json.dumps(record, indent=2).encode()]):
        with path.open("xb") as file:
            file.write(data)
    print(f"prepared {paths[0]} sha256={record['output_sha256']}")


if __name__ == "__main__":
    main()
