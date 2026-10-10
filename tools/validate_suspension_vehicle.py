"""Whole selected 9F1E10 with real preparation, packed contacts and substeps."""
import argparse
import hashlib
import json
import random
import struct
import subprocess
from pathlib import Path

from validate_suspension_contact import ROOT, TRACK, SURFACE, WORLD, MECHANICS, NATIVE, build_cases, put, ui, digest, execute

RECORD = 0x2614


def cases():
    result = build_cases()
    rng = random.Random(2008168)
    for i, (_, body) in enumerate(result):
        ui(body, 0, 0)
        ui(body, 0x2278, 0)
        put(body, 0x3d0, rng.uniform(.8, 1.2))
        put(body, 0x11e8, rng.uniform(.5, 1.5))
        put(body, 0x185c, rng.uniform(0, 1))
        intervals = [(2, 5, 10), (-1, .2, .8), (.1, .3, .7), (0, .5, 1)][i % 4]
        put(body, 0x2198, [rng.uniform(.6, 1.4) for _ in range(7)] + list(intervals) + [rng.uniform(.8, 1.2)])
        if i % 4 != 0:
            put(body, 0x1648, rng.uniform(-1, 2))
        for w in range(4):
            wheel = 0x1990 + w*0x150
            for offset, value in [(0x50, .25), (0x54, -.04), (0x58, 500), (0x60, 1000), (0x64, .05), (0x68, .6), (0x70, 1500), (0x114, .8)]:
                put(body, wheel+offset, value)
    return result


def request_bytes(current):
    return b'SF01'+struct.pack('<I', len(current))+b''.join(struct.pack('<If', i, delta)+body+bytes(12) for i, (delta, body) in enumerate(current))


