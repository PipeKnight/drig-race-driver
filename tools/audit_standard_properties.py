"""Audit complete-property evidence and strict diagnostic rejection boundaries.

Native cell framing and Rust source validation are separate contracts. Rejected
Rust numeric inputs do not establish original CRT parser behavior.
"""
import argparse
import csv
import io
import json
import subprocess
from pathlib import Path
import validate_standard_properties as v


def main():
    p = argparse.ArgumentParser()
    p.add_argument('output', type=Path)
    p.add_argument('reports', nargs=2, type=Path)
    a = p.parse_args()
    out = a.output.resolve()
    assert out.is_relative_to((v.ROOT / 'local').resolve())
    out.mkdir(exist_ok=False)
    proofs = []
    profiles = set()
    for path in a.reports:
        r = json.loads(path.read_text())
        assert r['native_reference_passed'] and r['rust_equivalence'] and not r['differences']
        profiles.add(r['profile'])
        assert r['sources'] == 44 and r['authored'] == 423
        assert r['properties_bytes'] == 467 * 0x65c
        for key in ['exe', 'rust_exe', 'baseline_path']:
            hash_key = 'baseline_sha256' if key == 'baseline_path' else key + '_sha256'
            assert v.digest(v.ROOT / r[key]) == r[hash_key]
        assert len(r['executions']) == 467 * 3
        for run in r['executions']:
            assert run['exit'] == 0 and not run['stderr']
            if run['kind'] != 'rust':
                assert 'data_restored=1 code_restored=1 error=0' in run['stdout']
        for i, row in enumerate(r['results']):
            assert v.digest(v.GAME / row['source']) == row['source_sha256']
            cells = path.parent / f'{i:03}-cells.bin'
            assert v.digest(cells) == row['cells_sha256']
            answers = [path.parent / f'{i:03}-{kind}.bin' for kind in ['original', 'repeat', 'rust']]
            assert all(q.stat().st_size == 0x65c and v.digest(q) == row['output_sha256'] for q in answers)
            assert row['repeat_exact'] and row['properties_bytes'] == 0x65c
        proofs.append(dict(path=str(path), sha256=v.digest(path), profile=r['profile'], records=467, bytes=r['properties_bytes']))
    assert profiles == {'debug', 'release'}
    image = v.ROOT / 'local/compatibility/grid-unpacked.exe'
    assert v.digest(image) == 'aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514'
    seed = (a.reports[0].parent / '000-cells.bin').read_bytes()
    invalid = [('short', seed[:-1]), ('long', seed + b'\0'),
               ('unterminated', b'A' * 64 + seed[64:]),
               ('nonascii', bytes([128]) + seed[1:]),
               ('empty-key', b'\0' * 64 + seed[64:])]
    guards = []

    def reject(label, command, target, native=False):
        run = subprocess.run(command, cwd=v.ROOT, capture_output=True, text=True, timeout=30)
        assert run.returncode != 0 and not target.exists(), (label, run, target)
        if native:
            assert 'data_restored=1 code_restored=1' in run.stdout and not run.stderr
        else:
            assert 'grid-standard-properties:' in run.stderr
        guards.append(dict(label=label, exit=run.returncode, stdout=run.stdout, stderr=run.stderr, no_output=True))

    for label, data in invalid:
        source = out / f'native-{label}.bin'
        source.write_bytes(data)
        target = out / f'native-{label}-output.bin'
        reject('native-' + label, [str(v.ROOT / 'local/behavior/standard-properties-probe.exe'), str(image), str(source), str(target)], target, True)
    base = next(r for r in csv.reader((v.GAME / 'cars/lac/lac.csv').read_text().splitlines()) if r)[:360]
    changes = [('mass-zero', 3, '0'), ('mass-negative', 3, '-1'),
               ('nan', 4, 'NaN'), ('infinite', 4, 'inf'),
               ('overflow-f32', 4, '1e100'), ('integer-overflow', 147, '2147483648'),
               ('integer-fraction', 147, '1.25'), ('long-name', 1, 'A' * 64),
               ('unicode', 1, '\u0410'), ('nul', 1, 'A\0B'),
               ('wrong-model', 2, '6Wheeler'), ('revision-infinite', 49, 'inf')]
    sources = []
    for label, column, value in changes:
        row = base.copy()
        row[column] = value
        buf = io.StringIO(newline='')
        csv.writer(buf, lineterminator='\r\n').writerow(row)
        source = out / f'{label}.csv'
        source.write_bytes(buf.getvalue().encode('utf-8'))
        sources.append((label, source))
    for label, data in [('empty', b''), ('short-row', b'A,B,Standard,1\r\n')]:
        source = out / f'{label}.csv'
        source.write_bytes(data)
        sources.append((label, source))
    for profile in sorted(profiles):
        for label, source in sources:
            target = out / f'{profile}-{label}-output.bin'
            reject(profile + '-' + label, [str(v.ROOT / f'target/{profile}/grid-standard-properties.exe'), str(source), str(target)], target)
    result = dict(passed=True, proofs=proofs, guards=guards, guard_count=len(guards),
                  limits='Native cell framing and strict Rust guards separately checked; no original tokenizer/CRT malformed-number equivalence or complete car construction.')
    (out / 'audit.json').write_text(json.dumps(result, indent=2))
    print(json.dumps(dict(passed=True, records_per_profile=467, bytes_per_profile=467 * 0x65c, guards=len(guards))))


if __name__ == '__main__':
    main()
