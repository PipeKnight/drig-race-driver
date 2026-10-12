"""Compare unchanged original CQTC decoders with independently rebuilt source.

Private complete constructor contexts/repeats and decoded geometry projections.
Tree traversal is not claimed by this decoder validation.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
import struct
import subprocess
from datetime import datetime, timezone


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def invoke(args, good=True):
    result = subprocess.run([str(a) for a in args], capture_output=True, text=True, timeout=60)
    if (result.returncode == 0) != good:
        raise AssertionError((args, result.returncode, result.stdout, result.stderr))
    return result.stdout + result.stderr


def f32(x):
    return struct.unpack('<f', struct.pack('<f', x))[0]


def geometry(b):
    bounds = struct.unpack_from('<6f', b)
    nt, nv, nm, vo, no, to, ro = struct.unpack_from('<7I', b, 24)
    scale = [f32(f32(bounds[j + 3] - bounds[j]) / d) for j, d in enumerate([16777216, 65536, 16777216])]
    out = bytearray()
    for i in range(nv):
        v = b[vo + i * 8:vo + i * 8 + 8]
        p = [int.from_bytes(v[:3], 'big'), int.from_bytes(v[3:5], 'big'), int.from_bytes(v[5:], 'big')]
        out.extend(struct.pack('<4f', *[f32(f32(p[j] * scale[j]) + bounds[j]) for j in range(3)], 1))
    for i in range(nt):
        t = b[to + i * 7:to + i * 7 + 7]
        a = int.from_bytes(t[:3], 'big')
        out.extend(struct.pack('<3I', a, a + t[4] + ((t[3] & 240) << 4), a + t[5] + ((t[3] & 15) << 8)))
        out.extend(b[52 + 4 * t[6]:56 + 4 * t[6]])
    nodes = [int.from_bytes(b[i:i + 3], 'big') for i in range(no, to, 3)]
    refs, cursor = {}, ro
    while cursor < len(b):
        start = cursor - ro
        v = int.from_bytes(b[cursor:cursor + 3], 'big')
        assert cursor + 3 <= len(b)
        cursor += 3
        base = v & 0x7fffff
        ids = [base]
        if v & 0x800000 == 0:
            while True:
                assert cursor + 2 <= len(b)
                delta = int.from_bytes(b[cursor:cursor + 2], 'big')
                cursor += 2
                ids.append(base + (delta & 0x7fff))
                if delta & 0x8000:
                    break
        assert all(x < nt for x in ids)
        refs[start] = ids
    for n in nodes:
        assert n == 0xffffff or ((n & 0x7fffff) in refs if n & 0x800000 else n + 4 <= len(nodes))
    return bytes(out), dict(vertices=nv, triangles=nt, materials=nm, nodes=len(nodes), lists=len(refs), references=sum(map(len, refs.values())))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('native', type=Path)
    ap.add_argument('debug', type=Path)
    ap.add_argument('release', type=Path)
    ap.add_argument('output', type=Path)
    ap.add_argument('--game', type=Path, default=Path(r'C:\Program Files (x86)\Steam\steamapps\common\Grid'))
    args = ap.parse_args()
    assert args.output.parts[0] == 'local'
    args.output.mkdir(parents=True, exist_ok=False)
    image = Path('local/compatibility/grid-unpacked.exe')
    assert sha(image) == 'aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514'
    sources = sorted((args.game / 'tracks/europe/donington').glob('route_*/boundarylines.cqtc'))
    assert len(sources) == 4
    pinned = {str(p): sha(p) for p in [args.native, args.debug, args.release, image, *sources]}
    rows, complete, selected = [], 0, 0
    for route, source in enumerate(sources):
        b = source.read_bytes()
        expected, counts = geometry(b)
        for profile, executable in [('debug', args.debug), ('release', args.release)]:
            rust = args.output / f'route-{route}-{profile}.bin'
            text = invoke([executable, source, rust])
            assert rust.read_bytes() == expected, (route, profile)
            rows.append(dict(kind='rust', route=route, profile=profile, output=str(rust), sha256=sha(rust), stdout=text))
        nm = counts['materials']
        for fill in [0, 0xcd]:
            for sheet in [0, (1 << nm) - 1]:
                name = f'route-{route}-fill-{fill}-sheet-{sheet}'
                request = args.output / (name + '.input')
                request.write_bytes(struct.pack('<4I', 0x31305143, len(b), fill, sheet) + b)
                outputs = []
                for repeat in [0, 1]:
                    output = args.output / (name + f'-{repeat}.bin')
                    text = invoke([args.native, image, request, output])
                    assert 'code_unchanged=1 globals_restored=1 code_replacements=0 error=0' in text
                    raw = output.read_bytes()
                    assert len(raw) == 0x624 + len(b) + len(expected)
                    assert raw[0x624 + len(b):] == expected, name
                    canonical = bytearray(b)
                    for j in range(4):
                        struct.pack_into('<I', canonical, 36 + j * 4, 0x100000 + struct.unpack_from('<I', b, 36 + j * 4)[0])
                    keys = [b[52 + j * 4:56 + j * 4] for j in range(nm)]
                    for j, key in enumerate(keys):
                        struct.pack_into('<I', canonical, 52 + j * 4, 0x4000 + 0x3c * keys.index(key))
                    assert raw[0x624:0x624 + len(b)] == canonical
                    assert struct.unpack_from('<I', raw, 0)[0] == 0x700a
                    assert struct.unpack_from('<I', raw, 0x10)[0] == 10
                    assert struct.unpack_from('<I', raw, 0x270 + 0x94 + 0x80)[0] == 28
                    outputs.append(raw)
                    rows.append(dict(kind='native', route=route, fill=fill, sheet=sheet, repeat=repeat, request=str(request), request_sha256=sha(request), output=str(output), sha256=sha(output), stdout=text))
                assert outputs[0] == outputs[1]
                complete += len(outputs[0])
                selected += len(expected)
        rows.append(dict(kind='source', route=route, path=str(source), sha256=sha(source), **counts))
    b = sources[0].read_bytes()
    h = struct.pack('<4I', 0x31305143, len(b), 0, 1)
    mutants = {}
    for label, offset, value in [('zero-count', 24, 0), ('negative-count', 24, 0xffffffff), ('node-overflow', 44, 0xfffffffc), ('vertex-offset', 36, 0)]:
        v = bytearray(b); struct.pack_into('<I', v, offset, value); mutants[label] = (h + v, True)
    v = bytearray(b); struct.pack_into('<f', v, 0, math.nan); mutants['nan-bounds'] = (h + v, True)
    to = struct.unpack_from('<I', b, 44)[0]
    v = bytearray(b); v[to:to + 3] = b'\xff\xff\xff'; mutants['vertex-index'] = (h + v, True)
    v = bytearray(b); v[to + 6] = 8; mutants['material-index'] = (h + v, True)
    mutants['truncated'] = (h + b[:-1], False)
    mutants['extra-byte'] = (h + b + b'\x00', False)
    mutants['invalid-fill'] = (struct.pack('<4I', 0x31305143, len(b), 1, 1) + b, False)
    mutants['invalid-sheet'] = (struct.pack('<4I', 0x31305143, len(b), 0, 2) + b, False)
    guards = []
    for name, (request_bytes, rust_bad) in mutants.items():
        request = args.output / (name + '.input'); request.write_bytes(request_bytes)
        output = args.output / (name + '.bin')
        invoke([args.native, image, request, output], False)
        assert not output.exists()
        guards.append('native:' + name)
        if rust_bad:
            source = args.output / (name + '.cqtc'); source.write_bytes(request_bytes[16:])
            for profile, exe in [('debug', args.debug), ('release', args.release)]:
                invoke([exe, source, output], False)
                assert not output.exists(); guards.append(profile + ':' + name)
    existing = args.output / 'existing.bin'; existing.write_bytes(b'KEEP')
    good_request = args.output / 'route-0-fill-0-sheet-0.input'
    invoke([args.native, image, good_request, existing], False)
    for exe in [args.debug, args.release]: invoke([exe, sources[0], existing], False)
    assert existing.read_bytes() == b'KEEP'; guards.append('all:existing-output')
    missing = args.output / 'wrong-image-output.bin'
    invoke([args.native, args.release, good_request, missing], False)
    assert not missing.exists(); guards.append('native:image-identity')
    assert all(sha(p) == digest for p, digest in pinned.items())
    result = dict(time_utc=datetime.now(timezone.utc).isoformat(), versions={'python': '3.12.14', 'rust': '1.99.0', 'msvc': '14.44.35207'}, inputs=pinned, cases=16, complete_bytes_per_original_run=complete, decoded_bytes_across_native_cases=selected, decoded_bytes_per_rust_profile=sum(len(geometry(p.read_bytes())[0]) for p in sources), guards=guards, rows=rows, scope='whole CQTC constructor and all vertex/triangle/material decoders; tree traversal not executed')
    (args.output / 'validation.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({k: result[k] for k in ['cases', 'complete_bytes_per_original_run', 'decoded_bytes_across_native_cases', 'decoded_bytes_per_rust_profile', 'guards']}))


if __name__ == '__main__':
    main()
