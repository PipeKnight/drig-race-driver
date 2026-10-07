"""Resolve field-name constants in private candidate decompilations, not semantics."""
import argparse
import hashlib
import json
import re
from pathlib import Path
from inspect_installation import inspect_pe


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('directory', type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    private = (root/'local').resolve()
    directory = args.directory.resolve()
    assert directory.is_relative_to(private)
    source = private/'compatibility/grid-unpacked.exe'
    raw = source.read_bytes()
    assert hashlib.sha256(raw).hexdigest() == 'aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514'
    pe = inspect_pe(source)
    base = int(pe['image_base'], 16)

    def at(va, size):
        for section in pe['sections']:
            delta = va-base-section['rva']
            if 0 <= delta and delta+size <= section['raw_size']:
                return raw[section['raw_offset']+delta:section['raw_offset']+delta+size]
        raise ValueError('range not file backed')

    result = {}
    for file in sorted(directory.glob('*.c.txt')):
        text = file.read_text()
        names = {}
        unmapped = []
        for address in sorted(set(re.findall(r'DAT_([0-9a-f]{8})', text))):
            try:
                value = at(int(address, 16), 256).split(b'\0', 1)[0]
            except ValueError:
                unmapped.append(address)
                continue
            if value and all(32 <= byte <= 126 for byte in value):
                names[address] = value.decode('ascii')
        calls = []
        for line in text.splitlines():
            if 'FUN_00a67b' in line:
                calls.append(dict(statement=line.strip(), names={k:v for k,v in names.items() if k in line}))
        result[file.name] = dict(constants=names, field_calls=calls, non_file_backed=unmapped)
    with (directory/'field-name-linkage.json').open('x') as out:
        json.dump(result, out, indent=2)
    print(json.dumps(result, indent=2))
