"""Compare the isolated original response slice and Rust with actual live returns."""
import argparse
import json
from pathlib import Path
import struct
import subprocess
from capture_reference_spawn import private, REFERENCE, ANALYSIS
from validate_steering_control import digest
from validate_impact_generation import blob

ROOT = Path(__file__).resolve().parents[1]
PROBE_SHA = 'ac471303fe7fe064750e5b5d5e1c02b814dbce59eaa99fc6db176bf090404191'


def from_live(row):
    solver = row.get('solver')
    d = blob(solver, 'descriptor_after_hex', 0xa500) if solver else blob(row, 'descriptor_after_hex', 0xa500)
    linked = blob(row, 'linked_before_hex', 0x424)
    count = row['contact_count']
    globals_ = blob(row, 'producer_globals_hex', 32)
    contacts = []
    for c in row['contacts']:
        b = blob(c, 'before_hex', 0x80)
        contacts.append(dict(enabled=int(b[0x73]!=0), reverse=int(b[0xc]==0 and struct.unpack_from('<I',b,8)[0]==int(row['linked'],16)),
                             baseline=struct.unpack_from('<f',b,0x38)[0], positive=struct.unpack_from('<f',b,0x3c)[0],normal=struct.unpack_from('<4f',b,0x60)))
    records = []
    if solver:
        raw = blob(solver, 'records_hex', solver['record_count']*48)
        for i in range(solver['record_count']):
            r = raw[i*48:(i+1)*48]
            records.append(dict(direction=struct.unpack_from('<4f',r), anchor=struct.unpack_from('<4f',r,16),
                                node=struct.unpack_from('<H',r,32)[0],contact=struct.unpack_from('<H',r,34)[0],weight=struct.unpack_from('<f',r,40)[0]))
    present = int(int(row['response'],16)!=0)
    return dict(nodes=[struct.unpack_from('<4f',d,0x2580+i*80) for i in range(400)],contacts=contacts,records=records,
                present=present,policy=int(globals_[4]!=0),dirty=int(linked[0x390]),scale=struct.unpack('<f',blob(row,'response_scale_hex',4))[0],
                gain=struct.unpack_from('<f',globals_)[0],velocity=struct.unpack_from('<4f',linked,0x360),accumulator=struct.unpack_from('<4f',linked,0x380),
                response=list(struct.unpack('<'+'f'*count,blob(row,'response_before_hex',count*4))) if present else [0.]*count)


def pack(rows):
    data = [struct.pack('<II',0x31525049,len(rows))]
    for r in rows:
        data.append(struct.pack('<6I10f',len(r['nodes']),len(r['contacts']),len(r['records']),r['present'],r['policy'],r['dirty'],r['scale'],r['gain'],*r['velocity'],*r['accumulator']))
        data.extend(struct.pack('<4f',*n) for n in r['nodes'])
        data.extend(struct.pack('<II6f',c['enabled'],c['reverse'],c['baseline'],c['positive'],*c['normal']) for c in r['contacts'])
        data.extend(struct.pack('<8fHHf',*record['direction'],*record['anchor'],record['node'],record['contact'],record['weight']) for record in r['records'])
        data.append(struct.pack('<'+'f'*len(r['response']),*r['response']))
    return b''.join(data)


def live_expected(rows):
    data = [struct.pack('<II',0x31525049,len(rows))]
    for r in rows:
        linked = blob(r,'linked_after_hex',0x424)
        response=blob(r,'response_after_hex',r['contact_count']*4) if int(r['response'],16)!=0 else bytes(r['contact_count']*4)
        data.extend([linked[0x380:0x390],struct.pack('<I',linked[0x390]),response])
    return b''.join(data)


def compare(actual, expected, rows):
    if len(actual)!=len(expected) or actual[:8]!=expected[:8]:
        raise ValueError('Response output framing mismatch')
    differences = [dict(offset=hex(i),actual=actual[i:i+4].hex(),expected=expected[i:i+4].hex()) for i in range(8,len(actual),4) if actual[i:i+4]!=expected[i:i+4]]
    cursor=8; changed=0
    for r in rows:
        changed += actual[cursor:cursor+16]!=struct.pack('<4f',*r['accumulator'])
        cursor += 20+len(r['contacts'])*4
    return dict(compared_words=(len(expected)-8)//4,mismatched_words=len(differences),first_differences=differences[:20],
                accumulator_changed_cases=changed,passed=not differences)


