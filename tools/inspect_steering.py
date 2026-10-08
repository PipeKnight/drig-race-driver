"""Hash-guarded, private aligned steering instructions and scalar constants."""
import hashlib
import json
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'local/tools/python'))
from capstone import Cs, CS_ARCH_X86, CS_MODE_32
from inspect_installation import inspect_pe

source = ROOT / 'local/compatibility/grid-unpacked.exe'
raw = source.read_bytes()
assert hashlib.sha256(raw).hexdigest() == 'aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514'
pe = inspect_pe(source)

def at(address, size):
    for section in pe['sections']:
        delta = address - 0x400000 - section['rva']
        if 0 <= delta and delta + size <= section['raw_size']:
            return raw[section['raw_offset'] + delta:section['raw_offset'] + delta + size]
    raise ValueError('range not file backed')

result = {'source_sha256': hashlib.sha256(raw).hexdigest(), 'instructions': {}, 'constants': {}}
for address, size in [(0x98f790, 0x12a), (0x9b3110, 0xd26), (0x9b40d0, 0x160), (0xa03670, 0x3400), (0xbf7112, 0x100), (0x98ebf0, 0x230), (0x9aec60, 0x7000), (0x9ae5a0, 0x300), (0xa0c9f0, 0x2400), (0xa0e880, 0x1400), (0x9b6350, 0x210), (0x9bd7f0, 0x700)]:
    instructions = []
    for i in Cs(CS_ARCH_X86, CS_MODE_32).disasm(at(address, size), address):
        instructions.append({'address': hex(i.address), 'mnemonic': i.mnemonic, 'operands': i.op_str})
    result['instructions'][hex(address)] = instructions
for address in [0xcaf888, 0xcaf88c, 0xcb01ac, 0xcb870c, 0xcb0c44, 0xd15038, 0xce769c, 0xcb0948, 0xcb093c, 0xcb8da4, 0xd1633c, 0xcb3ba8, 0xcb47e0, 0xcb8200, 0xcb0dc8, 0xcb31d4, 0xcb0f1c, 0xcb0de4, 0xcf8c6c, 0xcb46a0]:
    data = at(address, 4)
    result['constants'][hex(address)] = {'word': data.hex(), 'f32': struct.unpack('<f', data)[0]}
out = Path(sys.argv[1]).resolve()
result['masks'] = {hex(address): at(address, 16).hex() for address in [0xcaf430, 0xcaf460]}
assert out.is_relative_to((ROOT / 'local').resolve())
with out.open('x') as writer:
    json.dump(result, writer, indent=2)
print('private_output=' + str(out))
