"""Complete active B7B850 with both registered deformation phases."""
import argparse
import collections
import copy
import json
import struct
from pathlib import Path
import validate_deformation_physical_frame as preparation
import validate_deformation_layout as layout
import validate_collision_mesh as execution
from inspect_installation import inspect_pe

ROOT=layout.ROOT
PROBE=ROOT/'local/behavior/deforming-physical-frame-probe.exe'


def constants():
    image=layout.IMAGE.read_bytes();pe=inspect_pe(layout.IMAGE)
    def words(address,count):
        section=next(s for s in pe['sections'] if 0<=address-0x400000-s['rva']<=s['raw_size']-count*4)
        at=section['raw_offset']+address-0x400000-section['rva'];return image[at:at+count*4]
    f=lambda address:struct.unpack('<f',words(address,1))[0]
    return dict(generation=[f(a) for a in [0xe4fc2c,0xe51d40,0xe500c8,0xe500d0,0xe500d4,0xe500cc]],
                body=list(struct.unpack('<5f',words(0xe500a4,5))),grid=[f(a) for a in [0xe4fc88,0xe4fc90,0xe4fc94]],iterations=struct.unpack('<I',words(0xe4fc8c,1))[0],
                response_scale=f(0xe4fc38),correction_gain=f(0xe500c0),subtract=struct.unpack('<I',words(0xe500c4,1))[0],policy_global=f(0xd1676c))


def cases():
    rows,identities=preparation.cases();defaults=constants()
    for index,row in enumerate(rows):
        for _,contact in row['contacts']:contact[0xc]=index%2
        s=row['deformation'];stem='lac' if index<128 else 'bmw';path=ROOT/f'local/behavior/deformation-source-debug-20261013-final-01/{stem}-rust.bin';source=path.read_bytes()
        settings=source[44+3*layout.SIZE:];assert len(settings)==0x1054
        owner=bytearray(0xd230);owner[0x2300:0xc800]=s['descriptor']
        struct.pack_into('<I',owner,0xdb0,index%2)
        # Complete independent owned car context; only these fields and the
        # source grid/settings are supplied to the unchanged original callback.
        owner[0x15a8:0x15ac]=row['bodies'][0]['raw'][0x3f0:0x3f4]
        struct.pack_into('<4f',owner,0x1630,.1,-.2,.3,0.);struct.pack_into('<4f',owner,0xb20,*[[0.,0.,0.,0.],[5.,0.,2.,0.],[0.,-2.,4.,0.]][index%3])
        for i in range(10):struct.pack_into('<f',owner,0x290+i*4,(i%3)*.1 if index%3 else 0.)
        if index%3!=2:struct.pack_into('<f',owner,0x290,15.)
        struct.pack_into('<f',owner,0x2e0,.5);struct.pack_into('<f',owner,0xd16c,[1.,.75,1.25][index%3]);struct.pack_into('<f',owner,0xd22c,1.)
        p=defaults.copy();p['generation']=list(p['generation']);p['generation'][0:2]=[s['parameters'][2],s['parameters'][5]]
        p.update(clip=index%2,policy_scale=s['parameters'][6],table=s['parameters'][7:23],disabled=s['disabled'],seed=(1+index*214013)&0xffffffff)
        row.update(owner=bytes(owner),settings=settings,bulk=p,correction=[0,0.,0.,0.,0.])
    corrected=copy.deepcopy(rows)
    for row in corrected:row['correction']=[4,.01,1.,0.,.8]
    return rows+corrected,identities


def parameter_bytes(p):
    data=struct.pack('<14fIIffIff16fII',*p['generation'],*p['body'],*p['grid'],p['iterations'],p['clip'],p['response_scale'],p['correction_gain'],p['subtract'],p['policy_global'],p['policy_scale'],*p['table'],p['disabled'],p['seed'])
    assert len(data)==156;return data


def request(rows):
    out=bytearray(b'PFD3'+struct.pack('<I',len(rows)))
    for row in rows:out.extend(preparation.request([row])[8:]);out.extend(row['owner']);out.extend(row['settings']);out.extend(parameter_bytes(row['bulk']));out.extend(struct.pack('<I4f',*row['correction']))
    return bytes(out)


def answer_size(row):return preparation.answer_size(row)-layout.SIZE+0xd230+4


