"""Current complete cold-constructor proof, provenance and rejection checks."""
import argparse,csv,io,json,struct,subprocess
from pathlib import Path
import validate_standard_properties as v

def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path)
    p.add_argument('--prefix',default='standard-constructor')
    p.add_argument('--source-prelude',action='store_true');a=p.parse_args()
    out=a.output.resolve();assert out.is_relative_to((v.ROOT/'local').resolve());out.mkdir(exist_ok=False)
    reports=[];seeds=['200679','0','1','0x80000000','0xffffffff']
    native=v.ROOT/'local/behavior/standard-constructor-probe.exe';image=v.ROOT/'local/compatibility/grid-unpacked.exe'
    assert v.digest(image)=='aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514'
    for profile in ['debug','release']:
        rust=v.ROOT/f'target/{profile}/grid-standard-car.exe'
        for seed in seeds:
            root=v.ROOT/f'local/behavior/{a.prefix}-{profile}-20261011-final-{seed}';path=root/'validation.json'
            r=json.loads(path.read_text());assert r['native_reference_passed'] and r['rust_equivalence'] and not r['differences']
            assert r['profile']==profile and r['random_seed']==int(seed,0)
            assert r.get('source_prelude',False)==a.source_prelude
            assert r['exe_sha256']==v.digest(native) and r['rust_exe_sha256']==v.digest(rust)
            assert r['properties_report_sha256']==v.digest(Path(r['properties_report']))
            completed=[s for s in r['results'] if s.get('repeat_exact')];assert len(completed)==69
            assert sum(s['label']=='installed' for s in completed)==43
            assert sum(bool(s.get('skipped')) for s in r['results'])==1
            for i,source in enumerate(r['results']):
                if source.get('skipped'):continue
                assert source['source_sha256']==v.digest(v.GAME/source['source'])
                assert source['ned_sha256']==v.digest(Path(source['ned']))
                assert source['properties_sha256']==v.digest(Path(source['properties']))
                assert source['request_sha256']==v.digest(root/f'{i:03}-input.bin')
                answers=[root/f'{i:03}-{kind}.bin' for kind in ['original','repeat','rust']]
                assert all(q.stat().st_size==0x21000 and v.digest(q)==source['output_sha256'] for q in answers)
                assert all(e['exit']==0 and not e['stderr'] for e in source['executions'])
                assert all('excluded_stack_padding_bytes=2 code_unchanged=1 code_replacements=0 globals_restored=1 error=0' in e['stdout'] for e in source['executions'][:2])
                for q in answers:
                    b=q.read_bytes();assert struct.unpack_from('<I',b,0xd19c)[0]==0xffc00000
                    assert b[0x202e:0x2030]==b'\0\0'
                    assert struct.unpack_from('<I',b,0x18f4)[0]==0
                    assert struct.unpack_from('<I',b,0x196c)[0]==1
                    if a.source_prelude:
                        assert b[0xe78:0x14d4]==Path(source['properties']).read_bytes()
                        assert struct.unpack_from('<f',b,0x1560)[0]==1
                        assert struct.unpack_from('<I',b,0x9a8)[0]==4
                        assert struct.unpack_from('<I',b,0x9a4)[0]==0
                        assert all(b[o]==1 for o in [0xb13,0xb14,0x1ee0,0x21c8])
            reports.append(dict(path=str(path),sha256=v.digest(path),profile=profile,seed=int(seed,0),cases=69,observable_bytes=69*(0x21000-2)))
    guards=[]
    sample=(v.ROOT/f'local/behavior/{a.prefix}-debug-20261011-final-200679/000-input.bin').read_bytes()
    def reject(label,command,target,native_guard=False):
        e=subprocess.run(command,cwd=v.ROOT,capture_output=True,text=True,timeout=30)
        assert e.returncode!=0 and not target.exists(),(label,e)
        if native_guard:assert not e.stderr and 'code_unchanged=1 code_replacements=0 globals_restored=1' in e.stdout
        else:assert 'grid-standard-car:' in e.stderr
        guards.append(dict(label=label,exit=e.returncode,stdout=e.stdout,stderr=e.stderr,no_output=True))
    variants=[('short',sample[:-1]),('long',sample+b'\0')]
    for label,offset,value in [('tag',0,0),('model',8+0x140,2),('drive',8+0x304,4),('engine',8+0x1d4,0),('mass-zero',8+0x130,0),('rpm-limit',8+0x1e8,struct.unpack('<I',struct.pack('<f',20001))[0]),('point-nan',8+0x65c,0x7fc00000),('radius-zero',8+0x65c+64,0),('width-nan',8+0x65c+80,0x7fc00000),('curve-nan',8+0x1f4,0x7fc00000),('gear-nan',8+0x31c,0x7fc00000)]:
        data=bytearray(sample);struct.pack_into('<I',data,offset,value);variants.append((label,bytes(data)))
    for label,data in variants:
        source=out/f'native-{label}.bin';source.write_bytes(data);target=out/f'native-{label}-output.bin'
        reject('native-'+label,[str(native),str(image),str(source),str(target)],target,True)
    base=next(r for r in csv.reader((v.GAME/'cars/lac/lac.csv').read_text().splitlines()) if r)[:360]
    sources=[]
    for label,column,value in [('large-mass',3,'1000001'),('engine-off',14,'0'),('unknown-drive',67,'Unknown'),('rpm-limit',16,'20001'),('bad-spring',96,'0'),('nonfinite',4,'NaN')]:
        row=base.copy();row[column]=value;buf=io.StringIO(newline='');csv.writer(buf,lineterminator='\r\n').writerow(row)
        source=out/f'{label}.csv';source.write_bytes(buf.getvalue().encode('ascii'));sources.append((label,source))
    ned=v.GAME/'cars/lac/lac.ned'
    for profile in ['debug','release']:
        rust=v.ROOT/f'target/{profile}/grid-standard-car.exe'
        for label,source in sources:
            target=out/f'{profile}-{label}-output.bin'
            command=[str(rust),str(source),str(ned),'00030fe7',str(target)]
            if a.source_prelude:command.append('--source-prelude')
            reject(profile+'-'+label,command,target)
        target=out/f'{profile}-bad-seed-output.bin'
        reject(profile+'-bad-seed',[str(rust),str(v.GAME/'cars/lac/lac.csv'),str(ned),'nothex',str(target)],target)
    result=dict(passed=True,reports=reports,guards=guards,guard_count=len(guards),source_pairs=43,authored_variations=26,seeds=5,cases_per_profile=345,observable_bytes_per_profile=345*(0x21000-2),limits='Native framing and Rust source guards are distinct; two native padding bytes and explicit address canonicalization are excluded. Cold control NaN is retained; no outer factory/physical owner/driving claim.')
    result['source_prelude']=a.source_prelude
    (out/'audit.json').write_text(json.dumps(result,indent=2));print(json.dumps({k:v for k,v in result.items() if k not in ['reports','guards']}))
if __name__=='__main__':main()
