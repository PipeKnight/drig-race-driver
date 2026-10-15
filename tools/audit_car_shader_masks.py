"""Read-only LOD0 saved color-mask audit; proprietary metadata stays under local/."""
import argparse
import collections
import hashlib
import json
import struct
from pathlib import Path

from survey_pssg_payloads import survey


def audit(path):
    if path.stat().st_size > 256 * 1024 * 1024:
        raise ValueError("car resource exceeds bound")
    tree = survey(path)
    data = path.read_bytes()
    if hashlib.sha256(data).hexdigest() != tree["sha256"]:
        raise ValueError("source changed during survey")
    nodes = tree["nodes"]
    children = collections.defaultdict(list)
    for i, node in enumerate(nodes):
        children[node["parent"]].append(i)

    def attrs(i):
        # A four-byte zero has both the exploratory integer interpretation and
        # an empty length-prefixed string preview; numeric layout fields win.
        return {k: v.get("u32be", v.get("string")) for k, v in nodes[i]["attributes"].items()}

    blocks = {attrs(i)["id"]: i for i, n in enumerate(nodes) if n["name"] == "DATABLOCK"}
    sources = {attrs(i)["id"]: i for i, n in enumerate(nodes) if n["name"] == "RENDERDATASOURCE"}
    bundles = [i for i, n in enumerate(nodes) if n["name"] == "MATRIXPALETTEBUNDLENODE"
               and attrs(i).get("id") == "LOD0_"]
    if len(bundles) != 1:
        raise ValueError("expected unique LOD0 bundle")
    draws = []
    for i, node in enumerate(nodes):
        if node["name"] != "MATRIXPALETTERENDERINSTANCE":
            continue
        parent = node["parent"]
        if parent is None or nodes[parent]["parent"] != bundles[0]:
            continue
        draw = attrs(i)
        source = sources[draw["indices"].removeprefix("#")]
        colors = []
        for stream in children[source]:
            if nodes[stream]["name"] != "RENDERSTREAM":
                continue
            a = attrs(stream)
            block = blocks[a["dataBlock"].removeprefix("#")]
            descriptors = [j for j in children[block] if nodes[j]["name"] == "DATABLOCKSTREAM"]
            descriptor = attrs(descriptors[a["subStream"]])
            if descriptor["renderType"] != "Color":
                continue
            if descriptor["dataType"] != "uint_color_argb":
                raise ValueError("unsupported color representation")
            payload = [nodes[j] for j in children[block] if nodes[j]["name"] == "DATABLOCKDATA"]
            if len(payload) != 1:
                raise ValueError("expected unique color data")
            count = attrs(block)["elementCount"]
            stride, offset = descriptor["stride"], descriptor["offset"]
            start, length = draw["streamOffset"], draw["elementCountFromOffset"]
            if offset + 4 > stride or payload[0]["payload_size"] != count * stride or start + length > count:
                raise ValueError("color layout/draw bounds mismatch")
            values = [struct.unpack_from(">I", data, payload[0]["payload_offset"] + j * stride + offset)[0]
                      for j in range(start, start + length)]
            colors.append({"block": attrs(block)["id"], "vertices": length,
                           "words": {f"{word:08x}": n for word, n in collections.Counter(values).items()}})
        if len(colors) != 1:
            raise ValueError("expected one saved color stream per selected draw")
        draws.append({"draw": draw["id"], "material": draw["shader"].removeprefix("#"),
                      "joint": attrs(parent)["id"], "colors": colors[0]})
    if not draws:
        raise ValueError("no LOD0 draw colors")
    return {"source_sha256": tree["sha256"], "draws": draws,
            "scope": "saved ARGB words; runtime mutation and backend channel conversion not observed"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    private = (Path(__file__).resolve().parent.parent / "local").resolve()
    output = args.output.resolve()
    if not output.is_relative_to(private):
        parser.error("mask audit must stay under ignored local/")
    if output.exists():
        raise FileExistsError(output)
    report = audit(args.source)
    output.parent.mkdir(parents=True, exist_ok=True)
    with output.open("x", encoding="utf-8") as stream:
        json.dump(report, stream, indent=2, allow_nan=False)
    print(json.dumps({"draws": len(report["draws"]), "materials": sorted({d["material"] for d in report["draws"]})}))


if __name__ == "__main__":
    main()
