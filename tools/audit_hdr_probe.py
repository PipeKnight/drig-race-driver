"""Audit private six-face GPU RGBA16F radiance and plot a diagnostic preview."""
import argparse
import hashlib
import json
import os
from pathlib import Path

import numpy as np


def audit(data, size):
    if size < 16 or size > 2048 or size & (size - 1):
        raise ValueError("expected power-of-two face size 16..2048")
    if len(data) != 6 * size * size * 8:
        raise ValueError("RGBA16F six-face byte count mismatch")
    pixels = np.frombuffer(data, dtype="<f2").astype(np.float32).reshape(6, size, size, 4)
    if not np.isfinite(pixels).all():
        raise ValueError("nonfinite HDR data")
    rgb = pixels[..., :3]
    report = {"format": "little-endian RGBA16F", "size": size,
              "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest(),
              "max_rgb": float(rgb.max()), "min_rgb": float(rgb.min()),
              "pixels_above_one": int((rgb.max(axis=-1) > 1).sum()),
              "negative_rgb_pixels": int((rgb.min(axis=-1) < -1e-5).sum()),
              "faces": [], "scope": "raw GPU data, not filtered reflection or fidelity proof"}
    for i, name in enumerate(("+X", "-X", "+Y", "-Y", "-Z", "+Z")):
        face = rgb[i]
        report["faces"].append({"index": i, "world_direction": name,
            "min_rgb": float(face.min()), "max_rgb": float(face.max()),
            "mean_rgb": face.mean(axis=(0, 1)).tolist(),
            "luminance_mean": float((face @ np.array([0.2126, 0.7152, 0.0722])).mean()),
            "nonzero_pixels": int((face.max(axis=-1) > 1e-5).sum()),
            "alpha_min": float(pixels[i, ..., 3].min()),
            "alpha_max": float(pixels[i, ..., 3].max())})
    return pixels, report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("--size", type=int, default=256)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--preview", type=Path)
    args = parser.parse_args()
    private = (Path(__file__).resolve().parents[1] / "local").resolve()
    source, output = args.source.resolve(), args.output.resolve()
    paths = [source, output] + ([args.preview.resolve()] if args.preview else [])
    if any(not p.is_relative_to(private) for p in paths):
        parser.error("game-derived radiance/evidence must stay under ignored local/")
    if output.exists() or (args.preview and args.preview.exists()):
        raise FileExistsError("refusing existing evidence")
    if source.stat().st_size > 6 * 2048 * 2048 * 8:
        raise ValueError("probe exceeds supported size bound")
    data = source.read_bytes()
    pixels, report = audit(data, args.size)
    if hashlib.sha256(source.read_bytes()).hexdigest() != report["sha256"]:
        raise ValueError("probe changed during audit")
    if args.preview:
        os.environ.setdefault("MPLCONFIGDIR", str(private / "matplotlib-cache"))
        import matplotlib
        matplotlib.use("Agg")
        import matplotlib.pyplot as plt
        fig, axes = plt.subplots(2, 3, figsize=(12, 8), layout="constrained")
        for i, ax in enumerate(axes.flat):
            # Preview only; never feed this tonemapped figure to the renderer.
            rgb = np.maximum(pixels[i, ..., :3], 0)
            display = np.power(rgb / (1 + rgb), 1 / 2.2)
            ax.imshow(display)
            ax.set_title(f"Face {i}: world {report['faces'][i]['world_direction']}")
            ax.set_axis_off()
        fig.suptitle("HDR probe diagnostic: Reinhard + gamma preview; raw radiance unchanged")
        fig.savefig(args.preview, dpi=150)
        plt.close(fig)
        report["preview"] = str(args.preview.resolve())
        report["preview_scope"] = "diagnostic display transform, not source radiance"
    with output.open("x", encoding="utf-8") as f:
        json.dump(report, f, indent=2, allow_nan=False)
    print(json.dumps({key: report[key] for key in ("size", "max_rgb", "min_rgb", "pixels_above_one", "negative_rgb_pixels")}))


if __name__ == "__main__":
    main()
