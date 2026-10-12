"""Whole original constructor on private decoded properties and owned geometry.

Reference proof only until an independent Rust constructor is compared. Source
selection/caller preparation, outer factory, physical owner and driving excluded.
"""
import argparse
import csv
import io
import json
import struct
import subprocess
from pathlib import Path
import validate_standard_properties as v
from validate_binxml import decode


def main():
    p = argparse.ArgumentParser()
    p.add_argument('output', type=Path)
    p.add_argument('properties_report', type=Path)
    p.add_argument('--all', action='store_true')
    p.add_argument('--profile', choices=['debug', 'release'])
    p.add_argument('--seed',type=lambda s:int(s,0),default=200679)
    p.add_argument('--controls',action='store_true')
    p.add_argument('--source-prelude',action='store_true')
    p.add_argument('--mutable-prelude',action='store_true')
    p.add_argument('--outer-wheels',action='store_true')
    a = p.parse_args()
    if a.outer_wheels:a.mutable_prelude=True
    if a.mutable_prelude:a.source_prelude=True
    out = a.output.resolve()
    assert out.is_relative_to((v.ROOT / 'local').resolve())
    out.mkdir(exist_ok=False)
    r = json.loads(a.properties_report.read_text())
    assert r['native_reference_passed'] and r['rust_equivalence'] and not r['differences']
    exe = v.ROOT / ('local/behavior/outer-wheels-standard-probe.exe' if a.outer_wheels else 'local/behavior/mutable-standard-probe.exe' if a.mutable_prelude else 'local/behavior/standard-constructor-probe.exe')
    image = v.ROOT / 'local/compatibility/grid-unpacked.exe'
    assert v.digest(image) == 'aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514'
    f32 = lambda f: struct.unpack('<f', struct.pack('<f', f))[0]
    results = []
    differences = []
    fixtures=list(enumerate(r['results'][:44]))
    if a.controls:
        base=next(row for row in csv.reader((v.GAME/'cars/lac/lac.csv').read_text().splitlines()) if row)[:360]
        variations=[(3,'100'),(3,'2500'),(4,'100'),(5,'1000'),(6,'100'),(7,'0.1'),(7,'0.4'),(8,'0.25'),(8,'0.75'),(16,'0'),(16,'20000'),(67,'Front'),(67,'Rear'),(67,'Four'),(78,'Full'),(78,'Half'),(78,'Simple'),(78,'Custom'),(95,'10000'),(96,'20000'),(173,'0.5'),(173,'1.5'),(194,'-0.1'),(194,'0.1'),(77,'75'),(139,'1')]
        for j,(column,value) in enumerate(variations):
            row=base.copy();row[column]=value
            source=out/f'control-{j:03}.csv';buf=io.StringIO(newline='');csv.writer(buf,lineterminator='\r\n').writerow(row);source.write_bytes(buf.getvalue().encode('ascii'))
            cells=out/f'control-{j:03}-cells.bin';cells.write_bytes(b''.join(s.encode('ascii').ljust(64,b'\0') for s in row))
            props=out/f'control-{j:03}-properties.bin'
            run=subprocess.run([str(v.ROOT/'local/behavior/standard-properties-probe.exe'),str(image),str(cells),str(props)],cwd=v.ROOT,capture_output=True,text=True,timeout=30)
            assert run.returncode==0 and not run.stderr and 'data_restored=1 code_restored=1 error=0' in run.stdout
            fixtures.append((44+j,dict(source=str(source),source_sha256=v.digest(source),properties=str(props),output_sha256=v.digest(props),ned=str(v.GAME/'cars/lac/lac.ned'),label=f'column{column}={value}',property_execution=dict(exit=run.returncode,stdout=run.stdout,stderr=run.stderr))))
    for i, source in fixtures:
        if not a.all and Path(source['source']).as_posix() != 'cars/lac/lac.csv':
            continue
        path = v.GAME / source['source']
        assert v.digest(path) == source['source_sha256']
        ned = Path(source.get('ned',str(path.with_suffix('.ned'))))
        if not ned.exists():
            results.append(dict(source=source['source'], skipped='Unverified variant-to-base geometry selection'))
            continue
        attrs = [e.attrib for e in decode(ned.read_bytes())[1].iter() if e.tag == 'WHEELS']
        assert len(attrs) == 1
        props = Path(source.get('properties',str(a.properties_report.parent / f'{i:03}-original.bin')))
        assert v.digest(props) == source['output_sha256']
        row = next(r for r in csv.reader(path.read_text().splitlines()) if r)
        bias = f32(float(row[194] or 0))
        points, radii, widths = [], [], []
        # Explicit caller preparation: source axle points, mirrored X, Y-bias.
        # This asset association is verified separately from original A0C9F0.
        for axle in ['rear', 'front']:
            x, y, z, radius = [f32(float(attrs[0][f'm_{axle}_{c}'])) for c in 'xyzr']
            for side in [1, -1]:
                points.extend([x * side, f32(y - bias), z, 1])
                radii.append(radius)
            widths.extend(f32(float(attrs[0][f'm_{axle}_{side}_width'])) for side in ['l', 'r'])
        request = out / f'{i:03}-input.bin'
        request.write_bytes((b'SC05' if a.outer_wheels else b'SC04' if a.mutable_prelude else b'SC03' if a.source_prelude else b'SC02') + struct.pack('<I',a.seed) + props.read_bytes() + struct.pack('<24f', *points, *radii, *widths))
        assert request.stat().st_size == 1732
        executions = []
        for kind in ['original', 'repeat']:
            target = out / f'{i:03}-{kind}.bin'
            run = subprocess.run([str(exe), str(image), str(request), str(target)], cwd=v.ROOT, capture_output=True, text=True, timeout=30)
            execution = dict(source=source['source'], kind=kind, exit=run.returncode, stdout=run.stdout, stderr=run.stderr)
            executions.append(execution)
            (out / 'current-executions.json').write_text(json.dumps(executions, indent=2))
            if run.returncode or run.stderr:
                (out / 'failure.json').write_text(json.dumps(execution, indent=2))
                raise AssertionError(execution)
            assert 'code_unchanged=1 code_replacements=0 globals_restored=1 error=0' in run.stdout
            assert 'excluded_stack_padding_bytes=2' in run.stdout
            if a.outer_wheels:assert 'original_wheel_region_loop_steps=89 stopped_before_document_call=1 loop_error=0' in run.stdout
            assert target.stat().st_size == 0x21000
        answers = [(out / f'{i:03}-{kind}.bin').read_bytes() for kind in ['original', 'repeat']]
        assert answers[0] == answers[1]
        if a.profile:
            target = out / f'{i:03}-rust.bin'
            command = [str(v.ROOT / f'target/{a.profile}/grid-standard-car.exe'), str(path), str(ned), f'{a.seed:08x}', str(target)]
            if a.source_prelude: command.append('--outer-wheels' if a.outer_wheels else '--mutable-prelude' if a.mutable_prelude else '--source-prelude')
            run = subprocess.run(command, cwd=v.ROOT, capture_output=True, text=True, timeout=30)
            execution = dict(source=source['source'], kind='rust', exit=run.returncode, stdout=run.stdout, stderr=run.stderr)
            executions.append(execution)
            if run.returncode or run.stderr:
                (out / 'failure.json').write_text(json.dumps(execution, indent=2))
                raise AssertionError(execution)
            answer = target.read_bytes()
            rng=a.seed
            for _ in range(18 if a.mutable_prelude else 12):rng=(rng*0x41c64e6d+0x40584c0f)&0xffffffff
            assert all(f'random_state={rng:08x}' in e['stdout'] for e in executions)
            assert len(answer) == len(answers[0])
            differences.extend(dict(source=source['source'], offset=hex(o), original=answers[0][o:o+4].hex(), rust=answer[o:o+4].hex()) for o in range(0,len(answer),4) if answers[0][o:o+4]!=answer[o:o+4])
        results.append(dict(source=source['source'],label=source.get('label','installed'), source_sha256=v.digest(path),ned=str(ned), ned_sha256=v.digest(ned), properties=str(props),properties_sha256=v.digest(props), request_sha256=v.digest(request), output_sha256=v.digest(out / f'{i:03}-original.bin'), repeat_exact=True, executions=executions, property_execution=source.get('property_execution')))
    assert any(q.get('repeat_exact') for q in results)
    report = dict(native_reference_passed=True, rust_equivalence=bool(a.profile) and not differences, profile=a.profile, random_seed=a.seed, differences=differences, properties_report=str(a.properties_report), properties_report_sha256=v.digest(a.properties_report), exe_sha256=v.digest(exe), results=results,
                  limits='Whole A0C9F0 with decoded properties, explicit geometry/cold zero storage/gravity/RNG; two stack padding bytes excluded and owned pointers tokenized. Original outer factory/asset caller/physical owner/live initialization and driving excluded. Cold zero control ramp retains original intermediate NaN.')
    report['source_prelude'] = a.source_prelude
    report['mutable_prelude'] = a.mutable_prelude
    report['outer_wheels'] = a.outer_wheels
    if a.mutable_prelude:report['limits']='Whole9F3D30 then A0C9F0 with selected source stores and explicit properties/geometry/gravity/RNG. Eighteen draws, two undefined stack-padding bytes excluded. Resource factory, wheel-region stores, preferences, physical owner and driving excluded; cold NaN retained.'
    if a.profile:report.update(rust_exe_sha256=v.digest(v.ROOT/f'target/{a.profile}/grid-standard-car.exe'))
    (out / 'validation.json').write_text(json.dumps(report, indent=2))
    print(json.dumps(dict(native_reference_passed=True, rust_equivalence=report['rust_equivalence'], cases=sum(bool(q.get('repeat_exact')) for q in results), differences=len(differences), first=differences[:25])))
    assert not differences


if __name__ == '__main__':
    main()
