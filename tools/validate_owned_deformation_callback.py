"""Owned raw car adapter versus whole original static serial callback."""
import argparse
import collections
import copy
import json
import struct
from pathlib import Path
import validate_deformation_layout as layout
import validate_collision_mesh as execution
from check_collision_callback_controls import controls
from validate_serial_callback import pack,PROBE as PROBE_SHA

ROOT=layout.ROOT
PROBE=ROOT/'local/behavior/serial-callback-probe.exe'


def cases():
    sources=[ROOT/'local/behavior/live-impact-20261009-10/capture.json',ROOT/'local/behavior/live-callback-20261009-02/capture.json']
    expected=['c34887e65bb5bc0f6f7e8274422f4c169b6eb9f7351713e50eb04cc1cf745519','90e0c43a0ad21299038872d5ddadb83f4f9ead9ef2c4d61d18a643d2cfe5f73c']
    identities={str(layout.IMAGE):layout.digest(layout.IMAGE)}
    for path,sha in zip(sources,expected):assert layout.digest(path)==sha;identities[str(path)]=sha
    rows,bodies=controls(*[json.loads(p.read_text()) for p in sources]);extras=[]
    for row in rows:
        row['contacts']=row['contacts'][:64];row['contact_count']=len(row['contacts']);n=row['contact_count']
        row['response']='0x10000';row['response_before_hex']=bytes(n*4).hex()
        for c in row['contacts']:
            raw=bytearray.fromhex(c['before_hex']);struct.pack_into('<I',raw,4,int(row['linked'],16));struct.pack_into('<I',raw,8,0);raw[0xc]=1
            if raw[0x73] and struct.unpack_from('<f',raw,0x3c)[0]>0 and struct.unpack_from('<f',raw,0x40)[0]<=0:
                struct.pack_into('<f',raw,0x40,1.)
            c['before_hex']=raw.hex();c['counterpart']='0x0';c['counterpart_360_hex']=bytes(16).hex()
        extras.append(dict(registered=[0]*n,pending=[]))
    return rows,bodies,extras,identities


def guard_cases(rows,bodies,extras):
    r,b,e=copy.deepcopy(rows[6]),copy.deepcopy(bodies[6]),copy.deepcopy(extras[6]);good=pack([r],[b],[e])
    tests=[('magic',b'BAD!'+good[4:],False),('zero_count',good[:4]+bytes(4)+good[8:],False),('truncated',good[:-1],False),('trailing',good+b'x',False)]
    for name,key,at,value in [('dimension','descriptor_before_hex',0xa414,17),('edges','descriptor_before_hex',0xa410,1201),('nan_node','descriptor_before_hex',0x2580,0x7fc00000),('zero_spacing','descriptor_before_hex',0xa420,0),('zero_factor','body_d16c_hex',0,0),('zero_linked_mass','linked_before_hex',0x3f0,0)]:
        row=copy.deepcopy(r);raw=bytearray.fromhex(row[key]);struct.pack_into('<I',raw,at,value);row[key]=raw.hex();tests.append((name,pack([row],[b],[e]),False))
    row=copy.deepcopy(r);row['solver_clip_hex']='02';tests.append(('clip',pack([row],[b],[e]),False))
    body=copy.deepcopy(b);body['mass']=bytes(4);tests.append(('zero_mass',pack([r],[body],[e]),False))
    row=copy.deepcopy(r);row['contacts'][0]['counterpart']='0x1234';row['contacts'][0]['counterpart_360_hex']=bytes(16).hex()
    tests.append(('dynamic_peer',pack([row],[b],[e]),True))
    extra=copy.deepcopy(e);extra['pending']=[[0,.5]];tests.append(('pending_pair',pack([r],[b],[extra]),True))
    return good,tests


