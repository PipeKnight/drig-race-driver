"""Owned shape classification and null-second contacts through complete solving."""
import argparse,copy,json,struct,subprocess
from pathlib import Path
import validate_grouped_physical_frame as g
from validate_physical_tick import pack as tick_pack
from validate_main_contact_response import uget,uput,fget
from validate_steering_control import digest
from validate_contact_bias import compare,execute
from capture_reference_spawn import ANALYSIS,private
ROOT=g.ROOT
MAGIC=0x34534647

def pack(cases):
    data=bytearray(struct.pack('<II',MAGIC,len(cases)))
    for c in cases:
        raw=tick_pack([c])[8:]
        data.extend(raw[:164]);data.extend(struct.pack('<I',len(c['shapes'])))
        data.extend(b''.join(bytes(s) for s in c['shapes']));data.extend(raw[164:])
    assert len(data)<=32*1024*1024
    return data

def authored():
    cases=g.authored()
    for i,c in enumerate(cases):
        c['shapes']=[(i%2,0,1,0),(1,0,0,0)]
        for j,(mode,r) in enumerate(c['contacts']):
            uput(r,0x28,1);uput(r,0x2c,2);r[0x73]=int((i+j)%3!=0)
            if not mode and (i+j)%2:
                c['contacts'][j]=(2,r);r[0xc]=0;uput(r,8,0xffffffff);uput(r,0x20,0xffffffff)
    base=copy.deepcopy(next(c for c in cases if c['contacts'] and len(c['bodies'])>=3))
    for bits in range(64):
        c=copy.deepcopy(base);c['shapes']=[(bits%2,(bits>>0)&1,(bits>>2)&1,(bits>>4)&1),(1,(bits>>1)&1,(bits>>3)&1,(bits>>5)&1)]
        c['contacts']=c['contacts'][:4]
        for _,r in c['contacts']:r[0x73]=1
        c['classification_bits']=bits;cases.append(c)
    c=copy.deepcopy(base);c['shapes']=[(0,0,1,0)]+[(1,0,0,0)]*512
    for _,r in c['contacts']:uput(r,0x2c,513)
    cases.append(c)
    return cases

def semantics(cases,outputs,advance):
    checks=dict(classification=True,shape_material_identity=True,body_identity=True,null_second_index=True,special_retained=True,normal_retained=True,finished_forces=True,world_body_count=True,first_point=True,history=True)
    activity=dict(contacts=0,null_second=0,special=0,enabled=0,disabled=0,velocity_changes=0,pose_changes=0,bodies=0,groups=0,normal_rows=0,friction_rows=0,classification_controls=0)
    rounded=lambda x:struct.unpack('<f',struct.pack('<f',x))[0]
    for c,o in zip(cases,outputs):
        activity['bodies']+=len(c['bodies']);activity['groups']+=sum(uget(b,0x18)==i+1 for i,b in enumerate(o['bodies']))
        activity['classification_controls']+='classification_bits' in c
        checks['world_body_count'] &= o['world'][3]==len(c['bodies'])
        checks['finished_forces'] &= all(b[0x250:0x270]==bytes(32) and not b[0x40d] and not b[0x40e] for b in o['bodies'])
        for b,a in zip(c['bodies'],o['bodies']):
            activity['velocity_changes']+=b['raw'][0x210:0x230]!=a[0x210:0x230];activity['pose_changes']+=b['raw'][0x190:0x210]!=a[0x190:0x210]
            checks['history'] &= (b['raw'][0x190:0x210] if advance else b['raw'][0x270:0x2f0])==a[0x270:0x2f0]
        for (mode,r),a in zip(c['contacts'],o['contacts']):
            sa=c['shapes'][uget(r,0x28)-1];sb=c['shapes'][uget(r,0x2c)-1]
            disabled=not r[0x73] or sa[1] or sb[1] or (sa[2] and sb[3]) or (sb[2] and sa[3])
            checks['classification'] &= a[0xe]==bool(disabled)
            checks['shape_material_identity'] &= a[0x28:0x38]==r[0x28:0x38]
            checks['body_identity'] &= a[4:0xd]==r[4:0xd]
            checks['null_second_index'] &= (mode!=2 or uget(a,0x20)==0xffffffff)
            checks['special_retained'] &= a[0x71:0x74]==r[0x71:0x74]
            checks['normal_retained'] &= a[0x60:0x70]==r[0x60:0x70]
            body=c['bodies'][uget(r,4)]['raw']
            for k in range(4):
                center=fget(body,0x1c0+k*4)
                if advance:center=rounded(rounded(c['parameters'][0]*fget(body,0x220+k*4))+center)
                checks['first_point'] &= a[0x50+k*4:0x54+k*4]==struct.pack('<f',center+fget(r,0x50+k*4))
            activity['contacts']+=1;activity['null_second']+=mode==2;activity['special']+=bool(r[0x73]);activity['disabled' if disabled else 'enabled']+=1
            activity['normal_rows']+=uget(a,0x18);activity['friction_rows']+=uget(a,0x14)-uget(a,0x18)
    return checks,activity

