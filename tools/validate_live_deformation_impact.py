"""Compare captured nested solver output with isolated original and existing Rust."""
import argparse
import json
from pathlib import Path
import struct
import subprocess
from capture_reference_spawn import private, REFERENCE
from prepare_deformation_impact import PROBE_SHA, DESCRIPTOR, SETTINGS
from validate_deformation_impact import pack
from validate_steering_control import digest, PE_SHA

ROOT = Path(__file__).resolve().parents[1]


def exact_hex(row, field, size):
    value = bytes.fromhex(row[field])
    if len(value) != size:
        raise ValueError(f'{field} size mismatch')
    return value


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('capture', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--profile', choices=('debug', 'release'), default='debug')
    args = parser.parse_args()
    source, out = private(args.capture), private(args.output)
    if source.stat().st_size > 8*1024*1024:
        raise ValueError('Capture exceeds bound')
    capture = json.loads(source.read_text(encoding='utf-8'))
    if not (capture['completed'] and capture['hardware_breakpoints'] and capture['solver_code_exact']
            and capture['debugger_exit'] == 0 and capture['session']['ExeSHA256'] == REFERENCE):
        raise ValueError('Unverified nested capture')
    image, probe = ROOT/'local/compatibility/grid-unpacked.exe', ROOT/'local/behavior/deformation-impact-probe.exe'
    native = ROOT/f'target/{args.profile}/grid-deformation-impact.exe'
    if digest(image) != PE_SHA or digest(probe) != PROBE_SHA:
        raise ValueError('Reference/probe hash changed')
    cases, expected, details = [], [], []
    for outer in capture['cases']:
        solver = outer.get('solver')
        if solver is None:
            continue
        count = solver['record_count']
        if not 1 <= count <= 400 or solver['receiver'] != outer['descriptor']:
            raise ValueError('Nested count/receiver mismatch')
        stack = exact_hex(solver, 'stack_hex', 16)
        ret, _, stack_count, gain = struct.unpack('<4I', stack)
        if (ret, stack_count, f'{gain:08x}') != (0x9eb570, count, solver['gain_word']):
            raise ValueError('Nested entry ABI mismatch')
        if solver['return_registers'][:2] != [ret, solver['entry_registers'][1]+16]:
            raise ValueError('Nested return ABI mismatch')
        d = bytearray(exact_hex(solver, 'descriptor_before_hex', DESCRIPTOR))
        result = bytearray(exact_hex(solver, 'descriptor_after_hex', DESCRIPTOR))
        # Sole normalization: process-local settings address. Both diagnostics
        # already clear this pointer in their output; no numeric field changes.
        d[0xa4f8:0xa4fc] = result[0xa4f8:0xa4fc] = bytes(4)
        records = exact_hex(solver, 'records_hex', count*48)
        indices = [struct.unpack_from('<hh', records, i*48+0x20) for i in range(count)]
        if not all(0 <= node < 400 and 0 <= contact < outer['contact_count'] for node,contact in indices):
            raise ValueError('Generated node/contact ordinal outside scope')
        scalar = [struct.unpack_from('<f', records, i*48+0x24)[0] for i in range(count)]
        cases.append(dict(d=d, s=exact_hex(solver,'settings_hex',SETTINGS),
                          g=exact_hex(solver,'globals_hex',24), clip=exact_hex(solver,'clip_hex',1)[0],
                          r=records, gain=struct.unpack('<f', struct.pack('<I',gain))[0]))
        expected.append(result)
        details.append(dict(outer_case=outer['index'], contacts=outer['contact_count'], records=count,
                            unique_nodes=len(set(n for n,_ in indices)), scalar_range=[min(scalar),max(scalar)]))
    if not 1 <= len(cases) <= 32:
        raise ValueError('No bounded nested solver cases')
    out.mkdir(parents=True, exist_ok=False)
    request = out/'input.bin'
    request.write_bytes(pack(cases))
    live = struct.pack('<II',0x314d4947,len(cases)) + b''.join(expected)
    (out/'live-expected.bin').write_bytes(live)
    report = dict(capture_sha256=digest(source), input_sha256=digest(request),
                  analysis_sha256=PE_SHA, probe_sha256=PROBE_SHA, native_sha256=digest(native), profile=args.profile,
                  cases=details, normalization='Only descriptorA4F8 process-local settings pointer is zeroed.',
                  compared_descriptor_words=len(cases)*DESCRIPTOR//4, execution={}, comparisons={},
                  limits='Actual generated solver inputs; debugger changes timing. No whole contact-producer Rust or full collision simulation equivalence.')
    outputs = {}
    for label, command in [('original',[str(probe),str(image)]), ('original_repeat',[str(probe),str(image)]), ('rust',[str(native)])]:
        target = out/f'{label}.bin'
        run = subprocess.run(command+[str(request),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=60)
        report['execution'][label] = dict(exit=run.returncode,stdout=run.stdout,stderr=run.stderr)
        if run.returncode == 0:
            outputs[label] = target.read_bytes()
    for label, data in outputs.items():
        if len(data) != len(live):
            raise ValueError('Output size differs')
        differences = [dict(case=(i-8)//DESCRIPTOR,offset=hex((i-8)%DESCRIPTOR),live=live[i:i+4].hex(),replay=data[i:i+4].hex())
                       for i in range(8,len(live),4) if live[i:i+4] != data[i:i+4]]
        report['comparisons'][label] = dict(mismatched_words=len(differences),first_differences=differences[:20],sha256=digest(out/f'{label}.bin'))
    report['original_repeat_identical'] = outputs.get('original') == outputs.get('original_repeat') and 'original' in outputs
    report['passed'] = len(outputs)==3 and report['original_repeat_identical'] and all(c['mismatched_words']==0 for c in report['comparisons'].values())
    with (out/'validation.json').open('x',encoding='utf-8') as stream:
        json.dump(report,stream,indent=2,allow_nan=False)
    print(json.dumps(report,indent=2))
    if not report['passed']:
        raise RuntimeError('Live solver replay comparison failed; retained report contains differences')


if __name__ == '__main__':
    main()
