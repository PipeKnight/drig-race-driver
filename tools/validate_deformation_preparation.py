"""Whole bounded B77850/9A0C90 preparation versus complete Rust buffers."""
import argparse
import collections
import json
import random
import struct
from pathlib import Path
import validate_deformation_contact_limit as limit
import validate_deformation_layout as layout
import validate_collision_mesh as execution
from validate_deformation_volume import guard_checks

ROOT=layout.ROOT
ROW=16+layout.SIZE+limit.LINKED+16+96+64*128
ANSWER=16+layout.SIZE+limit.LINKED+64*128
PROBE=ROOT/'local/behavior/deformation-preparation-probe.exe'


def cases():
    sources,identities=limit.cases();rows=[];rng=random.Random(2008259)
    for index,source in enumerate(sources):
        count=[0,1,2,4,16,64,7,32][index%8]
        cursor=[0,count//2,count][(index//8)%3]
        budget=[0,1,2,count,10000][(index//24)%5]
        skip=[0,1,2,10000][(index//5)%4]
        # Ensure each block contains nontrivial full preparation as well as
        # cursor/zero-budget/skip-only controls.
        if index%16==5:cursor=0;budget=10000;skip=0
        contacts=[]
        for i in range(64):
            c=bytearray(rng.randbytes(128))
            if i<count:
                c=bytearray(source['contact'])
                struct.pack_into('<I',c,4,int(i%5==2))
                struct.pack_into('<f',c,0x3c,[0.,source['parameters'][5],100.,10000.,-.25][i%5])
                struct.pack_into('<4f',c,0x60,*[[1.,0.,0.,0.],[0.,0.,1.,0.],[-.6,0.,.8,0.],[0.,1.,0.,0.]][i%4])
                struct.pack_into('<f',c,0x38,[.001,.1,.5,2.][i%4])
            contacts.append(bytes(c))
        rows.append(dict(**source,count=count,cursor=cursor,budget=budget,skip=skip,contacts=contacts))
    return rows,identities


def request(rows):
    out=bytearray(b'DC02'+struct.pack('<I',len(rows)))
    for r in rows:
        out.extend(struct.pack('<4I',r['count'],r['cursor'],r['budget'],r['skip']));out.extend(r['descriptor']);out.extend(r['body'])
        out.extend(struct.pack('<4f',*r['center']));out.extend(struct.pack('<23fI',*r['parameters'],r['disabled']));out.extend(b''.join(r['contacts']))
    assert len(out)==8+len(rows)*ROW;return bytes(out)


def policy(row):
    cursor=row['cursor'];skip=row['skip'];budget=row['budget'];processed=[]
    while cursor<row['count']:
        if struct.unpack_from('<I',row['contacts'][cursor],4)[0]==0:
            if skip:skip-=1
            elif not budget:break
            else:processed.append(cursor);budget-=1
        cursor+=1
    return cursor,processed


def mutations():
    base=8+16+layout.SIZE;param=base+limit.LINKED+16;contacts=param+96
    return [('count',8,65),('cursor',12,65),('budget',16,10001),('skip',20,10001),
            ('dimension',24+0xa414,17),('ring',24+0xa4ec,10),('nan_node',24+0x2580,0x7fc00000),
            ('foreign_center',base+0x334,1),('foreign_callback',base+0x418,1),('foreign_user',base+0x420,1),
            ('zero_mass',base+0x3f0,0),('nan_parameter',param,0x7fc00000),('bad_disable',param+92,2),
            ('bad_owner',contacts+4,2),('dynamic_counterpart',contacts+12,0),('nan_old_cap',contacts+0x3c,0x7fc00000)]


def check_answer(row,r,coverage):
    cursor,processed=policy(row);assert struct.unpack_from('<4I',r,0)==(cursor,0,int(cursor==row['count']),0)
    d=r[16:16+layout.SIZE];body=r[16+layout.SIZE:16+layout.SIZE+limit.LINKED];c=r[16+layout.SIZE+limit.LINKED:]
    assert body==row['body'] and len(c)==8192
    allowed={0xa490,0xa494,0xa498,0xa4c4+struct.unpack_from('<I',row['descriptor'],0xa4ec)[0]*4}
    assert all(d[a:a+4]==row['descriptor'][a:a+4] for a in range(0,layout.SIZE,4) if a not in allowed)
    for i,original in enumerate(row['contacts']):
        answer=c[i*128:(i+1)*128]
        if i not in processed:assert answer==original
        else:
            assert all(answer[a:a+4]==original[a:a+4] for a in range(0,128,4) if a not in [0x3c,0x40,0x44])
            assert struct.unpack_from('<f',answer,0x44)[0]==row['parameters'][5]
            assert struct.unpack_from('<f',answer,0x3c)[0]<=struct.unpack_from('<f',original,0x3c)[0]
            coverage['cap_changed']+=answer[0x3c:0x40]!=original[0x3c:0x40]
    coverage['processed_contacts']+=len(processed);coverage['complete_cases']+=cursor==row['count']
    coverage['window_changed']+=d[0xa4c4:0xa4ec]!=row['descriptor'][0xa4c4:0xa4ec]


def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--profile',choices=['debug','release'],required=True);p.add_argument('--pilot',action='store_true');args=p.parse_args()
    out=args.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False);rows,identities=cases()
    if args.pilot:rows=rows[:32]
    rust=ROOT/f'target/{args.profile}/grid-deformation-preparation.exe';reports=[];coverage=collections.Counter()
    for start in range(0,len(rows),32):
        batch=rows[start:start+32];source=out/f'{start}-input.bin';source.write_bytes(request(batch));answers={};runs={}
        for kind in ['original','repeat','rust']:
            target=out/f'{start}-{kind}.bin';exe=rust if kind=='rust' else PROBE
            run=execution.run(exe,([layout.IMAGE] if kind!='rust' else [])+[source,target]);runs[kind]=dict(**run,executable_sha256=layout.digest(exe));(out/f'{start}-execution.json').write_text(json.dumps(runs,indent=2))
            assert run['exit']==0 and not run['stderr'],runs[kind]
            if kind!='rust':assert 'owned_canaries=1 full_image_unchanged=1 error=0' in run['stdout']
            answers[kind]=target.read_bytes()
        a,b=answers['original'],answers['rust'];assert len(a)==8+len(batch)*ANSWER and len(b)==len(a)
        differences=[dict(case=batch[i//ANSWER]['name'],offset=hex(i%ANSWER),original=a[8+i:12+i].hex(),rust=b[8+i:12+i].hex()) for i in range(0,len(a)-8,4) if a[8+i:12+i]!=b[8+i:12+i]]
        item=dict(start=start,cases=len(batch),bytes=len(a)-8,repeat_exact=a==answers['repeat'],differences=differences,executions=runs)
        (out/f'{start}-validation.json').write_text(json.dumps(item,indent=2));reports.append(item)
        assert item['repeat_exact'] and not differences,differences[:10]
        for i,row in enumerate(batch):check_answer(row,a[8+i*ANSWER:8+(i+1)*ANSWER],coverage)
    guards=guard_checks(out,request([rows[5]]),mutations(),args.profile,'grid-deformation-preparation',PROBE,7)
    result=dict(profile=args.profile,cases=len(rows),bytes=sum(r['bytes'] for r in reports),identities=identities,reports=reports,guards=guards,coverage=dict(coverage),
                files={p.name:dict(size=p.stat().st_size,sha256=layout.digest(p)) for p in sorted(out.iterdir()) if p.is_file()},
                scope='Whole first-pool B77850 with registered9A0C90, one owner/static peers, empty second pool; no B78530 ordering or complete physical group.')
    (out/'validation.json').write_text(json.dumps(result,indent=2));print(json.dumps({k:result[k] for k in ['profile','cases','bytes','coverage']}))


if __name__=='__main__':main()
