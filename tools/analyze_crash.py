"""Targeted runtime disassembly; Capstone must already exist under local/tools/python."""
import json
from pathlib import Path
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'local' / 'tools' / 'python'))
import capstone
from inspect_minidump import inspect

out = Path(__file__).resolve().parents[1] / 'local' / 'phase0'
dump = Path(sys.argv[1]) if len(sys.argv) > 1 else out / 'dumps' / 'grid.exe_261006_001146.dmp'
base = inspect(dump)
ebp = int(base['registers']['ebp'], 16)
eax = int(base['registers']['eax'], 16)
edi = int(base['registers']['edi'], 16)
windows = [(0xc01720, 256), (0xa6fc30, 240), (0xa71100, 240),
           (ebp - 12, 160), (eax, 256), (edi, 256), (0xa6fa00, 512)]
if len(sys.argv) > 2:
    windows = [(int(a, 16), 256) for a in sys.argv[2:]]
data = inspect(dump, windows)
md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)
lines = [f'Capstone {capstone.__version__}; target x86; runtime dump {dump.name}',
         'Disassembly started at known fault/call-return boundaries; other starts may be unaligned.']
for w in data['memory_windows']:
    addr = int(w['address'], 16)
    lines.append(f'\nMemory {addr:#x}')
    if not w['bytes']:
        lines.append('Not captured')
        continue
    b = bytes.fromhex(w['bytes'])
    if addr < 0x400000 or addr > 0x1000000:
        lines.append(''.join(chr(c) if 32 <= c < 127 else '.' for c in b))
        continue
    for i in md.disasm(b, addr):
        lines.append(f'{i.address:08x} {i.bytes.hex():20s} {i.mnemonic} {i.op_str}')
# Decode exactly at the fault, avoiding alignment assumptions.
lines.append('\nExact fault boundary')
for i in md.disasm(bytes.fromhex(data['instruction_bytes']), 0xc01796):
    lines.append(f'{i.address:08x} {i.bytes.hex():20s} {i.mnemonic} {i.op_str}')
suffix = '-targeted' if len(sys.argv) > 2 else ''
(out / f'runtime-disassembly{suffix}.txt').write_text('\n'.join(lines), encoding='utf-8')
(out / f'crash-memory-windows{suffix}.json').write_text(json.dumps(data['memory_windows'], indent=2), encoding='utf-8')
print('\n'.join(lines))
