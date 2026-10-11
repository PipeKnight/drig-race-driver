"""Source-configured car and source body through whole mutable runtime handling."""
import argparse,json,struct
from pathlib import Path
import validate_vehicle_handling as v

def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('--profile',required=True,choices=['debug','release']);a=p.parse_args()
    out=a.output.resolve();assert out.is_relative_to((v.ROOT/'local').resolve());out.mkdir(exist_ok=False)
    config=v.ROOT/'local/behavior/handling-configuration-debug-20261011-final-01/validation.json'
    factory=v.ROOT/'local/behavior/source-physical-debug-20261011-final-01/validation.json'
    cr=json.loads(config.read_text());fr=json.loads(factory.read_text());assert cr['passed'] and fr['passed']
    bodies={}
    for c in fr['results']:
        if c['label']!='source-configured' or c['selector'] or c['fill']:continue
        file=factory.parent/f'{c["index"]:03}-original.bin';assert v.digest(file)==c['output_sha256']
        bodies[c['car'],c['hdm']]=(file.read_bytes()[0x370a0:0x370a0+v.PHYSICAL],file,c['output_sha256'])
    rows=[];metadata=[]
    for c in cr['joined']:
        if c['hdm_index']>1:continue
        file=config.parent/f'joined-{c["index"]:04}-original.bin';assert v.digest(file)==c['output_sha256']
        body,physical_file,physical_hash=bodies[c['car'],c['hdm']]
        car=bytearray(file.read_bytes());car[0xb13]=car[0xb14]=1
        delta=[0,.001,.016,.02,.04,.1][len(rows)%6]
        rows.append(struct.pack('<f',delta)+car+body)
        metadata.append(dict(car=c['car'],hdm=c['hdm'],seed=c['seed'],views_active=c['active'],delta=delta,owner=str(file),owner_sha256=c['output_sha256'],physical_factory=str(physical_file),physical_factory_sha256=physical_hash))
    assert len(rows)==516
    lac=next(i for i,c in enumerate(metadata) if c['car'].endswith('lac.csv') and c['hdm']=='standard.hdm')
    controls=[]
    for i in range(64):
        row=bytearray(rows[lac]);v.put(row,0,[.001,.016,.02,.1][i%4])
        divisor=[10016380.,100_000_000.,3.4028234663852886e38,10.][(i//4)%4]
        v.put(row,4+v.M+0x2f0,999999. if i<32 else 100_000_000.)
        v.put(row,4+v.M+0x2f4,divisor);v.word(row,4+0x1644,[0,1,0xffffffff,200679][i%4])
        for j in range(4):v.put(row,4+0x1aa8+j*0x150,1_000_000.)
        for o in [0x430,0x434,0x438]:v.put(row,4+v.M+o,1.)
        # Shipped brake damage scales are zero. Positive wear is a separate
        # authored sensitivity control; never infer it from source admission.
        for j in range(6):
            for k,x in enumerate([.25,.2,.1]):v.put(row,4+v.M+0x28c+j*12+k*4,x)
        v.word(row,4+0x2118,i%2);v.word(row,4+0x211c,(i//2)%3)
        controls.append(bytes(row))
    all_rows=rows+controls;reports=[];answers=[]
    for start in range(0,len(all_rows),32):
        batch=all_rows[start:start+32];r,result=v.compare(out,a.profile,{k:batch for k in ['original','repeat','rust']},f'initial-{start}')
        reports.append(r);answers.extend(result['original'])
    # Each branch carries its own whole car/body through eight subsequent calls.
    branches={k:[answers[lac],*answers[516:527]] for k in ['original','repeat','rust']}
    for i in range(8):
        r,result=v.compare(out,a.profile,branches,f'carry-{i}');reports.append(r);branches=result
    accepted=[]
    for i,(before,after) in enumerate(zip(controls,answers[516:],strict=True)):
        active=i<32
        expected_rng=struct.unpack_from('<I',before,4+0x1644)[0]
        if active:
            for _ in range(4):expected_rng=(expected_rng*0x41c64e6d+0x40584c0f)&0xffffffff
        assert struct.unpack_from('<I',after,4+0x1644)[0]==expected_rng
        levels=struct.unpack_from('<4f',after,4+v.M+0x38)
        assert (any(x>0 for x in levels) if active else all(x==0 for x in levels))
        accepted.append(dict(index=i,active=active,force_divisor=struct.unpack_from('<f',before,4+v.M+0x2f4)[0],levels=levels,random_state=expected_rng))
    guards=[]
    for label,o,value in [('nan-threshold',v.M+0x2f0,0x7fc00000),('nan-divisor',v.M+0x2f4,0x7fc00000),('infinite-divisor',v.M+0x2f4,0x7f800000),('other-parameter-bound',v.M+0x318,0x49742410),('active-zero-divisor',v.M+0x2f4,0),('active-negative-divisor',v.M+0x2f4,0xbf800000)]:
        row=bytearray(controls[0]);v.word(row,4+o,value)
        for kind,exe in [('native',v.NATIVE),('rust',v.ROOT/f'target/{a.profile}/{v.BIN}.exe')]:
            source=out/f'guard-{label}-{kind}-input.bin';target=out/f'guard-{label}-{kind}-output.bin';source.write_bytes(v.request([row]))
            e=v.execution.execute(exe,[v.REFERENCE,source,target] if kind=='native' else [source,target],out,f'guard-{label}-{kind}',True)
            assert e['exit']!=0 and not target.exists();guards.append(dict(label=label,kind=kind,execution=e,no_output=True))
    result=dict(passed=True,profile=a.profile,source_cases=516,controls=64,carried_calls=96,calls=sum(r['calls'] for r in reports),record_bytes=sum(r['record_bytes'] for r in reports),reports=reports,source_owners=metadata,force_controls=accepted,guards=guards,guard_count=len(guards),upstream=[dict(path=str(q),sha256=v.digest(q)) for q in [config,factory]],executables={str(q.relative_to(v.ROOT)):v.digest(q) for q in [v.NATIVE,v.ROOT/f'target/{a.profile}/{v.BIN}.exe']},limits='Whole 9F3E80 runtime on source-configured cars/source body. Source outer B13/B14 active flags supplied explicitly; authored wear/style multipliers for force controls. No driver/world/spawn/live driving claim.')
    (out/'validation.json').write_text(json.dumps(result,indent=2));print(json.dumps({k:z for k,z in result.items() if k in ['passed','source_cases','controls','carried_calls','calls','record_bytes','guard_count']}))
if __name__=='__main__':main()
