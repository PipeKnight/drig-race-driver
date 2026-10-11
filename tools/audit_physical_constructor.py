"""Audit every complete constructor output against current binaries and sources."""
import argparse, ast, json, struct
from pathlib import Path
import validate_physical_constructor as v

def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('reports',nargs=2,type=Path);a=p.parse_args()
    out=a.output.resolve();assert out.is_relative_to((v.v.ROOT/'local').resolve());out.mkdir(exist_ok=False)
    reports=[];conditions=0
    for path in a.reports:
        r=json.loads(path.read_text());root=path.parent
        assert r['passed'] and r['cases']==544 and r['observable_bytes']==913920
        assert r['native_sha256']==v.v.digest(v.NATIVE)
        assert r['rust_sha256']==v.v.digest(v.v.ROOT/f'target/{r["profile"]}/grid-physical-constructor.exe')
        assert r['image_sha256']==v.v.digest(v.v.ROOT/'local/compatibility/grid-unpacked.exe')
        assert r['upstream_sha256']==v.v.digest(Path(r['upstream']))
        assert len(r['results'])==r['cases'] and len(r['guards'])==r['guard_count']==38
        for c in r['results']:
            i=c['index'];request=root/f'{i:03}-input.bin';assert v.v.digest(request)==c['input_sha256']
            if 'source' in c:assert v.v.digest(v.v.GAME/c['source'])==c['source_sha256']
            bodies=[]
            for kind in ['original','repeat','rust']:
                file=root/f'{i:03}-{kind}.bin';assert file.stat().st_size==0x690 and v.v.digest(file)==c['output_sha256'];bodies.append(file.read_bytes())
            assert all(e['exit']==0 and not e['stderr'] for e in c['executions'])
            b=bodies[0];word=lambda o:struct.unpack_from('<I',b,o)[0]
            assert all(word(o)==1 for o in [8,12,24])
            assert word(0x32c)==0x10000000 and word(0x430+0x64)==0
            assert word(0x400)==(c['selector'] if c['active'] else 0xffffffff)
            assert word(0x590+0x24)==1 and word(0x590+0x60)==c['selector']
            for selector in [0,1]:
                expected=int(bool(c['active']) and selector==c['selector'])
                assert word(0x590+0x3c+selector*4)==word(0x590+0x44+selector*4)==expected
            assert all(word(0x430+0xfc+r*16)==0 for r in range(3)) and word(0x430+0x12c)==0x3f800000
            conditions+=10
        for g in r['guards']:
            assert g['no_output'] and g['execution']['exit']!=0
            assert not (root/f'guard-{g["label"]}-{g["kind"]}.bin').exists()
            conditions+=1
        reports.append(dict(path=str(path),sha256=v.v.digest(path),profile=r['profile'],cases=r['cases'],guards=r['guard_count']))
    assert {r['profile'] for r in reports}=={'debug','release'}
    for name in ['validate_physical_constructor.py','audit_physical_constructor.py']:ast.parse((v.v.ROOT/'tools'/name).read_text())
    result=dict(passed=True,conditions=conditions,reports=reports,limits='Whole selected body constructor with explicit owned pointers and empty-world lists. Shape/resource/allocation/car factory/live spawn excluded.')
    (out/'audit.json').write_text(json.dumps(result,indent=2));print(json.dumps(result))
if __name__=='__main__':main()
