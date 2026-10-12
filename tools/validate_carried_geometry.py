"""Retained physical stages with source geometry and mixed point coordinates.

The contact reconstruction schedule is authored; no fresh detection is claimed.
"""
import argparse
import copy
import json
import struct
import subprocess
from pathlib import Path

import validate_mesh_contact_handoff as handoff
import validate_shaped_physical_frame as shaped
from validate_main_contact_response import uget, fget
from validate_contact_bias import compare

ROOT, IMAGE, digest = handoff.ROOT, handoff.IMAGE, handoff.digest
UPSTREAM = ROOT/'local/behavior/packed-complete-20261012-final-01/validation.json'
NATIVE = ROOT/'local/behavior/carried-geometry-probe.exe'
MAGIC = 0x32475447


def cases(limit=0):
    r = json.loads(UPSTREAM.read_text())
    assert r['passed'] and r['full_descriptor'] and not r['rust_generator_equivalence']
    legacy = ROOT/r['upstream']; assert digest(legacy) == r['upstream_sha256']
    old = json.loads(legacy.read_text()); assert old['attachment_index'] == 48
    attachment = ROOT/old['attachment']; assert digest(attachment) == old['attachment_sha256']
    source = Path(old['source_path']); assert digest(source) == old['source_sha256']
    rows = [row for row in r['results'] if row['label'] == 'source' and row['contacts']]
    assert len(rows) == 608
    if limit: rows = rows[:limit]
    out = []
    for row in rows:
        path = UPSTREAM.parent/f"{row['index']:04}-original.bin"
        assert digest(path) == row['output_sha256']
        base = handoff.fixture(path.read_bytes(), row['index'], True)
        n = len(base['contacts'])
        cycle = [[0, n], [min(2, n), n], [0, 0], [n, n], [0, 1], [0, 0], [0, n], [1, min(2, n)]]
        for mode in ['geometry', 'mixed']:
            for steps in [1, 15, 16]:
                c = copy.deepcopy(base)
                c.update(source_index=row['index'], source_sha256=digest(path), mode=mode, steps=steps,
                         counts=([[0, n]]*steps if mode == 'geometry' else [cycle[k % 8] for k in range(steps)]))
                out.append(c)
    return out


def pack(cases):
    data = bytearray(struct.pack('<II', MAGIC, len(cases)))
    for c in cases:
        raw = shaped.tick_pack([c])[8:]
        data.extend(raw[:164]); data.extend(struct.pack('<I', len(c['shapes'])))
        data.extend(b''.join(bytes(s) for s in c['shapes']))
        data.extend(struct.pack('<I', c['steps']))
        data.extend(b''.join(struct.pack('<2I', *counts) for counts in c['counts']))
        data.extend(raw[164:])
    return bytes(data)


def run(exe, inp, out, native=True):
    cmd = [str(exe)] + ([str(IMAGE)] if native else []) + [str(inp), str(out)]
    r = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, timeout=60)
    return dict(exit=r.returncode, stdout=r.stdout, stderr=r.stderr)


def semantics(cases, outputs):
    by_seed = {(c['source_index'], c['mode'], c['steps']): o for c, o in zip(cases, outputs, strict=True)}
    round32 = lambda x: struct.unpack('<f', struct.pack('<f', x))[0]
    world_points = relative_points = retained_slots = histories = 0
    for c, o in zip(cases, outputs, strict=True):
        external, used = c['counts'][-1]
        assert o['world'][5:7] == [c['steps'] % 2, 1-c['steps'] % 2]
        assert all(a[0x60:0x70] == b[0x60:0x70] for (_, a), b in zip(c['contacts'][:used], o['contacts'][:used]))
        for (_, before), after in zip(c['contacts'][external:used], o['contacts'][external:used]):
            assert before[0x50:0x60] == after[0x50:0x60]
            world_points += 1
        previous = by_seed.get((c['source_index'], c['mode'], c['steps']-1))
        if c['steps'] == 1:
            old_bodies = [b['raw'] for b in c['bodies']]
            old_slots = [r for _, r in c['contacts']]
        elif previous:
            old_bodies, old_slots = previous['bodies'], previous['contacts']
        else:
            continue
        assert all(b[0x190:0x210] == a[0x270:0x2f0] for b, a in zip(old_bodies, o['bodies']))
        histories += 1
        assert old_slots[used:] == o['contacts'][used:]
        retained_slots += len(old_slots)-used
        for (_, before), after in zip(c['contacts'][:external], o['contacts'][:external]):
            owner = uget(before, 4)
            b = old_bodies[owner]
            for j in range(4):
                center = fget(b, 0x1c0+4*j)
                relative = round32(fget(before, 0x50+4*j)-center)
                advanced = round32(round32(round32(c['parameters'][0])*fget(b, 0x220+4*j))+center)
                assert struct.pack('<f', round32(advanced+relative)) == after[0x50+4*j:0x54+4*j]
                relative_points += 1
    return dict(world_point_checks=world_points, relative_point_lane_checks=relative_points,
                inactive_slot_checks=retained_slots, history_pairs=histories)