def execute(rows, out, profile, expected=None):
    image, probe = ROOT/'local/compatibility/grid-unpacked.exe', ROOT/'local/behavior/impact-response-probe.exe'
    native=ROOT/f'target/{profile}/grid-impact-response.exe'
    if digest(image)!=ANALYSIS or digest(probe)!=PROBE_SHA:
        raise ValueError('Analysis image/probe identity changed')
    out.mkdir(parents=True,exist_ok=False)
    (out/'input.bin').write_bytes(pack(rows))
    report=dict(scope='Isolated original response slice9EB570..9EBCC5, supplied solved state/records; generation,9D4210,solver explicitly skipped via context continuation.',
                profile=profile,analysis_sha256=ANALYSIS,probe_sha256=digest(probe),native_sha256=digest(native),runs={})
    original=None
    for label,command in [('original',[str(probe),str(image)]),('repeat',[str(probe),str(image)]),('rust',[str(native)])]:
        target=out/f'{label}.bin'
        run=subprocess.run(command+[str(out/'input.bin'),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=60)
        result=dict(exit=run.returncode,stdout=run.stdout,stderr=run.stderr)
        if run.returncode==0:
            actual=target.read_bytes()
            if label=='original':original=actual
            if expected is not None:result['live']=compare(actual,expected,rows)
            if original is not None:result['original']=compare(actual,original,rows)
        report['runs'][label]=result
    report['passed']=all(r['exit']==0 and r.get('original',{}).get('passed',False) and (expected is None or r.get('live',{}).get('passed',False)) for r in report['runs'].values())
    return report


def pipeline(live, out, profile, expected):
    from validate_impact_generation import pack as generation_pack, decode as generation_decode
    from validate_deformation_impact import pack as solver_pack
    result=dict(scope='Rust contact generation -> Rust grid solver -> Rust response; compared with live outer return. Does not implement9D4210 physics or contact detection.',stages={})
    def run(label, request, binary):
        source=out/f'pipeline-{label}-input.bin';target=out/f'pipeline-{label}.bin';source.write_bytes(request)
        process=subprocess.run([str(binary),str(source),str(target)],cwd=ROOT,capture_output=True,text=True,timeout=60)
        result['stages'][label]=dict(exit=process.returncode,stdout=process.stdout,stderr=process.stderr,native_sha256=digest(binary))
        if process.returncode!=0:raise RuntimeError(f'Rust pipeline {label} failed')
        return target.read_bytes()
    generated=run('generation',generation_pack(live),ROOT/f'target/{profile}/grid-impact-generation.exe')
    result['stages']['generation']['cases']=generation_decode(generated,live)
    cursor=8;solver_rows=[];response_rows=[from_live(r) for r in live]
    for row,response in zip(live,response_rows):
        count,=struct.unpack_from('<I',generated,cursor);cursor+=4
        d=generated[cursor:cursor+0xa500];cursor+=0xa500
        records=generated[cursor:cursor+count*48];cursor+=count*48
        if row['solver']['gain_word']!='3f800000':raise ValueError('Unverified solver gain')
        solver_rows.append(dict(d=d,s=blob(row,'settings_hex',0x1050),g=blob(row,'solver_globals_hex',24),clip=blob(row,'solver_clip_hex',1)[0],r=records,gain=1.))
        response['records']=[]
        for i in range(count):
            r=records[i*48:(i+1)*48]
            response['records'].append(dict(direction=struct.unpack_from('<4f',r),anchor=struct.unpack_from('<4f',r,16),node=struct.unpack_from('<H',r,32)[0],contact=struct.unpack_from('<H',r,34)[0],weight=struct.unpack_from('<f',r,40)[0]))
    solved=run('solver',solver_pack(solver_rows),ROOT/f'target/{profile}/grid-deformation-impact.exe')
    if solved[:8]!=struct.pack('<II',0x314d4947,len(live)) or len(solved)!=8+len(live)*0xa500:raise ValueError('Solver pipeline framing')
    descriptor_differences=[]
    for i,(row,response) in enumerate(zip(live,response_rows)):
        d=solved[8+i*0xa500:8+(i+1)*0xa500]
        wanted=bytearray(blob(row,'descriptor_after_hex',0xa500));wanted[0xa4f8:0xa4fc]=bytes(4)
        descriptor_differences.extend(dict(case=i,offset=hex(j)) for j in range(0,0xa500,4) if d[j:j+4]!=wanted[j:j+4])
        response['nodes']=[struct.unpack_from('<4f',d,0x2580+j*80) for j in range(400)]
    result['stages']['solver'].update(compared_descriptor_words=len(live)*0xa500//4,mismatched_words=len(descriptor_differences),first_differences=descriptor_differences[:20])
    actual=run('response',pack(response_rows),ROOT/f'target/{profile}/grid-impact-response.exe')
    result['stages']['response'].update(compare(actual,expected,response_rows))
    result['passed']=all(c['passed'] for c in result['stages']['generation']['cases']) and not descriptor_differences and result['stages']['response']['passed']
    return result


def main():
    parser=argparse.ArgumentParser();parser.add_argument('capture',type=Path);parser.add_argument('output',type=Path);parser.add_argument('--profile',choices=('debug','release'),required=True);args=parser.parse_args()
    source,out=private(args.capture),private(args.output)
    if source.stat().st_size>8*1024*1024:raise ValueError('Capture size bound')
    capture=json.loads(source.read_text(encoding='utf-8'))
    if not(capture['completed'] and capture['debugger_exit']==0 and capture['hardware_breakpoints'] and capture['code_exact'] and capture['solver_code_exact']
           and capture['session']['ExeSHA256']==REFERENCE and 1<=len(capture['cases'])<=32):raise ValueError('Unverified capture')
    rows=[from_live(r) for r in capture['cases']]
    report=execute(rows,out,args.profile,live_expected(capture['cases']));report['capture_sha256']=digest(source)
    if report['passed'] and all(r.get('solver') for r in capture['cases']):
        report['pipeline']=pipeline(capture['cases'],out,args.profile,live_expected(capture['cases']))
        report['passed'] &= report['pipeline']['passed']
    (out/'validation.json').write_text(json.dumps(report,indent=2,allow_nan=False),encoding='utf-8');print(json.dumps(report,indent=2))
    if not report['passed']:raise RuntimeError('Response comparison failed; retained report is diagnostic evidence')


if __name__=='__main__':main()
