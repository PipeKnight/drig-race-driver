"""Whole source factory on verified source-configured owners and owned cold controls."""
import argparse,json,random,struct
from pathlib import Path
import validate_physical_constructor as pc
v=pc.v;CAR=0x370a0;OUTPUT=226828
NATIVE=v.ROOT/'local/behavior/source-physical-probe.exe'

def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--profile',required=True,choices=['debug','release']);a=p.parse_args()
    out=a.output.resolve();assert out.is_relative_to((v.ROOT/'local').resolve());out.mkdir(exist_ok=False)
    properties=v.ROOT/'local/behavior/standard-properties-debug-20261011-draft-05/validation.json'
    handling=v.ROOT/'local/behavior/handling-configuration-debug-20261011-final-01/validation.json'
    pr=json.loads(properties.read_text());hr=json.loads(handling.read_text());assert pr['rust_equivalence'] and not pr['differences'] and hr['passed']
    props={}
    for i,c in enumerate(pr['results'][:44]):
        file=properties.parent/f'{i:03}-original.bin';assert v.digest(file)==c['output_sha256'] and v.digest(v.GAME/c['source'])==c['source_sha256']
        props[c['source']]=(file.read_bytes(),c)
    cases=[]
    for c in hr['joined']:
        if c['seed']!=200679 or c['active']!=1 or c['hdm_index']>1:continue
        file=handling.parent/f'joined-{c["index"]:04}-original.bin';assert v.digest(file)==c['output_sha256'] and v.digest(Path(c['ned']))==c['ned_sha256']
        prop,source=props[c['car']];car=file.read_bytes()+bytes(CAR-0x21000)
        for selector in [0,1]:
            for fill in [0,0xcd]:
                cases.append((dict(label='source-configured',car=c['car'],source_sha256=c['source_sha256'],hdm=c['hdm'],hdm_sha256=c['hdm_sha256'],upstream_owner=str(file),owner_sha256=v.digest(file),properties_sha256=source['output_sha256']),selector,fill,prop,car))
    assert len(cases)==344
    rng=random.Random(2008206)
    for i in range(64):
        source=list(props.values())[i%44];prop,c=source;prop=bytearray(prop);car=bytearray([0xcd if i%2 else 0])*CAR
        struct.pack_into('<I',car,0xdb0,0);struct.pack_into('<I',car,0xdd8,0)
        if i>=44:struct.pack_into('<4f',prop,0x130,rng.uniform(100,2500),*(rng.uniform(50,5000) for _ in range(3)))
        cases.append((dict(label='cold-source-mass' if i<44 else 'authored-cold',car=c['source'],source_sha256=c['source_sha256']),i%2,[0,0xcd][i%3==0],bytes(prop),bytes(car)))
    rust=v.ROOT/f'target/{a.profile}/grid-source-physical.exe';image=v.ROOT/'local/compatibility/grid-unpacked.exe';results=[]
    for i,(metadata,selector,fill,prop,car) in enumerate(cases):
        request=out/f'{i:03}-input.bin';request.write_bytes(b'PF01'+struct.pack('<II',selector,fill)+prop+car)
        answers=[];executions=[]
        for kind in ['original','repeat','rust']:
            target=out/f'{i:03}-{kind}.bin';e=pc.run(rust if kind=='rust' else NATIVE,[request,target] if kind=='rust' else [image,request,target]);executions.append(e)
            (out/'current-executions.json').write_text(json.dumps(dict(index=i,executions=executions),indent=2));assert e['exit']==0 and not e['stderr'],(i,e)
            if kind!='rust':assert 'host_lock_imports=2 code_unchanged=1 globals_restored=1 error=0' in e['stdout']
            answer=target.read_bytes();assert len(answer)==OUTPUT;answers.append(answer)
        differences=[dict(offset=hex(o),original=answers[0][o:o+4].hex(),repeat=answers[1][o:o+4].hex(),rust=answers[2][o:o+4].hex()) for o in range(0,OUTPUT,4) if len({x[o:o+4] for x in answers})!=1]
        (out/'current-differences.json').write_text(json.dumps(dict(index=i,differences=differences),indent=2));assert not differences,(i,differences[:10])
        # Only the actual factory-owned car slots may change; all other complete
        # source constructor/settings/history/resource identity bytes survive.
        expected=bytearray(car)
        for o in range(0xdb0,0xdcc,4):struct.pack_into('<I',expected,o,0)
        struct.pack_into('<I',expected,0xdb0,1);expected[0x37090]=0
        assert answers[0][:CAR]==expected
        results.append(dict(index=i,**metadata,selector=selector,fill=fill,input_sha256=v.digest(request),output_sha256=v.digest(out/f'{i:03}-original.bin'),executions=executions))
    sample=(out/'000-input.bin').read_bytes();variants=[('short',sample[:-1]),('long',sample+b'\0')];guards=[]
    for label,o,value in [('tag',0,0),('selector',4,2),('fill',8,1),('mass-zero',12+0x130,0),('mass-nan',12+0x130,0x7fc00000),('mass-overflow-reciprocal',12+0x130,1),('mass-bound',12+0x130,0x49742410),('inertia-zero',12+0x134,0),('inertia-underflow',12+0x134,1),('inertia-nan',12+0x13c,0x7fc00000),('existing-wrapper',12+0x65c+0xdb0,1),('external-manager',12+0x65c+0xdd8,1)]:
        bad=bytearray(sample);struct.pack_into('<I',bad,o,value);variants.append((label,bytes(bad)))
    for label,bad in variants:
        source=out/f'guard-{label}.bin';source.write_bytes(bad)
        for kind,exe in [('native',NATIVE),('rust',rust)]:
            target=out/f'guard-{label}-{kind}.bin';e=pc.run(exe,[image,source,target] if kind=='native' else [source,target]);assert e['exit']!=0 and not target.exists(),(label,e)
            if kind=='native':assert not e['stderr'] and 'code_unchanged=1 globals_restored=1' in e['stdout']
            guards.append(dict(label=label,kind=kind,execution=e,no_output=True))
    result=dict(passed=True,profile=a.profile,cases=len(results),observable_bytes=len(results)*OUTPUT,results=results,guards=guards,guard_count=len(guards),native_sha256=v.digest(NATIVE),rust_sha256=v.digest(rust),image_sha256=v.digest(image),upstream=[dict(path=str(q),sha256=v.digest(q)) for q in [properties,handling]],limits='Whole 9B8D00/pool/B11F40/wrapper on source-configured or explicit cold owners; two actual host critical-section imports only. Lock object bytes and temporary manager addresses external. Shape/resource/spawn/runtime attachment excluded.')
    (out/'validation.json').write_text(json.dumps(result,indent=2));print(json.dumps({k:v for k,v in result.items() if k in ['passed','cases','observable_bytes','guard_count']}))
if __name__=='__main__':main()
