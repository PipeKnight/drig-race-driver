"""Audit every provider/legacy/native byte, source request and prior regression."""
import argparse
import json
from pathlib import Path

import validate_geometry_provider as primary
import replay_carried_geometry_regression as regression

ROOT, digest = primary.ROOT, primary.digest


def main():
    p=argparse.ArgumentParser()
    for name in ['primary','regression','output']: p.add_argument(name,type=Path)
    a=p.parse_args(); path=a.primary.resolve(); r=json.loads(path.read_text())
    old=json.loads(primary.REFERENCE.read_text()); cases=primary.source.cases()
    assert r['passed'] and r['cases']==len(cases)==3648 and r['intervals']==38912
    assert r['provider_calls_per_profile']==35264 and not r['fresh_detection']
    assert r['reference_sha256']==digest(primary.REFERENCE)
    assert r['source_script_sha256']==digest(ROOT/'tools/validate_carried_geometry.py')
    assert r['script_sha256']==digest(ROOT/'tools/validate_geometry_provider.py')
    assert r['native_sha256']==digest(primary.source.NATIVE) and r['image_sha256']==digest(primary.source.IMAGE)
    for profile,sha in r['rust_hashes'].items(): assert digest(ROOT/f'target/{profile}/grid-physical-tick.exe')==sha
    primary.source.shaped.MAGIC=primary.source.shaped.g.MAGIC=primary.source.MAGIC
    complete=0; observable=0; decoded={p:[] for p in ['debug','release']}
    for row, previous in zip(r['results'],old['batches'],strict=True):
        start,n=row['start'],row['cases']; assert start==previous['start'] and n==previous['cases']
        inp=primary.REFERENCE.parent/f'{start:04}-input.bin'
        assert digest(inp)==row['input_sha256']==previous['input_sha256'] and inp.read_bytes()==primary.source.pack(cases[start:start+n])
        original=primary.REFERENCE.parent/f'{start:04}-original.bin'; expected=original.read_bytes()
        assert digest(original)==row['output_sha256']==previous['output_sha256']
        repeat=primary.REFERENCE.parent/f'{start:04}-repeat.bin'; assert repeat.read_bytes()==expected
        assert row['bytes']==len(expected); observable+=len(expected)-8; complete+=2
        assert set(row['runs'])=={'native','debug-legacy','debug-provider','release-legacy','release-provider'}
        for mode,run in row['runs'].items():
            target=path.parent/f'{start:04}-{mode}.bin'; e=run['execution']
            assert e['exit']==0 and not e['stderr'] and digest(target)==run['sha256']==row['output_sha256'] and target.read_bytes()==expected
            if mode=='native': assert 'code_unchanged=1 no_imports=1' in e['stdout']
            if mode.endswith('-provider'): decoded[mode.split('-')[0]].extend(primary.source.shaped.decode(target.read_bytes(),cases[start:start+n]))
            complete+=1
    assert observable==r['observable_bytes_per_run']==51932160
    for profile,outputs in decoded.items(): assert primary.source.semantics(cases,outputs)==r['activity'][profile]==old['activity']
    known={g['label']:g for g in old['guards'] if g['kind']=='rust'}
    assert len(r['guards'])==40
    for g in r['guards']:
        label=g['label']; assert label in known and 0<g['execution']['exit']<256
        target=path.parent/f"guard-{label}-{g['profile']}-{g['mode']}.bin"
        if label=='existing': assert digest(target)==g['unchanged_sha256'] and target.read_bytes()==b'preserve existing evidence'
        else:
            assert digest(primary.REFERENCE.parent/f'guard-{label}-input.bin')==known[label]['input_sha256']
            assert g['no_output'] and not target.exists()
    rp=a.regression.resolve(); reg=json.loads(rp.read_text()); assert reg['passed'] and reg['cases_per_run']==6122
    assert reg['image_sha256']==r['image_sha256'] and reg['shared_source_sha256']==digest(ROOT/'tools/probe_physical_tick.c')
    assert reg['script_sha256']==digest(ROOT/'tools/replay_carried_geometry_regression.py')
    regression_files=0
    for row,(kind,directory,input_name,original_name) in zip(reg['results'],regression.SPECS,strict=True):
        ref=ROOT/'local/behavior'/directory; i=row['index']
        assert row['kind']==kind and row['reference_sha256']==digest(ref/'validation.json')
        inp=ref/input_name; original=ref/original_name
        assert digest(inp)==row['input_sha256'] and digest(original)==row['output_sha256']
        for e in row['executions']:
            mode=e['kind']; exe=ROOT/(f'local/behavior/carried-geometry-{kind}-regression.exe' if mode=='native' else f'target/{mode}/grid-physical-tick.exe')
            assert digest(exe)==e['sha256'] and e['execution']['exit']==0 and not e['execution']['stderr']
            target=rp.parent/f'{i:02}-{mode}.bin'
            assert digest(target)==e['output_sha256']==row['output_sha256'] and target.read_bytes()==original.read_bytes(); regression_files+=1
    output=a.output.resolve(); assert output.is_relative_to((ROOT/'local').resolve()) and not output.exists()
    result=dict(passed=True,primary_complete_files=complete,regression_complete_files=regression_files,guard_records=40,
                cases=3648,intervals=38912,provider_calls_per_profile=35264,regression_cases_per_run=6122,
                primary_sha256=digest(path),regression_sha256=digest(rp),script_sha256=digest(Path(__file__)))
    output.write_text(json.dumps(result,indent=2)); print(json.dumps(result),flush=True)


if __name__=='__main__':main()
