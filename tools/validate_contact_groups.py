"""Whole original cold active/contact group reconstruction versus independent Rust."""
import argparse,copy,json,random,struct,subprocess
from pathlib import Path
from capture_reference_spawn import ANALYSIS,private
from validate_contact_bias import compare,execute
from validate_main_contact_response import uget,uput,fput
from validate_steering_control import digest
ROOT=Path(__file__).resolve().parents[1]
MAGIC=0x31304743
POINTERS=[0,4,8,12,16,20,24,28,0x2f0,0x2f4,0x2fc,0x300,0x304,0x308,0x30c,0x318,0x320,0x324,0x328,0x32c,0x418,0x41c,0x420]


def case(rng,n,pairs,selector,order=None):
    bodies=[];contacts=[]
    for i in range(n):
        raw=bytearray(rng.randbytes(0x424))
        for o in POINTERS:uput(raw,o,0)
        uput(raw,0x334,0x1c0);uput(raw,0x400,selector)
        raw[0x2c]=rng.randrange(2);raw[0x2d]=0;raw[0x412]=0;raw[0x413]=0
        for k in range(4):fput(raw,0x1c0+4*k,rng.choice([-0.,0.,-1.,.125,100.,rng.uniform(-10000,10000)]))
        bodies.append(raw)
    for a,b in pairs:
        raw=bytearray(rng.randbytes(128));uput(raw,4,a+1);uput(raw,8,0 if b is None else b+1)
        raw[0xc]=int(b is None);raw[0xe]=rng.randrange(2);raw[0xf]=1;raw[0x73]=0
        for o in [0x10,0x28,0x2c]:uput(raw,o,0)
        for k in range(8):fput(raw,0x50+4*k,rng.choice([-0.,0.,.1,-2.,rng.uniform(-100,100)]))
        contacts.append(raw)
    return dict(bodies=bodies,contacts=contacts,selector=selector,order=rng.sample(list(range(n)),n) if order is None else order)


def authored():
    rng=random.Random(2008189);result=[]
    for selector in [0,1]:
        for order in [[0,1,2,3,4,5],[5,3,1,4,2,0]]:
            for pairs in [[],[(0,None)],[(0,1),(2,3),(4,5),(1,3),(3,5)],[(0,1),(0,1),(1,0)],[(i,None) for i in range(6)],[(i,i+1) for i in range(5)],[(1,0),(3,2),(5,4),(2,0),(4,0)]]:
                result.append(case(rng,6,pairs,selector,order))
    for i in range(600):
        n=1+i%16;pairs=[]
        for _ in range(i%65):
            a=rng.randrange(n);b=None if n==1 or rng.randrange(3)==0 else rng.choice([j for j in range(n) if j!=a]);pairs.append((a,b))
        result.append(case(rng,n,pairs,i%2))
    return result


def pack(cases):
    chunks=[struct.pack('<II',MAGIC,len(cases))]
    for c in cases:
        chunks.extend([struct.pack('<III',len(c['bodies']),len(c['contacts']),c['selector']),struct.pack(f'<{len(c["order"])}I',*c['order']),*c['bodies'],*c['contacts']])
    return b''.join(chunks)


def decode(data,cases):
    assert struct.unpack_from('<II',data)==(MAGIC,len(cases));at=8;result=[]
    for c in cases:
        state=list(struct.unpack_from('<11I',data,at));at+=44
        nb=len(c['bodies']);nc=len(c['contacts'])
        bodies=[data[at+j*0x424:at+(j+1)*0x424] for j in range(nb)];at+=nb*0x424
        contacts=[data[at+j*128:at+(j+1)*128] for j in range(nc)];at+=nc*128
        result.append(dict(world=state,bodies=bodies,contacts=contacts))
    assert at==len(data);return result


def chain(head,rows,offset):
    found=[]
    while head:
        assert 1<=head<=len(rows) and head not in found;found.append(head);head=uget(rows[head-1],offset)
    return found


