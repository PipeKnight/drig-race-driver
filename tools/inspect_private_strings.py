"""Bounded, hash-guarded ASCII strings from the private analysis PE."""
import argparse
import hashlib
import json
from pathlib import Path
from inspect_installation import inspect_pe

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('addresses', nargs='+', help='hex virtual addresses, up to 64')
    args = parser.parse_args()
    out = args.output.resolve()
    assert out.is_relative_to((ROOT/'local').resolve()) and len(args.addresses) <= 64
    image = ROOT/'local/compatibility/grid-unpacked.exe'
    raw = image.read_bytes()
    sha = hashlib.sha256(raw).hexdigest()
    assert sha == 'aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514'
    pe = inspect_pe(image)
    strings = []
    for item in args.addresses:
        address = int(item, 16)
        for section in pe['sections']:
            delta = address-int(pe['image_base'],16)-section['rva']
            if 0 <= delta < section['raw_size']:
                start = section['raw_offset']+delta
                data = raw[start:start+min(256,section['raw_size']-delta)]
                end = data.find(b'\0')
                if end < 0: raise ValueError('No bounded terminator')
                strings.append(dict(address=f'{address:08x}', text=data[:end].decode('ascii')))
                break
        else:
            raise ValueError('Address is not file backed')
    with out.open('x',encoding='utf-8') as writer:
        json.dump(dict(analysis_sha256=sha,strings=strings,
                       limits='Static file-backed strings; caller/layout provenance is separate.'),writer,indent=2)
    print(json.dumps(strings))


if __name__ == '__main__':
    main()
