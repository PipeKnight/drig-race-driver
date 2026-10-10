"""Whole correction/publication and fresh normal-solver response handoff.

Authored bodies/shapes/group membership; no live whole-frame claim.
"""
import argparse
import copy
import json
import math
from pathlib import Path
import random
import struct
import subprocess

from capture_reference_spawn import private, ANALYSIS
from validate_steering_control import digest
from validate_contact_bias import compare
from validate_physical_response import PROBE as RESPONSE_PROBE

ROOT = Path(__file__).resolve().parents[1]
PROBE = '879ec96e3bf23787e8c141d0e1518fab28973cfb56107eef2f52fa1a77ec6a6a'
MAGIC = 0x31435050
IDENTITY = [1.,0.,0.,0.,0.,1.,0.,0.,0.,0.,1.,0.,0.,0.,0.,1.]
POINTERS = [0x328,0x32c,0x334,0x338,0x33c,0x340]


def floats(raw, offset, values): struct.pack_into(f'<{len(values)}f', raw, offset, *values)
def words(raw, offset, values): struct.pack_into(f'<{len(values)}I', raw, offset, *values)
def get(raw, offset, count=16): return struct.unpack_from(f'<{count}f', raw, offset)


def case(rng, head=0, shapes=2):
    # Opaque sentinels make unintended writes observable across entire buffers.
    body=bytearray(rng.randbytes(0x424))
    for offset in POINTERS: words(body,offset,[0])
    for offset in [0x90,0x150,0x190]: floats(body,offset,IDENTITY)
    floats(body,0x230,[0.]*4)
    nodes=[]
    for index in range(shapes):
        node=bytearray(rng.randbytes(0x220));words(node,0xe0,[0])
        floats(node,0xf0,IDENTITY);node[0x214]=index%2;nodes.append(node)
    return dict(body=body,head=head,shapes=nodes,linear=[0.]*4,angular=[0.]*4)


