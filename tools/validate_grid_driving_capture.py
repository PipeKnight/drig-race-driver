"""Inspect bounded private DT01 read-only controls/motion observations."""
import argparse
import hashlib
import json
import math
import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def inspect(path):
    assert path.resolve().is_relative_to((ROOT / 'local').resolve())
    raw = path.read_bytes()
    assert 0 < len(raw) <= 2 * 1024 * 1024
    rows = [r.split('\t') for r in raw.decode('utf-8').splitlines()]
    header, footer = rows[0], rows[-1]
    assert header[0] in ('DT01', 'DT02') and len(header) == (7 if header[0] == 'DT01' else 9)
    scheduled = header[0] == 'DT02'
    frequency = int(header[1])
    assert frequency > 0 and int(header[2]) > 0
    assert footer[0] == 'S' and len(footer) == 3
    samples = rows[1:-1]
    assert 1 <= len(samples) <= 3100 and int(footer[1]) == len(samples)
    records = []
    for i, r in enumerate(samples):
        assert len(r) == (9 if scheduled else 8) and r[0] == 'T' and int(r[1]) == i
        begin, end = int(r[2]), int(r[3])
        assert 0 < begin <= end and r[4] in ('0', '1')
        if records:
            assert begin >= records[-1]['end']
        controls, motion, engine = map(bytes.fromhex, r[5:8])
        assert (len(controls), len(motion), len(engine)) == (40, 96, 24)
        c = struct.unpack('<10f', controls)
        m = struct.unpack('<24f', motion)
        e = struct.unpack('<6I', engine)
        angular, scalar, maximum, idle = [struct.unpack('<f', struct.pack('<I', e[j]))[0]
                                          for j in (0, 2, 3, 4)]
        assert all(math.isfinite(v) for v in (*c, *m, angular, scalar, maximum, idle))
        assert 0 < idle < maximum and 0 <= angular <= 2 * maximum and e[1] <= 9
        assert all(-1e-5 <= c[j] <= 1.00001 for j in (0, 1, 8, 9))
        speed = math.sqrt(sum(v * v for v in m[20:23]))
        schedule = struct.unpack('<Iff', bytes.fromhex(r[8])) if scheduled else None
        if schedule:
            assert all(math.isfinite(v) and 0 <= v <= 1 for v in schedule[1:])
        records.append(dict(begin=begin, end=end, repeat=r[4] == '1', controls=c,
                            speed=speed, gear=e[1], position=m[12:15],
                            axis2_speed=sum(m[8+j]*m[20+j] for j in range(3)), schedule=schedule))
    assert int(footer[2]) >= records[-1]['end']
    duration = (int(footer[2]) - records[0]['begin']) / frequency
    assert 0.4 <= duration <= 30.25
    fields = {name: dict(min=min(r['controls'][at] for r in records),
                         max=max(r['controls'][at] for r in records))
              for name, at in [('e1c', 0), ('e20', 1), ('e30', 5), ('e34', 6),
                               ('e3c', 8), ('e40', 9)]}
    # An authored >.5 display classification; raw values remain in the capture.
    spans = []
    for r in records:
        signature = tuple(r['controls'][at] > .5 for at in (0, 1, 8, 9))
        when = (r['begin'] - records[0]['begin']) / frequency
        if not spans or spans[-1]['pedals_above_half'] != signature:
            spans.append(dict(pedals_above_half=signature, first_seconds=when,
                              last_seconds=when, first_speed_m_s=r['speed'],
                              last_speed_m_s=r['speed'], first_gear=r['gear'],
                              last_gear=r['gear'], observations=1))
        else:
            spans[-1].update(last_seconds=when, last_speed_m_s=r['speed'],
                             last_gear=r['gear'],
                             observations=spans[-1]['observations'] + 1)
    return dict(path=str(path), sha256=hashlib.sha256(raw).hexdigest(),
                header=header, records=len(records), duration_seconds=duration,
                controls_repeat_exact=sum(r['repeat'] for r in records), fields=fields,
                maximum_read_microseconds=max((r['end']-r['begin'])*1e6/frequency for r in records),
                maximum_sample_gap_seconds=max(((b['begin']-a['begin'])/frequency
                                               for a,b in zip(records,records[1:])), default=0),
                first_speed_m_s=records[0]['speed'], last_speed_m_s=records[-1]['speed'],
                maximum_speed_m_s=max(r['speed'] for r in records),
                first_axis2_speed_m_s=records[0]['axis2_speed'],
                last_axis2_speed_m_s=records[-1]['axis2_speed'],
                gears=sorted({r['gear'] for r in records}), spans=spans,
                schedule=(dict(phases=sorted({r['schedule'][0] for r in records}),
                               world_delta_range=[min(r['schedule'][1] for r in records), max(r['schedule'][1] for r in records)],
                               job_delta_range=[min(r['schedule'][2] for r in records), max(r['schedule'][2] for r in records)]) if scheduled else None),
                scope='Sequential QPC observations, not atomic/suspended snapshots or original fixed-timestep evidence. Pedal span classification is authored; native key effects require macro/UI context.')


def main():
    p = argparse.ArgumentParser()
    p.add_argument('capture', type=Path)
    p.add_argument('output', type=Path)
    a = p.parse_args()
    assert a.output.resolve().is_relative_to((ROOT / 'local').resolve())
    report = inspect(a.capture)
    with a.output.open('x', encoding='utf-8') as f:
        json.dump(report, f, indent=2)
    print(json.dumps(report))


if __name__ == '__main__':
    main()
