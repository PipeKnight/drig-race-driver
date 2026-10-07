"""Export captured main-module pages as a private, non-executable Ghidra raw image.

This is an analysis snapshot, not a replacement executable. Missing pages are
zero-filled and explicitly recorded. No original installation files are written.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct
from inspect_minidump import inspect


def main():
    p = argparse.ArgumentParser()
    p.add_argument('dump', type=Path)
    p.add_argument('--out-dir', type=Path, required=True)
    args = p.parse_args()
    info = inspect(args.dump)
    module = next(m for m in info['modules'] if Path(m['name']).name.lower() == 'grid.exe')
    b = args.dump.read_bytes()
    count, directory = struct.unpack_from('<II', b, 8)
    ranges = []
    for i in range(count):
        kind, _, o = struct.unpack_from('<III', b, directory + i * 12)
        if kind == 5:
            n, = struct.unpack_from('<I', b, o)
            for j in range(n):
                ranges.append(struct.unpack_from('<QII', b, o + 4 + j * 16))
        elif kind == 9:
            n, rva = struct.unpack_from('<QQ', b, o)
            for j in range(n):
                start, size = struct.unpack_from('<QQ', b, o + 16 + j * 16)
                ranges.append((start, size, rva))
                rva += size
    image = bytearray(module['size'])
    covered = bytearray(module['size'])
    base = module['base']
    for addr, size, rva in ranges:
        start, end = max(addr, base), min(addr + size, base + len(image))
        if start >= end:
            continue
        source = rva + start - addr
        if source < 0 or source + end - start > len(b):
            raise ValueError('Invalid dump memory range')
        image[start - base:end - base] = b[source:source + end - start]
        covered[start - base:end - base] = b'\1' * (end - start)
    missing = []
    cursor = 0
    while cursor < len(covered):
        if covered[cursor]:
            cursor += 1
            continue
        end = covered.find(b'\1', cursor)
        if end < 0:
            end = len(covered)
        missing.append(dict(rva=hex(cursor), size=end - cursor))
        cursor = end
    args.out_dir.mkdir(parents=True, exist_ok=True)
    target = args.out_dir / 'grid-runtime-image.bin'
    if target.exists():
        p.error('Refusing to overwrite an existing runtime image')
    target.write_bytes(image)
    report = dict(source_dump=str(args.dump), dump_sha256=info['sha256'],
                  module=module, image_size=len(image), captured_bytes=sum(covered),
                  missing_ranges=missing, image_sha256=hashlib.sha256(image).hexdigest(),
                  importer='Ghidra raw binary; x86:LE:32:default; windows compiler spec',
                  image_base=hex(base), caveat='Memory-layout snapshot. Not an executable, not a Steam integrity reference. Zero-filled missing ranges must not be treated as observed bytes.')
    (args.out_dir / 'runtime-image.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
