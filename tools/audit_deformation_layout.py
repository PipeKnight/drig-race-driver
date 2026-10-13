"""Reconstruct layout requests and recheck complete outputs/current identities."""
import argparse
import json
from pathlib import Path
import validate_deformation_layout as layout


def source_audit(path, report):
    import compare_source_deformation_layout as source
    out, rows, provenance = path.parent, [], []
    parameters, flag = layout.parameters()
    for stem in ['lac', 'bmw']:
        for detail in ['highLOD', 'lowLOD']:
            candidates, _ = source.candidates(stem, detail)
            for case in candidates:
                rows.append(dict(name=stem + '-' + case['name'], bounds=[case['bounds']],
                                 parameters=parameters, token=0, flag=flag, descriptor=bytes(layout.SIZE)))
                provenance.append(dict(stem=stem, **case))
    assert provenance == report['candidates']
    assert (out / 'source-candidates-input.bin').read_bytes() == layout.request(rows)
    for name, expected in report['files'].items():
        p = (out / name).resolve()
        assert p.parent == out and p.stat().st_size == expected['size']
        assert layout.digest(p) == expected['sha256']
    for name, sha in report['identities'].items():
        assert layout.digest(Path(name)) == sha
    for kind, run in report['executions'].items():
        exe = layout.PROBE if kind != 'rust' else layout.ROOT / f"target/{report['profile']}/grid-deformation-layout.exe"
        assert run['exit'] == 0 and layout.digest(exe) == run['executable_sha256']
    original = (out / 'source-candidates-original.bin').read_bytes()
    assert original == (out / 'source-candidates-repeat.bin').read_bytes() == (out / 'source-candidates-rust.bin').read_bytes()
    assert len(original) == 8 + len(rows) * layout.SIZE and report['descriptor_bytes'] == len(original) - 8
    assert report['matches'] == [r for r in report['observations'] if r['fields_equal'] and r['rest_exact']]
    assert all(r['source_identity_extent_words'] == 7 for r in report['observations'])
    return dict(path=str(path.relative_to(layout.ROOT)), sha256=layout.digest(path),
                cases=len(rows), descriptor_bytes=len(original) - 8,
                complete_files=len(report['files']), source_identities=len(report['identities']),
                matching_combinations=len(report['matches']), scope='Read-only candidate provenance audit; original selection remains unproved')


def audit(path):
    path = path.resolve()
    assert path.is_relative_to((layout.ROOT / 'local').resolve())
    report = json.loads(path.read_text())
    if 'observations' in report:
        return source_audit(path, report)
    out, rows = path.parent, layout.cases()
    assert len(rows) == report['cases'] and [r['name'] for r in rows] == report['names']
    for name, expected in report['files'].items():
        p = (out / name).resolve()
        assert p.parent == out and p.stat().st_size == expected['size']
        assert layout.digest(p) == expected['sha256'], str(p)
    for name, sha in report['executables'].items():
        assert layout.digest(layout.ROOT / name) == sha, name
    total = 0
    for start in range(0, len(rows), 64):
        batch = rows[start:start + 64]
        assert (out / f'batch-{start}-input.bin').read_bytes() == layout.request(batch)
        original = (out / f'batch-{start}-original.bin').read_bytes()
        assert original == (out / f'batch-{start}-repeat.bin').read_bytes() == (out / f'batch-{start}-rust.bin').read_bytes()
        assert len(original) == 8 + len(batch) * layout.SIZE
        for i, row in enumerate(batch):
            layout.independent_checks(row, original[8 + i * layout.SIZE:8 + (i + 1) * layout.SIZE])
        total += len(original) - 8
    assert total == report['bytes']
    assert json.loads((out / 'guards.json').read_text()) == report['guards']
    assert all(c['passed'] for c in report['guards'])
    for check in report['guards']:
        if 'executions' in check:
            for kind in ['original', 'repeat', 'rust']:
                assert not (out / f"guard-{check['name']}-{kind}.bin").exists()
    return dict(path=str(path.relative_to(layout.ROOT)), sha256=layout.digest(path), cases=len(rows),
                descriptor_bytes=total, complete_files=len(report['files']),
                current_identities=len(report['executables']), guard_cases=len(report['guards']))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('reports', type=Path, nargs='+')
    args = parser.parse_args()
    output = args.output.resolve()
    assert output.is_relative_to((layout.ROOT / 'local').resolve())
    result = dict(reports=[audit(p) for p in args.reports])
    with output.open('x') as handle:
        json.dump(result, handle, indent=2)
    print(json.dumps(result))


if __name__ == '__main__':
    main()
