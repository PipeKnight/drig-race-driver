"""Whole original handling with terminal wheel state and absent event receiver."""
import argparse
import json
import math
import random
import struct
from pathlib import Path
from validate_handling_damage import fixtures as healthy, check, SIZE, PREFIX
from validate_collision_damage import IMAGE, CAR, M, sha, f, u, put, f32, run

NATIVE = Path('local/behavior/handling-terminal-probe.exe')
BINARIES = {p: Path(f'target/{p}/grid-handling-damage.exe') for p in ['debug', 'release']}


def fixtures():
    yield from healthy()
    base = next(r for label, r in healthy() if label == 'source' and f(r, 0) == f32(.016))
    rng = random.Random(2008270)
    for index in range(256):
        r = bytearray(base)
        v = PREFIX
        put(r, v+M+0x280, 27., .45, 105.)
        struct.pack_into('<I', r, v+M+0x444, 3 if index % 8 == 0 else 7)
        r[v+M+0x448] = index % 4
        r[v+M+0x442] = int(index % 8 != 1)
        r[v+0xc7ae] = index % 2
        put(r, v+0xc740, 0., 0., 0., 0.)
        first = index % 4
        for wheel in range(4):
            put(r, v+0x1790+wheel*4, 1. if wheel >= first else .1)
            put(r, v+0x1650+wheel*16, 0., 0., 0., 0.)
            put(r, v+0x1ac8+wheel*0x150, 0.)
        if index % 8 == 2:
            put(r, v+M+0x280, -0.)
        if index % 8 == 3:
            # Positive displacement only, including homogeneous W.
            for wheel in range(4):
                put(r, v+0x1790+wheel*4, 0.)
                put(r, v+0x1650+wheel*16, 0., 0., 0., .8 if wheel >= first else .1)
        if index % 8 == 4:
            put(r, v+M+0x288, 500.)
        if index % 8 == 5:
            put(r, v+M+0x288, 0.)
        if index % 8 == 6:
            # Simultaneous above-threshold collision, wear and critical wheel.
            for wheel in range(4):
                put(r, v+0x1ac8+wheel*0x150, rng.uniform(200000., 900000.))
        yield 'terminal', bytes(r)


def terminal_check(request, answer):
    stats = check(request, answer)
    stats.update(transitions=0, retained_terminal=0, terminal_outputs=0)
    for at in range(8, len(request), SIZE):
        r, o = request[at:at+SIZE], answer[at:at+SIZE]
        v = PREFIX
        status, ordinal = u(r, v+M+0x444), r[v+M+0x448]
        if status == 7 and r[v+M+0x442] and f(r, v+M+0x280) != 0.:
            for wheel in range(4):
                angle = f32(abs(f(r, v+0x1790+wheel*4)) /
                            f32(f(r, v+M+0x280)*f32(.01745329238474369)))
                values = [f(r, v+0x1650+wheel*16+lane*4) for lane in range(4)]
                if r[v+0xc7ae]:
                    values = [f32(x-f32(f(r, v+0xeefc)*f(r, v+0xc740+lane*4)))
                              for lane, x in enumerate(values)]
                distance = math.sqrt(sum(x*x for x in values))/f(r, v+M+0x284)
                threshold = f32(f(r, v+M+0x288)*f32(.01))
                # These fixtures are deliberately distant from the root boundary;
                # the whole native/Rust byte comparison checks the exact SSE root.
                assert abs(max(angle, distance)-threshold) > 1e-5
                if max(angle, distance) > threshold:
                    status, ordinal = 3, wheel
                    stats['transitions'] += 1
                    break
        assert u(o, v+M+0x444) == status
        assert o[v+M+0x448] == ordinal
        assert o[v+M+0x449:v+M+0x44c] == r[v+M+0x449:v+M+0x44c]
        stats['terminal_outputs'] += status == 3
        stats['retained_terminal'] += u(r, v+M+0x444) == 3 and status == 3
    return stats


