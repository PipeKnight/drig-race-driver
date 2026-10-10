"""Hash-guarded file-backed words and bounded raw address candidates, not live data."""
import argparse
import hashlib
import json
from pathlib import Path
import struct
from inspect_installation import inspect_pe

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('addresses', nargs='+', help='hex virtual addresses')
    parser.add_argument('--pointer-candidates', action='store_true')
    parser.add_argument('--byte-candidates', action='store_true',
                        help='Include unaligned raw occurrences, not established code references')
    args = parser.parse_args()
    if len(args.addresses) > 64:
        raise ValueError('At most 64 addresses')
    if not args.output.resolve().is_relative_to((ROOT / 'local').resolve()):
        raise ValueError('Private output only')
    source = ROOT / 'local/compatibility/grid-unpacked.exe'
    raw = source.read_bytes()
    digest = hashlib.sha256(raw).hexdigest()
    if digest != 'aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514':
        raise ValueError('Analysis image identity mismatch')
    pe = inspect_pe(source)
    base = int(pe['image_base'], 16)
    results = []
    for item in args.addresses:
        address = int(item, 16)
        row = {'address': f'{address:08x}', 'file_backed': False}
        for section in pe['sections']:
            delta = address - base - section['rva']
            if 0 <= delta and delta + 4 <= section['raw_size']:
                word = raw[section['raw_offset'] + delta:section['raw_offset'] + delta + 4]
                number, = struct.unpack('<f', word)
                row.update(file_backed=True, bytes_hex=word.hex(),
                           uint32=struct.unpack('<I', word)[0], float32_repr=repr(number))
                break
        if args.pointer_candidates:
            needle = struct.pack('<I', address)
            candidates = []
            for section in pe['sections']:
                for offset in range(0, section['raw_size'] - 3, 4):
                    start = section['raw_offset'] + offset
                    if raw[start:start + 4] == needle:
                        candidates.append(f'{base + section["rva"] + offset:08x}')
                        if len(candidates) > 256:
                            raise ValueError('Pointer candidate limit exceeded')
            row['aligned_pointer_candidates'] = candidates
        if args.byte_candidates:
            needle = struct.pack('<I', address)
            candidates = []
            for section in pe['sections']:
                data = raw[section['raw_offset']:section['raw_offset'] + section['raw_size']]
                start = 0
                while (offset := data.find(needle, start)) >= 0:
                    candidates.append(f'{base + section["rva"] + offset:08x}')
                    if len(candidates) > 256:
                        raise ValueError('Byte occurrence candidate limit exceeded')
                    start = offset + 1
            row['byte_occurrence_candidates'] = candidates
        results.append(row)
    with args.output.open('x', encoding='utf-8') as stream:
        json.dump({'source_sha256': digest, 'results': results,
                   'limits': 'Private file-backed words; numeric interpretations and raw address '
                             'occurrences are candidates, not live values or established code xrefs.'}, stream, indent=2)
    print(json.dumps(results))


if __name__ == '__main__':
    main()
