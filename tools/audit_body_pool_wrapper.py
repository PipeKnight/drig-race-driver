"""Current-input and complete-file audit of sequential registered pool replay."""
import argparse
import itertools
import json
from pathlib import Path
import struct

from capture_reference_spawn import private
from validate_body_pool_wrapper import CAR, PHYSICAL, RECORD, RESULT, SCENARIOS, sha


def main():
    parser=argparse.ArgumentParser();parser.add_argument('report',type=Path);parser.add_argument('output',type=Path);a=parser.parse_args()
    report_path=private(a.report);output=private(a.output);root=report_path.parent
    report=json.loads(report_path.read_text(encoding='utf-8'))
    assert report['passed'] and report['cases']==1800
    for path,digest in report['pins'].items():assert sha(path)==digest,(path,'current identity changed')
    assert len(report['batches'])==57
    seen=set();cases=contacts=external=changed=compared=files=0
    for b in report['batches']:
        key=(b['name'],b['index']);assert key not in seen;seen.add(key)
        inp=Path(b['request']);assert sha(inp)==b['request_sha256']
        raw=inp.read_bytes();count=struct.unpack_from('<I',raw,4)[0]
        assert raw[:4]==b'BW01' and count==b['cases'] and len(raw)==8+count*RECORD
        expected=inp.with_suffix('.expected');wanted=expected.read_bytes()
        assert sha(expected)==b['expected_sha256'] and wanted[:8]==raw[:8] and len(wanted)==b['bytes']==8+count*RESULT
        for profile in ['debug','release']:
            directory=root/f"{b['name']}-{profile}"
            assert (directory/f"{b['index']:03}.input").read_bytes()==raw
            assert (directory/f"{b['index']:03}.expected").read_bytes()==wanted
            files+=2
        for name in ['original','repeat']:
            r=b['runs'][name];target=Path(r['path'])
            assert target.read_bytes()==wanted and sha(target)==r['sha256'] and r['execution']['exit']==0
            assert 'complete_image_unchanged=1 owned_contexts_checked=1 original_wrapper=1 code_replacements=0 error=0' in r['execution']['stdout'];files+=1
        for index in range(count):
            r=raw[8+index*RECORD:8+(index+1)*RECORD];w=wanted[8+index*RESULT:8+(index+1)*RESULT]
            seed,delta,used,prefix=struct.unpack_from('<4I',r)
            assert delta==struct.unpack('<I',struct.pack('<f',.016))[0]
            assert used<=20 and prefix<=min(4,used)
            assert struct.unpack_from('<2I',w,4)==(0,used-prefix)
            assert w[12+CAR:]==r[16+CAR:]
            cases+=1;contacts+=used;external+=prefix;changed+=struct.unpack_from('<I',w)[0]!=seed
        compared+=len(wanted)
    assert seen==set(itertools.product(SCENARIOS,range(19)))
    assert (cases,contacts,external,changed,compared)==tuple(report[k] for k in ['cases','occupied_contacts','source_external_contacts','event_rng_changed_frames','complete_output_bytes_per_profile'])
    for c in report['partition_controls']:
        prefix=c['prefix'];inp=root/f'partition-{prefix}.input';target=root/f"partition-{prefix}-{c['label']}.bin"
        assert sha(inp)==c['request_sha256'] and sha(target)==c['result_sha256'] and c['execution']['exit']==0
        raw=inp.read_bytes();wanted=target.read_bytes()
        assert len(raw)==8+RECORD and len(wanted)==8+RESULT and raw[:8]==wanted[:8]==b'BW01'+struct.pack('<I',1)
        assert struct.unpack_from('<I',raw,8+12)[0]==prefix
        assert struct.unpack_from('<2I',wanted,8+4)==(0,c['used']-prefix)
    assert report['partition_comparisons']==len(report['partition_controls'])
    guards=report['guards'];assert report['guard_count']==len(guards)==18 and len({g['name'] for g in guards})==18
    for g in guards:
        assert g['preserved'] and g['execution']['exit']!=0
        if g['name']=='existing':assert (root/'guard-existing.bin').read_bytes()==b'KEEP'
        elif g['name'].startswith('capture-'):assert not (root/('guard-'+g['name'])).exists()
        else:assert not (root/f"guard-{g['name']}.bin").exists()
    result=dict(passed=True,cases=cases,complete_source_and_result_files=files,complete_output_bytes_per_profile=compared,
                contacts=contacts,external_prefix_records=external,event_rng_changed_frames=changed,guards=18,
                current_identities=len(report['pins']),report_sha256=sha(report_path),auditor_sha256=sha(Path(__file__)),
                scope='Current source/tool/data identities and all complete source/native files; shared capture generator. '
                'Native ownership assertions are not independently recaptured memory; no whole original solver/scheduler claim.')
    with output.open('x',encoding='utf-8') as file:json.dump(result,file,indent=2)
    print(json.dumps(result,indent=2))


if __name__=='__main__':main()
