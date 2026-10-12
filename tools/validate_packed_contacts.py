"""Fresh source-only Rust packed contacts versus whole-original PC08 captures.

Selected constructor extent state only; no runtime barrier claim.
"""
import argparse
import json
import struct
import subprocess
from pathlib import Path

import validate_packed_mesh_complete as source

ROOT, digest = source.ROOT, source.digest
REFERENCE = ROOT/'local/behavior/packed-complete-20261012-final-01/validation.json'
SAMPLING = ROOT/'local/behavior/contact-directions-source-20261012-final-02/validation.json'
TABLE = SAMPLING.parent/'directions.bin'
NATIVE = ROOT/'local/behavior/oriented-regression-packed_mesh_complete.exe'


def project(answer, capacity):
    assert len(answer) == 0x44f8 + capacity*8 + 0x40
    cache = answer[0x21c8:0x4218]
    return (cache[:0x20]+cache[0x2c:0x34]+cache[0x44:0x2048]+answer[0xfd0:0xfd4]+answer[0x44f8:-0x40]
            +answer[0xfb0:0xfb4]+answer[0xf60:0xf70]+answer[0xfe8:0x17e8])


def run(exe, inp, out, table=TABLE):
    p = subprocess.run([str(exe), str(inp), str(out), str(table)], cwd=ROOT, capture_output=True, text=True, timeout=30)
    return dict(exit=p.returncode, stdout=p.stdout, stderr=p.stderr)


