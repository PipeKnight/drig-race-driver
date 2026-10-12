"""Read-only current-hash/rebuilt-fixture/complete region-loader audit."""
import argparse
import json
from pathlib import Path
import struct
from validate_binxml import decode
from validate_damage_boxes import expected, fixtures, sha


def main():
    ap=argparse.ArgumentParser();ap.add_argument('report',type=Path);ap.add_argument('output',type=Path);a=ap.parse_args()
    assert a.report.resolve().is_relative_to(Path('local').resolve())
    assert a.output.resolve().is_relative_to(Path('local').resolve())
    r=json.loads(a.report.read_text());assert r['passed'] and r['cases']==272 and len(r['guards'])==48
    for path,digest in r['inputs'].items():assert sha(path)==digest,path
    rebuilt=list(fixtures());assert len(rebuilt)==len(r['rows'])
    files=0;total=0;adapted=0
    for item,row in zip(rebuilt,r['rows']):
        label,raw,root,offset,fill,mode=item
        assert label==row['label'] and mode==row['mode'] and list(offset)==row['offset'] and fill==row['fill']
        xml=Path(row['xml']);request=Path(row['request'])
        assert xml.read_bytes()==raw and sha(xml)==row['xml_sha256']
        assert request.read_bytes()==b'DB01'+struct.pack('<4fI',*offset,fill) and sha(request)==row['request_sha256']
        _,parsed,_=decode(raw);answer=expected(parsed,offset,fill)
        assert len(answer)==240
        for name,out in row['outputs'].items():
            path=Path(out['path']);assert sha(path)==out['sha256'] and path.read_bytes()==answer
            if name in ['original','repeat']:
                assert f'numeric_adapters={int(mode=="adapted")} restored=1 error=0' in out['execution']['stdout']
            files+=1
        total+=len(answer);adapted+=mode=='adapted';files+=2
    for label in r['guards']:
        kind,name=label.split(':')
        if name=='existing':assert (a.report.parent/'existing.bin').read_bytes()==b'KEEP'
        else:
            assert (a.report.parent/f'guard-{name}.ned').exists() and (a.report.parent/f'guard-{name}.input').exists()
            assert not (a.report.parent/f'guard-{name}-{kind}.bin').exists()
    assert total==r['bytes_per_run']==65280 and adapted==r['adapted_cases']==256
    result=dict(passed=True,report=str(a.report),report_sha256=sha(a.report),current_hashes=len(r['inputs']),
                primary_files=files,cases=len(rebuilt),complete_bytes_per_run=total,adapted_cases=adapted,
                guards=len(r['guards']),script_sha256=sha(__file__))
    with a.output.open('x') as f:json.dump(result,f,indent=2)
    print(json.dumps(result))


if __name__=='__main__':main()
