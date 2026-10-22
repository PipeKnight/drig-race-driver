"""Analyze opt-in raw motion/depth evidence; write results beside private input.

Uses GPU/frozen frame IDs and column-major float32 matrices. Conservative car-local
volumes identify upper rigid body and nearby static ground, not a full object-ID
segmentation. Output pixel errors measure reprojection consistency, not AA quality.
"""
import argparse
import hashlib
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



def unproject_pixels(xy, depth, inverse, size):
    """Pixel indices (not UVs), reverse depth, actual saved jittered projection."""
    clip = np.column_stack((2*(xy[:, 0]+0.5)/size[0]-1,
                            1-2*(xy[:, 1]+0.5)/size[1], depth, np.ones(len(depth))))
    world = clip @ inverse.T
    good = np.isfinite(world).all(axis=1) & (np.abs(world[:, 3]) > 1e-10)
    result = np.zeros_like(world)
    np.divide(world, world[:, 3, None], out=result, where=good[:, None])
    return result, good


def depth_history(world, previous_xy, previous_inverse, size, origin, depth):
    """Conservative four-tap visibility classes, not object IDs or DLSS state.

    All taps must agree. Tolerance includes two projected pixel footprints;
    steep surfaces and mixed silhouette pixels may remain ambiguous.
    """
    n = len(world)
    ix = np.floor(previous_xy[:, 0]).astype(np.int64)
    iy = np.floor(previous_xy[:, 1]).astype(np.int64)
    h, w = depth.shape
    inside = (ix >= origin[0]) & (iy >= origin[1]) & (ix+1 < origin[0]+w) & (iy+1 < origin[1]+h)
    projected = world @ np.linalg.inv(previous_inverse).T
    good = inside & (projected[:, 3] > 1e-10)
    expected_z = np.zeros(n)
    np.divide(projected[:, 2], projected[:, 3], out=expected_z, where=projected[:, 3] > 1e-10)
    near, ng = unproject_pixels(previous_xy, np.ones(n), previous_inverse, size)
    far, fg = unproject_pixels(previous_xy, np.full(n, 0.5), previous_inverse, size)
    direction = far[:, :3] - near[:, :3]
    length = np.linalg.norm(direction, axis=1)
    direction /= np.maximum(length[:, None], 1e-20)
    good &= ng & fg & (length > 1e-10)
    errors, footprints = [], []
    for dx, dy in [(0, 0), (1, 0), (0, 1), (1, 1)]:
        px, py = ix+dx, iy+dy
        z = depth[np.clip(py-origin[1], 0, h-1), np.clip(px-origin[0], 0, w-1)]
        coords = np.column_stack((px, py))
        sample, sg = unproject_pixels(coords, z, previous_inverse, size)
        plane, pg = unproject_pixels(coords, expected_z, previous_inverse, size)
        good &= sg & pg & (z > 0) & (z <= 1)
        errors.append(np.sum((world[:, :3]-sample[:, :3])*direction, axis=1))
        footprints.append(np.linalg.norm(world[:, :3]-plane[:, :3], axis=1))
    error = np.asarray(errors).T
    tolerance = 0.03 + 2*np.max(footprints, axis=0)
    stable = good & np.all(np.abs(error) <= tolerance[:, None], axis=1)
    nearer = good & np.all(error > tolerance[:, None], axis=1)
    farther = good & np.all(error < -tolerance[:, None], axis=1)
    mixed = good & ~stable & ~nearer & ~farther
    return dict(stable=stable, previous_nearer=nearer, previous_farther=farther,
                mixed=mixed, unavailable=~good), tolerance


