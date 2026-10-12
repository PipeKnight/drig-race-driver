"""Read-only complete impact state/RNG, request rebuild and current-hash audit."""
import argparse
import json
from pathlib import Path
import struct
from validate_collision_damage import CAR,PHYSICAL,SIZE,OUTSIZE,M,fixtures,sha,u,projection


def main():
    ap=argparse.ArgumentParser();ap.add_argument('report',type=Path);ap.add_argument('output',type=Path);a=ap.parse_args()
    assert a.report.resolve().is_relative_to(Path('local').resolve()) and a.output.resolve().is_relative_to(Path('local').resolve())
    r=json.loads(a.report.read_text());assert r['passed'] and r['cases']==768 and not r['runtime_integration']
    for path,digest in r['inputs'].items():assert sha(path)==digest,path
    fixtures_rebuilt=list(fixtures());files=0;total=0;applied=0;drawn=0;changed=0
    for index,row in enumerate(r['rows']):
        group=fixtures_rebuilt[index*32:(index+1)*32]
        request=b'DI01'+struct.pack('<I',len(group))+b''.join(v for _,v in group)
        assert [label for label,_ in group]==row['labels']
        assert Path(row['request']).read_bytes()==request and sha(row['request'])==row['request_sha256']
        outputs={}
        for name,item in row['outputs'].items():
            path=Path(item['path']);assert sha(path)==item['sha256'];outputs[name]=path.read_bytes();files+=1
        original=outputs['original'];assert all(v==original for v in outputs.values())
        assert len(original)==row['bytes']==8+len(group)*OUTSIZE and original[:8]==request[:8]
        for j,(label,inp) in enumerate(group):
            out=original[8+j*OUTSIZE:8+(j+1)*OUTSIZE]
            active,state,levels=projection(inp)
            assert u(out,0)==active and u(out,4)==state
            applied+=active;drawn+=state!=u(inp,0)
            assert all(out[8+M+at:8+M+at+4]==word for at,word in levels.items())
            difference=any(out[8+M+at:8+M+at+4]!=inp[32+M+at:32+M+at+4] for at in levels)
            changed+=difference
            assert out[8+CAR:]==inp[32+CAR:] and out[8:8+M]==inp[32:32+M]
            assert out[8+M+0x460:8+CAR]==inp[32+M+0x460:32+CAR]
            if label=='source':assert not difference
            if not active:assert out[8:]==inp[32:] and state==u(inp,0)
        total+=len(original);files+=1
    for label in r['guards']:
        kind,name=label.split(':')
        if name=='existing':assert (a.report.parent/'existing.bin').read_bytes()==b'KEEP'
        else:
            assert (a.report.parent/f'guard-{name}.input').exists()
            assert not (a.report.parent/f'guard-{name}-{kind}.bin').exists()
    assert len(r['guards'])==60 and total==r['bytes_per_run']==104629440
    assert applied==r['applied']==439 and drawn==r['random_changes']==296 and changed==r['level_changes']==233
    result=dict(passed=True,report=str(a.report),report_sha256=sha(a.report),primary_files=files,current_hashes=len(r['inputs']),
                cases=len(fixtures_rebuilt),bytes_per_run=total,applied=applied,random_changes=drawn,level_changes=changed,
                guards=len(r['guards']),script_sha256=sha(__file__))
    with a.output.open('x') as f:json.dump(result,f,indent=2)
    print(json.dumps(result))


if __name__=='__main__':main()