def execute(request, output, name):
    answers, records = {}, {}
    for kind in ['original', 'repeat', 'debug', 'release']:
        target = output/f'{name}-{kind}.bin'
        command = ([NATIVE, IMAGE, request, target] if kind in ['original', 'repeat'] else
                   [BINARIES[kind], request, target, '--empty-notification-receiver'])
        execution = run(command)
        if kind in ['original', 'repeat']:
            assert 'code_unchanged=1 globals_restored=1 code_replacements=0 error=0' in execution['stdout']
        answers[kind] = target.read_bytes()
        records[kind] = dict(path=str(target), sha256=sha(target), execution=execution)
    assert all(data == answers['original'] for data in answers.values())
    return answers['original'], records


def main():
    p = argparse.ArgumentParser()
    p.add_argument('output', type=Path)
    a = p.parse_args()
    assert a.output.resolve().is_relative_to(Path('local').resolve())
    a.output.mkdir(exist_ok=False)
    paths = [Path(__file__), NATIVE, IMAGE, *BINARIES.values(),
             Path('tools/probe_handling_damage.c'), Path('tools/vehicle_handling_probe.h'),
             Path('tools/handling_query_probe.h'), Path('tools/validate_handling_damage.py'),
             Path('tools/validate_collision_damage.py'), Path('tools/validate_damage_region.py'),
             Path('crates/grid-world/src/vehicle_handling.rs'),
             Path('crates/grid-tools/src/bin/grid-handling-damage.rs')]
    identities = {str(path): sha(path) for path in paths}
    items = list(fixtures())
    rows, total, stats = [], 0, {}
    for batch in range(0, len(items), 32):
        group = items[batch:batch+32]
        raw = b'VH02'+struct.pack('<I', len(group))+b''.join(r for _, r in group)
        request = a.output/f'{batch//32:03}.input'
        request.write_bytes(raw)
        answer, outputs = execute(request, a.output, f'{batch//32:03}')
        result = terminal_check(raw, answer)
        for key, value in result.items(): stats[key] = stats.get(key, 0)+value
        total += len(answer)-8
        rows.append(dict(request=str(request), sha256=sha(request), outputs=outputs, stats=result))
    # Carry the critical state through another whole update, rather than resetting
    # the status to healthy to hide the next-frame failure.
    carried = Path(rows[-1]['outputs']['original']['path']).read_bytes()
    request = a.output/'carried.input'
    request.write_bytes(carried)
    answer, outputs = execute(request, a.output, 'carried')
    carried_stats = terminal_check(carried, answer)
    assert carried_stats['retained_terminal'] > 0
    guards = []
    base = bytearray(b'VH02'+struct.pack('<I', 1)+items[-1][1])
    for name, offset, value in [('status', 8+PREFIX+M+0x444, 4),
                                ('ordinal', 8+PREFIX+M+0x448, 4),
                                ('owner-link', 8+PREFIX+M+0x214, 1),
                                ('delta-nan', 8, 0x7fc00000)]:
        raw = bytearray(base)
        if name == 'ordinal': struct.pack_into('<I', raw, 8+PREFIX+M+0x444, 3)
        struct.pack_into('<I', raw, offset, value)
        request = a.output/f'guard-{name}.input'
        request.write_bytes(raw)
        for kind in ['original', 'debug', 'release']:
            target = a.output/f'guard-{name}-{kind}.bin'
            command = ([NATIVE, IMAGE, request, target] if kind == 'original' else
                       [BINARIES[kind], request, target, '--empty-notification-receiver'])
            run(command, False)
            assert not target.exists()
            guards.append(f'{kind}:{name}')
    assert stats['transitions'] > 0 and stats['damage_changes'] > 0 and stats['wear_changes'] > 0
    result = dict(passed=True, cases=len(items), bytes_per_profile=total, identities=identities,
                  rows=rows, guards=guards, stats=stats,
                  carried=dict(request=str(a.output/'carried.input'), outputs=outputs, stats=carried_stats))
    (a.output/'validation.json').write_text(json.dumps(result, indent=2))
    print(json.dumps({k: result[k] for k in ['passed', 'cases', 'bytes_per_profile', 'stats']}))


if __name__ == '__main__': main()
