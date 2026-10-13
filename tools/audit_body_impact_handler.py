"""Read-only current-source, reconstructed-request and complete-output audit."""
import argparse
import json
from pathlib import Path
import struct

from capture_reference_spawn import private
from validate_body_impact_handler import CAR, PHYSICAL, PREFIX, OUTSIZE, fixtures, sha


def main():
    parser=argparse.ArgumentParser();parser.add_argument('evidence',type=Path);parser.add_argument('output',type=Path);a=parser.parse_args()
    root,output=private(a.evidence),private(a.output)
    report=json.loads((root/'validation.json').read_text(encoding='utf-8'))
    assert report['passed'] and report['cases']==768
    for name,digest in report['pins'].items():assert sha(name)==digest,(name,'changed')
    items=list(fixtures());assert len(items)==768 and len(report['batches'])==24
    compared=reached=changes=draws=packets=0
    for batch in report['batches']:
        index=batch['index'];group=items[index*32:(index+1)*32]
        request=b'BH01'+struct.pack('<I',len(group))+b''.join(r for _,r in group)
        source=root/f'{index:03}.input';assert source.read_bytes()==request and sha(source)==batch['request_sha256']
        original=(root/f'{index:03}-original.bin').read_bytes()
        assert len(original)==batch['bytes']==8+len(group)*OUTSIZE and original[:8]==request[:8]
        for name in ('original','repeat','debug','release'):
            target=root/f'{index:03}-{name}.bin';run=batch['runs'][name]
            assert target.read_bytes()==original and sha(target)==run['sha256']
            assert run['execution']['exit']==0
            if name in ('original','repeat'):assert 'globals_restored=1 owned_contexts_checked=1 code_replacements=0 error=0' in run['execution']['stdout']
        for i,(_,r) in enumerate(group):
            u=lambda b,o:struct.unpack_from('<I',b,o)[0]
            result=original[8+i*OUTSIZE:8+(i+1)*OUTSIZE];v=r[PREFIX:PREFIX+CAR];after=result[628:628+CAR]
            selected=u(r,4) not in (5,7) and u(r,8)!=0x7f7fffff
            reached+=selected;draws+=u(result,4)!=u(r,0);packets+=u(result,8)
            changes+=any(after[0xcdf0+o:0xcdf0+o+4]!=v[0xcdf0+o:0xcdf0+o+4] for o in [0,4,8,0x10,0x14,0x28,0x2c,0x30,0x34])
            assert result[-PHYSICAL:]==r[-PHYSICAL:]
            if not selected:assert after==v and u(result,4)==u(r,0) and u(result,8)==0
        compared+=len(original)
    assert (reached,changes,draws,packets)==tuple(report[k] for k in ('reached','damage_level_changes','random_changes','notification_packets'))
    assert compared==report['compared_bytes_per_profile']==105105600
    assert len(report['guards'])==48
    assert len({(g['name'],g['profile']) for g in report['guards']})==48
    for g in report['guards']:
        assert g['execution']['exit']!=0
        assert not (root/f"guard-{g['name']}-{g['profile']}.bin").exists()
    result=dict(passed=True,cases=768,complete_output_files=96,complete_bytes_per_profile=compared,guards=48,
                source_identities=len(report['pins']),report_sha256=sha(root/'validation.json'),auditor_sha256=sha(Path(__file__)),
                scope='All current pinned identities, independently reconstructed corpus requests and complete original/repeat/debug/release outputs. Native context assertions are not independent recaptured memory.')
    with output.open('x',encoding='utf-8') as f:json.dump(result,f,indent=2)
    print(json.dumps(result,indent=2))


if __name__=='__main__':main()
