"""Whole packed generator with separately owned full descriptor and sheet mask.

Other geometric context remains an explicit fixture; no runtime/factory claim.
"""
import argparse
import json
import struct
import subprocess
from pathlib import Path

import validate_packed_mesh_contact as legacy
import validate_packed_shape_constructor as constructor
from validate_vcqtc import chunks

ROOT, IMAGE, digest = legacy.ROOT, legacy.IMAGE, legacy.digest
NATIVE = ROOT/'local/behavior/packed-mesh-complete-probe.exe'
UPSTREAM = ROOT/'local/behavior/packed-mesh-contact-lacetti-20261012-final-01/validation.json'


def inputs():
    report = json.loads(UPSTREAM.read_text())
    assert report['passed'] and report['cases'] == 1417
    source = Path(report['source_path'])
    assert digest(source) == report['source_sha256']
    attachment = ROOT/report['attachment']
    assert digest(attachment) == report['attachment_sha256']
    row = json.loads(attachment.read_text())['results'][report['attachment_index']]
    assert row['source'] == r'cars\lac\lac.ned'
    for key in ['source', 'csv']:
        assert digest(constructor.GAME/row[key]) == row[key+'_sha256']
    seed_path = attachment.parent/f"{report['attachment_index']:03}-original.bin"
    assert digest(seed_path) == report['seed_sha256'] == row['output_sha256']
    seed = seed_path.read_bytes()
    geometry = {name: (positions, triangles) for name, positions, triangles, _, _ in chunks(source.read_bytes())}
    packed = {name: (raw, mask, alternate) for name, raw, _, mask, alternate, _ in constructor.sources()}
    for previous in report['results']:
        base = report['results'][previous['base_index']] if 'base_index' in previous else previous
        positions, triangles = geometry[base['chunk']]
        points = [positions[v] for v in triangles[base['triangle']][0]]
        center = [sum(v[j] for v in points)/3 for j in range(3)]
        center[1] += base['height']
        assert center == base['center']
        limits = (previous['contact_capacity'], previous['cache_capacity'], previous['candidate_capacity']) if 'base_index' in previous else (16, 1, 4096)
        raw, source_mask, alternate = packed[base['chunk']]
        old = struct.pack('<6I4f', 0x31304350, len(seed), len(raw), *limits, *center, 1.)+seed+raw
        path = UPSTREAM.parent/f"{previous['index']:04}-input.bin"
        assert digest(path) == previous['input_sha256'] and path.read_bytes() == old
        controls = [('source', source_mask)]
        if 'base_index' not in previous:
            controls.append(('all', (1 << -struct.unpack_from('<i', raw, 32)[0])-1))
        for label, mask in controls:
            request = struct.pack('<I', 0x38304350)+old[4:40]+struct.pack('<I', mask)+old[40:]
            yield dict(upstream_index=previous['index'], chunk=base['chunk'], label=label, mask=mask), request


def check(answer, request):
    _, ns, nc, limit, cache, indices = struct.unpack_from('<6I', request)
    center = struct.unpack_from('<4f', request, 24)
    mask = struct.unpack_from('<I', request, 40)[0]
    seed, chunk = request[44:44+ns], request[44+ns:]
    assert len(chunk) == nc
    tail = bytearray(0x40)
    struct.pack_into('<5I', tail, 0x10, mask, *([0x7f7fffff]*4))
    assert answer[-0x40:] == tail
    return legacy.check(answer[:-0x40], seed, chunk, center, limit, cache, indices)


def run(inp, out):
    r = subprocess.run([str(NATIVE), str(IMAGE), str(inp), str(out)], cwd=ROOT, capture_output=True, text=True, timeout=30)
    return dict(exit=r.returncode, stdout=r.stdout, stderr=r.stderr)


def main():
    p = argparse.ArgumentParser()
    p.add_argument('output', type=Path)
    p.add_argument('--max-cases', type=int, default=2642)
    a = p.parse_args()
    assert 1 <= a.max_cases <= 2642
    out = a.output.resolve()
    assert out.is_relative_to((ROOT/'local').resolve())
    out.mkdir(exist_ok=False)
    rows = []
    source_contacts = {}
    changed = 0
    for meta, request in inputs():
        i = len(rows)
        if i == a.max_cases: break
        inp = out/f'{i:04}-input.bin'
        inp.write_bytes(request)
        answers, executions = [], []
        for kind in ['original', 'repeat']:
            target = out/f'{i:04}-{kind}.bin'
            e = run(inp, target); executions.append(e)
            assert e['exit'] == 0 and not e['stderr'], (i, meta, e)
            assert 'code_unchanged=1 globals_restored=1 code_replacements=0 error=0' in e['stdout']
            answer = target.read_bytes()
            observed = check(answer, request)
            answers.append(answer)
        assert answers[0] == answers[1]
        contact_area = answers[0][0xfe8:0x17e8]
        if meta['label'] == 'source': source_contacts[meta['upstream_index']] = contact_area
        else: changed += contact_area != source_contacts[meta['upstream_index']]
        rows.append(dict(index=i, **meta, input_sha256=digest(inp), output_sha256=digest(out/f'{i:04}-original.bin'), bytes=len(answers[0]), executions=executions, **observed))
        if (i+1) % 200 == 0: print(f'cases={i+1}', flush=True)
    first = (out/'0000-input.bin').read_bytes()
    guards = []
    bad_inputs = [('short', first[:43]), ('long', first+b'\0')]
    for label, offset, value in [('tag', 0, 0), ('mask', 40, 256), ('outside-mask', 40, 128), ('contact-limit', 12, 17), ('cache-limit', 16, 2), ('candidate-limit', 20, 32769)]:
        bad = bytearray(first); struct.pack_into('<I', bad, offset, value); bad_inputs.append((label, bad))
    for label, bad in bad_inputs:
        inp = out/f'guard-{label}-input.bin'; inp.write_bytes(bad)
        target = out/f'guard-{label}.bin'; e = run(inp, target)
        assert 0 < e['exit'] < 256 and not target.exists() and not e['stderr'], (label, e)
        guards.append(dict(label=label, input_sha256=digest(inp), execution=e, no_output=True))
    target = out/'guard-existing.bin'; target.write_bytes(b'preserve existing evidence'); before = digest(target)
    e = run(out/'0000-input.bin', target)
    assert e['exit'] and digest(target) == before
    guards.append(dict(label='existing', execution=e, unchanged_sha256=before))
    report = dict(passed=True, cases=len(rows), results=rows, guards=guards,
                  contacts=sum(r['contacts'] for r in rows), source_all_contact_differences=changed,
                  observable_bytes=sum(r['bytes'] for r in rows),
                  upstream=str(UPSTREAM.relative_to(ROOT)), upstream_sha256=digest(UPSTREAM),
                  image_sha256=digest(IMAGE), native_sha256=digest(NATIVE), script_sha256=digest(Path(__file__)),
                  full_descriptor=True, rust_generator_equivalence=False, full_factory_equivalence=False)
    (out/'validation.json').write_text(json.dumps(report, indent=2))
    print(json.dumps({k: report[k] for k in ['passed', 'cases', 'contacts', 'observable_bytes', 'source_all_contact_differences']}))


if __name__ == '__main__': main()
