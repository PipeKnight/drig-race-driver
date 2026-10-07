"""Bounded offline x86 minidump thread/stack inspection; no stack unwind."""
import argparse
import hashlib
import json
from pathlib import Path, PureWindowsPath
import struct

ROOT = Path(__file__).resolve().parents[1]


def inspect(path):
    if path.stat().st_size > 512 * 1024 * 1024:
        raise ValueError('Mini dump exceeds 512 MiB inspection bound')
    data = path.read_bytes()

    def unpack(fmt, offset):
        if offset < 0 or offset + struct.calcsize(fmt) > len(data):
            raise ValueError('Read outside dump')
        return struct.unpack_from(fmt, data, offset)

    def span(offset, size):
        if offset < 0 or size < 0 or offset + size > len(data):
            raise ValueError('Span outside dump')
        return data[offset:offset + size]

    if span(0, 4) != b'MDMP':
        raise ValueError('Not a minidump')
    count, directory = unpack('<II', 8)
    if count > 1024:
        raise ValueError('Stream count bound')
    streams = {}
    for i in range(count):
        kind, size, offset = unpack('<III', directory + i * 12)
        span(offset, size)
        if kind == 0:  # Unused directory slots are legal padding.
            continue
        if kind in streams:
            raise ValueError('Duplicate stream')
        streams[kind] = (size, offset)
    modules = []
    size, offset = streams[4]
    n, = unpack('<I', offset)
    if n > 4096 or 4 + n * 108 > size:
        raise ValueError('Module stream bound')
    for i in range(n):
        base, length, _, _, name = unpack('<QIIII', offset + 4 + i * 108)
        name_len, = unpack('<I', name)
        if name_len > 65536 or name_len % 2:
            raise ValueError('Module name bound')
        label = PureWindowsPath(span(name + 4, name_len).decode('utf-16-le')).name
        modules.append({'base': base, 'size': length, 'name': label})

    def location(address):
        for module in modules:
            if module['base'] <= address < module['base'] + module['size']:
                return module['name'] + '+' + hex(address - module['base'])
        return None

    size, offset = streams[3]
    n, = unpack('<I', offset)
    if n > 4096 or 4 + n * 48 > size:
        raise ValueError('Thread stream bound')
    threads = []
    for i in range(n):
        tid, suspend, _, priority, teb, stack_va, stack_len, stack_rva, ctx_len, ctx = unpack(
            '<IIIIQQIIII', offset + 4 + i * 48)
        context = span(ctx, ctx_len)
        if ctx_len < 204 or not struct.unpack_from('<I', context)[0] & 0x10000:
            raise ValueError('Expected x86 context')
        regs = {name: struct.unpack_from('<I', context, pos)[0] for name, pos in
                [('edi', 156), ('esi', 160), ('ebx', 164), ('edx', 168),
                 ('ecx', 172), ('eax', 176), ('ebp', 180), ('eip', 184), ('esp', 196)]}
        stack = span(stack_rva, stack_len)
        candidates = []
        start = max(0, regs['esp'] - stack_va)
        for pos in range(start, min(len(stack), start + 1024) - 3, 4):
            word, = struct.unpack_from('<I', stack, pos)
            label = location(word)
            if label:
                candidates.append({'offset_from_esp': hex(stack_va + pos - regs['esp']),
                                   'address': hex(word), 'location': label})
        threads.append({'id': tid, 'suspend_count': suspend, 'priority': priority,
                        'teb': hex(teb), 'registers': {k: hex(v) for k, v in regs.items()},
                        'instruction_location': location(regs['eip']),
                        'raw_stack_candidates': candidates[:60]})
    return {'file': str(path), 'sha256': hashlib.sha256(data).hexdigest(),
            'size_bytes': len(data), 'threads': threads, 'modules': modules,
            'caveats': ['Stack candidates are possible pointers, not unwound call frames.',
                        'Suspend counts may include the dump capture mechanism.']}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('dump', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    for path in (args.dump, args.output):
        if not path.resolve().is_relative_to((ROOT / 'local').resolve()):
            raise ValueError('Private evidence only')
    result = inspect(args.dump)
    with args.output.open('x', encoding='utf-8') as stream:
        json.dump(result, stream, indent=2)
    print(json.dumps({'output': str(args.output), 'threads': len(result['threads']),
                      'suspended': [{k: t[k] for k in ('id', 'suspend_count', 'instruction_location')}
                                    for t in result['threads'] if t['suspend_count']]}))


if __name__ == '__main__':
    main()
