"""Read-only texture/shader inventory; purchased-resource metadata stays private."""
import argparse
import collections
import json
from pathlib import Path

from survey_pssg_payloads import survey

RESOURCES = (
    "tracks/europe/donington/tracksplit.pssg",
    "tracks/europe/donington/objects.pssg",
    "tracks/europe/donington/objectstextures.pssg",
    "tracks/europe/donington/patchup_ot.pssg",
    "tracks/europe/donington/lemans_sponsors.pssg",
    "track_sponsors/tag_heuer/sponsor_minor.pssg",
    "track_sponsors/bridgestone/sponsor_event.pssg",
    "cars/lac/livery_00/textures_high/lac_tex_high_00.pssg",
    "cars/lac/lac_gen_high.pssg",
)


def inventory(path):
    if path.stat().st_size > 256 * 1024 * 1024:
        raise ValueError("resource exceeds inventory bound")
    source = survey(path)
    nodes = source["nodes"]
    children = collections.defaultdict(list)
    for i, node in enumerate(nodes):
        children[node["parent"]].append(i)
    textures, samplers = [], collections.Counter()
    for i, node in enumerate(nodes):
        attrs = node["attributes"]
        if node["name"] == "SHADERINPUTDEFINITION" and attrs.get("type", {}).get("string") == "texture":
            samplers[attrs["name"]["string"]] += 1
        if node["name"] != "TEXTURE":
            continue
        width, height = attrs["width"]["u32be"], attrs["height"]["u32be"]
        fmt = attrs["texelFormat"]["string"]
        block_bytes = {"dxt1": 8, "dxt5": 16}.get(fmt)
        blocks = [j for j in children[i] if nodes[j]["name"] == "TEXTUREIMAGEBLOCK"]
        payload_bytes = sum(nodes[k]["payload_size"] for j in blocks for k in children[j]
                            if nodes[k]["name"] == "TEXTUREIMAGEBLOCKDATA")
        levels, packed_bytes, w, h = 0, 0, width, height
        if block_bytes is not None and width and height:
            while True:
                packed_bytes += ((w + 3) // 4) * ((h + 3) // 4) * block_bytes
                levels += 1
                if w == 1 and h == 1:
                    break
                w, h = max(w // 2, 1), max(h // 2, 1)
        declared = attrs.get("numberMipMapLevels", {}).get("u32be")
        # Mirrors the documented reader's size/count contract, not a decode proof.
        chain_matches = (len(blocks) == 1 and levels > 1 and declared == levels - 1
                         and payload_bytes == packed_bytes)
        textures.append(dict(id=attrs.get("id", {}).get("string"), width=width,
                             height=height, format=fmt, declared_tail_levels=declared,
                             payload_bytes=payload_bytes, expected_full_chain_bytes=packed_bytes,
                             current_chain_contract_matches=chain_matches))
    return dict(resource=str(path), bytes=source["bytes"], sha256=source["sha256"],
                textures=textures, texture_sampler_definitions=dict(sorted(samplers.items())))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--game-root", type=Path,
                        default=Path(r"C:\Program Files (x86)\Steam\steamapps\common\Grid"))
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    private = (Path(__file__).resolve().parent.parent / "local").resolve()
    output = args.output.resolve()
    if not output.is_relative_to(private):
        parser.error("inventory output must remain under this checkout's ignored local/")
    if output.exists():
        raise FileExistsError(output)
    resources = [inventory(args.game_root / relative) for relative in RESOURCES]
    summary = dict(resources=len(resources), textures=sum(len(r["textures"]) for r in resources),
                   chain_matches=sum(t["current_chain_contract_matches"]
                                     for r in resources for t in r["textures"]))
    report = dict(scope="Static resource inventory; sampler names do not prove channel semantics",
                  summary=summary, resources=resources)
    output.parent.mkdir(parents=True, exist_ok=True)
    with output.open("x", encoding="utf-8") as stream:
        json.dump(report, stream, indent=2, allow_nan=False)
    print(json.dumps(summary))


if __name__ == "__main__":
    main()
