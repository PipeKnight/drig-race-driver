"""Actual mesh contacts through original and Rust advance/group/solve stages.

Generation is the preceding verified native survey, not a Rust geometry query.
"""
import argparse
import copy
import json
import struct
import subprocess
from pathlib import Path
import validate_shaped_physical_frame as shaped
import validate_grouped_physical_frame as grouped
from validate_mesh_contact import ROOT, IMAGE, digest
from validate_main_contact_response import uget, uput, fget, fput
from validate_contact_bias import compare

MAGIC = 0x31475447
MAX = struct.unpack('<f', bytes.fromhex('ffff7f7f'))[0]


def fixture(answer, index, packed=False):
    body_offset, count_offset, contact_offset = ((0x17e8, 0xfb0, 0xfe8) if packed
                                                else (0x17c8, 0xf90, 0xfc8))
    body = bytearray(answer[body_offset:body_offset+0x424])
    # Convert the verified attached body to the existing cold serial solver scope.
    # Geometry shape/pair ownership stays in the preceding probe, not these queues.
    for o in [0, 4, 8, 12, 16, 20, 24, 28, 0x28, 0x2f0, 0x2f4, 0x2fc, 0x300,
              0x304, 0x308, 0x30c, 0x318, 0x320, 0x324, 0x328, 0x32c, 0x338,
              0x33c, 0x340, 0x400, 0x418, 0x41c, 0x420]:
        uput(body, o, 0)
    body[0x2c:0x2e] = bytes(2)
    body[0x412:0x414] = bytes(2)
    body[0x415] = 0
    fput(body, 0x3b4, MAX)
    count = uget(answer, count_offset)
    normal = struct.unpack_from('<4f', answer, contact_offset+0x60)
    for j in range(4):
        fput(body, 0x210+4*j, 0.)
        fput(body, 0x220+4*j, -normal[j]*[0., 1., 5., 15.][index % 4])
    contacts = []
    for j in range(count):
        c = bytearray(answer[contact_offset+j*128:contact_offset+(j+1)*128])
        assert uget(c, 4) == 0xa000 and uget(c, 8) == 0 and c[0xf] == 1
        for o, value in [(4, 0), (8, 0xffffffff), (0x1c, 0), (0x20, 0xffffffff),
                         (0x28, 1), (0x2c, 2), (0x30, 0), (0x34, 0)]:
            uput(c, o, value)
        contacts.append((2, c))
    return dict(bodies=[dict(raw=body, properties=[fget(body, 0x3f4), *struct.unpack_from('<16f', body, 0x150)], workspace=[0.]*8)],
                materials=[[0., .3, .4, .01, .5, .2, 2., .5, 3., .75, 20., 0., .125]],
                contacts=contacts, rows=[bytearray(192) for _ in range(64)], capacity=64,
                iterations=[1, 2, 4, 8][index % 4], parameters=[.016, 0., -9.81, 0., 0., .01, .2, .125, .8, .2, .2],
                ends=[0]*4, pair=[0xffffffff]*2, cache=[0.]*13,
                correction=[4, .01, 1., 0., .8], order=[0], shapes=[(0, 0, 0, 0), (1, 0, 0, 0)])


