"""Read-only local video inspection; keep generated game imagery under local/."""

import argparse
import hashlib
import json
import math
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "local/tools/video-python"))

import av
import numpy as np
from PIL import Image, ImageDraw


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--interval", type=float, default=5.0)
    parser.add_argument("--times", type=float, nargs="+")
    parser.add_argument("--audit", action="store_true")
    args = parser.parse_args()
    source = args.input.resolve(strict=True)
    output = args.output.resolve()
    if not output.is_relative_to(ROOT / "local"):
        parser.error("Generated evidence must remain under repository local/")
    if args.interval <= 0 or not math.isfinite(args.interval):
        parser.error("Interval must be finite and positive")
    output.mkdir(parents=True, exist_ok=False)
    digest = hashlib.sha256()
    with source.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    with av.open(str(source)) as container:
        stream = container.streams.video[0]
        stream.codec_context.thread_type = "AUTO"
        duration = float(stream.duration * stream.time_base)
        report = {
            "input": str(source), "bytes": source.stat().st_size,
            "sha256": digest.hexdigest(), "pyav": av.__version__,
            "libraries": av.library_versions, "metadata": container.metadata,
            "duration_s": duration, "width": stream.width, "height": stream.height,
            "average_rate": str(stream.average_rate), "declared_frames": stream.frames,
            "streams": [{"index": s.index, "type": s.type,
                         "codec": s.codec_context.name,
                         "time_base": str(s.time_base),
                         "duration": s.duration, "frames": s.frames}
                        for s in container.streams],
            "samples": [],
        }
        targets = args.times or list(np.arange(0, duration, args.interval))
        if any(not math.isfinite(t) or t < 0 or t >= duration for t in targets):
            parser.error("Sample time outside video")
        tiles = []
        for index, target in enumerate(targets):
            container.seek(int(target / stream.time_base), stream=stream,
                           backward=True, any_frame=False)
            for frame in container.decode(stream):
                actual = float(frame.pts * frame.time_base)
                if actual + 1e-7 >= target:
                    break
            else:
                raise RuntimeError(f"No decoded frame at {target}")
            picture = frame.to_image().convert("RGB")
            filename = f"frame-{target:08.3f}.jpg"
            picture.save(output / filename, quality=94)
            report["samples"].append({"requested_s": float(target),
                                      "actual_s": actual, "file": filename})
            picture.thumbnail((384, 308))
            tile = Image.new("RGB", (384, 338), "#161616")
            tile.paste(picture, ((384 - picture.width) // 2, 28))
            label = f"{int(target)//60:02d}:{target%60:06.3f}  actual {actual:.3f}s"
            ImageDraw.Draw(tile).text((8, 7), label, fill="white")
            tiles.append(tile)
            if len(tiles) == 12 or index == len(targets) - 1:
                sheet = Image.new("RGB", (1152, 338 * math.ceil(len(tiles) / 3)))
                for tile_index, item in enumerate(tiles):
                    sheet.paste(item, ((tile_index % 3) * 384, (tile_index // 3) * 338))
                sheet.save(output / f"sheet-{index//12:02d}.jpg", quality=94)
                tiles = []
    if args.audit:
        with av.open(str(source)) as container:
            stream = container.streams.video[0]
            stream.codec_context.thread_type = "AUTO"
            times = [float(f.pts * f.time_base) for f in container.decode(stream)]
        differences = np.diff(times)
        report["decode_audit"] = {
            "frames": len(times), "first_s": times[0], "last_s": times[-1],
            "nonpositive_timestamp_steps": int(np.count_nonzero(differences <= 0)),
            "min_step_s": float(differences.min()),
            "median_step_s": float(np.median(differences)),
            "max_step_s": float(differences.max()),
            "steps_above_50ms": int(np.count_nonzero(differences > 0.05)),
            "limit": "Timestamp regularity does not prove unique images or game tick rate.",
        }
        with av.open(str(source)) as container:
            audio = container.streams.audio
            if audio:
                sample_count = 0
                square_sum = 0.0
                peak = 0.0
                for frame in container.decode(audio[0]):
                    array = frame.to_ndarray()
                    if array.dtype.kind in "iu":
                        info = np.iinfo(array.dtype)
                        values = array.astype(np.float64)
                        if array.dtype.kind == "u":
                            values -= (info.max + 1) / 2
                        values /= max(abs(info.min), info.max + 1)
                    else:
                        values = array.astype(np.float64)
                    sample_count += values.size
                    square_sum += float(np.square(values).sum())
                    peak = max(peak, float(np.abs(values).max()))
                report["audio_audit"] = {
                    "sample_rate": audio[0].codec_context.sample_rate,
                    "channels": audio[0].codec_context.channels,
                    "scalar_samples": sample_count, "normalized_peak": peak,
                    "rms": math.sqrt(square_sum / sample_count),
                    "limit": "Signal statistics only; no claim of auditory review or source identity.",
                }
    (output / "inspection.json").write_text(json.dumps(report, indent=2), encoding="utf-8")
    print(json.dumps({"output": str(output), "sha256": report["sha256"],
                      "duration_s": duration, "samples": len(report["samples"]),
                      "decode_audit": report.get("decode_audit"),
                      "audio_audit": report.get("audio_audit")}, indent=2))


if __name__ == "__main__":
    main()
