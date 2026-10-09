"""Private original-prefix replay against actual generated record boundaries."""
import argparse
import json
from pathlib import Path
import struct
import subprocess
from capture_reference_spawn import private, REFERENCE, ANALYSIS
from validate_steering_control import digest

ROOT = Path(__file__).resolve().parents[1]
PROBE_SHA = '93cc9384c0a3718e0876e009f01afcce60dd61bd3531bed86fd91782a730aa31'


def blob(row, key, size):
    data = bytes.fromhex(row[key])
    if len(data) != size:
        raise ValueError(f'{key} size mismatch')
    return data


def pack(rows):
    parts = [struct.pack('<II', 0x31475049, len(rows))]
    for row in rows:
        parts.extend([struct.pack('<III', row['contact_count'], int(row['scalar_word'],16), int(row['linked'],16)),
                      blob(row,'descriptor_before_hex',0xa500),blob(row,'settings_hex',0x1050),
                      blob(row,'linked_before_hex',0x424),blob(row,'body_1630_hex',16),blob(row,'body_b20_hex',16),
                      blob(row,'producer_globals_hex',32),blob(row,'producer_corner_scale_hex',4),
                      blob(row,'producer_static_mass_hex',4)])
        if len(row['contacts']) != row['contact_count']:
            raise ValueError('Contact array mismatch')
        for contact in row['contacts']:
            counterpart = int(contact['counterpart'],16) != 0
            parts.extend([blob(contact,'before_hex',0x80),struct.pack('<I',counterpart),
                          blob(contact,'counterpart_360_hex',16) if counterpart else bytes(16)])
    return b''.join(parts)


