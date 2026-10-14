"""Check DT01 reader bounds/identity without input or game-state changes."""
import argparse
import hashlib
import json
import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def main():
    p = argparse.ArgumentParser()
    p.add_argument('session', type=Path)
    p.add_argument('controller', type=lambda v: int(v, 0))
    p.add_argument('other_controller', type=lambda v: int(v, 0))
    p.add_argument('existing', type=Path)
    p.add_argument('output', type=Path)
    a = p.parse_args()
    for path in (a.session, a.existing, a.output):
        assert path.resolve().is_relative_to((ROOT / 'local').resolve())
    assert not a.output.exists()
    a.output.mkdir()
    before = hashlib.sha256(a.existing.read_bytes()).hexdigest()
    original = json.loads(a.session.read_text(encoding='utf-8-sig'))
    wrong_start = a.output / 'wrong-start.json'
    wrong_hash = a.output / 'wrong-hash.json'
    wrong_start.write_text(json.dumps({**original, 'StartedUtc': '2020-01-01T00:00:00Z'}))
    wrong_hash.write_text(json.dumps({**original, 'ExeSHA256': '0' * 64}))
    cases = [('short', a.session, a.controller, 499, None),
             ('long', a.session, a.controller, 30001, None),
             ('zero-controller', a.session, 0, 500, None),
             ('large-controller', a.session, 0x80000000, 500, None),
             ('unmapped', a.session, 0x20000, 500, None),
             ('interior', a.session, a.controller + 4, 500, None),
             ('non-player', a.session, a.other_controller, 500, None),
             ('wrong-start', wrong_start, a.controller, 500, None),
             ('wrong-hash', wrong_hash, a.controller, 500, None),
             ('outside', a.session, a.controller, 500, ROOT / 'driving-guard-must-not-exist.tsv'),
             ('existing', a.session, a.controller, 500, a.existing)]
    outcomes = []
    reasons = {'short': ['DurationMs'], 'long': ['DurationMs'],
               'zero-controller': ['Controller outside bound'],
               'large-controller': ['Controller outside bound'],
               'unmapped': ['Owned read failed', 'Player Lacetti controller mismatch'],
               'interior': ['Player Lacetti controller mismatch'],
               'non-player': ['Player Lacetti controller mismatch'],
               'wrong-start': ['Private process identity mismatch'],
               'wrong-hash': ['Private executable identity mismatch'],
               'outside': ['New private local output'], 'existing': ['New private local output']}
    for name, session, controller, duration, path in cases:
        path = path or a.output / (name + '.tsv')
        if name != 'existing':
            assert not path.exists()
        result = subprocess.run([shutil.which('pwsh'), '-NoProfile', '-File',
                                 str(ROOT / 'tools/capture_grid_driving.ps1'),
                                 '-SessionJson', str(session.resolve()),
                                 '-ControllerAddress', str(controller), '-DurationMs', str(duration),
                                 '-Output', str(path.resolve())], cwd=ROOT, capture_output=True,
                                text=True, encoding='utf-8', errors='replace', timeout=15)
        (a.output / (name + '.txt')).write_text(result.stdout + result.stderr, encoding='utf-8')
        assert result.returncode != 0 and 'event=started' not in result.stdout, name
        assert any(reason in result.stderr for reason in reasons[name]), (name, result.stderr)
        if name != 'existing':
            assert not path.exists(), name
        outcomes.append(dict(case=name, rejected_before_capture=True, returncode=result.returncode))
    assert hashlib.sha256(a.existing.read_bytes()).hexdigest() == before
    report = dict(passed=True, cases=outcomes, existing_sha256_preserved=before,
                  script_sha256=hashlib.sha256((ROOT/'tools/capture_grid_driving.ps1').read_bytes()).hexdigest())
    (a.output / 'validation.json').write_text(json.dumps(report, indent=2))
    print(json.dumps(dict(passed=True, rejected=len(outcomes), existing_preserved=True)))


if __name__ == '__main__':
    main()