def semantic(cases,outputs):
    checks=dict(component_partition=True,each_body_once=True,each_contact_once=True,body_group_backlinks=True,group_counts=True,local_indices=True,manager_reverses_groups=True,dirty_reverses_active=True,source_list_empty=True,switches_generation=True,all_impacts_enabled=True,contact_indices=True,second_points_retained=True)
    activity=dict(groups=0,multibody_groups=0,joined_contact_components=0,empty_contact_cases=0,single_contacts=0,two_contacts=0,nonzero_first_heads=0,selectors=[0,0])
    for c,o in zip(cases,outputs):
        bs=o['bodies'];cs=o['contacts'];w=o['world'];roots=chain(w[0],bs,0x10);managed=chain(w[1],bs,0x1c)
        checks['manager_reverses_groups'] &= managed==list(reversed(roots))
        checks['dirty_reverses_active'] &= chain(w[4],bs,0x2f0)==[i+1 for i in reversed(c['order'])]
        checks['source_list_empty'] &= w[7+c['selector']]==0 and w[9+c['selector']]==0
        checks['switches_generation'] &= w[5:7]==[1-c['selector'],c['selector']] and all(uget(b,0x400)==1-c['selector'] for b in bs)
        assert set(chain(w[7+1-c['selector']],bs,0x300))==set(range(1,len(bs)+1))
        parent=list(range(len(bs)))
        def find(i):
            while parent[i]!=i:i=parent[i]
            return i
        for raw in c['contacts']:
            a=uget(raw,4)-1;b=uget(raw,8)-1
            if not raw[0xc]:parent[find(b)]=find(a)
        expected={frozenset(j+1 for j in range(len(bs)) if find(j)==find(i)) for i in range(len(bs))}
        groups=[];body_seen=[];contact_seen=[]
        for root in roots:
            bodies=chain(uget(bs[root-1],8),bs,0x2fc);contacts=chain(uget(bs[root-1],0),cs,0x10)
            groups.append(frozenset(bodies));body_seen.extend(bodies);contact_seen.extend(contacts)
            checks['body_group_backlinks'] &= all(uget(bs[b-1],0x18)==root for b in bodies)
            checks['group_counts'] &= uget(bs[root-1],0x20)==len(bodies) and uget(bs[root-1],0x24)==len(contacts)
            checks['local_indices'] &= [uget(bs[b-1],0x404) for b in bodies]==list(range(len(bodies)))
            activity['multibody_groups']+=len(bodies)>1
        checks['component_partition'] &= set(groups)==expected
        checks['each_body_once'] &= sorted(body_seen)==list(range(1,len(bs)+1))
        checks['each_contact_once'] &= sorted(contact_seen)==list(range(1,len(cs)+1))
        checks['all_impacts_enabled'] &= all(r[0xe]==1 for r in cs)
        checks['contact_indices'] &= all(uget(r,0x1c)==uget(bs[uget(r,4)-1],0x404) and uget(r,0x20)==(0xffffffff if r[0xc] else uget(bs[uget(r,8)-1],0x404)) for r in cs)
        checks['second_points_retained'] &= all(a[0x60:0x70]==b[0x60:0x70] for a,b in zip(c['contacts'],cs))
        assert w[2]==max(len(g) for g in groups) and w[3]==len(bs)
        activity['groups']+=len(roots);activity['joined_contact_components']+=len(roots)<len(bs);activity['empty_contact_cases']+=not cs
        activity['single_contacts']+=sum(r[0xc]!=0 for r in cs);activity['two_contacts']+=sum(r[0xc]==0 for r in cs)
        activity['nonzero_first_heads']+=c['order'][0]!=0;activity['selectors'][c['selector']]+=1
    return checks,activity