def main():
    p = argparse.ArgumentParser()
    p.add_argument('output', type=Path)
    p.add_argument('--upstream', type=Path, required=True)
    p.add_argument('--profile', choices=['debug', 'release'], required=True)
    p.add_argument('--limit', type=int, default=0)
    p.add_argument('--seed-offset', type=int, default=0)
    p.add_argument('--packed-track', action='store_true')
    p.add_argument('--packed-complete', action='store_true', help='Use source-mask records from the full nonaliased PC08 fixture')
    args = p.parse_args()
    assert args.limit >= 0 and args.seed_offset >= 0
    out = args.output.resolve()
    assert out.is_relative_to((ROOT/'local').resolve())
    out.mkdir(exist_ok=False)
    upstream = args.upstream.resolve()
    report = json.loads(upstream.read_text())
    assert report['passed']
    assert not args.packed_complete or args.packed_track
    if args.packed_complete:
        assert report['full_descriptor'] and not report['rust_generator_equivalence']
        origin = ROOT/report['upstream']
        assert digest(origin) == report['upstream_sha256']
        original_report = json.loads(origin.read_text())
        assert original_report['attachment_index'] == 48 and original_report['native_reference_only']
    elif args.packed_track:
        assert report['native_reference_only'] and report['attachment_index'] == 48
        assert report['cases'] >= 15 and 'B39500/AEA1F0' in report['scope']
    else:
        assert report['contact_slot_constructor'] and report['cases'] == 720
    shaped.MAGIC = grouped.MAGIC = MAGIC
    cases, provenance = [], []
    eligible = [row for row in report['results']
                if row['contacts' if args.packed_track else 'contact_count']
                and (not args.packed_complete or row['label'] == 'source')]
    assert args.seed_offset < len(eligible)
    for row in eligible[args.seed_offset:]:
        if not row['contacts' if args.packed_track else 'contact_count']:
            continue
        source = upstream.parent/f"{row['index']:04}-original.bin"
        assert digest(source) == row['output_sha256']
        baseline = fixture(source.read_bytes(), row['index'], args.packed_track)
        for kind in ['enabled', 'correction_disabled', 'empty']:
            c = copy.deepcopy(baseline)
            if kind == 'correction_disabled':
                c['shapes'][1] = (1, 1, 0, 0)
            if kind == 'empty':
                c['contacts'] = []
            cases.append(c)
            provenance.append(dict(source_index=row['index'], source_sha256=digest(source), kind=kind))
        if args.limit and len(provenance)//3 >= args.limit:
            break
    request = out/'input.bin'
    request.write_bytes(shaped.pack(cases))
    native = ROOT/'local/behavior/geometric-physical-tick-probe.exe'
    rust = ROOT/f'target/{args.profile}/grid-physical-tick.exe'
    answers, runs = [], []
    for kind, command in [('original', [native, IMAGE]), ('repeat', [native, IMAGE]), ('rust', [rust])]:
        target = out/f'{kind}.bin'
        e = subprocess.run([*map(str, command), str(request), str(target)], cwd=ROOT, capture_output=True, text=True, timeout=60)
        run = dict(kind=kind, exit=e.returncode, stdout=e.stdout, stderr=e.stderr)
        runs.append(run)
        (out/'current-executions.json').write_text(json.dumps(runs, indent=2))
        assert e.returncode == 0 and not e.stderr, run
        answers.append(target.read_bytes())
        run.update(compare(answers[-1], answers[0]))
        assert run['passed'], run
    outputs = shaped.decode(answers[0], cases)
    points_unchanged = all(after[0x50:0x70] == before[0x50:0x70]
                           for c, o in zip(cases, outputs) for (_, before), after in zip(c['contacts'], o['contacts']))
    assert points_unchanged
    responses = []
    for i in range(0, len(cases), 3):
        enabled, disabled, empty = outputs[i:i+3]
        assert all(c[0xe] == 1 for c in disabled['contacts'])
        assert all(c[0xe] == 0 for c in enabled['contacts'])
        assert disabled['bodies'][0][0x210:0x230] == enabled['bodies'][0][0x210:0x230]
        responses.append(enabled['bodies'][0][0x210:0x230] != empty['bodies'][0][0x210:0x230])
    assert any(responses)
    guards = []
    sample = cases[0]
    edits = [
        ('point-nan', lambda c: uput(c['contacts'][0][1], 0x50, 0x7fc00000)),
        ('normal-inf', lambda c: uput(c['contacts'][0][1], 0x60, 0x7f800000)),
        ('body-owner', lambda c: uput(c['contacts'][0][1], 4, 1)),
        ('shape-zero', lambda c: uput(c['contacts'][0][1], 0x28, 0)),
        ('material', lambda c: uput(c['contacts'][0][1], 0x30, 1)),
        ('limit-negative', lambda c: fput(c['contacts'][0][1], 0x3c, -1.)),
        ('capacity', lambda c: c.__setitem__('capacity', 0)),
        ('iterations', lambda c: c.__setitem__('iterations', 0)),
    ]
    variants = []
    for label, edit in edits:
        c = copy.deepcopy(sample)
        edit(c)
        variants.append((label, shaped.pack([c])))
    single = shaped.pack([sample])
    variants.extend([('short', single[:-1]), ('long', single+b'\0'), ('tag', bytes(4)+single[4:])])
    for label, data in variants:
        inp = out/f'guard-{label}-input.bin'
        inp.write_bytes(data)
        for kind, command in [('native', [native, IMAGE]), ('rust', [rust])]:
            target = out/f'guard-{label}-{kind}.bin'
            e = subprocess.run([*map(str, command), str(inp), str(target)], cwd=ROOT, capture_output=True, text=True, timeout=30)
            guard = dict(label=label, kind=kind, exit=e.returncode, stdout=e.stdout, stderr=e.stderr, no_output=not target.exists())
            guards.append(guard)
            (out/'current-guards.json').write_text(json.dumps(guards, indent=2))
            assert 0 < e.returncode < 256 and not target.exists(), guard
            if kind == 'native':
                assert not e.stderr and 'code_unchanged=1' in e.stdout, guard
    existing_input = out/'guard-existing-input.bin'
    existing_input.write_bytes(single)
    for kind, command in [('native', [native, IMAGE]), ('rust', [rust])]:
        target = out/f'guard-existing-{kind}.bin'
        target.write_bytes(b'preserve existing evidence')
        before = digest(target)
        e = subprocess.run([*map(str, command), str(existing_input), str(target)], cwd=ROOT, capture_output=True, text=True, timeout=30)
        assert 0 < e.returncode < 256 and digest(target) == before
        guards.append(dict(label='existing', kind=kind, exit=e.returncode, existing_output_preserved=True))
    result = dict(passed=True, profile=args.profile, packed_track=args.packed_track, packed_complete=args.packed_complete,
                  cases=len(cases), positive_mesh_seeds=len(cases)//3,
                  contacts=sum(len(c['contacts']) for c in cases), response_cases=sum(responses),
                  observable_bytes=len(answers[0])-8, world_points_unchanged=points_unchanged,
                  correction_flag_retains_main_response=True, guard_count=len(guards), guards=guards, runs=runs, provenance=provenance,
                  upstream=str(upstream.relative_to(ROOT)), upstream_sha256=digest(upstream),
                  input_sha256=digest(request), output_sha256=digest(out/'original.bin'),
                  native_sha256=digest(native), rust_sha256=digest(rust), script_sha256=digest(Path(__file__)),
                  scope='Verified native mesh contacts, cold-normalized source physical bodies, supplied finite material/velocity/solver settings -> unchanged AD7040/AD6C40/B77310 and Rust. Contacts use original producer930 pair path, not external924 pool. Stage-composed reference handoff, no full AD8D70, Rust generator, moving geometry lifecycle or runtime barrier claim.')
    (out/'validation.json').write_text(json.dumps(result, indent=2))
    print(json.dumps({k: result[k] for k in ['passed', 'cases', 'positive_mesh_seeds', 'contacts', 'response_cases', 'observable_bytes', 'world_points_unchanged', 'correction_flag_retains_main_response']}))


if __name__ == '__main__':
    main()
