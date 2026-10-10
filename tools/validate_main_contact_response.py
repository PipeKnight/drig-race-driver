"""Complete main preparation and four-span solve; constructed-row handoff."""
import argparse
import copy
import json
from pathlib import Path
import random
import struct
import subprocess

from capture_reference_spawn import private, ANALYSIS
from validate_contact_bias import compare, execute
from validate_contact_rows import decode as construction_decode, PROBE as CONSTRUCTION_PROBE
from validate_steering_control import digest

ROOT = Path(__file__).resolve().parents[1]
MAGIC = 0x3152434d
PROBE = 'e90f912bd17e1449606209bef2aae680151add18125d5995a6b4e9f15ffca921'


def fput(row, offset, value): struct.pack_into('<f', row, offset, value)
def uput(row, offset, value): struct.pack_into('<I', row, offset, value)
def fget(row, offset): return struct.unpack_from('<f', row, offset)[0]
def uget(row, offset): return struct.unpack_from('<I', row, offset)[0]


def pack(cases):
    data = bytearray(struct.pack('<II', MAGIC, len(cases)))
    for c in cases:
        data += struct.pack('<7I3f', len(c['rows']), len(c['bodies']), *c['ends'], c['iterations'], *c['parameters'])
        data += b''.join(c['rows'])
        for b in c['bodies']: data += struct.pack('<25f', *b)
    return bytes(data)


def bodies(rng, n, scale=1.):
    result = []
    for _ in range(n):
        matrix = [0.]*16
        for i in range(4):
            for j in range(4): matrix[i*4+j] = scale * ((1.+.25*i) if i == j else .025*(i+j+1))
        result.append([*[rng.uniform(-2, 2) for _ in range(8)], scale, *matrix])
    return result