def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--profile',choices=['debug','release'],required=True);a=p.parse_args()
    out=private(a.output);out.mkdir(exist_ok=False);reference=ROOT/'local/compatibility/grid-unpacked.exe';probe=ROOT/'local/behavior/contact-groups-probe.exe';rust=ROOT/f'target/{a.profile}/grid-contact-groups.exe'
    assert digest(reference)==ANALYSIS;cases=authored();source=out/'input.bin';source.write_bytes(pack(cases));runs={};original=None
    for kind,command in [('original',[str(probe),str(reference)]),('repeat',[str(probe),str(reference)]),('rust',[str(rust)])]:
        data,run=execute(command,source,out/f'{kind}.bin')
        if original is None:original=data
        run.update(compare(data,original));runs[kind]=run
        if kind!='rust':assert 'code_unchanged=1 no_imports=1' in run['stdout']
    checks,activity=semantic(cases,decode(original,cases));base=copy.deepcopy(cases[4]);good=pack([base]);bad={'truncated':good[:-1],'trailing':good+b'!','empty_cases':good[:4]+bytes(4)+good[8:]}
    mutations=[('foreign_order',lambda c:c['order'].__setitem__(0,16)),('duplicate_order',lambda c:c['order'].__setitem__(1,c['order'][0])),('selector',lambda c:c.update(selector=2))]
    for name,o,v in [('foreign_world',0x32c,1),('existing_link',0x320,1),('old_group',0x18,1),('sleeping_body',0x400,0xffffffff),('dirty_body',0x412,1),('special_group',0x2d,1),('nan_position',0x1c0,0x7fc00000)]:
        mutations.append((name,lambda c,o=o,v=v:c['bodies'][0].__setitem__(o,v) if o in [0x412,0x2d] else uput(c['bodies'][0],o,v)))
    for name,o,v in [('foreign_contact_body',4,17),('missing_contact_body',4,0),('contact_link',0x10,1),('shape_contact',0x73,1),('joint_contact',0xf,0),('nan_point',0x50,0x7fc00000)]:
        mutations.append((name,lambda c,o=o,v=v:c['contacts'][0].__setitem__(o,v) if o in [0x73,0xf] else uput(c['contacts'][0],o,v)))
    for name,change in mutations:c=copy.deepcopy(base);change(c);bad[name]=pack([c])
    c=copy.deepcopy(cases[2]);uput(c['contacts'][0],8,uget(c['contacts'][0],4));bad['self_two_contact']=pack([c])
    c=copy.deepcopy(base);fput(c['bodies'][0],0x1c0,3.4028234663852886e38);fput(c['contacts'][0],0x50,3.4028234663852886e38);bad['point_overflow']=pack([c])
    guards={}
    for name,data in bad.items():
        source=out/f'guard-{name}-input.bin';source.write_bytes(data);guards[name]={}
        for kind,command in [('original',[str(probe),str(reference)]),('rust',[str(rust)])]:
            target=out/f'guard-{name}-{kind}.bin';run=subprocess.run(command+[str(source),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=30)
            guards[name][kind]=dict(exit=run.returncode,stdout=run.stdout,stderr=run.stderr,passed=0<run.returncode<256 and 'contact_groups_fault=' not in run.stderr and not target.exists() and (kind=='rust' or not run.stderr and 'code_unchanged=1 no_imports=1' in run.stdout))
    report=dict(profile=a.profile,cases=len(cases),bytes=len(original)-8,probe_sha256=digest(probe),rust_sha256=digest(rust),analysis_sha256=ANALYSIS,runs=runs,checks=checks,activity=activity,guards=guards,scope='Whole serial AD6C40 cold all-active bodies and supplied contiguous impact pool; full body/contact words and selected world fields. Empty links/sleeping/shapes/persistent pools/parallel assignment. No geometry generation, solve or whole world/live cadence claim.')
    report['passed']=all(checks.values()) and all(r['passed'] for r in runs.values()) and all(r['passed'] for g in guards.values() for r in g.values());(out/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps({k:v for k,v in report.items() if k not in ['runs','guards']}));assert report['passed']
if __name__=='__main__':main()
