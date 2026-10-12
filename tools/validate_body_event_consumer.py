"""Whole original event dispatch against an independent bit-level event oracle.

Callbacks are explicit authored observers, not original vehicle handlers. No
proprietary contact data is embedded in this script or emitted outside local/.
"""
import argparse
import hashlib
import json
from pathlib import Path
import random
import struct
import subprocess

from capture_reference_spawn import ANALYSIS, private

ROOT = Path(__file__).resolve().parents[1]
MAGIC = 0x31454342
PROBE_SHA = 'dda43b9efb6917b7e6c932e6222157fa41a1d978b2b0616b89f71203a696ab72'


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def stored_float(word):
    # A binary32 x87 load/store quiets signaling NaNs; other bits survive here.
    if word & 0x7f800000 == 0x7f800000 and word & 0x007fffff:
        return word | 0x00400000
    return word


def corpus():
    # Registration selectors, callback identities, and raw user words.
    patterns = [
        (0, 0, 0, 0, 0, 0), (1, 0, 1, 0, 0, 0), (0, 2, 0, 2, 0, 17),
        (1, 2, 0, 0, 11, 19), (1, 2, 1, 0, 0, 19), (1, 2, 0, 2, 11, 0),
        (1, 2, 1, 1, 5, 5), (1, 2, 1, 1, 5, 7), (1, 2, 1, 2, 5, 5),
        (1, 2, 2, 1, 0xffffffff, 0), (1, 1, 1, 2, 31, 37),
        (2, 2, 1, 2, 31, 37), (2, 1, 1, 2, 43, 47),
        (1, 2, 2, 2, 0, 0), (2, 1, 0, 2, 53, 59),
        (1, 0, 0, 2, 61, 67), (0, 2, 1, 0, 71, 73),
        (1, 2, 2, 2, 0x80000000, 0xffffffff),
    ]
    floats = [0, 0x80000000, 1, 0x80000001, 0x007fffff, 0x00800000,
              0x3f800000, 0xbf800000, 0x7f7fffff, 0x7f800000, 0xff800000,
              0x7fc12345, 0xffc54321, 0x7f812345, 0xff854321]
    rng = random.Random(0x47524944)
    rows = []
    for pattern in patterns:
        for index, value in enumerate(floats):
            for payload in range(4):
                contact = bytearray(rng.getrandbits(8) for _ in range(0x80))
                struct.pack_into('<I', contact, 0x38, value)
                struct.pack_into('<I', contact, 0x3c, floats[(index+payload) % len(floats)])
                for mutation in (0, 1):
                    words = (*pattern, rng.getrandbits(32), value, mutation)
                    rows.append((bytes(contact), words))
    return rows


def pack(rows):
    return struct.pack('<II', MAGIC, len(rows)) + b''.join(
        contact + struct.pack('<9I', *words) for contact, words in rows)


