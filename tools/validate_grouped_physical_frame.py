"""Whole AD6C40 -> B77310 -> B7B850 against independently composed Rust groups."""
import argparse,copy,json,random,struct,subprocess
from pathlib import Path
from capture_reference_spawn import ANALYSIS,private
from validate_contact_bias import compare,execute
from validate_world_physical_tick import read_cases,INPUT_SHA
from validate_physical_tick import pack as tick_pack
from validate_impact_frame import decode as frame_decode
from validate_contact_groups import semantic as graph_semantic
from validate_main_contact_response import uget,uput,fget,fput
from validate_steering_control import digest
ROOT=Path(__file__).resolve().parents[1]
MAGIC=0x33524647
MAX=struct.unpack('<f',bytes.fromhex('ffff7f7f'))[0]


def configure(c,rng,randomize=False,pairs=None):
    c=copy.deepcopy(c);nb=len(c['bodies']);order=[0]+rng.sample(list(range(1,nb)),nb-1)
    c['order']=order;c['iterations']=c['iterations'] or 3;c['capacity']=256
    while len(c['rows'])<256:c['rows'].append(bytearray(rng.randbytes(192)))
    for b in c['bodies']:
        raw=b['raw']
        for o in [0,4,8,12,16,20,24,28,0x28,0x2f0,0x2f4,0x2fc,0x300,0x304,0x308,0x30c,0x318,0x320,0x324,0x328,0x32c,0x338,0x33c,0x340,0x400,0x418,0x41c,0x420]:uput(raw,o,0)
        raw[0x2c]=0;raw[0x2d]=0;raw[0x412]=0;raw[0x413]=0;fput(raw,0x3b4,MAX)
    for a,b in zip(order,order[1:]):uput(c['bodies'][a]['raw'],0x300,b+1)
    if pairs is not None:
        template=c['contacts'][0][1];c['contacts']=[(int(b is not None),copy.deepcopy(template)) for a,b in pairs]
    for i,(mode,r) in enumerate(c['contacts']):
        a=uget(r,4);b=uget(r,8)
        if pairs is not None:a,b=pairs[i];b=a if b is None else b
        elif randomize:
            a=rng.randrange(nb);b=rng.randrange(nb)
            if mode and nb>1:b=rng.choice([j for j in range(nb) if j!=a])
        elif mode and a==b and nb>1:b=(a+1)%nb
        if nb==1:mode=0;c['contacts'][i]=(mode,r)
        for o,value in [(4,a),(8,b),(0x1c,a),(0x20,b),(0x10,0),(0x28,0),(0x2c,0)]:uput(r,o,value)
        r[0xc]=0 if mode else 1;r[0xf]=1;r[0x73]=0;r[0xe]=0;r[0x70]=0
    return c


def authored():
    source=ROOT/'local/behavior/physical-tick-20261010-debug-final-02/input.bin';assert digest(source)==INPUT_SHA
    prior=read_cases(source.read_bytes());rng=random.Random(2008190)
    result=[configure(c,rng) for c in prior[:256]]
    result.extend(configure(prior[(i*17)%len(prior)],rng,True) for i in range(128))
    base=next(c for c in prior if len(c['bodies'])>=6 and c['contacts'])
    for i in range(16):
        pairs=[(0,1),(2,3),(4,5),(1,3),(3,5)] if i%2 else [(1,0),(3,2),(5,4),(2,0),(4,0)]
        c=configure(base,rng,pairs=pairs)
        if i%4==0:c['contacts'].append((0,copy.deepcopy(c['contacts'][0][1])));r=c['contacts'][-1][1];r[0xc]=1;uput(r,4,0);uput(r,8,0);uput(r,0x1c,0);uput(r,0x20,0)
        result.append(c)
    return result


def pack(cases):
    result=tick_pack(cases);uput(result,0,MAGIC);assert len(result)<=32*1024*1024;return result


def decode(data,cases):
    assert struct.unpack_from('<II',data)==(MAGIC,len(cases));at=8;frame=bytearray(struct.pack('<II',0x31524650,len(cases)));states=[]
    for c in cases:
        n=84+len(c['bodies'])*(100+0x424)+len(c['contacts'])*128+len(c['rows'])*192
        frame.extend(data[at:at+n]);at+=n;states.append(list(struct.unpack_from('<11I',data,at)));at+=44
    assert at==len(data);outputs=frame_decode(frame,cases)
    for o,state in zip(outputs,states):o['world']=state
    return outputs