def main():
    p = argparse.ArgumentParser(); p.add_argument('output', type=Path)
    p.add_argument('--profile', choices=['debug', 'release'], required=True)
    p.add_argument('--limit', type=int, default=0); a = p.parse_args()
    assert 0 <= a.limit <= 608
    out = a.output.resolve(); assert out.is_relative_to((ROOT/'local').resolve()); out.mkdir(exist_ok=False)
    source = cases(a.limit); rust = ROOT/f'target/{a.profile}/grid-physical-tick.exe'
    shaped.MAGIC = shaped.g.MAGIC = MAGIC
    batches, outputs = [], []
    for start in range(0, len(source), 120):
        batch = source[start:start+120]; inp = out/f'{start:04}-input.bin'; inp.write_bytes(pack(batch))
        answers, executions = [], []
        for kind, exe in [('original', NATIVE), ('repeat', NATIVE), ('rust', rust)]:
            target = out/f'{start:04}-{kind}.bin'; e = run(exe, inp, target, kind != 'rust'); executions.append(e)
            assert e['exit'] == 0 and not e['stderr'], (start, kind, e)
            if kind != 'rust': assert 'code_unchanged=1 no_imports=1' in e['stdout']
            answers.append(target.read_bytes())
            assert compare(answers[-1], answers[0])['passed'], (start, kind, compare(answers[-1], answers[0]))
        outputs.extend(shaped.decode(answers[0], batch))
        batches.append(dict(start=start, cases=len(batch), input_sha256=digest(inp), output_sha256=digest(out/f'{start:04}-original.bin'), bytes=len(answers[0]), executions=executions))
        print(f'cases={start+len(batch)}', flush=True)
    activity = semantics(source, outputs)
    sample = copy.deepcopy(source[0]); good = pack([sample]); bad = [('short', good[:-1]), ('long', good+b'\0')]
    for label, edit in [
        ('zero-steps', lambda c: c.update(steps=0, counts=[])),
        ('excess-steps', lambda c: c.update(steps=65, counts=[[0, 0]]*65)),
        ('prefix', lambda c: c['counts'].__setitem__(0, [2, 1])),
        ('used', lambda c: c['counts'].__setitem__(0, [0, len(c['contacts'])+1])),
        ('point-nan', lambda c: handoff.uput(c['contacts'][0][1], 0x50, 0x7fc00000)),
        ('inactive-normal', lambda c: (c.update(counts=[[0, 0]]), handoff.uput(c['contacts'][-1][1], 0x60, 0x7fc00000))),
        ('capacity', lambda c: c.update(capacity=0)),
    ]:
        c = copy.deepcopy(sample); edit(c); bad.append((label, pack([c])))
    guards = []
    for label, data in bad:
        inp = out/f'guard-{label}-input.bin'; inp.write_bytes(data)
        for kind, exe in [('native', NATIVE), ('rust', rust)]:
            target = out/f'guard-{label}-{kind}.bin'; e = run(exe, inp, target, kind == 'native')
            assert 0 < e['exit'] < 256 and not target.exists() and 'fault=' not in e['stderr'], (label, kind, e)
            if kind == 'native': assert not e['stderr'] and 'code_unchanged=1 no_imports=1' in e['stdout']
            guards.append(dict(label=label, kind=kind, input_sha256=digest(inp), execution=e, no_output=True))
    inp = out/'guard-existing-input.bin'; inp.write_bytes(good)
    for kind, exe in [('native', NATIVE), ('rust', rust)]:
        target = out/f'guard-existing-{kind}.bin'; target.write_bytes(b'preserve existing evidence'); before = digest(target)
        e = run(exe, inp, target, kind == 'native'); assert e['exit'] and digest(target) == before
        guards.append(dict(label='existing', kind=kind, execution=e, unchanged_sha256=before))
    report = dict(passed=True, profile=a.profile, source_placements=len(source)//6, cases=len(source),
        intervals=sum(c['steps'] for c in source), observable_bytes=sum(b['bytes']-8 for b in batches),
        activity=activity, batches=batches, guards=guards,
        provenance=[dict(source_index=c['source_index'], source_sha256=c['source_sha256'], mode=c['mode'], steps=c['steps']) for c in source],
        upstream_sha256=digest(UPSTREAM), image_sha256=digest(IMAGE), native_sha256=digest(NATIVE), rust_sha256=digest(rust),
        script_sha256=digest(Path(__file__)),
        scope='Retained AD7040/AD6C40/B77310 stages, supplied source-world contact templates and authored reconstruction/count schedule. No whole AD8D70, actual fresh geometry, wheel producer or runtime barriers.')
    (out/'validation.json').write_text(json.dumps(report, indent=2))
    print(json.dumps({k: report[k] for k in ['passed', 'profile', 'source_placements', 'cases', 'intervals', 'observable_bytes', 'activity']}))


if __name__ == '__main__': main()
