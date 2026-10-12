"""Whole second static registration versus independent source-input Rust assembly."""
import argparse
import hashlib
import json
import math
import struct
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
GAME = Path(r'C:\Program Files (x86)\Steam\steamapps\common\Grid')
IMAGE = ROOT / 'local/compatibility/grid-unpacked.exe'


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(exe, args):
    e = subprocess.run([str(exe), *map(str, args)], cwd=ROOT, capture_output=True,
                       text=True, timeout=30)
    return dict(exit=e.returncode, stdout=e.stdout, stderr=e.stderr)


def check(data, shift, mask, capacity):
    assert len(data) == 0xf80
    first, second = data[:0x250], data[0x250:0x4a0]
    world, cells, pair = data[0x4a0:0xf20], data[0xf20:0xf30], data[0xf30:]
    u = lambda b, o: struct.unpack_from('<I', b, o)[0]
    f = lambda b, o: struct.unpack_from('<f', b, o)[0]
    assert u(first, 0xd8) == 0xa000 and u(second, 0xd8) == 0
    assert u(first, 0x198) == u(second, 0x198) == 1
    assert struct.unpack('<4I', cells) == (0, 0xc000, 0, 0x1000)
    assert u(second, 0x194) == u(first, 0x194) + 1
    assert u(world, 0x80) == u(second, 0x194) + 1
    assert all(u(world, 0x974 + 4*i) == mask for i in range(32))
    for axis in range(3):
        for o in [0x20, 0x30]:
            expect = f(first, o + axis*4) + shift[axis]
            assert math.isclose(f(second, o + axis*4), expect, rel_tol=3e-6, abs_tol=3e-5)
        token = u(world, 0x900 + axis*4)
        previous = 0
        visited = []
        last = None
        while token:
            assert token not in visited and len(visited) < 4
            visited.append(token)
            base = 0x1000 if 0x1000 <= token < 0x1250 else 0xc000
            shape = first if base == 0x1000 else second
            offset = token-base
            assert offset in [0x19c+axis*40, 0x1b0+axis*40]
            assert u(shape, offset+4) == previous and u(shape, offset+8) == base
            minimum = offset == 0x19c+axis*40
            assert shape[offset+16] == int(minimum)
            coordinate = f(shape, offset+12)
            assert coordinate == f(shape, (0x20 if minimum else 0x30)+axis*4)
            key = (coordinate, not minimum)
            assert last is None or last <= key
            last = key
            previous, token = token, u(shape, offset)
        assert len(visited) == 4
    overlap = all(f(first, 0x20+j*4) <= f(second, 0x30+j*4)
                  and f(second, 0x20+j*4) <= f(first, 0x30+j*4) for j in range(3))
    allocated = bool(overlap and capacity and mask & (1 << second[0x19]))
    assert u(world, 0x910) == (0xd000 if allocated else 0)
    assert u(world, 0x914) == (0xd000 if capacity and not allocated else 0)
    expected_pair = bytearray(0x50)
    if allocated:
        struct.pack_into('<2I', expected_pair, 0x28, 0x1000, 0xc000)
    assert pair == expected_pair
    assert u(first, 0xd0) == u(second, 0xd4) == (0xd000 if allocated else 0)
    assert u(first, 0xd4) == u(second, 0xd0) == 0
    return dict(aabb_overlap=overlap, allocated=allocated)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('--profile', required=True, choices=['debug', 'release'])
    parser.add_argument('--upstream', type=Path, required=True)
    args = parser.parse_args()
    out = args.output.resolve()
    assert out.is_relative_to((ROOT/'local').resolve())
    out.mkdir(exist_ok=False)
    native = ROOT/'local/behavior/mesh-overlap-probe.exe'
    rust = ROOT/f'target/{args.profile}/grid-mesh-overlap.exe'
    upstream = args.upstream.resolve()
    report = json.loads(upstream.read_text())
    assert report['passed'] and report['physical_attachment'] and report['cases'] == 180
    assert digest(IMAGE) == 'aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514'
    results = []
    for row in report['results']:
        stem = upstream.parent/f"{row['index']:03}"
        seed_path = Path(str(stem)+'-original.bin')
        source_path = Path(str(stem)+'-input.bin')
        assert digest(seed_path) == row['output_sha256'] and digest(source_path) == row['input_sha256']
        if row['kind'] == 'source':
            assert digest(GAME/row['source']) == row['source_sha256']
            assert digest(GAME/row['csv']) == row['csv_sha256']
        seed, source = seed_path.read_bytes(), source_path.read_bytes()
        mesh_bytes = len(seed)-0x500-0x1120
        layer = seed[0x19]
        axis = row['index'] % 3
        width = struct.unpack_from('<f',seed,0x30+axis*4)[0]-struct.unpack_from('<f',seed,0x20+axis*4)[0]
        sign = -1 if row['index'] % 2 else 1
        partial = [0., 0., 0., 0.]
        partial[axis] = sign*width*.625
        separate = [0., 0., 0., 0.]
        separate[axis] = sign*(width*4+10)
        blocked_mask = [0, 1 << ((layer+1)%32), 0xffffffff][row['index']%3]
        controls = [('same', [0.]*4, 0xffffffff, 1),
                    ('partial', partial, 1 << layer, 1),
                    ('separate', separate, 0xffffffff, 1),
                    ('blocked', [0.]*4, blocked_mask, int(row['index']%3 != 2))]
        for label, shift, mask, capacity in controls:
            index = len(results)
            control = struct.pack('<4fII', *shift, mask, capacity)
            shift = struct.unpack('<4f', control[:16])
            np = out/f'{index:04}-native-input.bin'
            rp = out/f'{index:04}-rust-input.bin'
            np.write_bytes(b'MP01'+struct.pack('<I',mesh_bytes)+control+seed)
            rp.write_bytes(b'MO01'+bytes(4)+control+source)
            executions, answers = [], []
            for kind in ['original','repeat','rust']:
                target = out/f'{index:04}-{kind}.bin'
                e = run(rust if kind == 'rust' else native,
                        [rp,target] if kind == 'rust' else [IMAGE,np,target])
                executions.append(e)
                (out/'current-executions.json').write_text(json.dumps(dict(index=index,runs=executions),indent=2))
                assert e['exit'] == 0 and not e['stderr'], (index,e)
                if kind != 'rust':
                    assert 'code_unchanged=1 globals_restored=1 code_replacements=0 error=0' in e['stdout']
                answers.append(target.read_bytes())
            differences = [dict(offset=hex(o), original=answers[0][o:o+4].hex(), rust=answers[2][o:o+4].hex())
                           for o in range(0,len(answers[0]),4) if len({b[o:o+4] for b in answers}) != 1]
            (out/'current-differences.json').write_text(json.dumps(dict(index=index,differences=differences),indent=2))
            assert not differences, (index,differences[:12])
            result = check(answers[0],shift,mask,capacity)
            results.append(dict(index=index,upstream_index=row['index'],label=label,shift=shift,
                                mask=mask,capacity=capacity,**result,bytes=len(answers[0]),
                                native_input_sha256=digest(np),rust_input_sha256=digest(rp),
                                output_sha256=digest(out/f'{index:04}-original.bin'),executions=executions))
    guards = []
    for kind,exe in [('native',native),('rust',rust)]:
        sample=(out/f'0000-{kind}-input.bin').read_bytes()
        variants=[('short',sample[:-1]),('long',sample+b'\0')]
        for label,offset,value in [('tag',0,0),('shift-nan',8,0x7fc00000),('shift-inf',12,0x7f800000),('shift-bound',16,0x46200001),('shift-w',20,0x3f800000),('capacity',28,2)]:
            change=bytearray(sample);struct.pack_into('<I',change,offset,value);variants.append((label,change))
        for label,data in variants:
            inp=out/f'guard-{kind}-{label}.bin';target=out/f'guard-{kind}-{label}-output.bin';inp.write_bytes(data)
            e=run(exe,[IMAGE,inp,target] if kind=='native' else [inp,target])
            assert 0<e['exit']<256 and not target.exists(),(label,kind,e)
            if kind=='native':assert not e['stderr'] and 'globals_restored=1' in e['stdout']
            guards.append(dict(kind=kind,label=label,execution=e,no_output=True))
        existing=out/f'guard-{kind}-existing.bin';existing.write_bytes(b'preserve existing evidence');before=digest(existing)
        inp=out/f'0000-{kind}-input.bin';e=run(exe,[IMAGE,inp,existing] if kind=='native' else [inp,existing])
        assert 0<e['exit']<256 and digest(existing)==before
        guards.append(dict(kind=kind,label='existing',execution=e,existing_output_preserved=True))
    result=dict(passed=True,profile=args.profile,cases=len(results),observable_bytes=sum(r['bytes'] for r in results),
                allocated_pairs=sum(r['allocated'] for r in results),guards=guards,guard_count=len(guards),results=results,
                upstream=str(upstream.relative_to(ROOT)),upstream_sha256=digest(upstream),
                executables={str(p.relative_to(ROOT)):digest(p) for p in [IMAGE,native,rust]},
                scope='Whole B460F0/ABC7B0/B16D70/B15030/B15750/B14EA0 on one new static clone and a checked F217 dynamic seed. Native relocates verified seed objects; Rust independently reconstructs from source CM05 input. One region, two shapes, zero/one supplied pair slot; no narrow phase, contact constraints, world advance or runtime barrier response.')
    (out/'validation.json').write_text(json.dumps(result,indent=2))
    print(json.dumps({k:result[k] for k in ['passed','cases','observable_bytes','allocated_pairs','guard_count']}))


if __name__ == '__main__':
    main()
