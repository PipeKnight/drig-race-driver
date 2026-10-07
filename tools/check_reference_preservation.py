"""Recheck traced Steam files against the saved reference baseline, read-only."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path


def digest(path):
    with path.open('rb') as handle:
        return hashlib.file_digest(handle, 'sha256').hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('baseline', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    if not args.output.resolve().is_relative_to((root / 'local').resolve()):
        raise ValueError('Evidence output must remain under local/')
    baseline = json.loads(args.baseline.read_text(encoding='utf-8-sig'))
    steam = Path(baseline['steam_root']).resolve(strict=True)
    rows = []
    for row in baseline['resources']:
        path = (steam / row['relative_path']).resolve(strict=True)
        if not path.is_relative_to(steam) or not path.is_file():
            raise ValueError('Baseline resource escapes reference directory')
        actual = digest(path)
        rows.append({'relative_path': row['relative_path'], 'sha256': actual,
                     'matches': actual == row['steam_sha256']})
    executable = digest(steam / 'grid.exe')
    report = {
        'checked_utc': datetime.now(timezone.utc).isoformat(), 'baseline': str(args.baseline),
        'resources': rows, 'resource_count': len(rows), 'all_resources_match': all(row['matches'] for row in rows),
        'original_executable_sha256': executable,
        'original_executable_matches_baseline': executable == baseline['original_executable_sha256'],
    }
    with args.output.open('x', encoding='utf-8') as handle:
        json.dump(report, handle, indent=2)
    print(json.dumps({name: report[name] for name in ('checked_utc', 'resource_count', 'all_resources_match', 'original_executable_matches_baseline')}))
    if not report['all_resources_match'] or not report['original_executable_matches_baseline']:
        raise SystemExit('Reference differs from baseline; report preserved')


if __name__ == '__main__':
    main()
