"""Read-only full output and fixture reconstruction audit for VH02."""
import argparse
import json
from pathlib import Path
import struct
from validate_handling_damage import fixtures,check,SIZE,sha


def main():
    ap=argparse.ArgumentParser();ap.add_argument('report',type=Path);ap.add_argument('output',type=Path);a=ap.parse_args()
    assert a.report.resolve().is_relative_to(Path('local').resolve()) and a.output.resolve().is_relative_to(Path('local').resolve())
    r=json.loads(a.report.read_text());assert r['passed'] and r['cases']==640 and not r['runtime_enabled']
    for path,digest in r['inputs'].items():assert sha(path)==digest,path
    items=list(fixtures());files=0;total=0;stats=dict(reached_calls=0,global_changes=0,wear_changes=0,damage_changes=0)
    for index,row in enumerate(r['rows']):
        group=items[index*32:(index+1)*32];request=b'VH02'+struct.pack('<I',len(group))+b''.join(v for _,v in group)
        assert [label for label,_ in group]==row['labels']
        assert Path(row['request']).read_bytes()==request and sha(row['request'])==row['request_sha256'];files+=1
        outputs={}
        for name,item in row['outputs'].items():
            assert sha(item['path'])==item['sha256'];outputs[name]=Path(item['path']).read_bytes();files+=1
        original=outputs['original'];assert all(v==original for v in outputs.values())
        assert len(original)==row['bytes']==8+len(group)*SIZE
        for k,v in check(request,original).items():stats[k]+=v
        total+=len(original)
    for entry in r['guards']:
        kind,name=entry.split(':')
        if name=='existing':assert (a.report.parent/'existing.bin').read_bytes()==b'KEEP'
        else:
            assert (a.report.parent/f'guard-{name}.input').exists()
            assert not (a.report.parent/f'guard-{name}-{kind}.bin').exists()
    assert len(r['guards'])==39 and total==r['bytes_per_run']==87524000
    assert all(r[k]==v and v>0 for k,v in stats.items())
    result=dict(passed=True,report=str(a.report),report_sha256=sha(a.report),primary_files=files,current_hashes=len(r['inputs']),cases=len(items),bytes_per_run=total,guards=len(r['guards']),**stats,script_sha256=sha(__file__))
    with a.output.open('x') as file:json.dump(result,file,indent=2)
    print(json.dumps(result))


if __name__=='__main__':main()