def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--profile',choices=['debug','release'],required=True);p.add_argument('--pilot',action='store_true');args=p.parse_args()
    out=args.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False);rows,bodies,extras,identities=cases()
    if args.pilot:rows,bodies,extras=rows[:32],bodies[:32],extras[:32]
    assert layout.digest(PROBE)==PROBE_SHA
    source=out/'input.bin';source.write_bytes(pack(rows,bodies,extras));answers={};runs={}
    for kind in ['original','repeat','typed','owned']:
        exe=PROBE if kind in ['original','repeat'] else ROOT/f"target/{args.profile}/grid-{'serial-callback' if kind=='typed' else 'owned-deformation-callback'}.exe"
        target=out/f'{kind}.bin';run=execution.run(exe,([layout.IMAGE] if kind in ['original','repeat'] else [])+[source,target]);runs[kind]=dict(**run,executable_sha256=layout.digest(exe));(out/'execution.json').write_text(json.dumps(runs,indent=2))
        assert run['exit']==0 and not run['stderr'],runs[kind]
        answers[kind]=target.read_bytes()
    original=answers['original'];diffs={kind:[dict(offset=hex(i),original=original[i:i+4].hex(),actual=data[i:i+4].hex()) for i in range(0,len(original),4) if original[i:i+4]!=data[i:i+4]] for kind,data in answers.items() if kind!='original'}
    assert all(len(a)==len(original) for a in answers.values());(out/'differences.json').write_text(json.dumps(diffs,indent=2));assert not any(diffs.values()),{k:v[:10] for k,v in diffs.items()}
    coverage=collections.Counter();at=8
    for row,body in zip(rows,bodies):
        d=original[at:at+layout.SIZE];at+=layout.SIZE;linked=original[at:at+0x424];at+=0x424;state=original[at:at+48];at+=48;n=row['contact_count']
        accumulated=original[at:at+n*4];at+=n*4;assert struct.unpack_from('<I',original,at)[0]==0;at+=4
        coverage['seed_changed']+=struct.unpack_from('<I',state,44)[0]!=body['seed'];coverage['angular_changed']+=linked[0x210:0x220]!=bytes.fromhex(row['linked_before_hex'])[0x210:0x220]
        coverage['nodes_changed']+=d[0x2580:0xa280]!=bytes.fromhex(row['descriptor_before_hex'])[0x2580:0xa280]
        coverage['contact38_changed']+=accumulated!=b''.join(bytes.fromhex(c['before_hex'])[0x38:0x3c] for c in row['contacts'])
    assert at==len(original)
    good,bad=guard_cases(rows,bodies,extras);guards=[]
    for name,payload,owned_only in bad:
        source=out/f'guard-{name}-input.bin';source.write_bytes(payload);checked={}
        for kind in (['owned'] if owned_only else ['original','typed','owned']):
            exe=PROBE if kind=='original' else ROOT/f"target/{args.profile}/grid-{'serial-callback' if kind=='typed' else 'owned-deformation-callback'}.exe";target=out/f'guard-{name}-{kind}.bin'
            run=execution.run(exe,([layout.IMAGE] if kind=='original' else [])+[source,target]);checked[kind]=dict(**run,output_exists=target.exists())
            assert 0<run['exit']<256 and 'fault=' not in run['stderr'] and (kind=='original' or not target.exists()),checked[kind]
        guards.append(dict(name=name,owned_only=owned_only,executions=checked))
    for kind in ['original','owned']:
        source=out/f'guard-existing-{kind}-input.bin';source.write_bytes(good);target=out/f'guard-existing-{kind}.bin';target.write_bytes(b'owned sentinel');exe=PROBE if kind=='original' else ROOT/f'target/{args.profile}/grid-owned-deformation-callback.exe'
        run=execution.run(exe,([layout.IMAGE] if kind=='original' else [])+[source,target]);assert run['exit']==(9 if kind=='original' else 1) and target.read_bytes()==b'owned sentinel';guards.append(dict(name='existing-'+kind,execution=run,preserved=True))
    (out/'guards.json').write_text(json.dumps(guards,indent=2))
    report=dict(profile=args.profile,cases=len(rows),bytes=len(original)-8,identities=identities,runs=runs,guards=guards,coverage=dict(coverage),files={p.name:dict(size=p.stat().st_size,sha256=layout.digest(p)) for p in sorted(out.iterdir()) if p.is_file()},scope='Static-first contacts authored from two retained templates; complete projected descriptor/physical/history/seed/contact38 buffers, not whole original owner or physical phase scheduling.')
    (out/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps({k:report[k] for k in ['profile','cases','bytes','coverage']}))


if __name__=='__main__':main()