def analyze_history(directory, previous_directory, map_path=None):
    # Reuse all v1/header/finite/matrix validation; failed volume thresholds are
    # descriptive and do not make the raw capture invalid.
    analyze(directory)
    analyze(previous_directory)
    def packet(path):
        raw = (Path(path)/"motion-depth.bin").read_bytes()
        header = tuple(map(int, np.frombuffer(raw[:48], '<u4')))
        pixels = np.frombuffer(raw, '<f4', count=header[8]*header[9]*4, offset=48).reshape(header[9], header[8], 4)
        text = (Path(path)/'metadata.txt').read_text(encoding='utf-8-sig')
        return header, pixels, text
    h, a, text = packet(directory)
    hp, b, old_text = packet(previous_directory)
    if hp[1]+1 != h[1] or hp[4:6] != h[4:6] or hp[2] > h[2]:
        raise ValueError('history requires adjacent actual extracted frames, matching view sizes and ordered ticks')
    previous = matrix(text, 'previous_unjittered')
    if not np.allclose(previous, matrix(old_text, 'current_unjittered'), rtol=5e-6, atol=5e-5):
        raise ValueError('adjacent camera metadata does not match')
    if not np.allclose(matrix(text, 'previous_body'), matrix(old_text, 'current_body'), rtol=5e-6, atol=5e-5):
        raise ValueError('adjacent body metadata does not match')
    yy, xx = np.indices(a.shape[:2]); xy = np.column_stack((xx.ravel()+h[6], yy.ravel()+h[7]))
    pixels = a.reshape(-1, 4).astype(float)
    size = np.asarray(h[4:6])
    world, valid = unproject_pixels(xy, pixels[:, 2], matrix(text, 'inverse_jittered'), size)
    valid &= pixels[:, 2] > 0
    local = world @ np.linalg.inv(matrix(text, 'current_body')).T
    sides = (np.abs(local[:, 0]) > 1.7) & (np.abs(local[:, 0]) < 12) & (np.abs(local[:, 2]) < 12)
    rear = (np.abs(local[:, 0]) < 12) & (local[:, 2] < -2.3) & (local[:, 2] > -12)
    ground = valid & (sides | rear) & (local[:, 1] < 0.1) & (local[:, 1] > -2)
    old_inverse = matrix(old_text, 'inverse_jittered')
    ndc, pg = project(world, np.linalg.inv(old_inverse))
    previous_xy = (ndc*np.array([0.5, -0.5])+0.5)*size-0.5
    classes, tolerance = depth_history(world, previous_xy, old_inverse, size, np.asarray(hp[6:8]), b[:, :, 2])
    cur_ndc, cg = project(world, matrix(text, 'current_unjittered'))
    old_ndc, og = project(world, previous)
    expected = (cur_ndc-old_ndc)*[0.5, -0.5]
    residual = np.linalg.norm((pixels[:, :2]-expected)*size, axis=1)
    ground &= pg & cg & og
    def region(mask):
        selected = ground & mask
        count = int(selected.sum())
        return dict(samples=count,
                    screen_bounds=[int(xy[selected, 0].min()), int(xy[selected, 1].min()), int(xy[selected, 0].max()), int(xy[selected, 1].max())] if count else None,
                    static_motion_residual_pixels_p50_p99=np.percentile(residual[selected], [50, 99]).tolist() if count else None,
                    tolerance_metres_p50_p99=np.percentile(tolerance[selected], [50, 99]).tolist() if count else None)
    report = dict(source_sha256={"current": hashlib.sha256((Path(directory)/"motion-depth.bin").read_bytes()).hexdigest(),
                                 "previous": hashlib.sha256((Path(previous_directory)/"motion-depth.bin").read_bytes()).hexdigest()},
                  current_frame=h[1], previous_frame=hp[1], current_tick=h[2], previous_tick=hp[2],
                  current_roi=list(h[6:10]), previous_roi=list(hp[6:10]),
                  side_candidates=int((ground & sides).sum()), rear_candidates=int((ground & rear).sum()),
                  regions={name: region(mask) for name, mask in classes.items()},
                  motion_inconsistent=region(residual > 0.5),
                  scope='Conservative geometric volumes and four-tap depth agreement, not road/object-ID segmentation, visual quality or internal DLSS history. Occluder identity is unknown. Missing previous ROI is unavailable, never disocclusion.')
    if map_path:
        from PIL import Image
        image = np.full((len(ground), 3), [22, 25, 38], dtype=np.uint8)
        colours = dict(stable=[55, 170, 90], previous_nearer=[230, 60, 60], previous_farther=[70, 140, 245], mixed=[240, 160, 30], unavailable=[150, 70, 190])
        for name, mask in classes.items(): image[ground & mask] = colours[name]
        image[ground & (residual > 0.5)] = [255, 245, 30]
        Image.fromarray(image.reshape(h[9], h[8], 3)).save(map_path)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--out", type=Path)
    parser.add_argument("--previous", type=Path, help="adjacent previous raw audit directory; enables depth-history classification")
    parser.add_argument("--map", type=Path, help="optional native-pixel class map; requires --previous")
    args = parser.parse_args()
    if args.map and not args.previous:
        parser.error("--map requires --previous")
    try:
        report = analyze_history(args.directory, args.previous, args.map) if args.previous else analyze(args.directory)
    except (ValueError, OSError, np.linalg.LinAlgError) as error:
        parser.error(str(error))
    output = args.out or args.directory / "analysis.json"
    output.write_text(json.dumps(report, indent=2), encoding="utf-8")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
