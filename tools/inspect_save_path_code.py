"""Inspect bounded save-path strings/instructions in the hash-identified private PE."""
import hashlib
import json
from pathlib import Path
import sys

from inspect_installation import inspect_pe


def main():
    root = Path(__file__).resolve().parent.parent
    target = root / "local/compatibility/grid-unpacked.exe"
    raw = target.read_bytes()
    if hashlib.sha256(raw).hexdigest() != "aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514":
        raise ValueError("Private reference hash mismatch")
    pe = inspect_pe(target)

    def at(va, size):
        rva = va - int(pe["image_base"], 16)
        for section in pe["sections"]:
            delta = rva - section["rva"]
            if 0 <= delta and delta + size <= section["raw_size"]:
                start = section["raw_offset"] + delta
                return raw[start:start + size]
        raise ValueError("Requested range is not file backed")

    sys.path.insert(0, str(root / "local/tools/python"))
    from capstone import Cs, CS_ARCH_X86, CS_MODE_32
    decoder = Cs(CS_ARCH_X86, CS_MODE_32)
    strings = {}
    for va in (0xce018c, 0xce0184, 0xce0178, 0xce02d0):
        text = at(va, 256).split(b"\0", 1)[0]
        strings[hex(va)] = text.decode("ascii", errors="replace")
    instructions = {}
    for va in (0x5b9680, 0x5b9750):
        instructions[hex(va)] = [
            {"va": hex(i.address), "mnemonic": i.mnemonic, "operands": i.op_str}
            for i in decoder.disasm(at(va, 128), va)
        ]
    out = root / "local/autosave/static-survey/path-code.json"
    out.write_text(json.dumps({"strings": strings, "instructions": instructions}, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"strings": strings, "instruction_counts": {k: len(v) for k, v in instructions.items()}}, indent=2))


if __name__ == "__main__":
    main()
