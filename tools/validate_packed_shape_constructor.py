"""Whole packed constructor: owned source geometry and explicit sheet flags.

Original/repeat full output equality, plus independently decoded bounds, scales,
relocations, material lookup and mask checks. No Rust/full factory equivalence.
"""
import argparse
import hashlib
import json
import struct
import subprocess
from pathlib import Path

from validate_binxml import decode
from validate_vcqtc import chunks, f32

ROOT = Path(__file__).resolve().parents[1]
GAME = Path('C:/Program Files (x86)/Steam/steamapps/common/Grid')
TRACK = GAME/'tracks/europe/donington/route_0/track.jpk'
SURFACES = GAME/'surface_materials.xml'
IMAGE = ROOT/'local/compatibility/grid-unpacked.exe'
NATIVE = ROOT/'local/behavior/packed-shape-constructor-probe.exe'
TRACK_SHA = '6e161e5bee8f946ff0622ca836af9071c9474cec7aef53609180b84319f9d8ef'
SURFACE_SHA = '8a23a960ea0ca837ee49bafaab893cff7446c0089c9ca282e131030f4b3d3251'


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def execute(inp, target):
    r = subprocess.run([str(NATIVE), str(IMAGE), str(inp), str(target)],
                       cwd=ROOT, capture_output=True, text=True, timeout=30)
    return dict(exit=r.returncode, stdout=r.stdout, stderr=r.stderr)


def sources():
    assert digest(TRACK) == TRACK_SHA and digest(SURFACES) == SURFACE_SHA
    data = TRACK.read_bytes()
    _, surface_root, _ = decode(SURFACES.read_bytes())
    materials = [(int.from_bytes(m.get('name').encode('ascii'), 'little'),
                  m.get('sheet', '').lower() == 'true') for m in surface_root.findall('MATERIAL')]
    assert len(materials) == 89 and len(dict(materials)) == 89
    lookup = dict(materials)
    decoded = {name: positions for name, positions, _, _, _ in chunks(data)}
    count = struct.unpack_from('<I', data, 8)[0]
    assert count == 245
    for i in range(count):
        words = struct.unpack_from('<8I', data, 32+i*32)
        name = data[words[0]:data.index(0, words[0])].decode('ascii')
        assert words[1] == words[3]
        raw = data[words[2]:words[2]+words[3]]
        nm = -struct.unpack_from('<i', raw, 32)[0]
        keys = struct.unpack_from('<'+'I'*nm, raw, 52)
        sheet = sum(int(lookup.get(key, materials[0][1])) << j for j, key in enumerate(keys))
        unique = {key: j for j, key in enumerate(dict.fromkeys(keys))}
        alternate = sum((unique[key] % 2) << j for j, key in enumerate(keys))
        yield name, raw, decoded[name], sheet, alternate, sum(k not in lookup for k in keys)


def request(raw, fill, sheet):
    return struct.pack('<4sIII', b'PS02', len(raw), fill, sheet)+raw


def check(answer, raw, positions, fill, mask):
    u = lambda b, o: struct.unpack_from('<I', b, o)[0]
    assert len(answer) == 0x674+len(raw)
    shape = answer[:0x2c0]
    descriptor = answer[0x2c0:0x354]
    world = answer[0x354:0x454]
    manager = answer[0x454:0x474]
    material = answer[0x474:0x674]
    chunk = answer[0x674:]
    nt, nv, nm = (-v for v in struct.unpack_from('<3i', raw, 24))
    assert u(shape, 0) == 0x700b and u(shape, 0x10) == 11
    assert u(shape, 0x240) == 0xdead2008 and u(shape, 0x244) == 0x100000
    assert u(shape, 0xc8) == 0x4000 and u(shape, 0xcc) == 0x2000
    assert u(shape, 0x290) == mask
    assert shape[0x294:0x2a4] == struct.pack('<4I', *([0x7f7fffff]*4))
    assert shape[0x2a4:] == bytes([fill])*(0x2c0-0x2a4)
    bounds = struct.unpack_from('<6f', raw)
    scales = [f32(f32(bounds[j+3]-bounds[j])/(65536 if j == 1 else 16777216)) for j in range(3)]
    assert shape[0x250:0x260] == struct.pack('<4f', *scales, 1.)
    assert shape[0x260:0x270] == struct.pack('<4f', *bounds[:3], 0.)
    low = [min(p[j] for p in positions) for j in range(3)]
    high = [max(p[j] for p in positions) for j in range(3)]
    assert shape[0x270:0x290] == struct.pack('<8f', *low, 1., *high, 1.)
    assert u(descriptor, 0) == 0x100000 and u(descriptor, 4) == 0xdead2008
    assert u(descriptor, 0x5c) == 0x2000 and u(world, 0x14) == 0x3000 and u(world, 0x80) == 28
    assert struct.unpack_from('<3I', manager, 4) == (0x4000, nm, 0x3c) and manager[0x10] == 1
    keys = struct.unpack_from('<'+'I'*nm, raw, 52)
    expected = bytearray(0x200)
    for i, key in enumerate(keys):
        struct.pack_into('<I', expected, i*0x3c, key)
        expected[i*0x3c+0x38] = (mask >> i) & 1
    assert material == expected
    expected = bytearray(raw)
    struct.pack_into('<3I', expected, 24, nt, nv, nm)
    offsets = struct.unpack_from('<4I', raw, 36)
    struct.pack_into('<4I', expected, 36, *(0x100000+o for o in offsets))
    for i, key in enumerate(keys):
        struct.pack_into('<I', expected, 52+i*4, 0x4000+keys.index(key)*0x3c)
    assert chunk == expected


