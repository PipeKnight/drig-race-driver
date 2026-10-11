"""Current complete source factory, upstream owner and all-output audit."""
import argparse,ast,json,struct
from pathlib import Path
import validate_source_physical as v

def main():
    p=argparse.ArgumentParser();p.add_argument('output',type=Path);p.add_argument('reports',nargs=2,type=Path);a=p.parse_args()
    out=a.output.resolve();assert out.is_relative_to((v.v.ROOT/'local').resolve());out.mkdir(exist_ok=False)
    reports=[];checked=0
    for path in a.reports:
        r=json.loads(path.read_text());root=path.parent;profile=r['profile']
        assert r['passed'] and len(r['results'])==r['cases']==408 and r['observable_bytes']==92545824
        assert r['native_sha256']==v.v.digest(v.NATIVE) and r['rust_sha256']==v.v.digest(v.v.ROOT/f'target/{profile}/grid-source-physical.exe')
        assert r['image_sha256']==v.v.digest(v.v.ROOT/'local/compatibility/grid-unpacked.exe')
        assert r['guard_count']==len(r['guards'])==28
        for u in r['upstream']:assert v.v.digest(Path(u['path']))==u['sha256']
        hr=json.loads(Path(r['upstream'][1]['path']).read_text());assert v.v.digest(v.v.GAME/'raceload.jpk')==hr['archive_sha256']
        for c in r['results']:
            i=c['index'];request=root/f'{i:03}-input.bin';data=request.read_bytes();assert len(data)==12+0x65c+v.CAR and v.v.digest(request)==c['input_sha256']
            assert v.v.digest(v.v.GAME/c['car'])==c['source_sha256']
            if c['label']=='source-configured':assert v.v.digest(Path(c['upstream_owner']))==c['owner_sha256']
            for kind in ['original','repeat','rust']:
                file=root/f'{i:03}-{kind}.bin';assert file.stat().st_size==v.OUTPUT and v.v.digest(file)==c['output_sha256']
            assert all(e['exit']==0 and not e['stderr'] for e in c['executions'])
            b=(root/f'{i:03}-original.bin').read_bytes();u=lambda o:struct.unpack_from('<I',b,o)[0]
            assert u(0xdb0)==1 and u(0xdd8)==0 and b[0x37090]==0
            body=v.CAR;wrapper=body+0x430;world=wrapper+16;pool=world+0x100;cell=pool+0x1c
            assert u(body+0x424)==0x20000000 and u(body+0x32c)==0x10000000
            assert [u(wrapper+i*4) for i in range(4)]==[0xd314e4,0,1,0xcdcdcdcd if c['fill'] else 0]
            assert [u(pool+i*4) for i in range(7)]==[0,0,0,1,1,16,0]
            assert [u(cell+i*4) for i in range(4)]==[0x30000000,0xcdcdcdcd,0xcdcdcdcd,0xcdcdcdcd]
            assert u(body+0x400)==u(world+0x60)==c['selector'] and u(world+0x24)==1
            assert u(world+0x3c+4*c['selector'])==u(world+0x44+4*c['selector'])==1
            assert u(body+0x3a4)==u(body+0x3a8)==u(body+0x3f8)==0
            assert all(u(body+o)==0x7f7fffff for o in range(0x3b4,0x3c8,4)) and u(body+0x3fc)==0x7f7fffff
            checked+=1
        for g in r['guards']:
            assert g['no_output'] and g['execution']['exit']!=0 and not (root/f'guard-{g["label"]}-{g["kind"]}.bin').exists()
        reports.append(dict(profile=profile,path=str(path),sha256=v.v.digest(path),cases=r['cases'],guards=r['guard_count']))
    assert {r['profile'] for r in reports}=={'debug','release'}
    for name in ['validate_source_physical.py','audit_source_physical.py']:ast.parse((v.v.ROOT/'tools'/name).read_text())
    result=dict(passed=True,checked_complete_outputs=checked,reports=reports,limits='Source body factory with owned cold pool/world and explicit host lock imports. Shape/resource factory and runtime attachment remain separate.')
    (out/'audit.json').write_text(json.dumps(result,indent=2));print(json.dumps(result))
if __name__=='__main__':main()
