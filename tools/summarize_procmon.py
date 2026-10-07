"""Stream private Procmon CSV evidence; filter GRID events and map resource reads."""
import argparse
import collections
import csv
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
NORMAL_RESULTS = {'SUCCESS', 'REPARSE', 'BUFFER OVERFLOW', 'NO MORE FILES',
                  'NO MORE ENTRIES', 'END OF FILE'}


def summarize(source, out, pids, game_root=None):
    out = out.resolve()
    if not out.is_relative_to((ROOT / 'local').resolve()):
        raise ValueError('Output must remain under project local/')
    outputs = [out / 'grid-events.private.csv', out / 'procmon-summary.json']
    if any(path.exists() for path in outputs):
        raise FileExistsError('Refusing to replace existing evidence')
    out.mkdir(parents=True, exist_ok=True)
    counts, operations, failures = (collections.Counter() for _ in range(3))
    accesses = collections.OrderedDict()
    observed_pids = set()
    total = kept = 0
    last_events = collections.deque(maxlen=30)
    with source.open('r', encoding='utf-8-sig', newline='') as f:
        reader = csv.DictReader(f)
        required = {'Process Name', 'PID', 'Result', 'Operation', 'Path', 'Time of Day'}
        if not required.issubset(reader.fieldnames or []):
            raise ValueError('Missing required Procmon CSV columns')
        with outputs[0].open('x', encoding='utf-8', newline='') as dest:
            writer = csv.DictWriter(dest, fieldnames=reader.fieldnames)
            writer.writeheader()
            for row in reader:
                total += 1
                if row['Process Name'].lower() != 'grid.exe':
                    continue
                if pids and row['PID'] not in pids:
                    continue
                kept += 1
                writer.writerow(row)
                observed_pids.add(row['PID'])
                counts[row['Result']] += 1
                operations[row['Operation']] += 1
                last_events.append(row)
                if row['Result'] not in NORMAL_RESULTS:
                    failures[(row['Operation'], row['Result'], row['Path'])] += 1
                if row['Operation'] in ('CreateFile', 'ReadFile', 'Load Image') and row['Result'] == 'SUCCESS':
                    access = accesses.setdefault(row['Path'], dict(
                        first_time=row['Time of Day'], last_time=row['Time of Day'],
                        create_events=0, read_events=0, image_events=0))
                    access['last_time'] = row['Time of Day']
                    access[{'CreateFile': 'create_events', 'ReadFile': 'read_events',
                            'Load Image': 'image_events'}[row['Operation']]] += 1
    resources = []
    game_root = game_root.resolve() if game_root else None
    if game_root:
        for name, access in accesses.items():
            path = Path(name)
            if not path.is_relative_to(game_root) or not path.is_file():
                continue
            resources.append(dict(relative_path=str(path.relative_to(game_root)),
                                  bytes=path.stat().st_size, **access))
            with path.open('rb') as resource:
                resources[-1]['sha256'] = hashlib.file_digest(resource, 'sha256').hexdigest()
    summary = dict(source=str(source.resolve()), all_events=total, grid_events=kept,
                   requested_pids=sorted(pids), pids=sorted(observed_pids),
                   game_root=str(game_root) if game_root else None,
                   results=dict(counts), operations=dict(operations),
                   failures=[dict(operation=k[0], result=k[1], path=k[2], count=v)
                             for k, v in failures.most_common()],
                   successful_accesses=[dict(path=k, **v) for k, v in accesses.items()],
                   resources=resources, last_events=list(last_events),
                   caveat='Missing probes can be normal fallback behavior. Successful CreateFile '
                          'does not prove a payload read; check read_events. Cached resources '
                          'and captures starting after loading make dependency lists incomplete.')
    with outputs[1].open('x', encoding='utf-8') as dest:
        json.dump(summary, dest, indent=2)
    return {k: summary[k] for k in ('all_events', 'grid_events', 'pids', 'results', 'operations')} | {
        'resource_files': len(resources), 'summary': str(outputs[1])}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, default=ROOT / 'local/phase1/startup-all.private.csv')
    parser.add_argument('--out-dir', type=Path, default=ROOT / 'local/phase1/resummarized')
    parser.add_argument('--pid', action='append', default=[], help='Retain only this GRID PID; repeatable')
    parser.add_argument('--game-root', type=Path)
    args = parser.parse_args()
    print(json.dumps(summarize(args.source, args.out_dir, set(args.pid), args.game_root), indent=2))


if __name__ == '__main__':
    main()