def decode(data,rows):
    assert data[:8]==b'PFD3'+struct.pack('<I',len(rows));at=8;answers=[]
    for row in rows:
        size=answer_size(row);r=data[at:at+size];assert len(r)==size
        frame=r[:-0xd234];owner=r[-0xd234:-4];seed=struct.unpack_from('<I',r,len(r)-4)[0]
        decoded=preparation.frame.decode(b'PFR1'+struct.pack('<I',1)+frame,[row])[0];answers.append((decoded,owner,seed));at+=size
    assert at==len(data);return answers


def edit_owner(row,at,value):
    owner=bytearray(row['owner']);struct.pack_into('<I',owner,at,value);row['owner']=bytes(owner)
    if 0x2300<=at<0xc800:row['deformation']['descriptor']=bytes(owner[0x2300:0xc800])


def guard_payloads(rows):
    good=request([rows[5]]);bad={'magic':b'BAD!'+good[4:],'zero_count':good[:4]+bytes(4)+good[8:],'truncated':good[:-1],'trailing':good+b'x'}
    for name,mutate in [
        ('zero_delta',lambda r:r['parameters'].__setitem__(0,0.)),
        ('foreign_callback',lambda r:struct.pack_into('<I',r['bodies'][0]['raw'],0x418,1)),
        ('nan_basis',lambda r:struct.pack_into('<I',r['bodies'][0]['raw'],0x190,0x7fc00000)),
        ('null_shape',lambda r:struct.pack_into('<I',r['contacts'][0][1],0x28,0)),
        ('dynamic_contact',lambda r:r['contacts'].__setitem__(0,(1,r['contacts'][0][1]))),
        ('nan_old_cap',lambda r:struct.pack_into('<I',r['contacts'][0][1],0x3c,0x7fc00000)),
        ('invalid_static_flag',lambda r:r['contacts'][0][1].__setitem__(0xc,2)),
        ('ring',lambda r:edit_owner(r,0xc7ec,10)),('dimension',lambda r:edit_owner(r,0xc714,17)),
        ('nan_node',lambda r:edit_owner(r,0x4880,0x7fc00000)),('edges',lambda r:edit_owner(r,0xc710,1201)),
        ('foreign_lookup',lambda r:edit_owner(r,0xdb0,2)),('foreign_settings',lambda r:edit_owner(r,0xc7f8,1)),
        ('zero_owner_mass',lambda r:edit_owner(r,0x15a8,0)),('zero_factor',lambda r:edit_owner(r,0xd16c,0)),
        ('aliased_table',lambda r:r['bulk']['table'].__setitem__(0,r['bulk']['table'][0]+1.)),
        ('iterations',lambda r:r['bulk'].__setitem__('iterations',0)),('disable',lambda r:r['bulk'].__setitem__('disabled',2)),
        ('correction_iterations',lambda r:r['correction'].__setitem__(0,33)),('nan_correction',lambda r:r['correction'].__setitem__(1,float('nan'))),
    ]:
        r=copy.deepcopy(rows[5]);mutate(r);bad[name]=request([r])
    return good,bad


def check_answer(row,frame,owner,seed,coverage):
    assert len(owner)==0xd230
    old=row['owner'];coverage['finished']+=frame['started'] and frame['status']==0;coverage['unstarted']+=not frame['started'];coverage['capacity_failed']+=frame['started'] and frame['status']!=0
    coverage['seed_changed']+=seed!=row['bulk']['seed'];coverage['nodes_changed']+=owner[0x4880:0xc580]!=old[0x4880:0xc580]
    coverage['contact38_changed']+=any(c[0x38:0x3c]!=before[0x38:0x3c] for c,(_,before) in zip(frame['contacts'],row['contacts']))
    invoked=bool(frame['started'] and frame['status']==0 and row['correction'][0] and row['correction'][4]>0)
    coverage['correction_invoked']+=invoked
    coverage['correction_active']+=invoked and any(not c[0xe] and (struct.unpack_from('<f',c,0x38)[0]-row['correction'][3])*row['correction'][2]>=0 for c in frame['contacts'])
    assert all(owner[a:a+4]==old[a:a+4] for a in range(0,0xd230,4) if not (0x2300<=a<0xc800 or 0x290<=a<0x2b8 or a==0x2e0))
    for i in range(400):
        at=0x4880+i*80;assert owner[at+16:at+48]==old[at+16:at+48] and owner[at+64:at+80]==old[at+64:at+80]
    if not frame['started']:assert owner==old and seed==row['bulk']['seed']


