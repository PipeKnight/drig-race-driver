"""Whole unchanged waiting scheduler, owned voices, original repeat/Rust profiles."""
import argparse
import json
import random
import struct
import subprocess
from pathlib import Path

from validate_engine_audio_gain import fixtures as gain_fixtures, sha
from validate_engine_audio_control import source_curves

ROOT = Path(__file__).resolve().parents[1]
CTRL, VOICE, RECORD, RESULT, HEADER = 832, 128, 2444, 2376, 16392


def fixtures(count=1024):
    curves, _ = source_curves()
    base = gain_fixtures(count)
    rng = random.Random(4371)
    rows = []
    for case in range(count):
        row = bytearray(base[8 + case * 2436:8 + (case + 1) * 2436])
        delta = (0., .016, .0625, .125, 1.)[case % 5]
        for category in range(3):
            struct.pack_into('<f', row, 4 + 0x134 + category * 4,
                             delta + (0., .001, .125, 10.)[(case + category) % 4])
            row[4 + 0x14c + category] = (case + category) % 2
            row[4 + 0x14f + category] = (case // (category + 1)) % 2
        struct.pack_into('<f', row, 4 + 0x30c, (-1., -.05, 0., .05, 1.)[case % 5])
        for slot in range(12):
            length = (319292, 477036)[slot % 3 != 0]
            position = (rng.random() * length, -1.875, length, length * 1.5)[case % 4]
            struct.pack_into('<f', row, 4 + 0x230 + slot * 4, position)
            struct.pack_into('<I', row, 4 + 0x2c0 + slot * 4, length)
        rows.append(row + struct.pack('<d', delta))
    return b'ES01' + struct.pack('<I', count) + curves + b''.join(rows)


def run(exe, args, log, success=True):
    r = subprocess.run([str(exe), *map(str, args)], cwd=ROOT, capture_output=True, text=True, timeout=120)
    log.write_text(r.stdout + r.stderr)
    assert (r.returncode == 0) == success and 'exception=' not in r.stderr, (exe, r.stdout, r.stderr)
    if exe.name == 'engine-audio-schedule-probe.exe':
        assert 'entire_restored_image_unchanged=1' in r.stdout


def audit(request, result):
    count = struct.unpack_from('<I', request, 4)[0]
    assert result[:8] == request[:8] and len(result) == 8 + count * RESULT
    writable = set(range(8, 12)) | set(range(0xc8, 0xcc)) | set(range(0x134, 0x14f))
    writable |= set(range(0x1a4, 0x1ec)) | set(range(0x260, 0x290))
    mutable = set(range(0x14, 0x3c)) | set(range(0x50, 0x60)) | {0x78}
    raised = saturated = 0
    for case in range(count):
        before = request[HEADER + case * RECORD:HEADER + (case + 1) * RECORD]
        after = result[8 + case * RESULT:8 + (case + 1) * RESULT]
        assert before[:4] == after[:4] and after[-4:] == b'\0' * 4
        for byte in range(CTRL):
            if byte not in writable:
                assert before[4 + byte] == after[4 + byte], (case, 'controller', hex(byte))
        for category in range(3):
            assert before[4 + 0x14f + category] == after[4 + 0x14f + category]
            raised += before[4 + 0x14c + category] == 0 and after[4 + 0x14c + category] == 1
            saturated += abs(struct.unpack_from('<f', after, 4 + 0x140 + category * 4)[0]) == 1
            assert struct.unpack_from('<f', after, 4 + 0x134 + category * 4)[0] >= 0
        for slot in range(12):
            at = 4 + CTRL + slot * VOICE
            for byte in range(VOICE):
                if byte not in mutable:
                    assert before[at + byte] == after[at + byte], (case, slot, byte)
    return dict(cases=count, complete_state_bytes=count * (CTRL + 12 * VOICE),
                restart_flags_raised=raised, saturated_fades=saturated,
                pair_flags_unchanged=True, opaque_bytes_preserved=True)


def main():
    p = argparse.ArgumentParser()
    p.add_argument('output', type=Path)
    a = p.parse_args()
    out = a.output.resolve()
    assert out.is_relative_to((ROOT / 'local').resolve()) and not out.exists()
    out.mkdir()
    native = ROOT / 'local/audio/engine-audio-schedule-probe.exe'
    profiles = [('native', native, [ROOT / 'local/compatibility/grid-unpacked.exe']),
                ('native-repeat', native, [ROOT / 'local/compatibility/grid-unpacked.exe']),
                ('debug', ROOT / 'target/debug/grid-engine-audio-schedule.exe', []),
                ('release', ROOT / 'target/release/grid-engine-audio-schedule.exe', [])]
    request = fixtures()
    path = out / 'authored.request.bin'
    path.write_bytes(request)
    outputs = []
    for label, exe, prefix in profiles:
        result = out / f'{label}.bin'
        run(exe, [*prefix, path, result], out / f'{label}.txt')
        outputs.append(result.read_bytes())
    for index, data in enumerate(outputs[1:]):
        if data != outputs[0]:
            byte = next(i for i, (x, y) in enumerate(zip(data, outputs[0])) if x != y)
            raise AssertionError((profiles[index + 1][0], byte, (byte - 8) // RESULT, (byte - 8) % RESULT,
                                  outputs[0][byte:byte+8].hex(), data[byte:byte+8].hex()))
    result = audit(request, outputs[0])
    base = fixtures(1)
    guards = []
    for name, at, fmt, value in [
        ('negative_cooldown', HEADER + 4 + 0x13c, 'f', -1.),
        ('short_cooldown', HEADER + 4 + 0x134, 'f', -.001),
        ('bad_pair', HEADER + 4 + 0x151, 'B', 2),
        ('last_position_nan', HEADER + 4 + 0x25c, 'f', float('nan')),
        ('last_length_zero', HEADER + 4 + 0x2ec, 'I', 0),
        ('late_treble_nan', HEADER + 2428, 'f', float('nan')),
        ('delta_nan', HEADER + 2436, 'd', float('nan')),
        ('curve_zero', 8 + 4092, 'f', 0.),
    ]:
        raw = bytearray(base)
        struct.pack_into('<' + fmt, raw, at, value)
        q = out / f'{name}.request.bin'
        q.write_bytes(raw)
        for label, exe, prefix in profiles:
            if label == 'native-repeat':
                continue
            run(exe, [*prefix, q, out / f'{name}-{label}.bin'], out / f'{name}-{label}.txt', False)
            guards.append(dict(name=name, profile=label, rejected=True))
    files = [native, profiles[2][1], profiles[3][1], Path(__file__),
             ROOT / 'tools/probe_engine_audio_schedule.c', ROOT / 'tools/build_engine_audio_schedule_probe.cmd',
             ROOT / 'crates/grid-tools/src/engine_audio_schedule.rs',
             ROOT / 'crates/grid-probes/src/bin/grid-engine-audio-schedule.rs']
    files += list(out.glob('*.bin')) + list(out.glob('*.txt'))
    report = dict(passed=True, authored=result, guards=guards,
                  scope='Whole unchanged 4371C0 and actual helpers, all owned voices present, camera3, cooldowns remain nonnegative. Restart backend/play/seek path deliberately excluded; not whole451770, device mixing or audible equivalence.',
                  identities={str(f): sha(f) for f in files})
    (out / 'validation.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({k: v for k, v in report.items() if k != 'identities'}))


if __name__ == '__main__':
    main()

