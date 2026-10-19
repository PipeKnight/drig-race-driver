"""Analyze opt-in raw motion/depth evidence; write results beside private input.

Uses GPU/frozen frame IDs and column-major float32 matrices. Conservative car-local
volumes identify upper rigid body and nearby static ground, not a full object-ID
segmentation. Output pixel errors measure reprojection consistency, not AA quality.
"""
import argparse
import json
from pathlib import Path
import re
import numpy as np


def matrix(text, name):
    match = re.search(r"^" + re.escape(name) + r"=(\[[^\n]+\])$", text, re.M)
    if not match:
        raise ValueError(f"missing frozen matrix {name}")
    values = np.asarray(json.loads(match[1]), dtype=np.float32).astype(np.float64)
    if values.shape != (16,) or not np.isfinite(values).all():
        raise ValueError(f"invalid frozen matrix {name}")
    return values.reshape((4, 4), order="F")


def project(points, transform):
    clip = points @ transform.T
    good = np.isfinite(clip).all(axis=1) & (clip[:, 3] > 1e-10)
    ndc = np.zeros((len(points), 2), dtype=np.float64)
    np.divide(clip[:, :2], clip[:, 3, None], out=ndc, where=good[:, None])
    return ndc, good


def summarize(mask, observed, expected, size):
    error = (observed - expected) * size
    finite = np.isfinite(error).all(axis=1)
    chosen = mask & finite
    count = int(chosen.sum())
    if not count:
        return {"samples": 0, "threshold_pass": False}
    magnitude = np.linalg.norm(error[chosen], axis=1)
    quantiles = np.percentile(magnitude, [50, 90, 99, 100]).tolist()
    return {
        "samples": count,
        "residual_pixels_p50_p90_p99_max": quantiles,
        "mean_signed_pixel_error_xy": error[chosen].mean(axis=0).tolist(),
        "observed_motion_pixels_p50_p99": np.percentile(
            np.linalg.norm(observed[chosen] * size, axis=1), [50, 99]
        ).tolist(),
        "expected_motion_pixels_p50_p99": np.percentile(
            np.linalg.norm(expected[chosen] * size, axis=1), [50, 99]
        ).tolist(),
        "threshold_pass": bool(count >= 100 and quantiles[2] <= 0.5),
    }


def analyze(directory):
    directory = Path(directory)
    raw = (directory / "motion-depth.bin").read_bytes()
    if len(raw) < 48 or len(raw) > (3 + 512 * 384) * 16 or len(raw) % 16:
        raise ValueError("invalid bounded readback size")
    header = np.frombuffer(raw[:48], dtype="<u4").copy()
    request, frame, tick, reserved, width, height, ox, oy, rw, rh, r0, r1 = map(int, header)
    if request != 1 or reserved or r0 or r1 or not (0 < rw <= 512 and 0 < rh <= 384):
        raise ValueError("invalid capture header")
    if not width or not height or ox + rw > width or oy + rh > height:
        raise ValueError("ROI outside source texture")
    count = rw * rh
    if len(raw) < 48 + count * 16:
        raise ValueError("truncated pixels")
    pixels = np.frombuffer(raw[48:48 + count * 16], dtype="<f4").reshape((-1, 4)).astype(np.float64)
    if not np.isfinite(pixels).all() or not np.all(pixels[:, 3] == 1):
        raise ValueError("invalid/nonfinite GPU records")
    if not np.all((pixels[:, 2] >= 0) & (pixels[:, 2] <= 1)):
        raise ValueError("depth outside reversed-Z range")
    text = (directory / "metadata.txt").read_text(encoding="utf-8-sig")
    identity = re.search(r"request=(\d+) frame=(\d+) tick=(\d+)", text)
    if not identity or tuple(map(int, identity.groups())) != (request, frame, tick):
        raise ValueError("asynchronous frame/header mismatch")
    body = matrix(text, "current_body")
    old_body = matrix(text, "previous_body")
    current = matrix(text, "current_unjittered")
    previous = matrix(text, "previous_unjittered")
    inverse = matrix(text, "inverse_jittered")
    y, x = np.indices((rh, rw))
    clip = np.column_stack((
        2 * (x.ravel() + ox + 0.5) / width - 1,
        1 - 2 * (y.ravel() + oy + 0.5) / height,
        pixels[:, 2], np.ones(count),
    ))
    world_h = clip @ inverse.T
    valid = np.isfinite(world_h).all(axis=1) & (np.abs(world_h[:, 3]) > 1e-10) & (pixels[:, 2] > 0)
    world = np.zeros_like(world_h)
    np.divide(world_h, world_h[:, 3, None], out=world, where=valid[:, None])
    local = world @ np.linalg.inv(body).T
    earlier_world = local @ old_body.T
    current_xy, cgood = project(world, current)
    old_body_xy, bgood = project(earlier_world, previous)
    old_static_xy, sgood = project(world, previous)
    uv_scale = np.array([0.5, -0.5])
    expected_body = (current_xy - old_body_xy) * uv_scale
    expected_static = (current_xy - old_static_xy) * uv_scale
    upper_body = valid & cgood & bgood & (np.abs(local[:, 0]) < 0.75) & (np.abs(local[:, 2]) < 1.7) & (local[:, 1] > 0.55) & (local[:, 1] < 1.65)
    ground = valid & cgood & sgood & (np.abs(local[:, 0]) > 1.7) & (np.abs(local[:, 0]) < 12) & (np.abs(local[:, 2]) < 12) & (local[:, 1] < 0.1) & (local[:, 1] > -2)
    size = np.array([width, height])
    report = {
        "request": request, "frame": frame, "tick": tick,
        "source_size": [width, height], "roi": [ox, oy, rw, rh],
        "units": "pixels per rendered frame; no extra delta-time multiplication",
        "upper_rigid_body": summarize(upper_body, pixels[:, :2], expected_body, size),
        "near_static_ground": summarize(ground, pixels[:, :2], expected_static, size),
        "scope": "conservative car-local volumes, not object-ID/triangle segmentation; excludes sky; not visual-quality or complete temporal acceptance",
        "matrix_convention": "column-major float32 dump, float64 analysis; inverse actual jittered depth projection, unjittered velocity projection",
    }
    report["both_volume_checks_pass"] = report["upper_rigid_body"]["threshold_pass"] and report["near_static_ground"]["threshold_pass"]
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--out", type=Path)
    args = parser.parse_args()
    try:
        report = analyze(args.directory)
    except (ValueError, OSError, np.linalg.LinAlgError) as error:
        parser.error(str(error))
    output = args.out or args.directory / "analysis.json"
    output.write_text(json.dumps(report, indent=2), encoding="utf-8")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