def authored(rng, number):
    counts = [[0, 0, 0, 0], [1, 0, 0, 0], [0, 1, 0, 0], [1, 0, 3, 0], [0, 1, 0, 3],
              [2, 2, 6, 6], [5, 7, 15, 21], [16, 16, 48, 48], [32, 32, 96, 96]][number % 9]
    rows = []; ends = []; nb = 1+number % 8
    for span, count in enumerate(counts):
        remaining = count
        while remaining:
            length = min(remaining, 1+(number+remaining) % 4)
            a = rng.randrange(nb); b = a if number % 7 == 0 else rng.randrange(nb)
            for j in range(length):
                row = bytearray(struct.pack('<48f', *[rng.uniform(-1, 1) for _ in range(48)]))
                fput(row,0x80,rng.uniform(-5, 10)); fput(row,0x84,[0., .1, 1., 10.][number % 4])
                fput(row,0x88,-2. if number % 6 == 0 or span >= 2 else 0.)
                fput(row,0x8c,2. if span < 2 else .5)
                uput(row,0xa0,a); uput(row,0xa4,b); uput(row,0xb4,length if j == 0 else 0)
                uput(row,0xa8,rng.randrange(sum(counts[:2])) if span >= 2 else 0)
                rows.append(row)
            remaining -= length
        ends.append(len(rows))
    return dict(rows=rows,ends=ends,bodies=bodies(rng,nb), iterations=[0,1,2,4,8,16,32][number % 7],
                parameters=[[0., .5, 1., 2.][number % 4], [0., .1, 1., 10.][(number//4) % 4],
                            [0., .5, 1., 1.25][(number//16) % 4]])


def decode(data, cases):
    at=8; result=[]
    for c in cases:
        n, nb = struct.unpack_from('<II',data,at); at+=8
        if n!=len(c['rows']) or nb!=len(c['bodies']): raise ValueError('Output counts')
        stages=[]
        for _ in range(3):
            stages.append([data[at+i*192:at+(i+1)*192] for i in range(n)]);at+=n*192
        workspace=data[at:at+nb*32];at+=nb*32
        result.append(dict(stages=stages,workspace=workspace))
    if at!=len(data):raise ValueError('Output framing')
    return result


def semantic(cases, decoded):
    checks=dict(response_unscaled=True,zero_iterations_clear=True,friction_prefix_preserved=True,
                main_normalization_saved=True,opaque_fields_preserved=True)
    activity=dict(normals=0,friction=0,changed_bias=0,changed_softness=0,changed_response=0,
                  nonzero_normal_impulses=0,nonzero_friction_impulses=0,changed_workspace_cases=0,
                  zero_iteration_cases=0,empty_cases=0,alias_batches=0,negative_normal_prefix=0)
    for c,o in zip(cases,decoded):
        biased,prepared,solved=o['stages']; normal=c['ends'][1]
        activity['normals']+=normal;activity['friction']+=len(c['rows'])-normal
        activity['zero_iteration_cases']+=c['iterations']==0;activity['empty_cases']+=not c['rows']
        activity['changed_workspace_cases']+=any(o['workspace'])
        if c['iterations']==0:checks['zero_iterations_clear'] &= o['workspace']==bytes(len(o['workspace']))
        for i,(before,b,p,s) in enumerate(zip(c['rows'],biased,prepared,solved)):
            activity['changed_bias']+=before[0x90:0x94]!=b[0x90:0x94]
            activity['changed_softness']+=before[0x84:0x88]!=b[0x84:0x88]
            activity['changed_response']+=before[0x40:0x80]!=p[0x40:0x80]
            activity['alias_batches']+=uget(before,0xa0)==uget(before,0xa4) and uget(before,0xb4)>0
            activity['nonzero_normal_impulses' if i<normal else 'nonzero_friction_impulses']+=fget(s,0x94)!=0.
            activity['negative_normal_prefix']+=i<normal and fget(s,0x98)<0.
            checks['response_unscaled'] &= p[0x40:0x80]==s[0x40:0x80]
            checks['main_normalization_saved'] &= p[0xac:0xb4]==s[0xac:0xb4] and fget(p,0x94)==0.
            checks['opaque_fields_preserved'] &= before[0xa0:0xac]==s[0xa0:0xac] and before[0xb4:]==s[0xb4:]
            if i>=normal:checks['friction_prefix_preserved'] &= before[0x98:0x9c]==s[0x98:0x9c]
    return checks,activity


def construction_layout(data):
    at=8;result=[]
    if struct.unpack_from('<II',data)!=(0x31575243,711):raise ValueError('Construction request framing')
    for _ in range(711):
        nb,nm,nc=struct.unpack_from('<III',data,at);at+=96+nb*56+nm*52+nc*80
        pools=[]
        for p in range(4):
            count=max(nc,1)*(3 if p%2 else 1)
            pools.append([None]*count);at+=count*192
        result.append(dict(rows=pools,body_count=nb))
    if at!=len(data):raise ValueError('Construction input size')
    return result


def handoff_case(rng,layout,decoded):
    # Preserve supplied numeric/opaque words. Reframe selected batches and pointer
    # identities explicitly; discard normal limits outside this adapter's domain.
    pools=[[] for _ in range(4)];mapping={};skipped=0;skipped_friction=0
    counts=[v//192 for v in decoded['context'][5:9]]
    for p in [2,0]:
        for i,row in enumerate(decoded['rows'][p][:counts[p]]):
            if fget(row,0x88)>fget(row,0x8c):skipped+=1;continue
            mapping[(p,i)]=sum(len(pools[q]) for q in [2,0] if q!=p and q==2)+len(pools[p])
            pools[p].append(bytearray(row))
    for p in [3,1]:
        for row in decoded['rows'][p][:counts[p]]:
            token=uget(row,0xa8);key=((token>>24)-1,(token&0xffffff)//192)
            if key not in mapping:skipped_friction+=1;continue
            row=bytearray(row);uput(row,0xa8,mapping[key]);pools[p].append(row)
    result=[];ends=[]
    for p in [2,0,3,1]:
        pool=pools[p];start=0
        while start<len(pool):
            end=start+1;key=pool[start][0xa0:0xa8] if p>=2 else pool[start][0xa0:0xa4]
            while end<len(pool) and (pool[end][0xa0:0xa8] if p>=2 else pool[end][0xa0:0xa4])==key:end+=1
            uput(pool[start],0xb4,end-start);start=end
        result+=pool;ends.append(len(result))
    return dict(rows=result,ends=ends,bodies=bodies(rng,layout['body_count'],64.),iterations=4,
                parameters=[1.,.25,.8]),skipped,skipped_friction


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('output',type=Path)
    parser.add_argument('--profile',choices=['debug','release'],default='debug');args=parser.parse_args()
    image=ROOT/'local/compatibility/grid-unpacked.exe';probe=ROOT/'local/behavior/main-contact-response-probe.exe'
    rust=ROOT/f'target/{args.profile}/grid-main-contact-response.exe'
    if digest(image)!=ANALYSIS or digest(probe)!=PROBE:raise ValueError('Image/probe identity')
    out=private(args.output);out.mkdir(parents=True,exist_ok=False)
    rng=random.Random(200810100520);cases=[authored(rng,i) for i in range(512)]
    previous=ROOT/f'local/behavior/contact-rows-20261010-{args.profile}-final-04'
    upstream=previous/'input.bin';baseline=(previous/'original.bin').read_bytes()
    if digest(upstream)!='775937b06729661e39800ac43a0ef3f3cc5a4a8deaa7f79f24b81dcdeff017ec':raise ValueError('Construction request identity')
    if any((previous/f'{label}.bin').read_bytes()!=baseline for label in ['repeat','rust']):raise ValueError('Construction outputs disagree')
    construction=ROOT/'local/behavior/contact-rows-probe.exe'
    if digest(construction)!=CONSTRUCTION_PROBE:raise ValueError('Construction probe identity')
    upstream_runs={}
    for label,command in [('original',[str(construction),str(image)]),('rust',[str(ROOT/f'target/{args.profile}/grid-contact-rows.exe')])]:
        data,run=execute(command,upstream,out/f'construction-{label}.bin');run.update(compare(data,baseline));upstream_runs[label]=run
        if not run['passed']:raise ValueError('Fresh construction mismatch')
    layouts=construction_layout(upstream.read_bytes());constructed=construction_decode(baseline,layouts)
    skipped=skipped_friction=0
    for layout,decoded in zip(layouts[528:],constructed[528:]):
        c,s,t=handoff_case(rng,layout,decoded);cases.append(c);skipped+=s;skipped_friction+=t
    source=out/'input.bin';source.write_bytes(pack(cases))
    report=dict(profile=args.profile,analysis_sha256=ANALYSIS,probe_sha256=PROBE,rust_sha256=digest(rust),
                input_sha256=digest(source),cases=len(cases),authored_controls=512,handoff_cases=len(cases)-512,
                handoff_normal_limit_skips=skipped,handoff_dependent_friction_skips=skipped_friction,
                handoff_normals=sum(c['ends'][1] for c in cases[512:]),
                handoff_friction=sum(len(c['rows'])-c['ends'][1] for c in cases[512:]),
                upstream_runs=upstream_runs,construction_probe_sha256=CONSTRUCTION_PROBE,
                scope='Whole main preparation and four solve spans; supplied rows. Handoff preserves numeric row words, explicitly reframes batches/references, drops reversed normal bounds. No live frame.',
                runs={},guards={},live_verification=False)
    commands=[('original',[str(probe),str(image)]),('repeat',[str(probe),str(image)]),('rust',[str(rust)])]
    original=None
    for label,command in commands:
        data,run=execute(command,source,out/f'{label}.bin')
        if original is None:original=data
        run.update(compare(data,original));report['runs'][label]=run
    checks,activity=semantic(cases,decode(original,cases));report.update(semantic_checks=checks,activity=activity)
    base=authored(random.Random(42),3);good=pack([base])
    bad=dict(truncated=good[:-1],trailing=good+b'\0',zero_cases=good[:4]+bytes(4)+good[8:],
             excess_cases=good[:4]+struct.pack('<I',1025)+good[8:])
    for name,offset,value in [('excess_rows',8,257),('excess_bodies',12,65),('bad_span_order',16,99),('bad_final_end',28,99),('excess_iterations',32,33)]:
        data=bytearray(good);struct.pack_into('<I',data,offset,value);bad[name]=data
    for name,mutate in [
        ('zero_batch',lambda c:uput(c['rows'][0],0xb4,0)),('oversized_batch',lambda c:uput(c['rows'][0],0xb4,257)),
        ('unknown_body',lambda c:uput(c['rows'][0],0xa0,99)),('unknown_second_body',lambda c:uput(c['rows'][0],0xa4,99)),
        ('invalid_reference',lambda c:uput(c['rows'][1],0xa8,99)),
        ('nan_jacobian',lambda c:fput(c['rows'][0],0,float('nan'))),('nan_bias',lambda c:fput(c['rows'][0],0x80,float('nan'))),
        ('nan_response',lambda c:fput(c['rows'][0],0x40,float('nan'))),('nan_softness',lambda c:fput(c['rows'][0],0x84,float('nan'))),
        ('reversed_bounds',lambda c:fput(c['rows'][0],0x88,99.)),
        ('nan_workspace',lambda c:c['bodies'][0].__setitem__(0,float('nan'))),
        ('nan_mass',lambda c:c['bodies'][0].__setitem__(8,float('nan'))),
        ('nan_inertia',lambda c:c['bodies'][0].__setitem__(10,float('nan'))),
        ('nan_scale',lambda c:c['parameters'].__setitem__(0,float('nan'))),
        ('singular_normalization',lambda c:[fput(c['rows'][0],i*4,0.) for i in list(range(16))+[33]]),
        ('overflow_bias',lambda c:(fput(c['rows'][0],0x80,3e38),c['parameters'].__setitem__(1,3e38)))]:
        c=copy.deepcopy(base);mutate(c);bad[name]=pack([c])
    for name,request in bad.items():
        guard=out/f'guard-{name}.bin';guard.write_bytes(request);report['guards'][name]={}
        for label,command in [commands[0],commands[2]]:
            target=out/f'guard-{name}-{label}.bin';run=subprocess.run(command+[str(guard),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=20)
            report['guards'][name][label]=dict(exit=run.returncode,stderr=run.stderr,
                passed=0<run.returncode<256 and 'main_contact_response_fault=' not in run.stderr and (label=='original' or not target.exists()))
    report['regressions']={}
    for name,folder,request_sha,probe_sha in [
        ('physical-response','physical-response-20261010-{profile}-01','78146034994f68ba104c102924bdcddfa3941350e2cb0527a908aac61134fbaa','34c4e9b65c422ae421ea7a1598217080f9d99fb1a85bdadc05024ffc3cbed598'),
        ('impact-correction','impact-correction-20261010-{profile}-final-04','4341ffa6b5042c0c4def8f95a95091ca1c99a4c7aed546f6139379ff7c8e1be1','b1248de7fa8fb655d3598609da110e805099c88ba18f7762dc9ae36175885f57')]:
        prior=ROOT/'local/behavior'/folder.format(profile=args.profile);request=prior/'input.bin'
        original=(prior/'original.bin').read_bytes();native=ROOT/f'local/behavior/{name}-probe.exe'
        if digest(request)!=request_sha or digest(native)!=probe_sha or not json.loads((prior/'validation.json').read_text())['passed']:
            raise ValueError('Prior regression identity')
        if any((prior/f'{label}.bin').read_bytes()!=original for label in ['repeat','rust']):raise ValueError('Prior regression mismatch')
        report['regressions'][name]={}
        for label,command in [('original',[str(native),str(image)]),('rust',[str(ROOT/f'target/{args.profile}/grid-{name}.exe')])]:
            data,run=execute(command,request,out/f'regression-{name}-{label}.bin');run.update(compare(data,original))
            report['regressions'][name][label]=run
    report['passed']=bool(all(checks.values()) and all(activity.values()) and all(r['passed'] for r in report['runs'].values())
        and all(r['passed'] for group in report['regressions'].values() for r in group.values())
        and all(r['passed'] for r in upstream_runs.values()) and all(r['passed'] for guard in report['guards'].values() for r in guard.values()))
    (out/'validation.json').write_text(json.dumps(report,indent=2),encoding='utf-8');print(json.dumps(report,indent=2))
    if not report['passed']:raise RuntimeError('Main contact response comparison failed; inspect private evidence')


if __name__=='__main__':main()
