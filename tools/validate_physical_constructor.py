"""Complete owned physical constructor, diagonal supplied tensor/no shapes."""
import argparse,json,random,struct,subprocess
from pathlib import Path
import validate_standard_properties as v
NATIVE=v.ROOT/'local/behavior/physical-constructor-probe.exe'
def f(b,o,x):struct.pack_into('<f',b,o,x)
def u(b,o,x):struct.pack_into('<I',b,o,x)
def fixture(mass,inertia):
    b=bytearray(0x160);f(b,0,mass)
    for r,x in enumerate([*inertia,1.]):f(b,0x10+r*20,x)
    for r in range(4):f(b,0xf0+r*20,1.)
    b[0x60]=b[0x130]=b[0x131]=1
    for o in range(0xa0,0xb4,4):f(b,o,3.4028234663852886e38)
    return b
def run(exe,args):
    e=subprocess.run([str(exe),*map(str,args)],cwd=v.ROOT,capture_output=True,text=True,timeout=30)
    return dict(exit=e.returncode,stdout=e.stdout,stderr=e.stderr)
def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--profile',required=True,choices=['debug','release']);a=p.parse_args()
    out=a.output.resolve();assert out.is_relative_to((v.ROOT/'local').resolve());out.mkdir(exist_ok=False)
    upstream=v.ROOT/'local/behavior/standard-properties-debug-20261011-draft-05/validation.json';report=json.loads(upstream.read_text());assert report['rust_equivalence'] and not report['differences']
    cases=[]
    for i,source in enumerate(report['results'][:44]):
        props=upstream.parent/f'{i:03}-original.bin';assert v.digest(props)==source['output_sha256'] and v.digest(v.GAME/source['source'])==source['source_sha256']
        mass,*inertia=struct.unpack_from('<4f',props.read_bytes(),0x130)
        for selector in [0,1]:
            for active in [0,1]:
                for fill in [0,0xcd]:
                    b=fixture(mass,inertia);b[0x131]=active;cases.append((dict(source=source['source'],source_sha256=source['source_sha256'],properties_sha256=v.digest(props),label='source-mass-inertia'),selector,fill,b))
    rng=random.Random(2008205)
    for i in range(192):
        b=fixture(rng.uniform(100,2500),[rng.uniform(50,5000) for _ in range(3)])
        for o in [0x50,0x54,0x58,0x5c,*range(0x90,0xa0,4),*range(0xb4,0xf0,4),*range(0x140,0x160,4)]:f(b,o,rng.uniform(-2,2))
        if i%4==0:f(b,0xec,3.4028234663852886e38)
        for r in range(4):
            for c in range(4):f(b,0xf0+r*16+c*4,rng.uniform(-2,2))
        for j in range(4):b[0x130+j]=(i>>j)&1
        if i%9==0:
            for o in [0x50,0x54,0x58,*range(0x140,0x160,4)]:f(b,o,-0.)
        cases.append((dict(label='authored-'+str(i)),i%2,[0,0xcd][i%3==0],b))
    results=[];rust=v.ROOT/f'target/{a.profile}/grid-physical-constructor.exe';image=v.ROOT/'local/compatibility/grid-unpacked.exe'
    for i,(metadata,selector,fill,b) in enumerate(cases):
        request=out/f'{i:03}-input.bin';request.write_bytes(b'PC01'+struct.pack('<II',selector,fill)+b)
        answers=[];executions=[]
        for kind in ['original','repeat','rust']:
            target=out/f'{i:03}-{kind}.bin';exe=rust if kind=='rust' else NATIVE;args=[request,target] if kind=='rust' else [image,request,target]
            e=run(exe,args);executions.append(e);(out/'current-executions.json').write_text(json.dumps(dict(index=i,executions=executions),indent=2))
            assert e['exit']==0 and not e['stderr'],(i,e)
            if kind!='rust':assert 'code_unchanged=1 globals_restored=1 error=0' in e['stdout']
            answers.append(target.read_bytes());assert len(answers[-1])==0x690
        differences=[dict(offset=hex(o),original=answers[0][o:o+4].hex(),repeat=answers[1][o:o+4].hex(),rust=answers[2][o:o+4].hex()) for o in range(0,0x690,4) if len({x[o:o+4] for x in answers})!=1]
        (out/'current-differences.json').write_text(json.dumps(dict(index=i,differences=differences),indent=2));assert not differences,(i,differences[:10])
        results.append(dict(index=i,**metadata,selector=selector,active=b[0x131],fill=fill,input_sha256=v.digest(request),output_sha256=v.digest(out/f'{i:03}-original.bin'),executions=executions))
    guards=[];sample=(out/'000-input.bin').read_bytes()
    variants=[('short',sample[:-1]),('long',sample+b'\0')]
    for label,o,value in [('tag',0,0),('selector',4,2),('fill',8,1),('mass-zero',12,0),('mass-nan',12,0x7fc00000),('mass-reciprocal-overflow',12,1),('tensor-zero',12+0x10,0),('tensor-determinant-underflow',12+0x10,1),('tensor-offdiagonal',12+0x14,1),('mass-mode',12+0x60,0),('world-pointer',12+0x64,1),('shape-pointer',12+0x68,1),('shape-count',12+0x134,1),('pose-mode',12+0x130,2),('active-mode',12+0x131,2),('velocity-nan',12+0x140,0x7fc00000),('limit-square-overflow',12+0xec,0xff7fffff)]:
        b=bytearray(sample);struct.pack_into('<I',b,o,value);variants.append((label,bytes(b)))
    for label,b in variants:
        request=out/f'guard-{label}.bin';request.write_bytes(b)
        for kind,exe in [('native',NATIVE),('rust',rust)]:
            target=out/f'guard-{label}-{kind}.bin';e=run(exe,[image,request,target] if kind=='native' else [request,target]);assert e['exit']!=0 and not target.exists(),(label,e)
            if kind=='native':assert not e['stderr'] and 'code_unchanged=1 globals_restored=1' in e['stdout']
            guards.append(dict(label=label,kind=kind,execution=e,no_output=True))
    result=dict(passed=True,profile=a.profile,cases=len(results),observable_bytes=len(results)*0x690,results=results,guards=guards,guard_count=len(guards),native_sha256=v.digest(NATIVE),rust_sha256=v.digest(rust),upstream=str(upstream),upstream_sha256=v.digest(upstream),image_sha256=v.digest(image),limits='Complete B11F40 supplied positive diagonal mass/inertia, zero or CD cold fill, no shapes, owned empty-world registration; source mass/inertia with authored remaining settings. Allocation/car factory/spawn/live behavior excluded.')
    (out/'validation.json').write_text(json.dumps(result,indent=2));print(json.dumps({k:v for k,v in result.items() if k in ['passed','cases','observable_bytes','guard_count']}))
if __name__=='__main__':main()