def decode(data, rows):
    if data[:8] != struct.pack('<II',0x31475049,len(rows)):
        raise ValueError('Output header mismatch')
    cursor, results = 8, []
    for row in rows:
        count, = struct.unpack_from('<I',data,cursor)
        if count > 400 or cursor+4+0xa500+count*48 > len(data):
            raise ValueError('Output row bound')
        cursor += 4
        descriptor = data[cursor:cursor+0xa500]
        cursor += 0xa500
        records = data[cursor:cursor+count*48]
        cursor += count*48
        solver = row.get('solver')
        expected_count = solver['record_count'] if solver else 0
        expected_records = blob(solver,'records_hex',expected_count*48) if solver else b''
        expected_descriptor = bytearray(blob(solver,'descriptor_before_hex',0xa500) if solver else blob(row,'descriptor_after_hex',0xa500))
        expected_descriptor[0xa4f8:0xa4fc] = bytes(4)
        changes = [hex(i) for i in range(0,0xa500,4) if descriptor[i:i+4] != expected_descriptor[i:i+4]]
        differences = []
        for i in range(min(count,expected_count)):
            # Prefix writes 44 bytes. Record2C is uninitialized stack storage.
            for offset in range(0,44,4):
                p = i*48+offset
                if records[p:p+4] != expected_records[p:p+4]:
                    differences.append(dict(record=i,offset=hex(offset),actual=records[p:p+4].hex(),live=expected_records[p:p+4].hex()))
        results.append(dict(contacts=row['contact_count'],records=count,expected_records=expected_count,
                            descriptor_word_differences=len(changes),first_descriptor_differences=changes[:20],
                            record_word_differences=len(differences),first_record_differences=differences[:20],
                            compared_record_words=min(count,expected_count)*11,
                            passed=count==expected_count and not differences and not changes))
    if cursor != len(data):
        raise ValueError('Trailing output')
    return results


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('capture',type=Path)
    parser.add_argument('output',type=Path)
    parser.add_argument('--profile',choices=('debug','release'))
    args = parser.parse_args()
    source,out = private(args.capture),private(args.output)
    if source.stat().st_size > 8*1024*1024:
        raise ValueError('Capture size bound')
    capture = json.loads(source.read_text(encoding='utf-8'))
    if not(capture['completed'] and capture['debugger_exit']==0 and capture['hardware_breakpoints']
           and capture['code_exact'] and capture['session']['ExeSHA256']==REFERENCE and 1<=len(capture['cases'])<=32):
        raise ValueError('Unverified producer capture')
    image,probe = ROOT/'local/compatibility/grid-unpacked.exe', ROOT/'local/behavior/impact-generation-probe.exe'
    if digest(image)!=ANALYSIS or digest(probe)!=PROBE_SHA:
        raise ValueError('Analysis image/probe changed')
    request = pack(capture['cases'])
    out.mkdir(parents=True,exist_ok=False)
    (out/'input.bin').write_bytes(request)
    report = dict(capture_sha256=digest(source),probe_sha256=digest(probe),analysis_sha256=ANALYSIS,
                  scope='Original prefix ends at9EB534, before9D4210,solver,response. Only settings pointer normalized; record2C unwritten excluded.',runs={})
    results = []
    commands=[('original',[str(probe),str(image)]),('repeat',[str(probe),str(image)])]
    if args.profile:
        native=ROOT/f'target/{args.profile}/grid-impact-generation.exe'
        report['native_sha256']=digest(native)
        report['profile']=args.profile
        commands.append(('rust',[str(native)]))
    for label,command in commands:
        target=out/f'{label}.bin'
        run=subprocess.run(command+[str(out/'input.bin'),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=60)
        row=dict(exit=run.returncode,stdout=run.stdout,stderr=run.stderr)
        if run.returncode==0:
            row['cases']=decode(target.read_bytes(),capture['cases'])
            results.append(target.read_bytes())
        report['runs'][label]=row
    # Compare only written record bytes; uninitialized record2C may differ.
    report['passed']=len(results)==len(commands) and all(c['passed'] for r in report['runs'].values() for c in r.get('cases',[]))
    if args.profile and report['passed'] and all(row.get('solver') for row in capture['cases']):
        # Feed Rust-generated records and Rust prefix descriptor into the existing
        # independent solver, using outer captured policy rather than solver inputs.
        from validate_deformation_impact import pack as solver_pack
        rust_data=(out/'rust.bin').read_bytes();cursor=8;solver_rows=[];expected=[]
        for row in capture['cases']:
            count,=struct.unpack_from('<I',rust_data,cursor);cursor+=4
            d=rust_data[cursor:cursor+0xa500];cursor+=0xa500
            r=rust_data[cursor:cursor+count*48];cursor+=count*48
            if row['solver']['gain_word']!='3f800000':raise ValueError('Unexpected production solver gain')
            solver_rows.append(dict(d=d,s=blob(row,'settings_hex',0x1050),g=blob(row,'solver_globals_hex',24),
                                    clip=blob(row,'solver_clip_hex',1)[0],r=r,gain=1.))
            target=bytearray(blob(row['solver'],'descriptor_after_hex',0xa500));target[0xa4f8:0xa4fc]=bytes(4);expected.append(target)
        request=out/'pipeline-input.bin';request.write_bytes(solver_pack(solver_rows))
        native_solver=ROOT/f'target/{args.profile}/grid-deformation-impact.exe';target=out/'pipeline.bin'
        run=subprocess.run([str(native_solver),str(request),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=60)
        pipeline=dict(exit=run.returncode,stdout=run.stdout,stderr=run.stderr,native_sha256=digest(native_solver),
                      scope='Rust-generated records and descriptor into Rust solver; compared with live nested solver return. Excludes9D4210 physics and later response.')
        if run.returncode==0:
            actual=target.read_bytes();wanted=struct.pack('<II',0x314d4947,len(expected))+b''.join(expected)
            if len(actual)!=len(wanted):raise ValueError('Pipeline size mismatch')
            differences=[hex(i) for i in range(8,len(wanted),4) if actual[i:i+4]!=wanted[i:i+4]]
            pipeline.update(compared_descriptor_words=len(expected)*0xa500//4,mismatched_words=len(differences),
                            first_differences=differences[:20],passed=actual==wanted)
        report['pipeline']=pipeline;report['passed'] &= pipeline.get('passed',False)
    (out/'validation.json').write_text(json.dumps(report,indent=2,allow_nan=False),encoding='utf-8')
    print(json.dumps(report,indent=2))
    if not report['passed']:
        raise RuntimeError('Original contact-generation prefix replay failed; inspect retained report')


if __name__=='__main__':
    main()
