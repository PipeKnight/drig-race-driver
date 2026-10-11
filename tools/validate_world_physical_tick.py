"""Whole synchronous AD7040 body advance joined to supplied-contact B7B850."""
import argparse,collections,copy,json,random,struct,subprocess
from pathlib import Path
from validate_physical_tick import pack as tick_pack,semantic,PROBE as TICK_PROBE
from validate_impact_frame import layouts,decode
from validate_contact_bias import compare,execute
from validate_main_contact_response import uget,uput,fput
from validate_steering_control import digest
from capture_reference_spawn import ANALYSIS,private
ROOT=Path(__file__).resolve().parents[1]
MAGIC=0x33525750
INPUT_SHA='f3312e5bea0c899d27b25ec549716cf1e62a6fa4c9e43773f0b7db4c80a0dc4b'


def read_cases(data):
    result=[];at=8
    for _ in range(uget(data,4)):
        nb,nm,nc,nr,_,_=struct.unpack_from('<6I',data,at);size=164+nb*1160+nm*52+nc*132+nr*192
        row=data[at:at+size];frame=struct.pack('<II',0x31524650,1)+row[:144]+row[164:]
        c=layouts(frame)[0];c['correction']=list(struct.unpack_from('<I4f',row,144));result.append(c);at+=size
    assert at==len(data);return result


def pack(cases):
    data=tick_pack(cases);uput(data,0,MAGIC);return data


def links(c):
    nexts=[uget(b['raw'],0x300) for b in c['bodies']];previous=[0]*len(nexts);order=[];i=0
    while True:
        assert i not in order;order.append(i)
        if not nexts[i]:break
        previous[nexts[i]-1]=i+1;i=nexts[i]-1
    assert len(order)==len(nexts);return nexts,previous,order


def expected(baseline,cases):
    data=bytearray(baseline);uput(data,0,MAGIC);at=8
    for c in cases:
        nb=len(c['bodies']);nexts,previous,_=links(c);body=at+84+nb*100
        for i in range(nb):uput(data,body+i*0x424+0x300,nexts[i]);uput(data,body+i*0x424+0x318,previous[i])
        at+=84+nb*(100+0x424)+len(c['contacts'])*128+len(c['rows'])*192
    assert at==len(data);return data


def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--profile',choices=['debug','release'],required=True);a=p.parse_args()
    out=private(a.output);out.mkdir(exist_ok=False);reference=ROOT/'local/compatibility/grid-unpacked.exe';probe=ROOT/'local/behavior/world-physical-tick-probe.exe';rust=ROOT/f'target/{a.profile}/grid-physical-tick.exe'
    assert digest(reference)==ANALYSIS
    prior=ROOT/f'local/behavior/physical-tick-20261010-{a.profile}-final-02';source=prior/'input.bin';assert digest(source)==INPUT_SHA
    baseline=(prior/'original.bin').read_bytes();assert all((prior/f'{k}.bin').read_bytes()==baseline for k in ['repeat','rust'])
    manual=ROOT/'local/behavior/physical-tick-probe.exe';assert digest(manual)==TICK_PROBE
    regression={}
    for kind,command in [('original',[str(manual),str(reference)]),('rust',[str(rust)])]:
        data,run=execute(command,source,out/f'regression-{kind}.bin');run.update(compare(data,baseline));assert run['passed'];regression[kind]=run
    cases=read_cases(source.read_bytes());rng=random.Random(2008188)
    for c in cases:
        order=[0]+rng.sample(list(range(1,len(c['bodies']))),len(c['bodies'])-1)
        for i in range(len(c['bodies'])):uput(c['bodies'][i]['raw'],0x300,0);uput(c['bodies'][i]['raw'],0x318,0)
        for i,j in zip(order,order[1:]):uput(c['bodies'][i]['raw'],0x300,j+1)
    source=out/'input.bin';source.write_bytes(pack(cases));want=expected(baseline,cases);runs={};original=None
    for kind,command in [('original',[str(probe),str(reference)]),('repeat',[str(probe),str(reference)]),('rust',[str(rust)])]:
        data,run=execute(command,source,out/f'{kind}.bin');run.update(compare(data,want));runs[kind]=run
        if kind!='rust':assert 'code_unchanged=1 no_imports=1' in run['stdout']
        if original is None:original=data
        assert data==original
    framed=bytearray(original);uput(framed,0,0x31524650);decoded=decode(framed,cases);checks,activity=semantic(cases,decoded)
    checks['active_list_retained']=all([uget(b,0x300) for b in row['bodies']]==links(c)[0] for c,row in zip(cases,decoded))
    checks['update_list_reversed']=all([uget(b,0x318) for b in row['bodies']]==links(c)[1] for c,row in zip(cases,decoded))
    activity['nontrivial_permutations']=sum(links(c)[2]!=list(range(len(c['bodies']))) for c in cases)
    activity['selected_head_slots']=min(9,len(cases))
    base=copy.deepcopy(next(c for c in cases if len(c['bodies'])>=3));good=pack([base]);bad={'truncated':good[:-1],'trailing':good+b'!','empty_cases':good[:4]+bytes(4)+good[8:]}
    for name,o,v in [('self_cycle',0x300,1),('foreign_next',0x300,17),('missing_bodies',0x300,0),('stale_update',0x318,1),('body_callback',0x418,1),('body_link',0x324,1),('nan_basis',0x190,0x7fc00000)]:
        c=copy.deepcopy(base);uput(c['bodies'][0]['raw'],o,v);bad[name]=pack([c])
    c=copy.deepcopy(base);uput(c['bodies'][0]['raw'],0x300,2);uput(c['bodies'][1]['raw'],0x300,1);bad['two_body_cycle']=pack([c]);guards={}
    for name,data in bad.items():
        source=out/f'guard-{name}-input.bin';source.write_bytes(data);guards[name]={}
        for kind,command in [('original',[str(probe),str(reference)]),('rust',[str(rust)])]:
            target=out/f'guard-{name}-{kind}.bin';run=subprocess.run(command+[str(source),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=30)
            guards[name][kind]=dict(exit=run.returncode,stdout=run.stdout,stderr=run.stderr,passed=0<run.returncode<256 and 'physical_tick_fault=' not in run.stderr and (kind=='rust' or (not run.stderr and 'code_unchanged=1 no_imports=1' in run.stdout)) and not target.exists())
    report=dict(profile=a.profile,cases=len(cases),words=(len(original)-8)//4,analysis_sha256=ANALYSIS,probe_sha256=digest(probe),rust_sha256=digest(rust),upstream_sha256=INPUT_SHA,regression=regression,runs=runs,semantic_checks=checks,activity=activity,guards=guards,scope='Whole synchronous AD7040 -> AD6C00 -> B13E10, full owned active/update list with head0 and varying world head slot. Joined supplied-post-advance-contact B7B850 main/optional. Empty shapes/grouphead/links/active callbacks/wakeup, no asynchronous jobs/contact generation/fullAD8D70/live schedule/driving claim.')
    report['passed']=all(checks.values()) and all(r['passed'] for r in runs.values()) and all(r['passed'] for g in guards.values() for r in g.values())
    (out/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps({k:v for k,v in report.items() if k not in ['regression','runs','guards']}));assert report['passed']
if __name__=='__main__':main()
