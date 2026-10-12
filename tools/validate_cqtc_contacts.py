"""Owned original CQTC contact dispatch / source-only Rust comparison.

Original kind-10 collector and complete 16-slot manifold projection, cold pairs.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import subprocess
from datetime import datetime, timezone
from validate_cqtc_decode import geometry, sha
from validate_packed_contacts import project

IMAGE = Path('local/compatibility/grid-unpacked.exe')
GAME = Path(r'C:\Program Files (x86)\Steam\steamapps\common\Grid')
ATTACHMENT = Path('local/behavior/packed-mesh-contact-lacetti-20261012-final-01/validation.json')
TABLE = Path('local/body-contact-directions.bin')


def inputs():
    report = json.loads(ATTACHMENT.read_text())
    attachment = Path(report['attachment'])
    assert sha(attachment) == report['attachment_sha256']
    seed_path = attachment.parent / f"{report['attachment_index']:03}-original.bin"
    seed = seed_path.read_bytes()
    assert sha(seed_path) == report['seed_sha256']
    row = json.loads(attachment.read_text())['results'][report['attachment_index']]
    assert row['source'] == r'cars\lac\lac.ned'
    for k in ['source', 'csv']: assert sha(GAME / row[k]) == row[k + '_sha256']
    for route in range(4):
        source = GAME / f'tracks/europe/donington/route_{route}/boundarylines.cqtc'
        chunk = source.read_bytes()
        decoded, counts = geometry(chunk)
        for sample in range(32):
            triangle = sample * (counts['triangles'] - 1) // 31
            ids = struct.unpack_from('<3I', decoded, counts['vertices'] * 16 + triangle * 16)
            points = [struct.unpack_from('<4f', decoded, i * 16) for i in ids]
            for height in [-0.5, 0., 0.5, 4.]:
                center = [sum(p[j] for p in points) / 3 for j in range(3)]
                center[1] += height
                for sheet in [0, 1]:
                    controls = [(16, 1, 4096)]
                    if sample == 0 and height == 0:
                        controls += [(1, 1, 4096), (15, 1, 4096), (0, 1, 4096), (16, 0, 4096), (16, 1, 0), (16, 1, 1), (16, 1, 15), (16, 1, 16)]
                    for limit, cache, candidates in controls:
                        request = struct.pack('<6I4fI', 0x31304343, len(seed), len(chunk), limit, cache, candidates, *center, 1., sheet) + seed + chunk
                        yield dict(route=route, triangle=triangle, height=height, sheet=sheet, limit=limit, cache=cache, candidates=candidates, source=str(source), source_sha256=sha(source)), request


def execute(args, good=True):
    p = subprocess.run([str(x) for x in args], capture_output=True, text=True, timeout=60)
    assert (p.returncode == 0) == good, (args, p.returncode, p.stdout, p.stderr)
    return dict(exit=p.returncode, stdout=p.stdout, stderr=p.stderr)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('output', type=Path)
    ap.add_argument('--native', type=Path, default=Path('local/behavior/cqtc-contact-probe.exe'))
    ap.add_argument('--debug', type=Path, default=Path('target/debug/grid-cqtc-contacts.exe'))
    ap.add_argument('--release', type=Path, default=Path('target/release/grid-cqtc-contacts.exe'))
    ap.add_argument('--max-cases', type=int, default=1088)
    a = ap.parse_args()
    assert a.output.parts[0] == 'local' and 1 <= a.max_cases <= 1088
    a.output.mkdir(exist_ok=False, parents=True)
    from extract_contact_directions import extract
    assert extract()[0] == TABLE.read_bytes()
    assert sha(IMAGE) == 'aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514'
    pinned = {str(p): sha(p) for p in [a.native, a.debug, a.release, IMAGE, TABLE, ATTACHMENT]}
    rows = []
    for index, (meta, request) in enumerate(inputs()):
        if index == a.max_cases: break
        inp = a.output / f'{index:04}.input'; inp.write_bytes(request)
        answers, executions, outputs = [], {}, {}
        for kind in ['original', 'repeat']:
            target = a.output / f'{index:04}-{kind}.bin'
            executions[kind] = execute([a.native, IMAGE, inp, target])
            assert not executions[kind]['stderr'] and 'code_unchanged=1 globals_restored=1 code_replacements=0 error=0' in executions[kind]['stdout']
            answers.append(target.read_bytes()); outputs[kind] = dict(path=str(target), sha256=sha(target))
        assert answers[0] == answers[1]
        assert len(answers[0]) == 0x44f8 + meta['candidates'] * 8
        expected = project(answers[0] + bytes(64), meta['candidates'])
        for kind, exe in [('debug', a.debug), ('release', a.release)]:
            target = a.output / f'{index:04}-{kind}.bin'
            executions[kind] = execute([exe, inp, target, TABLE])
            actual = target.read_bytes()
            diff = next((i for i, (x, y) in enumerate(zip(expected, actual)) if x != y), None)
            assert actual == expected, (index, meta, 'first difference', diff, len(actual), len(expected))
            outputs[kind] = dict(path=str(target), sha256=sha(target))
        count = struct.unpack_from('<I', expected, 8240 + meta['candidates'] * 8)[0]
        accepted, visited = struct.unpack_from('<2I', expected, 32)
        rows.append(dict(index=index, **meta, request=str(inp), request_sha256=sha(inp), outputs=outputs, executions=executions, contacts=count, accepted=accepted, visited=visited, complete_bytes=len(answers[0]), projected_bytes=len(expected)))
        if (index + 1) % 100 == 0: print(f'cases={index + 1}', flush=True)
    first = (a.output / '0000.input').read_bytes()
    guards = []
    mutants = [('short', first[:43]), ('long', first + b'\x00')]
    for label, offset, value in [('tag', 0, 0), ('limit', 12, 17), ('cache', 16, 2), ('candidates', 20, 32769), ('sheet', 40, 2), ('center-nan', 24, 0x7fc00000)]:
        b = bytearray(first); struct.pack_into('<I', b, offset, value); mutants.append((label, b))
    ns = struct.unpack_from('<I', first, 4)[0]; start = 44 + ns
    no = struct.unpack_from('<I', first, start + 40)[0]
    b = bytearray(first); b[start + no:start + no + 3] = bytes(3); mutants.append(('cycle', b))
    for label, request in mutants:
        inp = a.output / f'guard-{label}.input'; inp.write_bytes(request)
        for kind, exe in [('native', a.native), ('debug', a.debug), ('release', a.release)]:
            target = a.output / f'guard-{label}-{kind}.bin'
            command = [exe, IMAGE, inp, target] if kind == 'native' else [exe, inp, target, TABLE]
            execute(command, False); assert not target.exists()
            guards.append(kind + ':' + label)
    target = a.output / 'existing.bin'; target.write_bytes(b'KEEP')
    for kind, exe in [('native', a.native), ('debug', a.debug), ('release', a.release)]:
        command = [exe, IMAGE, a.output / '0000.input', target] if kind == 'native' else [exe, a.output / '0000.input', target, TABLE]
        execute(command, False); assert target.read_bytes() == b'KEEP'; guards.append(kind + ':existing')
    assert all(sha(p) == digest for p, digest in pinned.items())
    report = dict(passed=True, time_utc=datetime.now(timezone.utc).isoformat(), cases=len(rows), contacts=sum(r['contacts'] for r in rows), positive=sum(r['contacts'] != 0 for r in rows), complete_bytes_per_native_run=sum(r['complete_bytes'] for r in rows), projected_bytes_per_rust_profile=sum(r['projected_bytes'] for r in rows), inputs=pinned, rows=rows, guards=guards, scope='whole original cold CQTC kind-10 dispatch versus source-only shared Rust candidate/query/manifold path; no full world or damage equivalence')
    (a.output / 'validation.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({k: report[k] for k in ['passed', 'cases', 'contacts', 'positive', 'complete_bytes_per_native_run', 'projected_bytes_per_rust_profile']}))


if __name__ == '__main__': main()
