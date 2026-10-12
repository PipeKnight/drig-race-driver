"""Rebuild all source constructor requests and verify every complete output."""
import argparse
import json
import struct
from pathlib import Path

import validate_packed_shape_constructor as boundary
from validate_packed_shape_constructor import ROOT, digest


def main():
    p = argparse.ArgumentParser()
    p.add_argument('report', type=Path)
    p.add_argument('output', type=Path)
    a = p.parse_args()
    path = a.report.resolve()
    r = json.loads(path.read_text())
    assert r['passed'] and r['shapes'] == 245 and r['cases'] == 1960
    assert r['full_constructor_original_repeat'] and not r['rust_constructor_equivalence']
    assert r['native_sha256'] == digest(boundary.NATIVE)
    assert r['image_sha256'] == digest(boundary.IMAGE)
    assert r['script_sha256'] == digest(ROOT/'tools/validate_packed_shape_constructor.py')
    assert r['source_sha256'] == digest(boundary.TRACK)
    assert r['surfaces_sha256'] == digest(boundary.SURFACES)
    expected = []
    for index, (name, raw, positions, mask, alternate, missing) in enumerate(boundary.sources()):
        nm = -struct.unpack_from('<i', raw, 32)[0]
        for fill in [0, 0xcd]:
            for label, sheet in [('source', mask), ('none', 0), ('all', (1 << nm)-1), ('alternate', alternate)]:
                expected.append((index, name, raw, positions, fill, label, sheet, missing))
    complete = observable = source_nonzero = missing_keys = 0
    for i, (row, fields) in enumerate(zip(r['results'], expected, strict=True)):
        index, name, raw, positions, fill, label, sheet, missing = fields
        assert (row['index'], row['source_index'], row['chunk'], row['fill'], row['label'], row['mask'], row['missing_source_keys']) == (i, index, name, fill, label, sheet, missing)
        inp = path.parent/f'{i:04}-input.bin'
        assert inp.read_bytes() == boundary.request(raw, fill, sheet)
        assert digest(inp) == row['input_sha256']
        answers = []
        for kind, e in zip(['original', 'repeat'], row['executions'], strict=True):
            assert e['exit'] == 0 and not e['stderr'] and 'code_unchanged=1 globals_restored=1 code_replacements=0 error=0' in e['stdout']
            target = path.parent/f'{i:04}-{kind}.bin'
            assert digest(target) == row['output_sha256']
            b = target.read_bytes()
            assert len(b) == row['bytes']
            boundary.check(b, raw, positions, fill, sheet)
            # Check the complete known context and mutated base descriptor,
            # beyond the selected numeric construction assertions.
            descriptor = bytearray(0x94)
            for o, v in [(0, 0x100000), (4, 0xdead2008), (0x4c, 0x3f800000), (0x5c, 0x2000), (0x7c, struct.unpack_from('<I', raw, 52)[0])]:
                struct.pack_into('<I', descriptor, o, v)
            assert b[0x2c0:0x354] == descriptor
            world = bytearray(0x100)
            struct.pack_into('<I', world, 0x14, 0x3000)
            struct.pack_into('<I', world, 0x80, 28)
            assert b[0x354:0x454] == world
            manager = bytearray(0x20)
            struct.pack_into('<3I', manager, 4, 0x4000, -struct.unpack_from('<i', raw, 32)[0], 0x3c)
            manager[0x10] = 1
            assert b[0x454:0x474] == manager
            answers.append(b); complete += 1
        assert answers[0] == answers[1]
        observable += row['bytes']
        source_nonzero += label == 'source' and sheet != 0
        if label == 'source' and fill == 0: missing_keys += missing
    assert observable == r['observable_bytes'] and source_nonzero == r['source_nonzero_masks']
    assert len(r['guards']) == 11
    for g in r['guards']:
        assert 0 < g['execution']['exit'] < 256 and 'fault=' not in g['execution']['stderr']
        if g['label'] == 'existing':
            assert digest(path.parent/'guard-existing.bin') == g['unchanged_sha256']
        else:
            assert digest(path.parent/f"guard-{g['label']}-input.bin") == g['input_sha256']
            assert g['no_output'] and not (path.parent/f"guard-{g['label']}.bin").exists()
    output = a.output.resolve()
    assert output.is_relative_to((ROOT/'local').resolve())
    result = dict(passed=True, complete_outputs=complete, guards=11, source_nonzero_masks=source_nonzero,
                  source_missing_keys=missing_keys, native_bytes=observable, current_hashes=True,
                  requests_rebuilt=True, complete_descriptor_context=True, report_sha256=digest(path))
    with output.open('x') as f: json.dump(result, f, indent=2)
    print(json.dumps(result, indent=2))


if __name__ == '__main__': main()
