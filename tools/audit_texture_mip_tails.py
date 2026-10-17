"""Read-only PSSG mip-tail inventory; game-derived reports remain private."""
import argparse
import collections
import hashlib
import json
from pathlib import Path
from survey_pssg_payloads import survey


def inspect(source):
    if source.stat().st_size > 256 * 1024 * 1024:
        raise ValueError("resource exceeds 256MiB bound")
    report = survey(source)
    data = source.read_bytes()
    if hashlib.sha256(data).hexdigest() != report["sha256"]:
        raise ValueError("source changed during survey")
    nodes = report["nodes"]
    children = collections.defaultdict(list)
    for i, node in enumerate(nodes):
        children[node["parent"]].append(i)
    rows = []
    for i, node in enumerate(nodes):
        if node["name"] != "TEXTURE":
            continue
        a = node["attributes"]
        fmt = a["texelFormat"].get("string")
        width, height = a["width"]["u32be"], a["height"]["u32be"]
        if fmt not in ("dxt1", "dxt5") or width == height:
            continue
        blocks = [j for j in children[i] if nodes[j]["name"] == "TEXTUREIMAGEBLOCK"]
        if len(blocks) != 1:
            raise ValueError("expected one image block")
        raw = [nodes[j] for j in children[blocks[0]] if nodes[j]["name"] == "TEXTUREIMAGEBLOCKDATA"]
        if len(raw) != 1:
            raise ValueError("expected one image payload")
        raw = raw[0]
        target = a["numberMipMapLevels"]["u32be"] + 1
        if not 1 <= target <= 32:
            raise ValueError("unbounded mip count")
        x, y, expected, levels = width, height, 0, 0
        block = 8 if fmt == "dxt1" else 16
        for _ in range(target):
            expected += ((x + 3) // 4) * ((y + 3) // 4) * block
            levels += 1
            if x == 1 and y == 1:
                break
            x, y = max(1, x // 2), max(1, y // 2)
        size = raw["payload_size"]
        extra = size - expected
        tail = data[raw["payload_offset"] + expected:raw["payload_offset"] + size] if extra >= 0 else b""
        nonzero = sum(v != 0 for v in tail)
        eligible = (levels == target and x == y == 1 and width > 0 and height > 0
                    and width & (width - 1) == height & (height - 1) == 0
                    and 0 < extra <= 2048 and extra % block == 0 and nonzero == 0)
        rows.append(dict(id=a["id"]["string"], format=fmt, dimensions=[width, height],
                         target_levels=target, levels=levels, stored_bytes=size, expected_bytes=expected,
                         extra_bytes=extra, extra_nonzero=nonzero, eligible_zero_tail=eligible))
    return dict(source=str(source), source_sha256=report["sha256"], rectangular_textures=rows)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("sources", nargs="+", type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    private = (Path(__file__).resolve().parent.parent / "local").resolve()
    if not args.output.resolve().is_relative_to(private):
        parser.error("report must stay under this checkout's ignored local/")
    result = [inspect(p) for p in args.sources]
    with args.output.open("x", encoding="utf8") as stream:
        json.dump(result, stream, indent=2, allow_nan=False)
    rows = [r for source in result for r in source["rectangular_textures"]]
    print(f"rectangular={len(rows)} eligible_zero_tails={sum(r['eligible_zero_tail'] for r in rows)}")


if __name__ == "__main__":
    main()