def controls():
    rng=random.Random(200810100416);cases=[]
    for i in range(8):
        c=case(rng,head=i%2,shapes=[0,1,2,64][i%4])
        if i>=2:c['linear']=[1.,2.,3.,.25]
        if i>=4:floats(c['body'],0x230,[.25,.5,.75,0.])
        if i>=6:floats(c['body'],0x1b0,[0.,0.,-1.,0.])
        cases.append(c)
    for index in range(504):
        c=case(rng,head=index%2,shapes=rng.choice([0,1,2,3,7,16]))
        angle=rng.uniform(-math.pi,math.pi);co,si=math.cos(angle),math.sin(angle)
        basis=[co,0.,-si,0.,0.,1.,0.,0.,si,0.,co,0.]+[rng.uniform(-500,500) for _ in range(3)]+[1.]
        if index%3==0:
            basis[:12]=[v+rng.uniform(-.15,.15) for v in basis[:12]]
        if index%7==0:basis[8:12]=[-v for v in basis[8:12]]
        floats(c['body'],0x190,basis)
        for offset in [0x90,0x150]:floats(c['body'],offset,[rng.uniform(-3,3) for _ in range(16)])
        floats(c['body'],0x230,[rng.uniform(-2,2) for _ in range(4)])
        c['linear']=[rng.uniform(-4,4) for _ in range(4)]
        c['angular']=[rng.uniform(-12,12) for _ in range(4)]
        if index<20:
            c['angular']=[0.]*4;c['angular'][index%4]=[0.,-0.,1e-25,1e-19,1e-17][index//4]
        for s in c['shapes']:floats(s,0xf0,[rng.uniform(-2,2) for _ in range(16)])
        cases.append(c)
    return cases


def pack(cases):
    data=bytearray(struct.pack('<II',MAGIC,len(cases)))
    for c in cases:
        data.extend(struct.pack('<II',c['head'],len(c['shapes'])))
        data.extend(c['body']);data.extend(struct.pack('<8f',*c['linear'],*c['angular']))
        for s in c['shapes']:data.extend(s)
    return data


def decode(data,cases):
    if data[:8]!=struct.pack('<II',MAGIC,len(cases)):raise ValueError('Output header')
    cursor=8;result=[]
    for c in cases:
        if struct.unpack_from('<II',data,cursor)!=(c['head'],len(c['shapes'])):raise ValueError('Output case header')
        cursor+=8;body=data[cursor:cursor+0x424];cursor+=0x424;shapes=[]
        for _ in c['shapes']:shapes.append(data[cursor:cursor+0x220]);cursor+=0x220
        result.append(dict(body=body,shapes=shapes))
    if cursor!=len(data):raise ValueError('Output length')
    return result


def execute(command,source,target):
    run=subprocess.run(command+[str(source),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=45)
    result=dict(exit=run.returncode,stdout=run.stdout,stderr=run.stderr)
    if run.returncode:raise RuntimeError(f'Execution failed: {result}')
    return target.read_bytes(),result


def semantic_checks(cases,results):
    identity=struct.pack('<16f',*IDENTITY)
    checks=dict(zero_correction_identity=results[0]['body'][0x190:0x1d0]==identity,
        translated_position=get(results[2]['body'],0x1c0,4)==(1.,2.,3.,1.25),
        offset_position=get(results[4]['body'],0x200,4)==(.75,1.5,2.25,1.25),
        negative_handedness=get(results[7]['body'],0x1b0,4)[2]<0,
        translation_ignored_by_derived=results[2]['body'][0xd0:0x110]==identity,
        full_shape_list=len(results[3]['shapes'])==64,
        unrelated_body_bytes_preserved=True,unrelated_shape_bytes_preserved=True,
        direct_shapes_equal_publication=True,group_heads_orthonormal=True)
    changed=[range(0xd0,0x150),range(0x190,0x210)]
    mask=set().union(*changed)
    for c,r in zip(cases,results):
        checks['unrelated_body_bytes_preserved'] &= all(a==b for i,(a,b) in enumerate(zip(c['body'],r['body'])) if i not in mask)
        for before,after in zip(c['shapes'],r['shapes']):
            checks['unrelated_shape_bytes_preserved'] &= all(a==b for i,(a,b) in enumerate(zip(before,after)) if not 0x130<=i<0x170)
            if not before[0x214]:checks['direct_shapes_equal_publication'] &= after[0x130:0x170]==r['body'][0x1d0:0x210]
        if c['head']:
            basis=[get(r['body'],0x190+i*16,4) for i in range(3)]
            checks['group_heads_orthonormal'] &= all(abs(sum(v*v for v in row)-1.)<2e-6 for row in basis)
            checks['group_heads_orthonormal'] &= abs(sum(a*b for a,b in zip(basis[0],basis[1])))<2e-6
    return checks


def main():
    parser=argparse.ArgumentParser();parser.add_argument('output',type=Path)
    parser.add_argument('--profile',choices=['debug','release'],default='debug');args=parser.parse_args()
    image=ROOT/'local/compatibility/grid-unpacked.exe';probe=ROOT/'local/behavior/physical-pose-probe.exe'
    response_probe=ROOT/'local/behavior/physical-response-probe.exe'
    rust=ROOT/f'target/{args.profile}/grid-physical-pose.exe'
    response_rust=ROOT/f'target/{args.profile}/grid-physical-response.exe'
    if digest(image)!=ANALYSIS or digest(probe)!=PROBE or digest(response_probe)!=RESPONSE_PROBE:raise ValueError('Image/probe identity')
    previous=ROOT/f'local/behavior/physical-response-20261010-{args.profile}-01'
    previous_report=json.loads((previous/'validation.json').read_text(encoding='utf-8'))
    if not previous_report['passed']:raise ValueError('Upstream validation')
    request=previous/'input.bin';baseline=(previous/'original.bin').read_bytes()
    if digest(request)!='78146034994f68ba104c102924bdcddfa3941350e2cb0527a908aac61134fbaa':raise ValueError('Upstream request identity')
    if any((previous/f'{label}.bin').read_bytes()!=baseline for label in ['repeat','rust']):raise ValueError('Upstream comparisons')
    out=private(args.output);out.mkdir(parents=True,exist_ok=False)
    report=dict(profile=args.profile,live_verification=False,analysis_sha256=ANALYSIS,probe_sha256=PROBE,
        rust_sha256=digest(rust),response_probe_sha256=RESPONSE_PROBE,response_rust_sha256=digest(response_rust),
        response_input_sha256=digest(request),upstream_runs={},runs={},guards={},
        scope='Whole B13B00 correction, B13720 refresh, conditional B12B10 Gram-Schmidt, B0F730 shapes, B100F0 derived matrices. Authored poses/group/shapes with fresh supplied normal-response corrections; no live whole physical frame.')
    for label,command in [('original',[str(response_probe),str(image)]),('rust',[str(response_rust)])]:
        data,r=execute(command,request,out/f'response-{label}.bin');r.update(compare(data,baseline));report['upstream_runs'][label]=r
        if not r['passed']:raise RuntimeError('Fresh normal response mismatch')
    cases=controls();authored=len(cases);cursor=8;handoff=[];skipped=0;candidates=[]
    scenarios,=struct.unpack_from('<I',baseline,4)
    rng=random.Random(200810100440)
    for scenario in range(scenarios):
        rows,bodies=struct.unpack_from('<II',baseline,cursor);cursor+=8+rows*192
        for body in range(bodies):
            correction=struct.unpack_from('<8f',baseline,cursor+body*32)
            if not any(correction):continue
            if sum(v*v for v in correction[4:])>4096:skipped+=1;continue
            candidates.append((scenario,body,correction))
        cursor+=bodies*32
    if cursor!=len(baseline):raise ValueError('Upstream output length')
    # Include both authored solver controls and the F152 correction-contact path.
    corrected=[c for c in candidates if c[0]>=518];authored_response=[c for c in candidates if c[0]<518]
    selected=corrected[:192]+authored_response[:192]
    if len(selected)<384:
        selected += corrected[192:192+384-len(selected)]
    if len(selected)<384:
        selected += authored_response[192:192+384-len(selected)]
    for scenario,body,correction in selected:
        c=case(rng,head=len(handoff)%2,shapes=len(handoff)%4)
        c['linear']=list(correction[:4]);c['angular']=list(correction[4:])
        floats(c['body'],0x230,[.2,-.3,.4,0.])
        cases.append(c);handoff.append(dict(scenario=scenario,body=body,correction_words=[f'{v:08x}' for v in struct.unpack('<8I',struct.pack('<8f',*correction))]))
    source=out/'input.bin';source.write_bytes(pack(cases));original=None;decoded=None
    commands=[('original',[str(probe),str(image)]),('repeat',[str(probe),str(image)]),('rust',[str(rust)])]
    for label,command in commands:
        data,r=execute(command,source,out/f'{label}.bin')
        if original is None:original=data;decoded=decode(data,cases)
        r.update(compare(data,original));report['runs'][label]=r
    report.update(cases=len(cases),authored_controls=authored,handoff_bodies=len(handoff),handoff=handoff,
        handoff_correction_path_bodies=sum(h['scenario']>=518 for h in handoff),
        handoff_authored_response_bodies=sum(h['scenario']<518 for h in handoff),
        upstream_angular_bound_skips=skipped,input_sha256=digest(source),semantic_checks=semantic_checks(cases,decoded))
    report['activity']=dict(group_head_cases=sum(c['head'] for c in cases),member_cases=sum(not c['head'] for c in cases),
        shapes=sum(len(c['shapes']) for c in cases),transformed_shapes=sum(s[0x214]!=0 for c in cases for s in c['shapes']),
        changed_basis_cases=sum(c['body'][0x190:0x1c0]!=r['body'][0x190:0x1c0] for c,r in zip(cases,decoded)),
        changed_position_cases=sum(c['body'][0x1c0:0x1d0]!=r['body'][0x1c0:0x1d0] for c,r in zip(cases,decoded)))
    base=case(random.Random(42),head=1,shapes=1);good=pack([base])
    bad=dict(truncated=good[:-1],trailing=good+b'\0',zero_cases=good[:4]+bytes(4)+good[8:],
        excess_cases=good[:4]+struct.pack('<I',1025)+good[8:])
    for name,mutate in [
        ('bad_head',lambda c:c.update(head=2)),('too_many_shapes',lambda c:c.update(shapes=c['shapes']*65)),
        ('body_pointer',lambda c:words(c['body'],0x334,[1])),
        ('shape_pointer',lambda c:words(c['shapes'][0],0xe0,[1])),
        ('shape_flag',lambda c:c['shapes'][0].__setitem__(0x214,2)),
        ('nan_basis',lambda c:floats(c['body'],0x190,[float('nan')])),
        ('nan_local',lambda c:floats(c['body'],0x90,[float('nan')])),
        ('infinite_offset',lambda c:floats(c['body'],0x230,[float('inf')])),
        ('nan_shape',lambda c:floats(c['shapes'][0],0xf0,[float('nan')])),
        ('angular_bound',lambda c:c.update(angular=[65.,0.,0.,0.])),
        ('nan_linear',lambda c:c.update(linear=[float('nan'),0.,0.,0.])),
        ('zero_basis',lambda c:floats(c['body'],0x190,[0.]*4)),
        ('parallel_basis',lambda c:floats(c['body'],0x1a0,[1.,0.,0.,0.]))]:
        c=copy.deepcopy(base);mutate(c);bad[name]=pack([c])
    for name,data in bad.items():
        source=out/f'guard-{name}.bin';source.write_bytes(data);report['guards'][name]={}
        for label,command in [commands[0],commands[2]]:
            target=out/f'guard-{name}-{label}.bin';run=subprocess.run(command+[str(source),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=20)
            report['guards'][name][label]=dict(exit=run.returncode,stderr=run.stderr,
                passed=0<run.returncode<256 and 'physical_pose_fault=' not in run.stderr and (label=='original' or not target.exists()))
    report['passed']=bool(all(report['semantic_checks'].values()) and all(report['activity'].values())
        and all(r['passed'] for r in report['upstream_runs'].values()) and all(r['passed'] for r in report['runs'].values())
        and all(r['passed'] for g in report['guards'].values() for r in g.values()))
    (out/'validation.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
    print(json.dumps({k:v for k,v in report.items() if k!='handoff'},indent=2))
    if not report['passed']:raise RuntimeError('Physical pose validation failed; inspect private evidence')


if __name__=='__main__':main()
