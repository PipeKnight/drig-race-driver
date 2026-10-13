"""Whole body handler/registered consumer vs complete Rust state and packets."""
import argparse
import hashlib
import json
from pathlib import Path
import random
import struct
import subprocess

from capture_reference_spawn import ANALYSIS, private
from validate_collision_damage import CAR, PHYSICAL, M, fixtures as damage_fixtures, f, u, put, f32

ROOT=Path(__file__).resolve().parents[1]
PREFIX=52
SIZE=PREFIX+CAR+PHYSICAL
OUTSIZE=12+11*56+CAR+PHYSICAL
SCALARS=Path('local/behavior/body-handler-scalars-20261013-draft-01.json')


def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def fixtures():
    rng=random.Random(200682)
    for index,(label,raw) in enumerate(damage_fixtures()):
        car=bytearray(raw[32:32+CAR]);physical=bytearray(raw[32+CAR:])
        car[0x308]=0
        flags=(index//8)%4
        car[0xb13]=int(flags in (0,1));car[0xb14]=int(flags in (0,2))
        car[0xba5]=0x5a
        put(car,0x1568,1.25)
        put(car,0x2e8,rng.choice([-1.,0.,4.,5.]))
        put(car,0x3d0,rng.choice([-1.,0.,.125,1.,5.]))
        put(car,0x15a0,rng.choice([0.,10.,1.e6]));put(car,0x15a4,rng.choice([0.,.0001,.01,1.]))
        linear=rng.choice([(0.,0.,0.,0.),(1.,2.,1.,2.),(4.,0.,0.,0.),(-1.,.5,-.25,-.125)])
        put(physical,0x220,*linear)
        impact=0x7f7fffff if index%17==0 else u(raw,8)
        normal=struct.pack('<4f',rng.uniform(-1,1),rng.uniform(-1,1),rng.uniform(-1,1),-0. if index%2 else 0.)
        mask=[0,1,1<<10,0x7ff,0x555,0x2aa][(index//8)%6]
        # Source/authored descriptors remain distinct; the handler flags,
        # timing, linear velocity, registrations and notifications are authored.
        yield label,struct.pack('<4I',u(raw,0),index%8,impact,0x3c83126f)+raw[16:32]+normal+struct.pack('<I',mask)+car+physical


def run(command,success=True):
    process=subprocess.run([str(v) for v in command],capture_output=True,text=True,timeout=60)
    if (process.returncode==0)!=success:
        raise RuntimeError((command,process.returncode,process.stdout,process.stderr))
    return dict(exit=process.returncode,stdout=process.stdout,stderr=process.stderr)


def main():
    ap=argparse.ArgumentParser();ap.add_argument('output',type=Path);a=ap.parse_args()
    out=private(a.output);out.mkdir(parents=True,exist_ok=False)
    image=Path('local/compatibility/grid-unpacked.exe')
    if sha(image)!=ANALYSIS:raise ValueError('Reference identity changed')
    native=Path('local/behavior/body-impact-handler-probe.exe')
    debug=Path('target/debug/grid-body-impact.exe');release=Path('target/release/grid-body-impact.exe')
    scalar_record=json.loads(SCALARS.read_text(encoding='utf-8'))
    assert scalar_record['source_sha256']==ANALYSIS
    scalars=scalar_record['results']
    assert [r['address'] for r in scalars]==['00e4fc30','00cb0f28','00cafa58','00cb870c','00d17b10','00cb0f1c']
    params=out/'parameters.bin';params.write_bytes(b''.join(bytes.fromhex(r['bytes_hex']) for r in scalars))
    pins={str(p):sha(p) for p in [image,native,debug,release,SCALARS,Path(__file__),Path('tools/probe_body_impact_handler.c'),Path('tools/probe_collision_damage.c'),Path('tools/isolated_reference.h'),Path('crates/grid-world/src/body_impact.rs'),Path('crates/grid-tools/src/bin/grid-body-impact.rs')]}
    items=list(fixtures());assert len(items)==768
    report=dict(scope='Whole original 9EF360 mode0, and A726B0/B0CFC0/A01910/A6A1D0 '
                'on explicit owned side graphs, notification observers and no dynamic peer. '
                'Source-derived damage descriptors with authored handler contexts; no source factory/runtime claim.',
                pins=pins,cases=len(items),batches=[],guards=[])
    reached=changed=draws=notifications=0
    for start in range(0,len(items),32):
        group=items[start:start+32];request=b'BH01'+struct.pack('<I',len(group))+b''.join(r for _,r in group)
        inp=out/f'{start//32:03}.input';inp.write_bytes(request)
        answers={};records={}
        for label,exe in [('original',native),('repeat',native),('debug',debug),('release',release)]:
            target=out/f'{start//32:03}-{label}.bin'
            command=[exe,image,inp,target] if label in ('original','repeat') else [exe,inp,params,target]
            execution=run(command);data=target.read_bytes()
            if label in ('original','repeat'):assert 'globals_restored=1 owned_contexts_checked=1 code_replacements=0 error=0' in execution['stdout']
            answers[label]=data;records[label]=dict(sha256=sha(target),execution=execution)
        original=answers['original'];assert original[:8]==request[:8] and len(original)==8+len(group)*OUTSIZE
        for label,data in answers.items():
            difference=next((i for i,(x,y) in enumerate(zip(data,original)) if x!=y),None)
            assert data==original,(start,label,difference,hex((difference-8)%OUTSIZE) if difference is not None else None)
        for index,(_,r) in enumerate(group):
            result=original[8+index*OUTSIZE:8+(index+1)*OUTSIZE]
            v=r[PREFIX:PREFIX+CAR];after=result[12+11*56:12+11*56+CAR]
            selected=u(r,4) not in (5,7) and u(r,8)!=0x7f7fffff
            reached+=selected;draws+=u(result,4)!=u(r,0);notifications+=u(result,8)
            changed+=any(after[M+o:M+o+4]!=v[M+o:M+o+4] for o in [0,4,8,0x10,0x14,0x28,0x2c,0x30,0x34])
            assert result[-PHYSICAL:]==r[-PHYSICAL:]
            if not selected:assert after==v and u(result,4)==u(r,0) and u(result,8)==0
            else:
                assert u(after,0x1568)==0
                assert u(result,8)==u(r,48).bit_count()
            assert result[12+u(result,8)*56:12+11*56]==bytes((11-u(result,8))*56)
        report['batches'].append(dict(index=start//32,request_sha256=sha(inp),bytes=len(original),runs=records))
        if start%128==96:print(json.dumps(dict(cases_complete=start+len(group))),flush=True)
    request=b'BH01'+struct.pack('<I',1)+items[0][1]
    variants=[('short',request[:-1]),('trailing',request+b'x')]
    for name,offset,word in [('tag',0,0),('count',4,0),('selector',12,8),('mask',8+48,0x800),
        ('normal-nan',8+32,0x7fc00000),('parameter',8+12,0x3f800000),('negative-impact',8+8,0xbf800000),
        ('dynamic-peer',8+PREFIX+0x308,1),('dispatch-flag',8+PREFIX+0xb14,2),
        ('linear-nan',8+PREFIX+CAR+0x220,0x7fc00000),('timer-nan',8+PREFIX+0x2e8,0x7fc00000),
        ('owner-link',8+PREFIX+0xdb0,1),('mass',8+PREFIX+CAR+0x3f0,0),('box-enable',8+PREFIX+M+0x60+32,2)]:
        data=bytearray(request);struct.pack_into('<I',data,offset,word);variants.append((name,data))
    for name,data in variants:
        inp=out/f'guard-{name}.input';inp.write_bytes(data)
        for label,exe in [('original',native),('debug',debug),('release',release)]:
            target=out/f'guard-{name}-{label}.bin'
            execution=run([exe,image,inp,target] if label=='original' else [exe,inp,params,target],False)
            assert not target.exists();report['guards'].append(dict(name=name,profile=label,execution=execution))
    report.update(reached=reached,damage_level_changes=changed,random_changes=draws,notification_packets=notifications,
                  compared_bytes_per_profile=sum(b['bytes'] for b in report['batches']),passed=True)
    (out/'validation.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
    print(json.dumps({k:report[k] for k in ('cases','reached','damage_level_changes','random_changes','notification_packets','compared_bytes_per_profile','passed')},indent=2))


if __name__=='__main__':main()
