"""Hash-guarded linear sweep of the private analysis PE for literal operands.

Prints every instruction whose operand text contains one of the requested hex
literals (for example a structure displacement such as 0xd220 or a string
address pushed as an immediate). Linear sweep can desynchronise on data inside
code sections, so hits are candidates to confirm with aligned disassembly or
Ghidra, not proof of use.
"""
import argparse
import hashlib
import json
import os
import re
import sys
from pathlib import Path

# A worktree has no local/; GRID_PROJECT_ROOT names the checkout that owns it.
ROOT = Path(os.environ.get('GRID_PROJECT_ROOT') or Path(__file__).resolve().parent.parent)
sys.path.insert(0, str(ROOT / 'local/tools/python'))
sys.path.insert(0, str(Path(__file__).resolve().parent))
from capstone import Cs, CS_ARCH_X86, CS_MODE_32  # noqa: E402
from inspect_installation import inspect_pe  # noqa: E402

IMAGE_SHA256 = 'aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514'


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('literals', nargs='+', help='hex literals, e.g. 0xd220')
    parser.add_argument('--strings', nargs='*', default=[],
                        help='ASCII strings whose addresses are also searched')
    args = parser.parse_args()
    output = args.output.resolve()
    if not output.is_relative_to((ROOT / 'local').resolve()):
        raise ValueError('Evidence output must stay private under local/')
    source = ROOT / 'local/compatibility/grid-unpacked.exe'
    raw = source.read_bytes()
    digest = hashlib.sha256(raw).hexdigest()
    if digest != IMAGE_SHA256:
        raise ValueError('Analysis image identity mismatch')
    pe = inspect_pe(source)
    base = int(pe['image_base'], 16)
    literals = {int(v, 16) for v in args.literals}
    strings = {}
    for text in args.strings:
        needle = text.encode('ascii') + b'\0'
        for match in re.finditer(re.escape(needle), raw):
            for section in pe['sections']:
                delta = match.start() - section['raw_offset']
                if 0 <= delta < section['raw_size'] and (match.start() == 0 or raw[match.start() - 1] == 0):
                    address = base + section['rva'] + delta
                    strings.setdefault(text, []).append(hex(address))
                    literals.add(address)
    patterns = {v: re.compile(r'(?<![0-9a-fx])' + re.escape(hex(v)) + r'(?![0-9a-f])') for v in literals}
    hits = []
    md = Cs(CS_ARCH_X86, CS_MODE_32)
    md.skipdata = True
    for section in pe['sections']:
        if not section['name'].startswith('.text'):
            continue
        start = section['raw_offset']
        code = raw[start:start + section['raw_size']]
        address = base + section['rva']
        for ins in md.disasm(code, address):
            text = ins.op_str
            for value, pattern in patterns.items():
                if pattern.search(text):
                    hits.append({'address': f'{ins.address:08x}', 'literal': hex(value),
                                 'instruction': f'{ins.mnemonic} {text}'})
    with output.open('x', encoding='utf-8') as handle:
        json.dump({'source_sha256': digest, 'strings': strings, 'hits': hits}, handle, indent=1)
    for text, addresses in strings.items():
        print('string', text, addresses)
    for hit in hits:
        print(hit['address'], hit['literal'], hit['instruction'])


if __name__ == '__main__':
    main()
