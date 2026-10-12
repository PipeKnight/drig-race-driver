"""Whole original suspension context plus source owners and reset sensitivity."""
import argparse,json,random,struct
from pathlib import Path
import validate_handling_configuration as v

def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--profile',required=True,choices=['debug','release']);a=p.parse_args()
    out=a.output.resolve();assert out.is_relative_to((v.ROOT/'local').resolve());out.mkdir(exist_ok=False)
    upstream=v.ROOT/'local/behavior/source-prelude-debug-20261011-final-200679/validation.json';r=json.loads(upstream.read_text());assert r['rust_equivalence'] and r['source_prelude'] and not r['differences']
    cases=[]
    for i,c in enumerate(r['results'][:44]):
        if c.get('skipped'):continue
        owner=upstream.parent/f'{i:03}-original.bin';request=upstream.parent/f'{i:03}-input.bin';assert v.v.digest(owner)==c['output_sha256'] and v.v.digest(request)==c['request_sha256']
        body=bytearray(owner.read_bytes()[:0x2600]);body[0x14e0:0x1520]=request.read_bytes()[8+0x65c:8+0x65c+64]
        cases.append((dict(label='source',car=c['source'],owner=str(owner),owner_sha256=v.v.digest(owner),request=str(request),request_sha256=v.v.digest(request)),body))
    assert len(cases)==43
    base=next(b for c,b in cases if c['car'].endswith('lac.csv'));rng=random.Random(2008209)
    for i in range(96):
        body=bytearray(base)
        for o in [0x103c,0x1040,0x1044,0x1048]:struct.pack_into('<f',body,o,[-1.,-0.,0.,1.,rng.uniform(-1,1)][i%5])
        # Strict front/side sign decisions, both-zero axes, and opaque/history
        # retention are independently controlled; native pointer values owned.
        for j in range(4):
            c=0x3d0+j*0x170
            struct.pack_into('<4f',body,0x14e0+j*16,[0.,-.7,.7][(i+j)%3],.3,[0.,-1.,1.][(i//3+j)%3],1.)
            for o in [0x68,0x6c,0x7c,0x80,0x150]:struct.pack_into('<I',body,c+o,rng.getrandbits(32))
            for o in [0x70,0x74,0x84,0x154]:struct.pack_into('<I',body,c+o,rng.getrandbits(32))
        body[0x9a0]=0x7f
        cases.append((dict(label='authored',control=i),body))
    native=v.ROOT/'local/behavior/suspension-context-probe.exe';rust=v.ROOT/f'target/{a.profile}/grid-suspension-context.exe';results=[]
    for i,(metadata,body) in enumerate(cases):
        source=out/f'{i:03}-input.bin';source.write_bytes(b'CT01'+body);answers=[];executions=[]
        for kind in ['original','repeat','rust']:
            target=out/f'{i:03}-{kind}.bin';e=v.run(rust if kind=='rust' else native,[source,target] if kind=='rust' else [v.IMAGE,source,target]);executions.append(e)
            (out/'current-executions.json').write_text(json.dumps(dict(index=i,executions=executions),indent=2));assert e['exit']==0 and not e['stderr'],(i,e)
            if kind!='rust':assert 'code_unchanged=1 code_replacements=0 globals_restored=1 error=0' in e['stdout']
            answers.append(target.read_bytes());assert len(answers[-1])==0x2600
        diff=[dict(offset=hex(o),original=answers[0][o:o+4].hex(),rust=answers[2][o:o+4].hex()) for o in range(0,0x2600,4) if len({b[o:o+4] for b in answers})!=1]
        (out/'current-differences.json').write_text(json.dumps(dict(index=i,differences=diff),indent=2));assert not diff,(i,diff[:12])
        expected=bytearray(body);expected[0x9a0]=0
        for j in range(4):
            c=0x3d0+j*0x170
            for o in [0x68,0x6c,0x7c,0x80,0x150]:expected[c+o:c+o+4]=bytes(4)
            expected[c+0x50:c+0x60]=answers[0][c+0x50:c+0x60];struct.pack_into('<I',expected,c+0x178,0x1990+j*0x150)
            axis=struct.unpack_from('<4f',answers[0],c+0x50);assert abs(sum(x*x for x in axis)-1)<1e-6 and axis[1]>0 and axis[3]==0
        assert answers[0]==expected
        results.append(dict(index=i,**metadata,input_sha256=v.v.digest(source),output_sha256=v.v.digest(out/f'{i:03}-original.bin'),executions=executions))
    sample=b'CT01'+base;variants=[('short',sample[:-1]),('long',sample+b'\0')]
    for label,o,value in [('tag',0,0),('foreign-self',4+0x9a4,1),('count',4+0x9a8,5),('angle-nan',4+0x103c,0x7fc00000),('angle-infinite',4+0x1040,0x7f800000),('angle-bound',4+0x1044,0x40000000),('position-nan',4+0x14e0,0x7fc00000)]:
        data=bytearray(sample);struct.pack_into('<I',data,o,value);variants.append((label,data))
    guards=[]
    for label,data in variants:
        source=out/f'guard-{label}.bin';source.write_bytes(data)
        for kind,exe in [('native',native),('rust',rust)]:
            target=out/f'guard-{label}-{kind}.bin';e=v.run(exe,[v.IMAGE,source,target] if kind=='native' else [source,target]);assert e['exit']!=0 and not target.exists()
            if kind=='native':assert not e['stderr'] and 'code_unchanged=1' in e['stdout'] and 'globals_restored=1' in e['stdout']
            guards.append(dict(label=label,kind=kind,execution=e,no_output=True))
    result=dict(passed=True,profile=a.profile,cases=len(results),source_cases=43,authored_controls=96,observable_bytes=len(results)*0x2600,results=results,guards=guards,guard_count=len(guards),upstream=str(upstream),upstream_sha256=v.v.digest(upstream),executables={str(q.relative_to(v.ROOT)):v.v.digest(q) for q in [native,rust,v.IMAGE]},limits='Whole 9C10B0 and original tangent/SSE normalization on owned context; source positions explicitly supplied, pointer tokens canonical. Original outer resource factory/spawn excluded.')
    (out/'validation.json').write_text(json.dumps(result,indent=2));print(json.dumps({k:z for k,z in result.items() if k in ['passed','cases','observable_bytes','guard_count']}))
if __name__=='__main__':main()