def main():
    global MAGIC
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--profile',required=True,choices=['debug','release']);p.add_argument('--advance',action='store_true');p.add_argument('--limit',type=int,default=0);a=p.parse_args()
    if a.advance:MAGIC=0x34535447
    out=private(a.output);out.mkdir(exist_ok=False);reference=ROOT/'local/compatibility/grid-unpacked.exe';assert digest(reference)==ANALYSIS
    probe=ROOT/f'local/behavior/shaped-physical-{"tick" if a.advance else "frame"}-probe.exe';rust=ROOT/f'target/{a.profile}/grid-physical-tick.exe'
    cases=authored();cases=cases[:a.limit] if a.limit else cases;source=out/'input.bin';source.write_bytes(pack(cases));runs={};original=None
    for kind,command in [('original',[str(probe),str(reference)]),('repeat',[str(probe),str(reference)]),('rust',[str(rust)])]:
        data,run=execute(command,source,out/f'{kind}.bin');original=data if original is None else original;run.update(compare(data,original));runs[kind]=run
        if kind!='rust':assert 'code_unchanged=1 no_imports=1' in run['stdout']
    g.MAGIC=MAGIC;checks,activity=semantics(cases,g.decode(original,cases),a.advance)
    base=copy.deepcopy(next(c for c in cases if c['contacts']));good=pack([base]);bad={'truncated':good[:-1],'trailing':good+b'!','empty':good[:4]+bytes(4)+good[8:]}
    mutations=[('empty_shapes',lambda c:c.update(shapes=[])),('shape_flag',lambda c:c['shapes'].__setitem__(0,(2,0,0,0))),('foreign_shape',lambda c:uput(c['contacts'][0][1],0x28,514)),('missing_special_shape',lambda c:(c['contacts'][0][1].__setitem__(0x73,1),uput(c['contacts'][0][1],0x28,0))),('foreign_body',lambda c:uput(c['contacts'][0][1],4,16)),('zero_iterations',lambda c:c.update(iterations=0)),('insufficient_capacity',lambda c:c.update(capacity=0)),('body_callback',lambda c:uput(c['bodies'][0]['raw'],0x418,1)),('nan_basis',lambda c:uput(c['bodies'][0]['raw'],0x190,0x7fc00000))]
    for name,change in mutations:c=copy.deepcopy(base);change(c);bad[name]=pack([c])
    guards={}
    for name,data in bad.items():
        source=out/f'guard-{name}-input.bin';source.write_bytes(data);guards[name]={}
        for kind,command in [('original',[str(probe),str(reference)]),('rust',[str(rust)])]:
            target=out/f'guard-{name}-{kind}.bin';r=subprocess.run(command+[str(source),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=30)
            guards[name][kind]=dict(exit=r.returncode,stdout=r.stdout,stderr=r.stderr,passed=0<r.returncode<256 and 'physical_tick_fault=' not in r.stderr and not target.exists() and (kind=='rust' or not r.stderr and 'code_unchanged=1 no_imports=1' in r.stdout))
    report=dict(profile=a.profile,advance=a.advance,cases=len(cases),bytes=len(original)-8,runs=runs,checks=checks,activity=activity,guards=guards,probe_sha256=digest(probe),rust_sha256=digest(rust),analysis_sha256=ANALYSIS,scope='Owned shape216/217/229/230 classification; ordinary/special and C0/nullsecond contacts; whole AD6C40/B77310/B7B850, optional preceding AD7040. Cold active bodies/MAX sleep duration/full capacity; no geometric production/persistence/full world schedule/driving.')
    report['passed']=all(checks.values()) and all(r['passed'] for r in runs.values()) and all(r['passed'] for pair in guards.values() for r in pair.values());(out/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps({k:v for k,v in report.items() if k not in ['runs','guards']}));assert report['passed']
if __name__=='__main__':main()
