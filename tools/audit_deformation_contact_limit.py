"""Read-only reconstruction of complete static-peer cap proofs and guards."""
import argparse
import collections
import json
import math
import struct
from pathlib import Path
import validate_deformation_contact_limit as validation
import validate_deformation_layout as layout


def audit(path):
    path=path.resolve();assert path.is_relative_to((layout.ROOT/'local').resolve())
    report=json.loads(path.read_text());out=path.parent;rows,identities=validation.cases()
    assert identities==report['identities'] and len(rows)==report['cases']
    for name,sha in identities.items():assert layout.digest(Path(name))==sha
    for name,expected in report['files'].items():
        p=(out/name).resolve();assert p.parent==out and p.stat().st_size==expected['size'] and layout.digest(p)==expected['sha256']
    rust=layout.ROOT/f"target/{report['profile']}/grid-deformation-contact-limit.exe"
    total=0;coverage=collections.Counter();seen=0
    for item in report['reports']:
        start=item['start'];batch=rows[start:start+32];assert start==seen and item['cases']==len(batch);seen+=len(batch)
        assert (out/f'{start}-input.bin').read_bytes()==validation.request(batch)
        answer=(out/f'{start}-original.bin').read_bytes()
        assert answer==(out/f'{start}-repeat.bin').read_bytes()==(out/f'{start}-rust.bin').read_bytes()
        assert len(answer)==8+len(batch)*validation.ANSWER and answer[:8]==validation.request(batch)[:8]
        assert item['repeat_exact'] and not item['differences'] and item['bytes']==len(answer)-8
        assert json.loads((out/f'{start}-execution.json').read_text())==item['executions']
        for kind,run in item['executions'].items():
            assert run['exit']==0 and not run['stderr'] and layout.digest(rust if kind=='rust' else validation.PROBE)==run['executable_sha256']
            if kind!='rust':assert 'owned_canaries=1 full_image_unchanged=1 error=0' in run['stdout']
        for i,row in enumerate(batch):
            r=answer[8+i*validation.ANSWER:8+(i+1)*validation.ANSWER]
            allowed={0xa490,0xa494,0xa498,0xa4c4+struct.unpack_from('<I',row['descriptor'],0xa4ec)[0]*4}
            assert all(r[at:at+4]==row['descriptor'][at:at+4] for at in range(0,layout.SIZE,4) if at not in allowed)
            assert r[layout.SIZE:layout.SIZE+validation.LINKED]==row['body']
            c=r[layout.SIZE+validation.LINKED:layout.SIZE+validation.LINKED+128]
            assert all(c[at:at+4]==row['contact'][at:at+4] for at in range(0,128,4) if at!=0x40)
            cap=struct.unpack_from('<d',r,len(r)-8)[0];assert math.isfinite(cap)
            coverage['finite_restricted_caps']+=cap!=row['parameters'][5]
            coverage['window_changed']+=r[0xa4c4:0xa4ec]!=row['descriptor'][0xa4c4:0xa4ec]
            coverage['lookup_state_changed']+=r[0xa490:0xa49c]!=row['descriptor'][0xa490:0xa49c]
            coverage['coefficient_changed']+=c[0x40:0x44]!=row['contact'][0x40:0x44]
        total+=len(answer)-8
    assert seen==len(rows) and total==report['bytes'] and dict(coverage)==report['coverage']
    payload=validation.request([rows[0]])
    guards={'magic':b'BAD!'+payload[4:],'zero_count':payload[:4]+bytes(4),'truncated':payload[:-1],'trailing':payload+b'x'}
    for name,at,value in validation.mutations():
        data=bytearray(payload);struct.pack_into('<I',data,at,value);guards[name]=bytes(data)
    assert json.loads((out/'guards.json').read_text())==report['guards'];outcomes=0
    for guard in report['guards']:
        name=guard['name']
        if 'executions' in guard:
            assert (out/f'guard-{name}-input.bin').read_bytes()==guards.pop(name)
            for kind,run in guard['executions'].items():
                assert run['exit']==(1 if kind=='rust' else guard['expected_original_exit']) and not run['output_exists']
                assert not (out/f'guard-{name}-{kind}.bin').exists();outcomes+=1
        else:
            kind=name.split('-')[1];assert guard['preserved'] and guard['exit']==(24 if kind=='original' else 1)
            assert (out/f'guard-existing-{kind}-input.bin').read_bytes()==payload
            assert (out/f'guard-existing-{kind}.bin').read_bytes()==b'owned sentinel';outcomes+=1
    assert not guards
    return dict(path=str(path.relative_to(layout.ROOT)),sha256=layout.digest(path),cases=len(rows),bytes=total,complete_files=len(report['files']),identities=len(identities),guard_cases=len(report['guards']),guard_outcomes=outcomes,coverage=dict(coverage))


def main():
    p=argparse.ArgumentParser();p.add_argument('reports',type=Path,nargs='+');p.add_argument('--output',type=Path,required=True);args=p.parse_args()
    result=[audit(path) for path in args.reports];target=args.output.resolve();assert target.is_relative_to((layout.ROOT/'local').resolve())
    with target.open('x') as f:json.dump(dict(reports=result),f,indent=2)
    print(json.dumps(result))


if __name__=='__main__':main()
