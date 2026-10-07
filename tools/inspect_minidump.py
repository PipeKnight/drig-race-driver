"""Extract exception, x86 registers and raw stack candidates, not a stack unwind."""
import argparse
import hashlib
import json
from pathlib import Path
import struct


def inspect(path, peeks=()):
    b = path.read_bytes()
    def u(fmt, offset):
        if offset < 0 or offset + struct.calcsize(fmt) > len(b):
            raise ValueError(f'Dump read outside file: {offset:#x}')
        return struct.unpack_from(fmt, b, offset)
    if b[:4] != b'MDMP':
        raise ValueError('Not a minidump')
    count, directory = u('<II', 8)
    if count > 1024:
        raise ValueError('Unreasonable stream count')
    streams = {}
    for i in range(count):
        kind, size, rva = u('<III', directory + 12 * i)
        streams[kind] = (size, rva)
    modules = []
    if 4 in streams:
        _, o = streams[4]
        n, = u('<I', o)
        for i in range(min(n, 4096)):
            m = o + 4 + 108 * i
            base, size, _, timestamp, name_rva = u('<QIIII', m)
            name_size, = u('<I', name_rva)
            name = b[name_rva + 4:name_rva + 4 + name_size].decode('utf-16-le', 'replace')
            modules.append(dict(base=base, size=size, name=name, timestamp=hex(timestamp)))
    ranges = []
    if 5 in streams:
        _, o = streams[5]
        n, = u('<I', o)
        for i in range(min(n, 1000000)):
            addr, size, rva = u('<QII', o + 4 + i * 16)
            ranges.append((addr, size, rva))
    if 9 in streams:
        _, o = streams[9]
        n, rva = u('<QQ', o)
        for i in range(min(n, 1000000)):
            addr, size = u('<QQ', o + 16 + i * 16)
            ranges.append((addr, size, rva))
            rva += size
    def read(addr, size):
        for start, length, rva in ranges:
            if start <= addr and addr + size <= start + length:
                o = rva + addr - start
                if o + size <= len(b):
                    return b[o:o + size]
        return None
    def location(addr):
        for m in modules:
            if m['base'] <= addr < m['base'] + m['size']:
                return Path(m['name']).name + '+' + hex(addr - m['base'])
        return None
    _, e = streams[6]
    thread, = u('<I', e)
    code, flags = u('<II', e + 8)
    address, = u('<Q', e + 24)
    n, = u('<I', e + 32)
    parameters = u('<' + 'Q' * min(n, 15), e + 40)
    ctx_size, ctx = u('<II', e + 160)
    context_flags, = u('<I', ctx)
    if not context_flags & 0x10000 or ctx_size < 204:
        raise ValueError('Expected x86 context')
    registers = {name: u('<I', ctx + off)[0] for name, off in
                 [('edi', 156), ('esi', 160), ('ebx', 164), ('edx', 168), ('ecx', 172),
                  ('eax', 176), ('ebp', 180), ('eip', 184), ('esp', 196)]}
    around = read(address, 48)
    stack = read(registers['esp'], 1024)
    candidates = []
    if stack:
        for i in range(0, len(stack), 4):
            addr, = struct.unpack_from('<I', stack, i)
            label = location(addr)
            if label:
                candidates.append(dict(stack_offset=hex(i), value=hex(addr), location=label))
    frame_chain = []
    frame = registers['ebp']
    for _ in range(32):
        pair = read(frame, 8)
        if not pair:
            break
        parent, ret = struct.unpack('<II', pair)
        frame_chain.append(dict(ebp=hex(frame), return_address=hex(ret), location=location(ret)))
        if parent <= frame or parent - frame > 1024 * 1024:
            break
        frame = parent
    windows = []
    for addr, size in peeks:
        memory = read(addr, size)
        windows.append(dict(address=hex(addr), bytes=memory.hex() if memory else None))
    return dict(file=str(path), sha256=hashlib.sha256(b).hexdigest(),
                exception=dict(thread_id=thread, code=hex(code), flags=hex(flags),
                               address=hex(address), location=location(address),
                               parameters=[hex(v) for v in parameters]),
                registers={k: hex(v) for k, v in registers.items()},
                instruction_bytes=around.hex(' ') if around else None,
                frame_pointer_chain=frame_chain,
                raw_stack_candidates=candidates[:60], modules=modules, memory_windows=windows,
                caveats=['Raw stack candidates are possible pointers, not verified call frames.',
                         'Frame-pointer chain is provisional; optimized code may reuse EBP.',
                         'Runtime bytes may differ from the protected on-disk executable.'])


if __name__ == '__main__':
    p = argparse.ArgumentParser()
    p.add_argument('dump', type=Path)
    p.add_argument('--out', type=Path, required=True)
    args = p.parse_args()
    data = inspect(args.dump)
    args.out.write_text(json.dumps(data, indent=2), encoding='utf-8')
    print(json.dumps({k: v for k, v in data.items() if k != 'modules'}, indent=2))
