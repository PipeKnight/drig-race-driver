"""Summarize private version-1 driving telemetry without inferring a lap."""
import argparse
import ast
import hashlib
import json
import math
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FIELDS = re.compile(r'(\w+)=((?:\[[^\]]*\])|\S+)')


def main():
    p = argparse.ArgumentParser()
    p.add_argument('log', type=Path)
    p.add_argument('output', type=Path)
    a = p.parse_args()
    source, target = a.log.resolve(), a.output.resolve()
    private = (ROOT / 'local').resolve()
    assert source.is_relative_to(private) and target.is_relative_to(private)
    raw = source.read_bytes()
    rows = []
    for line in raw.decode('utf-8', errors='strict').splitlines():
        if not line.startswith('drive_sample '):
            continue
        row = dict(FIELDS.findall(line))
        assert row['version'] == '1'
        for k in ['segment', 'tick', 'gear', 'wheel_hits']:
            row[k] = int(row[k])
        for k in ['seconds', 'wall_seconds', 'throttle', 'brake', 'steering']:
            row[k] = float(row[k])
            assert math.isfinite(row[k])
        for k, n in [('pose', 16), ('velocity', 4), ('wheel_brakes', 4)]:
            row[k] = ast.literal_eval(row[k])
            assert len(row[k]) == n and all(math.isfinite(v) for v in row[k])
        assert row['handbrake'] in ['true', 'false']
        row['handbrake'] = row['handbrake'] == 'true'
        row['speed_mps'] = math.sqrt(sum(v*v for v in row['velocity'][:3]))
        rows.append(row)
    assert rows
    episodes = []
    episode = None
    for row in rows:
        if episode and (row['segment'] != episode[-1]['segment'] or not row['handbrake']):
            episodes.append(episode)
            episode = None
        if row['handbrake']:
            if episode is None:
                episode = []
            episode.append(row)
    if episode:
        episodes.append(episode)
    result = dict(
        passed=True, source=str(source), source_sha256=hashlib.sha256(raw).hexdigest(),
        script_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        samples=len(rows), segments=sorted({r['segment'] for r in rows}),
        first=rows[0], last=rows[-1],
        failures=[r for r in rows if r['event'] == 'failure'],
        reverse_samples=sum(r['gear'] == 9 for r in rows),
        handbrake_episodes=[dict(samples=len(e), first=e[0], last=e[-1],
            sampled_duration_seconds=e[-1]['seconds']-e[0]['seconds'],
            speed_change_mps=e[-1]['speed_mps']-e[0]['speed_mps'],
            throttle_released=all(r['throttle'] == 0 for r in e),
            all_samples_have_brake_force=all(any(v > 0 for v in r['wheel_brakes']) for r in e))
            for e in episodes],
        limitation='Sampled observations only; no causal attribution, lap/checkpoint inference or original-game calibration.')
    with target.open('x', encoding='utf-8') as f:
        json.dump(result, f, indent=2)
    print(json.dumps({k: result[k] for k in ['passed', 'samples', 'segments', 'reverse_samples', 'failures']}))
    for e in result['handbrake_episodes']:
        print(json.dumps({k: e[k] for k in ['samples', 'sampled_duration_seconds', 'speed_change_mps', 'throttle_released', 'all_samples_have_brake_force']}))


if __name__ == '__main__':
    main()
