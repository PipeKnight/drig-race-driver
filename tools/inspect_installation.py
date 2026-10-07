"""Read-only GRID Phase 0 inventory. Uses only Python's standard library."""
import argparse
import collections
import csv
import datetime as dt
import hashlib
import json
from pathlib import Path
import struct


def sha256(path):
    h = hashlib.sha256()
    with path.open('rb') as f:
        for block in iter(lambda: f.read(1024 * 1024), b''):
            h.update(block)
    return h.hexdigest()


def inspect_pe(path):
    b = path.read_bytes()

    def unpack(fmt, offset):
        if offset < 0 or offset + struct.calcsize(fmt) > len(b):
            raise ValueError(f'PE read outside file at {offset:#x}')
        return struct.unpack_from(fmt, b, offset)

    if b[:2] != b'MZ':
        raise ValueError('Missing DOS signature')
    pe, = unpack('<I', 0x3c)
    if b[pe:pe + 4] != b'PE\0\0':
        raise ValueError('Missing PE signature')
    machine, count, timestamp, _, _, opt_size, flags = unpack('<HHIIIHH', pe + 4)
    opt = pe + 24
    magic, = unpack('<H', opt)
    if magic not in (0x10b, 0x20b) or not 1 <= count <= 96:
        raise ValueError('Unsupported PE optional header or section count')
    is64 = magic == 0x20b
    image_base, = unpack('<Q' if is64 else '<I', opt + (24 if is64 else 28))
    entry, = unpack('<I', opt + 16)
    image_size, header_size = unpack('<II', opt + 56)
    subsystem, dll_flags = unpack('<HH', opt + 68)
    sections = []
    for i in range(count):
        o = opt + opt_size + i * 40
        name = b[o:o + 8].rstrip(b'\0').decode('ascii', 'replace')
        virtual_size, rva, raw_size, raw_offset = unpack('<IIII', o + 8)
        characteristic, = unpack('<I', o + 36)
        sections.append(dict(name=name, rva=rva, virtual_size=virtual_size,
                             raw_size=raw_size, raw_offset=raw_offset,
                             characteristics=f'{characteristic:#x}'))

    def rva_offset(rva):
        if 0 <= rva < header_size and rva < len(b):
            return rva
        for s in sections:
            delta = rva - s['rva']
            if 0 <= delta < s['raw_size'] and s['raw_offset'] + delta < len(b):
                return s['raw_offset'] + delta
        raise ValueError(f'RVA has no file-backed section: {rva:#x}')

    def cstring(rva):
        o = rva_offset(rva)
        end = b.find(b'\0', o, min(o + 4096, len(b)))
        if end < 0:
            raise ValueError('Unterminated PE string')
        return b[o:end].decode('ascii', 'replace')

    directories = opt + (112 if is64 else 96)
    directory_count, = unpack('<I', directories - 4)
    imports = []
    if directory_count > 1:
        import_rva, import_size = unpack('<II', directories + 8)
        if import_rva:
            o = rva_offset(import_rva)
            for i in range(min(import_size // 20, 4096)):
                original, _, _, name_rva, first = unpack('<IIIII', o + i * 20)
                if not any((original, name_rva, first)):
                    break
                thunk = rva_offset(original or first)
                symbols = []
                for j in range(65536):
                    val, = unpack('<Q' if is64 else '<I', thunk + j * (8 if is64 else 4))
                    if val == 0:
                        break
                    mask = 1 << (63 if is64 else 31)
                    symbols.append(f'ordinal:{val & 0xffff}' if val & mask else cstring(val + 2))
                else:
                    raise ValueError('Unterminated import thunk')
                imports.append(dict(dll=cstring(name_rva), symbols=symbols))
    fault_rva = 0x801796
    try:
        fault_offset = rva_offset(fault_rva)
        fault = dict(rva=hex(fault_rva), preferred_va=hex(image_base + fault_rva),
                     file_offset=hex(fault_offset), section=next(s['name'] for s in sections
                         if s['rva'] <= fault_rva < s['rva'] + max(s['virtual_size'], s['raw_size'])))
    except (ValueError, StopIteration):
        fault = None
    return dict(machine=hex(machine), architecture={0x14c: 'x86 (32-bit)', 0x8664: 'x86-64'}.get(machine, 'unknown'),
                optional_header=hex(magic), timestamp=hex(timestamp),
                timestamp_utc=dt.datetime.fromtimestamp(timestamp, dt.timezone.utc).isoformat(),
                image_base=hex(image_base), entry_rva=hex(entry), image_size=image_size,
                coff_characteristics=hex(flags), large_address_aware=bool(flags & 0x20),
                subsystem=subsystem, dll_characteristics=hex(dll_flags),
                sections=sections, imports=imports, recorded_crash_location=fault,
                caveats=['Imports describe static dependencies only; runtime-loaded modules require observation.',
                         'PE timestamp is compiler metadata, not proof of distribution or integrity.',
                         'Crash location mapping is not a root-cause determination.'])


def main():
    p = argparse.ArgumentParser()
    p.add_argument('--game', type=Path, required=True)
    p.add_argument('--out', type=Path, required=True)
    args = p.parse_args()
    game = args.game.resolve(strict=True)
    out = args.out.resolve()
    if out == game or game in out.parents:
        p.error('Output must be outside original installation')
    out.mkdir(parents=True, exist_ok=True)
    files = sorted((f for f in game.rglob('*') if f.is_file()), key=lambda f: str(f).lower())
    counts = collections.Counter()
    size = 0
    with (out / 'installation-files.csv').open('w', encoding='utf-8', newline='') as f:
        writer = csv.writer(f)
        writer.writerow(['relative_path', 'size', 'mtime_utc'])
        for path in files:
            s = path.stat()
            size += s.st_size
            counts[path.suffix.lower() or '(none)'] += 1
            writer.writerow([path.relative_to(game).as_posix(), s.st_size,
                             dt.datetime.fromtimestamp(s.st_mtime, dt.timezone.utc).isoformat()])
    selected = [f for f in files if f.parent == game and f.suffix.lower() in ('.exe', '.dll')]
    selected += [f for f in files if f.parent == game / 'system' and f.suffix == '.xml']
    for rel in ('database/database.bin', 'raceload.jpk', 'cars/lac/lac_gen_high.pssg',
                'cars/lac/lac_highLOD.pssg', 'cars/lac/lac.csv',
                'tracks/europe/donington/route_0/track.jpk',
                'tracks/europe/donington/route_0/route.clm'):
        f = game / rel
        if f.is_file():
            selected.append(f)
    # Small representative car and track samples; presence does not establish race pairing.
    selected += sorted((game / 'cars' / 'lac').rglob('*.pssg'))[:2]
    selected += sorted((game / 'tracks' / 'europe' / 'donington').rglob('track.jpk'))[:1]
    hashes = [dict(path=f.relative_to(game).as_posix(), size=f.stat().st_size,
                   sha256=sha256(f)) for f in sorted(set(selected))]
    result = dict(collected_utc=dt.datetime.now(dt.timezone.utc).isoformat(), game=str(game),
                  file_count=len(files), total_bytes=size, extensions=dict(counts.most_common()),
                  selected_hashes=hashes, pe=inspect_pe(game / 'grid.exe'))
    (out / 'installation.json').write_text(json.dumps(result, indent=2), encoding='utf-8')
    print(json.dumps({k: result[k] for k in ('file_count', 'total_bytes', 'extensions')}, indent=2))
    print(json.dumps(result['pe'], indent=2))


if __name__ == '__main__':
    main()
