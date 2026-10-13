"""Write private DF01 file-backed defaults; purchased PE read-only, hash pinned."""
import argparse
import hashlib
import json
import struct
from pathlib import Path
from capture_reference_spawn import private
from inspect_installation import inspect_pe
from extract_body_impact_parameters import STEAM_SHA

def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--source',type=Path,default=Path(r'C:\Program Files (x86)\Steam\steamapps\common\Grid\grid.exe'));args=p.parse_args()
    output=private(args.output);raw=args.source.read_bytes();sha=hashlib.sha256(raw).hexdigest();assert sha==STEAM_SHA
    pe=inspect_pe(args.source);base=int(pe['image_base'],16);addresses=[]
    def data(address,count=4):
        s=next(s for s in pe['sections'] if 0<=address-base-s['rva']<=max(s['virtual_size'],s['raw_size'])-count)
        relative=address-base-s['rva'];available=max(0,min(count,s['raw_size']-relative))
        addresses.append(dict(address=hex(address),size=count,file_bytes=available,zero_fill_bytes=count-available))
        at=s['raw_offset']+relative;return raw[at:at+available]+bytes(count-available)
    disabled=struct.pack('<I',data(0xe85fad,1)[0])
    table=data(0xe4fc48,64);scale=data(0xe4fc30)
    limit=b''.join(data(a) for a in [0xcafbdc,0xcb093c,0xe4fc2c,0xe4fc3c,0xcb3ba8,0xe51d40])+scale+table+disabled
    bulk=b''.join(data(a) for a in [0xe4fc2c,0xe51d40,0xe500c8,0xe500d0,0xe500d4,0xe500cc])+data(0xe500a4,20)
    bulk+=b''.join(data(a) for a in [0xe4fc88,0xe4fc90,0xe4fc94])+data(0xe4fc8c)+struct.pack('<I',data(0xe500b8,1)[0])
    bulk+=data(0xe4fc38)+data(0xe500c0)+data(0xe500c4)+data(0xd1676c)+scale+table+disabled
    result=b'DF01'+limit+bulk;assert len(result)==252
    with output.open('xb') as f:f.write(result)
    record=dict(source=str(args.source),source_sha256=sha,output=str(output),output_sha256=hashlib.sha256(result).hexdigest(),addresses=addresses,scope='Initial mapped data: file-backed scalars and explicitly recorded loader zero-fill; no live global or startup-scheduling claim; read-only, no code execution.')
    with output.with_suffix('.json').open('x',encoding='utf-8') as f:json.dump(record,f,indent=2)
    print(json.dumps({k:record[k] for k in ['source_sha256','output_sha256','scope']}))
if __name__=='__main__':main()
