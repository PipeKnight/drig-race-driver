"""Independent selected source metadata and malformed semantic controls."""
import argparse
import copy
import hashlib
import json
import struct
import subprocess
from pathlib import Path
from validate_binxml import decode

ROOT = Path(__file__).resolve().parents[1]
SOURCE = Path("C:/Program Files (x86)/Steam/steamapps/common/Grid/audio/carsetup.bin")


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def encode(tree):
    """Independent binary XML writer for private mutations, not a source asset."""
    strings, string_ids, nodes = [], {}, [tree]
    def string(text):
        if text not in string_ids:
            string_ids[text] = len(strings)
            strings.append(text.encode() + b"\0")
        return string_ids[text]
    elements, attributes = [], []
    cursor = 0
    while cursor < len(nodes):
        node = nodes[cursor]
        name = string(node.tag)
        value = string(node.text) if node.text else 0
        start = len(attributes)
        attributes.extend((string(k), string(v)) for k, v in node.attrib.items())
        children = list(node)
        child_start = len(nodes) if children else 0
        nodes.extend(children)
        elements.append((name, value, len(node.attrib), start, len(children), child_start))
        cursor += 1
    blob, offsets = b"", []
    for text in strings:
        offsets.append(len(blob))
        blob += text
    blob += b"\0" * (-len(blob) % 4)
    sections = [blob, struct.pack(f"<{len(offsets)}I", *offsets),
                b"".join(struct.pack("<6I", *r) for r in elements), b"".join(struct.pack("<2I", *r) for r in attributes)]
    body = struct.pack("<II", 0x72522217, len(sections[0]) + len(sections[1]) + 16)
    for magic, section in zip((0x7252221d, 0x7252221e, 0x7252221b, 0x7252221c), sections):
        body += struct.pack("<II", magic, len(section)) + section
    return struct.pack("<II", 0x7252221a, len(body)) + body


def expected(car):
    rows = []
    for key, tag in (("engine_ini", "EngineIniFile"), ("gear_ini", "GearIniFile"), ("engine_bank", "EngineBank"), ("mechanics_bank", "MechBank")):
        rows.append([key, car.find(tag).get("value")])
    bank = car.find("EngineBank")
    rows += [["throttle_frames", bank.get("throttleLength")], ["idle_frames", bank.get("idleLength")], ["open_cockpit", car.get("openCockpit")]]
    def bits(value):
        return struct.pack("<f", float(value))[::-1].hex()
    for key in ("gear", "engine", "exhaust"):
        rows.append([key + "_trim", bits(car.find("VolumeTrims").get(key))])
    for key, tag in (("engine_bay", "EngineBay"), ("exhaust", "Exhaust"), ("gearbox", "GearBox"), ("cockpit", "Cockpit")):
        rows.append([key, *[bits(car.find("Offsets/" + tag).get(axis)) for axis in ("x", "y", "z")]])
    return ("\n".join("\t".join(row) for row in rows) + "\n").encode()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    out = args.output.resolve()
    assert out.is_relative_to((ROOT / "local").resolve()) and not out.exists()
    out.mkdir()
    assert sha(SOURCE) == "264f6744ab4566e4b7fc7da708621c8df7b883c9e8fccaf3a51ce92b81debbbe"
    _, tree, counts = decode(SOURCE.read_bytes())
    car, = [c for c in tree.find("Cars") if c.tag == "Car" and c.get("name") == "lac"]
    wanted = expected(car)
    reencoded = out / "reencoded.bin"
    reencoded.write_bytes(encode(tree))
    assert decode(reencoded.read_bytes())[1].find("Cars/Car[@name='lac']/EngineBank").attrib == car.find("EngineBank").attrib
    profiles = [(p, ROOT / f"target/{p}/grid-audio-setup.exe") for p in ("debug", "release")]
    for label, exe in profiles:
        for name, path in (("source", SOURCE), ("reencoded", reencoded)):
            report = out / f"{name}-{label}.tsv"
            result = subprocess.run([str(exe), str(path), "lac", str(report)], capture_output=True, text=True, timeout=30)
            (out / f"{name}-{label}.txt").write_text(result.stdout + result.stderr)
            assert result.returncode == 0 and report.read_bytes() == wanted
    mutations = [
        ("zero_length", lambda root, c: c.find("EngineBank").set("idleLength", "0")),
        ("overflow_length", lambda root, c: c.find("EngineBank").set("throttleLength", "4294967296")),
        ("negative_length", lambda root, c: c.find("EngineBank").set("idleLength", "-1")),
        ("nonfinite_trim", lambda root, c: c.find("VolumeTrims").set("engine", "NaN")),
        ("nonfinite_offset", lambda root, c: c.find("Offsets/Exhaust").set("x", "inf")),
        ("path_traversal", lambda root, c: c.find("EngineIniFile").set("value", "audio/cars/../lacengine.ini")),
        ("absolute_path", lambda root, c: c.find("EngineIniFile").set("value", "C:/lacengine.ini")),
        ("unsafe_bank", lambda root, c: c.find("EngineBank").set("value", "../eng_lac")),
        ("duplicate_car", lambda root, c: root.append(copy.deepcopy(c))),
        ("duplicate_engine", lambda root, c: c.append(copy.deepcopy(c.find("EngineBank")))),
        ("missing_engine", lambda root, c: c.remove(c.find("EngineBank"))),
        ("missing_offset", lambda root, c: c.find("Offsets").remove(c.find("Offsets/Exhaust"))),
        ("unknown_type", lambda root, c: c.set("type", "unknown")),
        ("invalid_flag", lambda root, c: c.set("openCockpit", "yes")),
    ]
    guards = []
    for name, mutate in mutations:
        changed = copy.deepcopy(tree)
        selected, = [c for c in changed.find("Cars") if c.tag == "Car" and c.get("name") == "lac"]
        mutate(changed.find("Cars"), selected)
        path = out / f"guard-{name}.bin"
        path.write_bytes(encode(changed))
        decode(path.read_bytes())  # Framing remains valid: these are semantic guards.
        for label, exe in profiles:
            report = out / f"guard-{name}-{label}.tsv"
            result = subprocess.run([str(exe), str(path), "lac", str(report)], capture_output=True, text=True, timeout=30)
            (out / f"guard-{name}-{label}.txt").write_text(result.stdout + result.stderr)
            assert result.returncode != 0 and not report.exists(), (name, label)
            guards.append(dict(name=name, profile=label, rejected=True))
    files = [SOURCE, *(p for _, p in profiles), ROOT / "crates/grid-formats/src/audio_car_setup.rs",
             ROOT / "crates/grid-probes/src/bin/grid-audio-setup.rs", Path(__file__)] + list(out.iterdir())
    result = dict(passed=True, source_counts=counts, typed_rows=14, debug_release_source_reencoded_exact=True,
                  guards=guards, identities={str(p): sha(p) for p in files},
                  scope="Selected Lacetti source metadata and valid-framing semantic guards; no live factory/backend/audio equivalence.")
    (out / "validation.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(dict(passed=True, typed_rows=14, guards=len(guards))))


if __name__ == "__main__":
    main()
