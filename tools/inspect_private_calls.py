"""Hash-guarded raw relative-call candidates; verify instruction alignment separately."""
import argparse
import hashlib
import json
import struct
from pathlib import Path

from inspect_installation import inspect_pe

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("output", type=Path)
    parser.add_argument("targets", nargs="+", help="up to 32 hexadecimal virtual addresses")
    args = parser.parse_args()
    assert 1 <= len(args.targets) <= 32
    output = args.output.resolve()
    assert output.is_relative_to((ROOT / "local").resolve()) and not output.exists()
    image = ROOT / "local/compatibility/grid-unpacked.exe"
    raw = image.read_bytes()
    sha = hashlib.sha256(raw).hexdigest()
    assert sha == "aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514"
    pe = inspect_pe(image)
    base = int(pe["image_base"], 16)
    targets = {int(v, 16): [] for v in args.targets}
    assert all(base <= target < base + 32 * 1024 * 1024 for target in targets)
    for section in pe["sections"]:
        if not int(section["characteristics"], 16) & 0x20000000:
            continue
        code = raw[section["raw_offset"]:section["raw_offset"] + section["raw_size"]]
        address = base + section["rva"]
        for opcode in (0xE8, 0xE9):
            cursor = 0
            while True:
                at = code.find(bytes([opcode]), cursor)
                if at < 0 or at + 5 > len(code):
                    break
                cursor = at + 1
                target = (address + at + 5 + struct.unpack_from("<i", code, at + 1)[0]) & 0xFFFFFFFF
                if target in targets:
                    assert len(targets[target]) < 2048
                    targets[target].append(dict(address=f"{address + at:08x}", opcode=f"{opcode:02x}"))
    result = dict(analysis_sha256=sha, script_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                  scope="Raw bytes in executable sections; may be unaligned or data. Confirm each candidate before treating it as a call or function boundary.",
                  targets={f"{target:08x}": rows for target, rows in targets.items()})
    with output.open("x", encoding="utf-8") as file:
        json.dump(result, file, indent=2)
        file.write("\n")
    print(json.dumps(result["targets"]))


if __name__ == "__main__":
    main()
