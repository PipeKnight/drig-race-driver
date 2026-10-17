"""Check private actual-physics practice traces and complete final car words."""
import argparse
import hashlib
import json
import math
import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def sha(path): return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def inspect(path, trial_laps=None):
    assert path.resolve().is_relative_to((ROOT/'local').resolve()) and path.stat().st_size <= 16*1024*1024
    rows = [line.split('\t') for line in path.read_text().splitlines()]
    header = rows[0]
    assert header[:2] == ['P', 'independent-pure-pursuit'] and len(header) == 5
    summary, = [r for r in rows if r[0] == 'S']
    damage, = [r for r in rows if r[0] == 'D']
    samples = [r for r in rows if r[0] == 'T']
    events = [r for r in rows if r[0] == 'E']
    expected_laps = trial_laps or 1
    assert int(summary[1]) <= int(header[2]) and int(summary[2]) == expected_laps
    trial = [r for r in rows if r[0] == 'R']
    laps = [r for r in rows if r[0] == 'L']
    if trial_laps is None:
        assert not trial and not laps
    else:
        assert 1 <= trial_laps <= 9 and len(trial) == 1 and len(laps) == trial_laps
        assert len(trial[0]) == 5 and int(trial[0][1]) == trial_laps and trial[0][4] == '0'
        assert all(len(r) == 4 and int(r[1]) == i+1 and r[3] == 'true' and math.isfinite(float(r[2])) and float(r[2]) > 0 for i,r in enumerate(laps))
        seconds = float(trial[0][2])
        times = [float(r[2]) for r in laps]
        assert math.isfinite(seconds) and sum(times) <= seconds <= int(summary[1])*struct.unpack('<f', struct.pack('<f', .016))[0]
        assert abs(float(trial[0][3])-min(times)) <= 1e-8 and abs(float(summary[3])-min(times)) <= 1e-8
    assert summary[4] == summary[7] == 'none' and int(summary[5]) >= 0
    assert len(samples) > 2 and all(len(r) == 16 for r in samples)
    assert all(int(a[1]) < int(b[1]) for a, b in zip(samples, samples[1:]))
    assert samples[-1][1] == summary[1]
    for row in samples:
        assert all(math.isfinite(float(row[i])) for i in [2,3,4,5,6,7,13,14,15])
        assert 0 <= float(row[5]) <= 1 and 0 <= float(row[6]) <= 1 and -1 <= float(row[7]) <= 1
        assert row[12] == 'none'
    vehicle, = [r for r in rows if r[0] == 'V']
    physical, = [r for r in rows if r[0] == 'B']
    assert len(vehicle) == 1+0x21000//4 and len(physical) == 1+0x424//4
    words = [int(word, 16) for word in vehicle[1:]]
    complete = struct.pack('<'+'I'*(len(words)+len(physical)-1), *words, *[int(word,16) for word in physical[1:]])
    dimensions = words[0xc714//4:0xc714//4+3]
    assert all(2 <= d <= 16 for d in dimensions) and math.prod(dimensions) <= 400
    moved = sum(any(words[0x4880//4+i*20+j] != words[0x4890//4+i*20+j] for j in range(3))
                for i in range(math.prod(dimensions)))
    assert words[0xd234//4] == 7 and len(damage) == 6
    assert len([r for r in events if 'Finished(Completed' in r[2]]) == expected_laps
    assert all('valid: true' in r[2] for r in events if 'Finished(Completed' in r[2])
    return dict(path=str(path), sha256=sha(path), bytes=path.stat().st_size,
                requested_maximum_speed=float(header[3]), steering_gain=float(header[4]),
                ticks=int(summary[1]), valid_laps=int(summary[2]), lap_seconds=float(summary[3]),
                trial_event_seconds=float(trial[0][2]) if trial else None,
                trial_lap_seconds=[float(r[2]) for r in laps],
                body_contacts=int(summary[5]), maximum_all_tick_lateral_error=float(summary[6]),
                minimum_all_tick_wheel_hits=int(damage[5]), bulk_calls=int(damage[1]), bulk_records=int(damage[2]),
                crt_seed=damage[3], damage_rng=damage[4], moved_grid_positions=moved,
                sampled_maximum_speed_m_s=max(float(r[4]) for r in samples),
                sampled_records=len(samples), timing_event_records=len(events),
                complete_final_car_physical_bytes=len(complete), complete_final_car_physical_sha256=hashlib.sha256(complete).hexdigest())


def main():
    p = argparse.ArgumentParser()
    p.add_argument('output', type=Path)
    p.add_argument('traces', nargs='+', type=Path)
    p.add_argument('--trial-laps', type=int)
    a = p.parse_args()
    assert a.output.resolve().is_relative_to((ROOT/'local').resolve()) and 1 <= len(a.traces) <= 12
    runs = [inspect(path, a.trial_laps) for path in a.traces]
    repeated = {}
    for speed in {r['requested_maximum_speed'] for r in runs}:
        group = [r for r in runs if r['requested_maximum_speed'] == speed]
        assert len({r['sha256'] for r in group}) == 1
        repeated[str(speed)] = dict(runs=len(group), comparison_available=len(group)>1,
                                   trace_bytes_exact=True if len(group)>1 else None,
                                   complete_final_words_exact=True if len(group)>1 else None)
    identities = {str(path.relative_to(ROOT)): sha(path) for path in [
        ROOT/'crates/grid-probes/src/bin/grid-practice-drive.rs', ROOT/'crates/grid-tools/src/source_vehicle.rs',
        ROOT/'crates/grid-world/src/lap_timing.rs', ROOT/'crates/grid-world/src/time_trial.rs',
        ROOT/'crates/grid-tools/src/time_trial.rs', ROOT/'local/body-contact-directions.bin',
        ROOT/'local/body-impact-parameters.bin', ROOT/'local/deformation-parameters.bin',
        ROOT/'target/debug/grid-practice-drive.exe', ROOT/'target/release/grid-practice-drive.exe']}
    report = dict(passed=True, runs=runs, repeated=repeated, current_identities=identities,
                  script_sha256=sha(__file__),
                  scope='Independent conservative diagnostic controller over purchased centerline. Normal car/ground/physical/contact/damage/timing paths; no teleports or replayed poses. Equality covers formatted sampled traces/events and complete final car/physical words, not every intermediate world byte or original AI/handling fidelity.')
    with a.output.open('x') as file: json.dump(report, file, indent=2)
    print(json.dumps(dict(passed=True, runs=len(runs), repeated=repeated)))


if __name__ == '__main__': main()