def paired(rows,answers):
    half=len(rows)//2;counts=collections.Counter()
    for row,base,corrected in zip(rows[:half],answers[:half],answers[half:]):
        a,owner,seed=base;b,other,other_seed=corrected
        assert owner==other and seed==other_seed and a['contacts']==b['contacts']
        counts['owner_seed_contacts_retained']+=1
        counts['pose_changed']+=a['bodies'][0][0x190:0x210]!=b['bodies'][0][0x190:0x210]
    return dict(counts)


def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--profile',choices=['debug','release'],required=True);p.add_argument('--pilot',action='store_true');args=p.parse_args()
    out=args.output.resolve();assert out.is_relative_to((ROOT/'local').resolve());out.mkdir(exist_ok=False);rows,identities=cases()
    if args.pilot:rows=rows[:16]+rows[256:272]
    rust=ROOT/f'target/{args.profile}/grid-deforming-physical-frame.exe';reports=[];coverage=collections.Counter();all_answers=[]
    for start in range(0,len(rows),16):
        batch=rows[start:start+16];source=out/f'{start}-input.bin';source.write_bytes(request(batch));answers={};runs={}
        for kind in ['original','repeat','rust']:
            exe=rust if kind=='rust' else PROBE;target=out/f'{start}-{kind}.bin';run=execution.run(exe,([layout.IMAGE] if kind!='rust' else [])+[source,target]);runs[kind]=dict(**run,executable_sha256=layout.digest(exe));(out/f'{start}-execution.json').write_text(json.dumps(runs,indent=2))
            assert run['exit']==0 and not run['stderr'],runs[kind]
            if kind!='rust':assert 'full_image_unchanged=1 owned_canaries=1 real_imports=7 original_rand=1' in run['stdout']
            answers[kind]=target.read_bytes()
        a,b=answers['original'],answers['rust'];assert len(a)==8+sum(answer_size(r) for r in batch) and len(b)==len(a)
        differences=[dict(offset=hex(i),original=a[i:i+4].hex(),rust=b[i:i+4].hex()) for i in range(8,len(a),4) if a[i:i+4]!=b[i:i+4]]
        item=dict(start=start,cases=len(batch),bytes=len(a)-8,repeat_exact=a==answers['repeat'],differences=differences,executions=runs);(out/f'{start}-validation.json').write_text(json.dumps(item,indent=2));reports.append(item)
        assert item['repeat_exact'] and not differences,differences[:12]
        decoded=decode(a,batch);all_answers.extend(decoded)
        for row,(frame,owner,seed) in zip(batch,decoded):
            check_answer(row,frame,owner,seed,coverage)
    pairs=paired(rows,all_answers)
    good,bad=guard_payloads(rows);guards=[]
    for name,payload in bad.items():
        source=out/f'guard-{name}-input.bin';source.write_bytes(payload);runs={}
        for kind in ['original','repeat','rust']:
            exe=rust if kind=='rust' else PROBE;target=out/f'guard-{name}-{kind}.bin';run=execution.run(exe,([layout.IMAGE] if kind!='rust' else [])+[source,target]);runs[kind]=dict(**run,output_exists=target.exists())
            assert (run['exit']==1 if kind=='rust' else 0<run['exit']<256) and 'fault=' not in run['stderr'] and not target.exists(),runs[kind]
        guards.append(dict(name=name,executions=runs))
    for kind in ['original','rust']:
        source=out/f'guard-existing-{kind}-input.bin';source.write_bytes(good);target=out/f'guard-existing-{kind}.bin';target.write_bytes(b'owned sentinel');exe=rust if kind=='rust' else PROBE
        run=execution.run(exe,([layout.IMAGE] if kind!='rust' else [])+[source,target]);assert run['exit']==(1 if kind=='rust' else 6) and target.read_bytes()==b'owned sentinel';guards.append(dict(name='existing-'+kind,execution=run,preserved=True))
    (out/'guards.json').write_text(json.dumps(guards,indent=2))
    report=dict(profile=args.profile,cases=len(rows),bytes=sum(r['bytes'] for r in reports),identities=identities,reports=reports,guards=guards,coverage=dict(coverage),pairs=pairs,files={p.name:dict(size=p.stat().st_size,sha256=layout.digest(p)) for p in sorted(out.iterdir()) if p.is_file()},scope='Whole singlebody/staticfirstpool B7B850 with both actual registrations, owned car/CRT/source grids/settings and paired correction on/off; authored solver/body state, no live cadence or driving dirty consumers.')
    (out/'validation.json').write_text(json.dumps(report,indent=2));print(json.dumps({k:report[k] for k in ['profile','cases','bytes','coverage']}))


if __name__=='__main__':main()
