"""Private full plane-intersection/volume proof, including source cell/box pairs."""
import argparse
import collections
import json
import math
import struct
import subprocess
from pathlib import Path
import validate_deformation_layout as layout
from prepare_deformation_initialization import configuration

ROOT = layout.ROOT
ROW = 244 + 128 * 16 + 256 * 12
ANSWER = 16 + 128 * 16 + 256 * 12
PROBE = ROOT / 'local/behavior/deformation-volume-probe.exe'
f = layout.f32


def box(minimum, maximum):
    normals, offsets = [], []
    for a in range(3):
        for sign in [-1., 1.]:
            n = [0.] * 4; n[a] = sign
            normals.append(n); offsets.append(f(sign * (minimum[a] if sign < 0 else maximum[a])))
    return normals, offsets


def source_cases():
    # Baseline layout is test evidence only; independently reconstruct it first.
    path = ROOT / 'local/behavior/deformation-initialization-source-20261013-draft-01/original.bin'
    assert layout.digest(path) == '1ed4cc2f2fc89e9b016981773413f73846ac19a37420516ac9024de4f93d6ee1'
    descriptor = path.read_bytes()[8 + layout.SIZE:8 + 2 * layout.SIZE]
    dims = struct.unpack_from('<3I', descriptor, 0xa414)
    spacing = struct.unpack_from('<f', descriptor, 0xa420)[0]
    minimum = struct.unpack_from('<4f', descriptor, 0xa430)
    settings, identity = configuration()
    rows = []
    for z in range(dims[2] - 1):
        for y in range(dims[1] - 1):
            for x in range(dims[0] - 1):
                lo = [f(f(v * spacing) + minimum[a]) for a, v in enumerate([x, y, z])]
                hi = [f(f((v + 1) * spacing) + minimum[a]) for a, v in enumerate([x, y, z])]
                base, limits = box(lo, hi)
                for b in range(struct.unpack_from('<I', settings, 0x1000)[0]):
                    matrix = struct.unpack_from('<16f', settings, b * 0x80)
                    sizes = struct.unpack_from('<4f', settings, b * 0x80 + 0x40)
                    normals, offsets = list(base), list(limits)
                    for axis in range(3):
                        for sign in [-1., 1.]:
                            n = [f(sign * matrix[axis * 4 + a]) for a in range(4)]
                            dot = f(f(f(n[1] * matrix[13]) + f(n[0] * matrix[12]))
                                    + f(f(n[3] * matrix[15]) + f(n[2] * matrix[14])))
                            normals.append(n); offsets.append(f(dot + sizes[axis]))
                    rows.append(dict(name=f'source-{z}-{y}-{x}-box{b}', planes=normals, offsets=offsets))
    return rows, {str(path): layout.digest(path), identity['source']: identity['sha256']}


def cases():
    rows = []
    for size in [.01, .1, .25, 1., 2.5]:
        for center in [[0., 0., 0.], [-2.5, .1, 3.]]:
            n, d = box([f(c - size) for c in center], [f(c + size) for c in center])
            rows.append(dict(name=f'box-{size}-{center}', planes=n, offsets=d))
            for shift in [0., .05, size, 2.5 * size]:
                n2, d2 = box([f(c - size + shift) for c in center], [f(c + size + shift) for c in center])
                rows.append(dict(name=f'pair-{size}-{center}-{shift}', planes=n + n2, offsets=d + d2))
    source, identities = source_cases()
    return rows + source, identities


def request(rows):
    out = bytearray(b'DV01' + struct.pack('<I', len(rows)))
    for row in rows:
        n = len(row['planes']); assert 3 <= n <= 12
        out.extend(struct.pack('<I', n))
        for p in row['planes'] + [[0.] * 4] * (12 - n): out.extend(struct.pack('<4f', *p))
        out.extend(struct.pack('<12f', *(row['offsets'] + [0.] * (12 - n))))
        # Opaque output padding must be preserved even when it contains NaNs.
        out.extend(bytes((i * 73 + 41) % 256 for i in range(128 * 16 + 256 * 12)))
    assert len(out) == 8 + len(rows) * ROW
    return bytes(out)


