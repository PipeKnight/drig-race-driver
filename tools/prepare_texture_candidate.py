"""Package an opaque diffuse upscale privately, preserving mean linear colour.

BGRA8 DDS with a complete linear-light mip chain; no game-install writes.
This first pilot deliberately rejects packed/cutout alpha and non-power-of-two
textures. Neural detail is authored, not recovered ground truth.
"""
import argparse
import hashlib
import io
import json
from pathlib import Path
import struct
import numpy as np
from PIL import Image

def linear(rgb):
    rgb = np.asarray(rgb, dtype=np.float32) / 255
    return np.where(rgb <= .04045, rgb / 12.92, ((rgb + .055) / 1.055) ** 2.4)

def encoded(rgb):
    rgb = np.clip(rgb, 0, 1)
    srgb = np.where(rgb <= .0031308, rgb * 12.92, 1.055 * rgb ** (1 / 2.4) - .055)
    return np.rint(srgb * 255).clip(0, 255).astype(np.uint8)

def package(source, prediction, scale, padding):
    original = Image.open(source).convert("RGBA")
    if original.getchannel("A").getextrema() != (255, 255):
        raise ValueError("pilot requires opaque diffuse; handle packed/cutout alpha separately")
    predicted = Image.open(prediction).convert("RGB")
    expected = ((original.width + 2 * padding) * scale, (original.height + 2 * padding) * scale)
    if predicted.size != expected:
        raise ValueError("prediction size does not match scale/wrap padding")
    w, h = original.width * scale, original.height * scale
    if any(n < 1 or n > 8192 or n & (n - 1) for n in (w, h)):
        raise ValueError("pilot supports power-of-two outputs up to8192")
    predicted = predicted.crop((padding * scale, padding * scale, padding * scale + w, padding * scale + h))
    target_mean = linear(original.convert("RGB")).mean(axis=(0, 1), dtype=np.float64)
    values = linear(predicted)
    ratio = target_mean / np.maximum(values.mean(axis=(0, 1), dtype=np.float64), 1e-8)
    corrected = values * ratio.astype(np.float32)
    clipped = float(np.mean(corrected > 1))
    base = encoded(corrected)
    values = linear(base)
    levels, payload = [], []
    while True:
        rgb = encoded(values)
        alpha = np.full((*rgb.shape[:2], 1), 255, dtype=np.uint8)
        rgba = np.concatenate((rgb, alpha), axis=2)
        payload.append(rgba[:, :, [2, 1, 0, 3]].tobytes())
        levels.append([rgb.shape[1], rgb.shape[0]])
        if rgb.shape[0] == rgb.shape[1] == 1:
            break
        if values.shape[0] > 1:
            values = values.reshape(values.shape[0] // 2, 2, values.shape[1], 3).mean(axis=1)
        if values.shape[1] > 1:
            values = values.reshape(values.shape[0], values.shape[1] // 2, 2, 3).mean(axis=2)
    header = bytearray(128)
    header[:4] = b"DDS "
    for offset, value in [(4,124),(8,0x2100f),(12,h),(16,w),(20,w*4),(28,len(levels)),
                          (76,32),(80,0x41),(88,32),(92,0xff0000),(96,0xff00),(100,0xff),
                          (104,0xff000000),(108,0x401008)]:
        struct.pack_into("<I", header, offset, value)
    dds = bytes(header) + b"".join(payload)
    decoded = Image.open(io.BytesIO(dds)).convert("RGBA")
    if not np.array_equal(np.asarray(decoded)[:,:,:3], base) or decoded.getchannel("A").getextrema() != (255,255):
        raise ValueError("DDS base colour/alpha round-trip failed")
    metadata = {"source_size": original.size, "size": [w,h], "mips": levels,
                "format": "opaque BGRA8, sRGB colour with linear-light mip averaging",
                "scale": scale, "wrap_padding": padding, "linear_mean_ratio": ratio.tolist(),
                "clipped_fraction": clipped, "source_linear_mean": target_mean.tolist(),
                "candidate_linear_mean": linear(base).mean(axis=(0,1), dtype=np.float64).tolist(),
                "source_dds_sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
                "prediction_sha256": hashlib.sha256(prediction.read_bytes()).hexdigest(),
                "dds_sha256": hashlib.sha256(dds).hexdigest(), "dds_bytes": len(dds),
                "packager_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                "status": "candidate; in-engine quality/tiling/mips validation required"}
    return dds, Image.fromarray(base), metadata

def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("source", type=Path)
    p.add_argument("prediction", type=Path)
    p.add_argument("--scale", type=int, default=4, choices=[2,4])
    p.add_argument("--padding", type=int, default=0)
    p.add_argument("--texture-key", required=True)
    p.add_argument("--output", type=Path, required=True)
    a = p.parse_args()
    private = (Path(__file__).resolve().parent.parent / "local").resolve()
    paths = [a.source, a.prediction, a.output]
    if any(not path.resolve().is_relative_to(private) for path in paths):
        p.error("all derived/source evidence must remain under this checkout's local/")
    if a.output.suffix != ".dds" or a.source.parent.resolve() != a.output.parent.resolve():
        p.error("DDS output and original-reference DDS must share the private package directory")
    if not 0 <= a.padding <= 256 or not a.texture_key or any(c in a.texture_key for c in "\t\r\n"):
        p.error("invalid padding or texture key")
    outputs = [a.output, a.output.with_suffix(".png"), a.output.with_suffix(".json"), a.output.with_suffix(".overrides.tsv")]
    if any(path.exists() for path in outputs):
        raise FileExistsError("refusing to overwrite any existing package output")
    dds, preview, metadata = package(a.source, a.prediction, a.scale, a.padding)
    metadata["texture_key"] = a.texture_key
    with outputs[0].open("xb") as f:
        f.write(dds)
    with outputs[1].open("xb") as f:
        preview.save(f, format="PNG")
    with outputs[2].open("x", encoding="utf-8") as f:
        json.dump(metadata, f, indent=2)
    with outputs[3].open("x", encoding="utf-8", newline="\n") as f:
        f.write(f"GRID_TEXTURE_OVERRIDES_V1\n{a.texture_key}\t{a.source.name}\t{a.output.name}\n")
    print(json.dumps(metadata, indent=2))

if __name__ == "__main__":
    main()