def main():
    p = argparse.ArgumentParser()
    p.add_argument('output', type=Path)
    p.add_argument('--rust', type=Path, default=ROOT/'target/debug/grid-packed-contacts.exe')
    p.add_argument('--max-cases', type=int, default=2642)
    p.add_argument('--fresh-native', action='store_true')
    a = p.parse_args()
    assert 1 <= a.max_cases <= 2642
    out = a.output.resolve()
    assert out.is_relative_to((ROOT/'local').resolve())
    out.mkdir(exist_ok=False)
    reference = json.loads(REFERENCE.read_text())
    assert reference['passed'] and reference['cases'] == 2642
    assert digest(REFERENCE) == '4e7ee1b733285fc68726c9f5f15d148c33cd92e2c19c87e893299cc9c812af34'
    sampling = json.loads(SAMPLING.read_text())
    assert sampling['passed'] and digest(TABLE) == sampling['output_sha256']
    assert digest(Path(sampling['source_path'])) == sampling['source_sha256']
    from extract_contact_directions import extract
    assert extract()[0] == TABLE.read_bytes()
    rows = []
    for index, (meta, request) in enumerate(source.inputs()):
        if index == a.max_cases: break
        row = reference['results'][index]
        assert all(row[k] == v for k, v in meta.items())
        inp = REFERENCE.parent/f'{index:04}-input.bin'
        assert digest(inp) == row['input_sha256'] and inp.read_bytes() == request
        original = REFERENCE.parent/f'{index:04}-original.bin'
        repeat = REFERENCE.parent/f'{index:04}-repeat.bin'
        assert digest(original) == digest(repeat) == row['output_sha256']
        native_execution = None
        if a.fresh_native:
            target = out/f'{index:04}-native.bin'
            e = subprocess.run([str(NATIVE), str(source.IMAGE), str(inp), str(target)], cwd=ROOT,
                               capture_output=True, text=True, timeout=30)
            native_execution = dict(exit=e.returncode, stdout=e.stdout, stderr=e.stderr)
            assert e.returncode == 0 and not e.stderr and 'code_unchanged=1 globals_restored=1 code_replacements=0 error=0' in e.stdout
            assert target.read_bytes() == original.read_bytes() and digest(target) == row['output_sha256']
        capacity = struct.unpack_from('<I', request, 20)[0]
        expected = project(original.read_bytes(), capacity)
        target = out/f'{index:04}-rust.bin'
        execution = run(a.rust, inp, target)
        assert execution['exit'] == 0 and not execution['stderr'], (index, meta, execution)
        actual = target.read_bytes()
        mismatch = next((i for i, (x, y) in enumerate(zip(expected, actual)) if x != y), None)
        assert actual == expected, (index, meta, 'difference', mismatch, 'sizes', len(actual), len(expected))
        accepted, visited = struct.unpack_from('<2I', actual, 32)
        stamps = sum(x != 0 for x in struct.unpack_from('<4096H', actual, 40))
        contacts = struct.unpack_from('<I', actual, 8240+8*capacity)[0]
        assert contacts == row['contacts']
        rows.append(dict(index=index, **meta, input_sha256=digest(inp), original_sha256=digest(original),
                         output_sha256=digest(target), bytes=len(actual), accepted=accepted, visited=visited,
                         stamps=stamps, contacts=contacts, execution=execution, native_execution=native_execution))
        if (index+1) % 200 == 0: print(f'cases={index+1}', flush=True)
    first = (REFERENCE.parent/'0000-input.bin').read_bytes()
    bad_inputs = [('short', first[:43]), ('long', first+b'\0')]
    for label, offset, value in [('tag', 0, 0), ('mask', 40, 256), ('outside-mask', 40, 128),
                                 ('contact-limit', 12, 17), ('cache-limit', 16, 2), ('candidate-limit', 20, 32769),
                                 ('center-nan', 24, 0x7fc00000), ('margin-negative', 44+0x220, 0xbf800000),
                                 ('mesh-kind', 44+0x10, 8)]:
        b = bytearray(first); struct.pack_into('<I', b, offset, value); bad_inputs.append((label, b))
    guards = []
    for label, b in bad_inputs:
        inp = out/f'guard-{label}-input.bin'; inp.write_bytes(b)
        target = out/f'guard-{label}.bin'
        execution = run(a.rust, inp, target)
        assert 0 < execution['exit'] < 256 and not target.exists(), (label, execution)
        guards.append(dict(label=label, input_sha256=digest(inp), execution=execution, no_output=True))
    target = out/'guard-existing.bin'; target.write_bytes(b'preserve existing evidence'); before = digest(target)
    execution = run(a.rust, REFERENCE.parent/'0000-input.bin', target)
    assert execution['exit'] and digest(target) == before
    guards.append(dict(label='existing', execution=execution, unchanged_sha256=before))
    table_bytes = TABLE.read_bytes()
    table_bad = [('short', table_bytes[:-1]), ('long', table_bytes+b'\0')]
    for label, offset, value in [('tag', 0, 0), ('count', 4, 43), ('nan', 8, 0x7fc00000),
                                 ('nonunit', 8, 0x41200000), ('point-w', 20, 0x3f800000)]:
        b = bytearray(table_bytes); struct.pack_into('<I', b, offset, value); table_bad.append((label, b))
    for label, b in table_bad:
        inp = out/f'guard-table-{label}-input.bin'; inp.write_bytes(b)
        target = out/f'guard-table-{label}.bin'
        execution = run(a.rust, REFERENCE.parent/'0000-input.bin', target, inp)
        assert 0 < execution['exit'] < 256 and not target.exists(), (label, execution)
        guards.append(dict(label='table-'+label, input_sha256=digest(inp), execution=execution, no_output=True))
    report = dict(passed=True, cases=len(rows), results=rows, guards=guards,
                  observable_bytes=sum(r['bytes'] for r in rows), accepted=sum(r['accepted'] for r in rows),
                  visited=sum(r['visited'] for r in rows), stamps=sum(r['stamps'] for r in rows),
                  contacts=sum(r['contacts'] for r in rows), positive=sum(r['contacts']>0 for r in rows),
                  reference=str(REFERENCE.relative_to(ROOT)), reference_sha256=digest(REFERENCE),
                  rust_path=str(a.rust.resolve()), rust_sha256=digest(a.rust), script_sha256=digest(Path(__file__)),
                  sampling_sha256=digest(SAMPLING), table_sha256=digest(TABLE),
                  native_sha256=digest(NATIVE), image_sha256=digest(source.IMAGE),
                  fresh_original_execution=a.fresh_native, selected_fresh_rust_generator=True, runtime_barriers=False)
    (out/'validation.json').write_text(json.dumps(report, indent=2))
    print(json.dumps({k: report[k] for k in ['passed', 'cases', 'contacts', 'positive', 'accepted', 'observable_bytes']}))


if __name__ == '__main__': main()