def expected(rows):
    result = [struct.pack('<II', MAGIC, len(rows))]
    for contact, w in rows:
        event = bytearray(0x50)
        struct.pack_into('<2I', event, 0, *w[:2])
        event[8:16] = contact[0x30:0x38]
        event[0x10:0x30] = contact[0x50:0x70]
        for destination, source in ((0x40, 0x3c), (0x4c, 0x38)):
            struct.pack_into('<I', event, destination,
                             stored_float(struct.unpack_from('<I', contact, source)[0]))
        callbacks = [(w[2+s-1], w[4+s-1]) if s else (0, 0) for s in w[:2]]
        if callbacks[0] == callbacks[1]:
            callbacks = callbacks[:1]
        calls = []
        for function, user in callbacks:
            if not function:
                continue
            calls.append(struct.pack('<4I', function, user, w[6], stored_float(w[7])) + event)
            if w[8]:
                value = struct.unpack_from('<I', event, 8)[0] ^ 0xa5a55a5a
                struct.pack_into('<I', event, 8, value)
        result.append(struct.pack('<I', len(calls)) + b''.join(calls) + bytes((2-len(calls))*96))
    return b''.join(result)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    out = private(args.output)
    image = ROOT / 'local/compatibility/grid-unpacked.exe'
    probe = ROOT / 'local/behavior/body-event-consumer-probe.exe'
    if digest(image) != ANALYSIS or digest(probe) != PROBE_SHA:
        raise ValueError('Current reference/probe hash mismatch')
    rows = corpus()
    wanted = expected(rows)
    call_counts = [struct.unpack_from('<I', wanted, 8+i*196)[0] for i in range(len(rows))]
    out.mkdir(parents=True, exist_ok=False)
    request = out / 'input.bin'
    request.write_bytes(pack(rows))
    (out / 'expected.bin').write_bytes(wanted)
    report = dict(scope='Whole original A726B0/B0CFC0/A6A270; owned observer callbacks only. '
                  'No original car handler, source registration or runtime integration proof.',
                  cases=len(rows), compared_bytes_per_run=len(wanted), runs={}, guards=[],
                  image_sha256=digest(image), probe_sha256=digest(probe),
                  request_sha256=digest(request), expected_sha256=digest(out / 'expected.bin'),
                  validator_sha256=digest(Path(__file__)),
                  sources={name: digest(ROOT / 'tools' / name) for name in
                           ('probe_body_event_consumer.c', 'build_body_event_consumer_probe.cmd',
                            'isolated_reference.h')},
                  callback_counts={str(n): call_counts.count(n) for n in range(3)},
                  shared_mutation_cases=sum(n == 2 and bool(w[8]) for n, (_, w) in zip(call_counts, rows)))

    def run(source, target, reference=image):
        return subprocess.run([str(probe), str(reference), str(source), str(target)],
                              cwd=ROOT, capture_output=True, text=True, timeout=120)

    original = None
    for label in ('original', 'repeat'):
        target = out / f'{label}.bin'
        process = run(request, target)
        actual = target.read_bytes() if target.exists() else b''
        if original is None:
            original = actual
        mismatch = next((i for i, (a, b) in enumerate(zip(actual, wanted)) if a != b), None)
        report['runs'][label] = dict(exit=process.returncode, stdout=process.stdout,
                                    stderr=process.stderr, actual_sha256=digest(target) if target.exists() else None,
                                    bytes=len(actual), first_mismatch=mismatch,
                                    passed=process.returncode == 0 and actual == wanted == original)
    valid_one = pack(rows[:1])
    guards = [('magic', struct.pack('<II', 0, 1)), ('empty', struct.pack('<II', MAGIC, 0)),
              ('count', struct.pack('<II', MAGIC, 4097)), ('truncated', valid_one[:-1]),
              ('trailing', valid_one+b'x')]
    for field in (0, 1, 2, 3, 8):
        broken = bytearray(valid_one)
        struct.pack_into('<I', broken, 8+0x80+field*4, 2 if field == 8 else 3)
        guards.append((f'field-{field}', broken))
    for label, data in guards:
        source, target = out / f'guard-{label}.bin', out / f'guard-{label}-output.bin'
        source.write_bytes(data)
        process = run(source, target)
        report['guards'].append(dict(label=label, exit=process.returncode,
                                     passed=process.returncode != 0 and not target.exists()))
    existing = out / 'existing.bin'
    existing.write_bytes(b'preserve existing evidence')
    process = run(request, existing)
    report['guards'].append(dict(label='existing-output', exit=process.returncode,
                                 passed=process.returncode != 0 and existing.read_bytes() == b'preserve existing evidence'))
    wrong_image = out / 'wrong-reference.bin'
    wrong_image.write_bytes(bytes(64))
    wrong_target = out / 'wrong-reference-output.bin'
    process = run(request, wrong_target, wrong_image)
    report['guards'].append(dict(label='wrong-reference', exit=process.returncode,
                                 passed=process.returncode != 0 and not wrong_target.exists()))
    report['passed'] = all(x['passed'] for x in [*report['runs'].values(), *report['guards']])
    (out / 'validation.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
    print(json.dumps(report, indent=2))
    if not report['passed']:
        raise RuntimeError('Whole registered event proof failed; retained private evidence')


if __name__ == '__main__':
    main()