def compare(data, original, repeat, rust):
    assert len(data) == len(original) == len(repeat) == len(rust)
    differences = []
    count = (len(data)-8)//RECORD
    coverage = dict(pose_changes=0, velocity_changes=0, force_changes=0, contact_flags=0, retained_materials=0)
    for i in range(count):
        start = 16+i*RECORD
        before, after = data[start:start+0x2600], original[start:start+0x2600]
        coverage['pose_changes'] += before[0x15e0:0x1620] != after[0x15e0:0x1620]
        coverage['velocity_changes'] += before[0x15c0:0x15e0] != after[0x15c0:0x15e0]
        assert after[0x3d0+0x690:0x3d0+0x6d0] == after[0x15e0:0x1620]
        assert after[0x3d0+0x6d0:0x3d0+0x6f0] == after[0x15c0:0x15e0]
        assert struct.unpack_from('<I', after, 0x3d0+0x5d0)[0] & 255 == 1
        assert after[0x15a8:0x15b4] == before[0x15a8:0x15b4] == original[start+0x2600:start+0x260c]
        assert after[0x3d0+0x610:0x3d0+0x614] == bytes(4)
        for w in range(4):
            c, wheel = 0x3d0+w*0x170, 0x1990+w*0x150
            coverage['force_changes'] += before[wheel+0x138:wheel+0x13c] != after[wheel+0x138:wheel+0x13c]
            coverage['contact_flags'] += bool(after[c+0x84])
            coverage['retained_materials'] += struct.unpack_from('<I', after, wheel+0xc0)[0] != 0
            for off in [0x140, 0x144, 0x148, 0x14c, 0x168, 0x16c, 0x170, 0x174]:
                assert after[c+off:c+off+4] == bytes(4)
            assert after[c+0x7c:c+0x80] == after[c+0x80:c+0x84]
        for off in range(0, 0x260c, 4):
            a, b = original[start+off:start+off+4], rust[start+off:start+off+4]
            if a != b:
                differences.append(dict(case=i, offset=f'{off:x}', original=a.hex(), rust=b.hex()))
    return dict(calls=count, words=count*0x260c//4, repeat_exact=original==repeat, differing_words=len(differences), differences=differences, coverage=coverage)


def run_set(out, profile, current, prefix):
    request = out/f'{prefix}-input.bin'
    data = request_bytes(current)
    request.write_bytes(data)
    rust = ROOT/f'target/{profile}/grid-suspension-contact.exe'
    runs = []
    for kind, exe, arguments in [
        ('original', NATIVE, [ROOT/'local/compatibility/grid-unpacked.exe', WORLD, request, out/f'{prefix}-original.bin', MECHANICS]),
        ('repeat', NATIVE, [ROOT/'local/compatibility/grid-unpacked.exe', WORLD, request, out/f'{prefix}-repeat.bin', MECHANICS]),
        ('rust', rust, [TRACK, SURFACE, request, out/f'{prefix}-rust.bin']),
    ]:
        runs.append(execute(exe, arguments, out, prefix+'-'+kind))
    outputs = {kind:(out/f'{prefix}-{kind}.bin').read_bytes() for kind in ['original', 'repeat', 'rust']}
    report = compare(data, outputs['original'], outputs['repeat'], outputs['rust'])
    report.update(executions=runs, hashes={kind:hashlib.sha256(value).hexdigest() for kind, value in outputs.items()}, request_sha256=digest(request))
    (out/f'{prefix}-validation.json').write_text(json.dumps(report, indent=2))
    return report, outputs


def guards(out, profile, first):
    rust = ROOT/f'target/{profile}/grid-suspension-contact.exe'
    tests = [('index',0,9), ('zero_delta',4,0), ('nan_delta',4,0x7fc00000), ('tiny_delta',4,1), ('large_delta',4,0x3e800000),
             ('count',8+0x3d0+0x5d8,5), ('extra_axes',8+0x3d0+0x70c,1), ('preselected',8+0x3d0+0x604,1),
             ('body_pointer',8+0x3d0+0x5d4,1), ('table_pointer',8+0x3d0+0x608,1), ('table_index',8+0x3d0+0x60c,1),
             ('wheel_pointer',8+0x3d0+0x178,1), ('object_pointer',8+0x3d0+0x17c,1), ('view_pointer',8,1),
             ('disabled',8+0x2278,0x100), ('zero_mass',8+0x15a0,0), ('zero_inertia',8+0x15a8,0),
             ('zero_scale',8+0x3d0,0), ('nan_basis',8+0x15e0,0x7fc00000), ('nan_axis',8+0x3d0+0x50,0x7fc00000),
             ('zero_radius',8+0x1990+0xe0,0), ('nan_spring',8+0x1990+0x5c,0x7fc00000),
             ('nan_damper',8+0x1990+0x60,0x7fc00000), ('zero_travel',8+0x1990+0x4c,0),
             ('negative_threshold',8+0x1990+0x64,0xbf800000), ('inertia_intervals',8+0x21b8,0x40000000)]
    results = []
    for name, offset, value in tests:
        record = bytearray(first)
        struct.pack_into('<I',record,offset,value)
        request = out/f'guard-{name}.bin'
        request.write_bytes(b'SF01'+struct.pack('<I',1)+record)
        for kind, exe, arguments in [
            ('original',NATIVE,[ROOT/'local/compatibility/grid-unpacked.exe',WORLD,request,out/f'guard-{name}-original.bin',MECHANICS]),
            ('rust',rust,[TRACK,SURFACE,request,out/f'guard-{name}-rust.bin'])]:
            run = subprocess.run([str(exe),*map(str,arguments)],cwd=ROOT,capture_output=True,text=True,timeout=30)
            passed = run.returncode in range(63,70) and 'records=0' in run.stdout and 'code_unchanged=1' in run.stdout if kind=='original' else run.returncode==1
            target = arguments[-2] if kind=='original' else arguments[-1]
            results.append(dict(name=name,kind=kind,exit=run.returncode,stdout=run.stdout,stderr=run.stderr,passed=passed and not target.exists()))
    assert all(r['passed'] for r in results), results
    return results


def carried(out, profile, seed):
    branches = {kind:[(d,bytearray(b)) for d,b in seed] for kind in ['original','repeat','rust']}
    rust = ROOT/f'target/{profile}/grid-suspension-contact.exe'
    reports = []
    for step in range(12):
        outputs, requests, runs = {}, {}, []
        for kind, current in branches.items():
            request = out/f'carry-{step}-{kind}-input.bin'
            data = request_bytes(current)
            request.write_bytes(data); requests[kind]=data
            target = out/f'carry-{step}-{kind}-out.bin'
            exe = rust if kind=='rust' else NATIVE
            arguments = [TRACK,SURFACE,request,target] if kind=='rust' else [ROOT/'local/compatibility/grid-unpacked.exe',WORLD,request,target,MECHANICS]
            runs.append(execute(exe,arguments,out,f'carry-{step}-{kind}'))
            outputs[kind]=target.read_bytes()
        report = compare(requests['original'],outputs['original'],outputs['repeat'],outputs['rust'])
        report['executions']=runs
        reports.append(report)
        if not report['repeat_exact'] or report['differences']:
            (out/f'carry-{step}-failure.json').write_text(json.dumps(report,indent=2))
            raise AssertionError(f'carried frame {step} differs')
        for kind,current in branches.items():
            following=[]
            for i,(delta,previous) in enumerate(current):
                body=bytearray(outputs[kind][16+i*RECORD:16+i*RECORD+0x2600])
                # Each branch advances its own complete suspension output. History
                # transfer and a pre-wrapper gravity contribution are authored.
                body[0xb40:0xb80]=previous[0x15e0:0x1620]
                body[0xb80:0xba0]=previous[0xb20:0xb40]
                body[0xb20:0xb40]=body[0x15c0:0x15e0]
                linear_y=struct.unpack_from('<f',body,0x15c4)[0]
                put(body,0x15c4,linear_y-9.81*delta)
                following.append((delta,body))
            branches[kind]=following
    return dict(sequences=len(seed),steps=12,calls=len(seed)*12,words=len(seed)*12*0x260c//4,
                final_sha256=hashlib.sha256(outputs['original']).hexdigest(),
                coverage={k:sum(r['coverage'][k] for r in reports) for k in reports[0]['coverage']}, reports=reports)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('--profile', required=True, choices=['debug', 'release'])
    args = parser.parse_args()
    out = args.output.resolve()
    assert out.is_relative_to((ROOT/'local').resolve())
    out.mkdir(exist_ok=False)
    wr = json.loads((WORLD.parent/'validation.json').read_text())
    sr = json.loads((MECHANICS.parent/'validation.json').read_text())
    assert digest(WORLD) == wr['original_fixture_sha256'] and digest(TRACK) == wr['source_sha256']
    assert digest(MECHANICS) == sr['mechanics_fixture_sha256'] and digest(SURFACE) == sr['source_mechanics_sha256']
    current=cases()
    report, _ = run_set(out, args.profile, current, 'initial')
    if report['repeat_exact'] and not report['differences']:
        report['guards']=guards(out,args.profile,request_bytes(current)[8:8+RECORD])
        report['carried']=carried(out,args.profile,current[8:24])
    report.update(profile=args.profile, scope='Whole unchanged9F1E10 with original inertia/preparation/world/contact/substep/finalizer dependencies; authored static-world/history inputs, selected extra-axes-off branch; no full vehicle/world/Bevy driving claim.')
    (out/'validation.json').write_text(json.dumps(report, indent=2))
    print(json.dumps({k:report[k] for k in ['profile','calls','words','repeat_exact','differing_words','coverage']} |
                     {'guards':len(report.get('guards',[])), 'carried_calls':report.get('carried',{}).get('calls',0)}))
    assert report['repeat_exact'] and not report['differences']


if __name__ == '__main__':
    main()
