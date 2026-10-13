"""Read-only reconstruction of full original/owned terminal handling evidence."""
import argparse
import json
import struct
from pathlib import Path
from validate_handling_terminal import fixtures, terminal_check, sha


def main():
    p = argparse.ArgumentParser()
    p.add_argument('report', type=Path)
    p.add_argument('output', type=Path)
    a = p.parse_args()
    assert all(path.resolve().is_relative_to(Path('local').resolve()) for path in [a.report, a.output])
    r = json.loads(a.report.read_text())
    assert r['passed'] and r['cases'] == 896
    for path, identity in r['identities'].items(): assert sha(path) == identity, path
    items = list(fixtures())
    stats, total, files = {}, 0, 0
    for i, record in enumerate(r['rows']):
        group = items[i*32:(i+1)*32]
        request = b'VH02'+struct.pack('<I', len(group))+b''.join(raw for _, raw in group)
        assert Path(record['request']).read_bytes() == request and sha(record['request']) == record['sha256']
        answers = []
        for result in record['outputs'].values():
            assert sha(result['path']) == result['sha256']
            answers.append(Path(result['path']).read_bytes())
        assert len(answers) == 4 and all(answer == answers[0] for answer in answers)
        found = terminal_check(request, answers[0])
        assert found == record['stats']
        for key, value in found.items(): stats[key] = stats.get(key, 0)+value
        total += len(answers[0])-8
        files += 5
    carried = r['carried']
    request = Path(carried['request']).read_bytes()
    assert request == Path(r['rows'][-1]['outputs']['original']['path']).read_bytes()
    answers = []
    for result in carried['outputs'].values():
        assert sha(result['path']) == result['sha256']
        answers.append(Path(result['path']).read_bytes())
    assert len(answers) == 4 and all(answer == answers[0] for answer in answers)
    assert terminal_check(request, answers[0]) == carried['stats']
    for guard in r['guards']:
        profile, name = guard.split(':')
        assert (a.report.parent/f'guard-{name}.input').is_file()
        assert not (a.report.parent/f'guard-{name}-{profile}.bin').exists()
    assert len(r['guards']) == 12 and total == r['bytes_per_profile'] == 122533376 and stats == r['stats']
    result = dict(passed=True, report_sha256=sha(a.report), primary_files=files+5,
                  current_identities=len(r['identities']), cases=len(items), bytes_per_profile=total,
                  rejection_outcomes=len(r['guards']), stats=stats, script_sha256=sha(__file__))
    with a.output.open('x') as file: json.dump(result, file, indent=2)
    print(json.dumps(result))


if __name__ == '__main__': main()