def guard_checks(out, payload, mutations, profile, binary, probe, framing_error):
    tests = [('magic', b'BAD!' + payload[4:], 7), ('zero_count', payload[:4] + bytes(4), 7),
             ('truncated', payload[:-1], framing_error), ('trailing', payload + b'x', framing_error)]
    for mutation in mutations:
        name, at, value, *override = mutation
        data = bytearray(payload); struct.pack_into('<I', data, at, value)
        tests.append((name, bytes(data), override[0] if override else 9))
    checks = []
    for name, data, expected in tests:
        source = out / f'guard-{name}-input.bin'; source.write_bytes(data); runs = {}
        for kind in ['original','repeat','rust']:
            exe = probe if kind != 'rust' else ROOT / f'target/{profile}/{binary}.exe'
            target = out / f'guard-{name}-{kind}.bin'
            run = subprocess.run([str(exe), *([str(layout.IMAGE)] if kind != 'rust' else []), str(source), str(target)], cwd=ROOT, capture_output=True,text=True,timeout=120)
            runs[kind] = dict(exit=run.returncode,stdout=run.stdout,stderr=run.stderr,output_exists=target.exists())
            assert run.returncode == (1 if kind=='rust' else expected) and not target.exists(), (name,kind,runs[kind])
        checks.append(dict(name=name,expected_original_exit=expected,executions=runs))
    for kind in ['original','rust']:
        source=out/f'guard-existing-{kind}-input.bin';source.write_bytes(payload)
        target=out/f'guard-existing-{kind}.bin';target.write_bytes(b'owned sentinel')
        exe=probe if kind=='original' else ROOT/f'target/{profile}/{binary}.exe'
        run=subprocess.run([str(exe),*([str(layout.IMAGE)] if kind=='original' else []),str(source),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=120)
        assert run.returncode==(24 if kind=='original' else 1) and target.read_bytes()==b'owned sentinel'
        checks.append(dict(name='existing-'+kind,exit=run.returncode,preserved=True,stdout=run.stdout,stderr=run.stderr))
    (out/'guards.json').write_text(json.dumps(checks,indent=2));return checks


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('output', type=Path)
    parser.add_argument('--profile', choices=['debug', 'release'], required=True)
    parser.add_argument('--pilot', action='store_true')
    args = parser.parse_args()
    out = args.output.resolve(); assert out.is_relative_to((ROOT / 'local').resolve())
    out.mkdir(exist_ok=False)
    rows, identities = cases()
    if args.pilot: rows = rows[:50] + rows[50:150]
    coverage = collections.Counter(); reports = []
    for start in range(0, len(rows), 64):
        batch = rows[start:start + 64]; input_ = out / f'{start}-input.bin'; input_.write_bytes(request(batch))
        answers, runs = {}, {}
        for kind in ['original', 'repeat', 'rust']:
            exe = PROBE if kind != 'rust' else ROOT / f'target/{args.profile}/grid-deformation-volume.exe'
            target = out / f'{start}-{kind}.bin'
            cmd = [str(exe), *([str(layout.IMAGE)] if kind != 'rust' else []), str(input_), str(target)]
            run = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, timeout=120)
            runs[kind] = dict(exit=run.returncode, stdout=run.stdout, stderr=run.stderr, executable_sha256=layout.digest(exe))
            (out / f'{start}-execution.json').write_text(json.dumps(runs, indent=2))
            assert run.returncode == 0 and not run.stderr, runs[kind]
            if kind != 'rust': assert 'owned_canaries=1 full_image_unchanged=1 error=0' in run.stdout
            answers[kind] = target.read_bytes()
        native, rust = answers['original'], answers['rust']
        assert len(native) == 8 + len(batch) * ANSWER and native == answers['repeat']
        diffs = [dict(case=batch[i // ANSWER]['name'], offset=hex(i % ANSWER), native=native[8+i:12+i].hex(), rust=rust[8+i:12+i].hex())
                 for i in range(0,len(native)-8,4) if native[8+i:12+i] != rust[8+i:12+i]]
        item = dict(start=start, cases=len(batch), bytes=len(native)-8, differences=diffs, executions=runs)
        (out / f'{start}-validation.json').write_text(json.dumps(item, indent=2)); reports.append(item)
        assert not diffs, diffs[:8]
        for i in range(len(batch)):
            n, t, volume = struct.unpack_from('<IId', native, 8 + i * ANSWER)
            assert n <= 128 and t <= 256 and math.isfinite(volume) and volume >= 0
            coverage['vertices'] += n; coverage['triangles'] += t; coverage['positive_volume_cases'] += volume > 0
    guards=guard_checks(out,request([rows[0]]),[('plane_count',8,13),('nan_normal',12,0x7fc00000),('normal_w',24,0x3f800000),('large_normal',12,0x40400000),('nan_offset',204,0x7fc00000)],args.profile,'grid-deformation-volume',PROBE,7)
    report = dict(profile=args.profile, cases=len(rows), bytes=sum(r['bytes'] for r in reports),guards=guards,
                  identities=identities, coverage=dict(coverage), reports=reports,
                  files={p.name: dict(size=p.stat().st_size,sha256=layout.digest(p)) for p in sorted(out.iterdir()) if p.is_file()},
                  scope='Whole original A15F50/9CC790 and independent Rust, complete fixed output pools including writes beyond final counts; source-derived cell/box inputs. No FEMS weighted query/node initialization/runtime integration.')
    (out / 'validation.json').write_text(json.dumps(report,indent=2))
    print(json.dumps({k:report[k] for k in ['profile','cases','bytes','coverage']}))


if __name__ == '__main__': main()