def semantics(cases,outputs,advance=False):
    gs=[];os=[]
    for c,o in zip(cases,outputs):
        contacts=[];after=[]
        for _,r in c['contacts']:
            r=bytearray(r);uput(r,4,uget(r,4)+1);uput(r,8,uget(r,8)+1);contacts.append(r)
        for r in o['contacts']:
            r=bytearray(r);uput(r,4,uget(r,4)+1);uput(r,8,uget(r,8)+1);after.append(r)
        gs.append(dict(bodies=[b['raw'] for b in c['bodies']],contacts=contacts,order=c['order'],selector=0))
        os.append(dict(bodies=o['bodies'],contacts=after,world=o['world']))
    checks,activity=graph_semantic(gs,os)
    checks['finished_forces_cleared']=all(b[0x250:0x270]==bytes(32) and not b[0x40d] and not b[0x40e] for o in outputs for b in o['bodies'])
    def rounded(v):return struct.unpack('<f',struct.pack('<f',v))[0]
    def center(c,r,k):
        raw=c['bodies'][uget(r,4)]['raw'];old=fget(raw,0x1c0+4*k)
        return rounded(rounded(c['parameters'][0]*fget(raw,0x220+4*k))+old) if advance else old
    checks['history_published']=all((b['raw'][0x190:0x210] if advance else b['raw'][0x270:0x2f0])==a[0x270:0x2f0] for c,o in zip(cases,outputs) for b,a in zip(c['bodies'],o['bodies']))
    checks['first_point_translated']=all(all(struct.pack('<f',center(c,r,k)+fget(r,0x50+4*k))==a[0x50+4*k:0x54+4*k] for k in range(4)) for c,o in zip(cases,outputs) for (_,r),a in zip(c['contacts'],o['contacts']))
    if advance:
        checks['update_chain_reversed']=True
        checks['advance_response_cleared']=all(b[0x360:0x370]==bytes(16) for o in outputs for b in o['bodies'])
        for c,o in zip(cases,outputs):
            previous=[0]*len(c['bodies']);old=0
            for i in c['order']:previous[i]=old;old=i+1
            checks['update_chain_reversed'] &= [uget(b,0x318) for b in o['bodies']]==previous
    activity['velocity_changes']=sum(b['raw'][0x210:0x230]!=a[0x210:0x230] for c,o in zip(cases,outputs) for b,a in zip(c['bodies'],o['bodies']))
    activity['pose_changes']=sum(b['raw'][0x190:0x210]!=a[0x190:0x210] for c,o in zip(cases,outputs) for b,a in zip(c['bodies'],o['bodies']))
    activity['body_count']=sum(len(c['bodies']) for c in cases)
    return checks,activity


def main():
    global MAGIC
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--profile',choices=['debug','release'],required=True);p.add_argument('--limit',type=int,default=0);p.add_argument('--advance',action='store_true');a=p.parse_args()
    if a.advance:MAGIC=0x33525447
    out=private(a.output);out.mkdir(exist_ok=False);reference=ROOT/'local/compatibility/grid-unpacked.exe';probe=ROOT/('local/behavior/grouped-physical-tick-probe.exe' if a.advance else 'local/behavior/grouped-physical-frame-probe.exe');rust=ROOT/f'target/{a.profile}/grid-physical-tick.exe'
    assert digest(reference)==ANALYSIS;cases=authored();cases=cases[:a.limit] if a.limit else cases;source=out/'input.bin';source.write_bytes(pack(cases));runs={};original=None
    for kind,command in [('original',[str(probe),str(reference)]),('repeat',[str(probe),str(reference)]),('rust',[str(rust)])]:
        data,run=execute(command,source,out/f'{kind}.bin')
        if original is None:original=data
        run.update(compare(data,original));runs[kind]=run
        if kind!='rust':assert 'code_unchanged=1 no_imports=1' in run['stdout']
    checks,activity=semantics(cases,decode(original,cases),a.advance)
    base=copy.deepcopy(next(c for c in cases if c['contacts'] and len(c['bodies'])>=3));good=pack([base]);bad={'truncated':good[:-1],'trailing':good+b'!','empty_cases':good[:4]+bytes(4)+good[8:]}
    for name,change in [('zero_iterations',lambda c:c.update(iterations=0)),('insufficient_capacity',lambda c:c.update(capacity=0)),('self_cycle',lambda c:uput(c['bodies'][0]['raw'],0x300,1)),('foreign_next',lambda c:uput(c['bodies'][0]['raw'],0x300,17)),('missing_bodies',lambda c:uput(c['bodies'][0]['raw'],0x300,0)),('special_body',lambda c:uput(c['bodies'][0]['raw'],0x28,1)),('sleeping_duration',lambda c:fput(c['bodies'][0]['raw'],0x3b4,.5)),('existing_link',lambda c:uput(c['bodies'][0]['raw'],0x320,1)),('dirty_body',lambda c:c['bodies'][0]['raw'].__setitem__(0x412,1)),('shape_filter',lambda c:c['contacts'][0][1].__setitem__(0x73,1)),('nan_basis',lambda c:uput(c['bodies'][0]['raw'],0x190,0x7fc00000))]:
        c=copy.deepcopy(base);change(c);bad[name]=pack([c])
    guards={}
    for name,data in bad.items():
        source=out/f'guard-{name}-input.bin';source.write_bytes(data);guards[name]={}
        for kind,command in [('original',[str(probe),str(reference)]),('rust',[str(rust)])]:
            target=out/f'guard-{name}-{kind}.bin';run=subprocess.run(command+[str(source),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=30)
            guards[name][kind]=dict(exit=run.returncode,stdout=run.stdout,stderr=run.stderr,passed=0<run.returncode<256 and 'physical_tick_fault=' not in run.stderr and not target.exists() and (kind=='rust' or not run.stderr and 'code_unchanged=1 no_imports=1' in run.stdout))
    report=dict(profile=a.profile,advance=a.advance,cases=len(cases),bytes=len(original)-8,runs=runs,checks=checks,activity=activity,guards=guards,probe_sha256=digest(probe),rust_sha256=digest(rust),analysis_sha256=ANALYSIS,upstream_sha256=INPUT_SHA,scope=('Whole serial AD7040 -> ' if a.advance else '')+'whole AD6C40 -> whole serial B77310 -> complete B7B850; cold active impact pool, empty shapes/links/callbacks/wakeup/special body, unsleepable MAX duration, full row capacity, positive iterations. Supplied contact offsets, no geometric production/async/live schedule/whole AD8D70/driving claim.')
    report['passed']=all(checks.values()) and all(r['passed'] for r in runs.values()) and all(r['passed'] for g in guards.values() for r in g.values());(out/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps({k:v for k,v in report.items() if k not in ['runs','guards']}));assert report['passed']
if __name__=='__main__':main()