def main():
    p = argparse.ArgumentParser()
    p.add_argument('output', type=Path)
    p.add_argument('--max-shapes', type=int, default=245)
    a = p.parse_args()
    assert 1 <= a.max_shapes <= 245
    out = a.output.resolve()
    assert out.is_relative_to((ROOT/'local').resolve())
    out.mkdir(exist_ok=False)
    rows = []
    source_rows = list(sources())
    for index, (name, raw, positions, source_mask, alternate, missing) in enumerate(source_rows[:a.max_shapes]):
        nm = -struct.unpack_from('<i', raw, 32)[0]
        for fill in [0, 0xcd]:
            for label, mask in [('source', source_mask), ('none', 0), ('all', (1 << nm)-1), ('alternate', alternate)]:
                i = len(rows)
                inp = out/f'{i:04}-input.bin'
                inp.write_bytes(request(raw, fill, mask))
                answers = []; executions = []
                for kind in ['original', 'repeat']:
                    target = out/f'{i:04}-{kind}.bin'
                    e = execute(inp, target); executions.append(e)
                    assert e['exit'] == 0 and not e['stderr'], (i, e)
                    assert 'code_unchanged=1 globals_restored=1 code_replacements=0 error=0' in e['stdout']
                    answers.append(target.read_bytes())
                    check(answers[-1], raw, positions, fill, mask)
                assert answers[0] == answers[1]
                rows.append(dict(index=i, source_index=index, chunk=name, fill=fill, label=label, mask=mask,
                                 missing_source_keys=missing, input_sha256=digest(inp),
                                 output_sha256=digest(out/f'{i:04}-original.bin'), bytes=len(answers[0]), executions=executions))
        if (index+1) % 25 == 0: print(f'shapes={index+1} cases={len(rows)}', flush=True)
    first = (out/'0000-input.bin').read_bytes()
    malformed = [('short', first[:15]), ('long', first+b'\0')]
    for label, o, v in [('tag', 0, 0), ('size', 4, 0), ('fill', 8, 1), ('mask', 12, 256),
                        ('zero-materials', 16+32, 0), ('vertices-offset', 16+36, 0),
                        ('bound-nan', 16, 0x7fc00000), ('too-many-materials', 16+32, (-9)&0xffffffff)]:
        bad = bytearray(first); struct.pack_into('<I', bad, o, v); malformed.append((label, bad))
    guards = []
    for label, bad in malformed:
        inp = out/f'guard-{label}-input.bin'; inp.write_bytes(bad)
        target = out/f'guard-{label}.bin'; e = execute(inp, target)
        assert 0 < e['exit'] < 256 and not target.exists() and 'fault=' not in e['stderr'], (label, e)
        guards.append(dict(label=label, input_sha256=digest(inp), no_output=True, execution=e))
    target = out/'guard-existing.bin'; target.write_bytes(b'preserve previous evidence'); before = digest(target)
    e = execute(out/'0000-input.bin', target); assert 0 < e['exit'] < 256 and digest(target) == before
    guards.append(dict(label='existing', unchanged_sha256=before, execution=e))
    assert digest(TRACK) == TRACK_SHA and digest(SURFACES) == SURFACE_SHA
    report = dict(passed=True, cases=len(rows), shapes=a.max_shapes, results=rows, guards=guards,
                  observable_bytes=sum(r['bytes'] for r in rows), source_nonzero_masks=sum(r['label']=='source' and r['mask']!=0 for r in rows),
                  source_path=str(TRACK), source_sha256=TRACK_SHA, surfaces_sha256=SURFACE_SHA,
                  image_sha256=digest(IMAGE), native_sha256=digest(NATIVE), script_sha256=digest(Path(__file__)),
                  full_constructor_original_repeat=True, rust_constructor_equivalence=False)
    (out/'validation.json').write_text(json.dumps(report, indent=2))
    print(json.dumps({k: report[k] for k in ['passed', 'cases', 'shapes', 'observable_bytes', 'source_nonzero_masks']}))


if __name__ == '__main__': main()
