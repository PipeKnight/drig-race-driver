"""Prepare private BP01 initial scalars from the identified purchased Steam PE.

Reads data only; no original code execution, disassembly or installation changes.
File-backed defaults are not arbitrary live-global values.
"""
import argparse
import hashlib
import json
from pathlib import Path

from capture_reference_spawn import private
from inspect_installation import inspect_pe

ADDRESSES=(0xe4fc30,0xcb0f28,0xcafa58,0xcb870c,0xd17b10,0xcb0f1c)
STEAM_SHA='dd7b0933abf0d421bec3fa6f123319d3d3cc7221e5ef82b7a1201c3d1979e48f'


def main():
    parser=argparse.ArgumentParser();parser.add_argument('output',type=Path);parser.add_argument('--source',type=Path,default=Path(r'C:\Program Files (x86)\Steam\steamapps\common\Grid\grid.exe'));a=parser.parse_args()
    output=private(a.output);raw=a.source.read_bytes();digest=hashlib.sha256(raw).hexdigest()
    if digest!=STEAM_SHA:raise ValueError('Purchased executable build identity mismatch')
    pe=inspect_pe(a.source);base=int(pe['image_base'],16);values=[]
    for address in ADDRESSES:
        section=next(s for s in pe['sections'] if 0<=address-base-s['rva']<=s['raw_size']-4)
        offset=section['raw_offset']+address-base-section['rva'];values.append(raw[offset:offset+4])
    data=b'BP01'+b''.join(values)
    with output.open('xb') as file:file.write(data)
    record=dict(source=str(a.source),source_sha256=digest,output=str(output),output_sha256=hashlib.sha256(data).hexdigest(),
                addresses=[hex(v) for v in ADDRESSES],scope='File-backed initial scalar data, not live values; no code execution.')
    with output.with_suffix('.json').open('x',encoding='utf-8') as file:json.dump(record,file,indent=2)
    print(json.dumps(record,indent=2))


if __name__=='__main__':main()
