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
    deformation_rows=[]
    lap_rows, lap_events, handling_rows = [], [], []
    for line in raw.decode('utf-8', errors='strict').splitlines():
        if line.startswith('drive_lap_sample '):
            row = dict(FIELDS.findall(line)); assert row['version'] == '1'
            for key in ['segment','tick','lap','next_gate','passed_gates','total_gates','required_gates','valid_laps','attempts']:
                row[key] = int(row[key]); assert row[key] >= 0
            for key in ['elapsed','best']:
                row[key] = None if row[key] == 'none' else float(row[key])
                assert row[key] is None or (math.isfinite(row[key]) and row[key] >= 0)
            assert row['status'] in ['waiting','running','invalid']
            lap_rows.append(row)
        if line.startswith('drive_lap_event '):
            row = dict(FIELDS.findall(line)); assert row['version'] == '1'
            for key in ['segment','tick','lap','gate','sector']:
                if key in row: row[key] = int(row[key]); assert row[key] >= 0
            for key in ['when','seconds']:
                if key in row: row[key] = float(row[key]); assert math.isfinite(row[key]) and row[key] >= 0
            assert row['type'] in ['start','checkpoint','sector','finish','position-jump']
            if row['type'] == 'finish':
                assert row['valid'] in ['true','false']; row['valid'] = row['valid'] == 'true'
                row['missed_gate'] = None if row['missed_gate'] == 'none' else int(row['missed_gate'])
                row['sectors'] = [None if s == 'none' else float(s) for s in row['sectors'].split(',')]
                assert all(s is None or (math.isfinite(s) and s >= 0) for s in row['sectors'])
            lap_events.append(row)
        if line.startswith('drive_handling_sample '):
            row = dict(FIELDS.findall(line)); assert row['version'] == '1'
            for key in ['segment','tick','status']: row[key] = int(row[key])
            row['terminal_wheel'] = None if row['terminal_wheel'] == 'none' else int(row['terminal_wheel'])
            assert row['terminal_wheel'] is None or 0 <= row['terminal_wheel'] <= 3
            for key, size in [('effects_3a0_3bc',8),('wheel_levels',4)]:
                row[key] = ast.literal_eval(row[key]); assert len(row[key]) == size and all(math.isfinite(v) for v in row[key])
            row['controls_disabled'] = float(row['controls_disabled']); assert row['controls_disabled'] in [0.,1.]
            handling_rows.append(row)
        if line.startswith('drive_deformation_sample '):
            row=dict(FIELDS.findall(line));assert row['version']=='1'
            for key in ['segment','tick','bulk_calls','records','mode','pending_geometry','moved_positions']:row[key]=int(row[key]);assert row[key]>=0
            assert row['pending_geometry']<=1 and row['moved_positions']<=400
            assert re.fullmatch(r'[0-9a-fA-F]{8}',row['crt_seed']);row['crt_seed']=int(row['crt_seed'],16)
            row['maximum_displacement']=float(row['maximum_displacement']);assert math.isfinite(row['maximum_displacement']) and row['maximum_displacement']>=0
            deformation_rows.append(row)
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
        for key in ['body_geometry', 'body_events', 'damage_rng_owned']:
            if key in row:
                assert row[key] in ['true', 'false']
                row[key] = row[key] == 'true'
        if 'body_contacts' in row:
            row['body_contacts'] = int(row['body_contacts'])
            assert row['body_contacts'] >= 0
        if 'damage_rng' in row:
            assert re.fullmatch(r'[0-9a-fA-F]{8}', row['damage_rng'])
            row['damage_rng'] = int(row['damage_rng'], 16)
            row['impact_flag'] = int(row['impact_flag'])
            assert 0 <= row['impact_flag'] <= 255
            row['suspension_gain'] = float(row['suspension_gain'])
            row['damage_levels'] = ast.literal_eval(row['damage_levels'])
            assert math.isfinite(row['suspension_gain'])
            assert len(row['damage_levels']) == 9 and all(math.isfinite(v) for v in row['damage_levels'])
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
    # Only successful tick records count contacts. Pause/exit/failure records
    # retain the preceding tick's count and must not add it a second time.
    ticks = {}
    for row in rows:
        if row['event'] == 'tick':
            key = (row['segment'], row['tick'])
            assert key not in ticks
            ticks[key] = row
    contact_runs = []
    contact_run = None
    previous = None
    sampled_path = 0.
    for row in ticks.values():
        if previous and previous['segment'] == row['segment']:
            assert row['tick'] > previous['tick']
            sampled_path += math.dist(row['pose'][12:15], previous['pose'][12:15])
        if row.get('body_contacts', 0):
            if contact_run and row['segment'] == contact_run['segment'] and row['tick'] == contact_run['last_tick'] + 1:
                contact_run['last_tick'] = row['tick']
                contact_run['last_seconds'] = row['seconds']
                contact_run['ticks'] += 1
                contact_run['records'] += row['body_contacts']
                contact_run['maximum_records_per_tick'] = max(contact_run['maximum_records_per_tick'], row['body_contacts'])
            else:
                contact_run = dict(segment=row['segment'], first_tick=row['tick'], last_tick=row['tick'],
                    first_seconds=row['seconds'], last_seconds=row['seconds'], ticks=1,
                    records=row['body_contacts'], maximum_records_per_tick=row['body_contacts'],
                    first_post_tick_speed_kmh=row['speed_mps'] * 3.6)
                contact_runs.append(contact_run)
        else:
            contact_run = None
        previous = row
    damage_rows = [r for r in ticks.values() if 'damage_rng' in r]
    damage = None
    if damage_rows:
        assert len(damage_rows) == len(ticks), 'Mixed damage telemetry within one log'
        transitions = [dict(segment=b['segment'], tick=b['tick'], seconds=b['seconds'],
                       before=f"{a['damage_rng']:08x}", after=f"{b['damage_rng']:08x}")
                       for a, b in zip(damage_rows, damage_rows[1:])
                       if a['segment'] == b['segment'] and a['damage_rng'] != b['damage_rng']]
        damage = dict(first_rng=f"{damage_rows[0]['damage_rng']:08x}",
            last_rng=f"{damage_rows[-1]['damage_rng']:08x}", observed_rng_transitions=transitions,
            first_levels=damage_rows[0]['damage_levels'], last_levels=damage_rows[-1]['damage_levels'],
            maximum_levels=[max(r['damage_levels'][i] for r in damage_rows) for i in range(9)],
            minimum_suspension_gain=min(r['suspension_gain'] for r in damage_rows),
            maximum_suspension_gain=max(r['suspension_gain'] for r in damage_rows),
            limitation='Observed RNG transitions are not draw/impact counts; a retained impact flag is not a new event. No exclusive cause is inferred.')
    result = dict(
        passed=True, source=str(source), source_sha256=hashlib.sha256(raw).hexdigest(),
        script_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        samples=len(rows), segments=sorted({r['segment'] for r in rows}),
        first=rows[0], last=rows[-1],
        failures=[r for r in rows if r['event'] == 'failure'],
        build_revision=next((line.split('revision=', 1)[1] for line in raw.decode('utf-8').splitlines()
                             if line.startswith('prototype_build revision=')), None),
        completed_exit=rows[-1]['event'] == 'exit',
        successful_tick_samples=len(ticks),
        maximum_sampled_speed_kmh=max(r['speed_mps'] for r in rows)*3.6,
        sampled_path_length_m=sampled_path,
        body_contacts=dict(logged_ticks=sum(r.get('body_contacts', 0) > 0 for r in ticks.values()),
            logged_records=sum(r.get('body_contacts', 0) for r in ticks.values()),
            runs=contact_runs, limitation='Contiguous logged contact ticks, not independently counted crashes; wheel-prefix callbacks are excluded from this geometry count.'),
        damage=damage,
        lap_timing=(dict(samples=len(lap_rows), first=lap_rows[0], last=lap_rows[-1], events=lap_events,
                        finished=[r for r in lap_events if r['type']=='finish'],
                        limitation='Emitted independent practice-timing events on successful simulation ticks; no inferred original race rules or original-game clock.') if lap_rows else None),
        handling=(dict(samples=len(handling_rows), first=handling_rows[0], last=handling_rows[-1],
                       terminal_samples=[r for r in handling_rows if r['terminal_wheel'] is not None],
                       limitation='Observed effect parameters and selected terminal state; no exclusive causal attribution.') if handling_rows else None),
        deformation=(dict(samples=len(deformation_rows),first=deformation_rows[0],last=deformation_rows[-1],maximum_moved_positions=max(r['moved_positions'] for r in deformation_rows),maximum_displacement=max(r['maximum_displacement'] for r in deformation_rows),limitation='Sampled grid/phase counters, not crashes, RNG draws or visual body dents; reset starts a new segment.') if deformation_rows else None),
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
