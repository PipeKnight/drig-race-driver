"""Audit private sampled-lap replay against a separately decoded gate oracle."""
import argparse
import ast
import json
import math
import struct
from pathlib import Path
from summarize_driving_log import FIELDS, ROOT
from replay_driving_lap import digest, number


def main():
    p = argparse.ArgumentParser()
    p.add_argument('debug', type=Path)
    p.add_argument('release', type=Path)
    p.add_argument('oracle', type=Path)
    p.add_argument('output', type=Path)
    a = p.parse_args()
    assert all(path.resolve().is_relative_to((ROOT/'local').resolve())
               for path in [a.debug, a.release, a.oracle, a.output])
    reports = [json.loads(path.read_text()) for path in [a.debug, a.release]]
    original = Path(reports[0]['source'])
    observations = []
    for line in original.read_text().splitlines():
        if not line.startswith('drive_sample '): continue
        row = dict(FIELDS.findall(line))
        if int(row['segment']) != reports[0]['segment']: continue
        assert row['event'] != 'failure'
        tick = int(row['tick'])
        if row['event'] == 'tick' or (row['event'] == 'exit' and tick > observations[-1][0]):
            observations.append((tick, ast.literal_eval(row['pose'])[12:15]))
    frames = [observations[0]]
    for (first, before), (last, after) in zip(observations, observations[1:]):
        for tick in range(first+1, last+1):
            fraction = (tick-first)/(last-first)
            position = [struct.unpack('<f', struct.pack('<f', x+fraction*(y-x)))[0]
                        for x, y in zip(before, after)]
            frames.append((tick, position))
    raw = b'LP01'+struct.pack('<I', len(frames))+b''.join(
        struct.pack('<Q3f', tick, *position) for tick, position in frames)
    outputs = []
    for profile, path, report in zip(['debug', 'release'], [a.debug, a.release], reports):
        assert report['profile'] == profile and not report['all_progress_gates']
        assert digest(original) == report['source_sha256']
        assert digest(ROOT/'tools/replay_driving_lap.py') == report['script_sha256']
        assert digest(ROOT/f'target/{profile}/grid-lap-replay.exe') == report['executable_sha256']
        for name, identity in report['implementation_sha256'].items(): assert digest(ROOT/name) == identity
        assert (path.parent/'input.bin').read_bytes() == raw
        for name, identity in report['files'].items():
            file = path.parent/name
            assert file.stat().st_size == identity['size'] and digest(file) == identity['sha256']
        first = (path.parent/'first.tsv').read_bytes()
        assert first == (path.parent/'repeat.tsv').read_bytes()
        outputs.append(first)
    assert outputs[0] == outputs[1]
    oracle = json.loads(a.oracle.read_text())
    assert oracle['input_sha256'] == digest(a.release.parent/'input.bin')
    assert oracle['script_sha256'] == digest(ROOT/'tools/inspect_lap_crossings.py')
    assert oracle['progress_sha256'] == reports[0]['progress_sha256'] == reports[1]['progress_sha256']
    crossings = sorted(oracle['accepted_source_split_crossings'], key=lambda r: (r['tick'], r['fraction']))
    assert [r['gate'] for r in crossings] == [0, 22, 45, 0]
    delta = struct.unpack('<f', struct.pack('<f', .016))[0]
    times = [(r['tick']-1+r['fraction'])*delta for r in crossings]
    rows = [line.split('\t') for line in outputs[0].decode().splitlines()]
    start = [r for r in rows if r[0] == 'T' and r[2] == 'START']
    gate = [r for r in rows if r[0] == 'T' and r[2] == 'GATE']
    lap, = [r for r in rows if r[0] == 'T' and r[2] == 'LAP']
    assert [int(r[1]) for r in start] == [crossings[0]['tick'], crossings[-1]['tick']]
    assert [int(r[3]) for r in gate] == [22, 45]
    for row, expected in zip(start, [times[0], times[-1]]):
        assert math.isclose(number(row[3]), expected, rel_tol=0, abs_tol=2e-12)
    for row, expected in zip(gate, times[1:3]):
        assert math.isclose(number(row[4]), expected, rel_tol=0, abs_tol=2e-12)
    assert int(lap[1]) == crossings[-1]['tick'] and lap[3:5] == ['1', '1'] and lap[6] == 'none'
    seconds = times[-1]-times[0]
    sectors = [b-a for a, b in zip(times, times[1:])]
    assert math.isclose(number(lap[5]), seconds, rel_tol=0, abs_tol=2e-12)
    assert all(math.isclose(number(word), expected, rel_tol=0, abs_tol=2e-12)
               for word, expected in zip(lap[7].split(','), sectors))
    result = dict(passed=True, report_sha256=[digest(a.debug), digest(a.release)],
                  oracle_sha256=digest(a.oracle), script_sha256=digest(Path(__file__)),
                  observed_samples=len(observations), interpolated_ticks=len(frames),
                  profiles_and_repeats_exact=True, independent_crossing_times_agree=True,
                  maximum_tolerance_seconds=2e-12, lap_seconds=seconds, sectors=sectors,
                  scope='Sampled independent prototype path, linear interpolation and explicit practice policy; not original race-rule equivalence or a new physical lap.')
    with a.output.open('x') as file: json.dump(result, file, indent=2)
    print(json.dumps(result))


if __name__ == '__main__': main()
