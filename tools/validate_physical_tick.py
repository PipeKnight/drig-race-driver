"""Whole body advance followed by whole supplied-contact physical frame."""
import argparse
import copy
import json
from pathlib import Path
import struct
import subprocess
from capture_reference_spawn import private, ANALYSIS
from validate_contact_bias import compare, execute
from validate_corrected_impact_frame import pack as corrected_pack, PROBE as CORRECTED_PROBE
from validate_impact_frame import decode, layouts
from validate_main_contact_response import fput, uput, fget
from validate_steering_control import digest

ROOT = Path(__file__).resolve().parents[1]
MAGIC = 0x33524650
PROBE = '16d1c64f4ef1246c1b6a026ba17d42b8a9ca14d745dbf524ae6c002ccc9b67b7'


def pack(cases):
    data = corrected_pack(cases)
    uput(data, 0, MAGIC)
    return data


def semantic(cases, outputs):
    checks = dict(previous_center=True, previous_publication=True, response_cleared=True,
                  zero_iterations_rows_contacts_retained=True, zero_iterations_translation=True,
                  finished_forces_cleared=True)
    activity = dict(advanced_bodies=0, moved_bodies=0, rotated_bodies=0, zero_angular_bodies=0, tiny_angular_bodies=0, zero_iterations=0,
                    sleeping=0, capacity_failed=0, finished=0, empty_contacts=0)
    for c, o in zip(cases, outputs):
        zero = c['iterations'] == 0
        finished = o['started'] and o['status'] == 0
        activity['zero_iterations'] += zero
        activity['finished'] += bool(finished)
        activity['sleeping'] += not o['started'] and o['status'] == 1
        activity['capacity_failed'] += o['started'] and o['status'] == 1
        activity['empty_contacts'] += not c['contacts']
        if zero:
            checks['zero_iterations_rows_contacts_retained'] &= o['rows'] == c['rows'] and o['contacts'] == [raw for _, raw in c['contacts']]
        for before, after in zip(c['bodies'], o['bodies']):
            raw = before['raw']
            activity['advanced_bodies'] += 1
            angular = [fget(raw, 0x210+4*i) for i in range(4)]
            activity['zero_angular_bodies'] += all(v == 0. for v in angular)
            activity['tiny_angular_bodies'] += any(v != 0. for v in angular) and max(abs(v) for v in angular) < 1e-18
            activity['moved_bodies'] += raw[0x1c0:0x1d0] != after[0x1c0:0x1d0]
            activity['rotated_bodies'] += raw[0x190:0x1c0] != after[0x190:0x1c0]
            checks['previous_center'] &= after[0x270:0x2b0] == raw[0x190:0x1d0]
            checks['previous_publication'] &= after[0x2b0:0x2f0] == raw[0x1d0:0x210]
            checks['response_cleared'] &= after[0x360:0x370] == bytes(16)
            if zero:
                expected = bytearray(16)
                for i in range(4):
                    rounded_product = struct.unpack('<f', struct.pack('<f', c['parameters'][0] * fget(raw, 0x220 + 4*i)))[0]
                    fput(expected, 4*i, rounded_product + fget(raw, 0x1c0 + 4*i))
                checks['zero_iterations_translation'] &= after[0x1c0:0x1d0] == expected
            if finished:
                checks['finished_forces_cleared'] &= after[0x250:0x270] == bytes(32) and not after[0x40d] and not after[0x40e]
    return checks, activity


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    parser.add_argument('--profile', choices=['debug', 'release'], default='debug')
    args = parser.parse_args()
    image = ROOT/'local/compatibility/grid-unpacked.exe'
    probe = ROOT/'local/behavior/physical-tick-probe.exe'
    rust = ROOT/f'target/{args.profile}/grid-physical-tick.exe'
    if digest(image) != ANALYSIS or digest(probe) != PROBE:
        raise ValueError('Image/probe identity')
    out = private(args.output)
    out.mkdir(parents=True, exist_ok=False)
    prior = ROOT/('local/behavior/corrected-impact-frame-20261010-debug-02' if args.profile == 'debug' else 'local/behavior/corrected-impact-frame-20261010-release-final-03')
    upstream = prior/'input.bin'
    baseline = (prior/'original.bin').read_bytes()
    if digest(upstream) != '7144ff633938b118bd4ff832200b62cf357ce45f9d8a404b927465708592a130' or any((prior/f'{label}.bin').read_bytes() != baseline for label in ['repeat', 'rust']):
        raise ValueError('Prior corrected frame identity/agreement')
    corrected_probe = ROOT/'local/behavior/corrected-impact-frame-probe.exe'
    if digest(corrected_probe) != CORRECTED_PROBE:
        raise ValueError('Corrected frame probe identity')
    upstream_runs = {}
    for label, command in [('original', [str(corrected_probe), str(image)]), ('rust', [str(ROOT/f'target/{args.profile}/grid-corrected-physical-frame.exe')])]:
        data, run = execute(command, upstream, out/f'upstream-{label}.bin')
        run.update(compare(data, baseline)); upstream_runs[label] = run
        if not run['passed']:
            raise ValueError('Fresh corrected frame regression mismatch')
    cases = layouts((ROOT/f'local/behavior/impact-frame-20261010-{args.profile}-final-04/input.bin').read_bytes())
    for i, c in enumerate(cases):
        c['correction'] = [[0,1,2,4,8,16,32][i%7], [.01,.1,1.][i%3], [.0001,0.,-.0001][(i//3)%3], 0. if i%2 else 1., [0.,.5,1.][(i//9)%3]]
    if corrected_pack(cases) != upstream.read_bytes():
        raise ValueError('Request reconstruction differs')
    # Added authored advance-stage controls exercise angular zero/tiny bypasses,
    # negative-zero translation and W-lane angular magnitude; no live provenance.
    angular_controls = [[0.,0.,0.,0.], [-0.,0.,0.,0.], [1e-20,0.,0.,0.], [0.,-1e-20,0.,0.],
                        [1e-10,0.,0.,0.], [0.,0.,0.,2.], [1.,-2.,3.,0.], [0.,1.,0.,0.]]
    for i in range(16):
        c = copy.deepcopy(cases[3]); c['iterations'] = 0
        for b in c['bodies']:
            for j, v in enumerate(angular_controls[i%8]): fput(b['raw'], 0x210+4*j, v)
            for j, v in enumerate([-0. if i<8 else 2., 0., -.5, 0.]): fput(b['raw'], 0x220+4*j, v)
            fput(b['raw'], 0x1c0, -0.)
        cases.append(c)
    source = out/'input.bin'; source.write_bytes(pack(cases))
    report = dict(profile=args.profile, analysis_sha256=ANALYSIS, probe_sha256=PROBE, rust_sha256=digest(rust), input_sha256=digest(source), cases=len(cases), upstream_runs=upstream_runs,
                  scope='Whole B13E10 per body, no group-head orthonormalization/shapes; supplied post-advance contacts then whole B7B850 main+optional, natural absent callbacks/joints/links/wakeup. Authored state, not whole world/live tick.', runs={}, guards={}, live_verification=False)
    commands = [('original', [str(probe), str(image)]), ('repeat', [str(probe), str(image)]), ('rust', [str(rust)])]
    original = None
    for label, command in commands:
        data, run = execute(command, source, out/f'{label}.bin')
        if original is None: original = data
        run.update(compare(data, original)); report['runs'][label] = run
    framed = bytearray(original); uput(framed, 0, 0x31524650)
    checks, activity = semantic(cases, decode(framed, cases))
    report.update(semantic_checks=checks, activity=activity)
    base = copy.deepcopy(cases[3]); good = pack([base])
    bad = dict(truncated=good[:-1], trailing=good+b'\0', zero_cases=good[:4]+bytes(4)+good[8:])
    for name, mutate in [('excess_correction_iterations', lambda c:c['correction'].__setitem__(0,33)), ('nan_softness', lambda c:c['correction'].__setitem__(1,float('nan'))),
                         ('nan_scale', lambda c:c['correction'].__setitem__(2,float('nan'))), ('nan_threshold', lambda c:c['correction'].__setitem__(3,float('nan'))), ('nan_relaxation', lambda c:c['correction'].__setitem__(4,float('nan'))),
                         ('body_callback', lambda c:uput(c['bodies'][0]['raw'],0x418,1)), ('body_links', lambda c:uput(c['bodies'][0]['raw'],0x324,1)), ('stale_metadata', lambda c:c['contacts'][0][1].__setitem__(0x70,1)),
                         ('nan_basis', lambda c:fput(c['bodies'][0]['raw'],0x190,float('nan')))]:
        c = copy.deepcopy(base); mutate(c); bad[name] = pack([c])
    for name, request in bad.items():
        source = out/f'guard-{name}.bin'; source.write_bytes(request); report['guards'][name] = {}
        for label, command in [commands[0], commands[2]]:
            target = out/f'guard-{name}-{label}.bin'
            run = subprocess.run(command+[str(source),str(target)], cwd=ROOT, capture_output=True, text=True, timeout=20)
            report['guards'][name][label] = dict(exit=run.returncode, stderr=run.stderr, passed=0<run.returncode<256 and 'physical_tick_fault=' not in run.stderr and (label=='original' or not target.exists()))
    report['passed'] = all(checks.values()) and all(activity.values()) and all(r['passed'] for r in report['runs'].values()) and all(r['passed'] for r in upstream_runs.values()) and all(r['passed'] for g in report['guards'].values() for r in g.values())
    (out/'validation.json').write_text(json.dumps(report, indent=2)); print(json.dumps(report, indent=2))
    if not report['passed']: raise RuntimeError('Whole body-advance/frame comparison failed; inspect private evidence')


if __name__ == '__main__':
    main()
